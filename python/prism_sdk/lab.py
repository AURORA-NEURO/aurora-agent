"""Typed inference-lab acquisition planning contracts.

The lab kernel orders caller-declared acquisitions.  It does not execute them, invent value or
cost models, or turn a privacy crossing into a low-scoring candidate.  This projection keeps the
ordered/excluded/stop/escalation evidence separate and preserves structured planning refusals.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .domain_requests import LabPlanRequest
from .errors import ArgumentError


LAB_STOP_REASONS = frozenset(
    {
        "obligations_discharged",
        "decision_robust_across_hypotheses",
        "marginal_value_below_floor",
        "budget_exhausted",
        "evidence_unreachable",
        "all_actions_planned",
    }
)
LAB_EXCLUSION_REASONS = frozenset(
    {"crosses_boundary", "targets_nothing_outstanding", "below_marginal_value_floor", "budget_exhausted"}
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab plan response", value)
    if "ok" in raw and any(key in raw for key in ("goal", "stage", "ordered")):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and "ok" in structured:
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"lab plan response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded lab plan response", decoded)
                if "ok" in decoded_mapping:
                    return decoded_mapping
    raise ArgumentError("response does not contain a lab plan projection")


@dataclass(frozen=True)
class LabStopReport:
    raw: dict[str, Any]
    reason: str
    action: str | None
    ratio: float | None
    surviving: str | None
    outstanding: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabStopReport":
        raw = _route_mapping("lab stop", value)
        reason = _route_text("lab stop reason", raw.get("stopped_because"))
        if reason not in LAB_STOP_REASONS:
            raise ArgumentError(f"unknown lab stop reason: {reason!r}")
        action = _optional_text("lab stop action", raw.get("action"))
        surviving = _optional_text("lab stop surviving", raw.get("surviving"))
        outstanding_value = raw.get("outstanding", [])
        outstanding = _route_strings("lab stop outstanding", outstanding_value)
        ratio = raw.get("ratio")
        if ratio is not None:
            if isinstance(ratio, bool) or not isinstance(ratio, (int, float)):
                raise ArgumentError("lab stop ratio must be numeric")
            ratio = float(ratio)
        if reason == "marginal_value_below_floor" and (action is None or ratio is None):
            raise ArgumentError("marginal-value stop must name its action and ratio")
        if reason == "decision_robust_across_hypotheses" and surviving is None:
            raise ArgumentError("robust-decision stop must name the surviving hypothesis")
        if reason == "evidence_unreachable" and not outstanding:
            raise ArgumentError("unreachable-evidence stop must name outstanding obligations")
        return cls(raw, reason, action, ratio, surviving, outstanding)


@dataclass(frozen=True)
class LabPlannedAcquisitionReport:
    raw: dict[str, Any]
    action: str
    kind: Mapping[str, Any]
    targets: tuple[str, ...]
    value_per_unit_cost: float
    cost: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabPlannedAcquisitionReport":
        raw = _route_mapping("lab planned acquisition", value)
        ratio = raw.get("value_per_unit_cost")
        if isinstance(ratio, bool) or not isinstance(ratio, (int, float)):
            raise ArgumentError("lab planned acquisition value_per_unit_cost must be numeric")
        return cls(
            raw,
            _route_text("lab planned action", raw.get("action")),
            _route_mapping("lab planned kind", raw.get("kind")),
            _route_strings("lab planned targets", raw.get("targets")),
            float(ratio),
            _route_mapping("lab planned cost", raw.get("cost")),
        )


@dataclass(frozen=True)
class LabExcludedActionReport:
    raw: tuple[Any, ...]
    action: str
    reason: Mapping[str, Any]
    reason_kind: str

    @classmethod
    def from_wire(cls, value: Any) -> "LabExcludedActionReport":
        if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) != 2:
            raise ArgumentError("lab excluded action must be a [action, reason] pair")
        action = _route_text("lab excluded action", value[0])
        reason = _route_mapping("lab exclusion reason", value[1])
        kind = _route_text("lab exclusion reason kind", reason.get("excluded_because"))
        if kind not in LAB_EXCLUSION_REASONS:
            raise ArgumentError(f"unknown lab exclusion reason: {kind!r}")
        return cls(tuple(value), action, reason, kind)


@dataclass(frozen=True)
class LabPlanReport:
    raw: dict[str, Any]
    ok: bool
    goal: str | None
    obligation_count: int | None
    frontier: tuple[Mapping[str, Any], ...]
    omitted_frontier: int
    separation: Mapping[str, Any] | None
    ordered: tuple[LabPlannedAcquisitionReport, ...]
    omitted_ordered: int
    excluded: tuple[LabExcludedActionReport, ...]
    omitted_excluded: int
    spent: Mapping[str, Any] | None
    stop: LabStopReport | None
    should_escalate: bool | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabPlanReport":
        raw = _payload(value)
        ok = _bool("lab plan ok", raw.get("ok"))
        fail_closed = _bool("lab plan fail_closed", raw.get("fail_closed", False))
        stage = _optional_text("lab plan stage", raw.get("stage"))
        refusal = _optional_text("lab plan refusal", raw.get("refusal"))
        guarantee = _optional_text("lab plan guarantee", raw.get("guarantee"))
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("refused lab plans require a fail-closed refusal")
            return cls(raw, False, None, None, (), 0, None, (), 0, (), 0, None, None, None, stage, refusal, True, guarantee, (), ())
        if fail_closed or refusal is not None or stage is not None:
            raise ArgumentError("successful lab plans cannot carry refusal evidence")
        frontier = tuple(_route_mapping(f"lab frontier[{index}]", item) for index, item in enumerate(_array("lab frontier", raw.get("frontier"))))
        omitted_frontier = _route_count("lab omitted_frontier", raw.get("omitted_frontier"))
        ordered = tuple(LabPlannedAcquisitionReport.from_wire(item) for item in _array("lab ordered", raw.get("ordered")))
        omitted_ordered = _route_count("lab omitted_ordered", raw.get("omitted_ordered"))
        excluded = tuple(LabExcludedActionReport.from_wire(item) for item in _array("lab excluded", raw.get("excluded")))
        omitted_excluded = _route_count("lab omitted_excluded", raw.get("omitted_excluded"))
        stop = LabStopReport.from_wire(raw.get("stop"))
        should_escalate = _bool("lab should_escalate", raw.get("should_escalate"))
        if should_escalate != (stop.reason == "evidence_unreachable"):
            raise ArgumentError("lab should_escalate does not reconcile with stop reason")
        separation_value = raw.get("separation")
        separation = None if separation_value is None else _route_mapping("lab separation", separation_value)
        return cls(
            raw,
            True,
            _route_text("lab goal", raw.get("goal")),
            _route_count("lab obligation_count", raw.get("obligation_count")),
            frontier,
            omitted_frontier,
            separation,
            ordered,
            omitted_ordered,
            excluded,
            omitted_excluded,
            _route_mapping("lab spent", raw.get("spent")),
            stop,
            should_escalate,
            None,
            None,
            False,
            None,
            _route_strings("lab guarantees", raw.get("guarantees")),
            _route_strings("lab limitations", raw.get("limitations")),
        )

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def execution_started(self) -> bool:
        return False


def lab_plan_report(value: Mapping[str, Any]) -> LabPlanReport:
    """Parse direct MCP or HTTP lab-plan output."""

    return LabPlanReport.from_wire(value)


__all__ = [
    "LAB_EXCLUSION_REASONS",
    "LAB_STOP_REASONS",
    "LabExcludedActionReport",
    "LabPlanReport",
    "LabPlanRequest",
    "LabPlannedAcquisitionReport",
    "LabStopReport",
    "lab_plan_report",
]
