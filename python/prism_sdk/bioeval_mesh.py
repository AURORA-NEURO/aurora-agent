"""Typed evaluator-mesh audits for independence classes and disagreement witnesses."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_MESH_SCHEMA = "bioprism-mcp/bioeval-mesh-audit/0.1"
BIOEVAL_MESH_KINDS = frozenset({
    "deterministic_property",
    "executable_analysis",
    "metamorphic_relation",
    "statistical_reference",
    "prospective_reveal",
    "expert_review",
    "calibrated_model_judge",
})
MAX_BIOEVAL_MESH_EVALUATORS = 1_024
MAX_BIOEVAL_MESH_VERDICTS = 1_024
MAX_BIOEVAL_MESH_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_MESH_TEXT_BYTES = 4_096
MAX_BIOEVAL_MESH_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value.strip()):
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_MESH_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_MESH_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval mesh response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_MESH_SCHEMA and isinstance(candidate.get("mesh"), Mapping)
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
                        raise ArgumentError(f"bioeval mesh response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval mesh projection")


@dataclass(frozen=True)
class BioevalMeshEvaluatorArgs:
    id: str
    kind: str
    inputs: tuple[str, ...] = ()
    derived_from: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        identifier = _text("bioeval mesh evaluator id", self.id)
        kind = _text("bioeval mesh evaluator kind", self.kind)
        if kind not in BIOEVAL_MESH_KINDS:
            raise ArgumentError("bioeval mesh evaluator kind is not recognized")
        inputs = tuple(_text("bioeval mesh evaluator input", item) for item in self.inputs)
        derived_from = tuple(_text("bioeval mesh evaluator derived_from", item) for item in self.derived_from)
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "inputs", inputs)
        object.__setattr__(self, "derived_from", derived_from)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMeshEvaluatorArgs":
        raw = _route_mapping("bioeval mesh evaluator", value)
        return cls(
            _text("bioeval mesh evaluator id", raw.get("id")),
            _text("bioeval mesh evaluator kind", raw.get("kind")),
            tuple(_text("bioeval mesh evaluator input", item) for item in _array("bioeval mesh evaluator inputs", raw.get("inputs", []))),
            tuple(_text("bioeval mesh evaluator derived_from", item) for item in _array("bioeval mesh evaluator derived_from", raw.get("derived_from", []))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "inputs": list(self.inputs), "derived_from": list(self.derived_from)}


@dataclass(frozen=True)
class BioevalMeshVerdictArgs:
    evaluator: str
    position: str = ""
    abstained: bool = False

    def __post_init__(self) -> None:
        evaluator = _text("bioeval mesh verdict evaluator", self.evaluator)
        if not isinstance(self.abstained, bool):
            raise ArgumentError("bioeval mesh verdict abstained must be a boolean")
        position = _text("bioeval mesh verdict position", self.position, allow_empty=self.abstained)
        if not self.abstained and not position.strip():
            raise ArgumentError("called mesh verdicts require a position")
        object.__setattr__(self, "evaluator", evaluator)
        object.__setattr__(self, "position", position)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMeshVerdictArgs":
        raw = _route_mapping("bioeval mesh verdict", value)
        return cls(_text("bioeval mesh verdict evaluator", raw.get("evaluator")), raw.get("position", ""), raw.get("abstained", False))

    def to_wire(self) -> dict[str, Any]:
        return {"evaluator": self.evaluator, "position": self.position, "abstained": self.abstained}


@dataclass(frozen=True)
class BioevalMeshAuditArgs:
    evaluators: tuple[BioevalMeshEvaluatorArgs, ...]
    system_artifacts: tuple[str, ...] = ()
    verdicts: tuple[BioevalMeshVerdictArgs, ...] = ()
    expected: str | None = None
    max_items: int = 100
    require_independence: bool = False
    require_independent_ratings: bool = False

    def __post_init__(self) -> None:
        evaluators = tuple(item if isinstance(item, BioevalMeshEvaluatorArgs) else BioevalMeshEvaluatorArgs.from_wire(item) for item in self.evaluators)
        if not evaluators or len(evaluators) > MAX_BIOEVAL_MESH_EVALUATORS:
            raise ArgumentError("bioeval mesh evaluators must contain 1 to 1024 rows")
        if len({item.id for item in evaluators}) != len(evaluators):
            raise ArgumentError("bioeval mesh evaluator ids must be unique")
        system_artifacts = tuple(_text("bioeval mesh system artifact", item) for item in self.system_artifacts)
        if len(set(system_artifacts)) != len(system_artifacts):
            raise ArgumentError("bioeval mesh system artifacts must be unique")
        verdicts = tuple(item if isinstance(item, BioevalMeshVerdictArgs) else BioevalMeshVerdictArgs.from_wire(item) for item in self.verdicts)
        if len(verdicts) > MAX_BIOEVAL_MESH_VERDICTS:
            raise ArgumentError("bioeval mesh verdicts are bounded at 1024 rows")
        if len({item.evaluator for item in verdicts}) != len(verdicts):
            raise ArgumentError("bioeval mesh verdict evaluator ids must be unique")
        evaluator_ids = {item.id for item in evaluators}
        if any(item.evaluator not in evaluator_ids for item in verdicts):
            raise ArgumentError("bioeval mesh verdicts must name declared evaluators")
        expected = None if self.expected is None else _text("bioeval mesh expected position", self.expected)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_MESH_OUTPUT_ITEMS:
            raise ArgumentError("bioeval mesh max_items must be between 1 and 1000")
        for name in ("require_independence", "require_independent_ratings"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval mesh {name} must be a boolean")
        object.__setattr__(self, "evaluators", evaluators)
        object.__setattr__(self, "system_artifacts", system_artifacts)
        object.__setattr__(self, "verdicts", verdicts)
        object.__setattr__(self, "expected", expected)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_MESH_INPUT_BYTES:
            raise ArgumentError("bioeval mesh input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMeshAuditArgs":
        raw = _route_mapping("bioeval mesh arguments", value)
        return cls(
            tuple(BioevalMeshEvaluatorArgs.from_wire(item) for item in _array("bioeval mesh evaluators", raw.get("evaluators"))),
            tuple(_text("bioeval mesh system artifact", item) for item in _array("bioeval mesh system_artifacts", raw.get("system_artifacts", []))),
            tuple(BioevalMeshVerdictArgs.from_wire(item) for item in _array("bioeval mesh verdicts", raw.get("verdicts", []))),
            None if raw.get("expected") is None else _text("bioeval mesh expected position", raw.get("expected")),
            raw.get("max_items", 100),
            raw.get("require_independence", False),
            raw.get("require_independent_ratings", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"system_artifacts": list(self.system_artifacts), "evaluators": [item.to_wire() for item in self.evaluators], "verdicts": [item.to_wire() for item in self.verdicts], "max_items": self.max_items, "require_independence": self.require_independence, "require_independent_ratings": self.require_independent_ratings}
        if self.expected is not None:
            result["expected"] = self.expected
        return result


@dataclass(frozen=True)
class BioevalMeshAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    mesh: Mapping[str, Any] | None
    evaluators: Mapping[str, Any] | None
    classes: Mapping[str, Any] | None
    verdicts: Mapping[str, Any] | None
    disagreements: Mapping[str, Any] | None
    independent_ratings: Mapping[str, Any] | None
    contributions: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalMeshAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval mesh refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, None, None, None, None, _route_text("bioeval mesh refusal stage", raw.get("stage")), _route_text("bioeval mesh refusal", raw.get("refusal")), _route_strings("bioeval mesh refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval mesh refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_MESH_SCHEMA:
            raise ArgumentError("bioeval mesh projection has an invalid schema")
        return cls(raw, True, BIOEVAL_MESH_SCHEMA, _route_text("bioeval mesh workflow", raw.get("workflow")), _route_mapping("bioeval mesh summary", raw.get("mesh")), _route_mapping("bioeval mesh evaluators", raw.get("evaluators")), _route_mapping("bioeval mesh classes", raw.get("classes")), _route_mapping("bioeval mesh verdicts", raw.get("verdicts")), _route_mapping("bioeval mesh disagreements", raw.get("disagreements")), _route_mapping("bioeval mesh ratings", raw.get("independent_ratings")), _route_mapping("bioeval mesh contributions", raw.get("contributions")), _route_mapping("bioeval mesh findings", raw.get("findings")), None, None, _route_strings("bioeval mesh guarantees", raw.get("guarantees", [])), _route_strings("bioeval mesh limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def independence_verified(self) -> bool | None:
        if self.mesh is None:
            return None
        value = self.mesh.get("independence_verified")
        return value if isinstance(value, bool) else None

    @property
    def independent_class_count(self) -> int | None:
        if self.mesh is None:
            return None
        value = self.mesh.get("independent_class_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def within_class_count(self) -> int | None:
        if self.disagreements is None:
            return None
        value = self.disagreements.get("within_class_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def across_class_count(self) -> int | None:
        if self.disagreements is None:
            return None
        value = self.disagreements.get("across_class_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    def finding_ids(self, name: str) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get(name), Mapping):
            return ()
        values = self.findings[name].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    @property
    def abstaining_evaluators(self) -> tuple[str, ...]:
        return self.finding_ids("abstaining_evaluators")

    @property
    def unreported_evaluators(self) -> tuple[str, ...]:
        return self.finding_ids("unreported_evaluators")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_mesh_audit_report(value: Mapping[str, Any]) -> BioevalMeshAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalMeshAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_MESH_SCHEMA",
    "BIOEVAL_MESH_KINDS",
    "MAX_BIOEVAL_MESH_EVALUATORS",
    "MAX_BIOEVAL_MESH_VERDICTS",
    "MAX_BIOEVAL_MESH_OUTPUT_ITEMS",
    "MAX_BIOEVAL_MESH_TEXT_BYTES",
    "MAX_BIOEVAL_MESH_INPUT_BYTES",
    "BioevalMeshEvaluatorArgs",
    "BioevalMeshVerdictArgs",
    "BioevalMeshAuditArgs",
    "BioevalMeshAuditReport",
    "bioeval_mesh_audit_report",
]
