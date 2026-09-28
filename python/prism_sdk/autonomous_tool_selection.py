"""Reviewed capability aliases and value-only adaptive tool selection.

This module owns the deterministic candidate policy shared with the TypeScript SDK. It ranks
registered tool contracts only; selection never authorizes a tool call or effect.
"""

from __future__ import annotations

import math
from collections.abc import Mapping, Sequence
from typing import Any, Protocol

from .autonomous_text_normalization import normalize_route_text
from .brain import BrainRunError
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES as AUTONOMOUS_DOMAINS
from .domain_tools import AutonomousDomainToolBinding

_SAFE_IDENTIFIER_CHARS = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-")


class _WorkflowStage(Protocol):
    """Minimal workflow fields needed by reviewed capability selection."""

    id: str
    objective: str
    required_capabilities: Sequence[str]
    read_only: bool
    approval_required: bool


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > 512:
        raise BrainRunError(f"{name} exceeds its bounded size")
    if len(value) > 128 or any(character not in _SAFE_IDENTIFIER_CHARS for character in value):
        raise BrainRunError(f"{name} must be a bounded identifier")
    return value


AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA = "bioprism-autonomous-tool-selection-state/0.1"
AUTONOMOUS_TOOL_SELECTION_POLICY = "stage_coverage_then_capability_then_ucb_value_then_task_relevance_then_read_only_then_name"
AUTONOMOUS_TOOL_RISK_ORDER = ("read_only", "reversible_effect", "external_effect", "high_impact_effect")
MAX_AUTONOMOUS_TOOL_SELECTION_ARMS = 512
MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS = 4096
MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE = 2

