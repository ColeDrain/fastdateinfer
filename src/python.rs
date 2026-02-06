//! Python bindings for fastdateinfer via PyO3

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

use crate::{infer_with_options, InferOptions, InferResult as RustInferResult};

/// Result of date format inference (Python class)
#[pyclass(name = "InferResult")]
#[derive(Clone)]
pub struct PyInferResult {
    /// The inferred strptime format string
    #[pyo3(get)]
    pub format: String,
    /// Confidence score (0.0 - 1.0)
    #[pyo3(get)]
    pub confidence: f64,
    /// Resolved token types as strings
    #[pyo3(get)]
    pub token_types: Vec<String>,
}

#[pymethods]
impl PyInferResult {
    fn __repr__(&self) -> String {
        format!(
            "InferResult(format='{}', confidence={:.2})",
            self.format, self.confidence
        )
    }

    fn __str__(&self) -> String {
        self.format.clone()
    }
}

impl From<RustInferResult> for PyInferResult {
    fn from(result: RustInferResult) -> Self {
        PyInferResult {
            format: result.format,
            confidence: result.confidence,
            token_types: result
                .token_types
                .into_iter()
                .map(|t| format!("{:?}", t))
                .collect(),
        }
    }
}

/// Infer date format from a list of example date strings.
///
/// Analyzes all examples together using consensus-based voting to resolve
/// ambiguous dates like "01/02/2025" (could be Jan 2 or Feb 1).
///
/// Args:
///     dates: List of date strings to analyze
///     prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)
///     min_confidence: Minimum confidence threshold (default: 0.0)
///     strict: Fail if any example doesn't match (default: False)
///
/// Returns:
///     InferResult with format string and confidence score
///
/// Raises:
///     ValueError: If inference fails
///
/// Example:
///     >>> import fastdateinfer
///     >>> result = fastdateinfer.infer(["15/03/2025", "01/02/2025"])
///     >>> print(result.format)
///     %d/%m/%Y
///     >>> print(result.confidence)
///     1.0
#[pyfunction]
#[pyo3(signature = (dates, prefer_dayfirst=true, min_confidence=0.0, strict=false))]
fn infer(
    dates: Vec<String>,
    prefer_dayfirst: bool,
    min_confidence: f64,
    strict: bool,
) -> PyResult<PyInferResult> {
    let options = InferOptions {
        prefer_dayfirst,
        min_confidence,
        strict,
    };

    infer_with_options(&dates, &options)
        .map(PyInferResult::from)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Infer date format and return just the format string.
///
/// Convenience function that returns only the format string.
///
/// Args:
///     dates: List of date strings to analyze
///     prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)
///
/// Returns:
///     strptime format string
///
/// Example:
///     >>> import fastdateinfer
///     >>> fmt = fastdateinfer.infer_format(["2025-01-15", "2025-03-20"])
///     >>> print(fmt)
///     %Y-%m-%d
#[pyfunction]
#[pyo3(signature = (dates, prefer_dayfirst=true))]
fn infer_format(dates: Vec<String>, prefer_dayfirst: bool) -> PyResult<String> {
    let options = InferOptions {
        prefer_dayfirst,
        min_confidence: 0.0,
        strict: false,
    };

    infer_with_options(&dates, &options)
        .map(|r| r.format)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Infer date formats for multiple columns at once.
///
/// Args:
///     columns: Dictionary mapping column names to lists of date strings
///     prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)
///
/// Returns:
///     Dictionary mapping column names to InferResult objects
///
/// Example:
///     >>> import fastdateinfer
///     >>> results = fastdateinfer.infer_batch({
///     ...     "date": ["15/03/2025", "20/04/2025"],
///     ...     "created_at": ["2025-01-15T10:30:00", "2025-01-16T14:45:00"]
///     ... })
///     >>> print(results["date"].format)
///     %d/%m/%Y
#[pyfunction]
#[pyo3(signature = (columns, prefer_dayfirst=true))]
fn infer_batch(
    columns: std::collections::HashMap<String, Vec<String>>,
    prefer_dayfirst: bool,
) -> PyResult<std::collections::HashMap<String, PyInferResult>> {
    let options = InferOptions {
        prefer_dayfirst,
        min_confidence: 0.0,
        strict: false,
    };

    let mut results = std::collections::HashMap::new();

    for (name, dates) in columns {
        let result = infer_with_options(&dates, &options)
            .map(PyInferResult::from)
            .map_err(|e| PyValueError::new_err(format!("Column '{}': {}", name, e)))?;
        results.insert(name, result);
    }

    Ok(results)
}

/// Fast, consensus-based date format inference.
///
/// This module provides functions to infer strptime format strings from
/// example date strings. Unlike per-element parsers (pandas, dateutil),
/// it analyzes ALL examples together to resolve ambiguous dates.
///
/// Example:
///     >>> import fastdateinfer
///     >>> # If you have "15/03/2025", we know it's DD/MM/YYYY
///     >>> # So "01/02/2025" must also be DD/MM/YYYY (Feb 1, not Jan 2)
///     >>> result = fastdateinfer.infer(["01/02/2025", "15/03/2025"])
///     >>> print(result.format)
///     %d/%m/%Y
#[pymodule]
fn fastdateinfer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInferResult>()?;
    m.add_function(wrap_pyfunction!(infer, m)?)?;
    m.add_function(wrap_pyfunction!(infer_format, m)?)?;
    m.add_function(wrap_pyfunction!(infer_batch, m)?)?;

    // Add version info
    m.add("__version__", "0.1.4")?;

    Ok(())
}
