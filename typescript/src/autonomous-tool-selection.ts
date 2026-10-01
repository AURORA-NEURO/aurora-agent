/** Reviewed capability admission and value-only adaptive tool selection. */

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import { boundedIdentifier } from "./autonomous-validation.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { normalizeRouteText } from "./autonomous-text-normalization.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type {
  AutonomousCapabilityCandidateRanking,
  AutonomousCapabilityCandidateReason,
  AutonomousDomainProfile,
  AutonomousDomainToolBinding,
  AutonomousToolRiskClass,
  AutonomousToolSelectionArm,
  AutonomousToolSelectionOutcome,
  AutonomousToolSelectionState,
  AutonomousWorkflow,
  AutonomousWorkflowStage,
} from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_WORKFLOW_STAGE_CONTRACT_SCHEMA = "bioprism-typescript-autonomous-workflow-stage-contract/0.1" as const;

/**
 * Reviewed workflow-to-adapter aliases. Workflow stages intentionally use a small stable
 * capability vocabulary while live tools expose narrower adapter capabilities. Keeping this
 * bridge explicit prevents fuzzy tool selection while making the built-in catalogue executable.
 */
export const WORKFLOW_CAPABILITY_ALIASES: Readonly<Record<AutonomousDomainName, Readonly<Record<string, readonly string[]>>>> = {
  coding: {
    review: ["engineering_contract_audit", "delivery_audit", "delivery_receipt_verification", "conformance_verification", "stewardship_review"],
    debugging: ["repository_inspection", "repository_impact_analysis", "ci_evidence_audit", "ci_evidence_normalization", "sdk_registry_audit"],
    implementation: ["developer_workbench", "developer_workbench_verification", "engineering_planning", "engineering_execution_plan", "mission_execution"],
    testing: ["ci_execution_audit", "ci_evidence_audit", "conformance_verification", "release_readiness", "delivery_receipt_verification"],
  },
  browser: {
    web_research: ["evidence_acquisition_discovery", "evidence_source_planning", "evidence_coverage", "hub_discovery", "lens_discovery"],
    navigation: ["capability_discovery", "capability_routing", "hub_resolution", "route_planning", "workspace_capability_discovery"],
    source_comparison: ["evidence_coverage", "route_plan_verification", "route_review", "evidence_source_planning"],
  },
  data: {
    data_analysis: ["context_compilation", "context_comparison", "context_refinement", "projection_bundling", "tabular_ingestion", "world_validation"],
    schema_validation: ["world_claim_validation", "context_verification", "obligation_gate", "data_adapter_planning"],
    lineage: ["lineage_audit", "context_explanation", "context_verification", "evidence_coverage"],
    quality_control: ["quality_control", "world_validation", "world_claim_validation", "evidence_coverage", "obligation_gate"],
  },
  science: {
    literature: ["literature_binding", "contradiction_review", "research_routing", "research_routing_replay", "epistemic_context_audit"],
    hypothesis: ["epistemic_selection_audit", "decision_quotient", "value_of_information", "influence_analysis"],
    experiment: ["laboratory_planning", "adaptive_acquisition_execution", "measurement_comparison", "value_of_information"],
    statistics: ["decision_quotient", "influence_analysis", "measurement_comparison", "laboratory_pareto_audit"],
    reproducibility: ["reproduction_check", "research_routing_replay", "laboratory_holdout_audit", "laboratory_branch_audit", "laboratory_evolution_audit"],
  },
  biomedical: {
    biomedical_review: ["biomedical_grounding_audit", "biomedical_reference_audit", "biomedical_estimand_audit", "literature_binding", "contradiction_review"],
    provenance: ["biomedical_reference_audit", "literature_binding", "measurement_comparison", "world_validation", "representation_audit"],
    safety_boundary: ["medical_boundary", "dual_use_review", "bioethics_validation", "bioethics_action_review", "oncology_boundary"],
    human_review: ["human_subject_screening", "bioethics_action_review", "bioethics_validation", "medical_boundary"],
  },
  neuroscience: {
    neuroscience_analysis: ["measurement_comparison", "influence_analysis", "trajectory_trace_analysis", "modality_catalogue"],
    signal_interpretation: ["modality_support", "modality_transport", "modality_comparability", "measurement_comparison"],
    study_design: ["value_of_information", "laboratory_holdout_audit", "measurement_comparison"],
    reproducibility: ["benchmark_trace_analysis", "laboratory_holdout_audit", "trajectory_evaluation", "trajectory_trace_analysis"],
  },
  operations: {
    observability: ["telemetry_projection", "operations_catalogue", "ledger_ingestion", "runtime_tape_verification"],
    incident_response: ["runtime_effect_check", "operations_acceptance", "quality_gate", "artifact_registry_audit"],
    risk_review: ["operational_readiness", "registry_gate", "factory_authority_verification", "release_audit"],
    rollback: ["storage_lifecycle_simulation", "cache_invalidation_simulation", "registry_lifecycle_simulation", "factory_lifecycle_simulation"],
    approval: ["factory_authority_verification", "registry_gate", "operations_acceptance", "quality_gate"],
    runbook: ["operational_readiness", "operations_catalogue", "release_audit", "runtime_tape_verification"],
  },
  enterprise: {
    workflow: ["governance_schema", "sandbox_runtime_simulation", "sandbox_admission", "provider_capability_verification"],
    governance: ["governance_schema", "stewardship_review", "security_program_audit", "security_privacy_audit", "hub_disclosure_review"],
    compliance: ["policy_screening", "release_audit", "safety_release_gate", "security_privacy_audit", "dual_use_review"],
    analytics: ["provider_capability_verification", "security_redteam_simulation", "safety_posture", "release_audit"],
    coordination: ["hub_submission_review", "hub_lock", "stewardship_review", "medical_boundary"],
  },
  multi_agent: {
    delegation: ["protocol_compilation", "workflow_execution", "mission_execution", "mission_evidence_import"],
    coordination: ["protocol_catalogue", "workflow_catalogue", "choreography_validation", "multi_agent_synthesis"],
    consensus: ["mission_evaluator_review", "mission_evaluator_replay_comparison", "mission_evidence_verification", "multi_agent_synthesis"],
    conflict_resolution: ["mission_evaluator_replay", "mission_evaluator_replay_comparison", "mission_evidence_lookup", "choreography_validation"],
    handoff: ["mission_evidence_import", "mission_evidence_query", "mission_evidence_verification", "workflow_execution"],
  },
  multimodal: {
    image: ["modality_catalogue", "modality_support", "modality_comparability", "projection_bundling", "hub_card_rendering"],
    audio: ["modality_catalogue", "modality_support", "modality_transport", "measurement_comparison"],
    video: ["modality_catalogue", "modality_support", "modality_transport", "measurement_comparison"],
    document: ["literature_binding", "context_comparison", "projection_bundling", "hub_card_rendering"],
    cross_modal_alignment: ["modality_comparability", "modality_transport", "modality_support", "measurement_comparison", "context_comparison"],
  },
  cross_domain: {
    routing: ["capability_discovery", "capability_routing", "route_planning", "route_review", "workspace_capability_discovery"],
    synthesis: ["evidence_intake", "evidence_source_execution", "provider_normalization", "workflow_portfolio"],
    evidence_alignment: ["evidence_coverage", "evidence_source_planning", "route_plan_verification", "workflow_portfolio_verification"],
    workflow_composition: ["workflow_catalogue", "workflow_instantiation", "workflow_scaffolding", "workflow_verification"],
  },
  evaluation: {
    benchmarking: ["benchmark_compilation", "benchmark_compilation_review", "benchmark_counterfactual", "benchmark_integrity_audit", "benchmark_oracle_review"],
    rubric: ["metrics_profile_audit", "metrics_analytics_audit", "evaluation_minimization", "posterior_gate"],
    replay: ["research_ci", "reproduction_check", "trajectory_evaluation", "benchmark_trace_analysis", "worldline_evaluation"],
    failure_analysis: ["benchmark_decision_audit", "benchmark_trace_analysis", "oracle_missingness", "oracle_combination"],
    reproducibility: ["reproduction_check", "research_ci", "adaptive_evaluation_panel", "benchmark_integrity_audit"],
  },
};
export const AUTONOMOUS_TOOL_RISK_ORDER: readonly AutonomousToolRiskClass[] = [
  "read_only",
  "reversible_effect",
  "external_effect",
  "high_impact_effect",
];