# A domain workflow uses a small, stable capability vocabulary while live tools often expose
# a narrower adapter name.  These aliases are reviewed policy, not fuzzy matching: a tool is
# bridged to a workflow stage only when its exact declared capability appears in this table.
# An exact high-level capability always matches too, which preserves application-defined tools
# such as an ``observability`` binding while making the curated built-ins useful immediately.
_AUTONOMOUS_CAPABILITY_TOOL_ALIASES: dict[str, dict[str, tuple[str, ...]]] = {
    "coding": {
        "review": ("engineering_contract_audit", "delivery_audit", "delivery_receipt_verification", "conformance_verification", "stewardship_review"),
        "debugging": ("repository_inspection", "repository_impact_analysis", "ci_evidence_audit", "ci_evidence_normalization", "sdk_registry_audit"),
        "implementation": ("developer_workbench", "developer_workbench_verification", "engineering_planning", "engineering_execution_plan", "mission_execution"),
        "testing": ("ci_execution_audit", "ci_evidence_audit", "conformance_verification", "release_readiness", "delivery_receipt_verification"),
    },
    "browser": {
        "web_research": ("evidence_acquisition_discovery", "evidence_source_planning", "evidence_coverage", "hub_discovery", "lens_discovery"),
        "navigation": ("capability_discovery", "capability_routing", "hub_resolution", "route_planning", "workspace_capability_discovery"),
        "source_comparison": ("evidence_coverage", "route_plan_verification", "route_review", "evidence_source_planning"),
    },
    "data": {
        "data_analysis": ("context_compilation", "context_comparison", "context_refinement", "projection_bundling", "tabular_ingestion", "world_validation"),
        "schema_validation": ("world_claim_validation", "context_verification", "obligation_gate", "data_adapter_planning"),
        "lineage": ("lineage_audit", "context_explanation", "context_verification", "evidence_coverage"),
        "quality_control": ("quality_control", "world_validation", "world_claim_validation", "evidence_coverage", "obligation_gate"),
    },
    "science": {
        "literature": ("literature_binding", "contradiction_review", "research_routing", "research_routing_replay", "epistemic_context_audit"),
        "hypothesis": ("epistemic_selection_audit", "decision_quotient", "value_of_information", "influence_analysis"),
        "experiment": ("laboratory_planning", "adaptive_acquisition_execution", "measurement_comparison", "value_of_information"),
        "statistics": ("decision_quotient", "influence_analysis", "measurement_comparison", "laboratory_pareto_audit"),
        "reproducibility": ("reproduction_check", "research_routing_replay", "laboratory_holdout_audit", "laboratory_branch_audit", "laboratory_evolution_audit"),
    },
    "biomedical": {
        "neurosurgical_specialty_discovery": ("neurosurgical_specialty_discovery", "neurosurgery_catalogue"),
        "neurosurgical_intake_routing": ("neurosurgical_intake_routing", "neurosurgery_intake_plan", "neurosurgery_intake_mission", "neurosurgery_intake_portfolio"),
        "neurosurgical_evidence_audit": ("neurosurgical_evidence_audit", "neurosurgery_evidence_audit"),
        "neurosurgical_specialty_evidence_map": ("neurosurgical_specialty_evidence_map", "neurosurgery_specialty_evidence_map"),
        "neurosurgical_case_asset_manifest": ("neurosurgical_case_asset_manifest", "neurosurgery_case_asset_manifest"),
        "neurosurgical_case_fhir_import": ("neurosurgical_case_fhir_import", "neurosurgery_case_fhir_import"),
        "neurosurgical_case_dicom_import": ("neurosurgical_case_dicom_import", "neurosurgery_case_dicom_import"),
        "neurosurgical_case_dicom_evidence_workflow": ("neurosurgical_case_dicom_evidence_workflow", "neurosurgery_case_dicom_evidence_workflow"),
        "neurosurgical_case_asset_review_disposition": ("neurosurgical_case_asset_review_disposition", "neurosurgery_case_asset_review_disposition"),
        "neurosurgical_evidence_synthesis": ("neurosurgical_evidence_synthesis", "neurosurgery_evidence_synthesis"),
        "neurosurgical_glioma_molecular_map": ("neurosurgical_glioma_molecular_map", "neurosurgery_glioma_molecular_map"),
        "neurosurgical_molecular_coverage": ("neurosurgical_molecular_coverage", "neurosurgery_real_data_molecular_coverage"),
        "neurosurgical_evidence_graph": ("neurosurgical_evidence_graph", "neurosurgery_evidence_graph"),
        "neurosurgical_real_data_coverage": ("neurosurgical_real_data_coverage", "neurosurgery_real_data_coverage"),
        "neurosurgical_real_data_reconciliation": ("neurosurgical_real_data_reconciliation", "neurosurgery_real_data_reconciliation"),
        "neurosurgical_real_data_freshness": ("neurosurgical_real_data_freshness", "neurosurgery_real_data_freshness"),
        "neurosurgical_real_data_diff": ("neurosurgical_real_data_diff", "neurosurgery_real_data_diff"),
        "neurosurgical_real_data_refresh_audit": ("neurosurgical_real_data_refresh_audit", "neurosurgery_real_data_refresh_audit"),
        "neurosurgical_real_data_review_queue": ("neurosurgical_real_data_review_queue", "neurosurgery_real_data_review_queue"),
        "neurosurgical_real_data_review_disposition": ("neurosurgical_real_data_review_disposition", "neurosurgery_real_data_review_disposition"),
        "neurosurgical_real_data_evidence_packet": ("neurosurgical_real_data_evidence_packet", "neurosurgery_real_data_evidence_packet"),
        "neurosurgical_real_data_autonomous_workflow": ("neurosurgical_real_data_autonomous_workflow", "neurosurgery_real_data_autonomous_workflow"),
        "neurosurgical_real_data_reasoning_context": ("neurosurgical_real_data_reasoning_context", "neurosurgery_real_data_reasoning_context"),
        "neurosurgical_real_data_draft_audit": ("neurosurgical_real_data_draft_audit", "neurosurgery_real_data_draft_audit"),
        "neurosurgical_public_literature_evidence_packet": ("neurosurgical_public_literature_evidence_packet", "neurosurgery_public_literature_evidence_packet"),
        "neurosurgical_public_literature_reasoning_context": ("neurosurgical_public_literature_reasoning_context", "neurosurgery_public_literature_reasoning_context"),
        "neurosurgical_public_literature_draft_audit": ("neurosurgical_public_literature_draft_audit", "neurosurgery_public_literature_draft_audit"),
        "neurosurgical_public_literature_matrix": ("neurosurgical_public_literature_matrix", "neurosurgery_public_literature_matrix"),
        "neurosurgical_public_literature_freshness": ("neurosurgical_public_literature_freshness", "neurosurgery_public_literature_freshness"),
        "neurosurgical_public_literature_refresh_audit": ("neurosurgical_public_literature_refresh_audit", "neurosurgery_public_literature_refresh_audit"),
        "neurosurgical_literature_link_audit": ("neurosurgical_literature_link_audit", "neurosurgery_literature_link_audit"),
        "neurosurgical_public_literature_integrity_audit": ("neurosurgical_public_literature_integrity_audit", "neurosurgery_public_literature_integrity_audit"),
        "neurosurgical_public_literature_review_queue": ("neurosurgical_public_literature_review_queue", "neurosurgery_public_literature_review_queue"),
        "neurosurgical_public_literature_workbench": ("neurosurgical_public_literature_workbench", "neurosurgery_public_literature_workbench"),
        "neurosurgical_public_literature_portfolio": ("neurosurgical_public_literature_portfolio", "neurosurgery_public_literature_portfolio"),
        "neurosurgical_research_brief": ("neurosurgical_research_brief", "neurosurgery_research_brief"),
        "neurosurgical_research_plan": ("neurosurgical_research_plan", "neurosurgery_research_plan"),
        "neurosurgical_evidence_acquisition": ("neurosurgical_evidence_acquisition", "neurosurgery_evidence_acquisition"),
        "neurosurgical_evidence_program": ("neurosurgical_evidence_program", "neurosurgery_evidence_program"),
        "neurosurgical_research_route": ("neurosurgical_research_route", "neurosurgery_plan"),
        "neurosurgical_public_data_query": ("neurosurgical_public_data_query", "neurosurgery_real_data_query"),
        "neurosurgical_public_literature_query": ("neurosurgical_public_literature_query", "neurosurgery_public_literature_query"),
        "neurosurgical_trial_landscape": ("neurosurgical_trial_landscape", "neurosurgery_real_data_trial_landscape"),
        "neurosurgical_resumable_session": ("neurosurgical_resumable_session", "neurosurgery_session"),
        "neurosurgical_research_mission": ("neurosurgical_research_mission", "neurosurgery_mission"),
        "biomedical_review": ("biomedical_grounding_audit", "biomedical_reference_audit", "biomedical_estimand_audit", "literature_binding", "contradiction_review"),
        "provenance": ("biomedical_reference_audit", "literature_binding", "measurement_comparison", "world_validation", "representation_audit"),
        "safety_boundary": ("medical_boundary", "dual_use_review", "bioethics_validation", "bioethics_action_review", "oncology_boundary"),
        "human_review": ("human_subject_screening", "bioethics_action_review", "bioethics_validation", "medical_boundary"),
    },
    "neuroscience": {
        "neurosurgical_specialty_discovery": ("neurosurgical_specialty_discovery", "neurosurgery_catalogue"),
        "neurosurgical_intake_routing": ("neurosurgical_intake_routing", "neurosurgery_intake_plan", "neurosurgery_intake_mission", "neurosurgery_intake_portfolio"),
        "neurosurgical_evidence_audit": ("neurosurgical_evidence_audit", "neurosurgery_evidence_audit"),
        "neurosurgical_specialty_evidence_map": ("neurosurgical_specialty_evidence_map", "neurosurgery_specialty_evidence_map"),
        "neurosurgical_case_asset_manifest": ("neurosurgical_case_asset_manifest", "neurosurgery_case_asset_manifest"),
        "neurosurgical_case_fhir_import": ("neurosurgical_case_fhir_import", "neurosurgery_case_fhir_import"),
        "neurosurgical_case_dicom_import": ("neurosurgical_case_dicom_import", "neurosurgery_case_dicom_import"),
        "neurosurgical_case_dicom_evidence_workflow": ("neurosurgical_case_dicom_evidence_workflow", "neurosurgery_case_dicom_evidence_workflow"),
        "neurosurgical_case_asset_review_disposition": ("neurosurgical_case_asset_review_disposition", "neurosurgery_case_asset_review_disposition"),
        "neurosurgical_evidence_synthesis": ("neurosurgical_evidence_synthesis", "neurosurgery_evidence_synthesis"),
        "neurosurgical_glioma_molecular_map": ("neurosurgical_glioma_molecular_map", "neurosurgery_glioma_molecular_map"),
        "neurosurgical_molecular_coverage": ("neurosurgical_molecular_coverage", "neurosurgery_real_data_molecular_coverage"),
        "neurosurgical_evidence_graph": ("neurosurgical_evidence_graph", "neurosurgery_evidence_graph"),
        "neurosurgical_real_data_coverage": ("neurosurgical_real_data_coverage", "neurosurgery_real_data_coverage"),
        "neurosurgical_real_data_reconciliation": ("neurosurgical_real_data_reconciliation", "neurosurgery_real_data_reconciliation"),
        "neurosurgical_real_data_freshness": ("neurosurgical_real_data_freshness", "neurosurgery_real_data_freshness"),
        "neurosurgical_real_data_diff": ("neurosurgical_real_data_diff", "neurosurgery_real_data_diff"),
        "neurosurgical_real_data_refresh_audit": ("neurosurgical_real_data_refresh_audit", "neurosurgery_real_data_refresh_audit"),
        "neurosurgical_real_data_review_queue": ("neurosurgical_real_data_review_queue", "neurosurgery_real_data_review_queue"),
        "neurosurgical_real_data_review_disposition": ("neurosurgical_real_data_review_disposition", "neurosurgery_real_data_review_disposition"),
        "neurosurgical_real_data_evidence_packet": ("neurosurgical_real_data_evidence_packet", "neurosurgery_real_data_evidence_packet"),
        "neurosurgical_real_data_autonomous_workflow": ("neurosurgical_real_data_autonomous_workflow", "neurosurgery_real_data_autonomous_workflow"),
        "neurosurgical_real_data_reasoning_context": ("neurosurgical_real_data_reasoning_context", "neurosurgery_real_data_reasoning_context"),
        "neurosurgical_real_data_draft_audit": ("neurosurgical_real_data_draft_audit", "neurosurgery_real_data_draft_audit"),
        "neurosurgical_public_literature_evidence_packet": ("neurosurgical_public_literature_evidence_packet", "neurosurgery_public_literature_evidence_packet"),
        "neurosurgical_public_literature_reasoning_context": ("neurosurgical_public_literature_reasoning_context", "neurosurgery_public_literature_reasoning_context"),
        "neurosurgical_public_literature_draft_audit": ("neurosurgical_public_literature_draft_audit", "neurosurgery_public_literature_draft_audit"),
        "neurosurgical_public_literature_matrix": ("neurosurgical_public_literature_matrix", "neurosurgery_public_literature_matrix"),
        "neurosurgical_public_literature_freshness": ("neurosurgical_public_literature_freshness", "neurosurgery_public_literature_freshness"),
        "neurosurgical_public_literature_refresh_audit": ("neurosurgical_public_literature_refresh_audit", "neurosurgery_public_literature_refresh_audit"),
        "neurosurgical_literature_link_audit": ("neurosurgical_literature_link_audit", "neurosurgery_literature_link_audit"),
        "neurosurgical_public_literature_integrity_audit": ("neurosurgical_public_literature_integrity_audit", "neurosurgery_public_literature_integrity_audit"),
        "neurosurgical_public_literature_review_queue": ("neurosurgical_public_literature_review_queue", "neurosurgery_public_literature_review_queue"),
        "neurosurgical_public_literature_workbench": ("neurosurgical_public_literature_workbench", "neurosurgery_public_literature_workbench"),
        "neurosurgical_public_literature_portfolio": ("neurosurgical_public_literature_portfolio", "neurosurgery_public_literature_portfolio"),
        "neurosurgical_research_brief": ("neurosurgical_research_brief", "neurosurgery_research_brief"),
        "neurosurgical_research_plan": ("neurosurgical_research_plan", "neurosurgery_research_plan"),
        "neurosurgical_evidence_acquisition": ("neurosurgical_evidence_acquisition", "neurosurgery_evidence_acquisition"),
        "neurosurgical_evidence_program": ("neurosurgical_evidence_program", "neurosurgery_evidence_program"),
        "neurosurgical_research_route": ("neurosurgical_research_route", "neurosurgery_plan"),
        "neurosurgical_public_data_query": ("neurosurgical_public_data_query", "neurosurgery_real_data_query"),
        "neurosurgical_public_literature_query": ("neurosurgical_public_literature_query", "neurosurgery_public_literature_query"),
        "neurosurgical_trial_landscape": ("neurosurgical_trial_landscape", "neurosurgery_real_data_trial_landscape"),
        "neurosurgical_resumable_session": ("neurosurgical_resumable_session", "neurosurgery_session"),
        "neurosurgical_research_mission": ("neurosurgical_research_mission", "neurosurgery_mission"),
        "neuroscience_analysis": ("measurement_comparison", "influence_analysis", "trajectory_trace_analysis", "modality_catalogue"),
        "signal_interpretation": ("modality_support", "modality_transport", "modality_comparability", "measurement_comparison"),
        "study_design": ("value_of_information", "laboratory_holdout_audit", "measurement_comparison"),
        "reproducibility": ("benchmark_trace_analysis", "laboratory_holdout_audit", "trajectory_evaluation", "trajectory_trace_analysis"),
    },
    "operations": {
        "observability": ("telemetry_projection", "operations_catalogue", "ledger_ingestion", "runtime_tape_verification"),
        "incident_response": ("runtime_effect_check", "operations_acceptance", "quality_gate", "artifact_registry_audit"),
        "risk_review": ("operational_readiness", "registry_gate", "factory_authority_verification", "release_audit"),
        "rollback": ("storage_lifecycle_simulation", "cache_invalidation_simulation", "registry_lifecycle_simulation", "factory_lifecycle_simulation"),
        "approval": ("factory_authority_verification", "registry_gate", "operations_acceptance", "quality_gate"),
        "runbook": ("operational_readiness", "operations_catalogue", "release_audit", "runtime_tape_verification"),
    },
    "enterprise": {
        "workflow": ("governance_schema", "sandbox_runtime_simulation", "sandbox_admission", "provider_capability_verification"),
        "governance": ("governance_schema", "stewardship_review", "security_program_audit", "security_privacy_audit", "hub_disclosure_review"),
        "compliance": ("policy_screening", "release_audit", "safety_release_gate", "security_privacy_audit", "dual_use_review"),
        "analytics": ("provider_capability_verification", "security_redteam_simulation", "safety_posture", "release_audit"),
        "coordination": ("hub_submission_review", "hub_lock", "stewardship_review", "medical_boundary"),
    },
    "multi_agent": {
        "delegation": ("protocol_compilation", "workflow_execution", "mission_execution", "mission_evidence_import"),
        "coordination": ("protocol_catalogue", "workflow_catalogue", "choreography_validation", "multi_agent_synthesis"),
        "consensus": ("mission_evaluator_review", "mission_evaluator_replay_comparison", "mission_evidence_verification", "multi_agent_synthesis"),
        "conflict_resolution": ("mission_evaluator_replay", "mission_evaluator_replay_comparison", "mission_evidence_lookup", "choreography_validation"),
        "handoff": ("mission_evidence_import", "mission_evidence_query", "mission_evidence_verification", "workflow_execution"),
    },
    "multimodal": {
        "image": ("modality_catalogue", "modality_support", "modality_comparability", "projection_bundling", "hub_card_rendering"),
        "audio": ("modality_catalogue", "modality_support", "modality_transport", "measurement_comparison"),
        "video": ("modality_catalogue", "modality_support", "modality_transport", "measurement_comparison"),
        "document": ("literature_binding", "context_comparison", "projection_bundling", "hub_card_rendering"),
        "cross_modal_alignment": ("modality_comparability", "modality_transport", "modality_support", "measurement_comparison", "context_comparison"),
    },
    "cross_domain": {
        "routing": ("capability_discovery", "capability_routing", "route_planning", "route_review", "workspace_capability_discovery"),
        "synthesis": ("evidence_intake", "evidence_source_execution", "provider_normalization", "workflow_portfolio"),
        "evidence_alignment": ("evidence_coverage", "evidence_source_planning", "route_plan_verification", "workflow_portfolio_verification"),
        "workflow_composition": ("workflow_catalogue", "workflow_instantiation", "workflow_scaffolding", "workflow_verification"),
    },
    "evaluation": {
        "benchmarking": ("benchmark_compilation", "benchmark_compilation_review", "benchmark_counterfactual", "benchmark_integrity_audit", "benchmark_oracle_review"),
        "rubric": ("metrics_profile_audit", "metrics_analytics_audit", "evaluation_minimization", "posterior_gate"),
        "replay": ("research_ci", "reproduction_check", "trajectory_evaluation", "benchmark_trace_analysis", "worldline_evaluation"),
        "failure_analysis": ("benchmark_decision_audit", "benchmark_trace_analysis", "oracle_missingness", "oracle_combination"),
        "reproducibility": ("reproduction_check", "research_ci", "adaptive_evaluation_panel", "benchmark_integrity_audit"),
    },
}


