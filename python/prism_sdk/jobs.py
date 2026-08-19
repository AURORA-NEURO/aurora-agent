"""Restart-safe orchestration journal for autonomous brain learning jobs.

This store is intentionally not an execution engine and not a credential store. It persists only
the metadata needed to coordinate a caller-owned resolver: digests, domain labels, leases,
attempt counters, checkpoints, and recovery decisions. A resolver rehydrates the actual task,
prompt, plan, evaluator, and BYOK handles after a process restart. If a lease expires after the
external-effect boundary, the job is quarantined for reconciliation instead of being replayed.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Callable, Mapping, Sequence
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
_BOUNDARY_ORDER = {value: index for index, value in enumerate(JOB_BOUNDARIES)}


class BrainJobError(RuntimeError):
    """A job submission, lease, checkpoint, or recovery operation was refused."""


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
                event = self._append_event_locked(
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

    def fail(self, job_id: str, worker_id: str, *, reason: str, retryable: bool = False) -> BrainJobRecord:
        reason = _job_text("job failure reason", reason, MAX_JOB_REASON_BYTES)
        if not isinstance(retryable, bool):
            raise BrainJobError("retryable must be boolean")
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
                self._transition_locked(
                    record,
                    event_type=event_type,
                    state=state,
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint={"phase": "failed", "reason": reason},
                )
                self._connection.execute("COMMIT")
                return self._row_to_record(
                    self._connection.execute("SELECT * FROM brain_jobs WHERE job_id = ?", (record.job_id,)).fetchone()
                )
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def cancel(self, job_id: str, *, reason: str = "cancelled by caller") -> BrainJobRecord:
        job_id = _job_text("job_id", job_id, MAX_JOB_ID_BYTES)
        reason = _job_text("cancellation reason", reason, MAX_JOB_REASON_BYTES)
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
                self._transition_locked(
                    record,
                    event_type="job_cancelled",
                    state="cancelled",
                    reason=reason,
                    lease_owner=None,
                    lease_expires_ns=None,
                    checkpoint={
                        **dict(record.checkpoint),
                        "phase": "cancelled",
                        "reason": reason,
                    },
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
    ) -> BrainJobEventReceipt:
        if state not in JOB_STATES:
            raise BrainJobError(f"unknown job state: {state}")
        if reason is not None:
            reason = _job_text("job reason", reason, MAX_JOB_REASON_BYTES)
        next_boundary = record.side_effect_boundary if side_effect_boundary is None else side_effect_boundary
        if next_boundary not in JOB_BOUNDARIES or _BOUNDARY_ORDER[next_boundary] < _BOUNDARY_ORDER[record.side_effect_boundary]:
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


__all__ = [
    "BrainJobError",
    "BrainJobEvent",
    "BrainJobEventReceipt",
    "BrainJobRecord",
    "BrainJobStore",
    "JOB_EVENT_SCHEMA",
    "JOB_SCHEMA",
]
