"""Comprehensive tests for the fastdateinfer Python module."""

import re
import tomllib
from pathlib import Path

import pytest

import fastdateinfer


# =========================================
# TestModuleAttributes
# =========================================


class TestModuleAttributes:
    """Tests for module-level attributes and metadata."""

    def test_version_is_string(self):
        assert isinstance(fastdateinfer.__version__, str)

    def test_version_is_semver(self):
        assert re.match(r"^\d+\.\d+\.\d+", fastdateinfer.__version__)

    def test_version_matches_cargo_toml(self):
        cargo_path = Path(__file__).resolve().parent.parent / "Cargo.toml"
        cargo = tomllib.loads(cargo_path.read_text())
        assert fastdateinfer.__version__ == cargo["package"]["version"]

    def test_exports_exist(self):
        assert callable(fastdateinfer.infer)
        assert callable(fastdateinfer.infer_format)
        assert callable(fastdateinfer.infer_batch)


# =========================================
# TestInfer
# =========================================


class TestInfer:
    """Tests for the infer() function."""

    def test_dmy_format(self):
        result = fastdateinfer.infer(["15/03/2025", "20/04/2025", "25/12/2025"])
        assert result.format == "%d/%m/%Y"

    def test_iso_format(self):
        result = fastdateinfer.infer(["2025-01-15", "2025-03-20"])
        assert result.format == "%Y-%m-%d"

    def test_american_format(self):
        result = fastdateinfer.infer(["12/13/2025", "01/25/2025"])
        assert result.format == "%m/%d/%Y"

    def test_month_names(self):
        result = fastdateinfer.infer(["15 Jan 2025", "20 Mar 2025"])
        assert result.format == "%d %b %Y"

    def test_datetime_with_time(self):
        result = fastdateinfer.infer(["2025-01-15 10:30:00", "2025-03-20 14:45:30"])
        assert result.format == "%Y-%m-%d %H:%M:%S"

    def test_consensus_resolves_ambiguous(self):
        # 01/02 is ambiguous, but 15/03 proves DD/MM
        result = fastdateinfer.infer(["01/02/2025", "15/03/2025"])
        assert result.format == "%d/%m/%Y"

    def test_prefer_dayfirst_false(self):
        result = fastdateinfer.infer(
            ["01/02/2025", "03/04/2025"], prefer_dayfirst=False
        )
        assert result.format == "%m/%d/%Y"

    def test_infer_result_attributes(self):
        result = fastdateinfer.infer(["15/03/2025", "20/04/2025"])
        assert hasattr(result, "format")
        assert hasattr(result, "confidence")
        assert hasattr(result, "token_types")
        assert isinstance(result.format, str)
        assert isinstance(result.confidence, float)
        assert isinstance(result.token_types, list)
        assert result.confidence > 0.0

    def test_repr(self):
        result = fastdateinfer.infer(["15/03/2025", "20/04/2025"])
        r = repr(result)
        assert "InferResult" in r
        assert result.format in r

    def test_str(self):
        result = fastdateinfer.infer(["15/03/2025", "20/04/2025"])
        assert str(result) == result.format

    def test_empty_input_raises(self):
        with pytest.raises(ValueError, match="no date strings"):
            fastdateinfer.infer([])

    def test_min_confidence(self):
        with pytest.raises(ValueError, match="confidence"):
            fastdateinfer.infer(
                ["01/02/2025", "03/04/2025"], min_confidence=1.1
            )


# =========================================
# TestInferFormat
# =========================================


class TestInferFormat:
    """Tests for the infer_format() convenience function."""

    def test_returns_string(self):
        fmt = fastdateinfer.infer_format(["15/03/2025", "20/04/2025"])
        assert isinstance(fmt, str)
        assert fmt == "%d/%m/%Y"

    def test_prefer_dayfirst(self):
        fmt = fastdateinfer.infer_format(
            ["01/02/2025", "03/04/2025"], prefer_dayfirst=False
        )
        assert fmt == "%m/%d/%Y"

    def test_error_propagation(self):
        with pytest.raises(ValueError):
            fastdateinfer.infer_format([])


# =========================================
# TestInferBatch
# =========================================


