//! # fastdateinfer
//!
//! Fast, consensus-based date format inference.
//!
//! Unlike per-element parsers (pandas, polars, dateutil), this library analyzes
//! ALL examples together to resolve ambiguous dates. If your dataset contains
//! `15/03/2025` (unambiguous: day=15), it can infer that `01/02/2025` uses
//! DD/MM/YYYY format.
//!
//! ## Example
//!
//! ```
//! use fastdateinfer::infer;
//!
//! let dates = vec!["01/02/2025", "15/03/2025", "20/04/2025"];
//! let result = infer(&dates).unwrap();
//!
//! assert_eq!(result.format, "%d/%m/%Y");
//! assert!(result.confidence > 0.9);
//! ```

mod constraints;
mod consensus;
mod error;
mod format;
mod prescan;
mod rules;
mod tokenizer;

pub use constraints::TokenType;
pub use error::{DateInferError, Result};
pub use tokenizer::Token;

use consensus::resolve_consensus;
use format::to_strptime;
use rules::apply_rules;
use rustc_hash::FxHashMap;
use tokenizer::tokenize;

/// Configuration options for inference
#[derive(Debug, Clone)]
pub struct InferOptions {
    /// Prefer day-first format for ambiguous dates (default: true)
    pub prefer_dayfirst: bool,
    /// Minimum confidence threshold (default: 0.0)
    pub min_confidence: f64,
    /// Fail if any example doesn't match the inferred format (default: false)
    pub strict: bool,
}

impl Default for InferOptions {
    fn default() -> Self {
        Self {
            prefer_dayfirst: true,
            min_confidence: 0.0,
            strict: false,
        }
    }
}

/// Result of date format inference
#[derive(Debug, Clone)]
pub struct InferResult {
    /// The inferred strptime format string
    pub format: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Resolved token types for each position
    pub token_types: Vec<TokenType>,
}

/// Infer date format from a list of example date strings.
///
/// Analyzes all examples together using consensus-based voting to resolve
/// ambiguous dates like `01/02/2025` (could be Jan 2 or Feb 1).
///
/// # Arguments
///
/// * `dates` - Slice of date strings to analyze
///
/// # Returns
///
/// * `Ok(InferResult)` - The inferred format with confidence score
/// * `Err(DateInferError)` - If inference fails
///
/// # Example
///
/// ```
/// use fastdateinfer::infer;
///
/// let dates = vec!["15/03/2025", "01/02/2025"];
/// let result = infer(&dates).unwrap();
/// assert_eq!(result.format, "%d/%m/%Y");
/// ```
pub fn infer<S: AsRef<str>>(dates: &[S]) -> Result<InferResult> {
    infer_with_options(dates, &InferOptions::default())
}

