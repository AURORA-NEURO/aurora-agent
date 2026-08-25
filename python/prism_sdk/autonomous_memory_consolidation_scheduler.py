"""Restart-safe scheduling for evaluator-gated memory consolidation.

The consolidator is deliberately provider-free, but a production autonomous runtime still
needs a durable boundary between evaluator callbacks and consolidation workers.  This module
provides that boundary without turning it into an implicit learning loop: callers must submit
explicit ``AutonomousMemoryConsolidationObservation`` values, workers only call the configured
consolidator, and snapshots contain bounded metadata and digests rather than prompts, lesson
text, provider output, credentials, or tool arguments.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import re
import time
from typing import Any, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_memory_consolidation import (
    MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES as MAX_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES,
    MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS,
    AutonomousMemoryConsolidationError,
    AutonomousMemoryConsolidationObservation,
    AutonomousMemoryConsolidator,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES


AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SCHEMA = "bioprism-python-autonomous-memory-consolidation-scheduler/0.1"
AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA = "bioprism-python-autonomous-memory-consolidation-scheduler-job/0.1"
AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-memory-consolidation-scheduler-snapshot/0.1"
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS = 4_096
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB = 1_024
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS = 8
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES = MAX_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS = 86_400

_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)
_STATUSES = ("queued", "leased", "completed", "quarantined")
_RETENTION = "metadata_only_evaluator_observations_no_text_payloads_or_provider_values"
_SECRET_MATERIAL = "never_returned"
_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")


class AutonomousMemoryConsolidationSchedulerTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousMemoryConsolidationSchedulerTransactionalTextStore(AutonomousMemoryConsolidationSchedulerTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class AutonomousMemoryConsolidationSchedulerError(AutonomousMemoryConsolidationError):
    """Raised when a scheduler input, lease, replay, or snapshot is unsafe."""


def _fail(message: str) -> None:
    raise AutonomousMemoryConsolidationSchedulerError(f"memory consolidation scheduler {message}")


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 256 or _ID_RE.fullmatch(value) is None:
        _fail(f"{name} is not a bounded identifier")
    return value


def _digest(name: str, value: Any, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_number(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bounds")
    return float(value)


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _canonical_number(value: float | int) -> float | int:
    """Keep integer-valued Python floats identical to JSON numbers in TypeScript."""

    return int(value) if isinstance(value, float) and value.is_integer() else value


def _observation(value: AutonomousMemoryConsolidationObservation | Mapping[str, Any]) -> AutonomousMemoryConsolidationObservation:
    if isinstance(value, AutonomousMemoryConsolidationObservation):
        return value
    if not isinstance(value, Mapping):
        _fail("observation must be a value object")
    # The consolidator's historical to_dict envelope uses the lesson schema for observation
    # rows.  Accept that envelope only after validating its markers, then normalize to the
    # strict observation parser so snapshots cannot smuggle arbitrary fields through a job.
    payload = dict(value)
    envelope = {"schema", "retention", "secret_material"}.intersection(payload)
    if envelope:
        if envelope != {"schema", "retention", "secret_material"} or payload.pop("retention", None) != "metadata_only_lesson_evidence_and_episode_digests_no_text_or_payloads" or payload.pop("secret_material", None) != "never_returned":
            _fail("observation retention markers are invalid")
        payload.pop("schema", None)
    return AutonomousMemoryConsolidationObservation.from_mapping(payload)


def _observation_projection(value: AutonomousMemoryConsolidationObservation) -> dict[str, Any]:
    return {
        "episode_id": value.episode_id, "lesson_id": value.lesson_id, "concept_id": value.concept_id,
        "variant_id": value.variant_id, "domain": value.domain, "capability": value.capability,
        "risk_class": value.risk_class, "evaluator_id": value.evaluator_id,
        "evaluator_version": value.evaluator_version, "reward": _canonical_number(value.reward), "passed": value.passed,
        "evidence_digest": value.evidence_digest, "lesson_digest": value.lesson_digest,
        "decision_digest": value.decision_digest, "observed_at": _canonical_number(value.observed_at),
        "transferable": value.transferable,
    }


def _ordered_domains(values: Sequence[str]) -> tuple[str, ...]:
    unknown = [value for value in values if value not in _DOMAINS]
    if unknown or len(set(values)) != len(values):
        _fail("job domains are malformed")
    return tuple(domain for domain in _DOMAINS if domain in values)


def _lease_digest(job_digest: str, job_id: str, worker_id: str, attempt: int, expires_at: float) -> str:
    return content_digest({"schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SCHEMA, "job_digest": job_digest, "job_id": job_id, "worker_id": worker_id, "attempt": attempt, "lease_expires_at": _canonical_number(expires_at)})


@dataclass(frozen=True, slots=True)
class AutonomousMemoryConsolidationScheduledJob:
    job_id: str
    observations: tuple[AutonomousMemoryConsolidationObservation, ...]
    domains: tuple[str, ...]
    priority: float
    submitted_at: float
    attempts: int
    max_attempts: int
    status: str
    lease_owner: str | None
    lease_expires_at: float | None
    lease_digest: str | None
    report_digest: str | None
    last_error_class: str | None
    job_digest: str

    def __post_init__(self) -> None:
        _identifier("job_id", self.job_id)
        if not self.observations or len(self.observations) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB:
            _fail("job observations exceed their bound")
        if any(not isinstance(item, AutonomousMemoryConsolidationObservation) for item in self.observations):
            _fail("job observations are malformed")
        if self.domains != _ordered_domains(self.domains) or not self.domains:
            _fail("job domains are malformed")
        _bounded_number("job priority", self.priority, 0.0, 1.0)
        _bounded_number("job submitted_at", self.submitted_at, 0.0, 9_223_372_036_854_775.0)
        _bounded_integer("job attempts", self.attempts, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS)
        _bounded_integer("job max_attempts", self.max_attempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS)
        if self.attempts > self.max_attempts:
            _fail("job attempts exceed max_attempts")
        if self.status not in _STATUSES:
            _fail("job status is unsupported")
        _identifier("job lease_owner", self.lease_owner) if self.lease_owner is not None else None
        _bounded_number("job lease_expires_at", self.lease_expires_at, 0.0, 9_223_372_036_854_775.0) if self.lease_expires_at is not None else None
        _digest("job lease_digest", self.lease_digest, optional=True)
        _digest("job report_digest", self.report_digest, optional=True)
        _identifier("job last_error_class", self.last_error_class) if self.last_error_class is not None else None
        _digest("job job_digest", self.job_digest)
        if self.status == "leased" and (self.lease_owner is None or self.lease_expires_at is None or self.lease_digest is None or self.report_digest is not None):
            _fail("leased job lease state is malformed")
        if self.status == "queued" and (self.lease_owner is not None or self.lease_expires_at is not None or self.lease_digest is not None or self.report_digest is not None):
            _fail("queued job lease state is malformed")
        if self.status == "completed" and (self.lease_owner is not None or self.lease_expires_at is not None or self.lease_digest is not None or self.report_digest is None):
            _fail("completed job state is malformed")
        if self.status == "quarantined" and (self.lease_owner is not None or self.lease_expires_at is not None or self.lease_digest is not None or self.report_digest is not None):
            _fail("quarantined job state is malformed")

    @property
    def observation_count(self) -> int:
        return len(self.observations)

    @property
    def age_domains(self) -> tuple[str, ...]:
        return self.domains

    def immutable_projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA,
            "job_id": self.job_id,
            "observations": [_observation_projection(item) for item in self.observations],
            "domains": list(self.domains), "priority": _canonical_number(self.priority), "submitted_at": _canonical_number(self.submitted_at),
            "max_attempts": self.max_attempts,
        }

    def public_projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA,
            "job_id": self.job_id, "observation_count": self.observation_count, "domains": list(self.domains),
            "priority": _canonical_number(self.priority), "submitted_at": _canonical_number(self.submitted_at), "attempts": self.attempts,
            "max_attempts": self.max_attempts, "status": self.status, "lease_owner": self.lease_owner,
            "lease_expires_at": None if self.lease_expires_at is None else _canonical_number(self.lease_expires_at), "lease_digest": self.lease_digest,
            "report_digest": self.report_digest, "last_error_class": self.last_error_class,
            "job_digest": self.job_digest, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL,
        }

    def snapshot_projection(self) -> dict[str, Any]:
        return {**self.public_projection(), "observations": [_observation_projection(item) for item in self.observations]}


@dataclass(frozen=True, slots=True)
class AutonomousMemoryConsolidationClaim:
    job_id: str
    job_digest: str
    worker_id: str
    attempt: int
    lease_expires_at: float
    lease_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id, "job_digest": self.job_digest, "worker_id": self.worker_id,
            "attempt": self.attempt, "lease_expires_at": self.lease_expires_at,
            "lease_digest": self.lease_digest, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL,
        }


def _job_from_snapshot(value: Mapping[str, Any], *, max_observations: int) -> AutonomousMemoryConsolidationScheduledJob:
    if not isinstance(value, Mapping) or set(value) != {
        "schema", "job_id", "observations", "observation_count", "domains", "priority", "submitted_at", "attempts", "max_attempts",
        "status", "lease_owner", "lease_expires_at", "lease_digest", "report_digest", "last_error_class", "job_digest", "retention", "secret_material",
    } or value.get("schema") != AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("snapshot job is malformed")
    raw_observations = value.get("observations")
    if not isinstance(raw_observations, Sequence) or isinstance(raw_observations, (str, bytes, bytearray)) or not raw_observations or len(raw_observations) > max_observations:
        _fail("snapshot job observations are malformed")
    observations = tuple(_observation(item) for item in raw_observations)
    if value.get("observation_count") != len(observations):
        _fail("snapshot job observation_count does not match observations")
    domains = _ordered_domains(value.get("domains")) if isinstance(value.get("domains"), Sequence) and not isinstance(value.get("domains"), (str, bytes, bytearray)) else _fail("snapshot job domains are malformed")
    job = AutonomousMemoryConsolidationScheduledJob(
        job_id=_identifier("snapshot job_id", value.get("job_id")), observations=observations, domains=domains,
        priority=_bounded_number("snapshot job priority", value.get("priority"), 0.0, 1.0),
        submitted_at=_bounded_number("snapshot job submitted_at", value.get("submitted_at"), 0.0, 9_223_372_036_854_775.0),
        attempts=_bounded_integer("snapshot job attempts", value.get("attempts"), 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS),
        max_attempts=_bounded_integer("snapshot job max_attempts", value.get("max_attempts"), 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS),
        status=value.get("status"), lease_owner=value.get("lease_owner"), lease_expires_at=value.get("lease_expires_at"),
        lease_digest=value.get("lease_digest"), report_digest=value.get("report_digest"), last_error_class=value.get("last_error_class"),
        job_digest=_digest("snapshot job_digest", value.get("job_digest")) or "",
    )
    if content_digest(job.immutable_projection()) != job.job_digest:
        _fail("snapshot job digest does not match its immutable projection")
    return job


def _coverage(jobs: Sequence[AutonomousMemoryConsolidationScheduledJob]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for item in _DOMAINS:
        selected = [job for job in jobs if item in job.domains]
        rows.append({
            "domain": item, "job_count": len(selected),
            "observation_count": sum(sum(1 for observation in job.observations if observation.domain == item) for job in selected),
            "queued_job_count": sum(1 for job in selected if job.status == "queued"),
            "leased_job_count": sum(1 for job in selected if job.status == "leased"),
            "completed_job_count": sum(1 for job in selected if job.status == "completed"),
            "quarantined_job_count": sum(1 for job in selected if job.status == "quarantined"),
        })
    return rows


def validate_autonomous_memory_consolidation_scheduler_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    expected_fields = {"schema", "generation", "previous_snapshot_digest", "policy", "jobs", "coverage", "retention", "secret_material", "snapshot_digest"}
    if not isinstance(value, Mapping) or set(value) != expected_fields or value.get("schema") != AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("snapshot is malformed")
    _bounded_integer("snapshot generation", value.get("generation"), 1, 2_147_483_647)
    _digest("snapshot previous_snapshot_digest", value.get("previous_snapshot_digest"), optional=True)
    policy = value.get("policy")
    if not isinstance(policy, Mapping) or set(policy) != {"max_jobs", "max_observations_per_job", "default_max_attempts", "lease_seconds"}:
        _fail("snapshot policy is malformed")
    max_jobs = _bounded_integer("snapshot policy max_jobs", policy.get("max_jobs"), 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS)
    max_observations = _bounded_integer("snapshot policy max_observations_per_job", policy.get("max_observations_per_job"), 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB)
    _bounded_integer("snapshot policy default_max_attempts", policy.get("default_max_attempts"), 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS)
    _bounded_number("snapshot policy lease_seconds", policy.get("lease_seconds"), 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS)
    raw_jobs = value.get("jobs")
    if not isinstance(raw_jobs, Sequence) or isinstance(raw_jobs, (str, bytes, bytearray)) or len(raw_jobs) > max_jobs:
        _fail("snapshot jobs are malformed")
    jobs = [_job_from_snapshot(item, max_observations=max_observations) for item in raw_jobs]
    if len({job.job_id for job in jobs}) != len(jobs):
        _fail("snapshot contains duplicate job identifiers")
    coverage = value.get("coverage")
    if not isinstance(coverage, Sequence) or isinstance(coverage, (str, bytes, bytearray)) or list(coverage) != _coverage(jobs):
        _fail("snapshot domain coverage does not match jobs")
    _digest("snapshot snapshot_digest", value.get("snapshot_digest"))
    descriptor = {"schema": value["schema"], "generation": value["generation"], "previous_snapshot_digest": value["previous_snapshot_digest"], "policy": dict(policy), "jobs": [job.snapshot_projection() for job in sorted(jobs, key=lambda item: item.job_id)], "coverage": list(coverage), "retention": value["retention"], "secret_material": value["secret_material"]}
    if content_digest(descriptor) != value["snapshot_digest"]:
        _fail("snapshot digest does not match its canonical projection")
    return json.loads(canonical_json(value))


class AutonomousMemoryConsolidationScheduler:
    """Bounded deterministic evaluator-observation queue and consolidation worker."""

    def __init__(
        self,
        consolidator: AutonomousMemoryConsolidator,
        *,
        max_jobs: int = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS,
        max_observations_per_job: int = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB,
        default_max_attempts: int = 3,
        lease_seconds: float = 300.0,
    ) -> None:
        if not isinstance(consolidator, AutonomousMemoryConsolidator):
            _fail("consolidator is malformed")
        self.consolidator = consolidator
        self.max_jobs = _bounded_integer("max_jobs", max_jobs, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS)
        self.max_observations_per_job = _bounded_integer("max_observations_per_job", max_observations_per_job, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB)
        self.default_max_attempts = _bounded_integer("default_max_attempts", default_max_attempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS)
        self.lease_seconds = _bounded_number("lease_seconds", lease_seconds, 1.0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS)
        self._jobs: dict[str, AutonomousMemoryConsolidationScheduledJob] = {}
        self._generation = 0
        self._previous_snapshot_digest: str | None = None

    @property
    def policy(self) -> dict[str, Any]:
        return {"max_jobs": self.max_jobs, "max_observations_per_job": self.max_observations_per_job, "default_max_attempts": self.default_max_attempts, "lease_seconds": _canonical_number(self.lease_seconds)}

    def _replace(self, job: AutonomousMemoryConsolidationScheduledJob, **changes: Any) -> AutonomousMemoryConsolidationScheduledJob:
        values = {field: getattr(job, field) for field in job.__dataclass_fields__}
        values.update(changes)
        return AutonomousMemoryConsolidationScheduledJob(**values)

    def submit(self, job_id: str, observations: Sequence[AutonomousMemoryConsolidationObservation | Mapping[str, Any]], *, priority: float = 0.5, submitted_at: float | None = None, max_attempts: int | None = None) -> dict[str, Any]:
        job_id = _identifier("job_id", job_id)
        if not isinstance(observations, Sequence) or isinstance(observations, (str, bytes, bytearray)) or not observations or len(observations) > self.max_observations_per_job:
            _fail("observations exceed their bound")
        normalized = tuple(_observation(item) for item in observations)
        domains = tuple(domain for domain in _DOMAINS if any(item.domain == domain for item in normalized))
        attempts_limit = self.default_max_attempts if max_attempts is None else _bounded_integer("max_attempts", max_attempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS)
        job = AutonomousMemoryConsolidationScheduledJob(job_id, normalized, domains, _bounded_number("priority", priority, 0.0, 1.0), _bounded_number("submitted_at", time.time() if submitted_at is None else submitted_at, 0.0, 9_223_372_036_854_775.0), 0, attempts_limit, "queued", None, None, None, None, None, "0" * 64)
        immutable = job.immutable_projection()
        job = self._replace(job, job_digest=content_digest(immutable))
        existing = self._jobs.get(job_id)
        if existing is not None:
            if existing.job_digest != job.job_digest:
                _fail("job identifier already exists with a different immutable payload")
            return existing.public_projection()
        if len(self._jobs) >= self.max_jobs:
            _fail("job queue is full")
        self._jobs[job_id] = job
        return job.public_projection()

    def get(self, job_id: str) -> dict[str, Any] | None:
        job = self._jobs.get(_identifier("job_id", job_id))
        return None if job is None else job.public_projection()

    def list_jobs(self, *, limit: int = 128) -> list[dict[str, Any]]:
        limit = _bounded_integer("list limit", limit, 1, self.max_jobs)
        return [job.public_projection() for job in sorted(self._jobs.values(), key=lambda item: (item.status != "queued", -item.priority, item.submitted_at, item.job_id))[:limit]]

    def _reclaim_expired(self, now: float) -> None:
        for job in list(self._jobs.values()):
            if job.status != "leased" or job.lease_expires_at is None or job.lease_expires_at > now:
                continue
            if job.attempts >= job.max_attempts:
                self._jobs[job.job_id] = self._replace(job, status="quarantined", lease_owner=None, lease_expires_at=None, lease_digest=None, last_error_class="lease_expired")
            else:
                self._jobs[job.job_id] = self._replace(job, status="queued", lease_owner=None, lease_expires_at=None, lease_digest=None, last_error_class="lease_expired")

    def claim_next(self, worker_id: str, *, now: float | None = None, lease_seconds: float | None = None) -> AutonomousMemoryConsolidationClaim | None:
        worker_id = _identifier("worker_id", worker_id)
        current = _bounded_number("claim now", time.time() if now is None else now, 0.0, 9_223_372_036_854_775.0)
        duration = self.lease_seconds if lease_seconds is None else _bounded_number("claim lease_seconds", lease_seconds, 1.0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS)
        self._reclaim_expired(current)
        queued = [job for job in self._jobs.values() if job.status == "queued" and job.attempts < job.max_attempts]
        if not queued:
            return None
        job = sorted(queued, key=lambda item: (-item.priority, -max(0.0, current - item.submitted_at), item.submitted_at, item.job_id))[0]
        attempt = job.attempts + 1
        expires_at = current + duration
        lease = _lease_digest(job.job_digest, job.job_id, worker_id, attempt, expires_at)
        self._jobs[job.job_id] = self._replace(job, attempts=attempt, status="leased", lease_owner=worker_id, lease_expires_at=expires_at, lease_digest=lease, last_error_class=None)
        return AutonomousMemoryConsolidationClaim(job.job_id, job.job_digest, worker_id, attempt, expires_at, lease)

    def _leased(self, job_id: str, worker_id: str, lease_digest: str, now: float) -> AutonomousMemoryConsolidationScheduledJob:
        job_id = _identifier("job_id", job_id)
        worker_id = _identifier("worker_id", worker_id)
        _digest("lease_digest", lease_digest)
        job = self._jobs.get(job_id)
        if job is None or job.status != "leased" or job.lease_owner != worker_id or job.lease_digest != lease_digest:
            _fail("lease is invalid or no longer owned by the worker")
        if job.lease_expires_at is None or job.lease_expires_at <= now:
            _fail("lease has expired")
        return job

    def complete(self, job_id: str, worker_id: str, lease_digest: str, report_digest: str, *, now: float | None = None) -> dict[str, Any]:
        current = _bounded_number("complete now", time.time() if now is None else now, 0.0, 9_223_372_036_854_775.0)
        _digest("report_digest", report_digest)
        job = self._leased(job_id, worker_id, lease_digest, current)
        self._jobs[job.job_id] = self._replace(job, status="completed", lease_owner=None, lease_expires_at=None, lease_digest=None, report_digest=report_digest, last_error_class=None)
        return self._jobs[job.job_id].public_projection()

    def fail(self, job_id: str, worker_id: str, lease_digest: str, error_class: str, *, now: float | None = None) -> dict[str, Any]:
        current = _bounded_number("fail now", time.time() if now is None else now, 0.0, 9_223_372_036_854_775.0)
        error_class = _identifier("error_class", error_class)
        job = self._leased(job_id, worker_id, lease_digest, current)
        status = "queued" if job.attempts < job.max_attempts else "quarantined"
        self._jobs[job.job_id] = self._replace(job, status=status, lease_owner=None, lease_expires_at=None, lease_digest=None, last_error_class=error_class)
        return self._jobs[job.job_id].public_projection()

    def run_next(self, worker_id: str, *, now: float | None = None) -> dict[str, Any] | None:
        current = time.time() if now is None else now
        claim = self.claim_next(worker_id, now=current)
        if claim is None:
            return None
        job = self._jobs[claim.job_id]
        try:
            report = self.consolidator.consolidate(list(job.observations))
        except Exception:
            row = self.fail(claim.job_id, claim.worker_id, claim.lease_digest, "memory_consolidation_failure", now=current)
            return {"job_id": claim.job_id, "status": row["status"], "attempt": claim.attempt, "error_class": "memory_consolidation_failure", "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}
        row = self.complete(claim.job_id, claim.worker_id, claim.lease_digest, report["report_digest"], now=current)
        return {"job_id": claim.job_id, "status": row["status"], "attempt": claim.attempt, "report_digest": report["report_digest"], "observation_count": row["observation_count"], "domains": row["domains"], "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}

    def run_until_idle(self, worker_id: str, *, max_cycles: int = 64, now: float | None = None) -> dict[str, Any]:
        max_cycles = _bounded_integer("max_cycles", max_cycles, 1, 1_024)
        current = time.time() if now is None else _bounded_number("run_until_idle now", now, 0.0, 9_223_372_036_854_775.0)
        results = []
        for _ in range(max_cycles):
            result = self.run_next(worker_id, now=current)
            if result is None:
                break
            results.append(result)
        return {"worker_id": _identifier("worker_id", worker_id), "cycles": len(results), "idle": len(results) < max_cycles and not any(job["status"] == "queued" for job in self._jobs.values()), "results": results, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}

    def snapshot(self) -> dict[str, Any]:
        self._generation += 1
        descriptor = {
            "schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA, "generation": self._generation,
            "previous_snapshot_digest": self._previous_snapshot_digest, "policy": self.policy,
            "jobs": [job.snapshot_projection() for job in sorted(self._jobs.values(), key=lambda item: item.job_id)],
            "coverage": _coverage(list(self._jobs.values())), "retention": _RETENTION, "secret_material": _SECRET_MATERIAL,
        }
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        return json.loads(canonical_json(snapshot))

    def restore(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        validated = validate_autonomous_memory_consolidation_scheduler_snapshot(snapshot)
        if validated["policy"] != self.policy:
            _fail("restored policy conflicts with the configured scheduler")
        jobs = [_job_from_snapshot(item, max_observations=self.max_observations_per_job) for item in validated["jobs"]]
        self._jobs = {job.job_id: job for job in jobs}
        self._generation = validated["generation"]
        self._previous_snapshot_digest = validated["snapshot_digest"]
        return json.loads(canonical_json(validated))


class JsonAutonomousMemoryConsolidationSchedulerPersistence:
    def __init__(self, text_store: AutonomousMemoryConsolidationSchedulerTextStore, *, max_bytes: int = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(text_store, "read", None)) or not callable(getattr(text_store, "write", None)):
            _fail("JSON text store is malformed")
        self.text_store = text_store
        self.max_bytes = _bounded_integer("JSON max_bytes", max_bytes, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.text_store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        try:
            parsed = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousMemoryConsolidationError("memory consolidation scheduler JSON is invalid") from error
        if canonical_json(parsed) != encoded:
            _fail("JSON snapshot is not canonical")
        return validate_autonomous_memory_consolidation_scheduler_snapshot(parsed)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        encoded = canonical_json(validate_autonomous_memory_consolidation_scheduler_snapshot(snapshot))
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        self.text_store.write(encoded)


class TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence(JsonAutonomousMemoryConsolidationSchedulerPersistence):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected_snapshot_digest", expected_snapshot_digest, optional=True)
        if not callable(getattr(self.text_store, "write_if_unchanged", None)):
            _fail("transactional JSON text store lacks compare-and-swap")
        encoded = canonical_json(validate_autonomous_memory_consolidation_scheduler_snapshot(snapshot))
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("transactional JSON snapshot exceeds its byte bound")
        return bool(self.text_store.write_if_unchanged(expected_snapshot_digest, encoded))


class AutonomousMemoryConsolidationSchedulerPersistenceCoordinator:
    def __init__(self, scheduler: AutonomousMemoryConsolidationScheduler, persistence: JsonAutonomousMemoryConsolidationSchedulerPersistence) -> None:
        if not isinstance(scheduler, AutonomousMemoryConsolidationScheduler) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("persistence coordinator inputs are malformed")
        self.scheduler = scheduler
        self.persistence = persistence
        self.expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            return None
        self.scheduler.restore(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.scheduler.snapshot()
        if isinstance(self.persistence, TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence):
            if not self.persistence.write_if_unchanged(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SCHEMA", "AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA", "AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA",
    "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS",
    "AutonomousMemoryConsolidationSchedulerError", "AutonomousMemoryConsolidationSchedulerTextStore", "AutonomousMemoryConsolidationSchedulerTransactionalTextStore", "AutonomousMemoryConsolidationScheduledJob", "AutonomousMemoryConsolidationClaim", "AutonomousMemoryConsolidationScheduler",
    "JsonAutonomousMemoryConsolidationSchedulerPersistence", "TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence", "AutonomousMemoryConsolidationSchedulerPersistenceCoordinator", "validate_autonomous_memory_consolidation_scheduler_snapshot",
]
