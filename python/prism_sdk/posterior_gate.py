"""Typed capability-posterior, release-gate, and dominance projections.

The posterior boundary deliberately keeps three decisions separate: the capability vector is the
primary evidence object, a release scalar is an optional and fail-closed projection of that
vector, and comparison is a capability-wise partial order rather than a leaderboard score.  This
module validates and exposes those layers without recomputing clustered statistics in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


POSTERIOR_GATE_SCHEMA = "bioprism-mcp/posterior-gate/0.1"
POSTERIOR_MAX_OBSERVATIONS = 10_000
POSTERIOR_MAX_CAPABILITIES = 1_000
POSTERIOR_DOMINANCE_KINDS = frozenset({"dominates", "dominated_by", "equivalent", "incomparable"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _nonnegative_number(name: str, value: Any) -> float:
    number = _number(name, value)
    if number < 0.0:
        raise ArgumentError(f"{name} must be non-negative")
    return number


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _texts(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a posterior report from direct MCP output or an HTTP REST envelope."""

    raw = _mapping("posterior gate response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"posterior gate response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == POSTERIOR_GATE_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a posterior gate report")


@dataclass(frozen=True)
class PosteriorGateArgs:
    """Bounded serialized observations and optional policy, gate, and comparison controls."""

    observations: tuple[dict[str, Any], ...]
    credit_policy: dict[str, Any] | None = None
    gate: dict[str, Any] | None = None
    other_observations: tuple[dict[str, Any], ...] | None = None
    tolerance: float | None = None
    min_effective: float | None = None

    def __init__(
        self,
        observations: Sequence[Mapping[str, Any]],
        credit_policy: Mapping[str, Any] | None = None,
        gate: Mapping[str, Any] | None = None,
        other_observations: Sequence[Mapping[str, Any]] | None = None,
        tolerance: float | None = None,
        min_effective: float | None = None,
    ) -> None:
        normalized_observations = tuple(_mapping(f"posterior observation[{index}]", item) for index, item in enumerate(_sequence("posterior observations", observations)))
        if len(normalized_observations) > POSTERIOR_MAX_OBSERVATIONS:
            raise ArgumentError(f"posterior observations exceeds the {POSTERIOR_MAX_OBSERVATIONS}-observation safety bound")
        normalized_other = None if other_observations is None else tuple(_mapping(f"comparison observation[{index}]", item) for index, item in enumerate(_sequence("comparison observations", other_observations)))
        if normalized_other is not None and len(normalized_other) > POSTERIOR_MAX_OBSERVATIONS:
            raise ArgumentError(f"comparison observations exceeds the {POSTERIOR_MAX_OBSERVATIONS}-observation safety bound")
        normalized_policy = None if credit_policy is None else _mapping("credit policy", credit_policy)
        normalized_gate = None if gate is None else _mapping("release gate", gate)
        normalized_tolerance = None if tolerance is None else _nonnegative_number("tolerance", tolerance)
        normalized_min_effective = None if min_effective is None else _nonnegative_number("min_effective", min_effective)
        object.__setattr__(self, "observations", normalized_observations)
        object.__setattr__(self, "credit_policy", normalized_policy)
        object.__setattr__(self, "gate", normalized_gate)
        object.__setattr__(self, "other_observations", normalized_other)
        object.__setattr__(self, "tolerance", normalized_tolerance)
        object.__setattr__(self, "min_effective", normalized_min_effective)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorGateArgs":
        raw = _mapping("posterior gate arguments", value)
        return cls(raw.get("observations"), raw.get("credit_policy"), raw.get("gate"), raw.get("other_observations"), raw.get("tolerance"), raw.get("min_effective"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"observations": list(self.observations)}
        for name, value in (("credit_policy", self.credit_policy), ("gate", self.gate)):
            if value is not None:
                result[name] = value
        if self.other_observations is not None:
            result["other_observations"] = list(self.other_observations)
        if self.tolerance is not None:
            result["tolerance"] = self.tolerance
        if self.min_effective is not None:
            result["min_effective"] = self.min_effective
        return result


@dataclass(frozen=True)
class PosteriorIccReport:
    raw: dict[str, Any]
    kind: str
    value: float | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorIccReport":
        raw = _mapping("posterior ICC", value)
        kind = _route_text("posterior ICC kind", raw.get("icc"))
        if kind == "estimated":
            return cls(raw, kind, _number("posterior ICC value", raw.get("value")), None)
        if kind == "undefined":
            return cls(raw, kind, None, _route_text("posterior ICC reason", raw.get("reason")))
        if kind == "not_applicable":
            return cls(raw, kind, 0.0, None)
        raise ArgumentError(f"unknown posterior ICC kind {kind!r}")


@dataclass(frozen=True)
class PosteriorEstimateReport:
    """One clustered mean with its naive mean, design information, and unknown share."""

    raw: dict[str, Any]
    label: str
    mean: float
    naive_instance_mean: float
    instances: int
    clusters: int
    largest_cluster: int
    icc: PosteriorIccReport
    effective_sample_size: float
    unknown_instances: int
    unknown_fraction: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorEstimateReport":
        raw = _mapping("posterior clustered estimate", value)
        return cls(
            raw,
            _route_text("posterior estimate label", raw.get("label")),
            _number("posterior estimate mean", raw.get("mean")),
            _number("posterior naive instance mean", raw.get("naive_instance_mean")),
            _integer("posterior estimate instances", raw.get("instances")),
            _integer("posterior estimate clusters", raw.get("clusters")),
            _integer("posterior estimate largest cluster", raw.get("largest_cluster")),
            PosteriorIccReport.from_wire(raw.get("icc")),
            _number("posterior effective sample size", raw.get("effective_sample_size")),
            _integer("posterior unknown instances", raw.get("unknown_instances")),
            _number("posterior unknown fraction", raw.get("unknown_fraction")),
        )

    @property
    def inflation_factor(self) -> float:
        return math.inf if self.effective_sample_size <= 0.0 else self.instances / self.effective_sample_size

    @property
    def parent_dominated(self) -> bool:
        return not math.isclose(self.mean, self.naive_instance_mean)


@dataclass(frozen=True)
class PosteriorVetoReport:
    raw: dict[str, Any]
    kind: str
    detail: str
    evaluator: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorVetoReport":
        raw = _mapping("posterior veto", value)
        return cls(raw, _route_text("posterior veto kind", raw.get("kind")), _route_text("posterior veto detail", raw.get("detail")), _route_text("posterior veto evaluator", raw.get("evaluator")))


@dataclass(frozen=True)
class PosteriorCapabilityReport:
    raw: dict[str, Any]
    capability: str
    pass_rate: PosteriorEstimateReport
    credit: PosteriorEstimateReport
    outcome_rate: PosteriorEstimateReport
    vetoes: tuple[PosteriorVetoReport, ...]
    disputed: int
    abstained: int
    optimistic_weak_evidence: int
    weakest_tier: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorCapabilityReport":
        raw = _mapping("posterior capability", value)
        return cls(
            raw,
            _route_text("posterior capability name", raw.get("capability")),
            PosteriorEstimateReport.from_wire(raw.get("pass_rate")),
            PosteriorEstimateReport.from_wire(raw.get("credit")),
            PosteriorEstimateReport.from_wire(raw.get("outcome_rate")),
            tuple(PosteriorVetoReport.from_wire(item) for item in _sequence("posterior capability vetoes", raw.get("vetoes", []))),
            _integer("posterior disputed count", raw.get("disputed")),
            _integer("posterior abstained count", raw.get("abstained")),
            _integer("posterior optimistic weak-evidence count", raw.get("optimistic_weak_evidence")),
            _route_text("posterior weakest tier", raw.get("weakest_tier")),
        )

    @property
    def unsupported_pass_gap(self) -> float:
        return self.outcome_rate.mean - self.pass_rate.mean

    @property
    def has_outstanding_veto(self) -> bool:
        return bool(self.vetoes)


@dataclass(frozen=True)
class PosteriorGateTermReport:
    raw: tuple[Any, ...]
    capability: str
    mean: float
    weight: float

    @classmethod
    def from_wire(cls, value: Any) -> "PosteriorGateTermReport":
        raw = _sequence("posterior gate term", value)
        if len(raw) != 3:
            raise ArgumentError("posterior gate terms must contain capability, mean, and weight")
        return cls(raw, _route_text("posterior gate term capability", raw[0]), _number("posterior gate term mean", raw[1]), _number("posterior gate term weight", raw[2]))


@dataclass(frozen=True)
class PosteriorSensitivityReport:
    raw: tuple[Any, ...]
    capability: str
    value: float

    @classmethod
    def from_wire(cls, value: Any) -> "PosteriorSensitivityReport":
        raw = _sequence("posterior sensitivity", value)
        if len(raw) != 2:
            raise ArgumentError("posterior sensitivity entries must contain capability and value")
        return cls(raw, _route_text("posterior sensitivity capability", raw[0]), _number("posterior sensitivity value", raw[1]))


@dataclass(frozen=True)
class PosteriorGateScalarReport:
    raw: dict[str, Any]
    gate: str
    value: float
    formula: str
    rationale: str
    terms: tuple[PosteriorGateTermReport, ...]
    sensitivity: tuple[PosteriorSensitivityReport, ...]
    weakest_tier: str
    min_effective_sample: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorGateScalarReport":
        raw = _mapping("posterior gate scalar", value)
        return cls(
            raw,
            _route_text("posterior scalar gate", raw.get("gate")),
            _number("posterior scalar value", raw.get("value")),
            _route_text("posterior scalar formula", raw.get("formula")),
            _route_text("posterior scalar rationale", raw.get("rationale")),
            tuple(PosteriorGateTermReport.from_wire(item) for item in _sequence("posterior scalar terms", raw.get("terms", []))),
            tuple(PosteriorSensitivityReport.from_wire(item) for item in _sequence("posterior scalar sensitivity", raw.get("sensitivity", []))),
            _route_text("posterior scalar weakest tier", raw.get("weakest_tier")),
            _number("posterior scalar minimum effective sample", raw.get("min_effective_sample")),
        )

    @property
    def largest_sensitivity(self) -> float:
        return max((abs(item.value - self.value) for item in self.sensitivity), default=0.0)


@dataclass(frozen=True)
class PosteriorGateDecisionReport:
    """Optional release scalar, represented as eligible or fail-closed refusal."""

    raw: dict[str, Any] | None
    state: str
    value: PosteriorGateScalarReport | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None

    @classmethod
    def absent(cls) -> "PosteriorGateDecisionReport":
        return cls(None, "absent", None, None, False, None)

    @classmethod
    def from_wire(cls, value: Any) -> "PosteriorGateDecisionReport":
        if value is None:
            return cls.absent()
        raw = _mapping("posterior gate decision", value)
        if _bool("posterior gate decision ok", raw.get("ok")):
            return cls(raw, "eligible", PosteriorGateScalarReport.from_wire(raw.get("value")), None, False, None)
        return cls(raw, "refused", None, _route_text("posterior gate refusal", raw.get("refusal")), _bool("posterior gate fail_closed", raw.get("fail_closed")), _optional_text("posterior gate guarantee", raw.get("guarantee")))

    @property
    def is_eligible(self) -> bool:
        return self.state == "eligible"


@dataclass(frozen=True)
class PosteriorComparisonReport:
    raw: dict[str, Any]
    dominance: str
    better: tuple[str, ...]
    worse: tuple[str, ...]
    uncertain: tuple[str, ...]
    tolerance: float
    min_effective: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorComparisonReport":
        raw = _mapping("posterior comparison", value)
        dominance = _route_text("posterior dominance", _mapping("posterior dominance value", raw.get("dominance")).get("dominance"))
        if dominance not in POSTERIOR_DOMINANCE_KINDS:
            raise ArgumentError(f"unknown posterior dominance {dominance!r}")
        payload = _mapping("posterior dominance value", raw.get("dominance"))
        return cls(raw, dominance, _texts("posterior better capabilities", payload.get("better", [])), _texts("posterior worse capabilities", payload.get("worse", [])), _texts("posterior uncertain capabilities", payload.get("uncertain", [])), _nonnegative_number("posterior comparison tolerance", raw.get("tolerance")), _nonnegative_number("posterior comparison minimum effective sample", raw.get("min_effective")))

    @property
    def is_incomparable(self) -> bool:
        return self.dominance == "incomparable"

    @property
    def is_decisive(self) -> bool:
        return self.dominance in {"dominates", "dominated_by"}


@dataclass(frozen=True)
class PosteriorGateReport:
    """Validated capability vector plus optional scalar and partial-order projections."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    stage: str | None
    refusal: str | None
    fail_closed: bool
    schema_version: str | None
    observations: int
    unprovenanced_observations: int
    capabilities: dict[str, PosteriorCapabilityReport]
    gate: PosteriorGateDecisionReport
    comparison: PosteriorComparisonReport | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PosteriorGateReport":
        raw = _payload(value)
        ok = _bool("posterior gate ok", raw.get("ok"))
        schema = _route_text("posterior gate schema", raw.get("schema"))
        if schema != POSTERIOR_GATE_SCHEMA:
            raise ArgumentError(f"unsupported posterior gate schema {schema!r}")
        if not ok:
            return cls(raw, False, schema, _optional_text("posterior refusal stage", raw.get("stage")), _route_text("posterior refusal", raw.get("refusal")), _bool("posterior refusal fail_closed", raw.get("fail_closed")), None, 0, 0, {}, PosteriorGateDecisionReport.absent(), None, (), ())
        capability_values = _mapping("posterior capabilities", raw.get("capabilities"))
        if len(capability_values) > POSTERIOR_MAX_CAPABILITIES:
            raise ArgumentError(f"posterior capabilities exceeds the {POSTERIOR_MAX_CAPABILITIES}-capability safety bound")
        capabilities: dict[str, PosteriorCapabilityReport] = {}
        for name, value_item in capability_values.items():
            if not isinstance(name, str) or not name.strip():
                raise ArgumentError("posterior capability keys must be non-empty strings")
            report = PosteriorCapabilityReport.from_wire(value_item)
            if report.capability != name:
                raise ArgumentError("posterior capability key does not reconcile with its report")
            capabilities[name] = report
        comparison_raw = raw.get("comparison")
        comparison = None
        if comparison_raw is not None:
            wrapper = _mapping("posterior comparison wrapper", comparison_raw)
            if not _bool("posterior comparison ok", wrapper.get("ok")):
                raise ArgumentError("posterior comparison refusals must be returned as top-level reports")
            comparison = PosteriorComparisonReport.from_wire(wrapper)
        return cls(raw, True, schema, None, None, False, _route_text("posterior schema version", raw.get("schema_version")), _integer("posterior observations", raw.get("observations")), _integer("posterior unprovenanced observations", raw.get("unprovenanced_observations")), capabilities, PosteriorGateDecisionReport.from_wire(raw.get("gate")), comparison, _texts("posterior guarantees", raw.get("guarantees", [])), _texts("posterior limitations", raw.get("limitations", [])))

    @property
    def has_provenance_gaps(self) -> bool:
        return self.unprovenanced_observations > 0

    @property
    def release_is_eligible(self) -> bool:
        return self.ok and self.gate.is_eligible

    @property
    def has_outstanding_veto(self) -> bool:
        return any(report.has_outstanding_veto for report in self.capabilities.values())

    @property
    def comparison_is_incomparable(self) -> bool:
        return self.comparison is not None and self.comparison.is_incomparable

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def posterior_gate_report(value: Mapping[str, Any]) -> PosteriorGateReport:
    """Parse a direct MCP result or HTTP envelope into a typed posterior report."""

    return PosteriorGateReport.from_wire(value)


__all__ = [
    "POSTERIOR_GATE_SCHEMA",
    "POSTERIOR_MAX_OBSERVATIONS",
    "POSTERIOR_MAX_CAPABILITIES",
    "PosteriorGateArgs",
    "PosteriorIccReport",
    "PosteriorEstimateReport",
    "PosteriorVetoReport",
    "PosteriorCapabilityReport",
    "PosteriorGateTermReport",
    "PosteriorSensitivityReport",
    "PosteriorGateScalarReport",
    "PosteriorGateDecisionReport",
    "PosteriorComparisonReport",
    "PosteriorGateReport",
    "posterior_gate_report",
]
