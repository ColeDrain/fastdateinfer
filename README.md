# fastdateinfer

Fast, consensus-based date format inference written in Rust with Python bindings.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)

## Why?

**The problem**: Is `01/02/2025` January 2nd or February 1st?

| Library | Approach | Problem |
|---------|----------|---------|
| pandas | `dayfirst=True` hint | You must know the format |
| dateutil | Guess per-element | Inconsistent results |
| hidateinfer | Consensus voting | Correct, but slow |

**The solution**: If your data contains `15/03/2025`, we **know** it's DD/MM/YYYY (15 can't be a month). This insight applies to ALL dates, resolving ambiguous ones like `01/02/2025`.

**fastdateinfer** implements this consensus algorithm in Rust — **270x faster than hidateinfer**.

## Installation

```bash
pip install fastdateinfer
```

## Quick Start

```python
import fastdateinfer

# Infer format from dates
result = fastdateinfer.infer(["15/03/2025", "01/02/2025", "28/12/2025"])
print(result.format)      # %d/%m/%Y
print(result.confidence)  # 1.0

# Just get the format string
fmt = fastdateinfer.infer_format(["2025-01-15", "2025-03-20"])
print(fmt)  # %Y-%m-%d

# Use with pandas
import pandas as pd
dates = ["15/03/2025", "01/02/2025", "28/12/2025"]
fmt = fastdateinfer.infer_format(dates)
df = pd.to_datetime(dates, format=fmt)
```

## Benchmarks

### vs hidateinfer (Python)

Tested on **29,351 real-world dates** across multiple formats:

| Library | Time | Speedup |
|---------|-----:|--------:|
| **fastdateinfer** | 22.5 ms | — |
| hidateinfer | 6,075 ms | **270x slower** |

### vs pandas / polars

Comparison on synthetic data (DD/MM/YYYY format):

| Dates | fastdateinfer | pandas (explicit) | pandas (mixed) | Ratio |
|------:|--------------:|------------------:|---------------:|------:|
| 100 | **0.05 ms** | 0.24 ms | 0.25 ms | 5x faster |
| 1,000 | **0.48 ms** | 0.97 ms | 1.02 ms | 2x faster |
| 10,000 | **0.74 ms** | 2.14 ms | 2.20 ms | 3x faster |
| 100,000 | **3.39 ms** | 17.00 ms | 17.50 ms | 5x faster |

> **Note**: fastdateinfer does format *inference* while pandas just *parses* a known format. Yet fastdateinfer is faster because it samples intelligently (consensus converges with ~1000 dates).

### Scaling

| Dates | Time | Per-date |
|------:|-----:|---------:|
| 1,000 | 0.48 ms | 0.48 µs |
| 10,000 | 0.74 ms | 0.07 µs |
| 100,000 | 3.39 ms | 0.03 µs |
| 1,000,000 | ~35 ms | 0.03 µs |

Performance is sublinear due to smart sampling — only ~1000 dates are fully analyzed regardless of input size.

## Supported Formats

| Format | Example | Output |
|--------|---------|--------|
| European | `15/03/2025` | `%d/%m/%Y` |
| American | `03/15/2025` | `%m/%d/%Y` |
| ISO 8601 | `2025-03-15` | `%Y-%m-%d` |
| ISO datetime | `2025-03-15T10:30:00` | `%Y-%m-%dT%H:%M:%S` |
| Month name | `15 Mar 2025` | `%d %b %Y` |
| Month name (full) | `15 March 2025` | `%d %B %Y` |
| Month first | `Mar 15, 2025` | `%b %d, %Y` |
| 2-digit year | `15/03/25` | `%d/%m/%y` |
| With time | `15/03/25 10.30.00` | `%d/%m/%y %H.%M.%S` |
| Month-year only | `March, 2025` | `%B, %Y` |
| Day-month only | `15/Mar` | `%d/%b` |

## API Reference

### `infer(dates, prefer_dayfirst=True, min_confidence=0.0, strict=False)`

Infer date format from a list of date strings.

