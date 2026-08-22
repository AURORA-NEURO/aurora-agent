"""Durable, metadata-only worker orchestration for autonomous evidence runtimes.

``autonomous_evidence_runtime`` owns the caller's acquisition, projection, evaluator, and
transient value.  This module owns the process boundary around it: idempotent work identities,
leases, fencing, bounded retry, explicit evaluator handoff, reconciliation quarantine, and
restart-safe snapshots.  It never serializes a source payload, prompt, credential, request
metadata value, or evaluator input.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
import math
import threading
import time
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_runtime import (
    AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeResult,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES, _identifier
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA = "bioprism-python-autonomous-evidence-work-item/0.1"
AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA = "bioprism-python-autonomous-evidence-work-queue/0.1"
AUTONOMOUS_EVIDENCE_WORKER_SCHEMA = "bioprism-python-autonomous-evidence-worker/0.1"
MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS = 4_096
MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS = 32
MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH = 128
MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS = 600_000
MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES = 8_000_000

_WORK_STATUSES = frozenset({
    "queued", "leased", "completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled",
})
_WORK_FAILURE_CLASSES = frozenset({
    None,
    "rehydration_missing", "rehydration_invalid", "identity_conflict", "lease_expired",
    "acquisition_failed", "projection_failed", "evaluation_pending", "evaluation_rejected",
    "executor_error", "transport_error", "unknown",
})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _digest(name: str, value: Any, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _timestamp(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 8_640_000_000_000_000:
        raise ArgumentError(f"{name} must be a bounded epoch millisecond timestamp")
    return value


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _request_identifier(name: str, value: Any) -> str:
    """Validate identifiers shared with the evidence-plan contract.

    Evidence requirement IDs are intentionally composite (for example,
    ``coding:scope:scope``), so they use the same bounded character set as
    ``autonomous_evidence`` rather than the narrower process/lease ID set.
    """

    result = _text(name, value, 256)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-+ /" for character in result):
        raise ArgumentError(f"{name} must be a bounded evidence identifier")
    return result


def _now_ms(value: int | None) -> int:
    return _timestamp("time", int(time.time() * 1000) if value is None else value)


def _domain(name: str, value: Any) -> str:
    result = _text(name, value, 256)
    if result not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError(f"{name} is not a supported autonomous domain")
    return result


def _digests(name: str, value: Any, maximum: int = 128) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} entries")
    result = tuple(_digest(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicates")
    return result  # type: ignore[return-value]


def _assert_metadata(value: Any, name: str, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = str(key).lower().replace("_", "")
            if normalized in {"apikey", "authorization", "bearer", "credential", "credentials", "password", "privatekey", "refreshtoken", "secret", "token"}:
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _assert_metadata(child, f"{name}.{key}", depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_metadata(child, f"{name}[{index}]", depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError(f"{name} contains a non-finite number")


def _request_mapping(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence work request must be a mapping")
    requirement_id = _request_identifier("evidence work request requirement_id", value.get("requirement_id"))
    source_id = _request_identifier("evidence work request source_id", value.get("source_id"))
    source_digest = _digest("evidence work request source_digest", value.get("source_digest"), allow_none=True)
    request_id = None if value.get("request_id") is None else _request_identifier("evidence work request request_id", value.get("request_id"))
    metadata = value.get("metadata", {})
    if not isinstance(metadata, Mapping):
        raise ArgumentError("evidence work request metadata must be a mapping")
    _assert_metadata(metadata, "evidence work request metadata")
    if len(canonical_json(metadata).encode("utf-8")) > 64_000:
        raise ArgumentError("evidence work request metadata exceeds its byte bound")
    return {"requirement_id": requirement_id, "source_id": source_id, "source_digest": source_digest, "request_id": request_id, "metadata": dict(metadata)}


def _request_digest(plan_digest: str, request: Mapping[str, Any]) -> str:
    normalized = _request_mapping(request)
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
        "plan_digest": plan_digest,
        "requirement_id": normalized["requirement_id"],
        "source_id": normalized["source_id"],
        "source_digest": normalized["source_digest"],
        "request_id": normalized["request_id"],
        "metadata": normalized["metadata"],
    })


def _requirement(plan: AutonomousEvidencePlan, requirement_id: str) -> Any:
    for requirement in plan.requirements:
        if requirement.requirement_id == requirement_id:
            return requirement
    raise ArgumentError(f"evidence work requirement is not in the plan: {requirement_id}")


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceWorkItem:
    work_id: str
    plan_digest: str
    requirement_id: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_id: str
    source_id: str
    source_digest: str | None
    request_digest: str
    parent_evidence_digests: tuple[str, ...]
    max_attempts: int
    attempts: int
    status: str
    available_at: int
    lease_owner: str | None
    lease_until: int | None
    receipt_digest: str | None
    assessment_digest: str | None
    result_digest: str | None
    failure_class: str | None
    last_error_class: str | None
    created_at: int
    updated_at: int
    item_digest: str = ""

    def __post_init__(self) -> None:
        _identifier("autonomous evidence work_id", self.work_id)
        _digest("autonomous evidence work plan_digest", self.plan_digest)
        _request_identifier("autonomous evidence work requirement_id", self.requirement_id)
        _domain("autonomous evidence work domain", self.domain)
        _identifier("autonomous evidence work workflow_id", self.workflow_id)
        _digest("autonomous evidence work workflow_digest", self.workflow_digest)
        _identifier("autonomous evidence work stage_id", self.stage_id)
        _request_identifier("autonomous evidence work source_id", self.source_id)
        _digest("autonomous evidence work source_digest", self.source_digest, allow_none=True)
        _digest("autonomous evidence work request_digest", self.request_digest)
        parents = _digests("autonomous evidence work parent_evidence_digests", self.parent_evidence_digests, 64)
        object.__setattr__(self, "parent_evidence_digests", parents)
        max_attempts = _bounded_integer("autonomous evidence work max_attempts", self.max_attempts, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS)
        attempts = _bounded_integer("autonomous evidence work attempts", self.attempts, 0, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS)
        if attempts > max_attempts:
            raise ArgumentError("autonomous evidence work attempts exceed max_attempts")
        object.__setattr__(self, "max_attempts", max_attempts)
        object.__setattr__(self, "attempts", attempts)
        if self.status not in _WORK_STATUSES:
            raise ArgumentError("autonomous evidence work status is invalid")
        _timestamp("autonomous evidence work available_at", self.available_at)
        _timestamp("autonomous evidence work created_at", self.created_at)
        _timestamp("autonomous evidence work updated_at", self.updated_at)
        if self.lease_owner is not None:
            _identifier("autonomous evidence work lease_owner", self.lease_owner)
        if self.lease_until is not None:
            _timestamp("autonomous evidence work lease_until", self.lease_until)
        if (self.status == "leased") != (self.lease_owner is not None and self.lease_until is not None):
            raise ArgumentError("autonomous evidence work lease state is inconsistent")
        for name, value in (("receipt_digest", self.receipt_digest), ("assessment_digest", self.assessment_digest), ("result_digest", self.result_digest)):
            _digest(f"autonomous evidence work {name}", value, allow_none=True)
        if self.failure_class not in _WORK_FAILURE_CLASSES or self.last_error_class not in _WORK_FAILURE_CLASSES:
            raise ArgumentError("autonomous evidence work failure class is invalid")
        if self.status not in {"awaiting_evaluation", "reconciliation_required", "failed", "cancelled"} and self.failure_class is not None:
            raise ArgumentError("autonomous evidence work active item cannot retain a terminal failure class")
        if self.item_digest:
            _digest("autonomous evidence work item_digest", self.item_digest)
            if self.item_digest != self.computed_digest:
                raise ArgumentError("autonomous evidence work item digest is invalid")
        else:
            object.__setattr__(self, "item_digest", self.computed_digest)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA,
            "work_id": self.work_id,
            "plan_digest": self.plan_digest,
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage_id,
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "request_digest": self.request_digest,
            "parent_evidence_digests": list(self.parent_evidence_digests),
            "max_attempts": self.max_attempts,
            "attempts": self.attempts,
            "status": self.status,
            "available_at": self.available_at,
            "lease_owner": self.lease_owner,
            "lease_until": self.lease_until,
            "receipt_digest": self.receipt_digest,
            "assessment_digest": self.assessment_digest,
            "result_digest": self.result_digest,
            "failure_class": self.failure_class,
            "last_error_class": self.last_error_class,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "retention": "metadata_only_request_and_values_caller_owned",
            "secret_material": "never_returned",
        }

    @property
    def computed_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "item_digest": self.item_digest}


def _work_item_from_mapping(value: Mapping[str, Any]) -> AutonomousEvidenceWorkItem:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA:
        raise ArgumentError("autonomous evidence work item is malformed")
    if value.get("retention") != "metadata_only_request_and_values_caller_owned" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous evidence work item retention is invalid")
    return AutonomousEvidenceWorkItem(
        work_id=value.get("work_id"), plan_digest=value.get("plan_digest"), requirement_id=value.get("requirement_id"),
        domain=value.get("domain"), workflow_id=value.get("workflow_id"), workflow_digest=value.get("workflow_digest"),
        stage_id=value.get("stage_id"), source_id=value.get("source_id"), source_digest=value.get("source_digest"),
        request_digest=value.get("request_digest"), parent_evidence_digests=tuple(value.get("parent_evidence_digests", ())),
        max_attempts=value.get("max_attempts"), attempts=value.get("attempts"), status=value.get("status"),
        available_at=value.get("available_at"), lease_owner=value.get("lease_owner"), lease_until=value.get("lease_until"),
        receipt_digest=value.get("receipt_digest"), assessment_digest=value.get("assessment_digest"), result_digest=value.get("result_digest"),
        failure_class=value.get("failure_class"), last_error_class=value.get("last_error_class"),
        created_at=value.get("created_at"), updated_at=value.get("updated_at"), item_digest=value.get("item_digest"),
    )


def _result_metadata(item: AutonomousEvidenceWorkItem, result: AutonomousEvidenceRuntimeResult) -> tuple[str, str, str | None, str, str]:
    if not isinstance(result, AutonomousEvidenceRuntimeResult):
        raise ArgumentError("evidence work result must be a typed runtime result")
    payload = result.to_dict()
    if payload.get("schema") != "bioprism-python-autonomous-evidence-runtime/0.1" or not isinstance(payload.get("plan"), Mapping) or not isinstance(payload.get("receipts"), Sequence):
        raise ArgumentError("evidence work result schema is invalid")
    receipt = next((candidate for candidate in payload["receipts"] if isinstance(candidate, Mapping) and candidate.get("request_digest") == item.request_digest), None)
    if not isinstance(receipt, Mapping) or not isinstance(receipt.get("receipt_digest"), str):
        raise ArgumentError("evidence work result does not contain the queued request")
    if receipt.get("plan_digest") != item.plan_digest:
        raise ArgumentError("evidence work result receipt belongs to a different plan")
    descriptor = {
        "schema": payload["schema"], "status": payload["status"], "plan_digest": payload["plan"]["plan_digest"],
        "receipt_digests": [candidate.get("receipt_digest") if isinstance(candidate, Mapping) else None for candidate in payload["receipts"]],
        "assessment_digests": [candidate.get("assessment_digest") if isinstance(candidate, Mapping) else None for candidate in payload.get("assessments", ())],
        "completed_requirement_ids": payload["completed_requirement_ids"],
        "pending_evaluation_requirement_ids": payload["pending_evaluation_requirement_ids"],
        "missing_requirement_ids": payload["missing_requirement_ids"],
        "next_stage_ids": payload["next_stage_ids"],
        "omitted_request_digests": payload["omitted_request_digests"],
        "retention": "metadata_only;raw_values_caller_owned",
        "secret_material": "never_returned",
    }
    result_digest = _digest("evidence work result result_digest", payload.get("result_digest"))
    if result_digest != content_digest(descriptor):
        raise ArgumentError("evidence work result digest is invalid")
    assessment = next((candidate for candidate in payload.get("assessments", ()) if isinstance(candidate, Mapping) and candidate.get("requirement_id") == item.requirement_id), None)
    assessment_digest = None if not isinstance(assessment, Mapping) else _digest("evidence work result assessment_digest", assessment.get("assessment_digest"))
    return result_digest, _digest("evidence work result receipt_digest", receipt.get("receipt_digest")), assessment_digest, str(receipt.get("status")), str(receipt.get("evaluator_status"))  # type: ignore[return-value]


class InMemoryAutonomousEvidenceWorkQueue:
    """Thread-safe queue with leases, retries, explicit handoffs, and digest-bound snapshots."""

    def __init__(self, *, max_items: int = MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS) -> None:
        self.max_items = _bounded_integer("autonomous evidence work queue max_items", max_items, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS)
        self._items: dict[str, AutonomousEvidenceWorkItem] = {}
        self._lock = threading.RLock()

    @staticmethod
    def _refresh(item: AutonomousEvidenceWorkItem, now: int, **updates: Any) -> AutonomousEvidenceWorkItem:
        return replace(item, **updates, updated_at=now, item_digest="")

    def enqueue(self, *, work_id: str, plan: AutonomousEvidencePlan, request: Mapping[str, Any], parent_evidence_digests: Sequence[str] = (), max_attempts: int = 3, available_at: int | None = None, now: int | None = None) -> AutonomousEvidenceWorkItem:
        if not isinstance(plan, AutonomousEvidencePlan):
            raise ArgumentError("autonomous evidence work enqueue requires a typed plan")
        work_id = _identifier("autonomous evidence work_id", work_id)
        normalized = _request_mapping(request)
        requirement = _requirement(plan, normalized["requirement_id"])
        request_digest = _request_digest(plan.plan_digest, normalized)
        parents = _digests("autonomous evidence work parent_evidence_digests", parent_evidence_digests, 64)
        max_attempts = _bounded_integer("autonomous evidence work max_attempts", max_attempts, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS)
        current = _now_ms(now)
        with self._lock:
            existing = self._items.get(work_id)
            if existing is not None:
                if existing.plan_digest != plan.plan_digest or existing.request_digest != request_digest or existing.requirement_id != requirement.requirement_id:
                    raise ArgumentError("autonomous evidence work identity conflicts with an existing work item")
                return existing
            if len(self._items) >= self.max_items:
                raise ArgumentError("autonomous evidence work queue is full")
            item = AutonomousEvidenceWorkItem(
                work_id=work_id, plan_digest=plan.plan_digest, requirement_id=requirement.requirement_id, domain=requirement.domain,
                workflow_id=requirement.workflow_id, workflow_digest=requirement.workflow_digest, stage_id=requirement.stage_id,
                source_id=normalized["source_id"], source_digest=normalized["source_digest"], request_digest=request_digest,
                parent_evidence_digests=parents, max_attempts=max_attempts, attempts=0, status="queued",
                available_at=_timestamp("autonomous evidence work available_at", current if available_at is None else available_at),
                lease_owner=None, lease_until=None, receipt_digest=None, assessment_digest=None, result_digest=None,
                failure_class=None, last_error_class=None, created_at=current, updated_at=current,
            )
            self._items[work_id] = item
            return item

    def get(self, work_id: str) -> AutonomousEvidenceWorkItem | None:
        with self._lock:
            return self._items.get(_identifier("autonomous evidence work_id", work_id))

    def pending(self, *, limit: int = 64, now: int | None = None) -> tuple[AutonomousEvidenceWorkItem, ...]:
        current = _now_ms(now)
        limit = _bounded_integer("autonomous evidence work pending limit", limit, 1, min(MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH, self.max_items))
        with self._lock:
            values = [item for item in self._items.values() if (item.status == "queued" and item.available_at <= current and item.attempts < item.max_attempts) or (item.status == "leased" and item.lease_until is not None and item.lease_until <= current and item.attempts < item.max_attempts)]
        return tuple(sorted(values, key=lambda item: (item.available_at, item.created_at, item.work_id))[:limit])

    def claim(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousEvidenceWorkItem | None:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        lease_ms = _bounded_integer("autonomous evidence work lease_ms", lease_ms, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {"completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"}:
                return None
            if item.status == "leased" and item.lease_until is not None and item.lease_until > current:
                return None
            if item.attempts >= item.max_attempts:
                self._items[work_id] = self._refresh(item, current, status="reconciliation_required", failure_class="lease_expired", last_error_class="lease_expired", lease_owner=None, lease_until=None)
                return None
            claimed = self._refresh(item, current, status="leased", attempts=item.attempts + 1, lease_owner=worker_id, lease_until=current + lease_ms, failure_class=None, last_error_class=None)
            self._items[work_id] = claimed
            return claimed

    def renew(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        lease_ms = _bounded_integer("autonomous evidence work lease_ms", lease_ms, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous evidence work lease cannot be renewed by this worker")
            renewed = self._refresh(item, current, lease_until=current + lease_ms)
            self._items[work_id] = renewed
            return renewed

    def complete(self, work_id: str, worker_id: str, result: AutonomousEvidenceRuntimeResult, *, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous evidence work completion is fenced by an expired or foreign lease")
            metadata = _result_metadata(item, result)
            if result.status != "completed" and not (result.status == "awaiting_evaluation" and metadata[4] == "accepted"):
                raise ArgumentError("autonomous evidence work completion requires an accepted queued requirement")
            finished = self._refresh(item, current, status="completed", lease_owner=None, lease_until=None, receipt_digest=metadata[1], assessment_digest=metadata[2], result_digest=metadata[0], failure_class=None)
            self._items[work_id] = finished
            return finished

    def await_evaluation(self, work_id: str, worker_id: str, result: AutonomousEvidenceRuntimeResult, *, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous evidence evaluation handoff is fenced by an expired or foreign lease")
            if result.status != "awaiting_evaluation":
                raise ArgumentError("autonomous evidence evaluation handoff requires an awaiting_evaluation runtime result")
            metadata = _result_metadata(item, result)
            waiting = self._refresh(item, current, status="awaiting_evaluation", lease_owner=None, lease_until=None, receipt_digest=metadata[1], assessment_digest=metadata[2], result_digest=metadata[0], failure_class="evaluation_pending", last_error_class="evaluation_pending")
            self._items[work_id] = waiting
            return waiting

    def fail(self, work_id: str, worker_id: str, error_class: str, *, retryable: bool, now: int | None = None, result: AutonomousEvidenceRuntimeResult | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        failure = error_class if error_class in _WORK_FAILURE_CLASSES else "unknown"
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous evidence work failure is fenced by an expired or foreign lease")
            metadata = None if result is None else _result_metadata(item, result)
            can_retry = retryable and item.attempts < item.max_attempts
            delay = min(3_600_000, 1_000 * (2 ** max(0, item.attempts - 1)))
            failed = self._refresh(item, current, status="queued" if can_retry else "failed", available_at=current + delay if can_retry else item.available_at, lease_owner=None, lease_until=None, receipt_digest=metadata[1] if metadata else item.receipt_digest, assessment_digest=metadata[2] if metadata else item.assessment_digest, result_digest=metadata[0] if metadata else item.result_digest, failure_class=None if can_retry else failure, last_error_class=failure)
            self._items[work_id] = failed
            return failed

    def reconcile(self, work_id: str, worker_id: str, error_class: str = "rehydration_missing", *, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        failure = error_class if error_class in _WORK_FAILURE_CLASSES else "unknown"
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status != "leased" or item.lease_owner != worker_id or item.lease_until is None or item.lease_until <= current:
                raise ArgumentError("autonomous evidence reconciliation is fenced by an expired or foreign lease")
            reconciled = self._refresh(item, current, status="reconciliation_required", lease_owner=None, lease_until=None, failure_class=failure, last_error_class=failure)
            self._items[work_id] = reconciled
            return reconciled

    def requeue(self, work_id: str, *, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status not in {"awaiting_evaluation", "reconciliation_required"}:
                raise ArgumentError("autonomous evidence work is not waiting for explicit requeue")
            if item.attempts >= item.max_attempts:
                raise ArgumentError("autonomous evidence work has exhausted its attempts")
            queued = self._refresh(item, current, status="queued", available_at=current, failure_class=None, last_error_class=item.last_error_class)
            self._items[work_id] = queued
            return queued

    def cancel(self, work_id: str, reason: str = "unknown", *, now: int | None = None) -> AutonomousEvidenceWorkItem:
        work_id = _identifier("autonomous evidence work_id", work_id)
        failure = reason if reason in _WORK_FAILURE_CLASSES else "unknown"
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {"completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"}:
                raise ArgumentError("autonomous evidence work cannot be cancelled in its current state")
            cancelled = self._refresh(item, current, status="cancelled", lease_owner=None, lease_until=None, failure_class=failure, last_error_class=failure)
            self._items[work_id] = cancelled
            return cancelled

    def rows(self) -> tuple[AutonomousEvidenceWorkItem, ...]:
        with self._lock:
            return tuple(sorted(self._items.values(), key=lambda item: (item.created_at, item.work_id)))

    def verify_integrity(self) -> dict[str, Any]:
        for item in self.rows():
            if item.item_digest != item.computed_digest:
                raise ArgumentError("autonomous evidence work item digest is invalid")
        return {"schema": AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA, "verified": True, "items": len(self._items), "retention": "metadata_only_request_and_values_caller_owned", "secret_material": "never_returned"}

    def snapshot(self) -> dict[str, Any]:
        self.verify_integrity()
        descriptor = {"schema": AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA, "items": [item.to_dict() for item in self.rows()], "retention": "metadata_only_request_and_values_caller_owned", "secret_material": "never_returned"}
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES:
            raise ArgumentError("autonomous evidence work queue snapshot exceeds its bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        if not isinstance(snapshot, Mapping) or snapshot.get("schema") != AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA or not isinstance(snapshot.get("items"), Sequence) or isinstance(snapshot.get("items"), (str, bytes)):
            raise ArgumentError("autonomous evidence work queue snapshot is malformed")
        if snapshot.get("retention") != "metadata_only_request_and_values_caller_owned" or snapshot.get("secret_material") != "never_returned":
            raise ArgumentError("autonomous evidence work queue snapshot retention is invalid")
        descriptor = {key: value for key, value in snapshot.items() if key != "snapshot_digest"}
        if content_digest(descriptor) != snapshot.get("snapshot_digest"):
            raise ArgumentError("autonomous evidence work queue snapshot digest is invalid")
        if len(snapshot["items"]) > self.max_items:
            raise ArgumentError("autonomous evidence work queue snapshot exceeds max_items")
        restored: dict[str, AutonomousEvidenceWorkItem] = {}
        for raw in snapshot["items"]:
            item = _work_item_from_mapping(raw)
            if item.work_id in restored:
                raise ArgumentError("autonomous evidence work queue snapshot contains duplicate work ids")
            restored[item.work_id] = item
        with self._lock:
            self._items = restored


class AutonomousEvidenceWorkQueuePersistenceCoordinator:
    def __init__(self, queue: InMemoryAutonomousEvidenceWorkQueue, persistence: Any) -> None:
        if not isinstance(queue, InMemoryAutonomousEvidenceWorkQueue):
            raise ArgumentError("autonomous evidence work persistence requires a typed queue")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            raise ArgumentError("autonomous evidence work persistence adapter is malformed")
        self.queue = queue
        self.persistence = persistence

    def restore(self) -> dict[str, Any]:
        snapshot = self.persistence.read()
        if snapshot is None:
            return {"status": "empty", "snapshot_digest": None, "items": 0}
        self.queue.restore(snapshot)
        return {"status": "restored", "snapshot_digest": snapshot["snapshot_digest"], "items": self.queue.verify_integrity()["items"]}

    def flush(self) -> dict[str, Any]:
        snapshot = self.queue.snapshot()
        self.persistence.write(snapshot)
        return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceWorkerRow:
    work_id: str
    outcome: str
    attempts: int
    receipt_digest: str | None
    assessment_digest: str | None
    result_digest: str | None
    value_retained: bool = False
    error_class: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {"work_id": self.work_id, "outcome": self.outcome, "attempts": self.attempts, "receipt_digest": self.receipt_digest, "assessment_digest": self.assessment_digest, "result_digest": self.result_digest, "value_retained": False, "error_class": self.error_class}


class AutonomousEvidenceWorker:
    """Run queued requests through a caller-owned runtime and explicit rehydrator."""

    def __init__(self, queue: InMemoryAutonomousEvidenceWorkQueue, rehydrate: Callable[[AutonomousEvidenceWorkItem], Mapping[str, Any]]) -> None:
        if not isinstance(queue, InMemoryAutonomousEvidenceWorkQueue):
            raise ArgumentError("autonomous evidence worker requires a typed work queue")
        if not callable(rehydrate):
            raise ArgumentError("autonomous evidence worker requires a rehydrator")
        self.queue = queue
        self.rehydrate = rehydrate

    def run(self, *, worker_id: str = "evidence-worker", limit: int = 64, lease_ms: int = 30_000, now: int | None = None, aborted: Callable[[], bool] | None = None, work_ids: Sequence[str] | None = None) -> dict[str, Any]:
        worker_id = _identifier("autonomous evidence worker_id", worker_id)
        limit = _bounded_integer("autonomous evidence worker limit", limit, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH)
        lease_ms = _bounded_integer("autonomous evidence worker lease_ms", lease_ms, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS)
        selected = None if work_ids is None else tuple(_identifier("autonomous evidence worker work_id", value) for value in work_ids)
        if selected is not None and (not 1 <= len(selected) <= MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH or len(set(selected)) != len(selected)):
            raise ArgumentError("autonomous evidence worker work_ids are outside their bound")
        deterministic = now is not None
        current = _now_ms(now)
        pending = self.queue.pending(limit=MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH if selected is not None else limit, now=current)
        candidates = tuple(item for item in pending if selected is None or item.work_id in selected)[:limit]
        rows: list[AutonomousEvidenceWorkerRow] = []
        for candidate in candidates:
            if aborted is not None and aborted():
                break
            claimed = self.queue.claim(candidate.work_id, worker_id, lease_ms=lease_ms, now=current)
            if claimed is None:
                rows.append(AutonomousEvidenceWorkerRow(candidate.work_id, "leased_elsewhere", candidate.attempts, candidate.receipt_digest, candidate.assessment_digest, candidate.result_digest))
                continue
            finish_now = current if deterministic else None
            try:
                hydrated = self.rehydrate(claimed)
                if not isinstance(hydrated, Mapping) or not isinstance(hydrated.get("runtime"), AutonomousEvidenceRuntime) or not isinstance(hydrated.get("plan"), AutonomousEvidencePlan) or not isinstance(hydrated.get("request"), Mapping) or not isinstance(hydrated.get("execute"), Mapping):
                    reconciled = self.queue.reconcile(claimed.work_id, worker_id, now=finish_now)
                    rows.append(self._row(reconciled, "reconciliation_required", "rehydration_missing"))
                    continue
                plan = hydrated["plan"]
                request = _request_mapping(hydrated["request"])
                requirement = _requirement(plan, request["requirement_id"])
                execute = dict(hydrated["execute"])
                if plan.plan_digest != claimed.plan_digest or hydrated["runtime"].plan.plan_digest != claimed.plan_digest or _request_digest(plan.plan_digest, request) != claimed.request_digest or requirement.domain != claimed.domain or requirement.workflow_digest != claimed.workflow_digest or request["source_id"] != claimed.source_id or request["source_digest"] != claimed.source_digest:
                    raise ArgumentError("autonomous evidence worker rehydrated identity conflicts with the work item")
                if "acquirer" not in execute:
                    raise ArgumentError("autonomous evidence worker rehydrated execution is missing an acquirer")
                result = hydrated["runtime"].execute(
                    [request], acquirer=execute["acquirer"], projector=execute.get("projector"), evaluator=execute.get("evaluator"),
                    rehydrate_value=execute.get("rehydrate_value"), parent_evidence_digests=claimed.parent_evidence_digests,
                    stop_on_failure=bool(execute.get("stop_on_failure", False)), reevaluate_pending=bool(execute.get("reevaluate_pending", False)),
                )
                queued_receipt = next((receipt for receipt in result.receipts if receipt.request_digest == claimed.request_digest), None)
                if result.status == "completed" or (result.status == "awaiting_evaluation" and queued_receipt is not None and queued_receipt.evaluator_status == "accepted"):
                    finished = self.queue.complete(claimed.work_id, worker_id, result, now=finish_now)
                    rows.append(self._row(finished, "replayed" if queued_receipt is not None and queued_receipt.replay == "replayed" else "completed", None))
                elif result.status == "awaiting_evaluation":
                    waiting = self.queue.await_evaluation(claimed.work_id, worker_id, result, now=finish_now)
                    rows.append(self._row(waiting, "awaiting_evaluation", "evaluation_pending"))
                elif result.status == "reconciliation_required":
                    reconciled = self.queue.reconcile(claimed.work_id, worker_id, now=finish_now)
                    rows.append(self._row(reconciled, "reconciliation_required", "rehydration_missing"))
                else:
                    failure = "projection_failed" if any(receipt.evidence_status == "projection_failed" for receipt in result.receipts) else "acquisition_failed"
                    failed = self.queue.fail(claimed.work_id, worker_id, failure, retryable=True, now=finish_now, result=result)
                    rows.append(self._row(failed, "retry_scheduled" if failed.status == "queued" else "failed", failure))
            except Exception as error:
                failure = self._classify(error)
                if failure in {"rehydration_missing", "rehydration_invalid", "identity_conflict"}:
                    reconciled = self.queue.reconcile(claimed.work_id, worker_id, failure, now=finish_now)
                    rows.append(self._row(reconciled, "reconciliation_required", failure))
                else:
                    failed = self.queue.fail(claimed.work_id, worker_id, failure, retryable=failure in {"executor_error", "transport_error", "unknown"}, now=finish_now)
                    rows.append(self._row(failed, "retry_scheduled" if failed.status == "queued" else "failed", failure))
        return {
            "schema": AUTONOMOUS_EVIDENCE_WORKER_SCHEMA,
            "worker_id": worker_id,
            "inspected": len(candidates),
            "completed": sum(row.outcome in {"completed", "replayed"} for row in rows),
            "retried": sum(row.outcome == "retry_scheduled" for row in rows),
            "awaiting_evaluation": sum(row.outcome == "awaiting_evaluation" for row in rows),
            "failed": sum(row.outcome == "failed" for row in rows),
            "reconciled": sum(row.outcome == "reconciliation_required" for row in rows),
            "leased_elsewhere": sum(row.outcome == "leased_elsewhere" for row in rows),
            "rows": [row.to_dict() for row in rows],
            "retention": "metadata_only_receipts_and_digests_no_values",
            "secret_material": "never_returned",
        }

    @staticmethod
    def _row(item: AutonomousEvidenceWorkItem, outcome: str, error_class: str | None) -> AutonomousEvidenceWorkerRow:
        return AutonomousEvidenceWorkerRow(item.work_id, outcome, item.attempts, item.receipt_digest, item.assessment_digest, item.result_digest, False, error_class)

    @staticmethod
    def _classify(error: Exception) -> str:
        message = str(error).lower()
        if "rehydrat" in message or "runtime plan" in message or "work item" in message:
            return "rehydration_missing" if "missing" in message else "identity_conflict" if "identity" in message or "conflict" in message else "rehydration_invalid"
        if "projection" in message:
            return "projection_failed"
        if "transport" in message:
            return "transport_error"
        if "acquisition" in message:
            return "acquisition_failed"
        if "evaluator" in message:
            return "evaluation_rejected"
        if "executor" in message:
            return "executor_error"
        return "unknown"


__all__ = [
    "AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA",
    "AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA",
    "AUTONOMOUS_EVIDENCE_WORKER_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS",
    "MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS",
    "MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH",
    "MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS",
    "MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES",
    "AutonomousEvidenceWorkItem",
    "InMemoryAutonomousEvidenceWorkQueue",
    "AutonomousEvidenceWorkQueuePersistenceCoordinator",
    "AutonomousEvidenceWorkerRow",
    "AutonomousEvidenceWorker",
]
