"""Typed oncology research-boundary contracts.

The OncoWorld boundary is a splitter: safe aggregate research may be released while individual
clinical use is refused and routed to a human process.  This module preserves that partial-release
state and the direct-identifier fail-closed refusal without offering any clinical recommendation.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ONCO_OUTPUT_USES = frozenset(
    {
        "cohort_analysis",
        "method_development",
        "hypothesis_generation",
        "quality_control",
        "individual_diagnosis",
        "individual_prognosis",
        "treatment_recommendation",
        "care_triage",
        "clinical_alerting",
    }
)
ONCO_DISPOSITIONS = frozenset({"release_in_full", "release_partial", "refuse_and_escalate"})
ONCO_TERMINAL_ACTIONS = frozenset({"stop", "abstain", "escalate"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("oncology boundary response", value)
    if "ok" in raw and any(key in raw for key in ("disposition", "stage", "permitted")):
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
                    raise ArgumentError(f"oncology boundary response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded oncology boundary response", decoded)
                if "ok" in decoded_mapping:
                    return decoded_mapping
    raise ArgumentError("response does not contain an oncology boundary projection")


@dataclass(frozen=True)
class OncoBoundaryArgs:
    request: Mapping[str, Any]
    boundary: Mapping[str, Any] | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryArgs":
        raw = _route_mapping("oncology boundary arguments", value)
        request = _route_mapping("oncology boundary request", raw.get("request"))
        boundary_value = raw.get("boundary")
        boundary = None if boundary_value is None else _route_mapping("oncology boundary policy", boundary_value)
        return cls(request, boundary)

    def __post_init__(self) -> None:
        request = _route_mapping("oncology boundary request", self.request)
        uses = _array("oncology requested_uses", request.get("requested_uses", []))
        if len(uses) > 100:
            raise ArgumentError("oncology boundary request exceeds the 100-use safety bound")
        for index, use in enumerate(uses):
            name = _route_text(f"oncology requested_uses[{index}]", use)
            if name not in ONCO_OUTPUT_USES:
                raise ArgumentError(f"unknown oncology output use: {name!r}")
        object.__setattr__(self, "request", request)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"request": dict(self.request)}
        if self.boundary is not None:
            result["boundary"] = dict(self.boundary)
        return result


@dataclass(frozen=True)
class OncoEscalationReport:
    raw: dict[str, Any]
    trigger: str
    route: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoEscalationReport":
        raw = _route_mapping("oncology escalation", value)
        trigger = _route_text("oncology escalation trigger", raw.get("trigger"))
        route = _route_text("oncology escalation route", raw.get("route"))
        return cls(raw, trigger, route)


@dataclass(frozen=True)
class OncoBoundaryDispositionReport:
    raw: dict[str, Any]
    kind: str
    released: tuple[str, ...]
    refused: tuple[str, ...]
    escalation: OncoEscalationReport | None
    terminal_action: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryDispositionReport":
        raw = _route_mapping("oncology disposition", value)
        kind = _route_text("oncology disposition kind", raw.get("disposition"))
        if kind not in ONCO_DISPOSITIONS:
            raise ArgumentError(f"unknown oncology disposition: {kind!r}")
        uses = lambda name: tuple(
            _route_text(f"oncology disposition {name}[{index}]", item)
            for index, item in enumerate(_array(f"oncology disposition {name}", raw.get(name, [])))
        )
        if kind == "release_in_full":
            released = uses("uses")
            refused: tuple[str, ...] = ()
            escalation = None
            terminal_action = "abstain"
        elif kind == "release_partial":
            released = uses("released")
            refused = uses("refused")
            escalation_value = _route_mapping("oncology disposition escalation", raw.get("escalation"))
            escalation = OncoEscalationReport.from_wire(escalation_value)
            terminal_action = "escalate"
        else:
            released = ()
            refused = uses("refused")
            escalation_value = _route_mapping("oncology disposition escalation", raw.get("escalation"))
            escalation = OncoEscalationReport.from_wire(escalation_value)
            terminal_action = "stop"
        for name in released + refused:
            if name not in ONCO_OUTPUT_USES:
                raise ArgumentError(f"unknown oncology disposition use: {name!r}")
        return cls(raw, kind, released, refused, escalation, terminal_action)


@dataclass(frozen=True)
class OncoBoundaryReport:
    raw: dict[str, Any]
    ok: bool
    permitted: tuple[str, ...]
    disposition: OncoBoundaryDispositionReport | None
    released: tuple[str, ...]
    refused: tuple[str, ...]
    terminal_action: str | None
    escalation: OncoEscalationReport | None
    research_statement: str | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryReport":
        raw = _payload(value)
        ok = _bool("oncology boundary ok", raw.get("ok"))
        fail_closed = _bool("oncology boundary fail_closed", raw.get("fail_closed", False))
        stage = None if raw.get("stage") is None else _route_text("oncology boundary stage", raw.get("stage"))
        refusal = None if raw.get("refusal") is None else _route_text("oncology boundary refusal", raw.get("refusal"))
        guarantee = None if raw.get("guarantee") is None else _route_text("oncology boundary guarantee", raw.get("guarantee"))
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("refused oncology boundary results require a fail-closed refusal")
            return cls(raw, False, (), None, (), (), None, None, None, stage, refusal, True, guarantee, (), ())
        if fail_closed or refusal is not None or stage is not None:
            raise ArgumentError("successful oncology boundary results cannot carry refusal evidence")
        permitted = _route_strings("oncology permitted uses", raw.get("permitted"))
        if any(use not in ONCO_OUTPUT_USES for use in permitted):
            raise ArgumentError("oncology permitted contains an unknown output use")
        disposition = OncoBoundaryDispositionReport.from_wire(raw.get("disposition"))
        released = _route_strings("oncology released uses", raw.get("released"))
        refused = _route_strings("oncology refused uses", raw.get("refused"))
        terminal_action = _route_text("oncology terminal action", raw.get("terminal_action"))
        if terminal_action not in ONCO_TERMINAL_ACTIONS or terminal_action != disposition.terminal_action:
            raise ArgumentError("oncology terminal action does not reconcile with disposition")
        if released != disposition.released or refused != disposition.refused:
            raise ArgumentError("oncology released/refused projections do not reconcile with disposition")
        escalation_value = raw.get("escalation")
        escalation = None if escalation_value is None else OncoEscalationReport.from_wire(escalation_value)
        if (escalation is None) != (disposition.escalation is None):
            raise ArgumentError("oncology escalation does not reconcile with disposition")
        return cls(
            raw,
            True,
            permitted,
            disposition,
            released,
            refused,
            terminal_action,
            escalation,
            _route_text("oncology research statement", raw.get("research_statement")),
            None,
            None,
            False,
            None,
            _route_strings("oncology guarantees", raw.get("guarantees")),
            _route_strings("oncology limitations", raw.get("limitations")),
        )

    @property
    def refused_individual_use(self) -> bool:
        return any(use.startswith("individual_") or use in {"treatment_recommendation", "care_triage", "clinical_alerting"} for use in self.refused)

    @property
    def research_only(self) -> bool:
        return self.ok and not self.refused_individual_use


def onco_boundary_report(value: Mapping[str, Any]) -> OncoBoundaryReport:
    """Parse direct MCP or HTTP oncology-boundary output."""

    return OncoBoundaryReport.from_wire(value)


__all__ = [
    "ONCO_DISPOSITIONS",
    "ONCO_OUTPUT_USES",
    "ONCO_TERMINAL_ACTIONS",
    "OncoBoundaryArgs",
    "OncoBoundaryDispositionReport",
    "OncoBoundaryReport",
    "OncoEscalationReport",
    "onco_boundary_report",
]
