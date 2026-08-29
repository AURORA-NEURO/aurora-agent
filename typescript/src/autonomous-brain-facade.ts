import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import { AutonomousProtectedRehydrationAdapter } from "./autonomous-protected-rehydration.js";
import {
  ProviderSetup,
  type AutonomousProvisionedBrainAdaptiveCycleOptions,
  type AutonomousProvisionedBrainAdaptiveCycleTraceOptions,
  type AutonomousProvisionedBrainApprovedSelectionOptions,
  type AutonomousProvisionedBrainApprovedSelectionTraceOptions,
  type AutonomousProvisionedBrainAutoExecuteOptions,
  type AutonomousProvisionedBrainAutoTraceOptions,
  type AutonomousProvisionedBrainCycleOptions,
  type AutonomousProvisionedBrainCycleTraceOptions,
  type AutonomousProvisionedBrainExecuteOptions,
  type AutonomousProvisionedBrainTraceOptions,
  type AutonomousProvisionedRun,
} from "./provider-setup.js";
import {
  AutonomousGoalAgentRuntime,
  type AutonomousGoalAgentLoopRunOptions,
  type AutonomousGoalAgentRuntimeOptions,
} from "./autonomous-goal-agent.js";
import { AutonomousGoalControlLoopPreview, type AutonomousGoalControlLoopResult } from "./autonomous-goal-control-loop.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  validateAutonomousRouteOverride,
  type AutonomousAgent,
  type AutonomousAgentMissionReplanOptions,
  type AutonomousApprovedModelSelectionOptions,
  type AutonomousAutoBlueprint,
  type AutonomousDomainToolPlan,
  type AutonomousCrossDomainBlueprint,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainRunResult,
  type AutonomousCrossDomainSubtask,
  type AutonomousDomainName,
  type AutonomousAutoRunOptions,
  type AutonomousAutoRunResult,
  type AutonomousEvidenceBackedRunOptions,
  type AutonomousEvidenceBackedRunResult,
  type AutonomousPromptChunk,
  type AutonomousRouteProposal,
  type AutonomousRunOptions,
  type AutonomousRunResult,
  type AutonomousRunStreamHandle,
  type AutonomousModelSelectionPreview,
  type AutonomousModelSelectionPreviewOptions,
  type AutonomousModelRefreshSpec,
  type AutonomousTaskBlueprint,
  type AutonomousClarificationRecompileProjection,
  type AutonomousClarificationRecompileResult,
} from "./autonomous.js";
import type {
  AutonomousTaskClarificationPlan,
  AutonomousTaskClarificationResolution,
} from "./autonomous-task-clarification.js";
import {
  semanticRouteAutonomousTask,
  type AutonomousSemanticRouteOptions,
  type AutonomousSemanticRouteResult,
} from "./autonomous-routing.js";
import {
  AutonomousConnectorOperationFacade,
  AutonomousConnectorOperationPlan,
  AutonomousConnectorIntentFacade,
  type AutonomousConnectorOperationExecution,
  type AutonomousConnectorOperationInput,
} from "./autonomous-connector-facade.js";
import {
  runAutonomousConnectorMission,
  runAutonomousConnectorMissionWithLaunchAdmission,
  runAutonomousConnectorMissionWithProviderPlanning,
  runAutonomousConnectorMissionWithProviderPlanningAndLaunchAdmission,
  type AutonomousConnectorMissionProviderPlanningOptions as ConnectorMissionProviderPlanningOptions,
  type AutonomousConnectorMissionRunOptions as ConnectorMissionRunOptions,
  type AutonomousConnectorPlannedMissionRun as ConnectorPlannedMissionRun,
} from "./autonomous-connector-mission.js";
import {
  AutonomousEvidenceBackedController,
  runAutonomousEvidenceBackedResumable,
  type AutonomousEvidenceBackedCheckpointStore,
  type AutonomousEvidenceBackedResumableExecutionOptions,
  type AutonomousEvidenceBackedResumableRun,
} from "./autonomous-evidence-backed-resumable.js";
import type {
  AutonomousDomainEvidenceBrainRunOptions,
  AutonomousDomainEvidenceBrainRunResult,
} from "./autonomous-domain-evidence-brain.js";
import {
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousCrossDomainReplanCycle,
  runAutonomousDecisionCycle,
  runAutonomousReplanCycle,
  type AutonomousAutoDecisionCycleOptions,
  type AutonomousAutoDecisionCycleResult,
  type AutonomousAutoReplanCycleOptions,
  type AutonomousAutoReplanCycleResult,
  type AutonomousCrossDomainDecisionCycleOptions,
  type AutonomousCrossDomainDecisionCycleResult,
  type AutonomousDecisionCycleSemanticOptions,
  type AutonomousCrossDomainReplanCycleOptions,
  type AutonomousCrossDomainReplanCycleResult,
  type AutonomousDecisionCycleOptions,
  type AutonomousDecisionCycleResult,
  type AutonomousReplanCycleOptions,
  type AutonomousReplanCycleResult,
} from "./autonomous-cycle.js";
import type { AutonomousCapabilityActivationSnapshotStore } from "./autonomous-activation.js";
import {
  autonomousRunTraceStatus,
  AutonomousRunTraceSession,
  type AutonomousRunTraceStore,
  type AutonomousRunTraceSummary,
} from "./autonomous-run-trace.js";
import {
  AutonomousRunTraceRegistry,
  AutonomousRunTraceRegistryPersistenceCoordinator,
  JsonAutonomousRunTraceRegistryPersistence,
  publishAutonomousRunTraceRegistrySnapshot,
  type AutonomousRunTraceRegistryEventQuery,
  type AutonomousRunTraceRegistryImportReport,
  type AutonomousRunTraceRegistryIntegrity,
  type AutonomousRunTraceRegistryPage,
  type AutonomousRunTraceRegistryPublication,
  type AutonomousRunTraceRegistryQuery,
  type AutonomousRunTraceRegistryRecord,
  type AutonomousRunTraceRegistrySnapshot,
} from "./autonomous-run-trace-registry.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type {
  AutonomousModelCandidate,
  AutonomousModelCandidateDefaults,
  AutonomousModelSelectionTraceEventCallback,
  ProviderInvocationObserver,
  ProviderModelDiscovery,
} from "./llm.js";
import { CredentialSession } from "./llm.js";
import { AutonomousCostBudget } from "./llm.js";
import type { AgentMissionArgs, JsonObject, JsonValue } from "./types.js";
import type { AutonomousMissionReplanResult } from "./mission-replan.js";
import {
  autonomousEvaluatorCalibrationAdmission,
  validateAutonomousEvaluatorCalibrationReport,
  type AutonomousEvaluatorCalibrationAdmission,
  type AutonomousEvaluatorCalibrationReport,
} from "./autonomous-evaluator-calibration.js";
import type {
  AutonomousEvaluatorCalibrationImport,
  AutonomousEvaluatorCalibrationQueryOptions,
  AutonomousEvaluatorCalibrationStoreSnapshot,
} from "./autonomous-evaluator-calibration-store.js";
import {
  AutonomousJointExecutionPolicy,
  type AutonomousExecutionPolicyCandidateInput,
  type AutonomousExecutionPolicyDecision,
} from "./autonomous-execution-policy.js";
import type { AutonomousMemorySnapshot } from "./autonomous-memory.js";
import type { AutonomousModelHealthSnapshot } from "./autonomous-control.js";
import type {
  AutonomousAgentPersistenceLifecycleOptions,
  AutonomousAgentPersistenceLifecycleReport,
} from "./autonomous-agent-lifecycle.js";
import type {
  AutonomousModelInventoryReadiness,
  AutonomousModelInventoryReadinessOptions,
  AutonomousModelInventoryRefreshOptions,
  AutonomousModelInventorySnapshot,
} from "./autonomous-model-inventory.js";
import type {
  AutonomousWorkflowPortfolioAdmission,
  AutonomousWorkflowPortfolioAdmissionOptions,
} from "./autonomous-workflow-portfolio-admission.js";
import type {
  AutonomousWorkflowPortfolioItemRequest,
  AutonomousWorkflowPortfolioPlan,
  AutonomousWorkflowPortfolioPlanOptions,
  AutonomousWorkflowPortfolioVerification,
} from "./autonomous-workflow-portfolio.js";
import type {
  AutonomousWorkflowPortfolioExecutionOptions,
  AutonomousWorkflowPortfolioExecutionResult,
} from "./autonomous-workflow-portfolio-execution.js";
import type { AutonomousWorkflowPortfolioResumableExecutionOptions } from "./autonomous-workflow-portfolio-resumable.js";
import type {
  AutonomousWorkflowPortfolioEvidenceExecutionResult,
  AutonomousWorkflowPortfolioEvidenceSupervisorOptions,
} from "./autonomous-workflow-portfolio-evidence.js";
import type { AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions } from "./autonomous-workflow-portfolio-evidence-resumable.js";
import {
  replanAutonomousInformationAcquisition,
  validateAutonomousInformationAcquisitionPlan,
  type AutonomousInformationAcquisitionPlan,
  type ReplanAutonomousInformationAcquisitionOptions,
} from "./autonomous-information-acquisition.js";
import {
  validateAutonomousClaimIntegrity,
  validateAutonomousClaimIntegrityAcquisitionBinding,
  validateAutonomousClaimIntegrityAcquisitionBridge,
  type AutonomousClaimIntegrityAcquisitionBinding,
  type AutonomousClaimIntegrityAcquisitionBridge,
  type AutonomousClaimIntegrityAssessment,
} from "./autonomous-claim-integrity.js";
import {
  assessAutonomousOutcomeIntegrity,
  bindAutonomousOutcomeIntegrityClaims,
  projectAutonomousOutcomeIntegrityRun,
  validateAutonomousOutcomeIntegrity,
  validateAutonomousOutcomeIntegritySnapshot,
  type AssessAutonomousOutcomeIntegrityOptions,
  type AutonomousOutcomeIntegrityAssessment,
  type AutonomousOutcomeIntegrityClaimBinding,
  type AutonomousOutcomeIntegrityClaimBindingInput,
  type AutonomousOutcomeIntegrityRun,
} from "./autonomous-outcome-integrity.js";
import {
  replayAutonomousCrossDomainResponseAssessment,
  validateAutonomousCrossDomainResponseAssessment,
  type AutonomousCrossDomainResponseAlignmentInput,
  type AutonomousCrossDomainResponseAssessment,
  type AutonomousCrossDomainResponseEntry,
} from "./autonomous-cross-domain-response.js";
import {
  auditAutonomousDomainContracts,
  type AutonomousDomainAuditOptions,
  type AutonomousDomainAuditReport,
} from "./autonomous-domain-audit.js";
import {
  buildAutonomousDomainOperatingKit,
  buildAutonomousDomainOperatingKits,
  validateAutonomousDomainOperatingKit,
  type AutonomousDomainOperatingKit,
} from "./autonomous-domain-operating-kit.js";
import {
  auditAutonomousBrainLaunchPreflight,
  type AutonomousLaunchPreflightOptions,
  type AutonomousLaunchPreflightReport,
} from "./autonomous-launch-preflight.js";
import {
  authorizeAutonomousLaunchDomains,
  createAutonomousLaunchAdmission,
  type AutonomousLaunchAdmissionOptions,
  type AutonomousLaunchAdmissionReport,
} from "./autonomous-launch-admission.js";
import {
  AutonomousActionPlan,
  AutonomousActionAdmission,
  admitAutonomousActionPlan,
  buildAutonomousActionPlan,
} from "./autonomous-action-plan.js";
import {
  validateAutonomousActionDispatchHandoff,
  type AutonomousActionDispatchHandoff,
} from "./autonomous-action-admission-controller.js";
import type {
  AutonomousActionAdmissionJSON,
  AutonomousActionPlanApproval,
  AutonomousActionPlanJSON,
} from "./autonomous-action-plan.js";
import {
  planAutonomousRecovery,
  AutonomousRecoveryHandoffLedger,
  type AutonomousRecoveryHandoffSubmissionResult,
  type AutonomousRecoveryObservation,
  type AutonomousRecoveryPlan,
} from "./autonomous-recovery.js";
import {
  AutonomousWorkflowExecutor,
  type AutonomousWorkflowCheckpointStore,
  type AutonomousWorkflowExecuteOptions,
  type AutonomousWorkflowExecutorOptions,
  type AutonomousWorkflowExecutionResult,
} from "./workflow-execution.js";
import {
  runAutonomousWorkflowCycle,
  type AutonomousWorkflowCycleOptions,
  type AutonomousWorkflowCycleResult as AutonomousWorkflowCycleRunResult,
} from "./autonomous-workflow-cycle.js";

/**
 * The application-facing composition boundary for the autonomous brain.
 *
 * `AutonomousAgent` deliberately exposes lower-level route, blueprint, run, and cross-domain
 * primitives because durable applications may need to own each checkpoint. This facade is the
 * safe default for ordinary callers: it compiles one request-free plan, optionally executes one
 * reviewed connector operation first, makes its bounded observation available to the transient
 * provider prompt, and then invokes either the selected domain or the reviewed fan-out/fan-in
 * route. It never stores the task, prompt, provider response, connector request, or credential
 * in a plan or batch digest.
 */
export const AUTONOMOUS_BRAIN_FACADE_SCHEMA = "bioprism-typescript-autonomous-brain-facade/0.1" as const;
export const AUTONOMOUS_BRAIN_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-brain-batch-checkpoint/0.1" as const;
export const AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-batch-controller/0.1" as const;
export const AUTONOMOUS_BRAIN_CYCLE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-cycle-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_ADAPTIVE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-adaptive-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_AUTO_CYCLE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-auto-cycle-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_AUTO_REPLAN_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-auto-replan-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_TRACED_AUTO_CYCLE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-traced-auto-cycle-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_TRACED_AUTO_REPLAN_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-traced-auto-replan-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-trace-registry-controller/0.1" as const;
export const AUTONOMOUS_BRAIN_SUMMARY_SCHEMA = "bioprism-typescript-autonomous-brain-plan-summary/0.1" as const;
export const AUTONOMOUS_BRAIN_EXECUTION_POLICY_SCHEMA = "bioprism-typescript-autonomous-brain-execution-policy/0.1" as const;
export const AUTONOMOUS_BRAIN_AUTO_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-brain-auto-execution/0.1" as const;
export const AUTONOMOUS_BRAIN_AUTO_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-auto-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_TRACED_AUTO_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-traced-auto-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_TRACED_MISSION_REPLAN_SCHEMA = "bioprism-typescript-autonomous-brain-traced-mission-replan/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_BATCH = 64;
export const MAX_AUTONOMOUS_BRAIN_PARALLELISM = 8;
export const MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES = 128_000;
export const MAX_AUTONOMOUS_BRAIN_CONTEXT_CHUNKS = 128;
export const MAX_AUTONOMOUS_BRAIN_OBSERVATION_BYTES = 1_000_000;

export type AutonomousBrainPlanStatus = "ready" | "route_review_required" | "connector_review_required";
export type AutonomousBrainExecutionStatus = AutonomousRunResult["status"] | AutonomousCrossDomainRunResult["status"] | "connector_blocked";
export type AutonomousBrainAutoExecutionStatus = AutonomousAutoRunResult["status"] | "connector_blocked";
export const AUTONOMOUS_ACTION_EXECUTION_FACADE_SCHEMA = "bioprism-typescript-autonomous-action-execution-facade/0.1" as const;

export interface AutonomousBrainRequest {
  task: string;
  domain?: AutonomousDomainName;
  capability?: string;
  hints?: readonly string[];
  allow_cross_domain?: boolean;
  context?: readonly AutonomousPromptChunk[];
  /** Optional caller-owned evidence operation to run before provider invocation. */
  connector?: AutonomousConnectorOperationInput;
}

/** Provider-free clarification controls for one single-domain brain intake. */
export interface AutonomousBrainClarificationOptions {
  /** Optional explicit domain; when omitted, only an unambiguous deterministic route is accepted. */
  domain?: AutonomousDomainName;
  capability?: string;
  riskClass?: string;
  constraints?: readonly string[];
  desiredOutputs?: readonly string[];
  maxInputTokens?: number;
  structuredDomainResponse?: boolean;
  maxQuestions?: number;
}

/** Options applied while compiling a fresh blueprint after a resolved clarification receipt. */
export interface AutonomousBrainClarificationRecompileOptions {
  capability?: string;
  riskClass?: string;
  constraints?: readonly string[];
  desiredOutputs?: readonly string[];
  maxInputTokens?: number;
  structuredDomainResponse?: boolean;
}

/**
 * Task-driven durable workflow controls exposed by the high-level facade.
 *
 * Checkpoint persistence, task/credential rehydration, and provider/effect approvals remain
 * caller-owned. The facade only binds the existing workflow executor to this exact agent.
 */
export interface AutonomousBrainWorkflowOptions extends AutonomousWorkflowExecuteOptions {
  checkpointStore: AutonomousWorkflowCheckpointStore;
  executorOptions?: AutonomousWorkflowExecutorOptions;
}

/** Resume controls for an already checkpointed task-driven workflow. */
export interface AutonomousBrainWorkflowResumeOptions extends Omit<AutonomousWorkflowExecuteOptions, "jobId"> {
  checkpointStore: AutonomousWorkflowCheckpointStore;
  executorOptions?: AutonomousWorkflowExecutorOptions;
}

/** Result of one bounded workflow start or resume operation. */
export type AutonomousBrainWorkflowResult = AutonomousWorkflowExecutionResult;

/** Durable workflow start controls plus a caller-owned metadata-only trace. */
export interface AutonomousBrainWorkflowTraceOptions extends AutonomousBrainWorkflowOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Durable workflow resume controls plus a caller-owned metadata-only trace. */
export interface AutonomousBrainWorkflowResumeTraceOptions extends AutonomousBrainWorkflowResumeOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** A durable workflow result paired with its metadata-only execution trace. */
export interface AutonomousBrainTracedWorkflowResult {
  execution: AutonomousBrainWorkflowResult;
  trace: AutonomousRunTraceSummary;
}

/**
 * Evaluator-guided workflow cycle controls exposed by the high-level facade.
 *
 * The workflow checkpoint store and optional cycle state store remain caller-owned; the facade
 * only binds both durable kernels to its exact agent and keeps provider/effect authority explicit.
 */
export interface AutonomousBrainWorkflowCycleOptions extends AutonomousWorkflowCycleOptions {
  checkpointStore: AutonomousWorkflowCheckpointStore;
  executorOptions?: AutonomousWorkflowExecutorOptions;
}

/** Result of one bounded workflow execution/evaluation/replan cycle. */
export type AutonomousBrainWorkflowCycleResult = AutonomousWorkflowCycleRunResult;

/** Evaluator-guided workflow cycle controls plus a caller-owned metadata-only trace. */
export interface AutonomousBrainWorkflowCycleTraceOptions extends AutonomousBrainWorkflowCycleOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** An evaluator-guided workflow cycle paired with its metadata-only trace. */
export interface AutonomousBrainTracedWorkflowCycleResult {
  cycle: AutonomousBrainWorkflowCycleResult;
  trace: AutonomousRunTraceSummary;
}

export interface AutonomousBrainExecutionPolicyOptions {
  candidates: readonly AutonomousExecutionPolicyCandidateInput[];
  policy?: AutonomousJointExecutionPolicy;
  requiredCapabilities?: readonly string[];
  preferredCapabilities?: readonly string[];
  requiredPath?: AutonomousExecutionPolicyCandidateInput["path"] | null;
  evidenceRequired?: boolean;
  structuredOutputRequired?: boolean;
  effectsRequested?: boolean;
  effectsApproved?: boolean;
  approvalGranted?: boolean;
  maxCostUnits?: number;
  maxLatencyMs?: number;
  maxRisk?: number;
  minScore?: number;
}

export interface AutonomousBrainExecutionPolicyPlan extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_EXECUTION_POLICY_SCHEMA;
  route: AutonomousRouteProposal;
  decision: AutonomousExecutionPolicyDecision;
  policy_plan_digest: string;
  retention: "route_and_policy_metadata_only;task_prompt_response_tool_and_credential_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousActionPlanExecutionOptions extends AutonomousAutoRunOptions {
  /** Explicit caller-owned gates bound to the action plan digest. */
  approvals?: Partial<Record<AutonomousActionPlanApproval, boolean>>;
  /** Acknowledges review reasons emitted by the task-decision layer. */
  reviewed?: boolean;
  /** Connector execution policy used when the request includes a reviewed connector operation. */
  connectorFirst?: boolean;
  /** Include the connector's transient observation in the provider context. */
  includeConnectorObservation?: boolean;
}

/** Execution options for a previously reviewed dispatch handoff. */
export type AutonomousActionHandoffExecutionOptions = Omit<AutonomousActionPlanExecutionOptions, "approvals" | "reviewed">;

export interface AutonomousActionPlanExecution {
  schema: typeof AUTONOMOUS_ACTION_EXECUTION_FACADE_SCHEMA;
  status: "review_required" | "blocked" | "route_review_required" | "completed";
  execution_status: string;
  plan: AutonomousActionPlanJSON;
  admission: AutonomousActionAdmissionJSON;
  result: AutonomousAutoRunResult | AutonomousBrainExecution | null;
  retention: "plan_and_admission_metadata_only;execution_result_is_caller_owned";
  authorization: "caller_owned_execution_result;provider_and_effect_authority_remain_explicit";
  secret_material: "never_returned";
}

export interface AutonomousBrainDomainPlanSummary extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_SUMMARY_SCHEMA;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  workflow_id: string;
  workflow_digest: string;
  domain_pack_digest: string;
  task_digest: string;
  route_digest: string;
  prompt_digest: string;
  plan_digest: string;
  learning_context_digest: string;
  evidence_plan_digest: string;
  domain_policy_digest: string;
  task_intent_digest: string;
  task_decision_digest: string;
  task_decision_posture: "admitted" | "review_required" | "blocked";
  task_decision_recommended_path: "provider" | "evidence_first" | "workflow" | "planning" | "cross_domain";
  task_decision_requested_effect: string;
  task_decision_evidence_posture: string;
  task_decision_preferred_model_capabilities: string[];
  task_decision_approval_requirements: string[];
  task_decision_review_reasons: string[];
  task_decision_blocking_reasons: string[];
  task_decision_next_actions: string[];
  required_capabilities: string[];
  allowed_tools: string[];
  stages: Array<{
    id: string;
    depends_on: string[];
    required_capabilities: string[];
    evaluator_signals: string[];
    evidence_outputs: string[];
    approval_required: boolean;
    read_only: boolean;
  }>;
  retention: "metadata_only_task_prompt_and_provider_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousBrainCrossDomainPlanSummary extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_SUMMARY_SCHEMA;
  task_digest: string;
  route_digest: string;
  plan_digest: string;
  child_ids: string[];
  children: AutonomousBrainDomainPlanSummary[];
  synthesis: AutonomousBrainDomainPlanSummary;
  dependency_graph: {
    fan_out: Array<{ id: string; task_digest: string; domain: AutonomousDomainName }>;
    fan_in: string;
  };
  retention: "metadata_only_task_prompt_and_provider_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousBrainPlanJSON {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainPlanStatus;
  route: AutonomousRouteProposal;
  /** Provider-assisted routing remains a proposal projection; it never grants execution authority. */
  semantic_route?: AutonomousSemanticRouteResult | null;
  domain_plan: AutonomousBrainDomainPlanSummary | null;
  cross_domain_plan: AutonomousBrainCrossDomainPlanSummary | null;
  connector_plan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null;
  selected_domains: AutonomousDomainName[];
  task_digest: string;
  plan_digest: string;
  retention: "metadata_only_task_prompt_connector_request_and_provider_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousBrainExecution {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainExecutionStatus;
  plan: AutonomousBrainPlanJSON;
  /** The semantic classifier projection used to produce the route, when enabled. */
  semantic_route?: AutonomousSemanticRouteResult | null;
  run: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  connector: AutonomousConnectorOperationExecution | null;
  error: { error_class: string; failure_code: string } | null;
  retention: "plan_metadata_only;run_and_connector_values_transient_to_caller";
  secret_material: "never_returned";
}

/**
 * Automatic execution result for the high-level facade. The nested automatic envelope retains
 * the route, deterministic/provider planning posture, and final direct or cross-domain result;
 * the outer plan retains only the request-free facade metadata. Connector observations and all
 * provider values remain transient to the caller.
 */
export interface AutonomousBrainAutoExecution {
  schema: typeof AUTONOMOUS_BRAIN_AUTO_EXECUTION_SCHEMA;
  status: AutonomousBrainAutoExecutionStatus;
  plan: AutonomousBrainPlanJSON;
  semantic_route?: AutonomousSemanticRouteResult | null;
  automatic: AutonomousAutoRunResult | null;
  connector: AutonomousConnectorOperationExecution | null;
  error: { error_class: string; failure_code: string } | null;
  retention: "plan_metadata_only;automatic_and_connector_values_transient_to_caller";
  authorization: "route_review_and_provider_or_effect_approval_remain_explicit";
  secret_material: "never_returned";
}

/** Lower-level provider/policy controls for the facade's transient direct stream. */
export type AutonomousBrainStreamOptions = Omit<AutonomousRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain">;

/** Automatic deterministic stream controls; provider-planned mode remains review-gated. */
export type AutonomousBrainAutoStreamOptions = Omit<AutonomousAutoRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain">;

/** Bounded cross-domain stream controls with the request owning the route identity. */
export type AutonomousBrainCrossDomainStreamOptions = Omit<AutonomousCrossDomainRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain">;

/** High-level brain execution plus the caller-owned metadata trace of its full boundary. */
export interface AutonomousBrainTraceOptions extends AutonomousBrainExecuteOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedExecution {
  execution: AutonomousBrainExecution;
  trace: AutonomousRunTraceSummary;
}

/** Closed-loop cycle options plus the same caller-owned metadata trace boundary. */
export interface AutonomousBrainCycleTraceOptions extends AutonomousBrainCycleOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedCycleExecution {
  execution: AutonomousBrainCycleExecution;
  trace: AutonomousRunTraceSummary;
}

/** Evaluator-guided cycle options plus the same caller-owned metadata trace boundary. */
export interface AutonomousBrainAdaptiveCycleTraceOptions extends AutonomousBrainAdaptiveCycleOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedAdaptiveCycleExecution {
  execution: AutonomousBrainAdaptiveCycleExecution;
  trace: AutonomousRunTraceSummary;
}

export interface AutonomousBrainExecuteOptions {
  /** Explicit provider approval; defaults to false even when a model is registered. */
  approveProviderCall?: boolean;
  /** Optional provider-assisted route proposal; classifier and execution approval remain separate. */
  semanticRouting?: AutonomousRunOptions["semanticRouting"];
  /** Run the optional connector operation before invoking the provider; defaults to true. */
  connectorFirst?: boolean;
  /** Include the connector's transient bounded observation in the provider context. */
  includeConnectorObservation?: boolean;
  /** Lower-level provider, tool, memory, learning, and effect controls. */
  run?: Omit<AutonomousRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain">;
}

/** Automatic route -> blueprint -> execution controls with route-owned request fields reserved. */
export interface AutonomousBrainAutoExecuteOptions extends Omit<AutonomousAutoRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain" | "semanticRouting"> {
  /** Optional provider-assisted route proposal; routing approval remains separate. */
  semanticRouting?: AutonomousAutoRunOptions["semanticRouting"];
  /** Run the optional connector operation before automatic planning/execution; defaults to true. */
  connectorFirst?: boolean;
  /** Include the connector's transient bounded observation in the automatic provider context. */
  includeConnectorObservation?: boolean;
}

/** Automatic execution plus the caller-owned metadata trace of planning and provider phases. */
export interface AutonomousBrainAutoTraceOptions extends AutonomousBrainAutoExecuteOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedAutoExecution {
  execution: AutonomousBrainAutoExecution;
  trace: AutonomousRunTraceSummary;
}

export type AutonomousBrainAutoBatchOptionFactory<T> = T | ((input: AutonomousBrainRequest, index: number) => T);

export interface AutonomousBrainAutoBatchOptions {
  maxParallelism?: number;
  stopOnError?: boolean;
  /** One automatic policy for every item, or a caller-owned per-item policy factory. */
  execution?: AutonomousBrainAutoBatchOptionFactory<AutonomousBrainAutoExecuteOptions>;
}

export interface AutonomousBrainAutoBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainAutoExecution;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainAutoBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_AUTO_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainAutoBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_automatic_connector_values_transient";
  secret_material: "never_returned";
}

/** Automatic batch controls plus a caller-owned metadata-only trace sink. */
export interface AutonomousBrainAutoBatchTraceOptions extends AutonomousBrainAutoBatchOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Ordered automatic batch result paired with its hash-chained lifecycle trace. */
export interface AutonomousBrainTracedAutoBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_TRACED_AUTO_BATCH_SCHEMA;
  batch: AutonomousBrainAutoBatchResult;
  trace: AutonomousRunTraceSummary;
  retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

/** Restart-safe mission replanning controls plus a caller-owned metadata-only lifecycle trace. */
export interface AutonomousBrainMissionReplanTraceOptions extends AutonomousAgentMissionReplanOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Full mission values remain transient to the caller; the paired trace is digest-only. */
export interface AutonomousBrainTracedMissionReplanResult {
  schema: typeof AUTONOMOUS_BRAIN_TRACED_MISSION_REPLAN_SCHEMA;
  result: AutonomousMissionReplanResult;
  trace: AutonomousRunTraceSummary;
  retention: "mission_execution_values_caller_owned;trace_metadata_only_no_prompts_responses_arguments_or_credentials";
  secret_material: "never_returned";
}

/** Direct connector mission controls exposed by the high-level brain facade. */
export type AutonomousBrainConnectorMissionOptions = ConnectorMissionRunOptions;

/** Provider-planned connector mission controls exposed by the high-level brain facade. */
export type AutonomousBrainConnectorMissionProviderPlanningOptions = ConnectorMissionProviderPlanningOptions;

/** Caller-owned connector mission execution values; checkpoints and events remain metadata-only. */
export type AutonomousBrainConnectorMissionExecution = Awaited<ReturnType<typeof runAutonomousConnectorMission>>;

/** Two-phase connector mission result with a safe metadata-only JSON projection. */
export type AutonomousBrainPlannedConnectorMission = ConnectorPlannedMissionRun;

/** Reviewed adapter execution controls exposed by the application-facing brain facade. */
export type AutonomousBrainEvidenceBackedRunOptions = AutonomousEvidenceBackedRunOptions;

/** Evidence-backed execution result; raw evidence and provider values remain caller-owned. */
export type AutonomousBrainEvidenceBackedRunResult = AutonomousEvidenceBackedRunResult;

/** Source-catalogue execution controls exposed by the application-facing brain facade. */
export type AutonomousBrainDomainEvidenceBrainRunOptions = AutonomousDomainEvidenceBrainRunOptions;

/** Catalogue-backed execution result with a metadata-only serialized projection. */
export type AutonomousBrainDomainEvidenceBrainRunResult = AutonomousDomainEvidenceBrainRunResult;

/** Restart-safe evidence execution controls with caller-owned checkpoint persistence. */
export type AutonomousBrainEvidenceBackedResumableExecutionOptions = AutonomousEvidenceBackedResumableExecutionOptions;

/** Restart-safe evidence result; provider dispatch after a pending checkpoint remains explicit. */
export type AutonomousBrainEvidenceBackedResumableRun = AutonomousEvidenceBackedResumableRun;

/** Long-horizon goal runtime controls with the facade-owned agent and brain omitted. */
export type AutonomousBrainGoalAgentRuntimeOptions = Omit<AutonomousGoalAgentRuntimeOptions, "agent" | "brain">;

/** Long-horizon scheduler/worker/evaluator/learner runtime bound to this brain facade. */
export type AutonomousBrainGoalAgentRuntime = AutonomousGoalAgentRuntime;

/** One-call long-horizon execution with construction and control-loop options kept separate. */
export interface AutonomousBrainGoalAgentControlOptions {
  runtime: AutonomousBrainGoalAgentRuntimeOptions;
  run?: AutonomousGoalAgentLoopRunOptions;
}

/** Value-only control-loop result returned to the initiating caller. */
export type AutonomousBrainGoalAgentControlResult = AutonomousGoalControlLoopResult;

/** One-call, provider-free goal admission inspection with no task rehydration requirement. */
export interface AutonomousBrainGoalAgentPreviewOptions {
  runtime: AutonomousBrainGoalAgentRuntimeOptions;
  schedule_options?: Record<string, unknown>;
}

/** Digest-bound metadata-only goal admission preview. */
export type AutonomousBrainGoalAgentPreview = AutonomousGoalControlLoopPreview;

/** Restart-safe evidence controls plus a caller-owned hash-chained metadata trace. */
export interface AutonomousBrainEvidenceBackedResumableTraceOptions extends AutonomousBrainEvidenceBackedResumableExecutionOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Evidence-backed facade controls plus one caller-owned hash-chained metadata trace. */
export interface AutonomousBrainEvidenceBackedTraceOptions extends AutonomousBrainEvidenceBackedRunOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Catalogue-backed facade controls plus one caller-owned hash-chained metadata trace. */
export interface AutonomousBrainDomainEvidenceBrainTraceOptions extends AutonomousBrainDomainEvidenceBrainRunOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Live evidence-backed result paired with a serialized-safe execution trace. */
export interface AutonomousBrainTracedEvidenceBackedRunResult {
  result: AutonomousBrainEvidenceBackedRunResult;
  trace: AutonomousRunTraceSummary;
  retention: "result_values_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials";
  secret_material: "never_returned";
}

/** Live catalogue-backed result paired with a serialized-safe execution trace. */
export interface AutonomousBrainTracedDomainEvidenceBrainRunResult {
  result: AutonomousBrainDomainEvidenceBrainRunResult;
  trace: AutonomousRunTraceSummary;
  retention: "result_values_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials";
  secret_material: "never_returned";
}

/** Restart-safe evidence result paired with its checkpoint and metadata-only trace. */
export interface AutonomousBrainTracedEvidenceBackedResumableRun {
  run: AutonomousBrainEvidenceBackedResumableRun;
  trace: AutonomousRunTraceSummary;
  retention: "result_values_and_checkpoints_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials";
  secret_material: "never_returned";
}

/** Options for executing one caller-approved, digest-bound model-selection preview. */
export interface AutonomousBrainApprovedSelectionOptions {
  run?: Omit<AutonomousApprovedModelSelectionOptions, "domain">;
}

/** Caller-owned metadata trace controls for one digest-bound approved model arm. */
export interface AutonomousBrainApprovedSelectionTraceOptions extends AutonomousBrainApprovedSelectionOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Approved model-arm execution paired with a digest-only lifecycle trace. */
export interface AutonomousBrainTracedApprovedSelection {
  execution: AutonomousBrainExecution;
  trace: AutonomousRunTraceSummary;
}

type AutonomousBrainCycleBoundKeys = "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain" | "semanticRouting";

/** Single-domain cycle controls with route-owned fields reserved for the brain facade. */
export type AutonomousBrainSingleCycleOptions = Omit<AutonomousDecisionCycleOptions, AutonomousBrainCycleBoundKeys>;

/** Cross-domain cycle controls with route-owned fields reserved for the brain facade. */
export type AutonomousBrainCrossDomainCycleOptions = Omit<AutonomousCrossDomainDecisionCycleOptions, AutonomousBrainCycleBoundKeys>;

export interface AutonomousBrainCycleOptions {
  /** Explicit provider approval; defaults to false even when a model is registered. */
  approveProviderCall?: boolean;
  /** Run the optional connector operation before the closed-loop cycle; defaults to true. */
  connectorFirst?: boolean;
  /** Include the connector's transient bounded observation in the cycle context. */
  includeConnectorObservation?: boolean;
  /** Optional provider-assisted route proposal before the durable cycle owns the route. */
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  /** Evaluator, memory, learning, provider-planning, persistence, and budget controls. */
  cycle?: AutonomousBrainSingleCycleOptions | AutonomousBrainCrossDomainCycleOptions;
}

export type AutonomousBrainCycleResult = AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult;
export type AutonomousBrainCycleStatus = AutonomousBrainCycleResult["status"] | "connector_blocked";

type AutonomousBrainAutomaticCycleBoundKeys = "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain";

/** Automatic evaluator-backed cycle controls with route identity owned by the request. */
export type AutonomousBrainAutoCycleOptions = Omit<AutonomousAutoDecisionCycleOptions, AutonomousBrainAutomaticCycleBoundKeys>;
export type AutonomousBrainAutoCycleResult = AutonomousAutoDecisionCycleResult;

/** Automatic evaluator-guided replan controls with route identity owned by the request. */
export type AutonomousBrainAutoReplanCycleOptions = Omit<AutonomousAutoReplanCycleOptions, AutonomousBrainAutomaticCycleBoundKeys>;
export type AutonomousBrainAutoReplanCycleResult = AutonomousAutoReplanCycleResult;

export interface AutonomousBrainCycleExecution {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainCycleStatus;
  plan: AutonomousBrainPlanJSON;
  semantic_route?: AutonomousSemanticRouteResult | null;
  cycle: AutonomousBrainCycleResult | null;
  connector: AutonomousConnectorOperationExecution | null;
  error: { error_class: string; failure_code: string } | null;
  retention: "plan_metadata_only;cycle_response_and_connector_values_transient_to_caller";
  secret_material: "never_returned";
}

type AutonomousBrainAdaptiveCycleBoundKeys = AutonomousBrainCycleBoundKeys;

/** Single-domain evaluator-guided loop controls with route-owned fields reserved for the facade. */
export type AutonomousBrainSingleAdaptiveCycleOptions = Omit<AutonomousReplanCycleOptions, AutonomousBrainAdaptiveCycleBoundKeys>;

/** Cross-domain evaluator-guided loop controls with route-owned fields reserved for the facade. */
export type AutonomousBrainCrossDomainAdaptiveCycleOptions = Omit<AutonomousCrossDomainReplanCycleOptions, AutonomousBrainAdaptiveCycleBoundKeys>;

export interface AutonomousBrainAdaptiveCycleOptions {
  /** Explicit provider approval; defaults to false even when a model is registered. */
  approveProviderCall?: boolean;
  /** Run the optional connector operation before the first attempt; defaults to true. */
  connectorFirst?: boolean;
  /** Include the connector's transient bounded observation in every attempt's context. */
  includeConnectorObservation?: boolean;
  /** Optional provider-assisted route proposal before the adaptive loop owns the route. */
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  /** Evaluator, bounded replan, learning, persistence, memory, and budget controls. */
  adaptive: AutonomousBrainSingleAdaptiveCycleOptions | AutonomousBrainCrossDomainAdaptiveCycleOptions;
}

export type AutonomousBrainAdaptiveCycleResult = AutonomousReplanCycleResult | AutonomousCrossDomainReplanCycleResult;
export type AutonomousBrainAdaptiveCycleStatus = AutonomousBrainAdaptiveCycleResult["status"] | "connector_blocked";

export interface AutonomousBrainAdaptiveCycleExecution {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainAdaptiveCycleStatus;
  plan: AutonomousBrainPlanJSON;
  semantic_route?: AutonomousSemanticRouteResult | null;
  adaptive: AutonomousBrainAdaptiveCycleResult | null;
  connector: AutonomousConnectorOperationExecution | null;
  error: { error_class: string; failure_code: string } | null;
  retention: "plan_metadata_only;adaptive_responses_and_connector_values_transient_to_caller";
  secret_material: "never_returned";
}

export type AutonomousBrainBatchOptionFactory<T> = T | ((input: AutonomousBrainRequest, index: number) => T);

export interface AutonomousBrainCycleBatchOptions {
  maxParallelism?: number;
  stopOnError?: boolean;
  /** One cycle policy for every item, or a caller-owned per-item policy factory. */
  cycle?: AutonomousBrainBatchOptionFactory<AutonomousBrainCycleOptions>;
}