/** Shared value-only state for adaptive reviewed-tool selection. */
export const AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA = "bioprism-autonomous-tool-selection-state/0.1" as const;
export const AUTONOMOUS_TOOL_SELECTION_POLICY = "stage_coverage_then_capability_then_ucb_value_then_task_relevance_then_read_only_then_name" as const;
export const MAX_AUTONOMOUS_TOOL_SELECTION_ARMS = 512;
export const MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS = 4096;
export const MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE = 2;

/** Shared reviewed capability predicate used by planning, stage admission, and domain audits. */
export function autonomousDomainToolBindingSupportsStage(profile: AutonomousDomainProfile, stage: AutonomousWorkflowStage, binding: AutonomousDomainToolBinding): boolean {
  return stage.required_capabilities.some((capability) => (
    binding.capability === capability || (WORKFLOW_CAPABILITY_ALIASES[profile.domain][capability] ?? []).includes(binding.capability)
  ));
}

function bindingSupportsStage(profile: AutonomousDomainProfile, stage: AutonomousWorkflowStage, binding: AutonomousDomainToolBinding): boolean {
  return autonomousDomainToolBindingSupportsStage(profile, stage, binding);
}

function workflowStageContractDescriptor(workflow: AutonomousWorkflow, stage: AutonomousWorkflowStage): JsonObject {
  return {
    schema: AUTONOMOUS_WORKFLOW_STAGE_CONTRACT_SCHEMA,
    domain: workflow.domain,
    workflow_id: workflow.workflow_id,
    workflow_digest: workflow.workflow_digest,
    stage_id: stage.id,
    objective: stage.objective,
    required_capabilities: [...stage.required_capabilities],
    depends_on: [...stage.depends_on],
    evidence_outputs: [...stage.evidence_outputs],
    evaluator_signals: [...stage.evaluator_signals],
    read_only: stage.read_only,
    approval_required: stage.approval_required,
  };
}

