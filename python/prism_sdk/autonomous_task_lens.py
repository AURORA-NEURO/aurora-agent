"""Domain-specific planning lenses for the autonomous brain.

The lens is a reviewed, provider-free contract that turns a generic domain route into a
useful planning posture.  It is deliberately metadata-only: it contains no task text,
provider output, credentials, or authority to invoke tools.  The same contract is mirrored
by the TypeScript SDK so prompt, selection, replay, and audit projections can agree across
language boundaries.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import content_digest
from .autonomous_domain_policy import AUTONOMOUS_DOMAIN_POLICY_DOMAINS
from .errors import ArgumentError


AUTONOMOUS_TASK_LENS_SCHEMA = "bioprism-autonomous-domain-task-lens/0.1"
AUTONOMOUS_TASK_LENS_VERSION = "0.1"
AUTONOMOUS_TASK_LENS_DOMAINS = AUTONOMOUS_DOMAIN_POLICY_DOMAINS
MAX_AUTONOMOUS_TASK_LENS_ITEMS = 8
MAX_AUTONOMOUS_TASK_LENS_TEXT_BYTES = 512


def _bounded_text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_AUTONOMOUS_TASK_LENS_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds its bound")
    return value


def _bounded_items(name: str, values: Any) -> tuple[str, ...]:
    if not isinstance(values, (tuple, list)) or not values or len(values) > MAX_AUTONOMOUS_TASK_LENS_ITEMS:
        raise ArgumentError(f"{name} must contain between 1 and {MAX_AUTONOMOUS_TASK_LENS_ITEMS} items")
    result = tuple(_bounded_text(f"{name} item", value) for value in values)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicates")
    return result


@dataclass(frozen=True, slots=True)
class AutonomousDomainTaskLens:
    """A bounded domain strategy used by planning and model-selection projections."""

    domain: str
    lens_id: str
    objective: str
    planning_dimensions: tuple[str, ...]
    decision_checks: tuple[str, ...]
    evidence_priorities: tuple[str, ...]
    evaluator_signals: tuple[str, ...]
    model_capability_hints: tuple[str, ...]
    output_sections: tuple[str, ...]
    failure_modes: tuple[str, ...]
    lens_version: str = AUTONOMOUS_TASK_LENS_VERSION

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_TASK_LENS_DOMAINS:
            raise ArgumentError(f"unsupported autonomous task-lens domain: {self.domain}")
        _bounded_text("task lens lens_id", self.lens_id)
        _bounded_text("task lens objective", self.objective)
        if self.lens_version != AUTONOMOUS_TASK_LENS_VERSION:
            raise ArgumentError("unsupported autonomous task-lens version")
        for name, values in (
            ("planning_dimensions", self.planning_dimensions),
            ("decision_checks", self.decision_checks),
            ("evidence_priorities", self.evidence_priorities),
            ("evaluator_signals", self.evaluator_signals),
            ("model_capability_hints", self.model_capability_hints),
            ("output_sections", self.output_sections),
            ("failure_modes", self.failure_modes),
        ):
            object.__setattr__(self, name, _bounded_items(f"task lens {name}", values))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_LENS_SCHEMA,
            "domain": self.domain,
            "lens_id": self.lens_id,
            "lens_version": self.lens_version,
            "objective": self.objective,
            "planning_dimensions": list(self.planning_dimensions),
            "decision_checks": list(self.decision_checks),
            "evidence_priorities": list(self.evidence_priorities),
            "evaluator_signals": list(self.evaluator_signals),
            "model_capability_hints": list(self.model_capability_hints),
            "output_sections": list(self.output_sections),
            "failure_modes": list(self.failure_modes),
            "retention": "value_only_strategy_metadata",
            "secret_material": "never_returned",
        }

    @property
    def lens_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "lens_digest": self.lens_digest}

    def prompt_contract(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_LENS_SCHEMA,
            "domain": self.domain,
            "lens_id": self.lens_id,
            "lens_digest": self.lens_digest,
            "objective": self.objective,
            "planning_dimensions": list(self.planning_dimensions),
            "decision_checks": list(self.decision_checks),
            "evidence_priorities": list(self.evidence_priorities),
            "evaluator_signals": list(self.evaluator_signals),
            "model_capability_hints": list(self.model_capability_hints),
            "output_sections": list(self.output_sections),
            "failure_modes": list(self.failure_modes),
            "model_hints_are": "preferences_only; they do not authorize or hard-gate a model",
            "execution": "guidance_only; provider_and_effect_boundaries_remain_separate",
            "secret_material": "never_returned",
        }


_LENS_SEEDS: Mapping[str, Mapping[str, Any]] = {
    "coding": {"lens_id": "coding-change-verification", "objective": "make the smallest verifiable change that satisfies the requested behavior", "planning_dimensions": ("scope", "dependency_impact", "implementation", "verification"), "decision_checks": ("reproduce_or_localize_before_changing", "minimize_change_surface", "preserve_existing_contracts", "run_relevant_checks"), "evidence_priorities": ("repository_state", "diff_impact", "test_or_ci_output"), "evaluator_signals": ("correctness", "regression_safety", "scope_discipline"), "model_capability_hints": ("code", "reasoning", "tool_use"), "output_sections": ("diagnosis", "change_plan", "implementation", "verification", "limitations"), "failure_modes": ("unreproduced_failure", "unverified_fix", "scope_creep")},
    "browser": {"lens_id": "browser-source-grounding", "objective": "acquire and compare bounded sources while preserving provenance and freshness", "planning_dimensions": ("source_discovery", "navigation", "provenance", "comparison"), "decision_checks": ("identify_source_scope", "separate_observation_from_inference", "record_freshness", "surface_conflict"), "evidence_priorities": ("source_identity", "retrieval_metadata", "cross_source_agreement"), "evaluator_signals": ("source_relevance", "provenance_completeness", "conflict_visibility"), "model_capability_hints": ("web_research", "reasoning", "structured_output"), "output_sections": ("sources", "observations", "comparison", "uncertainty", "next_queries"), "failure_modes": ("unsupported_source", "stale_source", "citation_overreach")},
    "data": {"lens_id": "data-lineage-quality", "objective": "turn bounded data into a traceable analysis without hiding schema or quality defects", "planning_dimensions": ("schema", "lineage", "quality", "analysis"), "decision_checks": ("validate_schema_before_analysis", "preserve_missingness", "track_transformations", "quantify_quality_limits"), "evidence_priorities": ("schema_metadata", "lineage", "quality_checks"), "evaluator_signals": ("schema_fidelity", "lineage_completeness", "analysis_reproducibility"), "model_capability_hints": ("data_analysis", "reasoning", "structured_output"), "output_sections": ("data_contract", "quality_findings", "analysis", "assumptions", "reproduction_steps"), "failure_modes": ("missingness_as_zero", "lineage_break", "unsupported_aggregation")},
    "science": {"lens_id": "science-epistemic-reproducibility", "objective": "separate observations, hypotheses, causal claims, and reproducible tests", "planning_dimensions": ("question", "evidence", "hypothesis", "test", "reproduction"), "decision_checks": ("identify_estimand_or_question", "distinguish_correlation_from_causality", "state_alternatives", "define_reproduction"), "evidence_priorities": ("primary_measurement", "method_details", "independent_reproduction"), "evaluator_signals": ("epistemic_calibration", "method_fidelity", "reproducibility"), "model_capability_hints": ("scientific_reasoning", "statistics", "structured_output"), "output_sections": ("observations", "hypotheses", "analysis", "confounders", "reproduction_plan"), "failure_modes": ("causal_overclaim", "hypothesis_as_fact", "simulation_as_measurement")},
    "biomedical": {"lens_id": "biomedical-grounding-safety", "objective": "organize biomedical evidence with explicit provenance, uncertainty, and human-review boundaries", "planning_dimensions": ("grounding", "estimand", "provenance", "safety", "review"), "decision_checks": ("identify_population_and_endpoint", "check_reference_provenance", "state_translation_limits", "require_qualified_review"), "evidence_priorities": ("primary_reference", "population_definition", "safety_and_bias"), "evaluator_signals": ("grounding", "estimand_fidelity", "safety_boundary"), "model_capability_hints": ("biomedical_reasoning", "literature", "structured_output"), "output_sections": ("evidence", "interpretation", "limitations", "safety_boundary", "human_review"), "failure_modes": ("diagnostic_overreach", "prescription_overreach", "population_mismatch")},
    "neuroscience": {"lens_id": "neuroscience-modality-measurement", "objective": "interpret neuroscience measurements while preserving modality, transport, and study-design limits", "planning_dimensions": ("modality", "measurement", "transport", "study_design", "reproduction"), "decision_checks": ("inventory_modalities", "separate_signal_from_measurement", "check_pseudoreplication", "state_transport_limits"), "evidence_priorities": ("modality_metadata", "measurement_comparability", "independent_trace"), "evaluator_signals": ("modality_fidelity", "measurement_validity", "transport_calibration"), "model_capability_hints": ("neuroscience", "signal_analysis", "structured_output"), "output_sections": ("modalities", "measurements", "interpretation", "confounds", "reproduction"), "failure_modes": ("modality_erasure", "pseudoreplication", "transport_overclaim")},
    "operations": {"lens_id": "operations-observe-control-recover", "objective": "move from observable state to reversible, authorized operational action", "planning_dimensions": ("observability", "incident", "risk", "approval", "rollback"), "decision_checks": ("confirm_current_state", "bound_effects", "require_authorization", "define_rollback", "verify_postcondition"), "evidence_priorities": ("telemetry", "change_record", "postcondition"), "evaluator_signals": ("operational_safety", "rollback_readiness", "postcondition_fidelity"), "model_capability_hints": ("operations", "systems_reasoning", "structured_output"), "output_sections": ("observed_state", "runbook", "approval_gate", "rollback", "verification"), "failure_modes": ("unbounded_effect", "missing_rollback", "uncertain_postcondition")},
    "enterprise": {"lens_id": "enterprise-governance-stewardship", "objective": "coordinate enterprise work through governance, privacy, security, and accountable ownership", "planning_dimensions": ("workflow", "governance", "compliance", "security", "ownership"), "decision_checks": ("identify_owner", "check_policy_scope", "separate_access_from_authority", "record_exception", "define_audit_trail"), "evidence_priorities": ("policy_version", "approval_record", "audit_evidence"), "evaluator_signals": ("policy_fidelity", "accountability", "auditability"), "model_capability_hints": ("enterprise_reasoning", "governance", "structured_output"), "output_sections": ("scope", "policy_mapping", "risks", "owners", "audit_plan"), "failure_modes": ("implicit_authority", "policy_drift", "unowned_exception")},
    "multi_agent": {"lens_id": "multi-agent-bounded-coordination", "objective": "decompose work into bounded roles with explicit handoffs, disagreement, and one effect authority", "planning_dimensions": ("decomposition", "role_contracts", "handoffs", "consensus", "accountability"), "decision_checks": ("bound_each_subproblem", "preserve_lineage", "surface_disagreement", "avoid_duplicate_effects", "assign_final_authority"), "evidence_priorities": ("subtask_contract", "handoff_receipt", "consensus_or_dissent"), "evaluator_signals": ("coverage", "handoff_integrity", "disagreement_fidelity"), "model_capability_hints": ("coordination", "reasoning", "structured_output"), "output_sections": ("decomposition", "role_results", "dissent", "synthesis", "accountability"), "failure_modes": ("duplicate_effect", "authority_ambiguity", "false_consensus")},
    "multimodal": {"lens_id": "multimodal-modality-aware-fusion", "objective": "fuse available modalities without implying that missing or transformed inputs were inspected", "planning_dimensions": ("modality_inventory", "transport", "alignment", "fusion", "uncertainty"), "decision_checks": ("list_available_modalities", "record_transport_loss", "check_alignment", "separate_fusion_from_observation", "state_missing_modalities"), "evidence_priorities": ("modality_manifest", "transport_metadata", "cross_modal_consistency"), "evaluator_signals": ("modality_completeness", "transport_fidelity", "fusion_calibration"), "model_capability_hints": ("multimodal", "vision_or_audio", "structured_output"), "output_sections": ("modality_manifest", "observations", "fusion", "blind_spots", "next_acquisition"), "failure_modes": ("absent_modality_claim", "transport_loss_erasure", "cross_modal_confusion")},
    "cross_domain": {"lens_id": "cross-domain-evidence-harmonization", "objective": "combine specialist results while keeping claims, evidence, and evaluators attached to their source domains", "planning_dimensions": ("routing", "specialist_contracts", "evidence_harmonization", "conflict", "synthesis"), "decision_checks": ("assign_domain_owners", "preserve_specialist_limits", "compare_incompatible_claims", "surface_conflict", "bound_synthesis"), "evidence_priorities": ("child_contracts", "domain_receipts", "cross_domain_conflicts"), "evaluator_signals": ("domain_coverage", "lineage_preservation", "synthesis_fidelity"), "model_capability_hints": ("cross_domain_reasoning", "coordination", "structured_output"), "output_sections": ("domain_map", "specialist_findings", "conflicts", "synthesis", "unresolved_gaps"), "failure_modes": ("domain_blending", "unsupported_synthesis", "lost_dissent")},
    "evaluation": {"lens_id": "evaluation-independent-measurement", "objective": "measure system behavior with independent evidence, holdouts, and explicit contamination controls", "planning_dimensions": ("rubric", "independence", "holdout", "metrics", "replay"), "decision_checks": ("freeze_rubric", "separate_system_from_evaluator", "protect_holdout", "track_missing_cases", "replay_exactly"), "evidence_priorities": ("evaluation_protocol", "holdout_coverage", "replay_trace"), "evaluator_signals": ("independence", "calibration", "reproducibility"), "model_capability_hints": ("evaluation", "statistics", "structured_output"), "output_sections": ("protocol", "metrics", "coverage", "failures", "replay"), "failure_modes": ("self_authored_pass", "holdout_contamination", "metric_drift")},
}


_BUILTIN_LENSES = {
    domain: AutonomousDomainTaskLens(domain=domain, **dict(seed))
    for domain, seed in _LENS_SEEDS.items()
}


def builtin_autonomous_domain_task_lenses() -> tuple[AutonomousDomainTaskLens, ...]:
    return tuple(_BUILTIN_LENSES[domain] for domain in AUTONOMOUS_TASK_LENS_DOMAINS)


def autonomous_domain_task_lens(domain: str) -> AutonomousDomainTaskLens:
    if domain not in _BUILTIN_LENSES:
        raise ArgumentError(f"unsupported autonomous task-lens domain: {domain}")
    return _BUILTIN_LENSES[domain]


__all__ = [
    "AUTONOMOUS_TASK_LENS_SCHEMA",
    "AUTONOMOUS_TASK_LENS_VERSION",
    "AUTONOMOUS_TASK_LENS_DOMAINS",
    "MAX_AUTONOMOUS_TASK_LENS_ITEMS",
    "AutonomousDomainTaskLens",
    "builtin_autonomous_domain_task_lenses",
    "autonomous_domain_task_lens",
]
