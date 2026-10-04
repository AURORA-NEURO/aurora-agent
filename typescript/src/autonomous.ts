import { ArgumentError, CredentialError, ProviderRuntimeError, isObject } from "./errors.js";
import { assertSafeTransientValue, boundedDigest, boundedIdentifier, boundedText, bytes } from "./autonomous-validation.js";
import {
  acceptedAutonomousPlan,
  acceptedCrossDomainPlan,
  validatePlanningWorkflow,
} from "./autonomous-plan-admission.js";
import {
  AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA,
  AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN,
  AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA,
  autonomousCrossDomainExecutionReceipt,
  validateAutonomousCrossDomainExecutionReceipt,
} from "./autonomous-cross-domain-receipt.js";
import type { ProviderErrorCode } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import {
  AUTONOMOUS_DOMAIN_PACK_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_SCHEMA,
  AUTONOMOUS_ROUTE_SCHEMA,
  AUTONOMOUS_WORKFLOW_SCHEMA,
  AUTONOMY_SCHEMA,
  buildDomainPack,
  builtinAutonomousDomainProfiles,
  profileFor,
  routeAutonomousEvidenceScope,
  routeAutonomousTask,
  validateAutonomousRouteOverride,
} from "./autonomous-domain-catalog.js";
import {
  AUTONOMOUS_PROMPT_SCHEMA,
  assembleAutonomousPrompt,
  buildTaskBlueprint,
} from "./autonomous-blueprint.js";
import {
  AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA,
  planningModelProjection,
  planningOutcomeDigest,
  planningProviderFailureDigest,
  planningProviderFailureProjection,
  prepareOrderedStepPlanning,
  prepareProviderPlanning,
  renderAutonomousRunPrompt,
  validateOrderedStepPlanningGraph,
} from "./autonomous-provider-planning.js";
import {
  AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS,
  AUTONOMOUS_MODEL_CATALOGUE_MAX_SNAPSHOT_BYTES,
  AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS,
  AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA,
  AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
  AUTONOMOUS_MODEL_REFRESH_SCHEMA,
  AutonomousModelCataloguePersistenceCoordinator,
  normalizeAutonomousModelCandidate,
  validateAutonomousModelCatalogueSnapshot,
} from "./autonomous-model-catalogue.js";
import {
  AUTONOMOUS_MEMORY_RUN_RETENTION,
  consolidatedMemoryContext,
  memoryEpisodeContext,
  memoryErrorClass,
  memoryIdentity,
  memoryProjection,
  memoryRouteProjection,
  memoryRunStatus,
} from "./autonomous-memory-projection.js";
import {
  AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
  MAX_AUTONOMOUS_EVIDENCE_BACKED_CONTEXT_BYTES,
  MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_CHUNKS,
  MAX_AUTONOMOUS_EVIDENCE_BACKED_RESULT_BYTES,
  defaultEvidenceBackedPromptContext,
  evidenceBackedStatus,
  normalizeAutonomousEvidenceExecutionMode,
  normalizeEvidenceBackedPromptContext,
} from "./autonomous-evidence-backed-run.js";
import type { AutonomousLaunchAdmissionReport } from "./autonomous-launch-admission.js";
import type { ApiClient } from "./client.js";
import { createAutonomousApiToolExecutor } from "./autonomous-api-adapter.js";
import {
  AutonomousCapabilityActivation,
  type AutonomousCapabilityActivationSnapshotStore,
  type AutonomousCapabilityActivationState,
} from "./autonomous-activation.js";
import { AutonomousSelectionPromotionLifecycle } from "./autonomous-selection-lifecycle.js";
import type { AutonomousSelectionLifecycleState, AutonomousSelectionLifecycleStore } from "./autonomous-selection-lifecycle.js";
import type { AutonomousSelectionPromotionReport } from "./autonomous-selection-promotion.js";
import {
  AutonomousBrainControlPlaneBridge,
  AutonomousModelHealthController,
  AutonomousModelHealthPersistenceCoordinator,
  type AutonomousModelHealthSnapshot,
  type AutonomousModelHealthStore,
} from "./autonomous-control.js";
import type {
  AutonomousExecutionController,
  AutonomousExecutionPersistenceCoordinator,
  AutonomousExecutionSnapshotJournal,
} from "./autonomous-execution.js";
import {
  AUTONOMOUS_CAPABILITY_BATCH_SCHEMA,
  AutonomousCapabilityRuntime,
  InMemoryAutonomousCapabilityLearningSettlementStore,
  autonomousCapabilityRefusal,
  settleAutonomousCapabilityLearning,
  settleAutonomousCapabilityLearningBatch,
} from "./autonomous-capabilities.js";
import type {
  AutonomousCapabilityBatchOptions,
  AutonomousCapabilityBatchResult,
  AutonomousCapabilityLearningBatchOptions,
  AutonomousCapabilityLearningBatchResult,
  AutonomousCapabilityLearningOptions,
  AutonomousCapabilityLearningSettlement,
  AutonomousCapabilityLearningSettlementStore,
  AutonomousCapabilityExecutionOptions,
  AutonomousCapabilityExecutionRecord,
  AutonomousCapabilityExecutionRequest,
  AutonomousCapabilityExecutionResult,
} from "./autonomous-capabilities.js";
import type {
  AutonomousCapabilityJournalPersistenceCoordinator,
  AutonomousCapabilityJournalStore,
} from "./autonomous-capability-persistence.js";
import {
  AutonomousToolOutcomeEvaluator,
  type AutonomousToolLearningReport,
} from "./autonomous-tool-evaluation.js";
import {
  AutonomousProviderOutcomeEvaluator,
  type AutonomousProviderLearningReport,
  type AutonomousProviderLearningUpdater,
  type AutonomousProviderOutcomeContext,
} from "./autonomous-provider-evaluation.js";
import type { AutonomousDecisionCyclePersistenceCoordinator } from "./autonomous-decision-persistence.js";
import {
  autonomousRunTraceStatus,
  AutonomousRunTraceSession,
  type AutonomousRunTraceStore,
  type AutonomousRunTraceSummary,
} from "./autonomous-run-trace.js";
import {
  analyzeAutonomousRunTrace,
  type AutonomousRunTraceAnalyticsPolicy,
  type AutonomousRunTraceAnalyticsReport,
} from "./autonomous-run-analytics.js";
import {
  AutonomousRunAnalyticsLedger,
  type AutonomousRunAnalyticsLedgerPolicy,
} from "./autonomous-run-analytics-ledger.js";
import {
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  type AutonomousConnectorDispatchRequest,
  type AutonomousConnectorDispatchResult,
  type AutonomousConnectorSelectionPlan,
  type AutonomousConnectorTraceEventCallback,
} from "./autonomous-connectors.js";
import { AutonomousEffectBoundary, AutonomousEffectReconciliationRequiredError, type AutonomousEffectExecutionContext } from "./autonomous-effects.js";
import { AutonomousAuthorizationError, type AutonomousAuthorizationContext } from "./autonomous-authorization.js";
import type { AutonomousLearningController } from "./autonomous-learning.js";
import type { AutonomousEvaluatorCalibrationReport } from "./autonomous-evaluator-calibration.js";
import type {
  AutonomousEvaluatorCalibrationImport,
  AutonomousEvaluatorCalibrationQueryOptions,
  AutonomousEvaluatorCalibrationRegistry,
  AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator,
  AutonomousEvaluatorCalibrationStoreSnapshot,
} from "./autonomous-evaluator-calibration-store.js";
import type { AutonomousModelInventoryReadiness, AutonomousModelInventoryReadinessOptions, AutonomousModelInventoryRefreshOptions, AutonomousModelInventorySnapshot } from "./autonomous-model-inventory.js";
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
import type {
  AutonomousWorkflowPortfolioAdmission,
  AutonomousWorkflowPortfolioAdmissionOptions,
} from "./autonomous-workflow-portfolio-admission.js";
import type {
  AutonomousWorkflowPortfolioResumableExecutionOptions,
} from "./autonomous-workflow-portfolio-resumable.js";
import type {
  AutonomousWorkflowPortfolioEvidenceExecutionResult,
  AutonomousWorkflowPortfolioEvidenceSupervisorOptions,
} from "./autonomous-workflow-portfolio-evidence.js";
import type {
  AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions,
} from "./autonomous-workflow-portfolio-evidence-resumable.js";
import { taskFacetDigests } from "./autonomous-memory.js";
import {
  AutonomousPromptLearningPersistenceCoordinator,
  extractAutonomousPromptLearningSelections,
} from "./autonomous-prompt-learning-persistence.js";
import type {
  AutonomousOnlineLearnerPersistenceCoordinator,
  AutonomousOnlineLearnerSnapshot,
} from "./autonomous-online-learner-persistence.js";
import {
  AutonomousToolSelectionPersistenceCoordinator,
  type AutonomousToolSelectionPersistence,
  type AutonomousToolSelectionSnapshot,
} from "./autonomous-tool-selection-persistence.js";
import { buildAutonomousEvidencePlan, type AutonomousEvidencePlan, type AutonomousEvidencePlanJSON } from "./autonomous-evidence.js";
import {
  AutonomousEvidenceRuntime,
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceRuntimeExecuteOptions,
  type AutonomousEvidenceRuntimeJournal,
  type AutonomousEvidenceRuntimeResult,
} from "./autonomous-evidence-runtime.js";
import type { AutonomousEvidenceAdapterRegistry } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceAdapterHealthStore } from "./autonomous-evidence-adapter-health.js";
import type { AutonomousEvidenceProviderContractRegistry } from "./autonomous-evidence-provider-contract.js";
import type { AutonomousEvidenceReadinessAuditOptions } from "./autonomous-evidence-readiness.js";
import type {
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceExecutionOptions,
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionPrepareOptions,
  AutonomousEvidenceExecutionResult,
} from "./autonomous-evidence-execution.js";
import type {
  AutonomousMissionCheckpointStore,
  AutonomousMissionExecuteOptions,
  AutonomousMissionExecutionResult,
  AutonomousMissionResultStore,
  AutonomousMissionStepResult,
} from "./mission-execution.js";
import type {
  AutonomousMissionReplanOptions,
  AutonomousMissionReplanPromptLearningProjection,
  AutonomousMissionReplanResult,
} from "./mission-replan.js";
import type {
  AutonomousConnectorMissionAgentRunOptions,
  AutonomousConnectorPlannedMissionRun,
  AutonomousConnectorMissionProviderPlanningOptions,
} from "./autonomous-connector-mission.js";
import type {
  AutonomousEvidenceExecutionCheckpointStore,
  AutonomousEvidenceExecutionReconciliationAuthorityIdentity,
  AutonomousEvidenceExecutionReconciliationReceiptJSON,
  AutonomousEvidenceExecutionResumablePolicyIdentity,
  AutonomousEvidenceExecutionResumableOptions,
  AutonomousEvidenceExecutionResumableRun,
} from "./autonomous-evidence-execution-resumable.js";
import {
  planAutonomousInformationAcquisition,
  type AutonomousInformationAcquisitionCandidate,
  type AutonomousInformationAcquisitionPlan,
  type AutonomousInformationAcquisitionPolicy,
  type AutonomousInformationAcquisitionPolicyInput,
} from "./autonomous-information-acquisition.js";
import type { AutonomousInformationAcquisitionCandidateInput } from "./autonomous-information-acquisition.js";
import {
  assessAutonomousClaimIntegrity,
  bindAutonomousClaimIntegrityAcquisitionRequests,
  planAutonomousClaimIntegrityAcquisition,
  reassessAutonomousClaimIntegrity,
  settleAutonomousClaimIntegrityAcquisition,
  type AssessAutonomousClaimIntegrityOptions,
  type AutonomousClaimIntegrityAssessment,
  type AutonomousClaimIntegrityClaim,
  type AutonomousClaimIntegrityClaimInput,
  type AutonomousClaimIntegrityEvidence,
  type AutonomousClaimIntegrityEvidenceInput,
  type AutonomousClaimIntegrityPolicy,
  type AutonomousClaimIntegrityPolicyInput,
} from "./autonomous-claim-integrity.js";
import type { AutonomousClaimIntegrityAcquisitionBinding, AutonomousClaimIntegrityAcquisitionRequestInput, PlanAutonomousClaimIntegrityAcquisitionOptions, SettleAutonomousClaimIntegrityAcquisitionOptions } from "./autonomous-claim-integrity.js";
import {
  assessAutonomousOutcomeIntegrity,
  bindAutonomousOutcomeIntegrityClaims,
  projectAutonomousOutcomeIntegrityRun,
  type AssessAutonomousOutcomeIntegrityOptions,
  type AutonomousOutcomeIntegrityAssessment,
  type AutonomousOutcomeIntegrityClaimBinding,
  type AutonomousOutcomeIntegrityClaimBindingInput,
  type AutonomousOutcomeIntegrityRun,
} from "./autonomous-outcome-integrity.js";
import {
  assessAutonomousCrossDomainResponseSet,
  type AutonomousCrossDomainResponseAssessment,
  type AutonomousCrossDomainResponseAlignmentInput,
  type AutonomousCrossDomainResponseEntry,
} from "./autonomous-cross-domain-response.js";
import type {
  AutonomousDomainEvidenceBrainRunOptions,
  AutonomousDomainEvidenceBrainRunResult,
} from "./autonomous-domain-evidence-brain.js";
import type {
  AutonomousEpisodicMemoryStore,
  AutonomousMemoryPersistenceCoordinator,
  AutonomousMemorySnapshot,
  AutonomousMemoryEpisode,
  AutonomousMemoryQuery,
  AutonomousMemoryReceipt,
} from "./autonomous-memory.js";
import type {
  AutonomousMemoryConsolidationObservation,
  AutonomousMemoryConsolidationPromptReference,
  AutonomousMemoryLessonContextResolver,
  AutonomousMemoryConsolidationReport,
  AutonomousMemoryConsolidator,
} from "./autonomous-memory-consolidation.js";
import {
  runAutonomousAutoDecisionCycle,
  runAutonomousAutoReplanCycle,
  runAutonomousCrossDomainReplanCycle,
  runAutonomousReplanCycle,
  type AutonomousAutoDecisionCycleOptions,
  type AutonomousAutoDecisionCycleResult,
  type AutonomousAutoReplanCycleOptions,
  type AutonomousAutoReplanCycleResult,
  type AutonomousCrossDomainReplanCycleOptions,
  type AutonomousCrossDomainReplanCycleResult,
  type AutonomousReplanCycleOptions,
  type AutonomousReplanCycleResult,
} from "./autonomous-cycle.js";
import {
  AUTONOMOUS_GOAL_RETENTION,
  AUTONOMOUS_GOAL_STEP_SCHEMA,
  InMemoryAutonomousGoalLedger,
  goalStatusForResult,
  goalTaskDigest,
  type AutonomousGoalCriterion,
  type AutonomousGoalRecord,
  type AutonomousGoalSettlementMetadata,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import {
  AutonomousRuntime,
  AutonomousCostBudget,
  type AutonomousCostBudgetSnapshot,
  type AutonomousModelCandidate,
  type AutonomousModelObservation,
  type AutonomousModelRanking,
  type AutonomousSelectionWeights,
  normalizeAutonomousSelectionWeights,
  normalizeAutonomousModelObservations,
  type AutonomousModelSelector,
  type AutonomousModelSelectionTraceEventCallback,
  type AutonomousSelectionDecision,
  type AutonomousSelectionRequest,
  type AutonomousExecutionPlan,
  type CredentialHandle,
  type ProviderInvocationObserver,
  type ProviderTransportDispatchContext,
  type ProviderTransportDispatchFence,
  type ProviderContentPart,
  type ProviderMessage,
  type ProviderRequest,
  type ProviderResponse,
  type ProviderTool,
  type ProviderToolCall,
  type ProviderToolResult,
  LLMRuntime,
  normalizeProviderContentParts,
  providerTextPart,
  providerModelsToCandidates,
  rankAutonomousModels,
  autonomousSelectionConfidence,
  type AutonomousModelCandidateDefaults,
  type ProviderModelDiscovery,
  type AutonomousProviderFailoverProjection,
  type AutonomousProviderInvocationReceipt,
  type AutonomousStreamCompletion,
  type AutonomousStreamHandle,
  type AutonomousStreamInvocationOptions,
  type ProviderStreamEvent,
  LLMRuntimeHealthPersistenceCoordinator,
  type LLMRuntimeHealthSnapshot,
} from "./llm.js";
import type { AutonomousModelContinuationPlan } from "./autonomous-continuation.js";
import { normalizeAutonomousContextBudget, type AutonomousContextBudgetOptions, type AutonomousContextBudgetPlan } from "./autonomous-context-budget.js";
import {
  buildAutonomousDomainResponseContract,
  evaluateAutonomousDomainResponse,
  validateAutonomousProviderDomainResponse,
} from "./autonomous-domain-response.js";
import type { AutonomousDomainResponseContract, AutonomousDomainResponseEvaluation } from "./autonomous-domain-response.js";
import {
  autonomousDomainTaskLens,
  autonomousTaskLensPromptContract,
  validateAutonomousDomainTaskLens,
  type AutonomousDomainTaskLens,
} from "./autonomous-task-lens.js";
import {
  autonomousTaskIntentPromptContract,
  inferAutonomousTaskIntent,
  validateAutonomousTaskIntent,
  type AutonomousTaskIntent,
} from "./autonomous-task-intent.js";
import {
  routeAutonomousCapability,
  type AutonomousCapabilityRoute,
} from "./autonomous-capability-routing.js";
import {
  autonomousTaskDecisionPromptContract,
  inferAutonomousTaskDecision,
  validateAutonomousTaskDecision,
  type AutonomousTaskDecision,
} from "./autonomous-task-decision.js";
import {
  AUTONOMOUS_TASK_CLARIFICATION_RECOMPILE_SCHEMA,
  planAutonomousTaskClarification,
  resolveAutonomousTaskClarification,
  validateAutonomousTaskClarificationPlan,
  validateAutonomousTaskClarificationRecompile,
  validateAutonomousTaskClarificationResolution,
  type AutonomousTaskClarificationPlan,
  type AutonomousTaskClarificationResolution,
} from "./autonomous-task-clarification.js";
import {
  semanticRouteAutonomousTask,
  type AutonomousSemanticRouteOptions,
  type AutonomousSemanticRouteResult,
} from "./autonomous-routing.js";
import { ToolCatalogue, canonicalJson, digestBytesSync, digestCanonicalJsonTextSync, digestJson, digestJsonSync } from "./tooling.js";
import {
  AUTONOMOUS_TOOL_RISK_ORDER,
  AUTONOMOUS_TOOL_SELECTION_POLICY,
  AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_CONTRACT_SCHEMA,
  MAX_AUTONOMOUS_TOOL_SELECTION_ARMS,
  MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE,
  MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS,
  WORKFLOW_CAPABILITY_ALIASES,
  autonomousDomainToolBindingSupportsStage as bindingSupportsStage,
  autonomousToolRiskAllowed,
  autonomousToolSelectionArmId,
  autonomousWorkflowStageContractDigest,
  boundedToolSelectionNumber,
  capabilityCandidateRanking,
  capabilityCandidateScore,
  compareCapabilityScores,
  normalizeAutonomousToolSelectionState,
  settleAutonomousToolSelectionOutcome,
  taskRelevanceTokens,
  toolSelectionArmFor,
  toolSelectionUtility,
} from "./autonomous-tool-selection.js";
import { normalizeRouteText, termMatches } from "./autonomous-text-normalization.js";
import { AUTONOMOUS_PLAN_SCHEMA, compileAutonomousPlan } from "./autonomous-plan-compiler.js";
import {
  AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA,
  AutonomousDomainToolRegistryBase,
} from "./autonomous-domain-tool-registry.js";
import {
  AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA,
  AutonomousDomainToolRuntime,
} from "./autonomous-domain-tool-runtime.js";
import {
  AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
  MAX_AUTONOMOUS_WORKFLOW_STAGE_PLAN_BYTES,
  compileAutonomousWorkflowStageExecutionPlan,
  validateAutonomousWorkflowStageExecutionPlan,
} from "./autonomous-workflow-stage-plan.js";
import {
  autonomousDomainPolicy,
  evaluateAutonomousDomainPolicy,
  validateAutonomousDomainPolicy,
  type AutonomousDomainPolicy,
  type AutonomousDomainPolicyAdmission,
  type AutonomousDomainPolicyExecutionMode,
  type AutonomousDomainPolicyOverrides,
} from "./autonomous-domain-policy.js";
import type {
  BrainBanditArm,
  BrainBanditContext,
  BrainBanditContextState,
  BrainBanditPolicy,
  BrainBanditState,
  BrainBanditUpdate,
  BrainContextualModelSelectionResult,
  BrainModelDescriptor,
  BrainModelSelectionArgs,
  BrainProviderHealth,
  AgentMissionArgs,
  AgentMissionStep,
  JsonObject,
  JsonValue,
  AutonomousCrossDomainPlanRefinementResult,
  AutonomousOrderedStepPlanRefinementResult,
  AutonomousPlanRefinementResult,
  RestToolResponse,
  ToolDefinition,
} from "./types.js";
import {
  AutonomousPromptAdaptiveSelection,
  AutonomousPromptRegistry,
  AutonomousPromptTemplate,
  type AutonomousPromptAdaptiveSelectionJSON,
  type AutonomousPromptLearningState,
  type AutonomousPromptLearningStateJSON,
  type AutonomousPromptSelectionPlan,
  type AutonomousPromptSelectionPlanJSON,
} from "./autonomous-prompt-registry.js";
import type {
  AutonomousRouteReason,
  AutonomousWorkflowStage,
  AutonomousWorkflowToolContext,
  AutonomousWorkflow,
  AutonomousDomainToolBinding,
  AutonomousToolRiskClass,
  AutonomousDomainToolExecutionReceipt,
  AutonomousDomainToolProfile,
  AutonomousDomainProfile,
  AutonomousRouteCandidate,
  AutonomousRouteProposal,
  AutonomousPromptChunk,
  AutonomousPromptMessage,
  AutonomousPromptResult,
  AutonomousPlanStep,
  AutonomousPlan,
  AutonomousDomainPack,
  AutonomousReadinessState,
  AutonomousReadinessModel,
  AutonomousReadinessProvider,
  AutonomousReadinessDomain,
  AutonomousReadinessReport,
  AutonomousModelSelectionPreviewStatus,
  AutonomousModelSelectionPreviewOptions,
  AutonomousModelSelectionContract,
  AutonomousApprovedModelSelectionOptions,
  AutonomousModelSelectionPreview,
  AutonomousTaskBlueprint,
  AutonomousClarificationRecompileProjection,
  AutonomousClarificationRecompileResult,
  AutonomousCapabilityContract,
  AutonomousWorkflowStageExecutionPlan,
  AutonomousCrossDomainSubtask,
  AutonomousCrossDomainBlueprint,
  AutonomousAutoBlueprint,
  AutonomousDomainToolCoverage,
  AutonomousDomainToolPlan,
  AutonomousCapabilitySelectionStatus,
  AutonomousToolSelectionArm,
  AutonomousToolSelectionCredit,
  AutonomousToolSelectionState,
  AutonomousToolSelectionOutcome,
  AutonomousAgentCapabilityLearningResult,
  AutonomousAgentCapabilityLearningBatchResult,
  AutonomousCapabilityPlanCoverage,
  AutonomousCapabilityPlanOmission,
  AutonomousCapabilityCandidateReason,
  AutonomousCapabilityCandidateRanking,
  AutonomousCapabilityPlan,
  AutonomousRunStatus,
  AutonomousProviderFailureProjection,
  AutonomousToolLoopStatus,
  AutonomousToolLoopSummary,
  AutonomousRunResult,
  AutonomousRunPromptProjection,
  AutonomousMemoryRunProjection,
  AutonomousEvidenceBackedRunStatus,
  AutonomousEvidenceExecutionMode,
  AutonomousEvidencePromptProjection,
  AutonomousEvidencePromptBuilder,
  AutonomousEvidenceBackedRunPreflight,
  AutonomousEvidenceBackedRunPreflightHook,
  AutonomousEvidenceBackedProviderDispatchHook,
  AutonomousEvidenceBackedRunOptions,
  AutonomousEvidenceBackedRunProjection,
  AutonomousEvidenceBackedRunResult,
  AutonomousGoalStepResult,
  AutonomousGoalLearningStepResult,
  AutonomousCrossDomainChildRun,
  AutonomousCrossDomainRunStatus,


  AutonomousCrossDomainRunResult,
  AutonomousAgentOptions,
  AutonomousReviewedEvidenceExecutionOptions,
  AutonomousReviewedEvidenceResumableExecutionOptions,
  AutonomousReviewedEvidencePreparationOptions,
  AutonomousProviderPlanningOptions,
  AutonomousOrderedStepPlanStep,
  AutonomousOrderedStepPlanRequest,
  AutonomousModelRefreshResult,
  AutonomousModelRefreshSpec,
  AutonomousModelRefreshFailure,
  AutonomousModelCatalogueRefreshResult,
  AutonomousModelCatalogueSnapshot,
  AutonomousModelCataloguePersistence,
  AutonomousRunSemanticRoutingOptions,
  AutonomousRunOptions,
  AutonomousCrossDomainRunOptions,
  AutonomousAgentMissionReplanOptions,
  AutonomousRunWithTraceOptions,
  AutonomousTracedRunResult,
  AutonomousTracedCrossDomainRunResult,
  DomainToolExecutor,
  DomainToolApprover,
  AutonomousPlanAndRunStatus,
  AutonomousPlanAndRunOptions,
  AutonomousPlanAndRunResult,
  AutonomousAutoPlanningMode,
  AutonomousAutoRunOptions,
  AutonomousAutoRunNextAction,
  AutonomousAutoRunResult,
  AutonomousRunStreamEvent,
  AutonomousRunStreamCompletion,
  AutonomousRunStreamHandle,
  AutonomousMemoryPreparation,
  ProfileSeed,
  WorkflowStageDefinition,
  WorkflowDefinition,

  RenderedAutonomousRunPrompt,
  PreparedProviderPlanning,
  AutonomousAcceptedCrossDomainPlan,
  AutonomousAcceptedPlan,
} from "./autonomous-agent-contracts.js";
import { AutonomousOnlineLearner, contextualSelector } from "./autonomous-online-learning.js";

export { AUTONOMOUS_PROMPT_SCHEMA, assembleAutonomousPrompt };
export { AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA };
export { AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA, MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_CHUNKS, MAX_AUTONOMOUS_EVIDENCE_BACKED_CONTEXT_BYTES, MAX_AUTONOMOUS_EVIDENCE_BACKED_RESULT_BYTES };
export { AUTONOMOUS_MODEL_REFRESH_SCHEMA, AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA, AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA, AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS, AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS, AUTONOMOUS_MODEL_CATALOGUE_MAX_SNAPSHOT_BYTES, AutonomousModelCataloguePersistenceCoordinator, validateAutonomousModelCatalogueSnapshot };

export {
  AUTONOMOUS_DOMAIN_PACK_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_SCHEMA,
  AUTONOMOUS_ROUTE_SCHEMA,
  AUTONOMOUS_WORKFLOW_SCHEMA,
  AUTONOMY_SCHEMA,
  builtinAutonomousDomainProfiles,
  routeAutonomousEvidenceScope,
  routeAutonomousTask,
  validateAutonomousRouteOverride,
};

export { AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA, AutonomousDomainToolRuntime };

export {
  AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
  AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA,
};

export { AUTONOMOUS_PLAN_SCHEMA, compileAutonomousPlan };

export {
  AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
  MAX_AUTONOMOUS_WORKFLOW_STAGE_PLAN_BYTES,
  compileAutonomousWorkflowStageExecutionPlan,
  validateAutonomousWorkflowStageExecutionPlan,
};

export {
  AUTONOMOUS_TOOL_RISK_ORDER,
  AUTONOMOUS_TOOL_SELECTION_POLICY,
  AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_CONTRACT_SCHEMA,
  MAX_AUTONOMOUS_TOOL_SELECTION_ARMS,
  MAX_AUTONOMOUS_TOOL_SELECTION_CANDIDATES_PER_STAGE,
  MAX_AUTONOMOUS_TOOL_SELECTION_CREDITS,
  autonomousToolSelectionArmId,
  autonomousWorkflowStageContractDigest,
  normalizeAutonomousToolSelectionState,
  settleAutonomousToolSelectionOutcome,
};
export { bindingSupportsStage as autonomousDomainToolBindingSupportsStage };

export {
  AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA,
  AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN,
  AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA,
  autonomousCrossDomainExecutionReceipt,
  validateAutonomousCrossDomainExecutionReceipt,
} from "./autonomous-cross-domain-receipt.js";

export {
  acceptedAutonomousPlan,
  acceptedCrossDomainPlan,
} from "./autonomous-plan-admission.js";

export type {
  AutonomousRouteReason,
  AutonomousWorkflowStage,
  AutonomousWorkflowToolContext,
  AutonomousWorkflow,
  AutonomousDomainToolBinding,
  AutonomousToolRiskClass,
  AutonomousDomainToolExecutionReceipt,
  AutonomousDomainToolProfile,
  AutonomousDomainProfile,
  AutonomousRouteCandidate,
  AutonomousRouteProposal,
  AutonomousPromptChunk,
  AutonomousPromptMessage,
  AutonomousPromptResult,
  AutonomousPlanStep,
  AutonomousPlan,
  AutonomousDomainPack,
  AutonomousReadinessState,
  AutonomousReadinessModel,
  AutonomousReadinessProvider,
  AutonomousReadinessDomain,
  AutonomousReadinessReport,
  AutonomousModelSelectionPreviewStatus,
  AutonomousModelSelectionPreviewOptions,
  AutonomousModelSelectionContract,
  AutonomousApprovedModelSelectionOptions,
  AutonomousModelSelectionPreview,
  AutonomousTaskBlueprint,
  AutonomousClarificationRecompileProjection,
  AutonomousClarificationRecompileResult,
  AutonomousCapabilityContract,
  AutonomousWorkflowStageExecutionPlan,
  AutonomousCrossDomainSubtask,
  AutonomousCrossDomainBlueprint,
  AutonomousAutoBlueprint,
  AutonomousDomainToolCoverage,
  AutonomousDomainToolPlan,
  AutonomousCapabilitySelectionStatus,
  AutonomousToolSelectionArm,
  AutonomousToolSelectionCredit,
  AutonomousToolSelectionState,
  AutonomousToolSelectionOutcome,
  AutonomousAgentCapabilityLearningResult,
  AutonomousAgentCapabilityLearningBatchResult,
  AutonomousCapabilityPlanCoverage,
  AutonomousCapabilityPlanOmission,
  AutonomousCapabilityCandidateReason,
  AutonomousCapabilityCandidateRanking,
  AutonomousCapabilityPlan,
  AutonomousRunStatus,
  AutonomousProviderFailureProjection,
  AutonomousToolLoopStatus,
  AutonomousToolLoopSummary,
  AutonomousRunResult,
  AutonomousRunPromptProjection,
  AutonomousMemoryRunProjection,
  AutonomousEvidenceBackedRunStatus,
  AutonomousEvidenceExecutionMode,
  AutonomousEvidencePromptProjection,
  AutonomousEvidencePromptBuilder,
  AutonomousEvidenceBackedRunPreflight,
  AutonomousEvidenceBackedRunPreflightHook,
  AutonomousEvidenceBackedProviderDispatchHook,
  AutonomousEvidenceBackedRunOptions,
  AutonomousEvidenceBackedRunProjection,
  AutonomousEvidenceBackedRunResult,
  AutonomousGoalStepResult,
  AutonomousGoalLearningStepResult,
  AutonomousCrossDomainChildRun,
  AutonomousCrossDomainRunStatus,
  AutonomousCrossDomainExecutionNextAction,
  AutonomousCrossDomainExecutionReceipt,
  AutonomousCrossDomainRunResult,
  AutonomousAgentOptions,
  AutonomousReviewedEvidenceExecutionOptions,
  AutonomousReviewedEvidenceResumableExecutionOptions,
  AutonomousReviewedEvidencePreparationOptions,
  AutonomousProviderPlanningOptions,
  AutonomousOrderedStepPlanStep,
  AutonomousOrderedStepPlanRequest,
  AutonomousModelRefreshResult,
  AutonomousModelRefreshSpec,
  AutonomousModelRefreshFailure,
  AutonomousModelCatalogueRefreshResult,
  AutonomousModelCatalogueSnapshot,
  AutonomousModelCataloguePersistence,
  AutonomousRunSemanticRoutingOptions,
  AutonomousRunOptions,
  AutonomousCrossDomainRunOptions,
  AutonomousAgentMissionReplanOptions,
  AutonomousRunWithTraceOptions,
  AutonomousTracedRunResult,
  AutonomousTracedCrossDomainRunResult,
  DomainToolExecutor,
  DomainToolApprover,
  AutonomousPlanAndRunStatus,
  AutonomousPlanAndRunOptions,
  AutonomousPlanAndRunResult,
  AutonomousAutoPlanningMode,
  AutonomousAutoRunOptions,
  AutonomousAutoRunNextAction,
  AutonomousAutoRunResult,
  AutonomousRunStreamEvent,
  AutonomousRunStreamCompletion,
  AutonomousRunStreamHandle,
  AutonomousAcceptedCrossDomainPlan,
  AutonomousAcceptedPlan,
} from "./autonomous-agent-contracts.js";



/** Cross-domain orchestration contracts shared with the Python autonomous façade. */
export const AUTONOMOUS_PLAN_REFINEMENT_SCHEMA = "bioprism-python-autonomous-plan-refinement/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_PLAN_REFINEMENT_SCHEMA = "bioprism-python-autonomous-cross-domain-plan-refinement/0.1" as const;
export const AUTONOMOUS_PLAN_AND_RUN_SCHEMA = "bioprism-typescript-autonomous-plan-and-run/0.1" as const;
export const AUTONOMOUS_AUTO_RUN_SCHEMA = "bioprism-typescript-autonomous-auto-run/0.1" as const;
export const AUTONOMOUS_RUN_STREAM_SCHEMA = "bioprism-typescript-autonomous-run-stream/0.1" as const;
export const AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA = "bioprism-typescript-autonomous-run-stream-completion/0.1" as const;
export const AUTONOMOUS_RUN_STREAM_MAX_QUEUED_EVENTS = 4_096;
/** Cross-SDK stage execution packet schema; intentionally matches the Python façade. */
export const AUTONOMOUS_LEARNING_SCHEMA = "bioprism-typescript-autonomous-online-learning/0.1" as const;
export const AUTONOMOUS_GOAL_LEARNING_SCHEMA = "bioprism-typescript-autonomous-goal-learning/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-cross-domain/0.1" as const;
export const AUTONOMOUS_READINESS_SCHEMA = "bioprism-autonomous-agent-readiness/0.1" as const;
export const AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA = "bioprism-typescript-autonomous-model-selection-preview/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_MAX_CONCURRENCY = 4;
export const MAX_AUTONOMOUS_MODEL_SELECTION_PREVIEW_BYTES = 250_000;

export { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
export type { AutonomousDomainName } from "./autonomous-domains.js";


function providerFailureRunResult(
  parentRoute: AutonomousRouteProposal,
  blueprint: AutonomousTaskBlueprint,
  error: ProviderRuntimeError | CredentialError,
  learning: AutonomousRunResult["learning"],
): AutonomousRunResult {
  const failure: AutonomousProviderFailureProjection = {
    error_class: error instanceof CredentialError ? "CredentialError" : "ProviderRuntimeError",
    code: error instanceof CredentialError ? "credential" : error.code,
    retryable: error instanceof CredentialError ? false : error.retryable,
    status_code: error instanceof CredentialError ? null : error.statusCode ?? null,
    circuit_open: error instanceof CredentialError ? false : error.circuitOpen,
    retention: "metadata_only;provider_error_message_and_payloads_not_retained",
    secret_material: "never_returned",
  };
  return {
    schema: "bioprism-typescript-autonomous-run/0.1",
    status: "child_failed",
    route: parentRoute,
    blueprint,
    plan_refinement_digest: null,
    selection: null,
    response: null,
    provider_invocations: [],
    provider_failover: null,
    context_budget: null,
    continuation_plan: null,
    prompt: null,
    response_evaluation: null,
    tool_loop: null,
    cross_domain: null,
    learning_episode_id: null,
    learning_episode_status: "not_eligible",
    learning_error_class: null,
    response_learning_episode_id: null,
    response_learning_episode_status: "not_eligible",
    response_learning_error_class: null,
    learning,
    failure,
    retention: "provider_response_local; value_only_learning_projection",
  };
}

async function goalLearningSettlementProjection(value: unknown): Promise<JsonObject> {
  if (!isObject(value)) return {};
  const assessment: JsonObject | null = isObject(value.assessment)
    ? Object.fromEntries(Object.entries(value.assessment).filter(([key]) => ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "failure_class", "feedback_digest", "evidence_digest"].includes(key))) as JsonObject
    : null;
  const nextState = isObject(value.next_state) ? { next_state_digest: await digestJson(value.next_state) } : {};
  if (isObject(value.trajectory)) {
    const trajectory = value.trajectory;
    const settlements = Array.isArray(trajectory.settlements)
      ? await Promise.all(trajectory.settlements.map((item) => goalLearningSettlementProjection(item)))
      : [];
    const responseSettlements = Array.isArray(value.response_settlements)
      ? await Promise.all(value.response_settlements.map((item) => goalLearningSettlementProjection(item)))
      : [];
    return {
      trajectory_digest: typeof trajectory.trajectory_digest === "string" ? trajectory.trajectory_digest : null,
      settlement_digest: typeof trajectory.settlement_digest === "string" ? trajectory.settlement_digest : null,
      settlements,
      ...(Array.isArray(value.response_settlements) ? { response_settlements: responseSettlements } : {}),
    };
  }
  const episode = isObject(value.episode) ? value.episode : null;
  return {
    episode_id: episode && typeof episode.episode_id === "string" ? episode.episode_id : null,
    assessment,
    ...nextState,
  };
}

const AUTONOMOUS_RUN_SEMANTIC_ROUTING_FIELDS = new Set([
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
  "domainPolicyMode",
  "domainPolicyEvidenceReady",
  "domainPolicyEvaluatorConfigured",
  "domainPolicyEffectsRequested",
  "domainPolicyEffectsApproved",
]);

function normalizeRunSemanticRouting(value: AutonomousRunOptions["semanticRouting"]): AutonomousRunSemanticRoutingOptions | null {
  if (value === undefined || value === false) return null;
  if (value === true) return {};
  if (!isObject(value)) throw new ArgumentError("semanticRouting must be a boolean or object");
  const unsupported = Object.keys(value).find((key) => !AUTONOMOUS_RUN_SEMANTIC_ROUTING_FIELDS.has(key));
  if (unsupported) throw new ArgumentError(`semanticRouting contains unsupported field: ${unsupported}`);
  return value as unknown as AutonomousRunSemanticRoutingOptions;
}

function semanticRouteRunStatus(status: AutonomousSemanticRouteResult["status"]): AutonomousRunStatus {
  if (status === "approval_required") return "approval_required";
  if (status === "policy_review_required") return "policy_review_required";
  if (status === "policy_blocked") return "policy_blocked";
  return "route_review_required";
}

function semanticRouteCrossDomainStatus(status: AutonomousSemanticRouteResult["status"]): AutonomousCrossDomainRunStatus {
  if (status === "approval_required") return "approval_required";
  if (status === "policy_review_required") return "policy_review_required";
  if (status === "policy_blocked") return "policy_blocked";
  return "route_review_required";
}

function composeInvocationObservers(...observers: readonly (ProviderInvocationObserver | undefined)[]): ProviderInvocationObserver | undefined {
  const active = observers.filter((observer): observer is ProviderInvocationObserver => observer !== undefined);
  if (!active.length) return undefined;
  return {
    before: async (metadata) => {
      for (const observer of active) await observer.before?.(metadata);
    },
    dispatch: async (metadata) => {
      for (const observer of active) await observer.dispatch?.(metadata);
    },
    after: async (metadata, outcome) => {
      for (const observer of active) await observer.after?.(metadata, outcome);
    },
  };
}

function resolveAutonomousCostBudget(options: Pick<AutonomousRunOptions, "maxTotalCostUnits" | "costBudget">): AutonomousCostBudget | undefined {
  if (options.costBudget !== undefined && !(options.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("costBudget must be an AutonomousCostBudget");
  if (options.costBudget !== undefined && options.maxTotalCostUnits !== undefined) throw new ArgumentError("costBudget and maxTotalCostUnits cannot both be supplied");
  return options.costBudget ?? (options.maxTotalCostUnits === undefined ? undefined : new AutonomousCostBudget(options.maxTotalCostUnits));
}

function resolvePlanAndRunBudget(options: AutonomousPlanAndRunOptions, planning: AutonomousProviderPlanningOptions | undefined): AutonomousCostBudget | undefined {
  const runConfigured = options.costBudget !== undefined || options.maxTotalCostUnits !== undefined;
  const planningConfigured = planning?.costBudget !== undefined || planning?.maxTotalCostUnits !== undefined;
  if (runConfigured && planningConfigured) {
    if (options.costBudget !== undefined && planning?.costBudget === options.costBudget) return options.costBudget;
    throw new ArgumentError("planAndRun accepts one shared cost budget configuration; provide the same AutonomousCostBudget object to both phases");
  }
  return runConfigured ? resolveAutonomousCostBudget(options) : planningConfigured ? resolveAutonomousCostBudget(planning!) : undefined;
}

let autonomousMemoryRunSequence = 0;
let autonomousLearningEpisodeSequence = 0;

async function scopedProviderIdempotencyKey(
  root: string | undefined,
  scope: JsonObject,
): Promise<string | undefined> {
  if (root === undefined) return undefined;
  const providerIdempotencyKey = boundedIdentifier("provider idempotency key", root);
  return digestJson({
    schema: "bioprism-typescript-autonomous-provider-idempotency-scope/0.1",
    provider_idempotency_key: providerIdempotencyKey,
    scope,
  });
}

function validateAutonomousStructuredOutputOptions(options: Pick<AutonomousRunOptions, "requireJson" | "responseSchema" | "structuredDomainResponse" | "requireStructuredResponseReview">): void {
  if (options.requireJson !== undefined && typeof options.requireJson !== "boolean") throw new ArgumentError("autonomous requireJson must be boolean");
  if (options.structuredDomainResponse !== undefined && typeof options.structuredDomainResponse !== "boolean") throw new ArgumentError("autonomous structuredDomainResponse must be boolean");
  if (options.requireStructuredResponseReview !== undefined && typeof options.requireStructuredResponseReview !== "boolean") throw new ArgumentError("autonomous requireStructuredResponseReview must be boolean");
  if (options.structuredDomainResponse === true && options.responseSchema !== undefined) throw new ArgumentError("structuredDomainResponse cannot be combined with a custom responseSchema");
  if (options.responseSchema !== undefined) {
    if (!isObject(options.responseSchema)) throw new ArgumentError("autonomous responseSchema must be a JSON object");
    if (options.requireJson !== true) throw new ArgumentError("autonomous responseSchema requires requireJson: true");
    let encoded: string | undefined;
    try { encoded = JSON.stringify(options.responseSchema); } catch { throw new ArgumentError("autonomous responseSchema must be JSON-serializable"); }
    if (!encoded || bytes(encoded) > 1_000_000) throw new ArgumentError("autonomous responseSchema exceeds its bounded size");
  }
}

function directStructuredResponseStatus(
  status: AutonomousRunStatus,
  evaluation: AutonomousDomainResponseEvaluation | null,
  requireReview: boolean | undefined,
): AutonomousRunStatus {
  return status === "completed" && requireReview !== false && evaluation !== null && !evaluation.passed
    ? "response_review_required"
    : status;
}

function normalizeAutonomousDomainPolicyMode(value: AutonomousDomainPolicyExecutionMode | undefined): AutonomousDomainPolicyExecutionMode {
  if (value === undefined) return "audit";
  if (value !== "audit" && value !== "strict") throw new ArgumentError("domainPolicyMode must be audit or strict");
  return value;
}

function domainPolicyAdmissionForBlueprint(
  route: AutonomousRouteProposal,
  blueprint: AutonomousTaskBlueprint,
  options: AutonomousRunOptions,
  acceptedPlan: boolean,
): AutonomousDomainPolicyAdmission | null {
  if (normalizeAutonomousDomainPolicyMode(options.domainPolicyMode) !== "strict") return null;
  return evaluateAutonomousDomainPolicy(blueprint.domain_policy, {
    route_confidence: route.confidence,
    route_abstained: route.abstained,
    estimated_input_tokens: blueprint.prompt.estimated_input_tokens,
    requested_output_tokens: options.maxOutputTokens ?? 1_024,
    estimated_cost_units: options.maxTotalCostUnits,
    structured_response: options.structuredDomainResponse === true,
    evidence_ready: options.domainPolicyEvidenceReady,
    evaluator_configured: options.domainPolicyEvaluatorConfigured ?? options.learning !== undefined,
    plan_accepted: options.domainPolicyPlanAccepted ?? acceptedPlan,
    effects_requested: options.domainPolicyEffectsRequested,
    effects_approved: options.domainPolicyEffectsApproved ?? options.approveEffects,
  });
}

function domainPolicyStatus(admission: AutonomousDomainPolicyAdmission): "policy_review_required" | "policy_blocked" {
  return admission.decision === "blocked" ? "policy_blocked" : "policy_review_required";
}

/**
 * Provider-free admission for a planner call. Planning is still a provider boundary, but it is
 * not execution-plan acceptance: the planner may propose a reorder while the later run() call
 * must re-check the actual accepted plan and effect posture.
 */
function domainPolicyAdmissionForPlanning(
  domain: AutonomousDomainName,
  estimatedInputTokens: number,
  options: AutonomousProviderPlanningOptions,
  costBudget: AutonomousCostBudget | undefined,
): AutonomousDomainPolicyAdmission | null {
  if (normalizeAutonomousDomainPolicyMode(options.domainPolicyMode) !== "strict") return null;
  const policy = autonomousDomainPolicy(domain);
  return evaluateAutonomousDomainPolicy(policy, {
    estimated_input_tokens: estimatedInputTokens,
    requested_output_tokens: options.maxOutputTokens ?? 1_024,
    estimated_cost_units: options.maxTotalCostUnits ?? costBudget?.snapshot().max_cost_units,
    structured_response: true,
    evidence_ready: options.domainPolicyEvidenceReady,
    evaluator_configured: options.domainPolicyEvaluatorConfigured,
    // This gate means the planner is allowed to propose a reviewed plan. The execution call
    // performs the separate acceptance check with the caller's actual accepted proposal.
    plan_accepted: true,
    effects_requested: options.domainPolicyEffectsRequested ?? false,
    effects_approved: options.domainPolicyEffectsApproved,
  });
}

function validateAutonomousDomainResponseOrThrow(
  response: { structured: unknown } | null,
  contract: AutonomousDomainResponseContract | null | undefined,
): ReturnType<typeof validateAutonomousProviderDomainResponse> {
  if (!contract) return null;
  try {
    return validateAutonomousProviderDomainResponse(response, contract);
  } catch {
    // Keep semantic response failures in the same redacted provider failure taxonomy as local
    // JSON-schema failures; the response body and validation detail remain caller-transient.
    throw new ProviderRuntimeError("provider returned an invalid reviewed domain response", { code: "invalid_response" });
  }
}

function evaluateAutonomousDomainResponseOrThrow(
  response: { structured: unknown } | null,
  contract: AutonomousDomainResponseContract | null | undefined,
): AutonomousDomainResponseEvaluation | null {
  const validated = validateAutonomousDomainResponseOrThrow(response, contract);
  return validated && contract ? evaluateAutonomousDomainResponse(validated, contract) : null;
}

function normalizedCrossDomainConcurrency(value: number | undefined, totalChildren: number): number {
  // Deterministic serial fan-out is the safe default for providers whose response does not carry
  // an application-level child id. Callers can explicitly opt into bounded parallelism once their
  // provider adapter associates each response with its child contract.
  const requested = value ?? 1;
  if (!Number.isSafeInteger(requested) || requested < 1 || requested > AUTONOMOUS_CROSS_DOMAIN_MAX_CONCURRENCY) {
    throw new ArgumentError(`cross-domain maxParallelChildren must be an integer within [1, ${AUTONOMOUS_CROSS_DOMAIN_MAX_CONCURRENCY}]`);
  }
  return Math.min(requested, totalChildren);
}

function assertAutonomousTaskDecisionAllowsProvider(
  decision: AutonomousTaskDecision,
  scope: string,
): void {
  if (decision.posture !== "blocked") return;
  const reasons = decision.blocking_reasons.length > 0
    ? decision.blocking_reasons.join(", ")
    : "unspecified_policy_block";
  throw new ProviderRuntimeError(`${scope} is blocked by the task decision posture: ${reasons}`);
}

/** Bind exact reviewed domain tools to a live catalogue without turning metadata into authority. */
export class AutonomousDomainToolRegistry extends AutonomousDomainToolRegistryBase {
  private constructor(catalogue: ToolCatalogue, profiles: readonly AutonomousDomainToolProfile[], workflowProfiles: readonly AutonomousDomainProfile[], digest: string) {
    super(catalogue, profiles, workflowProfiles, digest);
  }

  static async create(catalogue: ToolCatalogue, profiles?: readonly AutonomousDomainToolProfile[]): Promise<AutonomousDomainToolRegistry> {
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("autonomous domain tool registry requires a ToolCatalogue");
    const workflowProfiles = await builtinAutonomousDomainProfiles();
    const selected = profiles ? [...profiles] : workflowProfiles.map((profile) => profile.tool_profile);
    if (!selected.length || selected.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("autonomous domain tool registry profile count is outside its bounds");
    const profileDigest = await digestJson(selected.map((profile) => profile));
    return new AutonomousDomainToolRegistry(catalogue, selected, workflowProfiles.filter((profile) => selected.some((candidate) => candidate.domain === profile.domain)), profileDigest);
  }
}

/**
 * Full application-facing autonomous brain composition for the TypeScript embedding boundary.
 *
 * Routing, prompt assembly, workflow planning, tool binding, and learning are explicit. Provider
 * prompts/responses remain in the application process; only health, model candidates, selection,
 * and evaluator rewards may cross into the value-only Rust/Python control plane.
 */
export class AutonomousAgent {
  readonly llm: LLMRuntime;
  readonly runtime: AutonomousRuntime;
  readonly activation: AutonomousCapabilityActivation;
  readonly modelHealthController?: AutonomousModelHealthController;
  /** Caller-owned provider/model health ledger used as a durable selection prior. */
  readonly modelHealthStore?: AutonomousModelHealthStore;
  /** Caller-owned persistence for aggregate model-health observations and evaluator quality. */
  readonly modelHealthPersistence?: AutonomousModelHealthPersistenceCoordinator;
  /** Caller-owned persistence for transport counters and provider circuit continuity. */
  readonly runtimeHealthPersistence?: LLMRuntimeHealthPersistenceCoordinator;
  readonly modelHealthBridge?: AutonomousBrainControlPlaneBridge;
  readonly learner?: AutonomousOnlineLearner;
  /** Caller-owned persistence for the online learner; no state is written implicitly. */
  readonly learnerPersistence?: AutonomousOnlineLearnerPersistenceCoordinator;
  readonly selectionPromotion?: AutonomousSelectionPromotionLifecycle;
  readonly evaluatorCalibrationRegistry?: AutonomousEvaluatorCalibrationRegistry;
  /** Caller-owned persistence for aggregate evaluator calibration reports. */
  readonly evaluatorCalibrationPersistence?: AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator;
  private readonly apiClient?: ApiClient;
  private readonly modelsById = new Map<string, AutonomousModelCandidate>();
  private readonly toolCatalogue?: ToolCatalogue;
  private readonly toolExecutor?: DomainToolExecutor;
  private readonly toolApprover?: DomainToolApprover;
  private readonly effectBoundary?: AutonomousEffectBoundary;
  private readonly capabilityJournal?: AutonomousCapabilityJournalStore;
  /** Caller-owned persistence for capability replay identities; values remain transient. */
  readonly capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
  /** Metadata-only execution journal used by restart-aware callers. */
  readonly executionJournal?: AutonomousExecutionSnapshotJournal;
  /** Caller-owned persistence for execution checkpoints. */
  readonly executionPersistence?: AutonomousExecutionPersistenceCoordinator;
  /** Caller-owned persistence for route/planning/evaluation decision-cycle checkpoints. */
  readonly decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
  private readonly capabilityLearningSettlementStore: AutonomousCapabilityLearningSettlementStore;
  /** Caller-owned connector catalogue and runtime for bounded external evidence/provider work. */
  readonly connectorRegistry?: AutonomousConnectorRegistry;
  readonly connectorRuntime?: AutonomousConnectorRuntime;
  /** Caller-owned episodic memory; exposed so the learning controller can close evaluation feedback. */
  readonly memoryStore?: AutonomousEpisodicMemoryStore;
  /** Caller-owned persistence for episodic memory; no episode state is written implicitly. */
  readonly memoryPersistence?: AutonomousMemoryPersistenceCoordinator;
  /** Optional evaluator-gated lesson index; it never receives prompts, provider payloads, or keys. */
  readonly memoryConsolidator?: AutonomousMemoryConsolidator;
  readonly promptLearningCoordinator?: AutonomousPromptLearningPersistenceCoordinator;
  /** Caller-owned persistence for value-only adaptive tool-arm state. */
  readonly toolSelectionPersistence?: AutonomousToolSelectionPersistence;
  private toolSelectionStateValue: AutonomousToolSelectionState;
  private readonly toolSelectionConfigured: boolean;
  private readonly toolSelectionPersistenceCoordinator?: AutonomousToolSelectionPersistenceCoordinator;
  private domainToolRegistry?: AutonomousDomainToolRegistry;
  private domainToolRuntime?: AutonomousDomainToolRuntime;
  private capabilityRuntime?: AutonomousCapabilityRuntime;
  /**
   * One serialized model-inventory coordinator per agent lifecycle.
   *
   * Inventory refreshes may use a caller-owned CAS store. Recreating the coordinator for each
   * refresh would discard the last observed inventory digest and turn a legitimate sequential
   * refresh into a false stale-writer conflict. Keep the coordinator lazy so lightweight agents do
   * not load the optional inventory module until discovery or restore is requested.
   */
  private modelInventoryCoordinator?: import("./autonomous-model-inventory.js").AutonomousModelInventoryCoordinator;
  private persistenceLifecycleCoordinator?: import("./autonomous-agent-lifecycle.js").AutonomousAgentPersistenceLifecycleCoordinator;
  private persistenceLifecycleModelInventoryPersistence?: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence;
  private persistenceLifecycleActivationStore?: AutonomousCapabilityActivationSnapshotStore;
  private persistenceLifecycleSelectionPromotionStore?: AutonomousSelectionLifecycleStore;
  private persistenceLifecycleCapabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
  private persistenceLifecycleDecisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
  private persistenceLifecycleExecutionPersistence?: AutonomousExecutionPersistenceCoordinator;
  private persistenceLifecycleRequireAll?: boolean;
  private persistenceLifecycleContinueOnError?: boolean;

  constructor(llm: LLMRuntime, options: AutonomousAgentOptions = {}) {
    if (!(llm instanceof LLMRuntime)) throw new ProviderRuntimeError("AutonomousAgent requires an LLMRuntime");
    if (options.apiClient && typeof options.apiClient.brainModelSelectContextual !== "function") throw new ArgumentError("AutonomousAgent apiClient is malformed");
    if (options.toolCatalogue !== undefined && !(options.toolCatalogue instanceof ToolCatalogue)) throw new ArgumentError("AutonomousAgent toolCatalogue must be a ToolCatalogue");
    if (options.toolExecutor !== undefined && typeof options.toolExecutor !== "function") throw new ArgumentError("AutonomousAgent toolExecutor must be callable");
    if (options.effectBoundary !== undefined && !(options.effectBoundary instanceof AutonomousEffectBoundary)) throw new ArgumentError("AutonomousAgent effectBoundary must be an AutonomousEffectBoundary");
    const modelHealthStore = options.modelHealthStore ?? options.modelHealthPersistence?.store;
    if (options.modelHealthPersistence !== undefined && !(options.modelHealthPersistence instanceof AutonomousModelHealthPersistenceCoordinator)) throw new ArgumentError("AutonomousAgent modelHealthPersistence must be an AutonomousModelHealthPersistenceCoordinator");
    if (options.modelHealthPersistence !== undefined && options.modelHealthStore !== undefined && options.modelHealthPersistence.store !== options.modelHealthStore) throw new ArgumentError("AutonomousAgent modelHealthPersistence must be bound to the supplied modelHealthStore");
    if (modelHealthStore !== undefined && (
      typeof modelHealthStore.record !== "function"
      || typeof modelHealthStore.recordInvocation !== "function"
      || typeof modelHealthStore.recordEvaluation !== "function"
      || typeof modelHealthStore.health !== "function"
      || typeof modelHealthStore.selectorHealth !== "function"
      || typeof modelHealthStore.snapshot !== "function"
      || typeof modelHealthStore.restore !== "function"
    )) throw new ArgumentError("AutonomousAgent modelHealthStore is malformed");
    if (options.runtimeHealthPersistence !== undefined && !(options.runtimeHealthPersistence instanceof LLMRuntimeHealthPersistenceCoordinator)) throw new ArgumentError("AutonomousAgent runtimeHealthPersistence must be an LLMRuntimeHealthPersistenceCoordinator");
    if (options.runtimeHealthPersistence !== undefined && options.runtimeHealthPersistence.runtime !== llm) throw new ArgumentError("AutonomousAgent runtimeHealthPersistence must be bound to the supplied LLMRuntime");
    if (options.activation !== undefined && !(options.activation instanceof AutonomousCapabilityActivation)) throw new ArgumentError("AutonomousAgent activation must be an AutonomousCapabilityActivation");
    if (options.selectionPromotion !== undefined && !(options.selectionPromotion instanceof AutonomousSelectionPromotionLifecycle)) throw new ArgumentError("AutonomousAgent selectionPromotion must be an AutonomousSelectionPromotionLifecycle");
    if (options.evaluatorCalibrationRegistry !== undefined && (
      typeof options.evaluatorCalibrationRegistry.import !== "function"
      || typeof options.evaluatorCalibrationRegistry.get !== "function"
      || typeof options.evaluatorCalibrationRegistry.query !== "function"
      || typeof options.evaluatorCalibrationRegistry.snapshot !== "function"
      || typeof options.evaluatorCalibrationRegistry.restore !== "function"
    )) throw new ArgumentError("AutonomousAgent evaluatorCalibrationRegistry is malformed");
    if (options.evaluatorCalibrationPersistence !== undefined && (
      typeof options.evaluatorCalibrationPersistence.restore !== "function"
      || typeof options.evaluatorCalibrationPersistence.flush !== "function"
      || options.evaluatorCalibrationPersistence.registry !== options.evaluatorCalibrationRegistry
    )) throw new ArgumentError("AutonomousAgent evaluatorCalibrationPersistence must be bound to the supplied registry");
    if (options.promptLearningCoordinator !== undefined && !(options.promptLearningCoordinator instanceof AutonomousPromptLearningPersistenceCoordinator)) throw new ArgumentError("AutonomousAgent promptLearningCoordinator must be an AutonomousPromptLearningPersistenceCoordinator");
    if (options.toolSelectionPersistence !== undefined && (typeof options.toolSelectionPersistence.read !== "function" || typeof options.toolSelectionPersistence.write !== "function")) throw new ArgumentError("AutonomousAgent toolSelectionPersistence is malformed");
    if (options.connectorRegistry !== undefined && !(options.connectorRegistry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("AutonomousAgent connectorRegistry must be an AutonomousConnectorRegistry");
    if (options.connectorRuntime !== undefined && !(options.connectorRuntime instanceof AutonomousConnectorRuntime)) throw new ArgumentError("AutonomousAgent connectorRuntime must be an AutonomousConnectorRuntime");
    if (options.connectorRegistry !== undefined && options.connectorRuntime !== undefined && options.connectorRuntime.registry !== options.connectorRegistry) throw new ArgumentError("AutonomousAgent connectorRegistry and connectorRuntime must reference the same catalogue");
    if (options.learnerPersistence !== undefined && (
      typeof options.learnerPersistence.restore !== "function"
      || typeof options.learnerPersistence.flush !== "function"
      || options.learnerPersistence.learner !== options.learner
    )) throw new ArgumentError("AutonomousAgent learnerPersistence must be bound to the supplied learner");
    this.llm = llm;
    this.apiClient = options.apiClient;
    this.learner = options.learner;
    this.learnerPersistence = options.learnerPersistence;
    this.modelHealthStore = modelHealthStore;
    this.modelHealthPersistence = options.modelHealthPersistence;
    this.selectionPromotion = options.selectionPromotion;
    this.evaluatorCalibrationRegistry = options.evaluatorCalibrationRegistry;
    this.evaluatorCalibrationPersistence = options.evaluatorCalibrationPersistence;
    if (options.memoryStore !== undefined && (
      typeof options.memoryStore.retrieve !== "function"
      || typeof options.memoryStore.recordEpisode !== "function"
      || typeof options.memoryStore.get !== "function"
    )) throw new ArgumentError("AutonomousAgent memoryStore is malformed");
    this.memoryStore = options.memoryStore;
    if (options.memoryPersistence !== undefined && (
      typeof options.memoryPersistence.restore !== "function"
      || typeof options.memoryPersistence.flush !== "function"
      || options.memoryPersistence.store !== options.memoryStore
    )) throw new ArgumentError("AutonomousAgent memoryPersistence must be bound to the supplied memoryStore");
    this.memoryPersistence = options.memoryPersistence;
    if (options.memoryConsolidator !== undefined && (
      typeof options.memoryConsolidator.consolidate !== "function"
      || typeof options.memoryConsolidator.recall !== "function"
      || typeof options.memoryConsolidator.promptReferences !== "function"
    )) throw new ArgumentError("AutonomousAgent memoryConsolidator is malformed");
    this.memoryConsolidator = options.memoryConsolidator;
    this.promptLearningCoordinator = options.promptLearningCoordinator;
    this.toolSelectionPersistence = options.toolSelectionPersistence;
    this.toolSelectionStateValue = normalizeAutonomousToolSelectionState(options.toolSelectionState);
    this.toolSelectionConfigured = options.toolSelectionState !== undefined || options.toolSelectionPersistence !== undefined;
    if (this.toolSelectionPersistence !== undefined) {
      this.toolSelectionPersistenceCoordinator = new AutonomousToolSelectionPersistenceCoordinator({
        get: () => structuredClone(this.toolSelectionStateValue),
        set: (state) => { this.toolSelectionStateValue = normalizeAutonomousToolSelectionState(state); },
      }, this.toolSelectionPersistence);
    }
    this.activation = options.activation ?? new AutonomousCapabilityActivation();
    this.modelHealthController = modelHealthStore === undefined ? undefined : new AutonomousModelHealthController(modelHealthStore);
    this.runtimeHealthPersistence = options.runtimeHealthPersistence;
    if (options.modelHealthBridge !== undefined && !(options.modelHealthBridge instanceof AutonomousBrainControlPlaneBridge)) throw new ArgumentError("AutonomousAgent modelHealthBridge must be an AutonomousBrainControlPlaneBridge");
    this.modelHealthBridge = options.modelHealthBridge;
    this.toolCatalogue = options.toolCatalogue;
    this.toolExecutor = options.toolExecutor ?? (this.apiClient && this.toolCatalogue
      ? createAutonomousApiToolExecutor(this.apiClient, { catalogue: this.toolCatalogue })
      : undefined);
    this.toolApprover = options.toolApprover;
    this.effectBoundary = options.effectBoundary;
    if (this.effectBoundary !== undefined) {
      try {
        llm.bindEffectBoundary(this.effectBoundary);
      } catch (error) {
        throw new ProviderRuntimeError("runtime and agent effect boundaries must be the same instance", { code: "configuration" });
      }
    }
    if (options.capabilityJournal !== undefined && (typeof options.capabilityJournal.append !== "function" || typeof options.capabilityJournal.find !== "function" || typeof options.capabilityJournal.records !== "function")) throw new ArgumentError("AutonomousAgent capabilityJournal is malformed");
    if (options.capabilityJournalPersistence !== undefined && (typeof options.capabilityJournalPersistence.restore !== "function" || typeof options.capabilityJournalPersistence.flush !== "function" || options.capabilityJournalPersistence.store !== options.capabilityJournal)) throw new ArgumentError("AutonomousAgent capabilityJournalPersistence must be bound to the supplied capabilityJournal");
    if (options.executionJournal !== undefined && (typeof options.executionJournal.append !== "function" || typeof options.executionJournal.state !== "function" || typeof options.executionJournal.snapshot !== "function" || typeof options.executionJournal.restore !== "function")) throw new ArgumentError("AutonomousAgent executionJournal is malformed");
    if (options.executionPersistence !== undefined && (typeof options.executionPersistence.restore !== "function" || typeof options.executionPersistence.flush !== "function" || options.executionPersistence.journal !== options.executionJournal)) throw new ArgumentError("AutonomousAgent executionPersistence must be bound to the supplied executionJournal");
    if (options.decisionCyclePersistence !== undefined && (typeof options.decisionCyclePersistence.restore !== "function" || typeof options.decisionCyclePersistence.flush !== "function" || typeof options.decisionCyclePersistence.store?.snapshot !== "function" || typeof options.decisionCyclePersistence.store?.restore !== "function")) throw new ArgumentError("AutonomousAgent decisionCyclePersistence is malformed");
    this.capabilityJournal = options.capabilityJournal;
    this.capabilityJournalPersistence = options.capabilityJournalPersistence;
    this.executionJournal = options.executionJournal;
    this.executionPersistence = options.executionPersistence;
    this.decisionCyclePersistence = options.decisionCyclePersistence;
    if (options.capabilityLearningSettlementStore !== undefined && (typeof options.capabilityLearningSettlementStore.load !== "function" || typeof options.capabilityLearningSettlementStore.save !== "function")) throw new ArgumentError("AutonomousAgent capabilityLearningSettlementStore is malformed");
    this.capabilityLearningSettlementStore = options.capabilityLearningSettlementStore ?? new InMemoryAutonomousCapabilityLearningSettlementStore();
    this.connectorRegistry = options.connectorRegistry ?? options.connectorRuntime?.registry;
    this.connectorRuntime = options.connectorRuntime;
    const remoteHealthSelector = this.modelHealthBridge?.selector();
    const learnedSelector: AutonomousModelSelector | undefined = options.learner === undefined
      ? undefined
      : async (request: AutonomousSelectionRequest): Promise<AutonomousSelectionDecision> => {
        let learnedRequest = request;
        if (this.modelHealthController) {
          // Persisted health is a safety/availability prior, not a replacement for evaluator
          // learning. Merge it before the learner scores arms so circuits and observed quality
          // remain gates while contextual bandit rewards still adapt the chosen model.
          const persistentHealth = await this.modelHealthController.store.selectorHealth();
          learnedRequest = { ...request, model_health: { ...request.model_health, ...persistentHealth } };
        } else if (remoteHealthSelector) {
          // The remote bridge exposes only a value-only ranking. Preserve its eligibility gate
          // locally, then let the local learner choose among the remotely admissible candidates.
          // This keeps remote circuits authoritative without replaying or importing raw health.
          const healthDecision = await remoteHealthSelector(request);
          const remotelyEligible = new Set(
            healthDecision.ranking
              .filter((row) => row.eligible)
              .map((row) => `${row.provider}/${row.model}`),
          );
          if (healthDecision.selected_model) remotelyEligible.add(`${healthDecision.selected_model.provider}/${healthDecision.selected_model.model}`);
          if (remotelyEligible.size === 0) return healthDecision;
          learnedRequest = {
            ...request,
            candidates: request.candidates.filter((candidate) => remotelyEligible.has(`${candidate.provider}/${candidate.model}`)),
          };
        }
        return options.learner!.select(learnedRequest);
      };
    const baseSelector = options.selector ?? (learnedSelector ?? (this.modelHealthController ? this.modelHealthController.selector() : options.apiClient ? contextualSelector(options.apiClient) : remoteHealthSelector));
    const selector = options.learner !== undefined && this.selectionPromotion !== undefined
      ? async (request: AutonomousSelectionRequest): Promise<AutonomousSelectionDecision> => {
        if (!this.selectionPromotion!.isAdmitted()) {
          return { selected_model: null, strategy: "caller_selector", ranking: [], abstention_reason: `learned model selection is not admitted (${this.selectionPromotion!.state.status})`, selection_confidence: 0, min_selection_confidence: request.min_selection_confidence ?? null };
        }
        if (baseSelector === undefined) throw new ProviderRuntimeError("promoted learner selector is not configured");
        return baseSelector(request);
      }
      : baseSelector;
    this.runtime = new AutonomousRuntime(llm, { selector });
  }

  /**
   * Restore the digest-bound online bandit before the agent accepts evaluator feedback.
   *
   * Restoring is intentionally explicit: a missing coordinator is a configuration error rather
   * than an invitation to silently start a fresh learner, because that would discard the model
   * and domain history that the caller believed was being used for adaptation.
   */
  async restoreOnlineLearning(): Promise<AutonomousOnlineLearnerSnapshot | null> {
    if (!this.learner) throw new ArgumentError("AutonomousAgent has no AutonomousOnlineLearner");
    if (!this.learnerPersistence) throw new ArgumentError("AutonomousAgent online learner persistence is not configured");
    return this.learnerPersistence.restore();
  }

  /**
   * Flush the current bandit state through its caller-owned CAS boundary.
   *
   * The snapshot contains arm statistics, bounded evaluator digests, and contextual metadata
   * only. Prompts, provider responses, credentials, task text, and live tool values never enter
   * this persistence path. Callers choose when to flush so an application can coordinate the
   * write with its own transaction or feedback outbox.
   */
  async flushOnlineLearning(): Promise<AutonomousOnlineLearnerSnapshot> {
    if (!this.learner) throw new ArgumentError("AutonomousAgent has no AutonomousOnlineLearner");
    if (!this.learnerPersistence) throw new ArgumentError("AutonomousAgent online learner persistence is not configured");
    return this.learnerPersistence.flush();
  }

  /**
   * Restore aggregate provider/model health before admitting new selections.
   *
   * Health is an availability and quality prior, not evaluator authority. Restoration is
   * therefore explicit and coordinator-bound: it can never silently replace a missing ledger
   * with an empty one or make a previously unregistered model eligible.
   */
  async restoreHealth(): Promise<AutonomousModelHealthSnapshot | null> {
    if (!this.modelHealthStore) throw new ArgumentError("AutonomousAgent has no model health store");
    if (!this.modelHealthPersistence) throw new ArgumentError("AutonomousAgent model health persistence is not configured");
    return this.modelHealthPersistence.restore();
  }

  /** Flush aggregate provider/model health through the caller-owned CAS boundary. */
  async flushHealth(): Promise<AutonomousModelHealthSnapshot> {
    if (!this.modelHealthStore) throw new ArgumentError("AutonomousAgent has no model health store");
    if (!this.modelHealthPersistence) throw new ArgumentError("AutonomousAgent model health persistence is not configured");
    return this.modelHealthPersistence.flush();
  }

  /** Compatibility alias whose name makes the provider/model scope explicit. */
  async restoreModelHealth(): Promise<AutonomousModelHealthSnapshot | null> {
    return this.restoreHealth();
  }

  /** Compatibility alias whose name makes the provider/model scope explicit. */
  async flushModelHealth(): Promise<AutonomousModelHealthSnapshot> {
    return this.flushHealth();
  }

  /** Return the agent-owned value-only adaptive tool state without exposing mutable internals. */
  toolSelectionState(): AutonomousToolSelectionState {
    return structuredClone(this.toolSelectionStateValue);
  }

  /** Restore evaluator-approved tool-arm statistics before admitting new adaptive decisions. */
  async restoreToolSelection(): Promise<AutonomousToolSelectionSnapshot | null> {
    if (!this.toolSelectionPersistenceCoordinator) throw new ArgumentError("AutonomousAgent tool selection persistence is not configured");
    return this.toolSelectionPersistenceCoordinator.restore();
  }

  /** Flush evaluator-approved tool-arm statistics through the caller-owned CAS boundary. */
  async flushToolSelection(): Promise<AutonomousToolSelectionSnapshot> {
    if (!this.toolSelectionPersistenceCoordinator) throw new ArgumentError("AutonomousAgent tool selection persistence is not configured");
    return this.toolSelectionPersistenceCoordinator.flush();
  }

  /** Apply one independent evaluator reward to the agent-owned tool selector. */
  recordToolSelectionReward(outcome: AutonomousToolSelectionOutcome): AutonomousToolSelectionState {
    if (!this.toolSelectionConfigured) throw new ArgumentError("AutonomousAgent tool selection state is not configured");
    this.toolSelectionStateValue = settleAutonomousToolSelectionOutcome(this.toolSelectionStateValue, outcome);
    return this.toolSelectionState();
  }

  /**
   * Restore process-local provider transport counters and circuit state after a restart.
   *
   * This is deliberately explicit and coordinator-bound. A restored image is a transport
   * availability prior only; it does not restore credentials, prompts, responses, evaluator
   * rewards, or authorization. Provider registrations must be recreated before this call.
   */
  async restoreRuntimeHealth(): Promise<LLMRuntimeHealthSnapshot | null> {
    if (!this.runtimeHealthPersistence) throw new ArgumentError("AutonomousAgent runtime health persistence is not configured");
    return this.runtimeHealthPersistence.restore();
  }

  /** Flush process-local transport counters and circuit state through the caller-owned CAS boundary. */
  async flushRuntimeHealth(): Promise<LLMRuntimeHealthSnapshot> {
    if (!this.runtimeHealthPersistence) throw new ArgumentError("AutonomousAgent runtime health persistence is not configured");
    return this.runtimeHealthPersistence.flush();
  }

  /** Compatibility alias for restoring transport health through the agent boundary. */
  async restoreTransportHealth(): Promise<LLMRuntimeHealthSnapshot | null> {
    return this.restoreRuntimeHealth();
  }

  /** Compatibility alias for flushing transport health through the agent boundary. */
  async flushTransportHealth(): Promise<LLMRuntimeHealthSnapshot> {
    return this.flushRuntimeHealth();
  }

  /** Register a validated aggregate evaluator calibration report; persistence remains explicit. */
  registerEvaluatorCalibration(report: AutonomousEvaluatorCalibrationReport): AutonomousEvaluatorCalibrationImport {
    if (!this.evaluatorCalibrationRegistry) throw new ArgumentError("AutonomousAgent evaluator calibration registry is not configured");
    return this.evaluatorCalibrationRegistry.import(report);
  }

  /** Retrieve one persisted calibration report by its exact content digest. */
  evaluatorCalibrationReport(reportDigest: string): AutonomousEvaluatorCalibrationReport | null {
    if (!this.evaluatorCalibrationRegistry) throw new ArgumentError("AutonomousAgent evaluator calibration registry is not configured");
    return this.evaluatorCalibrationRegistry.get(reportDigest);
  }

  /** Query only aggregate calibration metadata; source cases and labels are never retained here. */
  evaluatorCalibrationReports(options: AutonomousEvaluatorCalibrationQueryOptions = {}): AutonomousEvaluatorCalibrationReport[] {
    if (!this.evaluatorCalibrationRegistry) throw new ArgumentError("AutonomousAgent evaluator calibration registry is not configured");
    return this.evaluatorCalibrationRegistry.query(options);
  }

  /** Restore evaluator calibration before enabling a calibration-gated learning path. */
  async restoreEvaluatorCalibration(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot | null> {
    if (!this.evaluatorCalibrationRegistry) throw new ArgumentError("AutonomousAgent evaluator calibration registry is not configured");
    if (!this.evaluatorCalibrationPersistence) throw new ArgumentError("AutonomousAgent evaluator calibration persistence is not configured");
    return this.evaluatorCalibrationPersistence.restore();
  }

  /** Flush aggregate evaluator calibration through the caller-owned CAS boundary. */
  async flushEvaluatorCalibration(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot> {
    if (!this.evaluatorCalibrationRegistry) throw new ArgumentError("AutonomousAgent evaluator calibration registry is not configured");
    if (!this.evaluatorCalibrationPersistence) throw new ArgumentError("AutonomousAgent evaluator calibration persistence is not configured");
    return this.evaluatorCalibrationPersistence.flush();
  }

  /**
   * Restore the hash-chained episodic memory before the agent accepts new run context.
   *
   * Restoration is explicit and exact-store-bound. A coordinator for a different memory store
   * is rejected during construction so a restart cannot appear successful while restoring into a
   * store that the agent does not actually query.
   */
  async restoreMemory(): Promise<AutonomousMemorySnapshot | null> {
    if (!this.memoryStore) throw new ArgumentError("AutonomousAgent has no episodic memory store");
    if (!this.memoryPersistence) throw new ArgumentError("AutonomousAgent memory persistence is not configured");
    return this.memoryPersistence.restore();
  }

  /**
   * Flush value-only memory episodes and evaluations through the caller-owned CAS boundary.
   * Prompts, task text, provider responses, tool payloads, and credentials never enter the
   * persisted memory image; only the memory store's validated digest projections are written.
   */
  async flushMemory(): Promise<AutonomousMemorySnapshot> {
    if (!this.memoryStore) throw new ArgumentError("AutonomousAgent has no episodic memory store");
    if (!this.memoryPersistence) throw new ArgumentError("AutonomousAgent memory persistence is not configured");
    return this.memoryPersistence.flush();
  }

  /** Consolidate explicit evaluator observations through the configured lesson index. */
  consolidateMemory(
    observations: readonly AutonomousMemoryConsolidationObservation[],
    options: { generation?: number } = {},
  ): AutonomousMemoryConsolidationReport {
    if (this.memoryConsolidator === undefined) throw new ArgumentError("AutonomousAgent memoryConsolidator is not configured");
    return this.memoryConsolidator.consolidate(observations, options);
  }

  /** Resolve stable, scope-checked lesson digests into transient prompt references. */
  memoryReferences(options: {
    domain: AutonomousDomainName;
    capability?: string;
    lessonResolver?: (lessonDigest: string) => string | null;
    lessonContextResolver?: AutonomousMemoryLessonContextResolver;
    limit?: number;
  }): AutonomousMemoryConsolidationPromptReference[] {
    if (this.memoryConsolidator === undefined) throw new ArgumentError("AutonomousAgent memoryConsolidator is not configured");
    return this.memoryConsolidator.promptReferences(options);
  }

  registerModel(candidate: AutonomousModelCandidate, options: { replaceExisting?: boolean } = {}): AutonomousModelCandidate {
    return this.registerModels([candidate], options)[0]!;
  }

  registerModels(candidates: readonly AutonomousModelCandidate[], options: { replaceExisting?: boolean } = {}): AutonomousModelCandidate[] {
    if (!Array.isArray(candidates) || !candidates.length || candidates.length > AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) throw new ArgumentError(`autonomous model catalogue must contain 1..=${AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS} candidates`);
    const normalized = candidates.map((candidate) => normalizeAutonomousModelCandidate(candidate));
    const batchIds = new Set<string>();
    for (const candidate of normalized) {
      const id = `${candidate.provider}/${candidate.model}`;
      if (batchIds.has(id)) throw new ArgumentError(`autonomous model ${id} is duplicated in the registration batch`);
      batchIds.add(id);
      if (this.modelsById.has(id) && options.replaceExisting !== true) throw new ArgumentError(`autonomous model ${id} is already registered`);
    }
    for (const candidate of normalized) this.modelsById.set(`${candidate.provider}/${candidate.model}`, candidate);
    return normalized.map((candidate) => ({ ...candidate, capabilities: candidate.capabilities ? [...candidate.capabilities] : undefined }));
  }

  models(): AutonomousModelCandidate[] {
    return [...this.modelsById.values()].sort((left, right) => `${left.provider}/${left.model}`.localeCompare(`${right.provider}/${right.model}`)).map((candidate) => ({ ...candidate, capabilities: candidate.capabilities ? [...candidate.capabilities] : undefined }));
  }

  /** Seal the current catalogue as a redacted, content-addressed restart projection. */
  async snapshotModels(): Promise<AutonomousModelCatalogueSnapshot> {
    const models = this.models();
    const body = {
      schema: AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
      models,
      catalogue_digest: await digestJson(models),
      retention: "model_metadata_only_hash_bound" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...body, snapshot_digest: await digestJson(body) };
    if (bytes(JSON.stringify(snapshot) ?? "") > AUTONOMOUS_MODEL_CATALOGUE_MAX_SNAPSHOT_BYTES) throw new ArgumentError("autonomous model catalogue snapshot exceeds its byte capacity");
    return structuredClone(snapshot);
  }

  /** Restore a catalogue only after full validation; a rejected snapshot leaves live state unchanged. */
  async restoreModels(raw: unknown): Promise<void> {
    const snapshot = await validateAutonomousModelCatalogueSnapshot(raw);
    const next = new Map<string, AutonomousModelCandidate>();
    for (const candidate of snapshot.models) next.set(`${candidate.provider}/${candidate.model}`, structuredClone(candidate));
    this.modelsById.clear();
    for (const [id, candidate] of next) this.modelsById.set(id, candidate);
  }

  /** Persist the current catalogue through a caller-owned adapter. */
  async saveModelCatalogue(persistence: AutonomousModelCataloguePersistence): Promise<AutonomousModelCatalogueSnapshot> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("model catalogue persistence adapter is malformed");
    const snapshot = await this.snapshotModels();
    await persistence.write(snapshot);
    return snapshot;
  }

  /** Restore the catalogue from a caller-owned adapter; null means no restart state exists. */
  async restoreModelCatalogue(persistence: AutonomousModelCataloguePersistence): Promise<AutonomousModelCatalogueSnapshot | null> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("model catalogue persistence adapter is malformed");
    const raw = await persistence.read();
    if (raw === null) return null;
    const snapshot = await validateAutonomousModelCatalogueSnapshot(raw);
    await this.restoreModels(snapshot);
    return snapshot;
  }

  /** Return the redacted activation state without exposing caller credentials or transient prompts. */
  activationState(): AutonomousCapabilityActivationState {
    return this.activation.state;
  }

  /** Return the digest-only learned-selection authority state. */
  selectionPromotionState() {
    return this.selectionPromotion?.state ?? null;
  }

  /** Apply a validated replay admission and make the learned selector eligible when admitted. */
  applySelectionPromotion(report: AutonomousSelectionPromotionReport) {
    if (!this.selectionPromotion) throw new ArgumentError("selection promotion lifecycle is not configured");
    return this.selectionPromotion.apply(report);
  }

  /** Immediately stop promoted learned selection while retaining only rollback metadata. */
  rollbackSelectionPromotion(reason = "selection_promotion_rollback") {
    if (!this.selectionPromotion) throw new ArgumentError("selection promotion lifecycle is not configured");
    return this.selectionPromotion.rollback(reason);
  }

  /** Persist only the digest-bound learned-selection authority state through a caller-owned store. */
  async saveSelectionPromotion(store: AutonomousSelectionLifecycleStore): Promise<AutonomousSelectionLifecycleState> {
    if (!store || typeof store.save !== "function" || typeof store.load !== "function") throw new ArgumentError("selection promotion store is malformed");
    if (!this.selectionPromotion) throw new ArgumentError("selection promotion lifecycle is not configured");
    await store.save(this.selectionPromotion.state);
    return this.selectionPromotion.state;
  }

  /** Restore learned-selection authority state after validating identity, revision, and digests. */
  async restoreSelectionPromotion(store: AutonomousSelectionLifecycleStore): Promise<AutonomousSelectionLifecycleState | null> {
    if (!store || typeof store.load !== "function" || typeof store.save !== "function") throw new ArgumentError("selection promotion store is malformed");
    if (!this.selectionPromotion) throw new ArgumentError("selection promotion lifecycle is not configured");
    const state = await store.load();
    return state === null ? null : this.selectionPromotion.restore(state);
  }

  /** Record provider onboarding posture; this never accepts or persists a key value. */
  recordActivationProviderStatuses(statuses: readonly JsonObject[]): AutonomousCapabilityActivationState {
    return this.activation.recordProviderStatuses(statuses);
  }

  /** Record the exact catalogue/profile binding plan that a caller may review and approve. */
  recordActivationBindingPlan(plan: AutonomousDomainToolPlan): AutonomousCapabilityActivationState {
    return this.activation.recordBindingPlan(plan);
  }

  /**
   * Recompute the local readiness audit and exact all-domain binding plan into activation state.
   * The operation is keyless: readiness reads opaque credential status only and performs no
   * discovery, provider call, tool call, prompt dispatch, or external effect.
   */
  async refreshActivation(options: { candidates?: readonly AutonomousModelCandidate[]; estimatedInputTokens?: number; requestedOutputTokens?: number; selectionPromotionReport?: AutonomousSelectionPromotionReport; requirePromotedSelection?: boolean } = {}): Promise<AutonomousCapabilityActivationState> {
    const report = await this.readiness(options);
    this.activation.recordProviderStatuses(report.providers);
    const registry = await this.ensureToolRegistry();
    if (registry) this.activation.recordBindingPlan(await registry.plan());
    else this.activation.recordRegisteredTools(0);
    return this.activation.state;
  }

  /** Approve only proposed read-only bindings from the previously recorded, digest-bound plan. */
  approveActivationBindings(plan: AutonomousDomainToolPlan, approvedTools: readonly string[], registeredToolCount?: number): AutonomousCapabilityActivationState {
    return this.activation.approveBindings(plan, approvedTools, registeredToolCount ?? this.activation.state.registered_tool_count);
  }

  /** Persist the redacted activation state through a caller-owned store. */
  async saveActivation(store: AutonomousCapabilityActivationSnapshotStore): Promise<AutonomousCapabilityActivationState> {
    if (!store || typeof store.save !== "function") throw new ArgumentError("activation store must implement save");
    await store.save(this.activation.state);
    return this.activation.state;
  }

  /** Restore redacted activation state through a caller-owned store; null means no state existed. */
  async restoreActivation(store: AutonomousCapabilityActivationSnapshotStore): Promise<AutonomousCapabilityActivationState | null> {
    if (!store || typeof store.load !== "function") throw new ArgumentError("activation store must implement load");
    const state = await store.load();
    return state === null ? null : this.activation.restore(state);
  }

  /** Revoke the activation and immediately close all tool admission paths. */
  revokeActivation(reason?: string): AutonomousCapabilityActivationState {
    return this.activation.revoke(reason);
  }

  /** Return exact connector coverage for the requested routed domains without dispatching anything. */
  connectorCoverage(domains: readonly AutonomousDomainName[], options: { capability?: string | null } = {}): JsonObject {
    if (!this.connectorRegistry) return { status: "connector_registry_required", domains: [...domains], capability: options.capability ?? null, execution: "planning_only;no_dispatch;no_authorization", secret_material: "never_returned" };
    return this.connectorRegistry.planForDomains(domains, options);
  }

  /** Select a digest-bound connector portfolio using deterministic or evaluator-backed evidence. */
  selectConnectors(
    domains: readonly AutonomousDomainName[],
    options: { capability?: string | null; strategy?: "lexicographic_connector_id" | "weighted_evidence"; selectionSignals?: Readonly<Record<string, JsonObject>> } = {},
  ): AutonomousConnectorSelectionPlan {
    if (!this.connectorRegistry) throw new ArgumentError("AutonomousAgent has no connector registry");
    return this.connectorRegistry.selectForDomains(domains, options);
  }

  /** Dispatch one already-reviewed connector request; external authority remains caller-owned. */
  async dispatchConnector(request: AutonomousConnectorDispatchRequest, options: { traceEventCallback?: AutonomousConnectorTraceEventCallback; authorizationContext?: AutonomousAuthorizationContext; authorizationDomain?: string; authorizationCapability?: string | null; authorizationRiskClass?: string | null } = {}): Promise<AutonomousConnectorDispatchResult> {
    if (!this.connectorRuntime) throw new ArgumentError("AutonomousAgent has no connector runtime");
    return this.connectorRuntime.dispatch(request, options);
  }

  /** Dispatch only when the digest-bound selection plan still matches the live connector catalogue. */
  async dispatchConnectorFromPlan(plan: AutonomousConnectorSelectionPlan | unknown, request: AutonomousConnectorDispatchRequest, options: { traceEventCallback?: AutonomousConnectorTraceEventCallback; authorizationContext?: AutonomousAuthorizationContext; authorizationDomain?: string; authorizationCapability?: string | null; authorizationRiskClass?: string | null } = {}): Promise<AutonomousConnectorDispatchResult> {
    if (!this.connectorRuntime) throw new ArgumentError("AutonomousAgent has no connector runtime");
    return this.connectorRuntime.dispatchFromPlan(plan, request, options);
  }

  /**
   * Execute an already-bound set of domain tool calls through the same registry, approval, and
   * effect boundary used by provider tool loops. Higher-level durable orchestrators use this
   * narrow method when a caller has resolved a mission step and wants the brain's exact tool
   * admission semantics without reaching into private runtime state.
   */
  async executeToolCalls(
    calls: readonly ProviderToolCall[],
    options: {
      domains: readonly string[];
      approveEffects?: boolean;
      execution?: AutonomousExecutionController;
      effectBoundary?: AutonomousEffectBoundary;
      workflowContext?: AutonomousWorkflowToolContext;
      authorizationContext?: AutonomousAuthorizationContext;
    },
  ): Promise<ProviderToolResult[]> {
    if (!Array.isArray(calls) || calls.length > 128) throw new ArgumentError("autonomous tool call count is outside its bounds");
    const runtime = this.toolRuntimeForRun() ?? (await this.ensureToolRegistry(), this.toolRuntimeForRun());
    if (!runtime) {
      return calls.map((call) => ({
        callId: call.id,
        approved: false,
        isError: true,
        content: { status: "authorization_required", tool: call.name, secret_material: "never_returned" },
      }));
    }
    return this.dispatchActivatedToolCalls(calls, (allowed) => runtime.authorizeAndExecute(allowed, {
      domains: options.domains,
      approveEffects: options.approveEffects,
      execution: options.execution,
      effectBoundary: options.effectBoundary ?? this.effectBoundary,
      workflowContext: options.workflowContext,
      authorizationContext: options.authorizationContext,
    }));
  }

  /**
   * Execute one reviewed capability with a replayable, evaluator-facing result envelope.
   * The returned `value` is transient; persist only `result.record` when building durable
   * memory, learning, or workflow checkpoints.
   */
  async executeCapability(
    request: AutonomousCapabilityExecutionRequest,
    options: AutonomousCapabilityExecutionOptions = {},
  ): Promise<AutonomousCapabilityExecutionResult> {
    const runtime = await this.ensureCapabilityRuntime();
    if (!runtime) return autonomousCapabilityRefusal(request, "authorization_required");
    return runtime.execute(request, options);
  }

  /** Execute an ordered capability batch with explicit omissions after a terminal failure. */
  async executeCapabilityBatch(
    requests: readonly AutonomousCapabilityExecutionRequest[],
    options: AutonomousCapabilityBatchOptions = {},
  ): Promise<AutonomousCapabilityBatchResult> {
    if (!Array.isArray(requests) || requests.length < 1 || requests.length > 64) throw new ArgumentError("capability batch must contain 1..=64 requests");
    const runtime = await this.ensureCapabilityRuntime();
    if (!runtime) {
      const items = await Promise.all(requests.map(async (request, index) => {
        const result = await autonomousCapabilityRefusal(request, "authorization_required");
        return { index, request_digest: result.record.request_digest, result, omission_reason: null as null };
      }));
      return {
        schema: AUTONOMOUS_CAPABILITY_BATCH_SCHEMA,
        batch_digest: await digestJson(items.map((item) => item.result.record)),
        status: "partial",
        items,
        completed_count: 0,
        failed_count: items.length,
        omitted_count: 0,
        execution: "ordered_serial",
        durable_projection: "records_and_digests_only",
        secret_material: "never_returned",
      };
    }
    return runtime.executeBatch(requests, options);
  }

  /** Restore metadata-only capability records and make completed calls replayable without redispatch. */
  async restoreCapabilityJournal(): Promise<{ restored: number; replayable: number; value_retention: "transient_caller_value_only" }> {
    const runtime = await this.ensureCapabilityRuntime();
    if (!runtime) return { restored: 0, replayable: 0, value_retention: "transient_caller_value_only" };
    return runtime.rehydrate();
  }

  /** Restore durable capability metadata and reopen the in-process replay barrier. */
  async restoreCapabilityJournalPersistence(): Promise<Record<string, unknown>> {
    if (this.capabilityJournalPersistence === undefined) throw new ArgumentError("restoreCapabilityJournalPersistence requires configured persistence");
    const persisted = await this.capabilityJournalPersistence.restore();
    const rehydrated = await this.restoreCapabilityJournal();
    return {
      schema: "bioprism-typescript-autonomous-capability-journal-snapshot/0.2",
      ...persisted,
      rehydrated: rehydrated.restored,
      replayable: rehydrated.replayable,
      value_retention: rehydrated.value_retention,
      retention: "metadata_only;caller_values_not_restored",
    };
  }

  /** Flush capability replay metadata without returning journal entries. */
  async flushCapabilityJournalPersistence(): Promise<Record<string, unknown>> {
    if (this.capabilityJournalPersistence === undefined) throw new ArgumentError("flushCapabilityJournalPersistence requires configured persistence");
    return this.capabilityJournalPersistence.flush();
  }

  /** Restore the metadata-only execution checkpoint used by long-horizon recovery. */
  async restoreExecutionPersistence(): Promise<Record<string, unknown>> {
    if (this.executionPersistence === undefined) throw new ArgumentError("restoreExecutionPersistence requires configured persistence");
    const snapshot = await this.executionPersistence.restore();
    if (snapshot === null) return { restored: false, snapshot_digest: null, events: 0, retention: "metadata_only" };
    return {
      restored: true,
      schema: snapshot.schema,
      snapshot_digest: snapshot.snapshot_digest,
      head_digest: snapshot.head_digest,
      events: snapshot.rows.length,
      retention: "metadata_only_hash_chained",
    };
  }

  /** Flush the execution journal while projecting only checkpoint metadata. */
  async flushExecutionPersistence(): Promise<Record<string, unknown>> {
    if (this.executionPersistence === undefined) throw new ArgumentError("flushExecutionPersistence requires configured persistence");
    const snapshot = await this.executionPersistence.flush();
    return {
      schema: snapshot.schema,
      snapshot_digest: snapshot.snapshot_digest,
      head_digest: snapshot.head_digest,
      events: snapshot.rows.length,
      retention: "metadata_only_hash_chained",
    };
  }

  /** Restore metadata-only route/planning/evaluation checkpoints for all persisted cycles. */
  async restoreDecisionCyclePersistence(): Promise<Record<string, unknown>> {
    if (this.decisionCyclePersistence === undefined) throw new ArgumentError("restoreDecisionCyclePersistence requires configured persistence");
    const snapshot = await this.decisionCyclePersistence.restore();
    if (snapshot === null) return { restored: false, snapshot_digest: null, cycles: 0, terminal_cycles: 0, retention: "metadata_only_hash_bound" };
    return {
      restored: true,
      schema: snapshot.schema,
      snapshot_digest: snapshot.snapshot_digest,
      cycles: snapshot.states.length,
      terminal_cycles: snapshot.states.filter((state) => state.phase === "terminal").length,
      retention: "metadata_only_hash_bound",
    };
  }

  /** Flush decision-cycle checkpoints without returning task, prompt, or provider payloads. */
  async flushDecisionCyclePersistence(): Promise<Record<string, unknown>> {
    if (this.decisionCyclePersistence === undefined) throw new ArgumentError("flushDecisionCyclePersistence requires configured persistence");
    const snapshot = await this.decisionCyclePersistence.flush();
    return {
      schema: snapshot.schema,
      snapshot_digest: snapshot.snapshot_digest,
      cycles: snapshot.states.length,
      terminal_cycles: snapshot.states.filter((state) => state.phase === "terminal").length,
      retention: "metadata_only_hash_bound",
    };
  }

  /** Return metadata-only capability records produced by this agent instance. */
  capabilityExecutionEvidence(): AutonomousCapabilityExecutionRecord[] {
    return this.capabilityRuntime?.executionEvidence() ?? [];
  }

  /** Evaluate and settle one reviewed capability result; transport success never becomes reward. */
  async evaluateCapabilityExecution(
    result: AutonomousCapabilityExecutionResult | AutonomousCapabilityExecutionRecord,
    options: Omit<AutonomousCapabilityLearningOptions, "recordEvaluatorReward"> & { toolSelectionState?: AutonomousToolSelectionState | null },
  ): Promise<AutonomousAgentCapabilityLearningResult> {
    if (!this.learner) throw new ArgumentError("AutonomousAgent has no AutonomousOnlineLearner");
    const { toolSelectionState, ...learningOptions } = options;
    const callerProvidedToolSelectionState = toolSelectionState !== undefined;
    const effectiveToolSelectionState = callerProvidedToolSelectionState ? toolSelectionState : this.effectiveToolSelectionState();
    const settlement = await settleAutonomousCapabilityLearning(result, {
      ...learningOptions,
      settlementStore: learningOptions.settlementStore ?? this.capabilityLearningSettlementStore,
      recordEvaluatorReward: (armId, reward, update) => this.recordEvaluatorReward(armId, reward, {
        failed: update.failed,
        outcomeDigest: update.outcomeDigest,
        contractDigest: update.contractDigest,
        contextDigest: update.contextDigest,
        context: update.context,
      }),
    });
    this.learner.restore(settlement.next_state);
    const record = ("record" in result ? (result as AutonomousCapabilityExecutionResult).record : result) as AutonomousCapabilityExecutionRecord;
    const nextToolSelectionState = settleAutonomousToolSelectionOutcome(effectiveToolSelectionState, {
      domain: record.domain as AutonomousDomainName,
      capability: record.capability ?? "capability_execution",
      tool: record.tool,
      reward: settlement.reward,
      failed: settlement.failed,
      latencyMs: record.duration_ms,
      outcomeDigest: settlement.outcome_digest,
    });
    if (!callerProvidedToolSelectionState && this.toolSelectionConfigured) this.toolSelectionStateValue = nextToolSelectionState;
    return { ...settlement, tool_selection_state: nextToolSelectionState, tool_selection_state_digest: await digestJson(nextToolSelectionState) };
  }

  /** Evaluate and settle reviewed capability results in input order with one bandit stream. */
  async evaluateCapabilityExecutions(
    results: readonly (AutonomousCapabilityExecutionResult | AutonomousCapabilityExecutionRecord)[],
    options: AutonomousCapabilityLearningBatchOptions & { toolSelectionState?: AutonomousToolSelectionState | null },
  ): Promise<AutonomousAgentCapabilityLearningBatchResult> {
    if (!this.learner) throw new ArgumentError("AutonomousAgent has no AutonomousOnlineLearner");
    const { toolSelectionState, ...learningOptions } = options;
    const callerProvidedToolSelectionState = toolSelectionState !== undefined;
    const effectiveToolSelectionState = callerProvidedToolSelectionState ? toolSelectionState : this.effectiveToolSelectionState();
    const settlement = await settleAutonomousCapabilityLearningBatch(results, {
      ...learningOptions,
      settlementStore: learningOptions.settlementStore ?? this.capabilityLearningSettlementStore,
      recordEvaluatorReward: (armId, reward, update) => this.recordEvaluatorReward(armId, reward, {
        failed: update.failed,
        outcomeDigest: update.outcomeDigest,
        contractDigest: update.contractDigest,
        contextDigest: update.contextDigest,
        context: update.context,
      }),
    });
    for (const item of settlement.settlements) this.learner.restore(item.next_state);
    let nextToolSelectionState = normalizeAutonomousToolSelectionState(effectiveToolSelectionState);
    for (const [index, item] of settlement.settlements.entries()) {
      const record = ("record" in results[index]! ? (results[index] as AutonomousCapabilityExecutionResult).record : results[index]) as AutonomousCapabilityExecutionRecord;
      nextToolSelectionState = settleAutonomousToolSelectionOutcome(nextToolSelectionState, {
        domain: record.domain as AutonomousDomainName,
        capability: record.capability ?? "capability_execution",
        tool: record.tool,
        reward: item.reward,
        failed: item.failed,
        latencyMs: record.duration_ms,
        outcomeDigest: item.outcome_digest,
      });
    }
    if (!callerProvidedToolSelectionState && this.toolSelectionConfigured) this.toolSelectionStateValue = nextToolSelectionState;
    return { ...settlement, tool_selection_state: nextToolSelectionState, tool_selection_state_digest: await digestJson(nextToolSelectionState) };
  }

  /** Return metadata-only adapter evidence collected by this agent; raw arguments/results are never exposed here. */
  toolExecutionEvidence(): AutonomousDomainToolExecutionReceipt[] {
    return this.domainToolRuntime?.receiptsSnapshot() ?? [];
  }

  /**
   * Evaluate live tool receipts through an independent value-only evaluator and advance the
   * caller-owned adaptive tool-selection state. Provider/tool transport status is deliberately
   * not converted into reward; only the evaluator assessment can create bandit credit.
   */
  async evaluateToolReceipts(
    options: {
      evaluator: AutonomousToolOutcomeEvaluator;
      receipts?: readonly AutonomousDomainToolExecutionReceipt[];
      evidence?: Readonly<Record<string, JsonObject>>;
      toolSelectionState?: AutonomousToolSelectionState | null;
    },
  ): Promise<AutonomousToolLearningReport> {
    if (!this.domainToolRuntime) throw new ArgumentError("evaluateToolReceipts requires a configured domain tool runtime");
    if (!(options?.evaluator instanceof AutonomousToolOutcomeEvaluator)) throw new ArgumentError("evaluateToolReceipts requires an AutonomousToolOutcomeEvaluator");
    const receipts = options.receipts === undefined ? this.domainToolRuntime.receiptsSnapshot() : [...options.receipts];
    const callerProvidedToolSelectionState = options.toolSelectionState !== undefined;
    const effectiveToolSelectionState = callerProvidedToolSelectionState ? options.toolSelectionState : this.effectiveToolSelectionState();
    const report = await options.evaluator.evaluateReceipts(receipts, {
      evidence: options.evidence,
      toolSelectionState: effectiveToolSelectionState,
      toolSelectionUpdater: settleAutonomousToolSelectionOutcome,
    });
    if (!callerProvidedToolSelectionState && this.toolSelectionConfigured && report.next_tool_selection_state !== null) this.toolSelectionStateValue = normalizeAutonomousToolSelectionState(report.next_tool_selection_state);
    return report;
  }

  /**
   * Evaluate provider invocation receipts and feed explicit model-quality credit into the
   * configured online learner. Provider transport success is never treated as task quality;
   * only the caller-owned evaluator can create a model-arm update.
   */
  async evaluateProviderReceipts(
    options: {
      evaluator: AutonomousProviderOutcomeEvaluator;
      receipts?: readonly AutonomousProviderInvocationReceipt[];
      contexts?: Readonly<Record<string, AutonomousProviderOutcomeContext>>;
      evidence?: Readonly<Record<string, JsonObject>>;
      learningState?: JsonObject | null;
      learning?: boolean;
      learningUpdater?: AutonomousProviderLearningUpdater;
    },
  ): Promise<AutonomousProviderLearningReport> {
    if (!(options?.evaluator instanceof AutonomousProviderOutcomeEvaluator)) throw new ArgumentError("evaluateProviderReceipts requires an AutonomousProviderOutcomeEvaluator");
    const receipts = options.receipts === undefined ? [] : [...options.receipts];
    const updater = options.learningUpdater ?? (options.learning === false || !this.learner
      ? undefined
      : (armId, reward, update) => this.recordEvaluatorReward(armId, reward, {
        failed: update.failed,
        outcomeDigest: update.outcomeDigest,
        contractDigest: update.contractDigest,
        contextDigest: update.contextDigest,
        context: update.context,
      }));
    const learningState = options.learningState !== undefined
      ? options.learningState
      : updater && this.learner
        ? this.learner.snapshot()
        : null;
    return options.evaluator.evaluateReceipts(receipts, {
      contexts: options.contexts,
      evidence: options.evidence,
      learningState,
      learningUpdater: updater,
    });
  }

  /** Discover live provider model metadata and atomically reconcile it into this agent's catalogue. */
  async refreshModels(
    provider: string,
    defaults: AutonomousModelCandidateDefaults,
    options: { credential?: CredentialHandle; signal?: AbortSignal; replaceExisting?: boolean } = {},
  ): Promise<AutonomousModelRefreshResult> {
    const normalizedProvider = boundedText("autonomous model refresh provider", provider, 128);
    const discovery = await this.llm.discoverModels(normalizedProvider, { credential: options.credential, signal: options.signal });
    const candidates = discovery.models.length === 0 ? [] : providerModelsToCandidates(discovery.models, defaults);
    if (candidates.some((candidate) => candidate.provider !== normalizedProvider)) throw new ProviderRuntimeError("provider model discovery returned a candidate for a different provider");
    const ids = candidates.map((candidate) => `${candidate.provider}/${candidate.model}`);
    const discoveredIds = new Set(ids);
    const existing = new Set(this.modelsById.keys());
    const replaced = options.replaceExisting === true ? ids.filter((id) => existing.has(id)) : [];
    const registered = ids.filter((id) => !existing.has(id));
    const removed = options.replaceExisting === true
      ? [...existing].filter((id) => id.startsWith(`${normalizedProvider}/`) && !discoveredIds.has(id)).sort()
      : [];
    const reconciled = candidates.length === 0
      ? []
      : this.registerModels(candidates, { replaceExisting: options.replaceExisting === true });
    for (const id of removed) this.modelsById.delete(id);
    return {
      schema: AUTONOMOUS_MODEL_REFRESH_SCHEMA,
      provider: normalizedProvider,
      discovered_model_count: discovery.model_count,
      candidate_count: reconciled.length,
      candidates: reconciled,
      registered_model_ids: registered,
      replaced_model_ids: replaced,
      removed_model_ids: removed,
      discovery,
      execution: "not_started;catalogue_registration_only",
      retention: "model_metadata_only;credentials_and_raw_catalogue_not_retained",
      secret_material: "never_returned",
    };
  }

  /**
   * Discover and reconcile several provider catalogues through one bounded, redacted operation.
   * Each provider is reconciled atomically by `refreshModels`; one unavailable provider therefore
   * cannot erase a healthy provider's catalogue. Credentials stay in the caller's resolver and
   * failures contain only stable classes/codes, never provider bodies or secret-bearing messages.
   */
  async refreshModelCatalogue(
    specs: readonly AutonomousModelRefreshSpec[],
    options: {
      credentialFor?: (provider: string) => CredentialHandle | undefined;
      signal?: AbortSignal;
      replaceExisting?: boolean;
      maxParallel?: number;
      stopOnError?: boolean;
    } = {},
  ): Promise<AutonomousModelCatalogueRefreshResult> {
    if (!Array.isArray(specs) || specs.length < 1 || specs.length > AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS) throw new ArgumentError(`autonomous model catalogue refresh must contain 1..=${AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS} providers`);
    if (options.credentialFor !== undefined && typeof options.credentialFor !== "function") throw new ArgumentError("autonomous model catalogue credentialFor must be callable");
    const maxParallel = options.maxParallel ?? Math.min(4, specs.length);
    if (!Number.isSafeInteger(maxParallel) || maxParallel < 1 || maxParallel > 8) throw new ArgumentError("autonomous model catalogue maxParallel is outside its bounds");
    const normalized = specs.map((spec) => {
      if (!isObject(spec)) throw new ArgumentError("autonomous model catalogue refresh spec must be an object");
      const provider = boundedText("autonomous model catalogue refresh provider", spec.provider, 128);
      if (!/^[A-Za-z0-9_.-]+$/.test(provider)) throw new ArgumentError("autonomous model catalogue refresh provider must be a bounded identifier");
      if (!isObject(spec.defaults)) throw new ArgumentError(`autonomous model catalogue defaults for ${provider} are malformed`);
      return { provider, defaults: spec.defaults as unknown as AutonomousModelCandidateDefaults };
    });
    if (new Set(normalized.map((spec) => spec.provider)).size !== normalized.length) throw new ArgumentError("autonomous model catalogue refresh providers must be unique");
    const refreshes: Array<AutonomousModelRefreshResult | null> = Array.from({ length: normalized.length }, () => null);
    const failures: Array<AutonomousModelRefreshFailure | null> = Array.from({ length: normalized.length }, () => null);
    const refreshOne = async (index: number): Promise<void> => {
      const spec = normalized[index]!;
      try {
        refreshes[index] = await this.refreshModels(spec.provider, spec.defaults, {
          credential: options.credentialFor?.(spec.provider),
          signal: options.signal,
          replaceExisting: options.replaceExisting,
        });
      } catch (error) {
        const failureCode = error instanceof ProviderRuntimeError ? error.code : error instanceof Error && error.constructor.name.trim() ? error.constructor.name : "UnknownError";
        const errorClass = error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name) ? error.constructor.name : "ProviderRefreshError";
        failures[index] = {
          provider: spec.provider,
          error_class: errorClass,
          failure_code: failureCode,
          retryable: error instanceof ProviderRuntimeError ? error.retryable : false,
        };
        if (options.stopOnError === true) throw error;
      }
    };
    if (options.stopOnError === true || maxParallel === 1) {
      for (let index = 0; index < normalized.length; index += 1) await refreshOne(index);
    } else {
      let next = 0;
      const worker = async (): Promise<void> => {
        while (true) {
          const index = next;
          next += 1;
          if (index >= normalized.length) return;
          await refreshOne(index);
        }
      };
      await Promise.all(Array.from({ length: Math.min(maxParallel, normalized.length) }, () => worker()));
    }
    const completed = refreshes.filter((refresh): refresh is AutonomousModelRefreshResult => refresh !== null);
    const failed = failures.filter((failure): failure is AutonomousModelRefreshFailure => failure !== null);
    return {
      schema: AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA,
      status: failed.length === 0 ? "completed" : completed.length === 0 ? "failed" : "partial",
      requested_provider_count: normalized.length,
      successful_provider_count: completed.length,
      failed_provider_count: failed.length,
      refreshes: completed,
      failures: failed,
      execution: "catalogue_registration_only",
      retention: "model_metadata_only;credentials_and_raw_catalogue_not_retained",
      secret_material: "never_returned",
    };
  }

  /**
   * Refresh provider inventory and calculate all-domain model readiness through the same agent.
   * The dynamic import keeps the inventory coordinator optional for lightweight embeddings while
   * preserving a single public entry point for protected-session onboarding flows.
   */
  async refreshModelInventory(
    specs: readonly AutonomousModelRefreshSpec[],
    options: AutonomousModelInventoryRefreshOptions = {},
  ): Promise<AutonomousModelInventorySnapshot> {
    const { AutonomousModelInventoryCoordinator } = await import("./autonomous-model-inventory.js");
    const coordinator = this.modelInventoryCoordinator ??= new AutonomousModelInventoryCoordinator(this);
    return coordinator.refresh(specs, options);
  }

  /**
   * Return the current model catalogue's live all-domain eligibility without discovery or
   * invocation. This is safe for onboarding/readiness screens before a user supplies a key.
   */
  async modelInventoryReadiness(
    options: AutonomousModelInventoryReadinessOptions = {},
  ): Promise<AutonomousModelInventoryReadiness> {
    const { AutonomousModelInventoryCoordinator } = await import("./autonomous-model-inventory.js");
    const coordinator = this.modelInventoryCoordinator ??= new AutonomousModelInventoryCoordinator(this);
    return coordinator.readiness(options);
  }

  /**
   * Restore the last validated model inventory into this agent and retain its CAS fence for the
   * next refresh. Provider registrations and credential handles are intentionally not restored;
   * the snapshot contains only model metadata and redacted all-domain coverage.
   */
  async restoreModelInventory(
    persistence: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence,
  ): Promise<AutonomousModelInventorySnapshot | null> {
    const { AutonomousModelInventoryCoordinator } = await import("./autonomous-model-inventory.js");
    const coordinator = this.modelInventoryCoordinator ??= new AutonomousModelInventoryCoordinator(this);
    return coordinator.restore(persistence);
  }

  /** Re-commit the last validated inventory image without rediscovering provider models. */
  async flushModelInventory(
    persistence: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence,
  ): Promise<AutonomousModelInventorySnapshot | null> {
    const { AutonomousModelInventoryCoordinator } = await import("./autonomous-model-inventory.js");
    const coordinator = this.modelInventoryCoordinator ??= new AutonomousModelInventoryCoordinator(this);
    return coordinator.flush(persistence);
  }

  private async persistenceLifecycleFor(options: {
    modelInventoryPersistence?: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence;
    activationStore?: AutonomousCapabilityActivationSnapshotStore;
    selectionPromotionStore?: AutonomousSelectionLifecycleStore;
    capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
    decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
    executionPersistence?: AutonomousExecutionPersistenceCoordinator;
    requireAll?: boolean;
    continueOnError?: boolean;
  } = {}): Promise<import("./autonomous-agent-lifecycle.js").AutonomousAgentPersistenceLifecycleCoordinator> {
    const { AutonomousAgentPersistenceLifecycleCoordinator } = await import("./autonomous-agent-lifecycle.js");
    const requireAll = options.requireAll ?? false;
    const continueOnError = options.continueOnError ?? false;
    if (
      this.persistenceLifecycleCoordinator === undefined
      || this.persistenceLifecycleModelInventoryPersistence !== options.modelInventoryPersistence
      || this.persistenceLifecycleActivationStore !== options.activationStore
      || this.persistenceLifecycleSelectionPromotionStore !== options.selectionPromotionStore
      || this.persistenceLifecycleCapabilityJournalPersistence !== options.capabilityJournalPersistence
      || this.persistenceLifecycleDecisionCyclePersistence !== options.decisionCyclePersistence
      || this.persistenceLifecycleExecutionPersistence !== options.executionPersistence
      || this.persistenceLifecycleRequireAll !== requireAll
      || this.persistenceLifecycleContinueOnError !== continueOnError
    ) {
      this.persistenceLifecycleCoordinator = new AutonomousAgentPersistenceLifecycleCoordinator(this, {
        modelInventoryPersistence: options.modelInventoryPersistence,
        activationStore: options.activationStore,
        selectionPromotionStore: options.selectionPromotionStore,
        capabilityJournalPersistence: options.capabilityJournalPersistence,
        decisionCyclePersistence: options.decisionCyclePersistence,
        executionPersistence: options.executionPersistence,
        requireAll,
        continueOnError,
      });
      this.persistenceLifecycleModelInventoryPersistence = options.modelInventoryPersistence;
      this.persistenceLifecycleActivationStore = options.activationStore;
      this.persistenceLifecycleSelectionPromotionStore = options.selectionPromotionStore;
      this.persistenceLifecycleCapabilityJournalPersistence = options.capabilityJournalPersistence;
      this.persistenceLifecycleDecisionCyclePersistence = options.decisionCyclePersistence;
      this.persistenceLifecycleExecutionPersistence = options.executionPersistence;
      this.persistenceLifecycleRequireAll = requireAll;
      this.persistenceLifecycleContinueOnError = continueOnError;
    }
    return this.persistenceLifecycleCoordinator;
  }

  /** Restore all configured metadata coordinators in the reviewed dependency order. */
  async restorePersistedState(options: {
    modelInventoryPersistence?: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence;
    activationStore?: AutonomousCapabilityActivationSnapshotStore;
    selectionPromotionStore?: AutonomousSelectionLifecycleStore;
    capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
    decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
    executionPersistence?: AutonomousExecutionPersistenceCoordinator;
    strict?: boolean;
    requireAll?: boolean;
    continueOnError?: boolean;
  } = {}): Promise<import("./autonomous-agent-lifecycle.js").AutonomousAgentPersistenceLifecycleReport> {
    const coordinator = await this.persistenceLifecycleFor(options);
    return coordinator.restore({ strict: options.strict, continueOnError: options.continueOnError });
  }

  /** Flush all configured metadata coordinators in reverse dependency order. */
  async flushPersistedState(options: {
    modelInventoryPersistence?: import("./autonomous-model-inventory.js").AutonomousModelInventoryPersistence;
    activationStore?: AutonomousCapabilityActivationSnapshotStore;
    selectionPromotionStore?: AutonomousSelectionLifecycleStore;
    capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
    decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
    executionPersistence?: AutonomousExecutionPersistenceCoordinator;
    strict?: boolean;
    requireAll?: boolean;
    continueOnError?: boolean;
  } = {}): Promise<import("./autonomous-agent-lifecycle.js").AutonomousAgentPersistenceLifecycleReport> {
    const coordinator = await this.persistenceLifecycleFor(options);
    return coordinator.flush({ strict: options.strict, continueOnError: options.continueOnError });
  }

  async profiles(): Promise<AutonomousDomainProfile[]> {
    return builtinAutonomousDomainProfiles();
  }

  /**
   * Project the complete keyless readiness posture for the autonomous brain.
   *
   * This is deliberately an application-local audit: it reads registered provider metadata,
   * opaque credential status, model priors, persisted health, and the optional tool catalogue.
   * It never discovers models, contacts a provider, executes a tool, or returns a credential.
   * Every built-in domain receives an independent capability/tool/learning row so onboarding can
   * tell a caller exactly what must happen before approval and dispatch.
   */
  async readiness(options: {
    candidates?: readonly AutonomousModelCandidate[];
    estimatedInputTokens?: number;
    requestedOutputTokens?: number;
    calibrationReport?: AutonomousEvaluatorCalibrationReport;
    /** Exact digest of a report previously registered and restored into this agent. */
    calibrationReportDigest?: string;
    requireCalibratedLearning?: boolean;
    selectionPromotionReport?: AutonomousSelectionPromotionReport;
    requirePromotedSelection?: boolean;
    evidenceReadiness?: {
      registry: AutonomousEvidenceAdapterRegistry;
      healthStore?: AutonomousEvidenceAdapterHealthStore;
      options?: AutonomousEvidenceReadinessAuditOptions;
    };
  } = {}): Promise<AutonomousReadinessReport> {
    const estimatedInputTokens = options.estimatedInputTokens ?? 4_096;
    const requestedOutputTokens = options.requestedOutputTokens ?? 1_024;
    for (const [name, value] of [["estimatedInputTokens", estimatedInputTokens], ["requestedOutputTokens", requestedOutputTokens]] as const) {
      if (!Number.isSafeInteger(value) || value < 1 || value > 10_000_000) throw new ArgumentError(`autonomous readiness ${name} is outside its bounds`);
    }
    if (options.requireCalibratedLearning !== undefined && typeof options.requireCalibratedLearning !== "boolean") throw new ArgumentError("autonomous readiness requireCalibratedLearning must be boolean");
    if (options.calibrationReport !== undefined && options.calibrationReportDigest !== undefined) throw new ArgumentError("autonomous readiness accepts calibrationReport or calibrationReportDigest, not both");
    const storedCalibrationReport = options.calibrationReportDigest === undefined
      ? undefined
      : this.evaluatorCalibrationReport(options.calibrationReportDigest);
    if (options.calibrationReportDigest !== undefined && storedCalibrationReport === null) throw new ArgumentError("autonomous readiness calibrationReportDigest was not found in the registry");
    const suppliedCalibrationReport = options.calibrationReport ?? storedCalibrationReport ?? undefined;
    if (options.requireCalibratedLearning === true && suppliedCalibrationReport === undefined) throw new ArgumentError("autonomous readiness requires calibrationReport or calibrationReportDigest when calibrated learning is required");
    if (options.requirePromotedSelection !== undefined && typeof options.requirePromotedSelection !== "boolean") throw new ArgumentError("autonomous readiness requirePromotedSelection must be boolean");
    const calibrationRuntime = suppliedCalibrationReport === undefined ? null : await import("./autonomous-evaluator-calibration.js");
    const calibrationReport = suppliedCalibrationReport === undefined ? null : calibrationRuntime!.validateAutonomousEvaluatorCalibrationReport(suppliedCalibrationReport);
    const selectionPromotionRuntime = options.selectionPromotionReport === undefined ? null : await import("./autonomous-selection-promotion.js");
    const selectionPromotionReport = options.selectionPromotionReport === undefined ? null : selectionPromotionRuntime!.validateAutonomousSelectionPromotionReport(options.selectionPromotionReport);
    const selectionPromotionState = this.selectionPromotion?.state ?? null;
    const selectionPromotionAdmitted = selectionPromotionState?.status === "admitted"
      && selectionPromotionState.active_promotion_digest !== null
      && (selectionPromotionReport === null || selectionPromotionState.active_promotion_digest === selectionPromotionReport.promotion_digest);
    const selectionPromotionBlocks = options.requirePromotedSelection === true && !selectionPromotionAdmitted;
    const candidates = (options.candidates === undefined ? this.models() : [...options.candidates].map(normalizeAutonomousModelCandidate));
    if (candidates.length > AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) throw new ArgumentError(`autonomous readiness candidates must contain at most ${AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS} models`);
    const candidateIds = new Set<string>();
    for (const candidate of candidates) {
      const id = `${candidate.provider}/${candidate.model}`;
      if (candidateIds.has(id)) throw new ArgumentError(`autonomous readiness model ${id} is duplicated`);
      candidateIds.add(id);
    }
    const profiles = await builtinAutonomousDomainProfiles();
    const modelInventoryReadiness = await this.modelInventoryReadiness({
      candidates,
      estimatedInputTokens,
      requestedOutputTokens,
    });
    let evidenceReadinessReport: import("./autonomous-evidence-readiness.js").AutonomousEvidenceReadinessReport | null = null;
    if (options.evidenceReadiness !== undefined) {
      const { AutonomousEvidenceReadinessAuditor } = await import("./autonomous-evidence-readiness.js");
      const { AutonomousEvidenceAdapterRegistry } = await import("./autonomous-evidence-adapters.js");
      if (!(options.evidenceReadiness.registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("autonomous readiness evidence registry is malformed");
      const requestedEvidenceDomains = profiles.map((profile) => profile.domain);
      const auditor = new AutonomousEvidenceReadinessAuditor(options.evidenceReadiness.registry, options.evidenceReadiness.healthStore);
      evidenceReadinessReport = await auditor.audit(requestedEvidenceDomains, options.evidenceReadiness.options ?? {});
    }
    const evidenceReadinessByDomain = evidenceReadinessReport === null ? null : new Map(evidenceReadinessReport.domains.map((row) => [row.domain, row]));
    const metadataByProvider = new Map(this.llm.providerMetadata().map((row) => [String(row.provider), row]));
    const providerNames = [...new Set([...metadataByProvider.keys(), ...candidates.map((candidate) => candidate.provider)])].sort();
    const providerRows: AutonomousReadinessProvider[] = [];
    const providerState = new Map<string, { registered: boolean; credentialReady: boolean; circuit: string; requiresCredential: boolean | null; nextAction: string }>();
    for (const provider of providerNames) {
      const metadata = metadataByProvider.get(provider);
      const registered = metadata !== undefined;
      const requiresCredential = typeof metadata?.requires_credential === "boolean" ? metadata.requires_credential : null;
      const credential = this.llm.credentials.status(provider, registered);
      const health = registered ? this.llm.providerStatus(provider) : null;
      const credentialReady = requiresCredential === false || credential.ready === true;
      const circuit = health?.circuit ?? "unconfigured";
      const nextAction = !registered ? "register_provider" : credentialReady ? "ready" : "collect_user_credential";
      providerState.set(provider, { registered, credentialReady, circuit, requiresCredential, nextAction });
      providerRows.push({
        provider,
        provider_registered: registered,
        requires_credential: requiresCredential,
        credential_ready: credentialReady,
        circuit,
        next_action: nextAction,
        credential: { ready: credentialReady, active_handles: credential.active_handles, expires_at: credential.expires_at, next_action: nextAction },
        health: health ? { attempts: health.attempts, successes: health.successes, failures: health.failures, success_rate: health.success_rate, mean_latency_ms: health.mean_latency_ms, last_model: health.last_model, last_status_code: health.last_status_code } : null,
        secret_material: "never_returned",
      });
    }
    const toolNames = new Set(this.toolCatalogue?.definitions.map((definition) => definition.name) ?? []);
    const modelRows: AutonomousReadinessModel[] = candidates.map((candidate) => ({ provider: candidate.provider, model: candidate.model, enabled: candidate.enabled !== false, provider_registered: providerState.get(candidate.provider)?.registered === true, credential_ready: providerState.get(candidate.provider)?.credentialReady === true, compatible_domains: [], eligible_domains: [] }));
    const modelById = new Map(modelRows.map((row) => [`${row.provider}/${row.model}`, row]));
    const domainRows: AutonomousReadinessDomain[] = [];
    const capabilityRows: JsonObject[] = [];
    for (const profile of profiles) {
      const requiredCapabilities = [...profile.required_model_capabilities];
      const compatible: AutonomousModelCandidate[] = [];
      const incompatible: JsonObject[] = [];
      for (const candidate of candidates) {
        const missingCapabilities = requiredCapabilities.filter((required) => !(candidate.capabilities ?? []).includes(required));
        const capacityMissing = candidate.context_window_tokens < estimatedInputTokens + requestedOutputTokens;
        const outputMissing = candidate.max_output_tokens < requestedOutputTokens;
        const enabled = candidate.enabled !== false;
        if (enabled && !missingCapabilities.length && !capacityMissing && !outputMissing) compatible.push(candidate);
        else incompatible.push({ arm_id: `${candidate.provider}/${candidate.model}`, missing_capabilities: missingCapabilities, context_capacity_ok: !capacityMissing, output_capacity_ok: !outputMissing, enabled });
      }
      const eligible = compatible.filter((candidate) => {
        const state = providerState.get(candidate.provider);
        return state?.registered === true && state.circuit !== "open" && state.credentialReady;
      });
      for (const candidate of compatible) modelById.get(`${candidate.provider}/${candidate.model}`)?.compatible_domains.push(profile.domain);
      for (const candidate of eligible) modelById.get(`${candidate.provider}/${candidate.model}`)?.eligible_domains.push(profile.domain);
      const uniqueBindings = [...new Map(profile.tool_profile.bindings.map((binding) => [binding.name, binding])).values()];
      const missingTools = uniqueBindings.filter((binding) => !toolNames.has(binding.name)).map((binding) => binding.name).sort();
      const context = { domain: profile.domain, capability: profile.default_capability, risk_class: profile.risk_class, task_family: profile.workflow.workflow_id };
      const learningContextDigest = digestCanonicalJsonTextSync(JSON.stringify(context));
      const providerMissing = compatible.some((candidate) => providerState.get(candidate.provider)?.registered !== true);
      const credentialMissing = compatible.some((candidate) => {
        const state = providerState.get(candidate.provider);
        return state?.registered === true && state.credentialReady === false;
      });
      const calibrationAdmission = calibrationReport === null ? null : calibrationRuntime!.autonomousEvaluatorCalibrationAdmission(calibrationReport, profile.domain);
      const calibrationBlocks = options.requireCalibratedLearning === true && calibrationAdmission?.decision !== "admit_learning";
      const selectionPromotionDomain = selectionPromotionReport?.domains.find((row) => row.domain === profile.domain) ?? null;
      const evidenceReadiness = evidenceReadinessByDomain?.get(profile.domain);
      const evidenceBlocks = evidenceReadiness !== undefined && evidenceReadiness.status !== "ready";
      const baseState: AutonomousReadinessState = !candidates.length ? "model_catalogue_required" : !compatible.length ? "model_capability_gap" : eligible.length ? "ready_for_caller_approval" : credentialMissing ? "credential_required" : providerMissing ? "provider_registration_required" : "partial";
      const state: AutonomousReadinessState = calibrationBlocks || evidenceBlocks || selectionPromotionBlocks ? "partial" : baseState;
      const nextActions = new Set<string>();
      if (state === "model_catalogue_required") nextActions.add("register at least one model candidate with the reviewed domain capabilities");
      if (state === "model_capability_gap") nextActions.add(`register a model declaring: ${requiredCapabilities.join(", ")}`);
      if (state === "provider_registration_required") nextActions.add("register the provider transport before requesting a credential");
      if (state === "credential_required") nextActions.add("collect a short-lived user credential through ProviderOnboarding");
      if (missingTools.length) nextActions.add("attach and review the live tool catalogue; missing tools remain optional provider-only fallbacks until bound");
      if (!this.learner) nextActions.add("attach AutonomousOnlineLearner and settle only explicit evaluator rewards");
      if (calibrationBlocks) nextActions.add(`hold evaluator calibration before learning: ${calibrationAdmission!.reasons.join(", ")}`);
      if (selectionPromotionBlocks) nextActions.add(selectionPromotionState === null ? "attach and apply an admitted all-domain selection promotion report before enabling learned selection" : `resolve selection promotion lifecycle hold: ${selectionPromotionState.last_reason ?? selectionPromotionState.status}`);
      if (evidenceBlocks) nextActions.add(`resolve evidence readiness before source dispatch: ${evidenceReadiness!.reason}`);
      const row: AutonomousReadinessDomain = { domain: profile.domain, workflow_id: profile.workflow.workflow_id, workflow_digest: profile.workflow.workflow_digest, required_model_capabilities: requiredCapabilities, compatible_model_count: compatible.length, eligible_model_count: eligible.length, required_tool_count: uniqueBindings.length, available_tool_count: uniqueBindings.length - missingTools.length, missing_tools: missingTools, learning_context_digest: learningContextDigest, ...(evidenceReadiness === undefined ? {} : { evidence_readiness: { status: evidenceReadiness.status, reason: evidenceReadiness.reason, selected_adapter_id: evidenceReadiness.selected_adapter_id, selected_manifest_digest: evidenceReadiness.selected_manifest_digest, health: evidenceReadiness.health, report_digest: evidenceReadinessReport!.report_digest, execution: "readiness_projection_only;does_not_dispatch_source", secret_material: "never_returned" } }), ...(calibrationAdmission === null ? {} : { calibration_admission: { decision: calibrationAdmission.decision, report_digest: calibrationAdmission.report_digest, evaluator_id: calibrationAdmission.evaluator_id, evaluator_version: calibrationAdmission.evaluator_version, reasons: [...calibrationAdmission.reasons], execution: "readiness_projection_only;does_not_invoke_provider_or_mutate_learning", secret_material: "never_returned" } }), ...(selectionPromotionReport === null && selectionPromotionState === null ? {} : { selection_promotion: { decision: selectionPromotionReport?.decision ?? (selectionPromotionState?.last_decision === "admit" && selectionPromotionAdmitted ? "admit" : "hold"), status: selectionPromotionState?.status ?? "unapplied", promotion_digest: selectionPromotionReport?.promotion_digest ?? selectionPromotionState?.promotion_digest ?? null, active_promotion_digest: selectionPromotionState?.active_promotion_digest ?? null, source_report_digest: selectionPromotionReport?.source_report_digest ?? selectionPromotionState?.source_report_digest ?? null, domain_decision: selectionPromotionDomain?.decision ?? null, reasons: selectionPromotionDomain?.reasons ?? (selectionPromotionState?.last_reason ? [selectionPromotionState.last_reason] : []), execution: "readiness_projection_only;does_not_mutate_learner_or_invoke_provider", secret_material: "never_returned" } }), state, next_actions: [...nextActions].sort() };
      domainRows.push(row);
      capabilityRows.push({ domain: profile.domain, required_model_capabilities: requiredCapabilities, compatible_model_ids: compatible.map((candidate) => `${candidate.provider}/${candidate.model}`), incompatible_models: incompatible });
    }
    const learning = { configured: this.learner !== undefined, domain_count: profiles.length, contexts: domainRows.map((row) => ({ domain: row.domain, context_digest: row.learning_context_digest })), calibration: calibrationReport === null ? { configured: false, required: options.requireCalibratedLearning === true, report_digest: null, status: null, decision: options.requireCalibratedLearning === true ? "hold_learning" : "not_required", admitted_domain_count: 0, held_domain_count: options.requireCalibratedLearning === true ? profiles.length : 0 } : { configured: true, required: options.requireCalibratedLearning === true, report_digest: calibrationReport.report_digest, status: calibrationReport.status, decision: calibrationReport.gate.decision, admitted_domain_count: domainRows.filter((row) => row.calibration_admission?.decision === "admit_learning").length, held_domain_count: domainRows.filter((row) => row.calibration_admission?.decision !== "admit_learning").length }, selection_promotion: { configured: selectionPromotionReport !== null || selectionPromotionState !== null, required: options.requirePromotedSelection === true, report_digest: selectionPromotionReport?.promotion_digest ?? null, source_report_digest: selectionPromotionReport?.source_report_digest ?? selectionPromotionState?.source_report_digest ?? null, lifecycle_status: selectionPromotionState?.status ?? "unconfigured", active_promotion_digest: selectionPromotionState?.active_promotion_digest ?? null, decision: selectionPromotionAdmitted ? "admit" : selectionPromotionReport?.decision ?? "hold", admitted_domain_count: selectionPromotionReport?.domains.filter((row) => row.decision === "admit").length ?? 0, held_domain_count: selectionPromotionReport === null ? (selectionPromotionBlocks ? profiles.length : 0) : selectionPromotionReport.domains.filter((row) => row.decision === "hold").length }, feedback_contract: "explicit_evaluator_reward_only; transport_success_is_not_task_quality", retention: "value_only_learning_metadata" };
    const domainPacks = await Promise.all(profiles.map((profile) => buildDomainPack(profile)));
    const nextActions = new Set<string>(domainRows.flatMap((row) => row.next_actions));
    for (const row of providerRows) if (row.next_action !== "ready") nextActions.add(`${row.next_action}: ${row.provider}`);
    if (!this.toolCatalogue) nextActions.add("attach a live ToolCatalogue to compute exact domain-tool coverage");
    if (!this.learner) nextActions.add("attach AutonomousOnlineLearner and settle only explicit evaluator rewards");
    if (options.requireCalibratedLearning === true && learning.calibration.decision !== "admit_learning") nextActions.add("resolve evaluator calibration holdout coverage before enabling learning");
    if (selectionPromotionBlocks) nextActions.add("apply an admitted selection promotion report and persist its lifecycle state before enabling learned model selection");
    if (evidenceReadinessReport !== null && evidenceReadinessReport.status !== "ready") nextActions.add("resolve evidence routing readiness before source dispatch");
    const activation = this.activation.state;
    if (activation.status === "created" || activation.status === "provider_pending" || activation.status === "catalogue_pending") nextActions.add("refresh activation metadata, then review and explicitly approve proposed bindings");
    if (activation.status === "review_required" || activation.status === "partially_activated") nextActions.add("review the digest-bound activation plan and approve only the intended read-only bindings");
    if (activation.status === "stale") nextActions.add("reconcile the changed catalogue before approving or invoking tools");
    if (activation.status === "revoked") nextActions.add("create a new activation after explicit caller review");
    const distinctStates = new Set(domainRows.map((row) => row.state));
    const readinessState: AutonomousReadinessState = distinctStates.size === 1 ? [...distinctStates][0]! : "partial";
    const connectorReadiness = this.connectorRegistry
      ? { configured: true, registry_digest: this.connectorRegistry.digest, connector_count: this.connectorRegistry.registrations().length, execution: "selection_and_dispatch_require_explicit_plan_and_approval", secret_material: "never_returned" as const }
      : { configured: false, registry_digest: null, connector_count: 0, execution: "caller_owned_connector_registry_not_configured", secret_material: "never_returned" as const };
    const descriptor = { schema: AUTONOMOUS_READINESS_SCHEMA, providers: providerRows, models: [...modelRows].sort((left, right) => `${left.provider}/${left.model}`.localeCompare(`${right.provider}/${right.model}`)), domains: domainRows, workflows: profiles.map((profile) => profile.workflow), domain_packs: domainPacks, model_capability_coverage: { domain_count: capabilityRows.length, rows: capabilityRows, evidence_posture: "static_caller_declared_capabilities_only" }, model_inventory_readiness: modelInventoryReadiness, model_health: this.llm.modelHealthSnapshot(), learning, tooling: { configured: this.toolCatalogue !== undefined, catalogue_digest: this.toolCatalogue?.digest ?? null, available_tool_count: toolNames.size, execution: "catalogue_metadata_only; registration_is_not_authorization", activation_status: activation.status }, ...(evidenceReadinessReport === null ? {} : { evidence: { configured: true, registry_digest: evidenceReadinessReport.registry_digest, report_digest: evidenceReadinessReport.report_digest, status: evidenceReadinessReport.status, ready_count: evidenceReadinessReport.ready_count, degraded_count: evidenceReadinessReport.degraded_count, blocked_count: evidenceReadinessReport.blocked_count, missing_count: evidenceReadinessReport.missing_count, domains: evidenceReadinessReport.domains.map((row) => row.toJSON()), execution: "readiness_projection_only;no_source_dispatch", secret_material: "never_returned" } }), connectors: connectorReadiness, activation, next_actions: [...nextActions].sort(), readiness_state: readinessState, execution: "not_started; no_provider_or_tool_calls" as const, credential_posture: "caller_supplied_opaque_handles" as const, secret_material: "never_returned" as const };
    return { ...descriptor, readiness_digest: await digestJson(descriptor) };
  }

  /**
   * Preview the domain-scoped selector without contacting a provider, connector, or tool.
   *
   * The method compiles the same prompt/workflow identity used by ``run`` and then asks only
   * the local/value-only selection runtime for a ranking. The task and prompt remain transient;
   * the returned projection contains only model metadata, eligibility evidence, and digests.
   */
  async modelSelectionPreview(
    task: string,
    options: AutonomousModelSelectionPreviewOptions,
  ): Promise<AutonomousModelSelectionPreview> {
    const taskText = boundedText("autonomous model selection preview task", task, 32_000);
    if (!options || !AUTONOMOUS_DOMAIN_NAMES.includes(options.domain)) {
      throw new ArgumentError("autonomous model selection preview requires a built-in domain");
    }
    const estimatedInputTokens = options.estimatedInputTokens ?? 4_096;
    const requestedOutputTokens = options.requestedOutputTokens ?? 1_024;
    for (const [name, value, maximum] of [
      ["estimatedInputTokens", estimatedInputTokens, 10_000_000],
      ["requestedOutputTokens", requestedOutputTokens, 10_000_000],
    ] as const) {
      if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
        throw new ArgumentError(`autonomous model selection preview ${name} is outside its bounds`);
      }
    }
    const selectionWeights = normalizeAutonomousSelectionWeights(options.selectionWeights);
    const selectionObservations = normalizeAutonomousModelObservations(options.selectionObservations);
    const selectionObservationsDigest = await digestJson(selectionObservations);
    const normalizedContextBudget = options.contextBudget === undefined
      ? null
      : normalizeAutonomousContextBudget(options.contextBudget);
    const effectiveEstimatedInputTokens = normalizedContextBudget === null
      ? estimatedInputTokens
      : Math.min(estimatedInputTokens, normalizedContextBudget.maxInputTokens);
    const blueprintEnvelope = await this.blueprint(taskText, {
      domain: options.domain,
      capability: options.capability,
      context: options.context,
      maxInputTokens: effectiveEstimatedInputTokens,
    });
    const blueprint = blueprintEnvelope.blueprint;
    if (!blueprint || blueprintEnvelope.route.cross_domain) {
      throw new ProviderRuntimeError("autonomous model selection preview requires a single-domain blueprint");
    }
    const candidates = (options.candidates === undefined
      ? this.models()
      : [...options.candidates].map((candidate) => normalizeAutonomousModelCandidate(candidate)));
    if (!candidates.length) throw new ProviderRuntimeError("autonomous model selection preview requires model candidates");
    const messages: ProviderMessage[] = blueprint.prompt.messages.map((message) => ({
      role: message.role,
      content: message.content,
    }));
    const request: ProviderRequest = {
      model: "selection-preview",
      messages,
      maxOutputTokens: requestedOutputTokens,
    };
    const executionPlan: AutonomousExecutionPlan = {
      task: taskText,
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      riskClass: blueprint.domain_profile.risk_class,
      taskFamily: blueprint.selection_context.task_family ?? undefined,
      learningContextDigest: blueprint.learning_context_digest,
      requiredCapabilities: [...blueprint.required_capabilities],
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      minSelectionConfidence: options.minSelectionConfidence,
      selectionWeights,
      selectionObservations,
      contextBudget: normalizedContextBudget === null ? undefined : normalizedContextBudget,
      candidates,
      request,
    };
    const selection = await this.runtime.select(executionPlan);
    const eligibleCandidateCount = selection.ranking.filter((row) => row.eligible).length;
    const executionPlanDigest = await digestJson({
      task_digest: blueprint.task_digest,
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      risk_class: blueprint.domain_profile.risk_class,
      workflow_id: blueprint.workflow.workflow_id,
      workflow_digest: blueprint.workflow.workflow_digest,
      prompt_digest: blueprint.prompt.prompt_digest,
      plan_digest: blueprint.plan.plan_digest,
      learning_context_digest: blueprint.learning_context_digest,
      required_capabilities: [...blueprint.required_capabilities],
      candidates,
      selection_constraints: {
        estimated_input_tokens: effectiveEstimatedInputTokens,
        requested_output_tokens: requestedOutputTokens,
        max_cost_per_million_tokens: options.maxCostPerMillionTokens ?? null,
        max_latency_ms: options.maxLatencyMs ?? null,
        min_quality: options.minQuality ?? null,
        min_selection_confidence: options.minSelectionConfidence ?? null,
        selection_weights: selectionWeights,
        selection_observations_digest: selectionObservationsDigest,
        context_budget: normalizedContextBudget === null ? null : {
          max_input_tokens: normalizedContextBudget.maxInputTokens,
          preserve_recent_messages: normalizedContextBudget.preserveRecentMessages,
          max_messages: normalizedContextBudget.maxMessages,
        },
      },
    });
    const selected = selection.selected_model !== null;
    const preview: AutonomousModelSelectionPreview = {
      schema: AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA,
      status: selected ? "selected" : "refused_no_eligible_model",
      task_digest: blueprint.task_digest,
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      risk_class: blueprint.domain_profile.risk_class,
      workflow_id: blueprint.workflow.workflow_id,
      workflow_digest: blueprint.workflow.workflow_digest,
      domain_pack_digest: blueprint.domain_pack.pack_digest,
      task_intent_digest: blueprint.task_intent.intent_digest,
      task_decision_digest: blueprint.task_decision.decision_digest,
      task_decision_posture: blueprint.task_decision.posture,
      selection_context_digest: blueprint.learning_context_digest,
      execution_plan_digest: executionPlanDigest,
      required_model_capabilities: [...blueprint.required_capabilities],
      candidate_count: candidates.length,
      eligible_candidate_count: eligibleCandidateCount,
      selection_contract: {
        task_digest: blueprint.task_digest,
        domain: blueprint.domain_profile.domain,
        capability: blueprint.selection_context.capability,
        risk_class: blueprint.domain_profile.risk_class,
        task_intent_digest: blueprint.task_intent.intent_digest,
        task_decision_digest: blueprint.task_decision.decision_digest,
        task_decision_posture: blueprint.task_decision.posture,
        required_model_capabilities: [...blueprint.required_capabilities],
        candidate_ids: candidates.map((candidate) => `${candidate.provider}/${candidate.model}`),
        input_tokens: effectiveEstimatedInputTokens,
        requested_output_tokens: requestedOutputTokens,
        max_cost_per_million_tokens: options.maxCostPerMillionTokens ?? null,
        max_latency_ms: options.maxLatencyMs ?? null,
        min_quality: options.minQuality ?? null,
        min_selection_confidence: options.minSelectionConfidence ?? null,
        selection_weights: selectionWeights,
        selection_observations_digest: selectionObservationsDigest,
        context_budget: normalizedContextBudget === null ? null : {
          max_input_tokens: normalizedContextBudget.maxInputTokens,
          preserve_recent_messages: normalizedContextBudget.preserveRecentMessages,
          max_messages: normalizedContextBudget.maxMessages,
        },
      },
      selection_audit: structuredClone(selection),
      review: {
        provider_call: "not_started",
        domain_tools: "not_started",
        caller_approval_required: true,
        next_action: blueprint.task_decision.posture === "blocked"
          ? "resolve_task_decision_block"
          : selected ? "review_selection_and_approve_provider_call" : "resolve_model_provider_or_credential_gates",
      },
      execution: "preview_only; no_provider_or_domain_tool_invocation",
      authority_posture: "selection_review_only; preview_does_not_authorize_provider_or_effects",
      credential_posture: "caller_opaque_handles_only; no_handles_returned",
      retention: "metadata_only_model_ranking_and_digests",
      secret_material: "never_returned",
    };
    const encoded = JSON.stringify(preview);
    if (!encoded || bytes(encoded) > MAX_AUTONOMOUS_MODEL_SELECTION_PREVIEW_BYTES) {
      throw new ProviderRuntimeError("autonomous model selection preview exceeds its bounded size");
    }
    return structuredClone(preview);
  }

  /**
   * Revalidate a metadata-only selection preview, then invoke only its exact reviewed model arm.
   *
   * The caller must supply the transient task and any context again. The local selector is
   * rerun against the current provider readiness, health, candidate catalogue, and constraints.
   * Drift refuses before dispatch; the final run uses one candidate and zero failovers so an
   * approved model cannot be silently replaced by a different arm.
   */
  async runApprovedModelSelection(
    task: string,
    preview: AutonomousModelSelectionPreview,
    options: AutonomousApprovedModelSelectionOptions,
  ): Promise<AutonomousRunResult> {
    const taskText = boundedText("approved autonomous task", task, 32_000);
    if (!isObject(preview) || preview.schema !== AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA || preview.status !== "selected") {
      throw new ProviderRuntimeError("approved model selection preview is invalid or not selected");
    }
    if (preview.task_decision_posture === "blocked") {
      throw new ProviderRuntimeError("approved model selection is blocked by the task decision posture");
    }
    if (!options || !AUTONOMOUS_DOMAIN_NAMES.includes(options.domain)) throw new ArgumentError("approved model selection requires a built-in domain");
    const contract = preview.selection_contract;
    if (!isObject(contract) || !Array.isArray(contract.candidate_ids) || contract.candidate_ids.some((candidateId) => typeof candidateId !== "string" || !candidateId.trim())) {
      throw new ProviderRuntimeError("approved model selection preview contract is malformed");
    }
    const reviewedWeights = normalizeAutonomousSelectionWeights(contract.selection_weights);
    const suppliedWeights = normalizeAutonomousSelectionWeights(options.selectionWeights);
    if (canonicalJson(reviewedWeights) !== canonicalJson(suppliedWeights)) throw new ProviderRuntimeError("approved model selection weights changed; re-review required");
    const suppliedObservations = normalizeAutonomousModelObservations(options.selectionObservations);
    if (typeof contract.selection_observations_digest !== "string" || !/^[0-9a-f]{64}$/.test(contract.selection_observations_digest)) throw new ProviderRuntimeError("approved model selection observation contract is malformed");
    if (await digestJson(suppliedObservations) !== contract.selection_observations_digest) throw new ProviderRuntimeError("approved model selection observations changed; re-review required");
    const audit = preview.selection_audit;
    if (!isObject(audit) || !isObject(audit.selected_model) || typeof audit.selected_model.provider !== "string" || typeof audit.selected_model.model !== "string") {
      throw new ProviderRuntimeError("approved model selection preview has no exact selected model");
    }
    const selectedId = `${audit.selected_model.provider}/${audit.selected_model.model}`;
    const candidates = (options.candidates === undefined
      ? this.models()
      : [...options.candidates].map((candidate) => normalizeAutonomousModelCandidate(candidate)));
    if (!candidates.length) throw new ProviderRuntimeError("approved model selection requires model candidates");
    const candidateIds = candidates.map((candidate) => `${candidate.provider}/${candidate.model}`);
    if (canonicalJson(contract.candidate_ids) !== canonicalJson(candidateIds)) throw new ProviderRuntimeError("approved model selection candidate catalogue changed; re-review required");
    const selectedCandidates = candidates.filter((candidate) => `${candidate.provider}/${candidate.model}` === selectedId);
    if (selectedCandidates.length !== 1) throw new ProviderRuntimeError("approved model selection selected model is absent or duplicated");

    const reviewedContextBudgetValue = contract.context_budget ?? null;
    let reviewedContextBudget: AutonomousContextBudgetOptions | undefined;
    if (reviewedContextBudgetValue !== null) {
      if (!isObject(reviewedContextBudgetValue)) throw new ProviderRuntimeError("approved model selection context budget contract is malformed");
      reviewedContextBudget = normalizeAutonomousContextBudget({
        maxInputTokens: reviewedContextBudgetValue.max_input_tokens,
        preserveRecentMessages: reviewedContextBudgetValue.preserve_recent_messages,
        maxMessages: reviewedContextBudgetValue.max_messages,
      });
    }
    if (options.contextBudget !== undefined) {
      const suppliedContextBudget = normalizeAutonomousContextBudget(options.contextBudget);
      if (canonicalJson(suppliedContextBudget) !== canonicalJson(reviewedContextBudget)) {
        throw new ProviderRuntimeError("approved model selection context budget changed; re-review required");
      }
    }

    const inputTokens = options.maxInputTokens ?? contract.input_tokens;
    const requestedOutputTokens = options.maxOutputTokens ?? contract.requested_output_tokens;
    const capability = options.capability ?? contract.capability;
    const maxCostPerMillionTokens = options.maxCostPerMillionTokens ?? (contract.max_cost_per_million_tokens ?? undefined);
    const maxLatencyMs = options.maxLatencyMs ?? (contract.max_latency_ms ?? undefined);
    const minQuality = options.minQuality ?? (contract.min_quality ?? undefined);
    const minSelectionConfidence = options.minSelectionConfidence ?? (contract.min_selection_confidence ?? undefined);
    if (options.maxInputTokens !== undefined && options.maxInputTokens !== contract.input_tokens) throw new ProviderRuntimeError("approved model selection input budget changed; re-review required");
    if (options.maxOutputTokens !== undefined && options.maxOutputTokens !== contract.requested_output_tokens) throw new ProviderRuntimeError("approved model selection output budget changed; re-review required");
    for (const [label, supplied, reviewed] of [
      ["cost", options.maxCostPerMillionTokens, contract.max_cost_per_million_tokens],
      ["latency", options.maxLatencyMs, contract.max_latency_ms],
      ["quality", options.minQuality, contract.min_quality],
      ["confidence", options.minSelectionConfidence, contract.min_selection_confidence],
    ] as const) {
      if (supplied !== undefined && supplied !== reviewed) throw new ProviderRuntimeError(`approved model selection ${label} constraint changed; re-review required`);
    }

    const fresh = await this.modelSelectionPreview(taskText, {
      domain: options.domain,
      capability,
      context: options.context,
      candidates,
      estimatedInputTokens: inputTokens,
      requestedOutputTokens,
      contextBudget: reviewedContextBudget,
      ...(maxCostPerMillionTokens === undefined ? {} : { maxCostPerMillionTokens }),
      ...(maxLatencyMs === undefined ? {} : { maxLatencyMs }),
      ...(minQuality === undefined ? {} : { minQuality }),
      ...(minSelectionConfidence === undefined ? {} : { minSelectionConfidence }),
      selectionWeights: suppliedWeights,
      selectionObservations: suppliedObservations,
    });
    for (const field of [
      "task_digest",
      "domain",
      "capability",
      "risk_class",
      "workflow_id",
      "workflow_digest",
      "domain_pack_digest",
      "task_intent_digest",
      "task_decision_digest",
      "task_decision_posture",
      "selection_context_digest",
      "execution_plan_digest",
      "required_model_capabilities",
      "selection_contract",
      "selection_audit",
    ] as const) {
      if (canonicalJson(fresh[field]) !== canonicalJson(preview[field])) throw new ProviderRuntimeError("approved model selection is stale; re-review required");
    }
    const freshSelected = fresh.selection_audit.selected_model;
    if (!freshSelected || `${freshSelected.provider}/${freshSelected.model}` !== selectedId) throw new ProviderRuntimeError("approved model selection changed; re-review required");

    const executionOptions: AutonomousRunOptions = {
      ...options,
      domain: options.domain,
      capability,
      candidates: [selectedCandidates[0]!],
      maxInputTokens: inputTokens,
      maxOutputTokens: requestedOutputTokens,
      ...(maxCostPerMillionTokens === undefined ? {} : { maxCostPerMillionTokens }),
      ...(maxLatencyMs === undefined ? {} : { maxLatencyMs }),
      ...(minQuality === undefined ? {} : { minQuality }),
      ...(minSelectionConfidence === undefined ? {} : { minSelectionConfidence }),
      selectionWeights: suppliedWeights,
      selectionObservations: suppliedObservations,
      contextBudget: reviewedContextBudget,
      approveProviderCall: true,
      maxProviderFailovers: 0,
    };
    return this.run(taskText, executionOptions);
  }

  async route(task: string, options: { domain?: AutonomousDomainName; hints?: readonly string[]; minConfidence?: number; minMargin?: number; maxDomains?: number; allowCrossDomain?: boolean } = {}): Promise<AutonomousRouteProposal> {
    const taskText = boundedText("autonomous task", task, 32_000);
    if (options.domain !== undefined) {
      const profile = await profileFor(options.domain);
      const taskDigest = await digestJson({ task: taskText });
      const descriptor = { schema: AUTONOMOUS_ROUTE_SCHEMA, task_digest: taskDigest, candidates: [{ domain: profile.domain, score: 1, matched_terms: [profile.domain], capability: profile.default_capability, risk_class: profile.risk_class, workflow_id: profile.workflow.workflow_id, evidence: "fixed_catalogue_term_matches_only" as const }], selected_domains: [profile.domain], primary_domain: profile.domain, confidence: 1, abstained: false, reason: "routed" as const, cross_domain: false, source: "deterministic_vocabulary" as const, retention: "route_scores_and_digests_only; task_text_is_not_retained_in_route" as const, does_not_claim: ["explicit domain selection is caller input, not semantic proof", "routing does not authorize tools, provider calls, or external effects"] };
      return { ...descriptor, route_digest: await digestJson(descriptor) };
    }
    return routeAutonomousTask(taskText, options);
  }

  /**
   * Resolve the route authority for a high-level execution call. Semantic routing is opt-in,
   * inherits all caller-owned boundaries, and returns its proposal separately so callers can
   * audit the classifier without confusing it with task evidence or execution authorization.
   */
  private async resolveExecutionRoute(
    taskText: string,
    options: AutonomousRunOptions,
    costBudget: AutonomousCostBudget | undefined,
  ): Promise<{ route: AutonomousRouteProposal; semanticRoute: AutonomousSemanticRouteResult | null }> {
    if (options.routeOverride !== undefined) {
      return { route: await validateAutonomousRouteOverride(taskText, options.routeOverride), semanticRoute: null };
    }
    const semanticRouting = normalizeRunSemanticRouting(options.semanticRouting);
    if (semanticRouting === null) {
      return {
        route: await this.route(taskText, {
          domain: options.domain,
          hints: options.hints,
          minConfidence: options.minConfidence,
          minMargin: options.minMargin,
          maxDomains: options.maxDomains,
          allowCrossDomain: options.allowCrossDomain,
        }),
        semanticRoute: null,
      };
    }
    if (options.domain !== undefined) throw new ArgumentError("semanticRouting cannot be combined with an explicit domain");
    const semanticOptions: AutonomousSemanticRouteOptions = {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: semanticRouting.approveProviderCall ?? options.approveProviderCall ?? false,
      minSemanticConfidence: semanticRouting.minSemanticConfidence,
      maxDomains: semanticRouting.maxDomains ?? 3,
      allowCrossDomain: semanticRouting.allowCrossDomain ?? options.allowCrossDomain,
      maxOutputTokens: semanticRouting.maxOutputTokens ?? options.maxOutputTokens ?? 1_024,
      temperature: semanticRouting.temperature ?? options.temperature,
      maxCostPerMillionTokens: semanticRouting.maxCostPerMillionTokens ?? options.maxCostPerMillionTokens,
      maxLatencyMs: semanticRouting.maxLatencyMs ?? options.maxLatencyMs,
      minQuality: semanticRouting.minQuality ?? options.minQuality,
      costBudget,
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: semanticRouting.maxProviderFailovers ?? options.maxProviderFailovers,
      executionLifecycle: options.executionLifecycle,
      signal: options.signal,
      observer: options.observer,
      domainPolicyMode: semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
    };
    const semanticRoute = await semanticRouteAutonomousTask(this, taskText, semanticOptions);
    return { route: semanticRoute.route, semanticRoute };
  }

  /**
   * Select the next bounded context/evidence acquisitions before the reviewed evidence queue.
   * Automatic mode binds the candidate plan to the deterministic route digest; explicit domains
   * are caller-selected and get a separate explicit-domain identity. No source, provider, tool,
   * learner, or credential operation occurs here.
   */
  async planInformationAcquisition(
    task: string,
    options: {
      candidates: readonly (AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>)[];
      domains?: readonly AutonomousDomainName[];
      hints?: readonly string[];
      maxDomains?: number;
      allowCrossDomain?: boolean;
      policy?: AutonomousInformationAcquisitionPolicy | AutonomousInformationAcquisitionPolicyInput;
      satisfiedCandidateIds?: readonly string[];
    },
  ): Promise<AutonomousInformationAcquisitionPlan> {
    const taskText = boundedText("information acquisition task", task, 32_000);
    let requestedDomains: readonly AutonomousDomainName[];
    let routeDigest: string;
    if (options.domains === undefined) {
      const route = await this.route(taskText, {
        hints: options.hints,
        maxDomains: options.maxDomains,
        allowCrossDomain: options.allowCrossDomain,
      });
      if (route.abstained) throw new ArgumentError("information acquisition planning requires route review before candidate selection");
      requestedDomains = route.selected_domains.length > 0
        ? route.selected_domains
        : route.primary_domain === null ? [] : [route.primary_domain];
      routeDigest = route.route_digest;
    } else {
      requestedDomains = options.domains;
      routeDigest = await digestJson({ schema: "bioprism-typescript-explicit-information-domains/0.1", domains: [...requestedDomains] });
    }
    if (requestedDomains.length === 0) throw new ArgumentError("information acquisition planning requires at least one routed domain");
    return planAutonomousInformationAcquisition({
      taskDigest: await digestJson({ task: taskText }),
      routeDigest,
      candidates: options.candidates,
      requestedDomains,
      policy: options.policy,
      satisfiedCandidateIds: options.satisfiedCandidateIds,
    });
  }

  /** Fuse caller-supplied evidence metadata into claim decisions without dispatching anything. */
  assessClaimIntegrity(
    task: string,
    options: {
      claims: readonly (AutonomousClaimIntegrityClaim | AutonomousClaimIntegrityClaimInput | Record<string, unknown>)[];
      evidence: readonly (AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>)[];
      referenceTime: string;
      policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput;
    },
  ): AutonomousClaimIntegrityAssessment {
    const taskText = boundedText("claim integrity task", task, 32_000);
    return assessAutonomousClaimIntegrity({
      contextDigest: digestJsonSync({ task: taskText }),
      claims: options.claims,
      evidence: options.evidence,
      referenceTime: options.referenceTime,
      policy: options.policy,
    } satisfies AssessAutonomousClaimIntegrityOptions);
  }

  /** Continue a claim-integrity chain with a digest-fenced generation and caller-owned values. */
  reassessClaimIntegrity(
    previous: AutonomousClaimIntegrityAssessment,
    options: {
      claims: AssessAutonomousClaimIntegrityOptions["claims"];
      evidence: AssessAutonomousClaimIntegrityOptions["evidence"];
      referenceTime: string;
      policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput;
    },
  ): AutonomousClaimIntegrityAssessment {
    return reassessAutonomousClaimIntegrity({ previous, ...options });
  }

  /** Reassess claims with reviewed evidence links fenced to the exact acquisition and evaluator. */
  settleClaimIntegrityAcquisition(
    options: SettleAutonomousClaimIntegrityAcquisitionOptions,
  ): AutonomousClaimIntegrityAssessment {
    return settleAutonomousClaimIntegrityAcquisition(options);
  }

  /** Translate integrity blockers into the existing reviewed acquisition planner. */
  planClaimIntegrityAcquisition(
    assessment: AutonomousClaimIntegrityAssessment,
    options: PlanAutonomousClaimIntegrityAcquisitionOptions,
  ) {
    return planAutonomousClaimIntegrityAcquisition(assessment, options);
  }

  /** Bind caller-owned requests to the exact candidates selected by a claim-integrity bridge. */
  bindClaimIntegrityAcquisition(
    bridge: Parameters<typeof bindAutonomousClaimIntegrityAcquisitionRequests>[0],
    requests: readonly (AutonomousClaimIntegrityAcquisitionRequestInput | Record<string, unknown>)[],
  ): AutonomousClaimIntegrityAcquisitionBinding {
    return bindAutonomousClaimIntegrityAcquisitionRequests(bridge, requests);
  }

  /** Execute only the integrity-selected evidence queue through the reviewed source boundary. */
  async executeClaimIntegrityAcquisition(
    bridge: Parameters<typeof bindAutonomousClaimIntegrityAcquisitionRequests>[0],
    registry: AutonomousEvidenceAdapterRegistry,
    requests: readonly (AutonomousClaimIntegrityAcquisitionRequestInput | Record<string, unknown>)[],
    options: {
      prepare?: AutonomousReviewedEvidencePreparationOptions;
      execute?: AutonomousEvidenceExecutionOptions;
      availableEvidence?: readonly string[];
      completedStages?: Readonly<Record<string, readonly string[]>>;
    } = {},
  ): Promise<AutonomousEvidenceExecutionResult> {
    const binding = this.bindClaimIntegrityAcquisition(bridge, requests);
    const acquisitionPlan = bridge.acquisitionPlan;
    if (acquisitionPlan === null) throw new ArgumentError("integrity acquisition bridge has no executable plan");
    const plan = await this.evidencePlan(acquisitionPlan.selectedDomains, { availableEvidence: options.availableEvidence, completedStages: options.completedStages });
    const prepareOptions = options.prepare ?? {};
    const { healthStore, ...controllerPrepareOptions } = prepareOptions;
    const controller = await this.createEvidenceExecutionController(registry, healthStore);
    const executionPlan = await controller.prepare(plan, controllerPrepareOptions);
    const executeOptions: AutonomousEvidenceExecutionOptions = {
      ...(options.execute ?? {}),
      ...(controllerPrepareOptions.providerContracts !== undefined && options.execute?.providerContracts === undefined
        ? { providerContracts: controllerPrepareOptions.providerContracts }
        : {}),
    };
    return controller.execute(executionPlan, plan, binding.requests, executeOptions);
  }

  /** Resume the integrity-selected evidence queue through the existing CAS-fenced checkpoint. */
  async executeClaimIntegrityAcquisitionResumable(
    bridge: Parameters<typeof bindAutonomousClaimIntegrityAcquisitionRequests>[0],
    registry: AutonomousEvidenceAdapterRegistry,
    requests: readonly (AutonomousClaimIntegrityAcquisitionRequestInput | Record<string, unknown>)[],
    options: AutonomousReviewedEvidenceResumableExecutionOptions,
  ): Promise<AutonomousEvidenceExecutionResumableRun> {
    const binding = this.bindClaimIntegrityAcquisition(bridge, requests);
    const acquisitionPlan = bridge.acquisitionPlan;
    if (acquisitionPlan === null) throw new ArgumentError("integrity acquisition bridge has no executable plan");
    return this.executeReviewedEvidenceResumable(registry, acquisitionPlan.selectedDomains, binding.requests, options);
  }

  /** Project a completed direct or cross-domain result into a metadata-only reliance identity. */
  projectOutcomeIntegrityRun(result: AutonomousRunResult | AutonomousCrossDomainRunResult): AutonomousOutcomeIntegrityRun {
    return projectAutonomousOutcomeIntegrityRun(result);
  }

  /** Bind explicit claims to the exact output and structural response of one autonomous result. */
  bindOutcomeIntegrityClaims(
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
    bindings: readonly (AutonomousOutcomeIntegrityClaimBindingInput | Record<string, unknown>)[],
  ): AutonomousOutcomeIntegrityClaimBinding[] {
    return bindAutonomousOutcomeIntegrityClaims(projectAutonomousOutcomeIntegrityRun(result), bindings);
  }

  /**
   * Decide whether caller-supplied claims may rely on one exact autonomous outcome.
   *
   * This is a provider-free final gate: claim/evidence values are evaluated transiently, while
   * the returned contract retains only digests, counts, statuses, and next actions.  It never
   * upgrades a model response into external truth or authorizes any downstream effect.
   */
  assessOutcomeIntegrity(
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
    options: Omit<AssessAutonomousOutcomeIntegrityOptions, "run">,
  ): AutonomousOutcomeIntegrityAssessment {
    return assessAutonomousOutcomeIntegrity({
      run: projectAutonomousOutcomeIntegrityRun(result),
      ...options,
    });
  }

  /** Gate structured specialist outputs before cross-domain synthesis. */
  assessCrossDomainResponses(
    responses: readonly AutonomousCrossDomainResponseEntry[],
    options: {
      task?: string;
      contextDigest?: string | null;
      requestedDomains?: readonly AutonomousDomainName[];
      alignments?: readonly AutonomousCrossDomainResponseAlignmentInput[];
      requireSynthesis?: boolean;
      requireCompleteAlignment?: boolean;
      minimumReward?: number;
      minimumAlignmentConfidence?: number;
      contradictionConfidenceThreshold?: number;
    } = {},
  ): AutonomousCrossDomainResponseAssessment {
    if (options.task !== undefined && options.contextDigest !== undefined && options.contextDigest !== null) throw new ArgumentError("cross-domain response assessment accepts task or contextDigest, not both");
    if (options.task !== undefined && (typeof options.task !== "string" || options.task.trim().length === 0 || options.task.includes("\u0000") || new TextEncoder().encode(options.task).byteLength > 32_000)) throw new ArgumentError("cross-domain response assessment task is outside its bound");
    const contextDigest = options.task === undefined ? options.contextDigest : digestJsonSync({ task: options.task });
    return assessAutonomousCrossDomainResponseSet(responses, {
      requestedDomains: options.requestedDomains,
      contextDigest,
      alignments: options.alignments,
      requireSynthesis: options.requireSynthesis,
      requireCompleteAlignment: options.requireCompleteAlignment,
      minimumReward: options.minimumReward,
      minimumAlignmentConfidence: options.minimumAlignmentConfidence,
      contradictionConfidenceThreshold: options.contradictionConfidenceThreshold,
    });
  }

  /** Resolve the bounded policy for a domain without provider, tool, or source activity. */
  domainPolicy(domain: AutonomousDomainName, overrides: AutonomousDomainPolicyOverrides = {}): AutonomousDomainPolicy {
    return autonomousDomainPolicy(domain, overrides);
  }

  /**
   * Explain whether a planned invocation has cleared the provider-free domain gates.
   * `admitted` is still descriptive; provider/effect authorization remains caller-owned.
   */
  admitDomainPolicy(domain: AutonomousDomainName, input: Parameters<typeof evaluateAutonomousDomainPolicy>[1] = {}): AutonomousDomainPolicyAdmission {
    return evaluateAutonomousDomainPolicy(this.domainPolicy(domain), input);
  }

  /**
   * Compile multiple explicit domain workflows into one dependency-aware, metadata-only
   * portfolio. This is planning authority only: it does not invoke a provider, tool, connector,
   * credential, or external effect.
   */
  async planWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioPlanOptions = {},
  ): Promise<AutonomousWorkflowPortfolioPlan> {
    const { planAutonomousWorkflowPortfolio } = await import("./autonomous-workflow-portfolio.js");
    return planAutonomousWorkflowPortfolio(this, requests, options);
  }

  /**
   * Project portfolio-wide model/provider/evidence/calibration admission before dispatch.
   * This is a metadata-only gate; caller approval remains required for every provider call.
   */
  async admitWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioAdmissionOptions = {},
  ): Promise<AutonomousWorkflowPortfolioAdmission> {
    const { admitAutonomousWorkflowPortfolio } = await import("./autonomous-workflow-portfolio-admission.js");
    return admitAutonomousWorkflowPortfolio(this, requests, options);
  }

  /** Recompile a caller-rehydrated portfolio and compare every digest-bound workflow identity. */
  async verifyWorkflowPortfolio(
    plan: AutonomousWorkflowPortfolioPlan,
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioPlanOptions = {},
  ): Promise<AutonomousWorkflowPortfolioVerification> {
    const { verifyAutonomousWorkflowPortfolio } = await import("./autonomous-workflow-portfolio.js");
    return verifyAutonomousWorkflowPortfolio(this, plan, requests, options);
  }

  /** Execute a reviewed portfolio in dependency waves; provider/tool effects remain caller-approved per item. */
  async executeWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioExecutionOptions = {},
  ): Promise<AutonomousWorkflowPortfolioExecutionResult> {
    const { executeAutonomousWorkflowPortfolio } = await import("./autonomous-workflow-portfolio-execution.js");
    return executeAutonomousWorkflowPortfolio(this, requests, options);
  }

  /** Acquire and evaluate caller-owned evidence for a completed portfolio without provider replay. */
  async executeWorkflowPortfolioEvidence(
    execution: AutonomousWorkflowPortfolioExecutionResult,
    options: AutonomousWorkflowPortfolioEvidenceSupervisorOptions,
  ): Promise<AutonomousWorkflowPortfolioEvidenceExecutionResult> {
    const { executeAutonomousWorkflowPortfolioEvidence } = await import("./autonomous-workflow-portfolio-evidence.js");
    return executeAutonomousWorkflowPortfolioEvidence(this, execution, options);
  }

  /** Resume portfolio evidence through digest-bound checkpoints and caller-owned journals. */
  async executeWorkflowPortfolioEvidenceResumable(
    execution: AutonomousWorkflowPortfolioExecutionResult,
    options: AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions,
  ): Promise<AutonomousWorkflowPortfolioEvidenceExecutionResult> {
    const { executeAutonomousWorkflowPortfolioEvidenceResumable } = await import("./autonomous-workflow-portfolio-evidence-resumable.js");
    return executeAutonomousWorkflowPortfolioEvidenceResumable(this, execution, options);
  }

  /** Resume a metadata-only portfolio checkpoint after caller-owned item rehydration. */
  async executeWorkflowPortfolioResumable(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioResumableExecutionOptions,
  ): Promise<AutonomousWorkflowPortfolioExecutionResult> {
    const { executeAutonomousWorkflowPortfolioResumable } = await import("./autonomous-workflow-portfolio-resumable.js");
    return executeAutonomousWorkflowPortfolioResumable(this, requests, options);
  }

  /** Compile evidence requirements and dependency-safe next stages without dispatching work. */
  async evidencePlan(
    domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES,
    options: { availableEvidence?: readonly string[]; completedStages?: Readonly<Record<string, readonly string[]>> } = {},
  ): Promise<AutonomousEvidencePlan> {
    if (!Array.isArray(domains) || domains.length < 1 || domains.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("evidencePlan domains are outside their bounds");
    if (new Set(domains).size !== domains.length || domains.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain))) throw new ArgumentError("evidencePlan domains must be unique built-in domains");
    const profiles = await Promise.all(domains.map((domain) => profileFor(domain)));
    return buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow), options);
  }

  /** Create the caller-owned acquisition/evaluation runtime for an evidence plan. */
  async evidenceRuntime(
    domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES,
    options: { availableEvidence?: readonly string[]; completedStages?: Readonly<Record<string, readonly string[]>>; journal?: AutonomousEvidenceRuntimeJournal } = {},
  ): Promise<AutonomousEvidenceRuntime> {
    const plan = await this.evidencePlan(domains, options);
    return new AutonomousEvidenceRuntime({ plan, journal: options.journal });
  }

  /** Acquire and optionally evaluate evidence through the explicit application adapter boundary. */
  async acquireEvidence(
    domains: readonly AutonomousDomainName[],
    requests: readonly AutonomousEvidenceAcquisitionRequest[],
    options: AutonomousEvidenceRuntimeExecuteOptions & { availableEvidence?: readonly string[]; completedStages?: Readonly<Record<string, readonly string[]>>; journal?: AutonomousEvidenceRuntimeJournal },
  ): Promise<AutonomousEvidenceRuntimeResult> {
    const { availableEvidence, completedStages, journal, ...executeOptions } = options;
    const runtime = await this.evidenceRuntime(domains, { availableEvidence, completedStages, journal });
    return runtime.execute(requests, executeOptions);
  }

  /** Create the reviewed evidence controller without coupling the brain facade to source transport. */
  async createEvidenceExecutionController(
    registry: AutonomousEvidenceAdapterRegistry,
    healthStore?: AutonomousEvidenceAdapterHealthStore,
  ): Promise<AutonomousEvidenceExecutionController> {
    const { AutonomousEvidenceExecutionController } = await import("./autonomous-evidence-execution.js");
    return new AutonomousEvidenceExecutionController(registry, healthStore);
  }

  /** Compile, select, readiness-audit, and bind a reviewed evidence execution plan from the brain facade. */
  async prepareReviewedEvidence(
    registry: AutonomousEvidenceAdapterRegistry,
    domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES,
    options: AutonomousReviewedEvidencePreparationOptions = {},
  ): Promise<AutonomousEvidenceExecutionPlan> {
    const plan = await this.evidencePlan(domains);
    const { healthStore, ...controllerOptions } = options;
    const controller = await this.createEvidenceExecutionController(registry, healthStore);
    return controller.prepare(plan, controllerOptions);
  }

  /**
   * Run the complete reviewed evidence lifecycle from the high-level brain facade. Preparation
   * remains separate in the returned plan, source dispatch still requires explicit approval, and
   * provider contract bindings are carried forward automatically when supplied at preparation.
   */
  async executeReviewedEvidence(
    registry: AutonomousEvidenceAdapterRegistry,
    domains: readonly AutonomousDomainName[],
    requests: readonly AutonomousEvidenceAcquisitionRequest[],
    options: AutonomousReviewedEvidenceExecutionOptions = {},
  ): Promise<AutonomousEvidenceExecutionResult> {
    const plan = await this.evidencePlan(domains, { availableEvidence: options.availableEvidence, completedStages: options.completedStages });
    const prepareOptions = options.prepare ?? {};
    const { healthStore, ...controllerPrepareOptions } = prepareOptions;
    const controller = await this.createEvidenceExecutionController(registry, healthStore);
    const executionPlan = await controller.prepare(plan, controllerPrepareOptions);
    const executeOptions: AutonomousEvidenceExecutionOptions = {
      ...(options.execute ?? {}),
      ...(controllerPrepareOptions.providerContracts !== undefined && options.execute?.providerContracts === undefined
        ? { providerContracts: controllerPrepareOptions.providerContracts }
        : {}),
    };
    return controller.execute(executionPlan, plan, requests, executeOptions);
  }

  /**
   * Execute reviewed evidence through a restart-safe job checkpoint. The caller still owns the
   * runtime journal and transient values; only the approval/readiness/settlement metadata is
   * persisted by the supplied checkpoint store.
   */
  async executeReviewedEvidenceResumable(
    registry: AutonomousEvidenceAdapterRegistry,
    domains: readonly AutonomousDomainName[],
    requests: readonly AutonomousEvidenceAcquisitionRequest[],
    options: AutonomousReviewedEvidenceResumableExecutionOptions,
  ): Promise<AutonomousEvidenceExecutionResumableRun> {
    if (!options || typeof options !== "object") throw new ArgumentError("resumable reviewed evidence options are malformed");
    if (typeof options.jobId !== "string" || !options.jobId.trim()) throw new ArgumentError("resumable reviewed evidence jobId is required");
    if (!options.checkpointStore || typeof options.checkpointStore.read !== "function" || typeof options.checkpointStore.write !== "function") throw new ArgumentError("resumable reviewed evidence checkpointStore is malformed");
    const plan = await this.evidencePlan(domains, { availableEvidence: options.availableEvidence, completedStages: options.completedStages });
    const prepareOptions = options.prepare ?? {};
    const { healthStore, ...controllerPrepareOptions } = prepareOptions;
    const controller = await this.createEvidenceExecutionController(registry, healthStore);
    const executionPlan = await controller.prepare(plan, controllerPrepareOptions);
    const { AutonomousEvidenceExecutionResumableController } = await import("./autonomous-evidence-execution-resumable.js");
    const resumable = new AutonomousEvidenceExecutionResumableController(controller, options.checkpointStore, options.jobId, {
      reconciliationAuthority: options.reconciliationAuthority,
    });
    const executeOptions: AutonomousEvidenceExecutionResumableOptions = {
      ...(options.execute ?? {}),
      ...(controllerPrepareOptions.providerContracts !== undefined && options.execute?.providerContracts === undefined
        ? { providerContracts: controllerPrepareOptions.providerContracts }
        : {}),
    };
    return resumable.run(executionPlan, plan, requests, executeOptions);
  }

  /**
   * Compose reviewed evidence acquisition with the ordinary autonomous provider run.
   * Source approval, evidence acceptance, and provider approval remain independently visible;
   * this method never turns a successful source call into provider or task-level success.
   */
  async runWithReviewedEvidence(
    task: string,
    options: AutonomousEvidenceBackedRunOptions,
  ): Promise<AutonomousEvidenceBackedRunResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("evidence-backed run options are malformed");
    if (!(options.registry instanceof (await import("./autonomous-evidence-adapters.js")).AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence-backed run requires a typed adapter registry");
    if (!Array.isArray(options.requests) || options.requests.length < 1) throw new ArgumentError("evidence-backed run requires acquisition requests");
    if (options.evidenceCheckpointStore !== undefined && (!options.evidenceCheckpointStore || typeof options.evidenceCheckpointStore.read !== "function" || typeof options.evidenceCheckpointStore.write !== "function")) throw new ArgumentError("evidence-backed source checkpoint store is malformed");
    if (options.evidenceCheckpointStore !== undefined && (typeof options.evidenceJobId !== "string" || !options.evidenceJobId.trim())) throw new ArgumentError("evidence-backed source checkpoint requires evidenceJobId");
    if (options.evidenceCheckpointStore === undefined && options.evidenceJobId !== undefined) throw new ArgumentError("evidence-backed evidenceJobId requires evidenceCheckpointStore");
    if (options.evidenceCheckpointStore === undefined && options.evidenceReconciliationAuthority !== undefined) throw new ArgumentError("evidence-backed evidenceReconciliationAuthority requires evidenceCheckpointStore");
    if (options.evidenceCheckpointStore === undefined && options.evidenceExecutionPolicyIdentity !== undefined) throw new ArgumentError("evidence-backed evidenceExecutionPolicyIdentity requires evidenceCheckpointStore");
    if (options.evidenceCheckpointStore === undefined && options.evidenceReconciliationReceipt !== undefined) throw new ArgumentError("evidence-backed evidenceReconciliationReceipt requires evidenceCheckpointStore");
    const taskText = boundedText("evidence-backed autonomous task", task, 32_000);
    const taskDigest = await digestJson({ task: taskText });
    const domains = options.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    const runMode = normalizeAutonomousEvidenceExecutionMode(options.runMode);
    const overrideCount = [options.providerRunOverride, options.automaticRunOverride, options.crossDomainRunOverride].filter((value) => value !== undefined).length;
    if (overrideCount > 1) throw new ArgumentError("evidence-backed execution accepts only one result override");
    if (options.providerRunOverride !== undefined && runMode !== "domain") throw new ArgumentError("evidence-backed providerRunOverride is supported only for domain mode");
    if (options.automaticRunOverride !== undefined && runMode !== "auto") throw new ArgumentError("evidence-backed automaticRunOverride is supported only for auto mode");
    if (options.crossDomainRunOverride !== undefined && runMode !== "cross_domain") throw new ArgumentError("evidence-backed crossDomainRunOverride is supported only for cross_domain mode");
    if (runMode === "cross_domain" && options.domains === undefined) throw new ArgumentError("cross-domain evidence execution requires an explicit 2..8 domain scope");
    if (runMode === "cross_domain" && (domains.length < 2 || domains.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN || domains.includes("cross_domain"))) throw new ArgumentError("cross-domain evidence execution requires 2..8 non-synthesis domains");
    const evidenceScopeRoute = options.domains === undefined ? null : await routeAutonomousEvidenceScope(taskText, domains);
    const plan = await this.evidencePlan(domains, {
      availableEvidence: options.availableEvidence,
      completedStages: options.completedStages,
    });
    const prepareOptions = options.prepare ?? {};
    const { healthStore, ...controllerPrepareOptions } = prepareOptions;
    const controller = await this.createEvidenceExecutionController(options.registry, healthStore);
    const executionPlan = await controller.prepare(plan, controllerPrepareOptions);
    const executeOptions: AutonomousEvidenceExecutionOptions = {
      ...(options.execute ?? {}),
      ...(controllerPrepareOptions.providerContracts !== undefined && options.execute?.providerContracts === undefined
        ? { providerContracts: controllerPrepareOptions.providerContracts }
        : {}),
    };

    const finish = async (
      status: AutonomousEvidenceBackedRunStatus,
      evidence: AutonomousEvidenceExecutionResult | null,
      promptContext: readonly AutonomousPromptChunk[],
      run: AutonomousRunResult | null,
      crossDomainRun: AutonomousCrossDomainRunResult | null = null,
      automatic: AutonomousAutoRunResult | null = null,
    ): Promise<AutonomousEvidenceBackedRunResult> => {
      const evidenceResultDigest = evidence?.result_digest ?? null;
      const metadataRun = run ?? crossDomainRun?.synthesis ?? (automatic?.result?.schema === "bioprism-typescript-autonomous-run/0.1" ? automatic.result : automatic?.result?.synthesis ?? null);
      const selectionDigest = metadataRun?.selection ? await digestJson(metadataRun.selection) : null;
      const responseDigest = metadataRun?.response ? await digestJson(metadataRun.response) : null;
      const descriptor = {
        schema: AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
        status,
        run_mode: runMode,
        task_digest: taskDigest,
        evidence_plan_digest: plan.plan_digest,
        execution_plan_digest: executionPlan.plan_digest,
        evidence_result_digest: evidenceResultDigest,
        prompt_projection_digest: promptContext.length ? await digestJson(promptContext) : null,
        run_status: run?.status ?? null,
        cross_domain_run_status: crossDomainRun?.status ?? null,
        automatic_status: automatic?.status ?? null,
        automatic_route_digest: automatic?.route.route_digest ?? null,
        automatic_next_action: automatic?.next_action ?? null,
        selection_digest: selectionDigest,
        response_digest: responseDigest,
        retention: "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned" as const,
        secret_material: "never_returned" as const,
      };
      const projection = { ...descriptor, result_digest: await digestJson(descriptor) } satisfies AutonomousEvidenceBackedRunProjection;
      if (bytes(JSON.stringify(projection)) > MAX_AUTONOMOUS_EVIDENCE_BACKED_RESULT_BYTES) throw new ProviderRuntimeError("evidence-backed run projection exceeds its bound");
      return {
        schema: AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
        status,
        run_mode: runMode,
        task_digest: taskDigest,
        execution_plan: executionPlan,
        evidence,
        prompt_context: structuredClone(promptContext),
        run,
        cross_domain_run: crossDomainRun,
        automatic,
        toJSON: () => structuredClone(projection),
      };
    };

    let evidence: AutonomousEvidenceExecutionResult | null = null;
    if (options.evidenceCheckpointStore !== undefined) {
      const { AutonomousEvidenceExecutionResumableController } = await import("./autonomous-evidence-execution-resumable.js");
      const resumable = new AutonomousEvidenceExecutionResumableController(controller, options.evidenceCheckpointStore, options.evidenceJobId!, {
        reconciliationAuthority: options.evidenceReconciliationAuthority,
      });
      const sourceRun = await resumable.run(executionPlan, plan, options.requests, {
        ...executeOptions,
        ...(options.evidenceExecutionPolicyIdentity === undefined ? {} : { executionPolicyIdentity: options.evidenceExecutionPolicyIdentity }),
        ...(options.evidenceReconciliationReceipt === undefined ? {} : { reconciliationReceipt: options.evidenceReconciliationReceipt }),
        resumeAfterReconciliation: options.evidenceResumeAfterReconciliation,
      });
      if (sourceRun.result === null) {
        const status: AutonomousEvidenceBackedRunStatus = sourceRun.status === "approval_required" ? "evidence_review_required" : sourceRun.status === "blocked" ? "evidence_blocked" : "evidence_failed";
        return finish(status, null, [], null);
      }
      evidence = sourceRun.result;
    } else {
      if (executeOptions.approveSourceDispatch !== true) return finish("evidence_review_required", null, [], null);
      if (executionPlan.status !== "ready_for_review") return finish("evidence_blocked", null, [], null);
      evidence = await controller.execute(executionPlan, plan, options.requests, executeOptions);
    }
    const evidenceIncomplete = evidence.status !== "completed";
    if (evidenceIncomplete && options.allowIncompleteEvidence !== true) {
      return finish(evidenceBackedStatus(evidence.status), evidence, [], null);
    }
    const promptProjection: AutonomousEvidencePromptProjection = {
      executionPlan,
      evidence,
      values: evidence.runtime.values,
    };
    const projectedContext = normalizeEvidenceBackedPromptContext(
      options.promptBuilder ? await options.promptBuilder(promptProjection) : defaultEvidenceBackedPromptContext(evidence),
    );
    const runOptions = options.run ?? {};
    const context = normalizeEvidenceBackedPromptContext([...(runOptions.context ?? []), ...projectedContext], 128);
    let run: AutonomousRunResult | null = null;
    let crossDomainRun: AutonomousCrossDomainRunResult | null = null;
    let automatic: AutonomousAutoRunResult | null = null;
    if (options.providerRunOverride !== undefined) {
      if (!isObject(options.providerRunOverride) || options.providerRunOverride.schema !== "bioprism-typescript-autonomous-run/0.1") throw new ArgumentError("evidence-backed provider run override is malformed");
      if (runOptions.approveProviderCall !== true) throw new ArgumentError("evidence-backed provider run override requires provider approval in the reviewed run options");
      await validateAutonomousRouteOverride(taskText, options.providerRunOverride.route);
      run = options.providerRunOverride;
    } else if (options.automaticRunOverride !== undefined) {
      if (!isObject(options.automaticRunOverride) || options.automaticRunOverride.schema !== AUTONOMOUS_AUTO_RUN_SCHEMA) throw new ArgumentError("evidence-backed automatic run override is malformed");
      if (runOptions.approveProviderCall !== true) throw new ArgumentError("evidence-backed automatic run override requires provider approval in the reviewed run options");
      if (!options.automaticRunOverride.result) throw new ArgumentError("evidence-backed automatic run override requires a completed result envelope");
      const overrideRoute = await validateAutonomousRouteOverride(taskText, options.automaticRunOverride.route);
      if (evidenceScopeRoute && overrideRoute.route_digest !== evidenceScopeRoute.route_digest) throw new ArgumentError("evidence-backed automatic run override does not match the reviewed evidence route");
      const nestedRoute = await validateAutonomousRouteOverride(taskText, options.automaticRunOverride.result.route);
      if (nestedRoute.route_digest !== overrideRoute.route_digest) throw new ArgumentError("evidence-backed automatic run override result route does not match its envelope");
      automatic = options.automaticRunOverride;
      const automaticResult = automatic.result;
      if (automaticResult && automaticResult.schema === "bioprism-typescript-autonomous-run/0.1") run = automaticResult;
      if (automaticResult && automaticResult.schema === AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA) crossDomainRun = automaticResult;
    } else if (options.crossDomainRunOverride !== undefined) {
      if (!isObject(options.crossDomainRunOverride) || options.crossDomainRunOverride.schema !== AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA) throw new ArgumentError("evidence-backed cross-domain run override is malformed");
      if (runOptions.approveProviderCall !== true) throw new ArgumentError("evidence-backed cross-domain run override requires provider approval in the reviewed run options");
      const overrideRoute = await validateAutonomousRouteOverride(taskText, options.crossDomainRunOverride.route);
      if (!overrideRoute.cross_domain) throw new ArgumentError("evidence-backed cross-domain run override must carry a cross-domain route");
      if (evidenceScopeRoute && overrideRoute.route_digest !== evidenceScopeRoute.route_digest) throw new ArgumentError("evidence-backed cross-domain run override does not match the reviewed evidence route");
      crossDomainRun = options.crossDomainRunOverride;
    } else {
      // Prepare provider-bound state before routing/planning, but commit attempted state only at
      // the transport-adjacent observer below. Policy and routing refusals remain pre-attempt.
      const providerPreflight = { executionPlan, evidence, promptContext: projectedContext };
      if (runOptions.approveProviderCall === true) {
        await options.beforeProviderRun?.(providerPreflight);
      }
      const providerDispatchFence: ProviderTransportDispatchFence | undefined = options.beforeProviderDispatch === undefined
        ? undefined
        : (dispatch) => options.beforeProviderDispatch!(providerPreflight, dispatch);
      const providerRunOptions: AutonomousAutoRunOptions = {
        ...runOptions,
        providerDispatchFence,
        ...(runOptions.planning === undefined && runOptions.planningMode !== "provider"
          ? {}
          : {
            planning: {
              ...(runOptions.planning ?? {}),
              providerDispatchFence,
            },
          }),
      };
      if (runMode === "domain") {
        const evidenceDomain = options.domains?.length === 1 ? options.domains[0] : undefined;
        if (evidenceDomain !== undefined && providerRunOptions.domain !== undefined && providerRunOptions.domain !== evidenceDomain) throw new ArgumentError("domain evidence scope does not match the requested provider domain");
        run = await this.run(taskText, {
          ...providerRunOptions,
          context,
          ...(evidenceDomain !== undefined && providerRunOptions.domain === undefined ? { domain: evidenceDomain } : {}),
        });
      } else if (runMode === "cross_domain") {
        if (!evidenceScopeRoute) throw new ProviderRuntimeError("cross-domain evidence route was not compiled");
        if (providerRunOptions.semanticRouting !== undefined && providerRunOptions.semanticRouting !== false) throw new ArgumentError("cross-domain evidence execution requires provider-free route binding");
        const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...crossRunOptions } = providerRunOptions;
        crossDomainRun = await this.runCrossDomain(taskText, {
          ...crossRunOptions,
          ...options.crossDomain,
          context,
          routeOverride: evidenceScopeRoute,
          domainPolicyEvidenceReady: true,
          semanticRouting: false,
        });
      } else {
        if (evidenceScopeRoute) {
          if (providerRunOptions.routeOverride !== undefined) throw new ArgumentError("automatic evidence execution owns the evidence-scope route override");
          if (providerRunOptions.semanticRouting !== undefined && providerRunOptions.semanticRouting !== false) throw new ArgumentError("automatic evidence execution cannot combine an exact evidence scope with provider-assisted semantic routing");
          const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...automaticRunOptions } = providerRunOptions;
          automatic = await this.runAuto(taskText, {
            ...automaticRunOptions,
            context,
            routeOverride: evidenceScopeRoute,
            semanticRouting: false,
            domainPolicyEvidenceReady: true,
          });
        } else {
          automatic = await this.runAuto(taskText, { ...providerRunOptions, context, domainPolicyEvidenceReady: true });
        }
        if (automatic.result?.schema === "bioprism-typescript-autonomous-run/0.1") run = automatic.result;
        if (automatic.result?.schema === "bioprism-typescript-autonomous-cross-domain-result/0.1") crossDomainRun = automatic.result;
      }
    }
    const status: AutonomousEvidenceBackedRunStatus = evidenceIncomplete
      ? "evidence_incomplete"
      : automatic?.status ?? crossDomainRun?.status ?? run?.status ?? "evidence_failed";
    return finish(status, evidence, projectedContext, run, crossDomainRun, automatic);
  }

  /**
   * Run the catalogue-backed evidence brain lifecycle. The source catalogue owns route and
   * normalizer identity; this facade owns prompt assembly, model selection, provider invocation,
   * memory, and optional online-learning feedback. Source approval and provider approval remain
   * independent, and the default prompt contains metadata-only evidence projections.
   */
  async runWithDomainEvidenceCatalogue(
    task: string,
    options: AutonomousDomainEvidenceBrainRunOptions,
  ): Promise<AutonomousDomainEvidenceBrainRunResult> {
    const { runAutonomousDomainEvidenceBacked } = await import("./autonomous-domain-evidence-brain.js");
    return runAutonomousDomainEvidenceBacked(this, task, options);
  }

  /**
   * Admit the complete declared evidence scope before source adapters, credentials, or providers
   * can be reached. Evidence acquisition is a separate authority from provider execution, so
   * the launch record covers every requested domain while the nested evidence options retain
   * their own source and provider approvals.
   */
  async runWithReviewedEvidenceWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousEvidenceBackedRunOptions,
  ): Promise<AutonomousEvidenceBackedRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithReviewedEvidence(task, options);
  }

  /** Catalogue equivalent of the evidence-first launch-admission handoff. */
  async runWithDomainEvidenceCatalogueWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousDomainEvidenceBrainRunOptions,
  ): Promise<AutonomousDomainEvidenceBrainRunResult> {
    const domains = options?.domains ?? AUTONOMOUS_DOMAIN_NAMES;
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    authorizeAutonomousLaunchDomains(admission, domains);
    return this.runWithDomainEvidenceCatalogue(task, options);
  }

  async blueprint(task: string, options: { domain?: AutonomousDomainName; routeOverride?: AutonomousRouteProposal; capability?: string; riskClass?: string; constraints?: readonly string[]; desiredOutputs?: readonly string[]; context?: readonly AutonomousPromptChunk[]; hints?: readonly string[]; minConfidence?: number; minMargin?: number; maxDomains?: number; allowCrossDomain?: boolean; maxInputTokens?: number; tools?: readonly string[]; subtasks?: readonly AutonomousCrossDomainSubtask[]; structuredDomainResponse?: boolean; toolSelectionState?: AutonomousToolSelectionState | null; toolSelectionExploration?: number; maxToolRiskClass?: AutonomousToolRiskClass } = {}): Promise<AutonomousAutoBlueprint> {
    const taskText = boundedText("autonomous task", task, 32_000);
    const route = options.routeOverride ? await validateAutonomousRouteOverride(taskText, options.routeOverride) : await this.route(taskText, { domain: options.domain, hints: options.hints, minConfidence: options.minConfidence, minMargin: options.minMargin, maxDomains: options.maxDomains, allowCrossDomain: options.allowCrossDomain });
    if (route.abstained || !route.primary_domain) return { schema: "bioprism-python-autonomous-auto-blueprint/0.1", route, blueprint: null, cross_domain_blueprint: null, execution: "not_started", authorization: "route_and_plan_only; no_provider_or_tool_effects_authorized" };
    if (route.cross_domain) {
      const crossDomain = await this.buildCrossDomainBlueprint(taskText, route, options);
      return { schema: "bioprism-python-autonomous-auto-blueprint/0.1", route, blueprint: crossDomain.child_blueprints[0] ?? null, cross_domain_blueprint: crossDomain, execution: "not_started", authorization: "route_and_plan_only; no_provider_or_tool_effects_authorized" };
    }
    const profile = await profileFor(route.primary_domain);
    const capabilityRoute = routeAutonomousCapability(taskText, profile.domain, options.capability === undefined ? {} : { explicitCapability: options.capability });
    const effectiveCapability = options.capability ?? capabilityRoute.selected_capability ?? undefined;
    const activeToolNames = options.tools ? this.filterActivatedToolNames([...options.tools]) : await this.liveToolNamesForTask(taskText, [route.primary_domain], effectiveCapability, this.effectiveToolSelectionState(options.toolSelectionState), options.toolSelectionExploration, options.maxToolRiskClass);
    const blueprint = await buildTaskBlueprint(profile, taskText, { taskDigest: route.task_digest, routeDigest: route.route_digest, capability: effectiveCapability, riskClass: options.riskClass, constraints: options.constraints, desiredOutputs: options.desiredOutputs, capabilityRoute, context: options.context, maxInputTokens: options.maxInputTokens, activeToolNames, selectedToolNames: activeToolNames, structuredDomainResponse: options.structuredDomainResponse });
    return { schema: "bioprism-python-autonomous-auto-blueprint/0.1", route, blueprint, cross_domain_blueprint: null, capability_route: capabilityRoute, execution: "not_started", authorization: "route_and_plan_only; no_provider_or_tool_effects_authorized" };
  }

  /**
   * Compile deterministic clarification questions from the same blueprint that would shape
   * execution. The returned plan is a user-interaction receipt only; it cannot authorize a
   * provider, source, tool, credential, evaluator, or external effect.
   */
  async clarificationPlan(
    task: string,
    options: {
      domain: AutonomousDomainName;
      capability?: string;
      riskClass?: string;
      constraints?: readonly string[];
      desiredOutputs?: readonly string[];
      context?: readonly AutonomousPromptChunk[];
      maxInputTokens?: number;
      structuredDomainResponse?: boolean;
      maxQuestions?: number;
    },
  ): Promise<AutonomousTaskClarificationPlan> {
    if (!options || !options.domain) throw new ArgumentError("clarificationPlan requires an explicit domain");
    const taskText = boundedText("autonomous clarification task", task, 32_000);
    const envelope = await this.blueprint(taskText, {
      domain: options.domain,
      capability: options.capability,
      riskClass: options.riskClass,
      constraints: options.constraints,
      desiredOutputs: options.desiredOutputs,
      context: options.context,
      maxInputTokens: options.maxInputTokens,
      structuredDomainResponse: options.structuredDomainResponse,
      allowCrossDomain: false,
    });
    if (!envelope.blueprint || envelope.cross_domain_blueprint) throw new ArgumentError("clarificationPlan requires a single-domain blueprint");
    return planAutonomousTaskClarification({
      intent: envelope.blueprint.task_intent,
      lens: envelope.blueprint.task_lens,
      policy: envelope.blueprint.domain_policy,
      decision: envelope.blueprint.task_decision,
      maxQuestions: options.maxQuestions,
    });
  }

  /** Resolve transient answers into a plan-bound, answer-digest-only receipt. */
  async resolveClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    task: string,
    answers: Readonly<Record<string, string>>,
  ): Promise<AutonomousTaskClarificationResolution> {
    const taskText = boundedText("autonomous clarification task", task, 32_000);
    return resolveAutonomousTaskClarification(plan, { taskDigest: digestJsonSync({ task: taskText }), answers });
  }

  /** Rehydrate a persisted clarification receipt against its exact plan. */
  async validateClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    receipt: AutonomousTaskClarificationResolution | unknown,
  ): Promise<AutonomousTaskClarificationResolution> {
    return validateAutonomousTaskClarificationResolution(receipt, plan);
  }

  /** Validate a persisted domain lens before task classification or planning resumes. */
  validateTaskLens(value: AutonomousDomainTaskLens | unknown, expectedDomain?: AutonomousDomainName): AutonomousDomainTaskLens {
    return validateAutonomousDomainTaskLens(value, expectedDomain);
  }

  /** Validate persisted intent metadata and optionally bind it to the current task/lens. */
  validateTaskIntent(
    value: AutonomousTaskIntent | unknown,
    options: { lens?: AutonomousDomainTaskLens; taskDigest?: string } = {},
  ): AutonomousTaskIntent {
    return validateAutonomousTaskIntent(value, options);
  }

  /** Validate a persisted domain policy before a resumed decision replay. */
  validateDomainPolicy(value: AutonomousDomainPolicy | unknown, expectedDomain?: AutonomousDomainName): AutonomousDomainPolicy {
    return validateAutonomousDomainPolicy(value, expectedDomain);
  }

  /** Validate a persisted task decision before a resumed execution boundary. */
  validateTaskDecision(
    value: AutonomousTaskDecision | unknown,
    options: {
      intent?: AutonomousTaskIntent;
      lens?: AutonomousDomainTaskLens;
      policy?: AutonomousDomainPolicy;
      requiredModelCapabilities?: readonly string[];
    } = {},
  ): AutonomousTaskDecision {
    return validateAutonomousTaskDecision(value, options);
  }

  /**
   * Recompile a caller-clarified task only after the original task and complete receipt verify.
   * The returned live blueprint remains transient; JSON serialization uses a metadata-only
   * projection so answer values and task text cannot accidentally enter a durable handoff.
   */
  async recompileClarification(
    plan: AutonomousTaskClarificationPlan | unknown,
    receipt: AutonomousTaskClarificationResolution | unknown,
    task: string,
    clarifiedTask: string,
    options: {
      capability?: string;
      riskClass?: string;
      constraints?: readonly string[];
      desiredOutputs?: readonly string[];
      context?: readonly AutonomousPromptChunk[];
      maxInputTokens?: number;
      structuredDomainResponse?: boolean;
    } = {},
  ): Promise<AutonomousClarificationRecompileResult> {
    const resolvedPlan = validateAutonomousTaskClarificationPlan(plan);
    const taskText = boundedText("autonomous clarification task", task, 32_000);
    const originalTaskDigest = digestJsonSync({ task: taskText });
    if (resolvedPlan.task_digest !== originalTaskDigest) throw new ArgumentError("clarification recompile task does not match the original plan");
    const resolvedReceipt = validateAutonomousTaskClarificationResolution(receipt, resolvedPlan);
    if (resolvedReceipt.task_digest !== originalTaskDigest) throw new ArgumentError("clarification recompile receipt does not match the original task");
    if (resolvedReceipt.status !== "resolved") throw new ArgumentError("clarification recompile requires a resolved clarification receipt");
    const clarifiedText = boundedText("autonomous clarified task", clarifiedTask, 32_000);
    const envelope = await this.blueprint(clarifiedText, {
      domain: resolvedPlan.domain,
      capability: options.capability,
      riskClass: options.riskClass,
      constraints: options.constraints,
      desiredOutputs: options.desiredOutputs,
      context: options.context,
      maxInputTokens: options.maxInputTokens,
      structuredDomainResponse: options.structuredDomainResponse,
      allowCrossDomain: false,
    });
    if (!envelope.blueprint || envelope.cross_domain_blueprint) throw new ArgumentError("clarification recompile requires a single-domain blueprint");
    const blueprint = envelope.blueprint;
    const descriptor = {
      schema: AUTONOMOUS_TASK_CLARIFICATION_RECOMPILE_SCHEMA,
      plan_digest: resolvedPlan.plan_digest,
      resolution_digest: resolvedReceipt.resolution_digest,
      original_task_digest: originalTaskDigest,
      recompiled_task_digest: blueprint.task_digest,
      domain: blueprint.domain_profile.domain,
      workflow_id: blueprint.workflow.workflow_id,
      recompiled_intent_digest: blueprint.task_intent.intent_digest,
      recompiled_decision_digest: blueprint.task_decision.decision_digest,
      execution_plan_digest: blueprint.plan.plan_digest,
      status: "ready" as const,
    };
    const recompileDigest = digestJsonSync(descriptor);
    const blueprintProjection: JsonObject = {
      schema: blueprint.schema,
      task_digest: blueprint.task_digest,
      route_digest: blueprint.route_digest,
      domain: blueprint.domain_profile.domain,
      workflow_id: blueprint.workflow.workflow_id,
      workflow_digest: blueprint.workflow.workflow_digest,
      task_intent_digest: blueprint.task_intent.intent_digest,
      task_decision_digest: blueprint.task_decision.decision_digest,
      task_decision_posture: blueprint.task_decision.posture,
      plan_digest: blueprint.plan.plan_digest,
      prompt_digest: blueprint.prompt.prompt_digest,
      learning_context_digest: blueprint.learning_context_digest,
      required_capabilities: [...blueprint.required_capabilities],
      stage_ids: blueprint.stage_execution_plans.map((stage) => stage.stage_id),
      execution: "not_started",
      authorization: "plan_only; provider_source_tool_and_effect_gates_remain_required",
      retention: "metadata_only; task_text_and_answer_values_not_retained",
      secret_material: "never_returned",
    };
    const toJSON = (): AutonomousClarificationRecompileProjection => ({
      ...descriptor,
      recompile_digest: recompileDigest,
      blueprint: blueprintProjection,
      execution: "not_started; fresh_blueprint_requires_existing_gates",
      authorization: "recompile_only; provider_source_tool_and_effect_gates_remain_required",
      retention: "metadata_only; task_text_and_answer_values_not_retained",
      secret_material: "never_returned",
    });
    return {
      ...descriptor,
      recompile_digest: recompileDigest,
      blueprint,
      toJSON,
    };
  }

  /** Validate a persisted recompile projection without restoring transient task values. */
  async validateClarificationRecompile(
    value: unknown,
    plan?: AutonomousTaskClarificationPlan | unknown,
    receipt?: AutonomousTaskClarificationResolution | unknown,
  ): Promise<AutonomousClarificationRecompileProjection> {
    return validateAutonomousTaskClarificationRecompile(value, plan, receipt) as AutonomousClarificationRecompileProjection;
  }

  /**
   * Route and execute one task through the same high-level boundary regardless of domain.
   * Deterministic mode performs a provider-free route/blueprint pass and then delegates to the
   * ordinary single- or cross-domain executor. Provider mode delegates to `planAndRun`, preserving
   * its separate planning acceptance and execution-approval gates. The route is resolved once and
   * then passed back as a digest-verified override, so semantic routing or model selection cannot
   * silently change between the preview and the actual invocation.
   */
  async runAuto(task: string, options: AutonomousAutoRunOptions = {}): Promise<AutonomousAutoRunResult> {
    const taskText = boundedText("autonomous runAuto task", task, 32_000);
    const planningMode = options.planningMode ?? "deterministic";
    if (planningMode !== "deterministic" && planningMode !== "provider") throw new ArgumentError("runAuto planningMode must be deterministic or provider");
    if (options.acceptedSingleDomainPlanRefinement !== undefined || options.acceptedCrossDomainPlanRefinement !== undefined) {
      throw new ArgumentError("runAuto creates its own route and plan boundary; apply an accepted refinement through run or runCrossDomain");
    }

    const nextAction = (status: AutonomousPlanAndRunStatus, result: AutonomousRunResult | AutonomousCrossDomainRunResult | null): AutonomousAutoRunNextAction => {
      if (status === "route_review_required" || status === "abstained") return "review_route";
      if (status === "plan_review_required" || status === "provider_invalid" || status === "provider_failed" || status === "provider_disagreement") return "review_plan";
      if (status === "approval_required" || status === "policy_review_required" || status === "policy_blocked" || status === "reconciliation_required") return "review_provider_or_effect_approval";
      if (result === null) return "inspect_result";
      return status === "completed" ? "complete" : "inspect_result";
    };

    if (planningMode === "provider") {
      const { planningMode: _planningMode, ...planningOptions } = options;
      const planned = await this.planAndRun(taskText, planningOptions);
      return {
        schema: AUTONOMOUS_AUTO_RUN_SCHEMA,
        status: planned.status,
        route: planned.route,
        semantic_route: planned.semantic_route ?? null,
        blueprint: planned.blueprint,
        planning: planned,
        result: planned.result,
        planning_mode: planningMode,
        next_action: nextAction(planned.status, planned.result),
        retention: "provider_response_local;route_and_plan_metadata_value_only;execution_result_caller_owned",
        authorization: "route_review_and_provider_or_effect_approval_remain_explicit",
      };
    }

    const {
      planningMode: _planningMode,
      planning: _planning,
      planningPromptStage: _planningPromptStage,
      planningPromptLearningState: _planningPromptLearningState,
      planningPromptLearningExploration: _planningPromptLearningExploration,
      acceptPlan: _acceptPlan,
      ...runOptions
    } = options;
    const costBudget = resolveAutonomousCostBudget(runOptions);
    const routeResolution = await this.resolveExecutionRoute(taskText, runOptions, costBudget);
    const route = routeResolution.route;
    const semanticRoute = routeResolution.semanticRoute;
    if (semanticRoute !== null && semanticRoute.status !== "completed") {
      const status = semanticRouteRunStatus(semanticRoute.status);
      return {
        schema: AUTONOMOUS_AUTO_RUN_SCHEMA,
        status,
        route,
        semantic_route: semanticRoute,
        blueprint: null,
        planning: null,
        result: null,
        planning_mode: planningMode,
        next_action: "review_route",
        retention: "provider_response_local;route_and_plan_metadata_value_only;execution_result_caller_owned",
        authorization: "route_review_and_provider_or_effect_approval_remain_explicit",
      };
    }
    const envelope = await this.blueprint(taskText, {
      domain: route.primary_domain ?? undefined,
      routeOverride: route,
      capability: runOptions.capability,
      context: runOptions.context,
      maxInputTokens: runOptions.maxInputTokens,
      tools: runOptions.tools?.map((tool) => tool.name),
      hints: runOptions.hints,
      minConfidence: runOptions.minConfidence,
      minMargin: runOptions.minMargin,
      maxDomains: runOptions.maxDomains,
      allowCrossDomain: runOptions.allowCrossDomain,
      subtasks: runOptions.subtasks,
      structuredDomainResponse: runOptions.structuredDomainResponse,
      toolSelectionState: runOptions.toolSelectionState,
      toolSelectionExploration: runOptions.toolSelectionExploration,
      maxToolRiskClass: runOptions.maxToolRiskClass,
    });
    if (route.abstained || !route.primary_domain || (!envelope.blueprint && !envelope.cross_domain_blueprint)) {
      return {
        schema: AUTONOMOUS_AUTO_RUN_SCHEMA,
        status: "route_review_required",
        route,
        semantic_route: semanticRoute,
        blueprint: envelope,
        planning: null,
        result: null,
        planning_mode: planningMode,
        next_action: "review_route",
        retention: "provider_response_local;route_and_plan_metadata_value_only;execution_result_caller_owned",
        authorization: "route_review_and_provider_or_effect_approval_remain_explicit",
      };
    }
    const executionOptions: AutonomousRunOptions = {
      ...runOptions,
      routeOverride: route,
      ...(runOptions.capability === undefined && envelope.blueprint?.capability_route?.selected_capability
        ? { capability: envelope.blueprint.capability_route.selected_capability }
        : {}),
      costBudget,
      maxTotalCostUnits: undefined,
    };
    const rawResult = envelope.cross_domain_blueprint
      ? await this.runCrossDomain(taskText, executionOptions)
      : await this.run(taskText, { ...executionOptions, domain: route.primary_domain });
    const result = semanticRoute === null ? rawResult : { ...rawResult, semantic_route: semanticRoute };
    return {
      schema: AUTONOMOUS_AUTO_RUN_SCHEMA,
      status: result.status,
      route,
      semantic_route: semanticRoute,
      blueprint: envelope,
      planning: null,
      result,
      planning_mode: planningMode,
      next_action: nextAction(result.status, result),
      retention: "provider_response_local;route_and_plan_metadata_value_only;execution_result_caller_owned",
      authorization: "route_review_and_provider_or_effect_approval_remain_explicit",
    };
  }

  /**
   * Compile one deterministic route, require a caller-owned launch admission for every selected
   * domain, and only then enter the normal provider/tool execution boundary.  The route is passed
   * back as an exact override so a second classification cannot widen or change the admitted
   * execution scope.  Provider-assisted semantic routing is rejected because its classifier is a
   * separate provider boundary and must be reviewed independently.
   */
  async runWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousRunOptions = {},
  ): Promise<AutonomousRunResult> {
    if (options.routeOverride !== undefined) throw new ArgumentError("runWithLaunchAdmission owns routeOverride; pass routing controls instead");
    if (options.semanticRouting !== undefined && options.semanticRouting !== false) throw new ArgumentError("launch-admitted execution requires provider-free routing; admit semantic routing separately");
    const route = await this.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    if (!route.abstained) authorizeAutonomousLaunchDomains(admission, route.selected_domains);
    const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...runOptions } = options;
    return this.run(task, { ...runOptions, routeOverride: route, semanticRouting: false });
  }

  /**
   * Route and admit an automatic run before any provider-assisted planning, credentials, or
   * tools are touched.  This covers both single-domain and cross-domain automatic execution and
   * preserves the frozen route through `runAuto`'s deterministic or reviewed planning mode.
   */
  async runAutoWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousAutoRunOptions = {},
  ): Promise<AutonomousAutoRunResult> {
    if (options.routeOverride !== undefined) throw new ArgumentError("runAutoWithLaunchAdmission owns routeOverride; pass routing controls instead");
    if (options.semanticRouting !== undefined && options.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic execution requires provider-free routing; admit semantic routing separately");
    const route = await this.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    if (!route.abstained) authorizeAutonomousLaunchDomains(admission, route.selected_domains);
    const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...runOptions } = options;
    return this.runAuto(task, { ...runOptions, routeOverride: route, semanticRouting: false });
  }

  /**
   * Perform only the provider-free route/admission half of `runAutoWithLaunchAdmission`.  This
   * gives a deployment a safe point to display or persist the exact launch decision before it
   * collects a short-lived credential or starts a provider session.
   */
  async authorizeAutoLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: Pick<AutonomousAutoRunOptions, "domain" | "hints" | "minConfidence" | "minMargin" | "maxDomains" | "allowCrossDomain" | "semanticRouting"> = {},
  ): Promise<AutonomousLaunchAdmissionReport> {
    if (options.semanticRouting !== undefined && options.semanticRouting !== false) throw new ArgumentError("launch-admitted automatic authorization requires provider-free routing; admit semantic routing separately");
    const route = await this.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
    const { authorizeAutonomousLaunchDomains, validateAutonomousLaunchAdmission } = await import("./autonomous-launch-admission.js");
    if (route.abstained) return validateAutonomousLaunchAdmission(admission);
    return authorizeAutonomousLaunchDomains(admission, route.selected_domains);
  }

  /**
   * Route once and execute the evaluator-backed single- or cross-domain decision cycle.
   *
   * This is the closed-loop counterpart to `runAuto()`: callers can supply an explicit
   * evaluator and learning controller, while the cycle retains the existing approval,
   * provider-planning, persistence, and rehydration boundaries.
   */
  async runAutoCycle(task: string, options: AutonomousAutoDecisionCycleOptions = {}): Promise<AutonomousAutoDecisionCycleResult> {
    const decisionStateStore = options.decisionStateStore ?? this.decisionCyclePersistence?.store;
    return runAutonomousAutoDecisionCycle(
      this,
      task,
      decisionStateStore === undefined || options.decisionStateStore !== undefined
        ? options
        : { ...options, decisionStateStore },
    );
  }

  /**
   * Run one automatic decision cycle only after a provider-free launch admission covers its
   * frozen route. Semantic routing is intentionally refused because its classifier call is a
   * separate provider boundary that must be reviewed independently.
   */
  async runAutoCycleWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousAutoDecisionCycleOptions = {},
  ): Promise<AutonomousAutoDecisionCycleResult> {
    if (options.routeOverride !== undefined) throw new ArgumentError("runAutoCycleWithLaunchAdmission owns routeOverride; pass routing controls instead");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted automatic decision-cycle execution requires provider-free routing; admit semantic routing separately before enabling it");
    const route = await this.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    if (!route.abstained) authorizeAutonomousLaunchDomains(admission, route.selected_domains);
    const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...cycleOptions } = options;
    return this.runAutoCycle(task, { ...cycleOptions, routeOverride: route, semanticRouting: undefined });
  }

  /**
   * Route once and execute the bounded evaluator-guided single- or cross-domain replan cycle.
   * Replan attempts retain the reviewed route, share the aggregate cost boundary, and preserve
   * the lower-level persistence and evaluator-settlement contracts.
   */
  async runAutoReplanCycle(task: string, options: AutonomousAutoReplanCycleOptions): Promise<AutonomousAutoReplanCycleResult> {
    return runAutonomousAutoReplanCycle(this, task, options);
  }

  /** Run the automatic evaluator/replan cycle only after its provider-free route is admitted. */
  async runAutoReplanCycleWithLaunchAdmission(
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousAutoReplanCycleOptions,
  ): Promise<AutonomousAutoReplanCycleResult> {
    if (options.routeOverride !== undefined) throw new ArgumentError("runAutoReplanCycleWithLaunchAdmission owns routeOverride; pass routing controls instead");
    if (options.semanticRouting?.enabled === true) throw new ArgumentError("launch-admitted automatic replan execution requires provider-free routing; admit semantic routing separately before enabling it");
    const route = await this.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
    const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
    if (!route.abstained) authorizeAutonomousLaunchDomains(admission, route.selected_domains);
    const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...cycleOptions } = options;
    return this.runAutoReplanCycle(task, { ...cycleOptions, routeOverride: route, semanticRouting: undefined });
  }

  /**
   * Compose the full agent-owned provider/tool boundary with the durable mission replan kernel.
   *
   * Callers provide the reviewed mission and evaluator. This method supplies the exact-step
   * autonomous adapter, the attached tool catalogue, provider credentials/models from `stepRun`,
   * and optional persistent prompt-learning receipts. Mission checkpoints contain only the
   * existing value-only metadata; provider responses, rendered prompts, credentials, and tool
   * arguments remain transient or in caller-owned stores.
   */
  async runMissionReplanCycle(
    mission: AgentMissionArgs,
    options: AutonomousAgentMissionReplanOptions,
  ): Promise<AutonomousMissionReplanResult> {
    if (!isObject(mission)) throw new ArgumentError("runMissionReplanCycle requires an AgentMissionArgs object");
    if (!isObject(options) || typeof options.evaluate !== "function") throw new ArgumentError("runMissionReplanCycle requires an evaluator callback");
    const catalogue = options.catalogue ?? this.toolCatalogue;
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("runMissionReplanCycle requires a ToolCatalogue on the agent or call options");

    // Dynamic import keeps the existing autonomous <-> mission-execution type boundary free of a
    // module-initialization cycle. `mission-execution` imports this class for its runtime adapter.
    const {
      AutonomousMissionExecutor,
      agentMissionStepExecutor,
      runAutonomousMissionReplanCycle,
    } = await import("./mission-execution.js").then(async (missionExecution) => ({
      AutonomousMissionExecutor: missionExecution.AutonomousMissionExecutor,
      agentMissionStepExecutor: missionExecution.agentMissionStepExecutor,
      runAutonomousMissionReplanCycle: (await import("./mission-replan.js")).runAutonomousMissionReplanCycle,
    }));

    const promptSelections: AutonomousPromptAdaptiveSelectionJSON[] = [];
    const promptSelectionDigests = new Set<string>();
    const promptCoordinator = this.promptLearningCoordinator;
    const defaultToolsForStep = (step: AgentMissionStep): readonly ProviderTool[] => {
      const definition = catalogue.get(step.tool);
      return [{ name: definition.name, description: definition.description, parameters: definition.inputSchema }];
    };
    const toolsForStep = (step: AgentMissionStep): readonly ProviderTool[] | undefined => options.toolsForStep?.(step) ?? defaultToolsForStep(step);
    const executeStep = agentMissionStepExecutor(this, {
      toolsForStep,
      run: options.stepRun,
      approveEffects: options.approveEffects,
      signal: options.signal,
      onPromptSelection: promptCoordinator === undefined
        ? undefined
        : (selection) => {
            const normalized = extractAutonomousPromptLearningSelections({ adaptive_selection: selection }, promptCoordinator.registry);
            for (const item of normalized) {
              if (promptSelectionDigests.has(item.selectionDigest)) continue;
              promptSelectionDigests.add(item.selectionDigest);
              promptSelections.push(item.toJSON());
            }
          },
    });
    const executor = new AutonomousMissionExecutor({
      agent: this,
      catalogue,
      executeStep,
      checkpointStore: options.checkpointStore,
      resultStore: options.resultStore,
      onStepOutcome: options.onStepOutcome,
    });
    const {
      catalogue: _catalogue,
      toolsForStep: _toolsForStep,
      stepRun: _stepRun,
      approveEffects: _approveEffects,
      checkpointStore: _checkpointStore,
      resultStore: _resultStore,
      onStepOutcome: _onStepOutcome,
      ...cycleOptions
    } = options;
    const result = await runAutonomousMissionReplanCycle(executor, mission, cycleOptions);
    if (promptCoordinator === undefined) return result;
    const prompt_learning: AutonomousMissionReplanPromptLearningProjection = {
      selection_count: promptSelections.length,
      selection_digests: promptSelections.map((selection) => selection.selection_digest),
      selections: promptSelections,
      retention: "selection_metadata_only;rendered_messages_transient",
      secret_material: "never_returned",
    };
    return { ...result, prompt_learning };
  }

  /**
   * Execute a caller-reviewed mission through the all-domain connector boundary.
   *
   * This is intentionally a thin agent-owned convenience method: the connector runtime,
   * approval decision, checkpoint store, and optional raw-result store remain caller-owned.
   * The attached ToolCatalogue is used when the caller does not provide an exact snapshot.
   */
  async runConnectorMission(
    mission: AgentMissionArgs,
    options: AutonomousConnectorMissionAgentRunOptions,
  ): Promise<AutonomousMissionExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("runConnectorMission requires execution options");
    const catalogue = options.catalogue ?? this.toolCatalogue;
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("runConnectorMission requires a ToolCatalogue on the agent or call options");
    const { runAutonomousConnectorMission } = await import("./autonomous-connector-mission.js");
    return runAutonomousConnectorMission(mission, { ...options, catalogue, agent: this });
  }

  /** Execute a connector mission only after the exact domain admission is approved. */
  async runConnectorMissionWithLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousConnectorMissionAgentRunOptions,
  ): Promise<AutonomousMissionExecutionResult> {
    if (!isObject(options)) throw new ArgumentError("runConnectorMissionWithLaunchAdmission requires execution options");
    const catalogue = options.catalogue ?? this.toolCatalogue;
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("runConnectorMissionWithLaunchAdmission requires a ToolCatalogue on the agent or call options");
    const { runAutonomousConnectorMissionWithLaunchAdmission } = await import("./autonomous-connector-mission.js");
    return runAutonomousConnectorMissionWithLaunchAdmission(mission, admission, { ...options, catalogue, agent: this });
  }

  /**
   * Provider-order an existing mission, require explicit caller acceptance, then execute it
   * through the strict connector adapter. A supplied accepted refinement is replayed locally and
   * never causes a second provider invocation.
   */
  async runConnectorMissionWithProviderPlanning(
    mission: AgentMissionArgs,
    options: AutonomousConnectorMissionProviderPlanningOptions,
  ): Promise<AutonomousConnectorPlannedMissionRun> {
    if (!isObject(options) || !isObject(options.execution)) throw new ArgumentError("runConnectorMissionWithProviderPlanning requires execution options");
    const catalogue = options.execution.catalogue ?? this.toolCatalogue;
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("runConnectorMissionWithProviderPlanning requires a ToolCatalogue on the agent or call options");
    const { runAutonomousConnectorMissionWithProviderPlanning } = await import("./autonomous-connector-mission.js");
    return runAutonomousConnectorMissionWithProviderPlanning(this, mission, {
      ...options,
      execution: { ...options.execution, catalogue, agent: this },
    });
  }

  /** Provider-order a mission with the launch admission checked before any planner call. */
  async runConnectorMissionWithProviderPlanningAndLaunchAdmission(
    mission: AgentMissionArgs,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousConnectorMissionProviderPlanningOptions,
  ): Promise<AutonomousConnectorPlannedMissionRun> {
    if (!isObject(options) || !isObject(options.execution)) throw new ArgumentError("runConnectorMissionWithProviderPlanningAndLaunchAdmission requires execution options");
    const catalogue = options.execution.catalogue ?? this.toolCatalogue;
    if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("runConnectorMissionWithProviderPlanningAndLaunchAdmission requires a ToolCatalogue on the agent or call options");
    const { runAutonomousConnectorMissionWithProviderPlanningAndLaunchAdmission } = await import("./autonomous-connector-mission.js");
    return runAutonomousConnectorMissionWithProviderPlanningAndLaunchAdmission(this, mission, admission, {
      ...options,
      execution: { ...options.execution, catalogue, agent: this },
    });
  }

  /**
   * Ask an approved provider to order an existing dependency-closed step graph.
   *
   * This is the shared planning primitive for mission execution and other schedulers that do
   * not own an AutonomousWorkflow. The provider sees bounded step metadata only. The returned
   * value is a proposal: callers must explicitly accept it and independently revalidate the
   * graph before dispatching any step.
   */
  async planOrderedStepsWithProvider(
    request: AutonomousOrderedStepPlanRequest,
    options: AutonomousProviderPlanningOptions = {},
  ): Promise<AutonomousOrderedStepPlanRefinementResult> {
    options = this.withPromptLearningOptions(options);
    if (!isObject(request) || typeof request.task !== "string" || !Array.isArray(request.steps)) throw new ArgumentError("ordered-step provider planning requires a task and step array");
    const taskText = boundedText("ordered-step planning task", request.task, 32_000);
    const steps = request.steps.map((step) => structuredClone(step));
    const ids = validateOrderedStepPlanningGraph(steps);
    const domains = [...new Set(steps.map((step) => step.domain))];
    const domain = request.domain ?? (domains.length === 1 ? domains[0] ?? "cross_domain" : "cross_domain");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`ordered-step provider planning has an unsupported domain: ${domain}`);
    const profile = await profileFor(domain);
    const taskDigest = await digestJson({ task: taskText });
    const derivedBasePlanDigest = await digestJson({ steps: steps.map((step) => ({ id: step.id, domain: step.domain, capability: step.capability, objective: step.objective, depends_on: [...(step.depends_on ?? [])], required: step.required ?? true })) });
    const basePlanDigest = request.basePlanDigest ?? derivedBasePlanDigest;
    if (typeof basePlanDigest !== "string" || !/^[0-9a-f]{64}$/.test(basePlanDigest)) throw new ArgumentError("ordered-step planning basePlanDigest must be a lowercase SHA-256 digest");
    if (request.protectedContractDigest !== undefined && request.protectedContractDigest !== null && !/^[0-9a-f]{64}$/.test(request.protectedContractDigest)) throw new ArgumentError("ordered-step planning protectedContractDigest must be a lowercase SHA-256 digest or null");
    let costBudget = resolveAutonomousCostBudget(options);
    const budgetSnapshot = (): AutonomousCostBudgetSnapshot | null => costBudget?.snapshot() ?? null;
    const prepared = await prepareOrderedStepPlanning({ ...request, task: taskText, steps }, profile, ids, taskDigest, basePlanDigest, options);
    const base = {
      schema: AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA,
      status: "approval_required",
      task_digest: taskDigest,
      base_plan_digest: basePlanDigest,
      protected_contract_digest: request.protectedContractDigest ?? null,
      priority_step_ids: [],
      focus_step_ids: [],
      review_required: true,
      confidence: 0,
      selected_model: null,
      selection_digest: null,
      planner_prompt_digest: prepared.promptDigest,
      ...(prepared.adaptiveSelection === undefined ? {} : { adaptive_selection: prepared.adaptiveSelection }),
      planner_plan_digest: null,
      outcome_digest: null,
      planner_context: prepared.learningContext,
      planner_context_digest: prepared.learningContextDigest,
      cost_budget: null,
      retention: "step_ids_and_digests_only; planner_transcript_not_retained",
      authorization: "plan_proposal_only; no_tools_arguments_or_effects_authorized",
    } satisfies AutonomousOrderedStepPlanRefinementResult;
    if (options.approveProviderCall !== true) return { ...base, cost_budget: budgetSnapshot() };
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("ordered-step provider planning requires at least one model candidate");
    options.authorizationContext?.authorizeOperation({
      operation: "plan",
      domain,
      resourceDigest: digestJsonSync({
        schema: "bioprism-autonomous-plan-authorization-resource/0.1",
        domain,
        task_digest: taskDigest,
        base_plan_digest: basePlanDigest,
        planner_prompt_digest: prepared.promptDigest,
        planner_context_digest: prepared.learningContextDigest,
      }),
    });
    let execution: Awaited<ReturnType<AutonomousRuntime["invoke"]>>;
    try {
      execution = await this.runtime.invoke({ ...prepared.plan, candidates }, {
        credential: options.credential,
        credentialFor: options.credentialFor,
        signal: options.signal,
        observer: options.observer,
        providerDispatchFence: options.providerDispatchFence,
        selectionEventCallback: options.selectionEventCallback,
        execution: options.execution,
        executionAttempt: options.executionAttempt,
        maxProviderFailovers: options.maxProviderFailovers,
        reserveCost: costBudget ? (costUnits) => costBudget.reserve(costUnits) : undefined,
        authorizationContext: options.authorizationContext,
        authorizationDomain: prepared.plan.domain,
      });
    } catch (error) {
      if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
      const failure = planningProviderFailureProjection(error);
      return {
        ...base,
        status: error instanceof ProviderRuntimeError && error.code === "invalid_response" ? "provider_invalid" : "provider_failed",
        failure,
        outcome_digest: await planningProviderFailureDigest(error),
        cost_budget: budgetSnapshot(),
      };
    }
    const metadata = {
      ...base,
      status: "provider_invalid" as const,
      selected_model: planningModelProjection(execution.selection),
      selection_digest: await digestJson(execution.selection),
      planner_plan_digest: await digestJson({ planner_output: execution.response.structured }),
      outcome_digest: await planningOutcomeDigest(execution, prepared.learningContextDigest, prepared.promptDigest),
      cost_budget: budgetSnapshot(),
    };
    const raw = execution.response.structured;
    if (!isObject(raw)) return metadata;
    const priority = raw.priority_order;
    const focus = raw.focus_step_ids;
    const reviewRequired = raw.review_required;
    const confidence = raw.confidence;
    const abstain = raw.abstain;
    const priorityIds = Array.isArray(priority) ? priority.filter((id): id is string => typeof id === "string") : [];
    const focusIds = Array.isArray(focus) ? focus.filter((id): id is string => typeof id === "string") : [];
    if (!Array.isArray(priority) || !Array.isArray(focus) || typeof reviewRequired !== "boolean" || typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1 || typeof abstain !== "boolean" || priorityIds.length !== priority.length || focusIds.length !== focus.length || priorityIds.length !== ids.length || new Set(priorityIds).size !== priorityIds.length || priorityIds.some((id) => !ids.includes(id)) || focusIds.some((id) => !ids.includes(id)) || new Set(focusIds).size !== focusIds.length) return metadata;
    const positions = new Map(priorityIds.map((id, index) => [id, index]));
    if (steps.some((step) => (step.depends_on ?? []).some((dependency) => (positions.get(dependency) ?? -1) > (positions.get(step.id) ?? -1)))) return { ...metadata, status: "provider_disagreement", priority_step_ids: [...priorityIds], focus_step_ids: [...focusIds], review_required: true, confidence };
    if (abstain) return { ...metadata, status: "provider_disagreement", priority_step_ids: [...priorityIds], focus_step_ids: [...focusIds], review_required: true, confidence };
    return { ...metadata, status: "completed", priority_step_ids: [...priorityIds], focus_step_ids: [...focusIds], review_required: reviewRequired, confidence };
  }

  /**
   * Ask an approved provider to refine an existing single-domain workflow.
   *
   * The provider receives only the reviewed stage catalogue and transient task prompt. The
   * returned value is a proposal: it cannot add stages, tools, credentials, permissions, effects,
   * or evidence, and it is never treated as authorization. Callers may persist the digest-only
   * result and explicitly apply it in a workflow executor.
   */
  async planWithProvider(
    blueprint: AutonomousTaskBlueprint,
    options: AutonomousProviderPlanningOptions = {},
  ): Promise<AutonomousPlanRefinementResult> {
    options = this.withPromptLearningOptions(options);
    if (!isObject(blueprint) || blueprint.schema !== "bioprism-python-autonomous-task/0.1") throw new ArgumentError("provider planning requires an AutonomousTaskBlueprint");
    if (!isObject(blueprint.workflow) || !Array.isArray(blueprint.workflow.stages)) throw new ProviderRuntimeError("provider planning workflow is malformed");
    let costBudget = resolveAutonomousCostBudget(options);
    const budgetSnapshot = (): AutonomousCostBudgetSnapshot | null => costBudget?.snapshot() ?? null;
    const stages = blueprint.workflow.stages;
    const stageIds = validatePlanningWorkflow(stages);
    const basePlanDigest = await digestJson(blueprint.plan);
    const contract: JsonObject = {
      schema: AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
      task_digest: blueprint.task_digest,
      base_plan_digest: basePlanDigest,
      workflow_digest: blueprint.workflow.workflow_digest,
      stage_catalogue: stages.map((stage) => ({ id: stage.id, depends_on: [...stage.depends_on], required_capabilities: [...stage.required_capabilities], evidence_outputs: [...stage.evidence_outputs], approval_required: stage.approval_required })),
      reconciliation: "priority_order_must_contain_each_existing_stage_exactly_once",
      does_not_authorize: ["tools", "provider effects", "external writes", "credentials"],
    };
    const prepared = await prepareProviderPlanning(blueprint.domain_profile, blueprint, stageIds, "focus_stage_ids", contract, options);
    const domainPolicyMode = normalizeAutonomousDomainPolicyMode(options.domainPolicyMode);
    const policy = autonomousDomainPolicy(blueprint.domain_profile.domain);
    const domainPolicyAdmission = domainPolicyAdmissionForPlanning(blueprint.domain_profile.domain, prepared.prompt.estimated_input_tokens, options, costBudget);
    const policyStatus = domainPolicyAdmission && domainPolicyAdmission.decision !== "admitted" ? domainPolicyStatus(domainPolicyAdmission) : null;
    if (policyStatus === null && domainPolicyMode === "strict" && costBudget === undefined) costBudget = new AutonomousCostBudget(policy.max_total_cost_units);
    const effectiveMaxProviderFailovers = domainPolicyMode === "strict"
      ? Math.min(options.maxProviderFailovers ?? Math.max(0, policy.max_provider_attempts - 1), Math.max(0, policy.max_provider_attempts - 1))
      : options.maxProviderFailovers;
    const base = {
      schema: AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
      status: "approval_required",
      task_digest: blueprint.task_digest,
      base_plan_digest: basePlanDigest,
      workflow_digest: blueprint.workflow.workflow_digest,
      priority_stage_ids: [],
      focus_stage_ids: [],
      review_required: true,
      confidence: 0,
      selected_model: null,
      selection_digest: null,
      planner_prompt_digest: prepared.promptDigest,
      ...(prepared.adaptiveSelection === undefined ? {} : { adaptive_selection: prepared.adaptiveSelection }),
      planner_plan_digest: null,
      outcome_digest: null,
      planner_context: prepared.learningContext,
      planner_context_digest: prepared.learningContextDigest,
      cost_budget: null,
      retention: "stage_ids_and_digests_only; planner_transcript_not_retained",
      authorization: "plan_proposal_only; no_tools_or_effects_authorized",
      ...(domainPolicyAdmission === null ? {} : { domain_policy_admission: domainPolicyAdmission }),
    } satisfies AutonomousPlanRefinementResult;
    if (policyStatus !== null) return { ...base, status: policyStatus };
    if (options.approveProviderCall !== true) return { ...base, status: "approval_required", cost_budget: budgetSnapshot() };
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("provider planning requires at least one model candidate");
    options.authorizationContext?.authorizeOperation({
      operation: "plan",
      domain: blueprint.domain_profile.domain,
      resourceDigest: digestJsonSync({
        schema: "bioprism-autonomous-plan-authorization-resource/0.1",
        domain: blueprint.domain_profile.domain,
        task_digest: blueprint.task_digest,
        base_plan_digest: basePlanDigest,
        planner_prompt_digest: prepared.promptDigest,
        planner_context_digest: prepared.learningContextDigest,
      }),
    });
    let execution: Awaited<ReturnType<AutonomousRuntime["invoke"]>>;
    try {
      execution = await this.runtime.invoke({ ...prepared.plan, candidates }, {
        credential: options.credential,
        credentialFor: options.credentialFor,
        signal: options.signal,
        observer: options.observer,
        providerDispatchFence: options.providerDispatchFence,
        selectionEventCallback: options.selectionEventCallback,
        execution: options.execution,
        executionAttempt: options.executionAttempt,
        maxProviderFailovers: effectiveMaxProviderFailovers,
        reserveCost: costBudget ? (costUnits) => costBudget.reserve(costUnits) : undefined,
        authorizationContext: options.authorizationContext,
        authorizationDomain: prepared.plan.domain,
      });
    } catch (error) {
      if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
      const failure = planningProviderFailureProjection(error);
      return {
        ...base,
        status: error instanceof ProviderRuntimeError && error.code === "invalid_response" ? "provider_invalid" : "provider_failed",
        failure,
        outcome_digest: await planningProviderFailureDigest(error),
        cost_budget: budgetSnapshot(),
      };
    }
    const selectionDigest = await digestJson(execution.selection);
    const outcomeDigest = await planningOutcomeDigest(execution, prepared.learningContextDigest, prepared.promptDigest);
    const plannerPlanDigest = await digestJson({ planner_output: execution.response.structured });
    const metadata = { ...base, selected_model: planningModelProjection(execution.selection), selection_digest: selectionDigest, planner_plan_digest: plannerPlanDigest, outcome_digest: outcomeDigest, cost_budget: budgetSnapshot() };
    const raw = execution.response.structured;
    if (!isObject(raw)) return { ...metadata, status: "provider_invalid" };
    const priority = raw.priority_order;
    const focus = raw.focus_stage_ids;
    const reviewRequired = raw.review_required;
    const confidence = raw.confidence;
    const abstain = raw.abstain;
    const priorityIds = Array.isArray(priority) ? priority.filter((id): id is string => typeof id === "string") : [];
    const focusIds = Array.isArray(focus) ? focus.filter((id): id is string => typeof id === "string") : [];
    if (!Array.isArray(priority) || !Array.isArray(focus) || typeof reviewRequired !== "boolean" || typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1 || typeof abstain !== "boolean" || priorityIds.length !== priority.length || focusIds.length !== focus.length || priorityIds.length !== stageIds.length || new Set(priorityIds).size !== priorityIds.length || priorityIds.some((id) => !stageIds.includes(id)) || focusIds.some((id) => !stageIds.includes(id)) || new Set(focusIds).size !== focusIds.length) return { ...metadata, status: "provider_invalid" };
    const positions = new Map(priorityIds.map((id, index) => [id, index]));
    if (stages.some((stage) => stage.depends_on.some((dependency) => (positions.get(dependency) ?? -1) > (positions.get(stage.id) ?? -1)))) return { ...metadata, priority_stage_ids: [...priorityIds], focus_stage_ids: [...focusIds], review_required: true, confidence, status: "provider_disagreement" };
    if (abstain) return { ...metadata, priority_stage_ids: [...priorityIds], focus_stage_ids: [...focusIds], review_required: true, confidence, status: "provider_disagreement" };
    return { ...metadata, priority_stage_ids: [...priorityIds], focus_stage_ids: [...focusIds], review_required: reviewRequired, confidence, status: "completed" };
  }

  /** Ask an approved provider to reorder only the already-reviewed cross-domain specialists. */
  async planCrossDomainWithProvider(
    blueprint: AutonomousCrossDomainBlueprint,
    options: AutonomousProviderPlanningOptions = {},
  ): Promise<AutonomousCrossDomainPlanRefinementResult> {
    options = this.withPromptLearningOptions(options);
    if (!isObject(blueprint) || blueprint.schema !== AUTONOMOUS_CROSS_DOMAIN_SCHEMA) throw new ArgumentError("cross-domain provider planning requires an AutonomousCrossDomainBlueprint");
    if (!Array.isArray(blueprint.child_ids) || !isObject(blueprint.dependency_graph) || !Array.isArray(blueprint.dependency_graph.fan_out)) throw new ProviderRuntimeError("cross-domain provider planning blueprint is malformed");
    let costBudget = resolveAutonomousCostBudget(options);
    const budgetSnapshot = (): AutonomousCostBudgetSnapshot | null => costBudget?.snapshot() ?? null;
    const childIds = [...blueprint.child_ids];
    if (childIds.length < 2 || childIds.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN || childIds.some((id) => typeof id !== "string" || !id.trim()) || new Set(childIds).size !== childIds.length) throw new ProviderRuntimeError("cross-domain provider planning children are malformed");
    const fanOutIds = blueprint.dependency_graph.fan_out.map((child) => isObject(child) && typeof child.id === "string" ? child.id : null);
    if (fanOutIds.length !== childIds.length || fanOutIds.some((id, index) => id !== childIds[index])) throw new ProviderRuntimeError("cross-domain provider planning dependency graph is not closed");
    const basePlanDigest = blueprint.plan_digest;
    const contract: JsonObject = {
      schema: AUTONOMOUS_CROSS_DOMAIN_PLAN_REFINEMENT_SCHEMA,
      task_digest: blueprint.task_digest,
      base_plan_digest: basePlanDigest,
      child_catalogue: blueprint.dependency_graph.fan_out.map((child) => ({ ...child })),
      reconciliation: "priority_order_must_contain_each_existing_child_exactly_once",
      does_not_authorize: ["new domains", "new tools", "new credentials", "effects", "synthesis authority"],
    };
    const prepared = await prepareProviderPlanning(blueprint.synthesis_blueprint.domain_profile, blueprint.synthesis_blueprint, childIds, "focus_child_ids", contract, options);
    const domainPolicyMode = normalizeAutonomousDomainPolicyMode(options.domainPolicyMode);
    const policy = autonomousDomainPolicy("cross_domain");
    const domainPolicyAdmission = domainPolicyAdmissionForPlanning("cross_domain", prepared.prompt.estimated_input_tokens, options, costBudget);
    const policyStatus = domainPolicyAdmission && domainPolicyAdmission.decision !== "admitted" ? domainPolicyStatus(domainPolicyAdmission) : null;
    if (policyStatus === null && domainPolicyMode === "strict" && costBudget === undefined) costBudget = new AutonomousCostBudget(policy.max_total_cost_units);
    const effectiveMaxProviderFailovers = domainPolicyMode === "strict"
      ? Math.min(options.maxProviderFailovers ?? Math.max(0, policy.max_provider_attempts - 1), Math.max(0, policy.max_provider_attempts - 1))
      : options.maxProviderFailovers;
    const base = {
      schema: AUTONOMOUS_CROSS_DOMAIN_PLAN_REFINEMENT_SCHEMA,
      status: "approval_required",
      task_digest: blueprint.task_digest,
      base_plan_digest: basePlanDigest,
      priority_child_ids: [],
      focus_child_ids: [],
      review_required: true,
      confidence: 0,
      selected_model: null,
      selection_digest: null,
      planner_prompt_digest: prepared.promptDigest,
      ...(prepared.adaptiveSelection === undefined ? {} : { adaptive_selection: prepared.adaptiveSelection }),
      planner_plan_digest: null,
      outcome_digest: null,
      planner_context: prepared.learningContext,
      planner_context_digest: prepared.learningContextDigest,
      cost_budget: null,
      retention: "child_ids_and_digests_only; planner_transcript_not_retained",
      authorization: "plan_proposal_only; no_tools_or_effects_authorized",
      ...(domainPolicyAdmission === null ? {} : { domain_policy_admission: domainPolicyAdmission }),
    } satisfies AutonomousCrossDomainPlanRefinementResult;
    if (policyStatus !== null) return { ...base, status: policyStatus };
    if (options.approveProviderCall !== true) return { ...base, cost_budget: budgetSnapshot() };
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("cross-domain provider planning requires at least one model candidate");
    options.authorizationContext?.authorizeOperation({
      operation: "plan",
      domain: "cross_domain",
      resourceDigest: digestJsonSync({
        schema: "bioprism-autonomous-plan-authorization-resource/0.1",
        domain: "cross_domain",
        task_digest: blueprint.task_digest,
        base_plan_digest: basePlanDigest,
        planner_prompt_digest: prepared.promptDigest,
        planner_context_digest: prepared.learningContextDigest,
      }),
    });
    let execution: Awaited<ReturnType<AutonomousRuntime["invoke"]>>;
    try {
      execution = await this.runtime.invoke({ ...prepared.plan, candidates }, {
        credential: options.credential,
        credentialFor: options.credentialFor,
        signal: options.signal,
        observer: options.observer,
        providerDispatchFence: options.providerDispatchFence,
        selectionEventCallback: options.selectionEventCallback,
        execution: options.execution,
        executionAttempt: options.executionAttempt,
        maxProviderFailovers: effectiveMaxProviderFailovers,
        reserveCost: costBudget ? (costUnits) => costBudget.reserve(costUnits) : undefined,
        authorizationContext: options.authorizationContext,
        authorizationDomain: prepared.plan.domain,
      });
    } catch (error) {
      if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
      const failure = planningProviderFailureProjection(error);
      return {
        ...base,
        status: error instanceof ProviderRuntimeError && error.code === "invalid_response" ? "provider_invalid" : "provider_failed",
        failure,
        outcome_digest: await planningProviderFailureDigest(error),
        cost_budget: budgetSnapshot(),
      };
    }
    const metadata = {
      ...base,
      status: "provider_invalid" as const,
      selected_model: planningModelProjection(execution.selection),
      selection_digest: await digestJson(execution.selection),
      planner_plan_digest: await digestJson({ planner_output: execution.response.structured }),
      outcome_digest: await planningOutcomeDigest(execution, prepared.learningContextDigest, prepared.promptDigest),
      cost_budget: budgetSnapshot(),
    };
    const raw = execution.response.structured;
    if (!isObject(raw)) return metadata;
    const priority = raw.priority_order;
    const focus = raw.focus_child_ids;
    const reviewRequired = raw.review_required;
    const confidence = raw.confidence;
    const abstain = raw.abstain;
    const priorityIds = Array.isArray(priority) ? priority.filter((id): id is string => typeof id === "string") : [];
    const focusIds = Array.isArray(focus) ? focus.filter((id): id is string => typeof id === "string") : [];
    if (!Array.isArray(priority) || !Array.isArray(focus) || typeof reviewRequired !== "boolean" || typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1 || typeof abstain !== "boolean" || priorityIds.length !== priority.length || focusIds.length !== focus.length || priorityIds.length !== childIds.length || new Set(priorityIds).size !== priorityIds.length || priorityIds.some((id) => !childIds.includes(id)) || focusIds.some((id) => !childIds.includes(id)) || new Set(focusIds).size !== focusIds.length) return metadata;
    if (abstain) return { ...metadata, status: "provider_disagreement", priority_child_ids: [...priorityIds], focus_child_ids: [...focusIds], review_required: true, confidence };
    return { ...metadata, status: "completed", priority_child_ids: [...priorityIds], focus_child_ids: [...focusIds], review_required: reviewRequired, confidence };
  }

  /**
   * Run the complete plan-review-invoke path for either a single domain or a cross-domain route.
   * Planning and execution retain separate approval gates; a provider proposal is never applied
   * unless the caller sets acceptPlan and the returned proposal is dependency-closed.
   */
  async planAndRun(task: string, options: AutonomousPlanAndRunOptions = {}): Promise<AutonomousPlanAndRunResult> {
    const taskText = boundedText("autonomous planAndRun task", task, 32_000);
    validateAutonomousStructuredOutputOptions(options);
    if (options.acceptedSingleDomainPlanRefinement !== undefined || options.acceptedCrossDomainPlanRefinement !== undefined) throw new ArgumentError("planAndRun creates its own accepted proposal; use run or runCrossDomain to apply an existing proposal");
    const planning = options.planning;
    const sharedBudget = resolvePlanAndRunBudget(options, planning);
    const routeResolution = await this.resolveExecutionRoute(taskText, options, sharedBudget);
    const route = routeResolution.route;
    const semanticRoute = routeResolution.semanticRoute;
    if (semanticRoute !== null && semanticRoute.status !== "completed") {
      return {
        schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA,
        status: semanticRouteRunStatus(semanticRoute.status),
        route,
        semantic_route: semanticRoute,
        blueprint: null,
        plan_refinement: null,
        result: null,
        retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned",
        authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval",
      };
    }
    const envelope = await this.blueprint(taskText, {
      domain: route.primary_domain ?? undefined,
      routeOverride: route,
      capability: options.capability,
      context: options.context,
      maxInputTokens: options.maxInputTokens,
      tools: options.tools?.map((tool) => tool.name),
      hints: options.hints,
      subtasks: options.subtasks,
      structuredDomainResponse: options.structuredDomainResponse,
      maxToolRiskClass: options.maxToolRiskClass,
    });
    if (route.abstained || !route.primary_domain || (!envelope.blueprint && !envelope.cross_domain_blueprint)) {
      return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status: "route_review_required", route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: null, result: null, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
    }
    const planningOptions: AutonomousProviderPlanningOptions = {
      ...(planning ?? {}),
      ...(planning?.runId === undefined && options.providerIdempotencyKey !== undefined
        ? { runId: await scopedProviderIdempotencyKey(options.providerIdempotencyKey, { phase: "planning" }) }
        : {}),
      ...(planning?.promptTemplate === undefined && options.promptTemplate !== undefined ? { promptTemplate: options.promptTemplate } : {}),
      ...(planning?.promptRegistry === undefined && options.promptRegistry !== undefined ? { promptRegistry: options.promptRegistry } : {}),
      ...(planning?.promptSelection === undefined && options.promptSelection !== undefined ? { promptSelection: options.promptSelection } : {}),
      ...(planning?.promptLearningState === undefined && options.planningPromptLearningState !== undefined ? { promptLearningState: options.planningPromptLearningState } : {}),
      ...(planning?.promptLearningState === undefined && options.planningPromptLearningState === undefined && options.promptLearningState !== undefined ? { promptLearningState: options.promptLearningState } : {}),
      ...(planning?.promptLearningExploration === undefined && options.planningPromptLearningExploration !== undefined ? { promptLearningExploration: options.planningPromptLearningExploration } : {}),
      ...(planning?.promptLearningExploration === undefined && options.planningPromptLearningExploration === undefined && options.promptLearningExploration !== undefined ? { promptLearningExploration: options.promptLearningExploration } : {}),
      ...(planning?.promptStage === undefined ? { promptStage: options.planningPromptStage ?? "planning" } : {}),
      ...(planning?.selectionWeights === undefined && options.selectionWeights !== undefined ? { selectionWeights: options.selectionWeights } : {}),
      ...(planning?.selectionObservations === undefined && options.selectionObservations !== undefined ? { selectionObservations: options.selectionObservations } : {}),
      ...(sharedBudget ? { costBudget: sharedBudget, maxTotalCostUnits: undefined } : {}),
      ...(options.domainPolicyMode === undefined ? {} : { domainPolicyMode: options.domainPolicyMode }),
      ...(options.domainPolicyEvidenceReady === undefined ? {} : { domainPolicyEvidenceReady: options.domainPolicyEvidenceReady }),
      ...(options.domainPolicyEvaluatorConfigured === undefined ? {} : { domainPolicyEvaluatorConfigured: options.domainPolicyEvaluatorConfigured }),
      ...(options.domainPolicyEffectsRequested === undefined ? {} : { domainPolicyEffectsRequested: options.domainPolicyEffectsRequested }),
      ...(options.domainPolicyEffectsApproved === undefined ? {} : { domainPolicyEffectsApproved: options.domainPolicyEffectsApproved }),
    };
    const executionOptions: AutonomousRunOptions = {
      ...options,
      routeOverride: route,
      costBudget: sharedBudget,
      maxTotalCostUnits: undefined,
      acceptedSingleDomainPlanRefinement: undefined,
      acceptedCrossDomainPlanRefinement: undefined,
    };
    delete (executionOptions as AutonomousPlanAndRunOptions).planning;
    delete (executionOptions as AutonomousPlanAndRunOptions).planningPromptStage;
    delete (executionOptions as AutonomousPlanAndRunOptions).planningPromptLearningState;
    delete (executionOptions as AutonomousPlanAndRunOptions).planningPromptLearningExploration;
    delete (executionOptions as AutonomousPlanAndRunOptions).acceptPlan;
    if (envelope.cross_domain_blueprint) {
      const proposal = await this.planCrossDomainWithProvider(envelope.cross_domain_blueprint, planningOptions);
      if (proposal.status !== "completed") {
        const status: AutonomousPlanAndRunStatus = proposal.status === "approval_required" ? "approval_required" : proposal.status === "policy_review_required" ? "policy_review_required" : proposal.status === "policy_blocked" ? "policy_blocked" : proposal.status === "provider_invalid" ? "provider_invalid" : proposal.status === "provider_failed" ? "provider_failed" : "provider_disagreement";
        return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status, route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result: null, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
      }
      if (proposal.review_required || options.acceptPlan !== true) return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status: "plan_review_required", route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result: null, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
      const result = await this.runCrossDomain(taskText, { ...executionOptions, subtasks: options.subtasks, acceptedCrossDomainPlanRefinement: proposal });
      return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status: result.status, route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
    }
    const blueprint = envelope.blueprint;
    if (!blueprint) throw new ProviderRuntimeError("planAndRun single-domain blueprint is missing");
    const proposal = await this.planWithProvider(blueprint, planningOptions);
    if (proposal.status !== "completed") {
      const status: AutonomousPlanAndRunStatus = proposal.status === "approval_required" ? "approval_required" : proposal.status === "policy_review_required" ? "policy_review_required" : proposal.status === "policy_blocked" ? "policy_blocked" : proposal.status === "provider_invalid" ? "provider_invalid" : proposal.status === "provider_failed" ? "provider_failed" : "provider_disagreement";
      return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status, route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result: null, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
    }
    if (proposal.review_required || options.acceptPlan !== true) return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status: "plan_review_required", route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result: null, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
    const result = await this.run(taskText, { ...executionOptions, acceptedSingleDomainPlanRefinement: proposal });
    return { schema: AUTONOMOUS_PLAN_AND_RUN_SCHEMA, status: result.status, route, semantic_route: semanticRoute, blueprint: envelope, plan_refinement: proposal, result, retention: "provider_response_local;plan_proposal_value_only;execution_result_caller_owned", authorization: "planning_acceptance_and_provider_invocation_require_separate_explicit_approval" };
  }

  /** Build a bounded fan-out/fan-in plan without contacting a provider or executing a tool. */
  private async buildCrossDomainBlueprint(
    taskText: string,
    route: AutonomousRouteProposal,
    options: { capability?: string; context?: readonly AutonomousPromptChunk[]; hints?: readonly string[]; maxInputTokens?: number; tools?: readonly string[]; subtasks?: readonly AutonomousCrossDomainSubtask[]; structuredDomainResponse?: boolean; toolSelectionState?: AutonomousToolSelectionState | null; toolSelectionExploration?: number; maxToolRiskClass?: AutonomousToolRiskClass } = {},
  ): Promise<AutonomousCrossDomainBlueprint> {
    const selectedDomains = route.selected_domains.slice(0, AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN);
    if (selectedDomains.length < 2) throw new ProviderRuntimeError("cross-domain blueprint requires at least two routed domains");
    const parentDigest = route.task_digest;
    const supplied: AutonomousCrossDomainSubtask[] = options.subtasks ? [...options.subtasks] : selectedDomains.map((domain, index) => ({
      id: `child-${index + 1}`,
      domain,
      task: `Analyze the ${domain} aspects of: ${taskText}`,
    } satisfies AutonomousCrossDomainSubtask));
    if (!supplied.length || supplied.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN) throw new ArgumentError("cross-domain subtasks must contain between 1 and 8 items");
    const selectedSet = new Set(selectedDomains);
    const childIds = new Set<string>();
    const children: AutonomousTaskBlueprint[] = [];
    const childMetadata: Array<{ id: string; domain: AutonomousDomainName; task_digest: string; workflow_id: string; workflow_digest: string }> = [];
    for (let index = 0; index < supplied.length; index += 1) {
      const subtask = supplied[index];
      if (!isObject(subtask) || !AUTONOMOUS_DOMAIN_NAMES.includes(subtask.domain)) throw new ArgumentError("cross-domain subtask has an unsupported domain");
      if (!selectedSet.has(subtask.domain)) throw new ArgumentError(`cross-domain subtask domain ${subtask.domain} was not selected by the route`);
      const id = boundedIdentifier("cross-domain child id", subtask.id ?? `child-${index + 1}`);
      if (childIds.has(id)) throw new ArgumentError(`cross-domain child id is duplicated: ${id}`);
      childIds.add(id);
      const childTask = boundedText(`cross-domain child ${id} task`, subtask.task, 32_000);
      const profile = await profileFor(subtask.domain);
      const capabilityRoute = routeAutonomousCapability(
        childTask,
        profile.domain,
        subtask.capability === undefined ? {} : { explicitCapability: subtask.capability },
      );
      const effectiveCapability = subtask.capability ?? capabilityRoute.selected_capability ?? profile.default_capability;
      const childContext: AutonomousPromptChunk[] = [
        ...(options.context ?? []),
        { id: "cross-domain-parent", content: `Parent route digest: ${parentDigest}; child id: ${id}`, required: true, priority: 100 },
        ...(subtask.context ?? []),
      ];
      const activeToolNames = options.tools ? this.filterActivatedToolNames([...options.tools]) : await this.liveToolNamesForTask(childTask, [subtask.domain], effectiveCapability, this.effectiveToolSelectionState(options.toolSelectionState), options.toolSelectionExploration, options.maxToolRiskClass);
      const child = await buildTaskBlueprint(profile, childTask, {
        capability: effectiveCapability,
        capabilityRoute,
        routeDigest: route.route_digest,
        context: childContext,
        maxInputTokens: options.maxInputTokens,
        activeToolNames,
        selectedToolNames: activeToolNames,
        structuredDomainResponse: options.structuredDomainResponse,
      });
      children.push(child);
      childMetadata.push({ id, domain: profile.domain, task_digest: child.task_digest, workflow_id: profile.workflow.workflow_id, workflow_digest: profile.workflow.workflow_digest });
    }
    const synthesisProfile = await profileFor("cross_domain");
    const synthesisContext: AutonomousPromptChunk[] = [
      ...(options.context ?? []),
      {
        id: "cross-domain-children",
        content: JSON.stringify({ parent_task_digest: parentDigest, children: childMetadata }),
        required: true,
        priority: 100,
      },
    ];
    const synthesisTask = `Synthesize the domain analyses for: ${taskText}`;
    const synthesisTools = options.tools ? this.filterActivatedToolNames([...options.tools]) : await this.liveToolNamesForTask(synthesisTask, [...selectedDomains, "cross_domain"], options.capability ?? synthesisProfile.default_capability, this.effectiveToolSelectionState(options.toolSelectionState), options.toolSelectionExploration, options.maxToolRiskClass);
    const synthesis = await buildTaskBlueprint(synthesisProfile, synthesisTask, {
      capability: options.capability ?? synthesisProfile.default_capability,
      routeDigest: route.route_digest,
      context: synthesisContext,
      maxInputTokens: options.maxInputTokens,
      activeToolNames: synthesisTools,
      selectedToolNames: synthesisTools,
      structuredDomainResponse: options.structuredDomainResponse,
    });
    const descriptor = {
      schema: AUTONOMOUS_CROSS_DOMAIN_SCHEMA,
      task_digest: parentDigest,
      route_digest: route.route_digest,
      child_ids: [...childIds],
      children: childMetadata,
      synthesis_task_digest: synthesis.task_digest,
      execution: "not_started" as const,
      authorization: "caller_approval_per_provider_or_effect_boundary" as const,
    };
    return {
      schema: AUTONOMOUS_CROSS_DOMAIN_SCHEMA,
      task_digest: parentDigest,
      route_digest: route.route_digest,
      child_ids: [...childIds],
      child_blueprints: children,
      synthesis_blueprint: synthesis,
      dependency_graph: { fan_out: childMetadata.map(({ id, domain, task_digest }) => ({ id, domain, task_digest })), fan_in: synthesis.task_digest },
      plan_digest: await digestJson(descriptor),
      execution: "not_started",
      authorization: "caller_approval_per_provider_or_effect_boundary",
    };
  }

  /**
   * Run one bounded attempt while advancing a durable objective lifecycle.
   *
   * The provider result is returned to the caller but never copied into the goal ledger.
   * The ledger receives only lifecycle state, criterion status, bounded blocker identifiers,
   * and a digest of the value-only outcome projection. This makes approval pauses, partial
   * progress, provider failures, and evaluator-incomplete results restartable across domains.
   */
  async runGoalStep(
    goalStore: InMemoryAutonomousGoalLedger,
    goalId: string,
    task: string,
    domain: AutonomousDomainName,
    options: {
      goalCriteria?: readonly AutonomousGoalCriterion[];
      goalMaxAttempts?: number;
      goalCapability?: string | null;
      goalRiskClass?: string | null;
      criterionUpdates?: readonly JsonObject[];
      settlementMetadata?: AutonomousGoalSettlementMetadata;
      runOptions?: AutonomousRunOptions;
    } = {},
  ): Promise<AutonomousGoalStepResult> {
    if (!(goalStore instanceof InMemoryAutonomousGoalLedger)) throw new ArgumentError("goalStore must be an InMemoryAutonomousGoalLedger");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("goal domain is unsupported");
    const taskText = boundedText("goal task", task, 32_000);
    const runOptions = options.runOptions ?? {};
    const settlementMetadata = options.settlementMetadata ?? {};
    for (const [name, value] of Object.entries(settlementMetadata)) {
      if (!["evaluator_digest", "learning_state_digest", "progress_digest"].includes(name)) throw new ArgumentError(`unsupported goal settlement metadata: ${name}`);
      if (value !== null && value !== undefined && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`goal settlement ${name} must be a digest or null`);
    }
    if (Object.prototype.hasOwnProperty.call(runOptions, "domain")) throw new ArgumentError("runOptions cannot override goal domain");
    const taskDigest = goalTaskDigest(taskText);
    const requestedCapability = options.goalCapability ?? runOptions.capability ?? null;
    const requestedRiskClass = options.goalRiskClass ?? null;
    let current = goalStore.get(goalId);
    if (current === null) {
      current = goalStore.create({
        goal_id: goalId,
        task_digest: taskDigest,
        domain,
        capability: requestedCapability,
        risk_class: requestedRiskClass,
        criteria: options.goalCriteria ?? [],
        max_attempts: options.goalMaxAttempts ?? 8,
      });
    } else {
      if (current.task_digest !== taskDigest || current.domain !== domain) throw new ArgumentError("goal identity does not match the requested task or domain");
      if (requestedCapability !== null && current.capability !== requestedCapability) throw new ArgumentError("goal capability does not match the requested capability");
      if (requestedRiskClass !== null && current.risk_class !== requestedRiskClass) throw new ArgumentError("goal risk class does not match the requested risk class");
    }
    if (current.status === "completed" || current.status === "cancelled") {
      return {
        schema: AUTONOMOUS_GOAL_STEP_SCHEMA,
        goal: current,
        result: null,
        result_status: "terminal",
        goal_status: current.status,
        outcome_digest: await digestJson({ goal_id: current.goal_id, attempt: current.attempt, result_status: "terminal" }),
        evaluator_digest: current.evaluator_digest,
        learning_state_digest: current.learning_state_digest,
        progress_digest: current.progress_digest,
        retention: AUTONOMOUS_GOAL_RETENTION,
        secret_material: "never_returned",
      };
    }
    if (current.status === "blocked" || current.status === "failed") current = goalStore.transition(current.goal_id, "ready", { expected_revision: current.revision });
    const running = goalStore.transition(current.goal_id, "running", { expected_revision: current.revision });
    const effectiveCapability = runOptions.capability ?? current.capability ?? undefined;
    const effectiveRunOptions: AutonomousRunOptions = {
      ...runOptions,
      domain,
      ...(effectiveCapability === undefined ? {} : { capability: effectiveCapability }),
    };
    let result: AutonomousRunResult;
    try {
      result = await this.run(taskText, effectiveRunOptions);
    } catch (error) {
      goalStore.transition(running.goal_id, "failed", {
        expected_revision: running.revision,
        blockers: [`exception:${error instanceof Error ? error.constructor.name : "UnknownError"}`],
        next_action_digest: goalTaskDigest("goal-retry"),
        outcome_digest: await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: `exception:${error instanceof Error ? error.constructor.name : "UnknownError"}` }),
      });
      throw error;
    }
    const candidateResultStatus = typeof result.status === "string" ? result.status.trim() : "";
    const resultStatus = candidateResultStatus && !candidateResultStatus.includes("\u0000") && new TextEncoder().encode(candidateResultStatus).byteLength <= 128 ? candidateResultStatus : "failed";
    const outcomeDigest = await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: resultStatus });
    let settled = running;
    let updated: AutonomousGoalRecord;
    let evaluatorDigest = settlementMetadata.evaluator_digest ?? null;
    try {
      if (options.criterionUpdates && options.criterionUpdates.length) settled = goalStore.updateCriteria(running.goal_id, options.criterionUpdates, { expected_revision: running.revision });
      if (options.criterionUpdates && options.criterionUpdates.length && evaluatorDigest === null) evaluatorDigest = await digestJson({ criteria: settled.criteria });
      const goalStatus = goalStatusForResult(resultStatus, settled.criteria.every((criterion) => !criterion.required || criterion.status === "satisfied" || criterion.status === "waived"));
      updated = goalStore.transition(settled.goal_id, goalStatus, {
        expected_revision: settled.revision,
        blockers: goalStatus === "completed" ? [] : [`result:${resultStatus}`],
        next_action_digest: goalStatus === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`),
        outcome_digest: outcomeDigest,
        ...(evaluatorDigest === null ? {} : { evaluator_digest: evaluatorDigest }),
        ...(settlementMetadata.learning_state_digest === null || settlementMetadata.learning_state_digest === undefined ? {} : { learning_state_digest: settlementMetadata.learning_state_digest }),
        ...(settlementMetadata.progress_digest === null || settlementMetadata.progress_digest === undefined ? {} : { progress_digest: settlementMetadata.progress_digest }),
      });
    } catch (error) {
      goalStore.transition(settled.goal_id, "blocked", {
        expected_revision: settled.revision,
        blockers: [`settlement:${error instanceof Error ? error.constructor.name : "UnknownError"}`],
        next_action_digest: goalTaskDigest("goal-settlement-review"),
        outcome_digest: outcomeDigest,
      });
      throw error;
    }
    return {
      schema: AUTONOMOUS_GOAL_STEP_SCHEMA,
      goal: updated,
      result,
      result_status: resultStatus,
      goal_status: updated.status,
      outcome_digest: outcomeDigest,
      evaluator_digest: updated.evaluator_digest,
      learning_state_digest: updated.learning_state_digest,
      progress_digest: updated.progress_digest,
      retention: AUTONOMOUS_GOAL_RETENTION,
      secret_material: "never_returned",
    };
  }

  /** Run one bounded cross-domain fan-out/fan-in attempt under the same durable goal contract. */
  async runCrossDomainGoalStep(
    goalStore: InMemoryAutonomousGoalLedger,
    goalId: string,
    task: string,
    options: {
      goalCriteria?: readonly AutonomousGoalCriterion[];
      goalMaxAttempts?: number;
      goalCapability?: string | null;
      goalRiskClass?: string | null;
      criterionUpdates?: readonly JsonObject[];
      settlementMetadata?: AutonomousGoalSettlementMetadata;
      runOptions?: AutonomousCrossDomainRunOptions;
    } = {},
  ): Promise<AutonomousGoalStepResult> {
    if (!(goalStore instanceof InMemoryAutonomousGoalLedger)) throw new ArgumentError("goalStore must be an InMemoryAutonomousGoalLedger");
    const taskText = boundedText("cross-domain goal task", task, 32_000);
    const runOptions = options.runOptions ?? {};
    if (Object.prototype.hasOwnProperty.call(runOptions, "domain")) throw new ArgumentError("runOptions cannot override cross-domain goal domain");
    const settlementMetadata = options.settlementMetadata ?? {};
    for (const [name, value] of Object.entries(settlementMetadata)) {
      if (!["evaluator_digest", "learning_state_digest", "progress_digest"].includes(name)) throw new ArgumentError(`unsupported goal settlement metadata: ${name}`);
      if (value !== null && value !== undefined && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`goal settlement ${name} must be a digest or null`);
    }
    const taskDigest = goalTaskDigest(taskText);
    const requestedCapability = options.goalCapability ?? runOptions.capability ?? null;
    const requestedRiskClass = options.goalRiskClass ?? null;
    let current = goalStore.get(goalId);
    if (current === null) {
      current = goalStore.create({ goal_id: goalId, task_digest: taskDigest, domain: "cross_domain", capability: requestedCapability, risk_class: requestedRiskClass, criteria: options.goalCriteria ?? [], max_attempts: options.goalMaxAttempts ?? 8 });
    } else {
      if (current.task_digest !== taskDigest || current.domain !== "cross_domain") throw new ArgumentError("cross-domain goal identity does not match the requested task");
      if (requestedCapability !== null && current.capability !== requestedCapability) throw new ArgumentError("goal capability does not match the requested capability");
      if (requestedRiskClass !== null && current.risk_class !== requestedRiskClass) throw new ArgumentError("goal risk class does not match the requested risk class");
    }
    if (current.status === "completed" || current.status === "cancelled") {
      return { schema: AUTONOMOUS_GOAL_STEP_SCHEMA, goal: current, result: null, result_status: "terminal", goal_status: current.status, outcome_digest: await digestJson({ goal_id: current.goal_id, attempt: current.attempt, result_status: "terminal" }), evaluator_digest: current.evaluator_digest, learning_state_digest: current.learning_state_digest, progress_digest: current.progress_digest, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
    }
    if (current.status === "blocked" || current.status === "failed") current = goalStore.transition(current.goal_id, "ready", { expected_revision: current.revision });
    const running = goalStore.transition(current.goal_id, "running", { expected_revision: current.revision });
    const effectiveCapability = runOptions.capability ?? current.capability ?? undefined;
    const effectiveRunOptions: AutonomousCrossDomainRunOptions = { ...runOptions, ...(effectiveCapability === undefined ? {} : { capability: effectiveCapability }) };
    let result: AutonomousCrossDomainRunResult;
    try {
      result = await this.runCrossDomain(taskText, effectiveRunOptions);
    } catch (error) {
      goalStore.transition(running.goal_id, "failed", { expected_revision: running.revision, blockers: [`exception:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-retry"), outcome_digest: await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: `exception:${error instanceof Error ? error.constructor.name : "UnknownError"}` }) });
      throw error;
    }
    const candidateResultStatus = typeof result.status === "string" ? result.status.trim() : "";
    const resultStatus = candidateResultStatus && !candidateResultStatus.includes("\u0000") && new TextEncoder().encode(candidateResultStatus).byteLength <= 128 ? candidateResultStatus : "failed";
    const outcomeDigest = await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: resultStatus });
    let settled = running;
    let evaluatorDigest = settlementMetadata.evaluator_digest ?? null;
    let progressDigest = settlementMetadata.progress_digest ?? null;
    if (progressDigest === null) progressDigest = await digestJson({ result_status: resultStatus, child_statuses: Array.isArray(result.child_runs) ? result.child_runs.map((child) => child.result.status) : [], completed_children: result.completed_children, total_children: result.total_children });
    let updated: AutonomousGoalRecord;
    try {
      if (options.criterionUpdates && options.criterionUpdates.length) settled = goalStore.updateCriteria(running.goal_id, options.criterionUpdates, { expected_revision: running.revision });
      if (options.criterionUpdates && options.criterionUpdates.length && evaluatorDigest === null) evaluatorDigest = await digestJson({ criteria: settled.criteria });
      const goalStatus = goalStatusForResult(resultStatus, settled.criteria.every((criterion) => !criterion.required || criterion.status === "satisfied" || criterion.status === "waived"));
      updated = goalStore.transition(settled.goal_id, goalStatus, { expected_revision: settled.revision, blockers: goalStatus === "completed" ? [] : [`result:${resultStatus}`], next_action_digest: goalStatus === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`), outcome_digest: outcomeDigest, ...(evaluatorDigest === null ? {} : { evaluator_digest: evaluatorDigest }), ...(settlementMetadata.learning_state_digest === null || settlementMetadata.learning_state_digest === undefined ? {} : { learning_state_digest: settlementMetadata.learning_state_digest }), progress_digest: progressDigest });
    } catch (error) {
      goalStore.transition(settled.goal_id, "blocked", { expected_revision: settled.revision, blockers: [`settlement:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-settlement-review"), outcome_digest: outcomeDigest });
      throw error;
    }
    return { schema: AUTONOMOUS_GOAL_STEP_SCHEMA, goal: updated, result, result_status: resultStatus, goal_status: updated.status, outcome_digest: outcomeDigest, evaluator_digest: updated.evaluator_digest, learning_state_digest: updated.learning_state_digest, progress_digest: updated.progress_digest, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  }

  /**
   * Run a durable single-domain goal through evaluator-guided online learning and bounded
   * replanning. The learning controller receives the evaluator packet; the goal ledger receives
   * only digests of the resulting value projection and stable cycle identity.
   */
  async runGoalLearningStep(
    goalStore: InMemoryAutonomousGoalLedger,
    goalId: string,
    task: string,
    domain: AutonomousDomainName,
    options: {
      evaluate: AutonomousReplanCycleOptions["evaluate"];
      learning?: AutonomousReplanCycleOptions["learning"];
      maxReplans?: number;
      cycleId?: string;
      goalCriteria?: readonly AutonomousGoalCriterion[];
      goalMaxAttempts?: number;
      goalCapability?: string | null;
      goalRiskClass?: string | null;
      criterionUpdates?: readonly JsonObject[];
      settlementMetadata?: AutonomousGoalSettlementMetadata;
      runOptions?: Omit<AutonomousReplanCycleOptions, "evaluate" | "learning" | "maxReplans" | "cycleId">;
    },
  ): Promise<AutonomousGoalLearningStepResult> {
    if (!(goalStore instanceof InMemoryAutonomousGoalLedger)) throw new ArgumentError("goalStore must be an InMemoryAutonomousGoalLedger");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("goal domain is unsupported");
    if (!options || typeof options.evaluate !== "function") throw new ArgumentError("goal learning requires an evaluator callback");
    const taskText = boundedText("goal learning task", task, 32_000);
    const runOptions = options.runOptions ?? {};
    const settlementMetadata = options.settlementMetadata ?? {};
    for (const [name, value] of Object.entries(settlementMetadata)) {
      if (!["evaluator_digest", "learning_state_digest", "progress_digest"].includes(name)) throw new ArgumentError(`unsupported goal settlement metadata: ${name}`);
      if (value !== null && value !== undefined && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`goal settlement ${name} must be a digest or null`);
    }
    if (options.cycleId !== undefined && (typeof options.cycleId !== "string" || !options.cycleId.trim() || options.cycleId.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(options.cycleId))) throw new ArgumentError("goal learning cycleId must be a bounded identifier");
    const taskDigest = goalTaskDigest(taskText);
    const requestedCapability = options.goalCapability ?? runOptions.capability ?? null;
    const requestedRiskClass = options.goalRiskClass ?? null;
    let current = goalStore.get(goalId);
    if (current === null) {
      current = goalStore.create({ goal_id: goalId, task_digest: taskDigest, domain, capability: requestedCapability, risk_class: requestedRiskClass, criteria: options.goalCriteria ?? [], max_attempts: options.goalMaxAttempts ?? 8 });
    } else {
      if (current.task_digest !== taskDigest || current.domain !== domain) throw new ArgumentError("goal identity does not match the requested task or domain");
      if (requestedCapability !== null && current.capability !== requestedCapability) throw new ArgumentError("goal capability does not match the requested capability");
      if (requestedRiskClass !== null && current.risk_class !== requestedRiskClass) throw new ArgumentError("goal risk class does not match the requested risk class");
    }
    if (current.status === "completed" || current.status === "cancelled") throw new ArgumentError("goal is already terminal");
    if (current.status === "blocked" || current.status === "failed") current = goalStore.transition(current.goal_id, "ready", { expected_revision: current.revision });
    const running = goalStore.transition(current.goal_id, "running", { expected_revision: current.revision });
    let cycle: AutonomousReplanCycleResult;
    try {
      cycle = await runAutonomousReplanCycle(this, taskText, {
        ...runOptions,
        domain,
        evaluate: options.evaluate,
        learning: options.learning,
        maxReplans: options.maxReplans ?? 0,
        ...(options.cycleId === undefined ? {} : { cycleId: options.cycleId }),
      });
    } catch (error) {
      goalStore.transition(running.goal_id, "failed", { expected_revision: running.revision, blockers: [`exception:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-retry"), outcome_digest: await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: "learning_cycle_exception" }) });
      throw error;
    }
    const finalAttempt = cycle.attempts[cycle.attempts.length - 1] ?? null;
    const result = cycle.final?.run ?? null;
    const resultStatus = cycle.status;
    const outcomeDigest = await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: resultStatus, cycle_outcome_digest: finalAttempt?.outcome_digest ?? null });
    const generatedEvaluatorDigest = await digestJson({ evaluations: cycle.evaluations });
    const generatedLearningDigest = await digestJson({ learning_episode_ids: cycle.learning_episode_ids, settlements: await Promise.all(cycle.settlements.map((settlement) => goalLearningSettlementProjection(settlement))) });
    const generatedProgressDigest = await digestJson({ cycle_id: options.cycleId ?? null, replan_count: cycle.replan_count, attempts: cycle.attempts.map((attempt) => ({ attempt: attempt.attempt, status: attempt.status, outcome_digest: attempt.outcome_digest, evaluation_digest: attempt.evaluation_digest })) });
    let settled = running;
    let updated: AutonomousGoalRecord;
    try {
      if (options.criterionUpdates && options.criterionUpdates.length) settled = goalStore.updateCriteria(running.goal_id, options.criterionUpdates, { expected_revision: running.revision });
      const evaluatorDigest = settlementMetadata.evaluator_digest ?? generatedEvaluatorDigest;
      const learningDigest = settlementMetadata.learning_state_digest ?? generatedLearningDigest;
      const progressDigest = settlementMetadata.progress_digest ?? generatedProgressDigest;
      const goalStatus = goalStatusForResult(resultStatus, settled.criteria.every((criterion) => !criterion.required || criterion.status === "satisfied" || criterion.status === "waived"));
      updated = goalStore.transition(settled.goal_id, goalStatus, { expected_revision: settled.revision, blockers: goalStatus === "completed" ? [] : [`result:${resultStatus}`], next_action_digest: goalStatus === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`), outcome_digest: outcomeDigest, evaluator_digest: evaluatorDigest, learning_state_digest: learningDigest, progress_digest: progressDigest });
    } catch (error) {
      goalStore.transition(settled.goal_id, "blocked", { expected_revision: settled.revision, blockers: [`settlement:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-settlement-review"), outcome_digest: outcomeDigest });
      throw error;
    }
    return { schema: AUTONOMOUS_GOAL_LEARNING_SCHEMA, goal: updated, result, result_status: resultStatus, goal_status: updated.status, outcome_digest: outcomeDigest, evaluator_digest: updated.evaluator_digest, learning_state_digest: updated.learning_state_digest, progress_digest: updated.progress_digest, learning_mode: "single_domain_replan", cycle, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  }

  /** Run a durable cross-domain goal through evaluator-guided fan-out/fan-in replanning. */
  async runCrossDomainGoalLearningStep(
    goalStore: InMemoryAutonomousGoalLedger,
    goalId: string,
    task: string,
    options: {
      evaluate: AutonomousCrossDomainReplanCycleOptions["evaluate"];
      learning?: AutonomousCrossDomainReplanCycleOptions["learning"];
      maxReplans?: number;
      cycleId?: string;
      goalCriteria?: readonly AutonomousGoalCriterion[];
      goalMaxAttempts?: number;
      goalCapability?: string | null;
      goalRiskClass?: string | null;
      criterionUpdates?: readonly JsonObject[];
      settlementMetadata?: AutonomousGoalSettlementMetadata;
      runOptions?: Omit<AutonomousCrossDomainReplanCycleOptions, "evaluate" | "learning" | "maxReplans" | "cycleId">;
    },
  ): Promise<AutonomousGoalLearningStepResult> {
    if (!(goalStore instanceof InMemoryAutonomousGoalLedger)) throw new ArgumentError("goalStore must be an InMemoryAutonomousGoalLedger");
    if (!options || typeof options.evaluate !== "function") throw new ArgumentError("cross-domain goal learning requires an evaluator callback");
    const taskText = boundedText("cross-domain goal learning task", task, 32_000);
    const runOptions = options.runOptions ?? {};
    const settlementMetadata = options.settlementMetadata ?? {};
    for (const [name, value] of Object.entries(settlementMetadata)) {
      if (!["evaluator_digest", "learning_state_digest", "progress_digest"].includes(name)) throw new ArgumentError(`unsupported goal settlement metadata: ${name}`);
      if (value !== null && value !== undefined && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`goal settlement ${name} must be a digest or null`);
    }
    if (options.cycleId !== undefined && (typeof options.cycleId !== "string" || !options.cycleId.trim() || options.cycleId.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(options.cycleId))) throw new ArgumentError("cross-domain goal learning cycleId must be a bounded identifier");
    const taskDigest = goalTaskDigest(taskText);
    const requestedCapability = options.goalCapability ?? runOptions.capability ?? null;
    const requestedRiskClass = options.goalRiskClass ?? null;
    let current = goalStore.get(goalId);
    if (current === null) current = goalStore.create({ goal_id: goalId, task_digest: taskDigest, domain: "cross_domain", capability: requestedCapability, risk_class: requestedRiskClass, criteria: options.goalCriteria ?? [], max_attempts: options.goalMaxAttempts ?? 8 });
    else {
      if (current.task_digest !== taskDigest || current.domain !== "cross_domain") throw new ArgumentError("cross-domain goal identity does not match the requested task");
      if (requestedCapability !== null && current.capability !== requestedCapability) throw new ArgumentError("goal capability does not match the requested capability");
      if (requestedRiskClass !== null && current.risk_class !== requestedRiskClass) throw new ArgumentError("goal capability does not match the requested risk class");
    }
    if (current.status === "completed" || current.status === "cancelled") throw new ArgumentError("goal is already terminal");
    if (current.status === "blocked" || current.status === "failed") current = goalStore.transition(current.goal_id, "ready", { expected_revision: current.revision });
    const running = goalStore.transition(current.goal_id, "running", { expected_revision: current.revision });
    let cycle: AutonomousCrossDomainReplanCycleResult;
    try {
      cycle = await runAutonomousCrossDomainReplanCycle(this, taskText, {
        ...runOptions,
        evaluate: options.evaluate,
        learning: options.learning,
        maxReplans: options.maxReplans ?? 0,
        ...(options.cycleId === undefined ? {} : { cycleId: options.cycleId }),
      });
    } catch (error) {
      goalStore.transition(running.goal_id, "failed", { expected_revision: running.revision, blockers: [`exception:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-retry"), outcome_digest: await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: "learning_cycle_exception" }) });
      throw error;
    }
    const finalAttempt = cycle.attempts[cycle.attempts.length - 1] ?? null;
    const result = cycle.final?.run ?? null;
    const resultStatus = cycle.status;
    const outcomeDigest = await digestJson({ goal_id: running.goal_id, attempt: running.attempt, result_status: resultStatus, cycle_outcome_digest: finalAttempt?.outcome_digest ?? null });
    const generatedEvaluatorDigest = await digestJson({ evaluations: cycle.evaluations });
    const generatedLearningDigest = await digestJson({ learning_episode_ids: cycle.learning_episode_ids, settlements: await Promise.all(cycle.settlements.map((settlement) => goalLearningSettlementProjection(settlement))) });
    const generatedProgressDigest = await digestJson({ cycle_id: options.cycleId ?? null, replan_count: cycle.replan_count, attempts: cycle.attempts.map((attempt) => ({ attempt: attempt.attempt, status: attempt.status, outcome_digest: attempt.outcome_digest, evaluation_digest: attempt.evaluation_digest, trajectory_id: attempt.trajectory_id })) });
    let settled = running;
    let updated: AutonomousGoalRecord;
    try {
      if (options.criterionUpdates && options.criterionUpdates.length) settled = goalStore.updateCriteria(running.goal_id, options.criterionUpdates, { expected_revision: running.revision });
      const evaluatorDigest = settlementMetadata.evaluator_digest ?? generatedEvaluatorDigest;
      const learningDigest = settlementMetadata.learning_state_digest ?? generatedLearningDigest;
      const progressDigest = settlementMetadata.progress_digest ?? generatedProgressDigest;
      const goalStatus = goalStatusForResult(resultStatus, settled.criteria.every((criterion) => !criterion.required || criterion.status === "satisfied" || criterion.status === "waived"));
      updated = goalStore.transition(settled.goal_id, goalStatus, { expected_revision: settled.revision, blockers: goalStatus === "completed" ? [] : [`result:${resultStatus}`], next_action_digest: goalStatus === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`), outcome_digest: outcomeDigest, evaluator_digest: evaluatorDigest, learning_state_digest: learningDigest, progress_digest: progressDigest });
    } catch (error) {
      goalStore.transition(settled.goal_id, "blocked", { expected_revision: settled.revision, blockers: [`settlement:${error instanceof Error ? error.constructor.name : "UnknownError"}`], next_action_digest: goalTaskDigest("goal-settlement-review"), outcome_digest: outcomeDigest });
      throw error;
    }
    return { schema: AUTONOMOUS_GOAL_LEARNING_SCHEMA, goal: updated, result, result_status: resultStatus, goal_status: updated.status, outcome_digest: outcomeDigest, evaluator_digest: updated.evaluator_digest, learning_state_digest: updated.learning_state_digest, progress_digest: updated.progress_digest, learning_mode: "cross_domain_replan", cycle, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  }

  /**
   * Execute one run while retaining a caller-owned, metadata-only hash-chained trace.
   *
   * The trace observer is composed with any caller observer and is propagated through
   * cross-domain children and synthesis. It records provider lifecycle metadata only; the
   * task, prompt, response, credentials, tool arguments, and transient evidence stay local.
   */
  async runWithTrace(task: string, options: AutonomousRunWithTraceOptions): Promise<AutonomousTracedRunResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous runWithTrace options must be an object");
    if (!options.traceStore || typeof options.traceStore.append !== "function" || typeof options.traceStore.events !== "function") throw new ArgumentError("autonomous runWithTrace requires a trace store");
    const taskText = boundedText("autonomous traced task", task, 32_000);
    const taskDigest = await digestJson({ task: taskText });
    const runOptions = options.run ?? {};
    const initialDomains = [runOptions.domain ?? "cross_domain"] as AutonomousDomainName[];
    const trace = new AutonomousRunTraceSession(options.traceStore, { run_id: options.runId, task_digest: taskDigest, domains: initialDomains, authorizationContext: runOptions.authorizationContext });
    await trace.started();
    try {
      const result = await this.run(taskText, { ...runOptions, observer: composeInvocationObservers(runOptions.observer, trace.providerObserver()), selectionEventCallback: trace.selectionEventCallback(runOptions.selectionEventCallback) });
      const routeDomains = result.route.selected_domains.length ? result.route.selected_domains : result.route.primary_domain ? [result.route.primary_domain] : initialDomains;
      const domains = [...new Set([...routeDomains, ...(result.route.cross_domain ? ["cross_domain" as const] : [])])] as AutonomousDomainName[];
      const planDigest = result.cross_domain?.blueprint?.plan_digest ?? result.blueprint?.plan.plan_digest ?? null;
      const selectionDigest = result.selection ? await digestJson(result.selection) : null;
      await trace.complete({ status: autonomousRunTraceStatus(result.status), domains, route_digest: result.route.route_digest, plan_digest: planDigest, selection_digest: selectionDigest });
      return { result, trace: await trace.summary() };
    } catch (error) {
      const failureClass = error instanceof Error ? error.constructor.name : "UnknownError";
      const failureCode = error instanceof ProviderRuntimeError ? error.code : error instanceof ArgumentError ? "argument_error" : "runtime_error";
      await trace.fail({ failure_class: failureClass, failure_code: failureCode, detail_digest: digestJsonSync({ failure_class: failureClass, failure_code: failureCode }) }).catch(() => undefined);
      throw error;
    }
  }

  /** Analyze a verified trace without invoking providers or retaining transient values. */
  analyzeRunTrace(snapshot: unknown, options: { policy?: Partial<AutonomousRunTraceAnalyticsPolicy> } = {}): AutonomousRunTraceAnalyticsReport {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous analyzeRunTrace options must be an object");
    return analyzeAutonomousRunTrace(snapshot, options);
  }

  /** Create bounded restart-safe storage for already-verified trace analytics reports. */
  createRunAnalyticsLedger(options: { policy?: Partial<AutonomousRunAnalyticsLedgerPolicy>; clock?: () => number; authorizationContext?: AutonomousAuthorizationContext } = {}): AutonomousRunAnalyticsLedger {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous createRunAnalyticsLedger options must be an object");
    return new AutonomousRunAnalyticsLedger(options);
  }

  /** Cross-domain variant of runWithTrace; the same trace contains specialist and synthesis turns. */
  async runCrossDomainWithTrace(task: string, options: AutonomousRunWithTraceOptions): Promise<AutonomousTracedCrossDomainRunResult> {
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous runCrossDomainWithTrace options must be an object");
    if (!options.traceStore || typeof options.traceStore.append !== "function" || typeof options.traceStore.events !== "function") throw new ArgumentError("autonomous runCrossDomainWithTrace requires a trace store");
    const taskText = boundedText("autonomous traced cross-domain task", task, 32_000);
    const taskDigest = await digestJson({ task: taskText });
    const runOptions = options.run ?? {};
    const trace = new AutonomousRunTraceSession(options.traceStore, { run_id: options.runId, task_digest: taskDigest, domains: ["cross_domain"], authorizationContext: runOptions.authorizationContext });
    await trace.started();
    try {
      const result = await this.runCrossDomain(taskText, { ...runOptions, observer: composeInvocationObservers(runOptions.observer, trace.providerObserver()), selectionEventCallback: trace.selectionEventCallback(runOptions.selectionEventCallback) });
      const domains = [...new Set([...result.route.selected_domains, "cross_domain"])] as AutonomousDomainName[];
      const planDigest = result.blueprint?.plan_digest ?? null;
      const selectionDigest = result.synthesis?.selection ? await digestJson(result.synthesis.selection) : null;
      await trace.complete({ status: autonomousRunTraceStatus(result.status), domains, route_digest: result.route.route_digest, plan_digest: planDigest, selection_digest: selectionDigest });
      return { result, trace: await trace.summary() };
    } catch (error) {
      const failureClass = error instanceof Error ? error.constructor.name : "UnknownError";
      const failureCode = error instanceof ProviderRuntimeError ? error.code : error instanceof ArgumentError ? "argument_error" : "runtime_error";
      await trace.fail({ failure_class: failureClass, failure_code: failureCode, detail_digest: digestJsonSync({ failure_class: failureClass, failure_code: failureCode }) }).catch(() => undefined);
      throw error;
    }
  }

  private withPromptLearningOptions<T extends { promptRegistry?: AutonomousPromptRegistry; promptLearningState?: AutonomousPromptLearningState | AutonomousPromptLearningStateJSON }>(options: T): T {
    const resolved = { ...options } as T & { memoryConsolidator?: AutonomousMemoryConsolidator };
    if (resolved.memoryConsolidator === undefined && this.memoryConsolidator !== undefined) {
      resolved.memoryConsolidator = this.memoryConsolidator;
    }
    const coordinator = this.promptLearningCoordinator;
    if (coordinator === undefined) return resolved as T;
    if (resolved.promptLearningState !== undefined && resolved.promptLearningState !== coordinator.state) throw new ArgumentError("promptLearningState cannot override the agent's persistent prompt learner");
    if (resolved.promptRegistry !== undefined && resolved.promptRegistry !== coordinator.registry) throw new ArgumentError("promptRegistry must be the same registry as the agent's prompt learner");
    return { ...resolved, promptRegistry: coordinator.registry, promptLearningState: coordinator.state } as T;
  }

  /** Recover exact registry-bound prompt choices from a direct, cross-domain, or workflow result. */
  promptLearningSelections(result: unknown): readonly AutonomousPromptAdaptiveSelection[] {
    const coordinator = this.promptLearningCoordinator;
    if (coordinator === undefined) throw new ArgumentError("prompt learning coordinator is not configured");
    return extractAutonomousPromptLearningSelections(result, coordinator.registry);
  }

  /** Apply explicit evaluator credit to one high-level prompt choice; provider output is never reward. */
  async settlePromptLearning(
    selection: AutonomousPromptAdaptiveSelection,
    options: { armId: string; evaluatorId: string; evaluatorVersion: string; reward: number; passed: boolean; outcomeDigest?: string; settlementKey?: string },
  ): Promise<unknown> {
    if (this.promptLearningCoordinator === undefined) throw new ArgumentError("prompt learning coordinator is not configured");
    return this.promptLearningCoordinator.settle(selection, options);
  }

  async restorePromptLearning(): Promise<unknown> {
    if (this.promptLearningCoordinator === undefined) throw new ArgumentError("prompt learning coordinator is not configured");
    return this.promptLearningCoordinator.restore();
  }

  async flushPromptLearning(): Promise<unknown> {
    if (this.promptLearningCoordinator === undefined) throw new ArgumentError("prompt learning coordinator is not configured");
    return this.promptLearningCoordinator.flush();
  }

  /**
   * Open a bounded cross-domain stream. Specialist streams are multiplexed into one transient
   * event channel, then their bounded local text is handed to a final synthesis stream. Nothing
   * in the fan-in queue or synthesis context is written to autonomy state.
   */
  async runCrossDomainStream(task: string, options: AutonomousCrossDomainRunOptions = {}): Promise<AutonomousRunStreamHandle> {
    options = this.withPromptLearningOptions(options);
    const taskText = boundedText("cross-domain stream task", task, 32_000);
    validateAutonomousStructuredOutputOptions(options);
    const contentParts = options.contentParts === undefined ? undefined : normalizeProviderContentParts(options.contentParts);
    const costBudget = resolveAutonomousCostBudget(options);
    const routeResolution = await this.resolveExecutionRoute(taskText, options, costBudget);
    const route = routeResolution.route;
    const emptyHandle = async (status: AutonomousRunStreamCompletion["status"], blueprint: AutonomousTaskBlueprint | AutonomousCrossDomainBlueprint | null = null): Promise<AutonomousRunStreamHandle> => {
      let consumed = false;
      const events: AsyncIterable<AutonomousRunStreamEvent> = {
        [Symbol.asyncIterator]: async function* (): AsyncGenerator<AutonomousRunStreamEvent> {
          if (consumed) throw new ArgumentError("autonomous stream handles are single-consumer");
          consumed = true;
        },
      };
      return {
        schema: AUTONOMOUS_RUN_STREAM_SCHEMA,
        route,
        semantic_route: routeResolution.semanticRoute,
        blueprint,
        selection: null,
        continuation_plan: null,
        context_budget: null,
        events,
        completion: Promise.resolve({
          schema: AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
          status,
          route_digest: route.route_digest,
          task_digest: route.task_digest,
          blueprint_digest: blueprint === null ? null : await digestJson(blueprint),
          event_count: 0,
          text_delta_bytes: 0,
          stage_count: 0,
          provider_invocations: [],
          provider_failover: null,
          inner_completions: [],
          effect_ids: [],
          error_code: null,
          error_class: null,
          retention: "metadata_only_no_stream_payloads_or_credentials",
          secret_material: "never_returned",
        }),
      };
    };
    if (routeResolution.semanticRoute !== null && routeResolution.semanticRoute.status !== "completed") return emptyHandle(semanticRouteCrossDomainStatus(routeResolution.semanticRoute.status));
    if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) return emptyHandle("route_review_required");

    // Cross-domain `run()` provides the canonical no-provider preflight, including child/synthesis
    // blueprints and strict policy admissions. Streaming starts only after that gate succeeds.
    const preflight = await this.runCrossDomain(taskText, {
      ...options,
      routeOverride: route,
      contentParts,
      approveProviderCall: false,
      recordMemory: false,
      learning: undefined,
      maxTotalCostUnits: undefined,
      costBudget,
    });
    if (options.approveProviderCall !== true || !preflight.blueprint || preflight.status !== "approval_required") return emptyHandle(preflight.status, preflight.blueprint);
    const blueprint = preflight.blueprint;
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("cross-domain stream requires at least one registered model candidate");

    type QueueItem = { event?: AutonomousRunStreamEvent; done?: boolean; error?: unknown };
    const queued: AutonomousRunStreamEvent[] = [];
    const waiters: Array<(item: QueueItem) => void> = [];
    const capacityWaiters: Array<() => void> = [];
    let queueClosed = false;
    let queueError: unknown = null;
    const wakeCapacityWaiter = (): void => {
      if (queued.length >= AUTONOMOUS_RUN_STREAM_MAX_QUEUED_EVENTS) return;
      capacityWaiters.shift()?.();
    };
    const push = async (event: AutonomousRunStreamEvent): Promise<void> => {
      // Backpressure is part of the stream contract.  A provider is allowed to be faster than
      // its consumer; that must suspend the producer at the bounded queue rather than inventing
      // an invalid-response failure or dropping a caller-visible delta.
      while (!queueClosed && queued.length >= AUTONOMOUS_RUN_STREAM_MAX_QUEUED_EVENTS && waiters.length === 0) {
        await new Promise<void>((resolve) => capacityWaiters.push(resolve));
      }
      if (queueClosed) return;
      const waiter = waiters.shift();
      if (waiter) waiter({ event });
      else queued.push(event);
    };
    const close = (error: unknown = null): void => {
      if (queueClosed) return;
      queueClosed = true;
      queueError = error;
      while (waiters.length) waiters.shift()!({ done: true, error });
      while (capacityWaiters.length) capacityWaiters.shift()!();
    };
    const next = async (): Promise<QueueItem> => {
      const event = queued.shift();
      if (event !== undefined) wakeCapacityWaiter();
      if (event !== undefined) return { event };
      if (queueClosed) return { done: true, error: queueError };
      return new Promise<QueueItem>((resolve) => waiters.push(resolve));
    };
    const abortController = new AbortController();
    if (options.signal?.aborted) abortController.abort(options.signal.reason);
    const abortListener = (): void => abortController.abort(options.signal?.reason);
    options.signal?.addEventListener("abort", abortListener, { once: true });
    const childOutputsByIndex: Array<{ id: string; domain: AutonomousDomainName; status: string; output: string } | undefined> = new Array(blueprint.child_blueprints.length);
    const innerCompletions: Array<AutonomousStreamCompletion | AutonomousRunStreamCompletion> = [];
    const providerInvocations: AutonomousProviderInvocationReceipt[] = [];
    let eventCount = 0;
    let textDeltaBytes = 0;
    let completionResolver: ((completion: AutonomousRunStreamCompletion) => void) | undefined;
    let completionSettled = false;
    const settle = async (status: AutonomousRunStreamCompletion["status"], error: unknown = null): Promise<void> => {
      if (completionSettled) return;
      completionSettled = true;
      const providerFailover = [...innerCompletions].reverse().find((item) => item.provider_failover !== null)?.provider_failover ?? null;
      const effectIds = [...new Set(innerCompletions.flatMap((item) => "effect_ids" in item && Array.isArray(item.effect_ids) ? item.effect_ids : []))];
      completionResolver?.({
        schema: AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
        status,
        route_digest: route.route_digest,
        task_digest: route.task_digest,
        blueprint_digest: await digestJson(blueprint),
        event_count: eventCount,
        text_delta_bytes: textDeltaBytes,
        stage_count: innerCompletions.length,
        provider_invocations: [...providerInvocations],
        provider_failover: providerFailover,
        inner_completions: [...innerCompletions],
        effect_ids: effectIds,
        error_code: error instanceof ProviderRuntimeError ? error.code : null,
        error_class: error instanceof Error ? error.constructor.name : error === null ? null : "UnknownError",
        retention: "metadata_only_no_stream_payloads_or_credentials",
        secret_material: "never_returned",
      });
    };
    let started = false;
    const start = (): void => {
      if (started) return;
      started = true;
      void (async (): Promise<void> => {
        try {
          const executionOrder = blueprint.child_blueprints.map((_, index) => index);
          const maxParallelChildren = normalizedCrossDomainConcurrency(options.maxParallelChildren, executionOrder.length);
          let nextIndex = 0;
          let stopDispatch = false;
          const executeChild = async (index: number): Promise<void> => {
            const child = blueprint.child_blueprints[index];
            if (!child) throw new ProviderRuntimeError(`cross-domain stream child ${index + 1} is missing`);
            const childId = blueprint.child_ids[index] ?? `child-${index + 1}`;
            const taskMessage = child.prompt.messages.find((message) => message.source_id === "task");
            if (!taskMessage) throw new ProviderRuntimeError(`cross-domain stream child ${childId} has no bounded task message`);
            const childContext: AutonomousPromptChunk[] = [
              ...(options.context ?? []),
              { id: "cross-domain-parent", content: `Parent route digest: ${route.route_digest}; child id: ${childId}`, required: true, priority: 100 },
            ];
            let handle: AutonomousRunStreamHandle | null = null;
            try {
              handle = await this.runStream(taskMessage.content, {
                ...options,
                domain: child.domain_profile.domain,
                routeOverride: undefined,
                semanticRouting: false,
                allowCrossDomain: false,
                context: childContext,
                contentParts,
                retrieveMemory: false,
                recordMemory: false,
                learning: undefined,
                approveProviderCall: true,
                signal: abortController.signal,
                maxTotalCostUnits: undefined,
                costBudget,
              });
              await push({ kind: "lifecycle", stage: "child", phase: "child_started", child_id: childId, domain: child.domain_profile.domain, selection_digest: handle.selection ? await digestJson(handle.selection) : null });
              let output = "";
              for await (const event of handle.events) {
                if (event.kind === "provider") {
                  output += event.event.textDelta;
                  if (bytes(output) > 48_000) output = `${output.slice(0, 47_000)}\n[child output bounded locally]`;
                  eventCount += 1;
                  textDeltaBytes += bytes(event.event.textDelta);
                  await push({ kind: "provider", stage: "child", child_id: childId, event: event.event });
                }
              }
              const childCompletion = await handle.completion;
              innerCompletions.push(childCompletion);
              providerInvocations.push(...childCompletion.provider_invocations);
              const completed = childCompletion.status === "completed";
              childOutputsByIndex[index] = { id: childId, domain: child.domain_profile.domain, status: childCompletion.status, output: output.trim() || "[child returned no textual output]" };
              await push({ kind: "lifecycle", stage: "child", phase: "child_completed", child_id: childId, domain: child.domain_profile.domain, status: childCompletion.status, event_count: childCompletion.event_count, text_delta_bytes: childCompletion.text_delta_bytes });
              if (!completed && !options.allowPartial) stopDispatch = true;
            } catch (error) {
              if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
              const childCompletion = handle === null ? null : await handle.completion;
              if (childCompletion) {
                innerCompletions.push(childCompletion);
                providerInvocations.push(...childCompletion.provider_invocations);
              }
              childOutputsByIndex[index] = { id: childId, domain: child.domain_profile.domain, status: "failed", output: "[child stream failed]" };
              await push({ kind: "lifecycle", stage: "child", phase: "child_completed", child_id: childId, domain: child.domain_profile.domain, status: "failed" });
              stopDispatch = true;
              if (!options.allowPartial) return;
            }
          };
          const worker = async (): Promise<void> => {
            while (true) {
              if (stopDispatch && !options.allowPartial) return;
              const sequenceIndex = nextIndex++;
              const index = executionOrder[sequenceIndex];
              if (index === undefined) return;
              await executeChild(index);
            }
          };
          await Promise.all(Array.from({ length: maxParallelChildren }, () => worker()));
          const childOutputs = executionOrder.flatMap((index) => childOutputsByIndex[index] ? [childOutputsByIndex[index]!] : []);
          const completedChildren = childOutputs.filter((child) => child.status === "completed").length;
          if (completedChildren === 0 || options.synthesize === false) {
            await settle(completedChildren === executionOrder.length ? "completed" : "child_failed");
            close();
            return;
          }
          const synthesisTaskMessage = blueprint.synthesis_blueprint.prompt.messages.find((message) => message.source_id === "task");
          if (!synthesisTaskMessage) throw new ProviderRuntimeError("cross-domain stream synthesis has no bounded task message");
          const synthesisContext: AutonomousPromptChunk[] = [
            ...(options.context ?? []),
            { id: "cross-domain-parent", content: `Parent route digest: ${route.route_digest}`, required: true, priority: 100 },
            ...childOutputs.filter((child) => child.status === "completed").map((child) => ({ id: `cross-domain-output-${child.id}`, content: JSON.stringify(child), priority: 90 })),
          ];
          const synthesis = await this.runStream(synthesisTaskMessage.content, {
            ...options,
            domain: "cross_domain",
            routeOverride: undefined,
            semanticRouting: false,
            allowCrossDomain: false,
            context: synthesisContext,
            contentParts,
            retrieveMemory: false,
            recordMemory: false,
            learning: undefined,
            approveProviderCall: true,
            signal: abortController.signal,
            maxTotalCostUnits: undefined,
            costBudget,
          });
            await push({ kind: "lifecycle", stage: "synthesis", phase: "synthesis_started", domain: "cross_domain", selection_digest: synthesis.selection ? await digestJson(synthesis.selection) : null });
          for await (const event of synthesis.events) {
            if (event.kind !== "provider") continue;
            eventCount += 1;
            textDeltaBytes += bytes(event.event.textDelta);
            await push({ kind: "provider", stage: "synthesis", event: event.event });
          }
          const synthesisCompletion = await synthesis.completion;
          innerCompletions.push(synthesisCompletion);
          providerInvocations.push(...synthesisCompletion.provider_invocations);
          await push({ kind: "lifecycle", stage: "synthesis", phase: "synthesis_completed", domain: "cross_domain", status: synthesisCompletion.status, event_count: synthesisCompletion.event_count, text_delta_bytes: synthesisCompletion.text_delta_bytes });
          await settle(synthesisCompletion.status === "completed" ? "completed" : synthesisCompletion.status === "abandoned" ? "abandoned" : "failed");
          close();
        } catch (error) {
          await settle("failed", error);
          close(error);
        } finally {
          options.signal?.removeEventListener("abort", abortListener);
        }
      })();
    };
    const completion = new Promise<AutonomousRunStreamCompletion>((resolve) => { completionResolver = resolve; });
    let consumed = false;
    const events: AsyncIterable<AutonomousRunStreamEvent> = {
      [Symbol.asyncIterator]: async function* (): AsyncGenerator<AutonomousRunStreamEvent> {
        if (consumed) throw new ArgumentError("autonomous stream handles are single-consumer");
        consumed = true;
        start();
        try {
          while (true) {
            const item = await next();
            if (item.error) throw item.error;
            if (item.done) return;
            if (item.event) yield item.event;
          }
        } finally {
          if (!queueClosed) {
            abortController.abort();
            await settle("abandoned");
            close();
          }
        }
      },
    };
    return { schema: AUTONOMOUS_RUN_STREAM_SCHEMA, route, semantic_route: routeResolution.semanticRoute, blueprint, selection: null, continuation_plan: null, context_budget: null, events, completion };
  }

  /** Automatic deterministic mode with the same live stream contract as `run()`. */
  async runAutoStream(task: string, options: AutonomousAutoRunOptions = {}): Promise<AutonomousRunStreamHandle> {
    if (options.planningMode === "provider" || options.planning !== undefined || options.acceptPlan === true) {
      throw new ArgumentError("provider-planned automatic streaming requires an explicit plan acceptance flow; use runAuto() before opening a stream");
    }
    return this.runStream(task, options);
  }

  /** Open the provider-neutral stream after the same route/blueprint/policy gates as `run()`. */
  async runStream(task: string, options: AutonomousRunOptions = {}): Promise<AutonomousRunStreamHandle> {
    options = this.withPromptLearningOptions(options);
    const taskText = boundedText("autonomous stream task", task, 32_000);
    validateAutonomousStructuredOutputOptions(options);
    const contentParts = options.contentParts === undefined ? undefined : normalizeProviderContentParts(options.contentParts);
    const costBudget = resolveAutonomousCostBudget(options);
    const routeResolution = await this.resolveExecutionRoute(taskText, options, costBudget);
    const route = routeResolution.route;
    if (route.cross_domain && options.domain === undefined) {
      if (options.acceptedSingleDomainPlanRefinement !== undefined) throw new ArgumentError("single-domain plan refinement cannot be applied to a cross-domain stream");
      const cross = await this.runCrossDomainStream(taskText, { ...options, routeOverride: route, contentParts });
      return routeResolution.semanticRoute === null ? cross : { ...cross, semantic_route: routeResolution.semanticRoute };
    }

    const emptyHandle = async (status: AutonomousRunStreamCompletion["status"], blueprint: AutonomousTaskBlueprint | AutonomousCrossDomainBlueprint | null = null): Promise<AutonomousRunStreamHandle> => {
      let consumed = false;
      const events: AsyncIterable<AutonomousRunStreamEvent> = {
        [Symbol.asyncIterator]: async function* (): AsyncGenerator<AutonomousRunStreamEvent> {
          if (consumed) throw new ArgumentError("autonomous stream handles are single-consumer");
          consumed = true;
        },
      };
      return {
        schema: AUTONOMOUS_RUN_STREAM_SCHEMA,
        route,
        semantic_route: routeResolution.semanticRoute,
        blueprint,
        selection: null,
        continuation_plan: null,
        context_budget: null,
        events,
        completion: Promise.resolve({
          schema: AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
          status,
          route_digest: route.route_digest,
          task_digest: route.task_digest,
          blueprint_digest: blueprint === null ? null : await digestJson(blueprint),
          event_count: 0,
          text_delta_bytes: 0,
          stage_count: 0,
          provider_invocations: [],
          provider_failover: null,
          inner_completions: [],
          effect_ids: [],
          error_code: null,
          error_class: null,
          retention: "metadata_only_no_stream_payloads_or_credentials",
          secret_material: "never_returned",
        }),
      };
    };
    if (routeResolution.semanticRoute !== null && routeResolution.semanticRoute.status !== "completed") return emptyHandle(semanticRouteRunStatus(routeResolution.semanticRoute.status));
    if (route.abstained || !route.primary_domain) return emptyHandle("route_review_required");

    // `run()` is used only as a provider-free preflight so the stream cannot bypass any existing
    // blueprint, task-decision, memory, structured-output, or explicit-approval gate.
    const preflight = await this.run(taskText, {
      ...options,
      routeOverride: route,
      contentParts,
      approveProviderCall: false,
      recordMemory: false,
      learning: undefined,
      maxTotalCostUnits: undefined,
      costBudget,
    });
    if (options.approveProviderCall !== true || !preflight.blueprint || preflight.status !== "approval_required") return emptyHandle(preflight.status, preflight.blueprint);
    let blueprint = preflight.blueprint;
    if (preflight.memory?.consolidated_retrieval_digest) {
      blueprint = { ...blueprint, selection_context: { ...blueprint.selection_context, consolidated_memory_retrieval_digest: preflight.memory.consolidated_retrieval_digest } };
    }
    const acceptedPlan = await acceptedAutonomousPlan(blueprint, options.acceptedSingleDomainPlanRefinement);
    const domainPolicyMode = normalizeAutonomousDomainPolicyMode(options.domainPolicyMode);
    const domainPolicyAdmission = domainPolicyAdmissionForBlueprint(route, blueprint, options, acceptedPlan !== null);
    if (domainPolicyMode === "strict" && domainPolicyAdmission && domainPolicyAdmission.decision !== "admitted") return emptyHandle(domainPolicyStatus(domainPolicyAdmission), blueprint);
    const domainPolicy = blueprint.domain_policy;
    const streamCostBudget = costBudget ?? (domainPolicyMode === "strict" ? new AutonomousCostBudget(domainPolicy.max_total_cost_units) : undefined);
    const effectiveMaxProviderFailovers = domainPolicyMode === "strict"
      ? Math.min(options.maxProviderFailovers ?? Math.max(0, domainPolicy.max_provider_attempts - 1), Math.max(0, domainPolicy.max_provider_attempts - 1))
      : options.maxProviderFailovers;
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("autonomous stream requires at least one registered model candidate");
    const selectedDomains = route.selected_domains.length ? route.selected_domains : [route.primary_domain];
    if (options.tools && this.toolCatalogue && this.toolExecutor) await this.ensureToolRegistry();
    const defaultToolNames = blueprint.plan.allowed_tools.filter((name) => name !== "provider.invoke");
    const tools = options.tools === undefined ? await this.liveToolsForNames(selectedDomains, defaultToolNames) : this.filterActivatedTools(options.tools);
    const renderedPrompt = await renderAutonomousRunPrompt(taskText, blueprint, route, options);
    const messages: ProviderMessage[] = renderedPrompt
      ? renderedPrompt.messages.map((message) => ({ ...message }))
      : blueprint.prompt.messages.map((message) => ({ role: message.role, content: message.content }));
    if (renderedPrompt) {
      const supportingMessages = blueprint.prompt.messages
        .filter((message) => !["domain-system", "domain-developer", "task"].includes(message.source_id))
        .map((message) => ({ role: message.role, content: message.content } as ProviderMessage));
      if (supportingMessages.length) {
        const lastUserIndex = messages.reduce((found, message, index) => message.role === "user" ? index : found, -1);
        messages.splice(lastUserIndex < 0 ? messages.length : lastUserIndex, 0, ...supportingMessages);
      }
    }
    if (contentParts) {
      let taskMessageIndex = -1;
      for (let index = messages.length - 1; index >= 0; index -= 1) if (messages[index]?.role === "user") { taskMessageIndex = index; break; }
      if (taskMessageIndex < 0) throw new ProviderRuntimeError("autonomous stream prompt has no user task message for content parts");
      const taskMessage = messages[taskMessageIndex];
      if (!taskMessage || typeof taskMessage.content !== "string") throw new ProviderRuntimeError("autonomous stream task message must be text before content parts are attached");
      messages[taskMessageIndex] = { ...taskMessage, content: [providerTextPart(taskMessage.content), ...contentParts] };
    }
    if (acceptedPlan) messages.push({ role: "user", content: `Accepted provider plan refinement (digest ${acceptedPlan.refinement_digest}). Follow this existing workflow order and focus only; do not add tools, effects, permissions, credentials, or claims. Priority stages: ${acceptedPlan.priority_stage_ids.join(", ")}. Focus stages: ${acceptedPlan.focus_stage_ids.join(", ")}.` });
    const promptProjection: AutonomousRunPromptProjection | null = renderedPrompt === null ? null : {
      mode: renderedPrompt.mode,
      prompt_id: renderedPrompt.metadata.prompt_id,
      version: renderedPrompt.metadata.version,
      domain: renderedPrompt.metadata.domain,
      stage: renderedPrompt.metadata.stage,
      manifest_digest: renderedPrompt.metadata.manifest_digest,
      rendered_prompt_digest: renderedPrompt.metadata.rendered_prompt_digest,
      final_prompt_digest: await digestJson(messages),
      selection_plan_digest: renderedPrompt.metadata.selection_plan_digest,
      ...(renderedPrompt.metadata.adaptive_selection_digest !== undefined ? { adaptive_selection_digest: renderedPrompt.metadata.adaptive_selection_digest } : {}),
      ...(renderedPrompt.metadata.adaptive_arm_id !== undefined ? { adaptive_arm_id: renderedPrompt.metadata.adaptive_arm_id } : {}),
      ...(renderedPrompt.metadata.adaptive_generation !== undefined ? { adaptive_generation: renderedPrompt.metadata.adaptive_generation } : {}),
      ...(renderedPrompt.metadata.selection_policy !== undefined ? { selection_policy: renderedPrompt.metadata.selection_policy } : {}),
      ...(renderedPrompt.metadata.adaptive_selection !== undefined ? { adaptive_selection: renderedPrompt.metadata.adaptive_selection } : {}),
      retention: "prompt_messages_transient;digest_only_projection",
      secret_material: "never_returned",
    };
    const requiredCapabilities = [...blueprint.required_capabilities];
    const requireJson = options.structuredDomainResponse === true || options.requireJson === true;
    const responseSchema = options.structuredDomainResponse === true ? blueprint.response_contract?.response_schema : options.responseSchema;
    if (options.structuredDomainResponse === true && !responseSchema) throw new ProviderRuntimeError("structured domain response contract was not compiled into the stream blueprint");
    if (requireJson && !requiredCapabilities.includes("structured_output")) requiredCapabilities.push("structured_output");
    const request: ProviderRequest = {
      model: "selection-delegated",
      messages,
      maxOutputTokens: options.maxOutputTokens ?? 1_024,
      temperature: options.temperature,
      ...(promptProjection ? { idempotencyKey: await digestJson({ schema: "bioprism-typescript-autonomous-run-stream-request/0.1", task_digest: blueprint.task_digest, plan_digest: blueprint.plan.plan_digest, prompt_digest: promptProjection.final_prompt_digest, manifest_digest: promptProjection.manifest_digest, selection_plan_digest: promptProjection.selection_plan_digest, consolidated_memory_retrieval_digest: preflight.memory?.consolidated_retrieval_digest ?? null }) } : {}),
      ...(requireJson ? { requireJson: true } : options.requireJson === false ? { requireJson: false } : {}),
      ...(responseSchema !== undefined ? { responseSchema } : {}),
      tools: tools.length ? tools : undefined,
      toolChoice: tools.length ? "auto" : undefined,
    };
    const executionPlan: AutonomousExecutionPlan = {
      task: taskText,
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      riskClass: blueprint.domain_profile.risk_class,
      taskFamily: blueprint.selection_context.task_family ?? undefined,
      learningContextDigest: blueprint.learning_context_digest,
      requiredCapabilities,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      minSelectionConfidence: domainPolicyMode === "strict" ? Math.max(options.minSelectionConfidence ?? 0, domainPolicy.min_selection_confidence) : options.minSelectionConfidence,
      selectionWeights: options.selectionWeights,
      selectionObservations: options.selectionObservations,
      contextBudget: options.contextBudget,
      candidates,
      request,
    };
    const healthObserver = this.modelHealthController?.observer({ domain: blueprint.domain_profile.domain, capability: executionPlan.capability ?? blueprint.domain_profile.default_capability, riskClass: blueprint.domain_profile.risk_class });
    const remoteHealthObserver = this.modelHealthBridge?.observer({ domain: blueprint.domain_profile.domain, capability: executionPlan.capability ?? blueprint.domain_profile.default_capability, riskClass: blueprint.domain_profile.risk_class });
    const feedbackObserver = composeInvocationObservers(options.observer, healthObserver, remoteHealthObserver);
    const inner = await this.runtime.invokeStream(executionPlan, {
      credential: options.credential,
      credentialFor: options.credentialFor,
      signal: options.signal,
      observer: feedbackObserver,
      providerDispatchFence: options.providerDispatchFence,
      selectionEventCallback: options.selectionEventCallback,
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: effectiveMaxProviderFailovers,
      reserveCost: streamCostBudget ? (costUnits) => streamCostBudget.reserve(costUnits) : undefined,
      effectBoundary: options.effectBoundary ?? this.effectBoundary,
      authorizationContext: options.authorizationContext,
      authorizationDomain: blueprint.domain_profile.domain,
    });
    let completionResolver: ((completion: AutonomousRunStreamCompletion) => void) | undefined;
    const completion = new Promise<AutonomousRunStreamCompletion>((resolve) => { completionResolver = resolve; });
    let consumed = false;
    const events: AsyncIterable<AutonomousRunStreamEvent> = {
      [Symbol.asyncIterator]: async function* (): AsyncGenerator<AutonomousRunStreamEvent> {
        if (consumed) throw new ArgumentError("autonomous stream handles are single-consumer");
        consumed = true;
        let eventCount = 0;
        let textDeltaBytes = 0;
        try {
          for await (const event of inner.events) {
            eventCount += 1;
            textDeltaBytes += bytes(event.textDelta);
            yield { kind: "provider", stage: "direct", event };
          }
        } finally {
          const innerCompletion = await inner.completion;
          completionResolver?.({
            schema: AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
            status: innerCompletion.status === "completed" ? "completed" : innerCompletion.status === "abandoned" ? "abandoned" : "failed",
            route_digest: route.route_digest,
            task_digest: route.task_digest,
            blueprint_digest: await digestJson(blueprint),
            event_count: eventCount,
            text_delta_bytes: textDeltaBytes,
            stage_count: 1,
            provider_invocations: innerCompletion.provider_invocations,
            provider_failover: innerCompletion.provider_failover,
            inner_completions: [innerCompletion],
            effect_ids: [...innerCompletion.effect_ids],
            error_code: innerCompletion.error_code,
            error_class: innerCompletion.error_class,
            retention: "metadata_only_no_stream_payloads_or_credentials",
            secret_material: "never_returned",
          });
        }
      },
    };
    return { schema: AUTONOMOUS_RUN_STREAM_SCHEMA, route, semantic_route: routeResolution.semanticRoute, blueprint, selection: inner.selection, continuation_plan: inner.continuation_plan, context_budget: inner.context_budget, events, completion };
  }

  async run(task: string, options: AutonomousRunOptions = {}): Promise<AutonomousRunResult> {
    options = this.withPromptLearningOptions(options);
    const taskText = boundedText("autonomous task", task, 32_000);
    validateAutonomousStructuredOutputOptions(options);
    const domainPolicyMode = normalizeAutonomousDomainPolicyMode(options.domainPolicyMode);
    const contentParts = options.contentParts === undefined
      ? undefined
      : normalizeProviderContentParts(options.contentParts);
    let costBudget = resolveAutonomousCostBudget(options);
    const routeResolution = await this.resolveExecutionRoute(taskText, options, costBudget);
    const route = routeResolution.route;
    const semanticRoute = routeResolution.semanticRoute;
    if (semanticRoute !== null && semanticRoute.status !== "completed") {
      return {
        schema: "bioprism-typescript-autonomous-run/0.1",
        status: semanticRouteRunStatus(semanticRoute.status),
        route,
        semantic_route: semanticRoute,
        blueprint: null,
        plan_refinement_digest: null,
        selection: null,
        response: null,
        tool_loop: null,
        cross_domain: null,
        learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only",
        retention: "provider_response_local; value_only_learning_projection",
      };
    }
    if (route.cross_domain && options.domain === undefined) {
      if (options.acceptedSingleDomainPlanRefinement !== undefined) throw new ArgumentError("single-domain plan refinement cannot be applied to a cross-domain route");
      const cross = await this.runCrossDomain(taskText, { ...options, routeOverride: route, contentParts, maxTotalCostUnits: undefined, costBudget });
      return {
        schema: "bioprism-typescript-autonomous-run/0.1",
        status: cross.status === "completed" ? "completed" : cross.status === "approval_required" ? "approval_required" : cross.status === "reconciliation_required" ? "reconciliation_required" : cross.status === "turn_limit_reached" ? "turn_limit_reached" : cross.status === "child_failed" ? "child_failed" : cross.status === "children_partial" ? "cross_domain_partial" : "route_review_required",
        route,
        semantic_route: semanticRoute,
        blueprint: cross.blueprint?.synthesis_blueprint ?? null,
        plan_refinement_digest: cross.plan_refinement_digest,
        selection: cross.synthesis?.selection ?? null,
        response: cross.synthesis?.response ?? null,
        provider_invocations: [
          ...cross.child_runs.flatMap((child) => child.result.provider_invocations ?? []),
          ...(cross.synthesis?.provider_invocations ?? []),
        ],
        provider_failover: cross.synthesis?.provider_failover ?? null,
        continuation_plan: cross.synthesis?.continuation_plan ?? null,
        prompt: cross.synthesis?.prompt ?? null,
        response_evaluation: cross.synthesis?.response_evaluation ?? null,
        tool_loop: cross.synthesis?.tool_loop ?? null,
        cross_domain: cross,
        memory: cross.memory ?? null,
        learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only",
        retention: "provider_response_local; value_only_learning_projection",
      };
    }
    if (route.abstained || !route.primary_domain) return { schema: "bioprism-typescript-autonomous-run/0.1", status: "route_review_required", route, semantic_route: semanticRoute, blueprint: null, plan_refinement_digest: null, selection: null, response: null, tool_loop: null, cross_domain: null, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" };
    const memory = await this.prepareMemory(taskText, route, options, [route.primary_domain]);
    const finish = async (result: AutonomousRunResult): Promise<AutonomousRunResult> => {
      const memoryProjection = memory.store ? await this.recordMemory(taskText, route, result, options, memory) : memory.projection;
      const withMemory = memoryProjection ? { ...result, memory: memoryProjection } : result;
      const withSemanticRoute = semanticRoute === null ? withMemory : { ...withMemory, semantic_route: semanticRoute };
      if (!options.learning) return withSemanticRoute;
      return { ...withSemanticRoute, ...(await this.prepareDirectLearning(withSemanticRoute, route, { ...options, memoryEpisodeId: memoryProjection?.recorded_episode_id ?? null })) };
    };
    const blueprintEnvelope = await this.blueprint(taskText, { domain: route.primary_domain, routeOverride: route, capability: options.capability, context: [...(options.context ?? []), ...memory.context], maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name), hints: options.hints, minConfidence: options.minConfidence, minMargin: options.minMargin, maxDomains: options.maxDomains, allowCrossDomain: options.allowCrossDomain, structuredDomainResponse: options.structuredDomainResponse, toolSelectionState: options.toolSelectionState, toolSelectionExploration: options.toolSelectionExploration, maxToolRiskClass: options.maxToolRiskClass });
    let blueprint = blueprintEnvelope.blueprint;
    if (!blueprint) return finish({ schema: "bioprism-typescript-autonomous-run/0.1", status: "route_review_required", route, blueprint: null, plan_refinement_digest: null, selection: null, response: null, tool_loop: null, cross_domain: null, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" });
    if (memory.projection?.consolidated_retrieval_digest !== null && memory.projection?.consolidated_retrieval_digest !== undefined) {
      blueprint = {
        ...blueprint,
        selection_context: {
          ...blueprint.selection_context,
          consolidated_memory_retrieval_digest: memory.projection.consolidated_retrieval_digest,
        },
      };
    }
    assertAutonomousTaskDecisionAllowsProvider(blueprint.task_decision, "autonomous execution");
    const acceptedPlan = await acceptedAutonomousPlan(blueprint, options.acceptedSingleDomainPlanRefinement);
    const planRefinementDigest = acceptedPlan?.refinement_digest ?? null;
    const domainPolicyAdmission = domainPolicyAdmissionForBlueprint(route, blueprint, options, acceptedPlan !== null);
    if (domainPolicyMode === "strict" && domainPolicyAdmission && domainPolicyAdmission.decision !== "admitted") {
      return finish({ schema: "bioprism-typescript-autonomous-run/0.1", status: domainPolicyStatus(domainPolicyAdmission), route, blueprint, plan_refinement_digest: planRefinementDigest, selection: null, response: null, tool_loop: null, cross_domain: null, domain_policy_admission: domainPolicyAdmission, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" });
    }
    const domainPolicy = blueprint.domain_policy;
    const effectiveMaxProviderFailovers = domainPolicyMode === "strict"
      ? Math.min(options.maxProviderFailovers ?? Math.max(0, domainPolicy.max_provider_attempts - 1), Math.max(0, domainPolicy.max_provider_attempts - 1))
      : options.maxProviderFailovers;
    const effectiveMaxToolTurns = domainPolicyMode === "strict"
      ? Math.min(options.maxToolTurns ?? domainPolicy.max_tool_turns, domainPolicy.max_tool_turns)
      : options.maxToolTurns;
    const effectiveMinSelectionConfidence = domainPolicyMode === "strict"
      ? Math.max(options.minSelectionConfidence ?? 0, domainPolicy.min_selection_confidence)
      : options.minSelectionConfidence;
    if (domainPolicyMode === "strict" && costBudget === undefined) costBudget = new AutonomousCostBudget(domainPolicy.max_total_cost_units);
    if (options.approveProviderCall !== true) return finish({ schema: "bioprism-typescript-autonomous-run/0.1", status: "approval_required", route, blueprint, plan_refinement_digest: planRefinementDigest, selection: null, response: null, tool_loop: null, cross_domain: null, domain_policy_admission: domainPolicyAdmission, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" });
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("autonomous run requires at least one registered model candidate");
    const selectedDomains = route.selected_domains.length ? route.selected_domains : [route.primary_domain];
    if (options.tools && this.toolCatalogue && this.toolExecutor) await this.ensureToolRegistry();
    const defaultToolNames = blueprint.plan.allowed_tools.filter((name) => name !== "provider.invoke");
    const tools = options.tools === undefined ? await this.liveToolsForNames(selectedDomains, defaultToolNames) : this.filterActivatedTools(options.tools);
    const renderedPrompt = await renderAutonomousRunPrompt(taskText, blueprint, route, options);
    const messages: ProviderMessage[] = renderedPrompt
      ? renderedPrompt.messages.map((message) => ({ ...message }))
      : blueprint.prompt.messages.map((message) => ({ role: message.role, content: message.content }));
    if (renderedPrompt) {
      // The versioned renderer controls the specialist framing, while the reviewed blueprint
      // remains the source of bounded caller/memory context. Keep those context messages without
      // reintroducing the generated domain system/developer prompt or duplicate task message.
      const supportingMessages = blueprint.prompt.messages
        .filter((message) => !["domain-system", "domain-developer", "task"].includes(message.source_id))
        .map((message) => ({ role: message.role, content: message.content } as ProviderMessage));
      if (supportingMessages.length) {
        const lastUserIndex = messages.reduce((found, message, index) => message.role === "user" ? index : found, -1);
        messages.splice(lastUserIndex < 0 ? messages.length : lastUserIndex, 0, ...supportingMessages);
      }
    }
    if (contentParts) {
      let taskMessageIndex = -1;
      for (let index = messages.length - 1; index >= 0; index -= 1) {
        if (messages[index]?.role === "user") {
          taskMessageIndex = index;
          break;
        }
      }
      if (taskMessageIndex < 0) throw new ProviderRuntimeError("autonomous prompt has no user task message for content parts");
      const taskMessage = messages[taskMessageIndex];
      if (!taskMessage || typeof taskMessage.content !== "string") throw new ProviderRuntimeError("autonomous task message must be text before content parts are attached");
      messages[taskMessageIndex] = { ...taskMessage, content: [providerTextPart(taskMessage.content), ...contentParts] };
    }
    if (acceptedPlan) messages.push({ role: "user", content: `Accepted provider plan refinement (digest ${acceptedPlan.refinement_digest}). Follow this existing workflow order and focus only; do not add tools, effects, permissions, credentials, or claims. Priority stages: ${acceptedPlan.priority_stage_ids.join(", ")}. Focus stages: ${acceptedPlan.focus_stage_ids.join(", ")}.` });
    const promptProjection: AutonomousRunPromptProjection | null = renderedPrompt === null
      ? null
      : {
        mode: renderedPrompt.mode,
        prompt_id: renderedPrompt.metadata.prompt_id,
        version: renderedPrompt.metadata.version,
        domain: renderedPrompt.metadata.domain,
        stage: renderedPrompt.metadata.stage,
        manifest_digest: renderedPrompt.metadata.manifest_digest,
        rendered_prompt_digest: renderedPrompt.metadata.rendered_prompt_digest,
        final_prompt_digest: await digestJson(messages),
        selection_plan_digest: renderedPrompt.metadata.selection_plan_digest,
        ...(renderedPrompt.metadata.adaptive_selection_digest !== undefined ? { adaptive_selection_digest: renderedPrompt.metadata.adaptive_selection_digest } : {}),
        ...(renderedPrompt.metadata.adaptive_arm_id !== undefined ? { adaptive_arm_id: renderedPrompt.metadata.adaptive_arm_id } : {}),
        ...(renderedPrompt.metadata.adaptive_generation !== undefined ? { adaptive_generation: renderedPrompt.metadata.adaptive_generation } : {}),
        ...(renderedPrompt.metadata.selection_policy !== undefined ? { selection_policy: renderedPrompt.metadata.selection_policy } : {}),
        ...(renderedPrompt.metadata.adaptive_selection !== undefined ? { adaptive_selection: renderedPrompt.metadata.adaptive_selection } : {}),
        retention: "prompt_messages_transient;digest_only_projection",
        secret_material: "never_returned",
      };
    const requiredCapabilities = [...blueprint.required_capabilities];
    const requireJson = options.structuredDomainResponse === true || options.requireJson === true;
    const responseSchema = options.structuredDomainResponse === true
      ? blueprint.response_contract?.response_schema
      : options.responseSchema;
    if (options.structuredDomainResponse === true && !responseSchema) throw new ProviderRuntimeError("structured domain response contract was not compiled into the blueprint");
    if (requireJson && !requiredCapabilities.includes("structured_output")) requiredCapabilities.push("structured_output");
    const request: ProviderRequest = {
      model: "selection-delegated",
      messages,
      maxOutputTokens: options.maxOutputTokens ?? 1_024,
      temperature: options.temperature,
      ...(options.providerIdempotencyKey !== undefined
        ? { idempotencyKey: boundedIdentifier("provider idempotency key", options.providerIdempotencyKey) }
        : promptProjection ? {
        idempotencyKey: await digestJson({
          schema: "bioprism-typescript-autonomous-run-prompt-request/0.1",
          task_digest: blueprint.task_digest,
          plan_digest: blueprint.plan.plan_digest,
          prompt_digest: promptProjection.final_prompt_digest,
          manifest_digest: promptProjection.manifest_digest,
          selection_plan_digest: promptProjection.selection_plan_digest,
          consolidated_memory_retrieval_digest: memory.projection?.consolidated_retrieval_digest ?? null,
        }),
      } : {}),
      ...(requireJson ? { requireJson: true } : options.requireJson === false ? { requireJson: false } : {}),
      ...(responseSchema !== undefined ? { responseSchema } : {}),
      tools: tools.length ? tools : undefined,
      toolChoice: tools.length ? "auto" : undefined,
    };
    const executionPlan: AutonomousExecutionPlan = { task: taskText, domain: blueprint.domain_profile.domain, capability: blueprint.selection_context.capability, riskClass: blueprint.domain_profile.risk_class, taskFamily: blueprint.selection_context.task_family ?? undefined, learningContextDigest: blueprint.learning_context_digest, requiredCapabilities, maxCostPerMillionTokens: options.maxCostPerMillionTokens, maxLatencyMs: options.maxLatencyMs, minQuality: options.minQuality, minSelectionConfidence: effectiveMinSelectionConfidence, selectionWeights: options.selectionWeights, selectionObservations: options.selectionObservations, contextBudget: options.contextBudget, candidates, request };
    const healthObserver = this.modelHealthController?.observer({ domain: blueprint.domain_profile.domain, capability: executionPlan.capability ?? blueprint.domain_profile.default_capability, riskClass: blueprint.domain_profile.risk_class });
    const remoteHealthObserver = this.modelHealthBridge?.observer({ domain: blueprint.domain_profile.domain, capability: executionPlan.capability ?? blueprint.domain_profile.default_capability, riskClass: blueprint.domain_profile.risk_class });
    const feedbackObserver = composeInvocationObservers(options.observer, healthObserver, remoteHealthObserver);
    if (tools.length || options.authorizeAndExecute || this.toolRuntimeForRun()) {
      const toolRuntime = this.toolRuntimeForRun();
      const authorizeAndExecute = options.authorizeAndExecute
        ? (calls: ProviderToolCall[]) => this.dispatchActivatedToolCalls(calls, options.authorizeAndExecute!)
        : (toolRuntime
          ? (calls: ProviderToolCall[]) => this.dispatchActivatedToolCalls(calls, (allowed) => toolRuntime.authorizeAndExecute(allowed, { domains: selectedDomains, approveEffects: options.approveEffects, execution: options.execution, effectBoundary: options.effectBoundary ?? this.effectBoundary, workflowContext: options.workflowContext, authorizationContext: options.authorizationContext }))
          : async (calls: ProviderToolCall[]) => calls.map((call) => ({ callId: call.id, approved: false, isError: true, content: { status: "authorization_required", tool: call.name, secret_material: "never_returned" } })));
      const toolReadOnly = options.toolReadOnly ?? (async (call: ProviderToolCall): Promise<boolean> => this.domainToolRegistry?.binding(call.name, selectedDomains)?.risk_class === "read_only");
      const loop = await this.runtime.invokeToolLoop(executionPlan, { credential: options.credential, credentialFor: options.credentialFor, authorizeAndExecute, maxTurns: effectiveMaxToolTurns, signal: options.signal, observer: feedbackObserver, providerDispatchFence: options.providerDispatchFence, selectionEventCallback: options.selectionEventCallback, execution: options.execution, executionAttempt: options.executionAttempt, maxProviderFailovers: effectiveMaxProviderFailovers, reserveCost: costBudget ? (costUnits) => costBudget!.reserve(costUnits) : undefined, toolReadOnly, authorizationContext: options.authorizationContext });
      const responseEvaluation = options.structuredDomainResponse === true && loop.loop.finalResponse
        ? evaluateAutonomousDomainResponseOrThrow(loop.loop.finalResponse, blueprint.response_contract)
        : null;
      const status: AutonomousRunStatus = directStructuredResponseStatus(
        loop.loop.status === "completed" ? "completed" : loop.loop.status === "authorization_required" ? "approval_required" : loop.loop.status === "reconciliation_required" ? "reconciliation_required" : "turn_limit_reached",
        responseEvaluation,
        options.requireStructuredResponseReview,
      );
      return finish({ schema: "bioprism-typescript-autonomous-run/0.1", status, route, blueprint, plan_refinement_digest: planRefinementDigest, selection: loop.selection, response: loop.loop.finalResponse, provider_invocations: loop.provider_invocations, provider_failover: loop.provider_failover, continuation_plan: loop.continuation_plan, context_budget: loop.context_budget, prompt: promptProjection, response_evaluation: responseEvaluation, tool_loop: { status: loop.loop.status, turns: loop.loop.turns, toolCalls: loop.loop.toolCalls }, cross_domain: null, domain_policy_admission: domainPolicyAdmission, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" });
    }
    const result = await this.runtime.invoke(executionPlan, { credential: options.credential, credentialFor: options.credentialFor, signal: options.signal, observer: feedbackObserver, providerDispatchFence: options.providerDispatchFence, selectionEventCallback: options.selectionEventCallback, execution: options.execution, executionAttempt: options.executionAttempt, maxProviderFailovers: effectiveMaxProviderFailovers, reserveCost: costBudget ? (costUnits) => costBudget!.reserve(costUnits) : undefined, authorizationContext: options.authorizationContext });
    const responseEvaluation = options.structuredDomainResponse === true
      ? evaluateAutonomousDomainResponseOrThrow(result.response, blueprint.response_contract)
      : null;
    return finish({ schema: "bioprism-typescript-autonomous-run/0.1", status: directStructuredResponseStatus("completed", responseEvaluation, options.requireStructuredResponseReview), route, blueprint, plan_refinement_digest: planRefinementDigest, selection: result.selection, response: result.response, provider_invocations: result.provider_invocations, provider_failover: result.provider_failover, continuation_plan: result.continuation_plan, context_budget: result.context_budget, prompt: promptProjection, response_evaluation: responseEvaluation, tool_loop: null, cross_domain: null, domain_policy_admission: domainPolicyAdmission, learning: this.learner ? "online_bandit_feedback_available" : "provider_health_feedback_only", retention: "provider_response_local; value_only_learning_projection" });
  }

  private crossDomainResponseEntries(
    blueprint: AutonomousCrossDomainBlueprint,
    childRuns: readonly AutonomousCrossDomainChildRun[],
    synthesis: AutonomousRunResult | null = null,
  ): AutonomousCrossDomainResponseEntry[] | null {
    if (!blueprint.child_blueprints.some((child) => child.response_contract !== undefined) && blueprint.synthesis_blueprint.response_contract === undefined) return null;
    const entries: AutonomousCrossDomainResponseEntry[] = [];
    for (const childRun of childRuns) {
      if (childRun.result.status !== "completed") continue;
      const child = blueprint.child_blueprints[blueprint.child_ids.indexOf(childRun.id)];
      if (!child?.response_contract) continue;
      const structured = childRun.result.response?.structured;
      if (structured === null || structured === undefined) throw new ProviderRuntimeError(`structured response is missing for cross-domain child ${childRun.id}`);
      entries.push({ domain: child.domain_profile.domain, contract: child.response_contract, response: structured, role: "specialist" });
    }
    if (synthesis?.status === "completed" && synthesis.blueprint?.response_contract) {
      const structured = synthesis.response?.structured;
      if (structured === null || structured === undefined) throw new ProviderRuntimeError("structured response is missing for cross-domain synthesis");
      entries.push({ domain: "cross_domain", contract: synthesis.blueprint.response_contract, response: structured, role: "synthesis" });
    }
    return entries.length ? entries : null;
  }

  /** Execute routed specialist children with bounded fan-out, then hand local outputs to synthesis. */
  async runCrossDomain(task: string, options: AutonomousCrossDomainRunOptions = {}): Promise<AutonomousCrossDomainRunResult> {
    options = this.withPromptLearningOptions(options);
    const taskText = boundedText("cross-domain task", task, 32_000);
    validateAutonomousStructuredOutputOptions(options);
    const domainPolicyMode = normalizeAutonomousDomainPolicyMode(options.domainPolicyMode);
    const contentParts = options.contentParts === undefined
      ? undefined
      : normalizeProviderContentParts(options.contentParts);
    let costBudget = resolveAutonomousCostBudget(options);
    const routeResolution = await this.resolveExecutionRoute(taskText, options, costBudget);
    const route = routeResolution.route;
    const semanticRoute = routeResolution.semanticRoute;
    const learning = this.learner ? "online_bandit_feedback_available" as const : "provider_health_feedback_only" as const;
    if (semanticRoute !== null && semanticRoute.status !== "completed") {
      const reviewed: AutonomousCrossDomainRunResult = {
        schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA,
        status: semanticRouteCrossDomainStatus(semanticRoute.status),
        route,
        semantic_route: semanticRoute,
        blueprint: null,
        child_runs: [],
        synthesis: null,
        completed_children: 0,
        total_children: route.selected_domains.length,
        partial: false,
        plan_refinement_digest: null,
        learning_episode_ids: [],
        response_learning_episode_ids: [],
        learning,
        retention: "provider_responses_local; child_digests_only_in_synthesis_metadata",
      };
      return { ...reviewed, execution_receipt: await autonomousCrossDomainExecutionReceipt(reviewed) };
    }
    if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) {
      const reviewed: AutonomousCrossDomainRunResult = { schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status: "route_review_required", route, semantic_route: semanticRoute, blueprint: null, child_runs: [], synthesis: null, completed_children: 0, total_children: route.selected_domains.length, partial: false, plan_refinement_digest: null, learning_episode_ids: [], response_learning_episode_ids: [], learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" };
      return { ...reviewed, execution_receipt: await autonomousCrossDomainExecutionReceipt(reviewed) };
    }
    const memory = await this.prepareMemory(taskText, route, options, [...route.selected_domains, "cross_domain"]);
    const finish = async (result: AutonomousCrossDomainRunResult): Promise<AutonomousCrossDomainRunResult> => {
      const withSemanticRoute = semanticRoute === null ? result : { ...result, semantic_route: semanticRoute };
      const withReceipt: AutonomousCrossDomainRunResult = { ...withSemanticRoute, execution_receipt: await autonomousCrossDomainExecutionReceipt(withSemanticRoute) };
      if (!memory.store) return memory.projection ? { ...withReceipt, memory: memory.projection } : withReceipt;
      return { ...withReceipt, memory: await this.recordMemory(taskText, route, withReceipt, options, memory) };
    };
    const blueprint = await this.buildCrossDomainBlueprint(taskText, route, {
      capability: options.capability,
      context: [...(options.context ?? []), ...memory.context],
      maxInputTokens: options.maxInputTokens,
      tools: options.tools?.map((tool) => tool.name),
      subtasks: options.subtasks,
      structuredDomainResponse: options.structuredDomainResponse,
      toolSelectionState: options.toolSelectionState,
      toolSelectionExploration: options.toolSelectionExploration,
      maxToolRiskClass: options.maxToolRiskClass,
    });
    for (const [index, child] of blueprint.child_blueprints.entries()) {
      assertAutonomousTaskDecisionAllowsProvider(child.task_decision, `cross-domain child ${index + 1}`);
    }
    assertAutonomousTaskDecisionAllowsProvider(blueprint.synthesis_blueprint.task_decision, "cross-domain synthesis");
    const acceptedPlan = await acceptedCrossDomainPlan(blueprint, options.acceptedCrossDomainPlanRefinement);
    const planRefinementDigest = acceptedPlan?.refinement_digest ?? null;
    const domainPolicyAdmissions = domainPolicyMode === "strict"
      ? Object.fromEntries([
        ...blueprint.child_blueprints.map((child) => [child.domain_profile.domain, domainPolicyAdmissionForBlueprint(route, child, options, acceptedPlan !== null)] as const),
        ["cross_domain", domainPolicyAdmissionForBlueprint(route, blueprint.synthesis_blueprint, options, acceptedPlan !== null)] as const,
      ]) as Record<string, AutonomousDomainPolicyAdmission>
      : undefined;
    const failedPolicyAdmissions = domainPolicyAdmissions === undefined
      ? []
      : Object.values(domainPolicyAdmissions).filter((admission) => admission.decision !== "admitted");
    if (failedPolicyAdmissions.length > 0) {
      const status = failedPolicyAdmissions.some((admission) => admission.decision === "blocked") ? "policy_blocked" : "policy_review_required";
      return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status, route, blueprint, child_runs: [], synthesis: null, completed_children: 0, total_children: blueprint.child_blueprints.length, partial: false, plan_refinement_digest: planRefinementDigest, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: [], response_learning_episode_ids: [], learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
    }
    if (domainPolicyMode === "strict" && costBudget === undefined) costBudget = new AutonomousCostBudget(autonomousDomainPolicy("cross_domain").max_total_cost_units);
    if (options.approveProviderCall !== true) {
      return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status: "approval_required", route, blueprint, child_runs: [], synthesis: null, completed_children: 0, total_children: blueprint.child_blueprints.length, partial: false, plan_refinement_digest: planRefinementDigest, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: [], response_learning_episode_ids: [], learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
    }
    const candidates = options.candidates ? [...options.candidates] : this.models();
    if (!candidates.length) throw new ProviderRuntimeError("cross-domain run requires at least one registered model candidate");
    const totalChildren = blueprint.child_blueprints.length;
    const maxParallelChildren = normalizedCrossDomainConcurrency(options.maxParallelChildren, totalChildren);
    const childRunsByIndex: Array<AutonomousCrossDomainChildRun | undefined> = new Array(totalChildren);
    const childOutputsByIndex: Array<{ id: string; domain: AutonomousDomainName; status: string; output: string } | undefined> = new Array(totalChildren);
    const learningEpisodeIdsByIndex: Array<string | null> = new Array(totalChildren).fill(null);
    const responseLearningEpisodeIdsByIndex: Array<string | null> = new Array(totalChildren).fill(null);
    const declarationOrder = blueprint.child_blueprints.map((_, index) => index);
    const executionOrder = acceptedPlan
      ? acceptedPlan.priority_child_ids.map((childId) => blueprint.child_ids.indexOf(childId))
      : declarationOrder;
    let nextChildIndex = 0;
    let stopDispatch = false;
    let fatalChildFailure = false;

    const executeChild = async (index: number): Promise<void> => {
      const child = blueprint.child_blueprints[index];
      if (!child) throw new ProviderRuntimeError(`cross-domain child blueprint ${index + 1} is missing`);
      const childId = blueprint.child_ids[index] ?? `child-${index + 1}`;
      const taskMessage = child.prompt.messages.find((message) => message.source_id === "task");
      if (!taskMessage) throw new ProviderRuntimeError(`cross-domain child ${childId} has no bounded task message`);
      let childResult: AutonomousRunResult;
      try {
        childResult = await this.run(taskMessage.content, {
          domain: child.domain_profile.domain,
          capability: child.selection_context.capability,
          candidates,
          credential: options.credential,
          credentialFor: options.credentialFor,
          context: [
            ...(options.context ?? []),
            ...memory.context,
            { id: "cross-domain-parent", content: `Parent route digest: ${route.route_digest}; child id: ${childId}`, required: true, priority: 100 },
            ...(acceptedPlan ? [{ id: "accepted-cross-domain-plan", content: JSON.stringify({ refinement_digest: acceptedPlan.refinement_digest, child_id: childId, priority_rank: acceptedPlan.priority_child_ids.indexOf(childId), focus: acceptedPlan.focus_child_ids.includes(childId) }), required: true, priority: 95 }] : []),
          ],
          promptTemplate: options.promptTemplate,
          promptRegistry: options.promptRegistry,
          promptSelection: options.promptSelection,
          promptStage: options.promptStage,
          promptLearningState: options.promptLearningState,
          promptLearningExploration: options.promptLearningExploration,
          contentParts,
          retrieveMemory: false,
          recordMemory: false,
          hints: [],
          maxInputTokens: options.maxInputTokens,
          maxOutputTokens: options.maxOutputTokens,
          maxCostPerMillionTokens: options.maxCostPerMillionTokens,
          maxLatencyMs: options.maxLatencyMs,
          minQuality: options.minQuality,
          minSelectionConfidence: options.minSelectionConfidence,
          requireJson: options.requireJson,
          responseSchema: options.responseSchema,
          structuredDomainResponse: options.structuredDomainResponse,
          requireStructuredResponseReview: false,
          domainPolicyMode: options.domainPolicyMode,
          domainPolicyEvidenceReady: options.domainPolicyEvidenceReady,
          domainPolicyEvaluatorConfigured: options.domainPolicyEvaluatorConfigured,
          domainPolicyPlanAccepted: options.domainPolicyPlanAccepted ?? acceptedPlan !== null,
          domainPolicyEffectsRequested: options.domainPolicyEffectsRequested,
          domainPolicyEffectsApproved: options.domainPolicyEffectsApproved,
          maxToolTurns: options.maxToolTurns,
          temperature: options.temperature,
          tools: options.tools,
          authorizeAndExecute: options.authorizeAndExecute,
          toolReadOnly: options.toolReadOnly,
          approveProviderCall: true,
          approveEffects: options.approveEffects,
          execution: options.execution,
          effectBoundary: options.effectBoundary ?? this.effectBoundary,
          maxTotalCostUnits: undefined,
          costBudget,
          executionAttempt: index + 1,
          providerIdempotencyKey: await scopedProviderIdempotencyKey(options.providerIdempotencyKey, {
            phase: "cross_domain_child",
            child_id: childId,
            child_index: index,
          }),
          maxProviderFailovers: options.maxProviderFailovers,
          signal: options.signal,
          observer: options.observer,
          providerDispatchFence: options.providerDispatchFence,
          selectionEventCallback: options.selectionEventCallback,
          toolSelectionState: options.toolSelectionState,
          toolSelectionExploration: options.toolSelectionExploration,
          maxToolRiskClass: options.maxToolRiskClass,
        });
      } catch (error) {
        if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
        // Provider/credential failures are expected operational outcomes for a bounded child.
        // Convert them to metadata-only results so allowPartial can preserve healthy siblings
        // and synthesis can explicitly see the omission without receiving an error message or
        // provider payload. Programming/configuration errors still propagate to the caller.
        childResult = providerFailureRunResult(route, child, error, learning);
      }
      const rawOutput = childResult.response?.text ?? (childResult.response?.structured === null || childResult.response?.structured === undefined ? "" : JSON.stringify(childResult.response.structured));
      const boundedOutput = rawOutput.length > 48_000 ? `${rawOutput.slice(0, 48_000)}\n[child output bounded locally]` : rawOutput;
      const output = boundedOutput.trim() || "[child returned no textual or structured output]";
      childOutputsByIndex[index] = { id: childId, domain: child.domain_profile.domain, status: childResult.status, output };
      childRunsByIndex[index] = { id: childId, domain: child.domain_profile.domain, task_digest: child.task_digest, result: childResult, output_digest: rawOutput ? await digestJson({ output: rawOutput }) : null, output_bytes: bytes(rawOutput) };
      if (options.learning && childResult.status === "completed") {
        const episodeId = `cross:${route.task_digest}:${childId}`;
        const episode = await options.learning.prepareRun(childResult, { episodeId, runId: episodeId, stageId: childId, parentJobId: `cross:${route.task_digest}`, planRefinementDigest });
        learningEpisodeIdsByIndex[index] = episode.episode_id;
        if (childResult.response_evaluation) {
          const responseEpisodeId = `response:${digestJsonSync({ episode_id: episode.episode_id }).slice(0, 64)}`;
          const responseEpisode = await options.learning.prepareRun(childResult, { episodeId: responseEpisodeId, runId: responseEpisodeId, stageId: childId, parentJobId: `cross:${route.task_digest}`, planRefinementDigest });
          responseLearningEpisodeIdsByIndex[index] = responseEpisode.episode_id;
        }
      }
      if (childResult.status !== "completed" && !options.allowPartial) stopDispatch = true;
    };

    const worker = async (): Promise<void> => {
      while (true) {
        if (fatalChildFailure || (stopDispatch && !options.allowPartial)) return;
        const sequenceIndex = nextChildIndex;
        nextChildIndex += 1;
        const index = executionOrder[sequenceIndex];
        if (index === undefined) return;
        try {
          await executeChild(index);
        } catch (error) {
          // A thrown child has no bounded result envelope. Stop scheduling new work and let the
          // caller retain the original typed failure rather than synthesizing incomplete output.
          fatalChildFailure = true;
          stopDispatch = true;
          throw error;
        }
      }
    };
    await Promise.all(Array.from({ length: maxParallelChildren }, () => worker()));

    const childRuns = executionOrder.flatMap((index) => {
      const child = childRunsByIndex[index];
      return child ? [child] : [];
    });
    const learningEpisodeIds = learningEpisodeIdsByIndex.flatMap((episodeId) => episodeId ? [episodeId] : []);
    const responseLearningEpisodeIds = responseLearningEpisodeIdsByIndex.flatMap((episodeId) => episodeId ? [episodeId] : []);
    const childOutputs = executionOrder.flatMap((index) => {
      const output = childOutputsByIndex[index];
      return output ? [output] : [];
    });
    const completedChildren = childRuns.filter((child) => child.result.status === "completed").length;
    const allChildrenCompleted = childRuns.length === blueprint.child_blueprints.length && completedChildren === blueprint.child_blueprints.length;
    const hasApproval = childRuns.some((child) => child.result.status === "approval_required");
    const hasTurnLimit = childRuns.some((child) => child.result.status === "turn_limit_reached");
    let responseAssessment: AutonomousCrossDomainResponseAssessment | null = null;
    if (options.structuredDomainResponse === true) {
      const entries = this.crossDomainResponseEntries(blueprint, childRuns);
      if (entries !== null) {
        responseAssessment = assessAutonomousCrossDomainResponseSet(entries, {
          requestedDomains: blueprint.child_blueprints.map((child) => child.domain_profile.domain),
          contextDigest: blueprint.task_digest,
          alignments: options.responseAlignments,
          requireSynthesis: false,
          requireCompleteAlignment: options.requireResponseAlignment ?? false,
          minimumReward: options.minimumResponseReward,
          minimumAlignmentConfidence: options.minimumResponseAlignmentConfidence,
          contradictionConfidenceThreshold: options.responseContradictionConfidenceThreshold,
        });
      }
    }
    // Partial fan-in is useful only when at least one specialist produced usable evidence.
    // Never spend another provider call synthesizing an empty or entirely blocked fan-out.
    // This also keeps credential/provider failures from being accidentally upgraded into a
    // successful-looking conclusion when allowPartial is enabled.
    if (!allChildrenCompleted && (!options.allowPartial || (options.synthesize !== false && completedChildren === 0))) {
      const hasReconciliation = childRuns.some((child) => child.result.status === "reconciliation_required");
      return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status: hasReconciliation ? "reconciliation_required" : hasApproval ? "approval_required" : hasTurnLimit ? "turn_limit_reached" : "child_failed", route, blueprint, child_runs: childRuns, synthesis: null, completed_children: completedChildren, total_children: blueprint.child_blueprints.length, partial: completedChildren > 0, plan_refinement_digest: planRefinementDigest, response_assessment: responseAssessment, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: learningEpisodeIds, response_learning_episode_ids: responseLearningEpisodeIds, learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
    }
    if (options.synthesize !== false && responseAssessment !== null && !responseAssessment.ready_to_synthesize) {
      return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status: "response_review_required", route, blueprint, child_runs: childRuns, synthesis: null, completed_children: completedChildren, total_children: blueprint.child_blueprints.length, partial: !allChildrenCompleted, plan_refinement_digest: planRefinementDigest, response_assessment: responseAssessment, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: learningEpisodeIds, response_learning_episode_ids: responseLearningEpisodeIds, learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
    }
    if (options.synthesize === false) {
      return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status: allChildrenCompleted ? "children_completed" : "children_partial", route, blueprint, child_runs: childRuns, synthesis: null, completed_children: completedChildren, total_children: blueprint.child_blueprints.length, partial: !allChildrenCompleted, plan_refinement_digest: planRefinementDigest, response_assessment: responseAssessment, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: learningEpisodeIds, response_learning_episode_ids: responseLearningEpisodeIds, learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
    }
    const synthesisTaskMessage = blueprint.synthesis_blueprint.prompt.messages.find((message) => message.source_id === "task");
    if (!synthesisTaskMessage) throw new ProviderRuntimeError("cross-domain synthesis has no bounded task message");
    const synthesisContext: AutonomousPromptChunk[] = [
      ...(options.context ?? []),
      ...memory.context,
      { id: "cross-domain-parent", content: `Parent route digest: ${route.route_digest}`, required: true, priority: 100 },
      ...(acceptedPlan ? [{ id: "accepted-cross-domain-plan", content: JSON.stringify({ refinement_digest: acceptedPlan.refinement_digest, priority_child_ids: acceptedPlan.priority_child_ids, focus_child_ids: acceptedPlan.focus_child_ids }), required: true, priority: 95 }] : []),
      ...childOutputs.map((child) => ({
        id: `cross-domain-output-${child.id}`,
        content: JSON.stringify(child),
        priority: 90,
      })),
    ];
    let synthesis: AutonomousRunResult;
    try {
      synthesis = await this.run(synthesisTaskMessage.content, {
      domain: "cross_domain",
      capability: "cross_domain_synthesis",
      candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      context: synthesisContext,
      promptTemplate: options.promptTemplate,
      promptRegistry: options.promptRegistry,
      promptSelection: options.promptSelection,
      promptStage: options.promptStage,
      promptLearningState: options.promptLearningState,
      promptLearningExploration: options.promptLearningExploration,
      contentParts,
      retrieveMemory: false,
      recordMemory: false,
      hints: [],
      maxInputTokens: options.maxInputTokens,
      maxOutputTokens: options.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      minSelectionConfidence: options.minSelectionConfidence,
      requireJson: options.requireJson,
      responseSchema: options.responseSchema,
      structuredDomainResponse: options.structuredDomainResponse,
      requireStructuredResponseReview: false,
      domainPolicyMode: options.domainPolicyMode,
      domainPolicyEvidenceReady: options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.domainPolicyEvaluatorConfigured,
      domainPolicyPlanAccepted: options.domainPolicyPlanAccepted ?? acceptedPlan !== null,
      domainPolicyEffectsRequested: options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.domainPolicyEffectsApproved,
      maxToolTurns: options.maxToolTurns,
      temperature: options.temperature,
      tools: options.tools,
      authorizeAndExecute: options.authorizeAndExecute,
      toolReadOnly: options.toolReadOnly,
      approveProviderCall: true,
      approveEffects: options.approveEffects,
      execution: options.execution,
      effectBoundary: options.effectBoundary ?? this.effectBoundary,
      maxTotalCostUnits: undefined,
      costBudget,
      executionAttempt: totalChildren + 1,
      providerIdempotencyKey: await scopedProviderIdempotencyKey(options.providerIdempotencyKey, {
        phase: "cross_domain_synthesis",
      }),
      maxProviderFailovers: options.maxProviderFailovers,
      signal: options.signal,
      observer: options.observer,
      providerDispatchFence: options.providerDispatchFence,
      selectionEventCallback: options.selectionEventCallback,
      toolSelectionState: options.toolSelectionState,
      toolSelectionExploration: options.toolSelectionExploration,
        maxToolRiskClass: options.maxToolRiskClass,
      });
    } catch (error) {
      if (!(error instanceof ProviderRuntimeError || error instanceof CredentialError)) throw error;
      // Keep synthesis failures on the same typed boundary as child failures. The parent
      // remains explicit about the failed synthesis and never serializes provider diagnostics.
      synthesis = providerFailureRunResult(route, blueprint.synthesis_blueprint, error, learning);
    }
    if (options.learning && synthesis.status === "completed") {
      const episodeId = `cross:${route.task_digest}:synthesis`;
      const episode = await options.learning.prepareRun(synthesis, { episodeId, runId: episodeId, stageId: "synthesis", parentJobId: `cross:${route.task_digest}`, planRefinementDigest });
      learningEpisodeIds.push(episode.episode_id);
      if (synthesis.response_evaluation) {
        const responseEpisodeId = `response:${digestJsonSync({ episode_id: episode.episode_id }).slice(0, 64)}`;
        const responseEpisode = await options.learning.prepareRun(synthesis, { episodeId: responseEpisodeId, runId: responseEpisodeId, stageId: "synthesis", parentJobId: `cross:${route.task_digest}`, planRefinementDigest });
        responseLearningEpisodeIds.push(responseEpisode.episode_id);
      }
    }
    if (options.structuredDomainResponse === true && synthesis.status === "completed") {
      const entries = this.crossDomainResponseEntries(blueprint, childRuns, synthesis);
      if (entries !== null) {
        responseAssessment = assessAutonomousCrossDomainResponseSet(entries, {
          requestedDomains: blueprint.child_blueprints.map((child) => child.domain_profile.domain),
          contextDigest: blueprint.task_digest,
          alignments: options.responseAlignments,
          requireSynthesis: true,
          requireCompleteAlignment: options.requireResponseAlignment ?? false,
          minimumReward: options.minimumResponseReward,
          minimumAlignmentConfidence: options.minimumResponseAlignmentConfidence,
          contradictionConfidenceThreshold: options.responseContradictionConfidenceThreshold,
        });
      }
    }
    const status: AutonomousCrossDomainRunStatus = synthesis.status === "completed"
      ? responseAssessment !== null && responseAssessment.status !== "completed" ? "response_review_required" : (allChildrenCompleted ? "completed" : "children_partial")
      : synthesis.status === "approval_required" ? "approval_required" : synthesis.status === "reconciliation_required" ? "reconciliation_required" : synthesis.status === "turn_limit_reached" ? "turn_limit_reached" : "child_failed";
    return finish({ schema: AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA, status, route, blueprint, child_runs: childRuns, synthesis, completed_children: completedChildren, total_children: blueprint.child_blueprints.length, partial: !allChildrenCompleted, plan_refinement_digest: planRefinementDigest, response_assessment: responseAssessment, ...(domainPolicyAdmissions === undefined ? {} : { domain_policy_admissions: domainPolicyAdmissions }), learning_episode_ids: learningEpisodeIds, response_learning_episode_ids: responseLearningEpisodeIds, learning, retention: "provider_responses_local; child_digests_only_in_synthesis_metadata" });
  }

  private memoryStoreForRun(options: Pick<AutonomousRunOptions, "memoryStore">): AutonomousEpisodicMemoryStore | undefined {
    return options.memoryStore ?? this.memoryStore;
  }

  /** Prepare a pending evaluator settlement boundary for ordinary direct runs. */
  private async prepareDirectLearning(
    result: AutonomousRunResult,
    route: AutonomousRouteProposal,
    options: Pick<AutonomousRunOptions, "learning" | "learningEpisodeId" | "memoryRunId"> & { memoryEpisodeId?: string | null },
  ): Promise<Pick<AutonomousRunResult, "learning_episode_id" | "learning_episode_status" | "learning_error_class" | "response_learning_episode_id" | "response_learning_episode_status" | "response_learning_error_class">> {
    if (!options.learning) return {};
    if (!result.blueprint || !result.selection?.selected_model) {
      return { learning_episode_id: null, learning_episode_status: "not_eligible", learning_error_class: null, response_learning_episode_id: null, response_learning_episode_status: "not_eligible", response_learning_error_class: null };
    }
    const taskEligible = result.status === "completed";
    let episode: Awaited<ReturnType<AutonomousLearningController["prepareRun"]>> | null = null;
    try {
      if (taskEligible) {
        const derivedId = options.learningEpisodeId
          ?? (options.memoryRunId
            ? `learning:${memoryIdentity("memory run id", options.memoryRunId)}`
            : `learning:${route.task_digest.slice(0, 24)}:${++autonomousLearningEpisodeSequence}`);
        const episodeId = memoryIdentity("learning episode id", derivedId);
        episode = await options.learning.prepareRun(result, { episodeId, runId: episodeId, memoryEpisodeId: options.memoryEpisodeId ?? null });
      }
      if (!result.response_evaluation) {
        return { learning_episode_id: episode?.episode_id ?? null, learning_episode_status: episode ? "prepared" : "not_eligible", learning_error_class: null, response_learning_episode_id: null, response_learning_episode_status: "not_eligible", response_learning_error_class: null };
      }
      try {
        const responseSeed = episode?.episode_id
          ? `response:${digestJsonSync({ episode_id: episode.episode_id }).slice(0, 64)}`
          : `response:${options.learningEpisodeId ?? `run:${route.task_digest.slice(0, 24)}:${result.response_evaluation.response_digest.slice(0, 40)}`}`;
        const responseEpisodeId = memoryIdentity("response learning episode id", responseSeed);
        const responseEpisode = await options.learning.prepareRun(result, { episodeId: responseEpisodeId, runId: responseEpisodeId, memoryEpisodeId: null, responseOnly: !taskEligible });
        return { learning_episode_id: episode?.episode_id ?? null, learning_episode_status: episode ? "prepared" : "not_eligible", learning_error_class: null, response_learning_episode_id: responseEpisode.episode_id, response_learning_episode_status: "prepared", response_learning_error_class: null };
      } catch (error) {
        return { learning_episode_id: episode?.episode_id ?? null, learning_episode_status: episode ? "prepared" : "not_eligible", learning_error_class: null, response_learning_episode_id: null, response_learning_episode_status: "failed", response_learning_error_class: memoryErrorClass(error) };
      }
    } catch (error) {
      // A requested learning adapter must be observable as failed, but it must not turn a valid
      // provider result into a fabricated provider failure or cause a provider replay.
      return { learning_episode_id: null, learning_episode_status: taskEligible ? "failed" : "not_eligible", learning_error_class: taskEligible ? memoryErrorClass(error) : null, response_learning_episode_id: null, response_learning_episode_status: result.response_evaluation ? "failed" : "not_eligible", response_learning_error_class: result.response_evaluation ? memoryErrorClass(error) : null };
    }
  }

  /** Retrieve only bounded, value-only episode projections before prompt assembly. */
  private async prepareMemory(
    taskText: string,
    route: AutonomousRouteProposal,
    options: Pick<AutonomousRunOptions, "memoryStore" | "memoryQuery" | "memoryRecall" | "memoryLimit" | "capability" | "retrieveMemory" | "memoryConsolidator" | "memoryLessonResolver" | "memoryLessonContextResolver" | "consolidatedMemoryLimit" | "retrieveConsolidatedMemory" | "consolidatedMemoryRequired">,
    domains: readonly AutonomousDomainName[],
  ): Promise<AutonomousMemoryPreparation> {
    const store = this.memoryStoreForRun(options);
    const authorizationContext = (options as Pick<AutonomousRunOptions, "authorizationContext">).authorizationContext;
    const consolidator = options.memoryConsolidator ?? this.memoryConsolidator;
    if (options.memoryLessonResolver !== undefined && typeof options.memoryLessonResolver !== "function") throw new ArgumentError("autonomous memoryLessonResolver must be callable");
    if (options.memoryLessonContextResolver !== undefined && typeof options.memoryLessonContextResolver !== "function") throw new ArgumentError("autonomous memoryLessonContextResolver must be callable");
    if (options.memoryLessonResolver !== undefined && options.memoryLessonContextResolver !== undefined) throw new ArgumentError("autonomous memoryLessonResolver and memoryLessonContextResolver are mutually exclusive");
    if (options.retrieveConsolidatedMemory !== undefined && typeof options.retrieveConsolidatedMemory !== "boolean") throw new ArgumentError("autonomous retrieveConsolidatedMemory must be boolean");
    if (options.consolidatedMemoryRequired !== undefined && typeof options.consolidatedMemoryRequired !== "boolean") throw new ArgumentError("autonomous consolidatedMemoryRequired must be boolean");
    const consolidationRequested = options.retrieveConsolidatedMemory !== false && consolidator !== undefined && (options.memoryLessonResolver !== undefined || options.memoryLessonContextResolver !== undefined);
    if (options.consolidatedMemoryRequired === true && !consolidationRequested) throw new ArgumentError("autonomous consolidatedMemoryRequired needs a consolidator and one lesson resolver");
    const consolidatedReferences = new Map<string, AutonomousMemoryConsolidationPromptReference>();
    let consolidatedErrorClass: string | null = null;
    if (consolidationRequested) {
      const consolidatedLimit = options.consolidatedMemoryLimit ?? 8;
      if (!Number.isSafeInteger(consolidatedLimit) || consolidatedLimit < 1 || consolidatedLimit > 32) throw new ArgumentError("autonomous consolidatedMemoryLimit must be between 1 and 32");
      try {
        for (const domain of [...new Set(domains)]) {
          if (authorizationContext) {
            await authorizationContext.authorizeOperation({
              operation: "memory_retrieval",
              domain,
              resourceDigest: await digestJson({
                schema: "bioprism-typescript-autonomous-consolidated-memory-authorization-resource/0.1",
                domain,
                capability: options.capability ?? null,
                limit: consolidatedLimit,
                resolver: options.memoryLessonContextResolver !== undefined ? "lesson_context" : "lesson",
              }),
            });
          }
          const references = consolidator!.promptReferences({ domain, capability: options.capability, lessonResolver: options.memoryLessonResolver, lessonContextResolver: options.memoryLessonContextResolver, limit: consolidatedLimit });
          for (const reference of references) {
            const key = `${reference.lesson_digest}:${reference.lesson_id}`;
            if (!consolidatedReferences.has(key)) consolidatedReferences.set(key, reference);
          }
        }
      } catch (error) {
        if (error instanceof AutonomousAuthorizationError) throw error;
        if (options.consolidatedMemoryRequired === true) throw error;
        consolidatedErrorClass = memoryErrorClass(error);
      }
    }
    const consolidated = [...consolidatedReferences.values()].slice(0, 32);
    const consolidatedLessonIds = consolidated.map((reference) => reference.lesson_id);
    const consolidatedLessonDigests = consolidated.map((reference) => reference.lesson_digest);
    const consolidatedRetrievalDigest = consolidated.length
      ? await digestJson({ lessons: consolidated.map(({ lesson_id, concept_id, lesson_digest, status, confidence }) => ({ lesson_id, concept_id, lesson_digest, status, confidence })) })
      : null;
    const consolidatedContext = consolidated.map(consolidatedMemoryContext);
    const consolidatedStatus = consolidatedErrorClass === null ? "retrieved" : "retrieval_failed";
    if (!store) {
      return {
        store: undefined,
        context: consolidatedContext,
        projection: consolidationRequested
          ? memoryProjection(consolidatedStatus, [], null, null, null, consolidatedErrorClass, consolidatedLessonIds, consolidatedLessonDigests, consolidatedRetrievalDigest)
          : null,
      };
    }
    if (options.retrieveMemory === false) {
      return { store, context: consolidatedContext, projection: memoryProjection(consolidatedErrorClass === null && consolidationRequested ? "retrieved" : "disabled", [], null, null, null, consolidatedErrorClass, consolidatedLessonIds, consolidatedLessonDigests, consolidatedRetrievalDigest) };
    }
    const supplied = options.memoryQuery ?? {};
    const taskFacets = supplied.task_facets === undefined ? taskFacetDigests(taskText) : supplied.task_facets;
    const limit = options.memoryLimit ?? supplied.limit ?? 8;
    const selectedDomains = supplied.domain === undefined && domains.length > 1 ? [...new Set(domains)] : [undefined];
    const episodesById = new Map<string, AutonomousMemoryEpisode>();
    try {
      for (const domain of selectedDomains) {
        const query: AutonomousMemoryQuery = {
          ...supplied,
          ...(domain === undefined ? {} : { domain }),
          ...(supplied.task_facets === undefined ? { task_facets: taskFacets } : {}),
          ...(supplied.capability === undefined && options.capability === undefined ? {} : { capability: supplied.capability ?? options.capability }),
          ranking: options.memoryRecall ?? supplied.ranking ?? "planning",
          limit,
        };
        if (authorizationContext) {
          const queryDomains = typeof query.domain === "string" ? [query.domain] : [...new Set(domains)];
          await authorizationContext.authorizeOperation({
            operation: "memory_retrieval",
            ...(queryDomains.length === 1 ? { domain: queryDomains[0] } : { domains: queryDomains }),
            resourceDigest: await digestJson({
              schema: "bioprism-autonomous-memory-authorization-resource/0.1",
              query_digest: await digestJson(query),
              limit,
            }),
          });
        }
        const episodes = await store.retrieve(query);
        for (const episode of episodes) episodesById.set(episode.episode_id, episode);
      }
      const ranking = options.memoryRecall ?? supplied.ranking ?? "planning";
      const episodes = [...episodesById.values()].sort((left, right) => {
        const planScore = (episode: AutonomousMemoryEpisode): number => {
          const quality = episode.evaluation?.reward ?? 0;
          const hasPlan = episode.digests.plan_refinement_digest !== null && episode.digests.plan_refinement_digest !== undefined;
          if (ranking === "quality") return quality * 100 + (episode.evaluation?.passed ? 5 : 0);
          if (ranking === "planning") return (hasPlan ? 100 : 0) + (episode.evaluation ? 20 + quality * 100 : 0) + (episode.evaluation?.passed ? 5 : 0);
          return episode.evaluation?.passed ? 2 : 0;
        };
        return planScore(right) - planScore(left) || right.updated_at - left.updated_at || left.episode_id.localeCompare(right.episode_id);
      }).slice(0, limit);
      const retrievalDigest = await digestJson({ episodes: episodes.map((episode) => ({ episode_id: episode.episode_id, episode_digest: episode.episode_digest })) });
      const projection = memoryProjection(consolidatedErrorClass === null ? "retrieved" : "retrieval_failed", episodes, retrievalDigest, null, null, consolidatedErrorClass, consolidatedLessonIds, consolidatedLessonDigests, consolidatedRetrievalDigest);
      return { store, context: [...episodes.map(memoryEpisodeContext), ...consolidatedContext], projection };
    } catch (error) {
      if (error instanceof AutonomousAuthorizationError) throw error;
      const projection = memoryProjection("retrieval_failed", [], null, null, null, memoryErrorClass(error), consolidatedLessonIds, consolidatedLessonDigests, consolidatedRetrievalDigest);
      return { store, context: consolidatedContext, projection };
    }
  }

  /** Record the run as a digest-only episode without allowing memory failure to masquerade as provider failure. */
  private async recordMemory(
    taskText: string,
    route: AutonomousRouteProposal,
    result: AutonomousRunResult | AutonomousCrossDomainRunResult,
    options: Pick<AutonomousRunOptions, "memoryStore" | "memoryRunId" | "learningEpisodeId" | "recordMemory" | "memoryTags" | "memoryLesson">,
    preparation: AutonomousMemoryPreparation,
  ): Promise<AutonomousMemoryRunProjection | null> {
    if (!preparation.store || options.recordMemory === false) return preparation.projection;
    const authorizationContext = (options as Pick<AutonomousRunOptions, "authorizationContext">).authorizationContext;
    const retrievedDigests = preparation.projection?.retrieved_episode_digests ?? [];
    const retrievalDigest = preparation.projection?.retrieval_digest ?? null;
    try {
      const blueprint = "synthesis" in result
        ? result.blueprint?.synthesis_blueprint ?? null
        : result.blueprint;
      const context = blueprint?.selection_context ?? {
        domain: route.primary_domain ?? "cross_domain",
        capability: "cross_domain_synthesis",
        risk_class: "cross_domain_integration",
        task_family: null,
      };
      const selection = "synthesis" in result ? result.synthesis?.selection ?? null : result.selection;
      const selectionDigest = selection ? await digestJson(selection) : null;
      const blueprintDigest = blueprint ? await digestJson(blueprint) : null;
      const outcomeDigest = await digestJson({
        status: result.status,
        route_digest: route.route_digest,
        blueprint_digest: blueprintDigest,
        selection_digest: selectionDigest,
        plan_refinement_digest: result.plan_refinement_digest,
        ...("completed_children" in result ? { completed_children: result.completed_children, total_children: result.total_children, partial: result.partial } : {}),
      });
      autonomousMemoryRunSequence += 1;
      const runId = memoryIdentity("memory run id", options.memoryRunId ?? (options.learningEpisodeId ? `learning-memory:${options.learningEpisodeId}` : `autonomous:${route.task_digest.slice(0, 24)}:${autonomousMemoryRunSequence}`));
      const episodeId = memoryIdentity("memory episode id", `episode:${runId}`);
      const memoryEpisodeInput = {
        episode_id: episodeId,
        run_id: runId,
        result_kind: "synthesis" in result ? "autonomous_cross_domain_run" : "autonomous_run",
        status: memoryRunStatus(result.status),
        task_digest: route.task_digest,
        task_facets: taskFacetDigests(taskText),
        context: {
          domain: context.domain,
          capability: context.capability,
          risk_class: context.risk_class,
          task_family: context.task_family ?? null,
        },
        selected_model: selection?.selected_model ?? null,
        digests: {
          route_digest: route.route_digest,
          blueprint_digest: blueprintDigest,
          selection_digest: selectionDigest,
          outcome_digest: outcomeDigest,
          plan_refinement_digest: result.plan_refinement_digest,
          retrieval_digest: retrievalDigest,
        },
        route: memoryRouteProjection(route),
        tags: options.memoryTags ?? [],
        lesson: options.memoryLesson ?? null,
        provenance: {
          source: "typescript_autonomous_agent",
          result_schema: result.schema,
        },
      } satisfies Parameters<AutonomousEpisodicMemoryStore["recordEpisode"]>[0];
      if (authorizationContext) {
        const authorizationDomain = typeof context.domain === "string" ? context.domain : route.primary_domain ?? "cross_domain";
        await authorizationContext.authorizeOperation({
          operation: "memory_write",
          domain: authorizationDomain,
          resourceDigest: await digestJson({
            schema: "bioprism-autonomous-memory-authorization-resource/0.1",
            episode_id: episodeId,
            run_id: runId,
            task_digest: route.task_digest,
            outcome_digest: outcomeDigest,
          }),
        });
      }
      const receipt = await preparation.store.recordEpisode(memoryEpisodeInput);
      const recorded = await preparation.store.get(episodeId);
      return {
        status: "recorded",
        retrieved_episode_ids: preparation.projection?.retrieved_episode_ids ?? [],
        retrieved_episode_digests: retrievedDigests,
        retrieval_digest: retrievalDigest,
        consolidated_lesson_ids: preparation.projection?.consolidated_lesson_ids ?? [],
        consolidated_lesson_digests: preparation.projection?.consolidated_lesson_digests ?? [],
        consolidated_retrieval_digest: preparation.projection?.consolidated_retrieval_digest ?? null,
        recorded_episode_id: recorded?.episode_id ?? episodeId,
        recorded_episode_digest: recorded?.episode_digest ?? null,
        record_event_digest: receipt.event_digest,
        error_class: preparation.projection?.error_class ?? null,
        retention: AUTONOMOUS_MEMORY_RUN_RETENTION,
        secret_material: "never_returned",
      };
    } catch (error) {
      if (error instanceof AutonomousAuthorizationError) throw error;
      return {
        status: "record_failed",
        retrieved_episode_ids: preparation.projection?.retrieved_episode_ids ?? [],
        retrieved_episode_digests: retrievedDigests,
        retrieval_digest: retrievalDigest,
        consolidated_lesson_ids: preparation.projection?.consolidated_lesson_ids ?? [],
        consolidated_lesson_digests: preparation.projection?.consolidated_lesson_digests ?? [],
        consolidated_retrieval_digest: preparation.projection?.consolidated_retrieval_digest ?? null,
        recorded_episode_id: null,
        recorded_episode_digest: null,
        record_event_digest: null,
        error_class: memoryErrorClass(error),
        retention: AUTONOMOUS_MEMORY_RUN_RETENTION,
        secret_material: "never_returned",
      };
    }
  }

  /** Apply explicit evaluator feedback locally; optionally reconcile the same value-only update through the control plane. */
  async recordEvaluatorReward(armId: string, reward: number, options: { failed?: boolean; outcomeDigest?: string | null; contractDigest?: string | null; remote?: boolean; contextDigest?: string | null; context?: BrainBanditContext } = {}): Promise<BrainBanditState> {
    if (!this.learner) throw new ArgumentError("AutonomousAgent has no AutonomousOnlineLearner");
    const contextDigest = options.contextDigest ?? null;
    if (contextDigest !== null && (typeof contextDigest !== "string" || !/^[0-9a-f]{64}$/.test(contextDigest) || !options.context)) throw new ArgumentError("contextual evaluator rewards require a valid context digest and context");
    if (contextDigest === null && options.context !== undefined) throw new ArgumentError("contextual evaluator rewards require a context digest");
    const update: BrainBanditUpdate = { arm_id: boundedText("armId", armId, 512), reward, failed: options.failed ?? false, outcome_digest: options.outcomeDigest ?? null, contract_digest: options.contractDigest ?? null, ...(contextDigest === null ? {} : { context_digest: contextDigest, context: options.context }) };
    if (options.remote === true && this.apiClient) {
      const response = await this.apiClient.brainBanditUpdate(this.learner.snapshot(), update);
      if (!response.ok || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError("remote bandit update returned a refusal");
      const projected = response.mcp.result?.structuredContent as BrainBanditState | undefined;
      if (!projected) throw new ProviderRuntimeError("remote bandit update returned no state");
      return this.learner.restore(projected);
    }
    return this.learner.update(update);
  }

  /** Return the explicit activation gate; before a plan exists, tool admission remains caller-owned. */
  private activationToolGate(): ReadonlySet<string> | null {
    const state = this.activation.state;
    if (state.plan_digest === null && state.status !== "revoked" && state.status !== "stale") return null;
    return new Set(state.approved_tools);
  }

  private filterActivatedTools(tools: readonly ProviderTool[]): ProviderTool[] {
    const gate = this.activationToolGate();
    return gate === null ? [...tools] : tools.filter((tool) => gate.has(tool.name));
  }

  private filterActivatedToolNames(names: readonly string[]): string[] {
    const gate = this.activationToolGate();
    return gate === null ? [...names] : names.filter((name) => gate.has(name));
  }

  private async dispatchActivatedToolCalls(
    calls: readonly ProviderToolCall[],
    authorize: (allowed: ProviderToolCall[]) => ProviderToolResult[] | Promise<ProviderToolResult[]>,
  ): Promise<ProviderToolResult[]> {
    const gate = this.activationToolGate();
    if (gate === null) return authorize([...calls]);
    const allowed = calls.filter((call) => gate.has(call.name));
    const blocked = new Set(calls.filter((call) => !gate.has(call.name)).map((call) => call.id));
    const results = allowed.length ? await authorize(allowed) : [];
    const resultById = new Map(results.map((result) => [result.callId, result]));
    return calls.map((call) => blocked.has(call.id)
      ? { callId: call.id, approved: false, isError: true, content: { status: "activation_required", tool: call.name, activation_status: this.activation.state.status, activation_plan_digest: this.activation.state.plan_digest, secret_material: "never_returned" } }
      : resultById.get(call.id) ?? { callId: call.id, approved: false, isError: true, content: { status: "authorization_result_missing", tool: call.name, secret_material: "never_returned" } });
  }

  private async liveToolNames(domains: readonly AutonomousDomainName[]): Promise<string[]> {
    const registry = await this.ensureToolRegistry();
    return registry ? this.filterActivatedToolNames((await registry.plan(domains)).available_curated_tools) : [];
  }

  private effectiveToolSelectionState(state?: AutonomousToolSelectionState | null): AutonomousToolSelectionState | null | undefined {
    if (state !== undefined) return state;
    return this.toolSelectionConfigured ? this.toolSelectionStateValue : undefined;
  }

  private async liveToolNamesForTask(task: string, domains: readonly AutonomousDomainName[], capability?: string, toolSelectionState?: AutonomousToolSelectionState | null, exploration?: number, maxRiskClass?: AutonomousToolRiskClass): Promise<string[]> {
    const registry = await this.ensureToolRegistry();
    if (!registry) return [];
    const gate = this.activationToolGate();
    const plan = await registry.planForTask(task, { domains, capability, allowedTools: gate === null ? undefined : [...gate], toolSelectionState, exploration, maxRiskClass });
    return this.filterActivatedToolNames(plan.selected_tool_order);
  }

  private async liveTools(domains: readonly AutonomousDomainName[]): Promise<ProviderTool[]> {
    const registry = await this.ensureToolRegistry();
    return registry ? this.filterActivatedTools(registry.toolsFor(domains)) : [];
  }

  private async liveToolsForNames(domains: readonly AutonomousDomainName[], names: readonly string[]): Promise<ProviderTool[]> {
    const registry = await this.ensureToolRegistry();
    if (!registry) return [];
    const selected = new Set(names);
    return this.filterActivatedTools(registry.toolsFor(domains).filter((tool) => selected.has(tool.name)));
  }

  private async ensureToolRegistry(): Promise<AutonomousDomainToolRegistry | undefined> {
    if (this.domainToolRegistry) return this.domainToolRegistry;
    if (!this.toolCatalogue) return undefined;
    this.domainToolRegistry = await AutonomousDomainToolRegistry.create(this.toolCatalogue);
    if (this.toolExecutor) {
      this.domainToolRuntime = new AutonomousDomainToolRuntime(this.domainToolRegistry, this.toolExecutor, { approver: this.toolApprover, effectBoundary: this.effectBoundary });
      this.capabilityRuntime = new AutonomousCapabilityRuntime(this.domainToolRuntime, {
        journal: this.capabilityJournal,
        admitTool: (tool) => {
          const gate = this.activationToolGate();
          return gate === null || gate.has(tool) || "activation_required";
        },
      });
    }
    return this.domainToolRegistry;
  }

  private async ensureCapabilityRuntime(): Promise<AutonomousCapabilityRuntime | undefined> {
    if (this.capabilityRuntime) return this.capabilityRuntime;
    await this.ensureToolRegistry();
    return this.capabilityRuntime;
  }

  private toolRuntimeForRun(): AutonomousDomainToolRuntime | undefined {
    return this.domainToolRuntime;
  }
}

export { AutonomousOnlineLearner, contextualSelector };