export function boundedToolSelectionNumber(name: string, value: unknown, minimum: number, maximum: number, integer = false): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum || (integer && !Number.isSafeInteger(value))) {
    throw new ArgumentError(`${name} is outside the tool-selection learning contract`);
  }
  return value;
}

/** Normalize caller-owned tool learning state without accepting transient payloads. */
export function normalizeAutonomousToolSelectionState(value: unknown): AutonomousToolSelectionState {
  if (value === undefined || value === null) return { schema: AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, generation: 0, arms: [], credited_outcomes: [] };
  if (!isObject(value)) throw new ArgumentError("tool selection state must be an object");
  if (Object.keys(value).some((key) => !["schema", "generation", "arms", "credited_outcomes"].includes(key))) throw new ArgumentError("tool selection state contains unsupported fields");
  if (value.schema !== undefined && value.schema !== AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA) throw new ArgumentError("tool selection state schema is unsupported");
  const generation = boundedToolSelectionNumber("tool selection generation", value.generation ?? 0, 0, 1_000_000_000, true);
  if (!Array.isArray(value.arms) || value.arms.length > MAX_AUTONOMOUS_TOOL_SELECTION_ARMS) throw new ArgumentError(`tool selection state arms must contain at most ${MAX_AUTONOMOUS_TOOL_SELECTION_ARMS} entries`);
  const seen = new Set<string>();
  const arms = value.arms.map((raw, index) => {
    if (!isObject(raw)) throw new ArgumentError(`tool selection arm ${index} is malformed`);
    if (Object.keys(raw).some((key) => !["arm_id", "pulls", "reward_sum", "failures", "latency_ms", "disabled"].includes(key))) throw new ArgumentError("tool selection arm contains unsupported fields");
    const armId = boundedIdentifier(`tool selection arm ${index} arm_id`, raw.arm_id);
    if (seen.has(armId)) throw new ArgumentError(`tool selection state contains duplicate arm ${armId}`);
    seen.add(armId);
    const pulls = boundedToolSelectionNumber(`tool selection arm ${armId} pulls`, raw.pulls ?? 0, 0, 1_000_000_000, true);
    const rewardSum = boundedToolSelectionNumber(`tool selection arm ${armId} reward_sum`, raw.reward_sum ?? 0, -pulls, pulls);
    const failures = boundedToolSelectionNumber(`tool selection arm ${armId} failures`, raw.failures ?? 0, 0, pulls, true);
    const latencyMs = raw.latency_ms === undefined || raw.latency_ms === null
      ? null
      : boundedToolSelectionNumber(`tool selection arm ${armId} latency_ms`, raw.latency_ms, 0, 3_600_000);
    if (raw.disabled !== undefined && typeof raw.disabled !== "boolean") throw new ArgumentError(`tool selection arm ${armId} disabled must be boolean`);
    return { arm_id: armId, pulls, reward_sum: rewardSum, failures, latency_ms: latencyMs, disabled: raw.disabled ?? false };
  }).sort((left, right) => left.arm_id.localeCompare(right.arm_id));
  const rawCredits = value.credited_outcomes ?? [];
  if (!Array.isArray(rawCredits) || rawCredits.length > MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS) throw new ArgumentError(`tool selection state credits must contain at most ${MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS} entries`);
  const creditIds = new Set<string>();
  const creditedOutcomes = rawCredits.map((raw, index) => {
    if (!isObject(raw)) throw new ArgumentError(`tool selection credit ${index} is malformed`);
    if (Object.keys(raw).some((key) => !["outcome_digest", "arm_id", "reward", "failed", "latency_ms"].includes(key))) throw new ArgumentError("tool selection credit contains unsupported fields");
    if (typeof raw.outcome_digest !== "string" || !/^[0-9a-f]{64}$/.test(raw.outcome_digest)) throw new ArgumentError(`tool selection credit ${index} outcome_digest must be a lowercase SHA-256 digest`);
    if (creditIds.has(raw.outcome_digest)) throw new ArgumentError(`tool selection state contains duplicate outcome ${raw.outcome_digest}`);
    creditIds.add(raw.outcome_digest);
    const armId = boundedIdentifier(`tool selection credit ${index} arm_id`, raw.arm_id);
    const reward = boundedToolSelectionNumber(`tool selection credit ${index} reward`, raw.reward, -1, 1);
    if (typeof raw.failed !== "boolean") throw new ArgumentError(`tool selection credit ${index} failed must be boolean`);
    const latencyMs = raw.latency_ms === undefined || raw.latency_ms === null ? null : boundedToolSelectionNumber(`tool selection credit ${index} latency_ms`, raw.latency_ms, 0, 3_600_000);
    return { outcome_digest: raw.outcome_digest, arm_id: armId, reward, failed: raw.failed, latency_ms: latencyMs };
  }).sort((left, right) => left.outcome_digest.localeCompare(right.outcome_digest));
  return { schema: AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, generation, arms, credited_outcomes: creditedOutcomes };
}

