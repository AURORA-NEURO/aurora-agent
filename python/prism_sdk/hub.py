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


def _normalize_hub_inputs(
    federation: Any,
    catalogs: Any,
) -> tuple[dict[str, Any], tuple[dict[str, Any], ...]]:
    if not isinstance(federation, Mapping):
        raise ArgumentError("federation must be an object")
    normalized_catalogs = _array_of_mappings("catalogs", catalogs)
    if len(normalized_catalogs) > HUB_MAX_CATALOGS:
        raise ArgumentError(f"catalogs must contain at most {HUB_MAX_CATALOGS} catalogs")
    for index, catalog in enumerate(normalized_catalogs):
        releases = catalog.get("releases")
        if isinstance(releases, Mapping) and len(releases) > HUB_MAX_RELEASES:
            raise ArgumentError(f"catalogs[{index}].releases exceeds {HUB_MAX_RELEASES} releases")
    return dict(federation), normalized_catalogs


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


def _payload_for_keys(
    value: Mapping[str, Any],
    required: tuple[str, ...],
    label: str,
) -> dict[str, Any]:
    """Extract a projection for the resolution and lock response families."""

    raw = _route_mapping(f"{label} response", value)
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
                    raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {label} projection")


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
        if not isinstance(query, Mapping):
            raise ArgumentError("query must be an object")
        normalized_federation, normalized_catalogs = _normalize_hub_inputs(federation, catalogs)
        _bounded_max_items("max_items", max_items)
        object.__setattr__(self, "federation", normalized_federation)
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
class HubResolveArgs:
    """One federated resolution request with explicit lifecycle and freshness policy JSON."""

    federation: Mapping[str, Any]
    catalogs: tuple[Mapping[str, Any], ...]
    request: Mapping[str, Any]

    def __init__(
        self,
        federation: Mapping[str, Any],
        catalogs: Sequence[Mapping[str, Any]],
        request: Mapping[str, Any],
    ) -> None:
        normalized_federation, normalized_catalogs = _normalize_hub_inputs(federation, catalogs)
        if not isinstance(request, Mapping):
            raise ArgumentError("hub resolution request must be an object")
        object.__setattr__(self, "federation", normalized_federation)
        object.__setattr__(self, "catalogs", normalized_catalogs)
        object.__setattr__(self, "request", dict(request))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubResolveArgs":
        raw = _route_mapping("hub resolve arguments", value)
        return cls(raw.get("federation"), raw.get("catalogs"), raw.get("request"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "federation": dict(self.federation),
            "catalogs": [dict(catalog) for catalog in self.catalogs],
            "request": dict(self.request),
        }


@dataclass(frozen=True)
class HubLockArgs:
    """Bounded transitive dependency-lock request over the same federation inputs."""

    federation: Mapping[str, Any]
    catalogs: tuple[Mapping[str, Any], ...]
    request: Mapping[str, Any]
    max_items: int = HUB_DEFAULT_MAX_ITEMS

    def __init__(
        self,
        federation: Mapping[str, Any],
        catalogs: Sequence[Mapping[str, Any]],
        request: Mapping[str, Any],
        max_items: int = HUB_DEFAULT_MAX_ITEMS,
    ) -> None:
        normalized_federation, normalized_catalogs = _normalize_hub_inputs(federation, catalogs)
        if not isinstance(request, Mapping):
            raise ArgumentError("hub lock request must be an object")
        _bounded_max_items("hub lock max_items", max_items)
        object.__setattr__(self, "federation", normalized_federation)
        object.__setattr__(self, "catalogs", normalized_catalogs)
        object.__setattr__(self, "request", dict(request))
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLockArgs":
        raw = _route_mapping("hub lock arguments", value)
        return cls(
            raw.get("federation"),
            raw.get("catalogs"),
            raw.get("request"),
            raw.get("max_items", HUB_DEFAULT_MAX_ITEMS),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "federation": dict(self.federation),
            "catalogs": [dict(catalog) for catalog in self.catalogs],
            "request": dict(self.request),
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
class HubFreshnessPolicyReport:
    raw: dict[str, Any]
    require_authority: bool
    accept_undetermined: bool
    accept_beyond_bound: bool
    max_accepted_lag: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubFreshnessPolicyReport":
        raw = _route_mapping("hub accepted freshness policy", value)
        maximum = raw.get("max_accepted_lag")
        return cls(
            raw,
            _bool("hub freshness require_authority", raw.get("require_authority")),
            _bool("hub freshness accept_undetermined", raw.get("accept_undetermined")),
            _bool("hub freshness accept_beyond_bound", raw.get("accept_beyond_bound")),
            None if maximum is None else _route_count("hub freshness max_accepted_lag", maximum),
        )


@dataclass(frozen=True)
class HubLifecycleNoteReport:
    raw: dict[str, Any]
    kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLifecycleNoteReport":
        raw = _route_mapping("hub lifecycle note", value)
        kind = _route_text("hub lifecycle note kind", raw.get("note"))
        if kind not in {"yanked_but_pinned", "deprecated"}:
            raise ArgumentError(f"unknown hub lifecycle note: {kind!r}")
        if kind == "yanked_but_pinned":
            _route_text("hub yanked note reason", raw.get("reason"))
            _route_count("hub yanked note epoch", raw.get("epoch"))
        else:
            for field_name in ("stage", "replacement", "reason"):
                _route_text(f"hub deprecated note {field_name}", raw.get(field_name))
        return cls(raw, kind)


@dataclass(frozen=True)
class HubResolutionSubjectReport:
    raw: dict[str, Any]
    name: str
    version: str
    digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubResolutionSubjectReport":
        raw = _route_mapping("hub resolution subject", value)
        return cls(
            raw,
            _route_text("hub resolved name", raw.get("name")),
            _route_text("hub resolved version", raw.get("version")),
            _route_text("hub resolved digest", raw.get("digest")),
        )


@dataclass(frozen=True)
class HubResolutionReport:
    raw: dict[str, Any]
    subject: HubResolutionSubjectReport
    authority: HubAuthorityReport
    freshness: HubFreshnessReport
    accepted_under: HubFreshnessPolicyReport
    notes: tuple[HubLifecycleNoteReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubResolutionReport":
        raw = _route_mapping("hub resolution", value)
        provenance = _route_mapping("hub resolution provenance", raw.get("provenance"))
        return cls(
            raw,
            HubResolutionSubjectReport.from_wire(raw.get("subject")),
            HubAuthorityReport.from_wire(provenance.get("authority")),
            HubFreshnessReport.from_wire(provenance.get("freshness")),
            HubFreshnessPolicyReport.from_wire(provenance.get("accepted_under")),
            tuple(
                HubLifecycleNoteReport.from_wire(item)
                for item in _array_of_mappings("hub resolution notes", provenance.get("notes", []))
            ),
        )

    @property
    def digest(self) -> str:
        return self.subject.digest

    @property
    def answered_by(self) -> str:
        return self.authority.answered_by

    @property
    def authoritative(self) -> bool:
        return self.authority.authoritative


@dataclass(frozen=True)
class HubResolveReport:
    raw: dict[str, Any]
    ok: bool
    resolution: HubResolutionReport
    answered_by: str
    authoritative: bool
    catalog_count: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubResolveReport":
        raw = _payload_for_keys(
            value,
            ("ok", "resolution", "answered_by", "authoritative", "catalog_count", "guarantees", "limitations"),
            "hub resolve",
        )
        if not _bool("hub resolve ok", raw.get("ok")):
            raise ArgumentError("hub resolve report is not successful")
        resolution = HubResolutionReport.from_wire(raw.get("resolution"))
        answered_by = _route_text("hub resolve answered_by", raw.get("answered_by"))
        authoritative = _bool("hub resolve authoritative", raw.get("authoritative"))
        if answered_by != resolution.answered_by or authoritative != resolution.authoritative:
            raise ArgumentError("hub resolve top-level provenance does not reconcile with resolution")
        return cls(
            raw,
            True,
            resolution,
            answered_by,
            authoritative,
            _route_count("hub resolve catalog_count", raw.get("catalog_count")),
            _route_strings("hub resolve guarantees", raw.get("guarantees")),
            _route_strings("hub resolve limitations", raw.get("limitations")),
        )


@dataclass(frozen=True)
class HubVersionRequirementReport:
    raw: dict[str, Any]
    kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubVersionRequirementReport":
        raw = _route_mapping("hub dependency version requirement", value)
        kind = _route_text("hub dependency requirement kind", raw.get("req"))
        if kind not in {"exact", "at_least", "compatible", "approximately", "range", "any"}:
            raise ArgumentError(f"unknown hub dependency requirement: {kind!r}")
        if kind == "any":
            if "spec" in raw:
                raise ArgumentError("any dependency requirement cannot carry a spec")
        elif raw.get("spec") is None:
            raise ArgumentError(f"{kind} dependency requirement must carry a spec")
        elif kind == "range":
            spec = _route_mapping("hub dependency range spec", raw.get("spec"))
            _route_text("hub dependency range low", spec.get("low"))
            _route_text("hub dependency range high", spec.get("high"))
        else:
            _route_text("hub dependency version spec", raw.get("spec"))
        return cls(raw, kind)


@dataclass(frozen=True)
class HubRequirementSourceReport:
    raw: dict[str, Any]
    kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubRequirementSourceReport":
        raw = _route_mapping("hub dependency requirement source", value)
        kind = _route_text("hub dependency source kind", raw.get("source"))
        if kind not in {"root", "pack"}:
            raise ArgumentError(f"unknown hub dependency source: {kind!r}")
        if kind == "pack":
            _route_text("hub dependency source name", raw.get("name"))
            _route_text("hub dependency source version", raw.get("version"))
        return cls(raw, kind)


@dataclass(frozen=True)
class HubRequirementReport:
    raw: dict[str, Any]
    on: str
    requirement: HubVersionRequirementReport
    source: HubRequirementSourceReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubRequirementReport":
        raw = _route_mapping("hub dependency requirement", value)
        return cls(
            raw,
            _route_text("hub dependency requirement on", raw.get("on")),
            HubVersionRequirementReport.from_wire(raw.get("req")),
            HubRequirementSourceReport.from_wire(raw.get("source")),
        )


@dataclass(frozen=True)
class HubLockEntryReport:
    raw: dict[str, Any]
    name: str
    resolution: HubResolutionReport
    required_by: tuple[HubRequirementReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLockEntryReport":
        raw = _route_mapping("hub lock entry", value)
        name = _route_text("hub lock entry name", raw.get("name"))
        locked = _route_mapping("hub locked payload", raw.get("locked"))
        required_by = tuple(
            HubRequirementReport.from_wire(item)
            for item in _array_of_mappings("hub locked required_by", locked.get("required_by", []))
        )
        if not required_by:
            raise ArgumentError("hub lock entries must retain required_by provenance")
        resolution = HubResolutionReport.from_wire(locked.get("resolution"))
        if resolution.subject.name != name:
            raise ArgumentError("hub lock entry name does not match its resolution subject")
        return cls(raw, name, resolution, required_by)


@dataclass(frozen=True)
class HubLockReport:
    raw: dict[str, Any]
    ok: bool
    entry_count: int
    fully_authoritative: bool
    answering_registries: tuple[str, ...]
    remarked_entry_count: int
    entries: tuple[HubLockEntryReport, ...]
    omitted_entries: int
    max_items: int
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubLockReport":
        raw = _payload_for_keys(
            value,
            (
                "ok",
                "entry_count",
                "fully_authoritative",
                "answering_registries",
                "remarked_entry_count",
                "entries",
                "omitted_entries",
                "max_items",
                "guarantees",
            ),
            "hub lock",
        )
        if not _bool("hub lock ok", raw.get("ok")):
            raise ArgumentError("hub lock report is not successful")
        entry_count = _route_count("hub lock entry_count", raw.get("entry_count"))
        max_items = _bounded_max_items("hub lock max_items", raw.get("max_items"))
        entries = tuple(HubLockEntryReport.from_wire(item) for item in _array_of_mappings("hub lock entries", raw.get("entries")))
        omitted_entries = _route_count("hub lock omitted_entries", raw.get("omitted_entries"))
        if entry_count < len(entries) or omitted_entries != entry_count - len(entries):
            raise ArgumentError("hub lock entry counts do not reconcile")
        if len(entries) > max_items:
            raise ArgumentError("hub lock visible entries exceed max_items")
        names = tuple(entry.name for entry in entries)
        if len(names) != len(set(names)):
            raise ArgumentError("hub lock entries must be unique by name")
        registries = _route_strings("hub lock answering_registries", raw.get("answering_registries"))
        visible_registries = {entry.resolution.answered_by for entry in entries}
        if not visible_registries.issubset(set(registries)):
            raise ArgumentError("hub lock answering_registries omits a visible answerer")
        remarked_entry_count = _route_count("hub lock remarked_entry_count", raw.get("remarked_entry_count"))
        visible_remarks = sum(bool(entry.resolution.notes) for entry in entries)
        if remarked_entry_count < visible_remarks:
            raise ArgumentError("hub lock remarked_entry_count omits a visible lifecycle note")
        return cls(
            raw,
            True,
            entry_count,
            _bool("hub lock fully_authoritative", raw.get("fully_authoritative")),
            registries,
            remarked_entry_count,
            entries,
            omitted_entries,
            max_items,
            _route_strings("hub lock guarantees", raw.get("guarantees")),
        )

    @property
    def exhaustive(self) -> bool:
        return self.omitted_entries == 0


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


def hub_resolve_report(value: Mapping[str, Any]) -> HubResolveReport:
    """Parse direct MCP or HTTP federated resolution output."""

    return HubResolveReport.from_wire(value)


def hub_lock_report(value: Mapping[str, Any]) -> HubLockReport:
    """Parse direct MCP or HTTP dependency-lock output."""

    return HubLockReport.from_wire(value)


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
    "HubFreshnessPolicyReport",
    "HubLifecycleNoteReport",
    "HubLockArgs",
    "HubLockEntryReport",
    "HubLockReport",
    "HubMatchReport",
    "HubRequirementReport",
    "HubRequirementSourceReport",
    "HubResolutionReport",
    "HubResolutionSubjectReport",
    "HubResolveArgs",
    "HubResolveReport",
    "HubSearchArgs",
    "HubSearchReport",
    "HubStalenessBoundReport",
    "HubVersionRequirementReport",
    "HubWhyReport",
    "hub_lock_report",
    "hub_resolve_report",
    "hub_search_report",
]