/// Infer date format with custom options.
///
/// # Arguments
///
/// * `dates` - Slice of date strings to analyze
/// * `options` - Configuration options
///
/// # Example
///
/// ```
/// use fastdateinfer::{infer_with_options, InferOptions};
///
/// let dates = vec!["01/02/2025", "03/04/2025"];
/// let options = InferOptions {
///     prefer_dayfirst: false, // Prefer MM/DD
///     ..Default::default()
/// };
/// let result = infer_with_options(&dates, &options).unwrap();
/// assert_eq!(result.format, "%m/%d/%Y");
/// ```
pub fn infer_with_options<S: AsRef<str>>(dates: &[S], options: &InferOptions) -> Result<InferResult> {
    if dates.is_empty() {
        return Err(DateInferError::EmptyInput);
    }

    // Phase 1: Tokenize dates (sample for large inputs - consensus converges quickly)
    const MAX_SAMPLE: usize = 1000;
    let sample: Vec<&S> = if dates.len() <= MAX_SAMPLE {
        dates.iter().collect()
    } else {
        // Take evenly distributed sample: first, last, and evenly spaced middle
        let step = dates.len() / MAX_SAMPLE;
        let mut sample: Vec<&S> = dates.iter().step_by(step).take(MAX_SAMPLE).collect();

        // Pre-scan ALL dates for disambiguating values (value > 12) that the
        // step_by sample may have missed. At most 2 replacements in the sample.
        let disambig = prescan::find_disambiguating_indices(dates);
        let sample_len = sample.len();
        for (pos, opt_idx) in disambig.iter().enumerate() {
            if let Some(idx) = opt_idx {
                // Only inject if this date isn't already in the sample
                let already_sampled = *idx % step == 0 && *idx / step < sample_len;
                if !already_sampled && sample_len > pos {
                    sample[sample_len - 1 - pos] = &dates[*idx];
                }
            }
        }

        sample
    };

    let tokenized_results: Vec<_> = sample
        .iter()
        .map(|d| tokenize(d.as_ref()).ok())
        .collect();

    // Count token lengths to find majority
    let mut length_counts: FxHashMap<usize, usize> = FxHashMap::default();
    for tokens in &tokenized_results {
        if let Some(t) = tokens {
            *length_counts.entry(t.len()).or_insert(0) += 1;
        }
    }

    let sample_count = tokenized_results.len();
    let (majority_len, majority_count) = length_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .unwrap_or((0, 0));

    // Require >50% of tokenizable dates to have the majority length
    if majority_count * 2 <= sample_count {
        return Err(DateInferError::InconsistentFormats);
    }

    let filter_ratio = majority_count as f64 / sample_count as f64;

    // Filter to only majority-length tokenized dates
    let tokenized: Vec<Vec<Token>> = tokenized_results
        .into_iter()
        .filter_map(|t| t.filter(|tokens| tokens.len() == majority_len))
        .collect();

    // Phase 2-3: Resolve consensus with constraints
    let (mut resolved_types, raw_confidence) = resolve_consensus(&tokenized, options)?;
    let confidence = raw_confidence * filter_ratio;

    // Phase 4: Apply rewrite rules for remaining ambiguities
    apply_rules(&mut resolved_types);

    // Check minimum confidence
    if confidence < options.min_confidence {
        return Err(DateInferError::LowConfidence {
            got: confidence,
            required: options.min_confidence,
        });
    }

    // Phase 5: Generate strptime format
    let format = to_strptime(&tokenized[0], &resolved_types);

    // Phase 6: Strict validation (if enabled)
    if options.strict {
        let mut failed_count = 0;
        for date in dates {
            if let Ok(tokens) = tokenize(date.as_ref()) {
                if !is_compatible(&tokens, &resolved_types) {
                    failed_count += 1;
                }
            } else {
                failed_count += 1;
            }
        }
        if failed_count > 0 {
            return Err(DateInferError::StrictValidationFailed {
                failed_count,
                total_count: dates.len(),
            });
        }
    }

    Ok(InferResult {
        format,
        confidence,
        token_types: resolved_types,
    })
}

/// Check if a token is compatible with a resolved type.
/// Handles Day/DayOrMonth equivalence: a token that could be DayOrMonth
/// is compatible with Day or Month resolved types.
fn is_token_compatible(token: &Token, resolved: &TokenType) -> bool {
    if token.possible_types.contains(resolved) {
        return true;
    }
    // DayOrMonth equivalence: if the resolved type is Day or Month,
    // a token with DayOrMonth in its possible types is compatible
    match resolved {
        TokenType::Day | TokenType::Month | TokenType::DayOrMonth => {
            token.possible_types.iter().any(|t| matches!(t,
                TokenType::Day | TokenType::Month | TokenType::DayOrMonth
            ))
        }
        _ => false,
    }
}

/// Check if a tokenized date is compatible with the resolved types.
fn is_compatible(tokens: &[Token], resolved_types: &[TokenType]) -> bool {
    if tokens.len() != resolved_types.len() {
        return false;
    }
    tokens.iter().zip(resolved_types.iter()).all(|(token, resolved)| {
        is_token_compatible(token, resolved)
    })
}