**Arguments:**
- `dates`: List of date strings
- `prefer_dayfirst`: Use DD/MM for fully ambiguous dates (default: `True`)
- `min_confidence`: Minimum confidence threshold (default: `0.0`)
- `strict`: Raise error if any date doesn't match (default: `False`)

**Returns:** `InferResult` with:
- `format`: strptime format string
- `confidence`: float between 0.0 and 1.0
- `token_types`: list of resolved token types

```python
result = fastdateinfer.infer(["01/02/2025", "03/04/2025"], prefer_dayfirst=False)
print(result.format)  # %m/%d/%Y (American format)
```

### `infer_format(dates, prefer_dayfirst=True)`

Convenience function that returns only the format string.

```python
fmt = fastdateinfer.infer_format(["2025-01-15", "2025-03-20"])
print(fmt)  # %Y-%m-%d
```

### `infer_batch(columns, prefer_dayfirst=True)`

Infer formats for multiple columns at once.

```python
results = fastdateinfer.infer_batch({
    "transaction_date": ["15/03/2025", "01/02/2025"],
    "created_at": ["2025-01-15T10:30:00", "2025-01-16T14:45:00"],
    "value_date": ["15-Mar-2025", "01-Feb-2025"]
})

for col, result in results.items():
    print(f"{col}: {result.format}")
# transaction_date: %d/%m/%Y
# created_at: %Y-%m-%dT%H:%M:%S
# value_date: %d-%b-%Y
```

## How It Works

1. **Tokenize**: Split `"15/03/2025"` into `[15, /, 03, /, 2025]`
2. **Constrain**: `15` can only be Day (>12), `03` could be Day or Month, `2025` is Year
3. **Vote**: Across all dates, count evidence for each position
4. **Resolve**: Position 1 has strong Day evidence → Position 2 must be Month
5. **Format**: Output `%d/%m/%Y`

The key insight: **consensus converges quickly**. Even with 1 million dates, we only need to analyze ~1000 to determine the format with high confidence.

## Use Cases

### CSV/Data Processing

```python
import pandas as pd
import fastdateinfer

# Read raw data
df = pd.read_csv("data.csv")

# Detect format automatically
fmt = fastdateinfer.infer_format(df["date"].dropna().tolist())

# Parse with detected format
df["date"] = pd.to_datetime(df["date"], format=fmt)
```

### Multi-format Data Pipeline

```python
# Different columns may have different formats
results = fastdateinfer.infer_batch({
    col: df[col].dropna().astype(str).tolist()
    for col in ["date", "value_date", "created_at"]
})

for col, result in results.items():
    df[col] = pd.to_datetime(df[col], format=result.format)
```

### Validation

```python
# Ensure high confidence
result = fastdateinfer.infer(dates, min_confidence=0.9)
if result.confidence < 0.9:
    raise ValueError(f"Low confidence: {result.confidence}")
```

## Comparison

| Feature | fastdateinfer | hidateinfer | pandas | dateutil |
|---------|:-------------:|:-----------:|:------:|:--------:|
| Consensus-based | ✅ | ✅ | ❌ | ❌ |
| Speed (10k dates) | **0.74 ms** | 200 ms | 2 ms* | N/A |
| Returns strptime format | ✅ | ✅ | ❌ | ❌ |
| Batch inference | ✅ | ❌ | ❌ | ❌ |
| Type hints | ✅ | ❌ | ✅ | ✅ |
| Pure Rust core | ✅ | ❌ | ❌ | ❌ |

*pandas time is for parsing only (you must already know the format)

## Building from Source

```bash
# Prerequisites
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
pip install maturin

# Clone and build
git clone https://github.com/coledrain/fastdateinfer
cd fastdateinfer
maturin develop --release

# Run tests
cargo test
```

## License

MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please open an issue or PR on GitHub.

## Acknowledgments

- Inspired by [hidateinfer](https://github.com/hi-primus/hi-dateinfer)
- Built with [PyO3](https://pyo3.rs/) for Python bindings
- Built for high-volume data processing pipelines
