"""Credentialless mission execution through reviewed autonomous connectors.

The provider-backed mission path is intentionally separate from this module.  This adapter is
for applications that already have a typed :class:`MissionRequest` and want to execute its
domain-labelled steps through a caller-owned connector portfolio without requiring a model key.

The module supplies the missing application boundary around the existing mission graph:

* capability selection is exact and digest-bound to the operation registry;
* every dispatch is approval-aware and idempotent through ``AutonomousConnectorRuntime``;
* checkpoints retain only step identities, receipt digests, and status metadata;
* replayed connector payloads require caller-owned, digest-verified rehydration;
* dependency outputs remain transient unless the caller explicitly rehydrates them on resume;
* evaluator feedback is accepted only as an explicit caller signal and can be fed into the next
  connector selection through the metadata-only feedback ledger.

This is an execution adapter, not an external-world oracle.  A built-in connector can exercise
the complete contract offline, while production applications may close over a browser session,
repository workspace, data platform, or provider credential inside their connector executor.
The executor remains outside the durable mission state and this module performs no discovery or
network I/O by itself.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_connector_worker import (
    AutonomousConnectorOperationRegistry,
    InMemoryAutonomousConnectorFeedbackLedger,
)
from .autonomous_protected_rehydration import AutonomousProtectedRehydrationAdapter
from .autonomous_connectors import (
    MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorDispatchResult,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
)
from .domain_tools import _json_safe, _reject_secret_fields
from .errors import ArgumentError
from .mission import MAX_MISSION_STEPS, MissionRequest, MissionStep, MissionPolicy


AUTONOMOUS_CONNECTOR_MISSION_SCHEMA = "bioprism-python-autonomous-connector-mission/0.1"
AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA = "bioprism-python-autonomous-connector-planned-mission/0.1"
AUTONOMOUS_CONNECTOR_MISSION_STEP_QUALITY_EVALUATION_SCHEMA = "bioprism-python-autonomous-connector-mission-step-quality-evaluation/0.1"
MAX_AUTONOMOUS_CONNECTOR_MISSION_STEP_CALLS = 256
MAX_AUTONOMOUS_CONNECTOR_MISSION_OUTPUT_BYTES = 2_000_000
AUTONOMOUS_CONNECTOR_MISSION_STEP_STATUSES = (
    "completed",
    "partial",
    "approval_required",
    "refused",
    "failed",
    "reconciliation_required",
    "quality_blocked",
)
AUTONOMOUS_CONNECTOR_MISSION_RUN_STATUSES = (
    "connector_observed",
    "connector_partial",
    "approval_required",
    "refused",
    "error",
    "unknown",
    "failed",
    "reconciliation_required",
    "quality_blocked",
)


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_id(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value or len(value) > 128:
        raise ArgumentError(f"{name} is outside its bound")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in value):
        raise ArgumentError(f"{name} contains unsupported characters")
    return value


def _safe_object(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_CONNECTOR_MISSION_OUTPUT_BYTES) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    safe = _json_safe(name, dict(value), maximum=maximum)
    _reject_secret_fields(safe)
    return safe


def _normalize_step(value: MissionStep | Mapping[str, Any]) -> MissionStep:
    if isinstance(value, MissionStep):
        return value
    if not isinstance(value, Mapping):
        raise ArgumentError("connector mission steps must contain MissionStep objects or mappings")
    required = ("id", "domain", "capability", "objective", "tool")
    missing = [name for name in required if name not in value]
    if missing:
        raise ArgumentError("connector mission step is missing: " + ", ".join(missing))
    return MissionStep(
        id=value["id"],
        domain=value["domain"],
        capability=value["capability"],
        objective=value["objective"],
        tool=value["tool"],
        arguments=value.get("arguments", {}),
        depends_on=tuple(value.get("depends_on", ())),
        required=value.get("required", True),
        bindings=tuple(value.get("bindings", ())),
    )


def _normalize_request(value: MissionRequest | Mapping[str, Any]) -> tuple[MissionRequest, tuple[MissionStep, ...]]:
    if isinstance(value, MissionRequest):
        request = value
    elif isinstance(value, Mapping):
        required = ("mission_id", "goal", "steps")
        missing = [name for name in required if name not in value]
        if missing:
            raise ArgumentError("connector mission request is missing: " + ", ".join(missing))
        request = MissionRequest(
            mission_id=value["mission_id"],
            goal=value["goal"],
            steps=value["steps"],
            policy=value.get("policy"),
            operations_gate_acceptance=value.get("operations_gate_acceptance"),
            claim_requests=value.get("claim_requests", ()),
            evaluator_review=value.get("evaluator_review"),
            workflow_binding=value.get("workflow_binding"),
            route_review=value.get("route_review"),
        )
    else:
        raise ArgumentError("connector mission request must be a MissionRequest or mapping")
    steps = tuple(_normalize_step(value) for value in request.steps)
    if not 1 <= len(steps) <= MAX_MISSION_STEPS:
        raise ArgumentError(f"connector mission steps must contain between 1 and {MAX_MISSION_STEPS} entries")
    if len({step.id for step in steps}) != len(steps):
        raise ArgumentError("connector mission step ids must be unique")
    return request, steps


def connector_mission_planner_steps(
    steps: Sequence[MissionStep | Mapping[str, Any]],
) -> tuple[dict[str, Any], ...]:
    """Project a mission graph into the provider-planner contract.

    Arguments, tool names, bindings, and policy values are intentionally excluded from this
    projection.  The provider may prioritize existing work, but it never receives the material
    needed to invent or authorize a connector call.
    """

    normalized = tuple(_normalize_step(value) for value in steps)
    if not 1 <= len(normalized) <= MAX_MISSION_STEPS:
        raise ArgumentError(f"connector mission planner steps must contain between 1 and {MAX_MISSION_STEPS} entries")
    ids = tuple(step.id for step in normalized)
    if len(set(ids)) != len(ids):
        raise ArgumentError("connector mission planner step ids must be unique")
    known = set(ids)
    for step in normalized:
        if len(set(step.depends_on)) != len(step.depends_on) or step.id in step.depends_on:
            raise ArgumentError(f"connector mission planner step {step.id} has invalid dependencies")
        if any(dependency not in known for dependency in step.depends_on):
            raise ArgumentError(f"connector mission planner step {step.id} depends on an unknown step")
    return tuple(
        {
            "id": step.id,
            "domain": step.domain,
            "capability": step.capability,
            "objective": step.objective,
            "depends_on": list(step.depends_on),
            "required": step.required,
        }
        for step in normalized
    )


def connector_mission_protected_contract_digest(
    mission: MissionRequest | Mapping[str, Any],
    *,
    steps: Sequence[MissionStep | Mapping[str, Any]] | None = None,
) -> str:
    """Return an order-independent digest for a connector mission's protected contract.

    An accepted provider ordering may change only the sequence of already-reviewed steps.  The
    digest therefore sorts full step descriptors by id while retaining arguments, bindings,
    policy, claims, route reviews, and every other caller-owned authorization input.
    """

    request, normalized_steps = _normalize_request(mission)
    selected_steps = normalized_steps if steps is None else tuple(_normalize_step(value) for value in steps)
    if (
        len(selected_steps) != len(normalized_steps)
        or len({step.id for step in selected_steps}) != len(selected_steps)
        or {step.id for step in selected_steps} != {step.id for step in normalized_steps}
    ):
        raise ArgumentError("connector mission protected contract steps do not match the mission")
    arguments = request.to_mcp_arguments()
    descriptor = {key: value for key, value in arguments.items() if key != "steps"}
    descriptor["steps"] = sorted(
        (step.to_dict() for step in selected_steps),
        key=lambda value: str(value["id"]),
    )
    return content_digest(descriptor)


def apply_autonomous_ordered_step_plan(
    mission: MissionRequest | Mapping[str, Any],
    refinement: Any,
    *,
    protected_contract_digest: str | None = None,
) -> MissionRequest:
    """Apply one explicitly accepted ordered-step proposal to a connector mission.

    This is the only promotion point from provider planning into connector scheduling.  It
    requires a completed, non-review proposal with an exact permutation of the existing graph,
    verifies every dependency edge, and rechecks the order-independent protected contract after
    rebuilding the request.  It never changes tools, arguments, bindings, policy, or approvals.
    """

    from .autonomy import AutonomousOrderedStepPlanRefinementResult

    if not isinstance(refinement, AutonomousOrderedStepPlanRefinementResult):
        raise ArgumentError("connector mission refinement must be an AutonomousOrderedStepPlanRefinementResult")
    request, steps = _normalize_request(mission)
    if refinement.status != "completed" or refinement.review_required:
        raise ArgumentError("only a completed, non-review connector mission plan may be accepted")
    expected_task_digest = content_digest({"task": request.goal})
    expected_base_digest = content_digest({"steps": list(connector_mission_planner_steps(steps))})
    if refinement.task_digest != expected_task_digest:
        raise ArgumentError("connector mission plan task does not match the mission goal")
    if refinement.base_plan_digest != expected_base_digest:
        raise ArgumentError("connector mission plan base does not match the mission step graph")
    expected_contract = connector_mission_protected_contract_digest(request, steps=steps)
    if protected_contract_digest is not None and protected_contract_digest != expected_contract:
        raise ArgumentError("connector mission protected contract digest does not match the mission")
    if refinement.protected_contract_digest not in (None, expected_contract):
        raise ArgumentError("connector mission plan protected contract does not match the mission")

    ids = tuple(step.id for step in steps)
    known_ids = set(ids)
    if any(dependency not in known_ids for step in steps for dependency in step.depends_on):
        raise ArgumentError("connector mission plan references an unknown dependency")
    priority = tuple(refinement.priority_step_ids)
    if len(priority) != len(ids) or len(set(priority)) != len(priority) or set(priority) != set(ids):
        raise ArgumentError("connector mission plan must contain every step exactly once")
    positions = {step_id: index for index, step_id in enumerate(priority)}
    if any(
        positions[dependency] > positions[step.id]
        for step in steps
        for dependency in step.depends_on
    ):
        raise ArgumentError("connector mission plan violates step dependencies")
    by_id = {step.id: step for step in steps}
    reordered = tuple(by_id[step_id] for step_id in priority)
    rebuilt = MissionRequest(
        mission_id=request.mission_id,
        goal=request.goal,
        steps=reordered,
        policy=request.policy,
        operations_gate_acceptance=request.operations_gate_acceptance,
        claim_requests=request.claim_requests,
        evaluator_review=request.evaluator_review,
        workflow_binding=request.workflow_binding,
        route_review=request.route_review,
    )
    if connector_mission_protected_contract_digest(rebuilt) != expected_contract:
        raise ArgumentError("accepted connector mission plan changed the protected contract")
    return rebuilt


def _policy_max_steps(request: MissionRequest) -> int:
    policy = request.policy
    if policy is None:
        return MAX_MISSION_STEPS
    if isinstance(policy, MissionPolicy):
        return policy.max_steps
    if isinstance(policy, Mapping):
        maximum = policy.get("max_steps", MAX_MISSION_STEPS)
        if isinstance(maximum, bool) or not isinstance(maximum, int) or not 1 <= maximum <= MAX_MISSION_STEPS:
            raise ArgumentError("connector mission policy.max_steps is outside its bound")
        return maximum
    raise ArgumentError("connector mission policy is invalid")


def _waves(steps: Sequence[MissionStep]) -> tuple[tuple[str, ...], ...]:
    by_id = {step.id: step for step in steps}
    remaining: dict[str, set[str]] = {}
    for step in steps:
        dependencies = set(step.depends_on)
        if step.id in dependencies:
            raise ArgumentError(f"connector mission step {step.id} depends on itself")
        if len(dependencies) != len(step.depends_on):
            raise ArgumentError(f"connector mission step {step.id} contains duplicate dependencies")
        unknown = sorted(dependencies.difference(by_id))
        if unknown:
            raise ArgumentError(
                f"connector mission step {step.id} has unknown dependencies: {', '.join(unknown)}"
            )
        remaining[step.id] = dependencies
    result: list[tuple[str, ...]] = []
    while remaining:
        ready = tuple(sorted(step_id for step_id, dependencies in remaining.items() if not dependencies))
        if not ready:
            raise ArgumentError("connector mission dependency graph contains a cycle")
        result.append(ready)
        for step_id in ready:
            remaining.pop(step_id, None)
        for dependencies in remaining.values():
            dependencies.difference_update(ready)
    return tuple(result)


@dataclass(frozen=True, slots=True)
class AutonomousConnectorMissionStepContext:
    """Transient step context passed to a caller-owned request builder."""

    mission_id: str
    goal: str
    mission_digest: str
    goal_digest: str
    step: MissionStep
    execution_attempt: int
    dependency_outputs: Mapping[str, Any]
    completed_step_ids: tuple[str, ...]

    def __post_init__(self) -> None:
        _bounded_id("connector mission_id", self.mission_id)
        if not isinstance(self.goal, str) or not self.goal.strip():
            raise ArgumentError("connector mission goal must be a non-empty string")
        _digest("connector mission_digest", self.mission_digest)
        _digest("connector goal_digest", self.goal_digest)
        if not isinstance(self.step, MissionStep):
            raise ArgumentError("connector mission step context step is invalid")
        if isinstance(self.execution_attempt, bool) or not isinstance(self.execution_attempt, int) or self.execution_attempt < 1:
            raise ArgumentError("connector mission execution_attempt must be positive")
        safe = _safe_object("connector mission dependency_outputs", self.dependency_outputs)
        object.__setattr__(self, "dependency_outputs", safe)
        if not isinstance(self.completed_step_ids, Sequence) or isinstance(self.completed_step_ids, (str, bytes)):
            raise ArgumentError("connector mission completed_step_ids must be a sequence")
        if len(set(self.completed_step_ids)) != len(self.completed_step_ids):
            raise ArgumentError("connector mission completed_step_ids contains duplicates")

    @property
    def step_digest(self) -> str:
        return content_digest(self.step.to_dict())

    @property
    def arguments_digest(self) -> str:
        return content_digest(dict(self.step.arguments or {}))

    @property
    def subject_digest(self) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
                "mission_digest": self.mission_digest,
                "goal_digest": self.goal_digest,
                "step_digest": self.step_digest,
                "attempt": self.execution_attempt,
                "arguments_digest": self.arguments_digest,
            }
        )

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
            "mission_id": self.mission_id,
            "mission_digest": self.mission_digest,
            "goal_digest": self.goal_digest,
            "step_id": self.step.id,
            "step_digest": self.step_digest,
            "domain": self.step.domain,
            "capability": self.step.capability,
            "tool": self.step.tool,
            "execution_attempt": self.execution_attempt,
            "dependency_step_ids": sorted(self.dependency_outputs),
            "completed_step_ids": list(self.completed_step_ids),
            "subject_digest": self.subject_digest,
            "retention": "transient_request_builder_context;checkpoint_metadata_only",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorMissionStepQualityContext:
    """Transient raw result context supplied to a caller-owned mission evaluator."""

    mission_id: str
    goal_digest: str
    wave: int
    attempt: int
    step: MissionStep
    result: Any
    result_digest: str

    def __post_init__(self) -> None:
        _bounded_id("connector mission quality mission_id", self.mission_id)
        _digest("connector mission quality goal_digest", self.goal_digest)
        if isinstance(self.wave, bool) or not isinstance(self.wave, int) or self.wave < 0:
            raise ArgumentError("connector mission quality wave must be a non-negative integer")
        if isinstance(self.attempt, bool) or not isinstance(self.attempt, int) or self.attempt < 1:
            raise ArgumentError("connector mission quality attempt must be positive")
        if not isinstance(self.step, MissionStep):
            raise ArgumentError("connector mission quality step is invalid")
        _digest("connector mission quality result_digest", self.result_digest)


def _normalize_quality_projection(
    value: Mapping[str, Any],
    *,
    context: AutonomousConnectorMissionStepQualityContext | None = None,
) -> dict[str, Any]:
    """Normalize a reward input into a metadata-only, identity-bound quality projection."""

    safe = _safe_object("connector mission quality evaluation", value, maximum=32_000)
    projection = safe.get("schema") == AUTONOMOUS_CONNECTOR_MISSION_STEP_QUALITY_EVALUATION_SCHEMA
    allowed = {
        "schema", "evaluator_id", "evaluator_version", "domain", "mission_id", "goal_digest", "step_id",
        "step_digest", "result_digest", "reward", "passed", "failed", "failure_class", "feedback_digest",
        "evidence_digest", "evaluator_authority", "retention", "secret_material", "evaluation_digest",
    }
    input_allowed = {
        "evaluator_id", "evaluator_version", "reward", "passed", "failed", "failure_class",
        "feedback_digest", "evidence_digest",
    }
    if projection and set(safe) != allowed:
        raise ArgumentError("connector mission quality evaluation is missing or carrying unsupported fields")
    if not projection and set(safe).difference(input_allowed):
        raise ArgumentError("connector mission quality evaluator returned unsupported fields")
    evaluator_id = _bounded_id("connector mission quality evaluator_id", safe.get("evaluator_id"))
    evaluator_version = _bounded_id("connector mission quality evaluator_version", safe.get("evaluator_version"))
    domain = _bounded_id(
        "connector mission quality domain",
        safe.get("domain") if safe.get("domain") is not None else None if context is None else context.step.domain,
    )
    mission_id = _bounded_id(
        "connector mission quality mission_id",
        safe.get("mission_id") if safe.get("mission_id") is not None else None if context is None else context.mission_id,
    )
    step_id = _bounded_id(
        "connector mission quality step_id",
        safe.get("step_id") if safe.get("step_id") is not None else None if context is None else context.step.id,
    )
    goal_digest = _digest(
        "connector mission quality goal_digest",
        safe.get("goal_digest") if safe.get("goal_digest") is not None else None if context is None else context.goal_digest,
    )
    step_digest = _digest(
        "connector mission quality step_digest",
        safe.get("step_digest") if safe.get("step_digest") is not None else None if context is None else content_digest(context.step.to_dict()),
    )
    result_digest = _digest(
        "connector mission quality result_digest",
        safe.get("result_digest") if safe.get("result_digest") is not None else None if context is None else context.result_digest,
    )
    if context is not None and (
        mission_id != context.mission_id
        or domain != context.step.domain
        or step_id != context.step.id
        or goal_digest != context.goal_digest
        or step_digest != content_digest(context.step.to_dict())
        or result_digest != context.result_digest
    ):
        raise ArgumentError("connector mission quality evaluation is not bound to the scheduled result")
    reward = safe.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not 0 <= float(reward) <= 1:
        raise ArgumentError("connector mission quality reward is outside [0, 1]")
    passed = safe.get("passed")
    if not isinstance(passed, bool):
        raise ArgumentError("connector mission quality passed flag is invalid")
    failed = safe.get("failed", not passed)
    if not isinstance(failed, bool) or failed == passed:
        raise ArgumentError("connector mission quality passed and failed flags are inconsistent")
    failure_class = safe.get("failure_class")
    if failure_class is None:
        failure_class = "MissionStepQualityGateRejected" if failed else None
    else:
        failure_class = _bounded_id("connector mission quality failure_class", failure_class)
    if passed and failure_class is not None:
        raise ArgumentError("passed connector mission quality evaluations cannot contain failure_class")
    evidence_digest = safe.get("evidence_digest")
    _digest("connector mission quality evidence_digest", evidence_digest, allow_none=True)
    feedback_digest = safe.get("feedback_digest")
    if feedback_digest is None:
        feedback_digest = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_MISSION_STEP_QUALITY_EVALUATION_SCHEMA,
                "evaluator_id": evaluator_id,
                "evaluator_version": evaluator_version,
                "mission_id": mission_id,
                "step_id": step_id,
                "result_digest": result_digest,
                "reward": float(reward),
                "passed": passed,
                "failed": failed,
                "failure_class": failure_class,
                "evidence_digest": evidence_digest,
            }
        )
    else:
        _digest("connector mission quality feedback_digest", feedback_digest)
    descriptor = {
        "schema": AUTONOMOUS_CONNECTOR_MISSION_STEP_QUALITY_EVALUATION_SCHEMA,
        "evaluator_id": evaluator_id,
        "evaluator_version": evaluator_version,
        "domain": domain,
        "mission_id": mission_id,
        "goal_digest": goal_digest,
        "step_id": step_id,
        "step_digest": step_digest,
        "result_digest": result_digest,
        "reward": float(reward),
        "passed": passed,
        "failed": failed,
        "failure_class": failure_class,
        "feedback_digest": feedback_digest,
        "evidence_digest": evidence_digest,
        "evaluator_authority": "caller_declared_signal_scoring_only",
        "retention": "value_only;step_result_not_retained",
        "secret_material": "never_returned",
    }
    evaluation_digest = content_digest(descriptor)
    if projection and (
        safe.get("evaluator_authority") != descriptor["evaluator_authority"]
        or safe.get("retention") != descriptor["retention"]
        or safe.get("secret_material") != descriptor["secret_material"]
        or safe.get("evaluation_digest") != evaluation_digest
    ):
        raise ArgumentError("connector mission quality evaluation authority, retention, or digest is invalid")
    return {**descriptor, "evaluation_digest": evaluation_digest}


@dataclass(frozen=True, slots=True)
class AutonomousConnectorMissionStepExecution:
    """Transient connector value paired with a metadata-only mission result."""

    step_id: str
    status: str
    run_status: str
    selection_plan: AutonomousConnectorSelectionPlan
    dispatch_result: AutonomousConnectorDispatchResult | None = None
    value: Any = None
    error_class: str | None = None
    detail: str | None = None
    decision: Mapping[str, Any] | None = None
    replay_recovery_required: bool = False
    quality: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _bounded_id("connector mission execution step_id", self.step_id)
        if self.status not in AUTONOMOUS_CONNECTOR_MISSION_STEP_STATUSES:
            raise ArgumentError("connector mission step execution status is invalid")
        if self.run_status not in AUTONOMOUS_CONNECTOR_MISSION_RUN_STATUSES:
            raise ArgumentError("connector mission step run_status is invalid")
        if not isinstance(self.selection_plan, AutonomousConnectorSelectionPlan):
            raise ArgumentError("connector mission step selection plan is invalid")
        if self.dispatch_result is not None and not isinstance(self.dispatch_result, AutonomousConnectorDispatchResult):
            raise ArgumentError("connector mission step dispatch result is invalid")
        if self.error_class is not None:
            _bounded_id("connector mission execution error_class", self.error_class)
        if self.detail is not None and (not isinstance(self.detail, str) or len(self.detail.encode("utf-8")) > 512):
            raise ArgumentError("connector mission execution detail is outside its bound")
        if self.decision is not None:
            safe = _safe_object("connector mission execution decision", self.decision, maximum=32_000)
            object.__setattr__(self, "decision", safe)
        if not isinstance(self.replay_recovery_required, bool):
            raise ArgumentError("connector mission replay recovery flag must be boolean")
        if self.quality is not None:
            object.__setattr__(self, "quality", _normalize_quality_projection(self.quality))

    @property
    def receipt(self) -> Any | None:
        return None if self.dispatch_result is None else self.dispatch_result.receipt

    def to_dict(self) -> dict[str, Any]:
        receipt = self.receipt
        return {
            "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
            "step_id": self.step_id,
            "status": self.status,
            "run_status": self.run_status,
            "selection_plan": self.selection_plan.to_dict(),
            "dispatch": None if self.dispatch_result is None else self.dispatch_result.to_dict(),
            "receipt_digest": None if receipt is None else content_digest(receipt.to_dict()),
            "payload_digest": None if receipt is None else receipt.payload_digest,
            "error_class": self.error_class,
            "detail": self.detail,
            "decision": None if self.decision is None else dict(self.decision),
            "replay_recovery_required": self.replay_recovery_required,
            "quality": None if self.quality is None else dict(self.quality),
            "value_retained": False,
            "retention": "metadata_only;connector_value_transient",
            "secret_material": "never_returned",
        }


class AutonomousConnectorMissionAdapter:
    """Plan-bound, approval-aware executor for one mission step."""

    def __init__(
        self,
        runtime: AutonomousConnectorRuntime,
        *,
        operation_registry: AutonomousConnectorOperationRegistry | None = None,
        approved: bool = False,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        feedback_ledger: InMemoryAutonomousConnectorFeedbackLedger | None = None,
        rehydrate_payload: Callable[[Any], Any] | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
    ) -> None:
        if not isinstance(runtime, AutonomousConnectorRuntime):
            raise ArgumentError("connector mission adapter requires an AutonomousConnectorRuntime")
        if operation_registry is not None and not isinstance(operation_registry, AutonomousConnectorOperationRegistry):
            raise ArgumentError("connector mission operation_registry is invalid")
        if not isinstance(approved, bool):
            raise ArgumentError("connector mission approved must be boolean")
        if selection_signals is not None and not isinstance(selection_signals, Mapping):
            raise ArgumentError("connector mission selection_signals must be an object")
        if feedback_ledger is not None and not isinstance(feedback_ledger, InMemoryAutonomousConnectorFeedbackLedger):
            raise ArgumentError("connector mission feedback_ledger is invalid")
        if rehydrate_payload is not None and not callable(rehydrate_payload):
            raise ArgumentError("connector mission rehydrate_payload must be callable")
        if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousProtectedRehydrationAdapter):
            raise ArgumentError("connector mission protected_rehydration adapter is malformed")
        self.runtime = runtime
        self.registry = runtime.registry
        self.operation_registry = operation_registry or AutonomousConnectorOperationRegistry()
        self.approved = approved
        self.selection_signals = None if selection_signals is None else {key: dict(value) for key, value in selection_signals.items()}
        self.feedback_ledger = feedback_ledger
        self.rehydrate_payload = rehydrate_payload
        self.protected_rehydration = protected_rehydration

    def _signals(self, domain: str, capability: str) -> Mapping[str, Mapping[str, Any]] | None:
        signals = {} if self.selection_signals is None else {key: dict(value) for key, value in self.selection_signals.items()}
        if self.feedback_ledger is not None:
            signals.update(self.feedback_ledger.signals(domain=domain, capability=capability))
        return signals or None

    def _select_plan(self, context: AutonomousConnectorMissionStepContext) -> AutonomousConnectorSelectionPlan:
        contracts = self.operation_registry.for_domain(context.step.domain)
        if len(contracts) != 1:
            raise ArgumentError(f"connector mission requires exactly one operation for {context.step.domain}")
        contract = contracts[0]
        if not contract.supports(context.step.capability):
            raise ArgumentError(
                f"connector operation {contract.operation_id} does not support {context.step.capability}"
            )
        signals = self._signals(context.step.domain, context.step.capability)
        if signals is None:
            plan = self.registry.select_for_domains(
                (context.step.domain,), capability=context.step.capability
            )
        else:
            plan = self.registry.select_adaptive_for_domains(
                (context.step.domain,),
                capability=context.step.capability,
                selection_signals=signals,
            )
        if not plan.complete:
            raise ArgumentError(
                f"no connector is selected for {context.step.domain}/{context.step.capability}"
            )
        return plan

    @staticmethod
    def _request(
        context: AutonomousConnectorMissionStepContext,
        plan: AutonomousConnectorSelectionPlan,
        operation_registry: AutonomousConnectorOperationRegistry,
        request_payload: Mapping[str, Any] | None,
    ) -> Mapping[str, Any]:
        if request_payload is None:
            arguments = dict(context.step.arguments or {})
            raw: Mapping[str, Any] = {
                "mission_id": context.mission_id,
                "step_id": context.step.id,
                "domain": context.step.domain,
                "capability": context.step.capability,
                "objective": context.step.objective,
                "goal_digest": context.goal_digest,
                "arguments": arguments,
                **arguments,
            }
        elif isinstance(request_payload, Mapping):
            raw = request_payload
        else:
            raise ArgumentError("connector mission request_for_step must return an object")
        safe = _safe_object(
            "connector mission step request",
            raw,
            maximum=MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES,
        )
        expected = next(iter(plan.rows)).domain
        contracts = operation_registry.for_domain(expected)
        if len(contracts) != 1:
            raise ArgumentError("connector mission operation registry has an ambiguous domain")
        contract = contracts[0]
        if safe.get("operation_id") is not None:
            if safe["operation_id"] != contract.operation_id:
                raise ArgumentError("connector mission request operation_id does not match its domain")
        safe.setdefault("operation_id", contract.operation_id)
        safe.setdefault("mission_id", context.mission_id)
        safe.setdefault("step_id", context.step.id)
        safe.setdefault("domain", context.step.domain)
        safe.setdefault("capability", context.step.capability)
        safe.setdefault("goal_digest", context.goal_digest)
        safe.setdefault("selection_plan_digest", plan.plan_digest)
        safe["subject_digest"] = _digest(
            "connector mission request subject_digest",
            safe.get("subject_digest", context.subject_digest),
        )
        return safe

    @staticmethod
    def _identities(
        context: AutonomousConnectorMissionStepContext,
        request: Mapping[str, Any],
        plan: AutonomousConnectorSelectionPlan,
    ) -> tuple[str, str, str]:
        identity = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
                "mission_id": context.mission_id,
                "step_id": context.step.id,
                "attempt": context.execution_attempt,
                "subject_digest": request["subject_digest"],
                "selection_plan_digest": plan.plan_digest,
            }
        )
        return (
            _bounded_id("connector mission dispatch_id", f"mission-dispatch-{identity[:48]}"),
            _bounded_id("connector mission execution_id", f"mission-execution-{identity[:48]}"),
            _bounded_id("connector mission call_id", f"mission-call-{identity[:48]}"),
        )

    def _rehydrate(self, result: AutonomousConnectorDispatchResult) -> tuple[Any, bool]:
        if result.replay != "replayed" or result.receipt.payload_digest is None or result.value is not None:
            return result.value, False
        if self.rehydrate_payload is None:
            if self.protected_rehydration is None:
                return None, True
        try:
            restored = self.rehydrate_payload(result.receipt) if self.rehydrate_payload is not None else self.protected_rehydration.resolve_receipt(result.receipt.to_dict(), domain=result.receipt.domain, purpose="connector_mission_payload", value_kind="connector_payload", one_time=False)
            safe = _json_safe(
                "connector mission rehydrated payload",
                restored,
                maximum=MAX_AUTONOMOUS_CONNECTOR_MISSION_OUTPUT_BYTES,
            )
            _reject_secret_fields(safe)
            if content_digest(safe) != result.receipt.payload_digest:
                return None, True
            return safe, False
        except Exception:
            return None, True

    def execute_step(
        self,
        context: AutonomousConnectorMissionStepContext,
        *,
        request_payload: Mapping[str, Any] | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> AutonomousConnectorMissionStepExecution:
        if not isinstance(context, AutonomousConnectorMissionStepContext):
            raise ArgumentError("connector mission execute_step requires typed context")
        plan = self._select_plan(context)
        request = self._request(context, plan, self.operation_registry, request_payload)
        dispatch_id, execution_id, call_id = self._identities(context, request, plan)
        parent_digests = (
            context.mission_digest,
            context.goal_digest,
            context.step_digest,
            context.arguments_digest,
            plan.plan_digest,
        )
        if context.dependency_outputs:
            parent_digests += (content_digest(context.dependency_outputs),)
        request_obj = AutonomousConnectorDispatchRequest(
            dispatch_id=dispatch_id,
            execution_id=execution_id,
            call_id=call_id,
            connector_id=plan.rows[0].connector_id,
            domains=(context.step.domain,),
            capability=context.step.capability,
            request=request,
            parent_digests=parent_digests,
            attempt_id=_bounded_id("connector mission attempt_id", f"a{context.execution_attempt}"),
            selection_plan_digest=plan.plan_digest,
            approved=self.approved,
        )
        result = self.runtime.dispatch_from_plan(
            plan,
            request_obj,
            trace_event_callback=trace_event_callback,
        )
        value, recovery_required = self._rehydrate(result)
        decision = {
            "selection_digest": plan.plan_digest,
            "provider": result.receipt.provider,
            "model": result.receipt.connector_version,
            "route_digest": None,
            "plan_digest": plan.plan_digest,
            "prompt_digest": content_digest(
                {"step_digest": context.step_digest, "request_digest": request_obj.request_digest}
            ),
        }
        if recovery_required:
            return AutonomousConnectorMissionStepExecution(
                context.step.id,
                "reconciliation_required",
                "reconciliation_required",
                plan,
                replace_dispatch_value(result, None),
                None,
                "rehydration_missing",
                "connector receipt is replayed but its caller-owned payload was not rehydrated",
                decision,
                True,
            )
        failure = result.receipt.failure_class
        if failure == "approval_required":
            return AutonomousConnectorMissionStepExecution(
                context.step.id, "approval_required", "approval_required", plan, result, None,
                failure, "connector dispatch requires explicit approval", decision,
            )
        if result.receipt.status == "refused":
            return AutonomousConnectorMissionStepExecution(
                context.step.id, "refused", "refused", plan, result, None,
                failure or "connector_refused", "connector dispatch was refused by scope or policy", decision,
            )
        if result.receipt.status in {"error", "unknown"}:
            return AutonomousConnectorMissionStepExecution(
                context.step.id, "failed", result.receipt.status, plan, result, None,
                failure or "connector_execution_failed", "connector dispatch did not produce an observation", decision,
            )
        return AutonomousConnectorMissionStepExecution(
            context.step.id,
            "completed" if result.receipt.status == "observed" else "partial",
            "connector_observed" if result.receipt.status == "observed" else "connector_partial",
            plan,
            result,
            value,
            None,
            None if result.receipt.status == "observed" else "connector returned a partial observation",
            decision,
        )

    def settle_evaluator_feedback(
        self,
        execution: AutonomousConnectorMissionStepExecution,
        feedback: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Record caller-owned reward after execution; transport status never becomes reward."""

        if self.feedback_ledger is None:
            raise ArgumentError("connector mission feedback settlement requires a feedback_ledger")
        if not isinstance(execution, AutonomousConnectorMissionStepExecution) or execution.receipt is None:
            raise ArgumentError("connector mission feedback requires a dispatched step execution")
        return self.feedback_ledger.record(feedback=feedback, receipt=execution.receipt)


