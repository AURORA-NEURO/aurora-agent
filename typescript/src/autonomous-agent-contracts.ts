/** Public autonomous-agent request, result, and lifecycle contracts. */

import type {
  AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA,
  AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA,
} from "./autonomous-cross-domain-receipt.js";
import type {
  AgentMissionStep,
  AutonomousCrossDomainPlanRefinementResult,
  AutonomousPlanRefinementResult,
  AutonomousPlanningFailureProjection,
  BrainBanditContext,
  BrainModelSelectionContext,
  JsonObject,
  JsonValue,
} from "./types.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import type { AutonomousModelInventoryReadiness } from "./autonomous-model-inventory.js";
import type { AutonomousCapabilityActivation, AutonomousCapabilityActivationState } from "./autonomous-activation.js";
import type {
  AutonomousCostBudget,
  AutonomousExecutionPlan,
  AutonomousModelCandidate,
  AutonomousModelCandidateDefaults,
  AutonomousModelObservation,
  AutonomousModelSelectionTraceEventCallback,
  AutonomousModelSelector,
  AutonomousProviderFailoverProjection,
  AutonomousProviderInvocationReceipt,
  AutonomousSelectionDecision,
  AutonomousSelectionWeights,
  AutonomousStreamCompletion,
  CredentialHandle,
  LLMRuntimeHealthPersistenceCoordinator,
  ProviderContentPart,
  ProviderInvocationObserver,
  ProviderMessage,
  ProviderModelDiscovery,
  ProviderResponse,
  ProviderStreamEvent,
  ProviderTool,
  ProviderToolCall,
  ProviderToolResult,
  ProviderTransportDispatchContext,
  ProviderTransportDispatchFence,
} from "./llm.js";
import type { AutonomousContextBudgetOptions, AutonomousContextBudgetPlan } from "./autonomous-context-budget.js";
import type { AutonomousTaskDecision } from "./autonomous-task-decision.js";
import type { AutonomousEvidencePlanJSON } from "./autonomous-evidence.js";
import type {
  AutonomousDomainPolicy,
  AutonomousDomainPolicyAdmission,
  AutonomousDomainPolicyExecutionMode,
} from "./autonomous-domain-policy.js";
import type { AutonomousDomainTaskLens } from "./autonomous-task-lens.js";
import type { AutonomousTaskIntent } from "./autonomous-task-intent.js";
import type { AutonomousCapabilityRoute } from "./autonomous-capability-routing.js";
import type {
  AutonomousDomainResponseContract,
  AutonomousDomainResponseEvaluation,
} from "./autonomous-domain-response.js";
import type {
  AutonomousCapabilityLearningBatchResult,
  AutonomousCapabilityLearningSettlement,
  AutonomousCapabilityLearningSettlementStore,
} from "./autonomous-capabilities.js";
import type { ProviderErrorCode } from "./errors.js";
import type { AutonomousSemanticRouteResult } from "./autonomous-routing.js";
import type { AutonomousModelContinuationPlan } from "./autonomous-continuation.js";
import type {
  AutonomousPromptAdaptiveSelectionJSON,
  AutonomousPromptLearningState,
  AutonomousPromptLearningStateJSON,
  AutonomousPromptRegistry,
  AutonomousPromptRenderResult,
  AutonomousPromptSelectionPlan,
  AutonomousPromptSelectionPlanJSON,
  AutonomousPromptTemplate,
} from "./autonomous-prompt-registry.js";
import type {
  AutonomousEvidenceExecutionOptions,
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionPrepareOptions,
  AutonomousEvidenceExecutionResult,
} from "./autonomous-evidence-execution.js";
import type { AutonomousEvidenceAdapterRegistry } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceAcquisitionRequest } from "./autonomous-evidence-runtime.js";
import type {
  AutonomousEvidenceExecutionCheckpointStore,
  AutonomousEvidenceExecutionReconciliationAuthorityIdentity,
  AutonomousEvidenceExecutionReconciliationReceiptJSON,
  AutonomousEvidenceExecutionResumableOptions,
  AutonomousEvidenceExecutionResumablePolicyIdentity,
} from "./autonomous-evidence-execution-resumable.js";
import type {
  AUTONOMOUS_GOAL_RETENTION,
  AUTONOMOUS_GOAL_STEP_SCHEMA,
  AutonomousGoalRecord,
  AutonomousGoalStatus,
} from "./autonomous-goals.js";
import type { AutonomousCrossDomainReplanCycleResult, AutonomousReplanCycleResult } from "./autonomous-cycle.js";
import type {
  AutonomousCrossDomainResponseAlignmentInput,
  AutonomousCrossDomainResponseAssessment,
} from "./autonomous-cross-domain-response.js";
import type {
  AutonomousBrainControlPlaneBridge,
  AutonomousModelHealthPersistenceCoordinator,
  AutonomousModelHealthStore,
} from "./autonomous-control.js";
import type { ApiClient } from "./client.js";
import type { ToolCatalogue } from "./tooling.js";
import type { AutonomousEffectBoundary, AutonomousEffectExecutionContext } from "./autonomous-effects.js";
import type {
  AutonomousCapabilityJournalPersistenceCoordinator,
  AutonomousCapabilityJournalStore,
} from "./autonomous-capability-persistence.js";
import type {
  AutonomousExecutionController,
  AutonomousExecutionPersistenceCoordinator,
  AutonomousExecutionSnapshotJournal,
} from "./autonomous-execution.js";
import type { AutonomousDecisionCyclePersistenceCoordinator } from "./autonomous-decision-persistence.js";
import type { AutonomousOnlineLearnerPersistenceCoordinator } from "./autonomous-online-learner-persistence.js";
import type { AutonomousSelectionPromotionLifecycle } from "./autonomous-selection-lifecycle.js";
import type {
  AutonomousEvaluatorCalibrationRegistry,
  AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator,
} from "./autonomous-evaluator-calibration-store.js";
import type {
  AutonomousEpisodicMemoryStore,
  AutonomousMemoryPersistenceCoordinator,
  AutonomousMemoryQuery,
} from "./autonomous-memory.js";
import type {
  AutonomousMemoryConsolidator,
  AutonomousMemoryLessonContextResolver,
} from "./autonomous-memory-consolidation.js";
import type { AutonomousPromptLearningPersistenceCoordinator } from "./autonomous-prompt-learning-persistence.js";
import type { AutonomousToolSelectionPersistence } from "./autonomous-tool-selection-persistence.js";
import type { AutonomousConnectorRegistry, AutonomousConnectorRuntime } from "./autonomous-connectors.js";
import type { AutonomousEvidenceAdapterHealthStore } from "./autonomous-evidence-adapter-health.js";
import type { AutonomousAuthorizationContext } from "./autonomous-authorization.js";
import type { AutonomousLearningController } from "./autonomous-learning.js";
import type { AutonomousMissionReplanOptions } from "./mission-replan.js";
import type {
  AutonomousMissionCheckpointStore,
  AutonomousMissionExecuteOptions,
  AutonomousMissionResultStore,
  AutonomousMissionStepResult,
} from "./mission-execution.js";
import type { AutonomousRunTraceStore, AutonomousRunTraceSummary } from "./autonomous-run-trace.js";
import type {
  AUTONOMOUS_AUTO_RUN_SCHEMA,
  AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA,
  AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
  AUTONOMOUS_CROSS_DOMAIN_SCHEMA,
  AUTONOMOUS_DOMAIN_PACK_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_SCHEMA,
  AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
  AUTONOMOUS_GOAL_LEARNING_SCHEMA,
  AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA,
  AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
  AUTONOMOUS_MODEL_REFRESH_SCHEMA,
  AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA,
  AUTONOMOUS_PLAN_AND_RUN_SCHEMA,
  AUTONOMOUS_PLAN_SCHEMA,
  AUTONOMOUS_PROMPT_SCHEMA,
  AUTONOMOUS_READINESS_SCHEMA,
  AUTONOMOUS_ROUTE_SCHEMA,
  AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
  AUTONOMOUS_RUN_STREAM_SCHEMA,
  AUTONOMOUS_TOOL_SELECTION_POLICY,
  AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
  AUTONOMOUS_WORKFLOW_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
  AUTONOMY_SCHEMA,
} from "./autonomous.js";

import type { AutonomousOnlineLearner } from "./autonomous-online-learning.js";

export type AutonomousRouteReason = "routed" | "cross_domain" | "no_matching_evidence" | "insufficient_confidence" | "insufficient_margin";

export interface AutonomousWorkflowStage extends JsonObject {
  id: string;
  objective: string;
  required_capabilities: string[];
  depends_on: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  read_only: boolean;
  approval_required: boolean;
}

/** Explicit identity carried from a reviewed workflow stage into live tool admission. */
export interface AutonomousWorkflowToolContext extends JsonObject {
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  /** Optional digest-bound stage packet supplied by workflow executors. */
  stage_plan_digest?: string;
  /** Exact reviewed stage contract digest; live dispatch rejects stale values. */
  stage_contract_digest?: string;
  /** Tool portfolio selected by the stage packet; omission means legacy domain admission. */
  selected_tool_names?: string[];
}

export interface AutonomousWorkflow extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_SCHEMA;
  workflow_id: string;
  domain: AutonomousDomainName;
  stages: AutonomousWorkflowStage[];
  route_intents: string[];
  evaluator_signals: string[];
  completion_contract: string;
  workflow_digest: string;
  execution: "strategy_metadata_only";
}

export interface AutonomousDomainToolBinding extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_TOOL_SCHEMA;
  name: string;
  domains: AutonomousDomainName[];
  capability: string;
  risk_class: "read_only" | "reversible_effect" | "external_effect" | "high_impact_effect";
  read_only: boolean;
  approval_required: boolean;
  authorization: "metadata_only; registration_is_not_authorization";
  secret_material: "never_returned";
}

/** Ordered risk ceiling used by provider-free tool portfolio selection. */
export type AutonomousToolRiskClass = AutonomousDomainToolBinding["risk_class"];

