"""Typed projections for deterministic storage tiering and quota accounting.

``storage_lifecycle_simulate`` is an in-memory planning surface, not a storage driver.  The Rust
authority owns tiering thresholds, pin handling, reserve semantics, class attribution, and
non-copyable quota delegation.  The SDK keeps those evidence planes separate: a transition plan is
not an applied move, a quota refusal is not a zero charge, and a delegated allowance is not a
copied budget.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


STORAGE_LIFECYCLE_MAX_INPUT_BYTES = 20_000_000
STORAGE_LIFECYCLE_MAX_ITEMS = 1_000
STORAGE_LIFECYCLE_MAX_DELEGATIONS = 100
STORAGE_TIERS = frozenset({"Hot", "Warm", "Cold"})
STORAGE_CLASSES = frozenset({"Objects", "Events", "Indexes", "Results", "Cache"})
STORAGE_CLASS_NAMES = frozenset({"objects", "events", "indexes", "results", "cache"})
STORAGE_PURPOSES = frozenset({"Ingest", "EvidenceFinalization", "Cleanup"})
STORAGE_PURPOSE_NAMES = frozenset({"ingest", "evidence-finalization", "cleanup"})
STORAGE_TIERING_REASONS = frozenset({"Idle", "Recent", "HeldByPin"})


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


def _mappings(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _enum(name: str, value: Any, values: frozenset[str]) -> str:
    text = _route_text(name, value)
    if text not in values:
        raise ArgumentError(f"unknown {name} {text!r}")
    return text


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("storage lifecycle response", value)
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
                            raise ArgumentError(f"storage lifecycle response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("ok") is True and isinstance(candidate.get("schema"), str) and isinstance(candidate.get("tiering"), Mapping) and isinstance(candidate.get("quota"), Mapping):
            return dict(candidate)
    raise ArgumentError("response does not contain a storage lifecycle projection")


@dataclass(frozen=True)
class StorageLifecycleSimulateArgs:
    """Bounded serialized storage inputs; Rust remains the semantic authority."""

    now: int
    tiering_policy: dict[str, Any]
    records: tuple[dict[str, Any], ...]
    quota: dict[str, Any]
    apply_tiering: bool = False
    charges: tuple[dict[str, Any], ...] = ()
    releases: tuple[dict[str, Any], ...] = ()
    delegations: tuple[dict[str, Any], ...] = ()
    absorb_delegated: tuple[Any, ...] = ()
    max_items: int = 100

    def __init__(
        self,
        now: int,
        tiering_policy: Mapping[str, Any],
        records: Sequence[Mapping[str, Any]],
        quota: Mapping[str, Any],
        apply_tiering: bool = False,
        charges: Sequence[Mapping[str, Any]] = (),
        releases: Sequence[Mapping[str, Any]] = (),
        delegations: Sequence[Mapping[str, Any]] = (),
        absorb_delegated: Sequence[Any] = (),
        max_items: int = 100,
    ) -> None:
        normalized_now = _integer("storage lifecycle now", now)
        normalized_policy = _route_mapping("storage lifecycle tiering_policy", tiering_policy)
        for field in ("demote_to_warm_after", "demote_to_cold_after", "promote_after_accesses", "promote_within"):
            _integer(f"storage lifecycle tiering_policy.{field}", normalized_policy.get(field))
        normalized_records = _mappings("storage lifecycle records", records)
        normalized_quota = _route_mapping("storage lifecycle quota", quota)
        quota_limit = _integer("storage lifecycle quota.limit", normalized_quota.get("limit"))
        quota_reserve = _integer("storage lifecycle quota.reserve", normalized_quota.get("reserve"))
        if quota_reserve >= quota_limit:
            raise ArgumentError("storage lifecycle quota.reserve must be below quota.limit")
        if len(normalized_records) > STORAGE_LIFECYCLE_MAX_ITEMS:
            raise ArgumentError("storage lifecycle records must contain at most 1000 objects")
        normalized_apply = _bool("storage lifecycle apply_tiering", apply_tiering)
        normalized_charges = _mappings("storage lifecycle charges", charges)
        normalized_releases = _mappings("storage lifecycle releases", releases)
        normalized_delegations = _mappings("storage lifecycle delegations", delegations)
        normalized_absorptions = _sequence("storage lifecycle absorb_delegated", absorb_delegated)
        if len(normalized_charges) > STORAGE_LIFECYCLE_MAX_ITEMS:
            raise ArgumentError("storage lifecycle charges must contain at most 1000 actions")
        if len(normalized_releases) > STORAGE_LIFECYCLE_MAX_ITEMS:
            raise ArgumentError("storage lifecycle releases must contain at most 1000 actions")
        if len(normalized_delegations) > STORAGE_LIFECYCLE_MAX_DELEGATIONS:
            raise ArgumentError("storage lifecycle delegations must contain at most 100 children")
        if len(normalized_absorptions) > STORAGE_LIFECYCLE_MAX_DELEGATIONS:
            raise ArgumentError("storage lifecycle absorb_delegated must contain at most 100 child indexes")
        normalized_max_items = _integer("storage lifecycle max_items", max_items)
        if not 1 <= normalized_max_items <= STORAGE_LIFECYCLE_MAX_ITEMS:
            raise ArgumentError("storage lifecycle max_items must be between 1 and 1000")
        arguments = {
            "now": normalized_now,
            "tiering_policy": normalized_policy,
            "records": list(normalized_records),
            "apply_tiering": normalized_apply,
            "quota": normalized_quota,
            "charges": list(normalized_charges),
            "releases": list(normalized_releases),
            "delegations": list(normalized_delegations),
            "absorb_delegated": list(normalized_absorptions),
            "max_items": normalized_max_items,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"storage lifecycle arguments are not JSON serializable: {error}") from error
        if len(encoded) > STORAGE_LIFECYCLE_MAX_INPUT_BYTES:
            raise ArgumentError("storage lifecycle input exceeds the 20 MB safety bound")
        object.__setattr__(self, "now", normalized_now)
        object.__setattr__(self, "tiering_policy", normalized_policy)
        object.__setattr__(self, "records", normalized_records)
        object.__setattr__(self, "quota", normalized_quota)
        object.__setattr__(self, "apply_tiering", normalized_apply)
        object.__setattr__(self, "charges", normalized_charges)
        object.__setattr__(self, "releases", normalized_releases)
        object.__setattr__(self, "delegations", normalized_delegations)
        object.__setattr__(self, "absorb_delegated", normalized_absorptions)
        object.__setattr__(self, "max_items", normalized_max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageLifecycleSimulateArgs":
        raw = _route_mapping("storage lifecycle arguments", value)
        return cls(
            raw.get("now"),
            raw.get("tiering_policy"),
            raw.get("records"),
            raw.get("quota"),
            raw.get("apply_tiering", False),
            raw.get("charges", []),
            raw.get("releases", []),
            raw.get("delegations", []),
            raw.get("absorb_delegated", []),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "now": self.now,
            "tiering_policy": dict(self.tiering_policy),
            "records": [dict(item) for item in self.records],
            "apply_tiering": self.apply_tiering,
            "quota": dict(self.quota),
            "charges": [dict(item) for item in self.charges],
            "releases": [dict(item) for item in self.releases],
            "delegations": [dict(item) for item in self.delegations],
            "absorb_delegated": list(self.absorb_delegated),
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class StorageTieringPolicyReport:
    raw: dict[str, Any]
    demote_to_warm_after: int
    demote_to_cold_after: int
    promote_after_accesses: int
    promote_within: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageTieringPolicyReport":
        raw = _route_mapping("storage tiering policy", value)
        warm = _route_count("storage tiering warm threshold", raw.get("demote_to_warm_after"))
        cold = _route_count("storage tiering cold threshold", raw.get("demote_to_cold_after"))
        if cold <= warm:
            raise ArgumentError("storage tiering policy must place the cold threshold after the warm threshold")
        return cls(raw, warm, cold, _route_count("storage tiering promotion accesses", raw.get("promote_after_accesses")), _route_count("storage tiering promotion window", raw.get("promote_within")))


@dataclass(frozen=True)
class StorageAccessRecordReport:
    raw: dict[str, Any]
    object: str
    tier: str
    last_access: int
    recent_accesses: int
    bytes: int
    pinned: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageAccessRecordReport":
        raw = _route_mapping("storage access record", value)
        tier = _enum("storage tier", raw.get("tier"), STORAGE_TIERS)
        return cls(raw, _route_text("storage access record object", raw.get("object")), tier, _route_count("storage access record last_access", raw.get("last_access")), _route_count("storage access record recent_accesses", raw.get("recent_accesses")), _route_count("storage access record bytes", raw.get("bytes")), _bool("storage access record pinned", raw.get("pinned")))


@dataclass(frozen=True)
class StorageTierReasonReport:
    raw: dict[str, Any]
    kind: str
    details: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageTierReasonReport":
        raw = _route_mapping("storage tier transition reason", value)
        matches = [(key, candidate) for key, candidate in raw.items() if key in STORAGE_TIERING_REASONS]
        if len(matches) != 1:
            raise ArgumentError("storage tier transition reason must contain exactly one known tagged variant")
        kind, details = matches[0]
        return cls(raw, kind, _route_mapping("storage tier transition reason details", details))

    @property
    def held_by_pin(self) -> bool:
        return self.kind == "HeldByPin"


@dataclass(frozen=True)
class StorageTierTransitionReport:
    raw: dict[str, Any]
    object: str
    from_tier: str
    to_tier: str
    reason: StorageTierReasonReport
    skipped_a_tier: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageTierTransitionReport":
        raw = _route_mapping("storage tier transition", value)
        return cls(raw, _route_text("storage tier transition object", raw.get("object")), _enum("storage transition from", raw.get("from"), STORAGE_TIERS), _enum("storage transition to", raw.get("to"), STORAGE_TIERS), StorageTierReasonReport.from_wire(raw.get("reason")), _bool("storage transition skipped_a_tier", raw.get("skipped_a_tier")))


@dataclass(frozen=True)
class StorageRowReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageRowReport":
        raw = _route_mapping("storage lifecycle row", value)
        index = _route_count("storage lifecycle row index", raw.get("index"))
        ok = _bool("storage lifecycle row ok", raw.get("ok"))
        refusal = _optional_text("storage lifecycle row refusal", raw.get("refusal"))
        fail_closed = _bool("storage lifecycle row fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed storage lifecycle rows must retain a refusal and fail_closed=true")
        if ok and refusal is not None:
            raise ArgumentError("successful storage lifecycle rows cannot retain a refusal")
        return cls(raw, index, ok, refusal, fail_closed)


@dataclass(frozen=True)
class StorageTieringReport:
    raw: dict[str, Any]
    policy: StorageTieringPolicyReport
    plan_now: int
    transitions: tuple[StorageTierTransitionReport, ...]
    transition_count: int
    bytes_by_target: tuple[dict[str, Any], ...]
    apply_requested: bool
    applied: int | None
    absent: int | None
    records: tuple[StorageAccessRecordReport, ...]
    omitted_records: int
    input_rows: tuple[StorageRowReport, ...]
    omitted_input_rows: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageTieringReport":
        raw = _route_mapping("storage tiering report", value)
        policy = StorageTieringPolicyReport.from_wire(raw.get("policy"))
        plan = _route_mapping("storage tiering plan", raw.get("plan"))
        transitions = tuple(StorageTierTransitionReport.from_wire(item) for item in _sequence("storage tier transitions", plan.get("transitions")))
        transition_count = _route_count("storage tier transition_count", raw.get("transition_count"))
        if transition_count != len(transitions):
            raise ArgumentError("storage tier transition_count does not match the plan")
        bytes_by_target = tuple(_route_mapping("storage bytes_by_target row", item) for item in _sequence("storage bytes_by_target", raw.get("bytes_by_target", [])))
        apply_requested = _bool("storage tier apply_requested", raw.get("apply_requested"))
        apply_report = raw.get("apply_report")
        applied = absent = None
        if apply_requested:
            report = _route_mapping("storage tier apply_report", apply_report)
            applied = _route_count("storage tier applied", report.get("applied"))
            absent = _route_count("storage tier absent", report.get("absent"))
        elif apply_report is not None:
            raise ArgumentError("storage tier dry runs must not include an apply report")
        records = tuple(StorageAccessRecordReport.from_wire(item) for item in _sequence("storage tier records", raw.get("records", [])))
        input_rows = tuple(StorageRowReport.from_wire(item) for item in _sequence("storage tier input_rows", raw.get("input_rows", [])))
        return cls(raw, policy, _route_count("storage tier plan now", plan.get("now")), transitions, transition_count, bytes_by_target, apply_requested, applied, absent, records, _route_count("storage tier omitted_records", raw.get("omitted_records")), input_rows, _route_count("storage tier omitted_input_rows", raw.get("omitted_input_rows")))

    @property
    def skipped_transition_count(self) -> int:
        return sum(item.skipped_a_tier for item in self.transitions)

    @property
    def pin_held_transition_count(self) -> int:
        return sum(item.reason.held_by_pin for item in self.transitions)

    @property
    def dry_run(self) -> bool:
        return not self.apply_requested

    @property
    def apply_reconciles(self) -> bool:
        return self.applied is not None and self.absent is not None and self.applied + self.absent == self.transition_count


@dataclass(frozen=True)
class StorageClassReport:
    raw: dict[str, Any]
    class_name: str
    name: str
    reconstructible: bool
    charged: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageClassReport":
        raw = _route_mapping("storage class report", value)
        class_name = _enum("storage class", raw.get("class"), STORAGE_CLASSES)
        name = _enum("storage class name", raw.get("name"), STORAGE_CLASS_NAMES)
        return cls(raw, class_name, name, _bool("storage class reconstructible", raw.get("reconstructible")), _route_count("storage class charged", raw.get("charged")))


@dataclass(frozen=True)
class StorageQuotaReport:
    raw: dict[str, Any]
    limit: int
    reserve: int
    used: int
    remaining: int
    remaining_for_ingest: int
    remaining_for_evidence_finalization: int
    remaining_for_cleanup: int
    classes: tuple[StorageClassReport, ...]
    charges: tuple[StorageRowReport, ...]
    releases: tuple[StorageRowReport, ...]
    delegations: tuple[StorageRowReport, ...]
    absorptions: tuple[StorageRowReport, ...]
    remaining_children: tuple[dict[str, Any], ...]
    omitted_charges: int
    omitted_releases: int
    omitted_delegations: int
    omitted_absorptions: int
    omitted_children: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageQuotaReport":
        raw = _route_mapping("storage quota report", value)
        limit = _route_count("storage quota limit", raw.get("limit"))
        reserve = _route_count("storage quota reserve", raw.get("reserve"))
        if reserve >= limit:
            raise ArgumentError("storage quota reserve must be below limit")
        used = _route_count("storage quota used", raw.get("used"))
        remaining = _route_count("storage quota remaining", raw.get("remaining"))
        if used + remaining != limit:
            raise ArgumentError("storage quota used plus remaining does not reconcile to limit")
        classes = tuple(StorageClassReport.from_wire(item) for item in _sequence("storage quota classes", raw.get("classes", [])))
        return cls(raw, limit, reserve, used, remaining, _route_count("storage remaining for ingest", raw.get("remaining_for_ingest")), _route_count("storage remaining for evidence finalization", raw.get("remaining_for_evidence_finalization")), _route_count("storage remaining for cleanup", raw.get("remaining_for_cleanup")), classes, tuple(StorageRowReport.from_wire(item) for item in _sequence("storage quota charges", raw.get("charges", []))), tuple(StorageRowReport.from_wire(item) for item in _sequence("storage quota releases", raw.get("releases", []))), tuple(StorageRowReport.from_wire(item) for item in _sequence("storage quota delegations", raw.get("delegations", []))), tuple(StorageRowReport.from_wire(item) for item in _sequence("storage quota absorptions", raw.get("absorptions", []))), tuple(_route_mapping("storage remaining child", item) for item in _sequence("storage quota remaining_children", raw.get("remaining_children", []))), _route_count("storage omitted charges", raw.get("omitted_charges")), _route_count("storage omitted releases", raw.get("omitted_releases")), _route_count("storage omitted delegations", raw.get("omitted_delegations")), _route_count("storage omitted absorptions", raw.get("omitted_absorptions")), _route_count("storage omitted children", raw.get("omitted_children")))

    @property
    def charge_refusal_count(self) -> int:
        return sum(not item.ok for item in self.charges)

    @property
    def release_refusal_count(self) -> int:
        return sum(not item.ok for item in self.releases)

    @property
    def delegation_refusal_count(self) -> int:
        return sum(not item.ok for item in self.delegations)

    @property
    def absorption_refusal_count(self) -> int:
        return sum(not item.ok for item in self.absorptions)

    @property
    def reserve_is_explicit(self) -> bool:
        return self.remaining_for_ingest <= self.remaining and self.remaining_for_evidence_finalization == self.remaining and self.remaining_for_cleanup == self.remaining


@dataclass(frozen=True)
class StorageLifecycleReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    max_items: int
    now: int
    tiering: StorageTieringReport
    quota: StorageQuotaReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StorageLifecycleReport":
        raw = _payload(value)
        ok = _bool("storage lifecycle ok", raw.get("ok"))
        schema = _route_text("storage lifecycle schema", raw.get("schema"))
        if schema != "bioprism-mcp/storage-lifecycle/0.1":
            raise ArgumentError(f"unknown storage lifecycle schema {schema!r}")
        return cls(raw, ok, schema, _route_count("storage lifecycle max_items", raw.get("max_items")), _route_count("storage lifecycle now", raw.get("now")), StorageTieringReport.from_wire(raw.get("tiering")), StorageQuotaReport.from_wire(raw.get("quota")), _route_strings("storage lifecycle guarantees", raw.get("guarantees", [])), _route_strings("storage lifecycle limitations", raw.get("limitations", [])))

    @property
    def deterministic_plan(self) -> bool:
        return any("caller-supplied logical epoch" in item and "same records and policy replay" in item for item in self.guarantees)

    @property
    def side_effect_free(self) -> bool:
        return any("does not move bytes" in item and "scheduler" in item for item in self.limitations)

    @property
    def fail_closed_row_count(self) -> int:
        rows = self.tiering.input_rows + self.quota.charges + self.quota.releases + self.quota.delegations + self.quota.absorptions
        return sum(not item.ok and item.fail_closed for item in rows)

    @property
    def reserve_protected_refusal_count(self) -> int:
        rows = self.quota.charges
        return sum(not item.ok and item.refusal is not None and "reserve" in item.refusal.lower() for item in rows)

    @property
    def allowance_is_non_copyable(self) -> bool:
        return any("allowance is not copied" in item for item in self.guarantees)

    @property
    def raw_class_attribution_is_preserved(self) -> bool:
        return any("raw attribution" in item and "reconstructible" in item for item in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def storage_lifecycle_report(value: Mapping[str, Any]) -> StorageLifecycleReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return StorageLifecycleReport.from_wire(value)


__all__ = [
    "STORAGE_LIFECYCLE_MAX_INPUT_BYTES",
    "STORAGE_LIFECYCLE_MAX_ITEMS",
    "STORAGE_LIFECYCLE_MAX_DELEGATIONS",
    "STORAGE_TIERS",
    "STORAGE_CLASSES",
    "STORAGE_CLASS_NAMES",
    "STORAGE_PURPOSES",
    "STORAGE_PURPOSE_NAMES",
    "STORAGE_TIERING_REASONS",
    "StorageLifecycleSimulateArgs",
    "StorageTieringPolicyReport",
    "StorageAccessRecordReport",
    "StorageTierReasonReport",
    "StorageTierTransitionReport",
    "StorageRowReport",
    "StorageTieringReport",
    "StorageClassReport",
    "StorageQuotaReport",
    "StorageLifecycleReport",
    "storage_lifecycle_report",
]