def _portfolio_task_tokens(task: str) -> tuple[str, ...]:
    """Derive bounded local ranking tokens without retaining the task text."""

    return tuple(dict.fromkeys(
        token for token in normalize_route_text(task).split()
        if len(token) >= 3
    ))[:128]


def _portfolio_binding_supports_stage(
    domain: str,
    stage: "_WorkflowStage",
    binding: AutonomousDomainToolBinding,
) -> bool:
    aliases = _AUTONOMOUS_CAPABILITY_TOOL_ALIASES.get(domain, {})
    return any(
        binding.capability == capability
        or binding.capability in aliases.get(capability, ())
        for capability in stage.required_capabilities
    )


def _portfolio_score(
    tokens: Sequence[str],
    requested_capabilities: Sequence[str],
    stage: "_WorkflowStage",
    binding: AutonomousDomainToolBinding,
    domain: str,
    tool_selection_state: Mapping[str, Any],
    total_pulls: int,
    exploration: float,
) -> tuple[int, int, float, int, int]:
    corpus = normalize_route_text(
        f"{binding.name} {binding.capability} {stage.id} {stage.objective}"
    )
    relevance = sum(1 for token in tokens if token in corpus)
    return (
        int(binding.capability in requested_capabilities),
        int(binding.capability in stage.required_capabilities),
        _tool_selection_utility(_tool_selection_arm_for(tool_selection_state, domain, stage, binding), total_pulls, exploration),
        relevance,
        int(binding.read_only),
    )


