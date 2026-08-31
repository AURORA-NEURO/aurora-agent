"""Digest-bound next-action handoff for the autonomous brain.

The routing and task-decision layers produce the information needed to choose a safe
execution boundary, but an application should not have to reimplement the precedence
rules that turn that information into a user-facing next step.  This module composes a
prepared automatic blueprint into one bounded action plan.

The plan is deliberately provider-free.  It contains no task text, prompt values,
credentials, model responses, source payloads, tool arguments, or authority.  It only
binds the reviewed route and its domain blueprints to the next explicit caller action.
Provider, evidence, tool, evaluator, and effect dispatch remain separate admission
boundaries owned by their existing APIs.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_domain_policy import autonomous_domain_policy
from .autonomous_task_decision import (
    AUTONOMOUS_TASK_DECISION_APPROVALS,
    AUTONOMOUS_TASK_DECISION_PATHS,
    AUTONOMOUS_TASK_DECISION_POSTURES,
    AutonomousTaskDecision,
    infer_autonomous_task_decision,
)
from .autonomous_task_intent import infer_autonomous_task_intent
from .autonomous_task_lens import autonomous_domain_task_lens
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_ACTION_PLAN_SCHEMA = "bioprism-python-autonomous-action-plan/0.1"
AUTONOMOUS_ACTION_PLAN_VERSION = "0.1"
AUTONOMOUS_ACTION_PLAN_STATUSES = (
    "ready",
    "review_required",
    "blocked",
    "route_review_required",
)
AUTONOMOUS_ACTION_PLAN_ROLES = ("single", "child", "synthesis")
AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS = (
    "review_route",
    "recompute_route",
    "review_task_decision",
    "resolve_policy_block",
    "acquire_evidence",
    "review_plan",
    "review_effect",
    "approve_provider_call",
    "settle_evaluator",
    "stop_before_dispatch",
)
MAX_AUTONOMOUS_ACTION_PLAN_CANDIDATES = 16
MAX_AUTONOMOUS_ACTION_PLAN_ITEMS = 128
MAX_AUTONOMOUS_ACTION_PLAN_TEXT_BYTES = 512


def _text(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_ACTION_PLAN_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bound")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _items(name: str, values: Any, *, maximum: int = MAX_AUTONOMOUS_ACTION_PLAN_ITEMS) -> tuple[str, ...]:
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes)) or len(values) > maximum:
        raise ArgumentError(f"{name} exceeds its item bound")
    result = tuple(_text(f"{name} item", value) for value in values)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate items")
    return result


def _identifiers(name: str, values: Any, *, maximum: int = 12) -> tuple[str, ...]:
    result = _items(name, values, maximum=maximum)
    for value in result:
        if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-+ /" for character in value):
            raise ArgumentError(f"{name} contains an unsupported identifier")
    return result


def _unique(values: Sequence[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


def _decision_for_blueprint(blueprint: Any) -> AutonomousTaskDecision:
    decision = getattr(blueprint, "task_decision", None)
    if isinstance(decision, AutonomousTaskDecision):
        return decision
    spec = blueprint.spec
    lens = getattr(blueprint, "task_lens", None) or autonomous_domain_task_lens(spec.domain)
    intent = getattr(blueprint, "task_intent", None) or infer_autonomous_task_intent(
        task=spec.task,
        task_digest=spec.task_digest,
        domain=spec.domain,
        capability=spec.capability,
        risk_class=spec.risk_class,
        workflow_id=blueprint.workflow.workflow_id,
        lens=lens,
        constraints=spec.constraints,
        desired_outputs=spec.desired_outputs,
    )
    return infer_autonomous_task_decision(
        intent=intent,
        lens=lens,
        policy=getattr(blueprint, "domain_policy", None) or autonomous_domain_policy(spec.domain),
        required_model_capabilities=blueprint.required_capabilities,
    )


def _candidate_descriptor(candidate: "AutonomousActionCandidate") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_ACTION_PLAN_SCHEMA,
        "version": AUTONOMOUS_ACTION_PLAN_VERSION,
        "candidate_id": candidate.candidate_id,
        "role": candidate.role,
        "domain": candidate.domain,
        "task_digest": candidate.task_digest,
        "route_digest": candidate.route_digest,
        "workflow_id": candidate.workflow_id,
        "workflow_digest": candidate.workflow_digest,
        "domain_pack_digest": candidate.domain_pack_digest,
        "domain_policy_digest": candidate.domain_policy_digest,
        "evidence_plan_digest": candidate.evidence_plan_digest,
        "capability": candidate.capability,
        "risk_class": candidate.risk_class,
        "task_intent_digest": candidate.task_intent_digest,
        "task_decision_digest": candidate.task_decision_digest,
        "task_decision_posture": candidate.task_decision_posture,
        "recommended_path": candidate.recommended_path,
        "requested_effect": candidate.requested_effect,
        "evidence_posture": candidate.evidence_posture,
        "required_model_capabilities": list(candidate.required_model_capabilities),
        "preferred_model_capabilities": list(candidate.preferred_model_capabilities),
        "approval_requirements": list(candidate.approval_requirements),
        "review_reasons": list(candidate.review_reasons),
        "blocking_reasons": list(candidate.blocking_reasons),
        "next_actions": list(candidate.next_actions),
    }


@dataclass(frozen=True, slots=True)
class AutonomousActionCandidate:
    """One digest-bound domain action candidate within an autonomous action plan."""

    candidate_id: str
    role: str
    domain: str
    task_digest: str
    route_digest: str
    workflow_id: str
    workflow_digest: str
    domain_pack_digest: str
    domain_policy_digest: str
    evidence_plan_digest: str
    capability: str
    risk_class: str
    task_intent_digest: str
    task_decision_digest: str
    task_decision_posture: str
    recommended_path: str
    requested_effect: str
    evidence_posture: str
    required_model_capabilities: tuple[str, ...]
    preferred_model_capabilities: tuple[str, ...]
    approval_requirements: tuple[str, ...]
    review_reasons: tuple[str, ...]
    blocking_reasons: tuple[str, ...]
    next_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        for name, value in (
            ("candidate_id", self.candidate_id),
            ("workflow_id", self.workflow_id),
            ("capability", self.capability),
            ("risk_class", self.risk_class),
            ("evidence_posture", self.evidence_posture),
            ("requested_effect", self.requested_effect),
        ):
            _text(f"action candidate {name}", value)
        if self.role not in AUTONOMOUS_ACTION_PLAN_ROLES:
            raise ArgumentError("action candidate role is unsupported")
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("action candidate domain is not in the built-in catalogue")
        for name, value in (
            ("task_digest", self.task_digest),
            ("route_digest", self.route_digest),
            ("workflow_digest", self.workflow_digest),
            ("domain_pack_digest", self.domain_pack_digest),
            ("domain_policy_digest", self.domain_policy_digest),
            ("evidence_plan_digest", self.evidence_plan_digest),
            ("task_intent_digest", self.task_intent_digest),
            ("task_decision_digest", self.task_decision_digest),
        ):
            _digest(f"action candidate {name}", value)
        if self.task_decision_posture not in AUTONOMOUS_TASK_DECISION_POSTURES:
            raise ArgumentError("action candidate task decision posture is unsupported")
        if self.recommended_path not in AUTONOMOUS_TASK_DECISION_PATHS:
            raise ArgumentError("action candidate recommended path is unsupported")
        for name, value in (
            ("required_model_capabilities", self.required_model_capabilities),
            ("preferred_model_capabilities", self.preferred_model_capabilities),
            ("approval_requirements", self.approval_requirements),
            ("review_reasons", self.review_reasons),
            ("blocking_reasons", self.blocking_reasons),
            ("next_actions", self.next_actions),
        ):
            _items(f"action candidate {name}", value)
        if any(value not in AUTONOMOUS_TASK_DECISION_APPROVALS for value in self.approval_requirements):
            raise ArgumentError("action candidate approval requirements contain an unsupported gate")
        if any(value not in AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS for value in self.next_actions):
            raise ArgumentError("action candidate next actions contain an unsupported action")
        if not self.next_actions:
            raise ArgumentError("action candidate must retain at least one next action")
        object.__setattr__(self, "required_model_capabilities", _unique(self.required_model_capabilities))
        object.__setattr__(self, "preferred_model_capabilities", _unique(self.preferred_model_capabilities))
        object.__setattr__(self, "approval_requirements", _unique(self.approval_requirements))
        object.__setattr__(self, "review_reasons", _unique(self.review_reasons))
        object.__setattr__(self, "blocking_reasons", _unique(self.blocking_reasons))
        object.__setattr__(self, "next_actions", _unique(self.next_actions))

    @property
    def candidate_digest(self) -> str:
        return content_digest(_candidate_descriptor(self))

    def to_dict(self) -> dict[str, Any]:
        return {
            **_candidate_descriptor(self),
            "candidate_digest": self.candidate_digest,
            "authority": "guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
            "retention": "metadata_only;task_prompt_provider_and_connector_values_not_retained",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousActionCandidate":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_ACTION_PLAN_SCHEMA:
            raise ArgumentError("action candidate schema is invalid")
        if value.get("version") != AUTONOMOUS_ACTION_PLAN_VERSION:
            raise ArgumentError("action candidate version is unsupported")
        if value.get("authority") != "guidance_only;does_not_authorize_provider_source_tool_or_effect_actions":
            raise ArgumentError("action candidate authority posture is invalid")
        if value.get("retention") != "metadata_only;task_prompt_provider_and_connector_values_not_retained" or value.get("secret_material") != "never_returned":
            raise ArgumentError("action candidate retention posture is invalid")
        fields = {
            "candidate_id": value.get("candidate_id"),
            "role": value.get("role"),
            "domain": value.get("domain"),
            "task_digest": value.get("task_digest"),
            "route_digest": value.get("route_digest"),
            "workflow_id": value.get("workflow_id"),
            "workflow_digest": value.get("workflow_digest"),
            "domain_pack_digest": value.get("domain_pack_digest"),
            "domain_policy_digest": value.get("domain_policy_digest"),
            "evidence_plan_digest": value.get("evidence_plan_digest"),
            "capability": value.get("capability"),
            "risk_class": value.get("risk_class"),
            "task_intent_digest": value.get("task_intent_digest"),
            "task_decision_digest": value.get("task_decision_digest"),
            "task_decision_posture": value.get("task_decision_posture"),
            "recommended_path": value.get("recommended_path"),
            "requested_effect": value.get("requested_effect"),
            "evidence_posture": value.get("evidence_posture"),
            "required_model_capabilities": tuple(value.get("required_model_capabilities", ())),
            "preferred_model_capabilities": tuple(value.get("preferred_model_capabilities", ())),
            "approval_requirements": tuple(value.get("approval_requirements", ())),
            "review_reasons": tuple(value.get("review_reasons", ())),
            "blocking_reasons": tuple(value.get("blocking_reasons", ())),
            "next_actions": tuple(value.get("next_actions", ())),
        }
        candidate = cls(**fields)
        if value.get("candidate_digest") != candidate.candidate_digest:
            raise ArgumentError("action candidate digest is invalid")
        return candidate


def _plan_descriptor(plan: "AutonomousActionPlan") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_ACTION_PLAN_SCHEMA,
        "version": AUTONOMOUS_ACTION_PLAN_VERSION,
        "status": plan.status,
        "route_digest": plan.route_digest,
        "task_digest": plan.task_digest,
        "selected_domains": list(plan.selected_domains),
        "cross_domain": plan.cross_domain,
        "route_confidence": plan.route_confidence,
        "route_reason": plan.route_reason,
        "route_source": plan.route_source,
        "semantic_route_status": plan.semantic_route_status,
        "recommended_path": plan.recommended_path,
        "candidates": [candidate.to_dict() for candidate in plan.candidates],
        "required_approvals": list(plan.required_approvals),
        "review_reasons": list(plan.review_reasons),
        "blocking_reasons": list(plan.blocking_reasons),
        "next_action": plan.next_action,
        "next_actions": list(plan.next_actions),
    }


@dataclass(frozen=True, slots=True)
class AutonomousActionPlan:
    """A complete, metadata-only next-action handoff for a prepared automatic route."""

    status: str
    route_digest: str
    task_digest: str
    selected_domains: tuple[str, ...]
    cross_domain: bool
    route_confidence: float
    route_reason: str
    route_source: str
    semantic_route_status: str | None
    recommended_path: str
    candidates: tuple[AutonomousActionCandidate, ...]
    required_approvals: tuple[str, ...]
    review_reasons: tuple[str, ...]
    blocking_reasons: tuple[str, ...]
    next_action: str
    next_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_ACTION_PLAN_STATUSES:
            raise ArgumentError("autonomous action plan status is unsupported")
        for name, value in (("route_digest", self.route_digest), ("task_digest", self.task_digest)):
            _digest(f"action plan {name}", value)
        selected = _identifiers("action plan selected_domains", self.selected_domains, maximum=12)
        if any(value not in AUTONOMOUS_DOMAIN_NAMES for value in selected):
            raise ArgumentError("action plan selected_domains contains an unknown domain")
        if not isinstance(self.cross_domain, bool):
            raise ArgumentError("action plan cross_domain must be boolean")
        if self.cross_domain != (len(selected) > 1):
            raise ArgumentError("action plan cross_domain does not match selected_domains")
        if isinstance(self.route_confidence, bool) or not isinstance(self.route_confidence, (int, float)):
            raise ArgumentError("action plan route_confidence must be numeric")
        if not 0.0 <= float(self.route_confidence) <= 1.0:
            raise ArgumentError("action plan route_confidence must be within [0, 1]")
        _text("action plan route_reason", self.route_reason)
        _text("action plan route_source", self.route_source)
        if self.semantic_route_status is not None:
            _text("action plan semantic_route_status", self.semantic_route_status)
        if self.recommended_path not in AUTONOMOUS_TASK_DECISION_PATHS and self.recommended_path != "route_review":
            raise ArgumentError("action plan recommended_path is unsupported")
        if not isinstance(self.candidates, Sequence) or isinstance(self.candidates, (str, bytes)):
            raise ArgumentError("action plan candidates must be a sequence")
        candidates = tuple(self.candidates)
        if len(candidates) > MAX_AUTONOMOUS_ACTION_PLAN_CANDIDATES:
            raise ArgumentError("action plan candidates exceed their bound")
        if any(not isinstance(candidate, AutonomousActionCandidate) for candidate in candidates):
            raise ArgumentError("action plan candidates are malformed")
        if len({candidate.candidate_id for candidate in candidates}) != len(candidates):
            raise ArgumentError("action plan candidate IDs must be unique")
        if any(candidate.route_digest != self.route_digest for candidate in candidates):
            raise ArgumentError("action plan candidate route digest does not match the plan")
        if any(candidate.task_digest != self.task_digest for candidate in candidates if candidate.role != "synthesis"):
            raise ArgumentError("action plan child task digest does not match the route task digest")
        if self.status == "route_review_required" and candidates:
            raise ArgumentError("route-review action plans cannot contain action candidates")
        if self.status != "route_review_required" and not candidates:
            raise ArgumentError("routed action plans require at least one action candidate")
        approvals = _items("action plan required_approvals", self.required_approvals, maximum=16)
        if any(value not in AUTONOMOUS_TASK_DECISION_APPROVALS for value in approvals):
            raise ArgumentError("action plan required_approvals contains an unsupported gate")
        review = _items("action plan review_reasons", self.review_reasons)
        blocked = _items("action plan blocking_reasons", self.blocking_reasons)
        if self.next_action not in AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS:
            raise ArgumentError("action plan next_action is unsupported")
        next_actions = _items("action plan next_actions", self.next_actions, maximum=32)
        if self.next_action not in next_actions:
            raise ArgumentError("action plan next_action must be present in next_actions")
        if self.status == "blocked" and not blocked:
            raise ArgumentError("blocked action plans must retain blocking reasons")
        if self.status == "review_required" and not review:
            raise ArgumentError("review-required action plans must retain review reasons")
        object.__setattr__(self, "selected_domains", selected)
        object.__setattr__(self, "route_confidence", float(self.route_confidence))
        object.__setattr__(self, "candidates", candidates)
        object.__setattr__(self, "required_approvals", _unique(approvals))
        object.__setattr__(self, "review_reasons", _unique(review))
        object.__setattr__(self, "blocking_reasons", _unique(blocked))
        object.__setattr__(self, "next_actions", _unique(next_actions))

    @property
    def plan_digest(self) -> str:
        return content_digest(_plan_descriptor(self))

    def to_dict(self) -> dict[str, Any]:
        return {
            **_plan_descriptor(self),
            "plan_digest": self.plan_digest,
            "authorization": "guidance_only;route_and_plan_metadata_do_not_authorize_dispatch",
            "retention": "metadata_only;task_prompt_provider_connector_and_credential_values_not_retained",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousActionPlan":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_ACTION_PLAN_SCHEMA:
            raise ArgumentError("autonomous action plan schema is invalid")
        if value.get("version") != AUTONOMOUS_ACTION_PLAN_VERSION:
            raise ArgumentError("autonomous action plan version is unsupported")
        if value.get("authorization") != "guidance_only;route_and_plan_metadata_do_not_authorize_dispatch":
            raise ArgumentError("autonomous action plan authorization posture is invalid")
        if value.get("retention") != "metadata_only;task_prompt_provider_connector_and_credential_values_not_retained" or value.get("secret_material") != "never_returned":
            raise ArgumentError("autonomous action plan retention posture is invalid")
        candidates_value = value.get("candidates", ())
        if not isinstance(candidates_value, Sequence) or isinstance(candidates_value, (str, bytes)):
            raise ArgumentError("autonomous action plan candidates are malformed")
        candidates = tuple(AutonomousActionCandidate.from_dict(item) for item in candidates_value)
        plan = cls(
            status=value.get("status"),
            route_digest=value.get("route_digest"),
            task_digest=value.get("task_digest"),
            selected_domains=tuple(value.get("selected_domains", ())),
            cross_domain=value.get("cross_domain"),
            route_confidence=value.get("route_confidence"),
            route_reason=value.get("route_reason"),
            route_source=value.get("route_source"),
            semantic_route_status=value.get("semantic_route_status"),
            recommended_path=value.get("recommended_path"),
            candidates=candidates,
            required_approvals=tuple(value.get("required_approvals", ())),
            review_reasons=tuple(value.get("review_reasons", ())),
            blocking_reasons=tuple(value.get("blocking_reasons", ())),
            next_action=value.get("next_action"),
            next_actions=tuple(value.get("next_actions", ())),
        )
        if value.get("plan_digest") != plan.plan_digest:
            raise ArgumentError("autonomous action plan digest is invalid")
        return plan


def _candidate_from_blueprint(
    blueprint: Any,
    *,
    candidate_id: str,
    role: str,
    route_digest: str,
) -> AutonomousActionCandidate:
    decision = _decision_for_blueprint(blueprint)
    evidence_plan = blueprint.evidence_plan()
    candidate_actions: list[str] = []
    if decision.posture == "blocked":
        candidate_actions.extend(("resolve_policy_block", "stop_before_dispatch"))
    else:
        if "evidence_dispatch" in decision.approval_requirements:
            candidate_actions.append("acquire_evidence")
        if "plan_acceptance" in decision.approval_requirements:
            candidate_actions.append("review_plan")
        if "effect_approval" in decision.approval_requirements:
            candidate_actions.append("review_effect")
        if "provider_call" in decision.approval_requirements:
            candidate_actions.append("approve_provider_call")
        if "evaluator_settlement" in decision.approval_requirements:
            candidate_actions.append("settle_evaluator")
    return AutonomousActionCandidate(
        candidate_id=candidate_id,
        role=role,
        domain=blueprint.profile.domain,
        task_digest=blueprint.spec.task_digest,
        route_digest=route_digest,
        workflow_id=blueprint.workflow.workflow_id,
        workflow_digest=blueprint.workflow.workflow_digest,
        domain_pack_digest=blueprint.domain_pack.pack_digest,
        domain_policy_digest=decision.policy_digest,
        evidence_plan_digest=evidence_plan.plan_digest,
        capability=blueprint.spec.capability,
        risk_class=blueprint.spec.risk_class,
        task_intent_digest=decision.intent_digest,
        task_decision_digest=decision.decision_digest,
        task_decision_posture=decision.posture,
        recommended_path=decision.recommended_path,
        requested_effect=decision.requested_effect,
        evidence_posture=decision.evidence_posture,
        required_model_capabilities=tuple(decision.required_model_capabilities),
        preferred_model_capabilities=tuple(decision.preferred_model_capabilities),
        approval_requirements=tuple(decision.approval_requirements),
        review_reasons=tuple(decision.review_reasons),
        blocking_reasons=tuple(decision.blocking_reasons),
        next_actions=tuple(_unique(candidate_actions)),
    )


def _aggregate_next_actions(
    *,
    status: str,
    approvals: Sequence[str],
    review_reasons: Sequence[str],
    blocking_reasons: Sequence[str],
) -> tuple[str, tuple[str, ...]]:
    if status == "route_review_required":
        return "review_route", ("review_route", "recompute_route")
    if status == "blocked":
        return "resolve_policy_block", ("resolve_policy_block", "stop_before_dispatch")
    actions: list[str] = []
    if review_reasons:
        actions.append("review_task_decision")
    if "evidence_dispatch" in approvals:
        actions.append("acquire_evidence")
    if "plan_acceptance" in approvals:
        actions.append("review_plan")
    if "effect_approval" in approvals:
        actions.append("review_effect")
    if "provider_call" in approvals:
        actions.append("approve_provider_call")
    if "evaluator_settlement" in approvals:
        actions.append("settle_evaluator")
    if not actions:
        actions.append("review_task_decision")
    return actions[0], tuple(_unique(actions))


def plan_autonomous_action(blueprint: Any) -> AutonomousActionPlan:
    """Compile a prepared automatic blueprint into a deterministic next-action plan.

    This function performs no provider, connector, source, tool, evaluator, credential, or
    effect operation.  It accepts only the provider-free result of ``prepare_auto``.
    """

    from .autonomy import AutonomousAutoBlueprint, AutonomousCrossDomainBlueprint, AutonomousTaskBlueprint

    if not isinstance(blueprint, AutonomousAutoBlueprint):
        raise ArgumentError("autonomous action planning requires an AutonomousAutoBlueprint")
    route = blueprint.route
    semantic_status = None if blueprint.semantic_route is None else blueprint.semantic_route.status
    route_review = route.abstained or (semantic_status is not None and semantic_status != "completed")
    if route_review:
        reasons = [f"route:{route.reason}"]
        if semantic_status is not None and semantic_status != "completed":
            reasons.append(f"semantic_route:{semantic_status}")
        next_action, next_actions = _aggregate_next_actions(
            status="route_review_required",
            approvals=(),
            review_reasons=reasons,
            blocking_reasons=(),
        )
        return AutonomousActionPlan(
            status="route_review_required",
            route_digest=route.route_digest,
            task_digest=route.task_digest,
            selected_domains=route.selected_domains,
            cross_domain=route.cross_domain,
            route_confidence=route.confidence,
            route_reason=route.reason,
            route_source=route.source,
            semantic_route_status=semantic_status,
            recommended_path="route_review",
            candidates=(),
            required_approvals=(),
            review_reasons=tuple(reasons),
            blocking_reasons=(),
            next_action=next_action,
            next_actions=next_actions,
        )

    candidates: list[AutonomousActionCandidate] = []
    if blueprint.blueprint is not None:
        candidates.append(
            _candidate_from_blueprint(
                blueprint.blueprint,
                candidate_id="single",
                role="single",
                route_digest=route.route_digest,
            )
        )
    elif isinstance(blueprint.cross_domain_blueprint, AutonomousCrossDomainBlueprint):
        cross = blueprint.cross_domain_blueprint
        for child_id, child in zip(cross.child_ids, cross.child_blueprints):
            candidates.append(
                _candidate_from_blueprint(
                    child,
                    candidate_id=child_id,
                    role="child",
                    route_digest=route.route_digest,
                )
            )
        candidates.append(
            _candidate_from_blueprint(
                cross.synthesis_blueprint,
                candidate_id="synthesis",
                role="synthesis",
                route_digest=route.route_digest,
            )
        )
    else:
        raise ArgumentError("routed automatic blueprint has no executable action candidate")

    approvals = _unique(tuple(item for candidate in candidates for item in candidate.approval_requirements))
    review_reasons = _unique(
        tuple(f"{candidate.candidate_id}:{reason}" for candidate in candidates for reason in candidate.review_reasons)
    )
    blocking_reasons = _unique(
        tuple(f"{candidate.candidate_id}:{reason}" for candidate in candidates for reason in candidate.blocking_reasons)
    )
    status = "blocked" if blocking_reasons else "review_required" if review_reasons else "ready"
    next_action, next_actions = _aggregate_next_actions(
        status=status,
        approvals=approvals,
        review_reasons=review_reasons,
        blocking_reasons=blocking_reasons,
    )
    recommended_path = "cross_domain" if route.cross_domain else candidates[0].recommended_path
    return AutonomousActionPlan(
        status=status,
        route_digest=route.route_digest,
        task_digest=route.task_digest,
        selected_domains=route.selected_domains,
        cross_domain=route.cross_domain,
        route_confidence=route.confidence,
        route_reason=route.reason,
        route_source=route.source,
        semantic_route_status=semantic_status,
        recommended_path=recommended_path,
        candidates=tuple(candidates),
        required_approvals=approvals,
        review_reasons=review_reasons,
        blocking_reasons=blocking_reasons,
        next_action=next_action,
        next_actions=next_actions,
    )


__all__ = [
    "AUTONOMOUS_ACTION_PLAN_SCHEMA",
    "AUTONOMOUS_ACTION_PLAN_VERSION",
    "AUTONOMOUS_ACTION_PLAN_STATUSES",
    "AUTONOMOUS_ACTION_PLAN_ROLES",
    "AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS",
    "MAX_AUTONOMOUS_ACTION_PLAN_CANDIDATES",
    "MAX_AUTONOMOUS_ACTION_PLAN_ITEMS",
    "AutonomousActionCandidate",
    "AutonomousActionPlan",
    "plan_autonomous_action",
]