/** Stable contextual arm identity shared by the TypeScript and Python planners. */
export function autonomousToolSelectionArmId(domain: AutonomousDomainName, capability: string, tool: string): string {
  return [boundedIdentifier("tool selection domain", domain), boundedIdentifier("tool selection capability", capability), boundedIdentifier("tool selection tool", tool)].join(".");
}

export function toolSelectionArmFor(
  state: AutonomousToolSelectionState,
  domain: AutonomousDomainName,
  stage: AutonomousWorkflowStage,
  binding: AutonomousDomainToolBinding,
): AutonomousToolSelectionArm | null {
  const ids = [
    autonomousToolSelectionArmId(domain, binding.capability, binding.name),
    autonomousToolSelectionArmId(domain, stage.required_capabilities[0] ?? binding.capability, binding.name),
  ];
  return state.arms.find((arm) => ids.includes(arm.arm_id)) ?? state.arms.find((arm) => arm.arm_id === binding.name) ?? null;
}

export function toolSelectionUtility(arm: AutonomousToolSelectionArm | null, totalPulls: number, exploration: number): number {
  if (arm === null) return 0;
  const pulls = arm.pulls;
  const meanReward = pulls === 0 ? 0 : arm.reward_sum / pulls;
  const failureRate = pulls === 0 ? 0 : arm.failures / pulls;
  const latencyPenalty = arm.latency_ms === null ? 0 : Math.min(arm.latency_ms / 10_000, 1) * 0.1;
  const explorationBonus = exploration * Math.sqrt(Math.log(totalPulls + 2) / (pulls + 1));
  return Number((meanReward - (failureRate * 0.5) - latencyPenalty + explorationBonus).toFixed(12));
}

export function autonomousToolRiskAllowed(riskClass: AutonomousToolRiskClass, maximum: AutonomousToolRiskClass): boolean {
  return AUTONOMOUS_TOOL_RISK_ORDER.indexOf(riskClass) <= AUTONOMOUS_TOOL_RISK_ORDER.indexOf(maximum);
}

function capabilityCandidateReason(
  binding: AutonomousDomainToolBinding,
  stage: AutonomousWorkflowStage,
  allowedTools: ReadonlySet<string> | null,
  readOnlyOnly: boolean,
  maximumRiskClass: AutonomousToolRiskClass,
  arm: AutonomousToolSelectionArm | null,
): AutonomousCapabilityCandidateReason {
  if (allowedTools !== null && !allowedTools.has(binding.name)) return "not_allowed";
  if (!autonomousToolRiskAllowed(binding.risk_class, maximumRiskClass)) return "risk_budget_exceeded";
  if (readOnlyOnly && !binding.read_only || stage.read_only && !binding.read_only) return "read_only_required";
  if (!stage.approval_required && binding.approval_required) return "approval_required";
  if (arm?.disabled === true) return "learning_disabled";
  return "eligible";
}

