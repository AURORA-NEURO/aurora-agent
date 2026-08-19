"""Held-out, value-only evaluation helpers for the autonomous routing and planning surfaces.

These evaluators deliberately sit outside execution. A holdout case can contain task text and
reference labels in the embedding application, but public projections retain only digests,
aggregate metrics, and bounded per-case status. The router or planner never receives the
reference label as context.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AutonomousPlanRefinementResult,
    AutonomousTaskBlueprint,
    AutonomousTaskRouter,
)
from .brain import BrainRunError


AUTONOMOUS_HOLDOUT_EVALUATION_SCHEMA = "bioprism-python-autonomous-holdout-evaluation/0.1"
MAX_AUTONOMOUS_HOLDOUT_CASES = 256


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 128:
        raise BrainRunError(f"{name} must be a bounded non-empty string")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-" for character in value):
        raise BrainRunError(f"{name} must be a safe identifier")
    return value


def _text(name: str, value: Any, maximum: int = 16_000) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a bounded non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return value


def _domains(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or not value:
        raise BrainRunError(f"{name} must be a non-empty sequence")
    result = tuple(value)
    if any(not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS for domain in result):
        raise BrainRunError(f"{name} contains an unsupported autonomous domain")
    if len(set(result)) != len(result):
        raise BrainRunError(f"{name} must contain unique domains")
    return result


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise BrainRunError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _workflow_order_is_dependency_valid(
    blueprint: AutonomousTaskBlueprint,
    order: Sequence[str],
    *,
    name: str,
) -> tuple[str, ...]:
    if not isinstance(order, Sequence) or isinstance(order, (str, bytes)):
        raise BrainRunError(f"{name} must be a sequence")
    normalized = tuple(_text(f"{name} stage id", value, maximum=128) for value in order)
    stage_ids = tuple(stage.id for stage in blueprint.workflow.stages)
    if len(normalized) != len(stage_ids) or len(set(normalized)) != len(normalized) or set(normalized) != set(stage_ids):
        raise BrainRunError(f"{name} must contain every blueprint workflow stage exactly once")
    positions = {stage_id: index for index, stage_id in enumerate(normalized)}
    for stage in blueprint.workflow.stages:
        if any(positions[dependency] >= positions[stage.id] for dependency in stage.depends_on):
            raise BrainRunError(f"{name} violates workflow dependencies")
    return normalized


@dataclass(frozen=True, slots=True)
class AutonomousRoutingHoldoutCase:
    """One caller-owned routing case whose label never enters the routing context."""

    case_id: str
    task: str
    expected_domains: tuple[str, ...]
    split: str = "holdout"

    def __post_init__(self) -> None:
        _identifier("routing holdout case_id", self.case_id)
        _text("routing holdout task", self.task)
        _domains("routing holdout expected_domains", self.expected_domains)
        if self.split != "holdout":
            raise BrainRunError("routing evaluation requires split='holdout'")

    @property
    def task_digest(self) -> str:
        return content_digest({"task": self.task})

    @property
    def label_digest(self) -> str:
        return content_digest({"expected_domains": list(self.expected_domains)})

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_HOLDOUT_EVALUATION_SCHEMA,
            "case_id": self.case_id,
            "split": self.split,
            "task_digest": self.task_digest,
            "expected_label_digest": self.label_digest,
            "retention": "task_and_label_transient_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousRoutingHoldoutReport:
    """Aggregate routing evidence with no task text or reference labels."""

    evaluator_id: str
    evaluator_version: str
    case_count: int
    routed_count: int
    abstained_count: int
    exact_match_count: int
    exact_accuracy: float
    coverage: float
    selective_accuracy: float
    case_statuses: tuple[Mapping[str, Any], ...]
    confusion_digest: str

    def __post_init__(self) -> None:
        _identifier("routing evaluator_id", self.evaluator_id)
        _identifier("routing evaluator_version", self.evaluator_version)
        if not isinstance(self.case_count, int) or isinstance(self.case_count, bool) or not 1 <= self.case_count <= MAX_AUTONOMOUS_HOLDOUT_CASES:
            raise BrainRunError("routing holdout case_count is outside the bound")
        for name, value in (
            ("routed_count", self.routed_count),
            ("abstained_count", self.abstained_count),
            ("exact_match_count", self.exact_match_count),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= self.case_count:
                raise BrainRunError(f"routing holdout {name} is outside the bound")
        if self.routed_count + self.abstained_count != self.case_count:
            raise BrainRunError("routing holdout routed and abstained counts must sum to case_count")
        if self.exact_match_count > self.routed_count:
            raise BrainRunError("routing holdout exact matches cannot exceed routed cases")
        for name, value in (
            ("exact_accuracy", self.exact_accuracy),
            ("coverage", self.coverage),
            ("selective_accuracy", self.selective_accuracy),
        ):
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0.0 <= float(value) <= 1.0:
                raise BrainRunError(f"routing holdout {name} must be within [0, 1]")
        if not isinstance(self.case_statuses, Sequence) or len(self.case_statuses) != self.case_count:
            raise BrainRunError("routing holdout case statuses must align with case_count")
        if any(not isinstance(status, Mapping) for status in self.case_statuses):
            raise BrainRunError("routing holdout case statuses must contain mappings")
        _digest("routing holdout confusion_digest", self.confusion_digest)
        object.__setattr__(self, "exact_accuracy", float(self.exact_accuracy))
        object.__setattr__(self, "coverage", float(self.coverage))
        object.__setattr__(self, "selective_accuracy", float(self.selective_accuracy))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_HOLDOUT_EVALUATION_SCHEMA,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "split": "holdout",
            "case_count": self.case_count,
            "routed_count": self.routed_count,
            "abstained_count": self.abstained_count,
            "exact_match_count": self.exact_match_count,
            "exact_accuracy": self.exact_accuracy,
            "coverage": self.coverage,
            "selective_accuracy": self.selective_accuracy,
            "case_statuses": [dict(status) for status in self.case_statuses],
            "confusion_digest": self.confusion_digest,
            "retention": "aggregate_metrics_and_digests_only",
            "authorization": "evaluation_only; no_tools_or_effects_authorized",
        }


class AutonomousRoutingHoldoutEvaluator:
    """Evaluate provider-free routing without exposing held-out labels to the router."""

    def __init__(self, router: AutonomousTaskRouter, *, evaluator_id: str, evaluator_version: str) -> None:
        if not isinstance(router, AutonomousTaskRouter):
            raise BrainRunError("routing holdout evaluator requires an AutonomousTaskRouter")
        self.router = router
        self.evaluator_id = _identifier("routing evaluator_id", evaluator_id)
        self.evaluator_version = _identifier("routing evaluator_version", evaluator_version)

    def evaluate(
        self,
        cases: Sequence[AutonomousRoutingHoldoutCase],
        *,
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
    ) -> AutonomousRoutingHoldoutReport:
        if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)):
            raise BrainRunError("routing holdout cases must be a sequence")
        normalized = tuple(cases)
        if not 1 <= len(normalized) <= MAX_AUTONOMOUS_HOLDOUT_CASES:
            raise BrainRunError("routing holdout cases must contain 1..256 items")
        if any(not isinstance(case, AutonomousRoutingHoldoutCase) for case in normalized):
            raise BrainRunError("routing holdout cases must contain AutonomousRoutingHoldoutCase values")
        if len({case.case_id for case in normalized}) != len(normalized):
            raise BrainRunError("routing holdout case ids must be unique")
        statuses: list[Mapping[str, Any]] = []
        matrix: list[Mapping[str, Any]] = []
        routed = abstained = exact = 0
        for case in normalized:
            proposal = self.router.route(
                task=case.task,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
            )
            predicted = tuple(proposal.selected_domains)
            if proposal.abstained:
                abstained += 1
                status = "abstained"
                predicted = ()
            else:
                routed += 1
                status = "routed"
                if set(predicted) == set(case.expected_domains):
                    exact += 1
                    status = "exact_match"
            statuses.append(
                {
                    "case_id": case.case_id,
                    "task_digest": case.task_digest,
                    "predicted_domains_digest": content_digest({"domains": list(predicted)}),
                    "route_digest": proposal.route_digest,
                    "status": status,
                }
            )
            matrix.append(
                {
                    "expected": case.label_digest,
                    "predicted": content_digest({"domains": list(predicted)}),
                    "status": status,
                }
            )
        coverage = routed / len(normalized)
        return AutonomousRoutingHoldoutReport(
            evaluator_id=self.evaluator_id,
            evaluator_version=self.evaluator_version,
            case_count=len(normalized),
            routed_count=routed,
            abstained_count=abstained,
            exact_match_count=exact,
            exact_accuracy=exact / len(normalized),
            coverage=coverage,
            selective_accuracy=0.0 if routed == 0 else exact / routed,
            case_statuses=tuple(statuses),
            confusion_digest=content_digest(matrix),
        )


@dataclass(frozen=True, slots=True)
class AutonomousPlanHoldoutCase:
    """A planning holdout case binds one proposal to one exact workflow blueprint."""

    case_id: str
    blueprint: AutonomousTaskBlueprint
    refinement: AutonomousPlanRefinementResult
    expected_priority_stage_ids: tuple[str, ...]

    def __post_init__(self) -> None:
        _identifier("plan holdout case_id", self.case_id)
        if not isinstance(self.blueprint, AutonomousTaskBlueprint):
            raise BrainRunError("plan holdout blueprint must be an AutonomousTaskBlueprint")
        if not isinstance(self.refinement, AutonomousPlanRefinementResult):
            raise BrainRunError("plan holdout refinement must be an AutonomousPlanRefinementResult")
        expected = _workflow_order_is_dependency_valid(
            self.blueprint,
            self.expected_priority_stage_ids,
            name="plan holdout expected priority order",
        )
        _workflow_order_is_dependency_valid(
            self.blueprint,
            self.refinement.priority_stage_ids,
            name="plan holdout refinement priority order",
        )
        if self.refinement.base_plan_digest != content_digest(self.blueprint.plan):
            raise BrainRunError("plan holdout refinement is not bound to the blueprint plan")
        if self.refinement.workflow_digest != self.blueprint.workflow.workflow_digest:
            raise BrainRunError("plan holdout refinement is not bound to the workflow")
        object.__setattr__(self, "expected_priority_stage_ids", expected)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_HOLDOUT_EVALUATION_SCHEMA,
            "case_id": self.case_id,
            "task_digest": self.blueprint.spec.task_digest,
            "workflow_digest": self.blueprint.workflow.workflow_digest,
            "base_plan_digest": content_digest(self.blueprint.plan),
            "refinement": self.refinement.to_dict(),
            "retention": "task_and_reference_order_transient_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousPlanHoldoutReport:
    """Aggregate provider-planning evidence without retaining stage labels or task text."""

    evaluator_id: str
    evaluator_version: str
    case_count: int
    completed_count: int
    exact_order_count: int
    review_count: int
    exact_order_accuracy: float
    case_statuses: tuple[Mapping[str, Any], ...]
    order_digest: str

    def __post_init__(self) -> None:
        _identifier("plan evaluator_id", self.evaluator_id)
        _identifier("plan evaluator_version", self.evaluator_version)
        if not isinstance(self.case_count, int) or isinstance(self.case_count, bool) or not 1 <= self.case_count <= MAX_AUTONOMOUS_HOLDOUT_CASES:
            raise BrainRunError("plan holdout case_count is outside the bound")
        for name, value in (
            ("completed_count", self.completed_count),
            ("exact_order_count", self.exact_order_count),
            ("review_count", self.review_count),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= self.case_count:
                raise BrainRunError(f"plan holdout {name} is outside the bound")
        if self.exact_order_count > self.completed_count:
            raise BrainRunError("plan holdout exact orders cannot exceed completed proposals")
        if not isinstance(self.exact_order_accuracy, (int, float)) or isinstance(self.exact_order_accuracy, bool) or not math.isfinite(float(self.exact_order_accuracy)) or not 0.0 <= float(self.exact_order_accuracy) <= 1.0:
            raise BrainRunError("plan holdout exact_order_accuracy must be within [0, 1]")
        if not isinstance(self.case_statuses, Sequence) or len(self.case_statuses) != self.case_count:
            raise BrainRunError("plan holdout case statuses must align with case_count")
        if any(not isinstance(status, Mapping) for status in self.case_statuses):
            raise BrainRunError("plan holdout case statuses must contain mappings")
        _digest("plan holdout order_digest", self.order_digest)
        object.__setattr__(self, "exact_order_accuracy", float(self.exact_order_accuracy))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_HOLDOUT_EVALUATION_SCHEMA,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "split": "holdout",
            "case_count": self.case_count,
            "completed_count": self.completed_count,
            "exact_order_count": self.exact_order_count,
            "review_count": self.review_count,
            "exact_order_accuracy": self.exact_order_accuracy,
            "case_statuses": [dict(status) for status in self.case_statuses],
            "order_digest": self.order_digest,
            "retention": "aggregate_metrics_and_digests_only",
            "authorization": "evaluation_only; no_tools_or_effects_authorized",
        }


class AutonomousPlanHoldoutEvaluator:
    """Score accepted planner proposals against caller-owned held-out stage orders."""

    def __init__(self, *, evaluator_id: str, evaluator_version: str) -> None:
        self.evaluator_id = _identifier("plan evaluator_id", evaluator_id)
        self.evaluator_version = _identifier("plan evaluator_version", evaluator_version)

    def evaluate(self, cases: Sequence[AutonomousPlanHoldoutCase]) -> AutonomousPlanHoldoutReport:
        if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)):
            raise BrainRunError("plan holdout cases must be a sequence")
        normalized = tuple(cases)
        if not 1 <= len(normalized) <= MAX_AUTONOMOUS_HOLDOUT_CASES:
            raise BrainRunError("plan holdout cases must contain 1..256 items")
        if len({case.case_id for case in normalized}) != len(normalized):
            raise BrainRunError("plan holdout case ids must be unique")
        completed = exact = review = 0
        statuses: list[Mapping[str, Any]] = []
        orders: list[Mapping[str, Any]] = []
        for case in normalized:
            result = case.refinement
            is_completed = result.status == "completed"
            is_exact = is_completed and tuple(result.priority_stage_ids) == case.expected_priority_stage_ids
            if is_completed:
                completed += 1
            if result.review_required:
                review += 1
            if is_exact:
                exact += 1
            statuses.append(
                {
                    "case_id": case.case_id,
                    "task_digest": case.blueprint.spec.task_digest,
                    "workflow_digest": case.blueprint.workflow.workflow_digest,
                    "refinement_digest": content_digest(result.to_dict()),
                    "status": "exact_order" if is_exact else "review" if result.review_required else "non_exact",
                }
            )
            orders.append(
                {
                    "case_id": case.case_id,
                    "expected_order_digest": content_digest({"order": list(case.expected_priority_stage_ids)}),
                    "observed_order_digest": content_digest({"order": list(result.priority_stage_ids)}),
                }
            )
        return AutonomousPlanHoldoutReport(
            evaluator_id=self.evaluator_id,
            evaluator_version=self.evaluator_version,
            case_count=len(normalized),
            completed_count=completed,
            exact_order_count=exact,
            review_count=review,
            exact_order_accuracy=exact / len(normalized),
            case_statuses=tuple(statuses),
            order_digest=content_digest(orders),
        )
