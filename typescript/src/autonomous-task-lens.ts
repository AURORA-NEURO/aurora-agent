import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_TASK_LENS_SCHEMA = "bioprism-autonomous-domain-task-lens/0.1" as const;
export const AUTONOMOUS_TASK_LENS_VERSION = "0.1" as const;
export const AUTONOMOUS_TASK_LENS_DOMAINS: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];
export const MAX_AUTONOMOUS_TASK_LENS_ITEMS = 8;
const MAX_TASK_LENS_TEXT_BYTES = 512;

export interface AutonomousDomainTaskLens extends JsonObject {
  schema: typeof AUTONOMOUS_TASK_LENS_SCHEMA;
  domain: AutonomousDomainName;
  lens_id: string;
  lens_version: typeof AUTONOMOUS_TASK_LENS_VERSION;
  objective: string;
  planning_dimensions: string[];
  decision_checks: string[];
  evidence_priorities: string[];
  evaluator_signals: string[];
  model_capability_hints: string[];
  output_sections: string[];
  failure_modes: string[];
  lens_digest: string;
  retention: "value_only_strategy_metadata";
  secret_material: "never_returned";
}

function text(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\0") || new TextEncoder().encode(value).length > MAX_TASK_LENS_TEXT_BYTES) throw new ArgumentError(`${name} is outside the task-lens bounds`);
  return value;
}

