# Changelog

All notable changes to this project will be documented in this file.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Python 3.14 support: PyO3 bumped 0.22 → 0.28, and 3.14 added to the CI test
  matrix and the release wheel matrix. Wheels now cover Python 3.10–3.14.

### Fixed
- Spelling-variant months no longer lower confidence: `"May"` is 3 letters so it
  tokenizes as a short month name while `"January"` is a full one, which split
  the votes at that position. Both now count toward the resolved month type, so
  `%d %B %Y` data containing "May" reports full confidence (the format was
  already correct; only the score dipped).

### Changed
- CI/release workflows: bumped `actions/checkout` v4→v5, `actions/setup-python`
  v5→v6, and `actions/{upload,download}-artifact` v4→v5 (Node 20 runtime is
  being retired). Rust toolchain pin raised 1.81 → 1.83 (PyO3 0.28 MSRV).

## [0.2.0] - 2026-06-04

### Fixed
- **ISO ordering with all-ambiguous values**: year-first dates whose day/month
  values are all ≤ 12 (e.g. `2025-02-03`) were inferred as `%Y-%d-%m`. A leading
  year now forces month-before-day (ISO 8601) regardless of `prefer_dayfirst`;
  the flag still governs genuinely-ambiguous trailing-year formats (DD/MM vs MM/DD).
- **Confidence now reflects unclassified literals**: positions that could not be
  classified (e.g. the `W` in `2025-W03-1`) were silently excluded from the
  confidence score, so partly-literal formats reported `confidence = 1.0`. Such
  positions now count toward the score as zero, so a stray literal lowers it.

### Added
- Oversized-token guard: a single component longer than 32 bytes (longer than
  any real date field) is now rejected during tokenization, so untrusted input
  with a giant junk run is filtered as an outlier instead of being echoed
  verbatim into the output format string. All-junk input errors cleanly.
- `DateInferError::ContradictoryFormat` (surfaced as `ValueError` in Python):
  inputs that force two components to both be the day-of-month — e.g.
  `13/13/2025`, or a dataset mixing DD/MM with MM/DD — are now rejected instead
  of producing an invalid `%d/%d/%Y` format string.

## [0.1.6] - 2026-02-06

### Added
- `strict` mode now enforced: when `strict=True`, every input date is validated against the inferred format. Returns `StrictValidationFailed` with counts if any are incompatible.
- Parallel `infer_batch`: columns are now processed in parallel via rayon with GIL released, significantly faster for many columns.
- Comprehensive Python test suite (`tests/test_fastdateinfer.py`) — 35 tests covering all public API surface.
- pytest added to CI workflow.
- Pre-scan for disambiguating dates: when sampling large datasets (>1000 dates), a lightweight byte-level scanner now finds dates with values >12 that prove DD/MM vs MM/DD ordering. These are injected into the sample (at most 2 replacements) to prevent misclassification when disambiguating dates fall outside the `step_by` sample.
- Weekday (`%a`/`%A`) and timezone (`%Z`) token support: dates like `Mon Jan 13 09:52:52 MST 2014` now infer correctly as `%a %b %d %H:%M:%S %Z %Y` — matching hidateinfer's flagship example.
- AM/PM 12-hour time support (`%I %p`): dates like `01/15/2025 02:30:00 PM` now correctly infer as `%m/%d/%Y %I:%M:%S %p`.
- Subsecond/microsecond support (`%f`): fractional seconds like `.123456` or `.123` after time components now infer as `%S.%f`.
- Negative timezone offset support: `-0500` and `-05:00` now correctly tokenized as `%z` instead of being split into separator + number.

### Fixed
- `T` in weekday names (Tue, Thu) was incorrectly treated as an ISO datetime separator, causing tokenization mismatch. The tokenizer now only treats standalone `T` after a numeric token as a separator.
- `-` in timezone offsets was incorrectly consumed by the date separator handler. The tokenizer now checks for time context before treating `-` followed by digits as a timezone offset.

### Changed
- `InconsistentFormats` is now tolerant: a majority (>50%) of dates with the same token structure is sufficient. Outliers (empty strings, "N/A", trailing spaces) are filtered out and confidence is reduced proportionally.
- `__version__` now reads from `Cargo.toml` at compile time via `env!("CARGO_PKG_VERSION")` instead of a hardcoded string.
- `pyproject.toml` version is now dynamic, sourced from `Cargo.toml` (single source of truth).

### Fixed
- `__version__` no longer drifts from `Cargo.toml` across releases.
- `strict=True` was accepted but silently ignored; now fully enforced.

## [0.1.4] - 2025-05-15

### Fixed
- Pinned Python versions in release workflow.

## [0.1.3] - 2025-05-14

### Changed
- Use trusted publishing for PyPI releases.

## [0.1.2] - 2025-05-13

### Fixed
- CI: pin Python versions in release workflow.

## [0.1.1] - 2025-05-12

### Fixed
- Bump Rust to 1.81 for half crate compatibility.
- Pin Rust to 1.80 for stable clippy.
- Fix rust-toolchain action usage.

## [0.1.0] - 2025-05-11

### Added
- Initial release.
- Consensus-based date format inference from example strings.
- Python bindings via PyO3/maturin.
- `infer()`, `infer_format()`, `infer_batch()` API.
- `prefer_dayfirst` option for ambiguous dates.
- Multi-platform wheels (Linux, macOS, Windows) for Python 3.10-3.13.