def _portfolio_score_key(
    score: tuple[int, int, float, int, int],
    name: str,
) -> tuple[int, int, float, int, int, str]:
    return (-score[0], -score[1], -score[2], -score[3], -score[4], name)


def _tool_selection_number(name: str, value: Any, minimum: float, maximum: float, *, integer: bool = False) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or value < minimum or value > maximum or (integer and not isinstance(value, int)):
        raise BrainRunError(f"{name} is outside the tool-selection learning contract")
    return value


def _tool_selection_json_number(value: int | float | None) -> int | float | None:
    """Keep integral floats byte-compatible with JSON.stringify's number projection."""

    if value is None:
        return None
    return int(value) if float(value).is_integer() else value


def normalize_autonomous_tool_selection_state(value: Mapping[str, Any] | None) -> dict[str, Any]:
    """Normalize bounded value-only tool-arm state for deterministic portfolio planning."""

    if value is None:
        return {"schema": AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, "generation": 0, "arms": [], "credited_outcomes": []}
    if not isinstance(value, Mapping):
        raise BrainRunError("tool selection state must be a mapping")
    if set(value).difference({"schema", "generation", "arms", "credited_outcomes"}):
        raise BrainRunError("tool selection state contains unsupported fields")
    if value.get("schema") not in (None, AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA):
        raise BrainRunError("tool selection state schema is unsupported")
    generation = int(_tool_selection_number("tool selection generation", value.get("generation", 0), 0, 1_000_000_000, integer=True))
    raw_arms = value.get("arms", [])
    if not isinstance(raw_arms, Sequence) or isinstance(raw_arms, (str, bytes)) or len(raw_arms) > MAX_AUTONOMOUS_TOOL_SELECTION_ARMS:
        raise BrainRunError(f"tool selection state arms must contain at most {MAX_AUTONOMOUS_TOOL_SELECTION_ARMS} entries")
    seen: set[str] = set()
    arms: list[dict[str, Any]] = []
    allowed = {"arm_id", "pulls", "reward_sum", "failures", "latency_ms", "disabled"}
    for index, raw in enumerate(raw_arms):
        if not isinstance(raw, Mapping):
            raise BrainRunError(f"tool selection arm {index} is malformed")
        if set(raw).difference(allowed):
            raise BrainRunError("tool selection arm contains unsupported fields")
        arm_id = _identifier(f"tool selection arm {index} arm_id", raw.get("arm_id"))
        if arm_id in seen:
            raise BrainRunError(f"tool selection state contains duplicate arm {arm_id}")
        seen.add(arm_id)
        pulls = int(_tool_selection_number(f"tool selection arm {arm_id} pulls", raw.get("pulls", 0), 0, 1_000_000_000, integer=True))
        reward_sum = _tool_selection_json_number(float(_tool_selection_number(f"tool selection arm {arm_id} reward_sum", raw.get("reward_sum", 0), -pulls, pulls)))
        failures = int(_tool_selection_number(f"tool selection arm {arm_id} failures", raw.get("failures", 0), 0, pulls, integer=True))
        latency_raw = raw.get("latency_ms")
        latency_ms = None if latency_raw is None else _tool_selection_json_number(float(_tool_selection_number(f"tool selection arm {arm_id} latency_ms", latency_raw, 0, 3_600_000)))
        disabled = raw.get("disabled", False)
        if not isinstance(disabled, bool):
            raise BrainRunError(f"tool selection arm {arm_id} disabled must be boolean")
        arms.append({"arm_id": arm_id, "pulls": pulls, "reward_sum": reward_sum, "failures": failures, "latency_ms": latency_ms, "disabled": disabled})
    arms.sort(key=lambda arm: arm["arm_id"])
    raw_credits = value.get("credited_outcomes", [])
    if not isinstance(raw_credits, Sequence) or isinstance(raw_credits, (str, bytes)) or len(raw_credits) > MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS:
        raise BrainRunError(f"tool selection state credits must contain at most {MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS} entries")
    credit_ids: set[str] = set()
    credited_outcomes: list[dict[str, Any]] = []
    for index, raw in enumerate(raw_credits):
        if not isinstance(raw, Mapping):
            raise BrainRunError(f"tool selection credit {index} is malformed")
        if set(raw).difference({"outcome_digest", "arm_id", "reward", "failed", "latency_ms"}):
            raise BrainRunError("tool selection credit contains unsupported fields")
        outcome_digest = raw.get("outcome_digest")
        if not isinstance(outcome_digest, str) or len(outcome_digest) != 64 or any(character not in "0123456789abcdef" for character in outcome_digest):
            raise BrainRunError(f"tool selection credit {index} outcome_digest must be a lowercase SHA-256 digest")
        if outcome_digest in credit_ids:
            raise BrainRunError(f"tool selection state contains duplicate outcome {outcome_digest}")
        credit_ids.add(outcome_digest)
        arm_id = _identifier(f"tool selection credit {index} arm_id", raw.get("arm_id"))
        reward = float(_tool_selection_number(f"tool selection credit {index} reward", raw.get("reward"), -1, 1))
        failed = raw.get("failed")
        if not isinstance(failed, bool):
            raise BrainRunError(f"tool selection credit {index} failed must be boolean")
        latency_raw = raw.get("latency_ms")
        latency_ms = None if latency_raw is None else _tool_selection_json_number(float(_tool_selection_number(f"tool selection credit {index} latency_ms", latency_raw, 0, 3_600_000)))
        credited_outcomes.append({"outcome_digest": outcome_digest, "arm_id": arm_id, "reward": _tool_selection_json_number(reward), "failed": failed, "latency_ms": latency_ms})
    credited_outcomes.sort(key=lambda credit: credit["outcome_digest"])
    return {"schema": AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, "generation": generation, "arms": arms, "credited_outcomes": credited_outcomes}


