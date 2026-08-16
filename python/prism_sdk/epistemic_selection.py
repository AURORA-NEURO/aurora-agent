"""Typed bounded evidence-selection audits.

The selection route keeps three questions separate: what the greedy planner chose, whether the
objective passed an exhaustive submodularity check, and how the choice compares with the exact
small-instance optimum. A selector without the second or third result is still useful, but it is
not allowed to masquerade as a guaranteed approximation or an exact plan.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .epistemic import EpistemicBeliefArgs, EpistemicDecisionProblemArgs
from .epistemic_context import EpistemicEvidenceItemArgs


EPISTEMIC_SELECTION_SCHEMA = "bioprism-mcp/epistemic-selection-audit/0.1"
MAX_EPISTEMIC_SELECTION_ITEMS = 64
MAX_EPISTEMIC_SELECTION_PROTECTED = 64
MAX_EPISTEMIC_SELECTION_INPUT_BYTES = 20_000_000
MAX_EPISTEMIC_SELECTION_EXHAUSTIVE = 20
MAX_EPISTEMIC_SELECTION_SUBMODULARITY = 12


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ArgumentError(f"{name} must be a finite number")
    return parsed


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("epistemic selection response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == EPISTEMIC_SELECTION_SCHEMA and isinstance(candidate.get("greedy"), Mapping)
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
                        raise ArgumentError(f"epistemic selection response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain an epistemic selection projection")


@dataclass(frozen=True)
class EpistemicSelectionEvidencePoolArgs:
    """Ordered observed evidence; indexes are stable planner inputs."""

    items: tuple[EpistemicEvidenceItemArgs, ...]

    def __post_init__(self) -> None:
        items = tuple(item if isinstance(item, EpistemicEvidenceItemArgs) else EpistemicEvidenceItemArgs.from_wire(item) for item in self.items)
        if not 1 <= len(items) <= MAX_EPISTEMIC_SELECTION_ITEMS:
            raise ArgumentError("epistemic selection evidence_pool must contain between 1 and 64 items")
        ids = [item.id for item in items]
        if len(ids) != len(set(ids)):
            raise ArgumentError("epistemic selection evidence ids must be unique")
        object.__setattr__(self, "items", items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicSelectionEvidencePoolArgs":
        raw = _route_mapping("epistemic selection evidence pool", value)
        return cls(tuple(EpistemicEvidenceItemArgs.from_wire(item) for item in _array("epistemic selection evidence items", raw.get("items"))))

    def to_wire(self) -> dict[str, Any]:
        return {"items": [item.to_wire() for item in self.items]}


@dataclass(frozen=True)
class EpistemicSelectionConstraintArgs:
    """Cardinality and/or scalarized budget bounds accepted by the Rust selector."""

    cardinality: int | None = None
    budget: float | None = None
    costs: tuple[float, ...] | None = None

    def __post_init__(self) -> None:
        cardinality = self.cardinality
        if cardinality is not None and (isinstance(cardinality, bool) or not isinstance(cardinality, int) or not 0 <= cardinality <= MAX_EPISTEMIC_SELECTION_ITEMS):
            raise ArgumentError("epistemic selection cardinality must be between 0 and 64")
        budget = None if self.budget is None else _finite("epistemic selection budget", self.budget)
        if budget is not None and budget < 0.0:
            raise ArgumentError("epistemic selection budget must be non-negative")
        costs = None if self.costs is None else tuple(_finite(f"epistemic selection costs[{index}]", value) for index, value in enumerate(self.costs))
        if costs is not None and any(value < 0.0 for value in costs):
            raise ArgumentError("epistemic selection costs must be non-negative")
        if cardinality is None and budget is None:
            raise ArgumentError("epistemic selection requires cardinality or budget")
        if budget is not None and costs is not None and any(value <= 0.0 for value in costs):
            raise ArgumentError("budgeted epistemic selection costs must be positive")
        object.__setattr__(self, "budget", budget)
        object.__setattr__(self, "costs", costs)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicSelectionConstraintArgs":
        raw = _route_mapping("epistemic selection constraint", value)
        raw_costs = raw.get("costs")
        return cls(
            raw.get("cardinality"),
            None if raw.get("budget") is None else _finite("epistemic selection budget", raw.get("budget")),
            None if raw_costs is None else tuple(_finite(f"epistemic selection costs[{index}]", item) for index, item in enumerate(_array("epistemic selection costs", raw_costs))),
        )

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"cardinality": self.cardinality, "budget": self.budget}
        if self.costs is not None:
            result["costs"] = list(self.costs)
        return result


@dataclass(frozen=True)
class EpistemicSelectionAuditArgs:
    problem: EpistemicDecisionProblemArgs
    belief: EpistemicBeliefArgs
    evidence_pool: EpistemicSelectionEvidencePoolArgs
    constraint: EpistemicSelectionConstraintArgs
    protected: tuple[int, ...] = ()
    check_submodularity: bool = True
    include_lazy: bool = True
    compare_optimum: bool = True
    tolerance: float = 1e-9

    def __post_init__(self) -> None:
        problem = self.problem if isinstance(self.problem, EpistemicDecisionProblemArgs) else EpistemicDecisionProblemArgs.from_wire(self.problem)
        belief = self.belief if isinstance(self.belief, EpistemicBeliefArgs) else EpistemicBeliefArgs.from_wire(self.belief)
        pool = self.evidence_pool if isinstance(self.evidence_pool, EpistemicSelectionEvidencePoolArgs) else EpistemicSelectionEvidencePoolArgs.from_wire(self.evidence_pool)
        constraint = self.constraint if isinstance(self.constraint, EpistemicSelectionConstraintArgs) else EpistemicSelectionConstraintArgs.from_wire(self.constraint)
        if len(belief.mass) != len(problem.models):
            raise ArgumentError("epistemic selection belief length must match problem models")
        for item in pool.items:
            if len(item.likelihood) != len(problem.models):
                raise ArgumentError("epistemic selection likelihood lengths must match problem models")
        if constraint.costs is not None and len(constraint.costs) != len(pool.items):
            raise ArgumentError(f"epistemic selection costs must contain exactly {len(pool.items)} entries")
        protected = tuple(self.protected)
        if len(protected) > MAX_EPISTEMIC_SELECTION_PROTECTED:
            raise ArgumentError("epistemic selection protected closure is bounded at 64 items")
        if any(isinstance(item, bool) or not isinstance(item, int) or not 0 <= item < len(pool.items) for item in protected):
            raise ArgumentError("epistemic selection protected indexes must be within the evidence pool")
        if len(protected) != len(set(protected)):
            raise ArgumentError("epistemic selection protected indexes must be unique")
        for name, value in (("check_submodularity", self.check_submodularity), ("include_lazy", self.include_lazy), ("compare_optimum", self.compare_optimum)):
            if not isinstance(value, bool):
                raise ArgumentError(f"epistemic selection {name} must be a boolean")
        tolerance = _finite("epistemic selection tolerance", self.tolerance)
        if tolerance < 0.0:
            raise ArgumentError("epistemic selection tolerance must be non-negative")
        object.__setattr__(self, "problem", problem)
        object.__setattr__(self, "belief", belief)
        object.__setattr__(self, "evidence_pool", pool)
        object.__setattr__(self, "constraint", constraint)
        object.__setattr__(self, "protected", protected)
        object.__setattr__(self, "tolerance", tolerance)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_EPISTEMIC_SELECTION_INPUT_BYTES:
            raise ArgumentError("epistemic selection input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicSelectionAuditArgs":
        raw = _route_mapping("epistemic selection arguments", value)
        return cls(
            EpistemicDecisionProblemArgs.from_wire(raw.get("problem")),
            EpistemicBeliefArgs.from_wire(raw.get("belief")),
            EpistemicSelectionEvidencePoolArgs.from_wire(raw.get("evidence_pool")),
            EpistemicSelectionConstraintArgs.from_wire(raw.get("constraint")),
            tuple(item for item in _array("epistemic selection protected", raw.get("protected", []))),
            raw.get("check_submodularity", True),
            raw.get("include_lazy", True),
            raw.get("compare_optimum", True),
            raw.get("tolerance", 1e-9),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "problem": self.problem.to_wire(),
            "belief": self.belief.to_wire(),
            "evidence_pool": self.evidence_pool.to_wire(),
            "constraint": self.constraint.to_wire(),
            "protected": list(self.protected),
            "check_submodularity": self.check_submodularity,
            "include_lazy": self.include_lazy,
            "compare_optimum": self.compare_optimum,
            "tolerance": self.tolerance,
        }


@dataclass(frozen=True)
class EpistemicSelectionAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    objective: str | None
    problem: Mapping[str, Any] | None
    evidence_pool: Mapping[str, Any] | None
    constraint: Mapping[str, Any] | None
    baseline: Mapping[str, Any] | None
    submodularity: Mapping[str, Any] | None
    greedy: Mapping[str, Any] | None
    lazy: Mapping[str, Any] | None
    comparisons: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicSelectionAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("epistemic selection refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, None, None, None, None, None, _route_text("epistemic selection refusal stage", raw.get("stage")), _route_text("epistemic selection refusal", raw.get("refusal")), _route_strings("epistemic selection refusal guarantees", raw.get("guarantees", [])), _route_strings("epistemic selection refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != EPISTEMIC_SELECTION_SCHEMA:
            raise ArgumentError("epistemic selection projection has an invalid schema")
        problem = _route_mapping("epistemic selection problem", raw.get("problem"))
        pool = _route_mapping("epistemic selection evidence pool", raw.get("evidence_pool"))
        constraint = _route_mapping("epistemic selection constraint", raw.get("constraint"))
        baseline = _route_mapping("epistemic selection baseline", raw.get("baseline"))
        submodularity = _route_mapping("epistemic selection submodularity", raw.get("submodularity"))
        greedy = _route_mapping("epistemic selection greedy", raw.get("greedy"))
        lazy_raw = raw.get("lazy")
        lazy = None if lazy_raw is None else _route_mapping("epistemic selection lazy", lazy_raw)
        comparisons = _route_mapping("epistemic selection comparisons", raw.get("comparisons"))
        return cls(raw, True, EPISTEMIC_SELECTION_SCHEMA, _route_text("epistemic selection objective", raw.get("objective")), problem, pool, constraint, baseline, submodularity, greedy, lazy, comparisons, None, None, _route_strings("epistemic selection guarantees", raw.get("guarantees", [])), _route_strings("epistemic selection limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def guarantee_applies(self) -> bool | None:
        if self.greedy is None:
            return None
        guarantee = self.greedy.get("guarantee")
        return isinstance(guarantee, Mapping) and guarantee.get("applicability") == "applies"

    @property
    def exact_status(self) -> str | None:
        if self.comparisons is None or not isinstance(self.comparisons.get("exact_optimum"), Mapping):
            return None
        return self.comparisons["exact_optimum"].get("status")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def epistemic_selection_audit_report(value: Mapping[str, Any]) -> EpistemicSelectionAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EpistemicSelectionAuditReport.from_wire(value)


__all__ = [
    "EPISTEMIC_SELECTION_SCHEMA",
    "MAX_EPISTEMIC_SELECTION_ITEMS",
    "MAX_EPISTEMIC_SELECTION_PROTECTED",
    "MAX_EPISTEMIC_SELECTION_INPUT_BYTES",
    "MAX_EPISTEMIC_SELECTION_EXHAUSTIVE",
    "MAX_EPISTEMIC_SELECTION_SUBMODULARITY",
    "EpistemicSelectionEvidencePoolArgs",
    "EpistemicSelectionConstraintArgs",
    "EpistemicSelectionAuditArgs",
    "EpistemicSelectionAuditReport",
    "epistemic_selection_audit_report",
]
