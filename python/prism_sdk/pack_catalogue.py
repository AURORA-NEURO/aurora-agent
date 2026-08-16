"""Typed benchmark-pack portfolio catalogue projections.

The pack catalogue is a declaration inventory, not a score board.  It records what each pack
claims to measure, its capability/domain axes, possible oracle tiers, declared release wave, and
duplicate-signature review candidates.  The parser deliberately does not expose a health score;
observed calibration and reportability belong to ``pack_health_assess``.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


PACK_CATALOGUE_MAX_ITEMS = 1_000
PACK_CATALOGUE_SECTIONS = frozenset({"all", "15", "29"})
ORACLE_TIERS = frozenset({"deterministic", "executable", "policy_veto", "statistical", "expert_review", "rubric"})
PACK_AXES = frozenset({"mechanism", "domain", "platform"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, structured MCP content, or an HTTP REST tool envelope."""

    raw = _route_mapping("pack catalogue response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return candidate.get("ok") is True and isinstance(candidate.get("returned"), list) and isinstance(candidate.get("section_counts"), Mapping)

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
            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"pack catalogue response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a pack catalogue projection")


@dataclass(frozen=True)
class PackCatalogueArgs:
    section: str = "all"
    max_items: int = 100

    def __post_init__(self) -> None:
        section = _route_text("pack catalogue section", self.section)
        if section not in PACK_CATALOGUE_SECTIONS:
            raise ArgumentError("pack catalogue section must be all, 15, or 29")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= PACK_CATALOGUE_MAX_ITEMS:
            raise ArgumentError(f"pack catalogue max_items must be between 1 and {PACK_CATALOGUE_MAX_ITEMS}")
        object.__setattr__(self, "section", section)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCatalogueArgs":
        raw = _route_mapping("pack catalogue arguments", value)
        return cls(raw.get("section", "all"), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"section": self.section, "max_items": self.max_items}


@dataclass(frozen=True)
class PackCatalogueEntryReport:
    raw: dict[str, Any]
    id: str
    title: str
    blueprint_module: str
    axis: str
    measures: str
    capabilities: tuple[str, ...]
    domains: tuple[str, ...]
    decision_families: tuple[str, ...]
    oracles: tuple[str, ...]
    strongest_oracle: str | None
    has_execution_grounded_oracle: bool
    release_wave: int | None
    capability_signature: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCatalogueEntryReport":
        raw = _route_mapping("pack catalogue entry", value)
        oracles = _route_strings("pack catalogue entry oracles", raw.get("oracles"))
        unknown = set(oracles) - ORACLE_TIERS
        if unknown:
            raise ArgumentError(f"unknown pack catalogue oracle tier(s): {sorted(unknown)!r}")
        strongest = _optional_text("pack catalogue strongest_oracle", raw.get("strongest_oracle"))
        if strongest is not None and strongest not in ORACLE_TIERS:
            raise ArgumentError(f"unknown pack catalogue strongest oracle {strongest!r}")
        release_raw = raw.get("release_wave")
        release_wave: int | None
        if release_raw is None or release_raw == "unsequenced":
            release_wave = None
        else:
            release = _route_mapping("pack catalogue release_wave", release_raw)
            release_wave = _route_count("pack catalogue release_wave.wave", release.get("wave"))
            if not 1 <= release_wave <= 8:
                raise ArgumentError("pack catalogue release wave must be between 1 and 8")
        axis = _route_text("pack catalogue entry axis", raw.get("axis"))
        if axis not in PACK_AXES:
            raise ArgumentError(f"unknown pack catalogue axis {axis!r}")
        return cls(
            raw,
            _route_text("pack catalogue entry id", raw.get("id")),
            _route_text("pack catalogue entry title", raw.get("title")),
            _route_text("pack catalogue entry blueprint_module", raw.get("blueprint_module")),
            axis,
            _route_text("pack catalogue entry measures", raw.get("measures")),
            _route_strings("pack catalogue entry capabilities", raw.get("capabilities")),
            _route_strings("pack catalogue entry domains", raw.get("domains")),
            _route_strings("pack catalogue entry decision_families", raw.get("decision_families")),
            oracles,
            strongest,
            _bool("pack catalogue entry has_execution_grounded_oracle", raw.get("has_execution_grounded_oracle")),
            release_wave,
            _route_text("pack catalogue entry capability_signature", raw.get("capability_signature")),
        )

    @property
    def is_sequenced(self) -> bool:
        return self.release_wave is not None

    @property
    def declaration_only(self) -> bool:
        return True


@dataclass(frozen=True)
class PackDuplicateSignatureReport:
    raw: dict[str, Any]
    signature: str
    pack_ids: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackDuplicateSignatureReport":
        raw = _route_mapping("pack duplicate signature", value)
        pack_ids = _route_strings("pack duplicate signature pack_ids", raw.get("pack_ids"))
        if len(pack_ids) < 2:
            raise ArgumentError("duplicate pack signatures must contain at least two pack ids")
        return cls(raw, _route_text("pack duplicate signature signature", raw.get("signature")), pack_ids)


@dataclass(frozen=True)
class PackCatalogueReport:
    raw: dict[str, Any]
    ok: bool
    section: str
    portfolio_count: int
    section_15_count: int
    section_29_count: int
    returned: tuple[PackCatalogueEntryReport, ...]
    omitted: int
    duplicate_signature_groups: tuple[PackDuplicateSignatureReport, ...]
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCatalogueReport":
        raw = _payload(value)
        section = _route_text("pack catalogue section", raw.get("section"))
        if section not in PACK_CATALOGUE_SECTIONS:
            raise ArgumentError(f"unknown pack catalogue section {section!r}")
        returned = tuple(PackCatalogueEntryReport.from_wire(item) for item in raw.get("returned", []))
        counts = _route_mapping("pack catalogue section_counts", raw.get("section_counts"))
        duplicate_groups = tuple(PackDuplicateSignatureReport.from_wire(item) for item in raw.get("duplicate_signature_groups", []))
        portfolio_count = _route_count("pack catalogue portfolio_count", raw.get("portfolio_count"))
        omitted = _route_count("pack catalogue omitted", raw.get("omitted"))
        if len(returned) + omitted > portfolio_count:
            raise ArgumentError("pack catalogue returned and omitted counts exceed portfolio count")
        return cls(
            raw,
            _bool("pack catalogue ok", raw.get("ok")),
            section,
            portfolio_count,
            _route_count("pack catalogue section_counts.15", counts.get("15")),
            _route_count("pack catalogue section_counts.29", counts.get("29")),
            returned,
            omitted,
            duplicate_groups,
            _route_strings("pack catalogue guarantees", raw.get("guarantees", [])),
        )

    @property
    def complete_for_request(self) -> bool:
        return self.omitted == 0

    @property
    def declaration_only(self) -> bool:
        return all(entry.declaration_only for entry in self.returned)

    @property
    def duplicate_review_count(self) -> int:
        return len(self.duplicate_signature_groups)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def pack_catalogue_report(value: Mapping[str, Any]) -> PackCatalogueReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return PackCatalogueReport.from_wire(value)


__all__ = [
    "PACK_CATALOGUE_MAX_ITEMS",
    "PACK_CATALOGUE_SECTIONS",
    "ORACLE_TIERS",
    "PACK_AXES",
    "PackCatalogueArgs",
    "PackCatalogueEntryReport",
    "PackDuplicateSignatureReport",
    "PackCatalogueReport",
    "pack_catalogue_report",
]
