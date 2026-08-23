"""Restart-safe orchestration journal for autonomous brain learning jobs.

This store is intentionally not an execution engine and not a credential store. It persists only
the metadata needed to coordinate a caller-owned resolver: digests, domain labels, leases,
attempt counters, checkpoints, and recovery decisions. A resolver rehydrates the actual task,
prompt, plan, evaluator, workflow blueprint, and BYOK handles after a process restart. Workflow
workers can cooperatively release a bounded stage continuation back to the queue; if a lease
expires after the external-effect boundary, the job is quarantined for reconciliation instead of
being replayed.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence
import uuid

from .memory import (
    BrainMemoryError,
    _bounded_string,
    _canonical,
    _digest,
    _safe_value,
    _valid_digest,
)


JOB_SCHEMA = "bioprism-brain-job/0.1"
JOB_EVENT_SCHEMA = "bioprism-brain-job-event/0.1"
MAX_JOB_ID_BYTES = 256
MAX_JOB_LABEL_BYTES = 256
MAX_JOB_CHECKPOINT_BYTES = 64_000
MAX_JOB_REASON_BYTES = 2_048
MAX_JOB_INVENTORY = 256
JOB_STATES = frozenset(
    {
        "queued",
        "leased",
        "running",
        "waiting_approval",
        "succeeded",
        "failed",
        "dead_lettered",
        "cancelled",
        "reconciliation_required",
    }
)
JOB_BOUNDARIES = ("not_started", "preflight", "dispatched", "unknown")
JOB_RECONCILIATION_OUTCOMES = frozenset({"succeeded", "failed", "not_executed", "unknown"})
JOB_RECONCILIATION_SCHEMA = "bioprism-brain-job-reconciliation/0.1"
JOB_SNAPSHOT_SCHEMA = "bioprism-brain-job-snapshot/0.1"
MAX_JOB_EVENTS = 100_000
MAX_JOB_SNAPSHOT_JOBS = 8_192
MAX_JOB_SNAPSHOT_BYTES = 64_000_000
_BOUNDARY_ORDER = {value: index for index, value in enumerate(JOB_BOUNDARIES)}


class BrainJobError(RuntimeError):
    """A job submission, lease, checkpoint, or recovery operation was refused."""


class BrainJobSnapshotTextStore(Protocol):
    """Portable text persistence for metadata-only durable worker snapshots."""

    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalBrainJobSnapshotTextStore(BrainJobSnapshotTextStore, Protocol):
    """Text persistence with compare-and-swap fencing for competing workers."""

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


def _safe_job_value(value: Any) -> Any:
    try:
        return _safe_value(value)
    except BrainMemoryError as error:
        raise BrainJobError("job metadata contains forbidden or unsupported content") from error


def _job_text(name: str, value: Any, maximum: int = MAX_JOB_LABEL_BYTES) -> str:
    try:
        return _bounded_string(value, name=name, maximum=maximum)
    except BrainMemoryError as error:
        raise BrainJobError(str(error)) from error


def _reconciliation_payload(
    *,
    outcome: str,
    evidence_digest: str,
    evidence_kind: str,
    operator: str,
    reason: str,
    metadata: Mapping[str, Any] | None,
) -> dict[str, Any]:
    if not isinstance(outcome, str) or outcome not in JOB_RECONCILIATION_OUTCOMES:
        raise BrainJobError(f"unknown reconciliation outcome: {outcome}")
    if not _valid_digest(evidence_digest):
        raise BrainJobError("reconciliation evidence_digest must be a lowercase SHA-256 digest")
    evidence_kind = _job_text("reconciliation evidence_kind", evidence_kind, 128)
    operator = _job_text("reconciliation operator", operator, MAX_JOB_ID_BYTES)
    reason = _job_text("reconciliation reason", reason, MAX_JOB_REASON_BYTES)
    safe_metadata = _safe_job_value({} if metadata is None else metadata)
    if not isinstance(safe_metadata, Mapping):
        raise BrainJobError("reconciliation metadata must be a mapping")
    if outcome == "not_executed" and safe_metadata.get("effect_absent") is not True:
        raise BrainJobError("not_executed reconciliation requires metadata.effect_absent=True")
    payload = {
        "schema": JOB_RECONCILIATION_SCHEMA,
        "outcome": outcome,
        "evidence_digest": evidence_digest,
        "evidence_kind": evidence_kind,
        "operator": operator,
        "reason": reason,
        "metadata": dict(safe_metadata),
    }
    if len(_canonical(payload).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
        raise BrainJobError("reconciliation metadata exceeds the bounded size")
    payload["decision_digest"] = _digest(payload)
    return payload


@dataclass(frozen=True, slots=True)
class BrainJobRecord:
    """Public, spec-free view of a persisted learning job."""

    job_id: str
    idempotency_key: str
    spec_digest: str
    domain: str
    capability: str
    risk_class: str
    priority: int
    max_attempts: int
    state: str
    attempts: int
    lease_owner: str | None
    lease_expires_ns: int | None
    checkpoint: Mapping[str, Any]
    side_effect_boundary: str
    recovered_after_restart: bool
    reason: str | None
    created_ns: int
    updated_ns: int
    record_sequence: int
    record_digest: str

    @property
    def terminal(self) -> bool:
        return self.state in {"succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"}

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": JOB_SCHEMA,
            "job_id": self.job_id,
            "idempotency_key": self.idempotency_key,
            "spec_digest": self.spec_digest,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "priority": self.priority,
            "max_attempts": self.max_attempts,
            "state": self.state,
            "attempts": self.attempts,
            "lease_owner": self.lease_owner,
            "lease_expires_ns": self.lease_expires_ns,
            "checkpoint": dict(self.checkpoint),
            "side_effect_boundary": self.side_effect_boundary,
            "recovered_after_restart": self.recovered_after_restart,
            "reason": self.reason,
            "created_ns": self.created_ns,
            "updated_ns": self.updated_ns,
            "record_sequence": self.record_sequence,
            "record_digest": self.record_digest,
            "spec": "not_returned; caller resolver owns rehydration",
        }


@dataclass(frozen=True, slots=True)
class BrainJobEventReceipt:
    event_type: str
    job_id: str
    sequence: int
    event_digest: str
    head_digest: str
    idempotent: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": JOB_EVENT_SCHEMA,
            "event_type": self.event_type,
            "job_id": self.job_id,
            "sequence": self.sequence,
            "event_digest": self.event_digest,
            "head_digest": self.head_digest,
            "idempotent": self.idempotent,
            "retention": "metadata_only_hash_chained",
        }


@dataclass(frozen=True, slots=True)
class BrainJobEvent:
    """A verified, metadata-only event from the durable job journal."""

    sequence: int
    event_type: str
    job_id: str
    payload: Mapping[str, Any]
    previous_digest: str
    event_digest: str
    created_ns: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": JOB_EVENT_SCHEMA,
            "sequence": self.sequence,
            "event_type": self.event_type,
            "job_id": self.job_id,
            "payload": dict(self.payload),
            "previous_digest": self.previous_digest,
            "event_digest": self.event_digest,
            "created_ns": self.created_ns,
            "retention": "metadata_only_hash_chained",
        }


class BrainJobStore:
    """Bounded SQLite job journal with leases, idempotency, checkpoints, and safe recovery."""

    def __init__(
        self,
        path: str | Path,
        *,
        max_jobs: int = 1_024,
        max_bytes: int = 64_000_000,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            raise BrainJobError("job path must be non-empty")
        if not isinstance(max_jobs, int) or isinstance(max_jobs, bool) or max_jobs <= 0:
            raise BrainJobError("max_jobs must be positive")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes <= 0:
            raise BrainJobError("max_bytes must be positive")
        if not callable(clock):
            raise BrainJobError("job clock must be callable")
        self.path = str(path)
        self.max_jobs = max_jobs
        self.max_bytes = max_bytes
        self._clock = clock
        self._lock = threading.RLock()
        if self.path != ":memory:":
            Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        self._connection = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
        self._connection.row_factory = sqlite3.Row
        with self._lock:
            self._connection.execute("PRAGMA foreign_keys=ON")
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS brain_job_events (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_type TEXT NOT NULL,
                    job_id TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    previous_digest TEXT NOT NULL,
                    event_digest TEXT NOT NULL UNIQUE,
                    created_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS brain_jobs (
                    job_id TEXT PRIMARY KEY,
                    idempotency_key TEXT NOT NULL UNIQUE,
                    spec_digest TEXT NOT NULL,
                    domain TEXT NOT NULL,
                    capability TEXT NOT NULL,
                    risk_class TEXT NOT NULL,
                    priority INTEGER NOT NULL,
                    max_attempts INTEGER NOT NULL,
                    state TEXT NOT NULL,
                    attempts INTEGER NOT NULL,
                    lease_owner TEXT,
                    lease_expires_ns INTEGER,
                    checkpoint_json TEXT NOT NULL,
                    side_effect_boundary TEXT NOT NULL,
                    recovered_after_restart INTEGER NOT NULL,
                    reason TEXT,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL,
                    record_sequence INTEGER NOT NULL,
                    record_digest TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS brain_jobs_claim_idx
                    ON brain_jobs(state, priority DESC, created_ns ASC, job_id ASC);
                CREATE INDEX IF NOT EXISTS brain_jobs_expiry_idx
                    ON brain_jobs(state, lease_expires_ns);
                """
            )

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "BrainJobStore":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def submit(self, packet: Mapping[str, Any]) -> tuple[BrainJobRecord, BrainJobEventReceipt]:
        normalized = self._normalize_submission(packet)
        with self._lock:
            self._begin_locked()
            try:
                existing = self._connection.execute(
                    "SELECT * FROM brain_jobs WHERE idempotency_key = ?",
                    (normalized["idempotency_key"],),
                ).fetchone()
                if existing is not None:
                    if existing["spec_digest"] != normalized["spec_digest"]:
                        raise BrainJobError("idempotency key is bound to a different spec_digest")
                    record = self._row_to_record(existing)
                    original = self._connection.execute(
                        "SELECT sequence, event_digest FROM brain_job_events WHERE job_id = ? AND event_type = 'job_submitted' ORDER BY sequence ASC LIMIT 1",
                        (record.job_id,),
                    ).fetchone()
                    if original is None:
                        raise BrainJobError("idempotent job is missing its submission event")
                    head = self._head_locked()
                    self._connection.execute("COMMIT")
                    return record, BrainJobEventReceipt(
                        event_type="job_submitted",
                        job_id=record.job_id,
                        sequence=int(original["sequence"]),
                        event_digest=str(original["event_digest"]),
                        head_digest=head,
                        idempotent=True,
                    )
                count = int(self._connection.execute("SELECT COUNT(*) FROM brain_jobs").fetchone()[0])
                if count >= self.max_jobs:
                    raise BrainJobError("brain job capacity is exhausted")
                checkpoint = normalized["checkpoint"]
                event = self._append_event_locked(
                    event_type="job_submitted",
                    job_id=normalized["job_id"],
                    details={
                        "spec_digest": normalized["spec_digest"],
                        "domain": normalized["domain"],
                        "capability": normalized["capability"],
                        "risk_class": normalized["risk_class"],
                        "priority": normalized["priority"],
                        "max_attempts": normalized["max_attempts"],
                        "checkpoint": checkpoint,
                    },
                )
                now = self._now_ns()
                self._connection.execute(
                    """
                    INSERT INTO brain_jobs (
                        job_id, idempotency_key, spec_digest, domain, capability, risk_class,
                        priority, max_attempts, state, attempts, lease_owner, lease_expires_ns,
                        checkpoint_json, side_effect_boundary, recovered_after_restart, reason,
                        created_ns, updated_ns, record_sequence, record_digest
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'queued', 0, NULL, NULL, ?, 'not_started', 0, NULL, ?, ?, ?, ?)
                    """,
                    (
                        normalized["job_id"],
                        normalized["idempotency_key"],
                        normalized["spec_digest"],
                        normalized["domain"],
                        normalized["capability"],
                        normalized["risk_class"],
                        normalized["priority"],
                        normalized["max_attempts"],
                        _canonical(checkpoint),
                        now,
                        now,
                        event.sequence,
                        event.event_digest,
                    ),
                )
                self._ensure_capacity_locked()
                self._connection.execute("COMMIT")
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (normalized["job_id"],)).fetchone()
                if row is None:
                    raise BrainJobError("job insert was not readable after commit")
                return self._row_to_record(row), event
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def claim(self, job_id: str, worker_id: str, *, lease_seconds: float = 60.0) -> BrainJobRecord:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        worker_id = _job_text("worker_id", worker_id, MAX_JOB_ID_BYTES)
        lease_ns = self._lease_ns(lease_seconds)
        with self._lock:
            self._begin_locked()
            try:
                self._recover_expired_locked(self._now_ns())
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                if row is None:
                    raise BrainJobError("unknown brain job")
                record = self._row_to_record(row)
                if record.state in {"succeeded", "failed", "dead_lettered", "cancelled"}:
                    self._connection.execute("COMMIT")
                    return record
                if record.state == "reconciliation_required":
                    raise BrainJobError("brain job requires operator reconciliation before it can be claimed")
                if record.state == "waiting_approval":
                    raise BrainJobError("brain job is waiting for caller approval")
                if record.state not in {"queued"}:
                    if record.lease_owner == worker_id:
                        self._connection.execute("COMMIT")
                        return record
                    raise BrainJobError("brain job is already leased by another worker")
                if record.attempts >= record.max_attempts:
                    self._transition_locked(
                        record,
                        event_type="job_dead_lettered",
                        state="dead_lettered",
                        reason="maximum attempts exhausted",
                        lease_owner=None,
                        lease_expires_ns=None,
                    )
                    self._connection.execute("COMMIT")
                    return self._row_to_record(
                        self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                    )
                expires = self._now_ns() + lease_ns
                self._transition_locked(
                    record,
                    event_type="job_claimed",
                    state="leased",
                    reason=None,
                    lease_owner=worker_id,
                    lease_expires_ns=expires,
                    attempts=record.attempts + 1,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def claim_next(self, worker_id: str, *, lease_seconds: float = 60.0) -> BrainJobRecord | None:
        """Atomically select and lease the next runnable job.

        ``inventory()`` followed by ``claim()`` is safe but creates a race between a scheduler
        and its workers: another process can take the displayed row before the scheduler claims
        it.  This primitive performs recovery, priority ordering, dead-letter admission, and the
        lease transition in one SQLite transaction.  It is deliberately metadata-only; the
        caller still owns task/prompt/provider rehydration.
        """

        worker_id = _job_text("worker_id", worker_id, MAX_JOB_ID_BYTES)
        lease_ns = self._lease_ns(lease_seconds)
        with self._lock:
            self._begin_locked()
            try:
                self._recover_expired_locked(self._now_ns())
                while True:
                    row = self._connection.execute(
                        "SELECT * FROM brain_jobs WHERE state = 'queued' ORDER BY priority DESC, created_ns ASC, job_id ASC LIMIT 1"
                    ).fetchone()
                    if row is None:
                        self._connection.execute("COMMIT")
                        return None
                    record = self._row_to_record(row)
                    if record.attempts >= record.max_attempts:
                        self._transition_locked(
                            record,
                            event_type="job_dead_lettered",
                            state="dead_lettered",
                            reason="maximum attempts exhausted",
                            lease_owner=None,
                            lease_expires_ns=None,
                        )
                        continue
                    self._transition_locked(
                        record,
                        event_type="job_claimed",
                        state="leased",
                        reason=None,
                        lease_owner=worker_id,
                        lease_expires_ns=self._now_ns() + lease_ns,
                        attempts=record.attempts + 1,
                    )
                    self._connection.execute("COMMIT")
                    current = self._connection.execute(
                        "SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)
                    ).fetchone()
                    if current is None:
                        raise BrainJobError("claimed brain job was not readable after commit")
                    return self._row_to_record(current)
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def renew(self, job_id: str, worker_id: str, *, lease_seconds: float = 60.0) -> BrainJobRecord:
        record = self._require_owned(job_id, worker_id)
        expires = self._now_ns() + self._lease_ns(lease_seconds)
        with self._lock:
            self._begin_locked()
            try:
                current = self._require_owned_locked(record.job_id, worker_id)
                self._connection.execute(
                    "UPDATE brain_jobs SET lease_expires_ns = ?, updated_ns = ? WHERE job_id = ?",
                    (expires, self._now_ns(), record.job_id),
                )
                self._append_event_locked(
                    event_type="job_lease_renewed",
                    job_id=record.job_id,
                    details={"lease_owner": worker_id, "lease_expires_ns": expires},
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (current.job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def checkpoint(
        self,
        job_id: str,
        worker_id: str,
        *,
        phase: str,
        checkpoint: Mapping[str, Any],
        side_effect_boundary: str = "not_started",
        waiting_for_approval: bool = False,
    ) -> BrainJobRecord:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        worker_id = _job_text("worker_id", worker_id, MAX_JOB_ID_BYTES)
        phase = _job_text("checkpoint phase", phase)
        if side_effect_boundary not in JOB_BOUNDARIES:
            raise BrainJobError(f"unknown side_effect_boundary: {side_effect_boundary}")
        normalized = _safe_job_value(checkpoint)
        if not isinstance(normalized, Mapping):
            raise BrainJobError("checkpoint must be a mapping")
        checkpoint_payload = {"phase": phase, **dict(normalized)}
        if len(_canonical(checkpoint_payload).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
            raise BrainJobError("checkpoint exceeds the bounded size")
        with self._lock:
            self._begin_locked()
            try:
                record = self._require_owned_locked(job_id, worker_id)
                if _BOUNDARY_ORDER[side_effect_boundary] < _BOUNDARY_ORDER[record.side_effect_boundary]:
                    raise BrainJobError("side_effect_boundary cannot move backwards")
                state = "waiting_approval" if waiting_for_approval else "running"
                lease_owner = None if waiting_for_approval else worker_id
                lease_expires_ns = None if waiting_for_approval else record.lease_expires_ns
                self._transition_locked(
                    record,
                    event_type="job_checkpointed",
                    state=state,
                    reason=None,
                    lease_owner=lease_owner,
                    lease_expires_ns=lease_expires_ns,
                    checkpoint=checkpoint_payload,
                    side_effect_boundary=side_effect_boundary,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def complete(self, job_id: str, worker_id: str, *, result_metadata: Mapping[str, Any] | None = None) -> BrainJobRecord:
        return self._finish(
            job_id,
            worker_id,
            state="succeeded",
            event_type="job_completed",
            reason=None,
            result_metadata={} if result_metadata is None else result_metadata,
        )

    def request_approval(
        self,
        job_id: str,
        *,
        requester: str,
        approval_id: str,
        approval_scope: str,
        request_digest: str,
        required_role: str = "operator",
    ) -> BrainJobRecord:
        """Park a queued job at an approval boundary without taking a worker lease.

        The MCP projection can request approval for a queued job because the request itself is
        an admission decision, not execution.  The older ``checkpoint(..., waiting_for_approval
        =True)`` path remains the correct operation for a leased worker that discovers an approval
        requirement mid-run.  Keeping both paths in the store prevents an HTTP/MCP adapter from
        having to mutate SQLite state outside the journal's transition helper.
        """

        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        requester = _job_text("approval requester", requester, MAX_JOB_ID_BYTES)
        approval_id = _job_text("approval_id", approval_id, MAX_JOB_ID_BYTES)
        approval_scope = _job_text("approval_scope", approval_scope, MAX_JOB_CHECKPOINT_BYTES)
        required_role = _job_text("required_role", required_role, 128)
        if not _valid_digest(request_digest):
            raise BrainJobError("approval request_digest must be a lowercase SHA-256 digest")
        with self._lock:
            self._begin_locked()
            try:
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                if row is None:
                    raise BrainJobError("unknown brain job")
                record = self._row_to_record(row)
                if record.state == "waiting_approval":
                    existing = record.checkpoint
                    if (
                        existing.get("approval_id") == approval_id
                        and existing.get("request_digest") == request_digest
                    ):
                        self._connection.execute("COMMIT")
                        return record
                    raise BrainJobError("brain job already has a different approval request")
                if record.state != "queued":
                    raise BrainJobError(f"cannot request approval while job is in state {record.state!r}")
                checkpoint = {
                    **dict(record.checkpoint),
                    "phase": "approval_requested",
                    "approval_id": approval_id,
                    "approval_scope": approval_scope,
                    "request_digest": request_digest,
                    "required_role": required_role,
                    "requested_by": requester,
                    "created_ns": self._now_ns(),
                }
                self._transition_locked(
                    record,
                    event_type="job_approval_requested",
                    state="waiting_approval",
                    reason=None,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint=checkpoint,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def resume_waiting(self, job_id: str, *, approver: str, reason: str = "caller approval granted") -> BrainJobRecord:
        """Release a waiting-approval job back to the queue without dispatching it."""

        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        approver = _job_text("approver", approver, MAX_JOB_ID_BYTES)
        reason = _job_text("approval reason", reason, MAX_JOB_REASON_BYTES)
        with self._lock:
            self._begin_locked()
            try:
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                if row is None:
                    raise BrainJobError("unknown brain job")
                record = self._row_to_record(row)
                if record.state != "waiting_approval":
                    raise BrainJobError("brain job is not waiting for approval")
                self._transition_locked(
                    record,
                    event_type="job_approval_released",
                    state="queued",
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint={
                        **dict(record.checkpoint),
                        "phase": "approval_released",
                        "approver": approver,
                    },
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def release(self, job_id: str, worker_id: str, *, reason: str = "checkpoint persisted; worker released lease") -> BrainJobRecord:
        """Return an active job to the queue after a durable cooperative checkpoint.

        This is the hand-off primitive for multi-step work. A worker may finish one bounded
        stage, persist the continuation, and release its lease so another process can claim the
        next stage. The operation refuses to requeue work after an external-effect boundary;
        that work must remain quarantined for reconciliation instead of being replayed.
        """

        reason = _job_text("job release reason", reason, MAX_JOB_REASON_BYTES)
        with self._lock:
            self._begin_locked()
            try:
                record = self._require_owned_locked(job_id, worker_id)
                if record.side_effect_boundary in {"dispatched", "unknown"}:
                    raise BrainJobError("job cannot be cooperatively released after external dispatch")
                self._transition_locked(
                    record,
                    event_type="job_released",
                    state="queued",
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint={
                        **dict(record.checkpoint),
                        "phase": "released",
                        "release_reason": reason,
                    },
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def fail(
        self,
        job_id: str,
        worker_id: str,
        *,
        reason: str,
        retryable: bool = False,
        reason_digest: str | None = None,
    ) -> BrainJobRecord:
        reason = _job_text("job failure reason", reason, MAX_JOB_REASON_BYTES)
        if not isinstance(retryable, bool):
            raise BrainJobError("retryable must be boolean")
        if reason_digest is not None and not _valid_digest(reason_digest):
            raise BrainJobError("reason_digest must be a lowercase SHA-256 digest")
        with self._lock:
            self._begin_locked()
            try:
                record = self._require_owned_locked(job_id, worker_id)
                if record.side_effect_boundary in {"dispatched", "unknown"}:
                    state = "reconciliation_required"
                    event_type = "job_reconciliation_required"
                elif retryable and record.attempts < record.max_attempts:
                    state = "queued"
                    event_type = "job_retry_queued"
                else:
                    state = "dead_lettered" if record.attempts >= record.max_attempts else "failed"
                    event_type = "job_dead_lettered" if state == "dead_lettered" else "job_failed"
                checkpoint = {
                    **dict(record.checkpoint),
                    "phase": "failed",
                    "reason": reason,
                }
                if reason_digest is not None:
                    checkpoint["reason_digest"] = reason_digest
                self._transition_locked(
                    record,
                    event_type=event_type,
                    state=state,
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint=checkpoint,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def reconcile(
        self,
        job_id: str,
        *,
        outcome: str,
        evidence_digest: str,
        evidence_kind: str = "caller_observation",
        operator: str = "caller",
        reason: str = "caller reconciled uncertain external state",
        metadata: Mapping[str, Any] | None = None,
    ) -> BrainJobRecord:
        """Resolve an uncertain external effect without replaying it implicitly.

        ``succeeded`` and ``failed`` close the job with the caller's verified outcome.
        ``not_executed`` is the only outcome that can safely return the job to the queue; it
        resets the side-effect boundary only because the caller has explicitly supplied evidence
        that no external effect occurred. ``unknown`` records a bounded review decision while
        keeping the job quarantined. Raw evidence remains caller-owned and only its digest and
        value-only metadata are retained.
        """

        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        decision = _reconciliation_payload(
            outcome=outcome,
            evidence_digest=evidence_digest,
            evidence_kind=evidence_kind,
            operator=operator,
            reason=reason,
            metadata=metadata,
        )
        with self._lock:
            self._begin_locked()
            try:
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                if row is None:
                    raise BrainJobError("unknown brain job")
                record = self._row_to_record(row)
                existing = record.checkpoint.get("reconciliation")
                if isinstance(existing, Mapping):
                    retained_phase = record.checkpoint.get("phase")
                    if retained_phase not in {
                        "reconciliation_deferred",
                        "reconciliation_retry_queued",
                        "reconciliation_completed",
                    }:
                        raise BrainJobError("job checkpoint contains untrusted reconciliation metadata")
                    if existing.get("decision_digest") == decision["decision_digest"]:
                        self._connection.execute("COMMIT")
                        return record
                    if record.state != "reconciliation_required" or existing.get("outcome") != "unknown":
                        raise BrainJobError("job already has a different reconciliation decision")
                if record.state != "reconciliation_required":
                    raise BrainJobError("job is not awaiting reconciliation")
                if outcome == "not_executed" and record.attempts >= record.max_attempts:
                    raise BrainJobError("reconciliation retry is unavailable after maximum attempts")
                if outcome == "succeeded":
                    state = "succeeded"
                    event_type = "job_reconciled_succeeded"
                    phase = "reconciliation_completed"
                    boundary = record.side_effect_boundary
                elif outcome == "failed":
                    state = "failed"
                    event_type = "job_reconciled_failed"
                    phase = "reconciliation_completed"
                    boundary = record.side_effect_boundary
                elif outcome == "not_executed":
                    state = "queued"
                    event_type = "job_reconciliation_retry_queued"
                    phase = "reconciliation_retry_queued"
                    boundary = "not_started"
                else:
                    state = "reconciliation_required"
                    event_type = "job_reconciliation_deferred"
                    phase = "reconciliation_deferred"
                    boundary = record.side_effect_boundary
                checkpoint = {
                    **dict(record.checkpoint),
                    "phase": phase,
                    "reconciliation": decision,
                }
                self._transition_locked(
                    record,
                    event_type=event_type,
                    state=state,
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint=checkpoint,
                    side_effect_boundary=boundary,
                    allow_reconciliation_reset=outcome == "not_executed",
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def cancel(
        self,
        job_id: str,
        *,
        reason: str = "cancelled by caller",
        reason_digest: str | None = None,
    ) -> BrainJobRecord:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        reason = _job_text("cancellation reason", reason, MAX_JOB_REASON_BYTES)
        if reason_digest is not None and not _valid_digest(reason_digest):
            raise BrainJobError("reason_digest must be a lowercase SHA-256 digest")
        with self._lock:
            self._begin_locked()
            try:
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                if row is None:
                    raise BrainJobError("unknown brain job")
                record = self._row_to_record(row)
                if record.terminal:
                    self._connection.execute("COMMIT")
                    return record
                if record.state in {"leased", "running"} and record.side_effect_boundary in {"dispatched", "unknown"}:
                    checkpoint = {
                        **dict(record.checkpoint),
                        "phase": "cancellation_quarantined",
                        "reason": reason,
                    }
                    if reason_digest is not None:
                        checkpoint["reason_digest"] = reason_digest
                    self._transition_locked(
                        record,
                        event_type="job_cancellation_quarantined",
                        state="reconciliation_required",
                        reason=reason,
                        lease_owner=None,
                        lease_expires_ns=None,
                        checkpoint=checkpoint,
                    )
                    self._connection.execute("COMMIT")
                    return self._row_to_record(
                        self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                    )
                checkpoint = {
                    **dict(record.checkpoint),
                    "phase": "cancelled",
                    "reason": reason,
                }
                if reason_digest is not None:
                    checkpoint["reason_digest"] = reason_digest
                self._transition_locked(
                    record,
                    event_type="job_cancelled",
                    state="cancelled",
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint=checkpoint,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def get(self, job_id: str) -> BrainJobRecord | None:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        with self._lock:
            self._begin_locked()
            try:
                self._recover_expired_locked(self._now_ns())
                row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
                self._connection.execute("COMMIT")
                return None if row is None else self._row_to_record(row)
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def inventory(self, *, limit: int = 100, state: str | None = None) -> tuple[BrainJobRecord, ...]:
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_JOB_INVENTORY:
            raise BrainJobError(f"inventory limit must be within [1, {MAX_JOB_INVENTORY}]")
        if state is not None and state not in JOB_STATES:
            raise BrainJobError(f"unknown inventory state: {state}")
        with self._lock:
            self._begin_locked()
            try:
                self._recover_expired_locked(self._now_ns())
                if state is None:
                    rows = self._connection.execute(
                        "SELECT * FROM brain_jobs ORDER BY priority DESC, created_ns ASC, job_id ASC LIMIT ?",
                        (limit,),
                    ).fetchall()
                else:
                    rows = self._connection.execute(
                        "SELECT * FROM brain_jobs WHERE state = ? ORDER BY priority DESC, created_ns ASC, job_id ASC LIMIT ?",
                        (state, limit),
                    ).fetchall()
                self._connection.execute("COMMIT")
                return tuple(self._row_to_record(row) for row in rows)
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def recover_expired(self) -> tuple[BrainJobRecord, ...]:
        with self._lock:
            self._begin_locked()
            try:
                changed = self._recover_expired_locked(self._now_ns())
                self._connection.execute("COMMIT")
                return tuple(changed)
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def events(
        self,
        *,
        after_sequence: int = 0,
        job_id: str | None = None,
        limit: int = 100,
    ) -> tuple[BrainJobEvent, ...]:
        """Read a bounded cursor page for cross-process workers and operator dashboards."""

        if not isinstance(after_sequence, int) or isinstance(after_sequence, bool) or after_sequence < 0:
            raise BrainJobError("after_sequence must be a non-negative integer")
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_JOB_INVENTORY:
            raise BrainJobError(f"event limit must be within [1, {MAX_JOB_INVENTORY}]")
        if job_id is not None:
            job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        with self._lock:
            try:
                if job_id is None:
                    rows = self._connection.execute(
                        "SELECT * FROM brain_job_events WHERE sequence > ? ORDER BY sequence ASC LIMIT ?",
                        (after_sequence, limit),
                    ).fetchall()
                else:
                    rows = self._connection.execute(
                        "SELECT * FROM brain_job_events WHERE sequence > ? AND job_id = ? ORDER BY sequence ASC LIMIT ?",
                        (after_sequence, job_id, limit),
                    ).fetchall()
                return tuple(self._row_to_event(row) for row in rows)
            except sqlite3.Error as error:
                raise BrainJobError("could not read brain job events") from error

    def head_digest(self) -> str:
        """Return the current event-chain head without exposing event bodies."""

        with self._lock:
            return self._head_locked()

    def snapshot(self) -> dict[str, Any]:
        """Return a verified, portable snapshot of queue state and its event chain.

        The snapshot is an explicit worker handoff artifact.  It contains the redacted job
        index and metadata-only events, never the resolver-owned task, prompt, provider output,
        credentials, or tool payload.  Restoring it into another ``BrainJobStore`` is a full
        replacement operation and must be coordinated by the caller-owned persistence adapter.
        """

        with self._lock:
            integrity = self.verify_integrity()
            if not integrity["ok"]:
                raise BrainJobError("cannot snapshot an invalid brain job journal")
            event_rows = self._connection.execute("SELECT * FROM brain_job_events ORDER BY sequence ASC").fetchall()
            job_rows = self._connection.execute("SELECT * FROM brain_jobs ORDER BY job_id ASC").fetchall()
            descriptor = {
                "schema": JOB_SNAPSHOT_SCHEMA,
                "events": [self._row_to_event(row).to_dict() for row in event_rows],
                "jobs": [self._row_to_record(row).to_dict() for row in job_rows],
                "head_digest": integrity["head_digest"],
                "retention": "metadata_only_hash_chained",
                "secret_material": "never_returned",
            }
            snapshot = {**descriptor, "snapshot_digest": _digest(descriptor)}
            if len(_canonical(snapshot).encode("utf-8")) > MAX_JOB_SNAPSHOT_BYTES:
                raise BrainJobError("brain job snapshot exceeds its byte capacity")
            return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        """Atomically replace this SQLite queue with a validated worker snapshot."""

        normalized = _normalize_job_snapshot(snapshot, max_jobs=self.max_jobs, max_bytes=self.max_bytes)
        with self._lock:
            self._begin_locked()
            try:
                self._connection.execute("DELETE FROM brain_jobs")
                self._connection.execute("DELETE FROM brain_job_events")
                for event in normalized["events"]:
                    payload = event["payload"]
                    self._connection.execute(
                        "INSERT INTO brain_job_events (sequence, event_type, job_id, payload_json, previous_digest, event_digest, created_ns) VALUES (?, ?, ?, ?, ?, ?, ?)",
                        (
                            event["sequence"],
                            event["event_type"],
                            event["job_id"],
                            _canonical(payload),
                            event["previous_digest"],
                            event["event_digest"],
                            event["created_ns"],
                        ),
                    )
                for job in normalized["jobs"]:
                    self._connection.execute(
                        """
                        INSERT INTO brain_jobs (
                            job_id, idempotency_key, spec_digest, domain, capability, risk_class,
                            priority, max_attempts, state, attempts, lease_owner, lease_expires_ns,
                            checkpoint_json, side_effect_boundary, recovered_after_restart, reason,
                            created_ns, updated_ns, record_sequence, record_digest
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            job["job_id"],
                            job["idempotency_key"],
                            job["spec_digest"],
                            job["domain"],
                            job["capability"],
                            job["risk_class"],
                            job["priority"],
                            job["max_attempts"],
                            job["state"],
                            job["attempts"],
                            job["lease_owner"],
                            job["lease_expires_ns"],
                            _canonical(job["checkpoint"]),
                            job["side_effect_boundary"],
                            1 if job["recovered_after_restart"] else 0,
                            job["reason"],
                            job["created_ns"],
                            job["updated_ns"],
                            job["record_sequence"],
                            job["record_digest"],
                        ),
                    )
                self._ensure_capacity_locked()
                self._connection.execute("COMMIT")
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            try:
                rows = self._connection.execute("SELECT * FROM brain_job_events ORDER BY sequence ASC").fetchall()
                previous = ""
                submitted: set[str] = set()
                for row in rows:
                    if row["previous_digest"] != previous:
                        raise BrainJobError(f"job event hash chain breaks at sequence {row['sequence']}")
                    try:
                        payload = json.loads(row["payload_json"])
                    except (TypeError, ValueError, json.JSONDecodeError) as error:
                        raise BrainJobError("job event contains invalid JSON") from error
                    if not isinstance(payload, Mapping) or payload.get("schema") != JOB_EVENT_SCHEMA:
                        raise BrainJobError(f"job event schema mismatch at sequence {row['sequence']}")
                    expected = _digest(
                        {
                            "schema": JOB_EVENT_SCHEMA,
                            "event_type": row["event_type"],
                            "job_id": row["job_id"],
                            "payload": payload,
                            "previous_digest": row["previous_digest"],
                            "sequence": row["sequence"],
                            "created_ns": row["created_ns"],
                        }
                    )
                    if row["event_digest"] != expected:
                        raise BrainJobError(f"job event digest mismatch at sequence {row['sequence']}")
                    if row["event_type"] == "job_submitted":
                        submitted.add(row["job_id"])
                    previous = row["event_digest"]
                indexed = {
                    row["job_id"] for row in self._connection.execute("SELECT job_id FROM brain_jobs").fetchall()
                }
                if not indexed.issubset(submitted):
                    raise BrainJobError("job index contains a record without a submission event")
                return {
                    "schema": JOB_SCHEMA,
                    "ok": True,
                    "event_count": len(rows),
                    "job_count": len(indexed),
                    "head_digest": previous,
                    "chain": "sha256_prev_digest",
                }
            except BrainJobError as error:
                return {
                    "schema": JOB_SCHEMA,
                    "ok": False,
                    "event_count": 0,
                    "job_count": 0,
                    "head_digest": None,
                    "chain": "sha256_prev_digest",
                    "reason": str(error),
                }

    def stats(self) -> dict[str, Any]:
        with self._lock:
            rows = self._connection.execute("SELECT state, COUNT(*) AS count FROM brain_jobs GROUP BY state").fetchall()
            return {
                "schema": JOB_SCHEMA,
                "job_count": int(self._connection.execute("SELECT COUNT(*) FROM brain_jobs").fetchone()[0]),
                "event_count": int(self._connection.execute("SELECT COUNT(*) FROM brain_job_events").fetchone()[0]),
                "states": {row["state"]: int(row["count"]) for row in rows},
                "max_jobs": self.max_jobs,
                "max_bytes": self.max_bytes,
                "retention": "metadata_only_hash_chained",
            }

    def _normalize_submission(self, packet: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(packet, Mapping) or any(not isinstance(key, str) for key in packet):
            raise BrainJobError("job submission must be a mapping with string keys")
        allowed = {
            "job_id",
            "idempotency_key",
            "spec_digest",
            "domain",
            "capability",
            "risk_class",
            "priority",
            "max_attempts",
            "checkpoint",
        }
        unknown = sorted(set(packet).difference(allowed))
        if unknown:
            raise BrainJobError("job submission contains unsupported fields: " + ", ".join(unknown))
        job_id = packet.get("job_id") or "job-" + uuid.uuid4().hex
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        idempotency_key = _job_text("idempotency_key", packet.get("idempotency_key"), MAX_JOB_ID_BYTES)
        spec_digest = packet.get("spec_digest")
        if not _valid_digest(spec_digest):
            raise BrainJobError("spec_digest must be a lowercase SHA-256 digest")
        domain = _job_text("job domain", packet.get("domain"))
        capability = _job_text("job capability", packet.get("capability"))
        risk_class = _job_text("job risk_class", packet.get("risk_class"))
        priority = packet.get("priority", 0)
        if not isinstance(priority, int) or isinstance(priority, bool) or not 0 <= priority <= 255:
            raise BrainJobError("priority must be within [0, 255]")
        max_attempts = packet.get("max_attempts", 3)
        if not isinstance(max_attempts, int) or isinstance(max_attempts, bool) or not 1 <= max_attempts <= 8:
            raise BrainJobError("max_attempts must be within [1, 8]")
        checkpoint = _safe_job_value(packet.get("checkpoint", {}))
        if not isinstance(checkpoint, Mapping):
            raise BrainJobError("initial checkpoint must be a mapping")
        if len(_canonical(checkpoint).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
            raise BrainJobError("initial checkpoint exceeds the bounded size")
        return {
            "job_id": job_id,
            "idempotency_key": idempotency_key,
            "spec_digest": spec_digest,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "priority": priority,
            "max_attempts": max_attempts,
            "checkpoint": dict(checkpoint),
        }

    def _finish(
        self,
        job_id: str,
        worker_id: str,
        *,
        state: str,
        event_type: str,
        reason: str | None,
        result_metadata: Mapping[str, Any],
    ) -> BrainJobRecord:
        if state not in {"succeeded"}:
            raise BrainJobError("unsupported finish state")
        normalized = _safe_job_value(result_metadata)
        if not isinstance(normalized, Mapping) or len(_canonical(normalized).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
            raise BrainJobError("result metadata must be a bounded mapping")
        with self._lock:
            self._begin_locked()
            try:
                record = self._require_owned_locked(job_id, worker_id)
                checkpoint = {"phase": "completed", "result_metadata": dict(normalized)}
                self._transition_locked(
                    record,
                    event_type=event_type,
                    state=state,
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint=checkpoint,
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def _transition_locked(
        self,
        record: BrainJobRecord,
        *,
        event_type: str,
        state: str,
        reason: str | None,
        lease_owner: str | None,
        lease_expires_ns: int | None,
        checkpoint: Mapping[str, Any] | None = None,
        side_effect_boundary: str | None = None,
        attempts: int | None = None,
        allow_reconciliation_reset: bool = False,
    ) -> BrainJobEventReceipt:
        if state not in JOB_STATES:
            raise BrainJobError(f"unknown job state: {state}")
        if reason is not None:
            reason = _job_text("job reason", reason, MAX_JOB_REASON_BYTES)
        next_boundary = record.side_effect_boundary if side_effect_boundary is None else side_effect_boundary
        if next_boundary not in JOB_BOUNDARIES:
            raise BrainJobError("job side_effect_boundary cannot move backwards")
        if (
            _BOUNDARY_ORDER[next_boundary] < _BOUNDARY_ORDER[record.side_effect_boundary]
            and not (
                allow_reconciliation_reset
                and record.state == "reconciliation_required"
                and next_boundary == "not_started"
            )
        ):
            raise BrainJobError("job side_effect_boundary cannot move backwards")
        next_checkpoint = dict(record.checkpoint) if checkpoint is None else dict(_safe_job_value(checkpoint))
        if len(_canonical(next_checkpoint).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
            raise BrainJobError("job checkpoint exceeds the bounded size")
        details = {
            "state": state,
            "reason": reason,
            "lease_owner": lease_owner,
            "lease_expires_ns": lease_expires_ns,
            "attempts": record.attempts if attempts is None else attempts,
            "checkpoint": next_checkpoint,
            "side_effect_boundary": next_boundary,
        }
        event = self._append_event_locked(event_type=event_type, job_id=record.job_id, details=details)
        now = self._now_ns()
        self._connection.execute(
            """
            UPDATE brain_jobs SET state = ?, attempts = ?, lease_owner = ?, lease_expires_ns = ?,
                checkpoint_json = ?, side_effect_boundary = ?, recovered_after_restart = ?, reason = ?,
                updated_ns = ?, record_sequence = ?, record_digest = ? WHERE job_id = ?
            """,
            (
                state,
                record.attempts if attempts is None else attempts,
                lease_owner,
                lease_expires_ns,
                _canonical(next_checkpoint),
                next_boundary,
                1 if record.recovered_after_restart else 0,
                reason,
                now,
                event.sequence,
                event.event_digest,
                record.job_id,
            ),
        )
        return event

    def _recover_expired_locked(self, now_ns: int) -> list[BrainJobRecord]:
        rows = self._connection.execute(
            "SELECT * FROM brain_jobs WHERE state IN ('leased', 'running') AND lease_expires_ns IS NOT NULL AND lease_expires_ns <= ?",
            (now_ns,),
        ).fetchall()
        changed: list[BrainJobRecord] = []
        for row in rows:
            record = self._row_to_record(row)
            if record.side_effect_boundary in {"not_started", "preflight"}:
                state = "queued"
                event_type = "job_lease_expired_requeued"
                reason = "lease expired before external dispatch; safe to reclaim"
            else:
                state = "reconciliation_required"
                event_type = "job_lease_expired_quarantined"
                reason = "lease expired after or near external dispatch; operator reconciliation required"
            event = self._append_event_locked(
                event_type=event_type,
                job_id=record.job_id,
                details={"previous_state": record.state, "previous_owner": record.lease_owner, "reason": reason},
            )
            now = self._now_ns()
            self._connection.execute(
                "UPDATE brain_jobs SET state = ?, lease_owner = NULL, lease_expires_ns = NULL, recovered_after_restart = 1, reason = ?, updated_ns = ?, record_sequence = ?, record_digest = ? WHERE job_id = ?",
                (state, reason, now, event.sequence, event.event_digest, record.job_id),
            )
            changed.append(
                self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)).fetchone()
                )
            )
        return changed

    def _require_owned(self, job_id: str, worker_id: str) -> BrainJobRecord:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        worker_id = _job_text("worker_id", worker_id, MAX_JOB_ID_BYTES)
        with self._lock:
            row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
            if row is None:
                raise BrainJobError("unknown brain job")
            record = self._row_to_record(row)
            if record.lease_owner != worker_id or record.state not in {"leased", "running"}:
                raise BrainJobError("worker does not own an active lease")
            if record.lease_expires_ns is None or record.lease_expires_ns <= self._now_ns():
                raise BrainJobError("brain job lease has expired")
            return record

    def _require_owned_locked(self, job_id: str, worker_id: str) -> BrainJobRecord:
        row = self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (job_id,)).fetchone()
        if row is None:
            raise BrainJobError("unknown brain job")
        record = self._row_to_record(row)
        if record.lease_owner != worker_id or record.state not in {"leased", "running"}:
            raise BrainJobError("worker does not own an active lease")
        if record.lease_expires_ns is None or record.lease_expires_ns <= self._now_ns():
            raise BrainJobError("brain job lease has expired")
        return record

    def _row_to_record(self, row: sqlite3.Row) -> BrainJobRecord:
        try:
            checkpoint = json.loads(row["checkpoint_json"])
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise BrainJobError("job checkpoint contains invalid JSON") from error
        if not isinstance(checkpoint, Mapping):
            raise BrainJobError("job checkpoint is not an object")
        return BrainJobRecord(
            job_id=row["job_id"],
            idempotency_key=row["idempotency_key"],
            spec_digest=row["spec_digest"],
            domain=row["domain"],
            capability=row["capability"],
            risk_class=row["risk_class"],
            priority=row["priority"],
            max_attempts=row["max_attempts"],
            state=row["state"],
            attempts=row["attempts"],
            lease_owner=row["lease_owner"],
            lease_expires_ns=row["lease_expires_ns"],
            checkpoint=dict(checkpoint),
            side_effect_boundary=row["side_effect_boundary"],
            recovered_after_restart=bool(row["recovered_after_restart"]),
            reason=row["reason"],
            created_ns=row["created_ns"],
            updated_ns=row["updated_ns"],
            record_sequence=row["record_sequence"],
            record_digest=row["record_digest"],
        )

    def _row_to_event(self, row: sqlite3.Row) -> BrainJobEvent:
        try:
            payload = json.loads(row["payload_json"])
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise BrainJobError("job event contains invalid JSON") from error
        if not isinstance(payload, Mapping) or payload.get("schema") != JOB_EVENT_SCHEMA:
            raise BrainJobError("job event has an invalid schema")
        return BrainJobEvent(
            sequence=int(row["sequence"]),
            event_type=str(row["event_type"]),
            job_id=str(row["job_id"]),
            payload=dict(payload),
            previous_digest=str(row["previous_digest"]),
            event_digest=str(row["event_digest"]),
            created_ns=int(row["created_ns"]),
        )

    def _append_event_locked(self, *, event_type: str, job_id: str, details: Mapping[str, Any]) -> BrainJobEventReceipt:
        previous = self._head_locked()
        sequence = int(self._connection.execute("SELECT COALESCE(MAX(sequence), 0) + 1 FROM brain_job_events").fetchone()[0])
        created_ns = self._now_ns()
        payload = {"schema": JOB_EVENT_SCHEMA, "event": event_type, "job_id": job_id, "details": dict(_safe_job_value(details))}
        envelope = {
            "schema": JOB_EVENT_SCHEMA,
            "event_type": event_type,
            "job_id": job_id,
            "payload": payload,
            "previous_digest": previous,
            "sequence": sequence,
            "created_ns": created_ns,
        }
        event_digest = _digest(envelope)
        try:
            self._connection.execute(
                "INSERT INTO brain_job_events (sequence, event_type, job_id, payload_json, previous_digest, event_digest, created_ns) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (sequence, event_type, job_id, _canonical(payload), previous, event_digest, created_ns),
            )
        except sqlite3.Error as error:
            raise BrainJobError("could not append brain job event") from error
        return BrainJobEventReceipt(event_type, job_id, sequence, event_digest, event_digest)

    def _head_locked(self) -> str:
        row = self._connection.execute("SELECT event_digest FROM brain_job_events ORDER BY sequence DESC LIMIT 1").fetchone()
        return "" if row is None else str(row["event_digest"])

    def _begin_locked(self) -> None:
        try:
            self._connection.execute("BEGIN IMMEDIATE")
        except sqlite3.Error as error:
            raise BrainJobError("could not begin brain job transaction") from error

    def _lease_ns(self, lease_seconds: float) -> int:
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not math.isfinite(float(lease_seconds)) or not 1 <= lease_seconds <= 86_400:
            raise BrainJobError("lease_seconds must be within [1, 86400]")
        return int(float(lease_seconds) * 1_000_000_000)

    def _now_ns(self) -> int:
        try:
            value = float(self._clock())
        except Exception as error:
            raise BrainJobError("job clock failed") from error
        if not math.isfinite(value):
            raise BrainJobError("job clock returned a non-finite value")
        return int(value * 1_000_000_000)

    def _ensure_capacity_locked(self) -> None:
        page_count = int(self._connection.execute("PRAGMA page_count").fetchone()[0])
        page_size = int(self._connection.execute("PRAGMA page_size").fetchone()[0])
        if page_count * page_size > self.max_bytes:
            raise BrainJobError("brain job byte capacity is exhausted")


def _normalize_job_snapshot(
    value: Mapping[str, Any],
    *,
    max_jobs: int = MAX_JOB_SNAPSHOT_JOBS,
    max_bytes: int = MAX_JOB_SNAPSHOT_BYTES,
) -> dict[str, Any]:
    """Validate the queue index, event chain, and snapshot digest as one unit."""

    expected_keys = {"schema", "events", "jobs", "head_digest", "retention", "secret_material", "snapshot_digest"}
    if not isinstance(value, Mapping) or set(value) != expected_keys:
        raise BrainJobError("brain job snapshot is malformed")
    if value.get("schema") != JOB_SNAPSHOT_SCHEMA:
        raise BrainJobError("brain job snapshot schema is unsupported")
    if value.get("retention") != "metadata_only_hash_chained" or value.get("secret_material") != "never_returned":
        raise BrainJobError("brain job snapshot retention is invalid")
    raw_events = value.get("events")
    if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes, bytearray)) or len(raw_events) > MAX_JOB_EVENTS:
        raise BrainJobError("brain job snapshot event count is outside its bound")
    event_keys = {"schema", "sequence", "event_type", "job_id", "payload", "previous_digest", "event_digest", "created_ns", "retention"}
    events: list[dict[str, Any]] = []
    events_by_sequence: dict[int, dict[str, Any]] = {}
    submitted: set[str] = set()
    previous_digest = ""
    for expected_sequence, raw_event in enumerate(raw_events, start=1):
        if not isinstance(raw_event, Mapping) or set(raw_event) != event_keys:
            raise BrainJobError("brain job snapshot contains an invalid event envelope")
        if raw_event.get("schema") != JOB_EVENT_SCHEMA or raw_event.get("retention") != "metadata_only_hash_chained":
            raise BrainJobError("brain job snapshot event schema or retention is invalid")
        if raw_event.get("sequence") != expected_sequence:
            raise BrainJobError("brain job snapshot event sequence is invalid")
        event_type = _job_text("brain job snapshot event_type", raw_event.get("event_type"), 256)
        job_id = _job_text("brain job snapshot event job_id", raw_event.get("job_id"), MAX_JOB_ID_BYTES)
        payload = _safe_job_value(raw_event.get("payload"))
        if not isinstance(payload, Mapping) or payload.get("schema") != JOB_EVENT_SCHEMA or payload.get("event") != event_type or payload.get("job_id") != job_id:
            raise BrainJobError("brain job snapshot event payload is invalid")
        if raw_event.get("previous_digest") != previous_digest:
            raise BrainJobError("brain job snapshot event hash chain is discontinuous")
        created_ns = raw_event.get("created_ns")
        if not isinstance(created_ns, int) or isinstance(created_ns, bool) or created_ns < 0:
            raise BrainJobError("brain job snapshot event timestamp is invalid")
        event_digest = raw_event.get("event_digest")
        if not _valid_digest(event_digest):
            raise BrainJobError("brain job snapshot event digest is invalid")
        envelope = {
            "schema": JOB_EVENT_SCHEMA,
            "event_type": event_type,
            "job_id": job_id,
            "payload": dict(payload),
            "previous_digest": previous_digest,
            "sequence": expected_sequence,
            "created_ns": created_ns,
        }
        if _digest(envelope) != event_digest:
            raise BrainJobError("brain job snapshot event digest does not match its metadata")
        normalized_event = {
            **envelope,
            "event_digest": event_digest,
            "retention": "metadata_only_hash_chained",
        }
        events.append(normalized_event)
        events_by_sequence[expected_sequence] = normalized_event
        if event_type == "job_submitted":
            submitted.add(job_id)
        previous_digest = event_digest
    head_digest = value.get("head_digest")
    if not isinstance(head_digest, str) or (head_digest and not _valid_digest(head_digest)):
        raise BrainJobError("brain job snapshot head_digest is invalid")
    if head_digest != previous_digest:
        raise BrainJobError("brain job snapshot head_digest is inconsistent")

    raw_jobs = value.get("jobs")
    if not isinstance(raw_jobs, Sequence) or isinstance(raw_jobs, (str, bytes, bytearray)) or len(raw_jobs) > max_jobs:
        raise BrainJobError("brain job snapshot job count is outside its bound")
    job_keys = {
        "schema", "job_id", "idempotency_key", "spec_digest", "domain", "capability", "risk_class",
        "priority", "max_attempts", "state", "attempts", "lease_owner", "lease_expires_ns", "checkpoint",
        "side_effect_boundary", "recovered_after_restart", "reason", "created_ns", "updated_ns",
        "record_sequence", "record_digest", "spec",
    }
    jobs: list[dict[str, Any]] = []
    seen_job_ids: set[str] = set()
    seen_idempotency: set[str] = set()
    for raw_job in raw_jobs:
        if not isinstance(raw_job, Mapping) or set(raw_job) != job_keys:
            raise BrainJobError("brain job snapshot contains an invalid job record")
        if raw_job.get("schema") != JOB_SCHEMA or raw_job.get("spec") != "not_returned; caller resolver owns rehydration":
            raise BrainJobError("brain job snapshot record schema or retention is invalid")
        job_id = _job_text("brain job snapshot job_id", raw_job.get("job_id"), MAX_JOB_ID_BYTES)
        idempotency_key = _job_text("brain job snapshot idempotency_key", raw_job.get("idempotency_key"), MAX_JOB_ID_BYTES)
        if job_id in seen_job_ids or idempotency_key in seen_idempotency:
            raise BrainJobError("brain job snapshot contains duplicate job identity")
        seen_job_ids.add(job_id)
        seen_idempotency.add(idempotency_key)
        spec_digest = raw_job.get("spec_digest")
        if not _valid_digest(spec_digest):
            raise BrainJobError("brain job snapshot spec_digest is invalid")
        domain = _job_text("brain job snapshot domain", raw_job.get("domain"))
        capability = _job_text("brain job snapshot capability", raw_job.get("capability"))
        risk_class = _job_text("brain job snapshot risk_class", raw_job.get("risk_class"))
        priority = raw_job.get("priority")
        if not isinstance(priority, int) or isinstance(priority, bool) or not 0 <= priority <= 255:
            raise BrainJobError("brain job snapshot priority is invalid")
        max_attempts = raw_job.get("max_attempts")
        if not isinstance(max_attempts, int) or isinstance(max_attempts, bool) or not 1 <= max_attempts <= 8:
            raise BrainJobError("brain job snapshot max_attempts is invalid")
        state = raw_job.get("state")
        if not isinstance(state, str) or state not in JOB_STATES:
            raise BrainJobError("brain job snapshot state is invalid")
        attempts = raw_job.get("attempts")
        if not isinstance(attempts, int) or isinstance(attempts, bool) or not 0 <= attempts <= max_attempts:
            raise BrainJobError("brain job snapshot attempts are invalid")
        lease_owner = raw_job.get("lease_owner")
        if lease_owner is not None:
            lease_owner = _job_text("brain job snapshot lease_owner", lease_owner, MAX_JOB_ID_BYTES)
        lease_expires_ns = raw_job.get("lease_expires_ns")
        if lease_expires_ns is not None and (not isinstance(lease_expires_ns, int) or isinstance(lease_expires_ns, bool) or lease_expires_ns < 0):
            raise BrainJobError("brain job snapshot lease expiry is invalid")
        checkpoint = _safe_job_value(raw_job.get("checkpoint"))
        if not isinstance(checkpoint, Mapping) or len(_canonical(checkpoint).encode("utf-8")) > MAX_JOB_CHECKPOINT_BYTES:
            raise BrainJobError("brain job snapshot checkpoint is invalid")
        side_effect_boundary = raw_job.get("side_effect_boundary")
        if not isinstance(side_effect_boundary, str) or side_effect_boundary not in JOB_BOUNDARIES:
            raise BrainJobError("brain job snapshot side_effect_boundary is invalid")
        recovered = raw_job.get("recovered_after_restart")
        if not isinstance(recovered, bool):
            raise BrainJobError("brain job snapshot recovered_after_restart is invalid")
        if state in {"leased", "running"} and (lease_owner is None or lease_expires_ns is None):
            raise BrainJobError("active brain job snapshot state requires a lease")
        if state not in {"leased", "running"} and (lease_owner is not None or lease_expires_ns is not None):
            raise BrainJobError("non-active brain job snapshot state cannot retain a lease")
        reason = raw_job.get("reason")
        if reason is not None:
            reason = _job_text("brain job snapshot reason", reason, MAX_JOB_REASON_BYTES)
        created_ns = raw_job.get("created_ns")
        updated_ns = raw_job.get("updated_ns")
        if any(not isinstance(item, int) or isinstance(item, bool) or item < 0 for item in (created_ns, updated_ns)):
            raise BrainJobError("brain job snapshot timestamps are invalid")
        record_sequence = raw_job.get("record_sequence")
        if not isinstance(record_sequence, int) or isinstance(record_sequence, bool) or not 1 <= record_sequence <= len(events):
            raise BrainJobError("brain job snapshot record_sequence is invalid")
        record_digest = raw_job.get("record_digest")
        if not _valid_digest(record_digest) or events_by_sequence[record_sequence]["event_digest"] != record_digest or events_by_sequence[record_sequence]["job_id"] != job_id:
            raise BrainJobError("brain job snapshot record_digest is not bound to its event")
        if job_id not in submitted:
            raise BrainJobError("brain job snapshot record has no submission event")
        jobs.append({
            "schema": JOB_SCHEMA,
            "job_id": job_id,
            "idempotency_key": idempotency_key,
            "spec_digest": spec_digest,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "priority": priority,
            "max_attempts": max_attempts,
            "state": state,
            "attempts": attempts,
            "lease_owner": lease_owner,
            "lease_expires_ns": lease_expires_ns,
            "checkpoint": dict(checkpoint),
            "side_effect_boundary": side_effect_boundary,
            "recovered_after_restart": recovered,
            "reason": reason,
            "created_ns": created_ns,
            "updated_ns": updated_ns,
            "record_sequence": record_sequence,
            "record_digest": record_digest,
            "spec": "not_returned; caller resolver owns rehydration",
        })
    if {event["job_id"] for event in events}.difference(seen_job_ids):
        raise BrainJobError("brain job snapshot event has no indexed job record")
    descriptor = {
        "schema": JOB_SNAPSHOT_SCHEMA,
        "events": events,
        "jobs": jobs,
        "head_digest": head_digest,
        "retention": "metadata_only_hash_chained",
        "secret_material": "never_returned",
    }
    snapshot_digest = value.get("snapshot_digest")
    if not _valid_digest(snapshot_digest) or _digest(descriptor) != snapshot_digest:
        raise BrainJobError("brain job snapshot digest does not match its metadata")
    normalized = {**descriptor, "snapshot_digest": snapshot_digest}
    if len(_canonical(normalized).encode("utf-8")) > min(max_bytes, MAX_JOB_SNAPSHOT_BYTES):
        raise BrainJobError("brain job snapshot exceeds its byte capacity")
    return normalized


def validate_brain_job_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Public strict validator for metadata-only durable worker snapshots."""

    return _normalize_job_snapshot(value)


class JsonBrainJobSnapshotPersistence:
    """Canonical JSON job persistence over a caller-owned text store."""

    def __init__(self, store: BrainJobSnapshotTextStore, *, max_bytes: int = MAX_JOB_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise BrainJobError("brain job JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_JOB_SNAPSHOT_BYTES:
            raise BrainJobError("brain job JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainJobError("brain job JSON snapshot exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BrainJobError("brain job JSON snapshot is invalid") from error
        if not isinstance(raw, Mapping):
            raise BrainJobError("brain job JSON snapshot must be an object")
        return _normalize_job_snapshot(raw, max_bytes=self.max_bytes)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _normalize_job_snapshot(snapshot, max_bytes=self.max_bytes)
        encoded = _canonical(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainJobError("brain job JSON snapshot exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonBrainJobSnapshotPersistence(JsonBrainJobSnapshotPersistence):
    """Canonical JSON job persistence with compare-and-swap fencing."""

    def __init__(self, store: TransactionalBrainJobSnapshotTextStore, *, max_bytes: int = MAX_JOB_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise BrainJobError("transactional brain job persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None and not _valid_digest(expected_snapshot_digest):
            raise BrainJobError("brain job expected snapshot digest is invalid")
        normalized = _normalize_job_snapshot(snapshot, max_bytes=self.max_bytes)
        encoded = _canonical(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainJobError("brain job JSON snapshot exceeds its byte bound")
        return self.store.write_if_unchanged(expected_snapshot_digest, encoded)


class BrainJobPersistenceCoordinator:
    """Flush and restore a durable worker queue through caller-owned snapshot storage."""

    def __init__(self, store: BrainJobStore, persistence: Any) -> None:
        if not isinstance(store, BrainJobStore):
            raise BrainJobError("brain job persistence requires a BrainJobStore")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise BrainJobError("brain job persistence adapter is malformed")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        raw = self.persistence.read()
        if raw is None:
            self._expected_snapshot_digest = None
            return None
        snapshot = _normalize_job_snapshot(raw, max_jobs=self.store.max_jobs, max_bytes=self.store.max_bytes)
        self.store.restore(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.store.snapshot()
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                raise BrainJobError("brain job persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "BrainJobError",
    "BrainJobEvent",
    "BrainJobEventReceipt",
    "BrainJobRecord",
    "BrainJobPersistenceCoordinator",
    "BrainJobSnapshotTextStore",
    "BrainJobStore",
    "JOB_SNAPSHOT_SCHEMA",
    "MAX_JOB_EVENTS",
    "MAX_JOB_SNAPSHOT_JOBS",
    "MAX_JOB_SNAPSHOT_BYTES",
    "JOB_RECONCILIATION_OUTCOMES",
    "JOB_RECONCILIATION_SCHEMA",
    "JOB_EVENT_SCHEMA",
    "JOB_SCHEMA",
    "JsonBrainJobSnapshotPersistence",
    "TransactionalBrainJobSnapshotTextStore",
    "TransactionalJsonBrainJobSnapshotPersistence",
    "validate_brain_job_snapshot",
]
