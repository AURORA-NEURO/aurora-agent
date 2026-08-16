"""Typed metamorphic-response audits for robustness and shortcut detection."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_METAMORPHIC_SCHEMA = "bioprism-mcp/bioeval-metamorphic-audit/0.1"
BIOEVAL_METAMORPHIC_RELATIONS = frozenset({"invariant", "directional_change"})
BIOEVAL_METAMORPHIC_DIRECTIONS = frozenset({"increase", "decrease"})
BIOEVAL_METAMORPHIC_RESPONSES = frozenset({"unchanged", "moved", "incomparable"})
MAX_BIOEVAL_METAMORPHIC_FAMILIES = 1_024
MAX_BIOEVAL_METAMORPHIC_TRIALS = 4_096
MAX_BIOEVAL_METAMORPHIC_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_METAMORPHIC_TEXT_BYTES = 4_096
MAX_BIOEVAL_METAMORPHIC_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_METAMORPHIC_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_METAMORPHIC_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval metamorphic response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_METAMORPHIC_SCHEMA and isinstance(candidate.get("suite"), Mapping)
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
                        raise ArgumentError(f"bioeval metamorphic response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval metamorphic projection")


@dataclass(frozen=True)
class BioevalMetamorphicRelationArgs:
    relation: str
    expected: str | None = None

    def __post_init__(self) -> None:
        relation = _text("bioeval metamorphic relation", self.relation)
        if relation not in BIOEVAL_METAMORPHIC_RELATIONS:
            raise ArgumentError("bioeval metamorphic relation must be invariant or directional_change")
        expected = None if self.expected is None else _text("bioeval metamorphic expected direction", self.expected)
        if relation == "directional_change" and expected not in BIOEVAL_METAMORPHIC_DIRECTIONS:
            raise ArgumentError("directional_change relation requires expected increase or decrease")
        if relation == "invariant" and expected is not None:
            raise ArgumentError("invariant relation cannot carry expected direction")
        object.__setattr__(self, "relation", relation)
        object.__setattr__(self, "expected", expected)

    @classmethod
    def from_wire(cls, value: str | Mapping[str, Any]) -> "BioevalMetamorphicRelationArgs":
        if isinstance(value, str):
            return cls(value)
        raw = _route_mapping("bioeval metamorphic relation", value)
        if len(raw) != 1 or "directional_change" not in raw:
            raise ArgumentError("directional_change relation must contain only expected")
        detail = _route_mapping("bioeval directional_change relation", raw["directional_change"])
        return cls("directional_change", _text("bioeval metamorphic expected direction", detail.get("expected")))

    def to_wire(self) -> str | dict[str, Any]:
        return self.relation if self.relation == "invariant" else {"directional_change": {"expected": self.expected}}


@dataclass(frozen=True)
class BioevalMetamorphicResponseArgs:
    response: str
    direction: str | None = None

    def __post_init__(self) -> None:
        response = _text("bioeval metamorphic response", self.response)
        if response not in BIOEVAL_METAMORPHIC_RESPONSES:
            raise ArgumentError("bioeval metamorphic response must be unchanged, moved, or incomparable")
        direction = None if self.direction is None else _text("bioeval observed direction", self.direction)
        if response == "moved" and direction not in BIOEVAL_METAMORPHIC_DIRECTIONS:
            raise ArgumentError("moved response requires increase or decrease direction")
        if response != "moved" and direction is not None:
            raise ArgumentError("only moved responses may carry direction")
        object.__setattr__(self, "response", response)
        object.__setattr__(self, "direction", direction)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMetamorphicResponseArgs":
        raw = _route_mapping("bioeval metamorphic response", value)
        return cls(_text("bioeval metamorphic response", raw.get("response")), None if raw.get("direction") is None else _text("bioeval observed direction", raw.get("direction")))

    def to_wire(self) -> dict[str, Any]:
        result = {"response": self.response}
        if self.direction is not None:
            result["direction"] = self.direction
        return result


@dataclass(frozen=True)
class BioevalMetamorphicTrialArgs:
    id: str
    relation: BioevalMetamorphicRelationArgs | str | Mapping[str, Any]
    response: BioevalMetamorphicResponseArgs | Mapping[str, Any]

    def __post_init__(self) -> None:
        identifier = _text("bioeval metamorphic trial id", self.id)
        relation = self.relation if isinstance(self.relation, BioevalMetamorphicRelationArgs) else BioevalMetamorphicRelationArgs.from_wire(self.relation)
        response = self.response if isinstance(self.response, BioevalMetamorphicResponseArgs) else BioevalMetamorphicResponseArgs.from_wire(self.response)
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "relation", relation)
        object.__setattr__(self, "response", response)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMetamorphicTrialArgs":
        raw = _route_mapping("bioeval metamorphic trial", value)
        return cls(_text("bioeval metamorphic trial id", raw.get("id")), BioevalMetamorphicRelationArgs.from_wire(raw.get("relation")), BioevalMetamorphicResponseArgs.from_wire(raw.get("response")))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "relation": self.relation.to_wire(), "response": self.response.to_wire()}  # type: ignore[union-attr]


@dataclass(frozen=True)
class BioevalMetamorphicFamilyArgs:
    id: str
    relation: BioevalMetamorphicRelationArgs | str | Mapping[str, Any]
    trials: tuple[BioevalMetamorphicTrialArgs, ...]

    def __post_init__(self) -> None:
        identifier = _text("bioeval metamorphic family id", self.id)
        relation = self.relation if isinstance(self.relation, BioevalMetamorphicRelationArgs) else BioevalMetamorphicRelationArgs.from_wire(self.relation)
        trials = tuple(item if isinstance(item, BioevalMetamorphicTrialArgs) else BioevalMetamorphicTrialArgs.from_wire(item) for item in self.trials)
        if not trials or len(trials) > MAX_BIOEVAL_METAMORPHIC_TRIALS:
            raise ArgumentError("bioeval metamorphic families must contain 1 to 4096 trials")
        if len({item.id for item in trials}) != len(trials):
            raise ArgumentError("bioeval metamorphic trial ids must be unique within a family")
        if any(item.relation != relation for item in trials):
            raise ArgumentError("bioeval metamorphic trial relation must match its family relation")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "relation", relation)
        object.__setattr__(self, "trials", trials)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMetamorphicFamilyArgs":
        raw = _route_mapping("bioeval metamorphic family", value)
        return cls(_text("bioeval metamorphic family id", raw.get("id")), BioevalMetamorphicRelationArgs.from_wire(raw.get("relation")), tuple(BioevalMetamorphicTrialArgs.from_wire(item) for item in _array("bioeval metamorphic trials", raw.get("trials"))))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "relation": self.relation.to_wire(), "trials": [item.to_wire() for item in self.trials]}  # type: ignore[union-attr]


@dataclass(frozen=True)
class BioevalMetamorphicAuditArgs:
    families: tuple[BioevalMetamorphicFamilyArgs, ...]
    max_items: int = 100
    require_both_relations: bool = False
    fail_on_undetermined: bool = False

    def __post_init__(self) -> None:
        families = tuple(item if isinstance(item, BioevalMetamorphicFamilyArgs) else BioevalMetamorphicFamilyArgs.from_wire(item) for item in self.families)
        if not families or len(families) > MAX_BIOEVAL_METAMORPHIC_FAMILIES:
            raise ArgumentError("bioeval metamorphic families must contain 1 to 1024 rows")
        if len({item.id for item in families}) != len(families):
            raise ArgumentError("bioeval metamorphic family ids must be unique")
        if sum(len(item.trials) for item in families) > MAX_BIOEVAL_METAMORPHIC_TRIALS:
            raise ArgumentError("bioeval metamorphic suite trials are bounded at 4096 rows")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_METAMORPHIC_OUTPUT_ITEMS:
            raise ArgumentError("bioeval metamorphic max_items must be between 1 and 1000")
        for name in ("require_both_relations", "fail_on_undetermined"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval metamorphic {name} must be a boolean")
        object.__setattr__(self, "families", families)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_METAMORPHIC_INPUT_BYTES:
            raise ArgumentError("bioeval metamorphic input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMetamorphicAuditArgs":
        raw = _route_mapping("bioeval metamorphic arguments", value)
        return cls(tuple(BioevalMetamorphicFamilyArgs.from_wire(item) for item in _array("bioeval metamorphic families", raw.get("families"))), raw.get("max_items", 100), raw.get("require_both_relations", False), raw.get("fail_on_undetermined", False))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"families": [item.to_wire() for item in self.families], "max_items": self.max_items, "require_both_relations": self.require_both_relations, "fail_on_undetermined": self.fail_on_undetermined}


@dataclass(frozen=True)
class BioevalMetamorphicAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    suite: Mapping[str, Any] | None
    families: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMetamorphicAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval metamorphic refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, _route_text("bioeval metamorphic refusal stage", raw.get("stage")), _route_text("bioeval metamorphic refusal", raw.get("refusal")), _route_strings("bioeval metamorphic refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval metamorphic refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_METAMORPHIC_SCHEMA:
            raise ArgumentError("bioeval metamorphic projection has an invalid schema")
        return cls(raw, True, BIOEVAL_METAMORPHIC_SCHEMA, _route_text("bioeval metamorphic workflow", raw.get("workflow")), _route_mapping("bioeval metamorphic suite", raw.get("suite")), _route_mapping("bioeval metamorphic families", raw.get("families")), _route_mapping("bioeval metamorphic findings", raw.get("findings")), None, None, _route_strings("bioeval metamorphic guarantees", raw.get("guarantees", [])), _route_strings("bioeval metamorphic limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def relation_coverage_complete(self) -> bool | None:
        if self.suite is None or not isinstance(self.suite.get("relation_coverage"), Mapping):
            return None
        value = self.suite["relation_coverage"].get("complete")
        return value if isinstance(value, bool) else None

    def finding_ids(self, name: str) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get(name), Mapping):
            return ()
        values = self.findings[name].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    @property
    def false_sensitivity_trials(self) -> tuple[str, ...]:
        return self.finding_ids("false_sensitivity_trials")

    @property
    def false_invariance_trials(self) -> tuple[str, ...]:
        return self.finding_ids("false_invariance_trials")

    @property
    def wrong_direction_trials(self) -> tuple[str, ...]:
        return self.finding_ids("wrong_direction_trials")

    @property
    def undetermined_trial_count(self) -> int | None:
        if self.suite is None:
            return None
        value = self.suite.get("undetermined_trial_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_metamorphic_audit_report(value: Mapping[str, Any]) -> BioevalMetamorphicAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalMetamorphicAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_METAMORPHIC_SCHEMA",
    "BIOEVAL_METAMORPHIC_RELATIONS",
    "BIOEVAL_METAMORPHIC_DIRECTIONS",
    "BIOEVAL_METAMORPHIC_RESPONSES",
    "MAX_BIOEVAL_METAMORPHIC_FAMILIES",
    "MAX_BIOEVAL_METAMORPHIC_TRIALS",
    "MAX_BIOEVAL_METAMORPHIC_OUTPUT_ITEMS",
    "MAX_BIOEVAL_METAMORPHIC_TEXT_BYTES",
    "MAX_BIOEVAL_METAMORPHIC_INPUT_BYTES",
    "BioevalMetamorphicRelationArgs",
    "BioevalMetamorphicResponseArgs",
    "BioevalMetamorphicTrialArgs",
    "BioevalMetamorphicFamilyArgs",
    "BioevalMetamorphicAuditArgs",
    "BioevalMetamorphicAuditReport",
    "bioeval_metamorphic_audit_report",
]