/** Metadata-only evidence emitted by the domain adapter boundary; raw arguments/results never enter it. */
export interface AutonomousDomainToolExecutionReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA;
  receipt_kind: "tool_execution_receipt";
  /** Provider call identity; retained so evaluator batches can reject ambiguous replays. */
  call_id?: string;
  /** Caller execution identity; null means the receipt was not attached to an execution journal. */
  execution_id?: string | null;
  /** Digest of the bounded arguments accepted by the catalogue; arguments themselves never persist. */
  arguments_digest?: string;
  domain: AutonomousDomainName | null;
  workflow_id: string | null;
  workflow_digest: string | null;
  stage_id: string | null;
  stage_contract_digest: string | null;
  stage_plan_digest?: string | null;
  required_evidence_outputs: string[];
  evidence_status: "tool_execution_only";
  does_not_claim: string[];
  tool: string;
  capability: string | null;
  status: "approval_required" | "authorization_required" | "executed" | "reconciliation_required" | "execution_failed";
  schema_digest?: string;
  result_digest?: string;
  effect?: string;
  effect_id?: string;
  idempotency_key?: string;
  error_class?: string;
  duration_ms: number;
  secret_material: "never_returned";
}

export interface AutonomousDomainToolProfile extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_TOOL_SCHEMA;
  domain: AutonomousDomainName;
  description: string;
  bindings: AutonomousDomainToolBinding[];
  execution: "metadata_only; no_live_catalogue_assumption";
}

export interface AutonomousDomainProfile extends JsonObject {
  schema: typeof AUTONOMY_SCHEMA;
  domain: AutonomousDomainName;
  risk_class: string;
  default_capability: string;
  required_model_capabilities: string[];
  capabilities: string[];
  guardrails: string[];
  system_instructions: string;
  evaluator_domain: "engineering" | "research" | "operations" | "data" | "biomedical";
  workflow: AutonomousWorkflow;
  tool_profile: AutonomousDomainToolProfile;
  execution: "strategy_metadata_only";
}

export interface AutonomousRouteCandidate extends JsonObject {
  domain: AutonomousDomainName;
  score: number;
  matched_terms: string[];
  capability: string;
  risk_class: string;
  workflow_id: string;
  evidence: "fixed_catalogue_term_matches_only" | "provider_semantic_candidate";
}

export interface AutonomousRouteProposal extends JsonObject {
  schema: typeof AUTONOMOUS_ROUTE_SCHEMA;
  task_digest: string;
  candidates: AutonomousRouteCandidate[];
  selected_domains: AutonomousDomainName[];
  primary_domain: AutonomousDomainName | null;
  confidence: number;
  abstained: boolean;
  reason: AutonomousRouteReason;
  cross_domain: boolean;
  source: "deterministic_vocabulary" | "provider_semantic_hybrid";
  route_digest: string;
  retention: "route_scores_and_digests_only; task_text_is_not_retained_in_route";
  does_not_claim: string[];
}

export interface AutonomousPromptChunk {
  id: string;
  content: string;
  required?: boolean;
  priority?: number;
}

export interface AutonomousPromptMessage extends JsonObject {
  role: "system" | "developer" | "user";
  content: string;
  source_id: string;
}

export interface AutonomousPromptResult extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_SCHEMA;
  messages: AutonomousPromptMessage[];
  included_context_ids: string[];
  omitted_context_ids: string[];
  estimated_input_tokens: number;
  complete: boolean;
  prompt_digest: string;
  warnings: string[];
}

export interface AutonomousPlanStep extends JsonObject {
  id: string;
  objective: string;
  tool: string;
  arguments: JsonObject;
  depends_on: string[];
  effect: "read_only" | "provider_call" | "external_write" | "irreversible";
  estimated_cost: number;
}

export interface AutonomousPlan extends JsonObject {
  schema: typeof AUTONOMOUS_PLAN_SCHEMA;
  objective: string;
  workflow_id: string;
  workflow_digest: string;
  ordered_step_ids: string[];
  steps: AutonomousPlanStep[];
  /** Deterministic dependency-closed batches for caller-owned scheduling. */
  execution_waves: string[][];
  critical_path_cost: number;
  max_parallelism: number;
  estimated_parallel_rounds: number;
  peak_parallelism: number;
  allowed_tools: string[];
  estimated_cost: number;
  requires_approval: boolean;
  execution: "not_started";
  /** Optional digest of the structured domain response contract bound to this plan. */
  response_contract_digest?: string;
  /** Digest of the provider-free domain policy bound to this plan. */
  domain_policy_digest: string;
  /** Digest of the provider-free task intent bound to this plan. */
  task_intent_digest: string;
  /** Digest of the intent-to-action decision bound to this plan. */
  task_decision_digest: string;
  plan_digest: string;
  does_not_claim: string[];
}

export interface AutonomousDomainPack extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_PACK_SCHEMA;
  domain: AutonomousDomainName;
  pack_id: string;
  pack_version: string;
  workflow_id: string;
  evaluator_domain: string;
  model_capabilities: string[];
  tool_capabilities: string[];
  evidence_requirements: string[];
  planning_principles: string[];
  review_triggers: string[];
  pack_digest: string;
  execution: "planning_only; dispatch_requires_caller_approval";
  credential_posture: "caller_supplied_opaque_handle_not_returned";
}

export type AutonomousReadinessState =
  | "ready_for_caller_approval"
  | "model_catalogue_required"
  | "provider_registration_required"
  | "credential_required"
  | "model_capability_gap"
  | "partial";

export interface AutonomousReadinessModel extends JsonObject {
  provider: string;
  model: string;
  enabled: boolean;
  provider_registered: boolean;
  credential_ready: boolean;
  compatible_domains: AutonomousDomainName[];
  eligible_domains: AutonomousDomainName[];
}

export interface AutonomousReadinessProvider extends JsonObject {
  provider: string;
  provider_registered: boolean;
  requires_credential: boolean | null;
  credential_ready: boolean;
  circuit: string;
  next_action: string;
  credential: JsonObject;
  health: JsonObject | null;
  secret_material: "never_returned";
}

export interface AutonomousReadinessDomain extends JsonObject {
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  required_model_capabilities: string[];
  compatible_model_count: number;
  eligible_model_count: number;
  required_tool_count: number;
  available_tool_count: number;
  missing_tools: string[];
  learning_context_digest: string;
  evidence_readiness?: JsonObject;
  calibration_admission?: JsonObject;
  selection_promotion?: JsonObject;
  state: AutonomousReadinessState;
  next_actions: string[];
}

export interface AutonomousReadinessReport extends JsonObject {
  schema: typeof AUTONOMOUS_READINESS_SCHEMA;
  providers: AutonomousReadinessProvider[];
  models: AutonomousReadinessModel[];
  domains: AutonomousReadinessDomain[];
  workflows: AutonomousWorkflow[];
  domain_packs: AutonomousDomainPack[];
  model_capability_coverage: JsonObject;
  model_inventory_readiness: AutonomousModelInventoryReadiness;
  model_health: JsonObject;
  learning: JsonObject;
  tooling: JsonObject;
  evidence?: JsonObject;
  connectors?: JsonObject;
  activation: AutonomousCapabilityActivationState;
  next_actions: string[];
  readiness_state: AutonomousReadinessState;
  execution: "not_started; no_provider_or_tool_calls";
  credential_posture: "caller_supplied_opaque_handles";
  secret_material: "never_returned";
  readiness_digest: string;
}

export type AutonomousModelSelectionPreviewStatus = "selected" | "refused_no_eligible_model";

export interface AutonomousModelSelectionPreviewOptions {
  domain: AutonomousDomainName;
  capability?: string;
  context?: readonly AutonomousPromptChunk[];
  candidates?: readonly AutonomousModelCandidate[];
  estimatedInputTokens?: number;
  /** Apply the same explicit context budget used by an eventual invocation. */
  contextBudget?: AutonomousContextBudgetOptions;
  requestedOutputTokens?: number;
  maxCostPerMillionTokens?: number;
  maxLatencyMs?: number;
  minQuality?: number;
  minSelectionConfidence?: number;
  /** Explicit weighted utility policy shared with the Rust brain kernel. */
  selectionWeights?: Partial<AutonomousSelectionWeights>;
  /** Optional value-only observations used by the deterministic preview ranker. */
  selectionObservations?: readonly AutonomousModelObservation[];
}

export interface AutonomousModelSelectionContract extends JsonObject {
  task_digest: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  task_intent_digest: string;
  task_decision_digest: string;
  task_decision_posture: AutonomousTaskDecision["posture"];
  required_model_capabilities: string[];
  candidate_ids: string[];
  input_tokens: number;
  requested_output_tokens: number;
  max_cost_per_million_tokens: number | null;
  max_latency_ms: number | null;
  min_quality: number | null;
  min_selection_confidence: number | null;
  selection_weights: AutonomousSelectionWeights;
  selection_observations_digest: string;
  context_budget: {
    max_input_tokens: number;
    preserve_recent_messages: number;
    max_messages: number;
  } | null;
}

/** Options for approving one previously reviewed model-selection preview. */
export type AutonomousApprovedModelSelectionOptions = Omit<AutonomousRunOptions, "approveProviderCall" | "domain"> & {
  domain: AutonomousDomainName;
};

/** Provider-free projection of the exact selection request that execution would use. */
export interface AutonomousModelSelectionPreview extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA;
  status: AutonomousModelSelectionPreviewStatus;
  task_digest: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  workflow_id: string;
  workflow_digest: string;
  domain_pack_digest: string;
  task_intent_digest: string;
  task_decision_digest: string;
  task_decision_posture: AutonomousTaskDecision["posture"];
  selection_context_digest: string;
  execution_plan_digest: string;
  required_model_capabilities: string[];
  candidate_count: number;
  eligible_candidate_count: number;
  selection_contract: AutonomousModelSelectionContract;
  selection_audit: AutonomousSelectionDecision;
  review: {
    provider_call: "not_started";
    domain_tools: "not_started";
    caller_approval_required: true;
    next_action: "review_selection_and_approve_provider_call" | "resolve_model_provider_or_credential_gates" | "resolve_task_decision_block";
  };
  execution: "preview_only; no_provider_or_domain_tool_invocation";
  authority_posture: "selection_review_only; preview_does_not_authorize_provider_or_effects";
  credential_posture: "caller_opaque_handles_only; no_handles_returned";
  retention: "metadata_only_model_ranking_and_digests";
  secret_material: "never_returned";
}