export interface AutonomousBrainCycleBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainCycleExecution;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainCycleBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_CYCLE_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainCycleBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_cycle_connector_values_transient";
  secret_material: "never_returned";
}

export interface AutonomousBrainAdaptiveBatchOptions {
  maxParallelism?: number;
  stopOnError?: boolean;
  /** Required evaluator/replan policy, shared or selected independently for each item. */
  adaptive: AutonomousBrainBatchOptionFactory<AutonomousBrainAdaptiveCycleOptions>;
}

export interface AutonomousBrainAdaptiveBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainAdaptiveCycleExecution;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainAdaptiveBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_ADAPTIVE_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainAdaptiveBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_adaptive_connector_values_transient";
  secret_material: "never_returned";
}

/** Bounded automatic evaluator-cycle controls with a shared or per-request policy factory. */
export interface AutonomousBrainAutoCycleBatchOptions {
  maxParallelism?: number;
  stopOnError?: boolean;
  cycle?: AutonomousBrainBatchOptionFactory<AutonomousBrainAutoCycleOptions>;
}

export interface AutonomousBrainAutoCycleBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainAutoCycleResult;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainAutoCycleBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_AUTO_CYCLE_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainAutoCycleBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_automatic_cycle_values_transient";
  secret_material: "never_returned";
}

/** Automatic evaluator-cycle batch plus one caller-owned metadata-only lifecycle trace. */
export interface AutonomousBrainAutoCycleBatchTraceOptions extends AutonomousBrainAutoCycleBatchOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedAutoCycleBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_TRACED_AUTO_CYCLE_BATCH_SCHEMA;
  batch: AutonomousBrainAutoCycleBatchResult;
  trace: AutonomousRunTraceSummary;
  retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

/** Bounded automatic evaluator/replan controls with a shared or per-request policy factory. */
export interface AutonomousBrainAutoReplanBatchOptions {
  maxParallelism?: number;
  stopOnError?: boolean;
  replan: AutonomousBrainBatchOptionFactory<AutonomousBrainAutoReplanCycleOptions>;
}

export interface AutonomousBrainAutoReplanBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainAutoReplanCycleResult;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainAutoReplanBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_AUTO_REPLAN_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainAutoReplanBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_automatic_replan_values_transient";
  secret_material: "never_returned";
}

/** Automatic evaluator/replan batch plus one caller-owned metadata-only lifecycle trace. */
export interface AutonomousBrainAutoReplanBatchTraceOptions extends AutonomousBrainAutoReplanBatchOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainTracedAutoReplanBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_TRACED_AUTO_REPLAN_BATCH_SCHEMA;
  batch: AutonomousBrainAutoReplanBatchResult;
  trace: AutonomousRunTraceSummary;
  retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

/** Restart-safe automatic evaluator-cycle batch controls. */
export interface AutonomousBrainAutoCycleBatchResumableOptions extends AutonomousBrainAutoCycleBatchOptions {
  jobId: string;
  /** Optional caller-owned identity for opaque evaluator/learning callback implementations. */
  policyDigest?: string;
  checkpoint?: AutonomousBrainBatchCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousBrainBatchCheckpointJSON) => Promise<void> | void;
  rehydrateCycle?: (context: AutonomousBrainBatchRehydrationContext) => Promise<AutonomousBrainAutoCycleResult> | AutonomousBrainAutoCycleResult;
}

/** Restart-safe automatic evaluator/replan batch controls. */
export interface AutonomousBrainAutoReplanBatchResumableOptions extends AutonomousBrainAutoReplanBatchOptions {
  jobId: string;
  /** Optional caller-owned identity for opaque evaluator/learning callback implementations. */
  policyDigest?: string;
  checkpoint?: AutonomousBrainBatchCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousBrainBatchCheckpointJSON) => Promise<void> | void;
  rehydrateReplan?: (context: AutonomousBrainBatchRehydrationContext) => Promise<AutonomousBrainAutoReplanCycleResult> | AutonomousBrainAutoReplanCycleResult;
}

export interface AutonomousBrainAutoCycleBatchResumableTraceOptions extends AutonomousBrainAutoCycleBatchResumableOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainAutoReplanBatchResumableTraceOptions extends AutonomousBrainAutoReplanBatchResumableOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

export interface AutonomousBrainAutoCycleBatchControllerRun {
  controller: AutonomousBrainBatchControllerProjection;
  batch: AutonomousBrainAutoCycleBatchResult;
}

export interface AutonomousBrainAutoReplanBatchControllerRun {
  controller: AutonomousBrainBatchControllerProjection;
  batch: AutonomousBrainAutoReplanBatchResult;
}

export type AutonomousBrainAutoCycleBatchControllerRunOptions = Omit<AutonomousBrainAutoCycleBatchResumableOptions, "checkpoint" | "checkpointSink">;
export type AutonomousBrainAutoReplanBatchControllerRunOptions = Omit<AutonomousBrainAutoReplanBatchResumableOptions, "checkpoint" | "checkpointSink">;

export interface AutonomousBrainAutoCycleBatchControllerTraceRun {
  controller: AutonomousBrainBatchControllerProjection;
  traced: AutonomousBrainTracedAutoCycleBatchResult;
}

export interface AutonomousBrainAutoReplanBatchControllerTraceRun {
  controller: AutonomousBrainBatchControllerProjection;
  traced: AutonomousBrainTracedAutoReplanBatchResult;
}

export type AutonomousBrainAutoCycleBatchControllerTraceRunOptions = Omit<AutonomousBrainAutoCycleBatchResumableTraceOptions, "checkpoint" | "checkpointSink">;
export type AutonomousBrainAutoReplanBatchControllerTraceRunOptions = Omit<AutonomousBrainAutoReplanBatchResumableTraceOptions, "checkpoint" | "checkpointSink">;

export type AutonomousBrainTraceRegistryControllerStatus =
  | "empty"
  | "restored"
  | "flushed"
  | "published"
  | "compacted"
  | "publication_failed"
  | "persistence_failed";

/** Construction controls for the facade-bound, metadata-only trace registry controller. */
export interface AutonomousBrainTraceRegistryControllerOptions {
  registry: AutonomousRunTraceRegistry;
  persistence: JsonAutonomousRunTraceRegistryPersistence;
}

/** Operator-safe projection of the trace registry controller state. */
export interface AutonomousBrainTraceRegistryControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA;
  status: AutonomousBrainTraceRegistryControllerStatus;
  snapshot_generation: number | null;
  snapshot_digest: string | null;
  runs: number;
  events: number;
  retained_event_count: number;
  policy: AutonomousRunTraceRegistrySnapshot["policy"] | null;
  persisted: boolean;
  retention: AutonomousRunTraceRegistrySnapshot["retention"];
  authority: AutonomousRunTraceRegistrySnapshot["authority"];
  secret_material: AutonomousRunTraceRegistrySnapshot["secret_material"];
}

/** Publication result with persistence separated from in-memory registry ingestion. */
export interface AutonomousBrainTraceRegistryPublicationRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  publication: AutonomousRunTraceRegistryPublication;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Imported trace snapshot with an explicit persistence outcome. */
export interface AutonomousBrainTraceRegistryImportRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  report: AutonomousRunTraceRegistryImportReport;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Retention-compaction result with an explicit persistence outcome. */
export interface AutonomousBrainTraceRegistryCompactRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  evicted_run_ids: string[];
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Options for the keyless readiness audit exposed at the application boundary. */
export type AutonomousBrainReadinessOptions = Parameters<AutonomousAgent["readiness"]>[0];
export type AutonomousBrainReadinessReport = Awaited<ReturnType<AutonomousAgent["readiness"]>>;
export type AutonomousBrainWorkflowPortfolioPlanOptions = AutonomousWorkflowPortfolioPlanOptions;
export type AutonomousBrainWorkflowPortfolioPlan = AutonomousWorkflowPortfolioPlan;
export type AutonomousBrainWorkflowPortfolioVerification = AutonomousWorkflowPortfolioVerification;
export type AutonomousBrainWorkflowPortfolioExecutionOptions = AutonomousWorkflowPortfolioExecutionOptions;
export type AutonomousBrainWorkflowPortfolioExecutionResult = AutonomousWorkflowPortfolioExecutionResult;
export type AutonomousBrainWorkflowPortfolioResumableExecutionOptions = AutonomousWorkflowPortfolioResumableExecutionOptions;
export type AutonomousBrainWorkflowPortfolioEvidenceSupervisorOptions = AutonomousWorkflowPortfolioEvidenceSupervisorOptions;
export type AutonomousBrainWorkflowPortfolioEvidenceResumableExecutionOptions = AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions;
export type AutonomousBrainWorkflowPortfolioEvidenceExecutionResult = AutonomousWorkflowPortfolioEvidenceExecutionResult;
export type AutonomousBrainWorkflowPortfolioAdmissionOptions = AutonomousWorkflowPortfolioAdmissionOptions;
export type AutonomousBrainWorkflowPortfolioAdmission = AutonomousWorkflowPortfolioAdmission;
export type AutonomousBrainActivationState = ReturnType<AutonomousAgent["activationState"]>;
export type AutonomousBrainActivationSnapshotStore = AutonomousCapabilityActivationSnapshotStore;
/** Provider-free information-acquisition controls bound to the facade's deterministic route. */
export type AutonomousBrainInformationAcquisitionOptions = Parameters<AutonomousAgent["planInformationAcquisition"]>[1];
export type AutonomousBrainInformationAcquisitionPlan = AutonomousInformationAcquisitionPlan;
export type AutonomousBrainInformationAcquisitionReplanOptions = ReplanAutonomousInformationAcquisitionOptions;
/** Claim-integrity assessment controls with task text kept transient to the caller. */
export type AutonomousBrainClaimIntegrityAssessmentOptions = Parameters<AutonomousAgent["assessClaimIntegrity"]>[1];
export type AutonomousBrainClaimIntegrityAssessment = AutonomousClaimIntegrityAssessment;
export type AutonomousBrainClaimIntegrityReassessmentOptions = Parameters<AutonomousAgent["reassessClaimIntegrity"]>[1];
export type AutonomousBrainClaimIntegrityAcquisitionPlanOptions = Parameters<AutonomousAgent["planClaimIntegrityAcquisition"]>[1];
export type AutonomousBrainClaimIntegrityAcquisitionBridge = AutonomousClaimIntegrityAcquisitionBridge;
export type AutonomousBrainClaimIntegrityAcquisitionBinding = AutonomousClaimIntegrityAcquisitionBinding;
export type AutonomousBrainClaimIntegrityAcquisitionExecutionOptions = Parameters<AutonomousAgent["executeClaimIntegrityAcquisition"]>[3];
export type AutonomousBrainClaimIntegrityAcquisitionExecutionResult = Awaited<ReturnType<AutonomousAgent["executeClaimIntegrityAcquisition"]>>;
export type AutonomousBrainClaimIntegrityAcquisitionResumableOptions = Parameters<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>[3];
export type AutonomousBrainClaimIntegrityAcquisitionResumableResult = Awaited<ReturnType<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>>;
/** Digest-only identity projected from a completed direct or cross-domain result. */
export type AutonomousBrainOutcomeIntegrityRun = AutonomousOutcomeIntegrityRun;
export type AutonomousBrainOutcomeIntegrityClaimBindingInput = AutonomousOutcomeIntegrityClaimBindingInput;
export type AutonomousBrainOutcomeIntegrityClaimBinding = AutonomousOutcomeIntegrityClaimBinding;
export type AutonomousBrainOutcomeIntegrityAssessmentOptions = Omit<AssessAutonomousOutcomeIntegrityOptions, "run">;
export type AutonomousBrainOutcomeIntegrityAssessment = AutonomousOutcomeIntegrityAssessment;
/** Cross-domain specialist/synthesis responses remain transient; only the alignment gate persists. */
export type AutonomousBrainCrossDomainResponseEntry = AutonomousCrossDomainResponseEntry;
export type AutonomousBrainCrossDomainResponseAlignmentInput = AutonomousCrossDomainResponseAlignmentInput;
export type AutonomousBrainCrossDomainResponseAssessment = AutonomousCrossDomainResponseAssessment;
export type AutonomousBrainCrossDomainResponseAssessmentOptions = Parameters<AutonomousAgent["assessCrossDomainResponses"]>[1];
export type AutonomousBrainCrossDomainResponseReplayOptions = Parameters<typeof replayAutonomousCrossDomainResponseAssessment>[2];
/** Capability/tool execution contracts retained at the facade without exposing raw values. */
export type AutonomousBrainToolCall = Parameters<AutonomousAgent["executeToolCalls"]>[0][number];
export type AutonomousBrainToolCallOptions = Parameters<AutonomousAgent["executeToolCalls"]>[1];
export type AutonomousBrainToolResult = Awaited<ReturnType<AutonomousAgent["executeToolCalls"]>>[number];
export type AutonomousBrainCapabilityExecutionRequest = Parameters<AutonomousAgent["executeCapability"]>[0];
export type AutonomousBrainCapabilityExecutionOptions = Parameters<AutonomousAgent["executeCapability"]>[1];
export type AutonomousBrainCapabilityExecutionResult = Awaited<ReturnType<AutonomousAgent["executeCapability"]>>;
export type AutonomousBrainCapabilityBatchOptions = Parameters<AutonomousAgent["executeCapabilityBatch"]>[1];
export type AutonomousBrainCapabilityBatchResult = Awaited<ReturnType<AutonomousAgent["executeCapabilityBatch"]>>;
export type AutonomousBrainCapabilityExecutionRecord = ReturnType<AutonomousAgent["capabilityExecutionEvidence"]>[number];
export type AutonomousBrainCapabilityLearningOptions = Parameters<AutonomousAgent["evaluateCapabilityExecution"]>[1];
export type AutonomousBrainCapabilityLearningResult = Awaited<ReturnType<AutonomousAgent["evaluateCapabilityExecution"]>>;
export type AutonomousBrainCapabilityLearningBatchOptions = Parameters<AutonomousAgent["evaluateCapabilityExecutions"]>[1];
export type AutonomousBrainCapabilityLearningBatchResult = Awaited<ReturnType<AutonomousAgent["evaluateCapabilityExecutions"]>>;
export type AutonomousBrainToolExecutionReceipt = ReturnType<AutonomousAgent["toolExecutionEvidence"]>[number];
export type AutonomousBrainToolLearningOptions = Parameters<AutonomousAgent["evaluateToolReceipts"]>[0];
export type AutonomousBrainToolLearningResult = Awaited<ReturnType<AutonomousAgent["evaluateToolReceipts"]>>;
export type AutonomousBrainProviderLearningOptions = Parameters<AutonomousAgent["evaluateProviderReceipts"]>[0];
export type AutonomousBrainProviderLearningResult = Awaited<ReturnType<AutonomousAgent["evaluateProviderReceipts"]>>;
/** Caller-owned stores and strictness controls for the coordinated restart lifecycle. */
export type AutonomousBrainPersistenceLifecycleOptions = AutonomousAgentPersistenceLifecycleOptions;
export type AutonomousBrainPersistenceLifecycleRestoreOptions = Parameters<AutonomousAgent["restorePersistedState"]>[0];
export type AutonomousBrainPersistenceLifecycleFlushOptions = Parameters<AutonomousAgent["flushPersistedState"]>[0];
export type AutonomousBrainPersistenceLifecycleReport = AutonomousAgentPersistenceLifecycleReport;
/** Protected discovery and inventory controls; credentials remain inside `CredentialSession`. */
export type AutonomousBrainModelDiscovery = ProviderModelDiscovery;
export type AutonomousBrainModelCandidateDefaults = AutonomousModelCandidateDefaults;
export type AutonomousBrainModelCandidate = AutonomousModelCandidate;
export type AutonomousBrainModelInventoryRefreshOptions = Omit<AutonomousModelInventoryRefreshOptions, "credentialFor" | "credentialSession">;
export type AutonomousBrainModelInventorySnapshot = AutonomousModelInventorySnapshot;
export type AutonomousBrainModelInventoryReadinessOptions = AutonomousModelInventoryReadinessOptions;
export type AutonomousBrainModelInventoryReadiness = AutonomousModelInventoryReadiness;

export interface AutonomousBrainBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  task_digest: string | null;
  execution?: AutonomousBrainExecution;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousBrainBatchResult {
  schema: typeof AUTONOMOUS_BRAIN_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousBrainBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  batch_digest: string;
  retention: "metadata_only_tasks_and_provider_connector_values_transient";
  secret_material: "never_returned";
}

export type AutonomousBrainBatchMode = "brain" | "automatic" | "automatic_cycle" | "automatic_replan";

export interface AutonomousBrainBatchRehydrationContext {
  job_id: string;
  index: number;
  mode: AutonomousBrainBatchMode;
  request_digest: string;
  task_digest: string;
  expected_result_digest: string;
}

/**
 * Adapt the protected receipt boundary to restart-safe brain batches.
 *
 * Checkpoints contain only digests. The receipt resolver sees the same bounded identity fields
 * that the batch engine verifies, while the adapter owns tenant/authorization/replay fencing.
 * A decoder is available for callers whose protected store returns a canonical JSON projection
 * that must be rebuilt into a richer in-memory execution object.
 */
export class AutonomousBrainBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainExecution | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => AutonomousBrainExecution | unknown;
    domain?: AutonomousDomainName;
    purpose?: string;
    valueKind?: string;
    oneTime?: boolean;
    digestScheme?: string;
  }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain batch protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain batch protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain batch protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain batch protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainExecution> {
    if (!context || context.mode !== "brain") throw new ArgumentError("autonomous brain batch protected rehydrator requires a direct brain checkpoint context");
    let receipt: unknown;
    try {
      receipt = await this.receiptResolver(context);
    } catch (error) {
      throw new ArgumentError(`autonomous brain batch protected receipt lookup failed for item ${context.index}`, { cause: error });
    }
    if (!isObject(receipt)) throw new ArgumentError("autonomous brain batch protected receiptResolver must return an object");
    for (const [key, expected] of [
      ["job_id", context.job_id],
      ["index", context.index],
      ["mode", context.mode],
      ["request_digest", context.request_digest],
      ["task_digest", context.task_digest],
      ["expected_result_digest", context.expected_result_digest],
    ] as const) {
      if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain batch protected receipt ${key} does not match item ${context.index}`);
    }
    try {
      const value = this.adapter.resolveReceipt(receipt, {
        domain: this.domain,
        purpose: this.purpose,
        valueKind: this.valueKind,
        oneTime: this.oneTime,
        digestScheme: this.digestScheme,
      });
      const decoded = this.valueDecoder === undefined ? value : await this.valueDecoder(value);
      if (!isObject(decoded)) throw new ArgumentError(`autonomous brain batch protected result for item ${context.index} is not an execution object`);
      return decoded as unknown as AutonomousBrainExecution;
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError(`autonomous brain batch protected result resolution failed for item ${context.index}`, { cause: error });
    }
  }
}

/** Protected-receipt adapter for automatic batches; it never widens a direct checkpoint into automatic execution. */
export class AutonomousBrainAutoBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoExecution | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => AutonomousBrainAutoExecution | unknown;
    domain?: AutonomousDomainName;
    purpose?: string;
    valueKind?: string;
    oneTime?: boolean;
    digestScheme?: string;
  }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic batch protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic batch protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic batch protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic batch protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoExecution> {
    if (!context || context.mode !== "automatic") throw new ArgumentError("autonomous brain automatic batch protected rehydrator requires an automatic checkpoint context");
    let receipt: unknown;
    try {
      receipt = await this.receiptResolver(context);
    } catch (error) {
      throw new ArgumentError(`autonomous brain automatic batch protected receipt lookup failed for item ${context.index}`, { cause: error });
    }
    if (!isObject(receipt)) throw new ArgumentError("autonomous brain automatic batch protected receiptResolver must return an object");
    for (const [key, expected] of [
      ["job_id", context.job_id],
      ["index", context.index],
      ["mode", context.mode],
      ["request_digest", context.request_digest],
      ["task_digest", context.task_digest],
      ["expected_result_digest", context.expected_result_digest],
    ] as const) {
      if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain automatic batch protected receipt ${key} does not match item ${context.index}`);
    }
    try {
      const value = this.adapter.resolveReceipt(receipt, {
        domain: this.domain,
        purpose: this.purpose,
        valueKind: this.valueKind,
        oneTime: this.oneTime,
        digestScheme: this.digestScheme,
      });
      const decoded = this.valueDecoder === undefined ? value : await this.valueDecoder(value);
      if (!isObject(decoded)) throw new ArgumentError(`autonomous brain automatic batch protected result for item ${context.index} is not an execution object`);
      return decoded as unknown as AutonomousBrainAutoExecution;
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError(`autonomous brain automatic batch protected result resolution failed for item ${context.index}`, { cause: error });
    }
  }
}

