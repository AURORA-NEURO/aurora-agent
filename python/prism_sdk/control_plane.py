"""Cross-process orchestration, model health, approvals, and offline replay.

The autonomous brain has two deliberately separate planes. ``BrainJobStore`` owns the durable
state machine and hash-chained journal; this module adds the bounded control-plane projections that
workers and operators need without turning the journal into a transcript store. Provider/model
telemetry is value-only and content-addressed. Offline replay requires the caller to re-supply an
evidence packet and prove its digest, so the SDK never reconstructs prompts, responses, credentials,
or external effects from retained metadata.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Callable, Mapping, Sequence
import uuid

from .brain import (
    BrainJobRunResult,
    BrainLearningLedger,
    BrainRunError,
    BrainOutcomeEvaluator,
)
from .evaluators import DomainEvaluatorRegistry
from .jobs import BrainJobError, BrainJobEvent, BrainJobRecord, BrainJobStore
from .memory import _canonical, _safe_value, _valid_digest


CONTROL_PLANE_SCHEMA = "bioprism-brain-control-plane/0.1"
MODEL_OBSERVATION_SCHEMA = "bioprism-brain-model-observation/0.1"
MODEL_HEALTH_SCHEMA = "bioprism-brain-model-health/0.1"
REPLAY_CASE_SCHEMA = "bioprism-brain-replay-case/0.1"
REPLAY_REPORT_SCHEMA = "bioprism-brain-replay-report/0.1"
MAX_CONTROL_PAGE = 256
MAX_APPROVAL_SCOPE_BYTES = 512
MAX_REPLAY_CASES = 256
MAX_REPLAY_EVIDENCE_BYTES = 350_000
MAX_MODEL_OBSERVATION_BYTES = 8_192
MAX_MODEL_HEALTH_EVENTS = 100_000
MAX_MODEL_HEALTH_BYTES = 64_000_000
_APPROVAL_STATES = frozenset({"pending", "approved", "denied"})
_MODEL_OUTCOMES = frozenset({"success", "failure", "unknown"})


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return value


def _digest(value: Any) -> str:
    try:
        return hashlib.sha256(_canonical(value).encode("utf-8")).hexdigest()
    except (TypeError, ValueError) as error:
        raise BrainRunError("value cannot be content-addressed") from error


def _safe_mapping(name: str, value: Mapping[str, Any], maximum: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise BrainRunError(f"{name} must be a mapping")
    try:
        safe = _safe_value(value)
        encoded = _canonical(safe).encode("utf-8")
    except Exception as error:
        raise BrainRunError(f"{name} contains unsupported or forbidden content") from error
    if len(encoded) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return dict(safe)


@dataclass(frozen=True, slots=True)
class BrainControlEventPage:
    """Cursor page over the durable job journal."""

    events: tuple[BrainJobEvent, ...]
    after_sequence: int
    next_after: int
    head_digest: str
    has_more: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": CONTROL_PLANE_SCHEMA,
            "events": [event.to_dict() for event in self.events],
            "after_sequence": self.after_sequence,
            "next_after": self.next_after,
            "head_digest": self.head_digest,
            "has_more": self.has_more,
            "retention": "metadata_only_hash_chained",
        }


@dataclass(frozen=True, slots=True)
class BrainApprovalRequest:
    """An approval projection derived from a waiting job checkpoint."""

    approval_id: str
    job_id: str
    request_digest: str
    approval_scope: str
    required_role: str
    state: str
    requested_by: str
    created_ns: int
    decided_by: str | None = None
    decision_reason: str | None = None

    def __post_init__(self) -> None:
        _text("approval_id", self.approval_id, 128)
        _text("approval job_id", self.job_id, 256)
        if not _valid_digest(self.request_digest):
            raise BrainRunError("approval request_digest must be a lowercase SHA-256 digest")
        _text("approval_scope", self.approval_scope, MAX_APPROVAL_SCOPE_BYTES)
        _text("required_role", self.required_role, 128)
        if self.state not in _APPROVAL_STATES:
            raise BrainRunError(f"unknown approval state: {self.state}")
        _text("approval requested_by", self.requested_by, 256)
        if not isinstance(self.created_ns, int) or self.created_ns < 0:
            raise BrainRunError("approval created_ns must be a non-negative integer")
        if self.decided_by is not None:
            _text("approval decided_by", self.decided_by, 256)
        if self.decision_reason is not None:
            _text("approval decision_reason", self.decision_reason, 2_048)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": CONTROL_PLANE_SCHEMA,
            "approval_id": self.approval_id,
            "job_id": self.job_id,
            "request_digest": self.request_digest,
            "approval_scope": self.approval_scope,
            "required_role": self.required_role,
            "state": self.state,
            "requested_by": self.requested_by,
            "created_ns": self.created_ns,
            "decided_by": self.decided_by,
            "decision_reason": self.decision_reason,
            "authorization": "approval_metadata_only; caller owns identity and policy",
        }


class BrainApprovalRouter:
    """Durable approval request/release routing built on the job checkpoint boundary."""

    def __init__(self, store: BrainJobStore, *, clock: Callable[[], float] = time.time) -> None:
        if not isinstance(store, BrainJobStore):
            raise BrainRunError("approval router requires a BrainJobStore")
        if not callable(clock):
            raise BrainRunError("approval clock must be callable")
        self.store = store
        self._clock = clock

    def request(
        self,
        job_id: str,
        worker_id: str,
        *,
        approval_scope: str,
        request_digest: str,
        required_role: str = "operator",
    ) -> BrainApprovalRequest:
        approval_scope = _text("approval_scope", approval_scope, MAX_APPROVAL_SCOPE_BYTES)
        required_role = _text("required_role", required_role, 128)
        if not _valid_digest(request_digest):
            raise BrainRunError("approval request_digest must be a lowercase SHA-256 digest")
        current = self.store.get(job_id)
        if current is None:
            raise BrainRunError("unknown brain job")
        if current.state == "waiting_approval":
            existing = self._from_job(current)
            if existing is not None:
                return existing
        approval_id = uuid.uuid4().hex
        try:
            updated = self.store.checkpoint(
                job_id,
                worker_id,
                phase="approval_requested",
                checkpoint={
                    **dict(current.checkpoint),
                    "approval_id": approval_id,
                    "approval_scope": approval_scope,
                    "request_digest": request_digest,
                    "required_role": required_role,
                    "requested_by": worker_id,
                    "created_ns": self._now_ns(),
                },
                side_effect_boundary=current.side_effect_boundary,
                waiting_for_approval=True,
            )
        except BrainJobError as error:
            raise BrainRunError("approval request could not be recorded") from error
        request = self._from_job(updated)
        if request is None:
            raise BrainRunError("approval request was not retained in the job checkpoint")
        return request

    def approve(self, job_id: str, *, approver: str, reason: str = "caller approval granted") -> BrainApprovalRequest:
        current = self.store.get(job_id)
        request = None if current is None else self._from_job(current)
        if request is None or request.state != "pending":
            raise BrainRunError("job has no pending approval request")
        updated = self.store.resume_waiting(job_id, approver=approver, reason=reason)
        approved = self._from_job(updated, decided_state="approved", decided_by=approver, reason=reason)
        if approved is None:
            raise BrainRunError("approved request could not be reconstructed")
        return approved

    def deny(self, job_id: str, *, approver: str, reason: str = "caller approval denied") -> BrainApprovalRequest:
        current = self.store.get(job_id)
        request = None if current is None else self._from_job(current)
        if request is None or request.state != "pending":
            raise BrainRunError("job has no pending approval request")
        updated = self.store.cancel(job_id, reason=reason)
        denied = self._from_job(updated, decided_state="denied", decided_by=approver, reason=reason)
        if denied is None:
            raise BrainRunError("denied request could not be reconstructed")
        return denied

    def get(self, job_id: str) -> BrainApprovalRequest | None:
        current = self.store.get(job_id)
        return None if current is None else self._from_job(current)

    def pending(self, *, limit: int = 100) -> tuple[BrainApprovalRequest, ...]:
        requests: list[BrainApprovalRequest] = []
        for job in self.store.inventory(limit=limit, state="waiting_approval"):
            request = self._from_job(job)
            if request is not None and request.state == "pending":
                requests.append(request)
        return tuple(requests)

    def _from_job(
        self,
        job: BrainJobRecord,
        *,
        decided_state: str | None = None,
        decided_by: str | None = None,
        reason: str | None = None,
    ) -> BrainApprovalRequest | None:
        checkpoint = job.checkpoint
        required = ("approval_id", "request_digest", "approval_scope", "required_role", "requested_by", "created_ns")
        if any(key not in checkpoint for key in required):
            return None
        if decided_state is not None:
            state = decided_state
        elif job.state == "waiting_approval":
            state = "pending"
        elif checkpoint.get("phase") == "approval_released":
            state = "approved"
        elif job.state == "cancelled":
            state = "denied"
        else:
            return None
        return BrainApprovalRequest(
            approval_id=checkpoint["approval_id"],
            job_id=job.job_id,
            request_digest=checkpoint["request_digest"],
            approval_scope=checkpoint["approval_scope"],
            required_role=checkpoint["required_role"],
            state=state,
            requested_by=checkpoint["requested_by"],
            created_ns=checkpoint["created_ns"],
            decided_by=decided_by or checkpoint.get("approver"),
            decision_reason=reason or job.reason,
        )

    def _now_ns(self) -> int:
        try:
            value = float(self._clock())
        except Exception as error:
            raise BrainRunError("approval clock failed") from error
        if not math.isfinite(value) or value < 0:
            raise BrainRunError("approval clock returned an invalid value")
        return int(value * 1_000_000_000)


class BrainControlPlane:
    """Operator/worker facade over jobs, cursor events, and approval decisions."""

    def __init__(self, store: BrainJobStore) -> None:
        if not isinstance(store, BrainJobStore):
            raise BrainRunError("control plane requires a BrainJobStore")
        self.store = store
        self.approvals = BrainApprovalRouter(store)

    def submit(self, packet: Mapping[str, Any]) -> tuple[BrainJobRecord, Mapping[str, Any]]:
        record, receipt = self.store.submit(packet)
        return record, receipt.to_dict()

    def get(self, job_id: str) -> BrainJobRecord | None:
        return self.store.get(job_id)

    def events(
        self,
        *,
        after_sequence: int = 0,
        job_id: str | None = None,
        limit: int = 100,
    ) -> BrainControlEventPage:
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_CONTROL_PAGE:
            raise BrainRunError(f"control event limit must be within [1, {MAX_CONTROL_PAGE}]")
        rows = self.store.events(
            after_sequence=after_sequence,
            job_id=job_id,
            limit=min(MAX_CONTROL_PAGE, limit + 1),
        )
        visible = rows[:limit]
        next_after = after_sequence if not visible else visible[-1].sequence
        return BrainControlEventPage(
            events=visible,
            after_sequence=after_sequence,
            next_after=next_after,
            head_digest=self.store.head_digest(),
            has_more=len(rows) > limit,
        )


@dataclass(frozen=True, slots=True)
class BrainModelObservation:
    """Value-only provider/model outcome telemetry."""

    provider: str
    model: str
    domain: str
    capability: str
    risk_class: str
    status: str
    outcome: str
    latency_ms: float
    input_tokens: int | None = None
    output_tokens: int | None = None
    failure_class: str | None = None
    quality_reward: float | None = None
    quality_passed: bool | None = None
    outcome_digest: str | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("provider", self.provider),
            ("model", self.model),
            ("domain", self.domain),
            ("capability", self.capability),
            ("risk_class", self.risk_class),
            ("status", self.status),
        ):
            _text(f"model observation {name}", value)
        if self.outcome not in _MODEL_OUTCOMES:
            raise BrainRunError(f"unknown model observation outcome: {self.outcome}")
        if not isinstance(self.latency_ms, (int, float)) or isinstance(self.latency_ms, bool) or not math.isfinite(float(self.latency_ms)) or self.latency_ms < 0:
            raise BrainRunError("model observation latency_ms must be finite and non-negative")
        for name, value in (("input_tokens", self.input_tokens), ("output_tokens", self.output_tokens)):
            if value is not None and (not isinstance(value, int) or isinstance(value, bool) or value < 0):
                raise BrainRunError(f"model observation {name} must be a non-negative integer or None")
        if self.failure_class is not None:
            _text("model observation failure_class", self.failure_class, 128)
        if self.quality_reward is not None and (not isinstance(self.quality_reward, (int, float)) or isinstance(self.quality_reward, bool) or not math.isfinite(float(self.quality_reward))):
            raise BrainRunError("model observation quality_reward must be finite")
        if self.quality_passed is not None and not isinstance(self.quality_passed, bool):
            raise BrainRunError("model observation quality_passed must be boolean or None")
        if self.outcome_digest is not None and not _valid_digest(self.outcome_digest):
            raise BrainRunError("model observation outcome_digest must be a lowercase SHA-256 digest")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "BrainModelObservation":
        safe = _safe_mapping("model observation", value, MAX_MODEL_OBSERVATION_BYTES)
        if safe.get("schema") not in {None, MODEL_OBSERVATION_SCHEMA}:
            raise BrainRunError("model observation schema is unsupported")
        allowed = {
            "schema", "provider", "model", "domain", "capability", "risk_class", "status", "outcome",
            "latency_ms", "input_tokens", "output_tokens", "failure_class", "quality_reward",
            "quality_passed", "outcome_digest", "retention",
        }
        if set(safe).difference(allowed):
            raise BrainRunError("model observation contains unsupported fields")
        return cls(
            provider=safe.get("provider"),
            model=safe.get("model"),
            domain=safe.get("domain"),
            capability=safe.get("capability"),
            risk_class=safe.get("risk_class"),
            status=safe.get("status"),
            outcome=safe.get("outcome"),
            latency_ms=safe.get("latency_ms"),
            input_tokens=safe.get("input_tokens"),
            output_tokens=safe.get("output_tokens"),
            failure_class=safe.get("failure_class"),
            quality_reward=safe.get("quality_reward"),
            quality_passed=safe.get("quality_passed"),
            outcome_digest=safe.get("outcome_digest"),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": MODEL_OBSERVATION_SCHEMA,
            "provider": self.provider,
            "model": self.model,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "status": self.status,
            "outcome": self.outcome,
            "latency_ms": float(self.latency_ms),
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "failure_class": self.failure_class,
            "quality_reward": self.quality_reward,
            "quality_passed": self.quality_passed,
            "outcome_digest": self.outcome_digest,
            "retention": "metadata_only_no_provider_payloads",
        }


@dataclass(frozen=True, slots=True)
class BrainModelHealth:
    provider: str
    model: str
    attempts: int
    successes: int
    failures: int
    unknown: int
    failure_rate: float
    average_latency_ms: float
    last_status: str
    last_outcome: str
    last_sequence: int
    circuit: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": MODEL_HEALTH_SCHEMA,
            "provider": self.provider,
            "model": self.model,
            "attempts": self.attempts,
            "successes": self.successes,
            "failures": self.failures,
            "unknown": self.unknown,
            "failure_rate": self.failure_rate,
            "average_latency_ms": self.average_latency_ms,
            "last_status": self.last_status,
            "last_outcome": self.last_outcome,
            "last_sequence": self.last_sequence,
            "circuit": self.circuit,
            "retention": "aggregated_metadata_only",
        }


class BrainModelHealthStore:
    """SQLite-backed, hash-chained model health telemetry for multiple worker processes."""

    def __init__(
        self,
        path: str | Path,
        *,
        max_events: int = MAX_MODEL_HEALTH_EVENTS,
        max_bytes: int = MAX_MODEL_HEALTH_BYTES,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            raise BrainRunError("model health path must be non-empty")
        if not isinstance(max_events, int) or isinstance(max_events, bool) or max_events <= 0:
            raise BrainRunError("max_events must be positive")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes <= 0:
            raise BrainRunError("max_bytes must be positive")
        if not callable(clock):
            raise BrainRunError("model health clock must be callable")
        self.path = str(path)
        self.max_events = max_events
        self.max_bytes = max_bytes
        self._clock = clock
        self._lock = threading.RLock()
        if self.path != ":memory:":
            Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        self._connection = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
        self._connection.row_factory = sqlite3.Row
        with self._lock:
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS brain_model_health_events (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    payload_json TEXT NOT NULL,
                    previous_digest TEXT NOT NULL,
                    event_digest TEXT NOT NULL UNIQUE,
                    created_ns INTEGER NOT NULL
                );
                """
            )

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "BrainModelHealthStore":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def record(self, observation: BrainModelObservation | Mapping[str, Any]) -> dict[str, Any]:
        normalized = observation if isinstance(observation, BrainModelObservation) else BrainModelObservation.from_mapping(observation)
        payload = normalized.to_dict()
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                count = int(self._connection.execute("SELECT COUNT(*) FROM brain_model_health_events").fetchone()[0])
                if count >= self.max_events:
                    raise BrainRunError("model health event capacity is exhausted")
                previous = self._head_locked()
                created_ns = self._now_ns()
                envelope = {
                    "schema": MODEL_OBSERVATION_SCHEMA,
                    "sequence": count + 1,
                    "payload": payload,
                    "previous_digest": previous,
                    "created_ns": created_ns,
                }
                event_digest = _digest(envelope)
                self._connection.execute(
                    "INSERT INTO brain_model_health_events (payload_json, previous_digest, event_digest, created_ns) VALUES (?, ?, ?, ?)",
                    (_canonical(payload), previous, event_digest, created_ns),
                )
                self._ensure_capacity_locked()
                self._connection.execute("COMMIT")
                return {
                    "schema": MODEL_HEALTH_SCHEMA,
                    "sequence": count + 1,
                    "event_digest": event_digest,
                    "provider": normalized.provider,
                    "model": normalized.model,
                    "retention": "metadata_only",
                }
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def health(
        self,
        *,
        provider: str | None = None,
        model: str | None = None,
        min_attempts: int = 1,
        failure_threshold: float = 0.75,
    ) -> tuple[BrainModelHealth, ...]:
        if provider is not None:
            provider = _text("health provider", provider)
        if model is not None:
            model = _text("health model", model)
        if not isinstance(min_attempts, int) or isinstance(min_attempts, bool) or min_attempts < 1:
            raise BrainRunError("min_attempts must be positive")
        if not isinstance(failure_threshold, (int, float)) or isinstance(failure_threshold, bool) or not 0 <= failure_threshold <= 1:
            raise BrainRunError("failure_threshold must be within [0, 1]")
        with self._lock:
            rows = self._connection.execute("SELECT sequence, payload_json FROM brain_model_health_events ORDER BY sequence ASC").fetchall()
        aggregate: dict[tuple[str, str], dict[str, Any]] = {}
        for row in rows:
            try:
                payload = json.loads(row["payload_json"])
            except (TypeError, ValueError, json.JSONDecodeError) as error:
                raise BrainRunError("model health event contains invalid JSON") from error
            observation = BrainModelObservation.from_mapping(payload)
            if provider is not None and observation.provider != provider:
                continue
            if model is not None and observation.model != model:
                continue
            key = (observation.provider, observation.model)
            entry = aggregate.setdefault(
                key,
                {"attempts": 0, "successes": 0, "failures": 0, "unknown": 0, "latency": 0.0, "last": observation, "sequence": 0},
            )
            entry["attempts"] += 1
            counter_key = {"success": "successes", "failure": "failures", "unknown": "unknown"}[observation.outcome]
            entry[counter_key] += 1
            entry["latency"] += float(observation.latency_ms)
            entry["last"] = observation
            entry["sequence"] = int(row["sequence"])
        result: list[BrainModelHealth] = []
        for (provider_name, model_name), entry in aggregate.items():
            attempts = int(entry["attempts"])
            failures = int(entry["failures"])
            result.append(
                BrainModelHealth(
                    provider=provider_name,
                    model=model_name,
                    attempts=attempts,
                    successes=int(entry["successes"]),
                    failures=failures,
                    unknown=int(entry["unknown"]),
                    failure_rate=failures / attempts if attempts else 0.0,
                    average_latency_ms=float(entry["latency"]) / attempts if attempts else 0.0,
                    last_status=entry["last"].status,
                    last_outcome=entry["last"].outcome,
                    last_sequence=int(entry["sequence"]),
                    circuit=(
                        "open"
                        if attempts >= min_attempts and failures / attempts >= failure_threshold
                        else "closed"
                    ),
                )
            )
        result.sort(key=lambda item: (-item.attempts, item.provider, item.model))
        return tuple(result)

    def provider_health(
        self,
        *,
        min_attempts: int = 2,
        failure_threshold: float = 0.75,
    ) -> dict[str, dict[str, Any]]:
        """Project durable health into the selector's provider-health extension shape."""

        projected: dict[str, dict[str, Any]] = {}
        for row in self.health(min_attempts=min_attempts, failure_threshold=failure_threshold):
            provider = projected.setdefault(
                row.provider,
                {"circuit": "closed", "models": {}, "historical_attempts": 0, "historical_failures": 0},
            )
            provider["models"][row.model] = row.to_dict()
            provider["historical_attempts"] += row.attempts
            provider["historical_failures"] += row.failures
        for provider in projected.values():
            attempts = int(provider["historical_attempts"])
            failures = int(provider["historical_failures"])
            if any(model.get("circuit") == "open" for model in provider["models"].values()):
                provider["circuit"] = "open"
            provider["historical_failure_rate"] = failures / attempts if attempts else 0.0
        return projected

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            rows = self._connection.execute("SELECT * FROM brain_model_health_events ORDER BY sequence ASC").fetchall()
        previous = ""
        for row in rows:
            payload = json.loads(row["payload_json"])
            envelope = {
                "schema": MODEL_OBSERVATION_SCHEMA,
                "sequence": int(row["sequence"]),
                "payload": payload,
                "previous_digest": previous,
                "created_ns": int(row["created_ns"]),
            }
            expected = _digest(envelope)
            if row["previous_digest"] != previous or row["event_digest"] != expected:
                raise BrainRunError(f"model health hash chain breaks at sequence {row['sequence']}")
            previous = expected
        return {"schema": MODEL_HEALTH_SCHEMA, "verified": True, "events": len(rows), "head_digest": previous}

    def _head_locked(self) -> str:
        row = self._connection.execute("SELECT event_digest FROM brain_model_health_events ORDER BY sequence DESC LIMIT 1").fetchone()
        return "" if row is None else str(row["event_digest"])

    def _now_ns(self) -> int:
        try:
            value = float(self._clock())
        except Exception as error:
            raise BrainRunError("model health clock failed") from error
        if not math.isfinite(value) or value < 0:
            raise BrainRunError("model health clock returned an invalid value")
        return int(value * 1_000_000_000)

    def _ensure_capacity_locked(self) -> None:
        page_count = int(self._connection.execute("PRAGMA page_count").fetchone()[0])
        page_size = int(self._connection.execute("PRAGMA page_size").fetchone()[0])
        if page_count * page_size > self.max_bytes:
            raise BrainRunError("model health byte capacity is exhausted")