export interface AutonomousTaskBlueprint extends JsonObject {
  schema: "bioprism-python-autonomous-task/0.1";
  task_digest: string;
  /** Digest of the approved route that shaped this blueprint; route material remains caller-owned. */
  route_digest: string;
  domain_profile: AutonomousDomainProfile;
  domain_pack: AutonomousDomainPack;
  workflow: AutonomousWorkflow;
  evidence_plan: AutonomousEvidencePlanJSON;
  selection_context: BrainModelSelectionContext;
  learning_context_digest: string;
  required_capabilities: string[];
  /** Provider-free bounded limits and approval posture for this domain. */
  domain_policy: AutonomousDomainPolicy;
  /** Domain-specific planning posture; guidance metadata never authorizes execution. */
  task_lens: AutonomousDomainTaskLens;
  /** Provider-free task interpretation; classification metadata never authorizes execution. */
  task_intent: AutonomousTaskIntent;
  /** Intent-to-action posture; guidance metadata never authorizes execution. */
  task_decision: AutonomousTaskDecision;
  /** Provider-free capability selection used to shape this blueprint; never execution authority. */
  capability_route: AutonomousCapabilityRoute;
  /** Digest-bound stage packets used by workflow dispatch and evaluator/checkpoint identity. */
  stage_execution_plans: AutonomousWorkflowStageExecutionPlan[];
  prompt: AutonomousPromptResult;
  plan: AutonomousPlan;
  /** Present only when the caller explicitly enables the reviewed structured domain response. */
  response_contract?: AutonomousDomainResponseContract;
  execution: "not_started";
  credential_posture: "caller_supplied_opaque_handle_not_returned";
}

export interface AutonomousClarificationRecompileProjection extends JsonObject {
  schema: "bioprism-autonomous-task-clarification-recompile/0.1";
  plan_digest: string;
  resolution_digest: string;
  original_task_digest: string;
  recompiled_task_digest: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  recompiled_intent_digest: string;
  recompiled_decision_digest: string;
  execution_plan_digest: string;
  status: "ready";
  recompile_digest: string;
  blueprint: JsonObject;
  execution: "not_started; fresh_blueprint_requires_existing_gates";
  authorization: "recompile_only; provider_source_tool_and_effect_gates_remain_required";
  retention: "metadata_only; task_text_and_answer_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousClarificationRecompileResult {
  schema: "bioprism-autonomous-task-clarification-recompile/0.1";
  plan_digest: string;
  resolution_digest: string;
  original_task_digest: string;
  recompiled_task_digest: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  recompiled_intent_digest: string;
  recompiled_decision_digest: string;
  execution_plan_digest: string;
  status: "ready";
  recompile_digest: string;
  /** Live blueprint; task text and prompt messages remain caller-owned and transient. */
  blueprint: AutonomousTaskBlueprint;
  toJSON(): AutonomousClarificationRecompileProjection;
}

export interface AutonomousCapabilityContract extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA;
  domain: AutonomousDomainName;
  capability: string;
  stage_ids: string[];
  tool_capabilities: string[];
  required_model_capabilities: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  read_only: boolean;
  approval_required: boolean;
  review_triggers: string[];
  fallback_policy: "provider_only_or_blocked" | "provider_only";
  contract_digest: string;
  adapter_posture: "exact_capability_aliases_only";
  credential_posture: "caller_supplied_opaque_handles";
  authority_posture: "metadata_only; no_provider_or_effect_authority";
}

/**
 * Exact runtime handoff for one reviewed workflow stage. This is metadata only: it narrows
 * provider-visible tools and binds evidence/capability identities, but never authorizes a
 * provider call, credential use, tool effect, or external-world claim.
 */
export interface AutonomousWorkflowStageExecutionPlan extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  stage_objective: string;
  required_capabilities: string[];
  tool_capabilities: string[];
  capability_contracts: AutonomousCapabilityContract[];
  required_model_capabilities: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  active_tool_names: string[];
  selected_tool_names: string[];
  withheld_tool_names: string[];
  approval_required: boolean;
  read_only: boolean;
  execution_posture: "approval_gated" | "tool_backed" | "provider_only_or_blocked";
  source_plan_digest: string | null;
  stage_plan_digest: string;
  capability_contract_digests: string[];
  credential_posture: "caller_supplied_opaque_handles; no_keys_or_handles";
  authority_posture: "metadata_only; stage_plan_does_not_grant_authority";
}

export interface AutonomousCrossDomainSubtask {
  id?: string;
  task: string;
  domain: AutonomousDomainName;
  capability?: string;
  context?: AutonomousPromptChunk[];
}

export interface AutonomousCrossDomainBlueprint {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_SCHEMA;
  task_digest: string;
  /** Digest of the reviewed parent route shared by every child and synthesis blueprint. */
  route_digest: string;
  child_ids: string[];
  child_blueprints: AutonomousTaskBlueprint[];
  synthesis_blueprint: AutonomousTaskBlueprint;
  dependency_graph: {
    fan_out: Array<{ id: string; task_digest: string; domain: AutonomousDomainName }>;
    fan_in: string;
  };
  plan_digest: string;
  execution: "not_started";
  authorization: "caller_approval_per_provider_or_effect_boundary";
}

export interface AutonomousAutoBlueprint {
  schema: "bioprism-python-autonomous-auto-blueprint/0.1";
  route: AutonomousRouteProposal;
  blueprint: AutonomousTaskBlueprint | null;
  cross_domain_blueprint?: AutonomousCrossDomainBlueprint | null;
  capability_route?: AutonomousCapabilityRoute | null;
  execution: "not_started";
  authorization: "route_and_plan_only; no_provider_or_tool_effects_authorized";
}

export interface AutonomousDomainToolCoverage extends JsonObject {
  domain: AutonomousDomainName;
  required_tool_count: number;
  available_tool_count: number;
  missing_tools: string[];
  review_required_tools: string[];
  coverage_ratio: number;
}

export interface AutonomousDomainToolPlan extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA;
  catalogue_digest: string;
  profile_digest: string;
  domains: AutonomousDomainName[];
  available_curated_tools: string[];
  missing_curated_tools: string[];
  review_required_tools: string[];
  unclassified_tools: string[];
  coverage: AutonomousDomainToolCoverage[];
  proposed_bindings: AutonomousDomainToolBinding[];
  review_bindings: AutonomousDomainToolBinding[];
  plan_digest: string;
  execution: "metadata_only; registration_is_not_authorization";
  secret_material: "never_returned";
}

export type AutonomousCapabilitySelectionStatus = "selected" | "activation_required" | "catalogue_missing" | "provider_only" | "capacity_limited" | "learning_disabled" | "risk_budget_blocked";

export interface AutonomousToolSelectionArm extends JsonObject {
  arm_id: string;
  pulls: number;
  reward_sum: number;
  failures: number;
  latency_ms: number | null;
  disabled: boolean;
}

/** Idempotency metadata for one evaluator-approved value-only tool credit. */
export interface AutonomousToolSelectionCredit extends JsonObject {
  outcome_digest: string;
  arm_id: string;
  reward: number;
  failed: boolean;
  latency_ms: number | null;
}

export interface AutonomousToolSelectionState extends JsonObject {
  schema: typeof AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA;
  generation: number;
  arms: AutonomousToolSelectionArm[];
  credited_outcomes: AutonomousToolSelectionCredit[];
}

export interface AutonomousToolSelectionOutcome {
  domain: AutonomousDomainName;
  capability: string;
  tool: string;
  reward: number;
  failed?: boolean;
  latencyMs?: number | null;
  /** Evaluator settlement identity; repeated identities are idempotent. */
  outcomeDigest?: string | null;
}

/** Capability-learning result with the updated adaptive tool-selection state attached. */
export interface AutonomousAgentCapabilityLearningResult extends AutonomousCapabilityLearningSettlement {
  tool_selection_state: AutonomousToolSelectionState;
  tool_selection_state_digest: string;
}

/** Ordered capability-learning batch with the updated adaptive tool-selection state attached. */
export interface AutonomousAgentCapabilityLearningBatchResult extends AutonomousCapabilityLearningBatchResult {
  tool_selection_state: AutonomousToolSelectionState;
  tool_selection_state_digest: string;
}

export interface AutonomousCapabilityPlanCoverage extends JsonObject {
  domain: AutonomousDomainName;
  stage_id: string;
  required_capabilities: string[];
  candidate_tool_names: string[];
  selected_tool: string | null;
  selected_capability: string | null;
  approval_required: boolean;
  selected_arm_id: string | null;
  selection_utility: number | null;
  candidate_ranking: AutonomousCapabilityCandidateRanking[];
  selection_rationale: "highest_ranked_eligible_candidate" | "portfolio_reuse_lower_rank_candidate" | "no_reviewed_binding_for_stage" | "no_live_catalogue_binding" | "activation_or_allowlist_required" | "all_candidates_learning_disabled" | "risk_budget_excluded_all_candidates" | "portfolio_capacity_limit";
  status: AutonomousCapabilitySelectionStatus;
}

export interface AutonomousCapabilityPlanOmission extends JsonObject {
  name: string;
  domains: AutonomousDomainName[];
  capability: string;
  reason: "not_required_for_reviewed_workflow" | "activation_required" | "capacity_limited" | "duplicate_binding" | "learning_disabled" | "risk_budget_limited";
}

export type AutonomousCapabilityCandidateReason = "eligible" | "not_allowed" | "risk_budget_exceeded" | "read_only_required" | "approval_required" | "learning_disabled";

