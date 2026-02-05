//! Error types for dateinfer-rs

use thiserror::Error;

/// Result type alias for dateinfer operations
pub type Result<T> = std::result::Result<T, DateInferError>;

/// Errors that can occur during date format inference
#[derive(Debug, Error)]
pub enum DateInferError {
    /// No date strings provided
    #[error("no date strings provided")]
    EmptyInput,

    /// Date strings have inconsistent token structures
    #[error("date strings have inconsistent formats")]
    InconsistentFormats,

    /// Could not parse a date string
    #[error("failed to tokenize date string: {0}")]
    TokenizeError(String),

    /// Confidence below required threshold
    #[error("confidence {got:.2} below required threshold {required:.2}")]
    LowConfidence { got: f64, required: f64 },

    /// Could not resolve ambiguous tokens
    #[error("could not resolve ambiguous date components")]
    UnresolvableAmbiguity,

    /// No valid date pattern found
    #[error("no valid date pattern found in input")]
    NoValidPattern,
}