def autonomous_tool_selection_arm_id(domain: str, capability: str, tool: str) -> str:
    if domain not in AUTONOMOUS_DOMAINS:
        raise BrainRunError("tool selection domain is unsupported")
    return ".".join((
        _identifier("tool selection domain", domain),
        _identifier("tool selection capability", capability),
        _identifier("tool selection tool", tool),
    ))


def _tool_selection_arm_for(state: Mapping[str, Any], domain: str, stage: "_WorkflowStage", binding: AutonomousDomainToolBinding) -> Mapping[str, Any] | None:
    arms = state.get("arms", [])
    ids = (
        autonomous_tool_selection_arm_id(domain, binding.capability, binding.name),
        autonomous_tool_selection_arm_id(domain, stage.required_capabilities[0] if stage.required_capabilities else binding.capability, binding.name),
    )
    return next((arm for arm in arms if arm.get("arm_id") in ids), next((arm for arm in arms if arm.get("arm_id") == binding.name), None))


def _tool_selection_utility(arm: Mapping[str, Any] | None, total_pulls: int, exploration: float) -> float:
    if arm is None:
        return 0.0
    pulls = int(arm["pulls"])
    mean_reward = 0.0 if pulls == 0 else float(arm["reward_sum"]) / pulls
    failure_rate = 0.0 if pulls == 0 else int(arm["failures"]) / pulls
    latency = arm.get("latency_ms")
    latency_penalty = 0.0 if latency is None else min(float(latency) / 10_000, 1.0) * 0.1
    exploration_bonus = exploration * math.sqrt(math.log(total_pulls + 2) / (pulls + 1))
    return round(mean_reward - (failure_rate * 0.5) - latency_penalty + exploration_bonus, 12)


