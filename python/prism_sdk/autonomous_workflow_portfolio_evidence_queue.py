"""Lease-fenced multi-worker execution for portfolio evidence.

The portfolio evidence supervisor is useful inside one process, but a service normally needs a
durable handoff between admission, workers, source adapters, evaluator settlement, and restart.
This module owns that handoff without owning any private task, source, credential, evaluator
payload, or raw evidence value.  Every work item is bound to the reviewed portfolio, provider
execution, evidence plan, request digest, optional admission, and optional evidence checkpoint.

Workers receive metadata identities and a caller-owned executor.  Leases fence completion and
renewal, dependency waves prevent a child from running before its direct predecessors settle,
retries are bounded and delayed, evaluator/reconciliation states require explicit requeue, and all
snapshots are canonical metadata-only projections.  The optional atomic coordinator reloads and
CAS-commits every state transition for services whose persistence adapter provides real compare-
and-swap semantics.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomy import AUTONOMOUS_DOMAINS
from .autonomous_workflow_portfolio import (
    AutonomousWorkflowPortfolioExecutionResult,
)
from .errors import ArgumentError


AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence-work-queue/0.2"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence-work-item/0.2"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SQLITE_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence-work-queue-sqlite/0.2"
)
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS = 64
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS = 300_000
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS = 8
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES = 256_000

_RETENTION = "metadata_only_task_sources_values_and_provider_payloads_never_persisted"
_WORKER_RETENTION = "metadata_only_receipts_and_digests_no_values"
_SECRET_MATERIAL = "never_returned"
_STATUSES = frozenset(
    {
        "queued",
        "leased",
        "completed",
        "awaiting_evaluation",
        "failed",
        "reconciliation_required",
        "cancelled",
    }
)
_FAILURES = frozenset(
    {
        "dependency_failed",
        "provider_execution_not_succeeded",
        "lease_expired",
        "approval_required",
        "rehydration_missing",
        "identity_conflict",
        "evaluator_pending",
        "executor_error",
        "transport_error",
        "unknown",
    }
)
_PROVIDER_STATUSES = frozenset(
    {
        "succeeded",
        "failed",
        "blocked",
        "approval_required",
        "reconciliation_required",
        "not_started",
        "omitted",
    }
)
_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-+"
)


def _identifier(label: str, value: Any, maximum: int = 256) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(character not in _IDENTIFIER_CHARS for character in value)
    ):
        raise ArgumentError(f"{label} is outside its bounded identifier contract")
    return value


def _digest(label: str, value: Any, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ArgumentError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _bounded_integer(label: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{label} must be an integer between {minimum} and {maximum}")
    return value


def _timestamp(label: str, value: Any) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or not 0 <= value <= 8_640_000_000_000_000
    ):
        raise ArgumentError(f"{label} must be a bounded epoch-millisecond timestamp")
    return value


def _now_ms(value: int | None) -> int:
    return _timestamp("portfolio evidence work now", int(time.time() * 1000) if value is None else value)


def _domain(label: str, value: Any) -> str:
    result = _identifier(label, value)
    if result not in AUTONOMOUS_DOMAINS:
        raise ArgumentError(f"{label} is not a supported autonomous domain")
    return result


def _failure(label: str, value: Any, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if value not in _FAILURES:
        raise ArgumentError(f"{label} is not a recognized portfolio evidence work failure")
    return value


def _sequence(label: str, value: Any, maximum: int) -> tuple[Any, ...]:
    if (
        isinstance(value, (str, bytes, bytearray))
        or not isinstance(value, Sequence)
        or len(value) > maximum
    ):
        raise ArgumentError(f"{label} must contain at most {maximum} entries")
    return tuple(value)


def _identifier_sequence(label: str, value: Any, maximum: int) -> tuple[str, ...]:
    values = _sequence(label, value, maximum)
    normalized = tuple(_identifier(f"{label}[{index}]", item) for index, item in enumerate(values))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{label} must not contain duplicates")
    return normalized


def _json_digest(label: str, value: Any) -> str:
    try:
        return content_digest(value)
    except (TypeError, ValueError, OverflowError) as error:
        raise ArgumentError(f"{label} must be JSON-safe") from error


def autonomous_workflow_portfolio_provider_execution_digest(
    execution: AutonomousWorkflowPortfolioExecutionResult,
) -> str:
    """Derive the metadata identity used to bind provider execution into queue admission."""

    if not isinstance(execution, AutonomousWorkflowPortfolioExecutionResult):
        raise ArgumentError("portfolio evidence provider execution digest requires a typed result")
    return content_digest(
        {
            "schema": "bioprism-python-autonomous-workflow-portfolio-evidence-checkpoint/0.1",
            "plan_digest": execution.plan.portfolio_digest,
            "admission_digest": execution.admission_digest,
            "checkpoint_digest": execution.checkpoint.checkpoint_digest,
            "items": [item.to_dict() for item in execution.items],
        }
    )


def _item_descriptor(item: "AutonomousWorkflowPortfolioEvidenceWorkItem") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA,
        "work_id": item.work_id,
        "job_id": item.job_id,
        "item_id": item.item_id,
        "domain": item.domain,
        "wave_index": item.wave_index,
        "dependency_item_ids": list(item.dependency_item_ids),
        "provider_status": item.provider_status,
        "portfolio_plan_digest": item.portfolio_plan_digest,
        "admission_digest": item.admission_digest,
        "provider_execution_digest": item.provider_execution_digest,
        "evidence_plan_digest": item.evidence_plan_digest,
        "request_digest": item.request_digest,
        "checkpoint_digest": item.checkpoint_digest,
        "max_attempts": item.max_attempts,
        "attempts": item.attempts,
        "status": item.status,
        "available_at": item.available_at,
        "lease_owner": item.lease_owner,
        "lease_until": item.lease_until,
        "result_digest": item.result_digest,
        "failure_class": item.failure_class,
        "last_error_class": item.last_error_class,
        "created_at": item.created_at,
        "updated_at": item.updated_at,
    }


def _item_digest(item: "AutonomousWorkflowPortfolioEvidenceWorkItem") -> str:
    return content_digest(_item_descriptor(item))


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceWorkItem:
    """One metadata-only, lease-fenced portfolio evidence work item."""

    work_id: str
    job_id: str
    item_id: str
    domain: str
    wave_index: int
    dependency_item_ids: tuple[str, ...]
    provider_status: str
    portfolio_plan_digest: str
    admission_digest: str | None
    provider_execution_digest: str
    evidence_plan_digest: str
    request_digest: str
    checkpoint_digest: str | None
    max_attempts: int
    attempts: int
    status: str
    available_at: int
    lease_owner: str | None
    lease_until: int | None
    result_digest: str | None
    failure_class: str | None
    last_error_class: str | None
    created_at: int
    updated_at: int
    item_digest: str = ""

    def __post_init__(self) -> None:
        _identifier("portfolio evidence work_id", self.work_id)
        _identifier("portfolio evidence work job_id", self.job_id)
        _identifier("portfolio evidence work item_id", self.item_id)
        _domain("portfolio evidence work domain", self.domain)
        _bounded_integer(
            "portfolio evidence work wave_index",
            self.wave_index,
            0,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        dependencies = _identifier_sequence(
            "portfolio evidence work dependency_item_ids",
            self.dependency_item_ids,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        object.__setattr__(self, "dependency_item_ids", tuple(sorted(dependencies)))
        if self.provider_status not in _PROVIDER_STATUSES:
            raise ArgumentError("portfolio evidence work provider_status is invalid")
        _digest("portfolio evidence work portfolio_plan_digest", self.portfolio_plan_digest)
        _digest("portfolio evidence work admission_digest", self.admission_digest, allow_none=True)
        _digest("portfolio evidence work provider_execution_digest", self.provider_execution_digest)
        _digest("portfolio evidence work evidence_plan_digest", self.evidence_plan_digest)
        _digest("portfolio evidence work request_digest", self.request_digest)
        _digest("portfolio evidence work checkpoint_digest", self.checkpoint_digest, allow_none=True)
        max_attempts = _bounded_integer(
            "portfolio evidence work max_attempts",
            self.max_attempts,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS,
        )
        attempts = _bounded_integer(
            "portfolio evidence work attempts",
            self.attempts,
            0,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS,
        )
        if attempts > max_attempts:
            raise ArgumentError("portfolio evidence work attempts exceed max_attempts")
        object.__setattr__(self, "max_attempts", max_attempts)
        object.__setattr__(self, "attempts", attempts)
        if self.status not in _STATUSES:
            raise ArgumentError("portfolio evidence work status is invalid")
        _timestamp("portfolio evidence work available_at", self.available_at)
        _timestamp("portfolio evidence work created_at", self.created_at)
        _timestamp("portfolio evidence work updated_at", self.updated_at)
        if self.lease_owner is not None:
            _identifier("portfolio evidence work lease_owner", self.lease_owner)
        if self.lease_until is not None:
            _timestamp("portfolio evidence work lease_until", self.lease_until)
        if self.status == "leased" and (self.lease_owner is None or self.lease_until is None):
            raise ArgumentError("leased portfolio evidence work must have a lease")
        if self.status != "leased" and (self.lease_owner is not None or self.lease_until is not None):
            raise ArgumentError("non-leased portfolio evidence work cannot retain a lease")
        _digest("portfolio evidence work result_digest", self.result_digest, allow_none=True)
        _failure("portfolio evidence work failure_class", self.failure_class, allow_none=True)
        _failure("portfolio evidence work last_error_class", self.last_error_class, allow_none=True)
        if self.status in {"completed", "awaiting_evaluation"} and self.result_digest is None:
            raise ArgumentError("terminal portfolio evidence work requires a result digest")
        if self.item_digest == "" or self.item_digest == "0" * 64:
            object.__setattr__(self, "item_digest", _item_digest(self))
        else:
            _digest("portfolio evidence work item_digest", self.item_digest)
            if _item_digest(self) != self.item_digest:
                raise ArgumentError("portfolio evidence work item_digest does not match its contents")

    @property
    def computed_digest(self) -> str:
        return _item_digest(self)

    def to_dict(self) -> dict[str, Any]:
        return {
            **_item_descriptor(self),
            "item_digest": self.item_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    @classmethod
    def from_dict(cls, value: Any) -> "AutonomousWorkflowPortfolioEvidenceWorkItem":
        if not isinstance(value, Mapping):
            raise ArgumentError("portfolio evidence work item must be an object")
        allowed = {
            "schema",
            "work_id",
            "job_id",
            "item_id",
            "domain",
            "wave_index",
            "dependency_item_ids",
            "provider_status",
            "portfolio_plan_digest",
            "admission_digest",
            "provider_execution_digest",
            "evidence_plan_digest",
            "request_digest",
            "checkpoint_digest",
            "max_attempts",
            "attempts",
            "status",
            "available_at",
            "lease_owner",
            "lease_until",
            "result_digest",
            "failure_class",
            "last_error_class",
            "created_at",
            "updated_at",
            "item_digest",
            "retention",
            "secret_material",
        }
        if set(value) != allowed:
            raise ArgumentError("portfolio evidence work item contains unsupported fields")
        if value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA:
            raise ArgumentError("portfolio evidence work item schema is invalid")
        if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("portfolio evidence work item retention contract is invalid")
        observed_item_digest = value.get("item_digest")
        item = cls(
            work_id=value.get("work_id"),
            job_id=value.get("job_id"),
            item_id=value.get("item_id"),
            domain=value.get("domain"),
            wave_index=value.get("wave_index"),
            dependency_item_ids=tuple(
                _sequence("portfolio evidence work dependency_item_ids", value.get("dependency_item_ids"), MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS)
            ),
            provider_status=value.get("provider_status"),
            portfolio_plan_digest=value.get("portfolio_plan_digest"),
            admission_digest=value.get("admission_digest"),
            provider_execution_digest=value.get("provider_execution_digest"),
            evidence_plan_digest=value.get("evidence_plan_digest"),
            request_digest=value.get("request_digest"),
            checkpoint_digest=value.get("checkpoint_digest"),
            max_attempts=value.get("max_attempts"),
            attempts=value.get("attempts"),
            status=value.get("status"),
            available_at=value.get("available_at"),
            lease_owner=value.get("lease_owner"),
            lease_until=value.get("lease_until"),
            result_digest=value.get("result_digest"),
            failure_class=value.get("failure_class"),
            last_error_class=value.get("last_error_class"),
            created_at=value.get("created_at"),
            updated_at=value.get("updated_at"),
            item_digest=observed_item_digest,
        )
        if item.item_digest != observed_item_digest:
            raise ArgumentError("portfolio evidence work item digest is invalid")
        return item


def validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
    value: Any,
    *,
    max_items: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
) -> dict[str, Any]:
    """Validate and normalize a metadata-only queue snapshot."""

    _bounded_integer(
        "portfolio evidence work snapshot max_items",
        max_items,
        1,
        MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
    )
    if not isinstance(value, Mapping):
        raise ArgumentError("portfolio evidence work queue snapshot must be an object")
    allowed = {"schema", "items", "retention", "secret_material", "snapshot_digest"}
    if set(value) != allowed:
        raise ArgumentError("portfolio evidence work queue snapshot contains unsupported fields")
    if value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA:
        raise ArgumentError("portfolio evidence work queue snapshot schema is invalid")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        raise ArgumentError("portfolio evidence work queue snapshot retention contract is invalid")
    raw_items = _sequence("portfolio evidence work queue snapshot items", value.get("items"), max_items)
    items = tuple(AutonomousWorkflowPortfolioEvidenceWorkItem.from_dict(item) for item in raw_items)
    if len({item.work_id for item in items}) != len(items):
        raise ArgumentError("portfolio evidence work queue snapshot contains duplicate work ids")
    _digest("portfolio evidence work snapshot_digest", value.get("snapshot_digest"))
    descriptor = {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
        "items": [item.to_dict() for item in items],
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    if content_digest(descriptor) != value.get("snapshot_digest"):
        raise ArgumentError("portfolio evidence work queue snapshot digest is invalid")
    normalized = {**descriptor, "snapshot_digest": value.get("snapshot_digest")}
    if len(canonical_json(normalized).encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES:
        raise ArgumentError("portfolio evidence work queue snapshot exceeds its byte bound")
    return normalized


class InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue:
    """Thread-safe dependency-aware queue with lease and retry fencing."""

    def __init__(self, *, max_items: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS) -> None:
        self.max_items = _bounded_integer(
            "portfolio evidence work queue max_items",
            max_items,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        self._items: dict[str, AutonomousWorkflowPortfolioEvidenceWorkItem] = {}
        self._lock = threading.RLock()

    @staticmethod
    def _refresh(
        item: AutonomousWorkflowPortfolioEvidenceWorkItem,
        now: int,
        **updates: Any,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return replace(item, **updates, updated_at=now, item_digest="")

    def admit(
        self,
        *,
        work_id: str,
        job_id: str,
        item_id: str,
        domain: str,
        wave_index: int,
        dependency_item_ids: Sequence[str] = (),
        provider_status: str,
        portfolio_plan_digest: str,
        admission_digest: str | None = None,
        provider_execution_digest: str,
        evidence_plan_digest: str,
        request_digest: str,
        checkpoint_digest: str | None = None,
        max_attempts: int = 3,
        available_at: int | None = None,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        job_id = _identifier("portfolio evidence work job_id", job_id)
        item_id = _identifier("portfolio evidence work item_id", item_id)
        domain = _domain("portfolio evidence work domain", domain)
        wave_index = _bounded_integer(
            "portfolio evidence work wave_index",
            wave_index,
            0,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        dependencies = tuple(
            sorted(
                _identifier_sequence(
                    "portfolio evidence work dependency_item_ids",
                    dependency_item_ids,
                    MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
                )
            )
        )
        if provider_status not in _PROVIDER_STATUSES:
            raise ArgumentError("portfolio evidence work provider_status is invalid")
        _digest("portfolio evidence work portfolio_plan_digest", portfolio_plan_digest)
        _digest("portfolio evidence work admission_digest", admission_digest, allow_none=True)
        _digest("portfolio evidence work provider_execution_digest", provider_execution_digest)
        _digest("portfolio evidence work evidence_plan_digest", evidence_plan_digest)
        _digest("portfolio evidence work request_digest", request_digest)
        _digest("portfolio evidence work checkpoint_digest", checkpoint_digest, allow_none=True)
        max_attempts = _bounded_integer(
            "portfolio evidence work max_attempts",
            max_attempts,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS,
        )
        current = _now_ms(now)
        available = current if available_at is None else _timestamp(
            "portfolio evidence work available_at", available_at
        )
        with self._lock:
            existing = self._items.get(work_id)
            if existing is not None:
                identity = (
                    existing.job_id == job_id
                    and existing.item_id == item_id
                    and existing.domain == domain
                    and existing.wave_index == wave_index
                    and existing.dependency_item_ids == dependencies
                    and existing.provider_status == provider_status
                    and existing.portfolio_plan_digest == portfolio_plan_digest
                    and existing.admission_digest == admission_digest
                    and existing.provider_execution_digest == provider_execution_digest
                    and existing.evidence_plan_digest == evidence_plan_digest
                    and existing.request_digest == request_digest
                    and existing.max_attempts == max_attempts
                )
                if not identity:
                    raise ArgumentError("portfolio evidence work identity conflicts with an existing item")
                return existing
            if len(self._items) >= self.max_items:
                raise ArgumentError("portfolio evidence work queue is full")
            item = AutonomousWorkflowPortfolioEvidenceWorkItem(
                work_id=work_id,
                job_id=job_id,
                item_id=item_id,
                domain=domain,
                wave_index=wave_index,
                dependency_item_ids=dependencies,
                provider_status=provider_status,
                portfolio_plan_digest=portfolio_plan_digest,
                admission_digest=admission_digest,
                provider_execution_digest=provider_execution_digest,
                evidence_plan_digest=evidence_plan_digest,
                request_digest=request_digest,
                checkpoint_digest=checkpoint_digest,
                max_attempts=max_attempts,
                attempts=0,
                status="queued",
                available_at=available,
                lease_owner=None,
                lease_until=None,
                result_digest=None,
                failure_class=None,
                last_error_class=None,
                created_at=current,
                updated_at=current,
            )
            self._items[work_id] = item
            return item

    def get(self, work_id: str) -> AutonomousWorkflowPortfolioEvidenceWorkItem | None:
        work_id = _identifier("portfolio evidence work_id", work_id)
        with self._lock:
            return self._items.get(work_id)

    def dependency_statuses(
        self,
        item: AutonomousWorkflowPortfolioEvidenceWorkItem,
    ) -> dict[str, str]:
        if not isinstance(item, AutonomousWorkflowPortfolioEvidenceWorkItem):
            raise ArgumentError("portfolio evidence dependency status requires a typed item")
        with self._lock:
            return {
                dependency: (
                    self._items[dependency].status
                    if dependency in self._items
                    else "missing"
                )
                for dependency in item.dependency_item_ids
            }

    def _dependency_ready(self, item: AutonomousWorkflowPortfolioEvidenceWorkItem) -> bool:
        return all(
            self._items.get(dependency) is not None
            and self._items[dependency].status == "completed"
            for dependency in item.dependency_item_ids
        )

    def _dependency_failed(self, item: AutonomousWorkflowPortfolioEvidenceWorkItem) -> bool:
        return any(
            self._items.get(dependency) is not None
            and self._items[dependency].status
            in {"failed", "reconciliation_required", "cancelled"}
            for dependency in item.dependency_item_ids
        )

    def pending(
        self,
        *,
        limit: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        now: int | None = None,
    ) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        current = _now_ms(now)
        limit = _bounded_integer(
            "portfolio evidence work pending limit",
            limit,
            1,
            min(MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, self.max_items),
        )
        with self._lock:
            values = [
                item
                for item in self._items.values()
                if item.status == "queued"
                and item.available_at <= current
                and item.attempts < item.max_attempts
                and (self._dependency_ready(item) or self._dependency_failed(item))
            ]
        return tuple(
            sorted(
                values,
                key=lambda item: (
                    item.wave_index,
                    item.available_at,
                    item.created_at,
                    item.work_id,
                ),
            )[:limit]
        )

    def reclaim_expired(
        self,
        *,
        limit: int | None = None,
        now: int | None = None,
    ) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        current = _now_ms(now)
        bounded_limit = self.max_items if limit is None else _bounded_integer(
            "portfolio evidence work reclaim limit",
            limit,
            1,
            min(MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, self.max_items),
        )
        with self._lock:
            expired = sorted(
                (
                    item
                    for item in self._items.values()
                    if item.status == "leased"
                    and item.lease_until is not None
                    and item.lease_until <= current
                ),
                key=lambda item: (item.lease_until or 0, item.work_id),
            )[:bounded_limit]
            result: list[AutonomousWorkflowPortfolioEvidenceWorkItem] = []
            for item in expired:
                next_item = self._refresh(
                    item,
                    current,
                    status="reconciliation_required",
                    lease_owner=None,
                    lease_until=None,
                    failure_class="lease_expired",
                    last_error_class="lease_expired",
                )
                self._items[item.work_id] = next_item
                result.append(next_item)
            return tuple(result)

    def claim(
        self,
        work_id: str,
        worker_id: str,
        *,
        lease_ms: int = 30_000,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem | None:
        work_id = _identifier("portfolio evidence work_id", work_id)
        worker_id = _identifier("portfolio evidence worker_id", worker_id)
        lease_ms = _bounded_integer(
            "portfolio evidence work lease_ms",
            lease_ms,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS,
        )
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {
                "completed",
                "failed",
                "awaiting_evaluation",
                "reconciliation_required",
                "cancelled",
            }:
                return None
            if item.status == "leased":
                if item.lease_until is not None and item.lease_until > current:
                    return None
                self._items[work_id] = self._refresh(
                    item,
                    current,
                    status="reconciliation_required",
                    lease_owner=None,
                    lease_until=None,
                    failure_class="lease_expired",
                    last_error_class="lease_expired",
                )
                return None
            if item.provider_status != "succeeded":
                self._items[work_id] = self._refresh(
                    item,
                    current,
                    status="reconciliation_required",
                    failure_class="provider_execution_not_succeeded",
                    last_error_class="provider_execution_not_succeeded",
                )
                return None
            if self._dependency_failed(item):
                self._items[work_id] = self._refresh(
                    item,
                    current,
                    status="reconciliation_required",
                    failure_class="dependency_failed",
                    last_error_class="dependency_failed",
                )
                return None
            if (
                not self._dependency_ready(item)
                or item.available_at > current
                or item.attempts >= item.max_attempts
            ):
                return None
            claimed = self._refresh(
                item,
                current,
                status="leased",
                attempts=item.attempts + 1,
                lease_owner=worker_id,
                lease_until=current + lease_ms,
                failure_class=None,
                last_error_class=None,
            )
            self._items[work_id] = claimed
            return claimed

    def renew(
        self,
        work_id: str,
        worker_id: str,
        *,
        lease_ms: int = 30_000,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        worker_id = _identifier("portfolio evidence worker_id", worker_id)
        lease_ms = _bounded_integer(
            "portfolio evidence work lease_ms",
            lease_ms,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS,
        )
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if (
                item is None
                or item.status != "leased"
                or item.lease_owner != worker_id
                or item.lease_until is None
                or item.lease_until <= current
            ):
                raise ArgumentError("portfolio evidence work lease cannot be renewed by this worker")
            renewed = self._refresh(item, current, lease_until=current + lease_ms)
            self._items[work_id] = renewed
            return renewed

    def complete(
        self,
        work_id: str,
        worker_id: str,
        *,
        status: str,
        result_digest: str,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        worker_id = _identifier("portfolio evidence worker_id", worker_id)
        if status not in {"completed", "awaiting_evaluation"}:
            raise ArgumentError("portfolio evidence work completion status is invalid")
        _digest("portfolio evidence work result_digest", result_digest)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if (
                item is None
                or item.status != "leased"
                or item.lease_owner != worker_id
                or item.lease_until is None
                or item.lease_until <= current
            ):
                raise ArgumentError("portfolio evidence work completion is fenced by an expired or foreign lease")
            pending = status == "awaiting_evaluation"
            finished = self._refresh(
                item,
                current,
                status=status,
                lease_owner=None,
                lease_until=None,
                result_digest=result_digest,
                failure_class="evaluator_pending" if pending else None,
                last_error_class="evaluator_pending" if pending else None,
            )
            self._items[work_id] = finished
            return finished

    def fail(
        self,
        work_id: str,
        worker_id: str,
        *,
        error_class: str,
        retryable: bool,
        result_digest: str | None = None,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        worker_id = _identifier("portfolio evidence worker_id", worker_id)
        error_class = _failure("portfolio evidence work failure", error_class)  # type: ignore[assignment]
        if not isinstance(retryable, bool):
            raise ArgumentError("portfolio evidence work retryable must be boolean")
        _digest("portfolio evidence work failure result_digest", result_digest, allow_none=True)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if (
                item is None
                or item.status != "leased"
                or item.lease_owner != worker_id
                or item.lease_until is None
                or item.lease_until <= current
            ):
                raise ArgumentError("portfolio evidence work failure is fenced by an expired or foreign lease")
            can_retry = retryable and item.attempts < item.max_attempts
            delay = min(3_600_000, 1_000 * (2 ** max(0, item.attempts - 1)))
            failed = self._refresh(
                item,
                current,
                status="queued" if can_retry else "failed",
                available_at=current + delay if can_retry else item.available_at,
                lease_owner=None,
                lease_until=None,
                result_digest=item.result_digest if result_digest is None else result_digest,
                failure_class=None if can_retry else error_class,
                last_error_class=error_class,
            )
            self._items[work_id] = failed
            return failed

    def reconcile(
        self,
        work_id: str,
        worker_id: str,
        *,
        error_class: str = "rehydration_missing",
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        worker_id = _identifier("portfolio evidence worker_id", worker_id)
        error_class = _failure(
            "portfolio evidence work reconciliation failure", error_class
        )  # type: ignore[assignment]
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if (
                item is None
                or item.status != "leased"
                or item.lease_owner != worker_id
                or item.lease_until is None
                or item.lease_until <= current
            ):
                raise ArgumentError("portfolio evidence work reconciliation is fenced by an expired or foreign lease")
            reconciled = self._refresh(
                item,
                current,
                status="reconciliation_required",
                lease_owner=None,
                lease_until=None,
                failure_class=error_class,
                last_error_class=error_class,
            )
            self._items[work_id] = reconciled
            return reconciled

    def requeue(
        self,
        work_id: str,
        *,
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status not in {"awaiting_evaluation", "reconciliation_required"}:
                raise ArgumentError("portfolio evidence work is not awaiting explicit requeue")
            if item.attempts >= item.max_attempts:
                raise ArgumentError("portfolio evidence work has exhausted its attempts")
            queued = self._refresh(
                item,
                current,
                status="queued",
                available_at=current,
                failure_class=None,
                last_error_class=item.last_error_class,
            )
            self._items[work_id] = queued
            return queued

    def cancel(
        self,
        work_id: str,
        *,
        error_class: str = "unknown",
        now: int | None = None,
    ) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        work_id = _identifier("portfolio evidence work_id", work_id)
        error_class = _failure("portfolio evidence cancellation failure", error_class)  # type: ignore[assignment]
        current = _now_ms(now)
        with self._lock:
            item = self._items.get(work_id)
            if item is None or item.status in {
                "completed",
                "failed",
                "awaiting_evaluation",
                "reconciliation_required",
                "cancelled",
            }:
                raise ArgumentError("portfolio evidence work cannot be cancelled in its current state")
            cancelled = self._refresh(
                item,
                current,
                status="cancelled",
                lease_owner=None,
                lease_until=None,
                failure_class=error_class,
                last_error_class=error_class,
            )
            self._items[work_id] = cancelled
            return cancelled

    def bind_checkpoint_digest(
        self,
        job_id: str,
        checkpoint_digest: str | None,
        *,
        now: int | None = None,
    ) -> int:
        job_id = _identifier("portfolio evidence checkpoint job_id", job_id)
        _digest("portfolio evidence checkpoint digest", checkpoint_digest, allow_none=True)
        current = _now_ms(now)
        count = 0
        with self._lock:
            for work_id, item in tuple(self._items.items()):
                if item.job_id != job_id:
                    continue
                self._items[work_id] = self._refresh(
                    item,
                    current,
                    checkpoint_digest=checkpoint_digest,
                )
                count += 1
        return count

    def rows(self) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        with self._lock:
            return tuple(
                sorted(
                    self._items.values(),
                    key=lambda item: (
                        item.wave_index,
                        item.created_at,
                        item.work_id,
                    ),
                )
            )

    def verify_integrity(self) -> dict[str, Any]:
        for item in self.rows():
            if item.computed_digest != item.item_digest:
                raise ArgumentError("portfolio evidence work item digest is invalid")
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
            "verified": True,
            "items": len(self._items),
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def snapshot(self) -> dict[str, Any]:
        self.verify_integrity()
        descriptor = {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
            "items": [item.to_dict() for item in self.rows()],
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES:
            raise ArgumentError("portfolio evidence work queue snapshot exceeds its byte bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
            snapshot,
            max_items=self.max_items,
        )
        items = {
            item.work_id: item
            for item in (
                AutonomousWorkflowPortfolioEvidenceWorkItem.from_dict(raw)
                for raw in normalized["items"]
            )
        }
        with self._lock:
            self._items = items


class AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore(
    AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore,
    Protocol,
):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence:
    """Validated in-memory persistence with a real compare-and-swap fence."""

    def __init__(self, initial: Mapping[str, Any] | None = None) -> None:
        self._snapshot: dict[str, Any] | None = None
        self._lock = threading.RLock()
        if initial is not None:
            self.write(initial)

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            return None if self._snapshot is None else json.loads(canonical_json(self._snapshot))

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(snapshot)
        with self._lock:
            self._snapshot = normalized

    def write_if_unchanged(
        self,
        expected_snapshot_digest: str | None,
        snapshot: Mapping[str, Any],
    ) -> bool:
        _digest("portfolio evidence expected snapshot digest", expected_snapshot_digest, allow_none=True)
        normalized = validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(snapshot)
        with self._lock:
            current = None if self._snapshot is None else self._snapshot["snapshot_digest"]
            if current != expected_snapshot_digest:
                return False
            self._snapshot = normalized
            return True


class JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence:
    """Strict canonical JSON persistence for queue snapshots."""

    def __init__(
        self,
        store: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore,
        *,
        max_items: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        max_bytes: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES,
    ) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("portfolio evidence work JSON persistence requires a text store")
        self.max_items = _bounded_integer(
            "portfolio evidence work JSON max_items",
            max_items,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        self.max_bytes = _bounded_integer(
            "portfolio evidence work JSON max_bytes",
            max_bytes,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES,
        )
        self.store = store

    def _encode(self, snapshot: Mapping[str, Any]) -> str:
        normalized = validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
            snapshot,
            max_items=self.max_items,
        )
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("portfolio evidence work JSON snapshot exceeds its byte bound")
        return encoded

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("portfolio evidence work JSON snapshot exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("portfolio evidence work JSON snapshot is invalid") from error
        if encoded != canonical_json(value):
            raise ArgumentError("portfolio evidence work JSON snapshot is not canonical")
        validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
            value,
            max_items=self.max_items,
        )
        return value

    def write(self, snapshot: Mapping[str, Any]) -> None:
        self.store.write(self._encode(snapshot))


class TransactionalJsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence(
    JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
):
    """Canonical JSON queue persistence with atomic compare-and-swap."""

    def __init__(
        self,
        store: TransactionalAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore,
        **kwargs: Any,
    ) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional portfolio evidence work persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(
        self,
        expected_snapshot_digest: str | None,
        snapshot: Mapping[str, Any],
    ) -> bool:
        _digest("portfolio evidence expected snapshot digest", expected_snapshot_digest, allow_none=True)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, self._encode(snapshot)))


class SQLiteAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence:
    """Transactional SQLite snapshot adapter for local services and test workers."""

    def __init__(
        self,
        path: str | Path,
        *,
        max_items: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        busy_timeout_ms: int = 5_000,
    ) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            raise ArgumentError("portfolio evidence work SQLite path must be non-empty")
        self.path = str(path)
        self.max_items = _bounded_integer(
            "portfolio evidence work SQLite max_items",
            max_items,
            1,
            MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        )
        self.busy_timeout_ms = _bounded_integer(
            "portfolio evidence work SQLite busy_timeout_ms",
            busy_timeout_ms,
            1,
            120_000,
        )
        self._lock = threading.RLock()
        if self.path != ":memory:":
            Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        try:
            self._connection = sqlite3.connect(
                self.path,
                isolation_level=None,
                check_same_thread=False,
            )
            self._connection.row_factory = sqlite3.Row
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.execute(f"PRAGMA busy_timeout={self.busy_timeout_ms}")
            self._connection.execute(
                """
                CREATE TABLE IF NOT EXISTS autonomous_portfolio_evidence_work_queue_snapshots (
                    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                    persistence_schema TEXT NOT NULL,
                    schema TEXT NOT NULL,
                    snapshot_json TEXT NOT NULL,
                    snapshot_digest TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                """
            )
        except sqlite3.Error as error:
            raise ArgumentError("could not initialize portfolio evidence work SQLite persistence") from error

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "SQLiteAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            try:
                row = self._connection.execute(
                    "SELECT persistence_schema, schema, snapshot_json, snapshot_digest FROM autonomous_portfolio_evidence_work_queue_snapshots WHERE singleton = 1"
                ).fetchone()
            except sqlite3.Error as error:
                raise ArgumentError("could not read portfolio evidence work SQLite persistence") from error
        if row is None:
            return None
        if (
            row["persistence_schema"]
            != AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SQLITE_SCHEMA
            or row["schema"] != AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA
        ):
            raise ArgumentError("portfolio evidence work SQLite snapshot schema is invalid")
        try:
            snapshot = json.loads(str(row["snapshot_json"]))
        except (TypeError, ValueError) as error:
            raise ArgumentError("portfolio evidence work SQLite snapshot JSON is invalid") from error
        if not isinstance(snapshot, Mapping) or snapshot.get("snapshot_digest") != row["snapshot_digest"]:
            raise ArgumentError("portfolio evidence work SQLite snapshot digest is invalid")
        return validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
            snapshot,
            max_items=self.max_items,
        )

    def _normalized(self, snapshot: Mapping[str, Any]) -> tuple[dict[str, Any], str]:
        value = validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(
            snapshot,
            max_items=self.max_items,
        )
        return value, canonical_json(value)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized, encoded = self._normalized(snapshot)
        current = _now_ms(None)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                self._connection.execute(
                    """
                    INSERT INTO autonomous_portfolio_evidence_work_queue_snapshots
                        (singleton, persistence_schema, schema, snapshot_json, snapshot_digest, updated_at)
                    VALUES (1, ?, ?, ?, ?, ?)
                    ON CONFLICT(singleton) DO UPDATE SET
                        persistence_schema = excluded.persistence_schema,
                        schema = excluded.schema,
                        snapshot_json = excluded.snapshot_json,
                        snapshot_digest = excluded.snapshot_digest,
                        updated_at = excluded.updated_at
                    """,
                    (
                        AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SQLITE_SCHEMA,
                        AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
                        encoded,
                        normalized["snapshot_digest"],
                        current,
                    ),
                )
                self._connection.execute("COMMIT")
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not write portfolio evidence work SQLite persistence") from error

    def write_if_unchanged(
        self,
        expected_snapshot_digest: str | None,
        snapshot: Mapping[str, Any],
    ) -> bool:
        _digest("portfolio evidence expected snapshot digest", expected_snapshot_digest, allow_none=True)
        normalized, encoded = self._normalized(snapshot)
        current = _now_ms(None)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                row = self._connection.execute(
                    "SELECT snapshot_digest FROM autonomous_portfolio_evidence_work_queue_snapshots WHERE singleton = 1"
                ).fetchone()
                observed = None if row is None else row["snapshot_digest"]
                if observed != expected_snapshot_digest:
                    self._connection.execute("ROLLBACK")
                    return False
                self._connection.execute(
                    """
                    INSERT INTO autonomous_portfolio_evidence_work_queue_snapshots
                        (singleton, persistence_schema, schema, snapshot_json, snapshot_digest, updated_at)
                    VALUES (1, ?, ?, ?, ?, ?)
                    ON CONFLICT(singleton) DO UPDATE SET
                        persistence_schema = excluded.persistence_schema,
                        schema = excluded.schema,
                        snapshot_json = excluded.snapshot_json,
                        snapshot_digest = excluded.snapshot_digest,
                        updated_at = excluded.updated_at
                    """,
                    (
                        AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SQLITE_SCHEMA,
                        AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
                        encoded,
                        normalized["snapshot_digest"],
                        current,
                    ),
                )
                self._connection.execute("COMMIT")
                return True
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not compare-and-swap portfolio evidence work SQLite persistence") from error


class AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator:
    """Serialize local queue restores and flushes; CAS is used when supplied."""

    def __init__(self, queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue, persistence: Any) -> None:
        if not isinstance(queue, InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue):
            raise ArgumentError("portfolio evidence work persistence requires a typed queue")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("portfolio evidence work persistence adapter is malformed")
        self.queue = queue
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
                return {"status": "empty", "snapshot_digest": None, "items": 0}
            self.queue.restore(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return {
                "status": "restored",
                "snapshot_digest": self._expected_snapshot_digest,
                "items": len(snapshot["items"]),
            }

    def flush(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.queue.snapshot()
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("portfolio evidence work persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return snapshot


class AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator:
    """Reload and CAS-commit every queue transition for shared-worker deployments."""

    def __init__(
        self,
        queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
        persistence: Any,
        *,
        max_conflict_retries: int = 4,
    ) -> None:
        if not isinstance(queue, InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue):
            raise ArgumentError("portfolio evidence atomic coordinator requires a typed queue")
        if not callable(getattr(persistence, "read", None)) or not callable(
            getattr(persistence, "write_if_unchanged", None)
        ):
            raise ArgumentError("portfolio evidence atomic coordinator requires compare-and-swap persistence")
        self.queue = queue
        self.persistence = persistence
        self.max_conflict_retries = _bounded_integer(
            "portfolio evidence atomic coordinator max_conflict_retries",
            max_conflict_retries,
            1,
            16,
        )
        self._lock = threading.RLock()

    def _empty_snapshot(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
            "items": [],
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
            "snapshot_digest": content_digest(
                {
                    "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
                    "items": [],
                    "retention": _RETENTION,
                    "secret_material": _SECRET_MATERIAL,
                }
            ),
        }

    def _load_latest(self) -> str | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            self.queue.restore(self._empty_snapshot())
            return None
        self.queue.restore(snapshot)
        return snapshot["snapshot_digest"]

    def restore(self) -> dict[str, Any]:
        with self._lock:
            digest = self._load_latest()
            return {
                "status": "empty" if digest is None else "restored",
                "snapshot_digest": digest,
                "items": len(self.queue.rows()),
            }

    def snapshot(self) -> dict[str, Any]:
        with self._lock:
            self._load_latest()
            return self.queue.snapshot()

    def get(self, work_id: str) -> AutonomousWorkflowPortfolioEvidenceWorkItem | None:
        with self._lock:
            self._load_latest()
            return self.queue.get(work_id)

    def pending(self, *, limit: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, now: int | None = None) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        with self._lock:
            self._load_latest()
            return self.queue.pending(limit=limit, now=now)

    def rows(self) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        with self._lock:
            self._load_latest()
            return self.queue.rows()

    def _transact(self, operation: Callable[[InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue], Any]) -> Any:
        with self._lock:
            for _ in range(self.max_conflict_retries):
                expected = self._load_latest()
                before = self.queue.snapshot()
                result = operation(self.queue)
                after = self.queue.snapshot()
                if after["snapshot_digest"] == before["snapshot_digest"]:
                    return result
                if self.persistence.write_if_unchanged(expected, after):
                    return result
            raise ArgumentError("portfolio evidence atomic transition conflicted repeatedly; reload before continuing")

    def admit(self, **kwargs: Any) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.admit(**kwargs))

    def claim(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem | None:
        return self._transact(lambda queue: queue.claim(work_id, worker_id, lease_ms=lease_ms, now=now))

    def renew(self, work_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.renew(work_id, worker_id, lease_ms=lease_ms, now=now))

    def complete(self, work_id: str, worker_id: str, *, status: str, result_digest: str, now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.complete(work_id, worker_id, status=status, result_digest=result_digest, now=now))

    def fail(self, work_id: str, worker_id: str, *, error_class: str, retryable: bool, result_digest: str | None = None, now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.fail(work_id, worker_id, error_class=error_class, retryable=retryable, result_digest=result_digest, now=now))

    def reconcile(self, work_id: str, worker_id: str, *, error_class: str = "rehydration_missing", now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.reconcile(work_id, worker_id, error_class=error_class, now=now))

    def requeue(self, work_id: str, *, now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.requeue(work_id, now=now))

    def cancel(self, work_id: str, *, error_class: str = "unknown", now: int | None = None) -> AutonomousWorkflowPortfolioEvidenceWorkItem:
        return self._transact(lambda queue: queue.cancel(work_id, error_class=error_class, now=now))

    def bind_checkpoint_digest(self, job_id: str, checkpoint_digest: str | None, *, now: int | None = None) -> int:
        return self._transact(lambda queue: queue.bind_checkpoint_digest(job_id, checkpoint_digest, now=now))

    def reclaim_expired(self, *, limit: int | None = None, now: int | None = None) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
        return self._transact(lambda queue: queue.reclaim_expired(limit=limit, now=now))


def admit_autonomous_workflow_portfolio_evidence_work_items(
    queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
    *,
    job_id: str,
    execution: AutonomousWorkflowPortfolioExecutionResult,
    evidence_plan_digest: str,
    item_request_digests: Sequence[str],
    checkpoint_digest: str | None = None,
    max_attempts: int = 3,
    now: int | None = None,
) -> tuple[AutonomousWorkflowPortfolioEvidenceWorkItem, ...]:
    """Admit every provider item under one exact portfolio/evidence identity."""

    if not isinstance(queue, InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue):
        raise ArgumentError("portfolio evidence work admission requires a typed queue")
    if not isinstance(execution, AutonomousWorkflowPortfolioExecutionResult):
        raise ArgumentError("portfolio evidence work admission requires a typed provider execution")
    job_id = _identifier("portfolio evidence work admission job_id", job_id)
    _digest("portfolio evidence work admission evidence_plan_digest", evidence_plan_digest)
    _digest("portfolio evidence work admission checkpoint_digest", checkpoint_digest, allow_none=True)
    request_digests = _sequence(
        "portfolio evidence work admission request digests",
        item_request_digests,
        MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
    )
    if len(request_digests) != len(execution.plan.items):
        raise ArgumentError("portfolio evidence work admission request digests must align with the reviewed plan")
    normalized_requests = tuple(
        _digest(f"portfolio evidence work admission request_digest[{index}]", value)
        for index, value in enumerate(request_digests)
    )
    waves = {
        item_id: index
        for index, wave in enumerate(execution.plan.dependency_graph.waves)
        for item_id in wave
    }
    providers = {item.item_id: item for item in execution.items}
    provider_execution_digest = autonomous_workflow_portfolio_provider_execution_digest(execution)
    admitted: list[AutonomousWorkflowPortfolioEvidenceWorkItem] = []
    for index, plan_item in enumerate(execution.plan.items):
        provider = providers.get(plan_item.item_id)
        admitted.append(
            queue.admit(
                work_id=f"{job_id}:{plan_item.item_id}",
                job_id=job_id,
                item_id=plan_item.item_id,
                domain=plan_item.domain,
                wave_index=waves.get(plan_item.item_id, 0),
                dependency_item_ids=tuple(
                    f"{job_id}:{dependency}" for dependency in plan_item.depends_on
                ),
                provider_status="omitted" if provider is None else provider.status,
                portfolio_plan_digest=execution.plan.portfolio_digest,
                admission_digest=execution.admission_digest,
                provider_execution_digest=provider_execution_digest,
                evidence_plan_digest=evidence_plan_digest,
                request_digest=normalized_requests[index],  # type: ignore[arg-type]
                checkpoint_digest=checkpoint_digest,
                max_attempts=max_attempts,
                now=now,
            )
        )
    return tuple(admitted)


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceWorkExecution:
    status: str
    result_digest: str | None = None
    error_class: str | None = None
    retryable: bool = False

    def __post_init__(self) -> None:
        if self.status not in {"completed", "awaiting_evaluation", "failed", "reconciliation_required"}:
            raise ArgumentError("portfolio evidence work execution status is invalid")
        _digest("portfolio evidence work execution result_digest", self.result_digest, allow_none=True)
        _failure("portfolio evidence work execution error_class", self.error_class, allow_none=True)
        if not isinstance(self.retryable, bool):
            raise ArgumentError("portfolio evidence work execution retryable must be boolean")
        if self.status in {"completed", "awaiting_evaluation"} and self.result_digest is None:
            raise ArgumentError("settled portfolio evidence work execution requires a result digest")

    @classmethod
    def from_value(cls, value: Any) -> "AutonomousWorkflowPortfolioEvidenceWorkExecution":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping):
            raise ArgumentError("portfolio evidence work executor result must be an object")
        return cls(
            status=value.get("status"),
            result_digest=value.get("result_digest", value.get("resultDigest")),
            error_class=value.get("error_class", value.get("errorClass")),
            retryable=value.get("retryable", False),
        )


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceWorkWorkerRow:
    work_id: str
    item_id: str
    domain: str
    outcome: str
    attempts: int
    result_digest: str | None
    error_class: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "work_id": self.work_id,
            "item_id": self.item_id,
            "domain": self.domain,
            "outcome": self.outcome,
            "attempts": self.attempts,
            "result_digest": self.result_digest,
            "error_class": self.error_class,
            "lease_retained": False,
        }


def _worker_result(
    worker_id: str,
    rows: Sequence[AutonomousWorkflowPortfolioEvidenceWorkWorkerRow],
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
        "worker_id": worker_id,
        "inspected": len(rows),
        "completed": sum(row.outcome == "completed" for row in rows),
        "awaiting_evaluation": sum(row.outcome == "awaiting_evaluation" for row in rows),
        "retried": sum(row.outcome == "retry_scheduled" for row in rows),
        "failed": sum(row.outcome == "failed" for row in rows),
        "reconciled": sum(row.outcome == "reconciliation_required" for row in rows),
        "leased_elsewhere": sum(row.outcome == "leased_elsewhere" for row in rows),
        "rows": [row.to_dict() for row in rows],
        "retention": _WORKER_RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }


def _run_work_queue_worker(
    queue: Any,
    execute: Callable[[AutonomousWorkflowPortfolioEvidenceWorkItem, Mapping[str, Any]], Any],
    *,
    worker_id: str,
    limit: int,
    lease_ms: int,
    now: int | None,
    aborted: Callable[[], bool] | None,
) -> dict[str, Any]:
    worker_id = _identifier("portfolio evidence work worker_id", worker_id)
    limit = _bounded_integer(
        "portfolio evidence work worker limit",
        limit,
        1,
        MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
    )
    lease_ms = _bounded_integer(
        "portfolio evidence work worker lease_ms",
        lease_ms,
        1,
        MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS,
    )
    if aborted is not None and not callable(aborted):
        raise ArgumentError("portfolio evidence work worker aborted callback must be callable")
    deterministic = now is not None
    current = _now_ms(now)
    rows: list[AutonomousWorkflowPortfolioEvidenceWorkWorkerRow] = []
    expired = queue.reclaim_expired(limit=limit, now=current)
    for item in expired:
        rows.append(
            AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                item.work_id,
                item.item_id,
                item.domain,
                "reconciliation_required",
                item.attempts,
                item.result_digest,
                item.failure_class,
            )
        )
    remaining = max(0, limit - len(rows))
    if remaining:
        active = tuple(
            item
            for item in queue.rows()
            if item.status == "leased"
            and item.lease_until is not None
            and item.lease_until > current
        )[:remaining]
        pending = queue.pending(
            limit=max(1, remaining - len(active)),
            now=current,
        )
        candidates = tuple((*active, *pending))[:remaining]
    else:
        candidates = ()
    for candidate in candidates:
        if aborted is not None and aborted():
            break
        claimed = queue.claim(
            candidate.work_id,
            worker_id,
            lease_ms=lease_ms,
            now=current,
        )
        if claimed is None:
            current_item = queue.get(candidate.work_id)
            rows.append(
                AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                    candidate.work_id,
                    candidate.item_id,
                    candidate.domain,
                    "reconciliation_required"
                    if current_item is not None
                    and current_item.status == "reconciliation_required"
                    else "leased_elsewhere",
                    candidate.attempts if current_item is None else current_item.attempts,
                    None if current_item is None else current_item.result_digest,
                    None if current_item is None else current_item.failure_class,
                )
            )
            continue
        try:
            finish_now = current if deterministic else None
            def renew(*, lease_ms: int = lease_ms, now: int | None = finish_now):
                return queue.renew(
                    claimed.work_id,
                    worker_id,
                    lease_ms=lease_ms,
                    now=now,
                )

            outcome = AutonomousWorkflowPortfolioEvidenceWorkExecution.from_value(
                execute(
                    claimed,
                    {
                        "renew": renew,
                    },
                )
            )
            if outcome.status in {"completed", "awaiting_evaluation"}:
                finished = queue.complete(
                    claimed.work_id,
                    worker_id,
                    status=outcome.status,
                    result_digest=outcome.result_digest,  # type: ignore[arg-type]
                    now=finish_now,
                )
                rows.append(
                    AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                        finished.work_id,
                        finished.item_id,
                        finished.domain,
                        outcome.status,
                        finished.attempts,
                        finished.result_digest,
                        finished.failure_class,
                    )
                )
            elif outcome.status == "reconciliation_required":
                reconciled = queue.reconcile(
                    claimed.work_id,
                    worker_id,
                    error_class=outcome.error_class or "rehydration_missing",
                    now=finish_now,
                )
                rows.append(
                    AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                        reconciled.work_id,
                        reconciled.item_id,
                        reconciled.domain,
                        "reconciliation_required",
                        reconciled.attempts,
                        reconciled.result_digest,
                        reconciled.failure_class,
                    )
                )
            else:
                failed = queue.fail(
                    claimed.work_id,
                    worker_id,
                    error_class=outcome.error_class or "executor_error",
                    retryable=outcome.retryable,
                    result_digest=outcome.result_digest,
                    now=finish_now,
                )
                rows.append(
                    AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                        failed.work_id,
                        failed.item_id,
                        failed.domain,
                        "retry_scheduled" if failed.status == "queued" else "failed",
                        failed.attempts,
                        failed.result_digest,
                        failed.last_error_class,
                    )
                )
        except Exception:
            try:
                failed = queue.fail(
                    claimed.work_id,
                    worker_id,
                    error_class="executor_error",
                    retryable=True,
                    now=current if deterministic else None,
                )
                rows.append(
                    AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                        failed.work_id,
                        failed.item_id,
                        failed.domain,
                        "retry_scheduled" if failed.status == "queued" else "failed",
                        failed.attempts,
                        failed.result_digest,
                        failed.last_error_class,
                    )
                )
            except Exception:
                current_item = queue.get(claimed.work_id)
                rows.append(
                    AutonomousWorkflowPortfolioEvidenceWorkWorkerRow(
                        claimed.work_id,
                        claimed.item_id,
                        claimed.domain,
                        "leased_elsewhere"
                        if current_item is None
                        or current_item.status == "leased"
                        else "reconciliation_required",
                        claimed.attempts if current_item is None else current_item.attempts,
                        None if current_item is None else current_item.result_digest,
                        "executor_error"
                        if current_item is None
                        else current_item.failure_class,
                    )
                )
    return _worker_result(worker_id, rows)


class AutonomousWorkflowPortfolioEvidenceWorkWorker:
    """Run caller-owned evidence work through a local queue without retaining values."""

    def __init__(
        self,
        queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
        execute: Callable[[AutonomousWorkflowPortfolioEvidenceWorkItem, Mapping[str, Any]], Any],
    ) -> None:
        if not isinstance(queue, InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue):
            raise ArgumentError("portfolio evidence work worker requires a typed queue")
        if not callable(execute):
            raise ArgumentError("portfolio evidence work worker requires an executor")
        self.queue = queue
        self.execute = execute

    def run(
        self,
        *,
        worker_id: str = "portfolio-evidence-worker",
        limit: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        lease_ms: int = 30_000,
        now: int | None = None,
        aborted: Callable[[], bool] | None = None,
    ) -> dict[str, Any]:
        return _run_work_queue_worker(
            self.queue,
            self.execute,
            worker_id=worker_id,
            limit=limit,
            lease_ms=lease_ms,
            now=now,
            aborted=aborted,
        )


class AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker:
    """Run evidence work through the CAS-backed coordinator."""

    def __init__(
        self,
        coordinator: AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator,
        execute: Callable[[AutonomousWorkflowPortfolioEvidenceWorkItem, Mapping[str, Any]], Any],
    ) -> None:
        if not isinstance(coordinator, AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator):
            raise ArgumentError("portfolio evidence atomic worker requires a CAS coordinator")
        if not callable(execute):
            raise ArgumentError("portfolio evidence atomic worker requires an executor")
        self.coordinator = coordinator
        self.execute = execute

    def run(
        self,
        *,
        worker_id: str = "portfolio-evidence-atomic-worker",
        limit: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
        lease_ms: int = 30_000,
        now: int | None = None,
        aborted: Callable[[], bool] | None = None,
    ) -> dict[str, Any]:
        return _run_work_queue_worker(
            self.coordinator,
            self.execute,
            worker_id=worker_id,
            limit=limit,
            lease_ms=lease_ms,
            now=now,
            aborted=aborted,
        )


__all__ = [
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SQLITE_SCHEMA",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES",
    "AutonomousWorkflowPortfolioEvidenceWorkItem",
    "AutonomousWorkflowPortfolioEvidenceWorkExecution",
    "AutonomousWorkflowPortfolioEvidenceWorkWorkerRow",
    "InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue",
    "AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore",
    "TransactionalAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore",
    "InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence",
    "JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence",
    "TransactionalJsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence",
    "SQLiteAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence",
    "AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator",
    "AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator",
    "AutonomousWorkflowPortfolioEvidenceWorkWorker",
    "AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker",
    "admit_autonomous_workflow_portfolio_evidence_work_items",
    "autonomous_workflow_portfolio_provider_execution_digest",
    "validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot",
]