/** Candidate-level, value-only explanation for one reviewed workflow stage. */
export interface AutonomousCapabilityCandidateRanking extends JsonObject {
  tool: string;
  capability: string;
  risk_class: AutonomousToolRiskClass;
  read_only: boolean;
  approval_required: boolean;
  eligible: boolean;
  rank: number | null;
  requested_capability_match: boolean;
  stage_capability_match: boolean;
  selection_utility: number;
  task_relevance: number;
  observed_pulls: number;
  observed_failure_rate: number;
  reason: AutonomousCapabilityCandidateReason;
}

/** Deterministic task-to-capability selection; this is a tool portfolio, never authorization. */
export interface AutonomousCapabilityPlan extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_PLAN_SCHEMA;
  task_digest: string;
  catalogue_digest: string | null;
  profile_digest: string;
  domains: AutonomousDomainName[];
  requested_capabilities: string[];
  max_tools: number;
  selected_tool_names: string[];
  /** Selection order is retained separately from the sorted set for stage execution. */
  selected_tool_order: string[];
  selected_bindings: AutonomousDomainToolBinding[];
  approval_required_tools: string[];
  missing_tools: string[];
  omissions: AutonomousCapabilityPlanOmission[];
  coverage: AutonomousCapabilityPlanCoverage[];
  selection_learning: JsonObject;
  selection_constraints: JsonObject;
  selection_policy: typeof AUTONOMOUS_TOOL_SELECTION_POLICY;
  execution: "metadata_only; no_provider_or_tool_calls";
  authorization: "selection_does_not_authorize_tools_or_effects";
  secret_material: "never_returned";
  plan_digest: string;
}

export type AutonomousRunStatus = "completed" | "route_review_required" | "approval_required" | "policy_review_required" | "policy_blocked" | "reconciliation_required" | "turn_limit_reached" | "abstained" | "cross_domain_partial" | "child_failed" | "response_review_required";

/**
 * Safe metadata for a provider/credential boundary failure captured inside a parent fan-out.
 * Error messages and provider payloads are intentionally absent: provider implementations may
 * include sensitive diagnostics in exception text, and a child failure must be safe to persist
 * in the parent execution receipt.
 */
export interface AutonomousProviderFailureProjection extends AutonomousPlanningFailureProjection {
  code: ProviderErrorCode | "credential";
}

export type AutonomousToolLoopStatus = "completed" | "authorization_required" | "reconciliation_required" | "turn_limit_reached";

export interface AutonomousToolLoopSummary {
  status: AutonomousToolLoopStatus;
  turns: number;
  toolCalls: number;
}

export interface AutonomousRunResult {
  schema: "bioprism-typescript-autonomous-run/0.1";
  status: AutonomousRunStatus;
  route: AutonomousRouteProposal;
  /** Optional provider-assisted routing proposal used by the high-level execution path. */
  semantic_route?: AutonomousSemanticRouteResult | null;
  blueprint: AutonomousTaskBlueprint | null;
  /** Digest of the explicitly accepted provider planning proposal that shaped invocation. */
  plan_refinement_digest: string | null;
  selection: AutonomousSelectionDecision | null;
  response: ProviderResponse | null;
  /** Metadata-only transport receipts for the provider turns used by this run. */
  provider_invocations?: AutonomousProviderInvocationReceipt[];
  /** Metadata-only failover projection; null when the selected provider completed directly. */
  provider_failover?: AutonomousProviderFailoverProjection | null;
  /** Metadata-only prompt-history compaction receipt, when an explicit budget was configured. */
  context_budget?: AutonomousContextBudgetPlan | null;
  /** Exact bounded model fallback ladder used by the provider invocation, when one was compiled. */
  continuation_plan?: AutonomousModelContinuationPlan | null;
  /** Digest-only identity for an explicitly selected versioned prompt; rendered messages remain transient. */
  prompt?: AutonomousRunPromptProjection | null;
  /** Deterministic value-only response composition signal; never task truth or effect evidence. */
  response_evaluation?: AutonomousDomainResponseEvaluation | null;
  /** Redacted provider-boundary failure captured by a parent cross-domain fan-out. */
  failure?: AutonomousProviderFailureProjection | null;
  tool_loop?: AutonomousToolLoopSummary | null;
  cross_domain?: AutonomousCrossDomainRunResult | null;
  /** Optional value-only episodic-memory projection; absent when memory is not configured. */
  memory?: AutonomousMemoryRunProjection | null;
  /** Pending value-only learning episode prepared only after a completed provider run. */
  learning_episode_id?: string | null;
  learning_episode_status?: "prepared" | "not_eligible" | "failed";
  learning_error_class?: string | null;
  /** Independent pending episode for the reviewed structured-response contract signal. */
  response_learning_episode_id?: string | null;
  response_learning_episode_status?: "prepared" | "not_eligible" | "failed";
  response_learning_error_class?: string | null;
  /** Strict-mode provider-free admission; absent for ordinary audit-mode runs. */
  domain_policy_admission?: AutonomousDomainPolicyAdmission | null;
  learning: "provider_health_feedback_only" | "online_bandit_feedback_available";
  retention: "provider_response_local; value_only_learning_projection";
}

export interface AutonomousRunPromptProjection extends JsonObject {
  mode: "versioned_template" | "registry_selection";
  prompt_id: string;
  version: string;
  domain: AutonomousDomainName;
  stage: string;
  manifest_digest: string;
  rendered_prompt_digest: string;
  final_prompt_digest: string;
  selection_plan_digest: string | null;
  adaptive_selection_digest?: string | null;
  adaptive_arm_id?: string | null;
  adaptive_generation?: number | null;
  /** Exact registry-bound adaptive selection receipt for explicit evaluator settlement. */
  adaptive_selection?: AutonomousPromptAdaptiveSelectionJSON;
  selection_policy?: string | null;
  retention: "prompt_messages_transient;digest_only_projection";
  secret_material: "never_returned";
}

/**
 * The only memory state attached to a run result. Episode metadata and digests are safe to
 * persist; task text, prompts, provider responses, credentials, and tool payloads never cross
 * this projection boundary.
 */
export interface AutonomousMemoryRunProjection extends JsonObject {
  status: "retrieved" | "retrieval_failed" | "recorded" | "record_failed" | "disabled";
  retrieved_episode_ids: string[];
  retrieved_episode_digests: string[];
  retrieval_digest: string | null;
  /** Stable evaluator-gated lesson identities recalled for this run; lesson text is never retained here. */
  consolidated_lesson_ids: string[];
  consolidated_lesson_digests: string[];
  consolidated_retrieval_digest: string | null;
  recorded_episode_id: string | null;
  recorded_episode_digest: string | null;
  record_event_digest: string | null;
  error_class: string | null;
  retention: "value_only_episode_metadata;transient_task_and_provider_payloads_not_retained";
  secret_material: "never_returned";
}

export type AutonomousEvidenceBackedRunStatus =
  | "evidence_review_required"
  | "evidence_blocked"
  | "evidence_failed"
  | "evidence_incomplete"
  | AutonomousPlanAndRunStatus;

/** Execution modes for the evidence-to-agent bridge. */
export type AutonomousEvidenceExecutionMode = "domain" | "cross_domain" | "auto";

/** Explicit transient bridge input for callers that want to project raw evidence into a prompt. */
export interface AutonomousEvidencePromptProjection {
  executionPlan: AutonomousEvidenceExecutionPlan;
  evidence: AutonomousEvidenceExecutionResult;
  values: Readonly<Record<string, JsonValue | null>>;
}

export type AutonomousEvidencePromptBuilder = (
  projection: AutonomousEvidencePromptProjection,
) => readonly AutonomousPromptChunk[] | Promise<readonly AutonomousPromptChunk[]>;

export interface AutonomousEvidenceBackedRunPreflight {
  executionPlan: AutonomousEvidenceExecutionPlan;
  evidence: AutonomousEvidenceExecutionResult;
  promptContext: readonly AutonomousPromptChunk[];
}

export type AutonomousEvidenceBackedRunPreflightHook = (
  preflight: AutonomousEvidenceBackedRunPreflight,
) => void | Promise<void>;

/** @internal Privileged transport fence used by restart-safe evidence controllers. */
export type AutonomousEvidenceBackedProviderDispatchHook = (
  preflight: AutonomousEvidenceBackedRunPreflight,
  dispatch: ProviderTransportDispatchContext,
) => void | Promise<void>;