#[cfg(feature = "python")]
mod python;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unambiguous_dmy() {
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_consensus_resolves_ambiguous() {
        // 01/02 is ambiguous, but 15/03 proves it's DD/MM
        let dates = vec!["01/02/2025", "15/03/2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_iso_format() {
        let dates = vec!["2025-01-15", "2025-03-20"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%Y-%m-%d");
    }

    #[test]
    fn test_month_name() {
        let dates = vec!["15 Jan 2025", "20 Mar 2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d %b %Y");
    }

    #[test]
    fn test_empty_input() {
        let dates: Vec<&str> = vec![];
        let result = infer(&dates);
        assert!(matches!(result, Err(DateInferError::EmptyInput)));
    }

    #[test]
    fn test_prefer_dayfirst_false() {
        // All ambiguous, rely on preference
        let dates = vec!["01/02/2025", "03/04/2025"];
        let options = InferOptions {
            prefer_dayfirst: false,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options).unwrap();
        assert_eq!(result.format, "%m/%d/%Y");
    }

    #[test]
    fn test_single_date_ambiguous() {
        // Single ambiguous date - uses rules + preference
        let dates = vec!["01/02/2025"];
        let result = infer(&dates).unwrap();
        // With dayfirst=true (default), should be DD/MM/YYYY
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_single_date_unambiguous() {
        // Single date with day > 12
        let dates = vec!["25/12/2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_datetime_with_time() {
        let dates = vec!["2025-01-15 10:30:00", "2025-03-20 14:45:30"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%Y-%m-%d %H:%M:%S");
    }

    #[test]
    fn test_full_month_name() {
        let dates = vec!["15 January 2025", "20 March 2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d %B %Y");
    }

    #[test]
    fn test_american_format() {
        // American format with unambiguous month (13 can't be month)
        let dates = vec!["12/13/2025", "01/25/2025"];
        let result = infer(&dates).unwrap();
        // 13 and 25 can't be months, so format must be MM/DD/YYYY
        assert_eq!(result.format, "%m/%d/%Y");
    }

    #[test]
    fn test_iso_with_t_separator() {
        let dates = vec!["2025-01-15T10:30:00", "2025-03-20T14:45:30"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%Y-%m-%dT%H:%M:%S");
    }

    // =========================================
    // Real-world format tests
    // =========================================

    #[test]
    fn test_dd_mmm_yyyy_dash() {
        
        let dates = vec!["26-May-2023", "01-Jul-2024", "02-Aug-2024"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d-%b-%Y");
    }

    #[test]
    fn test_dd_mmm_yy_uppercase() {
        // Abbreviated month, 2-digit year
        let dates = vec!["29-AUG-24", "05-SEP-24", "06-SEP-24"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d-%b-%y");
    }

    #[test]
    fn test_dd_mm_yy_slash() {
        // 2-digit year, slash separator
        let dates = vec!["10/06/24", "11/06/24", "12/06/24"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%y");
    }

    #[test]
    fn test_dd_mm_yy_with_dot_time() {
        // Date with dot-separated time
        let dates = vec!["10/06/24 12.25.10", "10/06/24 14.30.14", "12/06/24 19.55.14"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%y %H.%M.%S");
    }

    #[test]
    fn test_mon_dd_comma_yyyy() {
        // Month-first with comma: Mon DD, YYYY
        let dates = vec!["Dec 17, 2024", "Dec 18, 2024", "Jan 24, 2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%b %d, %Y");
    }

    #[test]
    fn test_non_padded_mdy() {
        // Non-zero-padded M/D/YYYY
        let dates = vec!["5/1/2024", "5/2/2024", "12/15/2024"];
        let result = infer(&dates).unwrap();
        // Should detect as MM/DD/YYYY because 15 > 12
        assert_eq!(result.format, "%m/%d/%Y");
    }

    #[test]
    fn test_month_year_only() {
        // Full month name, comma, year (no day)
        let dates = vec!["December, 2024", "January, 2025", "February, 2025"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%B, %Y");
    }

    #[test]
    fn test_dd_mmm_no_year() {
        // Day/abbreviated month, no year
        let dates = vec!["31/OCT", "01/NOV", "04/NOV"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%b");
    }

    // =========================================
    // InconsistentFormats tolerance tests
    // =========================================

    #[test]
    fn test_trailing_space_tolerated() {
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025 "];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_one_malformed_row_tolerated() {
        // 4 good dates + 1 "N/A" → succeeds with reduced confidence
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025", "01/01/2025", "N/A"];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_empty_string_tolerated() {
        // good dates + one "" → succeeds
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025", ""];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_inconsistent_formats_when_no_majority() {
        // Truly mixed token counts with no >50% majority → still errors
        let dates = vec!["15/03/2025", "2025-01-15T10:30:00", "Jan 2025"];
        let result = infer(&dates);
        assert!(matches!(result, Err(DateInferError::InconsistentFormats)));
    }

    #[test]
    fn test_confidence_reflects_filtered_proportion() {
        // 4 good dates + 1 bad → confidence reduced proportionally
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025", "01/06/2025", "N/A"];
        let result = infer(&dates).unwrap();
        // filter_ratio = 4/5 = 0.8, so confidence should be at most 0.8
        assert!(result.confidence <= 0.8 + f64::EPSILON);
        assert!(result.confidence > 0.0);
    }

    // =========================================
    // Strict mode tests
    // =========================================

    #[test]
    fn test_strict_passes_when_all_match() {
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025"];
        let options = InferOptions {
            strict: true,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_strict_fails_with_incompatible_token_structure() {
        // One date has different token count
        let dates = vec!["15/03/2025", "20/04/2025", "2025-01-15T10:30:00"];
        let options = InferOptions {
            strict: true,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_strict_fails_with_incompatible_token_types() {
        // Same token count but incompatible values (text where number expected)
        let dates = vec!["15/03/2025", "20/04/2025", "AB/CD/EFGH"];
        let options = InferOptions {
            strict: true,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_strict_false_ignores_issues() {
        // strict=false tolerates bad dates in the dataset (as long as majority is consistent)
        let dates = vec!["15/03/2025", "20/04/2025", "25/12/2025"];
        let options = InferOptions {
            strict: false,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_strict_validates_all_dates_not_just_sample() {
        // Dataset > 1000, bad date outside sample range, strict still catches it
        let mut dates: Vec<String> = (0..1100)
            .map(|i| format!("{:02}/03/2025", (i % 28) + 1))
            .collect();
        // Add an incompatible date at the end
        dates.push("NOT-A-DATE".to_string());
        let options = InferOptions {
            strict: true,
            ..Default::default()
        };
        let result = infer_with_options(&dates, &options);
        assert!(result.is_err());
    }

    // =========================================
    // Weekday and timezone tests
    // =========================================

    #[test]
    fn test_weekday_month_day_time_tz_year() {
        // hidateinfer's flagship example
        let dates = vec![
            "Mon Jan 13 09:52:52 MST 2014",
            "Tue Jan 21 15:30:00 EST 2014",
        ];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%a %b %d %H:%M:%S %Z %Y");
    }

    #[test]
    fn test_weekday_only_variation() {
        // Weekday varies, rest is consistent
        let dates = vec![
            "Mon 13 Jan 2014",
            "Tue 21 Jan 2014",
            "Wed 15 Feb 2014",
        ];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%a %d %b %Y");
    }

    #[test]
    fn test_timezone_variation() {
        // Different timezone abbreviations
        let dates = vec![
            "13 Jan 2014 09:52:52 MST",
            "21 Jan 2014 15:30:00 EST",
        ];
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d %b %Y %H:%M:%S %Z");
    }

    // =========================================
    // Pre-scan sampling fix tests
    // =========================================

    #[test]
    fn test_prescan_ddmm_disambiguating_at_non_sampled_index() {
        // 10,000 ambiguous dates (all values <= 12) + 1 disambiguating DD/MM
        // date placed at an index that step_by sampling would miss.
        let mut dates: Vec<String> = (0..10_000)
            .map(|i| format!("{:02}/{:02}/2025", (i % 12) + 1, (i % 12) + 1))
            .collect();
        // Place disambiguating date (day=25 > 12) at index 7 — not a multiple of step (10)
        dates[7] = "25/06/2025".to_string();
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%d/%m/%Y");
    }

    #[test]
    fn test_prescan_mmdd_disambiguating_at_non_sampled_index() {
        // 10,000 ambiguous dates + 1 disambiguating MM/DD date (position 1 > 12)
        let mut dates: Vec<String> = (0..10_000)
            .map(|i| format!("{:02}/{:02}/2025", (i % 12) + 1, (i % 12) + 1))
            .collect();
        // Place disambiguating date (day=25 at position 1) at non-sampled index
        dates[7] = "06/25/2025".to_string();
        let result = infer(&dates).unwrap();
        assert_eq!(result.format, "%m/%d/%Y");
    }

    #[test]
    fn test_prescan_no_disambiguation_uses_preference() {
        // All ambiguous — prescan finds nothing, falls back to prefer_dayfirst
        let dates: Vec<String> = (0..10_000)
            .map(|i| format!("{:02}/{:02}/2025", (i % 12) + 1, (i % 12) + 1))
            .collect();
        let result = infer(&dates).unwrap();
        // Default prefer_dayfirst=true → DD/MM
        assert_eq!(result.format, "%d/%m/%Y");
    }
}
