"""Typed decision-relative context compression and rate-distortion projections."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .epistemic import EpistemicBeliefArgs, EpistemicDecisionProblemArgs


EPISTEMIC_CONTEXT_SCHEMA = "bioprism-mcp/epistemic-context-audit/0.1"
EPISTEMIC_CONTEXT_CRITERIA = frozenset({"bayes_regret", "minimax_regret"})
MAX_EPISTEMIC_CONTEXT_ITEMS = 16
MAX_EPISTEMIC_CONTEXT_SUBSETS = 256
MAX_EPISTEMIC_CONTEXT_ROWS = 1_000
MAX_EPISTEMIC_CONTEXT_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("epistemic context response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == EPISTEMIC_CONTEXT_SCHEMA and isinstance(candidate.get("problem"), Mapping)
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
                        raise ArgumentError(f"epistemic context response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain an epistemic context projection")


@dataclass(frozen=True)
class EpistemicEvidenceItemArgs:
    id: str
    cost: float
    likelihood: tuple[float, ...]

    def __post_init__(self) -> None:
        identifier = _route_text("epistemic context evidence id", self.id)
        if len(identifier.encode("utf-8")) > 512:
            raise ArgumentError("epistemic context evidence ids must contain at most 512 UTF-8 bytes")
        cost = _finite("epistemic context evidence cost", self.cost)
        if cost < 0.0:
            raise ArgumentError("epistemic context evidence cost must be non-negative")
        likelihood = tuple(_finite(f"epistemic context likelihood[{index}]", value) for index, value in enumerate(self.likelihood))
        if not likelihood or len(likelihood) > 1_000 or any(value < 0.0 for value in likelihood):
            raise ArgumentError("epistemic context likelihood must contain 1 to 1000 non-negative values")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "cost", cost)
        object.__setattr__(self, "likelihood", likelihood)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicEvidenceItemArgs":
        raw = _route_mapping("epistemic context evidence item", value)
        return cls(
            _route_text("epistemic context evidence id", raw.get("id")),
            _finite("epistemic context evidence cost", raw.get("cost")),
            tuple(_finite(f"epistemic context likelihood[{index}]", item) for index, item in enumerate(_array("epistemic context evidence likelihood", raw.get("likelihood")))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "cost": self.cost, "likelihood": list(self.likelihood)}


@dataclass(frozen=True)
class EpistemicEvidencePoolArgs:
    items: tuple[EpistemicEvidenceItemArgs, ...]

    def __post_init__(self) -> None:
        items = tuple(item if isinstance(item, EpistemicEvidenceItemArgs) else EpistemicEvidenceItemArgs.from_wire(item) for item in self.items)
        if not 1 <= len(items) <= MAX_EPISTEMIC_CONTEXT_ITEMS:
            raise ArgumentError("epistemic context evidence_pool must contain between 1 and 16 items")
        ids = [item.id for item in items]
        if len(ids) != len(set(ids)):
            raise ArgumentError("epistemic context evidence ids must be unique")
        object.__setattr__(self, "items", items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicEvidencePoolArgs":
        raw = _route_mapping("epistemic context evidence pool", value)
        return cls(tuple(EpistemicEvidenceItemArgs.from_wire(item) for item in _array("epistemic context evidence items", raw.get("items"))))

    def to_wire(self) -> dict[str, Any]:
        return {"items": [item.to_wire() for item in self.items]}


@dataclass(frozen=True)
class EpistemicContextAuditArgs:
    problem: EpistemicDecisionProblemArgs
    belief: EpistemicBeliefArgs
    evidence_pool: EpistemicEvidencePoolArgs
    criterion: str
    tolerance: float
    compatibility_floor: float
    subsets: tuple[tuple[int, ...], ...] = ()
    include_frontier: bool = True
    max_rows: int = 100

    def __post_init__(self) -> None:
        problem = self.problem if isinstance(self.problem, EpistemicDecisionProblemArgs) else EpistemicDecisionProblemArgs.from_wire(self.problem)
        belief = self.belief if isinstance(self.belief, EpistemicBeliefArgs) else EpistemicBeliefArgs.from_wire(self.belief)
        pool = self.evidence_pool if isinstance(self.evidence_pool, EpistemicEvidencePoolArgs) else EpistemicEvidencePoolArgs.from_wire(self.evidence_pool)
        if len(belief.mass) != len(problem.models):
            raise ArgumentError("epistemic context belief length must match problem models")
        for item in pool.items:
            if len(item.likelihood) != len(problem.models):
                raise ArgumentError("epistemic context likelihood lengths must match problem models")
        criterion = _route_text("epistemic context criterion", self.criterion)
        if criterion not in EPISTEMIC_CONTEXT_CRITERIA:
            raise ArgumentError("epistemic context criterion must be bayes_regret or minimax_regret")
        tolerance = _finite("epistemic context tolerance", self.tolerance)
        if tolerance < 0.0:
            raise ArgumentError("epistemic context tolerance must be non-negative")
        floor = _finite("epistemic context compatibility_floor", self.compatibility_floor)
        if not 0.0 <= floor <= 1.0:
            raise ArgumentError("epistemic context compatibility_floor must be between 0 and 1")
        subsets: list[tuple[int, ...]] = []
        for index, subset in enumerate(_array("epistemic context subsets", self.subsets)):
            values = tuple(item for item in _array(f"epistemic context subsets[{index}]", subset))
            if len(values) != len(set(values)):
                raise ArgumentError(f"epistemic context subsets[{index}] must not repeat an evidence index")
            normalized: list[int] = []
            for position, item in enumerate(values):
                if isinstance(item, bool) or not isinstance(item, int) or item < 0 or item >= len(pool.items):
                    raise ArgumentError(f"epistemic context subsets[{index}][{position}] is outside the evidence pool")
                normalized.append(item)
            subsets.append(tuple(normalized))
        if len(subsets) > MAX_EPISTEMIC_CONTEXT_SUBSETS:
            raise ArgumentError("epistemic context subsets must contain at most 256 index sets")
        if not isinstance(self.include_frontier, bool):
            raise ArgumentError("epistemic context include_frontier must be a boolean")
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_EPISTEMIC_CONTEXT_ROWS:
            raise ArgumentError("epistemic context max_rows must be between 1 and 1000")
        object.__setattr__(self, "problem", problem)
        object.__setattr__(self, "belief", belief)
        object.__setattr__(self, "evidence_pool", pool)
        object.__setattr__(self, "criterion", criterion)
        object.__setattr__(self, "tolerance", tolerance)
        object.__setattr__(self, "compatibility_floor", floor)
        object.__setattr__(self, "subsets", tuple(subsets))
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_EPISTEMIC_CONTEXT_INPUT_BYTES:
            raise ArgumentError("epistemic context input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicContextAuditArgs":
        raw = _route_mapping("epistemic context arguments", value)
        return cls(
            EpistemicDecisionProblemArgs.from_wire(raw.get("problem")),
            EpistemicBeliefArgs.from_wire(raw.get("belief")),
            EpistemicEvidencePoolArgs.from_wire(raw.get("evidence_pool")),
            raw.get("criterion"),
            raw.get("tolerance"),
            raw.get("compatibility_floor"),
            tuple(tuple(item for item in _array(f"epistemic context subsets[{index}]", subset)) for index, subset in enumerate(_array("epistemic context subsets", raw.get("subsets", [])))),
            raw.get("include_frontier", True),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "problem": self.problem.to_wire(),
            "belief": self.belief.to_wire(),
            "evidence_pool": self.evidence_pool.to_wire(),
            "criterion": self.criterion,
            "tolerance": self.tolerance,
            "compatibility_floor": self.compatibility_floor,
            "subsets": [list(item) for item in self.subsets],
            "include_frontier": self.include_frontier,
            "max_rows": self.max_rows,
        }


@dataclass(frozen=True)
class EpistemicContextAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    criterion: str | None
    problem: Mapping[str, Any] | None
    evidence_pool: Mapping[str, Any] | None
    identification: Mapping[str, Any] | None
    sufficiency: Mapping[str, Any] | None
    frontier: Mapping[str, Any] | None
    subset_rows: tuple[Mapping[str, Any], ...]
    subset_count: int
    subset_refusal_count: int
    subset_rows_omitted: int
    max_rows: int
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicContextAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("epistemic context refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, None, None, None, (), 0, 0, 0, _route_count("epistemic context refusal max_rows", raw.get("max_rows", 100)), _route_text("epistemic context refusal stage", raw.get("stage")), _route_text("epistemic context refusal", raw.get("refusal")), _route_strings("epistemic context refusal guarantees", raw.get("guarantees", [])), _route_strings("epistemic context refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != EPISTEMIC_CONTEXT_SCHEMA:
            raise ArgumentError("epistemic context projection has an invalid schema")
        criterion = _route_text("epistemic context criterion", raw.get("criterion"))
        if criterion not in EPISTEMIC_CONTEXT_CRITERIA:
            raise ArgumentError("epistemic context criterion is not recognized")
        problem = _route_mapping("epistemic context problem", raw.get("problem"))
        evidence_pool = _route_mapping("epistemic context evidence pool", raw.get("evidence_pool"))
        identification = _route_mapping("epistemic context identification", raw.get("identification"))
        sufficiency = _route_mapping("epistemic context sufficiency", raw.get("sufficiency"))
        frontier_raw = raw.get("frontier")
        frontier = None if frontier_raw is None else _route_mapping("epistemic context frontier", frontier_raw)
        rows = tuple(_route_mapping("epistemic context subset row", item) for item in _array("epistemic context subset rows", raw.get("subset_rows", [])))
        count = _route_count("epistemic context subset count", raw.get("subset_count"))
        refused = _route_count("epistemic context subset refusal count", raw.get("subset_refusal_count"))
        omitted = _route_count("epistemic context subset rows omitted", raw.get("subset_rows_omitted"))
        if len(rows) + omitted != count:
            raise ArgumentError("epistemic context subset rows do not reconcile")
        if refused > count:
            raise ArgumentError("epistemic context subset refusal count exceeds subset count")
        max_rows = _route_count("epistemic context max_rows", raw.get("max_rows"))
        if not 1 <= max_rows <= MAX_EPISTEMIC_CONTEXT_ROWS or len(rows) > max_rows:
            raise ArgumentError("epistemic context max_rows or bounded rows are invalid")
        return cls(raw, True, EPISTEMIC_CONTEXT_SCHEMA, criterion, problem, evidence_pool, identification, sufficiency, frontier, rows, count, refused, omitted, max_rows, None, None, _route_strings("epistemic context guarantees", raw.get("guarantees", [])), _route_strings("epistemic context limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def abstained(self) -> bool | None:
        if self.sufficiency is None:
            return None
        return self.sufficiency.get("outcome") == "abstain"

    @property
    def frontier_evaluated(self) -> int | None:
        return None if self.frontier is None else self.frontier.get("evaluated")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def epistemic_context_audit_report(value: Mapping[str, Any]) -> EpistemicContextAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EpistemicContextAuditReport.from_wire(value)


__all__ = [
    "EPISTEMIC_CONTEXT_SCHEMA",
    "EPISTEMIC_CONTEXT_CRITERIA",
    "MAX_EPISTEMIC_CONTEXT_ITEMS",
    "MAX_EPISTEMIC_CONTEXT_SUBSETS",
    "MAX_EPISTEMIC_CONTEXT_ROWS",
    "MAX_EPISTEMIC_CONTEXT_INPUT_BYTES",
    "EpistemicEvidenceItemArgs",
    "EpistemicEvidencePoolArgs",
    "EpistemicContextAuditArgs",
    "EpistemicContextAuditReport",
    "epistemic_context_audit_report",
]
