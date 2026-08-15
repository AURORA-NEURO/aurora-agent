"""Dependency-free benchmark distribution and paired-effect ergonomics.

The Rust metrics kernel remains authoritative for transported analytics reports. This module is a
Python-side notebook/evaluator utility for distribution work that belongs above the kernel: it
keeps measured evidence separate from declarations, computes reproducible descriptive summaries,
and offers a deterministic non-parametric bootstrap with an explicit resampling unit. It never
performs hypothesis tests, causal estimation, clinical validation, or automatic evidence pooling.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import math
from typing import Any, Mapping, Sequence

from .analytics import AnalyticsDirection, AnalyticsEvidence
from .authoring import content_digest
from .errors import ArgumentError


MAX_BENCHMARK_OBSERVATIONS = 100_000
MAX_BOOTSTRAP_RESAMPLES = 10_000
MIN_BOOTSTRAP_RESAMPLES = 100
MAX_QUANTILES = 32
DEFAULT_QUANTILES = (0.05, 0.25, 0.5, 0.75, 0.95)
DEFAULT_BOOTSTRAP_SEED = 0xA0B0C0D0
_UINT64_MASK = (1 << 64) - 1
_MEASURED = {AnalyticsEvidence.OBSERVED.value, AnalyticsEvidence.REPRODUCED.value}
_EVIDENCE_VALUES = {evidence.value for evidence in AnalyticsEvidence}


class ResamplingUnit(str, Enum):
    OBSERVATION = "observation"
    REPLICATE_GROUP = "replicate_group"


def _text(name: str, value: str, maximum: int = 512) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")


def _finite(name: str, value: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number")
    value = float(value)
    if not math.isfinite(value):
        raise ArgumentError(f"{name} must be a finite number")
    return value


def _evidence(value: AnalyticsEvidence | str) -> str:
    candidate = value.value if isinstance(value, AnalyticsEvidence) else value
    if candidate not in _EVIDENCE_VALUES:
        raise ArgumentError(f"evidence is not a valid AnalyticsEvidence: {value!r}")
    return candidate


def _direction(value: AnalyticsDirection | str) -> str:
    candidate = value.value if isinstance(value, AnalyticsDirection) else value
    if candidate not in {direction.value for direction in AnalyticsDirection}:
        raise ArgumentError(f"direction is not a valid AnalyticsDirection: {value!r}")
    return candidate


def _unit(value: ResamplingUnit | str) -> ResamplingUnit:
    try:
        return value if isinstance(value, ResamplingUnit) else ResamplingUnit(value)
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"resampling_unit is not valid: {value!r}") from error


@dataclass(frozen=True)
class BenchmarkObservation:
    """One scalar benchmark row with evidence provenance and optional cluster identity."""

    id: str
    domain: str
    dimension: str
    system: str
    value: float | None
    evidence: AnalyticsEvidence | str = AnalyticsEvidence.OBSERVED
    replicate_group: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "domain", "dimension", "system"):
            _text(name, getattr(self, name))
        evidence = _evidence(self.evidence)
        object.__setattr__(self, "evidence", evidence)
        if self.value is not None:
            object.__setattr__(self, "value", _finite("value", self.value))
        elif evidence in _MEASURED:
            raise ArgumentError("measured benchmark observations require a value")
        if self.replicate_group is not None:
            _text("replicate_group", self.replicate_group)

    @property
    def measured(self) -> bool:
        return self.evidence in _MEASURED and self.value is not None

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "domain": self.domain,
            "dimension": self.dimension,
            "system": self.system,
            "value": self.value,
            "evidence": self.evidence,
        }
        if self.replicate_group is not None:
            result["replicate_group"] = self.replicate_group
        return result


@dataclass(frozen=True)
class PairedBenchmarkObservation:
    """A paired baseline/variant row; the result remains a contrast, never a causal effect."""

    id: str
    domain: str
    dimension: str
    baseline: float | None
    variant: float | None
    direction: AnalyticsDirection | str
    tolerance: float
    evidence: AnalyticsEvidence | str = AnalyticsEvidence.OBSERVED
    replicate_group: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "domain", "dimension"):
            _text(name, getattr(self, name))
        evidence = _evidence(self.evidence)
        direction = _direction(self.direction)
        object.__setattr__(self, "evidence", evidence)
        object.__setattr__(self, "direction", direction)
        object.__setattr__(self, "tolerance", _finite("tolerance", self.tolerance))
        if self.tolerance < 0:
            raise ArgumentError("tolerance must be non-negative")
        if self.baseline is not None:
            object.__setattr__(self, "baseline", _finite("baseline", self.baseline))
        if self.variant is not None:
            object.__setattr__(self, "variant", _finite("variant", self.variant))
        if evidence in _MEASURED and (self.baseline is None or self.variant is None):
            raise ArgumentError("measured paired observations require baseline and variant values")
        if self.replicate_group is not None:
            _text("replicate_group", self.replicate_group)

    @property
    def measured(self) -> bool:
        return self.evidence in _MEASURED and self.baseline is not None and self.variant is not None

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "domain": self.domain,
            "dimension": self.dimension,
            "baseline": self.baseline,
            "variant": self.variant,
            "direction": self.direction,
            "tolerance": self.tolerance,
            "evidence": self.evidence,
        }
        if self.replicate_group is not None:
            result["replicate_group"] = self.replicate_group
        return result


class _SplitMix64:
    """Small specified PRNG so bootstrap output does not depend on Python's RNG implementation."""

    def __init__(self, seed: int) -> None:
        if isinstance(seed, bool) or not isinstance(seed, int) or not 0 <= seed <= _UINT64_MASK:
            raise ArgumentError("seed must be an unsigned 64-bit integer")
        self.state = seed

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & _UINT64_MASK
        value = self.state
        value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & _UINT64_MASK
        value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & _UINT64_MASK
        return (value ^ (value >> 31)) & _UINT64_MASK


