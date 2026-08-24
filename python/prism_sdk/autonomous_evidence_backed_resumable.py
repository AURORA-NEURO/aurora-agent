"""Restart-safe execution for reviewed evidence-backed autonomous runs.

The one-shot evidence bridge deliberately keeps source values and provider results transient.
This module adds the process boundary around that bridge: a bounded, digest-verified checkpoint,
caller-owned evidence journal rehydration, explicit provider resume, and an optional compare-and-
swap persistence adapter.  It never serializes task text, requests, evidence bodies, prompts,
credentials, or provider responses.

The provider boundary is treated as a quarantine point.  A checkpoint written immediately before
dispatch is ``provider_pending``; an observed provider outcome that cannot safely be replayed is
``provider_reconciliation_required``.  Only an explicit caller rehydration callback or an
explicit ``resume_provider=True`` decision can cross that boundary after restart.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from threading import Lock
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_runtime import AutonomousEvidenceRuntimeJournal
from .autonomous_evidence_brain import (
    AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
    AutonomousEvidenceBackedPreflight,
    AutonomousEvidenceBackedRunResult,
    _bounded_domains,
    _bounded_task,
    _bounded_requests,
    _execution_metadata,
    run_autonomous_evidence_backed,
)
from .errors import ArgumentError
from .brain import BrainRunError


AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA = "bioprism-python-autonomous-evidence-backed-checkpoint/0.1"
AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-backed-resumable-result/0.1"
AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA = "bioprism-python-autonomous-evidence-backed-controller/0.1"
MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES = 64_000
AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES = (
    "evidence_review_required",
    "evidence_incomplete",
    "evidence_failed",
    "evidence_reconciliation_required",
    "provider_pending",
    "provider_reconciliation_required",
    "completed",
)
AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_STATUSES = (
    *AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES,
)
_RETENTION = "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned"
_RESULT_RETENTION = "metadata_only;raw_evidence_and_provider_payloads_caller_owned"
_CONTROLLER_RETENTION = "metadata_only_task_request_evidence_and_provider_payloads_caller_owned"
_SECRET_MATERIAL = "never_returned"


def _identifier(name: str, value: Any) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value.encode("utf-8")) > 256
        or "\x00" in value
        or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in value)
    ):
        raise ArgumentError(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _identifier(name, value)


def _json_bytes(value: Any, name: str) -> int:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return len(encoded)


def _policy_value(value: Any, *, depth: int = 0) -> Any:
    """Project run policy objects without retaining their transient values."""

    if depth > 12:
        raise ArgumentError("evidence-backed run policy is too deeply nested")
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, Mapping):
        return {str(key): _policy_value(child, depth=depth + 1) for key, child in sorted(value.items(), key=lambda item: str(item[0]))}
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 256:
            raise ArgumentError("evidence-backed run policy contains too many entries")
        return [_policy_value(child, depth=depth + 1) for child in value]
    serializer = getattr(value, "to_dict", None)
    if callable(serializer):
        return _policy_value(serializer(), depth=depth + 1)
    if callable(value):
        return {"callable_type": value.__class__.__name__}
    return {"object_type": value.__class__.__name__}


def _request_digest(requests: Sequence[Mapping[str, Any]]) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
            "requests": [_policy_value(request) for request in requests],
        }
    )


def _model_policy_value(candidates: Sequence[Any] | None) -> Any:
    if candidates is None:
        return None
    return [_policy_value(candidate.to_dict() if hasattr(candidate, "to_dict") else candidate) for candidate in candidates]


def _run_policy_digest(
    *,
    domains: Sequence[str],
    model_candidates: Sequence[Any] | None,
    run_mode: str,
    run_options: Mapping[str, Any] | None,
    approve_source_dispatch: bool,
    allow_incomplete_evidence: bool,
    prompt_builder: Any,
    evaluator: Any,
    available_evidence: Sequence[str],
    completed_stages: Mapping[str, Sequence[str]] | None,
    parent_evidence_digests: Sequence[str],
    stop_on_failure: bool,
    reevaluate_pending: bool,
) -> str:
    evaluator_identity = None
    if evaluator is not None:
        evaluator_identity = {
            "evaluator_id": getattr(evaluator, "evaluator_id", evaluator.__class__.__name__),
            "evaluator_version": getattr(evaluator, "evaluator_version", "unknown"),
        }
    payload = {
        "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
        "domains": list(domains),
        "model_candidates": _model_policy_value(model_candidates),
        "run_mode": run_mode,
        "run_options": _policy_value({} if run_options is None else run_options),
        "approve_source_dispatch": approve_source_dispatch,
        "allow_incomplete_evidence": allow_incomplete_evidence,
        "prompt_builder_configured": prompt_builder is not None,
        "evaluator": _policy_value(evaluator_identity),
        "available_evidence": list(available_evidence),
        "completed_stages": _policy_value({} if completed_stages is None else completed_stages),
        "parent_evidence_digests": list(parent_evidence_digests),
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
    }
    _json_bytes(payload, "evidence-backed run policy")
    return content_digest(payload)


def _execution_plan_digest(task_digest: str, evidence_plan_digest: str, domains: Sequence[str], run_mode: str) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "task_digest": task_digest,
            "evidence_plan_digest": evidence_plan_digest,
            "domains": list(domains),
            "run_mode": run_mode,
        }
    )


def _provider_result_was_observed(status: str | None) -> bool:
    return status is not None and status not in {"approval_required", "route_review_required", "abstained"} and not status.endswith("review_required")


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedCheckpoint:
    """Digest-bound metadata-only restart state for one evidence-backed run."""

    job_id: str
    task_digest: str
    request_digest: str
    run_policy_digest: str
    evidence_plan_digest: str
    execution_plan_digest: str
    evidence_result_digest: str | None
    prompt_projection_digest: str | None
    provider_result_digest: str | None
    provider_status: str | None
    status: str

    def __post_init__(self) -> None:
        _identifier("evidence-backed checkpoint job_id", self.job_id)
        for name, value in (
            ("task_digest", self.task_digest),
            ("request_digest", self.request_digest),
            ("run_policy_digest", self.run_policy_digest),
            ("evidence_plan_digest", self.evidence_plan_digest),
            ("execution_plan_digest", self.execution_plan_digest),
            ("evidence_result_digest", self.evidence_result_digest),
            ("prompt_projection_digest", self.prompt_projection_digest),
            ("provider_result_digest", self.provider_result_digest),
        ):
            _digest(f"evidence-backed checkpoint {name}", value, allow_none=name in {"evidence_result_digest", "prompt_projection_digest", "provider_result_digest"})
        _optional_text("evidence-backed checkpoint provider_status", self.provider_status)
        if self.status not in AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES:
            raise ArgumentError("evidence-backed checkpoint status is invalid")
        if self.status == "completed" and (self.provider_result_digest is None or self.provider_status != "completed"):
            raise ArgumentError("completed evidence-backed checkpoint requires a completed provider digest")
        if self.status == "provider_reconciliation_required" and (self.provider_result_digest is None or self.provider_status is None):
            raise ArgumentError("provider reconciliation checkpoint requires a provider result digest")
        if self.status in {"evidence_review_required", "evidence_incomplete", "evidence_failed", "evidence_reconciliation_required"} and (self.provider_result_digest is not None or self.provider_status is not None):
            raise ArgumentError("evidence-only checkpoint cannot contain provider result metadata")
        if self.status == "provider_pending" and self.provider_result_digest is not None:
            raise ArgumentError("provider-pending checkpoint cannot contain a provider result digest")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
            "job_id": self.job_id,
            "task_digest": self.task_digest,
            "request_digest": self.request_digest,
            "run_policy_digest": self.run_policy_digest,
            "evidence_plan_digest": self.evidence_plan_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "evidence_result_digest": self.evidence_result_digest,
            "prompt_projection_digest": self.prompt_projection_digest,
            "provider_result_digest": self.provider_result_digest,
            "provider_status": self.provider_status,
            "status": self.status,
        }

    @property
    def checkpoint_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        value = {
            **self._payload(),
            "checkpoint_digest": self.checkpoint_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        _json_bytes(value, "evidence-backed checkpoint")
        return value

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceBackedCheckpoint":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence-backed checkpoint must be a mapping")
        expected = {
            "schema", "job_id", "task_digest", "request_digest", "run_policy_digest",
            "evidence_plan_digest", "execution_plan_digest", "evidence_result_digest",
            "prompt_projection_digest", "provider_result_digest", "provider_status", "status",
            "checkpoint_digest", "retention", "secret_material",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA:
            raise ArgumentError("evidence-backed checkpoint contains unsupported or missing fields")
        if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("evidence-backed checkpoint retention markers are invalid")
        checkpoint = cls(
            job_id=value.get("job_id"),
            task_digest=value.get("task_digest"),
            request_digest=value.get("request_digest"),
            run_policy_digest=value.get("run_policy_digest"),
            evidence_plan_digest=value.get("evidence_plan_digest"),
            execution_plan_digest=value.get("execution_plan_digest"),
            evidence_result_digest=value.get("evidence_result_digest"),
            prompt_projection_digest=value.get("prompt_projection_digest"),
            provider_result_digest=value.get("provider_result_digest"),
            provider_status=value.get("provider_status"),
            status=value.get("status"),
        )
        supplied = _digest("evidence-backed checkpoint checkpoint_digest", value.get("checkpoint_digest"))
        if supplied != checkpoint.checkpoint_digest:
            raise ArgumentError("evidence-backed checkpoint digest is invalid")
        if canonical_json(value) != canonical_json(checkpoint.to_dict()):
            raise ArgumentError("evidence-backed checkpoint is not normalized")
        return checkpoint


def validate_autonomous_evidence_backed_checkpoint(value: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> AutonomousEvidenceBackedCheckpoint:
    """Validate a checkpoint before journal replay or provider dispatch."""

    return AutonomousEvidenceBackedCheckpoint.from_dict(value.to_dict() if isinstance(value, AutonomousEvidenceBackedCheckpoint) else value)


class AutonomousEvidenceBackedCheckpointStore(Protocol):
    def read(self) -> Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None: ...

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None: ...

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool: ...


class AutonomousEvidenceBackedCheckpointTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvidenceBackedCheckpointTextStore(AutonomousEvidenceBackedCheckpointTextStore, Protocol):
    def write_if_unchanged(self, expected_checkpoint_digest: str | None, value: str) -> bool: ...


class InMemoryAutonomousEvidenceBackedCheckpointStore:
    """Reference checkpoint store with optional compare-and-swap fencing."""

    def __init__(self, initial: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None = None) -> None:
        self._checkpoint = None if initial is None else validate_autonomous_evidence_backed_checkpoint(initial)

    def read(self) -> dict[str, Any] | None:
        return None if self._checkpoint is None else self._checkpoint.to_dict()

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None:
        self._checkpoint = validate_autonomous_evidence_backed_checkpoint(checkpoint)

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool:
        observed = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
        if observed != expected_checkpoint_digest:
            return False
        self.write(checkpoint)
        return True


class JsonAutonomousEvidenceBackedCheckpointPersistence:
    """Canonical JSON checkpoint persistence for files, browser storage, or service adapters."""

    def __init__(self, store: AutonomousEvidenceBackedCheckpointTextStore, *, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("evidence-backed JSON checkpoint store is malformed")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES:
            raise ArgumentError("evidence-backed JSON checkpoint max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("evidence-backed JSON checkpoint is invalid") from error
        normalized = validate_autonomous_evidence_backed_checkpoint(value).to_dict()
        if encoded != canonical_json(normalized):
            raise ArgumentError("evidence-backed JSON checkpoint is not canonical")
        return normalized

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None:
        normalized = validate_autonomous_evidence_backed_checkpoint(checkpoint).to_dict()
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(JsonAutonomousEvidenceBackedCheckpointPersistence):
    """Canonical JSON persistence with stale-writer compare-and-swap fencing."""

    def __init__(self, store: TransactionalAutonomousEvidenceBackedCheckpointTextStore, *, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional evidence-backed checkpoint store requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool:
        if expected_checkpoint_digest is not None:
            _digest("evidence-backed expected checkpoint digest", expected_checkpoint_digest)
        normalized = validate_autonomous_evidence_backed_checkpoint(checkpoint).to_dict()
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        result = self.store.write_if_unchanged(expected_checkpoint_digest, encoded)
        if not isinstance(result, bool):
            raise ArgumentError("transactional evidence-backed checkpoint store returned a non-boolean")
        return result


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedResumableRun:
    """Transient result plus a metadata-only restart checkpoint."""

    status: str
    job_id: str
    result: AutonomousEvidenceBackedRunResult
    checkpoint: AutonomousEvidenceBackedCheckpoint
    provider_rehydrated: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
            "status": self.status,
            "job_id": self.job_id,
            "checkpoint_digest": self.checkpoint.checkpoint_digest,
            "result_status": self.result.status,
            "provider_rehydrated": self.provider_rehydrated,
            "retention": _RESULT_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _checkpoint_status_for_result(result: AutonomousEvidenceBackedRunResult) -> str:
    if result.status == "evidence_review_required":
        return "evidence_review_required"
    if result.evidence is not None and result.evidence.status != "completed":
        return "evidence_incomplete" if result.evidence.status not in {"failed", "reconciliation_required"} else f"evidence_{result.evidence.status}"
    if result.execution_status is not None and (
        result.execution_status == "completed"
        or result.execution_status.startswith("completed")
        or result.execution_status in {"children_completed", "succeeded"}
    ):
        return "completed"
    if _provider_result_was_observed(result.execution_status):
        return "provider_reconciliation_required"
    return "provider_pending"


def _checkpoint_for_result(
    *,
    job_id: str,
    request_digest: str,
    run_policy_digest: str,
    result: AutonomousEvidenceBackedRunResult,
    status: str | None = None,
    provider_result_digest_override: str | None = None,
    provider_status_override: str | None = None,
) -> AutonomousEvidenceBackedCheckpoint:
    resolved_status = _checkpoint_status_for_result(result) if status is None else status
    provider_digest = (
        provider_result_digest_override
        if provider_result_digest_override is not None
        else result.execution_digest
        if _provider_result_was_observed(result.execution_status)
        else None
    )
    provider_status = (
        provider_status_override
        if provider_status_override is not None
        else result.execution_status
        if _provider_result_was_observed(result.execution_status)
        else None
    )
    if resolved_status == "completed":
        provider_digest = result.execution_digest
        provider_status = "completed"
    if resolved_status in {"provider_pending", "evidence_review_required", "evidence_incomplete", "evidence_failed", "evidence_reconciliation_required"}:
        provider_digest = None
        provider_status = None
    return AutonomousEvidenceBackedCheckpoint(
        job_id=job_id,
        task_digest=result.task_digest,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        evidence_plan_digest=result.evidence_plan.plan_digest,
        execution_plan_digest=result.execution_plan_digest,
        evidence_result_digest=None if result.evidence is None else result.evidence.result_digest,
        prompt_projection_digest=None if not result.prompt_context else content_digest(result.prompt_context),
        provider_result_digest=provider_digest,
        provider_status=provider_status,
        status=resolved_status,
    )


def _checkpoint_for_preflight(
    *,
    job_id: str,
    request_digest: str,
    run_policy_digest: str,
    preflight: AutonomousEvidenceBackedPreflight,
) -> AutonomousEvidenceBackedCheckpoint:
    return AutonomousEvidenceBackedCheckpoint(
        job_id=job_id,
        task_digest=preflight.task_digest,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        evidence_plan_digest=preflight.evidence_plan.plan_digest,
        execution_plan_digest=preflight.execution_plan_digest,
        evidence_result_digest=preflight.evidence.result_digest,
        prompt_projection_digest=None if not preflight.prompt_context else content_digest(preflight.prompt_context),
        provider_result_digest=None,
        provider_status=None,
        status="provider_pending",
    )


def _persist_checkpoint(sink: Callable[[AutonomousEvidenceBackedCheckpoint], Any], checkpoint: AutonomousEvidenceBackedCheckpoint) -> None:
    if not callable(sink):
        raise ArgumentError("evidence-backed checkpoint sink must be callable")
    sink(checkpoint)


def _assert_checkpoint_binding(
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    *,
    job_id: str,
    task_digest: str,
    request_digest: str,
    run_policy_digest: str,
    evidence_plan_digest: str,
    execution_plan_digest: str,
) -> None:
    if (
        checkpoint.job_id != job_id
        or checkpoint.task_digest != task_digest
        or checkpoint.request_digest != request_digest
        or checkpoint.run_policy_digest != run_policy_digest
        or checkpoint.evidence_plan_digest != evidence_plan_digest
        or checkpoint.execution_plan_digest != execution_plan_digest
    ):
        raise ArgumentError("evidence-backed checkpoint does not match the current task, plan, requests, policy, or job")


def _resumable_result(
    *,
    status: str,
    job_id: str,
    result: AutonomousEvidenceBackedRunResult,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    provider_rehydrated: bool,
) -> AutonomousEvidenceBackedResumableRun:
    return AutonomousEvidenceBackedResumableRun(status, job_id, result, checkpoint, provider_rehydrated)


def run_autonomous_evidence_backed_resumable(
    agent: Any,
    *,
    task: str,
    job_id: str,
    requests: Sequence[Mapping[str, Any]],
    acquirer: Any,
    credentials: Any,
    checkpoint_sink: Callable[[AutonomousEvidenceBackedCheckpoint], Any],
    checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None = None,
    domains: Sequence[str] | None = None,
    model_candidates: Sequence[Any] | None = None,
    projector: Any | None = None,
    evaluator: Any | None = None,
    rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
    parent_evidence_digests: Sequence[str] = (),
    stop_on_failure: bool = False,
    reevaluate_pending: bool = False,
    available_evidence: Sequence[str] = (),
    completed_stages: Mapping[str, Sequence[str]] | None = None,
    journal: AutonomousEvidenceRuntimeJournal | None = None,
    approve_source_dispatch: bool = False,
    allow_incomplete_evidence: bool = False,
    approve_provider_call: bool = False,
    prompt_builder: Callable[[Any], Mapping[str, Any]] | None = None,
    run_mode: str = "auto",
    run_options: Mapping[str, Any] | None = None,
    resume_provider: bool = False,
    rehydrate_provider_run: Callable[[AutonomousEvidenceBackedCheckpoint, AutonomousEvidenceBackedRunResult], Any | None] | None = None,
) -> AutonomousEvidenceBackedResumableRun:
    """Execute or resume an evidence-backed run without silently replaying provider work."""

    if not isinstance(resume_provider, bool):
        raise ArgumentError("evidence-backed resume_provider must be a boolean")
    if journal is None:
        raise ArgumentError("resumable evidence-backed execution requires a caller-owned evidence journal")
    normalized_job_id = _identifier("evidence-backed resumable job_id", job_id)
    source_requests = _bounded_requests(requests)
    from .autonomy import AUTONOMOUS_DOMAINS

    selected_domains = _bounded_domains(domains, AUTONOMOUS_DOMAINS)
    plan = agent.evidence_plan(
        selected_domains,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
    )
    normalized_task = _bounded_task(task)
    task_digest = content_digest({"task": normalized_task})
    request_digest_value = _request_digest(source_requests)
    run_policy_digest_value = _run_policy_digest(
        domains=selected_domains,
        model_candidates=model_candidates,
        run_mode=run_mode,
        run_options=run_options,
        approve_source_dispatch=approve_source_dispatch,
        allow_incomplete_evidence=allow_incomplete_evidence,
        prompt_builder=prompt_builder,
        evaluator=evaluator,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
        parent_evidence_digests=parent_evidence_digests,
        stop_on_failure=stop_on_failure,
        reevaluate_pending=reevaluate_pending,
    )
    execution_plan_digest_value = _execution_plan_digest(task_digest, plan.plan_digest, selected_domains, run_mode)
    restored = None if checkpoint is None else validate_autonomous_evidence_backed_checkpoint(checkpoint)
    if restored is not None:
        _assert_checkpoint_binding(
            restored,
            job_id=normalized_job_id,
            task_digest=task_digest,
            request_digest=request_digest_value,
            run_policy_digest=run_policy_digest_value,
            evidence_plan_digest=plan.plan_digest,
            execution_plan_digest=execution_plan_digest_value,
        )

    common: dict[str, Any] = {
        "task": normalized_task,
        "requests": source_requests,
        "acquirer": acquirer,
        "credentials": credentials,
        "domains": selected_domains,
        "model_candidates": model_candidates,
        "projector": projector,
        "evaluator": evaluator,
        "rehydrate_value": rehydrate_value,
        "parent_evidence_digests": parent_evidence_digests,
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
        "available_evidence": available_evidence,
        "completed_stages": completed_stages,
        "journal": journal,
        "approve_source_dispatch": approve_source_dispatch,
        "allow_incomplete_evidence": allow_incomplete_evidence,
        "prompt_builder": prompt_builder,
        "run_mode": run_mode,
        "run_options": run_options,
    }

    def execute_without_provider() -> AutonomousEvidenceBackedRunResult:
        return run_autonomous_evidence_backed(
            agent,
            **common,
            approve_provider_call=False,
        )

    def persist_result(result: AutonomousEvidenceBackedRunResult, status: str | None = None) -> AutonomousEvidenceBackedResumableRun:
        next_checkpoint = _checkpoint_for_result(
            job_id=normalized_job_id,
            request_digest=request_digest_value,
            run_policy_digest=run_policy_digest_value,
            result=result,
            status=status,
        )
        _persist_checkpoint(checkpoint_sink, next_checkpoint)
        return _resumable_result(
            status=next_checkpoint.status,
            job_id=normalized_job_id,
            result=result,
            checkpoint=next_checkpoint,
            provider_rehydrated=False,
        )

    if restored is not None and restored.status in {"completed", "provider_reconciliation_required"}:
        probe = execute_without_provider()
        if probe.evidence is None or probe.evidence.status != "completed":
            return persist_result(probe, "evidence_incomplete")
        if rehydrate_provider_run is not None and restored.provider_result_digest is not None:
            recovered = rehydrate_provider_run(restored, probe)
            if recovered is not None:
                _status, _route, recovered_digest = _execution_metadata(agent, recovered)
                if recovered_digest != restored.provider_result_digest:
                    raise BrainRunError("rehydrated provider result does not match its checkpoint digest")
                final = run_autonomous_evidence_backed(
                    agent,
                    **common,
                    approve_provider_call=True,
                    provider_run_override=recovered,
                )
                next_checkpoint = _checkpoint_for_result(
                    job_id=normalized_job_id,
                    request_digest=request_digest_value,
                    run_policy_digest=run_policy_digest_value,
                    result=final,
                    status="completed",
                )
                _persist_checkpoint(checkpoint_sink, next_checkpoint)
                return _resumable_result(
                    status="completed",
                    job_id=normalized_job_id,
                    result=final,
                    checkpoint=next_checkpoint,
                    provider_rehydrated=True,
                )
        if not resume_provider and not approve_provider_call:
            next_checkpoint = _checkpoint_for_result(
                job_id=normalized_job_id,
                request_digest=request_digest_value,
                run_policy_digest=run_policy_digest_value,
                result=probe,
                status="provider_reconciliation_required",
                provider_result_digest_override=restored.provider_result_digest,
                provider_status_override=restored.provider_status,
            )
            _persist_checkpoint(checkpoint_sink, next_checkpoint)
            return _resumable_result(
                status="provider_reconciliation_required",
                job_id=normalized_job_id,
                result=probe,
                checkpoint=next_checkpoint,
                provider_rehydrated=False,
            )

    if restored is not None and restored.status == "provider_pending" and not resume_provider and not approve_provider_call:
        return persist_result(execute_without_provider(), "provider_pending")

    effective_provider_approval = bool(approve_provider_call or resume_provider)
    if effective_provider_approval:
        def before_provider(preflight: AutonomousEvidenceBackedPreflight) -> None:
            _persist_checkpoint(
                checkpoint_sink,
                _checkpoint_for_preflight(
                    job_id=normalized_job_id,
                    request_digest=request_digest_value,
                    run_policy_digest=run_policy_digest_value,
                    preflight=preflight,
                ),
            )

        result = run_autonomous_evidence_backed(
            agent,
            **common,
            approve_provider_call=True,
            before_provider_run=before_provider,
        )
    else:
        result = execute_without_provider()
    return persist_result(result)


class AutonomousEvidenceBackedController:
    """Serialize local resumable operations and fence optional shared persistence."""

    def __init__(self, agent: Any, job_id: str, persistence: AutonomousEvidenceBackedCheckpointStore) -> None:
        if not hasattr(agent, "run_with_reviewed_evidence") or not callable(agent.run_with_reviewed_evidence):
            raise BrainRunError("evidence-backed controller requires an AutonomousAgent")
        self.agent = agent
        self.job_id = _identifier("evidence-backed controller job_id", job_id)
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("evidence-backed controller persistence is malformed")
        self.persistence = persistence
        self._checkpoint: AutonomousEvidenceBackedCheckpoint | None = None
        self._expected_checkpoint_digest: str | None = None
        self._status = "empty"
        self._running = False
        self._lock = Lock()

    def _projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA,
            "status": self._status,
            "job_id": self.job_id,
            "checkpoint_digest": None if self._checkpoint is None else self._checkpoint.checkpoint_digest,
            "persisted": True,
            "retention": _CONTROLLER_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def restore(self) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            raw = self.persistence.read()
            if raw is None:
                self._checkpoint = None
                self._expected_checkpoint_digest = None
                self._status = "empty"
            else:
                self._checkpoint = validate_autonomous_evidence_backed_checkpoint(raw)
                self._expected_checkpoint_digest = self._checkpoint.checkpoint_digest
                self._status = "restored"
            return self._projection()

    def _persist(self, checkpoint: AutonomousEvidenceBackedCheckpoint) -> None:
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_checkpoint_digest, verified.to_dict()):
                raise BrainRunError("evidence-backed checkpoint compare-and-swap conflict; reload before continuing")
        else:
            self.persistence.write(verified.to_dict())
        self._checkpoint = verified
        self._expected_checkpoint_digest = verified.checkpoint_digest
        self._status = verified.status

    def flush(self) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            if self._checkpoint is not None:
                self._persist(self._checkpoint)
            return self._projection()

    def run(self, *, task: str, **options: Any) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            if self._checkpoint is None:
                raw = self.persistence.read()
                if raw is not None:
                    self._checkpoint = validate_autonomous_evidence_backed_checkpoint(raw)
                    self._expected_checkpoint_digest = self._checkpoint.checkpoint_digest
            self._running = True
        try:
            if any(key in options for key in {"job_id", "checkpoint", "checkpoint_sink"}):
                raise ArgumentError("controller owns job_id, checkpoint, and checkpoint_sink")
            run = run_autonomous_evidence_backed_resumable(
                self.agent,
                task=task,
                job_id=self.job_id,
                checkpoint=None if self._checkpoint is None else self._checkpoint.to_dict(),
                checkpoint_sink=self._persist,
                **options,
            )
            with self._lock:
                self._checkpoint = run.checkpoint
                self._expected_checkpoint_digest = run.checkpoint.checkpoint_digest
                self._status = run.status
            return {"controller": self._projection(), "run": run}
        finally:
            with self._lock:
                self._running = False


__all__ = [
    "AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES",
    "AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES",
    "AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_STATUSES",
    "AutonomousEvidenceBackedCheckpoint",
    "validate_autonomous_evidence_backed_checkpoint",
    "AutonomousEvidenceBackedCheckpointStore",
    "AutonomousEvidenceBackedCheckpointTextStore",
    "TransactionalAutonomousEvidenceBackedCheckpointTextStore",
    "InMemoryAutonomousEvidenceBackedCheckpointStore",
    "JsonAutonomousEvidenceBackedCheckpointPersistence",
    "TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence",
    "AutonomousEvidenceBackedResumableRun",
    "run_autonomous_evidence_backed_resumable",
    "AutonomousEvidenceBackedController",
]
