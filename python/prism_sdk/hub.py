"""Typed federated-hub discovery contracts.

The hub search kernel already keeps exact facet reasons, registry authority, trust tier, digest,
and freshness in every result.  This module makes those distinctions survive the Python boundary:
near misses are not collapsed into absent rows, mirrors are not treated as origins, and a bounded
or truncated response cannot masquerade as an exhaustive catalog search.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


HUB_DEFAULT_MAX_ITEMS = 100
HUB_MAX_ITEMS = 1_000
HUB_MAX_CATALOGS = 100
HUB_MAX_RELEASES = 10_000
HUB_TRUST_TIERS = frozenset({"unranked", "exploratory", "generated_verified", "reviewed", "gold"})
HUB_AUTHORITY_KINDS = frozenset({"authoritative", "carried"})
HUB_FRESHNESS_KINDS = frozenset(
    {"authoritative", "within_bound", "beyond_bound", "undetermined", "ahead_of_reference"}
)
HUB_WHY_KINDS = frozenset(
    {
        "namespace_matched",
        "keyword_matched",
        "term_in_name",
        "term_in_summary",
        "tier_met",
        "dependency_matched",
        "usable_by_a_new_dependent",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _bounded_max_items(name: str, value: Any) -> int:
    result = _route_count(name, value)
    if not 1 <= result <= HUB_MAX_ITEMS:
        raise ArgumentError(f"{name} must be between 1 and {HUB_MAX_ITEMS}")
    return result


def _array_of_mappings(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array of objects")
    values = tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(value))
    return values


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a structured hub result from direct, MCP, or HTTP output."""

    raw = _route_mapping("hub search response", value)
    required = (
        "ok",
        "catalog_count",
        "release_count",
        "requested_limit",
        "effective_limit",
        "matches",
        "match_count",
        "excluded",
        "excluded_count",
        "omitted_excluded",
        "truncated",
        "guarantees",
        "limitations",
    )
    if all(key in raw for key in required):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and all(key in structured for key in required):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"hub search response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded hub search response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError("response does not contain a hub search projection")