export interface AutonomousEvidenceBackedRunOptions {
  registry: AutonomousEvidenceAdapterRegistry;
  domains?: readonly AutonomousDomainName[];
  requests: readonly AutonomousEvidenceAcquisitionRequest[];
  availableEvidence?: readonly string[];
  completedStages?: Readonly<Record<string, readonly string[]>>;
  prepare?: AutonomousReviewedEvidencePreparationOptions;
  execute?: AutonomousEvidenceExecutionOptions;
  /** Select the provider handoff shape; defaults to the historical single-domain handoff. */
  runMode?: AutonomousEvidenceExecutionMode;
  /** Normal agent options; automatic mode may additionally supply reviewed planning controls. */
  run?: AutonomousAutoRunOptions;
  /** Additional bounded controls for cross-domain fan-out and synthesis. */
  crossDomain?: Pick<AutonomousCrossDomainRunOptions, "subtasks" | "allowPartial" | "synthesize" | "maxParallelChildren" | "responseAlignments" | "requireResponseAlignment" | "minimumResponseReward" | "minimumResponseAlignmentConfidence" | "responseContradictionConfidenceThreshold">;
  /** Defaults to a metadata-only context. This callback is the explicit transient value bridge. */
  promptBuilder?: AutonomousEvidencePromptBuilder;
  /** Prepare caller-owned provider-bound metadata after evidence/prompt projection is settled. */
  beforeProviderRun?: AutonomousEvidenceBackedRunPreflightHook;
  /** Commit a durable dispatch transaction immediately before every actual provider transport. */
  beforeProviderDispatch?: AutonomousEvidenceBackedProviderDispatchHook;
  /** Rehydrate an already-completed caller-owned provider result without invoking a provider. */
  providerRunOverride?: AutonomousRunResult;
  /** Rehydrate an already-completed automatic envelope without invoking planning or execution. */
  automaticRunOverride?: AutonomousAutoRunResult;
  /** Rehydrate an already-completed cross-domain fan-out without invoking a provider. */
  crossDomainRunOverride?: AutonomousCrossDomainRunResult;
  /** Permit synthesis with unsettled evidence; the outer run remains `evidence_incomplete`. */
  allowIncompleteEvidence?: boolean;
  /** Optional job-level source checkpoint; source approval and provider approval remain separate. */
  evidenceCheckpointStore?: AutonomousEvidenceExecutionCheckpointStore;
  /** Required when evidenceCheckpointStore is configured; stable caller-owned source job identity. */
  evidenceJobId?: string;
  /** Stable deployment-owned reconciliation trust root; pass it before the first source checkpoint. */
  evidenceReconciliationAuthority?: AutonomousEvidenceExecutionReconciliationAuthorityIdentity;
  /** Stable callback and mutable-boundary identities for the checkpointed evidence execution. */
  evidenceExecutionPolicyIdentity?: AutonomousEvidenceExecutionResumablePolicyIdentity;
  /** Typed authority-bound decision for an uncertain checkpointed source dispatch. */
  evidenceReconciliationReceipt?: AutonomousEvidenceExecutionReconciliationReceiptJSON;
  /** Explicitly resolve an uncertain source-dispatch checkpoint before retrying. */
  evidenceResumeAfterReconciliation?: boolean;
}

export interface AutonomousEvidenceBackedRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA;
  status: AutonomousEvidenceBackedRunStatus;
  run_mode: AutonomousEvidenceExecutionMode;
  task_digest: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  evidence_result_digest: string | null;
  prompt_projection_digest: string | null;
  run_status: AutonomousRunStatus | null;
  cross_domain_run_status: AutonomousCrossDomainRunStatus | null;
  automatic_status: AutonomousPlanAndRunStatus | null;
  automatic_route_digest: string | null;
  automatic_next_action: AutonomousAutoRunNextAction | null;
  selection_digest: string | null;
  response_digest: string | null;
  retention: "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned";
  secret_material: "never_returned";
  result_digest: string;
}

/**
 * End-to-end evidence-backed execution. The execution plan is always returned for review;
 * source dispatch requires `execute.approveSourceDispatch`, evidence must complete unless the
 * caller explicitly opts into incomplete evidence, and provider invocation still uses the
 * ordinary model/credential/tool/effect approval gates. `toJSON()` excludes raw values and the
 * provider response even though both remain available transiently to the caller.
 */
export interface AutonomousEvidenceBackedRunResult {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA;
  status: AutonomousEvidenceBackedRunStatus;
  run_mode: AutonomousEvidenceExecutionMode;
  task_digest: string;
  execution_plan: AutonomousEvidenceExecutionPlan;
  evidence: AutonomousEvidenceExecutionResult | null;
  prompt_context: readonly AutonomousPromptChunk[];
  run: AutonomousRunResult | null;
  cross_domain_run: AutonomousCrossDomainRunResult | null;
  automatic: AutonomousAutoRunResult | null;
  toJSON(): AutonomousEvidenceBackedRunProjection;
}