def _tool_risk_allowed(risk_class: str, maximum: str) -> bool:
    return risk_class in AUTONOMOUS_TOOL_RISK_ORDER and maximum in AUTONOMOUS_TOOL_RISK_ORDER and AUTONOMOUS_TOOL_RISK_ORDER.index(risk_class) <= AUTONOMOUS_TOOL_RISK_ORDER.index(maximum)


def _portfolio_candidate_reason(
    binding: AutonomousDomainToolBinding,
    stage: "_WorkflowStage",
    caller_allowed: set[str] | None,
    read_only_only: bool,
    maximum_risk_class: str,
    arm: Mapping[str, Any] | None,
) -> str:
    if caller_allowed is not None and binding.name not in caller_allowed:
        return "not_allowed"
    if not _tool_risk_allowed(binding.risk_class, maximum_risk_class):
        return "risk_budget_exceeded"
    if (read_only_only and not binding.read_only) or (stage.read_only and not binding.read_only):
        return "read_only_required"
    if not stage.approval_required and binding.approval_required:
        return "approval_required"
    if arm is not None and bool(arm.get("disabled", False)):
        return "learning_disabled"
    return "eligible"


def _portfolio_candidate_ranking(
    tokens: Sequence[str],
    requested_capabilities: Sequence[str],
    stage: "_WorkflowStage",
    bindings: Sequence[AutonomousDomainToolBinding],
    domain: str,
    tool_selection_state: Mapping[str, Any],
    total_pulls: int,
    exploration: float,
    caller_allowed: set[str] | None,
    read_only_only: bool,
    maximum_risk_class: str,
) -> list[dict[str, Any]]:
    def reason_for(binding: AutonomousDomainToolBinding) -> str:
        return _portfolio_candidate_reason(
            binding,
            stage,
            caller_allowed,
            read_only_only,
            maximum_risk_class,
            _tool_selection_arm_for(tool_selection_state, domain, stage, binding),
        )

    eligible = [binding for binding in bindings if reason_for(binding) == "eligible"]
    ranked = sorted(
        eligible,
        key=lambda binding: _portfolio_score_key(
            _portfolio_score(tokens, requested_capabilities, stage, binding, domain, tool_selection_state, total_pulls, exploration),
            binding.name,
        ),
    )
    rank_by_name = {binding.name: index + 1 for index, binding in enumerate(ranked)}
    rows: list[dict[str, Any]] = []
    for binding in bindings:
        arm = _tool_selection_arm_for(tool_selection_state, domain, stage, binding)
        score = _portfolio_score(tokens, requested_capabilities, stage, binding, domain, tool_selection_state, total_pulls, exploration)
        pulls = int(arm["pulls"]) if arm is not None else 0
        rows.append({
            "tool": binding.name,
            "capability": binding.capability,
            "risk_class": binding.risk_class,
            "read_only": binding.read_only,
            "approval_required": binding.approval_required,
            "eligible": binding.name in rank_by_name,
            "rank": rank_by_name.get(binding.name),
            "requested_capability_match": score[0] == 1,
            "stage_capability_match": score[1] == 1,
            "selection_utility": score[2],
            "task_relevance": score[3],
            "observed_pulls": pulls,
            "observed_failure_rate": 0 if pulls == 0 else round(int(arm["failures"]) / pulls, 12),
            "reason": reason_for(binding),
        })
    ranked_eligible = sorted(
        (row for row in rows if row["eligible"]),
        key=lambda row: (row["rank"], row["tool"]),
    )
    rejection_priority = {
        "risk_budget_exceeded": 0,
        "read_only_required": 1,
        "approval_required": 2,
        "not_allowed": 3,
        "learning_disabled": 4,
    }
    ranked_rejected = sorted(
        (row for row in rows if not row["eligible"]),
        key=lambda row: (rejection_priority[row["reason"]], row["tool"]),
    )
    if not ranked_rejected:
        return ranked_eligible[:MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE]
    return (
        ranked_eligible[:1]
        + ranked_rejected[:MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE - 1]
    )[:MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE]


