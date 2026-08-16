"""Typed architecture-space validation, lineage, and component-diff projections."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LAB_SPACE_SCHEMA = "bioprism-mcp/lab-space-audit/0.1"
MAX_LAB_SPACE_CANDIDATES = 512
MAX_LAB_SPACE_INSPECT = 512
MAX_LAB_SPACE_COMPARISONS = 512
MAX_LAB_SPACE_ROWS = 1_000
MAX_LAB_SPACE_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab space response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == LAB_SPACE_SCHEMA and isinstance(candidate.get("space"), Mapping)
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
                        raise ArgumentError(f"lab space response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a lab space projection")


@dataclass(frozen=True)
class LabSpaceAuditArgs:
    cost_ceiling: int
    candidates: tuple[Mapping[str, Any], ...]
    inspect: tuple[str, ...] | None = None
    comparisons: tuple[Mapping[str, Any], ...] = ()
    include_components: bool = False
    max_rows: int = 100

    def __post_init__(self) -> None:
        if not isinstance(self.cost_ceiling, int) or isinstance(self.cost_ceiling, bool) or not 0 <= self.cost_ceiling <= 1_000_000_000:
            raise ArgumentError("lab space cost_ceiling must be between 0 and 1000000000")
        candidates = tuple(_route_mapping(f"lab space candidates[{index}]", item) for index, item in enumerate(_array("lab space candidates", self.candidates)))
        if not 1 <= len(candidates) <= MAX_LAB_SPACE_CANDIDATES:
            raise ArgumentError("lab space candidates must contain between 1 and 512 objects")
        for index, candidate in enumerate(candidates):
            identifier = _route_text(f"lab space candidates[{index}].id", candidate.get("id"))
            if len(identifier.encode("utf-8")) > 512:
                raise ArgumentError("lab space candidate ids must contain at most 512 bytes")
        inspect = None if self.inspect is None else tuple(_route_text(f"lab space inspect[{index}]", item) for index, item in enumerate(_array("lab space inspect", self.inspect)))
        if inspect is not None:
            if len(inspect) > MAX_LAB_SPACE_INSPECT:
                raise ArgumentError("lab space inspect must contain at most 512 ids")
            if any(not identifier for identifier in inspect):
                raise ArgumentError("lab space inspect ids must not be empty")
        comparisons = tuple(_route_mapping(f"lab space comparisons[{index}]", item) for index, item in enumerate(_array("lab space comparisons", self.comparisons)))
        if len(comparisons) > MAX_LAB_SPACE_COMPARISONS:
            raise ArgumentError("lab space comparisons must contain at most 512 objects")
        for index, comparison in enumerate(comparisons):
            _route_text(f"lab space comparisons[{index}].before", comparison.get("before"))
            _route_text(f"lab space comparisons[{index}].after", comparison.get("after"))
        if not isinstance(self.include_components, bool):
            raise ArgumentError("lab space include_components must be a boolean")
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_LAB_SPACE_ROWS:
            raise ArgumentError("lab space max_rows must be between 1 and 1000")
        arguments: dict[str, Any] = {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in candidates],
            "comparisons": [dict(item) for item in comparisons],
            "include_components": self.include_components,
            "max_rows": self.max_rows,
        }
        if inspect is not None:
            arguments["inspect"] = list(inspect)
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"lab space arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_LAB_SPACE_INPUT_BYTES:
            raise ArgumentError("lab space input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "candidates", candidates)
        object.__setattr__(self, "inspect", inspect)
        object.__setattr__(self, "comparisons", comparisons)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabSpaceAuditArgs":
        raw = _route_mapping("lab space arguments", value)
        inspect = raw.get("inspect")
        return cls(
            raw.get("cost_ceiling"),
            tuple(_route_mapping(f"lab space candidates[{index}]", item) for index, item in enumerate(_array("lab space candidates", raw.get("candidates")))),
            None if inspect is None else tuple(_route_text(f"lab space inspect[{index}]", item) for index, item in enumerate(_array("lab space inspect", inspect))),
            tuple(_route_mapping(f"lab space comparisons[{index}]", item) for index, item in enumerate(_array("lab space comparisons", raw.get("comparisons", [])))),
            raw.get("include_components", False),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in self.candidates],
            "comparisons": [dict(item) for item in self.comparisons],
            "include_components": self.include_components,
            "max_rows": self.max_rows,
        }
        if self.inspect is not None:
            arguments["inspect"] = list(self.inspect)
        return arguments


@dataclass(frozen=True)
class LabSpaceAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    candidate_count: int
    registered_count: int
    space_committed: bool
    space: Mapping[str, Any] | None
    candidate_rows: tuple[Mapping[str, Any], ...]
    candidate_rows_omitted: int
    inspection_count: int
    inspection_rows: tuple[Mapping[str, Any], ...]
    inspection_rows_omitted: int
    comparison_count: int
    comparison_rows: tuple[Mapping[str, Any], ...]
    comparison_rows_omitted: int
    max_rows: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabSpaceAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("lab space refusals must be fail-closed")
            candidate_rows = tuple(_route_mapping("lab space refusal candidate row", item) for item in _array("lab space refusal candidate rows", raw.get("candidate_rows", [])))
            omitted = _route_count("lab space refusal candidate rows omitted", raw.get("candidate_rows_omitted", 0))
            candidate_count = _route_count("lab space refusal candidate count", raw.get("candidate_count", len(candidate_rows) + omitted))
            if len(candidate_rows) + omitted > candidate_count:
                raise ArgumentError("lab space refusal candidate rows exceed candidate_count")
            max_rows = _route_count("lab space refusal max_rows", raw.get("max_rows", 100))
            if not 1 <= max_rows <= MAX_LAB_SPACE_ROWS:
                raise ArgumentError("lab space refusal max_rows is outside the declared bounds")
            return cls(raw, False, raw.get("schema"), candidate_count, _route_count("lab space refusal registered count", raw.get("registered_count", 0)), raw.get("space_committed") is True, None, candidate_rows, omitted, 0, (), 0, 0, (), 0, max_rows, _route_strings("lab space refusal guarantees", raw.get("guarantees", [])), _route_strings("lab space refusal limitations", raw.get("limitations", [])), _route_text("lab space refusal stage", raw.get("stage")), _route_text("lab space refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != LAB_SPACE_SCHEMA:
            raise ArgumentError("lab space projection has an invalid schema")
        candidate_count = _route_count("lab space candidate count", raw.get("candidate_count"))
        registered_count = _route_count("lab space registered count", raw.get("registered_count"))
        if registered_count != candidate_count:
            raise ArgumentError("a successful lab space projection must register every candidate")
        if raw.get("space_committed") is not True:
            raise ArgumentError("a successful lab space projection must commit the space")
        space = _route_mapping("lab space space", raw.get("space"))
        candidate_rows = tuple(_route_mapping("lab space candidate row", item) for item in _array("lab space candidate rows", raw.get("candidate_rows", [])))
        candidate_rows_omitted = _route_count("lab space candidate rows omitted", raw.get("candidate_rows_omitted"))
        if len(candidate_rows) + candidate_rows_omitted != candidate_count:
            raise ArgumentError("lab space candidate rows do not reconcile with candidate_count")
        inspection_rows = tuple(_route_mapping("lab space inspection row", item) for item in _array("lab space inspection rows", raw.get("inspection_rows", [])))
        inspection_count = _route_count("lab space inspection count", raw.get("inspection_count"))
        inspection_rows_omitted = _route_count("lab space inspection rows omitted", raw.get("inspection_rows_omitted"))
        if len(inspection_rows) + inspection_rows_omitted != inspection_count:
            raise ArgumentError("lab space inspection rows do not reconcile with inspection_count")
        comparison_rows = tuple(_route_mapping("lab space comparison row", item) for item in _array("lab space comparison rows", raw.get("comparison_rows", [])))
        comparison_count = _route_count("lab space comparison count", raw.get("comparison_count"))
        comparison_rows_omitted = _route_count("lab space comparison rows omitted", raw.get("comparison_rows_omitted"))
        if len(comparison_rows) + comparison_rows_omitted != comparison_count:
            raise ArgumentError("lab space comparison rows do not reconcile with comparison_count")
        max_rows = _route_count("lab space max_rows", raw.get("max_rows"))
        if not 1 <= max_rows <= MAX_LAB_SPACE_ROWS:
            raise ArgumentError("lab space max_rows is outside the declared bounds")
        if any(len(rows) > max_rows for rows in (candidate_rows, inspection_rows, comparison_rows)):
            raise ArgumentError("lab space bounded rows exceed max_rows")
        return cls(raw, True, LAB_SPACE_SCHEMA, candidate_count, registered_count, True, space, candidate_rows, candidate_rows_omitted, inspection_count, inspection_rows, inspection_rows_omitted, comparison_count, comparison_rows, comparison_rows_omitted, max_rows, _route_strings("lab space guarantees", raw.get("guarantees", [])), _route_strings("lab space limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def complete(self) -> bool:
        return self.ok and self.candidate_rows_omitted == 0 and self.inspection_rows_omitted == 0 and self.comparison_rows_omitted == 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def lab_space_audit_report(value: Mapping[str, Any]) -> LabSpaceAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LabSpaceAuditReport.from_wire(value)


__all__ = [
    "LAB_SPACE_SCHEMA",
    "MAX_LAB_SPACE_CANDIDATES",
    "MAX_LAB_SPACE_INSPECT",
    "MAX_LAB_SPACE_COMPARISONS",
    "MAX_LAB_SPACE_ROWS",
    "MAX_LAB_SPACE_INPUT_BYTES",
    "LabSpaceAuditArgs",
    "LabSpaceAuditReport",
    "lab_space_audit_report",
]