export function capabilityCandidateRanking(
  tokens: readonly string[],
  requestedCapabilities: readonly string[],
  stage: AutonomousWorkflowStage,
  bindings: readonly AutonomousDomainToolBinding[],
  domain: AutonomousDomainName,
  toolSelectionState: AutonomousToolSelectionState,
  totalPulls: number,
  exploration: number,
  allowedTools: ReadonlySet<string> | null,
  readOnlyOnly: boolean,
  maximumRiskClass: AutonomousToolRiskClass,
): AutonomousCapabilityCandidateRanking[] {
  const eligible = bindings.filter((binding) => capabilityCandidateReason(
    binding,
    stage,
    allowedTools,
    readOnlyOnly,
    maximumRiskClass,
    toolSelectionArmFor(toolSelectionState, domain, stage, binding),
  ) === "eligible");
  const rankByName = new Map(
    [...eligible].sort((left, right) => compareCapabilityScores(
      capabilityCandidateScore(tokens, requestedCapabilities, stage, left, domain, toolSelectionState, totalPulls, exploration),
      capabilityCandidateScore(tokens, requestedCapabilities, stage, right, domain, toolSelectionState, totalPulls, exploration),
    ) || left.name.localeCompare(right.name)).map((binding, index) => [binding.name, index + 1]),
  );
  const rows = [...bindings].map((binding) => {
    const arm = toolSelectionArmFor(toolSelectionState, domain, stage, binding);
    const score = capabilityCandidateScore(tokens, requestedCapabilities, stage, binding, domain, toolSelectionState, totalPulls, exploration);
    const pulls = arm?.pulls ?? 0;
    return {
      tool: binding.name,
      capability: binding.capability,
      risk_class: binding.risk_class,
      read_only: binding.read_only,
      approval_required: binding.approval_required,
      eligible: rankByName.has(binding.name),
      rank: rankByName.get(binding.name) ?? null,
      requested_capability_match: score[0] === 1,
      stage_capability_match: score[1] === 1,
      selection_utility: score[2],
      task_relevance: score[3],
      observed_pulls: pulls,
      observed_failure_rate: pulls === 0 ? 0 : Number(((arm?.failures ?? 0) / pulls).toFixed(12)),
      reason: capabilityCandidateReason(binding, stage, allowedTools, readOnlyOnly, maximumRiskClass, arm),
    };
  });
  const rankedEligible = rows.filter((row) => row.eligible).sort((left, right) => (left.rank! - right.rank!) || left.tool.localeCompare(right.tool));
  const rejectionPriority: Record<AutonomousCapabilityCandidateReason, number> = {
    eligible: 99,
    risk_budget_exceeded: 0,
    read_only_required: 1,
    approval_required: 2,
    not_allowed: 3,
    learning_disabled: 4,
  };
  const rankedRejected = rows.filter((row) => !row.eligible).sort((left, right) => (
    rejectionPriority[left.reason] - rejectionPriority[right.reason] || left.tool.localeCompare(right.tool)
  ));
  if (rankedRejected.length === 0) return rankedEligible.slice(0, MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE);
  return [
    ...rankedEligible.slice(0, 1),
    ...rankedRejected.slice(0, MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE - 1),
  ].slice(0, MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE);
}