def _quantile(sorted_values: Sequence[float], probability: float) -> float | None:
    if not sorted_values:
        return None
    position = (len(sorted_values) - 1) * probability
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return sorted_values[lower]
    fraction = position - lower
    return sorted_values[lower] + fraction * (sorted_values[upper] - sorted_values[lower])


def _validate_quantiles(quantiles: Sequence[float]) -> tuple[float, ...]:
    if isinstance(quantiles, (str, bytes)):
        raise ArgumentError(f"quantiles must contain at most {MAX_QUANTILES} probabilities")
    try:
        raw_quantiles = tuple(quantiles)
    except TypeError as error:
        raise ArgumentError("quantiles must be a finite sequence of probabilities") from error
    if len(raw_quantiles) > MAX_QUANTILES:
        raise ArgumentError(f"quantiles must contain at most {MAX_QUANTILES} probabilities")
    normalized = tuple(_finite("quantile", value) for value in raw_quantiles)
    if any(value < 0 or value > 1 for value in normalized):
        raise ArgumentError("quantiles must be between 0 and 1")
    if len(set(normalized)) != len(normalized):
        raise ArgumentError("quantiles must be unique")
    return tuple(sorted(normalized))


def _bootstrap_groups(
    values: Sequence[float],
    groups: Sequence[str | None] | None,
) -> tuple[tuple[float, ...], ...]:
    if groups is None:
        return tuple((value,) for value in values)
    if len(groups) != len(values):
        raise ArgumentError("bootstrap group count must match value count")
    grouped: dict[str, list[float]] = {}
    for value, group in zip(values, groups):
        if group is not None:
            _text("bootstrap group", group)
        key = group if group is not None else f"__observation_{len(grouped)}"
        grouped.setdefault(key, []).append(value)
    if any(not group for group in grouped.values()):
        raise ArgumentError("bootstrap groups must not be empty")
    return tuple(tuple(grouped[key]) for key in sorted(grouped))


@dataclass(frozen=True)
class BootstrapInterval:
    """Deterministic percentile bootstrap interval with its resampling assumptions attached."""

    statistic: str
    estimate: float | None
    lower: float | None
    upper: float | None
    confidence: float
    resamples: int
    seed: int
    resampling_unit: ResamplingUnit
    cluster_count: int
    method: str = "percentile_bootstrap"

    def to_dict(self) -> dict[str, Any]:
        return {
            "statistic": self.statistic,
            "estimate": self.estimate,
            "lower": self.lower,
            "upper": self.upper,
            "confidence": self.confidence,
            "resamples": self.resamples,
            "seed": self.seed,
            "resampling_unit": self.resampling_unit.value,
            "cluster_count": self.cluster_count,
            "method": self.method,
            "limitations": [
                "this is a descriptive percentile bootstrap, not a hypothesis test or causal interval",
                "exchangeability and independence are not established by this utility",
                "cluster bootstrap preserves declared replicate groups but does not discover unlabelled dependence",
            ],
        }


