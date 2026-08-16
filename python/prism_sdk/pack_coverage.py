"""Typed benchmark-pack capability coverage and gap projections."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


PACK_COVERAGE_SCHEMA = "bioprism-mcp/pack-coverage-audit/0.1"
PACK_COVERAGE_SECTIONS = frozenset({"all", "15", "29"})
MAX_PACK_COVERAGE_IDS = 100
MAX_PACK_COVERAGE_ITEMS = 1_000
MAX_PACK_COVERAGE_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("pack coverage response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == PACK_COVERAGE_SCHEMA and isinstance(candidate.get("summary"), Mapping)
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
                        raise ArgumentError(f"pack coverage response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a pack coverage projection")


@dataclass(frozen=True)
class PackCoverageAuditArgs:
    section: str = "all"
    pack_ids: tuple[str, ...] = ()
    max_items: int = 100

    def __post_init__(self) -> None:
        section = _route_text("pack coverage section", self.section)
        if section not in PACK_COVERAGE_SECTIONS:
            raise ArgumentError("pack coverage section must be all, 15, or 29")
        pack_ids = tuple(_route_text(f"pack coverage pack_ids[{index}]", item) for index, item in enumerate(_array("pack coverage pack_ids", self.pack_ids)))
        if len(pack_ids) > MAX_PACK_COVERAGE_IDS:
            raise ArgumentError("pack coverage pack_ids is bounded at 100 ids")
        if len(pack_ids) != len(set(pack_ids)):
            raise ArgumentError("pack coverage pack_ids must be unique")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= MAX_PACK_COVERAGE_ITEMS:
            raise ArgumentError("pack coverage max_items must be between 1 and 1000")
        arguments = {"section": section, "pack_ids": list(pack_ids), "max_items": self.max_items}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"pack coverage arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_PACK_COVERAGE_INPUT_BYTES:
            raise ArgumentError("pack coverage input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "section", section)
        object.__setattr__(self, "pack_ids", pack_ids)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCoverageAuditArgs":
        raw = _route_mapping("pack coverage arguments", value)
        return cls(raw.get("section", "all"), tuple(_route_text(f"pack coverage pack_ids[{index}]", item) for index, item in enumerate(_array("pack coverage pack_ids", raw.get("pack_ids", [])))), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"section": self.section, "max_items": self.max_items}
        if self.pack_ids:
            result["pack_ids"] = list(self.pack_ids)
        return result


@dataclass(frozen=True)
class PackCoverageAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    section: str | None
    selected_pack_count: int | None
    selected_pack_ids: tuple[str, ...]
    summary: Mapping[str, Any] | None
    rows: tuple[Mapping[str, Any], ...]
    rows_omitted: int
    uncovered: tuple[str, ...]
    uncovered_omitted: int
    singly_covered: tuple[str, ...]
    singly_covered_omitted: int
    weakly_covered: tuple[str, ...]
    weakly_covered_omitted: int
    matrix: tuple[Mapping[str, Any], ...]
    matrix_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCoverageAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("pack coverage refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, (), None, (), 0, (), 0, (), 0, (), 0, (), 0, _route_strings("pack coverage refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("pack coverage refusal stage", raw.get("stage")), _route_text("pack coverage refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != PACK_COVERAGE_SCHEMA:
            raise ArgumentError("pack coverage projection has an invalid schema")
        section = _route_text("pack coverage report section", raw.get("section"))
        if section not in PACK_COVERAGE_SECTIONS:
            raise ArgumentError("pack coverage report section is invalid")
        selected_count = _route_count("pack coverage selected pack count", raw.get("selected_pack_count"))
        summary = _route_mapping("pack coverage summary", raw.get("summary"))
        families = _route_count("pack coverage summary families", summary.get("families"))
        covered = _route_count("pack coverage summary covered", summary.get("covered"))
        uncovered_count = _route_count("pack coverage summary uncovered", summary.get("uncovered"))
        singly_count = _route_count("pack coverage summary singly_covered", summary.get("singly_covered"))
        weakly_count = _route_count("pack coverage summary weakly_covered", summary.get("weakly_covered"))
        coverage_fraction = summary.get("coverage_fraction")
        if isinstance(coverage_fraction, bool) or not isinstance(coverage_fraction, (int, float)) or not math.isfinite(float(coverage_fraction)) or not 0.0 <= float(coverage_fraction) <= 1.0:
            raise ArgumentError("pack coverage summary coverage_fraction must be between 0 and 1")
        _route_text("pack coverage gap summary", summary.get("gap_summary"))
        selected_ids = _route_strings("pack coverage selected ids", raw.get("selected_pack_ids", []))
        if selected_count != len(selected_ids):
            raise ArgumentError("pack coverage selected pack count must equal the selected id count")
        rows = tuple(_route_mapping("pack coverage row", item) for item in _array("pack coverage rows", raw.get("rows", [])))
        rows_omitted = _route_count("pack coverage rows omitted", raw.get("rows_omitted"))
        uncovered = _route_strings("pack coverage uncovered", raw.get("uncovered", []))
        uncovered_omitted = _route_count("pack coverage uncovered omitted", raw.get("uncovered_omitted"))
        singly_covered = _route_strings("pack coverage singly covered", raw.get("singly_covered", []))
        singly_covered_omitted = _route_count("pack coverage singly covered omitted", raw.get("singly_covered_omitted"))
        weakly_covered = _route_strings("pack coverage weakly covered", raw.get("weakly_covered", []))
        weakly_covered_omitted = _route_count("pack coverage weakly covered omitted", raw.get("weakly_covered_omitted"))
        matrix = tuple(_route_mapping("pack coverage matrix", item) for item in _array("pack coverage matrix", raw.get("matrix", [])))
        matrix_omitted = _route_count("pack coverage matrix omitted", raw.get("matrix_omitted"))
        if len(rows) + rows_omitted != families:
            raise ArgumentError("pack coverage rows do not reconcile with the family count")
        if len(uncovered) + uncovered_omitted != uncovered_count:
            raise ArgumentError("pack coverage uncovered rows do not reconcile with the summary")
        if len(singly_covered) + singly_covered_omitted != singly_count:
            raise ArgumentError("pack coverage singly-covered rows do not reconcile with the summary")
        if len(weakly_covered) + weakly_covered_omitted != weakly_count:
            raise ArgumentError("pack coverage weakly-covered rows do not reconcile with the summary")
        if covered + uncovered_count != families:
            raise ArgumentError("pack coverage covered and uncovered families do not reconcile")
        return cls(raw, True, PACK_COVERAGE_SCHEMA, section, selected_count, selected_ids, summary, rows, rows_omitted, uncovered, uncovered_omitted, singly_covered, singly_covered_omitted, weakly_covered, weakly_covered_omitted, matrix, matrix_omitted, _route_strings("pack coverage guarantees", raw.get("guarantees", [])), _route_strings("pack coverage limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def coverage_fraction(self) -> float | None:
        return None if self.summary is None else float(self.summary.get("coverage_fraction", 0.0))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def pack_coverage_audit_report(value: Mapping[str, Any]) -> PackCoverageAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return PackCoverageAuditReport.from_wire(value)


__all__ = [
    "PACK_COVERAGE_SCHEMA",
    "PACK_COVERAGE_SECTIONS",
    "MAX_PACK_COVERAGE_IDS",
    "MAX_PACK_COVERAGE_ITEMS",
    "MAX_PACK_COVERAGE_INPUT_BYTES",
    "PackCoverageAuditArgs",
    "PackCoverageAuditReport",
    "pack_coverage_audit_report",
]
