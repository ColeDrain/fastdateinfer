# fastdateinfer

Fast, consensus-based date-format inference in Rust — a maintained, ~270× faster successor to [dateinfer](https://github.com/jeffreystarr/dateinfer) / [hi-dateinfer](https://github.com/hi-primus/hi-dateinfer).

[![PyPI](https://img.shields.io/pypi/v/fastdateinfer.svg)](https://pypi.org/project/fastdateinfer/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python 3.10–3.14](https://img.shields.io/badge/python-3.10%E2%80%933.14-blue.svg)](https://www.python.org/downloads/)

## What it does

Give it a list (or column) of date strings; it returns the `strptime` format that parses them — inferred from the examples themselves.

The point is *how* it decides ordering: it reads the **whole column as evidence** rather than each value alone. So it resolves ambiguous `DD/MM` vs `MM/DD` by consensus — a single unambiguous `15/03/2025` (15 can't be a month) settles every ambiguous `01/02/2025` in the same column.

```python
import fastdateinfer

fastdateinfer.infer_format(["15/03/2025", "01/02/2025", "28/12/2025"])
# '%d/%m/%Y'
```

It also tolerates dirty rows (`""`, `"N/A"`, reflected in a confidence score), ISO datetimes with timezones, AM/PM, month names, and two-digit years.

## What it replaces

`dateinfer` and its fork `hi-dateinfer` are the established tools for this. The original is archived; the fork's last release was 2021 — both are pure Python. fastdateinfer is the same idea, rebuilt: maintained, type-hinted, a Rust core, wheels for Python 3.10–3.14, and ~270× faster.

| | fastdateinfer | hi-dateinfer | pandas | polars |
|---|:---:|:---:|:---:|:---:|
| Resolves ambiguous DD/MM vs MM/DD by consensus | ✅ | ✅ | ❌ (needs `dayfirst`) | ❌ (needs explicit format) |
| Returns a strptime format string | ✅ | ✅ | ❌ | ❌ |
| Speed (10k dates) | **~0.9 ms** | ~200 ms | n/a* | n/a* |
| Maintained | ✅ | ❌ (2021) | ✅ | ✅ |
| Pure Rust core | ✅ | ❌ | ❌ | ✅ |

<sub>*pandas/polars parse dates; they don't return a format string for you to reuse.</sub>

## Works with pandas and polars

fastdateinfer infers the format; pandas and polars do the parsing. It returns a plain format string, so it drops straight into both — and fills the gap where their own inference can't resolve ambiguous `DD/MM` vs `MM/DD`.

```python
import fastdateinfer

# pandas
import pandas as pd
fmt = fastdateinfer.infer_format(df["date"].dropna().astype(str).tolist())
df["date"] = pd.to_datetime(df["date"], format=fmt)

# polars
import polars as pl
fmt = fastdateinfer.infer_format(df["date"].drop_nulls().to_list())
df = df.with_columns(pl.col("date").str.to_datetime(fmt))
```

## Installation

```bash
pip install fastdateinfer
```

## Quick Start

```python
import fastdateinfer

# Full result: format, confidence, token types
result = fastdateinfer.infer(["15/03/2025", "01/02/2025", "28/12/2025"])
print(result.format)      # %d/%m/%Y
print(result.confidence)  # 1.0

# Just the format string
fmt = fastdateinfer.infer_format(["2025-01-15", "2025-03-20"])
print(fmt)  # %Y-%m-%d
```

## Handling Dirty Data

Real-world data is messy. **fastdateinfer** tolerates common issues:

```python
# Empty strings, "N/A", trailing spaces — all handled gracefully
dates = ["15/03/2025", "20/04/2025", "", "N/A", "25/12/2025 "]
result = fastdateinfer.infer(dates)
print(result.format)      # %d/%m/%Y
print(result.confidence)  # 0.6 (reduced proportionally to dirty rows)
```

As long as >50% of rows share the same token structure, inference succeeds. Outliers are filtered and confidence is reduced proportionally.

## Strict Mode

For pipelines where every row must conform:

```python
# Raises ValueError if ANY date doesn't match
try:
    result = fastdateinfer.infer(
        ["15/03/2025", "20/04/2025", "not-a-date"],
        strict=True
    )
except ValueError as e:
    print(e)  # strict validation failed: 1 of 3 dates incompatible
```

## Benchmarks

### Python API (Apple Silicon)

| Dates | `infer()` | `strict=True` |
|------:|----------:|--------------:|
| 100 | **0.05 ms** | 0.09 ms |
| 1,000 | **0.47 ms** | 0.84 ms |
| 10,000 | **0.80 ms** | 4.48 ms |
| 100,000 | **4.06 ms** | — |
| 1,000,000 | **36.7 ms** | — |

`infer_batch` (100 columns, 3 dates each): **0.22 ms** — columns processed in parallel with GIL released.

### Rust Core (Criterion)

| Dates | Time |
|------:|-----:|
| 100 | 43 µs |
| 1,000 | 436 µs |
| 10,000 | 518 µs |
| 100,000 | 1.2 ms |

Pre-scan overhead is negligible — adds < 5% to large-dataset inference.

### Scaling

| Dates | Time | Per-date |
|------:|-----:|---------:|
| 1,000 | 0.47 ms | 0.47 µs |
| 10,000 | 0.80 ms | 0.08 µs |
| 100,000 | 4.06 ms | 0.04 µs |
| 1,000,000 | 36.7 ms | 0.04 µs |

Performance is sublinear due to smart sampling — only ~1000 dates are fully analyzed regardless of input size. A lightweight pre-scan ensures disambiguating dates (value > 12) are always included in the sample.

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
| Weekday + timezone | `Mon Jan 13 09:52:52 MST 2014` | `%a %b %d %H:%M:%S %Z %Y` |
| 2-digit year | `15/03/25` | `%d/%m/%y` |
| With time | `15/03/25 10.30.00` | `%d/%m/%y %H.%M.%S` |
| Month-year only | `March, 2025` | `%B, %Y` |
| Day-month only | `15/Mar` | `%d/%b` |

> **Note on `%Z` (named timezones like `MST`, `EST`):** inference returns the
> correct `%Z` token, but Python's own `datetime.strptime` cannot reliably parse
> arbitrary timezone *abbreviations* back — `strptime("...MST...", "...%Z...")`
> raises in the stdlib. This is a CPython limitation, not an inference error. For
> round-trippable parsing, prefer numeric offsets (`%z`, e.g. `-0500`) or a
> parser such as `dateutil`.

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

Infer formats for multiple columns at once. Columns are processed in parallel (GIL released).

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
| Speed (10k dates) | **0.80 ms** | 200 ms | 2 ms* | N/A |
| Dirty data tolerance | ✅ | ❌ | ❌ | ❌ |
| Strict validation | ✅ | ❌ | ❌ | ❌ |
| Returns strptime format | ✅ | ✅ | ❌ | ❌ |
| Parallel batch inference | ✅ | ❌ | ❌ | ❌ |
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