def bootstrap_mean(
    values: Sequence[float],
    *,
    groups: Sequence[str | None] | None = None,
    confidence: float = 0.95,
    resamples: int = 1_000,
    seed: int = DEFAULT_BOOTSTRAP_SEED,
    resampling_unit: ResamplingUnit | str = ResamplingUnit.OBSERVATION,
) -> BootstrapInterval:
    """Compute a deterministic percentile bootstrap over observations or declared clusters."""

    if not 0 < confidence < 1:
        raise ArgumentError("confidence must be strictly between 0 and 1")
    if isinstance(resamples, bool) or not isinstance(resamples, int) or not MIN_BOOTSTRAP_RESAMPLES <= resamples <= MAX_BOOTSTRAP_RESAMPLES:
        raise ArgumentError(f"resamples must be between {MIN_BOOTSTRAP_RESAMPLES} and {MAX_BOOTSTRAP_RESAMPLES}")
    if not values:
        return BootstrapInterval("mean", None, None, None, confidence, resamples, seed, _unit(resampling_unit), 0)
    normalized = tuple(_finite("value", value) for value in values)
    unit = _unit(resampling_unit)
    if unit is ResamplingUnit.REPLICATE_GROUP and groups is None:
        raise ArgumentError("replicate_group bootstrap requires a group label for every value")
    if unit is ResamplingUnit.OBSERVATION:
        clusters = tuple((value,) for value in normalized)
    else:
        clusters = _bootstrap_groups(normalized, groups)
    if not clusters or not any(clusters):
        return BootstrapInterval("mean", None, None, None, confidence, resamples, seed, unit, len(clusters))
    estimate = sum(normalized) / len(normalized)
    generator = _SplitMix64(seed)
    estimates: list[float] = []
    cluster_count = len(clusters)
    for _ in range(resamples):
        selected: list[float] = []
        for _ in range(cluster_count):
            selected.extend(clusters[generator.next() % cluster_count])
        estimates.append(sum(selected) / len(selected))
    estimates.sort()
    alpha = (1 - confidence) / 2
    return BootstrapInterval(
        "mean",
        estimate,
        _quantile(estimates, alpha),
        _quantile(estimates, 1 - alpha),
        confidence,
        resamples,
        seed,
        unit,
        cluster_count,
    )


@dataclass(frozen=True)
class DistributionSummary:
    """Descriptive distribution with measured/evidence counts and optional uncertainty."""

    total_count: int
    measured_count: int
    excluded_count: int
    missing_count: int
    declared_count: int
    blocked_count: int
    not_applicable_count: int
    count: int
    mean: float | None
    median: float | None
    minimum: float | None
    maximum: float | None
    variance_sample: float | None
    standard_deviation_sample: float | None
    median_absolute_deviation: float | None
    interquartile_range: float | None
    quantiles: Mapping[str, float | None]
    values_digest: str | None
    bootstrap: BootstrapInterval | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "total_count": self.total_count,
            "measured_count": self.measured_count,
            "excluded_count": self.excluded_count,
            "missing_count": self.missing_count,
            "declared_count": self.declared_count,
            "blocked_count": self.blocked_count,
            "not_applicable_count": self.not_applicable_count,
            "count": self.count,
            "mean": self.mean,
            "median": self.median,
            "minimum": self.minimum,
            "maximum": self.maximum,
            "variance_sample": self.variance_sample,
            "standard_deviation_sample": self.standard_deviation_sample,
            "median_absolute_deviation": self.median_absolute_deviation,
            "interquartile_range": self.interquartile_range,
            "quantiles": dict(self.quantiles),
            "values_digest": self.values_digest,
            "bootstrap": self.bootstrap.to_dict() if self.bootstrap else None,
            "limitations": [
                "only observed and reproduced rows contribute to measured summaries",
                "sample variance uses denominator n-1 and is null for fewer than two measured values",
                "quantiles use linear interpolation at position (n-1)p",
                "this utility reports descriptive arithmetic and does not establish causal, clinical, or population validity",
            ],
        }


