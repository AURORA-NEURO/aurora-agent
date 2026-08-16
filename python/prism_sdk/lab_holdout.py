"""Typed holdout-contamination and rollback-audit requests and projections."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LAB_HOLDOUT_SCHEMA = "bioprism-mcp/lab-holdout-audit/0.1"
LAB_HOLDOUT_OPERATION_KINDS = frozenset({"checkpoint", "promote", "search", "measure", "rollback"})
MAX_LAB_HOLDOUT_CANDIDATES = 512
MAX_LAB_HOLDOUTS = 128
MAX_LAB_HOLDOUT_OPERATIONS = 2_000
MAX_LAB_HOLDOUT_ROWS = 1_000
MAX_LAB_HOLDOUT_INPUT_BYTES = 10_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab holdout response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == LAB_HOLDOUT_SCHEMA and isinstance(candidate.get("holdouts"), Sequence)
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
                        raise ArgumentError(f"lab holdout response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a lab holdout projection")


@dataclass(frozen=True)
class LabHoldoutAuditArgs:
    cost_ceiling: int
    candidates: tuple[Mapping[str, Any], ...]
    holdouts: tuple[Mapping[str, Any], ...]
    current: str
    operations: tuple[Mapping[str, Any], ...]
    max_rows: int = 100

    def __post_init__(self) -> None:
        if not isinstance(self.cost_ceiling, int) or isinstance(self.cost_ceiling, bool) or not 0 <= self.cost_ceiling <= 1_000_000_000:
            raise ArgumentError("lab holdout cost_ceiling must be between 0 and 1000000000")
        candidates = tuple(_route_mapping(f"lab holdout candidates[{index}]", item) for index, item in enumerate(_array("lab holdout candidates", self.candidates)))
        if not 1 <= len(candidates) <= MAX_LAB_HOLDOUT_CANDIDATES:
            raise ArgumentError("lab holdout candidates must contain between 1 and 512 objects")
        for index, candidate in enumerate(candidates):
            identifier = _route_text(f"lab holdout candidates[{index}].id", candidate.get("id"))
            if len(identifier.encode("utf-8")) > 512:
                raise ArgumentError("lab holdout candidate ids must contain at most 512 bytes")
        holdouts = tuple(_route_mapping(f"lab holdout holdouts[{index}]", item) for index, item in enumerate(_array("lab holdout holdouts", self.holdouts)))
        if not 1 <= len(holdouts) <= MAX_LAB_HOLDOUTS:
            raise ArgumentError("lab holdout holdouts must contain between 1 and 128 objects")
        for index, holdout in enumerate(holdouts):
            identifier = _route_text(f"lab holdout holdouts[{index}].id", holdout.get("id"))
            _route_text(f"lab holdout holdouts[{index}].partition", holdout.get("partition"))
            budget = holdout.get("query_budget")
            if not isinstance(budget, int) or isinstance(budget, bool) or budget < 0:
                raise ArgumentError(f"lab holdout holdouts[{index}].query_budget must be a non-negative integer")
            if len(identifier.encode("utf-8")) > 512:
                raise ArgumentError("lab holdout identifiers must contain at most 512 bytes")
        current = _route_text("lab holdout current", self.current)
        operations = tuple(_route_mapping(f"lab holdout operations[{index}]", item) for index, item in enumerate(_array("lab holdout operations", self.operations)))
        if not 1 <= len(operations) <= MAX_LAB_HOLDOUT_OPERATIONS:
            raise ArgumentError("lab holdout operations must contain between 1 and 2000 objects")
        for index, operation in enumerate(operations):
            kind = _route_text(f"lab holdout operations[{index}].kind", operation.get("kind"))
            if kind not in LAB_HOLDOUT_OPERATION_KINDS:
                raise ArgumentError(f"lab holdout operation kind {kind!r} is not recognized")
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_LAB_HOLDOUT_ROWS:
            raise ArgumentError("lab holdout max_rows must be between 1 and 1000")
        arguments = {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in candidates],
            "holdouts": [dict(item) for item in holdouts],
            "current": current,
            "operations": [dict(item) for item in operations],
            "max_rows": self.max_rows,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"lab holdout arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_LAB_HOLDOUT_INPUT_BYTES:
            raise ArgumentError("lab holdout input exceeds the 10000000-byte safety bound")
        object.__setattr__(self, "candidates", candidates)
        object.__setattr__(self, "holdouts", holdouts)
        object.__setattr__(self, "current", current)
        object.__setattr__(self, "operations", operations)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabHoldoutAuditArgs":
        raw = _route_mapping("lab holdout arguments", value)
        return cls(
            raw.get("cost_ceiling"),
            tuple(_route_mapping(f"lab holdout candidates[{index}]", item) for index, item in enumerate(_array("lab holdout candidates", raw.get("candidates")))),
            tuple(_route_mapping(f"lab holdout holdouts[{index}]", item) for index, item in enumerate(_array("lab holdout holdouts", raw.get("holdouts")))),
            raw.get("current"),
            tuple(_route_mapping(f"lab holdout operations[{index}]", item) for index, item in enumerate(_array("lab holdout operations", raw.get("operations")))),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in self.candidates],
            "holdouts": [dict(item) for item in self.holdouts],
            "current": self.current,
            "operations": [dict(item) for item in self.operations],
            "max_rows": self.max_rows,
        }


@dataclass(frozen=True)
class LabHoldoutAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    current: str | None
    space: Mapping[str, Any] | None
    holdouts: tuple[Mapping[str, Any], ...]
    checkpoints: tuple[Mapping[str, Any], ...]
    history: tuple[Mapping[str, Any], ...]
    operations: tuple[Mapping[str, Any], ...]
    operations_omitted: int
    operation_count: int | None
    measurement_count: int
    measurement_refusal_count: int
    rollback_count: int
    permanently_burned: tuple[Mapping[str, Any], ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabHoldoutAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("lab holdout refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, (), (), (), (), 0, None, 0, 0, 0, (), _route_strings("lab holdout refusal guarantees", raw.get("guarantees", [])), (), _route_text("lab holdout refusal stage", raw.get("stage")), _route_text("lab holdout refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != LAB_HOLDOUT_SCHEMA:
            raise ArgumentError("lab holdout projection has an invalid schema")
        current = _route_text("lab holdout current", raw.get("current"))
        space = _route_mapping("lab holdout space", raw.get("space"))
        _route_count("lab holdout candidate count", space.get("candidate_count"))
        holdouts = tuple(_route_mapping("lab holdout row", item) for item in _array("lab holdout holdouts", raw.get("holdouts", [])))
        if not 1 <= len(holdouts) <= MAX_LAB_HOLDOUTS:
            raise ArgumentError("lab holdout report contains an invalid holdout count")
        checkpoints = tuple(_route_mapping("lab holdout checkpoint", item) for item in _array("lab holdout checkpoints", raw.get("checkpoints", [])))
        history = tuple(_route_mapping("lab holdout history row", item) for item in _array("lab holdout history", raw.get("history", [])))
        operations = tuple(_route_mapping("lab holdout operation row", item) for item in _array("lab holdout operations", raw.get("operations", [])))
        operations_omitted = _route_count("lab holdout operations omitted", raw.get("operations_omitted"))
        operation_count = _route_count("lab holdout operation count", raw.get("operation_count"))
        if len(operations) + operations_omitted != operation_count:
            raise ArgumentError("lab holdout operation rows do not reconcile with operation_count")
        measurement_count = _route_count("lab holdout measurement count", raw.get("measurement_count"))
        measurement_refusal_count = _route_count("lab holdout measurement refusal count", raw.get("measurement_refusal_count"))
        rollback_count = _route_count("lab holdout rollback count", raw.get("rollback_count"))
        permanently_burned = tuple(_route_mapping("lab holdout burned row", item) for item in _array("lab holdout permanently burned", raw.get("permanently_burned", [])))
        max_rows = _route_count("lab holdout max_rows", raw.get("max_rows"))
        if not 1 <= max_rows <= MAX_LAB_HOLDOUT_ROWS:
            raise ArgumentError("lab holdout max_rows is outside the declared bounds")
        return cls(raw, True, LAB_HOLDOUT_SCHEMA, current, space, holdouts, checkpoints, history, operations, operations_omitted, operation_count, measurement_count, measurement_refusal_count, rollback_count, permanently_burned, _route_strings("lab holdout guarantees", raw.get("guarantees", [])), _route_strings("lab holdout limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def has_contamination_refusals(self) -> bool:
        return self.measurement_refusal_count > 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def lab_holdout_audit_report(value: Mapping[str, Any]) -> LabHoldoutAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LabHoldoutAuditReport.from_wire(value)


__all__ = [
    "LAB_HOLDOUT_SCHEMA",
    "LAB_HOLDOUT_OPERATION_KINDS",
    "MAX_LAB_HOLDOUT_CANDIDATES",
    "MAX_LAB_HOLDOUTS",
    "MAX_LAB_HOLDOUT_OPERATIONS",
    "MAX_LAB_HOLDOUT_ROWS",
    "MAX_LAB_HOLDOUT_INPUT_BYTES",
    "LabHoldoutAuditArgs",
    "LabHoldoutAuditReport",
    "lab_holdout_audit_report",
]
