"""Typed factorial-design audits for component attribution and interaction coverage."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_DESIGN_SCHEMA = "bioprism-mcp/bioeval-design-audit/0.1"
BIOEVAL_DESIGN_CONCLUSIONS = frozenset({
    "pass", "unsupported_pass", "contradicted_pass", "partial_credit", "fail",
    "vetoed", "disputed", "justification_unexamined", "unknown", "abstained",
})
BIOEVAL_DESIGN_TIERS = frozenset({"judge", "statistical", "property", "execution", "deterministic"})
MAX_BIOEVAL_DESIGN_FACTORS = 256
MAX_BIOEVAL_DESIGN_ARMS = 4_096
MAX_BIOEVAL_DESIGN_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_DESIGN_TEXT_BYTES = 4_096
MAX_BIOEVAL_DESIGN_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_DESIGN_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_DESIGN_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval design response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_DESIGN_SCHEMA and isinstance(candidate.get("design"), Mapping)
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
                        raise ArgumentError(f"bioeval design response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval design projection")


@dataclass(frozen=True)
class BioevalDesignArmArgs:
    id: str
    levels: Mapping[str, str]
    conclusion: str
    tier: str

    def __post_init__(self) -> None:
        identifier = _text("bioeval design arm id", self.id)
        if not isinstance(self.levels, Mapping) or not self.levels:
            raise ArgumentError("bioeval design arm levels must be a non-empty object")
        levels = {_text("bioeval design factor", key): _text("bioeval design level", value) for key, value in self.levels.items()}
        conclusion = _text("bioeval design arm conclusion", self.conclusion)
        if conclusion not in BIOEVAL_DESIGN_CONCLUSIONS:
            raise ArgumentError("bioeval design arm conclusion is not recognized")
        tier = _text("bioeval design arm tier", self.tier)
        if tier not in BIOEVAL_DESIGN_TIERS:
            raise ArgumentError("bioeval design arm tier is not recognized")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "levels", levels)
        object.__setattr__(self, "conclusion", conclusion)
        object.__setattr__(self, "tier", tier)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalDesignArmArgs":
        raw = _route_mapping("bioeval design arm", value)
        return cls(_text("bioeval design arm id", raw.get("id")), _route_mapping("bioeval design arm levels", raw.get("levels")), _text("bioeval design arm conclusion", raw.get("conclusion")), _text("bioeval design arm tier", raw.get("tier")))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "levels": dict(self.levels), "conclusion": self.conclusion, "tier": self.tier}


@dataclass(frozen=True)
class BioevalDesignAuditArgs:
    cell_id: str
    factors: tuple[str, ...]
    baseline: str
    arms: tuple[BioevalDesignArmArgs, ...]
    controlled: bool = False
    max_items: int = 100
    require_contrasts: bool = False
    require_complete_interactions: bool = False
    require_attribution: bool = False

    def __post_init__(self) -> None:
        cell_id = _text("bioeval design cell_id", self.cell_id)
        baseline = _text("bioeval design baseline", self.baseline)
        factors = tuple(_text("bioeval design factor", item) for item in self.factors)
        if not factors or len(factors) > MAX_BIOEVAL_DESIGN_FACTORS:
            raise ArgumentError("bioeval design factors must contain 1 to 256 names")
        if len(set(factors)) != len(factors):
            raise ArgumentError("bioeval design factors must be unique")
        arms = tuple(item if isinstance(item, BioevalDesignArmArgs) else BioevalDesignArmArgs.from_wire(item) for item in self.arms)
        if not 2 <= len(arms) <= MAX_BIOEVAL_DESIGN_ARMS:
            raise ArgumentError("bioeval design arms must contain 2 to 4096 rows")
        if len({item.id for item in arms}) != len(arms):
            raise ArgumentError("bioeval design arm ids must be unique")
        if baseline not in {item.id for item in arms}:
            raise ArgumentError("bioeval design baseline must name one of the arms")
        factor_set = set(factors)
        for arm in arms:
            if set(arm.levels) != factor_set:
                raise ArgumentError(f"bioeval design arm {arm.id!r} must assign every declared factor and no undeclared factor")
        for name in ("controlled", "require_contrasts", "require_complete_interactions", "require_attribution"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval design {name} must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_DESIGN_OUTPUT_ITEMS:
            raise ArgumentError("bioeval design max_items must be between 1 and 1000")
        object.__setattr__(self, "cell_id", cell_id)
        object.__setattr__(self, "factors", factors)
        object.__setattr__(self, "baseline", baseline)
        object.__setattr__(self, "arms", arms)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_DESIGN_INPUT_BYTES:
            raise ArgumentError("bioeval design input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalDesignAuditArgs":
        raw = _route_mapping("bioeval design arguments", value)
        return cls(
            _text("bioeval design cell_id", raw.get("cell_id")),
            tuple(_text("bioeval design factor", item) for item in _array("bioeval design factors", raw.get("factors"))),
            _text("bioeval design baseline", raw.get("baseline")),
            tuple(BioevalDesignArmArgs.from_wire(item) for item in _array("bioeval design arms", raw.get("arms"))),
            raw.get("controlled", False),
            raw.get("max_items", 100),
            raw.get("require_contrasts", False),
            raw.get("require_complete_interactions", False),
            raw.get("require_attribution", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"cell_id": self.cell_id, "factors": list(self.factors), "baseline": self.baseline, "arms": [item.to_wire() for item in self.arms], "controlled": self.controlled, "max_items": self.max_items, "require_contrasts": self.require_contrasts, "require_complete_interactions": self.require_complete_interactions, "require_attribution": self.require_attribution}


@dataclass(frozen=True)
class BioevalDesignAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    design: Mapping[str, Any] | None
    arms: Mapping[str, Any] | None
    contrasts: Mapping[str, Any] | None
    interactions: Mapping[str, Any] | None
    attributions: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalDesignAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval design refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, None, None, _route_text("bioeval design refusal stage", raw.get("stage")), _route_text("bioeval design refusal", raw.get("refusal")), _route_strings("bioeval design refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval design refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_DESIGN_SCHEMA:
            raise ArgumentError("bioeval design projection has an invalid schema")
        return cls(raw, True, BIOEVAL_DESIGN_SCHEMA, _route_text("bioeval design workflow", raw.get("workflow")), _route_mapping("bioeval design summary", raw.get("design")), _route_mapping("bioeval design arms", raw.get("arms")), _route_mapping("bioeval design contrasts", raw.get("contrasts")), _route_mapping("bioeval design interactions", raw.get("interactions")), _route_mapping("bioeval design attributions", raw.get("attributions")), _route_mapping("bioeval design findings", raw.get("findings")), None, None, _route_strings("bioeval design guarantees", raw.get("guarantees", [])), _route_strings("bioeval design limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def contrast_count(self) -> int | None:
        if self.design is None:
            return None
        value = self.design.get("contrast_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def causal_count(self) -> int | None:
        if self.attributions is None:
            return None
        value = self.attributions.get("causal_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    def finding_ids(self, name: str) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get(name), Mapping):
            return ()
        values = self.findings[name].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    @property
    def unattributable_arms(self) -> tuple[str, ...]:
        return self.finding_ids("unattributable_arms")

    @property
    def missing_interactions(self) -> tuple[str, ...]:
        return self.finding_ids("missing_interactions")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_design_audit_report(value: Mapping[str, Any]) -> BioevalDesignAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalDesignAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_DESIGN_SCHEMA",
    "BIOEVAL_DESIGN_CONCLUSIONS",
    "BIOEVAL_DESIGN_TIERS",
    "MAX_BIOEVAL_DESIGN_FACTORS",
    "MAX_BIOEVAL_DESIGN_ARMS",
    "MAX_BIOEVAL_DESIGN_OUTPUT_ITEMS",
    "MAX_BIOEVAL_DESIGN_TEXT_BYTES",
    "MAX_BIOEVAL_DESIGN_INPUT_BYTES",
    "BioevalDesignArmArgs",
    "BioevalDesignAuditArgs",
    "BioevalDesignAuditReport",
    "bioeval_design_audit_report",
]