async function resolveAutomaticCycleBatchProtectedValue(
  label: string,
  expectedMode: "automatic_cycle" | "automatic_replan",
  options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => unknown;
    domain?: AutonomousDomainName;
    purpose: string;
    valueKind: string;
    oneTime: boolean;
    digestScheme: string;
  },
  context: AutonomousBrainBatchRehydrationContext,
): Promise<unknown> {
  if (!context || context.mode !== expectedMode) throw new ArgumentError(`autonomous brain ${label} protected rehydrator requires a ${expectedMode} checkpoint context`);
  let receipt: unknown;
  try {
    receipt = await options.receiptResolver(context);
  } catch (error) {
    throw new ArgumentError(`autonomous brain ${label} protected receipt lookup failed for item ${context.index}`, { cause: error });
  }
  if (!isObject(receipt)) throw new ArgumentError(`autonomous brain ${label} protected receiptResolver must return an object`);
  for (const [key, expected] of [
    ["job_id", context.job_id],
    ["index", context.index],
    ["mode", context.mode],
    ["request_digest", context.request_digest],
    ["task_digest", context.task_digest],
    ["expected_result_digest", context.expected_result_digest],
  ] as const) {
    if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain ${label} protected receipt ${key} does not match item ${context.index}`);
  }
  try {
    const value = options.adapter.resolveReceipt(receipt, { domain: options.domain, purpose: options.purpose, valueKind: options.valueKind, oneTime: options.oneTime, digestScheme: options.digestScheme });
    const decoded = options.valueDecoder === undefined ? value : await options.valueDecoder(value);
    if (!isObject(decoded)) throw new ArgumentError(`autonomous brain ${label} protected result for item ${context.index} is not an execution object`);
    return decoded;
  } catch (error) {
    if (error instanceof ArgumentError) throw error;
    throw new ArgumentError(`autonomous brain ${label} protected result resolution failed for item ${context.index}`, { cause: error });
  }
}

/** Protected-receipt adapter for automatic evaluator-cycle batch results. */
export class AutonomousBrainAutoCycleBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoCycleResult | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: { adapter: AutonomousProtectedRehydrationAdapter; receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>; valueDecoder?: (value: unknown) => AutonomousBrainAutoCycleResult | unknown; domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; digestScheme?: string }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic cycle protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_cycle_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_cycle_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoCycleResult> {
    return await resolveAutomaticCycleBatchProtectedValue("automatic cycle", "automatic_cycle", this, context) as AutonomousBrainAutoCycleResult;
  }
}

/** Protected-receipt adapter for automatic evaluator/replan batch results. */
export class AutonomousBrainAutoReplanBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoReplanCycleResult | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: { adapter: AutonomousProtectedRehydrationAdapter; receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>; valueDecoder?: (value: unknown) => AutonomousBrainAutoReplanCycleResult | unknown; domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; digestScheme?: string }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic replan protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic replan protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic replan protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic replan protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_replan_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_replan_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoReplanCycleResult> {
    return await resolveAutomaticCycleBatchProtectedValue("automatic replan", "automatic_replan", this, context) as AutonomousBrainAutoReplanCycleResult;
  }
}

export interface AutonomousBrainBatchCheckpointJSON {
  schema: typeof AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA;
  job_id: string;
  mode: AutonomousBrainBatchMode;
  batch_input_digest: string;
  /** Digest of the non-secret semantic-routing policy; absent only on legacy deterministic checkpoints. */
  semantic_routing_policy_digest?: string;
  /** Digest of the non-secret automatic execution policy; present for automatic checkpoints. */
  automatic_execution_policy_digest?: string;
  request_digests: string[];
  completed_indices: number[];
  completed_result_digests: string[];
  max_parallelism: number;
  stop_on_error: boolean;
  status: "running" | "partial" | "completed";
  checkpoint_digest: string;
  retention: "request_and_result_digests_only;tasks_prompts_credentials_and_payloads_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousBrainResumableBatchOptions {
  jobId: string;
  maxParallelism?: number;
  stopOnError?: boolean;
  execution?: AutonomousBrainExecuteOptions;
  checkpoint?: AutonomousBrainBatchCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousBrainBatchCheckpointJSON) => Promise<void> | void;
  rehydrateExecution?: (context: AutonomousBrainBatchRehydrationContext) => Promise<AutonomousBrainExecution> | AutonomousBrainExecution;
}

/** Restart-safe automatic route -> blueprint -> invocation batch controls. */
export interface AutonomousBrainAutoBatchResumableOptions {
  jobId: string;
  maxParallelism?: number;
  stopOnError?: boolean;
  execution?: AutonomousBrainAutoExecuteOptions;
  checkpoint?: AutonomousBrainBatchCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousBrainBatchCheckpointJSON) => Promise<void> | void;
  rehydrateExecution?: (context: AutonomousBrainBatchRehydrationContext) => Promise<AutonomousBrainAutoExecution> | AutonomousBrainAutoExecution;
}

/** Restart-safe automatic batch controls plus one caller-owned lifecycle trace. */
export interface AutonomousBrainAutoBatchResumableTraceOptions extends AutonomousBrainAutoBatchResumableOptions {
  traceStore: AutonomousRunTraceStore;
  runId: string;
}

/** Caller-owned storage for one verified metadata-only brain batch checkpoint. */
export interface AutonomousBrainBatchCheckpointStore {
  read(): Promise<AutonomousBrainBatchCheckpointJSON | null> | AutonomousBrainBatchCheckpointJSON | null;
  write(checkpoint: AutonomousBrainBatchCheckpointJSON): Promise<void> | void;
}

export type AutonomousBrainBatchControllerStatus = "empty" | "restored" | "flushed" | "completed" | "partial" | "failed";

export interface AutonomousBrainBatchControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA;
  status: AutonomousBrainBatchControllerStatus;
  job_id: string | null;
  checkpoint_digest: string | null;
  completed_items: number;
  total_items: number | null;
  persisted: true;
  retention: "metadata_only_request_and_result_digests;task_prompt_provider_connector_values_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousBrainBatchControllerRun {
  controller: AutonomousBrainBatchControllerProjection;
  batch: AutonomousBrainBatchResult;
}

export interface AutonomousBrainAutoBatchControllerRun {
  controller: AutonomousBrainBatchControllerProjection;
  batch: AutonomousBrainAutoBatchResult;
}

export type AutonomousBrainBatchControllerRunOptions = Omit<AutonomousBrainResumableBatchOptions, "checkpoint" | "checkpointSink">;
export type AutonomousBrainAutoBatchControllerRunOptions = Omit<AutonomousBrainAutoBatchResumableOptions, "checkpoint" | "checkpointSink">;

interface PreparedBrainRequest {
  readonly request: AutonomousBrainRequest;
  readonly route: AutonomousRouteProposal;
  readonly semanticRoute: AutonomousSemanticRouteResult | null;
  readonly semanticBudget: AutonomousCostBudget | null;
  readonly plan: AutonomousBrainPlan;
  readonly connectorPlan: AutonomousConnectorOperationPlan | null;
}

const PLAN_RETENTION = "metadata_only_task_prompt_connector_request_and_provider_values_not_retained" as const;
const SUMMARY_RETENTION = "metadata_only_task_prompt_and_provider_values_not_retained" as const;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function domain(name: string, value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(value as AutonomousDomainName)) throw new ArgumentError(`${name} is not a supported autonomous domain`);
  return value as AutonomousDomainName;
}

function bindAutomaticCycleOptions(request: AutonomousBrainRequest, options: unknown): Record<string, unknown> {
  if (!isObject(options) || Array.isArray(options)) throw new ArgumentError("autonomous brain automatic cycle options must be an object");
  const raw = options as Record<string, unknown>;
  const routeBound = ["routeOverride", "domain", "capability", "context", "hints", "allowCrossDomain"] as const;
  const supplied = routeBound.find((key) => raw[key] !== undefined);
  if (supplied !== undefined) throw new ArgumentError(`autonomous brain automatic cycle owns ${supplied}; provide route fields on the request`);
  const {
    domain: _domain,
    routeOverride: _routeOverride,
    capability: _capability,
    context: _context,
    hints: _hints,
    allowCrossDomain: _allowCrossDomain,
    ...cycleOptions
  } = raw;
  return {
    ...cycleOptions,
    domain: request.domain,
    capability: request.capability,
    context: request.context,
    hints: request.hints,
    allowCrossDomain: request.allow_cross_domain,
  };
}

function errorProjection(error: unknown): { error_class: string; failure_code: string } {
  if (error instanceof ProviderRuntimeError) return { error_class: error.constructor.name, failure_code: error.code };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { error_class: error.constructor.name, failure_code: "error" };
  return { error_class: "AutonomousBrainError", failure_code: "error" };
}

function composeBrainObservers(...observers: readonly (ProviderInvocationObserver | undefined)[]): ProviderInvocationObserver | undefined {
  const active = observers.filter((observer): observer is ProviderInvocationObserver => observer !== undefined);
  if (!active.length) return undefined;
  return {
    before: async (metadata) => {
      for (const observer of active) await observer.before?.(metadata);
    },
    after: async (metadata, outcome) => {
      for (const observer of active) await observer.after?.(metadata, outcome);
    },
  };
}

function connectorSucceeded(status: AutonomousConnectorOperationExecution["status"]): boolean {
  return status === "observed" || status === "partial";
}

function batchOption<T>(value: AutonomousBrainBatchOptionFactory<T> | undefined, input: AutonomousBrainRequest, index: number): T | undefined {
  return typeof value === "function" ? (value as (request: AutonomousBrainRequest, itemIndex: number) => T)(input, index) : value;
}

function boundedBatchControls(options: { maxParallelism?: number; stopOnError?: boolean }): { maxParallelism: number; stopOnError: boolean } {
  const maxParallelism = options.maxParallelism ?? 4;
  if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > MAX_AUTONOMOUS_BRAIN_PARALLELISM) throw new ArgumentError("autonomous brain batch maxParallelism is outside its bound");
  const stopOnError = options.stopOnError ?? false;
  if (typeof stopOnError !== "boolean") throw new ArgumentError("autonomous brain batch stopOnError must be boolean");
  return { maxParallelism, stopOnError };
}

function cycleBatchSucceeded(status: AutonomousBrainCycleStatus): boolean {
  return status === "completed" || status === "children_completed";
}

function adaptiveBatchSucceeded(status: AutonomousBrainAdaptiveCycleStatus): boolean {
  return status === "completed";
}

function batchRefused(status: string): boolean {
  return status === "approval_required"
    || status === "route_review_required"
    || status === "plan_review_required"
    || status === "connector_blocked"
    || status === "provider_invalid"
    || status === "provider_disagreement";
}

function automaticBatchRefused(status: string): boolean {
  return batchRefused(status)
    || status === "provider_abstained"
    || status === "policy_review_required"
    || status === "policy_blocked"
    || status === "reconciliation_required"
    || status === "response_review_required"
    || status === "children_partial"
    || status === "child_failed";
}

function automaticCycleBatchSucceeded(status: string): boolean {
  return status === "completed" || status === "children_completed";
}

function automaticReplanBatchSucceeded(status: string): boolean {
  return status === "completed";
}

function batchStatus(completed: number, failed: number, omitted: number): "completed" | "partial" | "failed" {
  return failed === 0 && omitted === 0 ? "completed" : completed > 0 ? "partial" : "failed";
}

function batchDigest(items: readonly { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }[]): string {
  return digestJsonSync(items.map((item) => batchItemProjection(item)));
}

function automaticCycleBatchDigest(items: readonly {
  index: number;
  status: string;
  task_digest: string | null;
  error_class?: string;
  failure_code?: string;
  execution?: {
    status: string;
    mode: string | null;
    route: { route_digest: string };
    cycle: { schema: string; status: string } | null;
  };
}[]): string {
  return digestJsonSync(items.map((item) => ({
    index: item.index,
    status: item.status,
    task_digest: item.task_digest,
    error_class: item.error_class ?? null,
    failure_code: item.failure_code ?? null,
    mode: item.execution?.mode ?? null,
    route_digest: item.execution?.route.route_digest ?? null,
    cycle_schema: item.execution?.cycle?.schema ?? null,
    cycle_status: item.execution?.cycle?.status ?? null,
  })));
}

function batchItemProjection(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }): Record<string, unknown> {
  return { index: item.index, status: item.status, task_digest: item.task_digest, error_class: item.error_class ?? null, failure_code: item.failure_code ?? null, plan_digest: item.execution?.plan.plan_digest ?? null, execution_status: item.execution?.status ?? null };
}

function batchItemDigest(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }): string {
  return digestJsonSync(batchItemProjection(item));
}

function automaticCycleBatchItemDigest(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: unknown }): string {
  const execution = isObject(item.execution) ? item.execution : null;
  const route = execution !== null && isObject(execution.route) ? execution.route : null;
  const cycle = execution !== null && isObject(execution.cycle) ? execution.cycle : null;
  return digestJsonSync({
    index: item.index,
    status: item.status,
    task_digest: item.task_digest,
    error_class: item.error_class ?? null,
    failure_code: item.failure_code ?? null,
    execution_schema: execution?.schema ?? null,
    execution_status: execution?.status ?? null,
    execution_mode: execution?.mode ?? null,
    route_digest: route?.route_digest ?? null,
    cycle_schema: cycle?.schema ?? null,
    cycle_status: cycle?.status ?? null,
    next_action: execution?.next_action ?? null,
  });
}

function checkpointBatchItemDigest(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: unknown }): string {
  const execution = isObject(item.execution) ? item.execution : null;
  return execution !== null && isObject(execution.plan)
    ? batchItemDigest(item as { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } })
    : automaticCycleBatchItemDigest(item);
}

/**
 * Keep caller-owned automatic values usable for rehydration without making them part of the
 * serializable traced envelope.  A traced batch is routinely passed to logs, telemetry, and
 * persistence adapters; its ordinary JSON image must therefore remain metadata-only while the
 * direct property remains available to the caller that owns the transient result.
 */
function tracedAutoBatchResult(batch: AutonomousBrainAutoBatchResult): AutonomousBrainAutoBatchResult {
  return {
    ...batch,
    items: batch.items.map((item) => {
      if (item.execution === undefined) return { ...item };
      const projection = { ...item } as AutonomousBrainAutoBatchItem;
      Object.defineProperty(projection, "execution", {
        value: item.execution,
        enumerable: false,
        configurable: false,
        writable: false,
      });
      return projection;
    }),
  };
}

function tracedAutomaticCycleBatchResult<T extends AutonomousBrainAutoCycleBatchResult | AutonomousBrainAutoReplanBatchResult>(batch: T): T {
  return {
    ...batch,
    items: batch.items.map((item) => {
      if (item.execution === undefined) return { ...item };
      const projection = { ...item } as typeof item;
      Object.defineProperty(projection, "execution", {
        value: item.execution,
        enumerable: false,
        configurable: false,
        writable: false,
      });
      return projection;
    }),
  } as T;
}

function validateMissionForBrain(mission: AgentMissionArgs): AgentMissionArgs {
  if (!isObject(mission)) throw new ArgumentError("autonomous brain mission must be an object");
  const candidate = mission as unknown as AgentMissionArgs;
  boundedIdentifier("autonomous brain mission_id", candidate.mission_id);
  boundedText("autonomous brain mission goal", candidate.goal, 32_000);
  if (!Array.isArray(candidate.steps) || candidate.steps.length < 1 || candidate.steps.length > 128) throw new ArgumentError("autonomous brain mission steps must contain one to 128 entries");
  const ids = new Set<string>();
  for (const [index, raw] of candidate.steps.entries()) {
    if (!isObject(raw)) throw new ArgumentError(`autonomous brain mission step ${index} must be an object`);
    const step = raw as unknown as AgentMissionArgs["steps"][number];
    const id = boundedIdentifier(`autonomous brain mission step ${index} id`, step.id);
    if (ids.has(id)) throw new ArgumentError(`autonomous brain mission contains duplicate step id: ${id}`);
    ids.add(id);
    domain(`autonomous brain mission step ${index} domain`, step.domain);
    boundedText(`autonomous brain mission step ${index} objective`, step.objective, 32_000);
    boundedIdentifier(`autonomous brain mission step ${index} tool`, step.tool);
  }
  return candidate;
}

function missionDomains(mission: AgentMissionArgs): AutonomousDomainName[] {
  const domains = [...new Set(validateMissionForBrain(mission).steps.map((step) => domain("autonomous brain mission domain", step.domain)))];
  if (domains.length === 0) throw new ArgumentError("autonomous brain mission must declare at least one supported domain");
  return domains;
}

function workflowPortfolioDomains(plan: Pick<AutonomousWorkflowPortfolioPlan, "items">): AutonomousDomainName[] {
  if (!plan || !Array.isArray(plan.items) || plan.items.length < 1) throw new ArgumentError("autonomous brain workflow portfolio launch admission requires a non-empty plan");
  const domains = [...new Set(plan.items.map((item, index) => domain(`autonomous brain workflow portfolio item ${index} domain`, item.domain)))];
  if (domains.length < 1) throw new ArgumentError("autonomous brain workflow portfolio plan must declare at least one domain");
  return domains;
}

function workflowPortfolioExecutionDomains(execution: Pick<AutonomousWorkflowPortfolioExecutionResult, "items">): AutonomousDomainName[] {
  if (!execution || !Array.isArray(execution.items) || execution.items.length < 1) throw new ArgumentError("autonomous brain workflow portfolio launch admission requires a non-empty execution");
  const domains = [...new Set(execution.items.map((item, index) => domain(`autonomous brain workflow portfolio execution item ${index} domain`, item.domain)))];
  if (domains.length < 1) throw new ArgumentError("autonomous brain workflow portfolio execution must declare at least one domain");
  return domains;
}

function claimIntegrityAcquisitionDomains(bridge: AutonomousClaimIntegrityAcquisitionBridge): AutonomousDomainName[] {
  const verified = validateAutonomousClaimIntegrityAcquisitionBridge(bridge);
  if (verified.acquisitionPlan === null || verified.acquisitionPlan.selectedDomains.length < 1) throw new ArgumentError("integrity acquisition bridge has no executable plan");
  return [...verified.acquisitionPlan.selectedDomains];
}

function capabilityRequestDomains(requests: readonly unknown[]): AutonomousDomainName[] {
  if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError("autonomous brain capability requests are outside their bound");
  const domains: AutonomousDomainName[] = [];
  for (const [index, request] of requests.entries()) {
    if (!isObject(request) || !isObject(request.workflow_context)) throw new ArgumentError(`autonomous brain capability request ${index} is missing workflow context`);
    const domain = request.workflow_context.domain;
    if (typeof domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`autonomous brain capability request ${index} contains an unsupported domain`);
    domains.push(domain as AutonomousDomainName);
  }
  return [...new Set(domains)];
}

function toolCallDomains(domains: readonly string[]): AutonomousDomainName[] {
  if (!Array.isArray(domains)) throw new ArgumentError("autonomous brain tool call domains must be an array");
  return domains.map((domain, index) => {
    if (typeof domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`autonomous brain tool call domain ${index} is unsupported`);
    return domain as AutonomousDomainName;
  });
}

function composeSelectionCallbacks(...callbacks: readonly (AutonomousModelSelectionTraceEventCallback | undefined)[]): AutonomousModelSelectionTraceEventCallback | undefined {
  const active = callbacks.filter((callback): callback is AutonomousModelSelectionTraceEventCallback => callback !== undefined);
  if (!active.length) return undefined;
  return async (event) => {
    for (const callback of active) await callback(event);
  };
}

function evidenceTraceDomains(domains: readonly AutonomousDomainName[] | undefined, runMode: unknown): AutonomousDomainName[] {
  const result = [...(domains ?? AUTONOMOUS_DOMAIN_NAMES)];
  if (runMode === "cross_domain" && !result.includes("cross_domain")) result.push("cross_domain");
  return result;
}

function tracedEvidenceRunOptions(run: AutonomousAutoRunOptions | undefined, trace: AutonomousRunTraceSession): AutonomousAutoRunOptions {
  return {
    ...(run ?? {}),
    observer: composeBrainObservers(run?.observer, trace.providerObserver()),
    selectionEventCallback: trace.selectionEventCallback(run?.selectionEventCallback),
  };
}

type EvidenceTraceResult = AutonomousBrainEvidenceBackedRunResult | AutonomousBrainDomainEvidenceBrainRunResult;

function evidenceTraceMetadataRun(result: EvidenceTraceResult): AutonomousRunResult | null {
  const automaticResult = result.automatic?.result;
  return result.run
    ?? result.cross_domain_run?.synthesis
    ?? (automaticResult?.schema === "bioprism-typescript-autonomous-run/0.1" ? automaticResult : automaticResult?.synthesis ?? null);
}

function evidenceTraceRouteDigest(result: EvidenceTraceResult): string | null {
  return result.automatic?.route.route_digest
    ?? result.cross_domain_run?.route.route_digest
    ?? result.run?.route.route_digest
    ?? null;
}

function evidenceTraceStatus(status: string): ReturnType<typeof autonomousRunTraceStatus> {
  if (status === "evidence_review_required" || status === "evidence_blocked" || status === "provider_pending" || status === "provider_reconciliation_required") return "paused";
  if (status === "evidence_incomplete") return "partial";
  if (status === "evidence_failed") return "failed";
  return autonomousRunTraceStatus(status);
}

function missionTraceStatus(status: string): ReturnType<typeof autonomousRunTraceStatus> {
  if (status === "succeeded") return "completed";
  if (status === "partial") return "partial";
  if (status === "failed") return "failed";
  if (status === "cancelled") return "paused";
  return autonomousRunTraceStatus(status);
}

function validateMissionReplanOptions(options: AutonomousAgentMissionReplanOptions): AutonomousAgentMissionReplanOptions {
  if (!isObject(options) || typeof options.evaluate !== "function") throw new ArgumentError("autonomous brain mission execution requires an evaluator callback");
  return options;
}

function tracedMissionReplanResult(result: AutonomousMissionReplanResult, trace: AutonomousRunTraceSummary): AutonomousBrainTracedMissionReplanResult {
  const projection = {
    schema: AUTONOMOUS_BRAIN_TRACED_MISSION_REPLAN_SCHEMA,
    trace,
    retention: "mission_execution_values_caller_owned;trace_metadata_only_no_prompts_responses_arguments_or_credentials" as const,
    secret_material: "never_returned" as const,
  } as AutonomousBrainTracedMissionReplanResult;
  Object.defineProperty(projection, "result", {
    value: result,
    enumerable: false,
    configurable: false,
    writable: false,
  });
  return projection;
}

function automaticBatchTraceTaskDigest(inputs: readonly AutonomousBrainRequest[]): string {
  return digestJsonSync({
    schema: AUTONOMOUS_BRAIN_TRACED_AUTO_BATCH_SCHEMA,
    mode: "automatic",
    task_digests: inputs.map((input) => brainBatchTaskDigest(input)),
  });
}

function automaticCycleBatchTraceTaskDigest(inputs: readonly AutonomousBrainRequest[], mode: "automatic_cycle" | "automatic_replan"): string {
  return digestJsonSync({
    schema: mode === "automatic_cycle" ? AUTONOMOUS_BRAIN_TRACED_AUTO_CYCLE_BATCH_SCHEMA : AUTONOMOUS_BRAIN_TRACED_AUTO_REPLAN_BATCH_SCHEMA,
    mode,
    task_digests: inputs.map((input) => brainBatchTaskDigest(input)),
  });
}

async function recordAutomaticCycleBatchTrace(
  trace: AutonomousRunTraceSession,
  batch: AutonomousBrainAutoCycleBatchResult | AutonomousBrainAutoReplanBatchResult,
): Promise<void> {
  for (const item of batch.items) {
    const execution = item.execution;
    const rawStatus = execution === undefined
      ? item.status === "omitted" ? "paused" : item.status === "refused" ? "refused" : "failed"
      : autonomousRunTraceStatus(execution.status);
    const status = rawStatus === "running" || rawStatus === "unknown" ? "paused" : rawStatus;
    await trace.record({
      phase: status === "completed" || status === "partial" ? "completed" : status,
      status,
      domains: execution?.route.selected_domains,
      route_digest: execution?.route.route_digest,
      detail_digest: digestJsonSync({ index: item.index, item_status: item.status, execution_status: execution?.status ?? null }),
    });
  }
}

function brainBatchTaskDigest(input: AutonomousBrainRequest): string {
  return digestJsonSync({ task: input.task });
}

function brainBatchRequestDigest(input: AutonomousBrainRequest, index: number, mode: AutonomousBrainBatchMode = "brain"): string {
  return digestJsonSync({
    index,
    mode,
    task_digest: brainBatchTaskDigest(input),
    domain: input.domain ?? null,
    capability: input.capability ?? null,
    hints_digest: digestJsonSync(input.hints ?? []),
    allow_cross_domain: input.allow_cross_domain ?? true,
    context_digest: input.context === undefined ? null : digestJsonSync(input.context),
    connector_digest: input.connector === undefined ? null : digestJsonSync(input.connector),
  });
}

const BRAIN_SEMANTIC_ROUTING_POLICY_FIELDS = [
  "enabled",
  "approveProviderCall",
  "minSemanticConfidence",
  "maxDomains",
  "allowCrossDomain",
  "maxOutputTokens",
  "temperature",
  "maxCostPerMillionTokens",
  "maxLatencyMs",
  "minQuality",
  "maxProviderFailovers",
  "executionAttempt",
  "executionLifecycle",
  "domainPolicyMode",
  "domainPolicyEvidenceReady",
  "domainPolicyEvaluatorConfigured",
  "domainPolicyEffectsRequested",
  "domainPolicyEffectsApproved",
] as const;

function brainSemanticRoutingPolicyDigest(options: AutonomousBrainExecuteOptions | undefined): string | null {
  if (options === undefined) return null;
  const routing = selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting);
  const config = normalizeBrainSemanticRouting(routing);
  if (config === null) return null;
  const source = options.run ?? {};
  const semanticConfig: Record<string, unknown> = {};
  for (const field of BRAIN_SEMANTIC_ROUTING_POLICY_FIELDS) {
    const value = config[field];
    if (value !== undefined && (typeof value === "boolean" || typeof value === "number" || typeof value === "string")) semanticConfig[field] = value;
  }
  return digestJsonSync({
    schema: "bioprism-typescript-autonomous-brain-semantic-routing-policy/0.1",
    semantic_routing: semanticConfig,
    classifier_approval: options.approveProviderCall ?? null,
    inherited_approval: source.approveProviderCall ?? null,
    inherited_selection: {
      candidates_digest: source.candidates === undefined ? null : digestJsonSync(source.candidates),
      max_output_tokens: source.maxOutputTokens ?? null,
      temperature: source.temperature ?? null,
      max_cost_per_million_tokens: source.maxCostPerMillionTokens ?? null,
      max_latency_ms: source.maxLatencyMs ?? null,
      min_quality: source.minQuality ?? null,
      max_provider_failovers: source.maxProviderFailovers ?? null,
      execution_attempt: source.executionAttempt ?? null,
      cost_budget_max: source.costBudget instanceof AutonomousCostBudget ? source.costBudget.maxCostUnits : null,
      max_total_cost_units: source.maxTotalCostUnits ?? null,
      execution_controller_present: source.execution !== undefined,
      execution_policy_digest: source.execution?.state.policy_digest ?? null,
    },
  });
}

/** Digest automatic controls without retaining prompts, connector values, credentials, or callbacks. */
function brainAutomaticExecutionPolicyDigest(options: AutonomousBrainAutoExecuteOptions | undefined): string | null {
  if (options === undefined) return null;
  const planning = options.planning;
  const planningProjection = planning === undefined ? null : {
    candidates_digest: planning.candidates === undefined ? null : digestJsonSync(planning.candidates),
    context_digest: planning.context === undefined ? null : digestJsonSync(planning.context),
    prompt_selection_digest: planning.promptSelection === undefined ? null : digestJsonSync(planning.promptSelection),
    prompt_learning_state_digest: planning.promptLearningState === undefined ? null : digestJsonSync(planning.promptLearningState),
    prompt_learning_exploration: planning.promptLearningExploration ?? null,
    prompt_stage: planning.promptStage ?? null,
    max_input_tokens: planning.maxInputTokens ?? null,
    max_output_tokens: planning.maxOutputTokens ?? null,
    max_cost_per_million_tokens: planning.maxCostPerMillionTokens ?? null,
    max_latency_ms: planning.maxLatencyMs ?? null,
    min_quality: planning.minQuality ?? null,
    min_selection_confidence: planning.minSelectionConfidence ?? null,
    selection_weights_digest: planning.selectionWeights === undefined ? null : digestJsonSync(planning.selectionWeights),
    selection_observations_digest: planning.selectionObservations === undefined ? null : digestJsonSync(planning.selectionObservations),
    max_total_cost_units: planning.maxTotalCostUnits ?? null,
    cost_budget_max: planning.costBudget instanceof AutonomousCostBudget ? planning.costBudget.maxCostUnits : null,
    approve_provider_call: planning.approveProviderCall ?? false,
    execution_controller_present: planning.execution !== undefined,
    execution_policy_digest: planning.execution?.state.policy_digest ?? null,
    execution_attempt: planning.executionAttempt ?? null,
    max_provider_failovers: planning.maxProviderFailovers ?? null,
    domain_policy_mode: planning.domainPolicyMode ?? null,
    domain_policy_evidence_ready: planning.domainPolicyEvidenceReady ?? null,
    domain_policy_evaluator_configured: planning.domainPolicyEvaluatorConfigured ?? null,
    domain_policy_effects_requested: planning.domainPolicyEffectsRequested ?? null,
    domain_policy_effects_approved: planning.domainPolicyEffectsApproved ?? null,
  };
  return digestJsonSync({
    schema: "bioprism-typescript-autonomous-brain-automatic-execution-policy/0.1",
    planning_mode: options.planningMode ?? "deterministic",
    approve_provider_call: options.approveProviderCall ?? false,
    semantic_routing: options.semanticRouting === undefined || options.semanticRouting === false ? null : normalizeBrainSemanticRouting(options.semanticRouting),
    accept_plan: options.acceptPlan ?? false,
    candidates_digest: options.candidates === undefined ? null : digestJsonSync(options.candidates),
    content_parts_digest: options.contentParts === undefined ? null : digestJsonSync(options.contentParts),
    prompt_selection_digest: options.promptSelection === undefined ? null : digestJsonSync(options.promptSelection),
    prompt_learning_state_digest: options.promptLearningState === undefined ? null : digestJsonSync(options.promptLearningState),
    prompt_learning_exploration: options.promptLearningExploration ?? null,
    prompt_stage: options.promptStage ?? null,
    max_input_tokens: options.maxInputTokens ?? null,
    max_output_tokens: options.maxOutputTokens ?? null,
    max_cost_per_million_tokens: options.maxCostPerMillionTokens ?? null,
    max_latency_ms: options.maxLatencyMs ?? null,
    min_quality: options.minQuality ?? null,
    min_selection_confidence: options.minSelectionConfidence ?? null,
    selection_weights_digest: options.selectionWeights === undefined ? null : digestJsonSync(options.selectionWeights),
    selection_observations_digest: options.selectionObservations === undefined ? null : digestJsonSync(options.selectionObservations),
    max_total_cost_units: options.maxTotalCostUnits ?? null,
    cost_budget_max: options.costBudget instanceof AutonomousCostBudget ? options.costBudget.maxCostUnits : null,
    approve_effects: options.approveEffects ?? false,
    connector_first: options.connectorFirst ?? true,
    include_connector_observation: options.includeConnectorObservation ?? true,
    tool_names: options.tools?.map((tool) => tool.name).sort() ?? null,
    tool_read_only_present: options.toolReadOnly !== undefined,
    authorize_and_execute_present: options.authorizeAndExecute !== undefined,
    execution_controller_present: options.execution !== undefined,
    execution_policy_digest: options.execution?.state.policy_digest ?? null,
    execution_attempt: options.executionAttempt ?? null,
    max_provider_failovers: options.maxProviderFailovers ?? null,
    require_json: options.requireJson ?? false,
    response_schema_digest: options.responseSchema === undefined ? null : digestJsonSync(options.responseSchema),
    structured_domain_response: options.structuredDomainResponse ?? false,
    temperature: options.temperature ?? null,
    domain_policy_mode: options.domainPolicyMode ?? null,
    domain_policy_evidence_ready: options.domainPolicyEvidenceReady ?? null,
    domain_policy_evaluator_configured: options.domainPolicyEvaluatorConfigured ?? null,
    domain_policy_plan_accepted: options.domainPolicyPlanAccepted ?? null,
    domain_policy_effects_requested: options.domainPolicyEffectsRequested ?? null,
    domain_policy_effects_approved: options.domainPolicyEffectsApproved ?? null,
    planning: planningProjection,
  });
}

function policyDigestProjection(value: unknown, depth = 0): unknown {
  if (depth > 5) return "[depth_bound]";
  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value;
  if (typeof value === "function") return "[callback]";
  if (Array.isArray(value)) return value.slice(0, 128).map((entry) => policyDigestProjection(entry, depth + 1));
  if (!isObject(value)) return `[${typeof value}]`;
  const output: Record<string, unknown> = {};
  for (const key of Object.keys(value).sort()) {
    if (key === "observer" || key === "selectionEventCallback") continue;
    output[key] = policyDigestProjection(value[key], depth + 1);
  }
  return output;
}

function automaticCyclePolicyDigest(
  policy: AutonomousBrainAutoCycleOptions | AutonomousBrainAutoReplanCycleOptions,
  mode: "automatic_cycle" | "automatic_replan",
  explicitPolicyDigest?: string,
): string {
  if (explicitPolicyDigest !== undefined) digest("autonomous brain automatic cycle policyDigest", explicitPolicyDigest);
  return digestJsonSync({
    schema: "bioprism-typescript-autonomous-brain-automatic-cycle-batch-policy/0.1",
    mode,
    explicit_policy_digest: explicitPolicyDigest ?? null,
    policy: policyDigestProjection(policy),
  });
}

function checkpointText(name: string, value: unknown): string {
  return boundedIdentifier(name, value);
}

function validateBrainBatchCheckpoint(value: unknown): AutonomousBrainBatchCheckpointJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA || !["brain", "automatic", "automatic_cycle", "automatic_replan"].includes(value.mode as string)) throw new ArgumentError("autonomous brain batch checkpoint schema is invalid");
  const allowedKeys = new Set(["schema", "job_id", "mode", "batch_input_digest", "semantic_routing_policy_digest", "automatic_execution_policy_digest", "request_digests", "completed_indices", "completed_result_digests", "max_parallelism", "stop_on_error", "status", "checkpoint_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError("autonomous brain batch checkpoint contains unsupported metadata");
  const jobId = checkpointText("autonomous brain batch checkpoint job_id", value.job_id);
  const batchInputDigest = digest("autonomous brain batch checkpoint batch_input_digest", value.batch_input_digest);
  const semanticRoutingPolicyDigest = value.semantic_routing_policy_digest === undefined ? undefined : digest("autonomous brain batch checkpoint semantic_routing_policy_digest", value.semantic_routing_policy_digest);
  const automaticExecutionPolicyDigest = value.automatic_execution_policy_digest === undefined ? undefined : digest("autonomous brain batch checkpoint automatic_execution_policy_digest", value.automatic_execution_policy_digest);
  const requestDigests = value.request_digests;
  if (!Array.isArray(requestDigests) || requestDigests.length < 1 || requestDigests.length > MAX_AUTONOMOUS_BRAIN_BATCH || requestDigests.some((entry) => typeof entry !== "string" || !/^[0-9a-f]{64}$/.test(entry))) throw new ArgumentError("autonomous brain batch checkpoint request_digests are invalid");
  if (!Array.isArray(value.completed_indices) || value.completed_indices.length > requestDigests.length || value.completed_indices.some((entry) => !Number.isSafeInteger(entry) || (entry as number) < 0 || (entry as number) >= requestDigests.length)) throw new ArgumentError("autonomous brain batch checkpoint completed_indices are invalid");
  const completedIndices = [...(value.completed_indices as number[])];
  if (new Set(completedIndices).size !== completedIndices.length || completedIndices.some((entry, index) => index > 0 && entry <= completedIndices[index - 1]!)) throw new ArgumentError("autonomous brain batch checkpoint completed_indices must be sorted and unique");
  if (!Array.isArray(value.completed_result_digests) || value.completed_result_digests.length !== completedIndices.length || value.completed_result_digests.some((entry) => typeof entry !== "string" || !/^[0-9a-f]{64}$/.test(entry))) throw new ArgumentError("autonomous brain batch checkpoint result digests are invalid");
  if (!Number.isSafeInteger(value.max_parallelism) || (value.max_parallelism as number) < 1 || (value.max_parallelism as number) > MAX_AUTONOMOUS_BRAIN_PARALLELISM) throw new ArgumentError("autonomous brain batch checkpoint maxParallelism is invalid");
  if (typeof value.stop_on_error !== "boolean" || !["running", "partial", "completed"].includes(value.status as string)) throw new ArgumentError("autonomous brain batch checkpoint controls are invalid");
  if (value.status === "completed" && completedIndices.length !== requestDigests.length) throw new ArgumentError("completed autonomous brain batch checkpoint is incomplete");
  if (value.mode !== "brain" && automaticExecutionPolicyDigest === undefined) throw new ArgumentError("automatic brain batch checkpoint requires an automatic execution policy digest");
  if (value.mode === "brain" && automaticExecutionPolicyDigest !== undefined) throw new ArgumentError("direct brain batch checkpoint cannot contain an automatic execution policy digest");
  const payload = { schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, job_id: jobId, mode: value.mode as AutonomousBrainBatchMode, batch_input_digest: batchInputDigest, ...(semanticRoutingPolicyDigest === undefined ? {} : { semantic_routing_policy_digest: semanticRoutingPolicyDigest }), ...(automaticExecutionPolicyDigest === undefined ? {} : { automatic_execution_policy_digest: automaticExecutionPolicyDigest }), request_digests: [...requestDigests as string[]], completed_indices: completedIndices, completed_result_digests: [...(value.completed_result_digests as string[])], max_parallelism: value.max_parallelism as number, stop_on_error: value.stop_on_error as boolean, status: value.status as "running" | "partial" | "completed" };
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES) throw new ArgumentError("autonomous brain batch checkpoint exceeds its bounded size");
  if (digestJsonSync(payload) !== value.checkpoint_digest) throw new ArgumentError("autonomous brain batch checkpoint digest is invalid");
  if (value.retention !== "request_and_result_digests_only;tasks_prompts_credentials_and_payloads_never_persisted" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous brain batch checkpoint retention contract is invalid");
  return { ...payload, checkpoint_digest: value.checkpoint_digest as string, retention: value.retention, secret_material: value.secret_material };
}

function makeBrainBatchCheckpoint(input: { jobId: string; mode?: AutonomousBrainBatchMode; requestDigests: readonly string[]; batchInputDigest: string; semanticRoutingPolicyDigest: string | null; automaticExecutionPolicyDigest?: string | null; completed: readonly { index: number; item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: unknown } }[]; maxParallelism: number; stopOnError: boolean; status: "running" | "partial" | "completed" }): AutonomousBrainBatchCheckpointJSON {
  const mode = input.mode ?? "brain";
  if (mode === "automatic" && input.automaticExecutionPolicyDigest === undefined) throw new ArgumentError("automatic brain batch checkpoint requires an automatic execution policy digest");
  const payload = { schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, job_id: input.jobId, mode, batch_input_digest: input.batchInputDigest, ...(input.semanticRoutingPolicyDigest === null ? {} : { semantic_routing_policy_digest: input.semanticRoutingPolicyDigest }), ...(input.automaticExecutionPolicyDigest === undefined || input.automaticExecutionPolicyDigest === null ? {} : { automatic_execution_policy_digest: input.automaticExecutionPolicyDigest }), request_digests: [...input.requestDigests], completed_indices: input.completed.map((entry) => entry.index), completed_result_digests: input.completed.map((entry) => checkpointBatchItemDigest(entry.item)), max_parallelism: input.maxParallelism, stop_on_error: input.stopOnError, status: input.status };
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES) throw new ArgumentError("autonomous brain batch checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: digestJsonSync(payload), retention: "request_and_result_digests_only;tasks_prompts_credentials_and_payloads_never_persisted", secret_material: "never_returned" };
}

function projectTaskBlueprint(blueprint: AutonomousTaskBlueprint, routeDigest: string): AutonomousBrainDomainPlanSummary {
  return {
    schema: AUTONOMOUS_BRAIN_SUMMARY_SCHEMA,
    domain: blueprint.domain_profile.domain,
    capability: blueprint.selection_context.capability,
    risk_class: blueprint.domain_profile.risk_class,
    workflow_id: blueprint.workflow.workflow_id,
    workflow_digest: blueprint.workflow.workflow_digest,
    domain_pack_digest: blueprint.domain_pack.pack_digest,
    task_digest: blueprint.task_digest,
    route_digest: routeDigest,
    prompt_digest: blueprint.prompt.prompt_digest,
    plan_digest: blueprint.plan.plan_digest,
    learning_context_digest: blueprint.learning_context_digest,
    evidence_plan_digest: blueprint.evidence_plan.plan_digest,
    domain_policy_digest: blueprint.domain_policy.policy_digest,
    task_intent_digest: blueprint.task_intent.intent_digest,
    task_decision_digest: blueprint.task_decision.decision_digest,
    task_decision_posture: blueprint.task_decision.posture,
    task_decision_recommended_path: blueprint.task_decision.recommended_path,
    task_decision_requested_effect: blueprint.task_decision.requested_effect,
    task_decision_evidence_posture: blueprint.task_decision.evidence_posture,
    task_decision_preferred_model_capabilities: [...blueprint.task_decision.preferred_model_capabilities],
    task_decision_approval_requirements: [...blueprint.task_decision.approval_requirements],
    task_decision_review_reasons: [...blueprint.task_decision.review_reasons],
    task_decision_blocking_reasons: [...blueprint.task_decision.blocking_reasons],
    task_decision_next_actions: [...blueprint.task_decision.next_actions],
    required_capabilities: [...blueprint.required_capabilities],
    allowed_tools: [...blueprint.plan.allowed_tools],
    stages: blueprint.workflow.stages.map((stage) => ({
      id: stage.id,
      depends_on: [...stage.depends_on],
      required_capabilities: [...stage.required_capabilities],
      evaluator_signals: [...stage.evaluator_signals],
      evidence_outputs: [...stage.evidence_outputs],
      approval_required: stage.approval_required,
      read_only: stage.read_only,
    })),
    retention: SUMMARY_RETENTION,
    secret_material: "never_returned",
  };
}

function projectCrossDomainBlueprint(blueprint: AutonomousCrossDomainBlueprint): AutonomousBrainCrossDomainPlanSummary {
  return {
    schema: AUTONOMOUS_BRAIN_SUMMARY_SCHEMA,
    task_digest: blueprint.task_digest,
    route_digest: blueprint.route_digest,
    plan_digest: blueprint.plan_digest,
    child_ids: [...blueprint.child_ids],
    children: blueprint.child_blueprints.map((child) => projectTaskBlueprint(child, blueprint.route_digest)),
    synthesis: projectTaskBlueprint(blueprint.synthesis_blueprint, blueprint.route_digest),
    dependency_graph: {
      fan_out: blueprint.dependency_graph.fan_out.map((child) => ({ ...child })),
      fan_in: blueprint.dependency_graph.fan_in,
    },
    retention: SUMMARY_RETENTION,
    secret_material: "never_returned",
  };
}

function validateRequest(input: AutonomousBrainRequest): AutonomousBrainRequest {
  if (!isObject(input)) throw new ArgumentError("autonomous brain request must be an object");
  const task = boundedText("autonomous brain task", input.task, 32_000);
  const selectedDomain = input.domain === undefined ? undefined : domain("autonomous brain domain", input.domain);
  const capability = input.capability === undefined ? undefined : boundedIdentifier("autonomous brain capability", input.capability);
  const hints = input.hints === undefined ? undefined : [...input.hints].map((hint) => boundedText("autonomous brain hint", hint, 256));
  if (hints !== undefined && hints.length > 16) throw new ArgumentError("autonomous brain hints exceed their bound");
  if (input.allow_cross_domain !== undefined && typeof input.allow_cross_domain !== "boolean") throw new ArgumentError("autonomous brain allow_cross_domain must be boolean");
  if (input.context !== undefined) {
    if (!Array.isArray(input.context) || input.context.length > MAX_AUTONOMOUS_BRAIN_CONTEXT_CHUNKS) throw new ArgumentError("autonomous brain context exceeds its bound");
    for (const chunk of input.context) {
      if (!isObject(chunk)) throw new ArgumentError("autonomous brain context contains a malformed chunk");
      boundedIdentifier("autonomous brain context id", chunk.id);
      boundedText("autonomous brain context content", chunk.content, 256_000);
      if (chunk.required !== undefined && typeof chunk.required !== "boolean") throw new ArgumentError("autonomous brain context required must be boolean");
      if (chunk.priority !== undefined && (typeof chunk.priority !== "number" || !Number.isFinite(chunk.priority))) throw new ArgumentError("autonomous brain context priority must be finite");
    }
  }
  return { task, ...(selectedDomain === undefined ? {} : { domain: selectedDomain }), ...(capability === undefined ? {} : { capability }), ...(hints === undefined ? {} : { hints }), ...(input.allow_cross_domain === undefined ? {} : { allow_cross_domain: input.allow_cross_domain }), ...(input.context === undefined ? {} : { context: [...input.context] }), ...(input.connector === undefined ? {} : { connector: input.connector }) };
}

function assertBrainPlanTask(plan: AutonomousBrainPlan, request: AutonomousBrainRequest, message: string): void {
  if (plan.task_digest !== digestJsonSync({ task: request.task })) throw new ArgumentError(message);
}

type AutonomousBrainSemanticRoutingInput = AutonomousRunOptions["semanticRouting"] | AutonomousDecisionCycleSemanticOptions | undefined;
type AutonomousBrainSemanticSource = Partial<Omit<AutonomousRunOptions, "learning">>;

const AUTONOMOUS_BRAIN_SEMANTIC_ROUTING_FIELDS = new Set([
  "enabled",
  "approveProviderCall",
  "minSemanticConfidence",
  "maxDomains",
  "allowCrossDomain",
  "maxOutputTokens",
  "temperature",
  "maxCostPerMillionTokens",
  "maxLatencyMs",
  "minQuality",
  "execution",
  "executionAttempt",
  "maxProviderFailovers",
  "executionLifecycle",
  "signal",
  "observer",
  "domainPolicyMode",
  "domainPolicyEvidenceReady",
  "domainPolicyEvaluatorConfigured",
  "domainPolicyEffectsRequested",
  "domainPolicyEffectsApproved",
]);

function normalizeBrainSemanticRouting(value: AutonomousBrainSemanticRoutingInput): Record<string, unknown> | null {
  if (value === undefined || value === false) return null;
  if (value === true) return {};
  if (!isObject(value)) throw new ArgumentError("autonomous brain semanticRouting must be a boolean or object");
  if (value.enabled !== undefined && typeof value.enabled !== "boolean") throw new ArgumentError("autonomous brain semanticRouting.enabled must be boolean");
  if (value.enabled === false) return null;
  const unsupported = Object.keys(value).find((key) => !AUTONOMOUS_BRAIN_SEMANTIC_ROUTING_FIELDS.has(key));
  if (unsupported) throw new ArgumentError(`autonomous brain semanticRouting contains unsupported field: ${unsupported}`);
  return value;
}

function selectBrainSemanticRouting(primary: AutonomousBrainSemanticRoutingInput, nested: AutonomousBrainSemanticRoutingInput): AutonomousBrainSemanticRoutingInput {
  if (primary !== undefined && nested !== undefined) throw new ArgumentError("autonomous brain semanticRouting must be configured at one boundary");
  return primary ?? nested;
}

function prepareBrainSemanticRoute(
  request: AutonomousBrainRequest,
  routing: AutonomousBrainSemanticRoutingInput,
  source: AutonomousBrainSemanticSource,
  defaultApproval: boolean | undefined,
): { options: AutonomousSemanticRouteOptions; budget: AutonomousCostBudget | null } | null {
  const config = normalizeBrainSemanticRouting(routing);
  if (config === null) return null;
  if (request.domain !== undefined) throw new ArgumentError("autonomous brain semanticRouting cannot be combined with an explicit domain");
  if (source.costBudget !== undefined && !(source.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("autonomous brain semanticRouting costBudget must be an AutonomousCostBudget");
  if (source.costBudget !== undefined && source.maxTotalCostUnits !== undefined) throw new ArgumentError("autonomous brain semanticRouting costBudget and maxTotalCostUnits cannot both be supplied");
  const budget = source.costBudget ?? (source.maxTotalCostUnits === undefined ? null : new AutonomousCostBudget(source.maxTotalCostUnits));
  const value = (key: string): unknown => config[key];
  return {
    budget,
    options: {
      candidates: source.candidates,
      credential: source.credential,
      credentialFor: source.credentialFor,
      hints: request.hints,
      approveProviderCall: (value("approveProviderCall") as boolean | undefined) ?? defaultApproval ?? source.approveProviderCall ?? false,
      minSemanticConfidence: value("minSemanticConfidence") as number | undefined,
      maxDomains: (value("maxDomains") as number | undefined) ?? 3,
      allowCrossDomain: (value("allowCrossDomain") as boolean | undefined) ?? request.allow_cross_domain ?? true,
      maxOutputTokens: (value("maxOutputTokens") as number | undefined) ?? source.maxOutputTokens ?? 1_024,
      temperature: (value("temperature") as number | undefined) ?? source.temperature,
      maxCostPerMillionTokens: (value("maxCostPerMillionTokens") as number | undefined) ?? source.maxCostPerMillionTokens,
      maxLatencyMs: (value("maxLatencyMs") as number | undefined) ?? source.maxLatencyMs,
      minQuality: (value("minQuality") as number | undefined) ?? source.minQuality,
      costBudget: budget ?? undefined,
      execution: (value("execution") as AutonomousSemanticRouteOptions["execution"] | undefined) ?? source.execution,
      executionAttempt: (value("executionAttempt") as number | undefined) ?? source.executionAttempt,
      maxProviderFailovers: (value("maxProviderFailovers") as number | undefined) ?? source.maxProviderFailovers,
      executionLifecycle: (value("executionLifecycle") as AutonomousSemanticRouteOptions["executionLifecycle"] | undefined) ?? source.executionLifecycle,
      signal: (value("signal") as AbortSignal | undefined) ?? source.signal,
      observer: (value("observer") as ProviderInvocationObserver | undefined) ?? source.observer,
      domainPolicyMode: (value("domainPolicyMode") as AutonomousSemanticRouteOptions["domainPolicyMode"] | undefined) ?? source.domainPolicyMode,
      domainPolicyEvidenceReady: (value("domainPolicyEvidenceReady") as boolean | undefined) ?? source.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: (value("domainPolicyEvaluatorConfigured") as boolean | undefined) ?? source.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: (value("domainPolicyEffectsRequested") as boolean | undefined) ?? source.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: (value("domainPolicyEffectsApproved") as boolean | undefined) ?? source.domainPolicyEffectsApproved,
    },
  };
}

function observationChunk(execution: AutonomousConnectorOperationExecution): AutonomousPromptChunk {
  const metadata: JsonObject = {
    schema: "bioprism-typescript-autonomous-connector-observation-context/0.1",
    status: execution.status,
    replay: execution.replay,
    receipt: execution.dispatch.receipt.toJSON(),
    observation: execution.dispatch.value,
    does_not_claim: ["connector observation is caller-owned and may be incomplete", "connector status is not evaluator reward", "connector observation does not prove external-world truth"],
    secret_material: "never_returned",
  };
  const encoded = canonicalJson(metadata);
  if (bytes(encoded) > MAX_AUTONOMOUS_BRAIN_OBSERVATION_BYTES) throw new ProviderRuntimeError("autonomous connector observation exceeds the brain context bound", { code: "response_too_large" });
  return { id: "autonomous-connector-observation", content: encoded, required: false, priority: 80 };
}

/** Request-free, digest-bound plan for the high-level brain facade. */
export class AutonomousBrainPlan {
  readonly status: AutonomousBrainPlanStatus;
  readonly route: AutonomousRouteProposal;
  readonly semantic_route: AutonomousSemanticRouteResult | null;
  readonly domain_plan: AutonomousBrainDomainPlanSummary | null;
  readonly cross_domain_plan: AutonomousBrainCrossDomainPlanSummary | null;
  readonly connector_plan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null;
  readonly selected_domains: AutonomousDomainName[];
  readonly task_digest: string;
  readonly plan_digest: string;

  constructor(input: {
    status: AutonomousBrainPlanStatus;
    route: AutonomousRouteProposal;
    semantic_route?: AutonomousSemanticRouteResult | null;
    domain_plan: AutonomousBrainDomainPlanSummary | null;
    cross_domain_plan: AutonomousBrainCrossDomainPlanSummary | null;
    connector_plan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null;
  }) {
    if (input.status !== "ready" && input.status !== "route_review_required" && input.status !== "connector_review_required") throw new ArgumentError("autonomous brain plan status is invalid");
    if (!isObject(input.route) || typeof input.route.route_digest !== "string") throw new ArgumentError("autonomous brain plan route is malformed");
    this.status = input.status;
    this.route = structuredClone(input.route);
    this.semantic_route = input.semantic_route === undefined || input.semantic_route === null ? null : structuredClone(input.semantic_route);
    if (this.semantic_route !== null && this.semantic_route.route.route_digest !== this.route.route_digest) throw new ArgumentError("autonomous brain semantic route does not match the plan route");
    this.domain_plan = input.domain_plan === null ? null : structuredClone(input.domain_plan);
    this.cross_domain_plan = input.cross_domain_plan === null ? null : structuredClone(input.cross_domain_plan);
    this.connector_plan = input.connector_plan === null ? null : structuredClone(input.connector_plan);
    this.selected_domains = [...this.route.selected_domains];
    this.task_digest = digest("autonomous brain plan task_digest", this.route.task_digest);
    this.plan_digest = digestJsonSync(this.descriptor());
  }

  private descriptor(): Omit<AutonomousBrainPlanJSON, "plan_digest"> {
    const descriptor = {
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status: this.status,
      route: structuredClone(this.route),
      domain_plan: this.domain_plan === null ? null : structuredClone(this.domain_plan),
      cross_domain_plan: this.cross_domain_plan === null ? null : structuredClone(this.cross_domain_plan),
      connector_plan: this.connector_plan === null ? null : structuredClone(this.connector_plan),
      selected_domains: [...this.selected_domains],
      task_digest: this.task_digest,
      retention: PLAN_RETENTION,
      secret_material: "never_returned" as const,
    };
    return this.semantic_route === null ? descriptor : { ...descriptor, semantic_route: structuredClone(this.semantic_route) };
  }

  toJSON(): AutonomousBrainPlanJSON {
    return { ...this.descriptor(), plan_digest: this.plan_digest };
  }

  static fromJSON(value: unknown): AutonomousBrainPlan {
    if (!isObject(value) || value.schema !== AUTONOMOUS_BRAIN_FACADE_SCHEMA || value.retention !== PLAN_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous brain plan is malformed");
    const plan = new AutonomousBrainPlan({
      status: value.status as AutonomousBrainPlanStatus,
      route: value.route as AutonomousRouteProposal,
      semantic_route: value.semantic_route === undefined || value.semantic_route === null ? null : value.semantic_route as AutonomousSemanticRouteResult,
      domain_plan: (value.domain_plan as AutonomousBrainDomainPlanSummary | null) ?? null,
      cross_domain_plan: (value.cross_domain_plan as AutonomousBrainCrossDomainPlanSummary | null) ?? null,
      connector_plan: (value.connector_plan as ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null) ?? null,
    });
    if (value.plan_digest !== plan.plan_digest || value.task_digest !== plan.task_digest) throw new ArgumentError("autonomous brain plan digest is invalid");
    if (JSON.stringify(value.selected_domains) !== JSON.stringify(plan.selected_domains)) throw new ArgumentError("autonomous brain plan selected domains are invalid");
    return plan;
  }
}

/**
 * Compose routing, domain workflow planning, provider invocation, connector evidence, and
 * cross-domain execution behind one strongly bounded application API.
 */
export class AutonomousBrainFacade {
  readonly agent: AutonomousAgent;
  readonly connectorOperations?: AutonomousConnectorOperationFacade;
  readonly connectorIntent?: AutonomousConnectorIntentFacade;
  private provider_setup: ProviderSetup | undefined;

  constructor(options: { agent: AutonomousAgent; connectorOperations?: AutonomousConnectorOperationFacade }) {
    if (!options || !options.agent || typeof options.agent.route !== "function" || typeof options.agent.blueprint !== "function" || typeof options.agent.run !== "function" || typeof options.agent.runCrossDomain !== "function" || typeof options.agent.readiness !== "function" || typeof options.agent.refreshActivation !== "function") throw new ArgumentError("autonomous brain facade requires an AutonomousAgent");
    if (options.connectorOperations !== undefined && !(options.connectorOperations instanceof AutonomousConnectorOperationFacade)) throw new ArgumentError("autonomous brain connectorOperations is invalid");
    this.agent = options.agent;
    this.connectorOperations = options.connectorOperations;
    this.connectorIntent = options.connectorOperations === undefined
      ? undefined
      : new AutonomousConnectorIntentFacade({
        operationFacade: options.connectorOperations,
        route: (task, routeOptions) => this.agent.route(task, routeOptions),
      });
  }

  /**
   * Compile the same provider-free intake artifacts used by execution into a bounded
   * clarification questionnaire. An explicit domain is preferred; without one, the facade
   * accepts only a single deterministic route and refuses an abstained or cross-domain task so
   * that a question set cannot accidentally be applied to the wrong workflow.
   *
   * The returned plan contains only reviewed metadata and digests. It does not call a provider,
   * connector, source, tool, evaluator, or learner, and it cannot authorize any later action.
   */
  async clarificationPlan(
    input: AutonomousBrainRequest,
    options: AutonomousBrainClarificationOptions = {},
  ): Promise<AutonomousTaskClarificationPlan> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain clarification does not execute connector inputs");
    if (request.domain !== undefined && options.domain !== undefined && request.domain !== options.domain) throw new ArgumentError("autonomous brain clarification domain conflicts with the request");
    let domain = options.domain ?? request.domain;
    if (domain === undefined) {
      const route = await this.agent.route(request.task, {
        hints: request.hints,
        allowCrossDomain: false,
      });
      if (route.abstained || route.cross_domain || route.primary_domain === null) throw new ArgumentError("autonomous brain clarification requires an explicit or unambiguous single-domain route");
      domain = route.primary_domain;
    }
    return this.agent.clarificationPlan(request.task, {
      domain,
      capability: options.capability ?? request.capability,
      riskClass: options.riskClass,
      constraints: options.constraints,
      desiredOutputs: options.desiredOutputs,
      context: request.context,
      maxInputTokens: options.maxInputTokens,
      structuredDomainResponse: options.structuredDomainResponse,
      maxQuestions: options.maxQuestions,
    });
  }

  /** Resolve transient user answers into a plan-bound receipt containing answer digests only. */
  async resolveClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    task: string,
    answers: Readonly<Record<string, string>>,
  ): Promise<AutonomousTaskClarificationResolution> {
    return this.agent.resolveClarification(plan, task, answers);
  }

  /** Rehydrate and validate a clarification receipt against its exact questionnaire. */
  async validateClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    receipt: AutonomousTaskClarificationResolution | unknown,
  ): Promise<AutonomousTaskClarificationResolution> {
    return this.agent.validateClarification(plan, receipt);
  }

  /**
   * Recompile a clarified task only after its original task digest and resolved receipt verify.
   * The fresh blueprint is transient; use the normal facade plan, launch, provider, tool, and
   * effect gates before executing it.
   */
  async recompileClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    receipt: AutonomousTaskClarificationResolution | unknown,
    input: AutonomousBrainRequest,
    clarifiedTask: string,
    options: AutonomousBrainClarificationRecompileOptions = {},
  ): Promise<AutonomousClarificationRecompileResult> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain clarification recompile does not execute connector inputs");
    return this.agent.recompileClarification(plan, receipt, request.task, clarifiedTask, {
      capability: options.capability ?? request.capability,
      riskClass: options.riskClass,
      constraints: options.constraints,
      desiredOutputs: options.desiredOutputs,
      context: request.context,
      maxInputTokens: options.maxInputTokens,
      structuredDomainResponse: options.structuredDomainResponse,
    });
  }

  /** Validate a serialized clarified blueprint without restoring task or answer values. */
  async validateClarificationRecompile(
    value: unknown,
    plan?: AutonomousTaskClarificationPlan | unknown,
    receipt?: AutonomousTaskClarificationResolution | unknown,
  ): Promise<AutonomousClarificationRecompileProjection> {
    return this.agent.validateClarificationRecompile(value, plan, receipt);
  }

  /**
   * Reusable protected provider registration, credential-session, and revocation boundary.
   *
   * The setup object is lazy so provider-free facade doubles and structural agents can still be
   * used for planning, recovery, and review-only operations without constructing credentials.
   * Accessing this property requires the bound agent to provide a real `LLMRuntime`.
   */
  get providerSetup(): ProviderSetup {
    this.provider_setup ??= new ProviderSetup(this.agent.llm);
    return this.provider_setup;
  }

  /**
   * Create the application-facing operator projection for metadata-only run traces.
   *
   * The registry is deliberately supplied by the caller because retention, persistence, and
   * operator authorization belong to the deployment. The controller binds it to this exact
   * facade instance and exposes restore-before-query, CAS-aware flush, trace publication,
   * bounded retention, and integrity inspection without gaining execution authority.
   */
  createTraceRegistryController(
    options: AutonomousBrainTraceRegistryControllerOptions,
  ): AutonomousBrainTraceRegistryController {
    if (!isObject(options) || !(options.registry instanceof AutonomousRunTraceRegistry) || !(options.persistence instanceof JsonAutonomousRunTraceRegistryPersistence)) throw new ArgumentError("autonomous brain trace registry controller options are malformed");
    return new AutonomousBrainTraceRegistryController(this, options.registry, options.persistence);
  }

  /**
   * Discover provider model metadata inside an already-open protected onboarding session.
   *
   * The session owns opaque credential handles and is checked by `ProviderSetup`; this facade
   * never accepts a raw key or returns the provider response body. Discovery is observational and
   * does not register a model or authorize execution.
   */
  async discoverModels(
    session: CredentialSession,
    provider: string,
    options: { signal?: AbortSignal } = {},
  ): Promise<AutonomousBrainModelDiscovery> {
    return this.providerSetup.discoverModels(session, provider, options);
  }

  /** Normalize one protected discovery result into explicit candidate priors without dispatch. */
  modelCandidates(
    discovery: AutonomousBrainModelDiscovery,
    defaults: AutonomousBrainModelCandidateDefaults,
  ): AutonomousBrainModelCandidate[] {
    return this.providerSetup.modelCandidates(discovery, defaults);
  }

  /**
   * Reconcile one or more discovered provider catalogues into the agent's all-domain inventory.
   *
   * The protected session is forwarded only to the agent's inventory coordinator. The returned
   * snapshot contains model metadata, readiness, and twelve-domain coverage digests; provider
   * keys, raw catalogues, prompts, and responses remain outside the snapshot.
   */
  async refreshModelInventory(
    session: CredentialSession,
    specs: readonly AutonomousModelRefreshSpec[],
    options: AutonomousBrainModelInventoryRefreshOptions = {},
  ): Promise<AutonomousBrainModelInventorySnapshot> {
    return this.providerSetup.refreshModelInventory(this.agent, session, specs, options);
  }

  /**
   * Project live model eligibility across every built-in domain without opening onboarding,
   * discovering models, invoking a provider, or mutating the catalogue.
   */
  async modelInventoryReadiness(
    options: AutonomousBrainModelInventoryReadinessOptions = {},
  ): Promise<AutonomousBrainModelInventoryReadiness> {
    return this.agent.modelInventoryReadiness(options);
  }

  /** Validate and register one aggregate evaluator-calibration report for later learning gates. */
  registerEvaluatorCalibration(report: AutonomousEvaluatorCalibrationReport): AutonomousEvaluatorCalibrationImport {
    return this.agent.registerEvaluatorCalibration(report);
  }

  /** Retrieve one registered calibration report by its content digest. */
  evaluatorCalibrationReport(reportDigest: string): AutonomousEvaluatorCalibrationReport | null {
    return this.agent.evaluatorCalibrationReport(reportDigest);
  }

  /** Query aggregate calibration metadata without exposing caller-owned calibration cases. */
  evaluatorCalibrationReports(options: AutonomousEvaluatorCalibrationQueryOptions = {}): AutonomousEvaluatorCalibrationReport[] {
    return this.agent.evaluatorCalibrationReports(options);
  }

  /** Validate a calibration report before it is used as a learning-admission input. */
  validateEvaluatorCalibration(report: AutonomousEvaluatorCalibrationReport): AutonomousEvaluatorCalibrationReport {
    return validateAutonomousEvaluatorCalibrationReport(report);
  }

  /** Derive the explicit per-domain decision that admits or holds evaluator-driven learning. */
  evaluatorCalibrationAdmission(
    report: AutonomousEvaluatorCalibrationReport,
    domain: AutonomousDomainName,
  ): AutonomousEvaluatorCalibrationAdmission {
    return autonomousEvaluatorCalibrationAdmission(report, domain);
  }

  /** Restore calibration reports through the agent-bound caller-owned persistence coordinator. */
  async restoreEvaluatorCalibration(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot | null> {
    return this.agent.restoreEvaluatorCalibration();
  }

  /** Flush calibration reports through the agent-bound caller-owned CAS coordinator. */
  async flushEvaluatorCalibration(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot> {
    return this.agent.flushEvaluatorCalibration();
  }

  /** Restore digest-bound model-arm bandit state before admitting adaptive selection. */
  async restoreOnlineLearning(): Promise<Awaited<ReturnType<AutonomousAgent["restoreOnlineLearning"]>>> {
    return this.agent.restoreOnlineLearning();
  }

  /** Flush model-arm bandit state through the caller-owned CAS boundary. */
  async flushOnlineLearning(): Promise<Awaited<ReturnType<AutonomousAgent["flushOnlineLearning"]>>> {
    return this.agent.flushOnlineLearning();
  }

  /** Apply explicit evaluator credit to a model arm; transport success never becomes reward. */
  async recordEvaluatorReward(
    armId: Parameters<AutonomousAgent["recordEvaluatorReward"]>[0],
    reward: Parameters<AutonomousAgent["recordEvaluatorReward"]>[1],
    options: Parameters<AutonomousAgent["recordEvaluatorReward"]>[2] = {},
  ): Promise<Awaited<ReturnType<AutonomousAgent["recordEvaluatorReward"]>>> {
    return this.agent.recordEvaluatorReward(armId, reward, options);
  }

  /** Restore prompt-arm statistics without exposing rendered prompts or provider responses. */
  async restorePromptLearning(): Promise<Awaited<ReturnType<AutonomousAgent["restorePromptLearning"]>>> {
    return this.agent.restorePromptLearning();
  }

  /** Flush prompt-arm statistics through the caller-owned persistence boundary. */
  async flushPromptLearning(): Promise<Awaited<ReturnType<AutonomousAgent["flushPromptLearning"]>>> {
    return this.agent.flushPromptLearning();
  }

  /** Recover registry-bound prompt choices from a transient execution result for settlement. */
  promptLearningSelections(result: unknown): ReturnType<AutonomousAgent["promptLearningSelections"]> {
    return this.agent.promptLearningSelections(result);
  }

  /** Apply explicit evaluator credit to one prompt choice without retaining prompt text. */
  async settlePromptLearning(
    selection: Parameters<AutonomousAgent["settlePromptLearning"]>[0],
    options: Parameters<AutonomousAgent["settlePromptLearning"]>[1],
  ): Promise<Awaited<ReturnType<AutonomousAgent["settlePromptLearning"]>>> {
    return this.agent.settlePromptLearning(selection, options);
  }

  /** Restore evaluator-approved tool-arm statistics before new tool selection. */
  async restoreToolSelection(): Promise<Awaited<ReturnType<AutonomousAgent["restoreToolSelection"]>>> {
    return this.agent.restoreToolSelection();
  }

  /** Flush evaluator-approved tool-arm statistics without persisting tool values or arguments. */
  async flushToolSelection(): Promise<Awaited<ReturnType<AutonomousAgent["flushToolSelection"]>>> {
    return this.agent.flushToolSelection();
  }

  /** Return a cloned value-only tool bandit state for an audit or explicit caller settlement. */
  toolSelectionState(): ReturnType<AutonomousAgent["toolSelectionState"]> {
    return this.agent.toolSelectionState();
  }

  /** Apply explicit evaluator credit to one tool arm; tool execution success is not inferred as reward. */
  recordToolSelectionReward(
    outcome: Parameters<AutonomousAgent["recordToolSelectionReward"]>[0],
  ): ReturnType<AutonomousAgent["recordToolSelectionReward"]> {
    return this.agent.recordToolSelectionReward(outcome);
  }

  /** Restore metadata-only decision-cycle checkpoints for route, planning, and settlement replay. */
  async restoreDecisionCyclePersistence(): Promise<Awaited<ReturnType<AutonomousAgent["restoreDecisionCyclePersistence"]>>> {
    return this.agent.restoreDecisionCyclePersistence();
  }

  /** Flush metadata-only decision-cycle checkpoints through the caller-owned persistence boundary. */
  async flushDecisionCyclePersistence(): Promise<Awaited<ReturnType<AutonomousAgent["flushDecisionCyclePersistence"]>>> {
    return this.agent.flushDecisionCyclePersistence();
  }

  /**
   * Execute the complete facade boundary with deployment-managed credentials.
   *
   * The setup process opens a fresh session, provisions the requested providers from its
   * configured sources, optionally refreshes the model inventory, injects only transient opaque
   * handles into the nested provider policy, and revokes the session in a finally block. The
   * returned result keeps the live execution caller-owned; its `toJSON()` projection is safe for
   * durable job state and never contains provider output, task text, prompts, or credential data.
   */
  executeWithProvisionedCredentials(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainWithProvisionedCredentials(this, input, options);
  }

  /** Execute through protected provisioning while exposing the redacted brain lifecycle trace. */
  executeWithProvisionedCredentialsWithTrace(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedExecution>> {
    return this.providerSetup.runBrainWithProvisionedCredentialsWithTrace(this, input, options);
  }

  /** Provision and execute the facade only after a caller-owned launch admission covers the route. */
  executeWithProvisionedCredentialsWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainWithProvisionedCredentialsWithLaunchAdmission(this, input, admission, options);
  }

  /** Provision and trace direct execution only after a caller-owned launch admission. */
  executeWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedExecution>> {
    return this.providerSetup.runBrainWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, input, admission, options);
  }

  /** Execute a caller-approved model-selection arm through a protected credential session. */
  executeApprovedSelectionWithProvisionedCredentials(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousProvisionedBrainApprovedSelectionOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainApprovedSelectionWithProvisionedCredentials(this, input, preview, options);
  }

  /** Execute an approved model-selection arm with protected provisioning and a metadata trace. */
  executeApprovedSelectionWithProvisionedCredentialsWithTrace(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousProvisionedBrainApprovedSelectionTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedApprovedSelection>> {
    return this.providerSetup.runBrainApprovedSelectionWithProvisionedCredentialsWithTrace(this, input, preview, options);
  }

  /** Execute an approved model-selection arm only after a caller-owned launch admission. */
  executeApprovedSelectionWithProvisionedCredentialsWithLaunchAdmission(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainApprovedSelectionOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainApprovedSelectionWithProvisionedCredentialsWithLaunchAdmission(this, input, preview, admission, options);
  }

  /** Execute an approved model-selection arm after launch admission with a metadata trace. */
  executeApprovedSelectionWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainApprovedSelectionTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedApprovedSelection>> {
    return this.providerSetup.runBrainApprovedSelectionWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, input, preview, admission, options);
  }

  /** Execute automatic route/planning/invocation through one deployment-managed session. */
  executeAutoWithProvisionedCredentials(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAutoExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAutoExecution>> {
    return this.providerSetup.runBrainAutoWithProvisionedCredentials(this, input, options);
  }

  /** Execute automatic routing/planning through protected provisioning and a redacted trace. */
  executeAutoWithProvisionedCredentialsWithTrace(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAutoTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAutoExecution>> {
    return this.providerSetup.runBrainAutoWithProvisionedCredentialsWithTrace(this, input, options);
  }

  /** Run automatic facade execution only after a caller-owned launch admission covers the route. */
  executeAutoWithProvisionedCredentialsWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAutoExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAutoExecution>> {
    return this.providerSetup.runBrainAutoWithProvisionedCredentialsWithLaunchAdmission(this, input, admission, options);
  }

  /** Provision and trace automatic execution only after a caller-owned launch admission. */
  executeAutoWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAutoTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAutoExecution>> {
    return this.providerSetup.runBrainAutoWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, input, admission, options);
  }

  /** Execute the closed-loop evaluator and online-learning cycle inside one credential session. */
  executeCycleWithProvisionedCredentials(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    return this.providerSetup.runBrainCycleWithProvisionedCredentials(this, input, options);
  }

  /** Execute the closed-loop cycle through protected provisioning and a redacted trace. */
  executeCycleWithProvisionedCredentialsWithTrace(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedCycleExecution>> {
    return this.providerSetup.runBrainCycleWithProvisionedCredentialsWithTrace(this, input, options);
  }

  /** Run the closed-loop cycle only after a caller-owned launch admission covers the route. */
  executeCycleWithProvisionedCredentialsWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    return this.providerSetup.runBrainCycleWithProvisionedCredentialsWithLaunchAdmission(this, input, admission, options);
  }

  /** Provision and trace the closed-loop cycle only after a caller-owned launch admission. */
  executeCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedCycleExecution>> {
    return this.providerSetup.runBrainCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, input, admission, options);
  }

  /** Execute bounded evaluator-guided replanning with deployment-managed credentials. */
  executeAdaptiveCycleWithProvisionedCredentials(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainAdaptiveCycleWithProvisionedCredentials(this, input, options);
  }

  /** Execute bounded adaptive replanning through protected provisioning and a redacted trace. */
  executeAdaptiveCycleWithProvisionedCredentialsWithTrace(
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainAdaptiveCycleWithProvisionedCredentialsWithTrace(this, input, options);
  }

  /** Run bounded adaptive replanning only after a caller-owned launch admission covers the route. */
  executeAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(this, input, admission, options);
  }

  /** Provision and trace adaptive replanning only after a caller-owned launch admission. */
  executeAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, input, admission, options);
  }

  /** Replay a persisted direct plan through the protected provider session boundary. */
  executePlannedWithProvisionedCredentials(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainPlannedWithProvisionedCredentials(this, plan, input, options);
  }

  /** Replay a persisted direct plan through protected provisioning and a redacted trace. */
  executePlannedWithProvisionedCredentialsWithTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedExecution>> {
    return this.providerSetup.runBrainPlannedWithProvisionedCredentialsWithTrace(this, plan, input, options);
  }

  /** Replay a persisted direct plan after launch admission, protected provisioning, and trace. */
  executePlannedWithProvisionedCredentialsWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    return this.providerSetup.runBrainPlannedWithProvisionedCredentialsWithLaunchAdmission(this, plan, input, admission, options);
  }

  /** Replay a persisted direct plan after launch admission with protected provisioning and trace. */
  executePlannedWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedExecution>> {
    return this.providerSetup.runBrainPlannedWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, plan, input, admission, options);
  }

  /** Replay a persisted closed-loop plan through the protected provider session boundary. */
  executePlannedCycleWithProvisionedCredentials(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    return this.providerSetup.runBrainPlannedCycleWithProvisionedCredentials(this, plan, input, options);
  }

  /** Replay a persisted closed-loop plan through protected provisioning and a redacted trace. */
  executePlannedCycleWithProvisionedCredentialsWithTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedCycleExecution>> {
    return this.providerSetup.runBrainPlannedCycleWithProvisionedCredentialsWithTrace(this, plan, input, options);
  }

  /** Replay a persisted closed-loop plan after launch admission and protected provisioning. */
  executePlannedCycleWithProvisionedCredentialsWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    return this.providerSetup.runBrainPlannedCycleWithProvisionedCredentialsWithLaunchAdmission(this, plan, input, admission, options);
  }

  /** Replay a persisted closed-loop plan after launch admission with protected provisioning and trace. */
  executePlannedCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedCycleExecution>> {
    return this.providerSetup.runBrainPlannedCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, plan, input, admission, options);
  }

  /** Replay a persisted adaptive plan through the protected provider session boundary. */
  executePlannedAdaptiveCycleWithProvisionedCredentials(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainPlannedAdaptiveCycleWithProvisionedCredentials(this, plan, input, options);
  }

  /** Replay a persisted adaptive plan through protected provisioning and a redacted trace. */
  executePlannedAdaptiveCycleWithProvisionedCredentialsWithTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainPlannedAdaptiveCycleWithProvisionedCredentialsWithTrace(this, plan, input, options);
  }

  /** Replay a persisted adaptive plan after launch admission and protected provisioning. */
  executePlannedAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainPlannedAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(this, plan, input, admission, options);
  }

  /** Replay a persisted adaptive plan after launch admission with protected provisioning and trace. */
  executePlannedAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainTracedAdaptiveCycleExecution>> {
    return this.providerSetup.runBrainPlannedAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(this, plan, input, admission, options);
  }

  /**
   * Create the long-horizon goal runtime bound to this facade's exact agent instance.
   *
   * The caller still owns the ledger, task rehydration, evaluator, learner, recovery journal,
   * and any provider/effect approvals. Binding both `agent` and `brain` here prevents an action
   * handoff resolver from planning with one agent and executing with another, while preserving
   * the runtime's metadata-only persistence and explicit provider boundary.
   */
  createGoalAgentRuntime(options: AutonomousBrainGoalAgentRuntimeOptions): AutonomousBrainGoalAgentRuntime {
    if (!options || typeof options !== "object" || Array.isArray(options)) throw new ArgumentError("autonomous brain goal runtime options must be an object");
    return new AutonomousGoalAgentRuntime({ ...options, agent: this.agent, brain: this });
  }

  /**
   * Construct and run one bounded long-horizon goal loop through this facade.
   *
   * Runtime construction remains explicit under `runtime`, while scheduling, checkpoint, resume,
   * and cycle limits remain explicit under `run`. This mirrors the Python convenience entry point
   * without turning a one-call helper into an implicit persistence, credential, or approval layer.
   */
  runGoalControlLoop(options: AutonomousBrainGoalAgentControlOptions): Promise<AutonomousBrainGoalAgentControlResult> {
    if (!options || typeof options !== "object" || Array.isArray(options)) throw new ArgumentError("autonomous brain goal control options must be an object");
    if (!options.runtime || typeof options.runtime !== "object" || Array.isArray(options.runtime)) throw new ArgumentError("autonomous brain goal control runtime must be an object");
    if (options.run !== undefined && (typeof options.run !== "object" || options.run === null || Array.isArray(options.run))) throw new ArgumentError("autonomous brain goal control run options must be an object");
    return this.createGoalAgentRuntime(options.runtime).run(options.run ?? {});
  }

  /**
   * Inspect the next goal admission decision without requiring task text, credentials, or a
   * resolver. The returned preview is explanatory metadata, not an execution authorization.
   */
  previewGoalControlLoop(options: AutonomousBrainGoalAgentPreviewOptions): AutonomousBrainGoalAgentPreview {
    if (!options || typeof options !== "object" || Array.isArray(options)) throw new ArgumentError("autonomous brain goal preview options must be an object");
    if (!options.runtime || typeof options.runtime !== "object" || Array.isArray(options.runtime)) throw new ArgumentError("autonomous brain goal preview runtime must be an object");
    if (options.schedule_options !== undefined && (typeof options.schedule_options !== "object" || options.schedule_options === null || Array.isArray(options.schedule_options))) throw new ArgumentError("autonomous brain goal preview schedule options must be an object");
    return this.createGoalAgentRuntime(options.runtime).preview({ schedule_options: options.schedule_options });
  }

  /** Compile routing and workflow metadata without contacting a provider or connector. */
  async plan(input: AutonomousBrainRequest): Promise<AutonomousBrainPlan> {
    const request = validateRequest(input);
    const route = await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true });
    return this.buildPlanForRoute(request, route, null);
  }

  /**
   * Start a task-driven durable workflow through the high-level facade.
   *
   * The task is intentionally supplied only to the executor and is never copied into the
   * checkpoint store. A fresh executor is created for each call so learning and stage-adapter
   * ownership stay explicit while the caller controls the durable store and private rehydration.
   */
  async runWorkflow(task: string, options: AutonomousBrainWorkflowOptions): Promise<AutonomousBrainWorkflowResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow options must be an object");
    const { checkpointStore, executorOptions, ...workflowOptions } = options;
    const executor = new AutonomousWorkflowExecutor(this.agent, checkpointStore, executorOptions);
    return executor.start(task, workflowOptions);
  }

  /** Resume a task-driven durable workflow from caller-owned metadata-only checkpoint state. */
  async resumeWorkflow(
    jobId: string,
    task: string,
    options: AutonomousBrainWorkflowResumeOptions,
  ): Promise<AutonomousBrainWorkflowResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow options must be an object");
    const { checkpointStore, executorOptions, ...workflowOptions } = options;
    const executor = new AutonomousWorkflowExecutor(this.agent, checkpointStore, executorOptions);
    return executor.resume(jobId, task, workflowOptions);
  }

  /**
   * Start a workflow only after a provider-free launch admission covers its resolved domain.
   * Semantic routing is rejected because its classifier needs its own explicit admission.
   */
  async runWorkflowWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowOptions,
  ): Promise<AutonomousBrainWorkflowResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWorkflow(task, options);
  }

  /** Resume a workflow only after its caller-owned launch admission covers the resolved domain. */
  async resumeWorkflowWithLaunchAdmission(
    jobId: string,
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowResumeOptions,
  ): Promise<AutonomousBrainWorkflowResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.resumeWorkflow(jobId, task, options);
  }

  /**
   * Run a bounded workflow execution/evaluation cycle through the high-level facade.
   *
   * The evaluator owns task-quality evidence and any replan request; an optional learning
   * controller owns delayed credit. Every retry remains a new digest-bound workflow attempt.
   */
  async runWorkflowCycle(
    task: string,
    options: AutonomousBrainWorkflowCycleOptions,
  ): Promise<AutonomousBrainWorkflowCycleResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow cycle options must be an object");
    const { checkpointStore, executorOptions, ...cycleOptions } = options;
    const executor = new AutonomousWorkflowExecutor(this.agent, checkpointStore, executorOptions);
    return runAutonomousWorkflowCycle(task, executor, cycleOptions);
  }

  /** Run an evaluator-guided workflow cycle only after provider-free launch admission. */
  async runWorkflowCycleWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowCycleOptions,
  ): Promise<AutonomousBrainWorkflowCycleResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWorkflowCycle(task, options);
  }

  /** Run an evaluator-guided workflow cycle while recording metadata-only lifecycle telemetry. */
  async runWorkflowCycleWithTrace(
    task: string,
    options: AutonomousBrainWorkflowCycleTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowCycleResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow cycle trace options must be an object");
    const { checkpointStore, executorOptions, traceStore, runId, ...cycleOptions } = options;
    const trace = await this.createWorkflowTrace(task, cycleOptions, traceStore, runId);
    await trace.started();
    try {
      const executor = new AutonomousWorkflowExecutor(this.agent, checkpointStore, executorOptions);
      const cycle = await runAutonomousWorkflowCycle(task, executor, {
        ...cycleOptions,
        observer: composeBrainObservers(cycleOptions.observer, trace.providerObserver()),
        selectionEventCallback: trace.selectionEventCallback(cycleOptions.selectionEventCallback),
      });
      if (cycle.final) await this.recordWorkflowTracePlan(trace, cycle.final);
      await trace.complete(this.workflowCycleTraceCompletion(cycle));
      return { cycle, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Run a traced evaluator-guided cycle only after provider-free launch admission. */
  async runWorkflowCycleWithLaunchAdmissionAndTrace(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowCycleTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowCycleResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWorkflowCycleWithTrace(task, options);
  }

  /** Run a durable workflow while recording provider/selection lifecycle metadata only. */
  async runWorkflowWithTrace(
    task: string,
    options: AutonomousBrainWorkflowTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow trace options must be an object");
    const { checkpointStore, executorOptions, traceStore, runId, ...workflowOptions } = options;
    const trace = await this.createWorkflowTrace(task, workflowOptions, traceStore, runId);
    await trace.started();
    try {
      const execution = await this.runWorkflow(task, {
        checkpointStore,
        executorOptions,
        ...workflowOptions,
        observer: composeBrainObservers(workflowOptions.observer, trace.providerObserver()),
        selectionEventCallback: trace.selectionEventCallback(workflowOptions.selectionEventCallback),
      });
      await this.recordWorkflowTracePlan(trace, execution);
      await trace.complete(this.workflowTraceCompletion(execution));
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Resume a durable workflow while recording only restart-safe provider/selection metadata. */
  async resumeWorkflowWithTrace(
    jobId: string,
    task: string,
    options: AutonomousBrainWorkflowResumeTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow trace options must be an object");
    const { checkpointStore, executorOptions, traceStore, runId, ...workflowOptions } = options;
    const trace = await this.createWorkflowTrace(task, workflowOptions, traceStore, runId);
    await trace.started();
    try {
      const execution = await this.resumeWorkflow(jobId, task, {
        checkpointStore,
        executorOptions,
        ...workflowOptions,
        observer: composeBrainObservers(workflowOptions.observer, trace.providerObserver()),
        selectionEventCallback: trace.selectionEventCallback(workflowOptions.selectionEventCallback),
      });
      await this.recordWorkflowTracePlan(trace, execution);
      await trace.complete(this.workflowTraceCompletion(execution));
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Run a traced workflow only after provider-free launch admission covers its route. */
  async runWorkflowWithLaunchAdmissionAndTrace(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWorkflowWithTrace(task, options);
  }

  /** Resume a traced workflow only after provider-free launch admission covers its route. */
  async resumeWorkflowWithLaunchAdmissionAndTrace(
    jobId: string,
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowResumeTraceOptions,
  ): Promise<AutonomousBrainTracedWorkflowResult> {
    const domains = await this.workflowLaunchDomains(task, options);
    if (domains.length > 0) authorizeAutonomousLaunchDomains(admission, domains);
    return this.resumeWorkflowWithTrace(jobId, task, options);
  }

  /**
   * Validate the exact domains declared by a caller-owned mission against a deployment launch
   * admission. Mission execution is still separately gated by its policy, provider approval,
   * effect approval, and (when enabled) provider-planning review.
   */
  authorizeMissionLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
  ): AutonomousLaunchAdmissionReport {
    return authorizeAutonomousLaunchDomains(admission, missionDomains(mission));
  }

  /**
   * Execute a caller-owned connector mission through the high-level brain boundary.
   *
   * This keeps the mission graph, connector approval, tool catalogue, and transient result in
   * the caller's process while delegating scheduling, checkpointing, receipt idempotency,
   * evaluator hooks, and online feedback to the reviewed mission adapter. No provider call or
   * external effect is implied by exposing this convenience method.
   */
  async runConnectorMission(
    mission: AgentMissionArgs,
    options: AutonomousBrainConnectorMissionOptions,
  ): Promise<AutonomousBrainConnectorMissionExecution> {
    const validated = validateMissionForBrain(mission);
    return runAutonomousConnectorMission(validated, options);
  }

  /** Execute a connector mission only after a provider-free launch admission covers its domains. */
  async runConnectorMissionWithLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainConnectorMissionOptions,
  ): Promise<AutonomousBrainConnectorMissionExecution> {
    const validated = validateMissionForBrain(mission);
    if (options?.execute?.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted connector execution requires provider-free routing; admit semantic mission routing separately");
    return runAutonomousConnectorMissionWithLaunchAdmission(validated, admission, options);
  }

  /**
   * Produce or replay a provider-ordered connector mission proposal, requiring explicit plan
   * acceptance before any connector dispatch. Accepted replays never call the planner again.
   */
  async runConnectorMissionWithProviderPlanning(
    mission: AgentMissionArgs,
    options: AutonomousBrainConnectorMissionProviderPlanningOptions,
  ): Promise<AutonomousBrainPlannedConnectorMission> {
    const validated = validateMissionForBrain(mission);
    return runAutonomousConnectorMissionWithProviderPlanning(this.agent, validated, options);
  }

  /**
   * Provider-planned connector mission with launch admission checked before planner invocation.
   * Plan acceptance and connector approval remain independent caller decisions.
   */
  async runConnectorMissionWithProviderPlanningAndLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainConnectorMissionProviderPlanningOptions,
  ): Promise<AutonomousBrainPlannedConnectorMission> {
    const validated = validateMissionForBrain(mission);
    if (options?.execution?.execute?.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted connector execution requires provider-free routing; admit semantic mission routing separately");
    return runAutonomousConnectorMissionWithProviderPlanningAndLaunchAdmission(this.agent, validated, admission, options);
  }

  /**
   * Execute reviewed evidence acquisition before entering the ordinary provider boundary.
   * Source dispatch, evidence acceptance, provider approval, and prompt value projection remain
   * independent decisions; the facade only composes them and does not persist transient values.
   */
  async runWithReviewedEvidence(
    task: string,
    options: AutonomousBrainEvidenceBackedRunOptions,
  ): Promise<AutonomousBrainEvidenceBackedRunResult> {
    if (typeof this.agent.runWithReviewedEvidence !== "function") throw new ArgumentError("autonomous brain agent does not expose reviewed evidence execution");
    return this.agent.runWithReviewedEvidence(task, options);
  }

  /**
   * Execute the digest-bound source catalogue before provider invocation. Catalogue normalizers,
   * source reconciliation, provider selection, and caller-owned prompt projection stay visible
   * through the typed result while its JSON image remains metadata-only.
   */
  async runWithDomainEvidenceCatalogue(
    task: string,
    options: AutonomousBrainDomainEvidenceBrainRunOptions,
  ): Promise<AutonomousBrainDomainEvidenceBrainRunResult> {
    if (typeof this.agent.runWithDomainEvidenceCatalogue !== "function") throw new ArgumentError("autonomous brain agent does not expose domain evidence catalogue execution");
    return this.agent.runWithDomainEvidenceCatalogue(task, options);
  }

  /**
   * Attach one hash-chained, metadata-only trace to reviewed adapter evidence execution. The
   * trace covers plan readiness, provider selection/invocation, evidence settlement, and the
   * terminal state; the direct result remains the caller-owned transient value surface.
   */
  async runWithReviewedEvidenceWithTrace(
    task: string,
    options: AutonomousBrainEvidenceBackedTraceOptions,
  ): Promise<AutonomousBrainTracedEvidenceBackedRunResult> {
    const taskDigest = digestJsonSync({ task });
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: taskDigest,
      domains: evidenceTraceDomains(options.domains, options.runMode),
    });
    await trace.started();
    let planRecorded = false;
    try {
      const {
        traceStore: _traceStore,
        runId: _runId,
        beforeProviderRun: callerBeforeProviderRun,
        ...runOptions
      } = options;
      const result = await this.runWithReviewedEvidence(task, {
        ...runOptions,
        beforeProviderRun: async (preflight) => {
          planRecorded = true;
          await trace.record({
            phase: "plan_compiled",
            status: "running",
            plan_digest: preflight.executionPlan.plan_digest,
            detail_digest: digestJsonSync({
              evidence_status: preflight.evidence.status,
              evidence_result_digest: preflight.evidence.result_digest,
              prompt_projection_digest: digestJsonSync(preflight.promptContext),
            }),
          });
          await callerBeforeProviderRun?.(preflight);
        },
        run: tracedEvidenceRunOptions(runOptions.run, trace),
      });
      if (!planRecorded) {
        await trace.record({
          phase: "plan_compiled",
          status: "running",
          plan_digest: result.execution_plan.plan_digest,
          detail_digest: digestJsonSync({
            evidence_status: result.evidence?.status ?? null,
            evidence_result_digest: result.evidence?.result_digest ?? null,
            run_status: result.run?.status ?? null,
            cross_domain_run_status: result.cross_domain_run?.status ?? null,
            automatic_status: result.automatic?.status ?? null,
          }),
        });
      }
      await trace.record({
        phase: "evaluation_settled",
        status: "running",
        plan_digest: result.execution_plan.plan_digest,
        detail_digest: digestJsonSync({
          evidence_status: result.evidence?.status ?? null,
          result_status: result.status,
          prompt_projection_present: result.prompt_context.length > 0,
        }),
      });
      const metadataRun = evidenceTraceMetadataRun(result);
      await trace.complete({
        status: evidenceTraceStatus(result.status),
        domains: evidenceTraceDomains(options.domains, options.runMode),
        route_digest: evidenceTraceRouteDigest(result),
        plan_digest: result.execution_plan.plan_digest,
        selection_digest: metadataRun?.selection ? digestJsonSync(metadataRun.selection) : null,
        detail_digest: digestJsonSync({ status: result.status, evidence_status: result.evidence?.status ?? null }),
      });
      return {
        result,
        trace: await trace.summary(),
        retention: "result_values_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Attach a metadata-only trace to digest-bound catalogue evidence execution. */
  async runWithDomainEvidenceCatalogueWithTrace(
    task: string,
    options: AutonomousBrainDomainEvidenceBrainTraceOptions,
  ): Promise<AutonomousBrainTracedDomainEvidenceBrainRunResult> {
    const taskDigest = digestJsonSync({ task });
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: taskDigest,
      domains: evidenceTraceDomains(options.domains, options.runMode),
    });
    await trace.started();
    let planRecorded = false;
    try {
      const {
        traceStore: _traceStore,
        runId: _runId,
        beforeProviderRun: callerBeforeProviderRun,
        ...runOptions
      } = options;
      const result = await this.runWithDomainEvidenceCatalogue(task, {
        ...runOptions,
        beforeProviderRun: async (preflight) => {
          planRecorded = true;
          await trace.record({
            phase: "plan_compiled",
            status: "running",
            plan_digest: preflight.plan.plan_digest,
            detail_digest: digestJsonSync({
              prepared_requirements: preflight.prepared.length,
              prompt_projection_digest: digestJsonSync(preflight.prompt_context),
            }),
          });
          await callerBeforeProviderRun?.(preflight);
        },
        run: tracedEvidenceRunOptions(runOptions.run, trace),
      });
      if (!planRecorded) {
        await trace.record({
          phase: "plan_compiled",
          status: "running",
          plan_digest: result.plan.plan_digest,
          detail_digest: digestJsonSync({
            prepared_requirements: result.prepared.length,
            reconciled_requirements: result.prepared.filter((item) => item.result !== null).length,
            result_status: result.status,
          }),
        });
      }
      await trace.record({
        phase: "evaluation_settled",
        status: "running",
        plan_digest: result.plan.plan_digest,
        detail_digest: digestJsonSync({
          result_status: result.status,
          reconciliation_statuses: result.prepared.map((item) => item.result?.toJSON().status ?? null),
          prompt_projection_present: result.prompt_context.length > 0,
        }),
      });
      const metadataRun = evidenceTraceMetadataRun(result);
      await trace.complete({
        status: evidenceTraceStatus(result.status),
        domains: evidenceTraceDomains(options.domains, options.runMode),
        route_digest: evidenceTraceRouteDigest(result),
        plan_digest: result.plan.plan_digest,
        selection_digest: metadataRun?.selection ? digestJsonSync(metadataRun.selection) : null,
        detail_digest: digestJsonSync({ status: result.status, prepared_requirements: result.prepared.length }),
      });
      return {
        result,
        trace: await trace.summary(),
        retention: "result_values_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Run reviewed evidence only after a provider-free launch admission covers its full scope. */
  async runWithReviewedEvidenceWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainEvidenceBackedRunOptions,
  ): Promise<AutonomousBrainEvidenceBackedRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithReviewedEvidence(task, options);
  }

  /** Run catalogue-backed evidence only after a provider-free launch admission covers its scope. */
  async runWithDomainEvidenceCatalogueWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainDomainEvidenceBrainRunOptions,
  ): Promise<AutonomousBrainDomainEvidenceBrainRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted catalogue evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithDomainEvidenceCatalogue(task, options);
  }

  /** Launch-admitted reviewed evidence trace; provider-assisted routing is refused before sources. */
  async runWithReviewedEvidenceWithLaunchAdmissionAndTrace(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainEvidenceBackedTraceOptions,
  ): Promise<AutonomousBrainTracedEvidenceBackedRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted traced evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithReviewedEvidenceWithTrace(task, options);
  }

  /** Launch-admitted catalogue evidence trace with the same independent source/provider gates. */
  async runWithDomainEvidenceCatalogueWithLaunchAdmissionAndTrace(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainDomainEvidenceBrainTraceOptions,
  ): Promise<AutonomousBrainTracedDomainEvidenceBrainRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted traced catalogue evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithDomainEvidenceCatalogueWithTrace(task, options);
  }

  /**
   * Execute reviewed evidence through the restart-safe checkpoint protocol. A provider result is
   * never replayed implicitly: recovery requires a caller rehydrator or an explicit resume flag.
   */
  async runWithReviewedEvidenceResumable(
    task: string,
    options: AutonomousBrainEvidenceBackedResumableExecutionOptions,
  ): Promise<AutonomousBrainEvidenceBackedResumableRun> {
    return runAutonomousEvidenceBackedResumable(this.agent, task, options);
  }

  /** Restart-safe reviewed evidence execution with launch admission rechecked before recovery. */
  async runWithReviewedEvidenceResumableWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainEvidenceBackedResumableExecutionOptions,
  ): Promise<AutonomousBrainEvidenceBackedResumableRun> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted resumable evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithReviewedEvidenceResumable(task, options);
  }

  /**
   * Trace restart-safe evidence execution while keeping checkpoint persistence and provider
   * recovery in the caller's authority. Checkpoint events contain only status and digests; the
   * direct resumable result retains its caller-owned transient values outside the trace.
   */
  async runWithReviewedEvidenceResumableWithTrace(
    task: string,
    options: AutonomousBrainEvidenceBackedResumableTraceOptions,
  ): Promise<AutonomousBrainTracedEvidenceBackedResumableRun> {
    const taskDigest = digestJsonSync({ task });
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: taskDigest,
      domains: evidenceTraceDomains(options.domains, options.runMode),
    });
    await trace.started();
    let planRecorded = false;
    try {
      const {
        traceStore: _traceStore,
        runId: _runId,
        checkpointSink: callerCheckpointSink,
        ...runOptions
      } = options;
      const run = await this.runWithReviewedEvidenceResumable(task, {
        ...runOptions,
        checkpointSink: async (checkpoint) => {
          if (!planRecorded) {
            planRecorded = true;
            await trace.record({
              phase: "plan_compiled",
              status: "running",
              plan_digest: checkpoint.execution_plan_digest,
              detail_digest: digestJsonSync({ checkpoint_status: checkpoint.status }),
            });
          }
          await callerCheckpointSink(checkpoint);
          await trace.record({
            phase: "evaluation_settled",
            status: "running",
            plan_digest: checkpoint.execution_plan_digest,
            detail_digest: digestJsonSync({
              checkpoint_status: checkpoint.status,
              checkpoint_digest: checkpoint.checkpoint_digest,
              provider_result_digest: checkpoint.provider_result_digest,
              provider_rehydrated: checkpoint.provider_result_digest !== null,
            }),
          });
        },
        run: tracedEvidenceRunOptions(runOptions.run, trace),
      });
      const evidenceResult = run.result;
      if (!planRecorded) {
        await trace.record({
          phase: "plan_compiled",
          status: "running",
          plan_digest: evidenceResult.execution_plan.plan_digest,
          detail_digest: digestJsonSync({
            evidence_status: evidenceResult.evidence?.status ?? null,
            resumable_status: run.status,
            provider_rehydrated: run.provider_rehydrated,
          }),
        });
      }
      const metadataRun = evidenceTraceMetadataRun(evidenceResult);
      await trace.complete({
        status: evidenceTraceStatus(run.status),
        domains: evidenceTraceDomains(options.domains, options.runMode),
        route_digest: evidenceTraceRouteDigest(evidenceResult),
        plan_digest: evidenceResult.execution_plan.plan_digest,
        selection_digest: metadataRun?.selection ? digestJsonSync(metadataRun.selection) : null,
        detail_digest: digestJsonSync({ status: run.status, checkpoint_status: run.checkpoint.status }),
      });
      return {
        run,
        trace: await trace.summary(),
        retention: "result_values_and_checkpoints_caller_owned;trace_metadata_only_no_evidence_prompts_responses_or_credentials",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Launch-admitted restart-safe evidence trace with provider rerouting refused up front. */
  async runWithReviewedEvidenceResumableWithLaunchAdmissionAndTrace(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainEvidenceBackedResumableTraceOptions,
  ): Promise<AutonomousBrainTracedEvidenceBackedResumableRun> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    this.rejectLaunchAdmittedSemanticRouting(options?.run?.semanticRouting, "launch-admitted traced resumable evidence execution requires provider-free routing; admit semantic routing separately before enabling it");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithReviewedEvidenceResumableWithTrace(task, options);
  }

  /** Create a serialized, CAS-capable evidence controller for a caller-owned job. */
  createEvidenceBackedController(jobId: string, persistence: AutonomousEvidenceBackedCheckpointStore): AutonomousEvidenceBackedController {
    return new AutonomousEvidenceBackedController(this.agent, jobId, persistence);
  }

  /**
   * Run the durable mission planner/executor through the application-facing brain boundary.
   * The mission graph remains caller-owned: this method adds the autonomous per-step model,
   * prompt, tool, policy, checkpoint, evaluator, replanning, and learning composition without
   * granting any provider, credential, or external-effect authority implicitly.
   */
  async runMissionReplanCycle(
    mission: AgentMissionArgs,
    options: AutonomousAgentMissionReplanOptions,
  ): Promise<AutonomousMissionReplanResult> {
    const validated = validateMissionForBrain(mission);
    validateMissionReplanOptions(options);
    if (typeof this.agent.runMissionReplanCycle !== "function") throw new ArgumentError("autonomous brain agent does not expose mission replanning");
    return this.agent.runMissionReplanCycle(validated, options);
  }

  /** Run a mission only after a provider-free launch admission covers every declared domain. */
  async runMissionReplanCycleWithLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousAgentMissionReplanOptions,
  ): Promise<AutonomousMissionReplanResult> {
    const validated = validateMissionForBrain(mission);
    validateMissionReplanOptions(options);
    if (options.execute?.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted mission execution requires provider-free routing; admit semantic mission routing separately");
    this.authorizeMissionLaunchAdmission(validated, admission);
    return this.runMissionReplanCycle(validated, options);
  }

  /** Run a mission while attaching a hash-chained trace that never serializes mission values. */
  async runMissionReplanCycleWithTrace(
    mission: AgentMissionArgs,
    options: AutonomousBrainMissionReplanTraceOptions,
  ): Promise<AutonomousBrainTracedMissionReplanResult> {
    const validated = validateMissionForBrain(mission);
    validateMissionReplanOptions(options);
    const domains = missionDomains(validated);
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: digestJsonSync({ task: validated.goal }),
      domains,
    });
    await trace.started();
    try {
      const missionDigest = digestJsonSync(validated);
      await trace.record({ phase: "plan_compiled", status: "running", domains, plan_digest: missionDigest });
      const {
        traceStore: _traceStore,
        runId: _runId,
        stepRun: sourceStepRun,
        ...missionOptions
      } = options;
      const traceObserver = trace.providerObserver();
      const tracedStepRun = {
        ...(sourceStepRun ?? {}),
        observer: composeBrainObservers(sourceStepRun?.observer, traceObserver),
        selectionEventCallback: composeSelectionCallbacks(sourceStepRun?.selectionEventCallback, trace.selectionEventCallback()),
      };
      const result = await this.runMissionReplanCycle(validated, { ...missionOptions, stepRun: tracedStepRun });
      await trace.record({
        phase: "evaluation_settled",
        status: "running",
        route_digest: result.route_digest,
        plan_digest: result.protected_contract_digest,
        detail_digest: digestJsonSync({
          replan_count: result.replan_count,
          attempt_count: result.attempts.length,
          evaluation_count: result.evaluations.length,
          planning_status: result.planning_status,
          planner_learning_status: result.planner_learning_status,
          attempt_statuses: result.attempts.map((attempt) => ({ attempt: attempt.attempt, status: attempt.status, evaluation_digest: attempt.evaluation_digest })),
        }),
      });
      if (result.learning_settlements.length > 0 || result.prompt_learning !== undefined) {
        await trace.record({
          phase: "learning_prepared",
          status: "running",
          route_digest: result.route_digest,
          plan_digest: result.protected_contract_digest,
          detail_digest: digestJsonSync({
            settlement_count: result.learning_settlements.length,
            prompt_selection_count: result.prompt_learning?.selection_count ?? 0,
            planner_learning_status: result.planner_learning_status,
          }),
        });
      }
      await trace.complete({
        status: missionTraceStatus(result.status),
        domains,
        route_digest: result.route_digest,
        plan_digest: result.protected_contract_digest,
        detail_digest: digestJsonSync({ status: result.status, final_status: result.final_execution.status, replan_count: result.replan_count }),
      });
      return tracedMissionReplanResult(result, await trace.summary());
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Launch-admitted variant of the digest-only mission trace boundary. */
  async runMissionReplanCycleWithLaunchAdmissionAndTrace(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainMissionReplanTraceOptions,
  ): Promise<AutonomousBrainTracedMissionReplanResult> {
    const validated = validateMissionForBrain(mission);
    validateMissionReplanOptions(options);
    if (options.execute?.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted traced mission execution requires provider-free routing; admit semantic mission routing separately");
    this.authorizeMissionLaunchAdmission(validated, admission);
    return this.runMissionReplanCycleWithTrace(validated, options);
  }

  /** Convert a caller-owned execution failure projection into a deterministic recovery plan. */
  planRecovery(observation: AutonomousRecoveryObservation): AutonomousRecoveryPlan {
    return planAutonomousRecovery(observation);
  }

  /** Submit a recovery plan to a caller-owned review ledger without dispatching recovery work. */
  submitRecoveryHandoff(
    ledger: AutonomousRecoveryHandoffLedger,
    input: { observation: AutonomousRecoveryObservation; run_id_digest: string; attempt?: number },
  ): AutonomousRecoveryHandoffSubmissionResult {
    if (!(ledger instanceof AutonomousRecoveryHandoffLedger)) throw new ArgumentError("autonomous brain recovery handoff requires an AutonomousRecoveryHandoffLedger");
    if (!input || typeof input !== "object") throw new ArgumentError("autonomous brain recovery handoff input must be an object");
    return ledger.submit({ plan: this.planRecovery(input.observation), run_id_digest: input.run_id_digest, attempt: input.attempt ?? 0 });
  }

  /**
   * Select a bounded execution strategy after deterministic routing and before provider/tool
   * dispatch. This composes route identity with the joint policy, but keeps evaluator settlement
   * and every external authorization boundary with the caller-owned policy and run APIs.
   */
  async selectExecutionPolicy(input: AutonomousBrainRequest, options: AutonomousBrainExecutionPolicyOptions): Promise<AutonomousBrainExecutionPolicyPlan> {
    const request = validateRequest(input);
    if (!options || !Array.isArray(options.candidates) || options.candidates.length === 0) throw new ArgumentError("autonomous brain execution policy requires candidates");
    const route = await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true });
    if (route.abstained || route.selected_domains.length === 0) throw new ArgumentError("autonomous brain execution policy requires an admitted route");
    const primaryDomain = route.primary_domain ?? route.selected_domains[0]!;
    const domainPolicy = this.agent.domainPolicy(primaryDomain);
    const policy = options.policy ?? new AutonomousJointExecutionPolicy();
    const decision = policy.select({
      context_digest: route.task_digest,
      requested_domains: route.selected_domains,
      required_capabilities: options.requiredCapabilities ?? [],
      preferred_capabilities: options.preferredCapabilities ?? [],
      required_path: options.requiredPath ?? null,
      evidence_required: options.evidenceRequired ?? domainPolicy.evidence_mode === "required_before_provider",
      structured_output_required: options.structuredOutputRequired ?? domainPolicy.response_mode === "structured_required",
      effects_requested: options.effectsRequested ?? false,
      effects_approved: options.effectsApproved ?? false,
      approval_granted: options.approvalGranted ?? false,
      max_cost_units: options.maxCostUnits ?? domainPolicy.max_total_cost_units,
      max_latency_ms: options.maxLatencyMs,
      max_risk: options.maxRisk,
      min_score: options.minScore,
    }, options.candidates);
    const descriptor = { schema: AUTONOMOUS_BRAIN_EXECUTION_POLICY_SCHEMA, route_digest: route.route_digest, decision_digest: decision.decision_digest };
    return {
      schema: AUTONOMOUS_BRAIN_EXECUTION_POLICY_SCHEMA,
      route,
      decision,
      policy_plan_digest: digestJsonSync(descriptor),
      retention: "route_and_policy_metadata_only;task_prompt_response_tool_and_credential_values_not_retained",
      secret_material: "never_returned",
    };
  }

  /**
   * Compile one deterministic next-action handoff from the request-free plan.  This adds no
   * authority: provider, connector, evidence, tool, evaluator, credential, and effect gates
   * remain independently owned by their explicit APIs.
   */
  async actionPlan(input: AutonomousBrainRequest): Promise<AutonomousActionPlan> {
    const plan = await this.plan(input);
    return buildAutonomousActionPlan(plan.toJSON());
  }

  /**
   * Replay, admit, and execute one digest-bound action plan.
   *
   * The request is re-planned before admission, so a changed task, domain, connector, or
   * deterministic route cannot reuse an old approval. Missing gates return without touching a
   * connector or provider. Once admitted, the existing runAuto/execute boundaries retain
   * ownership of credentials, evidence, model selection, tools, evaluators, and effects.
   */
  async executeActionPlan(
    input: AutonomousBrainRequest,
    source: AutonomousActionPlan | AutonomousActionPlanJSON,
    options: AutonomousActionPlanExecutionOptions = {},
  ): Promise<AutonomousActionPlanExecution> {
    const request = validateRequest(input);
    const actionPlan = source instanceof AutonomousActionPlan ? source : AutonomousActionPlan.fromJSON(source);
    const expected = await this.actionPlan(request);
    if (expected.plan_digest !== actionPlan.plan_digest) throw new ArgumentError("autonomous action plan is stale or does not match the transient request");
    const admission = admitAutonomousActionPlan(actionPlan, { approvals: options.approvals, reviewed: options.reviewed ?? false });
    const base = (status: AutonomousActionPlanExecution["status"], result: AutonomousActionPlanExecution["result"]): AutonomousActionPlanExecution => ({
      schema: AUTONOMOUS_ACTION_EXECUTION_FACADE_SCHEMA,
      status,
      execution_status: result === null ? admission.status : (result.status ?? status),
      plan: actionPlan.toJSON(),
      admission: admission.toJSON(),
      result,
      retention: "plan_and_admission_metadata_only;execution_result_is_caller_owned",
      authorization: "caller_owned_execution_result;provider_and_effect_authority_remain_explicit",
      secret_material: "never_returned",
    });
    if (admission.status !== "admitted") return base(admission.status, null);

    const {
      approvals: _approvals,
      reviewed: _reviewed,
      connectorFirst,
      includeConnectorObservation,
      planningMode,
      ...callerRunOptions
    } = options;
    const runOptions: AutonomousAutoRunOptions = {
      ...callerRunOptions,
      domain: request.domain,
      capability: request.capability,
      context: request.context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
    };
    const enableGate = (key: "approveProviderCall" | "domainPolicyEvidenceReady" | "domainPolicyPlanAccepted" | "domainPolicyEffectsRequested" | "domainPolicyEffectsApproved"): void => {
      const current = runOptions[key];
      if (current !== undefined && current !== true) throw new ArgumentError(`action-plan approval contradicts ${key}=false`);
      (runOptions as Record<string, unknown>)[key] = true;
    };
    for (const gate of admission.approved_approvals) {
      if (gate === "provider_call") enableGate("approveProviderCall");
      else if (gate === "evidence_dispatch") enableGate("domainPolicyEvidenceReady");
      else if (gate === "plan_acceptance") enableGate("domainPolicyPlanAccepted");
      else if (gate === "effect_approval") {
        enableGate("domainPolicyEffectsRequested");
        enableGate("domainPolicyEffectsApproved");
      }
    }
    if (admission.execution_path === "planning") {
      if (planningMode !== undefined && planningMode !== "provider") throw new ArgumentError("planning action plans require planningMode='provider'");
      runOptions.planningMode = "provider";
    } else if (planningMode !== undefined) {
      runOptions.planningMode = planningMode;
    }

    let result: AutonomousActionPlanExecution["result"];
    if (request.connector !== undefined) {
      if (runOptions.planningMode === "provider") throw new ArgumentError("provider planning with connector action plans must use the explicit planAndRun connector boundary");
      const { domain: _domain, capability: _capability, context: _context, hints: _hints, allowCrossDomain: _allowCrossDomain, planningMode: _planning, ...connectorRun } = runOptions;
      result = await this.execute(request, {
        approveProviderCall: true,
        connectorFirst,
        includeConnectorObservation,
        run: connectorRun,
      });
    } else {
      result = await this.agent.runAuto(request.task, runOptions);
    }
    const finished = result.status === "completed";
    return base(finished ? "completed" : result.status === "route_review_required" ? "route_review_required" : result.status === "policy_blocked" ? "blocked" : "review_required", result);
  }

  /**
   * Revalidate and execute one operator-produced dispatch handoff.
   *
   * The handoff is continuity metadata, not a credential or execution token. This method
   * replays the embedded plan against the transient request, reproduces the admitted gates,
   * and then delegates to the existing action-plan execution boundary so model selection,
   * provider approval, evidence, tools, evaluators, and effects retain their independent gates.
   */
  async executeActionHandoff(
    input: AutonomousBrainRequest,
    source: AutonomousActionDispatchHandoff | JsonObject,
    options: AutonomousActionHandoffExecutionOptions = {},
  ): Promise<AutonomousActionPlanExecution> {
    const request = validateRequest(input);
    const handoff = validateAutonomousActionDispatchHandoff(source);
    if (request.domain !== undefined && !handoff.selected_domains.includes(request.domain) && !(request.domain === "cross_domain" && handoff.cross_domain)) throw new ArgumentError("autonomous action handoff does not cover the transient request domain");
    const actionPlan = AutonomousActionPlan.fromJSON(handoff.plan);
    const approvals = Object.fromEntries(handoff.admission.approved_approvals.map((gate) => [gate, true])) as Partial<Record<AutonomousActionPlanApproval, boolean>>;
    const execution = await this.executeActionPlan(request, actionPlan, { ...options, approvals, reviewed: true });
    if (execution.plan.plan_digest !== handoff.plan_digest || execution.admission.admission_digest !== handoff.admission_digest) throw new ArgumentError("autonomous action handoff admission drifted during execution replay");
    return execution;
  }

  private async buildPlanForRoute(
    request: AutonomousBrainRequest,
    route: AutonomousRouteProposal,
    semanticRoute: AutonomousSemanticRouteResult | null,
  ): Promise<AutonomousBrainPlan> {
    let domainPlan: AutonomousBrainDomainPlanSummary | null = null;
    let crossDomainPlan: AutonomousBrainCrossDomainPlanSummary | null = null;
    if ((semanticRoute === null || semanticRoute.status === "completed") && !route.abstained && route.primary_domain !== null) {
      const blueprint = await this.agent.blueprint(request.task, {
        routeOverride: route,
        capability: request.capability,
        context: request.context,
        hints: request.hints,
      });
      if (blueprint.cross_domain_blueprint) crossDomainPlan = projectCrossDomainBlueprint(blueprint.cross_domain_blueprint);
      else if (blueprint.blueprint) domainPlan = projectTaskBlueprint(blueprint.blueprint, route.route_digest);
    }
    let connectorPlan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null = null;
    let connectorStatus: AutonomousBrainPlanStatus | null = null;
    if (semanticRoute !== null && semanticRoute.status !== "completed") {
      return new AutonomousBrainPlan({ status: "route_review_required", route, semantic_route: semanticRoute, domain_plan: null, cross_domain_plan: null, connector_plan: null });
    }
    if (request.connector !== undefined) {
      if (!this.connectorOperations) throw new ArgumentError("autonomous brain connector input requires connectorOperations");
      if (route.abstained || route.primary_domain === null || !route.selected_domains.includes(request.connector.domain)) throw new ArgumentError("autonomous brain connector domain is outside the reviewed route");
      const typed = this.connectorOperations.plan(request.connector);
      connectorPlan = typed.toJSON();
      if (typed.status !== "ready") connectorStatus = "connector_review_required";
    }
    const status: AutonomousBrainPlanStatus = route.abstained || route.primary_domain === null
      ? "route_review_required"
      : connectorStatus ?? "ready";
    return new AutonomousBrainPlan({ status, route, semantic_route: semanticRoute, domain_plan: domainPlan, cross_domain_plan: crossDomainPlan, connector_plan: connectorPlan });
  }

  /** Execute a fresh request after compiling its request-free plan. */
  async execute(input: AutonomousBrainRequest, options: AutonomousBrainExecuteOptions = {}): Promise<AutonomousBrainExecution> {
    const prepared = await this.prepare(input, selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting), options.run, options.approveProviderCall);
    return this.executePrepared(prepared, options);
  }

  /**
   * Open a transient direct stream through the façade's request validation boundary.
   * Connector-bearing requests are rejected because a stream cannot silently skip the reviewed
   * connector phase; callers should use `execute()` when connector-first execution is required.
   */
  async executeStream(input: AutonomousBrainRequest, options: AutonomousBrainStreamOptions = {}): Promise<AutonomousRunStreamHandle> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain streaming does not skip connector execution; use execute() for connector-bearing requests");
    return this.agent.runStream(request.task, {
      ...options,
      ...(request.domain === undefined ? {} : { domain: request.domain }),
      ...(request.capability === undefined ? {} : { capability: request.capability }),
      ...(request.hints === undefined ? {} : { hints: request.hints }),
      ...(request.context === undefined ? {} : { context: request.context }),
      ...(request.allow_cross_domain === undefined ? {} : { allowCrossDomain: request.allow_cross_domain }),
    });
  }

  /** Open the deterministic automatic stream through the façade's validated request boundary. */
  async executeAutoStream(input: AutonomousBrainRequest, options: AutonomousBrainAutoStreamOptions = {}): Promise<AutonomousRunStreamHandle> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain streaming does not skip connector execution; use executeAuto() for connector-bearing requests");
    return this.agent.runAutoStream(request.task, {
      ...options,
      ...(request.domain === undefined ? {} : { domain: request.domain }),
      ...(request.capability === undefined ? {} : { capability: request.capability }),
      ...(request.hints === undefined ? {} : { hints: request.hints }),
      ...(request.context === undefined ? {} : { context: request.context }),
      ...(request.allow_cross_domain === undefined ? {} : { allowCrossDomain: request.allow_cross_domain }),
    });
  }

  /** Open the bounded specialist-fan-out/synthesis stream through the validated façade boundary. */
  async executeCrossDomainStream(input: AutonomousBrainRequest, options: AutonomousBrainCrossDomainStreamOptions = {}): Promise<AutonomousRunStreamHandle> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain streaming does not skip connector execution; use executeCrossDomain() for connector-bearing requests");
    if (request.domain !== undefined) throw new ArgumentError("autonomous brain cross-domain streaming does not accept an explicit single domain");
    return this.agent.runCrossDomainStream(request.task, {
      ...options,
      ...(request.capability === undefined ? {} : { capability: request.capability }),
      ...(request.hints === undefined ? {} : { hints: request.hints }),
      ...(request.context === undefined ? {} : { context: request.context }),
      ...(request.allow_cross_domain === undefined ? {} : { allowCrossDomain: request.allow_cross_domain }),
    });
  }

  /**
   * Execute the complete automatic route -> blueprint -> invocation boundary.
   *
   * This is the high-level entry point for applications that want the agent to choose its
   * single- or cross-domain path and then invoke through the deterministic or provider-planned
   * automatic runner. The route and blueprint are compiled once by the facade and passed back as
   * an exact digest-checked override, so automatic execution cannot silently widen its reviewed
   * scope between planning and invocation.
   */
  async executeAuto(input: AutonomousBrainRequest, options: AutonomousBrainAutoExecuteOptions = {}): Promise<AutonomousBrainAutoExecution> {
    const request = validateRequest(input);
    const prepared = await this.prepare(request, selectBrainSemanticRouting(options.semanticRouting, undefined), options, options.approveProviderCall);
    return this.executeAutoPrepared(prepared, options);
  }

  /**
   * Execute automatic planning only after a caller-owned, provider-free launch admission covers
   * the frozen route. Provider-assisted semantic routing is rejected here because its classifier
   * is a separate provider boundary that must be reviewed independently.
   */
  async executeAutoWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoExecuteOptions = {},
  ): Promise<AutonomousBrainAutoExecution> {
    if (options.semanticRouting !== undefined && options.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic execution requires provider-free routing; admit semantic routing separately before enabling it");
    const request = validateRequest(input);
    const prepared = await this.prepare(request, undefined, options, options.approveProviderCall);
    if (!prepared.route.abstained) authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeAutoPrepared(prepared, options);
  }

  /** Execute automatic planning and invocation while recording only metadata in a caller trace. */
  async executeAutoWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainAutoTraceOptions): Promise<AutonomousBrainTracedAutoExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeAutoWithTrace options must be an object");
    const prepared = await this.prepare(request, selectBrainSemanticRouting(options.semanticRouting, undefined), options, options.approveProviderCall);
    return this.executeAutoPreparedWithTrace(prepared, options);
  }

  /** Automatic route/planning/invocation with provider-free launch admission and a metadata trace. */
  async executeAutoWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoTraceOptions,
  ): Promise<AutonomousBrainTracedAutoExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeAutoWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting !== undefined && options.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic tracing requires provider-free routing; admit semantic routing separately before enabling it");
    const request = validateRequest(input);
    const prepared = await this.prepare(request, undefined, options, options.approveProviderCall);
    if (!prepared.route.abstained) authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeAutoPreparedWithTrace(prepared, options);
  }

  /**
   * Execute only when a caller-owned admission explicitly covers the final reviewed route.
   * Planning remains provider-free; the admission check is the last facade decision before
   * connector and provider dispatch.  Provider/effect approval is still independently required.
   */
  async executeWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainExecuteOptions = {},
  ): Promise<AutonomousBrainExecution> {
    if (options.semanticRouting === true || (isObject(options.semanticRouting) && options.semanticRouting.enabled === true) || options.run?.semanticRouting === true || (isObject(options.run?.semanticRouting) && options.run.semanticRouting.enabled === true)) throw new ArgumentError("launch-admitted execution requires provider-free routing; admit semantic routing separately before enabling it");
    const prepared = await this.prepare(input, selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting), options.run, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executePrepared(prepared, options);
  }

  /**
   * Execute the complete reviewed brain boundary while retaining a caller-owned trace of plan,
   * connector, provider, and terminal transitions. The trace never receives transient values.
   */
  async executeWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainTraceOptions): Promise<AutonomousBrainTracedExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeWithTrace options must be an object");
    const prepared = await this.prepare(request, selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting), options.run, options.approveProviderCall);
    return this.executePreparedWithTrace(prepared, options);
  }

  /** Direct facade execution with provider-free launch admission and a metadata trace. */
  async executeWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainTraceOptions,
  ): Promise<AutonomousBrainTracedExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting === true || (isObject(options.semanticRouting) && options.semanticRouting.enabled === true) || options.run?.semanticRouting === true || (isObject(options.run?.semanticRouting) && options.run.semanticRouting.enabled === true)) throw new ArgumentError("launch-admitted tracing requires provider-free routing; admit semantic routing separately before enabling it");
    const request = validateRequest(input);
    const prepared = await this.prepare(request, selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting), options.run, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executePreparedWithTrace(prepared, options);
  }

  /**
   * Revalidate a persisted plan and authorize its exact current route before credentials or
   * provider work are opened. The route is recomputed provider-free so a stale plan cannot use
   * an old domain set to pass admission after the catalogue or workflow contracts change.
   */
  async authorizePlannedLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
  ): Promise<AutonomousLaunchAdmissionReport> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain planned launch admission requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain planned launch admission does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain planned launch admission does not match current metadata");
    return authorizeAutonomousLaunchDomains(admission, reviewedPlan.selected_domains);
  }

  /** Recompile and verify a persisted metadata-only plan before supplying transient task values. */
  async executePlanned(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainExecuteOptions = {}): Promise<AutonomousBrainExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlanned requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain plan does not match the transient request");
    return this.executePrepared(prepared, options);
  }

  /** Replay a reviewed plan only after its exact route passes provider-free launch admission. */
  async executePlannedWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainExecuteOptions = {},
  ): Promise<AutonomousBrainExecution> {
    if (options.semanticRouting === true || (isObject(options.semanticRouting) && options.semanticRouting.enabled === true) || options.run?.semanticRouting === true || (isObject(options.run?.semanticRouting) && options.run.semanticRouting.enabled === true)) throw new ArgumentError("launch-admitted planned execution requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlanned(plan, input, options);
  }

  /** Rehydrate a reviewed plan, then execute it through the same full traced facade boundary. */
  async executePlannedWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainTraceOptions): Promise<AutonomousBrainTracedExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlannedWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain traced plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain traced plan does not match the transient request");
    return this.executePreparedWithTrace(prepared, options);
  }

  /** Traced plan replay with provider-free launch admission before any provider boundary. */
  async executePlannedWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainTraceOptions,
  ): Promise<AutonomousBrainTracedExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executePlannedWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting === true || (isObject(options.semanticRouting) && options.semanticRouting.enabled === true) || options.run?.semanticRouting === true || (isObject(options.run?.semanticRouting) && options.run.semanticRouting.enabled === true)) throw new ArgumentError("launch-admitted traced planned execution requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlannedWithTrace(plan, input, options);
  }

  /** Execute the closed-loop route -> invoke -> evaluate -> learn cycle behind the same plan boundary. */
  async executeCycle(input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    const prepared = await this.prepare(input, options.semanticRouting, options.cycle, options.approveProviderCall);
    return this.executeCyclePrepared(prepared, options);
  }

  /** Execute the automatic evaluator-backed cycle with request-owned route fields. */
  async executeAutoCycle(
    input: AutonomousBrainRequest,
    options: AutonomousBrainAutoCycleOptions = {},
  ): Promise<AutonomousBrainAutoCycleResult> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain automatic cycle does not skip connector execution; use executeCycle() for connector-bearing requests");
    return this.agent.runAutoCycle(request.task, bindAutomaticCycleOptions(request, options) as AutonomousAutoDecisionCycleOptions);
  }

  /** Execute an automatic evaluator-backed cycle only after provider-free launch admission. */
  async executeAutoCycleWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoCycleOptions = {},
  ): Promise<AutonomousBrainAutoCycleResult> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain launch-admitted automatic cycle does not skip connector execution; use executeCycleWithLaunchAdmission() for connector-bearing requests");
    const bound = bindAutomaticCycleOptions(request, options);
    if (isObject(bound.semanticRouting) && bound.semanticRouting.enabled === true) throw new ArgumentError("launch-admitted automatic cycle requires provider-free routing; admit semantic routing separately before enabling it");
    return this.agent.runAutoCycleWithLaunchAdmission(request.task, admission, bound as AutonomousAutoDecisionCycleOptions);
  }

  /** Execute the automatic evaluator-guided replan cycle with request-owned route fields. */
  async executeAutoReplanCycle(
    input: AutonomousBrainRequest,
    options: AutonomousBrainAutoReplanCycleOptions,
  ): Promise<AutonomousBrainAutoReplanCycleResult> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain automatic replan cycle does not skip connector execution; use executeAdaptiveCycle() for connector-bearing requests");
    return this.agent.runAutoReplanCycle(request.task, bindAutomaticCycleOptions(request, options) as AutonomousAutoReplanCycleOptions);
  }

  /** Execute the automatic replan cycle only after provider-free launch admission. */
  async executeAutoReplanCycleWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoReplanCycleOptions,
  ): Promise<AutonomousBrainAutoReplanCycleResult> {
    const request = validateRequest(input);
    if (request.connector !== undefined) throw new ArgumentError("autonomous brain launch-admitted automatic replan cycle does not skip connector execution; use executeAdaptiveCycleWithLaunchAdmission() for connector-bearing requests");
    const bound = bindAutomaticCycleOptions(request, options);
    if (isObject(bound.semanticRouting) && bound.semanticRouting.enabled === true) throw new ArgumentError("launch-admitted automatic replan cycle requires provider-free routing; admit semantic routing separately before enabling it");
    return this.agent.runAutoReplanCycleWithLaunchAdmission(request.task, admission, bound as AutonomousAutoReplanCycleOptions);
  }

  /** Run the closed-loop cycle only when the reviewed route is covered by launch admission. */
  async executeCycleWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCycleOptions = {},
  ): Promise<AutonomousBrainCycleExecution> {
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted cycle requires provider-free routing; admit semantic routing separately before enabling it");
    const prepared = await this.prepare(input, options.semanticRouting, options.cycle, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeCyclePrepared(prepared, options);
  }

  /** Rehydrate a persisted brain plan, then run the closed-loop evaluator/learning cycle. */
  async executePlannedCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlannedCycle requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain cycle plan does not match the transient request");
    return this.executeCyclePrepared(prepared, options);
  }

  /** Replay a reviewed closed-loop plan only after provider-free launch admission. */
  async executePlannedCycleWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCycleOptions = {},
  ): Promise<AutonomousBrainCycleExecution> {
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted planned cycle requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlannedCycle(plan, input, options);
  }

  /** Execute a closed-loop cycle while tracing planning, connectors, provider turns, evaluation, and learning. */
  async executeCycleWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainCycleTraceOptions): Promise<AutonomousBrainTracedCycleExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeCycleWithTrace options must be an object");
    const prepared = await this.prepare(request, options.semanticRouting, options.cycle, options.approveProviderCall);
    return this.executeCyclePreparedWithTrace(prepared, options);
  }

  /** Closed-loop cycle with provider-free launch admission and a metadata trace. */
  async executeCycleWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCycleTraceOptions,
  ): Promise<AutonomousBrainTracedCycleExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeCycleWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted cycle tracing requires provider-free routing; admit semantic routing separately before enabling it");
    const request = validateRequest(input);
    const prepared = await this.prepare(request, options.semanticRouting, options.cycle, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeCyclePreparedWithTrace(prepared, options);
  }

  /** Rehydrate a reviewed plan, then execute its closed-loop cycle through the trace boundary. */
  async executePlannedCycleWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainCycleTraceOptions): Promise<AutonomousBrainTracedCycleExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlannedCycleWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain traced cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain traced cycle plan does not match the transient request");
    return this.executeCyclePreparedWithTrace(prepared, options);
  }

  /** Traced closed-loop plan replay with provider-free launch admission before dispatch. */
  async executePlannedCycleWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCycleTraceOptions,
  ): Promise<AutonomousBrainTracedCycleExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executePlannedCycleWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted traced planned cycle requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlannedCycleWithTrace(plan, input, options);
  }

  /**
   * Execute the bounded evaluator -> learn -> optional replan loop behind the same route,
   * connector, approval, and metadata-only plan boundary. Replanning is always delegated to
   * the lower-level capped loop, so evaluator feedback cannot silently widen authority.
   */
  async executeAdaptiveCycle(input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    const prepared = await this.prepare(input, options.semanticRouting, options.adaptive, options.approveProviderCall);
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Run the bounded evaluator/replan loop only when every reviewed route domain is admitted. */
  async executeAdaptiveCycleWithLaunchAdmission(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAdaptiveCycleOptions,
  ): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted adaptive cycle requires provider-free routing; admit semantic routing separately before enabling it");
    const prepared = await this.prepare(input, options.semanticRouting, options.adaptive, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Rehydrate a persisted metadata-only plan, then run the bounded adaptive loop. */
  async executePlannedAdaptiveCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlannedAdaptiveCycle requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain adaptive cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain adaptive cycle plan does not match the transient request");
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Replay a reviewed adaptive plan only after provider-free launch admission. */
  async executePlannedAdaptiveCycleWithLaunchAdmission(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAdaptiveCycleOptions,
  ): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted planned adaptive cycle requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlannedAdaptiveCycle(plan, input, options);
  }

  /** Execute an evaluator-guided loop while tracing every bounded attempt and learning transition. */
  async executeAdaptiveCycleWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleTraceOptions): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeAdaptiveCycleWithTrace options must be an object");
    const prepared = await this.prepare(request, options.semanticRouting, options.adaptive, options.approveProviderCall);
    return this.executeAdaptiveCyclePreparedWithTrace(prepared, options);
  }

  /** Adaptive evaluator/replan execution with provider-free launch admission and a metadata trace. */
  async executeAdaptiveCycleWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeAdaptiveCycleWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted adaptive tracing requires provider-free routing; admit semantic routing separately before enabling it");
    const request = validateRequest(input);
    const prepared = await this.prepare(request, options.semanticRouting, options.adaptive, options.approveProviderCall);
    authorizeAutonomousLaunchDomains(admission, prepared.route.selected_domains);
    return this.executeAdaptiveCyclePreparedWithTrace(prepared, options);
  }

  /** Rehydrate a reviewed plan, then execute its evaluator-guided loop through the trace boundary. */
  async executePlannedAdaptiveCycleWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleTraceOptions): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    const reviewedPlan = this.revalidateBrainPlan(plan, "autonomous brain executePlannedAdaptiveCycleWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(reviewedPlan, request, "autonomous brain traced adaptive cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, reviewedPlan.route, reviewedPlan.semantic_route);
    if (prepared.plan.plan_digest !== reviewedPlan.plan_digest) throw new ArgumentError("autonomous brain traced adaptive cycle plan does not match the transient request");
    return this.executeAdaptiveCyclePreparedWithTrace(prepared, options);
  }

  /** Traced adaptive plan replay with provider-free launch admission before dispatch. */
  async executePlannedAdaptiveCycleWithLaunchAdmissionAndTrace(
    plan: AutonomousBrainPlan,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAdaptiveCycleTraceOptions,
  ): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executePlannedAdaptiveCycleWithLaunchAdmissionAndTrace options must be an object");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted traced planned adaptive cycle requires provider-free routing; reuse the persisted route or admit semantic routing separately");
    await this.authorizePlannedLaunchAdmission(plan, input, admission);
    return this.executePlannedAdaptiveCycleWithTrace(plan, input, options);
  }

  /** Return the redacted provider/model/tool posture needed to render onboarding UI. */
  async readiness(options: AutonomousBrainReadinessOptions = {}): Promise<AutonomousBrainReadinessReport> {
    return this.agent.readiness(options);
  }

  /** Restore the agent's caller-owned, digest-bound episodic memory before execution. */
  async restoreMemory(): Promise<AutonomousMemorySnapshot | null> {
    return this.agent.restoreMemory();
  }

  /** Flush the agent's value-only episodic memory through its CAS-fenced persistence boundary. */
  async flushMemory(): Promise<AutonomousMemorySnapshot> {
    return this.agent.flushMemory();
  }

  /** Restore the agent's caller-owned provider/model health prior before execution. */
  async restoreHealth(): Promise<AutonomousModelHealthSnapshot | null> {
    return this.agent.restoreHealth();
  }

  /** Flush the agent's aggregate provider/model health through its CAS-fenced boundary. */
  async flushHealth(): Promise<AutonomousModelHealthSnapshot> {
    return this.agent.flushHealth();
  }

  /**
   * Audit every reviewed domain contract and an optional caller-owned live surface.
   * This is intentionally keyless and side-effect free; it never invokes a provider,
   * acquires evidence, executes a tool, or treats registration as authorization.
   */
  async domainAudit(options: AutonomousDomainAuditOptions = {}): Promise<AutonomousDomainAuditReport> {
    return auditAutonomousDomainContracts(options);
  }

  /**
   * Return one digest-bound, provider-free operating contract for a reviewed domain.
   * This is a consistency/readiness projection and never authorizes dispatch.
   */
  async domainOperatingKit(domain: AutonomousDomainName): Promise<AutonomousDomainOperatingKit> {
    return buildAutonomousDomainOperatingKit(domain);
  }

  /** Return operating contracts for all requested domains in deterministic order. */
  async domainOperatingKits(domains?: readonly AutonomousDomainName[]): Promise<readonly AutonomousDomainOperatingKit[]> {
    return buildAutonomousDomainOperatingKits(domains);
  }

  /** Rebuild and validate a caller-held operating contract against current reviewed metadata. */
  async validateDomainOperatingKit(value: unknown): Promise<AutonomousDomainOperatingKit> {
    return validateAutonomousDomainOperatingKit(value);
  }

  /**
   * Compose every provider-free launch gate into one digest-bound, review-only handoff.
   * The projection covers all twelve domains and cannot authorize provider, source, tool,
   * credential, learner, queue, or effect dispatch.
   */
  async launchPreflight(options: AutonomousLaunchPreflightOptions = {}): Promise<AutonomousLaunchPreflightReport> {
    return auditAutonomousBrainLaunchPreflight(this, options);
  }

  /** Bind an explicit caller decision to one exact preflight without granting execution authority. */
  admitLaunchPreflight(preflight: AutonomousLaunchPreflightReport, options: AutonomousLaunchAdmissionOptions): AutonomousLaunchAdmissionReport {
    return createAutonomousLaunchAdmission(preflight, options);
  }

  /**
   * Compile the next bounded context/evidence acquisitions for the reviewed route.
   *
   * Routing and scoring are provider-free. The returned plan carries only candidate metadata,
   * digests, budgets, dependency order, and explicit omissions; source dispatch, credentials,
   * provider invocation, evaluation, and learning remain later caller-owned gates.
   */
  async planInformationAcquisition(
    task: string,
    options: AutonomousBrainInformationAcquisitionOptions,
  ): Promise<AutonomousBrainInformationAcquisitionPlan> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain information acquisition options must be an object");
    return this.agent.planInformationAcquisition(task, options);
  }

  /** Replan acquisition after caller-owned value-only observations, preserving the prior digest fence. */
  replanInformationAcquisition(
    options: AutonomousBrainInformationAcquisitionReplanOptions,
  ): AutonomousBrainInformationAcquisitionPlan {
    if (!isObject(options)) throw new ArgumentError("autonomous brain information acquisition replan options must be an object");
    return replanAutonomousInformationAcquisition(options);
  }

  /** Validate a caller-rehydrated acquisition plan before it is handed to a source worker. */
  validateInformationAcquisitionPlan(
    value: AutonomousBrainInformationAcquisitionPlan,
  ): AutonomousBrainInformationAcquisitionPlan {
    return validateAutonomousInformationAcquisitionPlan(value);
  }

  /** Assess caller-supplied evidence metadata before any downstream claim can rely on it. */
  assessClaimIntegrity(
    task: string,
    options: AutonomousBrainClaimIntegrityAssessmentOptions,
  ): AutonomousBrainClaimIntegrityAssessment {
    if (!isObject(options)) throw new ArgumentError("autonomous brain claim integrity options must be an object");
    return this.agent.assessClaimIntegrity(task, options);
  }

  /** Continue claim assessment after transient evidence changes with an explicit generation fence. */
  reassessClaimIntegrity(
    previous: AutonomousBrainClaimIntegrityAssessment,
    options: AutonomousBrainClaimIntegrityReassessmentOptions,
  ): AutonomousBrainClaimIntegrityAssessment {
    if (!isObject(options)) throw new ArgumentError("autonomous brain claim integrity reassessment options must be an object");
    return this.agent.reassessClaimIntegrity(previous, options);
  }

  /** Validate claim-integrity metadata before resuming a worker or creating an acquisition bridge. */
  validateClaimIntegrity(
    value: AutonomousBrainClaimIntegrityAssessment,
  ): AutonomousBrainClaimIntegrityAssessment {
    return validateAutonomousClaimIntegrity(value);
  }

  /** Translate unresolved claim actions into a reviewed, provider-free acquisition bridge. */
  planClaimIntegrityAcquisition(
    assessment: AutonomousBrainClaimIntegrityAssessment,
    options: AutonomousBrainClaimIntegrityAcquisitionPlanOptions,
  ): AutonomousBrainClaimIntegrityAcquisitionBridge {
    if (!isObject(options)) throw new ArgumentError("autonomous brain claim integrity acquisition options must be an object");
    return this.agent.planClaimIntegrityAcquisition(assessment, options);
  }

  /** Validate an acquisition bridge before allowing its selected domains into a source boundary. */
  validateClaimIntegrityAcquisitionBridge(
    value: AutonomousBrainClaimIntegrityAcquisitionBridge,
  ): AutonomousBrainClaimIntegrityAcquisitionBridge {
    return validateAutonomousClaimIntegrityAcquisitionBridge(value);
  }

  /** Bind caller-owned source requests to the exact candidates selected by an integrity bridge. */
  bindClaimIntegrityAcquisition(
    bridge: AutonomousBrainClaimIntegrityAcquisitionBridge,
    requests: Parameters<AutonomousAgent["bindClaimIntegrityAcquisition"]>[1],
  ): AutonomousBrainClaimIntegrityAcquisitionBinding {
    return this.agent.bindClaimIntegrityAcquisition(bridge, requests);
  }

  /** Validate a caller-rehydrated integrity binding before source dispatch. */
  validateClaimIntegrityAcquisitionBinding(
    value: AutonomousBrainClaimIntegrityAcquisitionBinding,
  ): AutonomousBrainClaimIntegrityAcquisitionBinding {
    return validateAutonomousClaimIntegrityAcquisitionBinding(value);
  }

  /** Execute only the integrity-selected evidence queue through the reviewed source boundary. */
  async executeClaimIntegrityAcquisition(
    bridge: AutonomousBrainClaimIntegrityAcquisitionBridge,
    registry: Parameters<AutonomousAgent["executeClaimIntegrityAcquisition"]>[1],
    requests: Parameters<AutonomousAgent["executeClaimIntegrityAcquisition"]>[2],
    options: AutonomousBrainClaimIntegrityAcquisitionExecutionOptions = {},
  ): Promise<AutonomousBrainClaimIntegrityAcquisitionExecutionResult> {
    return this.agent.executeClaimIntegrityAcquisition(bridge, registry, requests, options);
  }

  /** Execute the integrity-selected queue only after every selected domain passes launch admission. */
  async executeClaimIntegrityAcquisitionWithLaunchAdmission(
    bridge: AutonomousBrainClaimIntegrityAcquisitionBridge,
    registry: Parameters<AutonomousAgent["executeClaimIntegrityAcquisition"]>[1],
    requests: Parameters<AutonomousAgent["executeClaimIntegrityAcquisition"]>[2],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainClaimIntegrityAcquisitionExecutionOptions = {},
  ): Promise<AutonomousBrainClaimIntegrityAcquisitionExecutionResult> {
    authorizeAutonomousLaunchDomains(admission, claimIntegrityAcquisitionDomains(bridge));
    return this.executeClaimIntegrityAcquisition(bridge, registry, requests, options);
  }

  /** Resume the integrity-selected queue through its caller-owned, digest-fenced checkpoint. */
  async executeClaimIntegrityAcquisitionResumable(
    bridge: AutonomousBrainClaimIntegrityAcquisitionBridge,
    registry: Parameters<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>[1],
    requests: Parameters<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>[2],
    options: AutonomousBrainClaimIntegrityAcquisitionResumableOptions,
  ): Promise<AutonomousBrainClaimIntegrityAcquisitionResumableResult> {
    return this.agent.executeClaimIntegrityAcquisitionResumable(bridge, registry, requests, options);
  }

  /** Resume integrity acquisition only after checkpointed domains pass launch admission. */
  async executeClaimIntegrityAcquisitionResumableWithLaunchAdmission(
    bridge: AutonomousBrainClaimIntegrityAcquisitionBridge,
    registry: Parameters<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>[1],
    requests: Parameters<AutonomousAgent["executeClaimIntegrityAcquisitionResumable"]>[2],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainClaimIntegrityAcquisitionResumableOptions,
  ): Promise<AutonomousBrainClaimIntegrityAcquisitionResumableResult> {
    authorizeAutonomousLaunchDomains(admission, claimIntegrityAcquisitionDomains(bridge));
    return this.executeClaimIntegrityAcquisitionResumable(bridge, registry, requests, options);
  }

  /** Project a completed direct or cross-domain result into a metadata-only reliance identity. */
  projectOutcomeIntegrityRun(
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
  ): AutonomousBrainOutcomeIntegrityRun {
    return projectAutonomousOutcomeIntegrityRun(result);
  }

  /** Bind caller claims to the exact output and response digests of one autonomous result. */
  bindOutcomeIntegrityClaims(
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
    bindings: readonly (AutonomousBrainOutcomeIntegrityClaimBindingInput | Record<string, unknown>)[],
  ): AutonomousBrainOutcomeIntegrityClaimBinding[] {
    return bindAutonomousOutcomeIntegrityClaims(projectAutonomousOutcomeIntegrityRun(result), bindings);
  }

  /** Decide whether caller-supplied claims may rely on one exact autonomous outcome. */
  assessOutcomeIntegrity(
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
    options: AutonomousBrainOutcomeIntegrityAssessmentOptions,
  ): AutonomousBrainOutcomeIntegrityAssessment {
    return assessAutonomousOutcomeIntegrity({
      run: projectAutonomousOutcomeIntegrityRun(result),
      ...options,
    });
  }

  /** Validate an outcome assessment after caller-owned persistence or transport rehydration. */
  validateOutcomeIntegrity(value: unknown): AutonomousBrainOutcomeIntegrityAssessment {
    return validateAutonomousOutcomeIntegrity(value as AutonomousBrainOutcomeIntegrityAssessment);
  }

  /** Validate the same outcome projection through its public snapshot contract. */
  validateOutcomeIntegritySnapshot(value: unknown): AutonomousBrainOutcomeIntegrityAssessment {
    return validateAutonomousOutcomeIntegritySnapshot(value);
  }

  /** Gate specialist and synthesis responses before cross-domain reliance or synthesis. */
  assessCrossDomainResponses(
    responses: readonly AutonomousBrainCrossDomainResponseEntry[],
    options: AutonomousBrainCrossDomainResponseAssessmentOptions = {},
  ): AutonomousBrainCrossDomainResponseAssessment {
    if (!isObject(options)) throw new ArgumentError("autonomous brain cross-domain response options must be an object");
    return this.agent.assessCrossDomainResponses(responses, options);
  }

  /** Validate a caller-rehydrated cross-domain response assessment before synthesis. */
  validateCrossDomainResponseAssessment(value: unknown): AutonomousBrainCrossDomainResponseAssessment {
    return validateAutonomousCrossDomainResponseAssessment(value);
  }

  /** Recompute the cross-domain response gate and refuse drift from the persisted assessment. */
  replayCrossDomainResponseAssessment(
    responses: readonly AutonomousBrainCrossDomainResponseEntry[],
    expected: AutonomousBrainCrossDomainResponseAssessment,
    options: AutonomousBrainCrossDomainResponseReplayOptions = {},
  ): AutonomousBrainCrossDomainResponseAssessment {
    return replayAutonomousCrossDomainResponseAssessment(responses, expected, options);
  }

  /** Execute already-bound tool calls through the agent's activation, approval, and effect boundary. */
  async executeToolCalls(
    calls: readonly AutonomousBrainToolCall[],
    options: AutonomousBrainToolCallOptions,
  ): Promise<AutonomousBrainToolResult[]> {
    return this.agent.executeToolCalls(calls, options);
  }

  /** Execute tool calls only after every declared domain passes the provider-free launch gate. */
  async executeToolCallsWithLaunchAdmission(
    calls: readonly AutonomousBrainToolCall[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainToolCallOptions,
  ): Promise<AutonomousBrainToolResult[]> {
    authorizeAutonomousLaunchDomains(admission, toolCallDomains(options.domains));
    return this.executeToolCalls(calls, options);
  }

  /** Execute one reviewed capability with a transient value and metadata-only replay record. */
  async executeCapability(
    request: AutonomousBrainCapabilityExecutionRequest,
    options: AutonomousBrainCapabilityExecutionOptions = {},
  ): Promise<AutonomousBrainCapabilityExecutionResult> {
    return this.agent.executeCapability(request, options);
  }

  /** Execute a capability only after its workflow-context domain passes launch admission. */
  async executeCapabilityWithLaunchAdmission(
    request: AutonomousBrainCapabilityExecutionRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCapabilityExecutionOptions = {},
  ): Promise<AutonomousBrainCapabilityExecutionResult> {
    authorizeAutonomousLaunchDomains(admission, capabilityRequestDomains([request]));
    return this.executeCapability(request, options);
  }

  /** Execute an ordered capability batch while preserving explicit failure omissions. */
  async executeCapabilityBatch(
    requests: readonly AutonomousBrainCapabilityExecutionRequest[],
    options: AutonomousBrainCapabilityBatchOptions = {},
  ): Promise<AutonomousBrainCapabilityBatchResult> {
    return this.agent.executeCapabilityBatch(requests, options);
  }

  /** Execute a capability batch only after its complete domain scope passes launch admission. */
  async executeCapabilityBatchWithLaunchAdmission(
    requests: readonly AutonomousBrainCapabilityExecutionRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCapabilityBatchOptions = {},
  ): Promise<AutonomousBrainCapabilityBatchResult> {
    authorizeAutonomousLaunchDomains(admission, capabilityRequestDomains(requests));
    return this.executeCapabilityBatch(requests, options);
  }

  /** Return metadata-only capability records; raw arguments and adapter values stay transient. */
  capabilityExecutionEvidence(): AutonomousBrainCapabilityExecutionRecord[] {
    return this.agent.capabilityExecutionEvidence();
  }

  /** Restore capability replay metadata through the agent's caller-owned journal. */
  async restoreCapabilityJournal(): Promise<Awaited<ReturnType<AutonomousAgent["restoreCapabilityJournal"]>>> {
    return this.agent.restoreCapabilityJournal();
  }

  /** Restore durable capability metadata and reopen the local replay barrier. */
  async restoreCapabilityJournalPersistence(): Promise<Record<string, unknown>> {
    return this.agent.restoreCapabilityJournalPersistence();
  }

  /** Flush capability replay metadata without returning journal entries or raw values. */
  async flushCapabilityJournalPersistence(): Promise<Record<string, unknown>> {
    return this.agent.flushCapabilityJournalPersistence();
  }

  /** Evaluate one capability result using caller-declared evidence and settle its model/tool arm. */
  async evaluateCapabilityExecution(
    result: Parameters<AutonomousAgent["evaluateCapabilityExecution"]>[0],
    options: AutonomousBrainCapabilityLearningOptions,
  ): Promise<AutonomousBrainCapabilityLearningResult> {
    return this.agent.evaluateCapabilityExecution(result, options);
  }

  /** Evaluate a capability batch in input order and settle explicit value-only learning credit. */
  async evaluateCapabilityExecutions(
    results: Parameters<AutonomousAgent["evaluateCapabilityExecutions"]>[0],
    options: AutonomousBrainCapabilityLearningBatchOptions,
  ): Promise<AutonomousBrainCapabilityLearningBatchResult> {
    return this.agent.evaluateCapabilityExecutions(results, options);
  }

  /** Return metadata-only tool receipts from the activated domain-tool runtime. */
  toolExecutionEvidence(): AutonomousBrainToolExecutionReceipt[] {
    return this.agent.toolExecutionEvidence();
  }

  /** Evaluate tool receipts through an explicit evaluator before updating adaptive tool selection. */
  async evaluateToolReceipts(
    options: AutonomousBrainToolLearningOptions,
  ): Promise<AutonomousBrainToolLearningResult> {
    return this.agent.evaluateToolReceipts(options);
  }

  /** Evaluate provider invocation receipts without inferring task quality from transport success. */
  async evaluateProviderReceipts(
    options: AutonomousBrainProviderLearningOptions,
  ): Promise<AutonomousBrainProviderLearningResult> {
    return this.agent.evaluateProviderReceipts(options);
  }

  /**
   * Restore every configured metadata coordinator through the facade's exact agent.
   *
   * The lifecycle remains explicit and caller-owned: stores, CAS behavior, and the choice of
   * strictness stay in the embedding application. The returned report is metadata-only and is
   * safe to use as a startup audit; it never contains tasks, prompts, credentials, provider
   * payloads, tool values, or evidence contents.
   */
  async restorePersistedState(
    options: AutonomousBrainPersistenceLifecycleRestoreOptions = {},
  ): Promise<AutonomousBrainPersistenceLifecycleReport> {
    return this.agent.restorePersistedState(options);
  }

  /** Flush every configured metadata coordinator in the reviewed reverse dependency order. */
  async flushPersistedState(
    options: AutonomousBrainPersistenceLifecycleFlushOptions = {},
  ): Promise<AutonomousBrainPersistenceLifecycleReport> {
    return this.agent.flushPersistedState(options);
  }

  /** Project a portfolio-wide admission image before provider/tool/source dispatch. */
  async admitWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioAdmissionOptions = {},
  ): Promise<AutonomousWorkflowPortfolioAdmission> {
    return this.agent.admitWorkflowPortfolio(requests, options);
  }

  /** Compile a dependency-aware portfolio of reviewed domain workflows without dispatch. */
  async planWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousBrainWorkflowPortfolioPlanOptions = {},
  ): Promise<AutonomousBrainWorkflowPortfolioPlan> {
    return this.agent.planWorkflowPortfolio(requests, options);
  }

  /** Replay a caller-rehydrated workflow portfolio and report exact digest mismatches. */
  async verifyWorkflowPortfolio(
    plan: AutonomousBrainWorkflowPortfolioPlan,
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousBrainWorkflowPortfolioPlanOptions = {},
  ): Promise<AutonomousBrainWorkflowPortfolioVerification> {
    return this.agent.verifyWorkflowPortfolio(plan, requests, options);
  }

  /** Execute a reviewed workflow portfolio in bounded dependency waves. */
  async executeWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousBrainWorkflowPortfolioExecutionOptions = {},
  ): Promise<AutonomousBrainWorkflowPortfolioExecutionResult> {
    return this.agent.executeWorkflowPortfolio(requests, options);
  }

  /** Execute a workflow portfolio only after every planned domain passes launch admission. */
  async executeWorkflowPortfolioWithLaunchAdmission(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowPortfolioExecutionOptions = {},
  ): Promise<AutonomousBrainWorkflowPortfolioExecutionResult> {
    const plan = options.plan ?? await this.agent.planWorkflowPortfolio(requests, options.planOptions);
    const verification = await this.agent.verifyWorkflowPortfolio(plan, requests, options.planOptions);
    if (verification.status !== "verified") throw new ProviderRuntimeError("autonomous brain workflow portfolio plan verification failed before launch admission", { code: "protocol", retryable: false, operation: "workflow_portfolio_verify" });
    authorizeAutonomousLaunchDomains(admission, workflowPortfolioDomains(plan));
    return this.agent.executeWorkflowPortfolio(requests, { ...options, plan });
  }

  /** Resume a workflow portfolio after caller-owned item rehydration and digest validation. */
  async executeWorkflowPortfolioResumable(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousBrainWorkflowPortfolioResumableExecutionOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioExecutionResult> {
    return this.agent.executeWorkflowPortfolioResumable(requests, options);
  }

  /** Resume a workflow portfolio only after every checkpointed domain passes launch admission. */
  async executeWorkflowPortfolioResumableWithLaunchAdmission(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowPortfolioResumableExecutionOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain resumable workflow portfolio options must be an object");
    const plan = options.plan ?? await this.agent.planWorkflowPortfolio(requests, options.planOptions);
    const verification = await this.agent.verifyWorkflowPortfolio(plan, requests, options.planOptions);
    if (verification.status !== "verified") throw new ProviderRuntimeError("autonomous brain resumable workflow portfolio plan verification failed before launch admission", { code: "protocol", retryable: false, operation: "workflow_portfolio_verify" });
    authorizeAutonomousLaunchDomains(admission, workflowPortfolioDomains(plan));
    return this.agent.executeWorkflowPortfolioResumable(requests, { ...options, plan });
  }

  /** Supervise caller-owned evidence for a completed workflow portfolio without provider replay. */
  async executeWorkflowPortfolioEvidence(
    execution: AutonomousBrainWorkflowPortfolioExecutionResult,
    options: AutonomousBrainWorkflowPortfolioEvidenceSupervisorOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioEvidenceExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow portfolio evidence options must be an object");
    return this.agent.executeWorkflowPortfolioEvidence(execution, options);
  }

  /** Supervise portfolio evidence only after every execution domain passes launch admission. */
  async executeWorkflowPortfolioEvidenceWithLaunchAdmission(
    execution: AutonomousBrainWorkflowPortfolioExecutionResult,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowPortfolioEvidenceSupervisorOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioEvidenceExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain workflow portfolio evidence options must be an object");
    authorizeAutonomousLaunchDomains(admission, workflowPortfolioExecutionDomains(execution));
    return this.agent.executeWorkflowPortfolioEvidence(execution, options);
  }

  /** Resume portfolio evidence through caller-owned checkpoints and journal rehydration. */
  async executeWorkflowPortfolioEvidenceResumable(
    execution: AutonomousBrainWorkflowPortfolioExecutionResult,
    options: AutonomousBrainWorkflowPortfolioEvidenceResumableExecutionOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioEvidenceExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain resumable workflow portfolio evidence options must be an object");
    return this.agent.executeWorkflowPortfolioEvidenceResumable(execution, options);
  }

  /** Resume portfolio evidence only after every execution domain passes launch admission. */
  async executeWorkflowPortfolioEvidenceResumableWithLaunchAdmission(
    execution: AutonomousBrainWorkflowPortfolioExecutionResult,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainWorkflowPortfolioEvidenceResumableExecutionOptions,
  ): Promise<AutonomousBrainWorkflowPortfolioEvidenceExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("autonomous brain resumable workflow portfolio evidence options must be an object");
    authorizeAutonomousLaunchDomains(admission, workflowPortfolioExecutionDomains(execution));
    return this.agent.executeWorkflowPortfolioEvidenceResumable(execution, options);
  }

  /**
   * Preview the exact domain-scoped model ranking without dispatching a provider or domain tool.
   * An explicit domain is required so a UI cannot mistake lexical routing for model eligibility.
   */
  async modelSelectionPreview(
    input: AutonomousBrainRequest,
    options: Omit<AutonomousModelSelectionPreviewOptions, "domain"> = {},
  ): Promise<AutonomousModelSelectionPreview> {
    const request = validateRequest(input);
    if (request.domain === undefined) throw new ArgumentError("model selection preview requires an explicit domain");
    if (request.connector !== undefined) throw new ArgumentError("model selection preview does not accept connector dispatch inputs");
    return this.agent.modelSelectionPreview(request.task, {
      ...options,
      domain: request.domain,
      capability: options.capability ?? request.capability,
      context: options.context ?? request.context,
    });
  }

  /**
   * Revalidate and execute one previously reviewed model-selection preview.
   *
   * The agent recomputes the selection against current health and catalogue state. A stale
   * ranking refuses before provider dispatch, and the final invocation is narrowed to the exact
   * approved candidate with failover disabled.
   */
  async executeApprovedSelection(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousBrainApprovedSelectionOptions = {},
  ): Promise<AutonomousBrainExecution> {
    const request = validateRequest(input);
    if (request.domain === undefined) throw new ArgumentError("approved model selection requires an explicit domain");
    if (request.connector !== undefined) throw new ArgumentError("approved model selection does not accept connector dispatch inputs");
    const prepared = await this.prepare(request);
    if (prepared.plan.status !== "ready" || prepared.route.cross_domain) throw new ProviderRuntimeError("approved model selection requires a ready single-domain plan");
    return this.executeApprovedSelectionPrepared(prepared, preview, options);
  }

  /**
   * Revalidate and execute an approved model arm only after its explicit domain passes launch
   * admission. Planning remains provider-free; the admission check is the final facade gate
   * before the exact provider invocation.
   */
  async executeApprovedSelectionWithLaunchAdmission(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainApprovedSelectionOptions = {},
  ): Promise<AutonomousBrainExecution> {
    const request = validateRequest(input);
    if (request.domain === undefined) throw new ArgumentError("approved model selection requires an explicit domain");
    if (request.connector !== undefined) throw new ArgumentError("approved model selection does not accept connector dispatch inputs");
    const prepared = await this.prepare(request);
    if (prepared.plan.status !== "ready" || prepared.route.cross_domain) throw new ProviderRuntimeError("approved model selection requires a ready single-domain plan");
    authorizeAutonomousLaunchDomains(admission, [request.domain]);
    return this.executeApprovedSelectionPrepared(prepared, preview, options);
  }

  /** Execute one reviewed model arm while recording selection and provider metadata only. */
  async executeApprovedSelectionWithTrace(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousBrainApprovedSelectionTraceOptions,
  ): Promise<AutonomousBrainTracedApprovedSelection> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain approved selection trace options must be an object");
    if (request.domain === undefined) throw new ArgumentError("approved model selection requires an explicit domain");
    if (request.connector !== undefined) throw new ArgumentError("approved model selection does not accept connector dispatch inputs");
    const prepared = await this.prepare(request);
    if (prepared.plan.status !== "ready" || prepared.route.cross_domain) throw new ProviderRuntimeError("approved model selection requires a ready single-domain plan");
    return this.executeApprovedSelectionPreparedWithTrace(prepared, preview, options);
  }

  /** Execute one reviewed model arm after launch admission, with a digest-only lifecycle trace. */
  async executeApprovedSelectionWithLaunchAdmissionAndTrace(
    input: AutonomousBrainRequest,
    preview: AutonomousModelSelectionPreview,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainApprovedSelectionTraceOptions,
  ): Promise<AutonomousBrainTracedApprovedSelection> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain approved selection trace options must be an object");
    const request = validateRequest(input);
    if (request.domain === undefined) throw new ArgumentError("approved model selection requires an explicit domain");
    if (request.connector !== undefined) throw new ArgumentError("approved model selection does not accept connector dispatch inputs");
    const prepared = await this.prepare(request);
    if (prepared.plan.status !== "ready" || prepared.route.cross_domain) throw new ProviderRuntimeError("approved model selection requires a ready single-domain plan");
    authorizeAutonomousLaunchDomains(admission, [request.domain]);
    return this.executeApprovedSelectionPreparedWithTrace(prepared, preview, options);
  }

  /** Recompute keyless readiness and activation metadata without dispatching a provider or tool. */
  async refreshActivation(options: AutonomousBrainReadinessOptions = {}): Promise<AutonomousBrainActivationState> {
    return this.agent.refreshActivation(options);
  }

  /** Return the current redacted activation state; this does not itself grant authority. */
  activationState(): AutonomousBrainActivationState {
    return this.agent.activationState();
  }

  /** Approve only the caller-selected read-only bindings from a digest-bound domain tool plan. */
  approveActivationBindings(plan: AutonomousDomainToolPlan, approvedTools: readonly string[], registeredToolCount?: number): AutonomousBrainActivationState {
    return this.agent.approveActivationBindings(plan, approvedTools, registeredToolCount);
  }

  /** Persist activation metadata through a caller-owned store; credentials remain outside it. */
  async saveActivation(store: AutonomousBrainActivationSnapshotStore): Promise<AutonomousBrainActivationState> {
    return this.agent.saveActivation(store);
  }

  /** Restore activation metadata through a caller-owned store; null means no prior state. */
  async restoreActivation(store: AutonomousBrainActivationSnapshotStore): Promise<AutonomousBrainActivationState | null> {
    return this.agent.restoreActivation(store);
  }

  /** Revoke activation and close the tool admission path until a new review is completed. */
  revokeActivation(reason?: string): AutonomousBrainActivationState {
    return this.agent.revokeActivation(reason);
  }

  /** Execute automatic route-to-invocation work with bounded concurrency and deterministic order. */
  async executeAutoBatch(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainAutoBatchOptions = {}): Promise<AutonomousBrainAutoBatchResult> {
    return this.executeAutoBatchCore(inputs, options);
  }

  /**
   * Execute an automatic batch while appending one metadata-only hash chain for the entire
   * batch. The trace is an observability boundary: it records item planning, connector/model/
   * provider phases emitted by the nested execution, item terminal states, and the aggregate
   * result, but never receives task text, prompts, credentials, provider output, or tool data.
   */
  async executeAutoBatchWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic traced batch options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticBatchTraceTaskDigest(normalizedInputs),
      // A batch may route to any reviewed domain. Item events carry the concrete plan digests;
      // the envelope advertises the complete reviewed vocabulary so a cross-domain dashboard
      // can safely use one stable trace contract before routing has settled.
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const batch = await this.executeAutoBatchCore(normalizedInputs, batchOptions, trace);
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({
          batch_digest: batch.batch_digest,
          completed_count: batch.completed_count,
          failed_count: batch.failed_count,
          omitted_count: batch.omitted_count,
        }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_BATCH_SCHEMA,
        batch: tracedAutoBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Run the traced automatic batch only after one provider-free admission covers its routes. */
  async executeAutoBatchWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic traced batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.execution, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic traced batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutoBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoBatchWithTrace(inputs, { ...options, execution: (_input, index) => policies[index]! });
  }

  private async executeAutoBatchCore(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoBatchOptions,
    trace?: AutonomousRunTraceSession,
  ): Promise<AutonomousBrainAutoBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const items: Array<AutonomousBrainAutoBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          if (trace !== undefined) await trace.record({ phase: "paused", status: "paused", detail_digest: digestJsonSync({ index, state: "omitted" }) });
          continue;
        }
        try {
          const policy = batchOption(options.execution, inputs[index]!, index) ?? {};
          let execution: AutonomousBrainAutoExecution;
          let prepared: PreparedBrainRequest | undefined;
          if (trace === undefined) {
            execution = await this.executeAuto(inputs[index]!, policy);
          } else {
            prepared = await this.prepare(inputs[index]!, selectBrainSemanticRouting(policy.semanticRouting, undefined), policy, policy.approveProviderCall);
            await trace.record({
              phase: "plan_compiled",
              status: "running",
              domains: this.traceDomains(prepared),
              route_digest: prepared.route.route_digest,
              plan_digest: prepared.plan.plan_digest,
              detail_digest: digestJsonSync({ index, state: "prepared" }),
            });
            execution = await this.executeAutoPrepared(prepared, policy, trace);
            const traceStatus = autonomousRunTraceStatus(execution.status);
            const phase = traceStatus === "completed" || traceStatus === "partial" ? "completed" : traceStatus === "paused" ? "paused" : traceStatus === "refused" ? "refused" : "failed";
            await trace.record({
              phase,
              status: traceStatus,
              domains: this.traceDomains(prepared),
              route_digest: prepared.route.route_digest,
              plan_digest: prepared.plan.plan_digest,
              detail_digest: digestJsonSync({ index, state: execution.status }),
            });
          }
          const succeeded = execution.status === "completed";
          const refused = automaticBatchRefused(execution.status);
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          if (trace !== undefined) await trace.record({ phase: "failed", status: "failed", detail_digest: digestJsonSync({ index, ...projection }) }).catch(() => undefined);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_AUTO_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: batchDigest(normalized),
      retention: "metadata_only_tasks_and_automatic_connector_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Execute automatic batches only after every frozen route is covered by launch admission. */
  async executeAutoBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoBatchOptions = {},
  ): Promise<AutonomousBrainAutoBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.execution, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutoBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoBatch(inputs, { ...options, execution: (_input, index) => policies[index]! });
  }

  /** Execute independent brain requests with bounded concurrency and deterministic result order. */
  async executeBatch(inputs: readonly AutonomousBrainRequest[], options: { maxParallelism?: number; stopOnError?: boolean; execution?: AutonomousBrainExecuteOptions } = {}): Promise<AutonomousBrainBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const maxParallelism = options.maxParallelism ?? 4;
    if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > MAX_AUTONOMOUS_BRAIN_PARALLELISM) throw new ArgumentError("autonomous brain batch maxParallelism is outside its bound");
    const stopOnError = options.stopOnError ?? false;
    if (typeof stopOnError !== "boolean") throw new ArgumentError("autonomous brain batch stopOnError must be boolean");
    const items: Array<AutonomousBrainBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const execution = await this.execute(inputs[index]!, options.execution);
          const succeeded = execution.status === "completed";
          const refused = execution.status === "approval_required" || execution.status === "route_review_required" || execution.status === "connector_blocked";
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_BATCH_SCHEMA,
      status: failed === 0 && omitted === 0 ? "completed" : completed > 0 ? "partial" : "failed",
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: digestJsonSync(normalized.map((item) => ({ index: item.index, status: item.status, task_digest: item.task_digest, error_class: item.error_class ?? null, failure_code: item.failure_code ?? null, plan_digest: item.execution?.plan.plan_digest ?? null, run_status: item.execution?.status ?? null }))),
      retention: "metadata_only_tasks_and_provider_connector_values_transient",
      secret_material: "never_returned",
    };
  }

  /**
   * Execute a batch only after a provider-free preview proves that one launch admission covers
   * every selected route.  The preview happens before the ordinary batch engine can touch a
   * credential, connector, provider, or resumable result.
   */
  async executeBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: { maxParallelism?: number; stopOnError?: boolean; execution?: AutonomousBrainExecuteOptions } = {},
  ): Promise<AutonomousBrainBatchResult> {
    this.rejectLaunchAdmittedSemanticRouting(options.execution?.semanticRouting, "launch-admitted batch execution requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeBatchLaunchAdmission(inputs, admission);
    return this.executeBatch(inputs, options);
  }

  /**
   * Execute the ordinary batch with metadata-only restart checkpoints.
   *
   * Completed items are never trusted merely because they appear in a checkpoint: the caller's
   * rehydrator must return each transient execution and the facade verifies its task and redacted
   * outcome digest before dispatching any new item. Checkpoint sinks are caller-owned and should
   * use an atomic write; task text, prompts, credentials, provider responses, and connector
   * observations are intentionally absent from every checkpoint.
   */
  async executeBatchResumable(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainResumableBatchOptions): Promise<AutonomousBrainBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.jobId === undefined) throw new ArgumentError("autonomous brain resumable batch requires jobId");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const jobId = checkpointText("autonomous brain batch jobId", options.jobId);
    if (options.checkpointSink !== undefined && typeof options.checkpointSink !== "function") throw new ArgumentError("autonomous brain batch checkpointSink must be callable");
    if (options.rehydrateExecution !== undefined && typeof options.rehydrateExecution !== "function") throw new ArgumentError("autonomous brain batch rehydrateExecution must be callable");
    const taskDigests = normalizedInputs.map((input) => brainBatchTaskDigest(input));
    const requestDigests = normalizedInputs.map((input, index) => brainBatchRequestDigest(input, index));
    const semanticRoutingPolicyDigest = brainSemanticRoutingPolicyDigest(options.execution);
    const batchInputDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, mode: "brain", request_digests: requestDigests, ...(semanticRoutingPolicyDigest === null ? {} : { semantic_routing_policy_digest: semanticRoutingPolicyDigest }) });
    const restored = options.checkpoint === undefined ? null : validateBrainBatchCheckpoint(options.checkpoint);
    if (restored !== null) {
      if (restored.mode !== "brain" || restored.job_id !== jobId || JSON.stringify(restored.request_digests) !== JSON.stringify(requestDigests)) throw new ArgumentError("autonomous brain batch checkpoint does not match the current requests or mode");
      if (semanticRoutingPolicyDigest !== null && restored.semantic_routing_policy_digest === undefined) throw new ArgumentError("legacy autonomous brain batch checkpoint requires explicit semantic-routing policy rebinding");
      if ((restored.semantic_routing_policy_digest ?? null) !== semanticRoutingPolicyDigest) throw new ArgumentError("autonomous brain batch checkpoint semantic-routing policy does not match");
      if (restored.batch_input_digest !== batchInputDigest) throw new ArgumentError("autonomous brain batch checkpoint does not match the current execution policy");
      if (restored.max_parallelism !== maxParallelism || restored.stop_on_error !== stopOnError) throw new ArgumentError("autonomous brain batch checkpoint controls do not match");
      if (restored.completed_indices.length > 0 && options.rehydrateExecution === undefined) throw new ArgumentError("resuming an autonomous brain batch requires rehydrateExecution");
    }
    const items: Array<AutonomousBrainBatchItem | undefined> = new Array(normalizedInputs.length);
    if (restored !== null) {
      for (let position = 0; position < restored.completed_indices.length; position += 1) {
        const index = restored.completed_indices[position]!;
        const context: AutonomousBrainBatchRehydrationContext = { job_id: jobId, index, mode: "brain", request_digest: requestDigests[index]!, task_digest: taskDigests[index]!, expected_result_digest: restored.completed_result_digests[position]! };
        let execution: AutonomousBrainExecution;
        try {
          execution = await options.rehydrateExecution!(context);
        } catch {
          throw new ArgumentError(`autonomous brain batch rehydration failed for item ${index}`);
        }
        if (!execution || execution.status !== "completed" || execution.plan.task_digest !== taskDigests[index]) throw new ArgumentError(`rehydrated autonomous brain batch item ${index} is not a matching successful execution`);
        const item: AutonomousBrainBatchItem = { index, status: "succeeded", task_digest: taskDigests[index]!, execution };
        if (batchItemDigest(item) !== restored.completed_result_digests[position]) throw new ArgumentError(`rehydrated autonomous brain batch item ${index} does not match its checkpoint digest`);
        items[index] = item;
      }
    }
    let persistChain: Promise<void> = Promise.resolve();
    const queueCheckpoint = (snapshot: readonly (AutonomousBrainBatchItem | undefined)[], status: "running" | "partial" | "completed"): void => {
      if (options.checkpointSink === undefined) return;
      const completed = snapshot.flatMap((item, index) => item?.status === "succeeded" ? [{ index, item }] : []);
      const checkpoint = makeBrainBatchCheckpoint({ jobId, requestDigests, batchInputDigest, semanticRoutingPolicyDigest, completed, maxParallelism, stopOnError, status });
      persistChain = persistChain.then(() => options.checkpointSink!(checkpoint));
    };
    queueCheckpoint(items, "running");
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= normalizedInputs.length) return;
        if (items[index] !== undefined) continue;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const execution = await this.execute(normalizedInputs[index]!, options.execution);
          const succeeded = execution.status === "completed";
          const refused = batchRefused(execution.status);
          const item: AutonomousBrainBatchItem = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          items[index] = item;
          if (succeeded) queueCheckpoint([...items], "running");
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, normalizedInputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    const result: AutonomousBrainBatchResult = {
      schema: AUTONOMOUS_BRAIN_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: batchDigest(normalized),
      retention: "metadata_only_tasks_and_provider_connector_values_transient",
      secret_material: "never_returned",
    };
    queueCheckpoint(normalized, result.status === "completed" ? "completed" : "partial");
    await persistChain;
    return result;
  }

  /**
   * Run automatic batches with metadata-only restart checkpoints. Completed automatic envelopes
   * are rehydrated by the caller and verified against the automatic route/plan result digest;
   * resumed items never fall back to direct execution or silently re-plan under new controls.
   */
  async executeAutoBatchResumable(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainAutoBatchResumableOptions): Promise<AutonomousBrainAutoBatchResult> {
    return this.executeAutoBatchResumableCore(inputs, options);
  }

  /**
   * Resume an automatic batch while recording rehydration, checkpoint progress, and resumed
   * provider work in one metadata-only trace. Restoring the trace is never sufficient to resume
   * the batch: the normal digest-bound checkpoint and transient result rehydrator remain required.
   */
  async executeAutoBatchResumableWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic traced resumable batch options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticBatchTraceTaskDigest(normalizedInputs),
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const batch = await this.executeAutoBatchResumableCore(normalizedInputs, batchOptions, trace);
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({
          batch_digest: batch.batch_digest,
          completed_count: batch.completed_count,
          failed_count: batch.failed_count,
          omitted_count: batch.omitted_count,
        }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_BATCH_SCHEMA,
        batch: tracedAutoBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executeAutoBatchResumableCore(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoBatchResumableOptions,
    trace?: AutonomousRunTraceSession,
  ): Promise<AutonomousBrainAutoBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.jobId === undefined) throw new ArgumentError("autonomous brain automatic resumable batch requires jobId");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const jobId = checkpointText("autonomous brain automatic batch jobId", options.jobId);
    if (options.checkpointSink !== undefined && typeof options.checkpointSink !== "function") throw new ArgumentError("autonomous brain automatic batch checkpointSink must be callable");
    if (options.rehydrateExecution !== undefined && typeof options.rehydrateExecution !== "function") throw new ArgumentError("autonomous brain automatic batch rehydrateExecution must be callable");
    const taskDigests = normalizedInputs.map((input) => brainBatchTaskDigest(input));
    const requestDigests = normalizedInputs.map((input, index) => brainBatchRequestDigest(input, index, "automatic"));
    const automaticExecutionPolicyDigest = brainAutomaticExecutionPolicyDigest(options.execution ?? {})!;
    const batchInputDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, mode: "automatic", request_digests: requestDigests, automatic_execution_policy_digest: automaticExecutionPolicyDigest });
    const restored = options.checkpoint === undefined ? null : validateBrainBatchCheckpoint(options.checkpoint);
    if (restored !== null) {
      if (restored.mode !== "automatic" || restored.job_id !== jobId || JSON.stringify(restored.request_digests) !== JSON.stringify(requestDigests)) throw new ArgumentError("autonomous brain automatic batch checkpoint does not match the current requests or mode");
      if (restored.automatic_execution_policy_digest !== automaticExecutionPolicyDigest) throw new ArgumentError("autonomous brain automatic batch checkpoint automatic execution policy does not match");
      if (restored.batch_input_digest !== batchInputDigest) throw new ArgumentError("autonomous brain automatic batch checkpoint does not match the current execution policy");
      if (restored.max_parallelism !== maxParallelism || restored.stop_on_error !== stopOnError) throw new ArgumentError("autonomous brain automatic batch checkpoint controls do not match");
      if (restored.completed_indices.length > 0 && options.rehydrateExecution === undefined) throw new ArgumentError("resuming an autonomous brain automatic batch requires rehydrateExecution");
    }
    const items: Array<AutonomousBrainAutoBatchItem | undefined> = new Array(normalizedInputs.length);
    if (restored !== null) {
      for (let position = 0; position < restored.completed_indices.length; position += 1) {
        const index = restored.completed_indices[position]!;
        const context: AutonomousBrainBatchRehydrationContext = { job_id: jobId, index, mode: "automatic", request_digest: requestDigests[index]!, task_digest: taskDigests[index]!, expected_result_digest: restored.completed_result_digests[position]! };
        let execution: AutonomousBrainAutoExecution;
        try {
          execution = await options.rehydrateExecution!(context);
        } catch {
          throw new ArgumentError(`rehydrated autonomous brain automatic batch item ${index} failed`);
        }
        if (!execution || execution.status !== "completed" || execution.plan.task_digest !== taskDigests[index]) throw new ArgumentError(`rehydrated autonomous brain automatic batch item ${index} is not a matching successful execution`);
        const item: AutonomousBrainAutoBatchItem = { index, status: "succeeded", task_digest: taskDigests[index]!, execution };
        if (batchItemDigest(item) !== restored.completed_result_digests[position]) throw new ArgumentError(`rehydrated autonomous brain automatic batch item ${index} does not match its checkpoint digest`);
        items[index] = item;
        if (trace !== undefined) await trace.record({ phase: "completed", status: "completed", detail_digest: digestJsonSync({ index, state: "rehydrated" }) });
      }
    }
    let persistChain: Promise<void> = Promise.resolve();
    const queueCheckpoint = (snapshot: readonly (AutonomousBrainAutoBatchItem | undefined)[], status: "running" | "partial" | "completed"): void => {
      if (options.checkpointSink === undefined) return;
      const completed = snapshot.flatMap((item, index) => item?.status === "succeeded" ? [{ index, item }] : []);
      const checkpoint = makeBrainBatchCheckpoint({ mode: "automatic", jobId, requestDigests, batchInputDigest, semanticRoutingPolicyDigest: null, automaticExecutionPolicyDigest, completed, maxParallelism, stopOnError, status });
      persistChain = persistChain.then(() => options.checkpointSink!(checkpoint));
    };
    queueCheckpoint(items, "running");
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= normalizedInputs.length) return;
        if (items[index] !== undefined) continue;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          if (trace !== undefined) await trace.record({ phase: "paused", status: "paused", detail_digest: digestJsonSync({ index, state: "omitted" }) });
          continue;
        }
        try {
          let execution: AutonomousBrainAutoExecution;
          let prepared: PreparedBrainRequest | undefined;
          if (trace === undefined) {
            execution = await this.executeAuto(normalizedInputs[index]!, options.execution ?? {});
          } else {
            const policy = options.execution ?? {};
            prepared = await this.prepare(normalizedInputs[index]!, selectBrainSemanticRouting(policy.semanticRouting, undefined), policy, policy.approveProviderCall);
            await trace.record({
              phase: "plan_compiled",
              status: "running",
              domains: this.traceDomains(prepared),
              route_digest: prepared.route.route_digest,
              plan_digest: prepared.plan.plan_digest,
              detail_digest: digestJsonSync({ index, state: "prepared_for_resume" }),
            });
            execution = await this.executeAutoPrepared(prepared, policy, trace);
            const traceStatus = autonomousRunTraceStatus(execution.status);
            const phase = traceStatus === "completed" || traceStatus === "partial" ? "completed" : traceStatus === "paused" ? "paused" : traceStatus === "refused" ? "refused" : "failed";
            await trace.record({
              phase,
              status: traceStatus,
              domains: this.traceDomains(prepared),
              route_digest: prepared.route.route_digest,
              plan_digest: prepared.plan.plan_digest,
              detail_digest: digestJsonSync({ index, state: execution.status }),
            });
          }
          const succeeded = execution.status === "completed";
          const refused = automaticBatchRefused(execution.status);
          const item: AutonomousBrainAutoBatchItem = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          items[index] = item;
          if (succeeded) queueCheckpoint([...items], "running");
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          if (trace !== undefined) await trace.record({ phase: "failed", status: "failed", detail_digest: digestJsonSync({ index, ...projection }) }).catch(() => undefined);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, normalizedInputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    const result: AutonomousBrainAutoBatchResult = {
      schema: AUTONOMOUS_BRAIN_AUTO_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: batchDigest(normalized),
      retention: "metadata_only_tasks_and_automatic_connector_values_transient",
      secret_material: "never_returned",
    };
    queueCheckpoint(normalized, result.status === "completed" ? "completed" : "partial");
    await persistChain;
    return result;
  }

  /** Resume automatic batches only after re-reviewing every current provider-free route. */
  async executeAutoBatchResumableWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoBatchResumableOptions,
  ): Promise<AutonomousBrainAutoBatchResult> {
    if (options?.execution?.semanticRouting !== undefined && options.execution.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic resumable batch requires provider-free routing; admit semantic routing separately before enabling it");
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policy = options?.execution ?? {};
    await this.authorizeAutoBatchLaunchAdmission(inputs, inputs.map(() => policy), admission);
    return this.executeAutoBatchResumable(inputs, options);
  }

  /** Resume a traced automatic batch only after re-admitting its complete current route set. */
  async executeAutoBatchResumableWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoBatchResult> {
    if (options?.execution?.semanticRouting !== undefined && options.execution.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic traced resumable batch requires provider-free routing; admit semantic routing separately before enabling it");
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic traced resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policy = options?.execution ?? {};
    await this.authorizeAutoBatchLaunchAdmission(inputs, inputs.map(() => policy), admission);
    return this.executeAutoBatchResumableWithTrace(inputs, options);
  }

  /** Resume a batch only after re-reviewing the complete current route set. */
  async executeBatchResumableWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainResumableBatchOptions,
  ): Promise<AutonomousBrainBatchResult> {
    this.rejectLaunchAdmittedSemanticRouting(options?.execution?.semanticRouting, "launch-admitted resumable batch execution requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeBatchLaunchAdmission(inputs, admission);
    return this.executeBatchResumable(inputs, options);
  }

  /** Execute automatic evaluator cycles with bounded concurrency and deterministic item order. */
  async executeAutoCycleBatch(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchOptions = {},
  ): Promise<AutonomousBrainAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const items: Array<AutonomousBrainAutoCycleBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const policy = batchOption(options.cycle, inputs[index]!, index) ?? {};
          const execution = await this.executeAutoCycle(inputs[index]!, policy);
          const succeeded = automaticCycleBatchSucceeded(execution.status);
          const refused = automaticBatchRefused(execution.status);
          items[index] = {
            index,
            status: succeeded ? "succeeded" : refused ? "refused" : "failed",
            task_digest: brainBatchTaskDigest(inputs[index]!),
            execution,
          };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_AUTO_CYCLE_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: automaticCycleBatchDigest(normalized),
      retention: "metadata_only_tasks_and_automatic_cycle_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Execute automatic evaluator/replan batches with bounded concurrency and deterministic order. */
  async executeAutoReplanCycleBatch(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchOptions,
  ): Promise<AutonomousBrainAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan batch requires a replan policy");
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const items: Array<AutonomousBrainAutoReplanBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const policy = batchOption(options.replan, inputs[index]!, index);
          if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
          const execution = await this.executeAutoReplanCycle(inputs[index]!, policy);
          const succeeded = automaticReplanBatchSucceeded(execution.status);
          const refused = automaticBatchRefused(execution.status);
          items[index] = {
            index,
            status: succeeded ? "succeeded" : refused ? "refused" : "failed",
            task_digest: brainBatchTaskDigest(inputs[index]!),
            execution,
          };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_AUTO_REPLAN_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: automaticCycleBatchDigest(normalized),
      retention: "metadata_only_tasks_and_automatic_replan_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Execute an automatic cycle batch only after one provider-free admission covers every route. */
  async executeAutoCycleBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoCycleBatchOptions = {},
  ): Promise<AutonomousBrainAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.cycle, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic cycle batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoCycleBatch(inputs, { ...options, cycle: (_input, index) => policies[index]! });
  }

  /** Execute an automatic replan batch only after one provider-free admission covers every route. */
  async executeAutoReplanCycleBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoReplanBatchOptions,
  ): Promise<AutonomousBrainAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan batch requires a replan policy");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.replan, input, index);
      if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
      return policy;
    });
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic replan batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoReplanCycleBatch(inputs, { ...options, replan: (_input, index) => policies[index]! });
  }

  /** Execute automatic evaluator cycles while recording one redacted batch lifecycle trace. */
  async executeAutoCycleBatchWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoCycleBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic cycle trace options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticCycleBatchTraceTaskDigest(normalizedInputs, "automatic_cycle"),
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const cycle = (input: AutonomousBrainRequest, index: number): AutonomousBrainAutoCycleOptions => {
        const policy = (batchOption(batchOptions.cycle, input, index) ?? {}) as AutonomousBrainAutoCycleOptions & { observer?: ProviderInvocationObserver; selectionEventCallback?: AutonomousModelSelectionTraceEventCallback };
        return {
          ...policy,
          observer: composeBrainObservers(policy.observer, trace.providerObserver()),
          selectionEventCallback: trace.selectionEventCallback(policy.selectionEventCallback),
        };
      };
      const batch = await this.executeAutoCycleBatch(normalizedInputs, { ...batchOptions, cycle });
      for (const item of batch.items) {
        const execution = item.execution;
        const rawStatus = execution === undefined
          ? item.status === "omitted" ? "paused" : item.status === "refused" ? "refused" : "failed"
          : autonomousRunTraceStatus(execution.status);
        const status = rawStatus === "running" || rawStatus === "unknown" ? "paused" : rawStatus;
        await trace.record({
          phase: status === "completed" || status === "partial" ? "completed" : status,
          status,
          domains: execution?.route.selected_domains,
          route_digest: execution?.route.route_digest,
          detail_digest: digestJsonSync({ index: item.index, item_status: item.status, execution_status: execution?.status ?? null }),
        });
      }
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({ batch_digest: batch.batch_digest, completed_count: batch.completed_count, failed_count: batch.failed_count, omitted_count: batch.omitted_count }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_CYCLE_BATCH_SCHEMA,
        batch: tracedAutomaticCycleBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Execute automatic evaluator/replan cycles while recording one redacted batch trace. */
  async executeAutoReplanCycleBatchWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoReplanBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic replan trace options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticCycleBatchTraceTaskDigest(normalizedInputs, "automatic_replan"),
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const replan = (input: AutonomousBrainRequest, index: number): AutonomousBrainAutoReplanCycleOptions => {
        const policy = batchOption(batchOptions.replan, input, index) as (AutonomousBrainAutoReplanCycleOptions & { observer?: ProviderInvocationObserver; selectionEventCallback?: AutonomousModelSelectionTraceEventCallback }) | undefined;
        if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
        return {
          ...policy,
          observer: composeBrainObservers(policy.observer, trace.providerObserver()),
          selectionEventCallback: trace.selectionEventCallback(policy.selectionEventCallback),
        };
      };
      const batch = await this.executeAutoReplanCycleBatch(normalizedInputs, { ...batchOptions, replan });
      for (const item of batch.items) {
        const execution = item.execution;
        const rawStatus = execution === undefined
          ? item.status === "omitted" ? "paused" : item.status === "refused" ? "refused" : "failed"
          : autonomousRunTraceStatus(execution.status);
        const status = rawStatus === "running" || rawStatus === "unknown" ? "paused" : rawStatus;
        await trace.record({
          phase: status === "completed" || status === "partial" ? "completed" : status,
          status,
          domains: execution?.route.selected_domains,
          route_digest: execution?.route.route_digest,
          detail_digest: digestJsonSync({ index: item.index, item_status: item.status, execution_status: execution?.status ?? null }),
        });
      }
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({ batch_digest: batch.batch_digest, completed_count: batch.completed_count, failed_count: batch.failed_count, omitted_count: batch.omitted_count }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_REPLAN_BATCH_SCHEMA,
        batch: tracedAutomaticCycleBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Traced automatic evaluator cycles after one provider-free admission covers all routes. */
  async executeAutoCycleBatchWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoCycleBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle trace batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.cycle, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic cycle trace batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoCycleBatchWithTrace(inputs, { ...options, cycle: (_input, index) => policies[index]! });
  }

  /** Traced automatic evaluator/replan cycles after one provider-free admission covers all routes. */
  async executeAutoReplanCycleBatchWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoReplanBatchTraceOptions,
  ): Promise<AutonomousBrainTracedAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan trace batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan trace batch requires a replan policy");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.replan, input, index);
      if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
      return policy;
    });
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic replan trace batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoReplanCycleBatchWithTrace(inputs, { ...options, replan: (_input, index) => policies[index]! });
  }

  /** Run automatic evaluator cycles with digest-bound, metadata-only restart checkpoints. */
  async executeAutoCycleBatchResumable(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchResumableOptions,
  ): Promise<AutonomousBrainAutoCycleBatchResult> {
    return this.executeAutoCycleBatchResumableCore(inputs, options);
  }

  /** Resume automatic evaluator cycles while composing the same redacted lifecycle trace. */
  async executeAutoCycleBatchResumableWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoCycleBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic cycle resumable trace options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticCycleBatchTraceTaskDigest(normalizedInputs, "automatic_cycle"),
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const cycle = (input: AutonomousBrainRequest, index: number): AutonomousBrainAutoCycleOptions => {
        const policy = (batchOption(batchOptions.cycle, input, index) ?? {}) as AutonomousBrainAutoCycleOptions & { observer?: ProviderInvocationObserver; selectionEventCallback?: AutonomousModelSelectionTraceEventCallback };
        return { ...policy, observer: composeBrainObservers(policy.observer, trace.providerObserver()), selectionEventCallback: trace.selectionEventCallback(policy.selectionEventCallback) };
      };
      const batch = await this.executeAutoCycleBatchResumable(normalizedInputs, { ...batchOptions, cycle });
      await recordAutomaticCycleBatchTrace(trace, batch);
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({ batch_digest: batch.batch_digest, completed_count: batch.completed_count, failed_count: batch.failed_count, omitted_count: batch.omitted_count }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_CYCLE_BATCH_SCHEMA,
        batch: tracedAutomaticCycleBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Run automatic evaluator/replan cycles with digest-bound, metadata-only restart checkpoints. */
  async executeAutoReplanCycleBatchResumable(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchResumableOptions,
  ): Promise<AutonomousBrainAutoReplanBatchResult> {
    return this.executeAutoReplanCycleBatchResumableCore(inputs, options);
  }

  /** Resume automatic evaluator/replan cycles while composing the same redacted lifecycle trace. */
  async executeAutoReplanCycleBatchResumableWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoReplanBatchResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain automatic replan resumable trace options must be an object");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const trace = new AutonomousRunTraceSession(options.traceStore, {
      run_id: options.runId,
      task_digest: automaticCycleBatchTraceTaskDigest(normalizedInputs, "automatic_replan"),
      domains: [...AUTONOMOUS_DOMAIN_NAMES],
    });
    await trace.started();
    try {
      const { traceStore: _traceStore, runId: _runId, ...batchOptions } = options;
      const replan = (input: AutonomousBrainRequest, index: number): AutonomousBrainAutoReplanCycleOptions => {
        const policy = batchOption(batchOptions.replan, input, index) as (AutonomousBrainAutoReplanCycleOptions & { observer?: ProviderInvocationObserver; selectionEventCallback?: AutonomousModelSelectionTraceEventCallback }) | undefined;
        if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
        return { ...policy, observer: composeBrainObservers(policy.observer, trace.providerObserver()), selectionEventCallback: trace.selectionEventCallback(policy.selectionEventCallback) };
      };
      const batch = await this.executeAutoReplanCycleBatchResumable(normalizedInputs, { ...batchOptions, replan });
      await recordAutomaticCycleBatchTrace(trace, batch);
      await trace.complete({
        status: autonomousRunTraceStatus(batch.status),
        domains: [...AUTONOMOUS_DOMAIN_NAMES],
        detail_digest: digestJsonSync({ batch_digest: batch.batch_digest, completed_count: batch.completed_count, failed_count: batch.failed_count, omitted_count: batch.omitted_count }),
      });
      return {
        schema: AUTONOMOUS_BRAIN_TRACED_AUTO_REPLAN_BATCH_SCHEMA,
        batch: tracedAutomaticCycleBatchResult(batch),
        trace: await trace.summary(),
        retention: "batch_values_caller_owned;trace_metadata_only_no_prompts_responses_or_tool_payloads",
        secret_material: "never_returned",
      };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  /** Resume automatic evaluator cycles only after one provider-free admission covers all routes. */
  async executeAutoCycleBatchResumableWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoCycleBatchResumableOptions,
  ): Promise<AutonomousBrainAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.cycle, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic cycle resumable batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoCycleBatchResumable(inputs, { ...options, cycle: (_input, index) => policies[index]! });
  }

  /** Resume traced automatic evaluator cycles only after one provider-free admission covers all routes. */
  async executeAutoCycleBatchResumableWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoCycleBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle resumable trace batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const policies = inputs.map((input, index) => batchOption(options.cycle, input, index) ?? {});
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic cycle resumable trace batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoCycleBatchResumableWithTrace(inputs, { ...options, cycle: (_input, index) => policies[index]! });
  }

  /** Resume automatic evaluator/replan cycles only after one provider-free admission covers all routes. */
  async executeAutoReplanCycleBatchResumableWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoReplanBatchResumableOptions,
  ): Promise<AutonomousBrainAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan resumable batch requires a replan policy");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.replan, input, index);
      if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
      return policy;
    });
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic replan resumable batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoReplanCycleBatchResumable(inputs, { ...options, replan: (_input, index) => policies[index]! });
  }

  /** Resume traced automatic evaluator/replan cycles only after one provider-free admission covers all routes. */
  async executeAutoReplanCycleBatchResumableWithLaunchAdmissionAndTrace(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAutoReplanBatchResumableTraceOptions,
  ): Promise<AutonomousBrainTracedAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan resumable trace batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan resumable trace batch requires a replan policy");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.replan, input, index);
      if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
      return policy;
    });
    for (const policy of policies) this.rejectLaunchAdmittedSemanticRouting(policy.semanticRouting, "launch-admitted automatic replan resumable trace batch requires provider-free routing; admit semantic routing separately before enabling it");
    await this.authorizeAutomaticCycleBatchLaunchAdmission(inputs, policies, admission);
    return this.executeAutoReplanCycleBatchResumableWithTrace(inputs, { ...options, replan: (_input, index) => policies[index]! });
  }

  private async executeAutoCycleBatchResumableCore(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchResumableOptions,
  ): Promise<AutonomousBrainAutoCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic cycle resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.jobId === undefined) throw new ArgumentError("autonomous brain automatic cycle resumable batch requires jobId");
    if (options.checkpointSink !== undefined && typeof options.checkpointSink !== "function") throw new ArgumentError("autonomous brain automatic cycle checkpointSink must be callable");
    if (options.rehydrateCycle !== undefined && typeof options.rehydrateCycle !== "function") throw new ArgumentError("autonomous brain automatic cycle rehydrateCycle must be callable");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const jobId = checkpointText("autonomous brain automatic cycle jobId", options.jobId);
    const policies = normalizedInputs.map((input, index) => batchOption(options.cycle, input, index) ?? {});
    const taskDigests = normalizedInputs.map((input) => brainBatchTaskDigest(input));
    const requestDigests = normalizedInputs.map((input, index) => brainBatchRequestDigest(input, index, "automatic_cycle"));
    const automaticExecutionPolicyDigest = digestJsonSync({ schema: "bioprism-typescript-autonomous-brain-automatic-cycle-policy-set/0.1", mode: "automatic_cycle", policies: policies.map((policy) => automaticCyclePolicyDigest(policy, "automatic_cycle", options.policyDigest)) });
    const batchInputDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, mode: "automatic_cycle", request_digests: requestDigests, automatic_execution_policy_digest: automaticExecutionPolicyDigest });
    const restored = options.checkpoint === undefined ? null : validateBrainBatchCheckpoint(options.checkpoint);
    if (restored !== null) {
      if (restored.mode !== "automatic_cycle" || restored.job_id !== jobId || JSON.stringify(restored.request_digests) !== JSON.stringify(requestDigests)) throw new ArgumentError("autonomous brain automatic cycle checkpoint does not match the current requests or mode");
      if (restored.automatic_execution_policy_digest !== automaticExecutionPolicyDigest || restored.batch_input_digest !== batchInputDigest) throw new ArgumentError("autonomous brain automatic cycle checkpoint policy does not match");
      if (restored.max_parallelism !== maxParallelism || restored.stop_on_error !== stopOnError) throw new ArgumentError("autonomous brain automatic cycle checkpoint controls do not match");
      if (restored.completed_indices.length > 0 && options.rehydrateCycle === undefined) throw new ArgumentError("resuming an autonomous brain automatic cycle batch requires rehydrateCycle");
    }
    const items: Array<AutonomousBrainAutoCycleBatchItem | undefined> = new Array(normalizedInputs.length);
    if (restored !== null) {
      for (let position = 0; position < restored.completed_indices.length; position += 1) {
        const index = restored.completed_indices[position]!;
        const context: AutonomousBrainBatchRehydrationContext = { job_id: jobId, index, mode: "automatic_cycle", request_digest: requestDigests[index]!, task_digest: taskDigests[index]!, expected_result_digest: restored.completed_result_digests[position]! };
        let execution: AutonomousBrainAutoCycleResult;
        try { execution = await options.rehydrateCycle!(context); } catch { throw new ArgumentError(`rehydrated autonomous brain automatic cycle item ${index} failed`); }
        if (!execution || !execution.route || execution.route.task_digest !== taskDigests[index] || !automaticCycleBatchSucceeded(execution.status)) throw new ArgumentError(`rehydrated autonomous brain automatic cycle item ${index} is not a matching successful execution`);
        const item: AutonomousBrainAutoCycleBatchItem = { index, status: "succeeded", task_digest: taskDigests[index]!, execution };
        if (automaticCycleBatchItemDigest(item) !== restored.completed_result_digests[position]) throw new ArgumentError(`rehydrated autonomous brain automatic cycle item ${index} does not match its checkpoint digest`);
        items[index] = item;
      }
    }
    let persistChain: Promise<void> = Promise.resolve();
    const queueCheckpoint = (snapshot: readonly (AutonomousBrainAutoCycleBatchItem | undefined)[], status: "running" | "partial" | "completed"): void => {
      if (options.checkpointSink === undefined) return;
      const completed = snapshot.flatMap((item, index) => item?.status === "succeeded" ? [{ index, item }] : []);
      const checkpoint = makeBrainBatchCheckpoint({ mode: "automatic_cycle", jobId, requestDigests, batchInputDigest, semanticRoutingPolicyDigest: null, automaticExecutionPolicyDigest, completed, maxParallelism, stopOnError, status });
      persistChain = persistChain.then(() => options.checkpointSink!(checkpoint));
    };
    queueCheckpoint(items, "running");
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex++;
        if (index >= normalizedInputs.length) return;
        if (items[index] !== undefined) continue;
        if (halted) { items[index] = { index, status: "omitted", task_digest: null }; continue; }
        try {
          const execution = await this.executeAutoCycle(normalizedInputs[index]!, policies[index]!);
          const succeeded = automaticCycleBatchSucceeded(execution.status);
          const refused = automaticBatchRefused(execution.status);
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: taskDigests[index]!, execution };
          if (succeeded) queueCheckpoint([...items], "running");
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, normalizedInputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    const result: AutonomousBrainAutoCycleBatchResult = { schema: AUTONOMOUS_BRAIN_AUTO_CYCLE_BATCH_SCHEMA, status: batchStatus(completed, failed, omitted), items: normalized, completed_count: completed, failed_count: failed, omitted_count: omitted, max_parallelism: maxParallelism, stop_on_error: stopOnError, batch_digest: automaticCycleBatchDigest(normalized), retention: "metadata_only_tasks_and_automatic_cycle_values_transient", secret_material: "never_returned" };
    queueCheckpoint(normalized, result.status === "completed" ? "completed" : "partial");
    await persistChain;
    return result;
  }

  private async executeAutoReplanCycleBatchResumableCore(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchResumableOptions,
  ): Promise<AutonomousBrainAutoReplanBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic replan resumable batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.jobId === undefined) throw new ArgumentError("autonomous brain automatic replan resumable batch requires jobId");
    if (!options || options.replan === undefined) throw new ArgumentError("autonomous brain automatic replan resumable batch requires a replan policy");
    if (options.checkpointSink !== undefined && typeof options.checkpointSink !== "function") throw new ArgumentError("autonomous brain automatic replan checkpointSink must be callable");
    if (options.rehydrateReplan !== undefined && typeof options.rehydrateReplan !== "function") throw new ArgumentError("autonomous brain automatic replan rehydrateReplan must be callable");
    const normalizedInputs = inputs.map((input) => validateRequest(input));
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const jobId = checkpointText("autonomous brain automatic replan jobId", options.jobId);
    const policies = normalizedInputs.map((input, index) => {
      const policy = batchOption(options.replan, input, index);
      if (policy === undefined) throw new ArgumentError("automatic replan batch policy factory returned no policy");
      return policy;
    });
    const taskDigests = normalizedInputs.map((input) => brainBatchTaskDigest(input));
    const requestDigests = normalizedInputs.map((input, index) => brainBatchRequestDigest(input, index, "automatic_replan"));
    const automaticExecutionPolicyDigest = digestJsonSync({ schema: "bioprism-typescript-autonomous-brain-automatic-cycle-policy-set/0.1", mode: "automatic_replan", policies: policies.map((policy) => automaticCyclePolicyDigest(policy, "automatic_replan", options.policyDigest)) });
    const batchInputDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, mode: "automatic_replan", request_digests: requestDigests, automatic_execution_policy_digest: automaticExecutionPolicyDigest });
    const restored = options.checkpoint === undefined ? null : validateBrainBatchCheckpoint(options.checkpoint);
    if (restored !== null) {
      if (restored.mode !== "automatic_replan" || restored.job_id !== jobId || JSON.stringify(restored.request_digests) !== JSON.stringify(requestDigests)) throw new ArgumentError("autonomous brain automatic replan checkpoint does not match the current requests or mode");
      if (restored.automatic_execution_policy_digest !== automaticExecutionPolicyDigest || restored.batch_input_digest !== batchInputDigest) throw new ArgumentError("autonomous brain automatic replan checkpoint policy does not match");
      if (restored.max_parallelism !== maxParallelism || restored.stop_on_error !== stopOnError) throw new ArgumentError("autonomous brain automatic replan checkpoint controls do not match");
      if (restored.completed_indices.length > 0 && options.rehydrateReplan === undefined) throw new ArgumentError("resuming an autonomous brain automatic replan batch requires rehydrateReplan");
    }
    const items: Array<AutonomousBrainAutoReplanBatchItem | undefined> = new Array(normalizedInputs.length);
    if (restored !== null) {
      for (let position = 0; position < restored.completed_indices.length; position += 1) {
        const index = restored.completed_indices[position]!;
        const context: AutonomousBrainBatchRehydrationContext = { job_id: jobId, index, mode: "automatic_replan", request_digest: requestDigests[index]!, task_digest: taskDigests[index]!, expected_result_digest: restored.completed_result_digests[position]! };
        let execution: AutonomousBrainAutoReplanCycleResult;
        try { execution = await options.rehydrateReplan!(context); } catch { throw new ArgumentError(`rehydrated autonomous brain automatic replan item ${index} failed`); }
        if (!execution || !execution.route || execution.route.task_digest !== taskDigests[index] || !automaticReplanBatchSucceeded(execution.status)) throw new ArgumentError(`rehydrated autonomous brain automatic replan item ${index} is not a matching successful execution`);
        const item: AutonomousBrainAutoReplanBatchItem = { index, status: "succeeded", task_digest: taskDigests[index]!, execution };
        if (automaticCycleBatchItemDigest(item) !== restored.completed_result_digests[position]) throw new ArgumentError(`rehydrated autonomous brain automatic replan item ${index} does not match its checkpoint digest`);
        items[index] = item;
      }
    }
    let persistChain: Promise<void> = Promise.resolve();
    const queueCheckpoint = (snapshot: readonly (AutonomousBrainAutoReplanBatchItem | undefined)[], status: "running" | "partial" | "completed"): void => {
      if (options.checkpointSink === undefined) return;
      const completed = snapshot.flatMap((item, index) => item?.status === "succeeded" ? [{ index, item }] : []);
      const checkpoint = makeBrainBatchCheckpoint({ mode: "automatic_replan", jobId, requestDigests, batchInputDigest, semanticRoutingPolicyDigest: null, automaticExecutionPolicyDigest, completed, maxParallelism, stopOnError, status });
      persistChain = persistChain.then(() => options.checkpointSink!(checkpoint));
    };
    queueCheckpoint(items, "running");
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex++;
        if (index >= normalizedInputs.length) return;
        if (items[index] !== undefined) continue;
        if (halted) { items[index] = { index, status: "omitted", task_digest: null }; continue; }
        try {
          const execution = await this.executeAutoReplanCycle(normalizedInputs[index]!, policies[index]!);
          const succeeded = automaticReplanBatchSucceeded(execution.status);
          const refused = automaticBatchRefused(execution.status);
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: taskDigests[index]!, execution };
          if (succeeded) queueCheckpoint([...items], "running");
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, normalizedInputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    const result: AutonomousBrainAutoReplanBatchResult = { schema: AUTONOMOUS_BRAIN_AUTO_REPLAN_BATCH_SCHEMA, status: batchStatus(completed, failed, omitted), items: normalized, completed_count: completed, failed_count: failed, omitted_count: omitted, max_parallelism: maxParallelism, stop_on_error: stopOnError, batch_digest: automaticCycleBatchDigest(normalized), retention: "metadata_only_tasks_and_automatic_replan_values_transient", secret_material: "never_returned" };
    queueCheckpoint(normalized, result.status === "completed" ? "completed" : "partial");
    await persistChain;
    return result;
  }

  /** Execute ordinary closed-loop cycles with bounded concurrency and deterministic result order. */
  async executeCycleBatch(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainCycleBatchOptions = {}): Promise<AutonomousBrainCycleBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain cycle batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const items: Array<AutonomousBrainCycleBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const execution = await this.executeCycle(inputs[index]!, batchOption(options.cycle, inputs[index]!, index) ?? {});
          const succeeded = cycleBatchSucceeded(execution.status);
          const refused = batchRefused(execution.status);
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_CYCLE_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: batchDigest(normalized),
      retention: "metadata_only_tasks_and_cycle_connector_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Execute a cycle batch only when every reviewed route is covered by launch admission. */
  async executeCycleBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainCycleBatchOptions = {},
  ): Promise<AutonomousBrainCycleBatchResult> {
    this.rejectLaunchAdmittedSemanticRouting((options as unknown as { semanticRouting?: unknown }).semanticRouting, "launch-admitted cycle batch requires provider-free routing; admit semantic routing separately before enabling it");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.cycle, input, index) ?? {};
      this.rejectLaunchAdmittedSemanticRouting((policy as unknown as { semanticRouting?: unknown }).semanticRouting, "launch-admitted cycle batch requires provider-free routing; admit semantic routing separately before enabling it");
      return policy;
    });
    await this.authorizeBatchLaunchAdmission(inputs, admission);
    return this.executeCycleBatch(inputs, { ...options, cycle: (_input, index) => policies[index]! });
  }

  /** Execute evaluator-guided replanning loops with bounded concurrency and deterministic result order. */
  async executeAdaptiveCycleBatch(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainAdaptiveBatchOptions): Promise<AutonomousBrainAdaptiveBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain adaptive batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (!options || options.adaptive === undefined) throw new ArgumentError("autonomous brain adaptive batch requires an adaptive evaluator policy");
    const { maxParallelism, stopOnError } = boundedBatchControls(options);
    const items: Array<AutonomousBrainAdaptiveBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", task_digest: null };
          continue;
        }
        try {
          const adaptive = batchOption(options.adaptive, inputs[index]!, index);
          if (adaptive === undefined) throw new ArgumentError("adaptive batch policy factory returned no policy");
          const execution = await this.executeAdaptiveCycle(inputs[index]!, adaptive);
          const succeeded = adaptiveBatchSucceeded(execution.status);
          const refused = batchRefused(execution.status);
          items[index] = { index, status: succeeded ? "succeeded" : refused ? "refused" : "failed", task_digest: execution.plan.task_digest, execution };
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", task_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, task_digest: null, error_class: "AutonomousBrainError", failure_code: "missing_batch_result" });
    const completed = normalized.filter((item) => item.status === "succeeded").length;
    const failed = normalized.filter((item) => item.status === "failed" || item.status === "refused").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    return {
      schema: AUTONOMOUS_BRAIN_ADAPTIVE_BATCH_SCHEMA,
      status: batchStatus(completed, failed, omitted),
      items: normalized,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      batch_digest: batchDigest(normalized),
      retention: "metadata_only_tasks_and_adaptive_connector_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Execute adaptive/replan batches only when every reviewed route is covered by admission. */
  async executeAdaptiveCycleBatchWithLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousBrainAdaptiveBatchOptions,
  ): Promise<AutonomousBrainAdaptiveBatchResult> {
    if (!options || options.adaptive === undefined) throw new ArgumentError("autonomous brain adaptive batch requires an adaptive evaluator policy");
    this.rejectLaunchAdmittedSemanticRouting((options as unknown as { semanticRouting?: unknown }).semanticRouting, "launch-admitted adaptive batch requires provider-free routing; admit semantic routing separately before enabling it");
    const policies = inputs.map((input, index) => {
      const policy = batchOption(options.adaptive, input, index);
      if (policy === undefined) throw new ArgumentError("adaptive batch policy factory returned no policy");
      this.rejectLaunchAdmittedSemanticRouting((policy as unknown as { semanticRouting?: unknown }).semanticRouting, "launch-admitted adaptive batch requires provider-free routing; admit semantic routing separately before enabling it");
      return policy;
    });
    await this.authorizeBatchLaunchAdmission(inputs, admission);
    return this.executeAdaptiveCycleBatch(inputs, { ...options, adaptive: (_input, index) => policies[index]! });
  }

  private rejectLaunchAdmittedSemanticRouting(value: unknown, message: string): void {
    if (value === true || (isObject(value) && value.enabled === true)) throw new ArgumentError(message);
  }

  private async workflowLaunchDomains(
    task: string,
    options: Pick<AutonomousWorkflowExecuteOptions, "domain" | "hints" | "routeOverride" | "semanticRouting">,
  ): Promise<AutonomousDomainName[]> {
    this.rejectLaunchAdmittedSemanticRouting(options.semanticRouting, "launch-admitted workflow execution requires provider-free routing; admit semantic routing separately before enabling it");
    const route = options.routeOverride
      ? await validateAutonomousRouteOverride(task, options.routeOverride)
      : await this.agent.route(task, { domain: options.domain, hints: options.hints });
    return route.abstained ? [] : [...route.selected_domains];
  }

  private async createWorkflowTrace(
    task: string,
    options: Pick<AutonomousWorkflowExecuteOptions, "domain" | "routeOverride">,
    traceStore: AutonomousRunTraceStore,
    runId: string,
  ): Promise<AutonomousRunTraceSession> {
    const taskText = boundedText("autonomous brain workflow task", task, 32_000);
    const routeDomains = options.routeOverride?.selected_domains ?? [];
    const domains = routeDomains.length > 0
      ? [...routeDomains]
      : [options.domain ?? "cross_domain"];
    if (!traceStore || typeof traceStore.append !== "function" || typeof traceStore.events !== "function") throw new ArgumentError("autonomous brain workflow trace requires a trace store");
    return new AutonomousRunTraceSession(traceStore, {
      run_id: runId,
      task_digest: digestJsonSync({ task: taskText }),
      domains,
    });
  }

  private async recordWorkflowTracePlan(
    trace: AutonomousRunTraceSession,
    execution: AutonomousBrainWorkflowResult,
  ): Promise<void> {
    const route = execution.route;
    const domains = route?.selected_domains?.length
      ? [...route.selected_domains, ...(route.cross_domain ? ["cross_domain" as const] : [])]
      : execution.blueprint
        ? [execution.blueprint.domain_profile.domain]
        : trace.domains;
    const uniqueDomains = [...new Set(domains)] as AutonomousDomainName[];
    const selections = execution.stage_results
      .map((stage) => stage.run?.selection)
      .filter((selection): selection is NonNullable<AutonomousRunResult["selection"]> => selection !== null && selection !== undefined);
    await trace.record({
      phase: "plan_compiled",
      status: "running",
      domains: uniqueDomains,
      route_digest: route?.route_digest ?? null,
      plan_digest: execution.blueprint?.plan.plan_digest ?? null,
      selection_digest: selections.length > 0 ? digestJsonSync(selections) : null,
    });
  }

  private workflowTraceCompletion(execution: AutonomousBrainWorkflowResult): Parameters<AutonomousRunTraceSession["complete"]>[0] {
    const route = execution.route;
    const domains = route?.selected_domains?.length
      ? [...route.selected_domains, ...(route.cross_domain ? ["cross_domain" as const] : [])]
      : execution.blueprint
        ? [execution.blueprint.domain_profile.domain]
        : undefined;
    const selections = execution.stage_results
      .map((stage) => stage.run?.selection)
      .filter((selection): selection is NonNullable<AutonomousRunResult["selection"]> => selection !== null && selection !== undefined);
    return {
      status: autonomousRunTraceStatus(execution.status),
      ...(domains === undefined ? {} : { domains: [...new Set(domains)] as AutonomousDomainName[] }),
      route_digest: route?.route_digest ?? null,
      plan_digest: execution.blueprint?.plan.plan_digest ?? null,
      selection_digest: selections.length > 0 ? digestJsonSync(selections) : null,
      detail_digest: execution.checkpoint?.checkpoint_digest ?? null,
    };
  }

  private workflowCycleTraceCompletion(cycle: AutonomousBrainWorkflowCycleResult): Parameters<AutonomousRunTraceSession["complete"]>[0] {
    const execution = cycle.final;
    const completion = execution === null ? {} : this.workflowTraceCompletion(execution);
    return {
      ...completion,
      status: autonomousRunTraceStatus(cycle.status),
    };
  }

  /** Rehydrate a plan through its canonical projection so caller mutations cannot widen replay. */
  private revalidateBrainPlan(plan: AutonomousBrainPlan, message: string): AutonomousBrainPlan {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError(message);
    return AutonomousBrainPlan.fromJSON(plan.toJSON());
  }

  private async authorizeBatchLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    admission: AutonomousLaunchAdmissionReport,
  ): Promise<void> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain admitted batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    const selected = new Set<AutonomousDomainName>();
    for (const input of inputs) {
      const prepared = await this.prepare(validateRequest(input));
      for (const domainName of prepared.route.selected_domains) selected.add(domainName);
    }
    authorizeAutonomousLaunchDomains(admission, [...selected]);
  }

  private async authorizeAutoBatchLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    policies: readonly AutonomousBrainAutoExecuteOptions[],
    admission: AutonomousLaunchAdmissionReport,
  ): Promise<void> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain automatic batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (policies.length !== inputs.length) throw new ArgumentError("autonomous brain automatic batch policies do not match the requests");
    const selected = new Set<AutonomousDomainName>();
    for (const [index, input] of inputs.entries()) {
      const policy = policies[index]!;
      const prepared = await this.prepare(validateRequest(input), undefined, policy, policy.approveProviderCall);
      for (const domainName of prepared.route.selected_domains) selected.add(domainName);
    }
    if (selected.size > 0) authorizeAutonomousLaunchDomains(admission, [...selected]);
  }

  private async authorizeAutomaticCycleBatchLaunchAdmission(
    inputs: readonly AutonomousBrainRequest[],
    policies: readonly Record<string, unknown>[],
    admission: AutonomousLaunchAdmissionReport,
  ): Promise<void> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_BRAIN_BATCH) throw new ArgumentError(`autonomous brain admitted automatic cycle batch must contain 1..=${MAX_AUTONOMOUS_BRAIN_BATCH} entries`);
    if (policies.length !== inputs.length) throw new ArgumentError("autonomous brain automatic cycle batch policies do not match the requests");
    const selected = new Set<AutonomousDomainName>();
    for (const [index, input] of inputs.entries()) {
      const request = validateRequest(input);
      const policy = policies[index]!;
      const bound = bindAutomaticCycleOptions(request, policy) as {
        minConfidence?: number;
        minMargin?: number;
        maxDomains?: number;
        semanticRouting?: unknown;
      };
      this.rejectLaunchAdmittedSemanticRouting(bound.semanticRouting, "launch-admitted automatic cycle batch requires provider-free routing; admit semantic routing separately before enabling it");
      const route = await this.agent.route(request.task, {
        domain: request.domain,
        hints: request.hints,
        minConfidence: bound.minConfidence,
        minMargin: bound.minMargin,
        maxDomains: bound.maxDomains,
        allowCrossDomain: request.allow_cross_domain,
      });
      if (!route.abstained) for (const domainName of route.selected_domains) selected.add(domainName);
    }
    if (selected.size > 0) authorizeAutonomousLaunchDomains(admission, [...selected]);
  }

  private async prepare(
    input: AutonomousBrainRequest,
    semanticRouting?: AutonomousBrainSemanticRoutingInput,
    source: AutonomousBrainSemanticSource = {},
    defaultApproval?: boolean,
    routeOverride?: AutonomousRouteProposal,
    semanticRouteOverride?: AutonomousSemanticRouteResult | null,
  ): Promise<PreparedBrainRequest> {
    const request = validateRequest(input);
    const semanticConfig = routeOverride === undefined
      ? prepareBrainSemanticRoute(request, semanticRouting, source, defaultApproval)
      : null;
    const semanticRoute = semanticRouteOverride === undefined
      ? semanticConfig === null ? null : await semanticRouteAutonomousTask(this.agent, request.task, semanticConfig.options)
      : semanticRouteOverride;
    const route = routeOverride === undefined
      ? semanticRoute === null
        ? await this.agent.route(request.task, {
          domain: request.domain ?? source.domain,
          hints: request.hints ?? source.hints,
          minConfidence: source.minConfidence,
          minMargin: source.minMargin,
          maxDomains: source.maxDomains,
          allowCrossDomain: request.allow_cross_domain ?? source.allowCrossDomain ?? true,
        })
        : await validateAutonomousRouteOverride(request.task, semanticRoute.route)
      : await validateAutonomousRouteOverride(request.task, routeOverride);
    const plan = semanticRouteOverride === undefined
      ? await this.buildPlanForRoute(request, route, semanticRoute)
      : await this.buildPlanForRoute(request, route, semanticRouteOverride);
    const connectorPlan: AutonomousConnectorOperationPlan | null = plan.connector_plan === null
      ? null
      : this.connectorOperations === undefined
        ? null
        : this.connectorOperations.plan(request.connector!);
    const semanticBudget = semanticConfig?.budget ?? null;
    // Requiring the route digest to agree here catches an accidental route recomputation change
    // between plan construction and the returned prepared request without retaining task text.
    if (plan.route.route_digest !== route.route_digest) throw new ProviderRuntimeError("autonomous brain route changed while preparing execution", { code: "configuration" });
    if (semanticRoute !== null && semanticRoute.route.route_digest !== route.route_digest) throw new ProviderRuntimeError("autonomous brain semantic route changed while preparing execution", { code: "configuration" });
    return { request, route, semanticRoute, semanticBudget, plan, connectorPlan };
  }

  private traceDomains(prepared: PreparedBrainRequest): AutonomousDomainName[] {
    const domains = prepared.plan.selected_domains.length
      ? [...prepared.plan.selected_domains, ...(prepared.route.cross_domain ? ["cross_domain" as const] : [])]
      : [prepared.request.domain ?? "cross_domain"];
    return [...new Set(domains)] as AutonomousDomainName[];
  }

  private createTrace(prepared: PreparedBrainRequest, store: AutonomousRunTraceStore, runId: string): AutonomousRunTraceSession {
    if (!store || typeof store.append !== "function" || typeof store.events !== "function") throw new ArgumentError("autonomous brain traced execution requires a trace store");
    return new AutonomousRunTraceSession(store, { run_id: runId, task_digest: prepared.plan.task_digest, domains: this.traceDomains(prepared) });
  }

  private async recordCycleTraceStages(trace: AutonomousRunTraceSession, cycle: AutonomousBrainCycleResult | AutonomousBrainAdaptiveCycleResult): Promise<void> {
    const value = cycle as unknown as Record<string, unknown>;
    const evaluations = Array.isArray(value.evaluations) ? value.evaluations : value.evaluation === null || value.evaluation === undefined ? [] : [value.evaluation];
    if (evaluations.length > 0) {
      await trace.record({ phase: "evaluation_settled", status: "running", detail_digest: digestJsonSync({ count: evaluations.length, last: evaluations.at(-1) ?? null }) });
    }
    const learningEpisodeIds = Array.isArray(value.learning_episode_ids) ? value.learning_episode_ids : value.learning_episode_id ? [value.learning_episode_id] : [];
    if (learningEpisodeIds.length > 0) {
      await trace.record({ phase: "learning_prepared", status: "running", detail_digest: digestJsonSync({ count: learningEpisodeIds.length, episode_digests: learningEpisodeIds.map((entry) => digestJsonSync(entry)) }) });
    }
  }

  private async executePreparedWithTrace(prepared: PreparedBrainRequest, options: AutonomousBrainTraceOptions): Promise<AutonomousBrainTracedExecution> {
    const initialDomains = this.traceDomains(prepared);
    const trace = this.createTrace(prepared, options.traceStore, options.runId);
    await trace.started();
    try {
      await trace.record({
        phase: "plan_compiled",
        status: "running",
        domains: [...new Set(initialDomains)] as AutonomousDomainName[],
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
      });
      const execution = await this.executePrepared(prepared, options, trace);
      const run = execution.run;
      const selection = isObject(run) && isObject(run.selection)
        ? run.selection
        : isObject(run) && isObject(run.synthesis) && isObject(run.synthesis.selection)
          ? run.synthesis.selection
          : null;
      await trace.complete({
        status: autonomousRunTraceStatus(execution.status),
        domains: [...new Set(initialDomains)] as AutonomousDomainName[],
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
        selection_digest: selection === null ? null : digestJsonSync(selection as JsonObject),
      });
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executeCyclePreparedWithTrace(prepared: PreparedBrainRequest, options: AutonomousBrainCycleTraceOptions): Promise<AutonomousBrainTracedCycleExecution> {
    const initialDomains = this.traceDomains(prepared);
    const trace = this.createTrace(prepared, options.traceStore, options.runId);
    await trace.started();
    try {
      await trace.record({ phase: "plan_compiled", status: "running", domains: initialDomains, route_digest: prepared.route.route_digest, plan_digest: prepared.plan.plan_digest });
      const execution = await this.executeCyclePrepared(prepared, options, trace);
      if (execution.cycle) await this.recordCycleTraceStages(trace, execution.cycle);
      const cycleValue = execution.cycle as unknown as Record<string, unknown> | null;
      const run = cycleValue && isObject(cycleValue.run) ? cycleValue.run : cycleValue && isObject(cycleValue.final) && isObject(cycleValue.final.run) ? cycleValue.final.run : null;
      const selection = run && isObject(run.selection) ? run.selection : run && isObject(run.synthesis) && isObject(run.synthesis.selection) ? run.synthesis.selection : null;
      await trace.complete({ status: autonomousRunTraceStatus(execution.status), domains: initialDomains, route_digest: prepared.route.route_digest, plan_digest: prepared.plan.plan_digest, selection_digest: selection === null ? null : digestJsonSync(selection as JsonObject) });
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executeAdaptiveCyclePreparedWithTrace(prepared: PreparedBrainRequest, options: AutonomousBrainAdaptiveCycleTraceOptions): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    const initialDomains = this.traceDomains(prepared);
    const trace = this.createTrace(prepared, options.traceStore, options.runId);
    await trace.started();
    try {
      await trace.record({ phase: "plan_compiled", status: "running", domains: initialDomains, route_digest: prepared.route.route_digest, plan_digest: prepared.plan.plan_digest });
      const execution = await this.executeAdaptiveCyclePrepared(prepared, options, trace);
      if (execution.adaptive) await this.recordCycleTraceStages(trace, execution.adaptive);
      const adaptiveValue = execution.adaptive as unknown as Record<string, unknown> | null;
      const final = adaptiveValue && isObject(adaptiveValue.final) ? adaptiveValue.final : null;
      const run = final && isObject(final.run) ? final.run : null;
      const selection = run && isObject(run.selection) ? run.selection : run && isObject(run.synthesis) && isObject(run.synthesis.selection) ? run.synthesis.selection : null;
      await trace.complete({ status: autonomousRunTraceStatus(execution.status), domains: initialDomains, route_digest: prepared.route.route_digest, plan_digest: prepared.plan.plan_digest, selection_digest: selection === null ? null : digestJsonSync(selection as JsonObject) });
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executeAutoPreparedWithTrace(prepared: PreparedBrainRequest, options: AutonomousBrainAutoTraceOptions): Promise<AutonomousBrainTracedAutoExecution> {
    const initialDomains = this.traceDomains(prepared);
    const trace = this.createTrace(prepared, options.traceStore, options.runId);
    await trace.started();
    try {
      await trace.record({
        phase: "plan_compiled",
        status: "running",
        domains: [...new Set(initialDomains)] as AutonomousDomainName[],
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
      });
      const { traceStore: _traceStore, runId: _runId, ...executionOptions } = options;
      const execution = await this.executeAutoPrepared(prepared, executionOptions, trace);
      const automatic = execution.automatic;
      const run = automatic?.result ?? automatic?.planning?.result ?? null;
      const selection = isObject(run) && isObject(run.selection)
        ? run.selection
        : isObject(run) && isObject(run.synthesis) && isObject(run.synthesis.selection)
          ? run.synthesis.selection
          : null;
      await trace.complete({
        status: autonomousRunTraceStatus(execution.status),
        domains: [...new Set(initialDomains)] as AutonomousDomainName[],
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
        selection_digest: selection === null ? null : digestJsonSync(selection as JsonObject),
      });
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executeApprovedSelectionPrepared(
    prepared: PreparedBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousBrainApprovedSelectionOptions,
    trace?: AutonomousRunTraceSession,
  ): Promise<AutonomousBrainExecution> {
    const { request, plan } = prepared;
    const runOptions = {
      ...(options.run ?? {}),
      domain: request.domain!,
      capability: options.run?.capability ?? request.capability,
      context: options.run?.context ?? request.context,
      observer: composeBrainObservers(options.run?.observer, trace?.providerObserver()),
      selectionEventCallback: trace === undefined
        ? options.run?.selectionEventCallback
        : trace.selectionEventCallback(options.run?.selectionEventCallback),
    } as AutonomousApprovedModelSelectionOptions;
    const run = await this.agent.runApprovedModelSelection(request.task, preview, runOptions);
    return {
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status: run.status,
      plan: plan.toJSON(),
      run,
      connector: null,
      error: null,
      retention: "plan_metadata_only;run_and_connector_values_transient_to_caller",
      secret_material: "never_returned",
    };
  }

  private async executeApprovedSelectionPreparedWithTrace(
    prepared: PreparedBrainRequest,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousBrainApprovedSelectionTraceOptions,
  ): Promise<AutonomousBrainTracedApprovedSelection> {
    const initialDomains = this.traceDomains(prepared);
    const trace = this.createTrace(prepared, options.traceStore, options.runId);
    await trace.started();
    try {
      await trace.record({
        phase: "plan_compiled",
        status: "running",
        domains: initialDomains,
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
      });
      const execution = await this.executeApprovedSelectionPrepared(prepared, preview, options, trace);
      const run = execution.run;
      const selection = isObject(run) && isObject(run.selection) ? run.selection : null;
      await trace.complete({
        status: autonomousRunTraceStatus(execution.status),
        domains: initialDomains,
        route_digest: prepared.route.route_digest,
        plan_digest: prepared.plan.plan_digest,
        selection_digest: selection === null ? null : digestJsonSync(selection as JsonObject),
      });
      return { execution, trace: await trace.summary() };
    } catch (error) {
      const projection = errorProjection(error);
      await trace.fail({ failure_class: projection.error_class, failure_code: projection.failure_code, detail_digest: digestJsonSync(projection) }).catch(() => undefined);
      throw error;
    }
  }

  private async executePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainExecuteOptions, trace?: AutonomousRunTraceSession): Promise<AutonomousBrainExecution> {
    const { request, route, plan } = prepared;
    if (plan.status === "route_review_required") return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "route_review_required", plan: plan.toJSON(), semantic_route: prepared.semanticRoute, run: null, connector: null, error: null, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    if (plan.status === "connector_review_required" || (prepared.connectorPlan && prepared.connectorPlan.status !== "ready")) return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "connector_blocked", plan: plan.toJSON(), semantic_route: prepared.semanticRoute, run: null, connector: null, error: { error_class: "ConnectorOperationError", failure_code: "configuration" }, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    let connector: AutonomousConnectorOperationExecution | null = null;
    if (request.connector !== undefined && options.connectorFirst !== false) {
      if (!this.connectorOperations || !prepared.connectorPlan) throw new ArgumentError("autonomous brain connector plan is unavailable");
      connector = await this.connectorOperations.executePlanned(
        prepared.connectorPlan,
        request.connector,
        { traceEventCallback: trace === undefined ? undefined : (event) => trace.record(event) },
      );
      if (!connectorSucceeded(connector.status)) return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "connector_blocked", plan: plan.toJSON(), semantic_route: prepared.semanticRoute, run: null, connector, error: { error_class: "ConnectorOperationError", failure_code: connector.status }, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const approved = options.approveProviderCall ?? options.run?.approveProviderCall ?? false;
    const runOptions = { ...(options.run ?? {}), routeOverride: route, ...(prepared.semanticBudget === null ? {} : { costBudget: prepared.semanticBudget, maxTotalCostUnits: undefined }), semanticRouting: undefined, capability: request.capability, context, hints: request.hints, allowCrossDomain: request.allow_cross_domain, approveProviderCall: approved, observer: composeBrainObservers(options.run?.observer, trace?.providerObserver()), selectionEventCallback: trace === undefined ? options.run?.selectionEventCallback : trace.selectionEventCallback(options.run?.selectionEventCallback) } as AutonomousRunOptions;
    const run = route.cross_domain
      ? await this.agent.runCrossDomain(request.task, runOptions as AutonomousCrossDomainRunOptions)
      : await this.agent.run(request.task, { ...runOptions, domain: route.primary_domain ?? undefined });
    return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: run.status, plan: plan.toJSON(), semantic_route: prepared.semanticRoute, run, connector, error: null, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
  }

  private async executeAutoPrepared(prepared: PreparedBrainRequest, options: AutonomousBrainAutoExecuteOptions, trace?: AutonomousRunTraceSession): Promise<AutonomousBrainAutoExecution> {
    const { request, route, plan } = prepared;
    const base = (status: AutonomousBrainAutoExecutionStatus, automatic: AutonomousAutoRunResult | null, connector: AutonomousConnectorOperationExecution | null, error: { error_class: string; failure_code: string } | null): AutonomousBrainAutoExecution => ({
      schema: AUTONOMOUS_BRAIN_AUTO_EXECUTION_SCHEMA,
      status,
      plan: plan.toJSON(),
      semantic_route: prepared.semanticRoute,
      automatic,
      connector,
      error,
      retention: "plan_metadata_only;automatic_and_connector_values_transient_to_caller",
      authorization: "route_review_and_provider_or_effect_approval_remain_explicit",
      secret_material: "never_returned",
    });
    if (plan.status === "route_review_required") return base("route_review_required", null, null, null);
    if (plan.status === "connector_review_required" || (prepared.connectorPlan && prepared.connectorPlan.status !== "ready")) {
      return base("connector_blocked", null, null, { error_class: "ConnectorOperationError", failure_code: "configuration" });
    }
    let connector: AutonomousConnectorOperationExecution | null = null;
    if (request.connector !== undefined && options.connectorFirst !== false) {
      if (!this.connectorOperations || !prepared.connectorPlan) throw new ArgumentError("autonomous brain automatic connector plan is unavailable");
      connector = await this.connectorOperations.executePlanned(
        prepared.connectorPlan,
        request.connector,
        { traceEventCallback: trace === undefined ? undefined : (event) => trace.record(event) },
      );
      if (!connectorSucceeded(connector.status)) return base("connector_blocked", null, connector, { error_class: "ConnectorOperationError", failure_code: connector.status });
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const {
      connectorFirst: _connectorFirst,
      includeConnectorObservation: _includeConnectorObservation,
      semanticRouting: _semanticRouting,
      ...automaticOptions
    } = options;
    const approved = options.approveProviderCall ?? false;
    const runOptions: AutonomousAutoRunOptions = {
      ...automaticOptions,
      routeOverride: route,
      ...(prepared.semanticBudget === null ? {} : { costBudget: prepared.semanticBudget, maxTotalCostUnits: undefined }),
      semanticRouting: undefined,
      domain: route.primary_domain ?? undefined,
      capability: request.capability,
      context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
      approveProviderCall: approved,
      observer: composeBrainObservers(options.observer, trace?.providerObserver()),
      selectionEventCallback: trace === undefined ? options.selectionEventCallback : trace.selectionEventCallback(options.selectionEventCallback),
    };
    const automatic = await this.agent.runAuto(request.task, runOptions);
    return base(automatic.status, automatic, connector, null);
  }

  private async executeCyclePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainCycleOptions, trace?: AutonomousRunTraceSession): Promise<AutonomousBrainCycleExecution> {
    const { request, route, plan } = prepared;
    const base = (status: AutonomousBrainCycleStatus, cycle: AutonomousBrainCycleResult | null, connector: AutonomousConnectorOperationExecution | null, error: { error_class: string; failure_code: string } | null): AutonomousBrainCycleExecution => ({
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status,
      plan: plan.toJSON(),
      semantic_route: prepared.semanticRoute,
      cycle,
      connector,
      error,
      retention: "plan_metadata_only;cycle_response_and_connector_values_transient_to_caller",
      secret_material: "never_returned",
    });
    if (plan.status === "route_review_required") return base("route_review_required", null, null, null);
    if (plan.status === "connector_review_required" || (prepared.connectorPlan && prepared.connectorPlan.status !== "ready")) {
      return base("connector_blocked", null, null, { error_class: "ConnectorOperationError", failure_code: "configuration" });
    }
    if (isObject(options.cycle) && Object.prototype.hasOwnProperty.call(options.cycle, "semanticRouting")) throw new ArgumentError("autonomous brain cycle owns its reviewed route; semanticRouting is not available through executeCycle");
    let connector: AutonomousConnectorOperationExecution | null = null;
    if (request.connector !== undefined && options.connectorFirst !== false) {
      if (!this.connectorOperations || !prepared.connectorPlan) throw new ArgumentError("autonomous brain connector plan is unavailable");
      connector = await this.connectorOperations.executePlanned(
        prepared.connectorPlan,
        request.connector,
        { traceEventCallback: trace === undefined ? undefined : (event) => trace.record(event) },
      );
      if (!connectorSucceeded(connector.status)) return base("connector_blocked", null, connector, { error_class: "ConnectorOperationError", failure_code: connector.status });
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const cycleOptions = {
      ...(options.cycle ?? {}),
      routeOverride: route,
      ...(prepared.semanticBudget === null ? {} : { costBudget: prepared.semanticBudget, maxTotalCostUnits: undefined }),
      semanticRouting: undefined,
      domain: route.primary_domain ?? undefined,
      capability: request.capability,
      context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
      approveProviderCall: options.approveProviderCall ?? options.cycle?.approveProviderCall ?? false,
      observer: composeBrainObservers(options.cycle?.observer, trace?.providerObserver()),
      selectionEventCallback: trace === undefined ? options.cycle?.selectionEventCallback : trace.selectionEventCallback(options.cycle?.selectionEventCallback),
    };
    const cycle = route.cross_domain
      ? await runAutonomousCrossDomainDecisionCycle(this.agent, request.task, cycleOptions as AutonomousCrossDomainDecisionCycleOptions)
      : await runAutonomousDecisionCycle(this.agent, request.task, cycleOptions as AutonomousDecisionCycleOptions);
    return base(cycle.status, cycle, connector, null);
  }

  private async executeAdaptiveCyclePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainAdaptiveCycleOptions, trace?: AutonomousRunTraceSession): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (!options || !isObject(options.adaptive) || typeof options.adaptive.evaluate !== "function") throw new ArgumentError("autonomous brain adaptive cycle requires an evaluator callback");
    const { request, route, plan } = prepared;
    const base = (status: AutonomousBrainAdaptiveCycleStatus, adaptive: AutonomousBrainAdaptiveCycleResult | null, connector: AutonomousConnectorOperationExecution | null, error: { error_class: string; failure_code: string } | null): AutonomousBrainAdaptiveCycleExecution => ({
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status,
      plan: plan.toJSON(),
      semantic_route: prepared.semanticRoute,
      adaptive,
      connector,
      error,
      retention: "plan_metadata_only;adaptive_responses_and_connector_values_transient_to_caller",
      secret_material: "never_returned",
    });
    if (plan.status === "route_review_required") return base("route_review_required", null, null, null);
    if (plan.status === "connector_review_required" || (prepared.connectorPlan && prepared.connectorPlan.status !== "ready")) {
      return base("connector_blocked", null, null, { error_class: "ConnectorOperationError", failure_code: "configuration" });
    }
    if (isObject(options.adaptive) && Object.prototype.hasOwnProperty.call(options.adaptive, "semanticRouting")) throw new ArgumentError("autonomous brain adaptive cycle owns its reviewed route; semanticRouting is not available through executeAdaptiveCycle");
    let connector: AutonomousConnectorOperationExecution | null = null;
    if (request.connector !== undefined && options.connectorFirst !== false) {
      if (!this.connectorOperations || !prepared.connectorPlan) throw new ArgumentError("autonomous brain connector plan is unavailable");
      connector = await this.connectorOperations.executePlanned(
        prepared.connectorPlan,
        request.connector,
        { traceEventCallback: trace === undefined ? undefined : (event) => trace.record(event) },
      );
      if (!connectorSucceeded(connector.status)) return base("connector_blocked", null, connector, { error_class: "ConnectorOperationError", failure_code: connector.status });
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const adaptiveOptions = {
      ...(options.adaptive ?? {}),
      routeOverride: route,
      ...(prepared.semanticBudget === null ? {} : { costBudget: prepared.semanticBudget, maxTotalCostUnits: undefined }),
      semanticRouting: undefined,
      domain: route.primary_domain ?? undefined,
      capability: request.capability,
      context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
      approveProviderCall: options.approveProviderCall ?? options.adaptive.approveProviderCall ?? false,
      observer: composeBrainObservers(options.adaptive.observer, trace?.providerObserver()),
      selectionEventCallback: trace === undefined ? options.adaptive?.selectionEventCallback : trace.selectionEventCallback(options.adaptive?.selectionEventCallback),
    };
    const adaptive = route.cross_domain
      ? await runAutonomousCrossDomainReplanCycle(this.agent, request.task, adaptiveOptions as AutonomousCrossDomainReplanCycleOptions)
      : await runAutonomousReplanCycle(this.agent, request.task, adaptiveOptions as AutonomousReplanCycleOptions);
    return base(adaptive.status, adaptive, connector, null);
  }
}

/**
 * Own the application lifecycle around the metadata-only trace registry projection.
 *
 * A trace journal remains the source of lifecycle events; this controller only indexes validated
 * summaries and bounded event metadata. Restore is mandatory before reads or publication, all
 * mutations are serialized, and a persistence failure is returned separately from a successful
 * in-memory publication so operators never mistake an observability failure for an execution
 * failure or trigger an unsafe provider retry.
 */
export class AutonomousBrainTraceRegistryController {
  private readonly persistenceCoordinator: AutonomousRunTraceRegistryPersistenceCoordinator;
  private restored = false;
  private busy = false;
  private persisted = false;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly registry: AutonomousRunTraceRegistry,
    readonly persistence: JsonAutonomousRunTraceRegistryPersistence,
  ) {
    if (!(brain instanceof AutonomousBrainFacade)) throw new ArgumentError("autonomous brain trace registry controller requires an AutonomousBrainFacade");
    if (!(registry instanceof AutonomousRunTraceRegistry)) throw new ArgumentError("autonomous brain trace registry controller requires an AutonomousRunTraceRegistry");
    if (!(persistence instanceof JsonAutonomousRunTraceRegistryPersistence)) throw new ArgumentError("autonomous brain trace registry controller requires JSON registry persistence");
    this.persistenceCoordinator = new AutonomousRunTraceRegistryPersistenceCoordinator(registry, persistence);
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain trace registry controller must restore before use");
  }

  private requireIdle(): void {
    if (this.busy) throw new ArgumentError("autonomous brain trace registry controller already has an operation in progress");
  }

  private projection(status: AutonomousBrainTraceRegistryControllerStatus): AutonomousBrainTraceRegistryControllerProjection {
    const snapshot = this.registry.snapshot();
    return {
      schema: AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA,
      status,
      snapshot_generation: snapshot.snapshot_generation,
      snapshot_digest: snapshot.snapshot_digest,
      runs: snapshot.record_count,
      events: snapshot.event_count,
      retained_event_count: snapshot.retained_event_count,
      policy: structuredClone(snapshot.policy),
      persisted: this.persisted,
      retention: snapshot.retention,
      authority: snapshot.authority,
      secret_material: snapshot.secret_material,
    };
  }

  /** Restore and validate the last registry snapshot before any operator read or mutation. */
  async restore(): Promise<AutonomousBrainTraceRegistryControllerProjection> {
    this.requireIdle();
    const snapshot = await this.persistenceCoordinator.restore();
    this.restored = true;
    this.persisted = snapshot !== null;
    return this.projection(snapshot === null ? "empty" : "restored");
  }

  /** Flush the verified registry through its caller-owned JSON/CAS persistence adapter. */
  async flush(): Promise<AutonomousBrainTraceRegistryControllerProjection> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      await this.persistenceCoordinator.flush();
      this.persisted = true;
      return this.projection("flushed");
    } finally {
      this.busy = false;
    }
  }

  /** Publish one trace journal into the registry and persist it without coupling to execution. */
  async publish(traceStore: AutonomousRunTraceStore, runId: string): Promise<AutonomousBrainTraceRegistryPublicationRun> {
    this.requireRestored();
    this.requireIdle();
    if (!traceStore || typeof traceStore.snapshot !== "function") throw new ArgumentError("autonomous brain trace registry publication requires a trace store");
    this.busy = true;
    try {
      const publication = await publishAutonomousRunTraceRegistrySnapshot(this.registry, traceStore, runId);
      if (publication.status === "failed") {
        return {
          controller: this.projection("publication_failed"),
          publication,
          persisted: this.persisted,
          persistence_error: null,
        };
      }
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return {
          controller: this.projection("published"),
          publication,
          persisted: true,
          persistence_error: null,
        };
      } catch (error) {
        return {
          controller: this.projection("persistence_failed"),
          publication,
          persisted: false,
          persistence_error: errorProjection(error),
        };
      }
    } finally {
      this.busy = false;
    }
  }

  /** Import a validated trace snapshot, then persist the resulting retention projection. */
  async importSnapshot(raw: unknown): Promise<AutonomousBrainTraceRegistryImportRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      const report = this.registry.importSnapshot(raw);
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return { controller: this.projection("published"), report, persisted: true, persistence_error: null };
      } catch (error) {
        return { controller: this.projection("persistence_failed"), report, persisted: false, persistence_error: errorProjection(error) };
      }
    } finally {
      this.busy = false;
    }
  }

  /** Compact eligible terminal records and persist the new bounded retention projection. */
  async compact(): Promise<AutonomousBrainTraceRegistryCompactRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      const compacted = this.registry.compact();
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return { controller: this.projection("compacted"), evicted_run_ids: compacted.evicted_run_ids, persisted: true, persistence_error: null };
      } catch (error) {
        return { controller: this.projection("persistence_failed"), evicted_run_ids: compacted.evicted_run_ids, persisted: false, persistence_error: errorProjection(error) };
      }
    } finally {
      this.busy = false;
    }
  }

  /** Return one cloned metadata record after restore and idle checks. */
  get(runId: string): AutonomousRunTraceRegistryRecord | null {
    this.requireRestored();
    this.requireIdle();
    return this.registry.get(runId);
  }

  /** Query bounded run summaries and retained event metadata. */
  query(query: AutonomousRunTraceRegistryQuery = {}): AutonomousRunTraceRegistryPage {
    this.requireRestored();
    this.requireIdle();
    return this.registry.query(query);
  }

  /** Query retained lifecycle events without exposing source trace payloads. */
  events(query: AutonomousRunTraceRegistryEventQuery = {}): ReturnType<AutonomousRunTraceRegistry["events"]> {
    this.requireRestored();
    this.requireIdle();
    return this.registry.events(query);
  }

  /** Return the canonical registry snapshot for caller-owned export or inspection. */
  snapshot(): AutonomousRunTraceRegistrySnapshot {
    this.requireRestored();
    this.requireIdle();
    return this.registry.snapshot();
  }

  /** Revalidate every record, digest, count, retention, and snapshot lineage invariant. */
  verifyIntegrity(): AutonomousRunTraceRegistryIntegrity {
    this.requireRestored();
    this.requireIdle();
    return this.registry.verifyIntegrity();
  }
}

export interface AutonomousBrainBatchJobControllerOptions {
  protectedRehydration?: AutonomousBrainBatchProtectedRehydrator;
  automaticProtectedRehydration?: AutonomousBrainAutoBatchProtectedRehydrator;
  automaticCycleProtectedRehydration?: AutonomousBrainAutoCycleBatchProtectedRehydrator;
  automaticReplanProtectedRehydration?: AutonomousBrainAutoReplanBatchProtectedRehydrator;
}

/**
 * Own the process lifecycle around the verified resumable brain batch engine.
 *
 * The facade deliberately accepts a checkpoint sink so infrastructure can choose a database,
 * object store, or journal. This controller is the safer application boundary: startup restore
 * is explicit, only one run may mutate a checkpoint at a time, every checkpoint is validated
 * before it reaches the store, and task text, prompts, provider values, connector observations,
 * and credentials remain transient by construction.
 */
export class AutonomousBrainBatchJobController {
  private checkpoint: AutonomousBrainBatchCheckpointJSON | null = null;
  private restored = false;
  private running = false;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly persistence: AutonomousBrainBatchCheckpointStore,
    readonly options: AutonomousBrainBatchJobControllerOptions = {},
  ) {
    if (!(brain instanceof AutonomousBrainFacade)) throw new ArgumentError("autonomous brain batch controller requires an AutonomousBrainFacade");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous brain batch checkpoint store is malformed");
    if (options.protectedRehydration !== undefined && !(options.protectedRehydration instanceof AutonomousBrainBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller protectedRehydration is malformed");
    if (options.automaticProtectedRehydration !== undefined && !(options.automaticProtectedRehydration instanceof AutonomousBrainAutoBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticProtectedRehydration is malformed");
    if (options.automaticCycleProtectedRehydration !== undefined && !(options.automaticCycleProtectedRehydration instanceof AutonomousBrainAutoCycleBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticCycleProtectedRehydration is malformed");
    if (options.automaticReplanProtectedRehydration !== undefined && !(options.automaticReplanProtectedRehydration instanceof AutonomousBrainAutoReplanBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticReplanProtectedRehydration is malformed");
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain batch controller must restore before execution");
  }

  private requireIdle(): void {
    if (this.running) throw new ArgumentError("autonomous brain batch controller already has a run in progress");
  }

  private projection(status: AutonomousBrainBatchControllerStatus, totalItems: number | null = null, jobId: string | null = this.checkpoint?.job_id ?? null): AutonomousBrainBatchControllerProjection {
    return {
      schema: AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA,
      status,
      job_id: jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      completed_items: this.checkpoint?.completed_indices.length ?? 0,
      total_items: totalItems ?? (this.checkpoint?.request_digests.length ?? null),
      persisted: true,
      retention: "metadata_only_request_and_result_digests;task_prompt_provider_connector_values_never_persisted",
      secret_material: "never_returned",
    };
  }

  /** Restore and verify the last checkpoint before accepting any execution request. */
  async restore(): Promise<AutonomousBrainBatchControllerProjection> {
    this.requireIdle();
    const raw = await this.persistence.read();
    this.checkpoint = raw === null ? null : validateBrainBatchCheckpoint(raw);
    this.restored = true;
    return this.projection(this.checkpoint === null ? "empty" : "restored");
  }

  /** Re-write the last verified checkpoint through the caller-owned store. */
  async flush(): Promise<AutonomousBrainBatchControllerProjection> {
    this.requireRestored();
    this.requireIdle();
    if (this.checkpoint === null) return this.projection("empty");
    const verified = validateBrainBatchCheckpoint(this.checkpoint);
    await this.persistence.write(verified);
    this.checkpoint = verified;
    return this.projection("flushed");
  }

  /** Run a routed/domain/cross-domain batch while the controller owns persistence and restart state. */
  async run(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainBatchControllerRunOptions): Promise<AutonomousBrainBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain batch controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainResumableBatchOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain batch controller owns checkpoint and checkpointSink");
    const rehydrateExecution = options.rehydrateExecution ?? (this.options.protectedRehydration === undefined ? undefined : this.options.protectedRehydration.resolve.bind(this.options.protectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeBatchResumable(inputs, {
        ...options,
        ...(rehydrateExecution === undefined ? {} : { rehydrateExecution }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run a restart-safe automatic batch while sharing the controller's verified checkpoint. */
  async runAutomatic(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainAutoBatchControllerRunOptions): Promise<AutonomousBrainAutoBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic batch controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic batch controller owns checkpoint and checkpointSink");
    const rehydrateExecution = options.rehydrateExecution ?? (this.options.automaticProtectedRehydration === undefined ? undefined : this.options.automaticProtectedRehydration.resolve.bind(this.options.automaticProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoBatchResumable(inputs, {
        ...options,
        ...(rehydrateExecution === undefined ? {} : { rehydrateExecution }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run restart-safe automatic evaluator cycles with controller-owned checkpoint persistence. */
  async runAutomaticCycle(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchControllerRunOptions,
  ): Promise<AutonomousBrainAutoCycleBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic cycle controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoCycleBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic cycle controller owns checkpoint and checkpointSink");
    const rehydrateCycle = options.rehydrateCycle ?? (this.options.automaticCycleProtectedRehydration === undefined ? undefined : this.options.automaticCycleProtectedRehydration.resolve.bind(this.options.automaticCycleProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoCycleBatchResumable(inputs, {
        ...options,
        ...(rehydrateCycle === undefined ? {} : { rehydrateCycle }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run restart-safe automatic evaluator/replan cycles with controller-owned persistence. */
  async runAutomaticReplan(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchControllerRunOptions,
  ): Promise<AutonomousBrainAutoReplanBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic replan controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoReplanBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic replan controller owns checkpoint and checkpointSink");
    const rehydrateReplan = options.rehydrateReplan ?? (this.options.automaticReplanProtectedRehydration === undefined ? undefined : this.options.automaticReplanProtectedRehydration.resolve.bind(this.options.automaticReplanProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoReplanCycleBatchResumable(inputs, {
        ...options,
        ...(rehydrateReplan === undefined ? {} : { rehydrateReplan }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run automatic evaluator cycles with controller-owned checkpoints and one redacted trace. */
  async runAutomaticCycleWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchControllerTraceRunOptions,
  ): Promise<AutonomousBrainAutoCycleBatchControllerTraceRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic cycle traced controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoCycleBatchResumableTraceOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic cycle traced controller owns checkpoint and checkpointSink");
    const rehydrateCycle = options.rehydrateCycle ?? (this.options.automaticCycleProtectedRehydration === undefined ? undefined : this.options.automaticCycleProtectedRehydration.resolve.bind(this.options.automaticCycleProtectedRehydration));
    this.running = true;
    try {
      const traced = await this.brain.executeAutoCycleBatchResumableWithTrace(inputs, {
        ...options,
        ...(rehydrateCycle === undefined ? {} : { rehydrateCycle }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(traced.batch.status, inputs.length, options.jobId), traced };
    } finally {
      this.running = false;
    }
  }

  /** Run automatic evaluator/replan cycles with controller-owned checkpoints and one trace. */
  async runAutomaticReplanWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchControllerTraceRunOptions,
  ): Promise<AutonomousBrainAutoReplanBatchControllerTraceRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic replan traced controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoReplanBatchResumableTraceOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic replan traced controller owns checkpoint and checkpointSink");
    const rehydrateReplan = options.rehydrateReplan ?? (this.options.automaticReplanProtectedRehydration === undefined ? undefined : this.options.automaticReplanProtectedRehydration.resolve.bind(this.options.automaticReplanProtectedRehydration));
    this.running = true;
    try {
      const traced = await this.brain.executeAutoReplanCycleBatchResumableWithTrace(inputs, {
        ...options,
        ...(rehydrateReplan === undefined ? {} : { rehydrateReplan }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(traced.batch.status, inputs.length, options.jobId), traced };
    } finally {
      this.running = false;
    }
  }
}

/** A small verified store useful for local processes, tests, and wiring examples. */
export class InMemoryAutonomousBrainBatchCheckpointStore implements AutonomousBrainBatchCheckpointStore {
  private checkpoint: AutonomousBrainBatchCheckpointJSON | null = null;

  constructor(initial?: AutonomousBrainBatchCheckpointJSON | null) {
    if (initial !== undefined && initial !== null) this.checkpoint = validateBrainBatchCheckpoint(initial);
  }

  read(): AutonomousBrainBatchCheckpointJSON | null {
    return this.checkpoint === null ? null : structuredClone(this.checkpoint);
  }

  write(checkpoint: AutonomousBrainBatchCheckpointJSON): void {
    this.checkpoint = structuredClone(validateBrainBatchCheckpoint(checkpoint));
  }
}

export function createAutonomousBrainFacade(options: { agent: AutonomousAgent; connectorOperations?: AutonomousConnectorOperationFacade }): AutonomousBrainFacade {
  return new AutonomousBrainFacade(options);
}
