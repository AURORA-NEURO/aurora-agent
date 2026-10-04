"""Persistence and restart lifecycle for resumable autonomous batch jobs."""

from __future__ import annotations

from threading import Lock
from typing import Any, Callable, Mapping, Sequence

from .autonomy import (
    AUTONOMOUS_BATCH_CONTROLLER_SCHEMA,
    AutonomousAgent,
    AutonomousAutomaticBatchProtectedRehydration,
    AutonomousBatchCheckpoint,
    AutonomousBatchProtectedRehydration,
    AutonomousBatchRehydrationContext,
    BrainRunError,
    CredentialHandle,
    CredentialSession,
    ModelCandidate,
)


class AutonomousBrainBatchJobController:
    """Own the process lifecycle around the verified resumable autonomous batch engine.

    ``AutonomousAgent.run_resumable_batch`` intentionally accepts a checkpoint callback so a
    service can choose its own database, object store, or journal. This controller is the
    application-facing boundary: startup restoration is explicit, concurrent runs are rejected,
    every persisted value is parsed and re-serialized as a metadata-only checkpoint, and caller
    tasks, prompts, provider values, connector observations, and credentials remain transient.
    It supports the domain, route-first, automatic, and cross-domain batch modes through one API.
    """

    def __init__(
        self,
        agent: "AutonomousAgent",
        persistence: Any,
        *,
        protected_rehydration: AutonomousBatchProtectedRehydration | None = None,
        automatic_protected_rehydration: AutonomousAutomaticBatchProtectedRehydration | None = None,
    ) -> None:
        if not isinstance(agent, AutonomousAgent):
            raise BrainRunError("autonomous brain batch controller requires an AutonomousAgent")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise BrainRunError("autonomous brain batch checkpoint store is malformed")
        self.agent = agent
        self.persistence = persistence
        if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousBatchProtectedRehydration):
            raise BrainRunError("autonomous brain batch controller protected_rehydration is malformed")
        if automatic_protected_rehydration is not None and not isinstance(automatic_protected_rehydration, AutonomousAutomaticBatchProtectedRehydration):
            raise BrainRunError("autonomous brain batch controller automatic_protected_rehydration is malformed")
        self.protected_rehydration = protected_rehydration
        self.automatic_protected_rehydration = automatic_protected_rehydration
        self._checkpoint: AutonomousBatchCheckpoint | None = None
        self._expected_checkpoint_digest: str | None = None
        self._restored = False
        self._running = False
        self._lock = Lock()

    def _projection(
        self,
        status: str,
        *,
        total_items: int | None = None,
        job_id: str | None = None,
    ) -> dict[str, Any]:
        checkpoint = self._checkpoint
        return {
            "schema": AUTONOMOUS_BATCH_CONTROLLER_SCHEMA,
            "status": status,
            "job_id": job_id if job_id is not None else (None if checkpoint is None else checkpoint.job_id),
            "checkpoint_digest": None if checkpoint is None else checkpoint.checkpoint_digest,
            "completed_items": 0 if checkpoint is None else len(checkpoint.completed_indices),
            "total_items": total_items if total_items is not None else (None if checkpoint is None else len(checkpoint.request_digests)),
            "persisted": True,
            "retention": "metadata_only_request_and_result_digests;task_prompt_provider_connector_values_never_persisted",
            "secret_material": "never_returned",
        }

    def restore(self) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("autonomous brain batch controller already has a run in progress")
            raw = self.persistence.read()
            if raw is None:
                self._checkpoint = None
                self._expected_checkpoint_digest = None
                self._restored = True
                return self._projection("empty")
            if isinstance(raw, AutonomousBatchCheckpoint):
                checkpoint = AutonomousBatchCheckpoint.from_dict(raw.to_dict())
            elif isinstance(raw, Mapping):
                checkpoint = AutonomousBatchCheckpoint.from_dict(raw)
            else:
                raise BrainRunError("autonomous brain batch checkpoint store returned an invalid value")
            self._checkpoint = checkpoint
            self._expected_checkpoint_digest = checkpoint.checkpoint_digest
            self._restored = True
            return self._projection("restored")

    def flush(self) -> dict[str, Any]:
        with self._lock:
            if not self._restored:
                raise BrainRunError("autonomous brain batch controller must restore before flushing")
            if self._running:
                raise BrainRunError("autonomous brain batch controller already has a run in progress")
            if self._checkpoint is None:
                return self._projection("empty")
            verified = AutonomousBatchCheckpoint.from_dict(self._checkpoint.to_dict())
            self._write_checkpoint(verified)
            self._checkpoint = verified
            return self._projection("flushed")

    def _persist(self, checkpoint: AutonomousBatchCheckpoint) -> None:
        verified = AutonomousBatchCheckpoint.from_dict(checkpoint.to_dict())
        self._write_checkpoint(verified)
        self._checkpoint = verified

    def _write_checkpoint(self, checkpoint: AutonomousBatchCheckpoint) -> None:
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_checkpoint_digest, checkpoint.to_dict()):
                raise BrainRunError("autonomous batch checkpoint compare-and-swap conflict")
        else:
            self.persistence.write(checkpoint.to_dict())
        self._expected_checkpoint_digest = checkpoint.checkpoint_digest

    def run(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        mode: str = "domain",
        launch_admission: Mapping[str, Any] | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        rehydrate_result: Callable[[AutonomousBatchRehydrationContext], Any] | None = None,
    ) -> dict[str, Any]:
        with self._lock:
            if not self._restored:
                raise BrainRunError("autonomous brain batch controller must restore before execution")
            if self._running:
                raise BrainRunError("autonomous brain batch controller already has a run in progress")
            self._running = True
        try:
            effective_rehydrator = rehydrate_result
            if effective_rehydrator is None and mode == "auto" and self.automatic_protected_rehydration is not None:
                effective_rehydrator = self.automatic_protected_rehydration.resolve
            if effective_rehydrator is None and self.protected_rehydration is not None:
                effective_rehydrator = self.protected_rehydration.resolve
            run_kwargs = {
                "job_id": job_id,
                "mode": mode,
                "credentials": credentials,
                "model_candidates": model_candidates,
                "options_factory": options_factory,
                "max_parallelism": max_parallelism,
                "stop_on_error": stop_on_error,
                "checkpoint": None if self._checkpoint is None else self._checkpoint.to_dict(),
                "checkpoint_sink": self._persist,
                "rehydrate_result": effective_rehydrator,
            }
            if launch_admission is None:
                result = self.agent.run_resumable_batch(requests, **run_kwargs)
            else:
                result = self.agent.run_resumable_batch_with_launch_admission(
                    requests,
                    launch_admission=launch_admission,
                    **run_kwargs,
                )
            return {"controller": self._projection(result.status, total_items=len(requests), job_id=job_id), "batch": result}
        finally:
            with self._lock:
                self._running = False

__all__ = ["AutonomousBrainBatchJobController"]
