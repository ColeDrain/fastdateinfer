//! Consensus-based resolution of ambiguous date tokens

use crate::constraints::TokenType;
use crate::error::{DateInferError, Result};
use crate::tokenizer::Token;
use crate::InferOptions;
use rustc_hash::FxHashMap;

/// Resolve token types across all examples using consensus voting
pub fn resolve_consensus(
    tokenized_dates: &[Vec<Token>],
    options: &InferOptions,
) -> Result<(Vec<TokenType>, f64)> {
    if tokenized_dates.is_empty() {
        return Err(DateInferError::EmptyInput);
    }

    let num_positions = tokenized_dates[0].len();
    let num_examples = tokenized_dates.len();

    // Phase 2: Collect constraints from all examples for each position
    let mut position_votes: Vec<FxHashMap<TokenType, usize>> = vec![FxHashMap::default(); num_positions];
    let mut position_constraints: Vec<PositionConstraint> = vec![PositionConstraint::default(); num_positions];

    for tokens in tokenized_dates {
        for (pos, token) in tokens.iter().enumerate() {
            // Track if any example at this position MUST be a specific type
            if token.must_be_day() {
                position_constraints[pos].must_be_day = true;
            }

            // Vote for each possible type
            for token_type in &token.possible_types {
                *position_votes[pos].entry(*token_type).or_insert(0) += 1;
            }

            // Track the separator character if present
            if let Some(TokenType::Separator(c)) = token.possible_types.iter().find(|t| matches!(t, TokenType::Separator(_))) {
                position_constraints[pos].separator = Some(*c);
            }
        }
    }

    // Detect time sequences: positions connected by : or . separators
    // Time patterns: HH:MM:SS or HH.MM.SS (must be connected sequence)
    let mut is_time_position: Vec<bool> = vec![false; num_positions];

    // Find sequences of positions connected by : or .
    let mut i = 0;
    while i < num_positions {
        // Check if this starts a time sequence (X:Y or X.Y)
        if i + 2 < num_positions {
            let sep = position_constraints.get(i + 1).and_then(|c| c.separator);
            if sep == Some(':') || sep == Some('.') {
                // Found potential time start. Check if it's actually time by looking for
                // consistent separators (: or .) in a sequence
                let mut time_positions = vec![i, i + 2];
                let first_sep = sep.unwrap();

                // Look for more time components (X:Y:Z or X.Y.Z)
                let mut j = i + 2;
                while j + 2 < num_positions {
                    let next_sep = position_constraints.get(j + 1).and_then(|c| c.separator);
                    if next_sep == Some(first_sep) {
                        time_positions.push(j + 2);
                        j += 2;
                    } else {
                        break;
                    }
                }

                // Only mark as time if:
                // 1. Using colon (always time), OR
                // 2. Using dot AND there's a space before (date/time boundary)
                let is_after_space = i > 0 && (0..i).any(|p| {
                    position_constraints.get(p).and_then(|c| c.separator) == Some(' ')
                });
                let is_after_t = i > 0 && position_constraints.get(i - 1).and_then(|c| c.separator) == Some('T');

                if first_sep == ':' || is_after_space || is_after_t {
                    for &pos in &time_positions {
                        is_time_position[pos] = true;
                    }
                    i = *time_positions.last().unwrap() + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    // Detect likely Year2 position (last DATE numeric position, not time)
    let mut likely_year2_pos: Option<usize> = None;

    // Find all DATE numeric positions (non-separator, non-text, non-time)
    let numeric_positions: Vec<usize> = (0..num_positions)
        .filter(|&pos| {
            position_constraints[pos].separator.is_none()
                && !is_time_position[pos]
                && !position_votes[pos].contains_key(&TokenType::MonthName)
                && !position_votes[pos].contains_key(&TokenType::MonthNameShort)
                && !position_votes[pos].contains_key(&TokenType::WeekdayName)
                && !position_votes[pos].contains_key(&TokenType::WeekdayShort)
                && !position_votes[pos].contains_key(&TokenType::TzName)
                && !position_votes[pos].contains_key(&TokenType::TzZ)
                && !position_votes[pos].contains_key(&TokenType::TzOffset)
                && !position_votes[pos].contains_key(&TokenType::AmPm)
        })
        .collect();

    // Check if there's a month name
    let has_month_name = (0..num_positions).any(|p| {
        position_votes[p].contains_key(&TokenType::MonthName)
            || position_votes[p].contains_key(&TokenType::MonthNameShort)
    });

    // Check if Year4 exists anywhere (if so, don't use Year2)
    let has_year4 = (0..num_positions).any(|p| {
        position_votes[p].contains_key(&TokenType::Year4)
    });

    // Find the last position that could be Year2
    // We need at least 3 date components (day, month, year) to have a Year2
    // With month name: need at least 2 numeric positions (day + year)
    // Without month name: need at least 3 numeric positions (day + month + year)
    let min_numeric_for_year = if has_month_name { 2 } else { 3 };

    if let Some(&last_pos) = numeric_positions.last() {
        // Set Year2 if: has Year2 votes, enough numeric positions, and no Year4 elsewhere
        if position_votes[last_pos].contains_key(&TokenType::Year2)
            && numeric_positions.len() >= min_numeric_for_year
            && !has_year4
        {
            likely_year2_pos = Some(last_pos);
        }
    }

    // Phase 3: Determine resolved type for each position
    let mut resolved: Vec<TokenType> = Vec::with_capacity(num_positions);
    let mut total_confidence: f64 = 0.0;
    let mut confidence_count = 0;

    // Track which positions have been assigned Day and Month
    let mut day_assigned: Option<usize> = None;
    let mut month_assigned: Option<usize> = None;

    // Track time sequence state
    let mut time_component_index = 0; // 0=Hour, 1=Minute, 2=Second

    // First pass: resolve unambiguous positions
    for pos in 0..num_positions {
        let votes = &position_votes[pos];
        let constraint = &position_constraints[pos];

        // Handle separators
        if let Some(c) = constraint.separator {
            resolved.push(TokenType::Separator(c));
            continue;
        }

        // Handle time positions (detected by colon/dot context)
        if is_time_position[pos] {
            let time_type = match time_component_index {
                0 => TokenType::Hour24,
                1 => TokenType::Minute,
                _ => TokenType::Second,
            };
            resolved.push(time_type);
            time_component_index += 1;
            continue;
        }

        // Handle Year2 position (detected as last position after month name)
        if likely_year2_pos == Some(pos) {
            resolved.push(TokenType::Year2);
            continue;
        }

        // Handle positions that MUST be a specific type
        if constraint.must_be_day {
            resolved.push(TokenType::Day);
            day_assigned = Some(pos);
            continue;
        }

        // Check for unambiguous text tokens (month names, weekday names, etc.)
        if votes.contains_key(&TokenType::MonthName) || votes.contains_key(&TokenType::MonthNameShort) {
            let month_type = if votes.contains_key(&TokenType::MonthName) {
                TokenType::MonthName
            } else {
                TokenType::MonthNameShort
            };
            resolved.push(month_type);
            month_assigned = Some(pos);
            continue;
        }
        if votes.contains_key(&TokenType::WeekdayName) {
            resolved.push(TokenType::WeekdayName);
            continue;
        }
        if votes.contains_key(&TokenType::WeekdayShort) {
            resolved.push(TokenType::WeekdayShort);
            continue;
        }
        if votes.contains_key(&TokenType::TzName) {
            resolved.push(TokenType::TzName);
            continue;
        }
        if votes.contains_key(&TokenType::TzZ) {
            resolved.push(TokenType::TzZ);
            continue;
        }
        if votes.contains_key(&TokenType::TzOffset) {
            resolved.push(TokenType::TzOffset);
            continue;
        }
        if votes.contains_key(&TokenType::AmPm) {
            resolved.push(TokenType::AmPm);
            continue;
        }

        // Check for year
        if votes.contains_key(&TokenType::Year4) {
            resolved.push(TokenType::Year4);
            continue;
        }
        if votes.contains_key(&TokenType::Year2) && !votes.contains_key(&TokenType::DayOrMonth) {
            resolved.push(TokenType::Year2);
            continue;
        }

        // Mark as pending for second pass
        resolved.push(TokenType::Unknown);
    }

    // Second pass: resolve ambiguous positions using context and preferences
    for pos in 0..num_positions {
        if resolved[pos] != TokenType::Unknown {
            continue;
        }

        let votes = &position_votes[pos];

        // If this position has DayOrMonth votes
        if votes.contains_key(&TokenType::DayOrMonth) || votes.contains_key(&TokenType::Day) {
            // If we already assigned a month elsewhere, this must be day
            if month_assigned.is_some() && day_assigned.is_none() {
                resolved[pos] = TokenType::Day;
                day_assigned = Some(pos);
                continue;
            }

            // If we already assigned a day elsewhere, this must be month
            if day_assigned.is_some() && month_assigned.is_none() {
                resolved[pos] = TokenType::Month;
                month_assigned = Some(pos);
                continue;
            }

            // Neither assigned yet - use preference
            if day_assigned.is_none() && month_assigned.is_none() {
                // Find the other ambiguous position
                let other_ambiguous: Vec<usize> = (0..num_positions)
                    .filter(|&p| p != pos && resolved[p] == TokenType::Unknown)
                    .filter(|&p| {
                        position_votes[p].contains_key(&TokenType::DayOrMonth)
                            || position_votes[p].contains_key(&TokenType::Day)
                    })
                    .collect();

                if options.prefer_dayfirst {
                    // First ambiguous position is day
                    resolved[pos] = TokenType::Day;
                    day_assigned = Some(pos);
                    for &other in &other_ambiguous {
                        if resolved[other] == TokenType::Unknown {
                            resolved[other] = TokenType::Month;
                            month_assigned = Some(other);
                            break;
                        }
                    }
                } else {
                    // First ambiguous position is month
                    resolved[pos] = TokenType::Month;
                    month_assigned = Some(pos);
                    for &other in &other_ambiguous {
                        if resolved[other] == TokenType::Unknown {
                            resolved[other] = TokenType::Day;
                            day_assigned = Some(other);
                            break;
                        }
                    }
                }
                continue;
            }
        }

        // Check for time components
        if votes.contains_key(&TokenType::Hour24) {
            resolved[pos] = TokenType::Hour24;
            continue;
        }
        if votes.contains_key(&TokenType::Minute) {
            resolved[pos] = TokenType::Minute;
            continue;
        }
        if votes.contains_key(&TokenType::Second) {
            resolved[pos] = TokenType::Second;
            continue;
        }

        // Fallback
        resolved[pos] = TokenType::Unknown;
    }

    // Calculate confidence
    for pos in 0..num_positions {
        if matches!(resolved[pos], TokenType::Separator(_) | TokenType::Unknown) {
            continue;
        }

        let votes = &position_votes[pos];
        let resolved_type = &resolved[pos];

        // Count how many examples support this resolution
        let supporting = votes.get(resolved_type).copied().unwrap_or(0);

        // For DayOrMonth resolved to Day or Month, also count DayOrMonth votes
        // but cap at num_examples to avoid double-counting
        let supporting = if *resolved_type == TokenType::Day || *resolved_type == TokenType::Month {
            let total = supporting + votes.get(&TokenType::DayOrMonth).copied().unwrap_or(0);
            total.min(num_examples) // Cap to avoid confidence > 1.0
        } else {
            supporting
        };

        let position_confidence = supporting as f64 / num_examples as f64;
        total_confidence += position_confidence;
        confidence_count += 1;
    }

    let overall_confidence = if confidence_count > 0 {
        total_confidence / confidence_count as f64
    } else {
        0.0
    };

    Ok((resolved, overall_confidence))
}

#[derive(Debug, Default, Clone)]
struct PositionConstraint {
    must_be_day: bool,
    separator: Option<char>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::tokenize;

    #[test]
    fn test_consensus_unambiguous() {
        let dates: Vec<Vec<Token>> = vec![
            tokenize("15/03/2025").unwrap(),
            tokenize("20/04/2025").unwrap(),
        ];
        let options = InferOptions::default();
        let (resolved, confidence) = resolve_consensus(&dates, &options).unwrap();

        assert_eq!(resolved[0], TokenType::Day);
        assert_eq!(resolved[2], TokenType::Month);
        assert_eq!(resolved[4], TokenType::Year4);
        assert!(confidence > 0.9);
    }

    #[test]
    fn test_consensus_with_ambiguous() {
        // First date is ambiguous, second proves DD/MM
        let dates: Vec<Vec<Token>> = vec![
            tokenize("01/02/2025").unwrap(),
            tokenize("15/03/2025").unwrap(),
        ];
        let options = InferOptions::default();
        let (resolved, _) = resolve_consensus(&dates, &options).unwrap();

        assert_eq!(resolved[0], TokenType::Day);
        assert_eq!(resolved[2], TokenType::Month);
    }

    #[test]
    fn test_consensus_all_ambiguous_dayfirst() {
        let dates: Vec<Vec<Token>> = vec![
            tokenize("01/02/2025").unwrap(),
            tokenize("03/04/2025").unwrap(),
        ];
        let options = InferOptions {
            prefer_dayfirst: true,
            ..Default::default()
        };
        let (resolved, _) = resolve_consensus(&dates, &options).unwrap();

        assert_eq!(resolved[0], TokenType::Day);
        assert_eq!(resolved[2], TokenType::Month);
    }

    #[test]
    fn test_consensus_all_ambiguous_monthfirst() {
        let dates: Vec<Vec<Token>> = vec![
            tokenize("01/02/2025").unwrap(),
            tokenize("03/04/2025").unwrap(),
        ];
        let options = InferOptions {
            prefer_dayfirst: false,
            ..Default::default()
        };
        let (resolved, _) = resolve_consensus(&dates, &options).unwrap();

        assert_eq!(resolved[0], TokenType::Month);
        assert_eq!(resolved[2], TokenType::Day);
    }

    #[test]
    fn test_consensus_with_month_name() {
        let dates: Vec<Vec<Token>> = vec![
            tokenize("15 Jan 2025").unwrap(),
            tokenize("20 Mar 2025").unwrap(),
        ];
        let options = InferOptions::default();
        let (resolved, _) = resolve_consensus(&dates, &options).unwrap();

        assert_eq!(resolved[0], TokenType::Day);
        assert_eq!(resolved[2], TokenType::MonthNameShort);
        assert_eq!(resolved[4], TokenType::Year4);
    }
}