def summarize_distribution(
    observations: Sequence[BenchmarkObservation],
    *,
    quantiles: Sequence[float] = DEFAULT_QUANTILES,
    bootstrap_resamples: int = 0,
    bootstrap_confidence: float = 0.95,
    bootstrap_seed: int = DEFAULT_BOOTSTRAP_SEED,
    resampling_unit: ResamplingUnit | str = ResamplingUnit.OBSERVATION,
) -> DistributionSummary:
    """Summarize a bounded typed series without admitting unmeasured evidence into arithmetic."""

    if isinstance(observations, (str, bytes)) or len(observations) > MAX_BENCHMARK_OBSERVATIONS:
        raise ArgumentError(f"observations must contain at most {MAX_BENCHMARK_OBSERVATIONS} rows")
    rows = tuple(observations)
    if any(not isinstance(row, BenchmarkObservation) for row in rows):
        raise ArgumentError("observations must contain BenchmarkObservation values")
    probabilities = _validate_quantiles(quantiles)
    total = len(rows)
    values = tuple(row.value for row in rows if row.measured and row.value is not None)
    measured_count = len(values)
    missing_count = sum(1 for row in rows if row.evidence == AnalyticsEvidence.MISSING.value)
    declared_count = sum(1 for row in rows if row.evidence == AnalyticsEvidence.DECLARED.value)
    blocked_count = sum(1 for row in rows if row.evidence == AnalyticsEvidence.BLOCKED.value)
    not_applicable_count = sum(1 for row in rows if row.evidence == AnalyticsEvidence.NOT_APPLICABLE.value)
    excluded_count = total - measured_count
    sorted_values = tuple(sorted(values))
    mean = sum(values) / measured_count if measured_count else None
    median = _quantile(sorted_values, 0.5)
    minimum = sorted_values[0] if sorted_values else None
    maximum = sorted_values[-1] if sorted_values else None
    variance = None
    if measured_count >= 2 and mean is not None:
        variance = sum((value - mean) ** 2 for value in values) / (measured_count - 1)
    standard_deviation = math.sqrt(variance) if variance is not None else None
    mad = None
    if median is not None:
        mad = _quantile(tuple(sorted(abs(value - median) for value in values)), 0.5)
    q_values = {f"{probability:.6f}": _quantile(sorted_values, probability) for probability in probabilities}
    iqr = None
    if 0.25 in probabilities and 0.75 in probabilities:
        lower_quartile = q_values["0.250000"]
        upper_quartile = q_values["0.750000"]
        if lower_quartile is not None and upper_quartile is not None:
            iqr = upper_quartile - lower_quartile
    bootstrap = None
    if bootstrap_resamples:
        groups = tuple(row.replicate_group for row in rows if row.measured and row.value is not None)
        bootstrap = bootstrap_mean(
            values,
            groups=groups,
            confidence=bootstrap_confidence,
            resamples=bootstrap_resamples,
            seed=bootstrap_seed,
            resampling_unit=resampling_unit,
        )
    values_digest = content_digest(list(values)) if values else None
    return DistributionSummary(
        total,
        measured_count,
        excluded_count,
        missing_count,
        declared_count,
        blocked_count,
        not_applicable_count,
        measured_count,
        mean,
        median,
        minimum,
        maximum,
        variance,
        standard_deviation,
        mad,
        iqr,
        q_values,
        values_digest,
        bootstrap,
    )


@dataclass(frozen=True)
class PairedEffect:
    """Paired descriptive contrast with tolerance-aware outcomes."""

    total_count: int
    measured_count: int
    excluded_count: int
    improved_count: int
    degraded_count: int
    within_tolerance_count: int
    retention: float | None
    raw_delta_mean: float | None
    oriented_delta_mean: float | None
    oriented_delta_median: float | None
    oriented_delta_minimum: float | None
    oriented_delta_maximum: float | None
    delta_digest: str | None
    bootstrap: BootstrapInterval | None
    direction: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "total_count": self.total_count,
            "measured_count": self.measured_count,
            "excluded_count": self.excluded_count,
            "improved_count": self.improved_count,
            "degraded_count": self.degraded_count,
            "within_tolerance_count": self.within_tolerance_count,
            "retention": self.retention,
            "raw_delta_mean": self.raw_delta_mean,
            "oriented_delta_mean": self.oriented_delta_mean,
            "oriented_delta_median": self.oriented_delta_median,
            "oriented_delta_minimum": self.oriented_delta_minimum,
            "oriented_delta_maximum": self.oriented_delta_maximum,
            "delta_digest": self.delta_digest,
            "bootstrap": self.bootstrap.to_dict() if self.bootstrap else None,
            "direction": self.direction,
            "limitations": [
                "delta is a paired descriptive contrast, not a causal effect",
                "tolerance is caller-declared and is not estimated from the observations",
                "rows without measured baseline and variant values are excluded from arithmetic and remain counted",
            ],
        }


