"""Evaluator-gated automatic retries for staged autonomous workflows.

The normal workflow runner is deliberately conservative: it can pause after a stage and the
learning runner can report that an evaluator requested a replan, but neither surface silently
replays a provider call.  This module is the explicit opt-in composition layer for applications
that want a bounded recovery loop.

The loop retries the complete prepared workflow.  That is intentional.  A stage may have
crossed a provider, tool, or approval boundary, so replaying only the failed stage would make
the continuation depend on an unverified partial side effect.  Every retry therefore reuses the
same blueprint, route, model catalogue, credential handles, provider-tool set, and approval
options, while adding only a transient evaluator packet to the caller context.

Durable checkpoints are metadata-only.  They bind attempts to task/workflow/bandit digests and
outcome identities, but never retain task text, provider responses, tool arguments, credentials,
or the evaluator's transient instruction.  A restarted worker must rehydrate the context from
its own protected store and prove its digest matches the checkpoint before continuing.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import uuid
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .brain import (
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    BrainRunError,
)
from .autonomy import (
    MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS,
    AutonomousTaskBlueprint,
    AutonomousTaskOrchestrator,
    AutonomousWorkflowLearningResult,
    AutonomousWorkflowStageEvaluation,
)
from .evaluators import DomainEvaluatorRegistry


AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA = "bioprism-python-autonomous-workflow-cycle/0.1"
AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_SCHEMA = (
    "bioprism-python-autonomous-workflow-cycle-checkpoint/0.1"
)
AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_SCHEMA = "bioprism-python-autonomous-workflow-cycle-context/0.1"
AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_KEY = "_aurora_workflow_replan"
MAX_AUTONOMOUS_WORKFLOW_REPLANS = MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS
MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS = MAX_AUTONOMOUS_WORKFLOW_REPLANS + 1
MAX_AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_BYTES = 1_000_000
MAX_AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_BYTES = 256_000
MAX_AUTONOMOUS_WORKFLOW_CYCLE_TEXT_BYTES = 16_000


def _digest(value: Any, name: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise BrainRunError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _text(value: Any, name: str, *, maximum: int = MAX_AUTONOMOUS_WORKFLOW_CYCLE_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} must be a bounded non-empty string")
    return value


def _safe_mapping(value: Mapping[str, Any], name: str, *, maximum: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise BrainRunError(f"{name} must be a mapping")
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        normalized = json.loads(encoded)
    except (TypeError, ValueError) as error:
        raise BrainRunError(f"{name} must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    if not isinstance(normalized, dict):
        raise BrainRunError(f"{name} must remain a mapping")
    return normalized


def _identifier(value: Any, name: str) -> str:
    return _text(value, name, maximum=256)


def _decision_projection(decision: BrainEvaluatorDecision) -> dict[str, Any]:
    """Project a decision without copying the transient instruction into durable metadata."""

    return {
        "evaluator_id": decision.evaluator_id,
        "evaluator_version": decision.evaluator_version,
        "reward": decision.reward,
        "passed": decision.passed,
        "failed": decision.failed,
        "feedback_digest": decision.feedback_digest,
        "failure_class": decision.failure_class,
        "evidence_digest": decision.evidence_digest,
        "replan_requested": decision.replan_requested,
        "replan_instruction_digest": (
            None
            if decision.replan_instruction is None
            else content_digest(decision.replan_instruction)
        ),
    }


def _evaluation_projection(evaluation: AutonomousWorkflowStageEvaluation) -> dict[str, Any]:
    return {
        "stage_id": evaluation.stage_id,
        "stage_status": evaluation.stage_status,
        "decision": _decision_projection(evaluation.decision),
        "evidence_digest": evaluation.evidence_digest,
        "recording_status": evaluation.recording.get("status")
        if isinstance(evaluation.recording, Mapping)
        else None,
    }


def _workflow_outcome_digest(result: AutonomousWorkflowLearningResult) -> str:
    """Hash only workflow progress and evaluator value metadata, never raw provider content."""

    run = result.workflow
    stage_rows = []
    for stage in run.stage_results:
        stage_rows.append(
            {
                "stage_id": stage.stage.id,
                "execution_status": stage.execution_status,
                "declared_status": stage.declared_status,
                "response_digest": stage.response_digest,
                "validation_errors": list(stage.validation_errors),
            }
        )
    return content_digest(
        {
            "schema": AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA,
            "status": result.status,
            "run_id": run.run_id,
            "workflow_id": run.blueprint.workflow.workflow_id,
            "workflow_digest": run.blueprint.workflow.workflow_digest,
            "stage_rows": stage_rows,
            "evaluations": [_evaluation_projection(item) for item in result.evaluations],
            "replan_requested": result.replan_requested,
        }
    )


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowCycleAttempt:
    """One complete workflow-learning attempt and its value-only retry metadata."""

    attempt: int
    workflow: AutonomousWorkflowLearningResult
    outcome_digest: str
    replan_requested: bool = False
    replan_instruction_digest: str | None = None
    failure_classes: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if (
            not isinstance(self.attempt, int)
            or isinstance(self.attempt, bool)
            or not 1 <= self.attempt <= MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS
        ):
            raise BrainRunError("workflow cycle attempt is outside the bounded range")
        if not isinstance(self.workflow, AutonomousWorkflowLearningResult):
            raise BrainRunError("workflow cycle attempt contains an invalid learning result")
        _digest(self.outcome_digest, "workflow cycle attempt outcome_digest")
        if not isinstance(self.replan_requested, bool):
            raise BrainRunError("workflow cycle attempt replan_requested must be boolean")
        if self.replan_instruction_digest is not None:
            _digest(
                self.replan_instruction_digest,
                "workflow cycle attempt replan_instruction_digest",
            )
        if self.replan_requested and self.replan_instruction_digest is None:
            raise BrainRunError("workflow cycle replan requests require an instruction digest")
        if not isinstance(self.failure_classes, Sequence) or isinstance(
            self.failure_classes, (str, bytes)
        ):
            raise BrainRunError("workflow cycle failure_classes must be a sequence")
        normalized = tuple(
            _text(value, "workflow cycle failure class", maximum=256)
            for value in self.failure_classes
        )
        if len(normalized) > 32 or len(set(normalized)) != len(normalized):
            raise BrainRunError("workflow cycle failure_classes are outside their bound")
        object.__setattr__(self, "failure_classes", normalized)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA,
            "attempt": self.attempt,
            "workflow": self.workflow.to_dict(),
            "outcome_digest": self.outcome_digest,
            "replan_requested": self.replan_requested,
            "replan_instruction_digest": self.replan_instruction_digest,
            "failure_classes": list(self.failure_classes),
            "retention": (
                "provider_results_caller_owned; replan_instruction_transient; "
                "value_only_attempt_metadata"
            ),
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowCycleCheckpoint:
    """Metadata-only continuation state for a workflow retry cycle."""

    run_id: str
    task_digest: str
    workflow_id: str
    workflow_digest: str
    max_replans: int
    attempt: int
    status: str
    replan_count: int = 0
    attempt_run_ids: tuple[str, ...] = ()
    attempt_outcome_digests: tuple[str, ...] = ()
    last_outcome_digest: str | None = None
    next_context_digest: str | None = None
    replan_instruction_digest: str | None = None
    bandit_state_digest: str | None = None

    _STATUSES = frozenset(
        {
            "initial",
            "retry_ready",
            "completed",
            "completed_without_replan",
            "replan_limit_reached",
            "execution_blocked",
        }
    )

    def __post_init__(self) -> None:
        _identifier(self.run_id, "workflow cycle checkpoint run_id")
        _digest(self.task_digest, "workflow cycle checkpoint task_digest")
        _identifier(self.workflow_id, "workflow cycle checkpoint workflow_id")
        _digest(self.workflow_digest, "workflow cycle checkpoint workflow_digest")
        if (
            not isinstance(self.max_replans, int)
            or isinstance(self.max_replans, bool)
            or not 0 <= self.max_replans <= MAX_AUTONOMOUS_WORKFLOW_REPLANS
        ):
            raise BrainRunError("workflow cycle checkpoint max_replans is outside the bound")
        if (
            not isinstance(self.attempt, int)
            or isinstance(self.attempt, bool)
            or not 0 <= self.attempt <= self.max_replans + 1
        ):
            raise BrainRunError("workflow cycle checkpoint attempt is outside the bound")
        if self.status not in self._STATUSES:
            raise BrainRunError("workflow cycle checkpoint has an invalid status")
        if (
            not isinstance(self.replan_count, int)
            or isinstance(self.replan_count, bool)
            or self.replan_count != max(0, self.attempt - 1)
        ):
            raise BrainRunError("workflow cycle checkpoint replan_count must match attempt")
        if not isinstance(self.attempt_run_ids, Sequence) or isinstance(
            self.attempt_run_ids, (str, bytes)
        ):
            raise BrainRunError("workflow cycle checkpoint attempt_run_ids must be a sequence")
        run_ids = tuple(
            _identifier(value, "workflow cycle checkpoint attempt run_id")
            for value in self.attempt_run_ids
        )
        if len(run_ids) > MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS or len(set(run_ids)) != len(run_ids):
            raise BrainRunError("workflow cycle checkpoint attempt run IDs are outside the bound")
        if not isinstance(self.attempt_outcome_digests, Sequence) or isinstance(
            self.attempt_outcome_digests, (str, bytes)
        ):
            raise BrainRunError("workflow cycle checkpoint outcome digests must be a sequence")
        outcome_digests = tuple(
            _digest(value, "workflow cycle checkpoint outcome digest")
            for value in self.attempt_outcome_digests
        )
        if len(outcome_digests) > MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS:
            raise BrainRunError("workflow cycle checkpoint outcome history is too large")
        if len(run_ids) != self.attempt or len(outcome_digests) != self.attempt:
            raise BrainRunError("workflow cycle checkpoint attempt metadata must align")
        for name, value in (
            ("last_outcome_digest", self.last_outcome_digest),
            ("next_context_digest", self.next_context_digest),
            ("replan_instruction_digest", self.replan_instruction_digest),
            ("bandit_state_digest", self.bandit_state_digest),
        ):
            if value is not None:
                _digest(value, f"workflow cycle checkpoint {name}")
        if self.attempt == 0:
            if self.status != "initial" or any(
                value is not None
                for value in (
                    self.last_outcome_digest,
                    self.next_context_digest,
                    self.replan_instruction_digest,
                    self.bandit_state_digest,
                )
            ):
                raise BrainRunError("initial workflow cycle checkpoint contains attempt state")
        else:
            if self.last_outcome_digest is None or self.bandit_state_digest is None:
                raise BrainRunError("settled workflow cycle checkpoint is missing attempt digests")
        if self.status == "retry_ready":
            if self.attempt == 0 or self.attempt >= self.max_replans + 1:
                raise BrainRunError("retry-ready workflow cycle checkpoint is outside the retry bound")
            if self.next_context_digest is None or self.replan_instruction_digest is None:
                raise BrainRunError("retry-ready workflow cycle checkpoint is missing retry digests")
        elif self.next_context_digest is not None or self.replan_instruction_digest is not None:
            raise BrainRunError("terminal workflow cycle checkpoint retains retry context")
        payload = self._payload(run_ids=run_ids, outcome_digests=outcome_digests)
        encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_BYTES:
            raise BrainRunError("workflow cycle checkpoint exceeds its bounded size")
        object.__setattr__(self, "attempt_run_ids", run_ids)
        object.__setattr__(self, "attempt_outcome_digests", outcome_digests)

    def _payload(
        self,
        *,
        run_ids: Sequence[str] | None = None,
        outcome_digests: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_SCHEMA,
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "max_replans": self.max_replans,
            "attempt": self.attempt,
            "status": self.status,
            "replan_count": self.replan_count,
            "attempt_run_ids": list(self.attempt_run_ids if run_ids is None else run_ids),
            "attempt_outcome_digests": list(
                self.attempt_outcome_digests if outcome_digests is None else outcome_digests
            ),
            "last_outcome_digest": self.last_outcome_digest,
            "next_context_digest": self.next_context_digest,
            "replan_instruction_digest": self.replan_instruction_digest,
            "bandit_state_digest": self.bandit_state_digest,
        }

    @property
    def checkpoint_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "checkpoint_digest": self.checkpoint_digest,
            "retention": "task_and_workflow_attempt_value_digests_only; raw_retry_context_caller_owned",
            "authorization": "retry_reuses_original_tools_route_credentials_and_approval_boundary",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousWorkflowCycleCheckpoint":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_SCHEMA:
            raise BrainRunError("workflow cycle checkpoint has an invalid schema")
        checkpoint = cls(
            run_id=value.get("run_id"),
            task_digest=value.get("task_digest"),
            workflow_id=value.get("workflow_id"),
            workflow_digest=value.get("workflow_digest"),
            max_replans=value.get("max_replans"),
            attempt=value.get("attempt"),
            status=value.get("status"),
            replan_count=value.get("replan_count", 0),
            attempt_run_ids=tuple(value.get("attempt_run_ids", ())),
            attempt_outcome_digests=tuple(value.get("attempt_outcome_digests", ())),
            last_outcome_digest=value.get("last_outcome_digest"),
            next_context_digest=value.get("next_context_digest"),
            replan_instruction_digest=value.get("replan_instruction_digest"),
            bandit_state_digest=value.get("bandit_state_digest"),
        )
        supplied_digest = value.get("checkpoint_digest")
        if supplied_digest is not None and supplied_digest != checkpoint.checkpoint_digest:
            raise BrainRunError("workflow cycle checkpoint digest does not match its contents")
        return checkpoint


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowCycleResult:
    """Caller-visible result containing complete attempts and a metadata-only checkpoint."""

    status: str
    final: AutonomousWorkflowCycleAttempt | None
    attempts: tuple[AutonomousWorkflowCycleAttempt, ...]
    replan_count: int
    attempts_before: int = 0
    checkpoint: AutonomousWorkflowCycleCheckpoint | None = None

    def __post_init__(self) -> None:
        _identifier(self.status, "workflow cycle result status")
        if not isinstance(self.attempts, Sequence) or isinstance(self.attempts, (str, bytes)):
            raise BrainRunError("workflow cycle attempts must be a sequence")
        if len(self.attempts) > MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS:
            raise BrainRunError("workflow cycle contains too many attempts")
        if any(not isinstance(item, AutonomousWorkflowCycleAttempt) for item in self.attempts):
            raise BrainRunError("workflow cycle attempts contain an invalid value")
        if (
            not isinstance(self.attempts_before, int)
            or isinstance(self.attempts_before, bool)
            or not 0 <= self.attempts_before <= MAX_AUTONOMOUS_WORKFLOW_REPLANS + 1
        ):
            raise BrainRunError("workflow cycle attempts_before is outside the bound")
        if self.attempts_before + len(self.attempts) > MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS:
            raise BrainRunError("workflow cycle attempts exceed the bounded history")
        expected = tuple(
            range(self.attempts_before + 1, self.attempts_before + len(self.attempts) + 1)
        )
        if tuple(item.attempt for item in self.attempts) != expected:
            raise BrainRunError("workflow cycle attempts must be contiguous and ordered")
        if (
            not isinstance(self.replan_count, int)
            or isinstance(self.replan_count, bool)
            or self.replan_count != max(0, self.attempts_before + len(self.attempts) - 1)
        ):
            raise BrainRunError("workflow cycle replan_count must match the attempt sequence")
        if self.final is not None:
            if not isinstance(self.final, AutonomousWorkflowCycleAttempt):
                raise BrainRunError("workflow cycle final attempt is invalid")
            if not self.attempts or self.final.attempt != self.attempts[-1].attempt:
                raise BrainRunError("workflow cycle final attempt must be the latest attempt")
        elif self.attempts:
            raise BrainRunError("workflow cycle with attempts must expose a final attempt")
        if self.checkpoint is not None:
            if not isinstance(self.checkpoint, AutonomousWorkflowCycleCheckpoint):
                raise BrainRunError("workflow cycle checkpoint is invalid")
            if self.final is not None and self.checkpoint.attempt != self.final.attempt:
                raise BrainRunError("workflow cycle checkpoint must match the final attempt")
        object.__setattr__(self, "attempts", tuple(self.attempts))

    @property
    def replan_requested(self) -> bool:
        return bool(self.final is not None and self.final.replan_requested)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA,
            "status": self.status,
            "final": None if self.final is None else self.final.to_dict(),
            "attempts": [item.to_dict() for item in self.attempts],
            "replan_count": self.replan_count,
            "attempts_before": self.attempts_before,
            "replan_requested": self.replan_requested,
            "checkpoint": None if self.checkpoint is None else self.checkpoint.to_dict(),
            "retention": "provider_results_caller_owned; retry_instruction_transient",
        }


def _replan_context(
    *,
    next_attempt: int,
    workflow: AutonomousWorkflowLearningResult,
    outcome_digest: str,
    decision: BrainEvaluatorDecision,
) -> dict[str, Any]:
    instruction = _replan_instruction(decision)
    packet = {
        "schema": AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_SCHEMA,
        "workflow": "workflow_replan_context",
        "attempt": next_attempt,
        "previous": {
            "workflow_id": workflow.workflow.blueprint.workflow.workflow_id,
            "workflow_digest": workflow.workflow.blueprint.workflow.workflow_digest,
            "outcome_digest": outcome_digest,
        },
        "evaluator": _decision_projection(decision),
        "instruction": instruction,
        "bounded_replan": True,
        "does_not_authorize": [
            "new domains, capabilities, tools, credentials, approvals, or effects",
            "treating prior workflow output as verified truth",
            "claiming that an external action occurred",
        ],
    }
    return _safe_mapping(
        packet,
        "workflow cycle retry context",
        maximum=MAX_AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_BYTES,
    )


def _replan_instruction(decision: BrainEvaluatorDecision) -> str:
    return decision.replan_instruction or (
        "Address the evaluator failure class with new bounded evidence before retrying the "
        "same reviewed workflow."
    )


def _attempt_run_id(base: str, attempt: int) -> str:
    candidate = f"{base}-attempt-{attempt}"
    if len(candidate.encode("utf-8")) <= 256:
        return candidate
    return "workflow-cycle-" + content_digest({"base": base, "attempt": attempt})[:48]


def _attempt_key(base: str | None, attempt: int) -> str | None:
    if base is None:
        return None
    candidate = f"{base}-attempt-{attempt}"
    if len(candidate.encode("utf-8")) <= 256:
        return candidate
    return "workflow-cycle-key-" + content_digest({"base": base, "attempt": attempt})[:48]


def run_workflow_cycle(
    orchestrator: AutonomousTaskOrchestrator,
    *,
    blueprint: AutonomousTaskBlueprint,
    model_candidates: Sequence[Mapping[str, Any]],
    credentials: Mapping[str, Any],
    bandit_state: Mapping[str, Any],
    evaluator: BrainOutcomeEvaluator | None = None,
    evaluator_registry: DomainEvaluatorRegistry | None = None,
    stage_evidence: Mapping[str, Mapping[str, Any]] | None = None,
    memory_tags: Sequence[str] = (),
    memory: Any = None,
    max_replans: int = 1,
    automatic_replan: bool = True,
    run_id: str | None = None,
    idempotency_key: str | None = None,
    context: Mapping[str, Any] | None = None,
    checkpoint: AutonomousWorkflowCycleCheckpoint | Mapping[str, Any] | None = None,
    checkpoint_sink: Callable[[AutonomousWorkflowCycleCheckpoint], Any] | None = None,
    **workflow_kwargs: Any,
) -> AutonomousWorkflowCycleResult:
    """Execute evaluator-guided workflow attempts until success or a strict retry bound.

    A retry is never implicit in :meth:`run_workflow_learning`; callers must choose this method
    and can disable the second attempt with ``automatic_replan=False``.  A retry starts from the
    prepared blueprint, not a partial workflow checkpoint, so the authorization surface cannot
    be widened by evaluator text or by a stage that may have crossed an effect boundary.
    """

    if not isinstance(orchestrator, AutonomousTaskOrchestrator):
        raise BrainRunError("workflow cycle requires an AutonomousTaskOrchestrator")
    if not isinstance(blueprint, AutonomousTaskBlueprint):
        raise BrainRunError("workflow cycle requires an AutonomousTaskBlueprint")
    if not isinstance(bandit_state, Mapping):
        raise BrainRunError("workflow cycle bandit_state must be a mapping")
    BrainLearningLedger._assert_safe(bandit_state)
    if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
        raise BrainRunError("workflow cycle evaluator must be a BrainOutcomeEvaluator or None")
    if evaluator_registry is not None and not isinstance(evaluator_registry, DomainEvaluatorRegistry):
        raise BrainRunError("workflow cycle evaluator_registry must be a DomainEvaluatorRegistry or None")
    if not isinstance(max_replans, int) or isinstance(max_replans, bool) or not 0 <= max_replans <= MAX_AUTONOMOUS_WORKFLOW_REPLANS:
        raise BrainRunError(
            f"workflow cycle max_replans must be within [0, {MAX_AUTONOMOUS_WORKFLOW_REPLANS}]"
        )
    if not isinstance(automatic_replan, bool):
        raise BrainRunError("workflow cycle automatic_replan must be boolean")
    if checkpoint_sink is not None and not callable(checkpoint_sink):
        raise BrainRunError("workflow cycle checkpoint_sink must be callable or None")
    if stage_evidence is not None:
        _safe_mapping(stage_evidence, "workflow cycle stage_evidence", maximum=1_000_000)
    if context is None:
        base_context: dict[str, Any] = {}
    else:
        base_context = _safe_mapping(
            context,
            "workflow cycle context",
            maximum=MAX_AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_BYTES,
        )
    if "checkpoint" in workflow_kwargs:
        raise BrainRunError("workflow cycle checkpoint must be supplied as the cycle checkpoint")

    checkpoint_value: AutonomousWorkflowCycleCheckpoint | None
    if checkpoint is None:
        checkpoint_value = None
    elif isinstance(checkpoint, AutonomousWorkflowCycleCheckpoint):
        checkpoint_value = checkpoint
    elif isinstance(checkpoint, Mapping):
        checkpoint_value = AutonomousWorkflowCycleCheckpoint.from_dict(checkpoint)
    else:
        raise BrainRunError("workflow cycle checkpoint must be a checkpoint, mapping, or None")

    resolved_run_id = (
        checkpoint_value.run_id
        if checkpoint_value is not None
        else _identifier(run_id or f"workflow-cycle-{uuid.uuid4().hex}", "workflow cycle run_id")
    )
    if run_id is not None and _identifier(run_id, "workflow cycle run_id") != resolved_run_id:
        raise BrainRunError("workflow cycle checkpoint run_id does not match the request")
    if idempotency_key is not None:
        _text(idempotency_key, "workflow cycle idempotency_key", maximum=256)

    if checkpoint_value is not None:
        if checkpoint_value.task_digest != blueprint.spec.task_digest:
            raise BrainRunError("workflow cycle checkpoint task does not match the blueprint")
        if checkpoint_value.workflow_id != blueprint.workflow.workflow_id or checkpoint_value.workflow_digest != blueprint.workflow.workflow_digest:
            raise BrainRunError("workflow cycle checkpoint workflow does not match the blueprint")
        if checkpoint_value.max_replans != max_replans:
            raise BrainRunError("workflow cycle checkpoint max_replans does not match the request")
        if checkpoint_value.status in {
            "completed",
            "completed_without_replan",
            "replan_limit_reached",
            "execution_blocked",
        }:
            raise BrainRunError("workflow cycle checkpoint is already terminal")
        if checkpoint_value.attempt > 0 and checkpoint_value.bandit_state_digest != content_digest(bandit_state):
            raise BrainRunError("workflow cycle checkpoint bandit state does not match the request")
        if checkpoint_value.status == "retry_ready":
            retry_context = base_context.get(AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_KEY)
            if not isinstance(retry_context, Mapping):
                raise BrainRunError("resuming a workflow cycle requires caller-owned retry context")
            if checkpoint_value.next_context_digest != content_digest(retry_context):
                raise BrainRunError("caller-owned retry context does not match the workflow cycle checkpoint")
    else:
        checkpoint_value = AutonomousWorkflowCycleCheckpoint(
            run_id=resolved_run_id,
            task_digest=blueprint.spec.task_digest,
            workflow_id=blueprint.workflow.workflow_id,
            workflow_digest=blueprint.workflow.workflow_digest,
            max_replans=max_replans,
            attempt=0,
            status="initial",
        )
    if AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_KEY in base_context and checkpoint_value.status != "retry_ready":
        raise BrainRunError("workflow cycle retry context requires a retry-ready checkpoint")

    start_attempt = checkpoint_value.attempt + 1
    attempts_before = checkpoint_value.attempt
    state: Mapping[str, Any] = dict(bandit_state)
    current_context = dict(base_context)
    attempts: list[AutonomousWorkflowCycleAttempt] = []

    def persist(value: AutonomousWorkflowCycleCheckpoint) -> None:
        if checkpoint_sink is None:
            return
        try:
            checkpoint_sink(value)
        except Exception as error:
            raise BrainRunError("workflow cycle checkpoint persistence failed") from error

    def settle(
        attempt_result: AutonomousWorkflowCycleAttempt,
        *,
        status: str,
        next_context: Mapping[str, Any] | None = None,
    ) -> AutonomousWorkflowCycleCheckpoint:
        updated = AutonomousWorkflowCycleCheckpoint(
            run_id=resolved_run_id,
            task_digest=blueprint.spec.task_digest,
            workflow_id=blueprint.workflow.workflow_id,
            workflow_digest=blueprint.workflow.workflow_digest,
            max_replans=max_replans,
            attempt=attempt_result.attempt,
            status=status,
            replan_count=max(0, attempt_result.attempt - 1),
            attempt_run_ids=(*checkpoint_value.attempt_run_ids, attempt_result.workflow.workflow.run_id),
            attempt_outcome_digests=(*checkpoint_value.attempt_outcome_digests, attempt_result.outcome_digest),
            last_outcome_digest=attempt_result.outcome_digest,
            next_context_digest=None if next_context is None else content_digest(next_context),
            replan_instruction_digest=(
                None if next_context is None else attempt_result.replan_instruction_digest
            ),
            bandit_state_digest=content_digest(attempt_result.workflow.bandit_state),
        )
        persist(updated)
        return updated

    for attempt_number in range(start_attempt, max_replans + 2):
        attempt_run_id = _attempt_run_id(resolved_run_id, attempt_number)
        call_kwargs = dict(workflow_kwargs)
        call_kwargs.update(
            {
                "blueprint": blueprint,
                "model_candidates": model_candidates,
                "credentials": credentials,
                "context": dict(current_context),
                "run_id": attempt_run_id,
                "idempotency_key": _attempt_key(idempotency_key, attempt_number),
            }
        )
        learned = orchestrator.run_workflow_learning(
            bandit_state=state,
            evaluator=evaluator,
            evaluator_registry=evaluator_registry,
            stage_evidence=stage_evidence,
            memory_tags=[*memory_tags, f"workflow-cycle-attempt:{attempt_number}"],
            memory=memory,
            **call_kwargs,
        )
        state = dict(learned.bandit_state)
        requested = [
            evaluation.decision
            for evaluation in learned.evaluations
            if evaluation.decision.failed and evaluation.decision.replan_requested
        ]
        selected = requested[-1] if requested else None
        outcome_digest = _workflow_outcome_digest(learned)
        instruction_digest = (
            None if selected is None else content_digest(_replan_instruction(selected))
        )
        failure_classes = tuple(
            dict.fromkeys(
                evaluation.decision.failure_class
                for evaluation in learned.evaluations
                if evaluation.decision.failure_class is not None
            )
        )
        attempt_result = AutonomousWorkflowCycleAttempt(
            attempt=attempt_number,
            workflow=learned,
            outcome_digest=outcome_digest,
            replan_requested=selected is not None,
            replan_instruction_digest=instruction_digest,
            failure_classes=failure_classes,
        )
        attempts.append(attempt_result)
        if selected is None:
            passed = bool(learned.evaluations) and all(
                evaluation.decision.passed for evaluation in learned.evaluations
            )
            final_status = (
                "completed"
                if learned.workflow.status == "completed" and passed
                else "completed_without_replan"
                if learned.workflow.status in {"completed", "paused", "learning_replan_requested"}
                else "execution_blocked"
            )
            checkpoint_value = settle(attempt_result, status=final_status)
            return AutonomousWorkflowCycleResult(
                status=final_status,
                final=attempt_result,
                attempts=tuple(attempts),
                replan_count=attempt_number - 1,
                attempts_before=attempts_before,
                checkpoint=checkpoint_value,
            )
        if not automatic_replan:
            checkpoint_value = settle(attempt_result, status="completed_without_replan")
            return AutonomousWorkflowCycleResult(
                status="completed_without_replan",
                final=attempt_result,
                attempts=tuple(attempts),
                replan_count=attempt_number - 1,
                attempts_before=attempts_before,
                checkpoint=checkpoint_value,
            )
        if attempt_number > max_replans:
            checkpoint_value = settle(attempt_result, status="replan_limit_reached")
            return AutonomousWorkflowCycleResult(
                status="replan_limit_reached",
                final=attempt_result,
                attempts=tuple(attempts),
                replan_count=attempt_number - 1,
                attempts_before=attempts_before,
                checkpoint=checkpoint_value,
            )
        next_context = _replan_context(
            next_attempt=attempt_number + 1,
            workflow=learned,
            outcome_digest=outcome_digest,
            decision=selected,
        )
        current_context[AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_KEY] = next_context
        checkpoint_value = settle(
            attempt_result,
            status="retry_ready",
            next_context=next_context,
        )
    raise BrainRunError("workflow cycle exited without a terminal result")


__all__ = [
    "AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA",
    "AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_SCHEMA",
    "AUTONOMOUS_WORKFLOW_CYCLE_CONTEXT_KEY",
    "MAX_AUTONOMOUS_WORKFLOW_REPLANS",
    "MAX_AUTONOMOUS_WORKFLOW_CYCLE_ATTEMPTS",
    "MAX_AUTONOMOUS_WORKFLOW_CYCLE_CHECKPOINT_BYTES",
    "AutonomousWorkflowCycleAttempt",
    "AutonomousWorkflowCycleCheckpoint",
    "AutonomousWorkflowCycleResult",
    "run_workflow_cycle",
]
