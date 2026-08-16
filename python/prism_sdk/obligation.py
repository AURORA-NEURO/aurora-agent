"""Typed boundary for fail-closed action gates over dependency-aware obligations."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_text
from .errors import ArgumentError


OBLIGATION_GATE_SCHEMA = "bioprism-mcp/obligation-gate-check/0.1"
OBLIGATION_GATE_OUTCOME_KINDS = frozenset({"allowed", "blocked"})


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


@dataclass(frozen=True)
class ObligationGateCheckArgs:
    """Serialized obligation graph and action; Rust owns their exact schemas and semantics."""

    graph: Mapping[str, Any]
    action: Mapping[str, Any]
    max_items: int | None = None

    def __post_init__(self) -> None:
        graph = _object("obligation graph", self.graph)
        action = _object("obligation action", self.action)
        max_items = 100 if self.max_items is None else self.max_items
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= 1_000:
            raise ArgumentError("obligation gate max_items must be an integer between 1 and 1000")
        object.__setattr__(self, "graph", graph)
        object.__setattr__(self, "action", action)
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ObligationGateCheckArgs":
        raw = _object("obligation gate arguments", value)
        return cls(raw.get("graph"), raw.get("action"), raw.get("max_items"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"graph": dict(self.graph), "action": dict(self.action), "max_items": self.max_items}


@dataclass(frozen=True)
class ObligationGateCheckReport:
    raw: dict[str, Any]
    ok: bool
    outcome_kind: str
    allowed: bool
    goal: str
    action: dict[str, Any]
    gate: dict[str, Any]
    refusal: Mapping[str, Any] | None
    graph: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ObligationGateCheckReport":
        raw = _object("obligation gate report", value)
        if raw.get("ok") is not True:
            raise ArgumentError("obligation gate report transport projection is not successful")
        if raw.get("schema") != OBLIGATION_GATE_SCHEMA:
            raise ArgumentError(f"unknown obligation gate schema: {raw.get('schema')!r}")
        outcome_kind = _route_text("obligation gate outcome kind", raw.get("outcome_kind"))
        if outcome_kind not in OBLIGATION_GATE_OUTCOME_KINDS:
            raise ArgumentError(f"unknown obligation gate outcome kind: {outcome_kind!r}")
        allowed = raw.get("allowed")
        if not isinstance(allowed, bool) or allowed != (outcome_kind == "allowed"):
            raise ArgumentError("obligation gate outcome and allowed flag do not reconcile")
        goal = _route_text("obligation gate goal", raw.get("goal"))
        action = _route_mapping("obligation gate action", raw.get("action"))
        gate = _route_mapping("obligation gate decision", raw.get("gate"))
        graph = _route_mapping("obligation gate graph projection", raw.get("graph"))
        refusal_value = raw.get("refusal")
        refusal = None if refusal_value is None else _route_mapping("obligation gate refusal", refusal_value)
        if allowed and refusal is not None:
            raise ArgumentError("allowed obligation gates must not carry a refusal")
        if not allowed and refusal is None:
            raise ArgumentError("blocked obligation gates must retain a typed refusal")
        return cls(raw, True, outcome_kind, allowed, goal, action, gate, refusal, graph)


def obligation_gate_check_report(value: Mapping[str, Any]) -> ObligationGateCheckReport:
    return ObligationGateCheckReport.from_wire(value)


__all__ = [
    "OBLIGATION_GATE_SCHEMA",
    "OBLIGATION_GATE_OUTCOME_KINDS",
    "ObligationGateCheckArgs",
    "ObligationGateCheckReport",
    "obligation_gate_check_report",
]