@dataclass(frozen=True, slots=True)
class BrainReplayCase:
    """Caller-rehydrated evidence for an offline evaluator replay."""

    run_id: str
    domain: str
    capability: str
    risk_class: str
    evaluator_id: str
    evaluator_version: str
    evidence: Mapping[str, Any]
    evidence_digest: str
    expected_decision_digest: str | None = None

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "BrainReplayCase":
        safe = _safe_mapping("replay case", value, MAX_REPLAY_EVIDENCE_BYTES)
        allowed = {
            "schema", "run_id", "domain", "capability", "risk_class", "evaluator_id", "evaluator_version",
            "evidence", "evidence_digest", "expected_decision_digest", "retention",
        }
        if set(safe).difference(allowed):
            raise BrainRunError("replay case contains unsupported fields")
        evidence = safe.get("evidence")
        if not isinstance(evidence, Mapping):
            raise BrainRunError("replay case evidence must be a mapping")
        encoded = _canonical(dict(evidence)).encode("utf-8")
        if len(encoded) > MAX_REPLAY_EVIDENCE_BYTES:
            raise BrainRunError("replay case evidence exceeds its bounded size")
        actual_digest = hashlib.sha256(encoded).hexdigest()
        evidence_digest = safe.get("evidence_digest")
        if evidence_digest != actual_digest:
            raise BrainRunError("replay case evidence_digest does not bind the supplied evidence")
        expected = safe.get("expected_decision_digest")
        if expected is not None and not _valid_digest(expected):
            raise BrainRunError("expected_decision_digest must be a lowercase SHA-256 digest")
        return cls(
            run_id=_text("replay run_id", safe.get("run_id")),
            domain=_text("replay domain", safe.get("domain")),
            capability=_text("replay capability", safe.get("capability")),
            risk_class=_text("replay risk_class", safe.get("risk_class")),
            evaluator_id=_text("replay evaluator_id", safe.get("evaluator_id"), 128),
            evaluator_version=_text("replay evaluator_version", safe.get("evaluator_version"), 128),
            evidence=dict(evidence),
            evidence_digest=evidence_digest,
            expected_decision_digest=expected,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": REPLAY_CASE_SCHEMA,
            "run_id": self.run_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "evidence_digest": self.evidence_digest,
            "expected_decision_digest": self.expected_decision_digest,
            "retention": "caller_rehydrated_evidence_digest_bound",
        }


