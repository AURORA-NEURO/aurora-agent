"""Typed projections for cache keys, invalidation completeness, and replayable misses.

The cache authority deliberately separates a cold miss, a cross-build refusal, an unproven entry,
and a key collision.  This module preserves those distinctions while leaving key construction,
dependency traversal, and cache application in Rust.  In particular, a partial invalidation is
never represented as a clean plan, and an explicit apply is never confused with a dry run.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


CACHE_INVALIDATION_MAX_INPUT_BYTES = 20_000_000
CACHE_INVALIDATION_MAX_COMPONENTS = 128
CACHE_INVALIDATION_MAX_ITEMS = 1_000
CACHE_INVALIDATION_MAX_GRAPH_ROWS = 2_000
CACHE_REUSE_RULES = frozenset({"SameBuildOnly", "AcrossBuilds", "same_build_only", "across_builds", "same-build-only", "across-builds"})
CACHE_MISS_NAMES = frozenset({"no-entry", "schema-changed", "cross-build", "unproven"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("cache invalidation response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"cache invalidation response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("ok") is True and candidate.get("schema") == "bioprism-mcp/cache-invalidation/0.1" and isinstance(candidate.get("entries"), Mapping) and isinstance(candidate.get("cache"), Mapping):
            return dict(candidate)
    raise ArgumentError("response does not contain a cache invalidation projection")


@dataclass(frozen=True)
class CacheInvalidationSimulateArgs:
    schema: dict[str, Any]
    entries: tuple[Any, ...] = ()
    graph: dict[str, Any] | None = None
    changed: str | None = None
    lookups: tuple[Any, ...] = ()
    apply: bool = False
    apply_at: int | None = None
    reprove: tuple[Any, ...] = ()
    max_items: int = 100

    def __init__(self, schema: Mapping[str, Any], entries: Sequence[Any] = (), graph: Mapping[str, Any] | None = None, changed: str | None = None, lookups: Sequence[Any] = (), apply: bool = False, apply_at: int | None = None, reprove: Sequence[Any] = (), max_items: int = 100) -> None:
        normalized_schema = dict(schema) if isinstance(schema, Mapping) else _route_mapping("cache schema", schema)
        name = _route_text("cache schema.name", normalized_schema.get("name"))
        components = _sequence("cache schema.components", normalized_schema.get("components"))
        if not 1 <= len(components) <= CACHE_INVALIDATION_MAX_COMPONENTS:
            raise ArgumentError("cache schema.components must contain between 1 and 128 names")
        for index, component in enumerate(components):
            _route_text(f"cache schema.components[{index}]", component)
        reuse = normalized_schema.get("reuse", "same_build_only")
        if not isinstance(reuse, str) or reuse not in CACHE_REUSE_RULES:
            raise ArgumentError("cache schema.reuse must be same_build_only or across_builds")
        normalized_entries = _sequence("cache entries", entries)
        normalized_lookups = _sequence("cache lookups", lookups)
        normalized_reprove = _sequence("cache reprove", reprove)
        if len(normalized_entries) > CACHE_INVALIDATION_MAX_ITEMS:
            raise ArgumentError("cache entries must contain at most 1000 entries")
        if len(normalized_lookups) > CACHE_INVALIDATION_MAX_ITEMS:
            raise ArgumentError("cache lookups must contain at most 1000 requests")
        if len(normalized_reprove) > CACHE_INVALIDATION_MAX_ITEMS:
            raise ArgumentError("cache reprove must contain at most 1000 entries")
        normalized_graph = None if graph is None else _route_mapping("cache graph", graph)
        if normalized_graph is not None:
            for field, limit in (("declared", CACHE_INVALIDATION_MAX_GRAPH_ROWS), ("opaque", CACHE_INVALIDATION_MAX_GRAPH_ROWS)):
                if field in normalized_graph and len(_sequence(f"cache graph.{field}", normalized_graph[field])) > limit:
                    raise ArgumentError(f"cache graph.{field} must contain at most 2000 rows")
        normalized_changed = None if changed is None else _route_text("cache changed resource", changed)
        normalized_apply = _bool("cache apply", apply)
        normalized_apply_at = None if apply_at is None else _integer("cache apply_at", apply_at)
        normalized_max = _integer("cache max_items", max_items)
        if not 1 <= normalized_max <= CACHE_INVALIDATION_MAX_ITEMS:
            raise ArgumentError("cache max_items must be between 1 and 1000")
        arguments = {"schema": normalized_schema, "entries": list(normalized_entries), "graph": normalized_graph, "changed": normalized_changed, "lookups": list(normalized_lookups), "apply": normalized_apply, "apply_at": normalized_apply_at, "reprove": list(normalized_reprove), "max_items": normalized_max}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"cache invalidation arguments are not JSON serializable: {error}") from error
        if len(encoded) > CACHE_INVALIDATION_MAX_INPUT_BYTES:
            raise ArgumentError("cache invalidation input exceeds the 20 MB safety bound")
        object.__setattr__(self, "schema", normalized_schema)
        object.__setattr__(self, "entries", normalized_entries)
        object.__setattr__(self, "graph", normalized_graph)
        object.__setattr__(self, "changed", normalized_changed)
        object.__setattr__(self, "lookups", normalized_lookups)
        object.__setattr__(self, "apply", normalized_apply)
        object.__setattr__(self, "apply_at", normalized_apply_at)
        object.__setattr__(self, "reprove", normalized_reprove)
        object.__setattr__(self, "max_items", normalized_max)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheInvalidationSimulateArgs":
        raw = _route_mapping("cache invalidation arguments", value)
        return cls(raw.get("schema"), raw.get("entries", []), raw.get("graph"), raw.get("changed"), raw.get("lookups", []), raw.get("apply", False), raw.get("apply_at"), raw.get("reprove", []), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"schema": dict(self.schema), "entries": list(self.entries), "lookups": list(self.lookups), "apply": self.apply, "reprove": list(self.reprove), "max_items": self.max_items}
        if self.graph is not None:
            result["graph"] = dict(self.graph)
        if self.changed is not None:
            result["changed"] = self.changed
        if self.apply_at is not None:
            result["apply_at"] = self.apply_at
        return result


@dataclass(frozen=True)
class CacheKeySchemaReport:
    raw: dict[str, Any]
    name: str
    components: tuple[str, ...]
    reuse: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheKeySchemaReport":
        raw = _route_mapping("cache key schema", value)
        components = _route_strings("cache key schema components", raw.get("components"))
        reuse = _route_text("cache key schema reuse", raw.get("reuse"))
        if reuse not in CACHE_REUSE_RULES:
            raise ArgumentError(f"unknown cache reuse rule {reuse!r}")
        return cls(raw, _route_text("cache key schema name", raw.get("name")), components, reuse)


@dataclass(frozen=True)
class CacheEntryRowReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    digest: str | None
    dependencies: Any
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheEntryRowReport":
        raw = _route_mapping("cache entry row", value)
        ok = _bool("cache entry row ok", raw.get("ok"))
        refusal = None if raw.get("refusal") is None else _route_text("cache entry row refusal", raw.get("refusal"))
        fail_closed = _bool("cache entry row fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed cache entry rows must be fail-closed")
        return cls(raw, _route_count("cache entry row index", raw.get("index")), ok, _optional_text("cache entry digest", raw.get("digest")), raw.get("dependencies"), refusal, fail_closed)


@dataclass(frozen=True)
class CacheEntriesReport:
    raw: dict[str, Any]
    accepted: int
    submitted: int
    rows: tuple[CacheEntryRowReport, ...]
    omitted_rows: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheEntriesReport":
        raw = _route_mapping("cache entries report", value)
        rows = tuple(CacheEntryRowReport.from_wire(item) for item in _sequence("cache entry rows", raw.get("rows", [])))
        accepted = _route_count("cache accepted entries", raw.get("accepted"))
        submitted = _route_count("cache submitted entries", raw.get("submitted"))
        if accepted > submitted or len(rows) > submitted:
            raise ArgumentError("cache entry counts do not reconcile")
        return cls(raw, accepted, submitted, rows, _route_count("cache omitted entry rows", raw.get("omitted_rows")))


@dataclass(frozen=True)
class CacheGraphReport:
    raw: dict[str, Any]
    known_resources: tuple[str, ...]
    known_resource_count: int
    opaque_resources: tuple[str, ...]
    cycle: tuple[str, ...] | None
    cycle_is_scheduler_defect_not_hang: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheGraphReport":
        raw = _route_mapping("cache graph report", value)
        cycle_raw = raw.get("cycle")
        cycle = None if cycle_raw is None else tuple(_route_strings("cache dependency cycle", cycle_raw))
        known = _route_strings("cache known resources", raw.get("known_resources", []))
        return cls(raw, known, _route_count("cache known_resource_count", raw.get("known_resource_count")), _route_strings("cache opaque resources", raw.get("opaque_resources", [])), cycle, _bool("cache cycle scheduler flag", raw.get("cycle_is_a_scheduler_defect_not_an_invalidation_hang")))


@dataclass(frozen=True)
class CacheUnknownRegionReport:
    raw: dict[str, Any]
    opaque_resources: tuple[str, ...]
    unknown_resources: tuple[str, ...]
    entries_without_declared_dependencies: tuple[str, ...]
    entries_depending_on_opaque_resources: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheUnknownRegionReport":
        raw = _route_mapping("cache unknown region", value)
        return cls(raw, _route_strings("cache unknown opaque resources", raw.get("opaque_resources", [])), _route_strings("cache unknown resources", raw.get("unknown_resources", [])), _route_strings("cache entries without declarations", raw.get("entries_without_declared_dependencies", [])), _route_strings("cache entries depending on opaque resources", raw.get("entries_depending_on_opaque_resources", [])))


@dataclass(frozen=True)
class CacheCompletenessReport:
    raw: Any
    kind: str
    unknown_region: CacheUnknownRegionReport | None

    @classmethod
    def from_wire(cls, value: Any) -> "CacheCompletenessReport":
        if value == "Complete":
            return cls(value, "Complete", None)
        raw = _route_mapping("cache completeness", value)
        partial = raw.get("Partial")
        if partial is None:
            raise ArgumentError("cache completeness must be Complete or Partial")
        return cls(raw, "Partial", CacheUnknownRegionReport.from_wire(partial))

    @property
    def complete(self) -> bool:
        return self.kind == "Complete"


@dataclass(frozen=True)
class CacheInvalidationPlanReport:
    raw: dict[str, Any]
    changed: str
    affected_resources: tuple[str, ...]
    invalid_entries: tuple[str, ...]
    proved_unaffected: tuple[str, ...]
    completeness: CacheCompletenessReport
    population: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheInvalidationPlanReport":
        raw = _route_mapping("cache invalidation plan", value)
        return cls(raw, _route_text("cache changed", raw.get("changed")), _route_strings("cache affected resources", raw.get("affected_resources", [])), _route_strings("cache invalid entries", raw.get("invalid_entries", [])), _route_strings("cache proved unaffected entries", raw.get("proved_unaffected", [])), CacheCompletenessReport.from_wire(raw.get("completeness")), _route_count("cache invalidation population", raw.get("population")))


@dataclass(frozen=True)
class CacheApplyReport:
    raw: dict[str, Any]
    removed: tuple[str, ...]
    marked_unproven: tuple[str, ...]
    left_proven: tuple[str, ...]
    invalidation_was_complete: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheApplyReport":
        raw = _route_mapping("cache apply report", value)
        return cls(raw, _route_strings("cache removed", raw.get("removed", [])), _route_strings("cache marked_unproven", raw.get("marked_unproven", [])), _route_strings("cache left_proven", raw.get("left_proven", [])), _bool("cache invalidation_was_complete", raw.get("invalidation_was_complete")))


@dataclass(frozen=True)
class CacheLookupReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    hit: bool | None
    value: Any
    proof: Any
    miss_reason: Any
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheLookupReport":
        raw = _route_mapping("cache lookup", value)
        ok = _bool("cache lookup ok", raw.get("ok"))
        hit = raw.get("hit")
        if hit is not None:
            hit = _bool("cache lookup hit", hit)
        refusal = _optional_text("cache lookup refusal", raw.get("refusal"))
        fail_closed = _bool("cache lookup fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed cache lookups must be fail-closed")
        return cls(raw, _route_count("cache lookup index", raw.get("index")), ok, hit, raw.get("value"), raw.get("proof"), raw.get("miss_reason"), refusal, fail_closed)

    @property
    def miss_name(self) -> str | None:
        if not isinstance(self.miss_reason, Mapping):
            return None
        names = [key for key in self.miss_reason if key in {"NoEntry", "SchemaChanged", "CrossBuild", "UnprovenAfterPartialInvalidation"}]
        return None if not names else names[0]


@dataclass(frozen=True)
class CacheReproveReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    digest: str | None
    reproved_by: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheReproveReport":
        raw = _route_mapping("cache reprove row", value)
        ok = _bool("cache reprove ok", raw.get("ok"))
        refusal = _optional_text("cache reprove refusal", raw.get("refusal"))
        fail_closed = _bool("cache reprove fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed cache reprove rows must be fail-closed")
        return cls(raw, _route_count("cache reprove index", raw.get("index")), ok, _optional_text("cache reprove digest", raw.get("digest")), _optional_text("cache reproved_by", raw.get("reproved_by")), refusal, fail_closed)


@dataclass(frozen=True)
class CacheSnapshotReport:
    raw: dict[str, Any]
    entry_count: int
    unproven: tuple[str, ...]
    hits: int
    misses_by_reason: tuple[dict[str, Any], ...]
    hit_rate: float
    entries: tuple[dict[str, Any], ...]
    omitted_entries: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheSnapshotReport":
        raw = _route_mapping("cache snapshot", value)
        hit_rate = raw.get("hit_rate")
        if not isinstance(hit_rate, (int, float)) or isinstance(hit_rate, bool) or not 0 <= hit_rate <= 1:
            raise ArgumentError("cache hit_rate must be between 0 and 1")
        return cls(raw, _route_count("cache entry_count", raw.get("entry_count")), _route_strings("cache unproven", raw.get("unproven", [])), _route_count("cache hits", raw.get("hits")), tuple(_route_mapping("cache miss count", item) for item in _sequence("cache misses_by_reason", raw.get("misses_by_reason", []))), float(hit_rate), tuple(_route_mapping("cache snapshot entry", item) for item in _sequence("cache snapshot entries", raw.get("entries", []))), _route_count("cache omitted entries", raw.get("omitted_entries")))


@dataclass(frozen=True)
class CacheInvalidationReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    max_items: int
    key_schema: CacheKeySchemaReport
    entries: CacheEntriesReport
    graph: CacheGraphReport
    changed: str | None
    plan: CacheInvalidationPlanReport | None
    apply_requested: bool
    apply: CacheApplyReport | None
    pre_apply: tuple[CacheLookupReport, ...] | None
    post_apply: tuple[CacheLookupReport, ...]
    omitted_post_apply: int
    reprove: tuple[CacheReproveReport, ...]
    cache: CacheSnapshotReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CacheInvalidationReport":
        raw = _payload(value)
        invalidation = _route_mapping("cache invalidation", raw.get("invalidation"))
        changed = _optional_text("cache invalidation changed", invalidation.get("changed"))
        plan_raw = invalidation.get("plan")
        plan = None if plan_raw is None else CacheInvalidationPlanReport.from_wire(plan_raw)
        apply_requested = _bool("cache apply_requested", invalidation.get("apply_requested"))
        apply_raw = invalidation.get("apply_report")
        apply = None if apply_raw is None else CacheApplyReport.from_wire(apply_raw)
        if apply_requested and apply is None:
            raise ArgumentError("applied cache invalidation must retain an apply report")
        lookups = _route_mapping("cache lookups report", raw.get("lookups"))
        pre_raw = lookups.get("pre_apply")
        pre_apply = None if pre_raw is None else tuple(CacheLookupReport.from_wire(item) for item in _sequence("cache pre_apply lookups", pre_raw))
        return cls(raw, _bool("cache invalidation ok", raw.get("ok")), _route_text("cache invalidation schema", raw.get("schema")), _route_count("cache invalidation max_items", raw.get("max_items")), CacheKeySchemaReport.from_wire(raw.get("key_schema")), CacheEntriesReport.from_wire(raw.get("entries")), CacheGraphReport.from_wire(raw.get("graph")), changed, plan, apply_requested, apply, pre_apply, tuple(CacheLookupReport.from_wire(item) for item in _sequence("cache post_apply lookups", lookups.get("post_apply", []))), _route_count("cache omitted post_apply", lookups.get("omitted_post_apply")), tuple(CacheReproveReport.from_wire(item) for item in _sequence("cache reprove rows", raw.get("reprove", []))), CacheSnapshotReport.from_wire(raw.get("cache")), _route_strings("cache guarantees", raw.get("guarantees", [])), _route_strings("cache limitations", raw.get("limitations", [])))

    @property
    def partial_invalidation(self) -> bool:
        return self.plan is not None and not self.plan.completeness.complete

    @property
    def explicit_dry_run(self) -> bool:
        return not self.apply_requested

    @property
    def unproven_count(self) -> int:
        return len(self.cache.unproven)

    @property
    def failed_entry_count(self) -> int:
        return sum(not row.ok for row in self.entries.rows)

    @property
    def lookup_hit_count(self) -> int:
        rows = (self.pre_apply or ()) + self.post_apply
        return sum(row.ok and row.hit is True for row in rows)

    @property
    def key_reconstruction_is_claimed(self) -> bool:
        return any("rebuilt from every declared component" in item for item in self.guarantees)

    @property
    def partial_unknowns_are_not_served(self) -> bool:
        return any("marks unknown entries unproven" in item and "optimistically" in item for item in self.guarantees)

    @property
    def reproof_is_attributed(self) -> bool:
        return any("re-proving names the digest and build" in item for item in self.guarantees)

    @property
    def side_effect_free(self) -> bool:
        return any("in-memory projections" in item and "external invalidation feed" in item for item in self.limitations)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def cache_invalidation_report(value: Mapping[str, Any]) -> CacheInvalidationReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return CacheInvalidationReport.from_wire(value)


__all__ = [
    "CACHE_INVALIDATION_MAX_INPUT_BYTES",
    "CACHE_INVALIDATION_MAX_COMPONENTS",
    "CACHE_INVALIDATION_MAX_ITEMS",
    "CACHE_INVALIDATION_MAX_GRAPH_ROWS",
    "CACHE_REUSE_RULES",
    "CACHE_MISS_NAMES",
    "CacheInvalidationSimulateArgs",
    "CacheKeySchemaReport",
    "CacheEntryRowReport",
    "CacheEntriesReport",
    "CacheGraphReport",
    "CacheUnknownRegionReport",
    "CacheCompletenessReport",
    "CacheInvalidationPlanReport",
    "CacheApplyReport",
    "CacheLookupReport",
    "CacheReproveReport",
    "CacheSnapshotReport",
    "CacheInvalidationReport",
    "cache_invalidation_report",
]
