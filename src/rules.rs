//! Pattern matching rewrite rules for date disambiguation
//!
//! These rules handle edge cases that consensus voting alone can't solve,
//! such as single-date inference or duplicate token resolution.

use crate::constraints::TokenType;

/// Apply rewrite rules to resolve remaining ambiguities
pub fn apply_rules(tokens: &mut [TokenType]) {
    // Apply rules in order of specificity (most specific first)
    rule_month_name_adjacency(tokens);
    rule_duplicate_day_or_month(tokens);
    rule_month_month_sequence(tokens);
    rule_year_position_hints(tokens);
    rule_time_sequence(tokens);
}

/// Rule: If DayOrMonth appears twice, first is Day, second is Month
///
/// Pattern: DayOrMonth sep DayOrMonth sep Year
/// Example: "01/02/2025" → Day/Month/Year (with dayfirst=true already applied)
fn rule_duplicate_day_or_month(tokens: &mut [TokenType]) {
    let ambiguous_positions: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| **t == TokenType::DayOrMonth)
        .map(|(i, _)| i)
        .collect();

    // If exactly two ambiguous positions, resolve them
    if ambiguous_positions.len() == 2 {
        // This should already be handled by consensus with prefer_dayfirst
        // But as a fallback, first becomes Day, second becomes Month
        tokens[ambiguous_positions[0]] = TokenType::Day;
        tokens[ambiguous_positions[1]] = TokenType::Month;
    }
}

/// Rule: If we see Month followed by Month (after initial resolution),
/// the second one is likely Day
///
/// Pattern: Month sep Month → Month sep Day
/// Example: Wrongly inferred "03/15/2025" as Month/Month/Year
fn rule_month_month_sequence(tokens: &mut [TokenType]) {
    for i in 0..tokens.len() {
        if tokens[i] == TokenType::Month {
            // Look for another Month after separators
            if let Some(next) = tokens[(i + 1)..].iter_mut().find(|t| !matches!(t, TokenType::Separator(_))) {
                if *next == TokenType::Month {
                    // Two months in a row - second one should be Day
                    *next = TokenType::Day;
                }
            }
        }
    }
}

/// Rule: Year position hints
///
/// - If Year4 is first, it's likely ISO format (YYYY-MM-DD)
/// - If Year4 is last, Day/Month come before it
fn rule_year_position_hints(tokens: &mut [TokenType]) {
    // Find Year4 position
    let year_pos = tokens.iter().position(|t| *t == TokenType::Year4);

    if let Some(pos) = year_pos {
        // Find positions of DayOrMonth tokens
        let ambiguous: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == TokenType::DayOrMonth)
            .map(|(i, _)| i)
            .collect();

        if ambiguous.len() == 2 && pos == 0 {
            // Year first (ISO format): YYYY-MM-DD
            // Second ambiguous is Month, third is Day
            tokens[ambiguous[0]] = TokenType::Month;
            tokens[ambiguous[1]] = TokenType::Day;
            // Year last is handled by prefer_dayfirst in consensus
        }
    }
}

/// Rule: Time sequence detection
///
/// Pattern: number:number:number → Hour:Minute:Second
/// Pattern: number:number → Hour:Minute
fn rule_time_sequence(tokens: &mut [TokenType]) {
    let mut i = 0;
    while i + 2 < tokens.len() {
        // Look for X:Y pattern
        if matches!(tokens[i + 1], TokenType::Separator(':')) {
            // Check if both sides could be time components
            let left_could_be_time = matches!(
                tokens[i],
                TokenType::Hour24 | TokenType::Hour12 | TokenType::DayOrMonth | TokenType::Unknown
            );
            let right_could_be_time = matches!(
                tokens[i + 2],
                TokenType::Minute | TokenType::Second | TokenType::DayOrMonth | TokenType::Unknown
            );

            if left_could_be_time && right_could_be_time {
                // This is likely a time sequence
                tokens[i] = TokenType::Hour24;
                tokens[i + 2] = TokenType::Minute;

                // Check for seconds (X:Y:Z)
                if i + 4 < tokens.len()
                    && matches!(tokens[i + 3], TokenType::Separator(':'))
                {
                    tokens[i + 4] = TokenType::Second;
                    i += 4;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// Rule: If Month name is present, adjacent number is Day (not Month)
///
/// Pattern: MonthName number → MonthName Day
/// Pattern: number MonthName → Day MonthName
pub fn rule_month_name_adjacency(tokens: &mut [TokenType]) {
    // Find positions of month names first (to avoid borrow issues)
    let month_positions: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, TokenType::MonthName | TokenType::MonthNameShort))
        .map(|(i, _)| i)
        .collect();

    for i in month_positions {
        // Check left neighbor (skip separators)
        if let Some(left) = tokens[..i].iter_mut().rev().find(|t| !matches!(t, TokenType::Separator(_))) {
            if *left == TokenType::DayOrMonth {
                *left = TokenType::Day;
            }
        }

        // Check right neighbor (skip separators)
        if let Some(right) = tokens[(i + 1)..].iter_mut().find(|t| !matches!(t, TokenType::Separator(_))) {
            if *right == TokenType::DayOrMonth {
                *right = TokenType::Day;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_day_or_month() {
        let mut tokens = vec![
            TokenType::DayOrMonth,
            TokenType::Separator('/'),
            TokenType::DayOrMonth,
            TokenType::Separator('/'),
            TokenType::Year4,
        ];
        rule_duplicate_day_or_month(&mut tokens);
        assert_eq!(tokens[0], TokenType::Day);
        assert_eq!(tokens[2], TokenType::Month);
    }

    #[test]
    fn test_month_month_sequence() {
        let mut tokens = vec![
            TokenType::Month,
            TokenType::Separator('/'),
            TokenType::Month,
            TokenType::Separator('/'),
            TokenType::Year4,
        ];
        rule_month_month_sequence(&mut tokens);
        assert_eq!(tokens[0], TokenType::Month);
        assert_eq!(tokens[2], TokenType::Day);
    }

    #[test]
    fn test_iso_format_year_first() {
        let mut tokens = vec![
            TokenType::Year4,
            TokenType::Separator('-'),
            TokenType::DayOrMonth,
            TokenType::Separator('-'),
            TokenType::DayOrMonth,
        ];
        rule_year_position_hints(&mut tokens);
        assert_eq!(tokens[2], TokenType::Month);
        assert_eq!(tokens[4], TokenType::Day);
    }

    #[test]
    fn test_time_sequence() {
        let mut tokens = vec![
            TokenType::Unknown,
            TokenType::Separator(':'),
            TokenType::Unknown,
            TokenType::Separator(':'),
            TokenType::Unknown,
        ];
        rule_time_sequence(&mut tokens);
        assert_eq!(tokens[0], TokenType::Hour24);
        assert_eq!(tokens[2], TokenType::Minute);
        assert_eq!(tokens[4], TokenType::Second);
    }

    #[test]
    fn test_month_name_adjacency() {
        let mut tokens = vec![
            TokenType::DayOrMonth,
            TokenType::Separator(' '),
            TokenType::MonthNameShort,
            TokenType::Separator(' '),
            TokenType::Year4,
        ];
        rule_month_name_adjacency(&mut tokens);
        assert_eq!(tokens[0], TokenType::Day);
    }
}
