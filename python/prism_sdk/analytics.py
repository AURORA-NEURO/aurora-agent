"""Typed request models for the Rust metrics analytics kernel.

The models validate transport shape and numeric bounds locally, then preserve the exact Rust wire
contract. They do not reproduce any scoring or inference logic; ``metrics_analytics_audit`` remains
the authoritative implementation of summaries, paired contrasts, and calibration arithmetic.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import math
from typing import Any, Mapping, Sequence

from .errors import ArgumentError


class AnalyticsDirection(str, Enum):
    HIGHER_IS_BETTER = "higher_is_better"
    LOWER_IS_BETTER = "lower_is_better"


class AnalyticsEvidence(str, Enum):
    OBSERVED = "observed"
    REPRODUCED = "reproduced"
    DECLARED = "declared"
    MISSING = "missing"
    BLOCKED = "blocked"
    NOT_APPLICABLE = "not_applicable"


def _text(name: str, value: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    return value


def _number(name: str, value: float, *, nonnegative: bool = False) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise ArgumentError(f"{name} must be a number")
    value = float(value)
    if not math.isfinite(value):
        raise ArgumentError(f"{name} must be finite")
    if nonnegative and value < 0:
        raise ArgumentError(f"{name} must be non-negative")
    return value


def _probability(name: str, value: float) -> float:
    value = _number(name, value)
    if not 0 <= value <= 1:
        raise ArgumentError(f"{name} must be between 0 and 1")
    return value


def _enum_value(name: str, value: Enum | str, enum: type[Enum]) -> str:
    try:
        return (value if isinstance(value, enum) else enum(value)).value
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not a valid {enum.__name__}") from error


def _wire(value: Mapping[str, Any] | Any) -> dict[str, Any]:
    if hasattr(value, "to_dict"):
        return dict(value.to_dict())
    if not isinstance(value, Mapping):
        raise ArgumentError("analytics rows must be mappings or typed analytics models")
    return dict(value)


@dataclass(frozen=True)
class MetricObservation:
    id: str
    dimension: str
    domain: str
    system: str
    value: float
    direction: AnalyticsDirection | str
    unit: str
    condition: str
    replicate_group: str | None = None
    cost: float | None = None
    latency_ms: float | None = None
    evidence: AnalyticsEvidence | str = AnalyticsEvidence.OBSERVED

    def __post_init__(self) -> None:
        for name in ("id", "dimension", "domain", "system", "unit", "condition"):
            _text(name, getattr(self, name))
        _number("value", self.value)
        _enum_value("direction", self.direction, AnalyticsDirection)
        _enum_value("evidence", self.evidence, AnalyticsEvidence)
        if self.replicate_group is not None:
            _text("replicate_group", self.replicate_group)
        if self.cost is not None:
            _number("cost", self.cost, nonnegative=True)
        if self.latency_ms is not None:
            _number("latency_ms", self.latency_ms, nonnegative=True)

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "dimension": self.dimension,
            "domain": self.domain,
            "system": self.system,
            "value": float(self.value),
            "direction": _enum_value("direction", self.direction, AnalyticsDirection),
            "unit": self.unit,
            "condition": self.condition,
            "evidence": _enum_value("evidence", self.evidence, AnalyticsEvidence),
        }
        if self.replicate_group is not None:
            result["replicate_group"] = self.replicate_group
        if self.cost is not None:
            result["cost"] = float(self.cost)
        if self.latency_ms is not None:
            result["latency_ms"] = float(self.latency_ms)
        return result


@dataclass(frozen=True)
class PairedObservation:
    id: str
    dimension: str
    domain: str
    baseline: float
    variant: float
    direction: AnalyticsDirection | str
    tolerance: float
    evidence: AnalyticsEvidence | str = AnalyticsEvidence.OBSERVED

    def __post_init__(self) -> None:
        for name in ("id", "dimension", "domain"):
            _text(name, getattr(self, name))
        _number("baseline", self.baseline)
        _number("variant", self.variant)
        _number("tolerance", self.tolerance, nonnegative=True)
        _enum_value("direction", self.direction, AnalyticsDirection)
        _enum_value("evidence", self.evidence, AnalyticsEvidence)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "dimension": self.dimension,
            "domain": self.domain,
            "baseline": float(self.baseline),
            "variant": float(self.variant),
            "direction": _enum_value("direction", self.direction, AnalyticsDirection),
            "tolerance": float(self.tolerance),
            "evidence": _enum_value("evidence", self.evidence, AnalyticsEvidence),
        }


@dataclass(frozen=True)
class CalibrationObservation:
    id: str
    domain: str
    predicted: float
    observed: float
    evidence: AnalyticsEvidence | str = AnalyticsEvidence.OBSERVED
    group: str | None = None

    def __post_init__(self) -> None:
        _text("id", self.id)
        _text("domain", self.domain)
        _probability("predicted", self.predicted)
        _probability("observed", self.observed)
        _enum_value("evidence", self.evidence, AnalyticsEvidence)
        if self.group is not None:
            _text("group", self.group)

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "domain": self.domain,
            "predicted": float(self.predicted),
            "observed": float(self.observed),
            "evidence": _enum_value("evidence", self.evidence, AnalyticsEvidence),
        }
        if self.group is not None:
            result["group"] = self.group
        return result


@dataclass(frozen=True)
class AnalyticsRequest:
    observations: tuple[MetricObservation | Mapping[str, Any], ...]
    pairs: tuple[PairedObservation | Mapping[str, Any], ...] = ()
    calibration: tuple[CalibrationObservation | Mapping[str, Any], ...] = ()
    calibration_bins: int = 10

    def __post_init__(self) -> None:
        if not isinstance(self.calibration_bins, int) or isinstance(self.calibration_bins, bool):
            raise ArgumentError("calibration_bins must be an integer")
        if not 2 <= self.calibration_bins <= 100:
            raise ArgumentError("calibration_bins must be between 2 and 100")
        for row in (*self.observations, *self.pairs, *self.calibration):
            _wire(row)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "observations": [_wire(row) for row in self.observations],
            "pairs": [_wire(row) for row in self.pairs],
            "calibration": [_wire(row) for row in self.calibration],
            "calibration_bins": self.calibration_bins,
        }


def analytics_request(
    observations: Sequence[MetricObservation | Mapping[str, Any]],
    *,
    pairs: Sequence[PairedObservation | Mapping[str, Any]] = (),
    calibration: Sequence[CalibrationObservation | Mapping[str, Any]] = (),
    calibration_bins: int = 10,
) -> AnalyticsRequest:
    """Construct an immutable analytics request with local transport validation."""

    return AnalyticsRequest(tuple(observations), tuple(pairs), tuple(calibration), calibration_bins)


__all__ = [
    "AnalyticsDirection",
    "AnalyticsEvidence",
    "MetricObservation",
    "PairedObservation",
    "CalibrationObservation",
    "AnalyticsRequest",
    "analytics_request",
]
