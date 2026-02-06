# Changelog

All notable changes to this project will be documented in this file.

This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `strict` mode now enforced: when `strict=True`, every input date is validated against the inferred format. Returns `StrictValidationFailed` with counts if any are incompatible.
- Parallel `infer_batch`: columns are now processed in parallel via rayon with GIL released, significantly faster for many columns.
- Comprehensive Python test suite (`tests/test_fastdateinfer.py`) — 35 tests covering all public API surface.
- pytest added to CI workflow.
- Pre-scan for disambiguating dates: when sampling large datasets (>1000 dates), a lightweight byte-level scanner now finds dates with values >12 that prove DD/MM vs MM/DD ordering. These are injected into the sample (at most 2 replacements) to prevent misclassification when disambiguating dates fall outside the `step_by` sample.
- Weekday (`%a`/`%A`) and timezone (`%Z`) token support: dates like `Mon Jan 13 09:52:52 MST 2014` now infer correctly as `%a %b %d %H:%M:%S %Z %Y` — matching hidateinfer's flagship example.

### Fixed
- `T` in weekday names (Tue, Thu) was incorrectly treated as an ISO datetime separator, causing tokenization mismatch. The tokenizer now only treats standalone `T` after a numeric token as a separator.

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
