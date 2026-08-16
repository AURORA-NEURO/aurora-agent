"""Typed Pareto-front audit requests and projections for the inference lab."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LAB_PARETO_SCHEMA = "bioprism-mcp/lab-pareto-audit/0.1"
LAB_PARETO_DIRECTIONS = frozenset({"higher_is_better", "lower_is_better"})
LAB_PARETO_SELECTIONS = frozenset({"unique", "ambiguous", "empty"})
MAX_LAB_PARETO_OBJECTIVES = 64
MAX_LAB_PARETO_PROFILES = 512
MAX_LAB_PARETO_RELATIONS = 256
MAX_LAB_PARETO_ROWS = 1_000
MAX_LAB_PARETO_INPUT_BYTES = 10_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab Pareto response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == LAB_PARETO_SCHEMA and isinstance(candidate.get("front"), Mapping)
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
                        raise ArgumentError(f"lab Pareto response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a lab Pareto projection")


@dataclass(frozen=True)
class LabParetoAuditArgs:
    objectives: tuple[Mapping[str, Any], ...]
    profiles: tuple[Mapping[str, Any], ...]
    relations: tuple[Mapping[str, Any], ...] = ()
    max_rows: int = 100

    def __post_init__(self) -> None:
        objectives = tuple(
            _route_mapping(f"lab Pareto objectives[{index}]", item)
            for index, item in enumerate(_array("lab Pareto objectives", self.objectives))
        )
        if not 1 <= len(objectives) <= MAX_LAB_PARETO_OBJECTIVES:
            raise ArgumentError("lab Pareto objectives must contain between 1 and 64 objects")
        for index, objective in enumerate(objectives):
            axis = _route_text(f"lab Pareto objectives[{index}].axis", objective.get("axis"))
            if len(axis.encode("utf-8")) > 256:
                raise ArgumentError("lab Pareto objective axes must contain at most 256 bytes")
            direction = _route_text(f"lab Pareto objectives[{index}].direction", objective.get("direction"))
            if direction not in LAB_PARETO_DIRECTIONS:
                raise ArgumentError("lab Pareto objective direction is not recognized")

        profiles = tuple(
            _route_mapping(f"lab Pareto profiles[{index}]", item)
            for index, item in enumerate(_array("lab Pareto profiles", self.profiles))
        )
        if not 1 <= len(profiles) <= MAX_LAB_PARETO_PROFILES:
            raise ArgumentError("lab Pareto profiles must contain between 1 and 512 objects")
        for index, profile in enumerate(profiles):
            candidate = _route_text(f"lab Pareto profiles[{index}].candidate", profile.get("candidate"))
            if len(candidate.encode("utf-8")) > 512:
                raise ArgumentError("lab Pareto candidate identifiers must contain at most 512 bytes")
            _route_mapping(f"lab Pareto profiles[{index}].values", profile.get("values"))

        relations = tuple(
            _route_mapping(f"lab Pareto relations[{index}]", item)
            for index, item in enumerate(_array("lab Pareto relations", self.relations))
        )
        if len(relations) > MAX_LAB_PARETO_RELATIONS:
            raise ArgumentError("lab Pareto relations must contain at most 256 objects")
        for index, relation in enumerate(relations):
            _route_text(f"lab Pareto relations[{index}].left", relation.get("left"))
            _route_text(f"lab Pareto relations[{index}].right", relation.get("right"))
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_LAB_PARETO_ROWS:
            raise ArgumentError("lab Pareto max_rows must be between 1 and 1000")
        arguments = {
            "objectives": [dict(item) for item in objectives],
            "profiles": [dict(item) for item in profiles],
            "relations": [dict(item) for item in relations],
            "max_rows": self.max_rows,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"lab Pareto arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_LAB_PARETO_INPUT_BYTES:
            raise ArgumentError("lab Pareto input exceeds the 10000000-byte safety bound")
        object.__setattr__(self, "objectives", objectives)
        object.__setattr__(self, "profiles", profiles)
        object.__setattr__(self, "relations", relations)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabParetoAuditArgs":
        raw = _route_mapping("lab Pareto arguments", value)
        return cls(
            tuple(_route_mapping(f"lab Pareto objectives[{index}]", item) for index, item in enumerate(_array("lab Pareto objectives", raw.get("objectives")))),
            tuple(_route_mapping(f"lab Pareto profiles[{index}]", item) for index, item in enumerate(_array("lab Pareto profiles", raw.get("profiles")))),
            tuple(_route_mapping(f"lab Pareto relations[{index}]", item) for index, item in enumerate(_array("lab Pareto relations", raw.get("relations", [])))),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "objectives": [dict(item) for item in self.objectives],
            "profiles": [dict(item) for item in self.profiles],
            "relations": [dict(item) for item in self.relations],
            "max_rows": self.max_rows,
        }


@dataclass(frozen=True)
class LabParetoAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    objective_count: int | None
    profile_count: int | None
    admissions: tuple[Mapping[str, Any], ...]
    admissions_omitted: int
    front: Mapping[str, Any] | None
    front_members: tuple[Mapping[str, Any], ...]
    front_selection: str | None
    archived_count: int
    archived: tuple[Mapping[str, Any], ...]
    archived_omitted: int
    relations: tuple[Mapping[str, Any], ...]
    relations_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabParetoAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("lab Pareto refusals must be fail-closed")
            return cls(
                raw=raw,
                ok=False,
                schema=raw.get("schema"),
                objective_count=None,
                profile_count=None,
                admissions=(),
                admissions_omitted=0,
                front=None,
                front_members=(),
                front_selection=None,
                archived_count=0,
                archived=(),
                archived_omitted=0,
                relations=(),
                relations_omitted=0,
                guarantees=_route_strings("lab Pareto refusal guarantees", raw.get("guarantees", [])),
                limitations=(),
                stage=_route_text("lab Pareto refusal stage", raw.get("stage")),
                refusal=_route_text("lab Pareto refusal", raw.get("refusal")),
                fail_closed=True,
            )
        if raw.get("ok") is not True or raw.get("schema") != LAB_PARETO_SCHEMA:
            raise ArgumentError("lab Pareto projection has an invalid schema")
        objective_count = _route_count("lab Pareto objective count", raw.get("objective_count"))
        profile_count = _route_count("lab Pareto profile count", raw.get("profile_count"))
        if not 1 <= objective_count <= MAX_LAB_PARETO_OBJECTIVES or not 1 <= profile_count <= MAX_LAB_PARETO_PROFILES:
            raise ArgumentError("lab Pareto counts are outside the declared bounds")
        admissions = tuple(_route_mapping("lab Pareto admission", item) for item in _array("lab Pareto admissions", raw.get("admissions", [])))
        admissions_omitted = _route_count("lab Pareto admissions omitted", raw.get("admissions_omitted"))
        if len(admissions) + admissions_omitted != profile_count:
            raise ArgumentError("lab Pareto admissions do not reconcile with profile_count")
        front = _route_mapping("lab Pareto front", raw.get("front"))
        front_count = _route_count("lab Pareto front count", front.get("count"))
        front_members = tuple(_route_mapping("lab Pareto front member", item) for item in _array("lab Pareto front members", front.get("members", [])))
        if len(front_members) != front_count:
            raise ArgumentError("lab Pareto front members do not reconcile with front.count")
        unresolved = _array("lab Pareto unresolved", front.get("unresolved", []))
        unresolved_count = _route_count("lab Pareto unresolved count", front.get("unresolved_count"))
        if len(unresolved) != unresolved_count:
            raise ArgumentError("lab Pareto unresolved rows do not reconcile with unresolved_count")
        selection = _route_mapping("lab Pareto selection", front.get("selection"))
        selection_label = _route_text("lab Pareto selection label", selection.get("selection"))
        if selection_label not in LAB_PARETO_SELECTIONS:
            raise ArgumentError("lab Pareto selection label is not recognized")
        archived_count = _route_count("lab Pareto archived count", raw.get("archived_count"))
        archived = tuple(_route_mapping("lab Pareto archived row", item) for item in _array("lab Pareto archived", raw.get("archived", [])))
        archived_omitted = _route_count("lab Pareto archived omitted", raw.get("archived_omitted"))
        if len(archived) + archived_omitted != archived_count:
            raise ArgumentError("lab Pareto archived rows do not reconcile with archived_count")
        relations = tuple(_route_mapping("lab Pareto relation", item) for item in _array("lab Pareto relations", raw.get("relations", [])))
        relations_omitted = _route_count("lab Pareto relations omitted", raw.get("relations_omitted"))
        max_rows = _route_count("lab Pareto max_rows", raw.get("max_rows"))
        if not 1 <= max_rows <= MAX_LAB_PARETO_ROWS:
            raise ArgumentError("lab Pareto max_rows is outside the declared bounds")
        return cls(
            raw=raw,
            ok=True,
            schema=LAB_PARETO_SCHEMA,
            objective_count=objective_count,
            profile_count=profile_count,
            admissions=admissions,
            admissions_omitted=admissions_omitted,
            front=front,
            front_members=front_members,
            front_selection=selection_label,
            archived_count=archived_count,
            archived=archived,
            archived_omitted=archived_omitted,
            relations=relations,
            relations_omitted=relations_omitted,
            guarantees=_route_strings("lab Pareto guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("lab Pareto limitations", raw.get("limitations", [])),
            stage=None,
            refusal=None,
            fail_closed=False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def ambiguous(self) -> bool:
        return self.front_selection == "ambiguous"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def lab_pareto_audit_report(value: Mapping[str, Any]) -> LabParetoAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LabParetoAuditReport.from_wire(value)


__all__ = [
    "LAB_PARETO_SCHEMA",
    "LAB_PARETO_DIRECTIONS",
    "LAB_PARETO_SELECTIONS",
    "MAX_LAB_PARETO_OBJECTIVES",
    "MAX_LAB_PARETO_PROFILES",
    "MAX_LAB_PARETO_RELATIONS",
    "MAX_LAB_PARETO_ROWS",
    "MAX_LAB_PARETO_INPUT_BYTES",
    "LabParetoAuditArgs",
    "LabParetoAuditReport",
    "lab_pareto_audit_report",
]