def replace_dispatch_value(result: AutonomousConnectorDispatchResult, value: Any) -> AutonomousConnectorDispatchResult:
    """Rebuild a dispatch result without retaining a replayed payload."""

    return AutonomousConnectorDispatchResult(result.receipt, value, replay=result.replay)


def _snapshot(step: MissionStep, execution: AutonomousConnectorMissionStepExecution, *, attempt: int) -> dict[str, Any]:
    receipt = execution.receipt
    return {
        "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
        "step_id": step.id,
        "step_digest": content_digest(step.to_dict()),
        "domain": step.domain,
        "capability": step.capability,
        "status": execution.status,
        "run_status": execution.run_status,
        "attempt": attempt,
        "selection_plan_digest": execution.selection_plan.plan_digest,
        "receipt_digest": None if receipt is None else content_digest(receipt.to_dict()),
        "payload_digest": None if receipt is None else receipt.payload_digest,
        "error_class": execution.error_class,
        "replay_recovery_required": execution.replay_recovery_required,
        "quality": None if execution.quality is None else dict(execution.quality),
        "retention": "metadata_only_no_request_or_payload",
        "secret_material": "never_returned",
    }


def _checkpoint(
    request: MissionRequest,
    steps: Sequence[MissionStep],
    snapshots: Mapping[str, Mapping[str, Any]],
    *,
    mission_digest: str,
    goal_digest: str,
) -> dict[str, Any]:
    rows = [dict(snapshots[step.id]) for step in steps if step.id in snapshots]
    return {
        "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
        "mission_id": request.mission_id,
        "goal_digest": goal_digest,
        "mission_digest": mission_digest,
        "steps": rows,
        "completed_step_ids": [row["step_id"] for row in rows if row["status"] == "completed"],
        "retention": "metadata_only_step_status_receipt_and_plan_digests",
        "secret_material": "never_returned",
    }


