"""Metadata-only evaluator wiring for automatic autonomous learning cycles.

The Python autonomous façade already contains reviewed value-only evaluators and bounded online,
trajectory, and replan learners.  What applications previously had to assemble themselves was
the safe hand-off between an executed run and the caller's independent evidence system.  This
module supplies that hand-off without making the SDK a source of domain truth:

* the registry must cover every built-in autonomous domain;
* the evidence callback receives route/identity/status metadata only;
* evidence remains transient and is never copied into the bridge or its catalogue;
* single-domain adapters preserve the exact reviewed evaluator identity; and
* cross-domain adapters use the exact specialist rubric per routed domain while exposing one
  stable composite identity to the trajectory ledger.

Provider success, response presence, model confidence, and transport latency are deliberately
not converted into reward.  The caller must return bounded value-only evidence, which the normal
``DomainEvaluatorAdapter`` validates before a decision can cross the learning boundary.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import re
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_evaluator_calibration import (
    admit_autonomous_evaluator_calibration,
    validate_autonomous_evaluator_calibration_report,
)
from .autonomous_evidence_source import AutonomousEvidenceSourceReceipt
from .brain import (
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainLearningTrajectory,
    BrainOutcomeEvaluator,
    BrainRunError,
    build_brain_evaluation_input_from_metadata,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .evaluators import (
    DOMAIN_EVALUATOR_SCHEMA,
    DomainEvaluatorAdapter,
    DomainEvaluatorRegistry,
)


AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA = "bioprism-python-autonomous-cycle-evaluator-bridge/0.1"
AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION = (
    "metadata_only;caller_evidence_factory_owns_values"
)
AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_POLICY = "caller_declared_value_only_evidence"
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_MAX_CONTEXT_TEXT_BYTES = 256
_MAX_SELECTED_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)


def _bounded_text(value: Any, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > _MAX_CONTEXT_TEXT_BYTES:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return value


def _optional_digest(value: Any, name: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        raise BrainRunError(f"{name} must be a lowercase SHA-256 digest or None")
    return value


def _normalize_domains(value: Sequence[str], name: str, *, allow_cross_domain: bool) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise BrainRunError(f"{name} must be a sequence")
    if not value or len(value) > _MAX_SELECTED_DOMAINS:
        raise BrainRunError(f"{name} must contain between 1 and {_MAX_SELECTED_DOMAINS} entries")
    normalized: list[str] = []
    for domain in value:
        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise BrainRunError(f"{name} contains an unsupported autonomous domain")
        if not allow_cross_domain and domain == "cross_domain":
            raise BrainRunError(f"{name} cannot contain the cross_domain synthesis domain")
        if domain in normalized:
            raise BrainRunError(f"{name} contains duplicate domains")
        normalized.append(domain)
    return tuple(normalized)


@dataclass(frozen=True, slots=True)
class AutonomousCycleEvaluatorEvidenceContext:
    """The only context exposed to a bridge evidence factory.

    This projection intentionally has no task text, prompt content, provider response, tool
    arguments, credential handle, or evidence body.  ``to_dict`` returns a fresh mapping so a
    callback cannot mutate the bridge's internal metadata.
    """

    schema: str
    mode: str
    domain: str
    role: str
    run_id: str | None
    run_status: str
    result_kind: str
    outcome_digest: str | None
    learning_outcome_digest: str | None
    context_digest: str | None
    route_digest: str | None
    learning_episode_id: str | None
    learning_episode_ids: tuple[str, ...]
    selected_domains: tuple[str, ...]
    child_count: int
    completed_child_count: int
    evaluator_id: str
    evaluator_version: str
    required_signals: tuple[str, ...]
    pass_threshold: float
    source_receipt_digest: str | None = None
    source_id: str | None = None
    source_kind: str | None = None
    source_authority: str | None = None
    source_freshness: str | None = None
    source_decision: str = "not_configured"
    evaluator_calibration_digest: str | None = None
    evaluator_calibration_decision: str = "not_configured"
    retention: str = AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION
    secret_material: str = "never_returned"

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "mode": self.mode,
            "domain": self.domain,
            "role": self.role,
            "run_id": self.run_id,
            "run_status": self.run_status,
            "result_kind": self.result_kind,
            "outcome_digest": self.outcome_digest,
            "learning_outcome_digest": self.learning_outcome_digest,
            "context_digest": self.context_digest,
            "route_digest": self.route_digest,
            "learning_episode_id": self.learning_episode_id,
            "learning_episode_ids": list(self.learning_episode_ids),
            "selected_domains": list(self.selected_domains),
            "child_count": self.child_count,
            "completed_child_count": self.completed_child_count,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "required_signals": list(self.required_signals),
            "pass_threshold": self.pass_threshold,
            "source_receipt_digest": self.source_receipt_digest,
            "source_id": self.source_id,
            "source_kind": self.source_kind,
            "source_authority": self.source_authority,
            "source_freshness": self.source_freshness,
            "source_decision": self.source_decision,
            "evaluator_calibration_digest": self.evaluator_calibration_digest,
            "evaluator_calibration_decision": self.evaluator_calibration_decision,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


AutonomousCycleEvaluatorEvidenceFactory = Callable[
    [Mapping[str, Any]], Mapping[str, Any]
]
AutonomousCycleEvaluatorSourceReceiptFactory = Callable[
    [Mapping[str, Any]], Mapping[str, Any] | None
]
AutonomousCycleEvaluatorCalibrationFactory = Callable[
    [Mapping[str, Any]], Mapping[str, Any] | None
]


def _metadata_context(
    evaluation_input: Mapping[str, Any],
    *,
    mode: str,
    domain: str,
    role: str,
    selected_domains: tuple[str, ...],
    evaluator: DomainEvaluatorAdapter,
) -> AutonomousCycleEvaluatorEvidenceContext:
    raw_context = evaluation_input.get("context")
    context = raw_context if isinstance(raw_context, Mapping) else {}
    observed_domain = context.get("domain")
    if observed_domain is not None and observed_domain != domain:
        raise BrainRunError(
            f"cycle evaluator bridge received {domain!r} evaluator for {observed_domain!r} context"
        )
    route = context.get("autonomous_route")
    route_digest = route.get("route_digest") if isinstance(route, Mapping) else context.get("route_digest")
    context_digest = evaluation_input.get("context_digest", context.get("context_digest"))
    run_id = evaluation_input.get("run_id")
    run_status = evaluation_input.get("status", "unknown")
    result_kind = evaluation_input.get("result_kind", "unknown")
    for name, value in (
        ("cycle evaluator bridge run_id", run_id),
        ("cycle evaluator bridge run_status", run_status),
        ("cycle evaluator bridge result_kind", result_kind),
    ):
        if value is not None:
            _bounded_text(value, name)
    return AutonomousCycleEvaluatorEvidenceContext(
        schema=AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
        mode=mode,
        domain=domain,
        role=role,
        run_id=run_id,
        run_status=run_status,
        result_kind=result_kind,
        outcome_digest=_optional_digest(evaluation_input.get("outcome_digest"), "cycle evaluator bridge outcome_digest"),
        learning_outcome_digest=_optional_digest(
            evaluation_input.get("learning_outcome_digest"),
            "cycle evaluator bridge learning_outcome_digest",
        ),
        context_digest=_optional_digest(context_digest, "cycle evaluator bridge context_digest"),
        route_digest=_optional_digest(route_digest, "cycle evaluator bridge route_digest"),
        learning_episode_id=None,
        learning_episode_ids=(),
        selected_domains=selected_domains,
        child_count=len(selected_domains) if mode == "cross_domain" else 0,
        completed_child_count=(
            len(selected_domains)
            if mode == "cross_domain" and isinstance(run_status, str) and run_status.startswith("completed")
            else int(isinstance(run_status, str) and run_status.startswith("completed"))
        ),
        evaluator_id=evaluator.evaluator_id,
        evaluator_version=evaluator.evaluator_version,
        required_signals=tuple(evaluator.profile.required_signals),
        pass_threshold=float(evaluator.profile.pass_threshold),
    )


class _EvidenceBoundDomainEvaluator(BrainOutcomeEvaluator):
    """Preserve an exact domain evaluator identity while sourcing evidence on demand."""

    def __init__(
        self,
        inner: DomainEvaluatorAdapter,
        *,
        evidence_for: AutonomousCycleEvaluatorEvidenceFactory,
        mode: str,
        role: str,
        selected_domains: tuple[str, ...],
        source_receipt_for: AutonomousCycleEvaluatorSourceReceiptFactory | None,
        evaluator_calibration_for: AutonomousCycleEvaluatorCalibrationFactory | None,
    ) -> None:
        if not isinstance(inner, DomainEvaluatorAdapter):
            raise BrainRunError("cycle evaluator bridge inner evaluator is malformed")
        self.inner = inner
        self.profile = inner.profile
        self._evidence_for = evidence_for
        self._mode = mode
        self._role = role
        self._selected_domains = selected_domains
        self._source_receipt_for = source_receipt_for
        self._evaluator_calibration_for = evaluator_calibration_for
        super().__init__(
            self._unused_callback,
            evaluator_id=inner.evaluator_id,
            evaluator_version=inner.evaluator_version,
        )

    @staticmethod
    def _unused_callback(_evaluation_input: Mapping[str, Any]) -> Mapping[str, Any]:
        raise BrainRunError("cycle evaluator bridge callback was not routed through its evidence boundary")

    def _assess_with_evidence_boundary(
        self,
        evaluation_input: Mapping[str, Any],
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        if not isinstance(evaluation_input, Mapping):
            raise BrainRunError("cycle evaluator bridge input must be a mapping")
        supplied = evaluation_input.get("evidence")
        if supplied not in (None, {}):
            raise BrainRunError(
                "cycle evaluator bridge does not accept inline evidence; evidence_for owns the value boundary"
            )
        context = _metadata_context(
            evaluation_input,
            mode=self._mode,
            domain=self.profile.domain,
            role=self._role,
            selected_domains=self._selected_domains,
            evaluator=self.inner,
        )
        context = self._admitted_context(context)
        try:
            evidence = self._evidence_for(context.to_dict())
        except BrainRunError:
            raise
        except Exception as error:
            raise BrainRunError("cycle evaluator bridge evidence callback failed") from error
        if not isinstance(evidence, Mapping):
            raise BrainRunError("cycle evaluator bridge evidence callback must return a mapping")
        evidence_copy = dict(evidence)
        BrainLearningLedger._assert_safe(evidence_copy)
        generated = dict(evaluation_input)
        generated["evidence"] = evidence_copy
        generated["evidence_digest"] = content_digest(evidence_copy)
        return self.inner.assess_value_only_input(generated), evidence_copy

    def _admitted_context(
        self,
        context: AutonomousCycleEvaluatorEvidenceContext,
    ) -> AutonomousCycleEvaluatorEvidenceContext:
        admitted = context
        if self._source_receipt_for is not None:
            try:
                raw_receipt = self._source_receipt_for(admitted.to_dict())
                if raw_receipt is None:
                    raise BrainRunError(
                        "cycle evaluator bridge source receipt is required when source_receipt_for is configured"
                    )
                receipt = AutonomousEvidenceSourceReceipt.from_dict(raw_receipt)
            except BrainRunError:
                raise
            except Exception as error:
                raise BrainRunError(
                    "cycle evaluator bridge source receipt is malformed"
                ) from error
            if receipt.domain != context.domain:
                raise BrainRunError(
                    "cycle evaluator bridge source receipt domain does not match the routed evaluator"
                )
            if (
                receipt.decision != "accepted"
                or receipt.status != "observed"
                or receipt.source_digest is None
                or receipt.authority == "caller_declared"
            ):
                raise BrainRunError(
                    "cycle evaluator bridge source receipt is not an accepted authoritative observation"
                )
            admitted = replace(
                admitted,
                source_receipt_digest=receipt.receipt_digest,
                source_id=receipt.source_id,
                source_kind=receipt.source_kind,
                source_authority=receipt.authority,
                source_freshness=receipt.freshness,
                source_decision="accepted",
            )
        if self._evaluator_calibration_for is not None:
            try:
                raw_report = self._evaluator_calibration_for(admitted.to_dict())
                if raw_report is None:
                    raise BrainRunError(
                        "cycle evaluator bridge evaluator calibration report is required when evaluator_calibration_for is configured"
                    )
                report = validate_autonomous_evaluator_calibration_report(raw_report)
                admission = admit_autonomous_evaluator_calibration(report, context.domain)
            except BrainRunError:
                raise
            except Exception as error:
                raise BrainRunError(
                    "cycle evaluator bridge evaluator calibration report is malformed"
                ) from error
            if (
                admission["decision"] != "admit_learning"
                or admission["evaluator_id"] != context.evaluator_id
                or admission["evaluator_version"] != context.evaluator_version
            ):
                raise BrainRunError(
                    f"cycle evaluator bridge evaluator calibration holds {context.domain} learning"
                )
            admitted = replace(
                admitted,
                evaluator_calibration_digest=report["report_digest"],
                evaluator_calibration_decision="admit_learning",
            )
        return admitted

    def _assess_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        return self._assess_with_evidence_boundary(evaluation_input)[0]

    def evaluate_trajectory(
        self,
        brain: Any,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        ledger: Any = None,
    ) -> Any:
        return _evaluate_bridge_trajectory(
            self,
            brain,
            trajectory,
            bandit_state=bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=ledger,
        )

    def catalogue_entry(self) -> dict[str, Any]:
        return self.inner.catalogue_entry()


class _EvidenceCompositeDomainEvaluator(BrainOutcomeEvaluator):
    """Composite cross-domain routing with one evidence boundary per trajectory step."""

    def __init__(
        self,
        evaluators: Mapping[str, _EvidenceBoundDomainEvaluator],
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> None:
        if not evaluators or any(
            not isinstance(domain, str) or not isinstance(evaluator, _EvidenceBoundDomainEvaluator)
            for domain, evaluator in evaluators.items()
        ):
            raise BrainRunError("cycle evaluator bridge composite evaluators are malformed")
        self.evaluators = dict(evaluators)
        super().__init__(
            self._unused_callback,
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
        )

    @staticmethod
    def _unused_callback(_evaluation_input: Mapping[str, Any]) -> Mapping[str, Any]:
        raise BrainRunError("cycle evaluator bridge composite callback was not routed through its boundary")

    def _assess_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        return self._assess_with_evidence_boundary(evaluation_input)[0]

    def _assess_with_evidence_boundary(
        self,
        evaluation_input: Mapping[str, Any],
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        context = evaluation_input.get("context") if isinstance(evaluation_input, Mapping) else None
        domain = context.get("domain") if isinstance(context, Mapping) else None
        evaluator = self.evaluators.get(domain) if isinstance(domain, str) else None
        if evaluator is None:
            return (
                BrainEvaluatorDecision(
                    evaluator_id=self.evaluator_id,
                    evaluator_version=self.evaluator_version,
                    reward=0.0,
                    passed=False,
                    failed=True,
                    failure_class="unmapped_domain_evaluator",
                    replan_requested=True,
                    replan_instruction="Provide an explicit reviewed evaluator for the routed domain.",
                ),
                {},
            )
        decision, evidence = evaluator._assess_with_evidence_boundary(evaluation_input)
        return (
            replace(
                decision,
                evaluator_id=self.evaluator_id,
                evaluator_version=self.evaluator_version,
            ),
            evidence,
        )

    def evaluate_trajectory(
        self,
        brain: Any,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        ledger: Any = None,
    ) -> Any:
        return _evaluate_bridge_trajectory(
            self,
            brain,
            trajectory,
            bandit_state=bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=ledger,
        )

    def catalogue_entry(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "domains": [
                {
                    "domain": domain,
                    "evaluator_id": evaluator.inner.evaluator_id,
                    "evaluator_version": evaluator.inner.evaluator_version,
                }
                for domain, evaluator in sorted(self.evaluators.items())
            ],
            "execution": "metadata_only_value_boundary_routing",
            "retention": "evaluator_id_version_and_domain_keys_only",
        }


def _evaluate_bridge_trajectory(
    evaluator: BrainOutcomeEvaluator,
    brain: Any,
    trajectory: BrainLearningTrajectory | Mapping[str, Any],
    *,
    bandit_state: Mapping[str, Any],
    evidence_by_step: Sequence[Mapping[str, Any] | None] | None,
    ledger: Any,
) -> Any:
    """Evaluate once, retain packets only in memory, then settle with matching digests."""

    if evidence_by_step is not None:
        if not isinstance(evidence_by_step, Sequence) or isinstance(evidence_by_step, (str, bytes)):
            raise BrainRunError("cycle evaluator bridge trajectory evidence must be a sequence or None")
        if any(item is not None for item in evidence_by_step):
            raise BrainRunError(
                "cycle evaluator bridge does not accept inline trajectory evidence"
            )
    normalized = (
        trajectory
        if isinstance(trajectory, BrainLearningTrajectory)
        else BrainLearningTrajectory.from_mapping(trajectory)
    )
    decisions: list[BrainEvaluatorDecision] = []
    evidence_packets: list[Mapping[str, Any]] = []
    for episode in normalized.episodes:
        metadata = build_brain_evaluation_input_from_metadata(episode.evaluation_input)
        if isinstance(evaluator, _EvidenceBoundDomainEvaluator):
            decision, evidence = evaluator._assess_with_evidence_boundary(metadata)
        elif isinstance(evaluator, _EvidenceCompositeDomainEvaluator):
            decision, evidence = evaluator._assess_with_evidence_boundary(metadata)
        else:  # pragma: no cover - bridge factories only construct the two types above.
            raise BrainRunError("cycle evaluator bridge trajectory evaluator is malformed")
        decisions.append(decision)
        evidence_packets.append(evidence)
    return evaluator.settle_trajectory(
        brain,
        normalized,
        decisions=decisions,
        bandit_state=bandit_state,
        evidence_by_step=evidence_packets,
        ledger=ledger,
    )


class AutonomousCycleEvaluatorBridge:
    """Build exact-domain evaluators for automatic online, trajectory, and replan paths."""

    schema = AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA

    def __init__(
        self,
        evidence_for: AutonomousCycleEvaluatorEvidenceFactory,
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        source_receipt_for: AutonomousCycleEvaluatorSourceReceiptFactory | None = None,
        evaluator_calibration_for: AutonomousCycleEvaluatorCalibrationFactory | None = None,
    ) -> None:
        if not callable(evidence_for):
            raise BrainRunError("cycle evaluator bridge evidence_for must be callable")
        registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("cycle evaluator bridge evaluator_registry must be a DomainEvaluatorRegistry")
        for domain in AUTONOMOUS_DOMAIN_NAMES:
            registry.resolve_for_autonomous_domain(domain)
        self.registry = registry
        self._evidence_for = evidence_for
        if source_receipt_for is not None and not callable(source_receipt_for):
            raise BrainRunError("cycle evaluator bridge source_receipt_for must be callable or None")
        if evaluator_calibration_for is not None and not callable(evaluator_calibration_for):
            raise BrainRunError("cycle evaluator bridge evaluator_calibration_for must be callable or None")
        self._source_receipt_for = source_receipt_for
        self._evaluator_calibration_for = evaluator_calibration_for
        self._single: dict[tuple[str, str, str, tuple[str, ...]], BrainOutcomeEvaluator] = {}
        self._cross: dict[tuple[str, ...], BrainOutcomeEvaluator] = {}
        self.evaluator_catalogue_digest = content_digest(
            {
                "schema": self.schema,
                "catalogue": registry.catalogue(),
                "authority": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_POLICY,
                "retention": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION,
            }
        )
        self.policy_digest = content_digest(
            {
                "schema": self.schema,
                "evaluator_catalogue_digest": self.evaluator_catalogue_digest,
                "modes": ["single_domain", "cross_domain"],
                "roles": ["single", "specialist", "synthesis"],
                "provider_success_is_not_reward": True,
                "reward_source": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_POLICY,
                "source_receipt_admission": "optional;accepted_observed_non_caller_declared_source_digest_required",
                "evaluator_calibration_admission": "optional;ready_exact_evaluator_identity_required",
                "retention": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION,
            }
        )

    def evaluator_for_domain(self, domain: str) -> BrainOutcomeEvaluator:
        """Return an exact reviewed evaluator for a single-domain learning cycle."""

        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAIN_NAMES or domain == "cross_domain":
            raise BrainRunError("cycle evaluator bridge single-domain evaluator requires a built-in non-synthesis domain")
        key = ("single_domain", domain, "single", (domain,))
        if key not in self._single:
            inner = self.registry.resolve_for_autonomous_domain(domain)
            self._single[key] = _EvidenceBoundDomainEvaluator(
                inner,
                evidence_for=self._evidence_for,
                mode="single_domain",
                role="single",
                selected_domains=(domain,),
                source_receipt_for=self._source_receipt_for,
                evaluator_calibration_for=self._evaluator_calibration_for,
            )
        return self._single[key]

    def evaluator_for_cross_domain(
        self,
        selected_domains: Sequence[str] | None = None,
    ) -> BrainOutcomeEvaluator:
        """Return a composite evaluator for a reviewed cross-domain trajectory.

        ``selected_domains`` names specialists only.  The synthesis episode is always routed
        through the reviewed ``cross_domain`` evaluator and receives the ``synthesis`` role.
        """

        domains = _normalize_domains(
            tuple(domain for domain in AUTONOMOUS_DOMAIN_NAMES if domain != "cross_domain")
            if selected_domains is None
            else selected_domains,
            "cycle evaluator bridge selected_domains",
            allow_cross_domain=False,
        )
        if domains not in self._cross:
            evaluators: dict[str, _EvidenceBoundDomainEvaluator] = {}
            for domain in domains:
                inner = self.registry.resolve_for_autonomous_domain(domain)
                evaluators[domain] = _EvidenceBoundDomainEvaluator(
                    inner,
                    evidence_for=self._evidence_for,
                    mode="cross_domain",
                    role="specialist",
                    selected_domains=domains,
                    source_receipt_for=self._source_receipt_for,
                    evaluator_calibration_for=self._evaluator_calibration_for,
                )
            synthesis = self.registry.resolve_for_autonomous_domain("cross_domain")
            evaluators["cross_domain"] = _EvidenceBoundDomainEvaluator(
                synthesis,
                evidence_for=self._evidence_for,
                mode="cross_domain",
                role="synthesis",
                selected_domains=domains,
                source_receipt_for=self._source_receipt_for,
                evaluator_calibration_for=self._evaluator_calibration_for,
            )
            self._cross[domains] = _EvidenceCompositeDomainEvaluator(
                evaluators,
                evaluator_id="autonomous-cycle-cross-domain-quality",
                evaluator_version="1",
            )
        return self._cross[domains]

    def to_dict(self) -> dict[str, Any]:
        """Return a bounded catalogue/policy projection suitable for readiness reports."""

        return {
            "schema": self.schema,
            "evaluator_catalogue_digest": self.evaluator_catalogue_digest,
            "policy_digest": self.policy_digest,
            "domain_count": len(AUTONOMOUS_DOMAIN_NAMES),
            "domains": list(AUTONOMOUS_DOMAIN_NAMES),
            "retention": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION,
            "reward_source": AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_POLICY,
            "provider_success_is_not_reward": True,
            "source_receipt_gate": self._source_receipt_for is not None,
            "evaluator_calibration_gate": self._evaluator_calibration_for is not None,
            "secret_material": "never_returned",
        }


def create_autonomous_cycle_evaluator_bridge(
    evidence_for: AutonomousCycleEvaluatorEvidenceFactory,
    *,
    evaluator_registry: DomainEvaluatorRegistry | None = None,
    source_receipt_for: AutonomousCycleEvaluatorSourceReceiptFactory | None = None,
    evaluator_calibration_for: AutonomousCycleEvaluatorCalibrationFactory | None = None,
) -> AutonomousCycleEvaluatorBridge:
    """Create a reviewed all-domain bridge for caller-owned evidence factories."""

    return AutonomousCycleEvaluatorBridge(
        evidence_for,
        evaluator_registry=evaluator_registry,
        source_receipt_for=source_receipt_for,
        evaluator_calibration_for=evaluator_calibration_for,
    )


__all__ = [
    "AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA",
    "AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_RETENTION",
    "AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_POLICY",
    "AutonomousCycleEvaluatorEvidenceContext",
    "AutonomousCycleEvaluatorEvidenceFactory",
    "AutonomousCycleEvaluatorSourceReceiptFactory",
    "AutonomousCycleEvaluatorCalibrationFactory",
    "AutonomousCycleEvaluatorBridge",
    "create_autonomous_cycle_evaluator_bridge",
]