/** Pure, caller-owned online update for one value-only tool outcome. */
export function settleAutonomousToolSelectionOutcome(
  state: AutonomousToolSelectionState | null | undefined,
  outcome: AutonomousToolSelectionOutcome,
): AutonomousToolSelectionState {
  if (!outcome || !AUTONOMOUS_DOMAIN_NAMES.includes(outcome.domain)) throw new ArgumentError("tool selection outcome domain is unsupported");
  const capability = boundedIdentifier("tool selection outcome capability", outcome.capability);
  const tool = boundedIdentifier("tool selection outcome tool", outcome.tool);
  const reward = boundedToolSelectionNumber("tool selection outcome reward", outcome.reward, -1, 1);
  if (outcome.failed !== undefined && typeof outcome.failed !== "boolean") throw new ArgumentError("tool selection outcome failed must be boolean");
  const latencyMs = outcome.latencyMs === undefined || outcome.latencyMs === null ? null : boundedToolSelectionNumber("tool selection outcome latencyMs", outcome.latencyMs, 0, 3_600_000);
  const current = normalizeAutonomousToolSelectionState(state);
  const armId = autonomousToolSelectionArmId(outcome.domain, capability, tool);
  const outcomeDigest = outcome.outcomeDigest ?? null;
  if (outcomeDigest !== null && (typeof outcomeDigest !== "string" || !/^[0-9a-f]{64}$/.test(outcomeDigest))) throw new ArgumentError("tool selection outcome outcomeDigest must be a lowercase SHA-256 digest");
  const failed = outcome.failed === true;
  if (outcomeDigest !== null) {
    const priorCredit = current.credited_outcomes.find((credit) => credit.outcome_digest === outcomeDigest);
    if (priorCredit) {
      if (priorCredit.arm_id !== armId || priorCredit.reward !== reward || priorCredit.failed !== failed || priorCredit.latency_ms !== latencyMs) throw new ArgumentError("tool selection outcome digest was reused with contradictory metadata");
      return current;
    }
  }
  const prior = current.arms.find((arm) => arm.arm_id === armId);
  const nextArm: AutonomousToolSelectionArm = {
    arm_id: armId,
    pulls: (prior?.pulls ?? 0) + 1,
    reward_sum: Number(((prior?.reward_sum ?? 0) + reward).toFixed(12)),
    failures: (prior?.failures ?? 0) + Number(failed),
    latency_ms: latencyMs === null ? prior?.latency_ms ?? null : Number((((prior?.latency_ms ?? latencyMs) * (prior?.pulls ?? 0) + latencyMs) / ((prior?.pulls ?? 0) + 1)).toFixed(6)),
    disabled: prior?.disabled ?? false,
  };
  const arms = [...current.arms.filter((arm) => arm.arm_id !== armId), nextArm].sort((left, right) => left.arm_id.localeCompare(right.arm_id));
  if (arms.length > MAX_AUTONOMOUS_TOOL_SELECTION_ARMS) throw new ArgumentError("tool selection state has reached its arm bound");
  const creditedOutcomes = outcomeDigest === null
    ? current.credited_outcomes
    : [...current.credited_outcomes, { outcome_digest: outcomeDigest, arm_id: armId, reward, failed, latency_ms: latencyMs }].sort((left, right) => left.outcome_digest.localeCompare(right.outcome_digest));
  if (creditedOutcomes.length > MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS) throw new ArgumentError("tool selection credit ledger has reached its bound");
  return { schema: AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, generation: current.generation + 1, arms, credited_outcomes: creditedOutcomes };
}

/** Digest the exact stage contract that a live adapter receipt is bound to. */
export async function autonomousWorkflowStageContractDigest(workflow: AutonomousWorkflow, stageId: string): Promise<string> {
  const stage = workflow.stages.find((candidate) => candidate.id === stageId);
  if (!stage) throw new ArgumentError(`autonomous workflow stage is unavailable: ${stageId}`);
  return digestJson(workflowStageContractDescriptor(workflow, stage));
}

export function taskRelevanceTokens(task: string): string[] {
  return [...new Set(normalizeRouteText(task).split(" ").filter((token) => token.length >= 3))].slice(0, 128);
}

export function capabilityCandidateScore(
  tokens: readonly string[],
  requestedCapabilities: readonly string[],
  stage: AutonomousWorkflowStage,
  binding: AutonomousDomainToolBinding,
  domain: AutonomousDomainName,
  toolSelectionState: AutonomousToolSelectionState,
  totalPulls: number,
  exploration: number,
): readonly [number, number, number, number, number] {
  const corpus = normalizeRouteText(`${binding.name} ${binding.capability} ${stage.id} ${stage.objective}`);
  const relevance = tokens.reduce((score, token) => score + (corpus.includes(token) ? 1 : 0), 0);
  const requested = requestedCapabilities.includes(binding.capability) ? 1 : 0;
  const stageExact = stage.required_capabilities.includes(binding.capability) ? 1 : 0;
  const utility = toolSelectionUtility(toolSelectionArmFor(toolSelectionState, domain, stage, binding), totalPulls, exploration);
  return [requested, stageExact, utility, relevance, binding.read_only ? 1 : 0];
}

export function compareCapabilityScores(left: readonly number[], right: readonly number[]): number {
  for (let index = 0; index < left.length; index += 1) {
    const difference = (right[index] ?? 0) - (left[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}