class TestInferBatch:
    """Tests for the infer_batch() function."""

    def test_multiple_columns(self):
        results = fastdateinfer.infer_batch({
            "date": ["15/03/2025", "20/04/2025"],
            "created_at": ["2025-01-15T10:30:00", "2025-01-16T14:45:00"],
        })
        assert "date" in results
        assert "created_at" in results
        assert results["date"].format == "%d/%m/%Y"
        assert results["created_at"].format == "%Y-%m-%dT%H:%M:%S"

    def test_returns_dict(self):
        results = fastdateinfer.infer_batch({
            "col1": ["15/03/2025", "20/04/2025"],
        })
        assert isinstance(results, dict)

    def test_batch_matches_individual(self):
        columns = {
            "dmy": ["15/03/2025", "20/04/2025", "25/12/2025"],
            "iso": ["2025-01-15", "2025-03-20", "2025-06-01"],
            "named": ["15 Jan 2025", "20 Mar 2025", "01 Jun 2025"],
        }
        batch = fastdateinfer.infer_batch(columns)
        for name, dates in columns.items():
            individual = fastdateinfer.infer(dates)
            assert batch[name].format == individual.format

    def test_error_propagation(self):
        with pytest.raises(ValueError):
            fastdateinfer.infer_batch({"bad": []})

    def test_empty_dict(self):
        results = fastdateinfer.infer_batch({})
        assert results == {}

    def test_batch_many_columns(self):
        # 1000 columns exercising parallelism at scale
        columns = {}
        for i in range(1000):
            columns[f"col_{i}"] = ["15/03/2025", "20/04/2025", "25/12/2025"]
        results = fastdateinfer.infer_batch(columns)
        assert len(results) == 1000
        for name, result in results.items():
            assert result.format == "%d/%m/%Y"


# =========================================
# TestStrictMode
# =========================================


class TestStrictMode:
    """Tests for the strict validation option."""

    def test_passes_with_consistent_dates(self):
        result = fastdateinfer.infer(
            ["15/03/2025", "20/04/2025", "25/12/2025"], strict=True
        )
        assert result.format == "%d/%m/%Y"

    def test_fails_with_incompatible(self):
        with pytest.raises(ValueError, match="strict validation failed"):
            fastdateinfer.infer(
                ["15/03/2025", "20/04/2025", "not-a-date"], strict=True
            )

    def test_false_tolerates(self):
        # strict=False (default) should not fail
        result = fastdateinfer.infer(
            ["15/03/2025", "20/04/2025", "25/12/2025"], strict=False
        )
        assert result.format == "%d/%m/%Y"


# =========================================
# TestInconsistentFormatsTolerance
# =========================================


class TestInconsistentFormatsTolerance:
    """Tests for tolerating minor inconsistencies."""

    def test_trailing_space_tolerated(self):
        result = fastdateinfer.infer(["15/03/2025", "20/04/2025", "25/12/2025 "])
        assert result.format == "%d/%m/%Y"

    def test_malformed_row_tolerated(self):
        result = fastdateinfer.infer(
            ["15/03/2025", "20/04/2025", "25/12/2025", "01/01/2025", "N/A"]
        )
        assert result.format == "%d/%m/%Y"
        assert result.confidence < 1.0

    def test_confidence_reduction(self):
        # 4 good dates + 1 bad -> filter_ratio = 0.8
        result = fastdateinfer.infer(
            ["15/03/2025", "20/04/2025", "25/12/2025", "01/06/2025", "N/A"]
        )
        assert result.confidence <= 0.8 + 1e-9
        assert result.confidence > 0.0


# =========================================
# TestPrescanSamplingFix
# =========================================


class TestPrescanSamplingFix:
    """Tests for the pre-scan fix that ensures disambiguating dates are sampled."""

    def test_ddmm_disambiguating_at_non_sampled_index(self):
        # 10,000 ambiguous dates (all values <= 12) + 1 disambiguating DD/MM
        # date placed at an index that step_by sampling would miss.
        dates = [
            f"{(i % 12) + 1:02d}/{(i % 12) + 1:02d}/2025" for i in range(10_000)
        ]
        # Place disambiguating date (day=25 > 12) at non-sampled index
        dates[7] = "25/06/2025"
        result = fastdateinfer.infer(dates)
        assert result.format == "%d/%m/%Y"

    def test_mmdd_disambiguating_at_non_sampled_index(self):
        # 10,000 ambiguous dates + 1 disambiguating MM/DD (position 1 > 12)
        dates = [
            f"{(i % 12) + 1:02d}/{(i % 12) + 1:02d}/2025" for i in range(10_000)
        ]
        dates[7] = "06/25/2025"
        result = fastdateinfer.infer(dates)
        assert result.format == "%m/%d/%Y"

    def test_no_disambiguation_uses_preference(self):
        # All ambiguous — prescan finds nothing, falls back to prefer_dayfirst
        dates = [
            f"{(i % 12) + 1:02d}/{(i % 12) + 1:02d}/2025" for i in range(10_000)
        ]
        result = fastdateinfer.infer(dates)
        assert result.format == "%d/%m/%Y"  # default prefer_dayfirst=True

    def test_no_disambiguation_monthfirst(self):
        dates = [
            f"{(i % 12) + 1:02d}/{(i % 12) + 1:02d}/2025" for i in range(10_000)
        ]
        result = fastdateinfer.infer(dates, prefer_dayfirst=False)
        assert result.format == "%m/%d/%Y"