@dataclass(frozen=True)
class HubSearchArgs:
    """Bounded search over caller-supplied federation and catalog evidence."""

    federation: Mapping[str, Any]
    catalogs: tuple[Mapping[str, Any], ...]
    query: Mapping[str, Any]
    max_items: int = HUB_DEFAULT_MAX_ITEMS

    def __init__(
        self,
        federation: Mapping[str, Any],
        catalogs: Sequence[Mapping[str, Any]],
        query: Mapping[str, Any],
        max_items: int = HUB_DEFAULT_MAX_ITEMS,
    ) -> None:
        if not isinstance(federation, Mapping):
            raise ArgumentError("federation must be an object")
        if not isinstance(query, Mapping):
            raise ArgumentError("query must be an object")
        normalized_catalogs = _array_of_mappings("catalogs", catalogs)
        if len(normalized_catalogs) > HUB_MAX_CATALOGS:
            raise ArgumentError(f"catalogs must contain at most {HUB_MAX_CATALOGS} catalogs")
        for index, catalog in enumerate(normalized_catalogs):
            releases = catalog.get("releases")
            if isinstance(releases, Mapping) and len(releases) > HUB_MAX_RELEASES:
                raise ArgumentError(f"catalogs[{index}].releases exceeds {HUB_MAX_RELEASES} releases")
        _bounded_max_items("max_items", max_items)
        object.__setattr__(self, "federation", dict(federation))
        object.__setattr__(self, "catalogs", normalized_catalogs)
        object.__setattr__(self, "query", dict(query))
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubSearchArgs":
        raw = _route_mapping("hub search arguments", value)
        return cls(
            raw.get("federation"),
            raw.get("catalogs"),
            raw.get("query"),
            raw.get("max_items", HUB_DEFAULT_MAX_ITEMS),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "federation": dict(self.federation),
            "catalogs": [dict(catalog) for catalog in self.catalogs],
            "query": dict(self.query),
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class HubAuthorityReport:
    """Registry standing attached to one discovered release."""

    raw: dict[str, Any]
    kind: str
    registry: str
    mirror: str | None
    origin: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubAuthorityReport":
        raw = _route_mapping("hub match authority", value)
        kind = _route_text("hub authority kind", raw.get("authority"))
        if kind not in HUB_AUTHORITY_KINDS:
            raise ArgumentError(f"unknown hub authority kind: {kind!r}")
        if kind == "authoritative":
            registry = _route_text("hub authoritative registry", raw.get("registry"))
            if "mirror" in raw or "origin" in raw:
                raise ArgumentError("authoritative hub authority cannot carry mirror provenance")
            return cls(raw, kind, registry, None, None)
        mirror = _route_text("hub carried mirror", raw.get("mirror"))
        origin = _route_text("hub carried origin", raw.get("origin"))
        if mirror == origin:
            raise ArgumentError("carried hub authority must distinguish mirror and origin")
        return cls(raw, kind, mirror, mirror, origin)

    @property
    def authoritative(self) -> bool:
        return self.kind == "authoritative"

    @property
    def answered_by(self) -> str:
        return self.registry

    @property
    def decision_owner(self) -> str:
        return self.registry if self.authoritative else self.origin  # type: ignore[return-value]


@dataclass(frozen=True)
class HubStalenessBoundReport:
    raw: dict[str, Any]
    max_lag_epochs: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubStalenessBoundReport":
        raw = _route_mapping("hub freshness bound", value)
        return cls(raw, _route_count("hub freshness max_lag_epochs", raw.get("max_lag_epochs")))


@dataclass(frozen=True)
class HubFreshnessReport:
    """Freshness claim without collapsing origin, checked mirrors, and unknown currency."""

    raw: dict[str, Any]
    kind: str
    lag: int | None
    bound: HubStalenessBoundReport | None
    synced_at: int | None
    reference: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubFreshnessReport":
        raw = _route_mapping("hub match freshness", value)
        kind = _route_text("hub freshness kind", raw.get("freshness"))
        if kind not in HUB_FRESHNESS_KINDS:
            raise ArgumentError(f"unknown hub freshness kind: {kind!r}")
        if kind == "authoritative":
            return cls(raw, kind, None, None, None, None)
        if kind in {"within_bound", "beyond_bound"}:
            return cls(
                raw,
                kind,
                _route_count("hub freshness lag", raw.get("lag")),
                HubStalenessBoundReport.from_wire(raw.get("bound")),
                _route_count("hub freshness synced_at", raw.get("synced_at")),
                None,
            )
        if kind == "undetermined":
            return cls(
                raw,
                kind,
                None,
                HubStalenessBoundReport.from_wire(raw.get("bound")),
                _route_count("hub freshness synced_at", raw.get("synced_at")),
                None,
            )
        return cls(
            raw,
            kind,
            None,
            None,
            _route_count("hub freshness synced_at", raw.get("synced_at")),
            _route_count("hub freshness reference", raw.get("reference")),
        )

    @property
    def from_authority(self) -> bool:
        return self.kind == "authoritative"

    @property
    def within_declared_bound(self) -> bool:
        return self.kind in {"authoritative", "within_bound"}

    @property
    def undetermined(self) -> bool:
        return self.kind in {"undetermined", "ahead_of_reference"}


@dataclass(frozen=True)
class HubWhyReport:
    raw: dict[str, Any]
    kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubWhyReport":
        raw = _route_mapping("hub match reason", value)
        kind = _route_text("hub match reason kind", raw.get("why"))
        if kind not in HUB_WHY_KINDS:
            raise ArgumentError(f"unknown hub match reason: {kind!r}")
        return cls(raw, kind)


@dataclass(frozen=True)
class HubMatchReport:
    raw: dict[str, Any]
    name: str
    version: str
    digest: str
    summary: str
    tier: str
    authority: HubAuthorityReport
    freshness: HubFreshnessReport
    why: tuple[HubWhyReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubMatchReport":
        raw = _route_mapping("hub match", value)
        raw_why = raw.get("why")
        if not isinstance(raw_why, Sequence) or isinstance(raw_why, (str, bytes)):
            raise ArgumentError("hub match why must be an array")
        why = tuple(HubWhyReport.from_wire(item) for item in raw_why)
        if not why:
            raise ArgumentError("hub matches must carry at least one facet reason")
        tier = _route_text("hub match tier", raw.get("tier"))
        if tier not in HUB_TRUST_TIERS:
            raise ArgumentError(f"unknown hub trust tier: {tier!r}")
        return cls(
            raw,
            _route_text("hub match name", raw.get("name")),
            _route_text("hub match version", raw.get("version")),
            _route_text("hub match digest", raw.get("digest")),
            _route_text("hub match summary", raw.get("summary")),
            tier,
            HubAuthorityReport.from_wire(raw.get("authority")),
            HubFreshnessReport.from_wire(raw.get("freshness")),
            why,
        )


@dataclass(frozen=True)
class HubExcludedReport:
    raw: dict[str, Any]
    name: str
    version: str
    failed: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubExcludedReport":
        raw = _route_mapping("hub excluded release", value)
        return cls(
            raw,
            _route_text("hub excluded name", raw.get("name")),
            _route_text("hub excluded version", raw.get("version")),
            _route_text("hub excluded facet", raw.get("failed")),
        )


@dataclass(frozen=True)
class HubSearchReport:
    """Bounded hub search evidence with match/exclusion and provenance invariants."""

    raw: dict[str, Any]
    ok: bool
    catalog_count: int
    release_count: int
    requested_limit: int | None
    effective_limit: int
    matches: tuple[HubMatchReport, ...]
    match_count: int
    excluded: tuple[HubExcludedReport, ...]
    excluded_count: int
    omitted_excluded: int
    truncated: bool
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubSearchReport":
        raw = _payload(value)
        if not _bool("hub search ok", raw.get("ok")):
            raise ArgumentError("hub search report is not successful")
        catalog_count = _route_count("hub catalog_count", raw.get("catalog_count"))
        release_count = _route_count("hub release_count", raw.get("release_count"))
        if catalog_count > HUB_MAX_CATALOGS:
            raise ArgumentError("hub catalog_count exceeds the server bound")
        if release_count > HUB_MAX_RELEASES:
            raise ArgumentError("hub release_count exceeds the server bound")
        requested_value = raw.get("requested_limit")
        requested_limit = None if requested_value is None else _route_count("hub requested_limit", requested_value)
        effective_limit = _bounded_max_items("hub effective_limit", raw.get("effective_limit"))
        raw_matches = _array_of_mappings("hub matches", raw.get("matches"))
        matches = tuple(HubMatchReport.from_wire(item) for item in raw_matches)
        match_count = _route_count("hub match_count", raw.get("match_count"))
        if match_count != len(matches):
            raise ArgumentError("hub match_count does not reconcile with visible matches")
        if match_count > effective_limit:
            raise ArgumentError("hub visible matches exceed the effective limit")
        match_keys = tuple((match.name, match.version) for match in matches)
        if len(match_keys) != len(set(match_keys)):
            raise ArgumentError("hub matches must be unique by name and version")
        raw_excluded = _array_of_mappings("hub excluded", raw.get("excluded"))
        excluded = tuple(HubExcludedReport.from_wire(item) for item in raw_excluded)
        excluded_count = _route_count("hub excluded_count", raw.get("excluded_count"))
        omitted_excluded = _route_count("hub omitted_excluded", raw.get("omitted_excluded"))
        if excluded_count < len(excluded) or omitted_excluded != excluded_count - len(excluded):
            raise ArgumentError("hub excluded counts do not reconcile")
        truncated = _bool("hub truncated", raw.get("truncated"))
        if omitted_excluded and not truncated:
            raise ArgumentError("hub omitted exclusions require a truncated response")
        return cls(
            raw,
            True,
            catalog_count,
            release_count,
            requested_limit,
            effective_limit,
            matches,
            match_count,
            excluded,
            excluded_count,
            omitted_excluded,
            truncated,
            _route_strings("hub guarantees", raw.get("guarantees")),
            _route_strings("hub limitations", raw.get("limitations")),
        )

    @property
    def exhaustive(self) -> bool:
        return not self.truncated

    @property
    def authoritative_match_count(self) -> int:
        return sum(match.authority.authoritative for match in self.matches)

    @property
    def undetermined_freshness_count(self) -> int:
        return sum(match.freshness.undetermined for match in self.matches)


def hub_search_report(value: Mapping[str, Any]) -> HubSearchReport:
    """Parse direct MCP or HTTP federated hub-search output."""

    return HubSearchReport.from_wire(value)


__all__ = [
    "HUB_AUTHORITY_KINDS",
    "HUB_DEFAULT_MAX_ITEMS",
    "HUB_FRESHNESS_KINDS",
    "HUB_MAX_CATALOGS",
    "HUB_MAX_ITEMS",
    "HUB_MAX_RELEASES",
    "HUB_TRUST_TIERS",
    "HUB_WHY_KINDS",
    "HubAuthorityReport",
    "HubExcludedReport",
    "HubFreshnessReport",
    "HubMatchReport",
    "HubSearchArgs",
    "HubSearchReport",
    "HubStalenessBoundReport",
    "HubWhyReport",
    "hub_search_report",
]
