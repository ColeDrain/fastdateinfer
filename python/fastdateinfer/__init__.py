"""Fast, consensus-based date format inference."""

from .fastdateinfer import (
    InferResult,
    infer,
    infer_format,
    infer_batch,
    __version__,
)

__all__ = [
    "InferResult",
    "infer",
    "infer_format",
    "infer_batch",
    "__version__",
]
