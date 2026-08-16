"""Typed scoring-plane audits for cross-capability evaluation.

The scoring plane is deliberately not a universal leaderboard score. It records whether each
declared dimension was measured, was not measured for a named reason, or was inapplicable to the
system's capability tier. A fold is available only when the real Rust plane policy can justify it.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_PLANE_SCHEMA = "bioprism-mcp/bioeval-plane-audit/0.1"
BIOEVAL_PLANE_TIERS = frozenset({"fixed_input_model", "workflow_pipeline", "tool_using_agent", "human_in_the_loop", "multi_agent_molecule"})
BIOEVAL_PLANE_CELL_STATES = frozenset({"scored", "unscored", "inapplicable"})
BIOEVAL_PLANE_UNSCORED_REASONS = frozenset({"not_attempted", "evaluator_unhealthy", "no_reference_standard", "sealed"})
MAX_BIOEVAL_PLANE_DIMENSIONS = 4_096
MAX_BIOEVAL_PLANE_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_PLANE_TEXT_BYTES = 4_096
MAX_BIOEVAL_PLANE_INPUT_BYTES = 20_000_000

_TIER_ORDER = {
    "fixed_input_model": 0,
    "workflow_pipeline": 1,
    "tool_using_agent": 2,
    "human_in_the_loop": 3,
    "multi_agent_molecule": 4,
}


def _text(name: str, value: Any, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str):
        raise ArgumentError(f"{name} must be a string")
    if not allow_empty and not value.strip():
        raise ArgumentError(f"{name} must be non-empty")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_PLANE_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_PLANE_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval plane response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_PLANE_SCHEMA and isinstance(candidate.get("plane"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"bioeval plane response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval plane projection")


@dataclass(frozen=True)
class BioevalPlaneDimensionArgs:
    id: str
    required: str
    weight: float = 1.0

    def __post_init__(self) -> None:
        identifier = _text("bioeval plane dimension id", self.id)
        required = _text("bioeval plane dimension required tier", self.required)
        if required not in BIOEVAL_PLANE_TIERS:
            raise ArgumentError("bioeval plane dimension required tier is not recognized")
        if isinstance(self.weight, bool) or not isinstance(self.weight, (int, float)) or not math.isfinite(float(self.weight)) or self.weight <= 0:
            raise ArgumentError("bioeval plane dimension weight must be finite and positive")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "required", required)
        object.__setattr__(self, "weight", float(self.weight))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalPlaneDimensionArgs":
        raw = _route_mapping("bioeval plane dimension", value)
        return cls(_text("bioeval plane dimension id", raw.get("id")), _text("bioeval plane dimension required tier", raw.get("required")), raw.get("weight"))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "required": self.required, "weight": self.weight}


@dataclass(frozen=True)
class BioevalPlaneCellArgs:
    state: str
    score: float | None = None
    reason: str | None = None
    evaluator: str | None = None
    note: str | None = None
    registration: str | None = None
    required: str | None = None
    declared: str | None = None

    def __post_init__(self) -> None:
        state = _text("bioeval plane cell state", self.state)
        if state not in BIOEVAL_PLANE_CELL_STATES:
            raise ArgumentError("bioeval plane cell state must be scored, unscored, or inapplicable")
        score = self.score
        if state == "scored":
            if isinstance(score, bool) or not isinstance(score, (int, float)) or not math.isfinite(float(score)) or not 0 <= float(score) <= 1:
                raise ArgumentError("bioeval scored cell score must be finite and within 0..1")
            score = float(score)
        elif score is not None:
            raise ArgumentError("only scored cells may carry a score")
        reason = None if self.reason is None else _text("bioeval plane unscored reason", self.reason)
        required = None if self.required is None else _text("bioeval plane inapplicable required tier", self.required)
        declared = None if self.declared is None else _text("bioeval plane inapplicable declared tier", self.declared)
        for name, value in (("evaluator", self.evaluator), ("note", self.note), ("registration", self.registration)):
            if value is not None:
                _text(f"bioeval plane {name}", value)
        if state == "unscored":
            if reason not in BIOEVAL_PLANE_UNSCORED_REASONS:
                raise ArgumentError("unscored cells require a recognized reason")
            if reason == "evaluator_unhealthy" and not self.evaluator:
                raise ArgumentError("evaluator_unhealthy cells require evaluator")
            if reason == "no_reference_standard" and not self.note:
                raise ArgumentError("no_reference_standard cells require note")
            if reason == "sealed" and not self.registration:
                raise ArgumentError("sealed cells require registration")
        if state == "inapplicable":
            if required not in BIOEVAL_PLANE_TIERS or declared not in BIOEVAL_PLANE_TIERS:
                raise ArgumentError("inapplicable cells require recognized required and declared tiers")
        object.__setattr__(self, "state", state)
        object.__setattr__(self, "score", score)
        object.__setattr__(self, "reason", reason)
        object.__setattr__(self, "required", required)
        object.__setattr__(self, "declared", declared)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalPlaneCellArgs":
        raw = _route_mapping("bioeval plane cell", value)
        return cls(
            _text("bioeval plane cell state", raw.get("state")),
            raw.get("score"),
            None if raw.get("reason") is None else _text("bioeval plane unscored reason", raw.get("reason")),
            None if raw.get("evaluator") is None else _text("bioeval plane evaluator", raw.get("evaluator")),
            None if raw.get("note") is None else _text("bioeval plane note", raw.get("note")),
            None if raw.get("registration") is None else _text("bioeval plane registration", raw.get("registration")),
            None if raw.get("required") is None else _text("bioeval plane inapplicable required tier", raw.get("required")),
            None if raw.get("declared") is None else _text("bioeval plane inapplicable declared tier", raw.get("declared")),
        )

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"state": self.state}
        if self.state == "scored":
            result["score"] = self.score
        elif self.state == "unscored":
            result["reason"] = self.reason
            if self.evaluator is not None:
                result["evaluator"] = self.evaluator
            if self.note is not None:
                result["note"] = self.note
            if self.registration is not None:
                result["registration"] = self.registration
        else:
            result["required"] = self.required
            result["declared"] = self.declared
        return result


@dataclass(frozen=True)
class BioevalScorePlaneArgs:
    system: str
    tier: str
    dimensions: tuple[BioevalPlaneDimensionArgs, ...]
    cells: Mapping[str, BioevalPlaneCellArgs | Mapping[str, Any]]

    def __post_init__(self) -> None:
        system = _text("bioeval plane system", self.system)
        tier = _text("bioeval plane tier", self.tier)
        if tier not in BIOEVAL_PLANE_TIERS:
            raise ArgumentError("bioeval plane tier is not recognized")
        dimensions = tuple(item if isinstance(item, BioevalPlaneDimensionArgs) else BioevalPlaneDimensionArgs.from_wire(item) for item in self.dimensions)
        if not 0 < len(dimensions) <= MAX_BIOEVAL_PLANE_DIMENSIONS:
            raise ArgumentError("bioeval plane dimensions must contain 1 to 4096 rows")
        if len({item.id for item in dimensions}) != len(dimensions):
            raise ArgumentError("bioeval plane dimension ids must be unique")
        if not isinstance(self.cells, Mapping):
            raise ArgumentError("bioeval plane cells must be an object")
        cells = {str(key): (value if isinstance(value, BioevalPlaneCellArgs) else BioevalPlaneCellArgs.from_wire(value)) for key, value in self.cells.items()}
        dimension_ids = {item.id for item in dimensions}
        if set(cells) != dimension_ids:
            raise ArgumentError("bioeval plane cells must exactly match declared dimensions")
        for dimension in dimensions:
            cell = cells[dimension.id]
            applicable = _TIER_ORDER[tier] >= _TIER_ORDER[dimension.required]
            if cell.state == "inapplicable" and applicable:
                raise ArgumentError(f"bioeval plane cell {dimension.id!r} is inapplicable despite a capable tier")
            if cell.state != "inapplicable" and not applicable:
                raise ArgumentError(f"bioeval plane cell {dimension.id!r} must be inapplicable for this tier")
            if cell.state == "inapplicable" and (cell.required != dimension.required or cell.declared != tier):
                raise ArgumentError(f"bioeval plane cell {dimension.id!r} has inconsistent tier metadata")
        object.__setattr__(self, "system", system)
        object.__setattr__(self, "tier", tier)
        object.__setattr__(self, "dimensions", dimensions)
        object.__setattr__(self, "cells", cells)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalScorePlaneArgs":
        raw = _route_mapping("bioeval score plane", value)
        raw_cells = _route_mapping("bioeval plane cells", raw.get("cells"))
        return cls(
            _text("bioeval plane system", raw.get("system")),
            _text("bioeval plane tier", raw.get("tier")),
            tuple(BioevalPlaneDimensionArgs.from_wire(item) for item in _array("bioeval plane dimensions", raw.get("dimensions"))),
            raw_cells,
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "system": self.system,
            "tier": self.tier,
            "dimensions": [item.to_wire() for item in self.dimensions],
            "cells": {key: value.to_wire() for key, value in self.cells.items()},  # type: ignore[union-attr]
        }


@dataclass(frozen=True)
class BioevalPlaneAuditArgs:
    plane: BioevalScorePlaneArgs | Mapping[str, Any]
    max_items: int = 100
    require_fold: bool = False

    def __post_init__(self) -> None:
        plane = self.plane if isinstance(self.plane, BioevalScorePlaneArgs) else BioevalScorePlaneArgs.from_wire(self.plane)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_PLANE_OUTPUT_ITEMS:
            raise ArgumentError("bioeval plane max_items must be between 1 and 1000")
        if not isinstance(self.require_fold, bool):
            raise ArgumentError("bioeval plane require_fold must be a boolean")
        object.__setattr__(self, "plane", plane)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_PLANE_INPUT_BYTES:
            raise ArgumentError("bioeval plane input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalPlaneAuditArgs":
        raw = _route_mapping("bioeval plane arguments", value)
        return cls(
            BioevalScorePlaneArgs.from_wire(raw.get("plane")),
            raw.get("max_items", 100),
            raw.get("require_fold", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"plane": self.plane.to_wire(), "max_items": self.max_items, "require_fold": self.require_fold}  # type: ignore[union-attr]


@dataclass(frozen=True)
class BioevalPlaneAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    plane: Mapping[str, Any] | None
    dimensions: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    fold: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalPlaneAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval plane refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, _route_text("bioeval plane refusal stage", raw.get("stage")), _route_text("bioeval plane refusal", raw.get("refusal")), _route_strings("bioeval plane refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval plane refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_PLANE_SCHEMA:
            raise ArgumentError("bioeval plane projection has an invalid schema")
        return cls(raw, True, BIOEVAL_PLANE_SCHEMA, _route_text("bioeval plane workflow", raw.get("workflow")), _route_mapping("bioeval plane summary", raw.get("plane")), _route_mapping("bioeval plane dimensions", raw.get("dimensions")), _route_mapping("bioeval plane findings", raw.get("findings")), _route_mapping("bioeval plane fold", raw.get("fold")), None, None, _route_strings("bioeval plane guarantees", raw.get("guarantees", [])), _route_strings("bioeval plane limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def folded(self) -> bool | None:
        return None if self.fold is None else self.fold.get("folded") if isinstance(self.fold.get("folded"), bool) else None

    @property
    def fold_value(self) -> float | None:
        if self.fold is None or not isinstance(self.fold.get("value"), (int, float)):
            return None
        return float(self.fold["value"])

    @property
    def unscored_dimensions(self) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get("unscored_dimensions"), Mapping):
            return ()
        values = self.findings["unscored_dimensions"].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_plane_audit_report(value: Mapping[str, Any]) -> BioevalPlaneAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalPlaneAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_PLANE_SCHEMA",
    "BIOEVAL_PLANE_TIERS",
    "BIOEVAL_PLANE_CELL_STATES",
    "BIOEVAL_PLANE_UNSCORED_REASONS",
    "MAX_BIOEVAL_PLANE_DIMENSIONS",
    "MAX_BIOEVAL_PLANE_OUTPUT_ITEMS",
    "MAX_BIOEVAL_PLANE_TEXT_BYTES",
    "MAX_BIOEVAL_PLANE_INPUT_BYTES",
    "BioevalPlaneDimensionArgs",
    "BioevalPlaneCellArgs",
    "BioevalScorePlaneArgs",
    "BioevalPlaneAuditArgs",
    "BioevalPlaneAuditReport",
    "bioeval_plane_audit_report",
]
