"""Provider-free clarification planning for autonomous task intake.

The autonomous brain already classifies a task and records when its interpretation is ambiguous,
effectful, evidence-dependent, or high risk.  A classification flag is not useful to an
application unless it can be turned into a bounded interaction with the user.  This module is
that interaction boundary:

* it converts the existing intent, domain lens, policy, and decision into stable questions;
* question wording is generated from reviewed metadata and never includes the task text;
* answers are accepted transiently and represented durably only by plan-bound digests; and
* a resolved answer set is a review receipt, not a route, provider, tool, evaluator, or effect
  authorization.

The planner is deliberately deterministic and shared with the TypeScript SDK.  It does not use a
provider to decide what to ask, and it does not pretend that supplying an answer proves that the
answer is true.  Callers should recompile the task intent/decision after receiving a materially
new task description or scope answer.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_domain_policy import AutonomousDomainPolicy
from .autonomous_task_decision import AutonomousTaskDecision
from .autonomous_task_intent import AutonomousTaskIntent
from .autonomous_task_lens import AutonomousDomainTaskLens
from .errors import ArgumentError


AUTONOMOUS_TASK_CLARIFICATION_SCHEMA = "bioprism-autonomous-task-clarification/0.1"
AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA = "bioprism-autonomous-task-clarification-answer/0.1"
AUTONOMOUS_TASK_CLARIFICATION_VERSION = "0.1"
AUTONOMOUS_TASK_CLARIFICATION_STATUSES = ("not_required", "required", "blocked")
AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES = ("resolved", "still_required", "blocked")
AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS = (
    "action",
    "output",
    "scope",
    "evidence",
    "authority",
    "reviewer",
    "specialist",
    "success",
)
AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS = ("text", "choice", "approval_scope")
MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS = 8
MAX_AUTONOMOUS_TASK_CLARIFICATION_OPTIONS = 12
MAX_AUTONOMOUS_TASK_CLARIFICATION_TEXT_BYTES = 512
MAX_AUTONOMOUS_TASK_CLARIFICATION_ANSWER_BYTES = 4_096


class AutonomousTaskClarificationError(ArgumentError):
    """Raised when a clarification plan, answer, or digest binding is invalid."""


def _text(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_TASK_CLARIFICATION_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousTaskClarificationError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomousTaskClarificationError(f"{name} exceeds its bound")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise AutonomousTaskClarificationError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_items(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS, allow_empty: bool = True) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise AutonomousTaskClarificationError(f"{name} must be a sequence")
    if (not allow_empty and not value) or len(value) > maximum:
        raise AutonomousTaskClarificationError(f"{name} exceeds its bound")
    result = tuple(_text(f"{name} item", item) for item in value)
    if len(set(result)) != len(result):
        raise AutonomousTaskClarificationError(f"{name} contains duplicate values")
    return result


def _bounded_count(name: str, value: Any, *, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise AutonomousTaskClarificationError(f"{name} is outside its bound")
    return value


def _unique(values: Sequence[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


@dataclass(frozen=True, slots=True)
class AutonomousTaskClarificationQuestion:
    """One deterministic question raised before a task crosses a risky boundary."""

    question_id: str
    kind: str
    dimension: str
    priority: int
    required: bool
    answer_kind: str
    prompt: str
    reason_code: str
    options: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("clarification question_id", self.question_id)
        if self.kind not in AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS:
            raise AutonomousTaskClarificationError("clarification question kind is unsupported")
        _text("clarification question dimension", self.dimension)
        if isinstance(self.priority, bool) or not isinstance(self.priority, int) or not 1 <= self.priority <= 4:
            raise AutonomousTaskClarificationError("clarification question priority is outside its bound")
        if not isinstance(self.required, bool):
            raise AutonomousTaskClarificationError("clarification question required must be boolean")
        if self.answer_kind not in AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS:
            raise AutonomousTaskClarificationError("clarification question answer_kind is unsupported")
        _text("clarification question prompt", self.prompt)
        _text("clarification question reason_code", self.reason_code)
        options = _bounded_items(
            "clarification question options",
            self.options,
            maximum=MAX_AUTONOMOUS_TASK_CLARIFICATION_OPTIONS,
        )
        if self.answer_kind == "choice" and not options:
            raise AutonomousTaskClarificationError("choice clarification questions require options")
        if self.answer_kind != "choice" and options:
            raise AutonomousTaskClarificationError("non-choice clarification questions cannot have options")
        object.__setattr__(self, "options", options)

    def to_dict(self) -> dict[str, Any]:
        return {
            "question_id": self.question_id,
            "kind": self.kind,
            "dimension": self.dimension,
            "priority": self.priority,
            "required": self.required,
            "answer_kind": self.answer_kind,
            "prompt": self.prompt,
            "reason_code": self.reason_code,
            "options": list(self.options),
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousTaskClarificationQuestion":
        if not isinstance(value, Mapping):
            raise AutonomousTaskClarificationError("clarification question must be an object")
        allowed = {"question_id", "kind", "dimension", "priority", "required", "answer_kind", "prompt", "reason_code", "options"}
        if set(value).difference(allowed):
            raise AutonomousTaskClarificationError("clarification question contains unsupported fields")
        return cls(
            question_id=value.get("question_id"),
            kind=value.get("kind"),
            dimension=value.get("dimension"),
            priority=value.get("priority"),
            required=value.get("required"),
            answer_kind=value.get("answer_kind"),
            prompt=value.get("prompt"),
            reason_code=value.get("reason_code"),
            options=tuple(value.get("options", ())),
        )


@dataclass(frozen=True, slots=True)
class AutonomousTaskClarificationPlan:
    """Digest-bound, non-executing clarification work for one reviewed task interpretation."""

    domain: str
    workflow_id: str
    task_digest: str
    intent_id: str
    intent_digest: str
    lens_digest: str
    policy_digest: str
    decision_digest: str
    status: str
    questions: tuple[AutonomousTaskClarificationQuestion, ...]
    review_dimensions: tuple[str, ...]
    missing_contracts: tuple[str, ...]
    omitted_contracts: tuple[str, ...]
    next_actions: tuple[str, ...]
    clarification_version: str = AUTONOMOUS_TASK_CLARIFICATION_VERSION

    def __post_init__(self) -> None:
        for name, value in (("domain", self.domain), ("workflow_id", self.workflow_id), ("intent_id", self.intent_id)):
            _text(f"clarification {name}", value)
        for name, value in (("task_digest", self.task_digest), ("intent_digest", self.intent_digest), ("lens_digest", self.lens_digest), ("policy_digest", self.policy_digest), ("decision_digest", self.decision_digest)):
            _digest(f"clarification {name}", value)
        if self.clarification_version != AUTONOMOUS_TASK_CLARIFICATION_VERSION:
            raise AutonomousTaskClarificationError("unsupported clarification version")
        if self.status not in AUTONOMOUS_TASK_CLARIFICATION_STATUSES:
            raise AutonomousTaskClarificationError("clarification status is unsupported")
        if not isinstance(self.questions, Sequence) or isinstance(self.questions, (str, bytes)) or len(self.questions) > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS:
            raise AutonomousTaskClarificationError("clarification questions exceed their bound")
        if any(not isinstance(question, AutonomousTaskClarificationQuestion) for question in self.questions):
            raise AutonomousTaskClarificationError("clarification questions are malformed")
        question_ids = tuple(question.question_id for question in self.questions)
        if len(set(question_ids)) != len(question_ids):
            raise AutonomousTaskClarificationError("clarification question IDs must be unique")
        for name, value in (("review_dimensions", self.review_dimensions), ("missing_contracts", self.missing_contracts), ("omitted_contracts", self.omitted_contracts), ("next_actions", self.next_actions)):
            _bounded_items(f"clarification {name}", value)
        if self.status == "not_required" and self.questions:
            raise AutonomousTaskClarificationError("not_required clarification cannot contain questions")
        if self.status == "required" and not self.questions and not self.omitted_contracts:
            raise AutonomousTaskClarificationError("required clarification must contain questions or omitted contracts")
        if self.status == "blocked" and self.questions:
            raise AutonomousTaskClarificationError("blocked clarification cannot offer bypass questions")
        object.__setattr__(self, "questions", tuple(self.questions))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_CLARIFICATION_SCHEMA,
            "clarification_version": self.clarification_version,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "task_digest": self.task_digest,
            "intent_id": self.intent_id,
            "intent_digest": self.intent_digest,
            "lens_digest": self.lens_digest,
            "policy_digest": self.policy_digest,
            "decision_digest": self.decision_digest,
            "status": self.status,
            "questions": [question.to_dict() for question in self.questions],
            "review_dimensions": list(self.review_dimensions),
            "missing_contracts": list(self.missing_contracts),
            "omitted_contracts": list(self.omitted_contracts),
            "next_actions": list(self.next_actions),
        }

    @property
    def plan_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "plan_digest": self.plan_digest,
            "retention": "value_only_clarification_metadata;task_text_and_answers_not_retained",
            "authorization": "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
            "secret_material": "never_returned",
        }

    def prompt_contract(self, *, compact: bool = False) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": AUTONOMOUS_TASK_CLARIFICATION_SCHEMA,
            "plan_digest": self.plan_digest,
            "status": self.status,
            "domain": self.domain,
            "question_count": len(self.questions),
            "required_question_count": sum(1 for question in self.questions if question.required),
            "missing_contracts": list(self.missing_contracts),
            "next_actions": list(self.next_actions),
            "authority": "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
        }
        if not compact:
            result["questions"] = [question.to_dict() for question in self.questions]
            result["review_dimensions"] = list(self.review_dimensions)
            result["omitted_contracts"] = list(self.omitted_contracts)
        result["secret_material"] = "never_returned"
        return result

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousTaskClarificationPlan":
        if not isinstance(value, Mapping):
            raise AutonomousTaskClarificationError("clarification plan must be an object")
        allowed = {
            "schema", "clarification_version", "domain", "workflow_id", "task_digest", "intent_id", "intent_digest",
            "lens_digest", "policy_digest", "decision_digest", "status", "questions", "review_dimensions",
            "missing_contracts", "omitted_contracts", "next_actions", "plan_digest", "retention", "authorization", "secret_material",
        }
        if set(value).difference(allowed):
            raise AutonomousTaskClarificationError("clarification plan contains unsupported fields")
        if value.get("schema") != AUTONOMOUS_TASK_CLARIFICATION_SCHEMA or value.get("retention") != "value_only_clarification_metadata;task_text_and_answers_not_retained" or value.get("authorization") != "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions" or value.get("secret_material") != "never_returned":
            raise AutonomousTaskClarificationError("clarification plan markers are invalid")
        questions_value = value.get("questions")
        if not isinstance(questions_value, Sequence) or isinstance(questions_value, (str, bytes)):
            raise AutonomousTaskClarificationError("clarification plan questions must be a sequence")
        plan = cls(
            domain=value.get("domain"),
            workflow_id=value.get("workflow_id"),
            task_digest=value.get("task_digest"),
            intent_id=value.get("intent_id"),
            intent_digest=value.get("intent_digest"),
            lens_digest=value.get("lens_digest"),
            policy_digest=value.get("policy_digest"),
            decision_digest=value.get("decision_digest"),
            status=value.get("status"),
            questions=tuple(AutonomousTaskClarificationQuestion.from_mapping(item) for item in questions_value),
            review_dimensions=tuple(value.get("review_dimensions", ())),
            missing_contracts=tuple(value.get("missing_contracts", ())),
            omitted_contracts=tuple(value.get("omitted_contracts", ())),
            next_actions=tuple(value.get("next_actions", ())),
            clarification_version=value.get("clarification_version"),
        )
        if value.get("plan_digest") != plan.plan_digest:
            raise AutonomousTaskClarificationError("clarification plan digest does not match its metadata")
        return plan


@dataclass(frozen=True, slots=True)
class AutonomousTaskClarificationResolution:
    """Metadata-only receipt indicating which clarification questions were answered."""

    plan_digest: str
    task_digest: str
    status: str
    answered_count: int
    required_answer_count: int
    unanswered_question_ids: tuple[str, ...]
    answer_digests: tuple[tuple[str, str], ...]

    def __post_init__(self) -> None:
        _digest("clarification resolution plan_digest", self.plan_digest)
        _digest("clarification resolution task_digest", self.task_digest)
        if self.status not in AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES:
            raise AutonomousTaskClarificationError("clarification resolution status is unsupported")
        _bounded_count("clarification resolution answered_count", self.answered_count, maximum=MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS)
        _bounded_count("clarification resolution required_answer_count", self.required_answer_count, maximum=MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS)
        unanswered = _bounded_items("clarification resolution unanswered_question_ids", self.unanswered_question_ids)
        if self.status == "resolved" and unanswered:
            raise AutonomousTaskClarificationError("resolved clarification cannot have unanswered questions")
        if self.status == "blocked" and self.answered_count:
            raise AutonomousTaskClarificationError("blocked clarification cannot accept answers")
        if not isinstance(self.answer_digests, Sequence) or isinstance(self.answer_digests, (str, bytes)) or len(self.answer_digests) > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS:
            raise AutonomousTaskClarificationError("clarification answer digests exceed their bound")
        normalized: list[tuple[str, str]] = []
        for item in self.answer_digests:
            if not isinstance(item, Sequence) or isinstance(item, (str, bytes)) or len(item) != 2:
                raise AutonomousTaskClarificationError("clarification answer digest row is malformed")
            question_id = _text("clarification answer question_id", item[0])
            answer_digest = _digest("clarification answer digest", item[1])
            normalized.append((question_id, answer_digest))
        if len({question_id for question_id, _ in normalized}) != len(normalized):
            raise AutonomousTaskClarificationError("clarification answer question IDs must be unique")
        if len(normalized) != self.answered_count:
            raise AutonomousTaskClarificationError("clarification answered_count does not match answer digests")
        object.__setattr__(self, "unanswered_question_ids", unanswered)
        object.__setattr__(self, "answer_digests", tuple(normalized))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA,
            "plan_digest": self.plan_digest,
            "task_digest": self.task_digest,
            "status": self.status,
            "answered_count": self.answered_count,
            "required_answer_count": self.required_answer_count,
            "unanswered_question_ids": list(self.unanswered_question_ids),
            "answer_digests": [{"question_id": question_id, "answer_digest": answer_digest} for question_id, answer_digest in self.answer_digests],
        }

    @property
    def resolution_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "resolution_digest": self.resolution_digest,
            "retention": "answer_digests_only;answer_values_not_retained",
            "authorization": "review_receipt_only;requires_recompiled_intent_and_decision",
            "secret_material": "never_returned",
        }


def _validate_inputs(
    intent: AutonomousTaskIntent,
    lens: AutonomousDomainTaskLens,
    policy: AutonomousDomainPolicy,
    decision: AutonomousTaskDecision,
) -> None:
    if not isinstance(intent, AutonomousTaskIntent) or not isinstance(lens, AutonomousDomainTaskLens) or not isinstance(policy, AutonomousDomainPolicy) or not isinstance(decision, AutonomousTaskDecision):
        raise AutonomousTaskClarificationError("clarification requires valid intent, lens, policy, and decision")
    if intent.domain != lens.domain or intent.domain != policy.domain or intent.domain != decision.domain:
        raise AutonomousTaskClarificationError("clarification artifacts must use the same domain")
    if intent.workflow_id != decision.workflow_id or intent.task_digest != decision.task_digest or intent.intent_id != decision.intent_id or intent.intent_digest != decision.intent_digest or lens.lens_digest != decision.lens_digest or policy.policy_digest != decision.policy_digest:
        raise AutonomousTaskClarificationError("clarification artifacts are not bound to the same decision")


def _question(
    intent: AutonomousTaskIntent,
    *,
    kind: str,
    dimension: str,
    priority: int,
    answer_kind: str,
    reason_code: str,
    prompt: str,
    options: Sequence[str] = (),
) -> AutonomousTaskClarificationQuestion:
    return AutonomousTaskClarificationQuestion(
        question_id=f"{intent.intent_id}:clarify:{kind}",
        kind=kind,
        dimension=dimension,
        priority=priority,
        required=True,
        answer_kind=answer_kind,
        prompt=prompt,
        reason_code=reason_code,
        options=tuple(dict.fromkeys(options)),
    )


def plan_autonomous_task_clarification(
    *,
    intent: AutonomousTaskIntent,
    lens: AutonomousDomainTaskLens,
    policy: AutonomousDomainPolicy,
    decision: AutonomousTaskDecision,
    max_questions: int = MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS,
) -> AutonomousTaskClarificationPlan:
    """Turn provider-free decision signals into a deterministic clarification questionnaire."""

    _validate_inputs(intent, lens, policy, decision)
    if isinstance(max_questions, bool) or not isinstance(max_questions, int) or not 1 <= max_questions <= MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS:
        raise AutonomousTaskClarificationError("max_questions is outside its bound")

    if decision.posture == "blocked":
        missing = _unique((*decision.blocking_reasons, "policy_blocker"))
        next_actions = _unique((*decision.next_actions, "resolve_blocking_policy_before_provider"))
        return AutonomousTaskClarificationPlan(
            domain=intent.domain,
            workflow_id=intent.workflow_id,
            task_digest=intent.task_digest,
            intent_id=intent.intent_id,
            intent_digest=intent.intent_digest,
            lens_digest=lens.lens_digest,
            policy_digest=policy.policy_digest,
            decision_digest=decision.decision_digest,
            status="blocked",
            questions=(),
            review_dimensions=lens.planning_dimensions,
            missing_contracts=missing,
            omitted_contracts=(),
            next_actions=next_actions,
        )

    candidates: list[AutonomousTaskClarificationQuestion] = []
    flags = set(intent.ambiguity_flags)
    approvals = set(decision.approval_requirements)
    dimensions = lens.planning_dimensions
    first_dimension = dimensions[0]
    last_dimension = dimensions[-1]
    action_options = _unique((intent.action_mode, *intent.alternative_action_modes, "other"))
    if "missing_action_signal" in flags or "competing_action_modes" in flags:
        candidates.append(_question(
            intent,
            kind="action",
            dimension=first_dimension,
            priority=1,
            answer_kind="choice",
            reason_code="ambiguous_action_mode",
            prompt=f"Choose the primary action for the reviewed {intent.domain} workflow.",
            options=action_options,
        ))
    if "no_explicit_output_contract" in flags:
        candidates.append(_question(
            intent,
            kind="output",
            dimension="output_contract",
            priority=1,
            answer_kind="text",
            reason_code="missing_output_contract",
            prompt=f"What concrete output should the {intent.domain} workflow produce? Name the artifact, decision, or handoff.",
        ))
    if "uncertainty_language" in flags:
        candidates.append(_question(
            intent,
            kind="success",
            dimension=last_dimension,
            priority=2,
            answer_kind="text",
            reason_code="uncertainty_tolerance_missing",
            prompt=f"What observable success criterion and acceptable uncertainty should end the {intent.domain} workflow?",
        ))
    if intent.requested_effect == "external_effect" or "effect_approval" in approvals:
        candidates.append(_question(
            intent,
            kind="authority",
            dimension="authority",
            priority=1,
            answer_kind="approval_scope",
            reason_code="effect_scope_and_authority_missing",
            prompt="What exact effect is in scope, who approves it, and what rollback or postcondition is required?",
        ))
    if policy.evidence_mode == "required_before_provider" or "evidence_dispatch" in approvals:
        candidates.append(_question(
            intent,
            kind="evidence",
            dimension=lens.evidence_priorities[0],
            priority=1,
            answer_kind="text",
            reason_code="evidence_boundary_missing",
            prompt=f"Which caller-owned evidence or source boundary must be satisfied before {intent.domain} provider work?",
        ))
    if intent.domain == "cross_domain" or "specialist_boundaries_require_review" in decision.review_reasons or intent.action_mode in {"coordinate", "synthesize"}:
        candidates.append(_question(
            intent,
            kind="specialist",
            dimension="specialist_contracts",
            priority=1,
            answer_kind="text",
            reason_code="specialist_scope_missing",
            prompt="Which specialist domains or handoff boundaries must participate in this cross-domain result?",
        ))
    substantive_risks = tuple(signal for signal in intent.risk_signals if signal not in {"domain_policy_review", "output_contract_missing"})
    if substantive_risks or "risk_signals_require_review" in decision.review_reasons:
        candidates.append(_question(
            intent,
            kind="reviewer",
            dimension=last_dimension,
            priority=2,
            answer_kind="text",
            reason_code="accountable_reviewer_missing",
            prompt=f"Which qualified reviewer or accountable owner must review the {intent.domain} result before reliance?",
        ))
    if decision.posture == "review_required" and not candidates:
        candidates.append(_question(
            intent,
            kind="scope",
            dimension=first_dimension,
            priority=2,
            answer_kind="text",
            reason_code="review_scope_missing",
            prompt=f"What scope should the reviewed {intent.domain} workflow cover, and what is explicitly out of scope?",
        ))

    order = {kind: index for index, kind in enumerate(AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS)}
    candidates.sort(key=lambda question: (question.priority, order[question.kind]))
    selected = tuple(candidates[:max_questions])
    omitted = tuple(question.reason_code for question in candidates[max_questions:])
    missing = _unique(tuple(question.reason_code for question in candidates) + omitted)
    if omitted:
        missing = _unique((*missing, "clarification_question_limit_reached"))
    status = "required" if selected or omitted else "not_required"
    if status == "required":
        next_actions = _unique(("answer_clarification_questions", "recompile_intent_and_decision_before_execution", *decision.next_actions))
    else:
        next_actions = ("continue_to_reviewed_execution_boundary",)
    return AutonomousTaskClarificationPlan(
        domain=intent.domain,
        workflow_id=intent.workflow_id,
        task_digest=intent.task_digest,
        intent_id=intent.intent_id,
        intent_digest=intent.intent_digest,
        lens_digest=lens.lens_digest,
        policy_digest=policy.policy_digest,
        decision_digest=decision.decision_digest,
        status=status,
        questions=selected,
        review_dimensions=lens.planning_dimensions,
        missing_contracts=missing,
        omitted_contracts=omitted,
        next_actions=next_actions,
    )


def validate_autonomous_task_clarification_plan(value: AutonomousTaskClarificationPlan | Mapping[str, Any]) -> AutonomousTaskClarificationPlan:
    """Validate and rehydrate a plan before it is shown or used for answer collection."""

    if isinstance(value, AutonomousTaskClarificationPlan):
        return value
    return AutonomousTaskClarificationPlan.from_mapping(value)


def resolve_autonomous_task_clarification(
    plan: AutonomousTaskClarificationPlan | Mapping[str, Any],
    *,
    task_digest: str,
    answers: Mapping[str, str],
) -> AutonomousTaskClarificationResolution:
    """Record a transient answer set as plan-bound digests without retaining answer values."""

    resolved_plan = validate_autonomous_task_clarification_plan(plan)
    _digest("clarification resolution task_digest", task_digest)
    if task_digest != resolved_plan.task_digest:
        raise AutonomousTaskClarificationError("clarification answers do not match the task digest")
    if not isinstance(answers, Mapping):
        raise AutonomousTaskClarificationError("clarification answers must be a mapping")
    if len(answers) > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS:
        raise AutonomousTaskClarificationError("clarification answers exceed their bound")
    questions = {question.question_id: question for question in resolved_plan.questions}
    unknown = set(answers).difference(questions)
    if unknown:
        raise AutonomousTaskClarificationError("clarification answers contain an unknown question ID")
    if resolved_plan.status == "blocked":
        if answers:
            raise AutonomousTaskClarificationError("blocked clarification cannot accept answers")
        return AutonomousTaskClarificationResolution(
            plan_digest=resolved_plan.plan_digest,
            task_digest=task_digest,
            status="blocked",
            answered_count=0,
            required_answer_count=0,
            unanswered_question_ids=(),
            answer_digests=(),
        )
    answer_digests: list[tuple[str, str]] = []
    for question in resolved_plan.questions:
        if question.question_id not in answers:
            continue
        answer = answers[question.question_id]
        _text("clarification answer", answer, maximum=MAX_AUTONOMOUS_TASK_CLARIFICATION_ANSWER_BYTES)
        if question.answer_kind == "choice" and answer not in question.options:
            raise AutonomousTaskClarificationError(f"clarification answer for {question.question_id} is not one of the offered options")
        answer_digests.append((question.question_id, content_digest({
            "schema": AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA,
            "plan_digest": resolved_plan.plan_digest,
            "question_id": question.question_id,
            "answer": answer,
        })))
    unanswered = tuple(question.question_id for question in resolved_plan.questions if question.required and question.question_id not in answers)
    required_count = sum(1 for question in resolved_plan.questions if question.required)
    status = "resolved" if not unanswered and not resolved_plan.omitted_contracts else "still_required"
    if resolved_plan.omitted_contracts:
        unanswered = _unique((*unanswered, "clarification_question_limit_reached"))
    return AutonomousTaskClarificationResolution(
        plan_digest=resolved_plan.plan_digest,
        task_digest=task_digest,
        status=status,
        answered_count=len(answer_digests),
        required_answer_count=required_count,
        unanswered_question_ids=unanswered,
        answer_digests=tuple(answer_digests),
    )


__all__ = [
    "AUTONOMOUS_TASK_CLARIFICATION_SCHEMA",
    "AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA",
    "AUTONOMOUS_TASK_CLARIFICATION_VERSION",
    "AUTONOMOUS_TASK_CLARIFICATION_STATUSES",
    "AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES",
    "AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS",
    "AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS",
    "MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS",
    "MAX_AUTONOMOUS_TASK_CLARIFICATION_OPTIONS",
    "MAX_AUTONOMOUS_TASK_CLARIFICATION_TEXT_BYTES",
    "MAX_AUTONOMOUS_TASK_CLARIFICATION_ANSWER_BYTES",
    "AutonomousTaskClarificationError",
    "AutonomousTaskClarificationQuestion",
    "AutonomousTaskClarificationPlan",
    "AutonomousTaskClarificationResolution",
    "plan_autonomous_task_clarification",
    "validate_autonomous_task_clarification_plan",
    "resolve_autonomous_task_clarification",
]