def _validate_checkpoint(
    value: Mapping[str, Any],
    request: MissionRequest,
    steps: Sequence[MissionStep],
    *,
    mission_digest: str,
    goal_digest: str,
) -> dict[str, dict[str, Any]]:
    expected = {
        "schema", "mission_id", "goal_digest", "mission_digest", "steps", "completed_step_ids",
        "retention", "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected:
        raise ArgumentError("connector mission checkpoint is malformed")
    if value.get("schema") != AUTONOMOUS_CONNECTOR_MISSION_SCHEMA or value.get("mission_id") != request.mission_id:
        raise ArgumentError("connector mission checkpoint identity is stale")
    if value.get("retention") != "metadata_only_step_status_receipt_and_plan_digests" or value.get("secret_material") != "never_returned":
        raise ArgumentError("connector mission checkpoint retention is invalid")
    if value.get("mission_digest") != mission_digest or value.get("goal_digest") != goal_digest:
        raise ArgumentError("connector mission checkpoint does not match the mission")
    raw_steps = value.get("steps")
    if not isinstance(raw_steps, Sequence) or isinstance(raw_steps, (str, bytes)):
        raise ArgumentError("connector mission checkpoint steps must be a sequence")
    by_id = {step.id: step for step in steps}
    snapshots: dict[str, dict[str, Any]] = {}
    required = {
        "schema", "step_id", "step_digest", "domain", "capability", "status", "run_status", "attempt",
        "selection_plan_digest", "receipt_digest", "payload_digest", "error_class",
        "replay_recovery_required", "retention", "secret_material",
    }
    allowed = required | {"quality"}
    for raw in raw_steps:
        if not isinstance(raw, Mapping) or not required.issubset(raw) or set(raw).difference(allowed):
            raise ArgumentError("connector mission checkpoint step is malformed")
        step_id = _bounded_id("connector mission checkpoint step_id", raw.get("step_id"))
        step = by_id.get(step_id)
        if step is None:
            raise ArgumentError("connector mission checkpoint contains an unknown step")
        if raw.get("step_digest") != content_digest(step.to_dict()) or raw.get("domain") != step.domain or raw.get("capability") != step.capability:
            raise ArgumentError("connector mission checkpoint step identity is stale")
        if raw.get("status") not in AUTONOMOUS_CONNECTOR_MISSION_STEP_STATUSES or raw.get("run_status") not in AUTONOMOUS_CONNECTOR_MISSION_RUN_STATUSES:
            raise ArgumentError("connector mission checkpoint step status is invalid")
        attempt = raw.get("attempt")
        if isinstance(attempt, bool) or not isinstance(attempt, int) or not 1 <= attempt <= MAX_AUTONOMOUS_CONNECTOR_MISSION_STEP_CALLS:
            raise ArgumentError("connector mission checkpoint step attempt is invalid")
        _digest("connector mission checkpoint selection_plan_digest", raw.get("selection_plan_digest"))
        _digest("connector mission checkpoint receipt_digest", raw.get("receipt_digest"), allow_none=True)
        _digest("connector mission checkpoint payload_digest", raw.get("payload_digest"), allow_none=True)
        if not isinstance(raw.get("replay_recovery_required"), bool):
            raise ArgumentError("connector mission checkpoint replay flag is invalid")
        if raw.get("retention") != "metadata_only_no_request_or_payload" or raw.get("secret_material") != "never_returned":
            raise ArgumentError("connector mission checkpoint step retention is invalid")
        if raw.get("quality") is not None:
            quality = _normalize_quality_projection(raw["quality"])
            if quality["mission_id"] != request.mission_id or quality["step_id"] != step_id or quality["domain"] != step.domain or quality["step_digest"] != content_digest(step.to_dict()):
                raise ArgumentError("connector mission checkpoint quality identity is stale")
        if step_id in snapshots:
            raise ArgumentError("connector mission checkpoint contains duplicate steps")
        snapshots[step_id] = dict(raw)
    completed = value.get("completed_step_ids")
    if not isinstance(completed, Sequence) or isinstance(completed, (str, bytes)):
        raise ArgumentError("connector mission checkpoint completed_step_ids must be a sequence")
    expected_completed = [step.id for step in steps if snapshots.get(step.id, {}).get("status") == "completed"]
    if tuple(completed) != tuple(expected_completed):
        raise ArgumentError("connector mission checkpoint completed_step_ids is inconsistent")
    return snapshots


@dataclass(frozen=True, slots=True)
class AutonomousConnectorMissionRun:
    """Metadata-rich mission run with transient connector values deliberately omitted on export."""

    mission_id: str
    mission_digest: str
    goal_digest: str
    status: str
    step_executions: tuple[AutonomousConnectorMissionStepExecution, ...]
    checkpoint: Mapping[str, Any]
    completed_step_ids: tuple[str, ...]
    next_step_ids: tuple[str, ...]
    feedback_receipts: tuple[Mapping[str, Any], ...] = ()

    def __post_init__(self) -> None:
        _bounded_id("connector mission run mission_id", self.mission_id)
        _digest("connector mission run mission_digest", self.mission_digest)
        _digest("connector mission run goal_digest", self.goal_digest)
        if self.status not in {
            "completed", "partial", "paused", "blocked", "checkpoint_blocked", "approval_required",
            "refused", "failed", "reconciliation_required",
        }:
            raise ArgumentError("connector mission run status is invalid")
        _safe_object("connector mission run checkpoint", self.checkpoint, maximum=2_000_000)
        if not isinstance(self.step_executions, Sequence) or isinstance(self.step_executions, (str, bytes)):
            raise ArgumentError("connector mission run step_executions must be a sequence")
        if len({execution.step_id for execution in self.step_executions}) != len(self.step_executions):
            raise ArgumentError("connector mission run step_executions contains duplicate steps")
        for values, name in ((self.completed_step_ids, "completed_step_ids"), (self.next_step_ids, "next_step_ids")):
            if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
                raise ArgumentError(f"connector mission run {name} must be a sequence")
            if len(set(values)) != len(values) or any(not isinstance(value, str) or not value for value in values):
                raise ArgumentError(f"connector mission run {name} contains invalid ids")
        for feedback in self.feedback_receipts:
            _safe_object("connector mission feedback receipt", feedback, maximum=32_000)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_MISSION_SCHEMA,
            "mission_id": self.mission_id,
            "mission_digest": self.mission_digest,
            "goal_digest": self.goal_digest,
            "status": self.status,
            "step_executions": [execution.to_dict() for execution in self.step_executions],
            "checkpoint": dict(self.checkpoint),
            "completed_step_ids": list(self.completed_step_ids),
            "next_step_ids": list(self.next_step_ids),
            "feedback_receipts": [dict(receipt) for receipt in self.feedback_receipts],
            "authorization": {
                "connector_dispatch": "caller_approved_only",
                "evaluator_reward": "caller_supplied_only",
            },
            "retention": "metadata_only_checkpoint_and_receipts;connector_values_transient",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorPlannedMissionRun:
    """Provider-planning handoff paired with an optional connector mission execution.

    ``mission`` is caller-owned because it contains connector arguments.  The serialized result
    deliberately exports only the protected contract identity, planning projection, and the
    connector execution's metadata-only projection.
    """

    status: str
    mission: MissionRequest
    protected_contract_digest: str
    plan_refinement: Any
    execution: AutonomousConnectorMissionRun | None = None
    schema: str = AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA:
            raise ArgumentError("connector planned mission schema is invalid")
        if self.status not in {
            "planning_approval_required",
            "planning_review_required",
            "planning_policy_review_required",
            "planning_policy_blocked",
            "planning_provider_invalid",
            "planning_provider_disagreement",
            "completed",
            "partial",
            "paused",
            "blocked",
            "checkpoint_blocked",
            "approval_required",
            "refused",
            "failed",
            "reconciliation_required",
        }:
            raise ArgumentError("connector planned mission status is invalid")
        if not isinstance(self.mission, MissionRequest):
            raise ArgumentError("connector planned mission requires a MissionRequest")
        _digest("connector planned mission protected_contract_digest", self.protected_contract_digest)
        from .autonomy import AutonomousOrderedStepPlanRefinementResult

        if not isinstance(self.plan_refinement, AutonomousOrderedStepPlanRefinementResult):
            raise ArgumentError("connector planned mission refinement is invalid")
        if self.execution is not None and not isinstance(self.execution, AutonomousConnectorMissionRun):
            raise ArgumentError("connector planned mission execution is invalid")

    @property
    def plan_refinement_digest(self) -> str:
        """Return the stable digest of the value-only planner projection."""

        return content_digest(self.plan_refinement.to_dict())

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "mission_id": self.mission.mission_id,
            "goal_digest": content_digest({"goal": self.mission.goal}),
            "protected_contract_digest": self.protected_contract_digest,
            "plan_refinement_digest": self.plan_refinement_digest,
            "plan_refinement": self.plan_refinement.to_dict(),
            "execution": None if self.execution is None else self.execution.to_dict(),
            "authorization": {
                "provider_planning": "caller_approved_provider_boundary;proposal_only_until_accept_plan",
                "connector_dispatch": "caller_approved_only",
            },
            "retention": "planning_projection_and_connector_metadata_only;mission_arguments_transient",
            "secret_material": "never_returned",
        }