def paired_effect(
    observations: Sequence[PairedBenchmarkObservation],
    *,
    bootstrap_resamples: int = 0,
    bootstrap_confidence: float = 0.95,
    bootstrap_seed: int = DEFAULT_BOOTSTRAP_SEED,
    resampling_unit: ResamplingUnit | str = ResamplingUnit.OBSERVATION,
) -> PairedEffect:
    """Compute a direction-aware paired contrast with optional deterministic bootstrap."""

    if isinstance(observations, (str, bytes)) or len(observations) > MAX_BENCHMARK_OBSERVATIONS:
        raise ArgumentError(f"observations must contain at most {MAX_BENCHMARK_OBSERVATIONS} rows")
    rows = tuple(observations)
    if any(not isinstance(row, PairedBenchmarkObservation) for row in rows):
        raise ArgumentError("observations must contain PairedBenchmarkObservation values")
    directions = {row.direction for row in rows if row.measured}
    if len(directions) > 1:
        raise ArgumentError("all measured paired rows must use one direction")
    direction = next(iter(directions), AnalyticsDirection.HIGHER_IS_BETTER.value)
    deltas = tuple(
        ((row.variant or 0.0) - (row.baseline or 0.0))
        * (1 if direction == AnalyticsDirection.HIGHER_IS_BETTER.value else -1)
        for row in rows
        if row.measured
    )
    raw_deltas = tuple((row.variant or 0.0) - (row.baseline or 0.0) for row in rows if row.measured)
    improved = sum(1 for delta, row in zip(deltas, (row for row in rows if row.measured)) if delta > row.tolerance)
    degraded = sum(1 for delta, row in zip(deltas, (row for row in rows if row.measured)) if delta < -row.tolerance)
    within = len(deltas) - improved - degraded
    sorted_deltas = tuple(sorted(deltas))
    bootstrap = None
    if bootstrap_resamples:
        groups = tuple(row.replicate_group for row in rows if row.measured)
        bootstrap = bootstrap_mean(
            deltas,
            groups=groups,
            confidence=bootstrap_confidence,
            resamples=bootstrap_resamples,
            seed=bootstrap_seed,
            resampling_unit=resampling_unit,
        )
    return PairedEffect(
        total_count=len(rows),
        measured_count=len(deltas),
        excluded_count=len(rows) - len(deltas),
        improved_count=improved,
        degraded_count=degraded,
        within_tolerance_count=within,
        retention=within / len(deltas) if deltas else None,
        raw_delta_mean=sum(raw_deltas) / len(raw_deltas) if raw_deltas else None,
        oriented_delta_mean=sum(deltas) / len(deltas) if deltas else None,
        oriented_delta_median=_quantile(sorted_deltas, 0.5),
        oriented_delta_minimum=sorted_deltas[0] if sorted_deltas else None,
        oriented_delta_maximum=sorted_deltas[-1] if sorted_deltas else None,
        delta_digest=content_digest(list(deltas)) if deltas else None,
        bootstrap=bootstrap,
        direction=direction,
    )


__all__ = [
    "BenchmarkObservation",
    "BootstrapInterval",
    "DEFAULT_BOOTSTRAP_SEED",
    "DEFAULT_QUANTILES",
    "DistributionSummary",
    "MAX_BENCHMARK_OBSERVATIONS",
    "MAX_BOOTSTRAP_RESAMPLES",
    "MIN_BOOTSTRAP_RESAMPLES",
    "PairedBenchmarkObservation",
    "PairedEffect",
    "ResamplingUnit",
    "bootstrap_mean",
    "paired_effect",
    "summarize_distribution",
]