def settle_autonomous_tool_selection_outcome(
    state: Mapping[str, Any] | None,
    *,
    domain: str,
    capability: str,
    tool: str,
    reward: float,
    failed: bool = False,
    latency_ms: float | None = None,
    outcome_digest: str | None = None,
) -> dict[str, Any]:
    """Apply one evaluator-approved value-only outcome to caller-owned tool-arm state."""

    if domain not in AUTONOMOUS_DOMAINS:
        raise BrainRunError("tool selection outcome domain is unsupported")
    if not isinstance(failed, bool):
        raise BrainRunError("tool selection outcome failed must be boolean")
    capability = _identifier("tool selection outcome capability", capability)
    tool = _identifier("tool selection outcome tool", tool)
    reward = float(_tool_selection_number("tool selection outcome reward", reward, -1, 1))
    if latency_ms is not None:
        latency_ms = float(_tool_selection_number("tool selection outcome latency_ms", latency_ms, 0, 3_600_000))
    current = normalize_autonomous_tool_selection_state(state)
    arm_id = autonomous_tool_selection_arm_id(domain, capability, tool)
    if outcome_digest is not None and (not isinstance(outcome_digest, str) or len(outcome_digest) != 64 or any(character not in "0123456789abcdef" for character in outcome_digest)):
        raise BrainRunError("tool selection outcome outcome_digest must be a lowercase SHA-256 digest")
    if outcome_digest is not None:
        prior_credit = next((credit for credit in current["credited_outcomes"] if credit["outcome_digest"] == outcome_digest), None)
        if prior_credit is not None:
            normalized_latency = _tool_selection_json_number(latency_ms)
            if prior_credit["arm_id"] != arm_id or prior_credit["reward"] != _tool_selection_json_number(reward) or prior_credit["failed"] != failed or prior_credit["latency_ms"] != normalized_latency:
                raise BrainRunError("tool selection outcome digest was reused with contradictory metadata")
            return current
    prior = next((arm for arm in current["arms"] if arm["arm_id"] == arm_id), None)
    pulls = int(prior["pulls"] if prior else 0)
    prior_latency = prior.get("latency_ms") if prior else None
    next_arm = {
        "arm_id": arm_id,
        "pulls": pulls + 1,
        "reward_sum": _tool_selection_json_number(round(float(prior["reward_sum"] if prior else 0) + reward, 12)),
        "failures": int(prior["failures"] if prior else 0) + int(failed),
        "latency_ms": None if latency_ms is None and prior_latency is None else _tool_selection_json_number(round(((float(prior_latency if prior_latency is not None else latency_ms) * pulls) + float(latency_ms if latency_ms is not None else prior_latency)) / (pulls + 1), 6)),
        "disabled": bool(prior["disabled"]) if prior else False,
    }
    arms = [arm for arm in current["arms"] if arm["arm_id"] != arm_id] + [next_arm]
    if len(arms) > MAX_AUTONOMOUS_TOOL_SELECTION_ARMS:
        raise BrainRunError("tool selection state has reached its arm bound")
    credited_outcomes = list(current["credited_outcomes"])
    if outcome_digest is not None:
        credited_outcomes.append({"outcome_digest": outcome_digest, "arm_id": arm_id, "reward": _tool_selection_json_number(reward), "failed": failed, "latency_ms": _tool_selection_json_number(latency_ms)})
    if len(credited_outcomes) > MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS:
        raise BrainRunError("tool selection credit ledger has reached its bound")
    return {"schema": AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, "generation": current["generation"] + 1, "arms": sorted(arms, key=lambda arm: arm["arm_id"]), "credited_outcomes": sorted(credited_outcomes, key=lambda credit: credit["outcome_digest"])}


def _update_autonomous_tool_selection_state(
    state: Mapping[str, Any] | None,
    outcome: Mapping[str, Any],
) -> dict[str, Any]:
    """Bridge metadata-only evaluator credit into the adaptive tool selector."""

    if not isinstance(outcome, Mapping):
        raise BrainRunError("tool selection evaluator outcome must be a mapping")
    return settle_autonomous_tool_selection_outcome(
        state,
        domain=outcome.get("domain"),
        capability=outcome.get("capability"),
        tool=outcome.get("tool"),
        reward=outcome.get("reward"),
        failed=outcome.get("failed", False),
        latency_ms=outcome.get("latency_ms"),
        outcome_digest=outcome.get("outcome_digest"),
    )

__all__ = [
    "AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA",
    "AUTONOMOUS_TOOL_SELECTION_POLICY",
    "AUTONOMOUS_TOOL_RISK_ORDER",
    "MAX_AUTONOMOUS_TOOL_SELECTION_ARMS",
    "MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS",
    "MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE",
    "normalize_autonomous_tool_selection_state",
    "autonomous_tool_selection_arm_id",
    "settle_autonomous_tool_selection_outcome",
]