function items(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_AUTONOMOUS_TASK_LENS_ITEMS) throw new ArgumentError(`${name} must contain between 1 and ${MAX_AUTONOMOUS_TASK_LENS_ITEMS} items`);
  const result = value.map((item) => text(`${name} item`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

type LensSeed = Omit<AutonomousDomainTaskLens, "schema" | "domain" | "lens_version" | "lens_digest" | "retention" | "secret_material">;

/** Validate a persisted lens before it is used to classify or plan a task. */
export function validateAutonomousDomainTaskLens(value: unknown, expectedDomain?: AutonomousDomainName): AutonomousDomainTaskLens {
  if (!isObject(value)) throw new ArgumentError("task lens must be an object");
  const allowed = new Set([
    "schema", "domain", "lens_id", "lens_version", "objective", "planning_dimensions", "decision_checks",
    "evidence_priorities", "evaluator_signals", "model_capability_hints", "output_sections", "failure_modes",
    "lens_digest", "retention", "secret_material",
  ]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("task lens contains unsupported fields");
  if (Object.keys(value).length !== allowed.size || [...allowed].some((key) => !(key in value))) throw new ArgumentError("task lens is missing required fields");
  if (value.schema !== AUTONOMOUS_TASK_LENS_SCHEMA || value.lens_version !== AUTONOMOUS_TASK_LENS_VERSION) throw new ArgumentError("task lens schema or version is invalid");
  if (value.retention !== "value_only_strategy_metadata" || value.secret_material !== "never_returned") throw new ArgumentError("task lens retention markers are invalid");
  if (typeof value.domain !== "string" || !AUTONOMOUS_TASK_LENS_DOMAINS.includes(value.domain as AutonomousDomainName)) throw new ArgumentError("task lens domain is unsupported");
  if (expectedDomain !== undefined && value.domain !== expectedDomain) throw new ArgumentError("task lens domain does not match the expected domain");
  const domain = value.domain as AutonomousDomainName;
  const descriptor = {
    schema: AUTONOMOUS_TASK_LENS_SCHEMA,
    domain,
    lens_id: text("task lens lens_id", value.lens_id),
    lens_version: AUTONOMOUS_TASK_LENS_VERSION,
    objective: text("task lens objective", value.objective),
    planning_dimensions: items("task lens planning_dimensions", value.planning_dimensions),
    decision_checks: items("task lens decision_checks", value.decision_checks),
    evidence_priorities: items("task lens evidence_priorities", value.evidence_priorities),
    evaluator_signals: items("task lens evaluator_signals", value.evaluator_signals),
    model_capability_hints: items("task lens model_capability_hints", value.model_capability_hints),
    output_sections: items("task lens output_sections", value.output_sections),
    failure_modes: items("task lens failure_modes", value.failure_modes),
    retention: "value_only_strategy_metadata" as const,
    secret_material: "never_returned" as const,
  };
  const lensDigest = digest("task lens lens_digest", value.lens_digest);
  if (lensDigest !== digestJsonSync(descriptor)) throw new ArgumentError("task lens digest does not match its metadata");
  return Object.freeze({ ...descriptor, lens_digest: lensDigest }) as AutonomousDomainTaskLens;
}

const SEEDS: Record<AutonomousDomainName, LensSeed> = {
  coding: { lens_id: "coding-change-verification", objective: "make the smallest verifiable change that satisfies the requested behavior", planning_dimensions: ["scope", "dependency_impact", "implementation", "verification"], decision_checks: ["reproduce_or_localize_before_changing", "minimize_change_surface", "preserve_existing_contracts", "run_relevant_checks"], evidence_priorities: ["repository_state", "diff_impact", "test_or_ci_output"], evaluator_signals: ["correctness", "regression_safety", "scope_discipline"], model_capability_hints: ["code", "reasoning", "tool_use"], output_sections: ["diagnosis", "change_plan", "implementation", "verification", "limitations"], failure_modes: ["unreproduced_failure", "unverified_fix", "scope_creep"] },
  browser: { lens_id: "browser-source-grounding", objective: "acquire and compare bounded sources while preserving provenance and freshness", planning_dimensions: ["source_discovery", "navigation", "provenance", "comparison"], decision_checks: ["identify_source_scope", "separate_observation_from_inference", "record_freshness", "surface_conflict"], evidence_priorities: ["source_identity", "retrieval_metadata", "cross_source_agreement"], evaluator_signals: ["source_relevance", "provenance_completeness", "conflict_visibility"], model_capability_hints: ["web_research", "reasoning", "structured_output"], output_sections: ["sources", "observations", "comparison", "uncertainty", "next_queries"], failure_modes: ["unsupported_source", "stale_source", "citation_overreach"] },
  data: { lens_id: "data-lineage-quality", objective: "turn bounded data into a traceable analysis without hiding schema or quality defects", planning_dimensions: ["schema", "lineage", "quality", "analysis"], decision_checks: ["validate_schema_before_analysis", "preserve_missingness", "track_transformations", "quantify_quality_limits"], evidence_priorities: ["schema_metadata", "lineage", "quality_checks"], evaluator_signals: ["schema_fidelity", "lineage_completeness", "analysis_reproducibility"], model_capability_hints: ["data_analysis", "reasoning", "structured_output"], output_sections: ["data_contract", "quality_findings", "analysis", "assumptions", "reproduction_steps"], failure_modes: ["missingness_as_zero", "lineage_break", "unsupported_aggregation"] },
  science: { lens_id: "science-epistemic-reproducibility", objective: "separate observations, hypotheses, causal claims, and reproducible tests", planning_dimensions: ["question", "evidence", "hypothesis", "test", "reproduction"], decision_checks: ["identify_estimand_or_question", "distinguish_correlation_from_causality", "state_alternatives", "define_reproduction"], evidence_priorities: ["primary_measurement", "method_details", "independent_reproduction"], evaluator_signals: ["epistemic_calibration", "method_fidelity", "reproducibility"], model_capability_hints: ["scientific_reasoning", "statistics", "structured_output"], output_sections: ["observations", "hypotheses", "analysis", "confounders", "reproduction_plan"], failure_modes: ["causal_overclaim", "hypothesis_as_fact", "simulation_as_measurement"] },
  biomedical: { lens_id: "biomedical-grounding-safety", objective: "organize biomedical evidence with explicit provenance, uncertainty, and human-review boundaries", planning_dimensions: ["grounding", "estimand", "provenance", "safety", "review"], decision_checks: ["identify_population_and_endpoint", "check_reference_provenance", "state_translation_limits", "require_qualified_review"], evidence_priorities: ["primary_reference", "population_definition", "safety_and_bias"], evaluator_signals: ["grounding", "estimand_fidelity", "safety_boundary"], model_capability_hints: ["biomedical_reasoning", "literature", "structured_output"], output_sections: ["evidence", "interpretation", "limitations", "safety_boundary", "human_review"], failure_modes: ["diagnostic_overreach", "prescription_overreach", "population_mismatch"] },
  neuroscience: { lens_id: "neuroscience-modality-measurement", objective: "interpret neuroscience measurements while preserving modality, transport, and study-design limits", planning_dimensions: ["modality", "measurement", "transport", "study_design", "reproduction"], decision_checks: ["inventory_modalities", "separate_signal_from_measurement", "check_pseudoreplication", "state_transport_limits"], evidence_priorities: ["modality_metadata", "measurement_comparability", "independent_trace"], evaluator_signals: ["modality_fidelity", "measurement_validity", "transport_calibration"], model_capability_hints: ["neuroscience", "signal_analysis", "structured_output"], output_sections: ["modalities", "measurements", "interpretation", "confounds", "reproduction"], failure_modes: ["modality_erasure", "pseudoreplication", "transport_overclaim"] },
  operations: { lens_id: "operations-observe-control-recover", objective: "move from observable state to reversible, authorized operational action", planning_dimensions: ["observability", "incident", "risk", "approval", "rollback"], decision_checks: ["confirm_current_state", "bound_effects", "require_authorization", "define_rollback", "verify_postcondition"], evidence_priorities: ["telemetry", "change_record", "postcondition"], evaluator_signals: ["operational_safety", "rollback_readiness", "postcondition_fidelity"], model_capability_hints: ["operations", "systems_reasoning", "structured_output"], output_sections: ["observed_state", "runbook", "approval_gate", "rollback", "verification"], failure_modes: ["unbounded_effect", "missing_rollback", "uncertain_postcondition"] },
  enterprise: { lens_id: "enterprise-governance-stewardship", objective: "coordinate enterprise work through governance, privacy, security, and accountable ownership", planning_dimensions: ["workflow", "governance", "compliance", "security", "ownership"], decision_checks: ["identify_owner", "check_policy_scope", "separate_access_from_authority", "record_exception", "define_audit_trail"], evidence_priorities: ["policy_version", "approval_record", "audit_evidence"], evaluator_signals: ["policy_fidelity", "accountability", "auditability"], model_capability_hints: ["enterprise_reasoning", "governance", "structured_output"], output_sections: ["scope", "policy_mapping", "risks", "owners", "audit_plan"], failure_modes: ["implicit_authority", "policy_drift", "unowned_exception"] },
  multi_agent: { lens_id: "multi-agent-bounded-coordination", objective: "decompose work into bounded roles with explicit handoffs, disagreement, and one effect authority", planning_dimensions: ["decomposition", "role_contracts", "handoffs", "consensus", "accountability"], decision_checks: ["bound_each_subproblem", "preserve_lineage", "surface_disagreement", "avoid_duplicate_effects", "assign_final_authority"], evidence_priorities: ["subtask_contract", "handoff_receipt", "consensus_or_dissent"], evaluator_signals: ["coverage", "handoff_integrity", "disagreement_fidelity"], model_capability_hints: ["coordination", "reasoning", "structured_output"], output_sections: ["decomposition", "role_results", "dissent", "synthesis", "accountability"], failure_modes: ["duplicate_effect", "authority_ambiguity", "false_consensus"] },
  multimodal: { lens_id: "multimodal-modality-aware-fusion", objective: "fuse available modalities without implying that missing or transformed inputs were inspected", planning_dimensions: ["modality_inventory", "transport", "alignment", "fusion", "uncertainty"], decision_checks: ["list_available_modalities", "record_transport_loss", "check_alignment", "separate_fusion_from_observation", "state_missing_modalities"], evidence_priorities: ["modality_manifest", "transport_metadata", "cross_modal_consistency"], evaluator_signals: ["modality_completeness", "transport_fidelity", "fusion_calibration"], model_capability_hints: ["multimodal", "vision_or_audio", "structured_output"], output_sections: ["modality_manifest", "observations", "fusion", "blind_spots", "next_acquisition"], failure_modes: ["absent_modality_claim", "transport_loss_erasure", "cross_modal_confusion"] },
  cross_domain: { lens_id: "cross-domain-evidence-harmonization", objective: "combine specialist results while keeping claims, evidence, and evaluators attached to their source domains", planning_dimensions: ["routing", "specialist_contracts", "evidence_harmonization", "conflict", "synthesis"], decision_checks: ["assign_domain_owners", "preserve_specialist_limits", "compare_incompatible_claims", "surface_conflict", "bound_synthesis"], evidence_priorities: ["child_contracts", "domain_receipts", "cross_domain_conflicts"], evaluator_signals: ["domain_coverage", "lineage_preservation", "synthesis_fidelity"], model_capability_hints: ["cross_domain_reasoning", "coordination", "structured_output"], output_sections: ["domain_map", "specialist_findings", "conflicts", "synthesis", "unresolved_gaps"], failure_modes: ["domain_blending", "unsupported_synthesis", "lost_dissent"] },
  evaluation: { lens_id: "evaluation-independent-measurement", objective: "measure system behavior with independent evidence, holdouts, and explicit contamination controls", planning_dimensions: ["rubric", "independence", "holdout", "metrics", "replay"], decision_checks: ["freeze_rubric", "separate_system_from_evaluator", "protect_holdout", "track_missing_cases", "replay_exactly"], evidence_priorities: ["evaluation_protocol", "holdout_coverage", "replay_trace"], evaluator_signals: ["independence", "calibration", "reproducibility"], model_capability_hints: ["evaluation", "statistics", "structured_output"], output_sections: ["protocol", "metrics", "coverage", "failures", "replay"], failure_modes: ["self_authored_pass", "holdout_contamination", "metric_drift"] },
};

function makeLens(domain: AutonomousDomainName): AutonomousDomainTaskLens {
  const seed = SEEDS[domain];
  const descriptor: Omit<AutonomousDomainTaskLens, "lens_digest"> = { schema: AUTONOMOUS_TASK_LENS_SCHEMA, domain, lens_version: AUTONOMOUS_TASK_LENS_VERSION, ...seed, retention: "value_only_strategy_metadata", secret_material: "never_returned" };
  const groups = ["planning_dimensions", "decision_checks", "evidence_priorities", "evaluator_signals", "model_capability_hints", "output_sections", "failure_modes"] as const;
  for (const group of groups) {
    const values = descriptor[group];
    if (!Array.isArray(values) || values.length < 1 || values.length > MAX_AUTONOMOUS_TASK_LENS_ITEMS || new Set(values).size !== values.length) throw new ArgumentError(`task lens ${domain}.${group} is outside its bounds`);
    values.forEach((value) => text(`task lens ${domain}.${group}`, value));
  }
  return Object.freeze({ ...descriptor, lens_digest: digestJsonSync(descriptor) }) as AutonomousDomainTaskLens;
}

const BUILTIN_LENSES = new Map(AUTONOMOUS_TASK_LENS_DOMAINS.map((domain) => [domain, makeLens(domain)]));

export function builtinAutonomousDomainTaskLenses(): readonly AutonomousDomainTaskLens[] {
  return AUTONOMOUS_TASK_LENS_DOMAINS.map((domain) => BUILTIN_LENSES.get(domain) as AutonomousDomainTaskLens);
}

export function autonomousDomainTaskLens(domain: AutonomousDomainName): AutonomousDomainTaskLens {
  return BUILTIN_LENSES.get(domain) as AutonomousDomainTaskLens ?? makeLens(domain);
}

export function autonomousTaskLensPromptContract(lens: AutonomousDomainTaskLens, compact = false): JsonObject {
  const reviewedLens = validateAutonomousDomainTaskLens(lens);
  if (compact) {
    return {
      schema: AUTONOMOUS_TASK_LENS_SCHEMA,
      domain: reviewedLens.domain,
      lens_id: reviewedLens.lens_id,
      lens_digest: reviewedLens.lens_digest,
      execution: "guidance_only; provider_and_effect_boundaries_remain_separate",
    };
  }
  return {
    schema: AUTONOMOUS_TASK_LENS_SCHEMA,
    domain: reviewedLens.domain,
    lens_id: reviewedLens.lens_id,
    lens_digest: reviewedLens.lens_digest,
    objective: reviewedLens.objective,
    planning_dimensions: [...reviewedLens.planning_dimensions],
    decision_checks: [...reviewedLens.decision_checks],
    evidence_priorities: [...reviewedLens.evidence_priorities],
    evaluator_signals: [...reviewedLens.evaluator_signals],
    model_capability_hints: [...reviewedLens.model_capability_hints],
    output_sections: [...reviewedLens.output_sections],
    failure_modes: [...reviewedLens.failure_modes],
    model_hints_are: "preferences_only; they do not authorize or hard-gate a model",
    execution: "guidance_only; provider_and_effect_boundaries_remain_separate",
    secret_material: "never_returned",
  };
}
