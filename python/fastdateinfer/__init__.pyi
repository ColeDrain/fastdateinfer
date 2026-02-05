"""Type stubs for fastdateinfer."""

from typing import Dict, List

__version__: str

class InferResult:
    """Result of date format inference."""

    format: str
    """The inferred strptime format string."""

    confidence: float
    """Confidence score (0.0 - 1.0)."""

    token_types: List[str]
    """Resolved token types as strings."""

def infer(
    dates: List[str],
    prefer_dayfirst: bool = True,
    min_confidence: float = 0.0,
    strict: bool = False,
) -> InferResult:
    """
    Infer date format from a list of example date strings.

    Analyzes all examples together using consensus-based voting to resolve
    ambiguous dates like "01/02/2025" (could be Jan 2 or Feb 1).

    Args:
        dates: List of date strings to analyze
        prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)
        min_confidence: Minimum confidence threshold (default: 0.0)
        strict: Fail if any example doesn't match (default: False)

    Returns:
        InferResult with format string and confidence score

    Raises:
        ValueError: If inference fails

    Example:
        >>> result = infer(["15/03/2025", "01/02/2025"])
        >>> print(result.format)
        %d/%m/%Y
    """
    ...

def infer_format(
    dates: List[str],
    prefer_dayfirst: bool = True,
) -> str:
    """
    Infer date format and return just the format string.

    Args:
        dates: List of date strings to analyze
        prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)

    Returns:
        strptime format string

    Example:
        >>> fmt = infer_format(["2025-01-15", "2025-03-20"])
        >>> print(fmt)
        %Y-%m-%d
    """
    ...

def infer_batch(
    columns: Dict[str, List[str]],
    prefer_dayfirst: bool = True,
) -> Dict[str, InferResult]:
    """
    Infer date formats for multiple columns at once.

    Args:
        columns: Dictionary mapping column names to lists of date strings
        prefer_dayfirst: Prefer DD/MM format for ambiguous dates (default: True)

    Returns:
        Dictionary mapping column names to InferResult objects

    Example:
        >>> results = infer_batch({
        ...     "date": ["15/03/2025", "20/04/2025"],
        ...     "created_at": ["2025-01-15T10:30:00", "2025-01-16T14:45:00"]
        ... })
        >>> print(results["date"].format)
        %d/%m/%Y
    """
    ...