/** One bounded autonomous attempt plus its durable, value-only objective projection. */
export interface AutonomousGoalStepResult {
  schema: typeof AUTONOMOUS_GOAL_STEP_SCHEMA;
  goal: AutonomousGoalRecord;
  result: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  result_status: string;
  goal_status: AutonomousGoalStatus;
  outcome_digest: string;
  evaluator_digest: string | null;
  learning_state_digest: string | null;
  progress_digest: string | null;
  retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

/** Goal settlement plus the bounded evaluator/learning cycle that produced it. */
export interface AutonomousGoalLearningStepResult {
  schema: typeof AUTONOMOUS_GOAL_LEARNING_SCHEMA;
  goal: AutonomousGoalRecord;
  result: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  result_status: string;
  goal_status: AutonomousGoalStatus;
  outcome_digest: string;
  evaluator_digest: string | null;
  learning_state_digest: string | null;
  progress_digest: string | null;
  learning_mode: "single_domain_replan" | "cross_domain_replan";
  cycle: AutonomousReplanCycleResult | AutonomousCrossDomainReplanCycleResult;
  retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousCrossDomainChildRun {
  id: string;
  domain: AutonomousDomainName;
  task_digest: string;
  result: AutonomousRunResult;
  output_digest: string | null;
  output_bytes: number;
}

export type AutonomousCrossDomainRunStatus = "completed" | "children_completed" | "children_partial" | "approval_required" | "policy_review_required" | "policy_blocked" | "reconciliation_required" | "turn_limit_reached" | "child_failed" | "route_review_required" | "response_review_required";

export type AutonomousCrossDomainExecutionNextAction = "review_route" | "approve_child" | "reconcile_child" | "retry_child" | "synthesize" | "approve_synthesis" | "reconcile_synthesis" | "inspect_synthesis_failure" | "inspect_partial_synthesis" | "review_response_gate" | "complete";

/**
 * Value-only operational projection for cross-domain execution.
 *
 * This receipt deliberately contains statuses, identifiers, and digests only. It is safe to
 * persist for UI progress, evaluator admission, replay coordination, and restart recovery;
 * provider responses, prompts, credentials, and tool payloads remain caller-owned.
 */
export interface AutonomousCrossDomainExecutionReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA;
  status: AutonomousCrossDomainRunStatus;
  execution_child_ids: string[];
  child_domains: Record<string, AutonomousDomainName>;
  child_statuses: Record<string, string>;
  child_result_digests: Record<string, string | null>;
  completed_child_ids: string[];
  incomplete_child_ids: string[];
  synthesis_status: string | null;
  synthesis_result_digest: string | null;
  completed_units: number;
  total_units: number;
  progress: number;
  next_action: AutonomousCrossDomainExecutionNextAction;
  safe_to_synthesize: boolean;
  reconciliation_required: boolean;
  receipt_digest: string;
  retention: "status_and_outcome_digests_only; provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousCrossDomainRunResult {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA;
  status: AutonomousCrossDomainRunStatus;
  route: AutonomousRouteProposal;
  /** Optional provider-assisted routing proposal used by the high-level execution path. */
  semantic_route?: AutonomousSemanticRouteResult | null;
  blueprint: AutonomousCrossDomainBlueprint | null;
  child_runs: AutonomousCrossDomainChildRun[];
  synthesis: AutonomousRunResult | null;
  completed_children: number;
  total_children: number;
  partial: boolean;
  plan_refinement_digest: string | null;
  /** Optional value-only episodic-memory projection; absent when memory is not configured. */
  memory?: AutonomousMemoryRunProjection | null;
  learning_episode_ids: string[];
  /** Separate structural-response episodes; never task correctness or external-world truth. */
  response_learning_episode_ids?: string[];
  learning: "provider_health_feedback_only" | "online_bandit_feedback_available";
  retention: "provider_responses_local; child_digests_only_in_synthesis_metadata";
  /** Value-only execution/recovery projection; provider payloads never enter this receipt. */
  execution_receipt?: AutonomousCrossDomainExecutionReceipt;
  /** Per-domain strict-mode admissions; absent for ordinary audit-mode runs. */
  domain_policy_admissions?: Record<string, AutonomousDomainPolicyAdmission>;
  /** Digest-only specialist/synthesis structural admission; provider values remain transient. */
  response_assessment?: AutonomousCrossDomainResponseAssessment | null;
}

export interface AutonomousAgentOptions {
  selector?: AutonomousModelSelector;
  /** Optional caller-owned persisted health ledger used for selection and invocation telemetry. */
  modelHealthStore?: AutonomousModelHealthStore;
  /** Optional CAS-fenced persistence coordinator bound to `modelHealthStore` for restart-safe health. */
  modelHealthPersistence?: AutonomousModelHealthPersistenceCoordinator;
  /** Optional CAS-fenced persistence coordinator for process-local transport health and circuits. */
  runtimeHealthPersistence?: LLMRuntimeHealthPersistenceCoordinator;
  /** Optional Rust/Python control-plane sink for restart-safe transport health observations. */
  modelHealthBridge?: AutonomousBrainControlPlaneBridge;
  apiClient?: ApiClient;
  toolCatalogue?: ToolCatalogue;
  toolExecutor?: DomainToolExecutor;
  toolApprover?: DomainToolApprover;
  /** Optional caller-owned durable effect ledger used for idempotency and restart reconciliation. */
  effectBoundary?: AutonomousEffectBoundary;
  /** Optional caller-owned metadata-only capability journal used for restart-safe replay. */
  capabilityJournal?: AutonomousCapabilityJournalStore;
  /** Optional durable coordinator for the capability replay barrier. */
  capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
  /** Optional metadata-only execution journal used for long-horizon recovery. */
  executionJournal?: AutonomousExecutionSnapshotJournal;
  /** Optional durable coordinator for execution checkpoints. */
  executionPersistence?: AutonomousExecutionPersistenceCoordinator;
  /** Optional durable coordinator for route/planning/evaluation decision-cycle checkpoints. */
  decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
  /** Optional caller-owned durable replay barrier for capability evaluator settlements. */
  capabilityLearningSettlementStore?: AutonomousCapabilityLearningSettlementStore;
  learner?: AutonomousOnlineLearner;
  /** Optional CAS-fenced persistence coordinator bound to `learner` for restart-safe bandit state. */
  learnerPersistence?: AutonomousOnlineLearnerPersistenceCoordinator;
  /** Optional digest-only lifecycle that gates learned model selection until replay admission. */
  selectionPromotion?: AutonomousSelectionPromotionLifecycle;
  /** Optional validated aggregate-only evaluator calibration registry used by readiness gates. */
  evaluatorCalibrationRegistry?: AutonomousEvaluatorCalibrationRegistry;
  /** Optional CAS-fenced persistence coordinator bound to the evaluator calibration registry. */
  evaluatorCalibrationPersistence?: AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator;
  /** Optional caller-owned episodic memory used for bounded retrieval and value-only run recording. */
  memoryStore?: AutonomousEpisodicMemoryStore;
  /** Optional CAS-fenced persistence coordinator bound to `memoryStore` for restart-safe episodes. */
  memoryPersistence?: AutonomousMemoryPersistenceCoordinator;
  /** Optional evaluator-gated lesson index; raw lesson text remains caller-resolved and transient. */
  memoryConsolidator?: AutonomousMemoryConsolidator;
  /** Optional registry-bound, CAS-fenced prompt learner used by every high-level run. */
  promptLearningCoordinator?: AutonomousPromptLearningPersistenceCoordinator;
  /** Optional caller-owned CAS-fenced persistence for evaluator-approved tool selection state. */
  toolSelectionPersistence?: AutonomousToolSelectionPersistence;
  /** Optional initial value-only tool selection state restored by the application before execution. */
  toolSelectionState?: AutonomousToolSelectionState | null;
  /** Optional caller-owned activation state machine; keys and raw prompts never enter its state. */
  activation?: AutonomousCapabilityActivation;
  /** Optional caller-owned external connector catalogue; registration never authorizes dispatch. */
  connectorRegistry?: AutonomousConnectorRegistry;
  /** Optional connector runtime with approval, replay, and receipt boundaries. */
  connectorRuntime?: AutonomousConnectorRuntime;
}

/** High-level composition options for the reviewed source-evidence lifecycle. */
export interface AutonomousReviewedEvidenceExecutionOptions {
  availableEvidence?: readonly string[];
  completedStages?: Readonly<Record<string, readonly string[]>>;
  prepare?: AutonomousReviewedEvidencePreparationOptions;
  execute?: AutonomousEvidenceExecutionOptions;
}

/** Restart-safe reviewed evidence execution options for a caller-owned metadata checkpoint. */
export interface AutonomousReviewedEvidenceResumableExecutionOptions extends Omit<AutonomousReviewedEvidenceExecutionOptions, "execute"> {
  jobId: string;
  checkpointStore: AutonomousEvidenceExecutionCheckpointStore;
  /** Must remain identical for the full job; approved dispatch is refused when it is absent. */
  reconciliationAuthority?: AutonomousEvidenceExecutionReconciliationAuthorityIdentity;
  execute?: AutonomousEvidenceExecutionResumableOptions;
}

/** Preparation options that keep the caller-owned evidence health ledger at the facade boundary. */
export interface AutonomousReviewedEvidencePreparationOptions extends AutonomousEvidenceExecutionPrepareOptions {
  healthStore?: AutonomousEvidenceAdapterHealthStore;
}

/** Caller-owned controls for one provider-assisted planning proposal. */
export interface AutonomousProviderPlanningOptions {
  candidates?: readonly AutonomousModelCandidate[];
  credential?: CredentialHandle;
  credentialFor?: (provider: string) => CredentialHandle | undefined;
  context?: readonly AutonomousPromptChunk[];
  /** Explicit versioned prompt implementation for the planner; rendered messages remain transient. */
  promptTemplate?: AutonomousPromptTemplate;
  /** Reviewed prompt registry used to select the planner implementation. */
  promptRegistry?: AutonomousPromptRegistry;
  /** Optional digest-bound planner prompt selection; omitted plans are selected at call time. */
  promptSelection?: AutonomousPromptSelectionPlan | AutonomousPromptSelectionPlanJSON;
  /** Caller-owned value-only prompt-arm state; evaluator settlement remains an explicit follow-up. */
  promptLearningState?: AutonomousPromptLearningState | AutonomousPromptLearningStateJSON;
  /** UCB exploration weight for adaptive planner prompt selection. */
  promptLearningExploration?: number;
  /** Versioned planner prompt stage; defaults to `planning`. */
  promptStage?: string;
  maxInputTokens?: number;
  maxOutputTokens?: number;
  maxCostPerMillionTokens?: number;
  maxLatencyMs?: number;
  minQuality?: number;
  minSelectionConfidence?: number;
  /** Explicit weighted utility policy for this run's model decision. */
  selectionWeights?: Partial<AutonomousSelectionWeights>;
  /** Caller-owned global online observations used by the deterministic selection ranker. */
  selectionObservations?: readonly AutonomousModelObservation[];
  /** Aggregate estimated spend ceiling for this planning call and any provider failover. */
  maxTotalCostUnits?: number;
  /** Share a caller-owned aggregate budget across planning and the eventual execution. */
  costBudget?: AutonomousCostBudget;
  approveProviderCall?: boolean;
  runId?: string;
  temperature?: number;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  maxProviderFailovers?: number;
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  /** @internal Privileged final fence immediately before each concrete transport attempt. */
  providerDispatchFence?: ProviderTransportDispatchFence;
  /** Metadata-only lifecycle callback for each model-selection attempt. */
  selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
  /** Apply the same provider-free domain admission boundary to the planning call. */
  domainPolicyMode?: AutonomousDomainPolicyExecutionMode;
  domainPolicyEvidenceReady?: boolean;
  domainPolicyEvaluatorConfigured?: boolean;
  /** Planning itself has no effect by default; callers may declare an effectful planner explicitly. */
  domainPolicyEffectsRequested?: boolean;
  domainPolicyEffectsApproved?: boolean;
  /** Optional caller-issued grant enforced before the planner provider call. */
  authorizationContext?: AutonomousAuthorizationContext;
}

/** Metadata-safe input for planning an existing dependency-closed step graph. */
export interface AutonomousOrderedStepPlanStep extends JsonObject {
  id: string;
  domain: string;
  capability: string;
  objective: string;
  depends_on?: string[];
  required?: boolean;
}

/** Caller-owned mission/portfolio planning request; raw objectives are transient prompt input. */
export interface AutonomousOrderedStepPlanRequest {
  task: string;
  steps: AutonomousOrderedStepPlanStep[];
  domain?: AutonomousDomainName;
  capability?: string;
  basePlanDigest?: string;
  protectedContractDigest?: string | null;
  context?: AutonomousPromptChunk[];
}

export interface AutonomousModelRefreshResult {
  schema: typeof AUTONOMOUS_MODEL_REFRESH_SCHEMA;
  provider: string;
  discovered_model_count: number;
  candidate_count: number;
  candidates: AutonomousModelCandidate[];
  registered_model_ids: string[];
  replaced_model_ids: string[];
  removed_model_ids: string[];
  discovery: ProviderModelDiscovery;
  execution: "not_started;catalogue_registration_only";
  retention: "model_metadata_only;credentials_and_raw_catalogue_not_retained";
  secret_material: "never_returned";
}

/** One provider discovery request in an aggregate model-catalogue refresh. */
export interface AutonomousModelRefreshSpec extends JsonObject {
  provider: string;
  defaults: AutonomousModelCandidateDefaults;
}

/** Redacted failure metadata from one provider discovery attempt. */
export interface AutonomousModelRefreshFailure extends JsonObject {
  provider: string;
  error_class: string;
  failure_code: string;
  retryable: boolean;
}

/** Bounded multi-provider model discovery and atomic per-provider reconciliation result. */
export interface AutonomousModelCatalogueRefreshResult {
  schema: typeof AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA;
  status: "completed" | "partial" | "failed";
  requested_provider_count: number;
  successful_provider_count: number;
  failed_provider_count: number;
  refreshes: AutonomousModelRefreshResult[];
  failures: AutonomousModelRefreshFailure[];
  execution: "catalogue_registration_only";
  retention: "model_metadata_only;credentials_and_raw_catalogue_not_retained";
  secret_material: "never_returned";
}

/** Restart-safe model metadata; credentials, prompts, responses, and raw catalogues are excluded. */
export interface AutonomousModelCatalogueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA;
  models: AutonomousModelCandidate[];
  catalogue_digest: string;
  snapshot_digest: string;
  retention: "model_metadata_only_hash_bound";
  secret_material: "never_returned";
}

/** Caller-owned durable adapter for a model catalogue snapshot. */
export interface AutonomousModelCataloguePersistence {
  read(): Promise<AutonomousModelCatalogueSnapshot | null> | AutonomousModelCatalogueSnapshot | null;
  write(snapshot: AutonomousModelCatalogueSnapshot): Promise<void> | void;
}

/**
 * Per-run controls for the optional provider-assisted semantic router. Candidate models,
 * credentials, context, execution controllers, and the aggregate cost budget are inherited
 * from the enclosing run so routing cannot silently escape its caller-owned boundaries.
 */
export interface AutonomousRunSemanticRoutingOptions {
  approveProviderCall?: boolean;
  minSemanticConfidence?: number;
  maxDomains?: number;
  allowCrossDomain?: boolean;
  maxOutputTokens?: number;
  temperature?: number;
  maxCostPerMillionTokens?: number;
  maxLatencyMs?: number;
  minQuality?: number;
  maxProviderFailovers?: number;
  domainPolicyMode?: AutonomousDomainPolicyExecutionMode;
  domainPolicyEvidenceReady?: boolean;
  domainPolicyEvaluatorConfigured?: boolean;
  domainPolicyEffectsRequested?: boolean;
  domainPolicyEffectsApproved?: boolean;
}

