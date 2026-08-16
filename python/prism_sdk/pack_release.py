"""Typed release-order and explicit-unsequenced benchmark-pack projections."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


PACK_RELEASE_SCHEMA = "bioprism-mcp/pack-release-audit/0.1"
PACK_RELEASE_SECTIONS = frozenset({"all", "15", "29"})
MAX_PACK_RELEASE_IDS = 100
MAX_PACK_RELEASE_ITEMS = 1_000
MAX_PACK_RELEASE_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("pack release response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == PACK_RELEASE_SCHEMA and isinstance(candidate.get("selected_pack_count"), int)
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
                        raise ArgumentError(f"pack release response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a pack release projection")


@dataclass(frozen=True)
class PackReleaseAuditArgs:
    section: str = "all"
    pack_ids: tuple[str, ...] = ()
    max_items: int = 100

    def __post_init__(self) -> None:
        section = _route_text("pack release section", self.section)
        pack_ids = tuple(_route_text(f"pack release pack_ids[{index}]", item) for index, item in enumerate(_array("pack release pack_ids", self.pack_ids)))
        if section not in PACK_RELEASE_SECTIONS:
            raise ArgumentError("pack release section must be all, 15, or 29")
        if len(pack_ids) > MAX_PACK_RELEASE_IDS:
            raise ArgumentError("pack release pack_ids is bounded at 100 ids")
        if len(pack_ids) != len(set(pack_ids)):
            raise ArgumentError("pack release pack_ids must be unique")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= MAX_PACK_RELEASE_ITEMS:
            raise ArgumentError("pack release max_items must be between 1 and 1000")
        arguments = {"section": section, "pack_ids": list(pack_ids), "max_items": self.max_items}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"pack release arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_PACK_RELEASE_INPUT_BYTES:
            raise ArgumentError("pack release input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "section", section)
        object.__setattr__(self, "pack_ids", pack_ids)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackReleaseAuditArgs":
        raw = _route_mapping("pack release arguments", value)
        return cls(raw.get("section", "all"), tuple(_route_text(f"pack release pack_ids[{index}]", item) for index, item in enumerate(_array("pack release pack_ids", raw.get("pack_ids", [])))), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"section": self.section, "max_items": self.max_items}
        if self.pack_ids:
            result["pack_ids"] = list(self.pack_ids)
        return result


@dataclass(frozen=True)
class PackReleaseAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    section: str | None
    selected_pack_count: int | None
    selected_pack_ids: tuple[str, ...]
    sequenced_count: int | None
    unsequenced_count: int | None
    release_coverage_fraction: float | None
    wave_counts: Mapping[str, int]
    axis_counts: Mapping[str, int]
    release_order: tuple[Mapping[str, Any], ...]
    release_order_omitted: int
    unsequenced: tuple[Mapping[str, Any], ...]
    unsequenced_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackReleaseAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("pack release refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, (), None, None, None, {}, {}, (), 0, (), 0, _route_strings("pack release refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("pack release refusal stage", raw.get("stage")), _route_text("pack release refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != PACK_RELEASE_SCHEMA:
            raise ArgumentError("pack release projection has an invalid schema")
        section = _route_text("pack release report section", raw.get("section"))
        if section not in PACK_RELEASE_SECTIONS:
            raise ArgumentError("pack release report section is invalid")
        selected_count = _route_count("pack release selected pack count", raw.get("selected_pack_count"))
        selected_ids = _route_strings("pack release selected ids", raw.get("selected_pack_ids", []))
        if selected_count != len(selected_ids):
            raise ArgumentError("pack release selected pack count must equal the selected id count")
        sequenced_count = _route_count("pack release sequenced count", raw.get("sequenced_count"))
        unsequenced_count = _route_count("pack release unsequenced count", raw.get("unsequenced_count"))
        if sequenced_count + unsequenced_count != selected_count:
            raise ArgumentError("pack release sequence counts do not reconcile with the selected pack count")
        fraction = raw.get("release_coverage_fraction")
        if isinstance(fraction, bool) or not isinstance(fraction, (int, float)) or not math.isfinite(float(fraction)) or not 0.0 <= float(fraction) <= 1.0:
            raise ArgumentError("pack release coverage fraction must be between 0 and 1")

        def counts(name: str, value: Any) -> dict[str, int]:
            mapping = _route_mapping(name, value)
            return {key: _route_count(f"{name}.{key}", count) for key, count in mapping.items()}

        wave_counts = counts("pack release wave counts", raw.get("wave_counts", {}))
        axis_counts = counts("pack release axis counts", raw.get("axis_counts", {}))
        release_order = tuple(_route_mapping("pack release order row", item) for item in _array("pack release order", raw.get("release_order", [])))
        release_order_omitted = _route_count("pack release order omitted", raw.get("release_order_omitted"))
        unsequenced = tuple(_route_mapping("pack release unsequenced row", item) for item in _array("pack release unsequenced", raw.get("unsequenced", [])))
        unsequenced_omitted = _route_count("pack release unsequenced omitted", raw.get("unsequenced_omitted"))
        if len(release_order) + release_order_omitted != sequenced_count:
            raise ArgumentError("pack release order rows do not reconcile with the sequenced count")
        if len(unsequenced) + unsequenced_omitted != unsequenced_count:
            raise ArgumentError("pack release unsequenced rows do not reconcile with the unsequenced count")
        return cls(raw, True, PACK_RELEASE_SCHEMA, section, selected_count, selected_ids, sequenced_count, unsequenced_count, float(fraction), wave_counts, axis_counts, release_order, release_order_omitted, unsequenced, unsequenced_omitted, _route_strings("pack release guarantees", raw.get("guarantees", [])), _route_strings("pack release limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def has_unsequenced(self) -> bool:
        return bool(self.unsequenced_count)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def pack_release_audit_report(value: Mapping[str, Any]) -> PackReleaseAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return PackReleaseAuditReport.from_wire(value)


__all__ = [
    "PACK_RELEASE_SCHEMA",
    "PACK_RELEASE_SECTIONS",
    "MAX_PACK_RELEASE_IDS",
    "MAX_PACK_RELEASE_ITEMS",
    "MAX_PACK_RELEASE_INPUT_BYTES",
    "PackReleaseAuditArgs",
    "PackReleaseAuditReport",
    "pack_release_audit_report",
]
