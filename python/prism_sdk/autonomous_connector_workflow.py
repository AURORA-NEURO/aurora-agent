"""Durable workflow execution through reviewed autonomous connectors.

The ordinary Python workflow runner executes one structured provider decision per stage and
therefore requires caller-owned model candidates and credential handles.  This module provides
the complementary offline/provider-agnostic path: an already prepared
``AutonomousTaskBlueprint`` can run its exact stage DAG through
``AutonomousConnectorRuntime`` without invoking an LLM.

The adapter deliberately reuses the existing workflow checkpoint and stage-status contracts.
Connector selection is still review-only until dispatch, every stage is bound to its exact
capability and plan digest, and a journal replay never restores a payload unless the caller
rehydrates and digest-verifies it.  This is an execution adapter, not a source of external truth;
real browser, literature, FHIR, object-store, or provider execution remains caller-owned.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import uuid
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_connector_worker import AutonomousConnectorOperationRegistry
from .autonomous_connectors import (
    MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorDispatchResult,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
)
from .autonomous_evidence_runtime import AutonomousEvidenceRuntime
from .autonomous_protected_rehydration import AutonomousProtectedRehydrationAdapter
from .domain_tools import _json_safe, _reject_secret_fields
from .errors import ArgumentError
from .autonomy import (
    AutonomousTaskBlueprint,
    AutonomousTaskOrchestrator,
    AutonomousWorkflowCheckpoint,
    AutonomousWorkflowRun,
    AutonomousWorkflowStage,
    AutonomousWorkflowStageResult,
    compile_autonomous_workflow_stage_execution_plan,
)


AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA = "bioprism-python-autonomous-connector-workflow-adapter/0.1"
MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_REQUEST_BYTES = MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES
MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_CALLS = 16


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_id(name: str, value: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 128:
        raise ArgumentError(f"{name} is outside its bound")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in value):
        raise ArgumentError(f"{name} contains unsupported characters")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousConnectorWorkflowStageContext:
    """Transient context supplied to a caller-owned stage request builder."""

    blueprint: AutonomousTaskBlueprint
    run_id: str
    checkpoint: AutonomousWorkflowCheckpoint
    stage: AutonomousWorkflowStage
    stage_attempt: int
    dependency_outputs: Mapping[str, Any]
    completed_stage_ids: tuple[str, ...]

    def __post_init__(self) -> None:
        if not isinstance(self.blueprint, AutonomousTaskBlueprint):
            raise ArgumentError("connector workflow stage blueprint is invalid")
        if not isinstance(self.checkpoint, AutonomousWorkflowCheckpoint):
            raise ArgumentError("connector workflow stage checkpoint is invalid")
        if not isinstance(self.stage, AutonomousWorkflowStage):
            raise ArgumentError("connector workflow stage is invalid")
        _bounded_id("connector workflow stage run_id", self.run_id)
        if self.checkpoint.run_id != self.run_id:
            raise ArgumentError("connector workflow stage checkpoint run_id does not match context")
        if self.checkpoint.workflow_digest != self.blueprint.workflow.workflow_digest:
            raise ArgumentError("connector workflow stage checkpoint workflow does not match context")
        if isinstance(self.stage_attempt, bool) or not isinstance(self.stage_attempt, int) or self.stage_attempt < 1:
            raise ArgumentError("connector workflow stage attempt must be positive")
        if not isinstance(self.dependency_outputs, Mapping):
            raise ArgumentError("connector workflow stage dependency_outputs must be an object")
        safe = _json_safe(
            "connector workflow stage dependency_outputs",
            dict(self.dependency_outputs),
            maximum=250_000,
        )
        _reject_secret_fields(safe)
        object.__setattr__(self, "dependency_outputs", safe)
        if not isinstance(self.completed_stage_ids, Sequence) or isinstance(self.completed_stage_ids, (str, bytes)):
            raise ArgumentError("connector workflow completed_stage_ids must be a sequence")
        if len(set(self.completed_stage_ids)) != len(self.completed_stage_ids):
            raise ArgumentError("connector workflow completed_stage_ids contains duplicates")

    @property
    def task_digest(self) -> str:
        return self.blueprint.spec.task_digest

    @property
    def workflow_digest(self) -> str:
        return self.blueprint.workflow.workflow_digest

    @property
    def stage_digest(self) -> str:
        return content_digest(self.stage.to_dict())

    @property
    def subject_digest(self) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
                "task_digest": self.task_digest,
                "workflow_digest": self.workflow_digest,
                "stage_digest": self.stage_digest,
                "stage_attempt": self.stage_attempt,
            }
        )

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "workflow_id": self.blueprint.workflow.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage.id,
            "stage_digest": self.stage_digest,
            "stage_attempt": self.stage_attempt,
            "dependency_stage_ids": sorted(self.dependency_outputs),
            "completed_stage_ids": list(self.completed_stage_ids),
            "subject_digest": self.subject_digest,
            "retention": "transient_request_builder_context;checkpoint_metadata_only",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorWorkflowStageExecution:
    """One connector stage result plus the transient dispatch outcome."""

    stage_result: AutonomousWorkflowStageResult
    dispatch_result: AutonomousConnectorDispatchResult | None
    selection_plan: AutonomousConnectorSelectionPlan
    replay_recovery_required: bool = False

    def __post_init__(self) -> None:
        if not isinstance(self.stage_result, AutonomousWorkflowStageResult):
            raise ArgumentError("connector workflow stage result is invalid")
        if not isinstance(self.selection_plan, AutonomousConnectorSelectionPlan):
            raise ArgumentError("connector workflow selection plan is invalid")
        if self.dispatch_result is not None and not isinstance(
            self.dispatch_result, AutonomousConnectorDispatchResult
        ):
            raise ArgumentError("connector workflow dispatch result is invalid")
        if not isinstance(self.replay_recovery_required, bool):
            raise ArgumentError("connector workflow replay recovery flag must be boolean")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
            "stage_result": self.stage_result.to_dict(),
            "dispatch": None if self.dispatch_result is None else self.dispatch_result.to_dict(),
            "selection_plan": self.selection_plan.to_dict(),
            "replay_recovery_required": self.replay_recovery_required,
            "retention": "metadata_only;connector_value_transient",
            "secret_material": "never_returned",
        }


class AutonomousConnectorWorkflowAdapter:
    """Plan-bound, approval-aware executor for one prepared workflow stage."""

    def __init__(
        self,
        runtime: AutonomousConnectorRuntime,
        *,
        operation_registry: AutonomousConnectorOperationRegistry | None = None,
        approved: bool = False,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        rehydrate_payload: Callable[[Any], Any] | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
        evidence_runtime: AutonomousEvidenceRuntime | None = None,
        evidence_projector: Any | None = None,
        evidence_evaluator: Any | None = None,
        require_evidence_acceptance: bool | None = None,
        parent_evidence_digests: Sequence[str] = (),
    ) -> None:
        if not isinstance(runtime, AutonomousConnectorRuntime):
            raise ArgumentError("connector workflow adapter requires an AutonomousConnectorRuntime")
        if operation_registry is not None and not isinstance(
            operation_registry, AutonomousConnectorOperationRegistry
        ):
            raise ArgumentError("connector workflow operation_registry is invalid")
        if not isinstance(approved, bool):
            raise ArgumentError("connector workflow approved must be boolean")
        if selection_signals is not None and not isinstance(selection_signals, Mapping):
            raise ArgumentError("connector workflow selection_signals must be an object")
        if rehydrate_payload is not None and not callable(rehydrate_payload):
            raise ArgumentError("connector workflow rehydrate_payload must be callable")
        if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousProtectedRehydrationAdapter):
            raise ArgumentError("connector workflow protected_rehydration adapter is malformed")
        if evidence_runtime is not None and not isinstance(evidence_runtime, AutonomousEvidenceRuntime):
            raise ArgumentError("connector workflow evidence_runtime is invalid")
        if evidence_projector is not None and not callable(getattr(evidence_projector, "project", None)) and not callable(evidence_projector):
            raise ArgumentError("connector workflow evidence_projector is malformed")
        if evidence_evaluator is not None and not callable(getattr(evidence_evaluator, "evaluate", None)) and not callable(evidence_evaluator):
            raise ArgumentError("connector workflow evidence_evaluator is malformed")
        if require_evidence_acceptance is not None and not isinstance(require_evidence_acceptance, bool):
            raise ArgumentError("connector workflow require_evidence_acceptance must be boolean")
        if evidence_runtime is not None and require_evidence_acceptance is not False and (evidence_projector is None or evidence_evaluator is None):
            raise ArgumentError("strict connector workflow evidence requires both a projector and evaluator")
        if not isinstance(parent_evidence_digests, Sequence) or isinstance(parent_evidence_digests, (str, bytes, bytearray)):
            raise ArgumentError("connector workflow parent_evidence_digests must be a sequence")
        for digest in parent_evidence_digests:
            _digest("connector workflow parent evidence digest", digest)
        self.runtime = runtime
        self.registry = runtime.registry
        self.operation_registry = operation_registry or AutonomousConnectorOperationRegistry()
        self.approved = approved
        self.selection_signals = selection_signals
        self.rehydrate_payload = rehydrate_payload
        self.protected_rehydration = protected_rehydration
        self.evidence_runtime = evidence_runtime
        self.evidence_projector = evidence_projector
        self.evidence_evaluator = evidence_evaluator
        self.require_evidence_acceptance = evidence_runtime is not None if require_evidence_acceptance is None else require_evidence_acceptance
        self.parent_evidence_digests = tuple(parent_evidence_digests)

    def _select_plan(
        self,
        context: AutonomousConnectorWorkflowStageContext,
    ) -> tuple[AutonomousConnectorSelectionPlan, Any]:
        domain = context.blueprint.spec.domain
        contracts = self.operation_registry.for_domain(domain)
        if len(contracts) != 1:
            raise ArgumentError(f"connector workflow requires exactly one operation for {domain}")
        contract = contracts[0]
        capability = context.stage.required_capabilities[0]
        if capability not in contract.capabilities:
            raise ArgumentError(
                f"connector operation {contract.operation_id} does not support stage capability {capability}"
            )
        if self.selection_signals is None:
            plan = self.registry.select_for_domains((domain,), capability=capability)
        else:
            plan = self.registry.select_adaptive_for_domains(
                (domain,),
                capability=capability,
                selection_signals=self.selection_signals,
            )
        if not plan.complete:
            raise ArgumentError(f"no connector is selected for {domain}/{capability}")
        row = plan.rows[0]
        registration = self.registry.resolve(row.connector_id)
        missing = sorted(set(context.stage.required_capabilities).difference(registration.manifest.capabilities))
        if missing:
            raise ArgumentError(
                "selected connector does not cover the complete stage capability contract: "
                + ", ".join(missing)
            )
        return plan, contract

    @staticmethod
    def _subject_digest(context: AutonomousConnectorWorkflowStageContext, attempt: int) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
                "task_digest": context.task_digest,
                "workflow_digest": context.workflow_digest,
                "stage_digest": context.stage_digest,
                "stage_attempt": attempt,
            }
        )

    @staticmethod
    def _request(
        context: AutonomousConnectorWorkflowStageContext,
        contract: Any,
        plan: AutonomousConnectorSelectionPlan,
        request_payload: Mapping[str, Any] | None,
        *,
        stable_attempt: int | None = None,
    ) -> Mapping[str, Any]:
        if request_payload is None:
            raw: Mapping[str, Any] = {}
        elif isinstance(request_payload, Mapping):
            raw = request_payload
        else:
            raise ArgumentError("connector workflow request_for_stage must return an object")
        safe = _json_safe(
            "connector workflow stage request",
            dict(raw),
            maximum=MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_REQUEST_BYTES,
        )
        _reject_secret_fields(safe)
        expected_operation = contract.operation_id
        if safe.get("operation_id") not in (None, expected_operation):
            raise ArgumentError("connector workflow request operation_id does not match the domain operation")
        attempt = context.stage_attempt if stable_attempt is None else stable_attempt
        expected_subject = AutonomousConnectorWorkflowAdapter._subject_digest(context, attempt)
        supplied_subject = safe.get("subject_digest", expected_subject)
        subject_digest = _digest("connector workflow subject_digest", supplied_subject)
        safe["operation_id"] = expected_operation
        safe["subject_digest"] = subject_digest
        safe.setdefault("stage_id", context.stage.id)
        safe.setdefault("stage_digest", context.stage_digest)
        safe.setdefault("workflow_digest", context.workflow_digest)
        if stable_attempt is not None and safe.get("stage_attempt", stable_attempt) != stable_attempt:
            raise ArgumentError("connector workflow evidence binding requires a stable stage_attempt of 1")
        safe.setdefault("stage_attempt", attempt)
        safe.setdefault("selection_plan_digest", plan.plan_digest)
        return safe

    @staticmethod
    def _identities(
        context: AutonomousConnectorWorkflowStageContext,
        request: Mapping[str, Any],
        plan: AutonomousConnectorSelectionPlan,
        *,
        stable_attempt: int | None = None,
    ) -> tuple[str, str, str]:
        attempt = context.stage_attempt if stable_attempt is None else stable_attempt
        identity = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
                "run_id": context.run_id,
                "stage_id": context.stage.id,
                "stage_attempt": attempt,
                "subject_digest": request["subject_digest"],
                "selection_plan_digest": plan.plan_digest,
            }
        )
        return (
            _bounded_id("connector workflow dispatch_id", f"workflow-dispatch-{identity[:48]}"),
            _bounded_id("connector workflow execution_id", f"workflow-execution-{identity[:48]}"),
            _bounded_id("connector workflow call_id", f"workflow-call-{identity[:48]}"),
        )

    def _rehydrate(
        self,
        result: AutonomousConnectorDispatchResult,
    ) -> tuple[Any, bool]:
        if result.replay != "replayed" or result.receipt.payload_digest is None or result.value is not None:
            return result.value, False
        if self.rehydrate_payload is None:
            if self.protected_rehydration is None:
                return None, True
        try:
            restored = self.rehydrate_payload(result.receipt) if self.rehydrate_payload is not None else self.protected_rehydration.resolve_receipt(result.receipt.to_dict(), domain=result.receipt.domain, purpose="connector_workflow_payload", value_kind="connector_payload", one_time=False)
            safe = _json_safe(
                "connector workflow rehydrated payload",
                restored,
                maximum=2_000_000,
            )
            _reject_secret_fields(safe)
            if content_digest(safe) != result.receipt.payload_digest:
                return None, True
            return safe, False
        except Exception:
            return None, True

    def _evidence_requirements(
        self,
        context: AutonomousConnectorWorkflowStageContext,
    ) -> tuple[Any, ...]:
        if self.evidence_runtime is None:
            return ()
        requirements = tuple(
            requirement
            for requirement in self.evidence_runtime.plan.requirements
            if requirement.domain == context.blueprint.spec.domain
            and requirement.workflow_id == context.blueprint.workflow.workflow_id
            and requirement.workflow_digest == context.blueprint.workflow.workflow_digest
            and requirement.stage_id == context.stage.id
        )
        expected = tuple(context.stage.evidence_outputs)
        if len(requirements) != len(expected) or any(requirement.label not in expected for requirement in requirements):
            raise ArgumentError(f"evidence plan does not exactly cover workflow stage {context.stage.id}")
        return tuple(sorted(requirements, key=lambda item: item.requirement_id))

    def _execute_evidence(
        self,
        context: AutonomousConnectorWorkflowStageContext,
        result: AutonomousConnectorDispatchResult,
        payload: Any,
    ) -> tuple[Any, bool] | None:
        if self.evidence_runtime is None:
            return None
        requirements = self._evidence_requirements(context)
        source_digest = result.receipt.payload_digest or result.receipt.request_digest
        requests = [
            {
                "requirement_id": requirement.requirement_id,
                "source_id": result.receipt.connector_id,
                "source_digest": source_digest,
                "request_id": f"workflow-evidence-{result.receipt.dispatch_id}-{index}",
                "metadata": {
                    "schema": "bioprism-python-autonomous-connector-evidence-request/0.1",
                    "workflow_id": context.blueprint.workflow.workflow_id,
                    "workflow_digest": context.blueprint.workflow.workflow_digest,
                    "stage_id": context.stage.id,
                    "connector_id": result.receipt.connector_id,
                    "connector_request_digest": result.receipt.request_digest,
                    "connector_status": result.receipt.status,
                    "retention": "metadata_only;connector_value_caller_owned",
                    "secret_material": "never_returned",
                },
            }
            for index, requirement in enumerate(requirements)
        ]
        evidence = self.evidence_runtime.execute(
            requests,
            acquirer=lambda _context: payload,
            projector=self.evidence_projector,
            evaluator=self.evidence_evaluator,
            rehydrate_value=lambda _receipt: payload,
            parent_evidence_digests=tuple(
                self.parent_evidence_digests
                + (result.receipt.request_digest,)
                + ((result.receipt.payload_digest,) if result.receipt.payload_digest else ())
            ),
            reevaluate_pending=True,
        )
        accepted = bool(evidence.receipts) and all(
            receipt.status == "observed"
            and receipt.evaluator_status == "accepted"
            and receipt.requirement_id in receipt.observed_requirement_ids
            for receipt in evidence.receipts
        )
        return evidence, accepted

    @staticmethod
    def _evidence_metadata(result: Any) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-connector-evidence-binding/0.1",
            "status": result.status,
            "result_digest": result.result_digest,
            "receipt_digests": [receipt.receipt_digest for receipt in result.receipts],
            "assessment_digests": [assessment.assessment_digest for assessment in result.assessments],
            "completed_requirement_ids": list(result.completed_requirement_ids),
            "pending_evaluation_requirement_ids": list(result.pending_evaluation_requirement_ids),
            "missing_requirement_ids": list(result.missing_requirement_ids),
            "retention": "metadata_only;connector_value_and_evaluator_payloads_caller_owned",
            "secret_material": "never_returned",
        }

    @staticmethod
    def _stage_plan(
        context: AutonomousConnectorWorkflowStageContext,
        selection_plan: AutonomousConnectorSelectionPlan,
        receipt: Any | None,
        *,
        dispatch: str,
    ) -> dict[str, Any]:
        compiled = compile_autonomous_workflow_stage_execution_plan(
            context.blueprint,
            context.stage,
            provider_tools=(),
        )
        packet = compiled.to_dict()
        packet.update(
            {
                "connector_selection_plan_digest": selection_plan.plan_digest,
                "connector_dispatch": dispatch,
                "connector_receipt": None if receipt is None else receipt.to_dict(),
                "connector_value_retained": False,
                "connector_execution": "caller_owned_executor;provider_invocation_not_implied",
            }
        )
        return packet

    @staticmethod
    def _structured(
        context: AutonomousConnectorWorkflowStageContext,
        result: AutonomousConnectorDispatchResult,
        *,
        recovery_required: bool,
    ) -> tuple[Mapping[str, Any] | None, str | None, tuple[str, ...], tuple[str, ...]]:
        receipt = result.receipt
        if recovery_required:
            return None, None, (), ("connector_payload_rehydration_required",)
        if receipt.status not in {"observed", "partial"}:
            return None, None, (), (receipt.failure_class or "connector_execution_failed",)
        observed = receipt.status == "observed"
        uncertainty = []
        if not observed:
            uncertainty.append("connector returned a partial observation")
        if result.replay == "replayed":
            uncertainty.append("connector payload was caller-rehydrated from its digest")
        structured = {
            "stage_id": context.stage.id,
            "status": "completed" if observed else "proposed",
            "evidence": [
                f"connector:{receipt.connector_id}",
                f"capability:{context.stage.required_capabilities[0]}",
                f"payload:{receipt.payload_digest or 'none'}",
            ],
            "uncertainty": uncertainty,
            "notes": f"connector receipt {receipt.request_digest}",
            "next_actions": []
            if observed
            else ["review partial connector evidence before treating the stage as complete"],
        }
        declared = "completed" if observed else "proposed"
        return structured, declared, tuple(structured["evidence"]), tuple(uncertainty)

    def execute_stage(
        self,
        context: AutonomousConnectorWorkflowStageContext,
        *,
        request_payload: Mapping[str, Any] | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> AutonomousConnectorWorkflowStageExecution:
        if not isinstance(context, AutonomousConnectorWorkflowStageContext):
            raise ArgumentError("connector workflow execute_stage requires typed context")
        selection_plan, contract = self._select_plan(context)
        if context.stage.approval_required and not self.approved:
            stage_plan = self._stage_plan(context, selection_plan, None, dispatch="approval_not_granted")
            stage_result = AutonomousWorkflowStageResult(
                stage=context.stage,
                execution_status="approval_required",
                declared_status=None,
                result=None,
                structured=None,
                validation_errors=("connector_stage_approval_required",),
                attempt=context.stage_attempt,
                stage_execution_plan=stage_plan,
            )
            return AutonomousConnectorWorkflowStageExecution(stage_result, None, selection_plan)
        stable_attempt = 1 if self.evidence_runtime is not None else None
        request = self._request(context, contract, selection_plan, request_payload, stable_attempt=stable_attempt)
        dispatch_id, execution_id, call_id = self._identities(context, request, selection_plan, stable_attempt=stable_attempt)
        parent_digests = (
            context.task_digest,
            context.workflow_digest,
            context.checkpoint.checkpoint_digest,
            context.stage_digest,
            selection_plan.plan_digest,
        )
        if context.dependency_outputs:
            parent_digests += (content_digest(context.dependency_outputs),)
        dispatch_request = AutonomousConnectorDispatchRequest(
            dispatch_id=dispatch_id,
            execution_id=execution_id,
            call_id=call_id,
            connector_id=selection_plan.rows[0].connector_id,
            domains=(context.blueprint.spec.domain,),
            capability=context.stage.required_capabilities[0],
            request=request,
            parent_digests=parent_digests,
            attempt_id=_bounded_id("connector workflow attempt_id", f"a{stable_attempt or context.stage_attempt}"),
            selection_plan_digest=selection_plan.plan_digest,
            approved=self.approved,
        )
        dispatch_result = self.runtime.dispatch_from_plan(
            selection_plan,
            dispatch_request,
            trace_event_callback=trace_event_callback,
        )
        payload, recovery_required = self._rehydrate(dispatch_result)
        if recovery_required:
            stage_result = AutonomousWorkflowStageResult(
                stage=context.stage,
                execution_status="paused",
                declared_status=None,
                result=None,
                structured=None,
                uncertainty=("connector_payload_rehydration_required",),
                attempt=context.stage_attempt,
                stage_execution_plan=self._stage_plan(
                    context,
                    selection_plan,
                    dispatch_result.receipt,
                    dispatch="replayed_recovery_required",
                ),
            )
            return AutonomousConnectorWorkflowStageExecution(
                stage_result,
                replace(dispatch_result, value=None),
                selection_plan,
                replay_recovery_required=True,
            )
        evidence_result = self._execute_evidence(context, dispatch_result, payload) if dispatch_result.receipt.status in {"observed", "partial"} else None
        del payload  # The value remains transient; the structured stage projection is persisted.
        structured, declared, evidence, uncertainty = self._structured(
            context,
            dispatch_result,
            recovery_required=False,
        )
        if structured is not None and evidence_result is not None:
            result, accepted = evidence_result
            structured = {
                **structured,
                "status": "completed" if accepted else "proposed",
                "evidence_runtime": self._evidence_metadata(result),
                "uncertainty": list(structured["uncertainty"]) + ([] if accepted else ["evidence requires explicit evaluator acceptance before stage completion"]),
                "next_actions": list(structured["next_actions"]) if accepted else ["rehydrate the evidence runtime and provide an explicit evaluator verdict"],
            }
            declared = "completed" if accepted else "proposed"
            evidence = tuple(structured["evidence"])
            uncertainty = tuple(structured["uncertainty"])
        receipt = dispatch_result.receipt
        if receipt.failure_class == "approval_required":
            execution_status = "approval_required"
        elif evidence_result is not None and self.require_evidence_acceptance and not evidence_result[1]:
            execution_status = "paused"
        elif receipt.status in {"observed", "partial"}:
            execution_status = "completed"
        else:
            execution_status = "provider_failed"
        stage_plan = self._stage_plan(context, selection_plan, receipt, dispatch="completed")
        response_digest = None
        if structured is not None:
            response_digest = content_digest(
                {
                    "schema": AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA,
                    "receipt": receipt.to_dict(),
                    "structured": structured,
                    "replay": dispatch_result.replay,
                }
            )
        stage_result = AutonomousWorkflowStageResult(
            stage=context.stage,
            execution_status=execution_status,
            declared_status=declared,
            result=None,
            structured=structured,
            evidence=evidence,
            uncertainty=uncertainty,
            validation_errors=(),
            attempt=context.stage_attempt,
            response_digest=response_digest,
            stage_execution_plan=stage_plan,
        )
        return AutonomousConnectorWorkflowStageExecution(stage_result, dispatch_result, selection_plan)


def _checkpoint(
    blueprint: AutonomousTaskBlueprint,
    *,
    run_id: str,
    snapshots: Sequence[Mapping[str, Any]],
    plan_refinement_digest: str | None,
) -> AutonomousWorkflowCheckpoint:
    return AutonomousWorkflowCheckpoint(
        run_id=run_id,
        task_digest=blueprint.spec.task_digest,
        workflow_id=blueprint.workflow.workflow_id,
        workflow_digest=blueprint.workflow.workflow_digest,
        stages=tuple(dict(snapshot) for snapshot in snapshots),
        plan_refinement_digest=plan_refinement_digest,
    )


def run_autonomous_connector_workflow(
    runtime: AutonomousConnectorRuntime,
    *,
    blueprint: AutonomousTaskBlueprint,
    checkpoint: AutonomousWorkflowCheckpoint | Mapping[str, Any] | None = None,
    run_id: str | None = None,
    approved: bool = False,
    retry_blocked: bool = False,
    max_stage_calls: int | None = None,
    request_for_stage: Callable[[AutonomousConnectorWorkflowStageContext], Mapping[str, Any]] | None = None,
    rehydrate_payload: Callable[[Any], Any] | None = None,
    protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
    operation_registry: AutonomousConnectorOperationRegistry | None = None,
    selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
    evidence_runtime: AutonomousEvidenceRuntime | None = None,
    evidence_projector: Any | None = None,
    evidence_evaluator: Any | None = None,
    require_evidence_acceptance: bool | None = None,
    parent_evidence_digests: Sequence[str] = (),
    trace_event_callback: Callable[..., Any] | None = None,
) -> AutonomousWorkflowRun:
    """Execute every ready workflow stage through the connector adapter and checkpoint it."""

    if not isinstance(runtime, AutonomousConnectorRuntime):
        raise ArgumentError("connector workflow runtime is invalid")
    if not isinstance(blueprint, AutonomousTaskBlueprint):
        raise ArgumentError("connector workflow blueprint is invalid")
    if not isinstance(approved, bool) or not isinstance(retry_blocked, bool):
        raise ArgumentError("connector workflow approval and retry flags must be boolean")
    if request_for_stage is not None and not callable(request_for_stage):
        raise ArgumentError("connector workflow request_for_stage must be callable")
    if rehydrate_payload is not None and not callable(rehydrate_payload):
        raise ArgumentError("connector workflow rehydrate_payload must be callable")
    if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousProtectedRehydrationAdapter):
        raise ArgumentError("connector workflow protected_rehydration adapter is malformed")
    if trace_event_callback is not None and not callable(trace_event_callback):
        raise ArgumentError("connector workflow trace_event_callback must be callable")
    if max_stage_calls is None:
        max_stage_calls = len(blueprint.workflow.stages)
    if isinstance(max_stage_calls, bool) or not isinstance(max_stage_calls, int) or not 1 <= max_stage_calls <= MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_CALLS:
        raise ArgumentError("connector workflow max_stage_calls is outside its bound")
    if checkpoint is None:
        current = None
    elif isinstance(checkpoint, AutonomousWorkflowCheckpoint):
        current = checkpoint
    elif isinstance(checkpoint, Mapping):
        current = AutonomousWorkflowCheckpoint.from_dict(checkpoint)
    else:
        raise ArgumentError("connector workflow checkpoint is invalid")
    resolved_run_id = run_id or (current.run_id if current is not None else f"connector-workflow-{uuid.uuid4().hex}")
    _bounded_id("connector workflow run_id", resolved_run_id)
    if current is None:
        current = _checkpoint(
            blueprint,
            run_id=resolved_run_id,
            snapshots=(),
            plan_refinement_digest=None,
        )
    if current.run_id != resolved_run_id:
        raise ArgumentError("connector workflow checkpoint run_id does not match")
    if current.task_digest != blueprint.spec.task_digest:
        raise ArgumentError("connector workflow checkpoint task does not match blueprint")
    if current.workflow_id != blueprint.workflow.workflow_id or current.workflow_digest != blueprint.workflow.workflow_digest:
        raise ArgumentError("connector workflow checkpoint workflow does not match blueprint")
    snapshots: dict[str, dict[str, Any]] = {row["stage_id"]: dict(row) for row in current.stages}
    prior_attempts = {
        stage_id: int(row.get("attempt", 1))
        for stage_id, row in snapshots.items()
    }
    stage_by_id = {stage.id: stage for stage in blueprint.workflow.stages}
    if any(stage_id not in stage_by_id for stage_id in snapshots):
        raise ArgumentError("connector workflow checkpoint contains an unknown stage")
    blocked = {"blocked", "proposed", "not_attempted"}
    if any(row["status"] in blocked for row in snapshots.values()) and not retry_blocked:
        ids = tuple(sorted(stage_id for stage_id, row in snapshots.items() if row["status"] in blocked))
        return AutonomousWorkflowRun(
            resolved_run_id,
            "stage_blocked" if any(row["status"] == "blocked" for row in snapshots.values()) else "stage_proposed",
            blueprint,
            (),
            _checkpoint(
                blueprint,
                run_id=resolved_run_id,
                snapshots=tuple(snapshots.values()),
                plan_refinement_digest=current.plan_refinement_digest,
            ),
            ids,
        )
    if retry_blocked:
        for stage_id in tuple(snapshots):
            if snapshots[stage_id]["status"] in blocked:
                del snapshots[stage_id]
    adapter = AutonomousConnectorWorkflowAdapter(
        runtime,
        operation_registry=operation_registry,
        approved=approved,
        selection_signals=selection_signals,
        rehydrate_payload=rehydrate_payload,
        protected_rehydration=protected_rehydration,
        evidence_runtime=evidence_runtime,
        evidence_projector=evidence_projector,
        evidence_evaluator=evidence_evaluator,
        require_evidence_acceptance=require_evidence_acceptance,
        parent_evidence_digests=parent_evidence_digests,
    )
    stage_results: list[AutonomousWorkflowStageResult] = []
    calls = 0
    while calls < max_stage_calls:
        completed = {
            stage_id
            for stage_id, snapshot in snapshots.items()
            if snapshot.get("status") == "completed" and snapshot.get("execution_status") == "completed"
        }
        ready = next(
            (
                stage
                for stage in blueprint.workflow.stages
                if stage.id not in snapshots and set(stage.depends_on).issubset(completed)
            ),
            None,
        )
        if ready is None:
            remaining = tuple(stage.id for stage in blueprint.workflow.stages if stage.id not in snapshots)
            return AutonomousWorkflowRun(
                resolved_run_id,
                "completed" if not remaining else "stage_blocked",
                blueprint,
                tuple(stage_results),
                _checkpoint(
                    blueprint,
                    run_id=resolved_run_id,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=current.plan_refinement_digest,
                ),
                remaining,
            )
        calls += 1
        context = AutonomousConnectorWorkflowStageContext(
            blueprint=blueprint,
            run_id=resolved_run_id,
            checkpoint=current,
            stage=ready,
            stage_attempt=prior_attempts.get(ready.id, 0) + 1,
            dependency_outputs={
                dependency: snapshots[dependency]["structured"]
                for dependency in ready.depends_on
                if dependency in snapshots
            },
            completed_stage_ids=tuple(sorted(completed)),
        )
        request_payload = None if request_for_stage is None else request_for_stage(context)
        execution = adapter.execute_stage(
            context,
            request_payload=request_payload,
            trace_event_callback=trace_event_callback,
        )
        stage_result = execution.stage_result
        if stage_result.structured is not None:
            declared, evidence, uncertainty, errors = AutonomousTaskOrchestrator._validate_workflow_stage_output(
                ready,
                stage_result.structured,
            )
            stage_result = replace(
                stage_result,
                declared_status=declared,
                evidence=evidence,
                uncertainty=uncertainty,
                validation_errors=errors,
            )
        stage_results.append(stage_result)
        snapshot = stage_result.checkpoint_snapshot()
        if snapshot is not None and not stage_result.validation_errors:
            snapshots[ready.id] = snapshot
        if stage_result.execution_status == "approval_required":
            return AutonomousWorkflowRun(
                resolved_run_id,
                "approval_required",
                blueprint,
                tuple(stage_results),
                _checkpoint(
                    blueprint,
                    run_id=resolved_run_id,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=current.plan_refinement_digest,
                ),
                (ready.id,),
            )
        if stage_result.execution_status == "paused":
            return AutonomousWorkflowRun(
                resolved_run_id,
                "paused",
                blueprint,
                tuple(stage_results),
                _checkpoint(
                    blueprint,
                    run_id=resolved_run_id,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=current.plan_refinement_digest,
                ),
                (ready.id,),
            )
        if stage_result.execution_status != "completed":
            return AutonomousWorkflowRun(
                resolved_run_id,
                "stage_failed",
                blueprint,
                tuple(stage_results),
                _checkpoint(
                    blueprint,
                    run_id=resolved_run_id,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=current.plan_refinement_digest,
                ),
                (ready.id,),
            )
        if stage_result.declared_status != "completed" or stage_result.validation_errors:
            status = {
                "blocked": "stage_blocked",
                "proposed": "stage_proposed",
                "not_attempted": "stage_not_attempted",
            }.get(stage_result.declared_status, "stage_failed")
            return AutonomousWorkflowRun(
                resolved_run_id,
                status,
                blueprint,
                tuple(stage_results),
                _checkpoint(
                    blueprint,
                    run_id=resolved_run_id,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=current.plan_refinement_digest,
                ),
                (ready.id,),
            )
        current = _checkpoint(
            blueprint,
            run_id=resolved_run_id,
            snapshots=tuple(snapshots.values()),
            plan_refinement_digest=current.plan_refinement_digest,
        )
    completed = {
        stage_id
        for stage_id, snapshot in snapshots.items()
        if snapshot.get("status") == "completed" and snapshot.get("execution_status") == "completed"
    }
    next_ids = tuple(
        stage.id
        for stage in blueprint.workflow.stages
        if stage.id not in snapshots and set(stage.depends_on).issubset(completed)
    )
    remaining = tuple(stage.id for stage in blueprint.workflow.stages if stage.id not in snapshots)
    return AutonomousWorkflowRun(
        resolved_run_id,
        "completed" if not remaining else ("paused" if next_ids else "stage_blocked"),
        blueprint,
        tuple(stage_results),
        _checkpoint(
            blueprint,
            run_id=resolved_run_id,
            snapshots=tuple(snapshots.values()),
            plan_refinement_digest=current.plan_refinement_digest,
        ),
        next_ids,
    )


__all__ = [
    "AUTONOMOUS_CONNECTOR_WORKFLOW_ADAPTER_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_REQUEST_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_WORKFLOW_STAGE_CALLS",
    "AutonomousConnectorWorkflowStageContext",
    "AutonomousConnectorWorkflowStageExecution",
    "AutonomousConnectorWorkflowAdapter",
    "run_autonomous_connector_workflow",
]