export interface AutonomousRunOptions {
  domain?: AutonomousDomainName;
  /** Internal reviewed-stage identity; workflow executors populate this before provider dispatch. */
  workflowContext?: AutonomousWorkflowToolContext;
  /** Reuse a route already approved by a caller-owned semantic router. */
  routeOverride?: AutonomousRouteProposal;
  /**
   * Opt into provider-assisted semantic routing for the high-level execution path. The
   * classifier is a proposal only; it shares this run's credential, policy, approval, and
   * aggregate cost boundary, and execution remains fail-closed on review outcomes.
   */
  semanticRouting?: boolean | AutonomousRunSemanticRoutingOptions;
  capability?: string;
  candidates?: readonly AutonomousModelCandidate[];
  credential?: CredentialHandle;
  credentialFor?: (provider: string) => CredentialHandle | undefined;
  /** Optional caller-issued grant enforced immediately before each provider attempt/turn. */
  authorizationContext?: AutonomousAuthorizationContext;
  context?: readonly AutonomousPromptChunk[];
  /** Explicit versioned prompt implementation; its rendered messages remain transient. */
  promptTemplate?: AutonomousPromptTemplate;
  /** Reviewed prompt registry used to select a versioned template for the run's domain/stage. */
  promptRegistry?: AutonomousPromptRegistry;
  /** Optional digest-bound selection plan; omitted plans are selected from promptRegistry at run time. */
  promptSelection?: AutonomousPromptSelectionPlan | AutonomousPromptSelectionPlanJSON;
  /** Caller-owned value-only prompt-arm state; evaluator settlement remains an explicit follow-up. */
  promptLearningState?: AutonomousPromptLearningState | AutonomousPromptLearningStateJSON;
  /** UCB exploration weight for adaptive run prompt selection. */
  promptLearningExploration?: number;
  /** Prompt workflow stage used for versioned prompt selection; defaults to `answer`. */
  promptStage?: string;
  /** Transient multimodal evidence appended to the task message only; never retained in autonomy state. */
  contentParts?: readonly ProviderContentPart[];
  /** Override the agent memory store for this run. */
  memoryStore?: AutonomousEpisodicMemoryStore;
  /** Additional bounded filters for value-only episodic retrieval. */
  memoryQuery?: AutonomousMemoryQuery;
  /** Ranking policy for recalled memory; planning is the default and remains advisory only. */
  memoryRecall?: "relevance" | "quality" | "planning";
  memoryLimit?: number;
  memoryTags?: readonly string[];
  /** Stable caller-owned identity for idempotent memory recording across restarts. */
  memoryRunId?: string;
  /** Record a value-only episode after this run; defaults to true when a store exists. */
  recordMemory?: boolean;
  /** Retrieve prior value-only episodes before prompt assembly; defaults to true when a store exists. */
  retrieveMemory?: boolean;
  /** Optional caller-owned lesson; it is screened and retained only as bounded memory metadata. */
  memoryLesson?: string | null;
  /** Override the agent lesson index for this run; the index retains digests, not lesson text. */
  memoryConsolidator?: AutonomousMemoryConsolidator;
  /** Resolve a stable lesson digest to transient prompt text. The resolver remains caller-owned. */
  memoryLessonResolver?: (lessonDigest: string) => string | null;
  /** Resolve a stable lesson with domain/capability/scope metadata for caller-owned authorization. */
  memoryLessonContextResolver?: AutonomousMemoryLessonContextResolver;
  /** Maximum number of evaluator-gated lesson references recalled per routed domain. */
  consolidatedMemoryLimit?: number;
  /** Disable evaluator-gated lesson recall without disabling ordinary episodic memory. */
  retrieveConsolidatedMemory?: boolean;
  /** Fail closed when the requested consolidated lesson boundary cannot be assembled. */
  consolidatedMemoryRequired?: boolean;
  /** Optional controller that prepares a pending bandit-learning episode after a completed run. */
  learning?: AutonomousLearningController;
  /** Stable caller-owned identity for the pending learning episode. */
  learningEpisodeId?: string;
  hints?: readonly string[];
  /** Provider-free route confidence floor used when no explicit route override is supplied. */
  minConfidence?: number;
  /** Provider-free route separation floor used when no explicit route override is supplied. */
  minMargin?: number;
  /** Maximum number of domains selected by the provider-free route. */
  maxDomains?: number;
  allowCrossDomain?: boolean;
  maxInputTokens?: number;
  /** Explicit lossy context budget; system/developer instructions and recent turns are protected. */
  contextBudget?: AutonomousContextBudgetOptions;
  maxOutputTokens?: number;
  /** Refuse candidates above this caller-owned cost prior. */
  maxCostPerMillionTokens?: number;
  /** Refuse candidates above this caller-owned latency prior. */
  maxLatencyMs?: number;
  /** Refuse candidates below this caller-owned quality prior. */
  minQuality?: number;
  /** Abstain when eligible model ranking separation is below this normalized floor. */
  minSelectionConfidence?: number;
  /** Explicit weighted utility policy for this run's model decision. */
  selectionWeights?: Partial<AutonomousSelectionWeights>;
  /** Caller-owned global online observations used by the deterministic selection ranker. */
  selectionObservations?: readonly AutonomousModelObservation[];
  /** Aggregate estimated spend ceiling shared by nested provider calls in this run. */
  maxTotalCostUnits?: number;
  /** Share a caller-owned aggregate budget across fan-out, synthesis, retries, or cycles. */
  costBudget?: AutonomousCostBudget;
  /** Require a provider response that parses as JSON; disabled by default. */
  requireJson?: boolean;
  /** Optional JSON Schema checked locally and, when supported, enforced by the provider. */
  responseSchema?: JsonObject;
  /** Opt into the reviewed domain-specific JSON response contract for this run. */
  structuredDomainResponse?: boolean;
  /** Hold a structurally valid but below-threshold structured response for caller review. Defaults to true. */
  requireStructuredResponseReview?: boolean;
  temperature?: number;
  tools?: readonly ProviderTool[];
  authorizeAndExecute?: (calls: ProviderToolCall[]) => ProviderToolResult[] | Promise<ProviderToolResult[]>;
  /** Classify custom provider tool calls for an execution controller; unknown tools are not read-only by default. */
  toolReadOnly?: (call: ProviderToolCall) => boolean | Promise<boolean>;
  approveProviderCall?: boolean;
  approveEffects?: boolean;
  /** Optional caller-owned policy/state controller enforced at provider and tool boundaries. */
  execution?: AutonomousExecutionController;
  /** Optional caller-owned effect ledger; uncertain external effects must be reconciled before retry. */
  effectBoundary?: AutonomousEffectBoundary;
  /** A completed, non-review provider proposal that may reorder existing cross-domain children. */
  acceptedCrossDomainPlanRefinement?: AutonomousCrossDomainPlanRefinementResult;
  /** A completed, non-review provider proposal that may reorder existing workflow stages. */
  acceptedSingleDomainPlanRefinement?: AutonomousPlanRefinementResult;
  /** Logical attempt number recorded in execution metadata; it never changes provider authority. */
  executionAttempt?: number;
  /**
   * Stable root key for provider requests belonging to one already-authorized operation.
   * Resumable controllers derive this from their durable operation digest; cross-domain
   * execution derives distinct child/synthesis keys from the same root.
   */
  providerIdempotencyKey?: string;
  /** Maximum number of retryable provider failures that may trigger a new provider selection. */
  maxProviderFailovers?: number;
  /** Internal composition mode for a higher-level session that owns terminal transitions. */
  executionLifecycle?: "managed" | "observe_only";
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  /** @internal Privileged final fence immediately before each concrete transport attempt. */
  providerDispatchFence?: ProviderTransportDispatchFence;
  /** Metadata-only lifecycle callback for each model-selection attempt. */
  selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
  /** Caller-owned value-only tool-arm statistics used to adapt reviewed tool ranking. */
  toolSelectionState?: AutonomousToolSelectionState | null;
  /** Deterministic UCB exploration weight for tool-arm ranking. */
  toolSelectionExploration?: number;
  /** Maximum reviewed tool risk admitted to the provider-visible portfolio. */
  maxToolRiskClass?: AutonomousToolRiskClass;
  /** Audit records policy posture; strict blocks before provider/tool dispatch until every gate passes. */
  domainPolicyMode?: AutonomousDomainPolicyExecutionMode;
  /** Explicit evidence acceptance required by strict policies before provider invocation. */
  domainPolicyEvidenceReady?: boolean;
  /** Explicit evaluator configuration required by strict policies before provider invocation. */
  domainPolicyEvaluatorConfigured?: boolean;
  /** Explicit plan acceptance for a caller-owned plan not represented by a refinement object. */
  domainPolicyPlanAccepted?: boolean;
  /** Explicitly declare whether this invocation intends to request effectful work. */
  domainPolicyEffectsRequested?: boolean;
  /** Approval state for effectful work when the selected domain permits it. */
  domainPolicyEffectsApproved?: boolean;
  /** Strict-mode upper bound for provider tool-loop turns. */
  maxToolTurns?: number;
}

export interface AutonomousCrossDomainRunOptions extends AutonomousRunOptions {
  subtasks?: readonly AutonomousCrossDomainSubtask[];
  allowPartial?: boolean;
  synthesize?: boolean;
  /** Maximum number of specialist provider calls in flight during bounded fan-out. */
  maxParallelChildren?: number;
  /** Caller-supplied digest-bound pairwise response alignments for synthesis admission. */
  responseAlignments?: readonly AutonomousCrossDomainResponseAlignmentInput[];
  /** Require complete pairwise alignment before synthesis; disabled by default because alignment is semantic work. */
  requireResponseAlignment?: boolean;
  /** Minimum structural reward required for each specialist/synthesis response. */
  minimumResponseReward?: number;
  /** Minimum confidence required for an alignment to count as unresolved/low-confidence evidence. */
  minimumResponseAlignmentConfidence?: number;
  /** Confidence threshold for blocking high-confidence contradictions. */
  responseContradictionConfidenceThreshold?: number;
}

/**
 * Agent-owned composition boundary for durable mission replanning.
 *
 * The mission contract, checkpoint stores, evaluator, and provider-planning controls remain
 * caller-owned. The agent supplies the exact model/prompt/tool invocation adapter for each step,
 * so the TypeScript façade cannot accidentally fall back to a provider-free executor or lose
 * prompt-learning receipts at the mission boundary.
 */