@dataclass(frozen=True, slots=True)
class BrainReplayReport:
    cases: int
    decisions: tuple[Mapping[str, Any], ...]
    by_domain: Mapping[str, Mapping[str, Any]]
    disagreement_count: int
    next_bandit_state: Mapping[str, Any] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": REPLAY_REPORT_SCHEMA,
            "status": "completed",
            "cases": self.cases,
            "decisions": [dict(decision) for decision in self.decisions],
            "by_domain": {key: dict(value) for key, value in self.by_domain.items()},
            "disagreement_count": self.disagreement_count,
            "next_bandit_state": None if self.next_bandit_state is None else dict(self.next_bandit_state),
            "retention": "decision_metadata_and_digests_only",
        }


class BrainReplayEngine:
    """Replay caller-supplied evidence through the same evaluator and adaptation boundary."""

    def replay(
        self,
        cases: Sequence[BrainReplayCase | Mapping[str, Any]],
        *,
        evaluators: DomainEvaluatorRegistry | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any]], Mapping[str, Any]] | None = None,
    ) -> BrainReplayReport:
        if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)):
            raise BrainRunError("replay cases must be a sequence")
        if not 1 <= len(cases) <= MAX_REPLAY_CASES:
            raise BrainRunError(f"replay cases must contain 1..{MAX_REPLAY_CASES} entries")
        registry = evaluators or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("evaluators must be a DomainEvaluatorRegistry")
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise BrainRunError("bandit_state must be a mapping or None")
            BrainLearningLedger._assert_safe(bandit_state)
        if bandit_updater is not None and not callable(bandit_updater):
            raise BrainRunError("bandit_updater must be callable or None")
        current_state = None if bandit_state is None else dict(bandit_state)
        decisions: list[dict[str, Any]] = []
        aggregates: dict[str, dict[str, Any]] = {}
        disagreements = 0
        for raw_case in cases:
            case = raw_case if isinstance(raw_case, BrainReplayCase) else BrainReplayCase.from_mapping(raw_case)
            evaluator = registry.resolve_for_replay(
                case.domain,
                evaluator_id=case.evaluator_id,
                evaluator_version=case.evaluator_version,
            )
            if evaluator.evaluator_id != case.evaluator_id or evaluator.evaluator_version != case.evaluator_version:
                raise BrainRunError("replay case evaluator identity does not match the registered domain evaluator")
            decision = evaluator.assess_value_only_input(
                {
                    "schema": "bioprism-brain-evaluator-input/0.1",
                    "result_kind": "offline_replay",
                    "run_id": case.run_id,
                    "domain": case.domain,
                    "capability": case.capability,
                    "risk_class": case.risk_class,
                    "evidence_digest": case.evidence_digest,
                    "evidence": dict(case.evidence),
                }
            )
            decision_digest = _digest(decision.to_dict())
            disagreement = case.expected_decision_digest is not None and case.expected_decision_digest != decision_digest
            if disagreement:
                disagreements += 1
            row = {
                "run_id": case.run_id,
                "domain": case.domain,
                "capability": case.capability,
                "risk_class": case.risk_class,
                "evaluator_id": decision.evaluator_id,
                "evaluator_version": decision.evaluator_version,
                "reward": decision.reward,
                "passed": decision.passed,
                "failed": decision.failed,
                "failure_class": decision.failure_class,
                "evidence_digest": case.evidence_digest,
                "decision_digest": decision_digest,
                "expected_decision_digest": case.expected_decision_digest,
                "disagreement": disagreement,
            }
            decisions.append(row)
            aggregate = aggregates.setdefault(case.domain, {"cases": 0, "passed": 0, "failed": 0, "reward_total": 0.0})
            aggregate["cases"] += 1
            aggregate["passed"] += int(decision.passed)
            aggregate["failed"] += int(decision.failed)
            aggregate["reward_total"] += float(decision.reward)
            if bandit_updater is not None:
                update_input = {
                    "schema": REPLAY_REPORT_SCHEMA,
                    "run_id": case.run_id,
                    "domain": case.domain,
                    "capability": case.capability,
                    "risk_class": case.risk_class,
                    "arm_id": f"{case.domain}/{case.capability}",
                    "reward": decision.reward,
                    "passed": decision.passed,
                    "failure_class": decision.failure_class,
                    "evidence_digest": case.evidence_digest,
                    "decision_digest": decision_digest,
                    "bandit_state": {} if current_state is None else dict(current_state),
                }
                updated = bandit_updater(update_input)
                if not isinstance(updated, Mapping) or not isinstance(updated.get("next_state"), Mapping):
                    raise BrainRunError("bandit_updater must return a mapping with next_state")
                BrainLearningLedger._assert_safe(updated)
                current_state = dict(updated["next_state"])
        by_domain: dict[str, dict[str, Any]] = {}
        for domain, aggregate in aggregates.items():
            count = int(aggregate["cases"])
            by_domain[domain] = {
                "cases": count,
                "passed": int(aggregate["passed"]),
                "failed": int(aggregate["failed"]),
                "pass_rate": int(aggregate["passed"]) / count if count else 0.0,
                "mean_reward": float(aggregate["reward_total"]) / count if count else 0.0,
            }
        return BrainReplayReport(
            cases=len(decisions),
            decisions=tuple(decisions),
            by_domain=by_domain,
            disagreement_count=disagreements,
            next_bandit_state=current_state,
        )


