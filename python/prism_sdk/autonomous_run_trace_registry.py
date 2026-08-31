"""Bounded operator query and retention projection for autonomous run traces.

The append-only :mod:`autonomous_run_trace` journal is still the source of truth for event
ordering.  This module imports a validated snapshot, groups its metadata by run, and exposes a
small deterministic index suitable for an operator console or queue monitor.  It deliberately
does not retain prompts, provider responses, credentials, evidence bodies, tool arguments, or
effect values, and nothing in the registry authorizes replay or dispatch.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
import threading
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_run_trace import (
    AUTONOMOUS_RUN_TRACE_PHASES,
    AUTONOMOUS_RUN_TRACE_SCHEMA,
    AUTONOMOUS_RUN_TRACE_STATUSES,
    MAX_AUTONOMOUS_RUN_TRACE_EVENTS,
    MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES,
    AutonomousRunTraceEvent,
    AutonomousRunTraceSnapshot,
    AutonomousRunTraceSummary,
    AutonomousRunTraceTextStore,
    AutonomousRunTraceTransactionalTextStore,
    validate_autonomous_run_trace_snapshot,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA = "bioprism-python-autonomous-run-trace-registry/0.1"
AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-run-trace-registry-snapshot/0.1"
AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION = "metadata_only_no_prompts_responses_tool_payloads_credentials_evidence_or_effect_values"
AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY = "operator_query_and_retention_projection_only;does_not_authorize_execution"
AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL = "never_returned"
AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA = "bioprism-python-autonomous-run-trace-registry-publication/0.1"
MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS = 10_000
MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS = MAX_AUTONOMOUS_RUN_TRACE_EVENTS
MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES
_MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_COUNTER = 9_007_199_254_740_991


def _bounded_text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value or len(value) > maximum or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    text = _bounded_text(name, value)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in text):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return text


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _count(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise ArgumentError(f"{name} is outside its bounds")
    return value


def _limit(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} is outside its bounds")
    return value


def _sorted_strings(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) > 512:
        raise ArgumentError(f"{name} must be a bounded sequence")
    result = tuple(_bounded_text(name, item) for item in value)
    if result != tuple(sorted(set(result))) or len(set(result)) != len(result):
        raise ArgumentError(f"{name} must be sorted and unique")
    return result


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRetentionPolicy:
    max_runs: int
    max_events: int
    max_bytes: int
    retain_events: bool = True
    keep_incomplete: bool = True
    eviction: str = "oldest_eligible_terminal_run"

    def to_dict(self) -> dict[str, Any]:
        return {
            "max_runs": self.max_runs,
            "max_events": self.max_events,
            "max_bytes": self.max_bytes,
            "retain_events": self.retain_events,
            "keep_incomplete": self.keep_incomplete,
            "eviction": self.eviction,
        }


def _policy(value: Mapping[str, Any] | AutonomousRunTraceRetentionPolicy | None = None) -> AutonomousRunTraceRetentionPolicy:
    raw = {} if value is None else value.to_dict() if isinstance(value, AutonomousRunTraceRetentionPolicy) else dict(value)
    if not isinstance(raw, Mapping):
        raise ArgumentError("autonomous run trace registry policy must be a mapping")
    max_runs = _limit("autonomous run trace registry max_runs", raw.get("max_runs", MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS), MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS)
    max_events = _limit("autonomous run trace registry max_events", raw.get("max_events", MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS), MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS)
    max_bytes = _limit("autonomous run trace registry max_bytes", raw.get("max_bytes", MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES), MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES)
    if max_bytes < 16_000:
        raise ArgumentError("autonomous run trace registry max_bytes is too small for a registry snapshot")
    if "retain_events" in raw and not isinstance(raw["retain_events"], bool):
        raise ArgumentError("autonomous run trace registry retain_events must be boolean")
    if "keep_incomplete" in raw and not isinstance(raw["keep_incomplete"], bool):
        raise ArgumentError("autonomous run trace registry keep_incomplete must be boolean")
    if raw.get("eviction", "oldest_eligible_terminal_run") != "oldest_eligible_terminal_run":
        raise ArgumentError("autonomous run trace registry eviction policy is unsupported")
    return AutonomousRunTraceRetentionPolicy(
        max_runs=max_runs,
        max_events=max_events,
        max_bytes=max_bytes,
        retain_events=raw.get("retain_events", True),
        keep_incomplete=raw.get("keep_incomplete", True),
    )


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistryRecord:
    run_id: str
    summary: AutonomousRunTraceSummary
    providers: tuple[str, ...]
    models: tuple[str, ...]
    source_snapshot_digest: str
    source_sequence: int
    source_head_digest: str
    events: tuple[AutonomousRunTraceEvent, ...]
    retained_event_count: int
    record_digest: str
    schema: str = AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA
    retention: str = AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION
    authority: str = AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY
    secret_material: str = AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "run_id": self.run_id,
            "summary": self.summary.to_dict(),
            "providers": list(self.providers),
            "models": list(self.models),
            "source_snapshot_digest": self.source_snapshot_digest,
            "source_sequence": self.source_sequence,
            "source_head_digest": self.source_head_digest,
            "events": [event.to_dict() for event in self.events],
            "retained_event_count": self.retained_event_count,
            "record_digest": self.record_digest,
            "retention": self.retention,
            "authority": self.authority,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistrySnapshot:
    policy: AutonomousRunTraceRetentionPolicy
    records: tuple[AutonomousRunTraceRegistryRecord, ...]
    snapshot_generation: int
    previous_snapshot_digest: str | None
    snapshot_digest: str
    schema: str = AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA
    retention: str = AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION
    authority: str = AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY
    secret_material: str = AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL

    @property
    def record_count(self) -> int:
        return len(self.records)

    @property
    def event_count(self) -> int:
        return sum(record.summary.event_count for record in self.records)

    @property
    def retained_event_count(self) -> int:
        return sum(record.retained_event_count for record in self.records)

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": self.schema,
            "snapshot_generation": self.snapshot_generation,
            "previous_snapshot_digest": self.previous_snapshot_digest,
            "policy": self.policy.to_dict(),
            "record_count": self.record_count,
            "event_count": self.event_count,
            "retained_event_count": self.retained_event_count,
            "records": [record.to_dict() for record in self.records],
            "retention": self.retention,
            "authority": self.authority,
            "secret_material": self.secret_material,
        }
        body["snapshot_digest"] = self.snapshot_digest
        return body


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistryPage:
    records: tuple[AutonomousRunTraceRegistryRecord, ...]
    next_after_run_id: str | None
    total_matches: int
    retained_event_count: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "records": [record.to_dict() for record in self.records],
            "next_after_run_id": self.next_after_run_id,
            "total_matches": self.total_matches,
            "retained_event_count": self.retained_event_count,
            "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistryImportReport:
    imported_run_ids: tuple[str, ...]
    replaced_run_ids: tuple[str, ...]
    unchanged_run_ids: tuple[str, ...]
    evicted_run_ids: tuple[str, ...]
    snapshot: AutonomousRunTraceRegistrySnapshot

    def to_dict(self) -> dict[str, Any]:
        return {
            "imported_run_ids": list(self.imported_run_ids),
            "replaced_run_ids": list(self.replaced_run_ids),
            "unchanged_run_ids": list(self.unchanged_run_ids),
            "evicted_run_ids": list(self.evicted_run_ids),
            "snapshot": self.snapshot.to_dict(),
            "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistryPublication:
    """Bounded result of projecting a trace store into the operator registry."""

    status: str
    run_id: str
    run_import_state: str
    source_snapshot_digest: str | None
    registry_snapshot_digest: str | None
    evicted_run_count: int
    error_class: str | None = None
    failure_code: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA,
            "status": self.status,
            "run_id": self.run_id,
            "run_import_state": self.run_import_state,
            "source_snapshot_digest": self.source_snapshot_digest,
            "registry_snapshot_digest": self.registry_snapshot_digest,
            "evicted_run_count": self.evicted_run_count,
            "error_class": self.error_class,
            "failure_code": self.failure_code,
            "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceRegistryIntegrity:
    runs: int
    events: int
    retained_event_count: int
    snapshot_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "verified": True,
            "runs": self.runs,
            "events": self.events,
            "retained_event_count": self.retained_event_count,
            "snapshot_digest": self.snapshot_digest,
            "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        }


def _summary(run_id: str, events: Sequence[AutonomousRunTraceEvent]) -> AutonomousRunTraceSummary:
    if not events:
        raise ArgumentError("autonomous run trace registry cannot index an empty run")
    task_digest = events[0].task_digest
    if any(event.run_id != run_id or event.task_digest != task_digest for event in events):
        raise ArgumentError("autonomous run trace registry run events have inconsistent identity")
    domains = tuple(sorted({domain for event in events for domain in event.domains}))
    selections = tuple(sorted({event.selection_digest for event in events if event.selection_digest is not None}))
    failures = tuple(sorted({event.failure_code for event in events if event.failure_code is not None}))
    completed = tuple(event for event in events if event.phase == "provider_invocation_finished")
    body = {
        "schema": AUTONOMOUS_RUN_TRACE_SCHEMA,
        "run_id": run_id,
        "task_digest": task_digest,
        "domains": list(domains),
        "status": events[-1].status,
        "first_sequence": events[0].sequence,
        "last_sequence": events[-1].sequence,
        "event_count": len(events),
        "provider_invocations": len(completed),
        "provider_failures": sum(event.failure_code is not None for event in completed),
        "input_tokens": sum(event.input_tokens or 0 for event in completed),
        "output_tokens": sum(event.output_tokens or 0 for event in completed),
        "tool_calls": sum(event.tool_count or 0 for event in completed),
        "route_digest": next((event.route_digest for event in reversed(events) if event.route_digest is not None), None),
        "plan_digest": next((event.plan_digest for event in reversed(events) if event.plan_digest is not None), None),
        "selection_digests": list(selections),
        "failure_codes": list(failures),
        "retention": "metadata_only_no_prompts_responses_or_tool_payloads",
        "secret_material": "never_returned",
    }
    return AutonomousRunTraceSummary(
        schema=AUTONOMOUS_RUN_TRACE_SCHEMA,
        run_id=run_id,
        task_digest=task_digest,
        domains=domains,
        status=events[-1].status,
        first_sequence=events[0].sequence,
        last_sequence=events[-1].sequence,
        event_count=len(events),
        provider_invocations=len(completed),
        provider_failures=sum(event.failure_code is not None for event in completed),
        input_tokens=sum(event.input_tokens or 0 for event in completed),
        output_tokens=sum(event.output_tokens or 0 for event in completed),
        tool_calls=sum(event.tool_count or 0 for event in completed),
        route_digest=body["route_digest"],
        plan_digest=body["plan_digest"],
        selection_digests=selections,
        failure_codes=failures,
        trace_digest=content_digest(body),
    )


def _record_body(record: AutonomousRunTraceRegistryRecord, *, include_digest: bool = False) -> dict[str, Any]:
    body = record.to_dict()
    if not include_digest:
        body.pop("record_digest", None)
    return body


def _record(source: AutonomousRunTraceSnapshot, run_id: str, events: Sequence[AutonomousRunTraceEvent], policy: AutonomousRunTraceRetentionPolicy) -> AutonomousRunTraceRegistryRecord:
    summary = _summary(run_id, events)
    providers = tuple(sorted({event.provider for event in events if event.provider is not None}))
    models = tuple(sorted({event.model for event in events if event.model is not None}))
    retained = tuple(events) if policy.retain_events else ()
    if len(retained) > policy.max_events:
        raise ArgumentError(f"autonomous run trace registry run {run_id} exceeds max_events")
    provisional = AutonomousRunTraceRegistryRecord(
        run_id=run_id,
        summary=summary,
        providers=providers,
        models=models,
        source_snapshot_digest=source.snapshot_digest,
        source_sequence=source.sequence,
        source_head_digest=source.head_digest or content_digest({"snapshot": source.snapshot_digest}),
        events=retained,
        retained_event_count=len(retained),
        record_digest="0" * 64,
    )
    return replace(provisional, record_digest=content_digest(_record_body(provisional)))


def _record_key(record: AutonomousRunTraceRegistryRecord) -> tuple[int, str]:
    return (record.summary.last_sequence or 0, record.run_id)


def _incomplete(status: str) -> bool:
    return status in {"running", "partial", "paused", "unknown"}


def _snapshot_body(records: Sequence[AutonomousRunTraceRegistryRecord], policy: AutonomousRunTraceRetentionPolicy, generation: int, previous: str | None) -> dict[str, Any]:
    ordered = sorted(records, key=_record_key)
    return {
        "schema": AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA,
        "snapshot_generation": generation,
        "previous_snapshot_digest": previous,
        "policy": policy.to_dict(),
        "record_count": len(ordered),
        "event_count": sum(record.summary.event_count for record in ordered),
        "retained_event_count": sum(record.retained_event_count for record in ordered),
        "records": [record.to_dict() for record in ordered],
        "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
        "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
        "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
    }


def _validate_summary(raw: Mapping[str, Any]) -> AutonomousRunTraceSummary:
    required = {"schema", "run_id", "task_digest", "domains", "status", "first_sequence", "last_sequence", "event_count", "provider_invocations", "provider_failures", "input_tokens", "output_tokens", "tool_calls", "route_digest", "plan_digest", "selection_digests", "failure_codes", "trace_digest", "retention", "secret_material"}
    if set(raw) != required:
        raise ArgumentError("autonomous run trace registry summary fields are incomplete")
    if raw["schema"] != AUTONOMOUS_RUN_TRACE_SCHEMA or raw["retention"] != "metadata_only_no_prompts_responses_or_tool_payloads" or raw["secret_material"] != "never_returned":
        raise ArgumentError("autonomous run trace registry summary retention is invalid")
    run_id = _identifier("autonomous run trace registry summary run_id", raw["run_id"])
    task_digest = _digest("autonomous run trace registry summary task_digest", raw["task_digest"])
    domains = tuple(raw["domains"])
    if not domains or any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains) or len(set(domains)) != len(domains) or domains != tuple(sorted(domains)):
        raise ArgumentError("autonomous run trace registry summary domains are invalid")
    status = raw["status"]
    if status not in AUTONOMOUS_RUN_TRACE_STATUSES:
        raise ArgumentError("autonomous run trace registry summary status is invalid")
    first = _count("autonomous run trace registry summary first_sequence", raw["first_sequence"], MAX_AUTONOMOUS_RUN_TRACE_EVENTS)
    last = _count("autonomous run trace registry summary last_sequence", raw["last_sequence"], MAX_AUTONOMOUS_RUN_TRACE_EVENTS)
    if first < 1 or first > last:
        raise ArgumentError("autonomous run trace registry summary sequence range is invalid")
    counts = {name: _count(f"autonomous run trace registry summary {name}", raw[name], MAX_AUTONOMOUS_RUN_TRACE_EVENTS) for name in ("event_count", "provider_invocations", "provider_failures")}
    counts.update({name: _count(f"autonomous run trace registry summary {name}", raw[name], _MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_COUNTER) for name in ("input_tokens", "output_tokens", "tool_calls")})
    if counts["provider_failures"] > counts["provider_invocations"]:
        raise ArgumentError("autonomous run trace registry summary provider failure count is invalid")
    route = None if raw["route_digest"] is None else _digest("autonomous run trace registry summary route_digest", raw["route_digest"])
    plan = None if raw["plan_digest"] is None else _digest("autonomous run trace registry summary plan_digest", raw["plan_digest"])
    selections = _sorted_strings("autonomous run trace registry summary selection_digests", raw["selection_digests"])
    for selection in selections:
        _digest("autonomous run trace registry summary selection_digest", selection)
    failures = _sorted_strings("autonomous run trace registry summary failure_codes", raw["failure_codes"])
    supplied = _digest("autonomous run trace registry summary trace_digest", raw["trace_digest"])
    normalized = {
        "schema": AUTONOMOUS_RUN_TRACE_SCHEMA,
        "run_id": run_id,
        "task_digest": task_digest,
        "domains": list(domains),
        "status": status,
        "first_sequence": first,
        "last_sequence": last,
        **counts,
        "route_digest": route,
        "plan_digest": plan,
        "selection_digests": list(selections),
        "failure_codes": list(failures),
        "retention": "metadata_only_no_prompts_responses_or_tool_payloads",
        "secret_material": "never_returned",
    }
    if content_digest(normalized) != supplied:
        raise ArgumentError("autonomous run trace registry summary digest is invalid")
    return AutonomousRunTraceSummary(
        schema=AUTONOMOUS_RUN_TRACE_SCHEMA,
        run_id=run_id,
        task_digest=task_digest,
        domains=domains,
        status=status,
        first_sequence=first,
        last_sequence=last,
        event_count=counts["event_count"],
        provider_invocations=counts["provider_invocations"],
        provider_failures=counts["provider_failures"],
        input_tokens=counts["input_tokens"],
        output_tokens=counts["output_tokens"],
        tool_calls=counts["tool_calls"],
        route_digest=route,
        plan_digest=plan,
        selection_digests=selections,
        failure_codes=failures,
        trace_digest=supplied,
    )


def _validate_record(raw: Mapping[str, Any], policy: AutonomousRunTraceRetentionPolicy) -> AutonomousRunTraceRegistryRecord:
    allowed = {"schema", "run_id", "summary", "providers", "models", "source_snapshot_digest", "source_sequence", "source_head_digest", "events", "retained_event_count", "record_digest", "retention", "authority", "secret_material"}
    if set(raw) != allowed:
        raise ArgumentError("autonomous run trace registry record fields are incomplete")
    if raw["schema"] != AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA or raw["retention"] != AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION or raw["authority"] != AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY or raw["secret_material"] != AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL:
        raise ArgumentError("autonomous run trace registry record retention is invalid")
    run_id = _identifier("autonomous run trace registry run_id", raw["run_id"])
    summary = _validate_summary(raw["summary"])
    if summary.run_id != run_id:
        raise ArgumentError("autonomous run trace registry record summary identity does not match")
    providers = _sorted_strings("autonomous run trace registry providers", raw["providers"])
    models = _sorted_strings("autonomous run trace registry models", raw["models"])
    source_snapshot_digest = _digest("autonomous run trace registry source_snapshot_digest", raw["source_snapshot_digest"])
    source_sequence = _count("autonomous run trace registry source_sequence", raw["source_sequence"], MAX_AUTONOMOUS_RUN_TRACE_EVENTS)
    source_head_digest = _digest("autonomous run trace registry source_head_digest", raw["source_head_digest"])
    if source_sequence < (summary.last_sequence or 0):
        raise ArgumentError("autonomous run trace registry source sequence predates the run")
    if not isinstance(raw["events"], Sequence) or isinstance(raw["events"], (str, bytes)) or len(raw["events"]) > policy.max_events:
        raise ArgumentError("autonomous run trace registry retained events exceed policy")
    events = tuple(AutonomousRunTraceEvent.from_dict(event) for event in raw["events"])
    if policy.retain_events and len(events) != summary.event_count:
        raise ArgumentError("autonomous run trace registry retained event count does not match the summary")
    if not policy.retain_events and events:
        raise ArgumentError("autonomous run trace registry policy forbids retained events")
    if any(event.run_id != run_id or event.task_digest != summary.task_digest for event in events):
        raise ArgumentError("autonomous run trace registry event identity does not match")
    if policy.retain_events and _summary(run_id, events).to_dict() != summary.to_dict():
        raise ArgumentError("autonomous run trace registry summary does not match retained events")
    retained_count = _count("autonomous run trace registry retained_event_count", raw["retained_event_count"], policy.max_events)
    if retained_count != len(events):
        raise ArgumentError("autonomous run trace registry retained event count is inconsistent")
    supplied = _digest("autonomous run trace registry record_digest", raw["record_digest"])
    provisional = AutonomousRunTraceRegistryRecord(
        run_id=run_id,
        summary=summary,
        providers=providers,
        models=models,
        source_snapshot_digest=source_snapshot_digest,
        source_sequence=source_sequence,
        source_head_digest=source_head_digest,
        events=events,
        retained_event_count=retained_count,
        record_digest="0" * 64,
    )
    if content_digest(_record_body(provisional)) != supplied:
        raise ArgumentError("autonomous run trace registry record digest is invalid")
    return replace(provisional, record_digest=supplied)


def _validate_snapshot(raw: Mapping[str, Any], maximum_bytes: int) -> AutonomousRunTraceRegistrySnapshot:
    if isinstance(raw, AutonomousRunTraceRegistrySnapshot):
        raw = raw.to_dict()
    allowed = {"schema", "snapshot_generation", "previous_snapshot_digest", "policy", "record_count", "event_count", "retained_event_count", "records", "snapshot_digest", "retention", "authority", "secret_material"}
    if not isinstance(raw, Mapping) or set(raw) != allowed or not isinstance(raw["records"], Sequence) or isinstance(raw["records"], (str, bytes)):
        raise ArgumentError("autonomous run trace registry snapshot fields are incomplete")
    if raw["schema"] != AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA or raw["retention"] != AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION or raw["authority"] != AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY or raw["secret_material"] != AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL:
        raise ArgumentError("autonomous run trace registry snapshot retention is invalid")
    generation = raw["snapshot_generation"]
    if isinstance(generation, bool) or not isinstance(generation, int) or generation < 1:
        raise ArgumentError("autonomous run trace registry snapshot generation is invalid")
    previous = raw["previous_snapshot_digest"]
    if previous is not None:
        _digest("autonomous run trace registry previous_snapshot_digest", previous)
    if (generation == 1) != (previous is None):
        raise ArgumentError("autonomous run trace registry snapshot lineage is inconsistent")
    policy = _policy(raw["policy"])
    if len(raw["records"]) > policy.max_runs:
        raise ArgumentError("autonomous run trace registry records exceed policy")
    records = tuple(_validate_record(record, policy) for record in raw["records"])
    if len({record.run_id for record in records}) != len(records):
        raise ArgumentError("autonomous run trace registry contains duplicate run ids")
    if tuple(sorted(records, key=_record_key)) != records:
        raise ArgumentError("autonomous run trace registry records are not deterministically ordered")
    if raw["record_count"] != len(records) or raw["event_count"] != sum(record.summary.event_count for record in records) or raw["retained_event_count"] != sum(record.retained_event_count for record in records):
        raise ArgumentError("autonomous run trace registry snapshot counts are inconsistent")
    body = _snapshot_body(records, policy, generation, previous)
    supplied = _digest("autonomous run trace registry snapshot_digest", raw["snapshot_digest"])
    if content_digest(body) != supplied:
        raise ArgumentError("autonomous run trace registry snapshot digest is invalid")
    snapshot = AutonomousRunTraceRegistrySnapshot(policy=policy, records=records, snapshot_generation=generation, previous_snapshot_digest=previous, snapshot_digest=supplied)
    if len(canonical_json(snapshot.to_dict()).encode("utf-8")) > maximum_bytes:
        raise ArgumentError("autonomous run trace registry snapshot exceeds its byte capacity")
    return snapshot


class AutonomousRunTraceRegistry:
    """Thread-safe metadata index with deterministic pagination and fail-closed retention."""

    def __init__(self, policy: Mapping[str, Any] | AutonomousRunTraceRetentionPolicy | None = None) -> None:
        self.policy = _policy(policy)
        self._records: dict[str, AutonomousRunTraceRegistryRecord] = {}
        self._lock = threading.RLock()
        self._snapshot_generation = 0
        self._previous_snapshot_digest: str | None = None
        self._cached_snapshot: AutonomousRunTraceRegistrySnapshot | None = None
        self._cached_signature: tuple[str, ...] | None = None

    @property
    def size(self) -> int:
        with self._lock:
            return len(self._records)

    def get(self, run_id: str) -> AutonomousRunTraceRegistryRecord | None:
        run_id = _identifier("autonomous run trace registry query run_id", run_id)
        with self._lock:
            record = self._records.get(run_id)
            return None if record is None else _validate_record(record.to_dict(), self.policy)

    def query(self, query: Mapping[str, Any] | None = None) -> AutonomousRunTraceRegistryPage:
        if query is not None and not isinstance(query, Mapping):
            raise ArgumentError("autonomous run trace registry query must be a mapping")
        raw = {} if query is None else dict(query)
        limit = _limit("autonomous run trace registry query limit", raw.get("limit", 256), 10_000)
        for key in ("run_id", "after_run_id"):
            if raw.get(key) is not None:
                _identifier(f"autonomous run trace registry query {key}", raw[key])
        if raw.get("domain") is not None and raw["domain"] not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous run trace registry query domain is unsupported")
        if raw.get("status") is not None and raw["status"] not in AUTONOMOUS_RUN_TRACE_STATUSES:
            raise ArgumentError("autonomous run trace registry query status is unsupported")
        for key in ("provider", "model"):
            if raw.get(key) is not None:
                _bounded_text(f"autonomous run trace registry query {key}", raw[key])
        with self._lock:
            ordered = sorted(self._records.values(), key=_record_key)
            after_index = -1
            if raw.get("after_run_id") is not None:
                after_index = next((index for index, record in enumerate(ordered) if record.run_id == raw["after_run_id"]), -1)
                if after_index < 0:
                    raise ArgumentError("autonomous run trace registry query cursor is stale or unknown")
            matches = [record for record in ordered[after_index + 1:] if (
                raw.get("run_id") is None or record.run_id == raw["run_id"]
            ) and (
                raw.get("domain") is None or raw["domain"] in record.summary.domains
            ) and (
                raw.get("status") is None or record.summary.status == raw["status"]
            ) and (
                raw.get("provider") is None or raw["provider"] in record.providers
            ) and (
                raw.get("model") is None or raw["model"] in record.models
            )]
            selected = tuple(_validate_record(record.to_dict(), self.policy) for record in matches[:limit])
            return AutonomousRunTraceRegistryPage(
                records=selected,
                next_after_run_id=selected[-1].run_id if len(matches) > len(selected) else None,
                total_matches=len(matches),
                retained_event_count=sum(record.retained_event_count for record in selected),
            )

    def events(self, query: Mapping[str, Any] | None = None) -> tuple[AutonomousRunTraceEvent, ...]:
        if query is not None and not isinstance(query, Mapping):
            raise ArgumentError("autonomous run trace registry event query must be a mapping")
        raw = {} if query is None else dict(query)
        after = _count("autonomous run trace registry event query after_sequence", raw.get("after_sequence", 0), MAX_AUTONOMOUS_RUN_TRACE_EVENTS)
        limit = _limit("autonomous run trace registry event query limit", raw.get("limit", 10_000), 10_000)
        for key in ("run_id", "provider", "model"):
            if raw.get(key) is not None:
                (_identifier if key == "run_id" else _bounded_text)(f"autonomous run trace registry event query {key}", raw[key])
        if raw.get("domain") is not None and raw["domain"] not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous run trace registry event query domain is unsupported")
        if raw.get("phase") is not None and raw["phase"] not in AUTONOMOUS_RUN_TRACE_PHASES:
            raise ArgumentError("autonomous run trace registry event query phase is unsupported")
        if raw.get("status") is not None and raw["status"] not in AUTONOMOUS_RUN_TRACE_STATUSES:
            raise ArgumentError("autonomous run trace registry event query status is unsupported")
        with self._lock:
            values = [event for record in self._records.values() for event in record.events]
            values = [event for event in values if event.sequence > after and (raw.get("run_id") is None or event.run_id == raw["run_id"]) and (raw.get("domain") is None or raw["domain"] in event.domains) and (raw.get("phase") is None or event.phase == raw["phase"]) and (raw.get("status") is None or event.status == raw["status"]) and (raw.get("provider") is None or event.provider == raw["provider"]) and (raw.get("model") is None or event.model == raw["model"])]
            return tuple(sorted(values, key=lambda event: (event.sequence, event.run_id))[:limit])

    def import_snapshot(self, raw: Mapping[str, Any] | AutonomousRunTraceSnapshot) -> AutonomousRunTraceRegistryImportReport:
        source = validate_autonomous_run_trace_snapshot(raw)
        grouped: dict[str, list[AutonomousRunTraceEvent]] = {}
        for event in source.events:
            grouped.setdefault(event.run_id, []).append(event)
        with self._lock:
            next_records = dict(self._records)
            imported: list[str] = []
            replaced: list[str] = []
            unchanged: list[str] = []
            for run_id, events in grouped.items():
                record = _record(source, run_id, events, self.policy)
                current = next_records.get(run_id)
                if current is None:
                    next_records[run_id] = record
                    imported.append(run_id)
                elif current.record_digest == record.record_digest:
                    unchanged.append(run_id)
                elif record.source_sequence < current.source_sequence:
                    raise ArgumentError(f"autonomous run trace registry rejected stale run {run_id}")
                elif record.source_sequence == current.source_sequence:
                    raise ArgumentError(f"autonomous run trace registry rejected conflicting run {run_id}")
                elif current.summary.task_digest != record.summary.task_digest:
                    raise ArgumentError(f"autonomous run trace registry rejected run identity drift for {run_id}")
                else:
                    next_records[run_id] = record
                    replaced.append(run_id)
            evicted = self._fit(next_records)
            self._records = next_records
            self._invalidate()
            return AutonomousRunTraceRegistryImportReport(tuple(sorted(imported)), tuple(sorted(replaced)), tuple(sorted(unchanged)), tuple(sorted(evicted)), self.snapshot())

    def compact(self) -> tuple[tuple[str, ...], AutonomousRunTraceRegistrySnapshot]:
        with self._lock:
            next_records = dict(self._records)
            evicted = self._fit(next_records)
            self._records = next_records
            if evicted:
                self._invalidate()
            return tuple(sorted(evicted)), self.snapshot()

    def snapshot(self) -> AutonomousRunTraceRegistrySnapshot:
        with self._lock:
            ordered = sorted(self._records.values(), key=_record_key)
            signature = tuple(record.record_digest for record in ordered)
            if self._cached_snapshot is not None and self._cached_signature == signature:
                return self._cached_snapshot
            body = _snapshot_body(ordered, self.policy, self._snapshot_generation + 1, None if self._snapshot_generation == 0 else self._previous_snapshot_digest)
            snapshot = AutonomousRunTraceRegistrySnapshot(policy=self.policy, records=tuple(ordered), snapshot_generation=body["snapshot_generation"], previous_snapshot_digest=body["previous_snapshot_digest"], snapshot_digest=content_digest(body))
            if len(canonical_json(snapshot.to_dict()).encode("utf-8")) > self.policy.max_bytes:
                raise ArgumentError("autonomous run trace registry snapshot exceeds its byte capacity")
            self._snapshot_generation = snapshot.snapshot_generation
            self._previous_snapshot_digest = snapshot.snapshot_digest
            self._cached_snapshot = snapshot
            self._cached_signature = signature
            return snapshot

    def restore(self, raw: Mapping[str, Any] | AutonomousRunTraceRegistrySnapshot) -> None:
        snapshot = _validate_snapshot(raw, self.policy.max_bytes)
        if snapshot.policy != self.policy:
            raise ArgumentError("autonomous run trace registry restore policy does not match the configured policy")
        with self._lock:
            self._records = {record.run_id: record for record in snapshot.records}
            self._snapshot_generation = snapshot.snapshot_generation
            self._previous_snapshot_digest = snapshot.snapshot_digest
            self._cached_snapshot = snapshot
            self._cached_signature = tuple(record.record_digest for record in snapshot.records)

    def verify_integrity(self) -> dict[str, Any]:
        snapshot = self.snapshot()
        _validate_snapshot(snapshot.to_dict(), self.policy.max_bytes)
        return AutonomousRunTraceRegistryIntegrity(snapshot.record_count, snapshot.event_count, snapshot.retained_event_count, snapshot.snapshot_digest).to_dict()

    def _fit(self, records: dict[str, AutonomousRunTraceRegistryRecord]) -> list[str]:
        evicted: list[str] = []

        def violates() -> bool:
            if len(records) > self.policy.max_runs:
                return True
            if sum(record.retained_event_count for record in records.values()) > self.policy.max_events:
                return True
            body = _snapshot_body(list(records.values()), self.policy, self._snapshot_generation + 1, None if self._snapshot_generation == 0 else self._previous_snapshot_digest)
            probe = dict(body)
            probe["snapshot_digest"] = content_digest(body)
            return len(canonical_json(probe).encode("utf-8")) > self.policy.max_bytes

        while violates():
            candidate = next((record for record in sorted(records.values(), key=_record_key) if not self.policy.keep_incomplete or not _incomplete(record.summary.status)), None)
            if candidate is None:
                raise ArgumentError("autonomous run trace registry retention cannot evict an eligible terminal run")
            del records[candidate.run_id]
            evicted.append(candidate.run_id)
        return evicted

    def _invalidate(self) -> None:
        self._cached_snapshot = None
        self._cached_signature = None


def publish_autonomous_run_trace_registry_snapshot(
    registry: AutonomousRunTraceRegistry,
    trace_store: Any,
    run_id: str,
) -> AutonomousRunTraceRegistryPublication:
    """Best-effort metadata publication that never turns a completed run into a retry.

    Source-journal validation and registry retention failures are returned as a bounded report.
    The caller can alert or persist that report while preserving the original provider outcome
    and its external-effect reconciliation boundary.
    """

    normalized_run_id = _identifier("autonomous run trace registry publication run_id", run_id)
    base = {
        "run_id": normalized_run_id,
        "run_import_state": "unknown",
        "source_snapshot_digest": None,
        "registry_snapshot_digest": None,
        "evicted_run_count": 0,
    }
    source_snapshot_digest: str | None = None
    try:
        if not isinstance(registry, AutonomousRunTraceRegistry):
            raise ArgumentError("autonomous run trace registry publication requires a registry")
        if not callable(getattr(trace_store, "snapshot", None)):
            raise ArgumentError("autonomous run trace registry publication requires a trace store")
        source = trace_store.snapshot()
        candidate_digest = getattr(source, "snapshot_digest", None)
        if isinstance(candidate_digest, str) and len(candidate_digest) == 64 and all(character in "0123456789abcdef" for character in candidate_digest):
            source_snapshot_digest = candidate_digest
        report = registry.import_snapshot(source)
        if normalized_run_id in report.imported_run_ids:
            run_import_state = "imported"
        elif normalized_run_id in report.replaced_run_ids:
            run_import_state = "replaced"
        elif normalized_run_id in report.unchanged_run_ids:
            run_import_state = "unchanged"
        else:
            run_import_state = "not_present"
        return AutonomousRunTraceRegistryPublication(
            status="published",
            run_id=normalized_run_id,
            run_import_state=run_import_state,
            source_snapshot_digest=source.snapshot_digest,
            registry_snapshot_digest=report.snapshot.snapshot_digest,
            evicted_run_count=len(report.evicted_run_ids),
        )
    except Exception as error:
        if isinstance(error, ArgumentError) and "trace snapshot" in str(error):
            failure_code = "trace_snapshot_invalid"
        elif isinstance(error, ArgumentError) and "registry" in str(error):
            failure_code = "trace_registry_rejected"
        else:
            failure_code = "trace_registry_publication_failed"
        error_class = type(error).__name__ if type(error).__name__.replace("_", "").isalnum() else "AutonomousRunTraceRegistryPublicationError"
        return AutonomousRunTraceRegistryPublication(
            status="failed",
            run_id=normalized_run_id,
            run_import_state=base["run_import_state"],
            source_snapshot_digest=source_snapshot_digest,
            registry_snapshot_digest=base["registry_snapshot_digest"],
            evicted_run_count=base["evicted_run_count"],
            error_class=error_class[:128],
            failure_code=failure_code,
        )


def validate_autonomous_run_trace_registry_snapshot(raw: Mapping[str, Any] | AutonomousRunTraceRegistrySnapshot, *, max_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES) -> AutonomousRunTraceRegistrySnapshot:
    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 16_000 <= max_bytes <= MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES:
        raise ArgumentError("autonomous run trace registry validation max_bytes is outside its bounds")
    return _validate_snapshot(raw.to_dict() if isinstance(raw, AutonomousRunTraceRegistrySnapshot) else raw, max_bytes)


class JsonAutonomousRunTraceRegistryPersistence:
    """Canonical JSON adapter over an application-owned registry text store."""

    def __init__(self, store: AutonomousRunTraceTextStore, *, max_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous run trace registry JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 16_000 <= max_bytes <= MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES:
            raise ArgumentError("autonomous run trace registry persistence max_bytes is outside its bounds")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> AutonomousRunTraceRegistrySnapshot | None:
        text = self.store.read()
        if text is None:
            return None
        if not isinstance(text, str) or len(text.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("autonomous run trace registry JSON exceeds its byte bound")
        try:
            raw = json.loads(text)
        except (TypeError, json.JSONDecodeError) as error:
            raise ArgumentError("autonomous run trace registry JSON is invalid") from error
        if canonical_json(raw) != text:
            raise ArgumentError("autonomous run trace registry JSON is not canonical")
        return _validate_snapshot(raw, self.max_bytes)

    def write(self, snapshot: AutonomousRunTraceRegistrySnapshot | Mapping[str, Any]) -> None:
        raw = snapshot.to_dict() if isinstance(snapshot, AutonomousRunTraceRegistrySnapshot) else snapshot
        verified = _validate_snapshot(raw, self.max_bytes)
        self.store.write(canonical_json(verified.to_dict()))


class TransactionalJsonAutonomousRunTraceRegistryPersistence(JsonAutonomousRunTraceRegistryPersistence):
    def __init__(self, store: AutonomousRunTraceTransactionalTextStore, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("autonomous run trace registry transactional persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: AutonomousRunTraceRegistrySnapshot | Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest("autonomous run trace registry expected snapshot_digest", expected_snapshot_digest)
        raw = snapshot.to_dict() if isinstance(snapshot, AutonomousRunTraceRegistrySnapshot) else snapshot
        verified = _validate_snapshot(raw, self.max_bytes)
        return self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(verified.to_dict()))


class AutonomousRunTraceRegistryPersistenceCoordinator:
    """Restore/flush coordinator retaining an atomic snapshot digest fence."""

    def __init__(self, registry: AutonomousRunTraceRegistry, persistence: JsonAutonomousRunTraceRegistryPersistence) -> None:
        if not all(callable(getattr(registry, name, None)) for name in ("snapshot", "restore")):
            raise ArgumentError("autonomous run trace registry persistence requires a registry")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous run trace registry persistence adapter is malformed")
        self.registry = registry
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> AutonomousRunTraceRegistrySnapshot | None:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
                return None
            self.registry.restore(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot

    def flush(self) -> AutonomousRunTraceRegistrySnapshot:
        with self._lock:
            snapshot = self.registry.snapshot()
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("autonomous run trace registry persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot


__all__ = [
    "AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION",
    "AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY",
    "AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL",
    "MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS",
    "MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS",
    "MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES",
    "AutonomousRunTraceRetentionPolicy",
    "AutonomousRunTraceRegistryRecord",
    "AutonomousRunTraceRegistrySnapshot",
    "AutonomousRunTraceRegistryPage",
    "AutonomousRunTraceRegistryImportReport",
    "AutonomousRunTraceRegistryPublication",
    "AutonomousRunTraceRegistryIntegrity",
    "AutonomousRunTraceRegistry",
    "validate_autonomous_run_trace_registry_snapshot",
    "JsonAutonomousRunTraceRegistryPersistence",
    "TransactionalJsonAutonomousRunTraceRegistryPersistence",
    "AutonomousRunTraceRegistryPersistenceCoordinator",
    "AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA",
    "publish_autonomous_run_trace_registry_snapshot",
]
