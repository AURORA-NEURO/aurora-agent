"""Reviewed, domain-specific quality policies for structured autonomous responses.

The response schema is intentionally stable across providers.  This module supplies the missing
operating knowledge: each built-in domain gets prompt guidance, critical decision fields, safety
controls, and stage-specific completeness rules.  The evaluator only measures structural
readiness and never establishes external truth or authorizes an effect.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA = "bioprism-python-autonomous-domain-quality-policy/0.1"
AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION = "1"
AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA = "bioprism-python-autonomous-domain-quality-report/0.1"
AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD = 0.8
MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTIONS = 12
MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTION_BYTES = 2_048

AUTONOMOUS_DOMAIN_NAMES = (
    "coding", "browser", "data", "science", "biomedical", "neuroscience",
    "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation",
)
_DETAIL_FIELDS: dict[str, tuple[str, ...]] = {
    "coding": ("files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"),
    "browser": ("sources", "citations", "freshness", "retrieval_gaps"),
    "data": ("schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"),
    "science": ("estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"),
    "biomedical": ("scope_boundary", "provenance", "population_and_applicability", "uncertainty", "human_review_and_escalation"),
    "neuroscience": ("measurement_contract", "preprocessing_and_exclusions", "confounds", "model_sensitivity", "validation_plan"),
    "operations": ("observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"),
    "enterprise": ("stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"),
    "multi_agent": ("subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"),
    "multimodal": ("available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"),
    "cross_domain": ("domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"),
    "evaluation": ("rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"),
}
_SEEDS: dict[str, dict[str, tuple[str, ...]]] = {
    "coding": {
        "critical": ("files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"),
        "safety": ("residual_risks", "rollback_or_follow_up"),
        "instructions": (
            "Name the exact files, modules, interfaces, or deployment units in scope; distinguish inspected artifacts from proposed edits.",
            "Report verification as executable checks with their observed result, not as a claim that generated code is correct.",
            "Call out compatibility, security, migration, and operational risks, including the smallest safe rollback or follow-up action.",
            "Keep implementation, test evidence, and remaining work separate so a reviewer can reproduce the decision.",
        ),
    },
    "browser": {
        "critical": ("sources", "citations", "freshness", "retrieval_gaps"),
        "safety": ("freshness", "retrieval_gaps"),
        "instructions": (
            "Identify each source and retrieval boundary; do not turn a search result, snippet, or unvisited link into verified evidence.",
            "Attach claims to citations and state publication or retrieval freshness whenever time can change the answer.",
            "Separate source-reported observations from synthesis and disclose inaccessible, conflicting, or missing sources.",
            "Never imply that browsing performed an external action unless a caller-owned effect receipt explicitly proves it.",
        ),
    },
    "data": {
        "critical": ("schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"),
        "safety": ("quality_metrics", "anomalies_and_transformations"),
        "instructions": (
            "State grain, schema, units, null semantics, time basis, and population before interpreting a metric or transformation.",
            "Trace important values to their input and transformation lineage; distinguish observed measurements from calculated estimates.",
            "Report quality metrics, missingness, outliers, leakage, and anomalies before presenting a conclusion.",
            "Make transformations reproducible and identify any irreversible or lossy operation that needs caller approval.",
        ),
    },
    "science": {
        "critical": ("estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"),
        "safety": ("estimand_and_assumptions", "design_and_controls", "reproduction_plan"),
        "instructions": (
            "Define the estimand, population, assumptions, and decision criterion before describing a result as supporting a hypothesis.",
            "Map evidence to claims and distinguish prior literature, supplied observations, model output, and speculation.",
            "State falsifiable predictions, controls, confounds, and the smallest reproduction or sensitivity check that could change the conclusion.",
            "Do not convert an association, simulation, or proposed experiment into a causal or externally validated finding.",
        ),
    },
    "biomedical": {
        "critical": ("scope_boundary", "provenance", "population_and_applicability", "uncertainty", "human_review_and_escalation"),
        "safety": ("scope_boundary", "uncertainty", "human_review_and_escalation"),
        "instructions": (
            "State the clinical or biological scope and separate educational analysis from diagnosis, treatment, or patient-specific advice.",
            "Track provenance, cohort, sample limitations, applicability, and uncertainty for every clinically meaningful claim.",
            "Escalate decisions requiring licensed, ethical, institutional, or patient-specific review; do not silently fill missing clinical context.",
            "Treat generated text, literature summaries, and model outputs as reviewable evidence projections, never as medical authorization.",
        ),
    },
    "neuroscience": {
        "critical": ("measurement_contract", "preprocessing_and_exclusions", "confounds", "model_sensitivity", "validation_plan"),
        "safety": ("preprocessing_and_exclusions", "confounds", "model_sensitivity", "validation_plan"),
        "instructions": (
            "Define the signal, sampling, cohort, task, units, and measurement validity before interpreting a neural effect.",
            "Make preprocessing, exclusions, artifact handling, leakage controls, and multiple-comparison choices explicit.",
            "Report confounds and model sensitivity, including whether the conclusion survives reasonable preprocessing or specification changes.",
            "Provide a validation and reproduction plan; do not equate decoded, simulated, or correlated activity with mechanism or subjective experience.",
        ),
    },
    "operations": {
        "critical": ("observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"),
        "safety": ("blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"),
        "instructions": (
            "Describe the observed state, scope, dependencies, and blast radius before proposing a change or remediation.",
            "Define measurable stop conditions, rollback or recovery steps, and the owner who can approve an effect.",
            "Keep simulation, recommendation, dry-run, and dispatched effect separate; an agent response never proves an operational change occurred.",
            "Surface approval requirements and unknowns before irreversible, customer-facing, security-sensitive, or high-blast-radius work.",
        ),
    },
    "enterprise": {
        "critical": ("stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"),
        "safety": ("policy_constraints", "decision_and_approver", "audit_plan"),
        "instructions": (
            "Name stakeholders, accountable owners, decision rights, and impacted systems rather than treating an organization as a single actor.",
            "State applicable policy, compliance, contractual, privacy, and segregation-of-duties constraints before ranking options.",
            "Compare alternatives with explicit tradeoffs, reversibility, cost, risk, and evidence; record who must approve the decision.",
            "Define the audit trail and follow-up measurement needed to verify adoption without claiming organizational effect prematurely.",
        ),
    },
    "multi_agent": {
        "critical": ("subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"),
        "safety": ("assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"),
        "instructions": (
            "Decompose work into bounded subtasks with typed inputs, outputs, budgets, dependencies, and a clear completion condition.",
            "Record agent or worker assignments and reconcile outputs by digest, provenance, and contract rather than majority vote alone.",
            "Preserve dissent, conflicts, and missing outputs; do not hide disagreement in a synthesized narrative.",
            "Name the accountable authority for approval and final decisions; delegation does not transfer responsibility to an unreviewed worker.",
        ),
    },
    "multimodal": {
        "critical": ("available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"),
        "safety": ("alignment", "missing_modalities", "blind_spots"),
        "instructions": (
            "Inventory available modalities, resolution, timestamps, provenance, and missing inputs before making a cross-modal claim.",
            "Keep modality-specific observations separate and describe how records, coordinates, identities, or time windows were aligned.",
            "Call out blind spots, unobserved modalities, quality differences, and contradictory signals instead of averaging them away.",
            "Do not infer a real-world event from a generated or weakly aligned modality without an explicit validation boundary.",
        ),
    },
    "cross_domain": {
        "critical": ("domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"),
        "safety": ("terminology_and_units", "disagreements", "decision_gate", "open_questions"),
        "instructions": (
            "Attribute each material claim to its contributing domain and preserve domain-specific assumptions, units, and terminology.",
            "Reconcile disagreements explicitly, including incompatible evidence, time bases, populations, or definitions.",
            "State the decision gate, authority, and evidence required before a cross-domain recommendation can advance.",
            "Keep unresolved questions visible; synthesis is not permission to erase uncertainty or claim one domain validated another.",
        ),
    },
    "evaluation": {
        "critical": ("rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"),
        "safety": ("rubric_and_pass_criteria", "failures_and_regressions", "reproduction_and_next_learning"),
        "instructions": (
            "Define measurable pass criteria, evaluator authority, and the boundary between structural checks and external correctness.",
            "Report case coverage, representative failures, regressions, flaky behavior, and replay determinism rather than only aggregate reward.",
            "Preserve failure identity and reproduction steps so the next learning or bandit update can be audited.",
            "Never treat a high score from an incomplete test set as proof that the underlying agent, model, or external task is correct.",
        ),
    },
}
_STAGE_REQUIREMENTS = {
    "complete": {"evidence": True, "findings": True, "uncertainty": False, "open_questions": False},
    "partial": {"evidence": True, "findings": True, "uncertainty": True, "open_questions": True},
    "blocked": {"evidence": False, "findings": False, "uncertainty": True, "open_questions": True},
    "not_attempted": {"evidence": False, "findings": False, "uncertainty": True, "open_questions": True},
}


def _text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is malformed")
    return value


def _fraction(total: int, satisfied: float) -> float:
    return 0.0 if total <= 0 else round(max(0.0, min(1.0, satisfied / total)), 12)


@dataclass(frozen=True, slots=True)
class AutonomousDomainQualityPolicy:
    schema: str
    version: str
    domain: str
    required_detail_fields: tuple[str, ...]
    critical_detail_fields: tuple[str, ...]
    safety_detail_fields: tuple[str, ...]
    required_top_level_sections: tuple[str, ...]
    stage_requirements: Mapping[str, Mapping[str, bool]]
    prompt_instructions: tuple[str, ...]
    policy_digest: str
    retention: str
    secret_material: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "version": self.version,
            "domain": self.domain,
            "required_detail_fields": list(self.required_detail_fields),
            "critical_detail_fields": list(self.critical_detail_fields),
            "safety_detail_fields": list(self.safety_detail_fields),
            "required_top_level_sections": list(self.required_top_level_sections),
            "stage_requirements": {key: dict(value) for key, value in self.stage_requirements.items()},
            "prompt_instructions": list(self.prompt_instructions),
            "policy_digest": self.policy_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainQualityReport:
    schema: str
    domain: str
    policy_digest: str
    signals: Mapping[str, float]
    weights: Mapping[str, float]
    missing_signals: tuple[str, ...]
    recommendations: tuple[str, ...]
    score: float
    passed: bool
    authority: str
    retention: str
    secret_material: str
    report_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": self.schema,
            "domain": self.domain,
            "policy_digest": self.policy_digest,
            "signals": dict(self.signals),
            "weights": dict(self.weights),
            "missing_signals": list(self.missing_signals),
            "recommendations": list(self.recommendations),
            "score": self.score,
            "passed": self.passed,
            "authority": self.authority,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        return {**descriptor, "report_digest": self.report_digest}


def _build(domain: str) -> AutonomousDomainQualityPolicy:
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError(f"unsupported autonomous domain quality policy: {domain!r}")
    seed = _SEEDS[domain]
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA,
        "version": AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION,
        "domain": domain,
        "required_detail_fields": list(_DETAIL_FIELDS[domain]),
        "critical_detail_fields": list(seed["critical"]),
        "safety_detail_fields": list(seed["safety"]),
        "required_top_level_sections": ["observations", "inferences", "uncertainty", "evidence_gaps", "next_actions"],
        "stage_requirements": {key: dict(value) for key, value in _STAGE_REQUIREMENTS.items()},
        "prompt_instructions": list(seed["instructions"]),
        "retention": "policy_metadata_only;does_not_establish_external_truth",
        "secret_material": "never_returned",
    }
    return AutonomousDomainQualityPolicy(
        **descriptor,
        policy_digest=content_digest(descriptor),
    )


_CACHE: dict[str, AutonomousDomainQualityPolicy] = {}


def autonomous_domain_quality_policy(domain: str) -> AutonomousDomainQualityPolicy:
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError(f"unsupported autonomous domain quality policy: {domain!r}")
    if domain not in _CACHE:
        _CACHE[domain] = _build(domain)
    return _CACHE[domain]


def builtin_autonomous_domain_quality_policies() -> tuple[AutonomousDomainQualityPolicy, ...]:
    return tuple(autonomous_domain_quality_policy(domain) for domain in AUTONOMOUS_DOMAIN_NAMES)


def validate_autonomous_domain_quality_policy(value: AutonomousDomainQualityPolicy | Mapping[str, Any]) -> AutonomousDomainQualityPolicy:
    if isinstance(value, AutonomousDomainQualityPolicy):
        candidate = value
    elif isinstance(value, Mapping):
        domain = value.get("domain")
        if not isinstance(domain, str):
            raise ArgumentError("domain quality policy domain is invalid")
        candidate = _build(domain)
        if dict(value) != candidate.to_dict():
            raise ArgumentError("domain quality policy is stale or tampered")
    else:
        raise ArgumentError("domain quality policy must be a mapping")
    current = autonomous_domain_quality_policy(candidate.domain)
    if candidate.to_dict() != current.to_dict():
        raise ArgumentError("domain quality policy is stale or tampered")
    return current


def _stage_quality(stage: Any, policy: AutonomousDomainQualityPolicy) -> float:
    requirements = policy.stage_requirements.get(stage.status)
    if requirements is None:
        return 0.0
    checks = (
        not requirements["evidence"] or bool(stage.evidence),
        not requirements["findings"] or bool(stage.findings),
        not requirements["uncertainty"] or bool(stage.uncertainty),
        not requirements["open_questions"] or bool(stage.open_questions),
    )
    return _fraction(len(checks), sum(checks))


def evaluate_autonomous_domain_response_quality(
    response: Any,
    contract: Any,
    supplied_policy: AutonomousDomainQualityPolicy | Mapping[str, Any] | None = None,
) -> AutonomousDomainQualityReport:
    """Return bounded domain-specific structural quality signals without retaining the response."""

    if getattr(response, "domain", None) != getattr(contract, "domain", None):
        raise ArgumentError("domain quality evaluation identity is malformed")
    policy = validate_autonomous_domain_quality_policy(supplied_policy) if supplied_policy is not None else autonomous_domain_quality_policy(response.domain)
    statuses = tuple(stage.status for stage in response.stages)
    all_complete = bool(statuses) and all(status == "complete" for status in statuses)
    incomplete = any(status != "complete" for status in statuses)
    has_blocked = "blocked" in statuses
    disclosures = bool(response.uncertainty or response.evidence_gaps or response.next_actions)
    status_coherent = (
        all_complete if response.status == "complete" else
        incomplete and disclosures if response.status == "partial" else
        has_blocked and bool(response.next_actions) if response.status == "blocked" else
        disclosures and incomplete
    )
    stage_scores = tuple(_stage_quality(stage, policy) for stage in response.stages)
    critical_scores = tuple(bool(response.domain_details.get(field)) for field in policy.critical_detail_fields)
    safety_scores = tuple(bool(response.domain_details.get(field)) for field in policy.safety_detail_fields)
    signals = {
        "quality_status_coherence": float(status_coherent),
        "quality_stage_contract_coverage": _fraction(len(stage_scores), sum(stage_scores)),
        "quality_critical_detail_coverage": _fraction(len(critical_scores), sum(critical_scores)),
        "quality_safety_control_coverage": _fraction(len(safety_scores), sum(safety_scores)),
        "quality_reasoning_trace": float(bool(response.observations and response.inferences)),
        "quality_actionability": float(bool(response.next_actions)),
        "quality_evidence_boundary": float(bool(response.uncertainty and response.evidence_gaps)),
    }
    weights = {
        "quality_status_coherence": 2.5,
        "quality_stage_contract_coverage": 2.5,
        "quality_critical_detail_coverage": 2.0,
        "quality_safety_control_coverage": 2.0,
        "quality_reasoning_trace": 1.5,
        "quality_actionability": 1.5,
        "quality_evidence_boundary": 1.5,
    }
    total_weight = sum(weights.values())
    score = round(sum(signals[name] * weight for name, weight in weights.items()) / total_weight, 12)
    missing = tuple(name for name, score_value in signals.items() if score_value < 1.0)
    advice = {
        "quality_status_coherence": "align the top-level status with every stage status and disclose why incomplete work remains",
        "quality_stage_contract_coverage": "complete each stage's evidence/findings or explicitly record uncertainty and open questions",
        "quality_critical_detail_coverage": f"populate every critical {response.domain} decision field: {', '.join(policy.critical_detail_fields)}",
        "quality_safety_control_coverage": f"address {response.domain} safety controls: {', '.join(policy.safety_detail_fields)}",
        "quality_reasoning_trace": "separate observed facts from bounded inferences",
        "quality_actionability": "provide caller-reviewable next actions",
        "quality_evidence_boundary": "state evidence gaps and uncertainty explicitly",
    }
    recommendations = tuple(advice[name] for name in missing)
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA,
        "domain": response.domain,
        "policy_digest": policy.policy_digest,
        "signals": signals,
        "weights": weights,
        "missing_signals": list(missing),
        "recommendations": list(recommendations),
        "score": score,
        # A high aggregate score cannot hide one missing safety or stage-control signal.  The
        # quality report is a readiness gate, so every domain control must be satisfied as well.
        "passed": score >= AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD and not missing,
        "authority": "structural_domain_quality_only;not_external_truth",
        "retention": "value_only;response_payload_not_retained",
        "secret_material": "never_returned",
    }
    return AutonomousDomainQualityReport(
        schema=descriptor["schema"],
        domain=descriptor["domain"],
        policy_digest=descriptor["policy_digest"],
        signals=signals,
        weights=weights,
        missing_signals=missing,
        recommendations=recommendations,
        score=score,
        passed=bool(descriptor["passed"]),
        authority=descriptor["authority"],
        retention=descriptor["retention"],
        secret_material=descriptor["secret_material"],
        report_digest=content_digest(descriptor),
    )


def autonomous_domain_quality_prompt(policy: AutonomousDomainQualityPolicy) -> str:
    normalized = validate_autonomous_domain_quality_policy(policy)
    return " ".join(
        (
            f"Apply quality policy {normalized.policy_digest} for {normalized.domain}.",
            *normalized.prompt_instructions,
            f"Required top-level sections: {', '.join(normalized.required_top_level_sections)}.",
            "A quality pass is a structural readiness signal only; it is not external validation or permission to create an effect.",
        )
    )


def assert_autonomous_domain_quality_policy_coverage() -> bool:
    policies = builtin_autonomous_domain_quality_policies()
    if len(policies) != len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("domain quality policy registry is incomplete")
    for policy in policies:
        fields = set(policy.required_detail_fields)
        if any(field not in fields for field in (*policy.critical_detail_fields, *policy.safety_detail_fields)):
            raise ArgumentError(f"domain quality policy {policy.domain} references an unknown detail field")
        if not 1 <= len(policy.prompt_instructions) <= MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTIONS:
            raise ArgumentError(f"domain quality policy {policy.domain} has invalid prompt guidance")
        for instruction in policy.prompt_instructions:
            _text("domain quality instruction", instruction, MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTION_BYTES)
    return True


__all__ = [
    "AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA",
    "AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION",
    "AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA",
    "AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD",
    "MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTIONS",
    "MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTION_BYTES",
    "AutonomousDomainQualityPolicy",
    "AutonomousDomainQualityReport",
    "autonomous_domain_quality_policy",
    "builtin_autonomous_domain_quality_policies",
    "validate_autonomous_domain_quality_policy",
    "evaluate_autonomous_domain_response_quality",
    "autonomous_domain_quality_prompt",
    "assert_autonomous_domain_quality_policy_coverage",
]