def run_autonomous_connector_mission(
    runtime: AutonomousConnectorRuntime,
    *,
    mission: MissionRequest | Mapping[str, Any],
    checkpoint: Mapping[str, Any] | None = None,
    approved: bool = False,
    retry_blocked: bool = False,
    max_step_calls: int | None = None,
    request_for_step: Callable[[AutonomousConnectorMissionStepContext], Mapping[str, Any]] | None = None,
    rehydrate_payload: Callable[[Any], Any] | None = None,
    protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
    resume_outputs: Mapping[str, Any] | None = None,
    operation_registry: AutonomousConnectorOperationRegistry | None = None,
    selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
    feedback_ledger: InMemoryAutonomousConnectorFeedbackLedger | None = None,
    feedback_by_step: Mapping[str, Mapping[str, Any]] | None = None,
    quality_evaluator: Callable[[AutonomousConnectorMissionStepQualityContext], Mapping[str, Any]] | None = None,
    trace_event_callback: Callable[..., Any] | None = None,
) -> AutonomousConnectorMissionRun:
    """Execute a typed mission DAG through reviewed connectors without model credentials."""

    if not isinstance(runtime, AutonomousConnectorRuntime):
        raise ArgumentError("connector mission runtime is invalid")
    if not isinstance(approved, bool) or not isinstance(retry_blocked, bool):
        raise ArgumentError("connector mission approval and retry flags must be boolean")
    if request_for_step is not None and not callable(request_for_step):
        raise ArgumentError("connector mission request_for_step must be callable")
    if rehydrate_payload is not None and not callable(rehydrate_payload):
        raise ArgumentError("connector mission rehydrate_payload must be callable")
    if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousProtectedRehydrationAdapter):
        raise ArgumentError("connector mission protected_rehydration adapter is malformed")
    if trace_event_callback is not None and not callable(trace_event_callback):
        raise ArgumentError("connector mission trace_event_callback must be callable")
    if quality_evaluator is not None and not callable(quality_evaluator):
        raise ArgumentError("connector mission quality_evaluator must be callable")
    if resume_outputs is not None:
        resume_outputs = _safe_object("connector mission resume_outputs", resume_outputs, maximum=2_000_000)
    if feedback_by_step is not None:
        feedback_by_step = _safe_object("connector mission feedback_by_step", feedback_by_step, maximum=500_000)
    request, steps = _normalize_request(mission)
    step_ids = {step.id for step in steps}
    if feedback_by_step is not None:
        unknown_feedback_steps = sorted(set(feedback_by_step).difference(step_ids))
        if unknown_feedback_steps:
            raise ArgumentError(
                "connector mission feedback_by_step contains unknown steps: "
                + ", ".join(unknown_feedback_steps)
            )
    if len(steps) > _policy_max_steps(request):
        raise ArgumentError("connector mission exceeds policy.max_steps")
    waves = _waves(steps)
    del waves  # Graph validation is the authority; execution remains deterministically serial.
    mission_digest = content_digest(request.to_mcp_arguments())
    goal_digest = content_digest({"goal": request.goal})
    snapshots = {} if checkpoint is None else _validate_checkpoint(
        checkpoint, request, steps, mission_digest=mission_digest, goal_digest=goal_digest
    )
    prior_attempts = {step_id: int(row["attempt"]) for step_id, row in snapshots.items()}
    blocked_statuses = {"partial", "approval_required", "refused", "failed", "reconciliation_required", "quality_blocked"}
    if any(row["status"] in blocked_statuses for row in snapshots.values()) and not retry_blocked:
        blocked = tuple(step.id for step in steps if snapshots.get(step.id, {}).get("status") in blocked_statuses)
        return AutonomousConnectorMissionRun(
            request.mission_id, mission_digest, goal_digest, "checkpoint_blocked", (),
            _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
            tuple(step.id for step in steps if snapshots.get(step.id, {}).get("status") == "completed"),
            blocked,
        )
    if retry_blocked:
        for step_id in tuple(snapshots):
            if snapshots[step_id]["status"] in blocked_statuses:
                del snapshots[step_id]
    if max_step_calls is None:
        max_step_calls = MAX_AUTONOMOUS_CONNECTOR_MISSION_STEP_CALLS
    if isinstance(max_step_calls, bool) or not isinstance(max_step_calls, int) or not 1 <= max_step_calls <= MAX_AUTONOMOUS_CONNECTOR_MISSION_STEP_CALLS:
        raise ArgumentError("connector mission max_step_calls is outside its bound")
    adapter = AutonomousConnectorMissionAdapter(
        runtime,
        operation_registry=operation_registry,
        approved=approved,
        selection_signals=selection_signals,
        feedback_ledger=feedback_ledger,
        rehydrate_payload=rehydrate_payload,
        protected_rehydration=protected_rehydration,
    )
    transient_outputs: dict[str, Any] = {}
    executions: list[AutonomousConnectorMissionStepExecution] = []
    feedback_receipts: list[Mapping[str, Any]] = []
    calls = 0
    while calls < max_step_calls:
        completed = {step_id for step_id, row in snapshots.items() if row["status"] == "completed"}
        ready = next(
            (
                step for step in steps
                if step.id not in snapshots and set(step.depends_on).issubset(completed)
            ),
            None,
        )
        if ready is None:
            remaining = tuple(step.id for step in steps if step.id not in snapshots)
            if not remaining:
                status = "completed" if all(snapshots[step.id]["status"] == "completed" for step in steps) else "partial"
                return AutonomousConnectorMissionRun(
                    request.mission_id, mission_digest, goal_digest, status, tuple(executions),
                    _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
                    tuple(step.id for step in steps if snapshots[step.id]["status"] == "completed"), (),
                    tuple(feedback_receipts),
                )
            status = "blocked" if any(snapshots.get(dep, {}).get("status") != "completed" for step in steps for dep in step.depends_on if step.id in remaining) else "paused"
            return AutonomousConnectorMissionRun(
                request.mission_id, mission_digest, goal_digest, status, tuple(executions),
                _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
                tuple(sorted(completed)), remaining, tuple(feedback_receipts),
            )
        dependencies: dict[str, Any] = {}
        missing_outputs: list[str] = []
        for dependency in ready.depends_on:
            if dependency in transient_outputs:
                dependencies[dependency] = transient_outputs[dependency]
            elif resume_outputs is not None and dependency in resume_outputs:
                dependencies[dependency] = resume_outputs[dependency]
            else:
                missing_outputs.append(dependency)
        if missing_outputs:
            return AutonomousConnectorMissionRun(
                request.mission_id, mission_digest, goal_digest, "reconciliation_required", tuple(executions),
                _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
                tuple(sorted(completed)), (ready.id,), tuple(feedback_receipts),
            )
        calls += 1
        context = AutonomousConnectorMissionStepContext(
            mission_id=request.mission_id,
            goal=request.goal,
            mission_digest=mission_digest,
            goal_digest=goal_digest,
            step=ready,
            execution_attempt=prior_attempts.get(ready.id, 0) + 1,
            dependency_outputs=dependencies,
            completed_step_ids=tuple(sorted(completed)),
        )
        payload = None if request_for_step is None else request_for_step(context)
        execution = adapter.execute_step(
            context,
            request_payload=payload,
            trace_event_callback=trace_event_callback,
        )
        if quality_evaluator is not None and execution.status == "completed":
            quality_context = AutonomousConnectorMissionStepQualityContext(
                mission_id=request.mission_id,
                goal_digest=goal_digest,
                wave=0,
                attempt=context.execution_attempt,
                step=ready,
                result=execution.value,
                result_digest=content_digest(execution.value),
            )
            try:
                quality = _normalize_quality_projection(
                    quality_evaluator(quality_context),
                    context=quality_context,
                )
            except Exception as error:
                execution = replace(
                    execution,
                    status="quality_blocked",
                    run_status="quality_blocked",
                    value=None,
                    error_class="QualityEvaluatorError",
                    detail=str(error)[:512],
                )
            else:
                execution = replace(execution, quality=quality)
                if not quality["passed"]:
                    execution = replace(
                        execution,
                        status="quality_blocked",
                        run_status="quality_blocked",
                        value=None,
                        error_class=quality["failure_class"] or "MissionStepQualityGateRejected",
                        detail="connector mission quality gate rejected the result",
                    )
        executions.append(execution)
        if feedback_by_step is not None and ready.id in feedback_by_step and execution.status == "completed":
            feedback_receipts.append(adapter.settle_evaluator_feedback(execution, feedback_by_step[ready.id]))
        snapshots[ready.id] = _snapshot(ready, execution, attempt=context.execution_attempt)
        if execution.status == "completed":
            transient_outputs[ready.id] = execution.value
            continue
        if execution.status == "partial":
            return AutonomousConnectorMissionRun(
                request.mission_id, mission_digest, goal_digest, "partial", tuple(executions),
                _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
                tuple(sorted(completed)), (ready.id,), tuple(feedback_receipts),
            )
        return AutonomousConnectorMissionRun(
            request.mission_id, mission_digest, goal_digest, "blocked" if execution.status == "quality_blocked" else execution.status, tuple(executions),
            _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
            tuple(sorted(completed)), (ready.id,), tuple(feedback_receipts),
        )
    completed = {step_id for step_id, row in snapshots.items() if row["status"] == "completed"}
    remaining = tuple(step.id for step in steps if step.id not in snapshots)
    return AutonomousConnectorMissionRun(
        request.mission_id, mission_digest, goal_digest, "paused", tuple(executions),
        _checkpoint(request, steps, snapshots, mission_digest=mission_digest, goal_digest=goal_digest),
        tuple(sorted(completed)), remaining, tuple(feedback_receipts),
    )


__all__ = [
    "AUTONOMOUS_CONNECTOR_MISSION_SCHEMA",
    "AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_MISSION_STEP_CALLS",
    "MAX_AUTONOMOUS_CONNECTOR_MISSION_OUTPUT_BYTES",
    "AUTONOMOUS_CONNECTOR_MISSION_STEP_STATUSES",
    "AUTONOMOUS_CONNECTOR_MISSION_RUN_STATUSES",
    "AutonomousConnectorMissionStepContext",
    "AutonomousConnectorMissionStepExecution",
    "AutonomousConnectorMissionAdapter",
    "AutonomousConnectorMissionRun",
    "AutonomousConnectorPlannedMissionRun",
    "connector_mission_planner_steps",
    "connector_mission_protected_contract_digest",
    "apply_autonomous_ordered_step_plan",
    "run_autonomous_connector_mission",
]