class BrainWorker:
    """A restart-safe worker process facade with optional lease heartbeat and health recording."""

    def __init__(
        self,
        brain: Any,
        store: BrainJobStore,
        *,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: BrainOutcomeEvaluator | None,
        bandit_state: Mapping[str, Any],
        ledger: BrainLearningLedger | None = None,
        memory: Any | None = None,
        health: BrainModelHealthStore | None = None,
        approval_router: BrainApprovalRouter | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
        lease_seconds: float = 300.0,
        heartbeat_seconds: float = 30.0,
        execution_kind: str = "mission_learning",
        workflow_checkpoint_sink: Callable[[str, Any], Any] | None = None,
    ) -> None:
        if execution_kind not in {"mission_learning", "workflow_learning"}:
            raise BrainRunError("worker execution_kind must be mission_learning or workflow_learning")
        required_method = (
            "run_resumable_learning_job"
            if execution_kind == "mission_learning"
            else "run_resumable_workflow_job"
        )
        if not hasattr(brain, required_method):
            raise BrainRunError(f"worker brain must expose {required_method}")
        if not isinstance(store, BrainJobStore):
            raise BrainRunError("worker requires a BrainJobStore")
        self.brain = brain
        self.store = store
        self.worker_id = _text("worker_id", worker_id)
        if not callable(resolver):
            raise BrainRunError("worker resolver must be callable")
        if execution_kind == "mission_learning" and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("worker evaluator must be a BrainOutcomeEvaluator")
        if execution_kind == "workflow_learning" and evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("workflow worker evaluator must be a BrainOutcomeEvaluator or None")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("worker bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if health is not None and not isinstance(health, BrainModelHealthStore):
            raise BrainRunError("worker health must be a BrainModelHealthStore or None")
        if approval_router is not None and not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("worker approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > MAX_APPROVAL_SCOPE_BYTES
        ):
            raise BrainRunError("worker approval_scope must be a bounded non-empty string or None")
        required_approval_role = _text("worker required_approval_role", required_approval_role, 128)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("worker lease_seconds must be within [1, 86400]")
        if not isinstance(heartbeat_seconds, (int, float)) or isinstance(heartbeat_seconds, bool) or not 0.1 <= heartbeat_seconds < lease_seconds:
            raise BrainRunError("worker heartbeat_seconds must be within [0.1, lease_seconds)")
        if workflow_checkpoint_sink is not None and not callable(workflow_checkpoint_sink):
            raise BrainRunError("workflow_checkpoint_sink must be callable or None")
        self.resolver = resolver
        self.evaluator = evaluator
        self.bandit_state = dict(bandit_state)
        self.ledger = ledger
        self.memory = memory
        self.health = health
        self.approvals = approval_router or BrainApprovalRouter(store)
        self.approval_scope = approval_scope
        self.required_approval_role = required_approval_role
        self.lease_seconds = float(lease_seconds)
        self.heartbeat_seconds = float(heartbeat_seconds)
        self.execution_kind = execution_kind
        self.workflow_checkpoint_sink = workflow_checkpoint_sink

    def run_once(self, job_id: str | None = None) -> BrainJobRunResult | None:
        if job_id is None:
            queued = self.store.inventory(limit=1, state="queued")
            if not queued:
                return None
            job_id = queued[0].job_id
        claimed = self.store.claim(job_id, self.worker_id, lease_seconds=self.lease_seconds)
        if claimed.terminal:
            return BrainJobRunResult(status="already_terminal", job=claimed.to_dict(), cycle=None, workflow=None)
        stop = threading.Event()
        heartbeat_errors: list[Exception] = []

        def heartbeat() -> None:
            while not stop.wait(self.heartbeat_seconds):
                try:
                    self.store.renew(job_id, self.worker_id, lease_seconds=self.lease_seconds)
                except Exception as error:  # the main operation remains the source of truth
                    heartbeat_errors.append(error)
                    stop.set()

        thread = threading.Thread(target=heartbeat, name=f"aurora-brain-heartbeat-{self.worker_id}", daemon=True)
        started = time.perf_counter()
        thread.start()
        result: BrainJobRunResult | None = None
        runtime = getattr(self.brain, "runtime", None)
        observation_callback: Callable[[Mapping[str, Any]], None] | None = None
        if self.health is not None and runtime is not None and hasattr(runtime, "add_observation_callback"):
            def observe_provider_failure(payload: Mapping[str, Any]) -> None:
                if payload.get("outcome") != "failure":
                    return
                provider = payload.get("provider")
                model = payload.get("model")
                status = payload.get("status")
                if not all(isinstance(value, str) and value.strip() for value in (provider, model, status)):
                    return
                self.health.record(
                    BrainModelObservation(
                        provider=provider,
                        model=model,
                        domain=claimed.domain,
                        capability=claimed.capability,
                        risk_class=claimed.risk_class,
                        status=status,
                        outcome="failure",
                        latency_ms=payload.get("latency_ms", 0.0),
                        input_tokens=payload.get("input_tokens"),
                        output_tokens=payload.get("output_tokens"),
                        failure_class=payload.get("failure_class", "provider_error"),
                    )
                )

            observation_callback = observe_provider_failure
            runtime.add_observation_callback(observation_callback)
        operation_error: Exception | None = None
        try:
            common = {
                "job_id": job_id,
                "worker_id": self.worker_id,
                "resolver": self.resolver,
                "evaluator": self.evaluator,
                "bandit_state": self.bandit_state,
                "provider_health": None if self.health is None else self.health.provider_health(),
                "ledger": self.ledger,
                "memory": self.memory,
                "approval_router": self.approvals,
                "approval_scope": self.approval_scope,
                "required_approval_role": self.required_approval_role,
                "lease_seconds": self.lease_seconds,
            }
            if self.execution_kind == "workflow_learning":
                common["checkpoint_sink"] = self.workflow_checkpoint_sink
                result = self.brain.run_resumable_workflow_job(self.store, **common)
            else:
                result = self.brain.run_resumable_learning_job(self.store, **common)
        except Exception as error:
            operation_error = error
        finally:
            if observation_callback is not None and hasattr(runtime, "remove_observation_callback"):
                runtime.remove_observation_callback(observation_callback)
            stop.set()
            thread.join(timeout=max(1.0, self.heartbeat_seconds))
        if operation_error is not None:
            try:
                current = self.store.get(job_id)
                if current is not None and current.lease_owner == self.worker_id and current.state in {"leased", "running"}:
                    self.store.checkpoint(
                        job_id,
                        self.worker_id,
                        phase="worker_execution_error",
                        checkpoint={"error_class": type(operation_error).__name__},
                        side_effect_boundary="unknown",
                    )
                    failed = self.store.fail(
                        job_id,
                        self.worker_id,
                        reason="worker execution raised; reconciliation required",
                        retryable=False,
                    )
                    return BrainJobRunResult(
                        status=failed.state,
                        job=failed.to_dict(),
                        cycle=None,
                        error_class=type(operation_error).__name__,
                    )
            except Exception as persistence_error:
                raise BrainRunError("worker failure could not be durably recorded") from persistence_error
            raise operation_error
        if result is None:
            raise BrainRunError("worker execution returned no result")
        if result.workflow is not None:
            next_state = getattr(result.workflow, "bandit_state", None)
            if isinstance(next_state, Mapping):
                self.bandit_state = dict(next_state)
            if self.health is not None:
                for stage in result.workflow.workflow.stage_results:
                    if stage.result is None:
                        continue
                    brain_result = getattr(stage.result, "brain_run", stage.result)
                    selection = getattr(brain_result, "selection", {})
                    selected = selection.get("selected_model") if isinstance(selection, Mapping) else None
                    if not isinstance(selected, Mapping):
                        continue
                    provider = selected.get("provider")
                    model = selected.get("model")
                    if not isinstance(provider, str) or not isinstance(model, str):
                        continue
                    response = getattr(brain_result, "response", None)
                    usage = getattr(response, "usage", {}) if response is not None else {}
                    decision = next(
                        (
                            evaluation.decision.to_dict()
                            for evaluation in result.workflow.evaluations
                            if evaluation.stage_id == stage.stage.id
                        ),
                        {},
                    )
                    stage_outcome = "success" if stage.execution_status == "completed" else (
                        "unknown" if stage.execution_status == "approval_required" else "failure"
                    )
                    self.health.record(
                        BrainModelObservation(
                            provider=provider,
                            model=model,
                            domain=claimed.domain,
                            capability=claimed.capability,
                            risk_class=claimed.risk_class,
                            status=stage.execution_status,
                            outcome=stage_outcome,
                            latency_ms=(time.perf_counter() - started) * 1000.0,
                            input_tokens=usage.get("input_tokens") if isinstance(usage, Mapping) else None,
                            output_tokens=usage.get("output_tokens") if isinstance(usage, Mapping) else None,
                            quality_reward=decision.get("reward") if isinstance(decision, Mapping) else None,
                            quality_passed=decision.get("passed") if isinstance(decision, Mapping) else None,
                            outcome_digest=getattr(brain_result, "outcome_digest", None),
                        )
                    )
        if self.health is not None and result.cycle is not None:
            final = result.cycle.final_result
            outcome = (
                "success"
                if result.status == "succeeded"
                else "unknown"
                if result.status == "waiting_approval"
                else "failure"
            )
            selected = final.brain_run.selection.get("selected_model")
            if isinstance(selected, Mapping) and isinstance(selected.get("provider"), str) and isinstance(selected.get("model"), str):
                response = final.brain_run.response
                decision = result.cycle.evaluations[-1].get("decision", {}) if result.cycle.evaluations else {}
                failover = final.brain_run.provider_failover
                if isinstance(failover, Mapping) and isinstance(failover.get("attempts"), Sequence):
                    for attempt in failover["attempts"]:
                        if not isinstance(attempt, Mapping) or attempt.get("status") == "completed":
                            continue
                        provider = attempt.get("provider")
                        model = attempt.get("model")
                        if not isinstance(provider, str) or not isinstance(model, str):
                            continue
                        failure_class = attempt.get("reason")
                        self.health.record(
                            BrainModelObservation(
                                provider=provider,
                                model=model,
                                domain=claimed.domain,
                                capability=claimed.capability,
                                risk_class=claimed.risk_class,
                                status=str(attempt.get("status")),
                                outcome="failure",
                                latency_ms=0.0,
                                failure_class=failure_class if isinstance(failure_class, str) else "provider_failover",
                            )
                        )
                self.health.record(
                    BrainModelObservation(
                        provider=selected["provider"],
                        model=selected["model"],
                        domain=claimed.domain,
                        capability=claimed.capability,
                        risk_class=claimed.risk_class,
                        status=final.status,
                        outcome=outcome,
                        latency_ms=(time.perf_counter() - started) * 1000.0,
                        input_tokens=None if response is None else response.usage.get("input_tokens"),
                        output_tokens=None if response is None else response.usage.get("output_tokens"),
                        quality_reward=decision.get("reward") if isinstance(decision, Mapping) else None,
                        quality_passed=decision.get("passed") if isinstance(decision, Mapping) else None,
                        outcome_digest=final.brain_run.outcome_digest,
                    )
                )
        if heartbeat_errors and result.status == "succeeded":
            # The job result is retained, but make the lease anomaly visible to the caller. A
            # later reconciliation query can decide whether the external state is trustworthy.
            return BrainJobRunResult(
                status="heartbeat_anomaly",
                job=result.job,
                cycle=result.cycle,
                error_class=type(heartbeat_errors[0]).__name__,
                workflow=result.workflow,
            )
        return result

    def run_available(self, *, max_jobs: int = 1) -> tuple[BrainJobRunResult, ...]:
        if not isinstance(max_jobs, int) or isinstance(max_jobs, bool) or not 1 <= max_jobs <= MAX_CONTROL_PAGE:
            raise BrainRunError(f"max_jobs must be within [1, {MAX_CONTROL_PAGE}]")
        results: list[BrainJobRunResult] = []
        for _ in range(max_jobs):
            result = self.run_once()
            if result is None:
                break
            results.append(result)
        return tuple(results)


__all__ = [
    "BrainApprovalRequest",
    "BrainApprovalRouter",
    "BrainControlEventPage",
    "BrainControlPlane",
    "BrainModelHealth",
    "BrainModelHealthStore",
    "BrainModelObservation",
    "BrainReplayCase",
    "BrainReplayEngine",
    "BrainReplayReport",
    "BrainWorker",
    "CONTROL_PLANE_SCHEMA",
    "MODEL_HEALTH_SCHEMA",
    "MODEL_OBSERVATION_SCHEMA",
    "REPLAY_CASE_SCHEMA",
    "REPLAY_REPORT_SCHEMA",
]
