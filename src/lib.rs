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
mod rules;
mod tokenizer;

pub use constraints::TokenType;
pub use error::{DateInferError, Result};
pub use tokenizer::Token;

use consensus::resolve_consensus;
use format::to_strptime;
use rules::apply_rules;
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
        dates.iter().step_by(step).take(MAX_SAMPLE).collect()
    };

    let tokenized: Vec<_> = sample
        .iter()
        .map(|d| tokenize(d.as_ref()))
        .collect::<Result<Vec<_>>>()?;

    // Check all dates have same token structure
    let first_len = tokenized[0].len();
    if !tokenized.iter().all(|t| t.len() == first_len) {
        return Err(DateInferError::InconsistentFormats);
    }

    // Phase 2-3: Resolve consensus with constraints
    let (mut resolved_types, confidence) = resolve_consensus(&tokenized, options)?;

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

    Ok(InferResult {
        format,
        confidence,
        token_types: resolved_types,
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
}