export interface AutonomousAgentMissionReplanOptions extends Omit<AutonomousMissionReplanOptions, "execute"> {
  /** Aggregate mission execution controls; provider calls remain explicitly approved. */
  execute?: Omit<AutonomousMissionExecuteOptions, "signal" | "execution_attempt">;
  /** Per-step autonomous run controls, including candidates, credentials, prompts, and learning. */
  stepRun?: Omit<AutonomousRunOptions, "domain" | "capability" | "tools" | "authorizeAndExecute" | "context" | "approveProviderCall" | "signal">;
  /** Use a caller-owned catalogue instead of the catalogue attached to this agent. */
  catalogue?: ToolCatalogue;
  /** Narrow the provider-visible tool definition for each exact mission step. */
  toolsForStep?: (step: AgentMissionStep) => readonly ProviderTool[] | undefined;
  /** Explicit effect approval passed to the existing tool/effect boundary. */
  approveEffects?: boolean;
  /** Caller-owned metadata checkpoint and raw-result stores used by the mission executor. */
  checkpointStore?: AutonomousMissionCheckpointStore;
  resultStore?: AutonomousMissionResultStore;
  /** Metadata-only step outcome observer; raw values remain in the caller-owned result store. */
  onStepOutcome?: (outcome: AutonomousMissionStepResult, context: { mission_id: string; wave: number }) => Promise<void> | void;
}

/** Explicit caller-owned metadata trace controls for one autonomous run. */
export interface AutonomousRunWithTraceOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
  run?: AutonomousRunOptions;
}

export interface AutonomousTracedRunResult {
  result: AutonomousRunResult;
  trace: AutonomousRunTraceSummary;
}

export interface AutonomousTracedCrossDomainRunResult {
  result: AutonomousCrossDomainRunResult;
  trace: AutonomousRunTraceSummary;
}

export interface DomainToolExecutor {
  (tool: AutonomousDomainToolBinding, arguments_: JsonObject, effect?: AutonomousEffectExecutionContext): JsonValue | Promise<JsonValue>;
}

export interface DomainToolApprover {
  (tool: AutonomousDomainToolBinding, call: ProviderToolCall): boolean | Promise<boolean>;
}

export type AutonomousPlanAndRunStatus =
  | AutonomousRunStatus
  | AutonomousCrossDomainRunStatus
  | "plan_review_required"
  | "provider_invalid"
  | "provider_failed"
  | "provider_disagreement";

/** Options for the explicit provider-planning -> human acceptance -> execution bridge. */
export interface AutonomousPlanAndRunOptions extends AutonomousRunOptions {
  /** Provider planning is disabled unless supplied; its own approval is separate from execution approval. */
  planning?: AutonomousProviderPlanningOptions;
  /** Optional caller-reviewed specialist tasks for a routed cross-domain plan. */
  subtasks?: readonly AutonomousCrossDomainSubtask[];
  /** Prompt stage used when the outer run supplies prompt controls to nested provider planning. */
  planningPromptStage?: string;
  /** Optional independent value-only prompt state for the nested planning proposal. */
  planningPromptLearningState?: AutonomousPromptLearningState | AutonomousPromptLearningStateJSON;
  /** UCB exploration weight for nested planning prompt selection. */
  planningPromptLearningExploration?: number;
  /** Only true allows a completed, non-review proposal to shape the subsequent invocation. */
  acceptPlan?: boolean;
}

/** Value-only envelope for a provider-planned autonomous invocation. */
export interface AutonomousPlanAndRunResult {
  schema: typeof AUTONOMOUS_PLAN_AND_RUN_SCHEMA;
  status: AutonomousPlanAndRunStatus;
  route: AutonomousRouteProposal;
  /** Optional provider-assisted routing proposal used before planning. */
  semantic_route?: AutonomousSemanticRouteResult | null;
  blueprint: AutonomousAutoBlueprint | null;
  plan_refinement: AutonomousPlanRefinementResult | AutonomousCrossDomainPlanRefinementResult | null;
  result: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned";
  authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval";
}

export type AutonomousAutoPlanningMode = "deterministic" | "provider";

/** One high-level automatic route that can execute deterministically or through reviewed planning. */
export interface AutonomousAutoRunOptions extends AutonomousPlanAndRunOptions {
  /** Deterministic execution is the default; provider planning is an explicit opt-in. */
  planningMode?: AutonomousAutoPlanningMode;
}

export type AutonomousAutoRunNextAction =
  | "review_route"
  | "review_plan"
  | "review_provider_or_effect_approval"
  | "inspect_result"
  | "complete";

/** Value-only envelope for the route -> plan -> execute automatic brain boundary. */
export interface AutonomousAutoRunResult {
  schema: typeof AUTONOMOUS_AUTO_RUN_SCHEMA;
  status: AutonomousPlanAndRunStatus;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  blueprint: AutonomousAutoBlueprint | null;
  planning: AutonomousPlanAndRunResult | null;
  result: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  planning_mode: AutonomousAutoPlanningMode;
  next_action: AutonomousAutoRunNextAction;
  retention: "provider_response_local;route_and_plan_metadata_value_only;execution_result_caller_owned";
  authorization: "route_review_and_provider_or_effect_approval_remain_explicit";
}

/**
 * Transient event emitted by the application-facing autonomous stream.
 *
 * Provider text and tool-call deltas are available only while the caller consumes `events`.
 * Lifecycle events intentionally contain digests and counters rather than task text, provider
 * payloads, credentials, or specialist output.
 */
export type AutonomousRunStreamEvent =
  | {
      kind: "provider";
      stage: "direct" | "child" | "synthesis";
      child_id?: string;
      event: ProviderStreamEvent;
    }
  | {
      kind: "lifecycle";
      stage: "route" | "child" | "synthesis";
      phase: "child_started" | "child_completed" | "synthesis_started" | "synthesis_completed";
      child_id?: string;
      domain?: string;
      status?: string;
      event_count?: number;
      text_delta_bytes?: number;
      selection_digest?: string | null;
    };

/** Metadata-only terminal receipt for an application-facing autonomous stream. */
export interface AutonomousRunStreamCompletion extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA;
  status: AutonomousRunStatus | AutonomousCrossDomainRunStatus | "failed" | "abandoned";
  route_digest: string;
  task_digest: string;
  blueprint_digest: string | null;
  event_count: number;
  text_delta_bytes: number;
  stage_count: number;
  provider_invocations: AutonomousProviderInvocationReceipt[];
  provider_failover: AutonomousProviderFailoverProjection | null;
  inner_completions: Array<AutonomousStreamCompletion | AutonomousRunStreamCompletion>;
  error_code: ProviderErrorCode | null;
  error_class: string | null;
  retention: "metadata_only_no_stream_payloads_or_credentials";
  secret_material: "never_returned";
}

/**
 * Application-facing stream handle. Selection and blueprint metadata are available before the
 * first event; text remains transient and is never copied into the completion receipt.
 */
export interface AutonomousRunStreamHandle {
  schema: typeof AUTONOMOUS_RUN_STREAM_SCHEMA;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  blueprint: AutonomousTaskBlueprint | AutonomousCrossDomainBlueprint | null;
  selection: AutonomousSelectionDecision | null;
  continuation_plan: AutonomousModelContinuationPlan | null;
  context_budget: AutonomousContextBudgetPlan | null;
  events: AsyncIterable<AutonomousRunStreamEvent>;
  completion: Promise<AutonomousRunStreamCompletion>;
}

export interface AutonomousMemoryPreparation {
  readonly store: AutonomousEpisodicMemoryStore | undefined;
  readonly context: AutonomousPromptChunk[];
  readonly projection: AutonomousMemoryRunProjection | null;
}

export interface ProfileSeed {
  domain: AutonomousDomainName;
  riskClass: string;
  defaultCapability: string;
  requiredModelCapabilities: string[];
  capabilities: string[];
  terms: string[];
  systemInstructions: string;
  evaluatorDomain: AutonomousDomainProfile["evaluator_domain"];
  workflowId: string;
  toolRows: string;
  description: string;
}

export interface WorkflowStageDefinition {
  id: string;
  objective: string;
  required_capabilities: string[];
  depends_on: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  read_only: boolean;
  approval_required: boolean;
}

export interface WorkflowDefinition {
  workflowId: string;
  stages: WorkflowStageDefinition[];
  routeIntents: string[];
  evaluatorSignals: string[];
  completionContract: string;
}

export type AutonomousCrossDomainExecutionReceiptFields = {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA;
  status: AutonomousCrossDomainRunStatus;
  execution_child_ids: string[];
  child_domains: Record<string, AutonomousDomainName>;
  child_statuses: Record<string, string>;
  child_result_digests: Record<string, string | null>;
  completed_child_ids: string[];
  incomplete_child_ids: string[];
  synthesis_status: string | null;
  synthesis_result_digest: string | null;
  completed_units: number;
  total_units: number;
  progress: number;
  next_action: AutonomousCrossDomainExecutionNextAction;
  safe_to_synthesize: boolean;
  reconciliation_required: boolean;
  retention: "status_and_outcome_digests_only; provider_payloads_caller_owned";
  secret_material: "never_returned";
};

export type RenderedAutonomousRunPrompt = {
  messages: readonly ProviderMessage[];
  metadata: AutonomousPromptRenderResult;
  mode: "versioned_template" | "registry_selection";
};

export interface PreparedProviderPlanning {
  prompt: AutonomousPromptResult;
  /** Digest of the exact transient planner prompt boundary, including version metadata. */
  promptDigest: string;
  /** Exact registry-bound adaptive prompt receipt; rendered messages remain transient. */
  adaptiveSelection?: AutonomousPromptAdaptiveSelectionJSON;
  plan: AutonomousExecutionPlan;
  learningContext: BrainBanditContext;
  learningContextDigest: string;
}

export interface AutonomousAcceptedCrossDomainPlan {
  priority_child_ids: string[];
  focus_child_ids: string[];
  refinement_digest: string;
}

export interface AutonomousAcceptedPlan {
  priority_stage_ids: string[];
  focus_stage_ids: string[];
  refinement_digest: string;
}
