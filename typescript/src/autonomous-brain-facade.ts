import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  validateAutonomousRouteOverride,
  type AutonomousAgent,
  type AutonomousApprovedModelSelectionOptions,
  type AutonomousAutoBlueprint,
  type AutonomousDomainToolPlan,
  type AutonomousCrossDomainBlueprint,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainRunResult,
  type AutonomousCrossDomainSubtask,
  type AutonomousDomainName,
  type AutonomousPromptChunk,
  type AutonomousRouteProposal,
  type AutonomousRunOptions,
  type AutonomousRunResult,
  type AutonomousModelSelectionPreview,
  type AutonomousModelSelectionPreviewOptions,
  type AutonomousTaskBlueprint,
} from "./autonomous.js";
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
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousCrossDomainReplanCycle,
  runAutonomousDecisionCycle,
  runAutonomousReplanCycle,
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
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type { ProviderInvocationObserver } from "./llm.js";
import { AutonomousCostBudget } from "./llm.js";
import type { JsonObject, JsonValue } from "./types.js";
import type {
  AutonomousWorkflowPortfolioAdmission,
  AutonomousWorkflowPortfolioAdmissionOptions,
} from "./autonomous-workflow-portfolio-admission.js";
import type { AutonomousWorkflowPortfolioItemRequest } from "./autonomous-workflow-portfolio.js";
import {
  auditAutonomousDomainContracts,
  type AutonomousDomainAuditOptions,
  type AutonomousDomainAuditReport,
} from "./autonomous-domain-audit.js";
import {
  auditAutonomousBrainLaunchPreflight,
  type AutonomousLaunchPreflightOptions,
  type AutonomousLaunchPreflightReport,
} from "./autonomous-launch-preflight.js";

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
export const AUTONOMOUS_BRAIN_SUMMARY_SCHEMA = "bioprism-typescript-autonomous-brain-plan-summary/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_BATCH = 64;
export const MAX_AUTONOMOUS_BRAIN_PARALLELISM = 8;
export const MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES = 128_000;
export const MAX_AUTONOMOUS_BRAIN_CONTEXT_CHUNKS = 128;
export const MAX_AUTONOMOUS_BRAIN_OBSERVATION_BYTES = 1_000_000;

export type AutonomousBrainPlanStatus = "ready" | "route_review_required" | "connector_review_required";
export type AutonomousBrainExecutionStatus = AutonomousRunResult["status"] | AutonomousCrossDomainRunResult["status"] | "connector_blocked";

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

/** Options for executing one caller-approved, digest-bound model-selection preview. */
export interface AutonomousBrainApprovedSelectionOptions {
  run?: Omit<AutonomousApprovedModelSelectionOptions, "domain">;
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

/** Options for the keyless readiness audit exposed at the application boundary. */
export type AutonomousBrainReadinessOptions = Parameters<AutonomousAgent["readiness"]>[0];
export type AutonomousBrainReadinessReport = Awaited<ReturnType<AutonomousAgent["readiness"]>>;
export type AutonomousBrainWorkflowPortfolioAdmissionOptions = AutonomousWorkflowPortfolioAdmissionOptions;
export type AutonomousBrainWorkflowPortfolioAdmission = AutonomousWorkflowPortfolioAdmission;
export type AutonomousBrainActivationState = ReturnType<AutonomousAgent["activationState"]>;
export type AutonomousBrainActivationSnapshotStore = AutonomousCapabilityActivationSnapshotStore;

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

export type AutonomousBrainBatchMode = "brain";

export interface AutonomousBrainBatchRehydrationContext {
  job_id: string;
  index: number;
  mode: AutonomousBrainBatchMode;
  request_digest: string;
  task_digest: string;
  expected_result_digest: string;
}

export interface AutonomousBrainBatchCheckpointJSON {
  schema: typeof AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA;
  job_id: string;
  mode: AutonomousBrainBatchMode;
  batch_input_digest: string;
  /** Digest of the non-secret semantic-routing policy; absent only on legacy deterministic checkpoints. */
  semantic_routing_policy_digest?: string;
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

export type AutonomousBrainBatchControllerRunOptions = Omit<AutonomousBrainResumableBatchOptions, "checkpoint" | "checkpointSink">;

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

function batchStatus(completed: number, failed: number, omitted: number): "completed" | "partial" | "failed" {
  return failed === 0 && omitted === 0 ? "completed" : completed > 0 ? "partial" : "failed";
}

function batchDigest(items: readonly { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }[]): string {
  return digestJsonSync(items.map((item) => batchItemProjection(item)));
}

function batchItemProjection(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }): Record<string, unknown> {
  return { index: item.index, status: item.status, task_digest: item.task_digest, error_class: item.error_class ?? null, failure_code: item.failure_code ?? null, plan_digest: item.execution?.plan.plan_digest ?? null, execution_status: item.execution?.status ?? null };
}

function batchItemDigest(item: { index: number; status: string; task_digest: string | null; error_class?: string; failure_code?: string; execution?: { plan: { plan_digest: string }; status: string } }): string {
  return digestJsonSync(batchItemProjection(item));
}

function brainBatchTaskDigest(input: AutonomousBrainRequest): string {
  return digestJsonSync({ task: input.task });
}

function brainBatchRequestDigest(input: AutonomousBrainRequest, index: number): string {
  return digestJsonSync({
    index,
    mode: "brain",
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

function checkpointText(name: string, value: unknown): string {
  return boundedIdentifier(name, value);
}

function validateBrainBatchCheckpoint(value: unknown): AutonomousBrainBatchCheckpointJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA || value.mode !== "brain") throw new ArgumentError("autonomous brain batch checkpoint schema is invalid");
  const allowedKeys = new Set(["schema", "job_id", "mode", "batch_input_digest", "semantic_routing_policy_digest", "request_digests", "completed_indices", "completed_result_digests", "max_parallelism", "stop_on_error", "status", "checkpoint_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError("autonomous brain batch checkpoint contains unsupported metadata");
  const jobId = checkpointText("autonomous brain batch checkpoint job_id", value.job_id);
  const batchInputDigest = digest("autonomous brain batch checkpoint batch_input_digest", value.batch_input_digest);
  const semanticRoutingPolicyDigest = value.semantic_routing_policy_digest === undefined ? undefined : digest("autonomous brain batch checkpoint semantic_routing_policy_digest", value.semantic_routing_policy_digest);
  const requestDigests = value.request_digests;
  if (!Array.isArray(requestDigests) || requestDigests.length < 1 || requestDigests.length > MAX_AUTONOMOUS_BRAIN_BATCH || requestDigests.some((entry) => typeof entry !== "string" || !/^[0-9a-f]{64}$/.test(entry))) throw new ArgumentError("autonomous brain batch checkpoint request_digests are invalid");
  if (!Array.isArray(value.completed_indices) || value.completed_indices.length > requestDigests.length || value.completed_indices.some((entry) => !Number.isSafeInteger(entry) || (entry as number) < 0 || (entry as number) >= requestDigests.length)) throw new ArgumentError("autonomous brain batch checkpoint completed_indices are invalid");
  const completedIndices = [...(value.completed_indices as number[])];
  if (new Set(completedIndices).size !== completedIndices.length || completedIndices.some((entry, index) => index > 0 && entry <= completedIndices[index - 1]!)) throw new ArgumentError("autonomous brain batch checkpoint completed_indices must be sorted and unique");
  if (!Array.isArray(value.completed_result_digests) || value.completed_result_digests.length !== completedIndices.length || value.completed_result_digests.some((entry) => typeof entry !== "string" || !/^[0-9a-f]{64}$/.test(entry))) throw new ArgumentError("autonomous brain batch checkpoint result digests are invalid");
  if (!Number.isSafeInteger(value.max_parallelism) || (value.max_parallelism as number) < 1 || (value.max_parallelism as number) > MAX_AUTONOMOUS_BRAIN_PARALLELISM) throw new ArgumentError("autonomous brain batch checkpoint maxParallelism is invalid");
  if (typeof value.stop_on_error !== "boolean" || !["running", "partial", "completed"].includes(value.status as string)) throw new ArgumentError("autonomous brain batch checkpoint controls are invalid");
  if (value.status === "completed" && completedIndices.length !== requestDigests.length) throw new ArgumentError("completed autonomous brain batch checkpoint is incomplete");
  const payload = { schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, job_id: jobId, mode: "brain" as const, batch_input_digest: batchInputDigest, ...(semanticRoutingPolicyDigest === undefined ? {} : { semantic_routing_policy_digest: semanticRoutingPolicyDigest }), request_digests: [...requestDigests as string[]], completed_indices: completedIndices, completed_result_digests: [...(value.completed_result_digests as string[])], max_parallelism: value.max_parallelism as number, stop_on_error: value.stop_on_error as boolean, status: value.status as "running" | "partial" | "completed" };
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES) throw new ArgumentError("autonomous brain batch checkpoint exceeds its bounded size");
  if (digestJsonSync(payload) !== value.checkpoint_digest) throw new ArgumentError("autonomous brain batch checkpoint digest is invalid");
  if (value.retention !== "request_and_result_digests_only;tasks_prompts_credentials_and_payloads_never_persisted" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous brain batch checkpoint retention contract is invalid");
  return { ...payload, checkpoint_digest: value.checkpoint_digest as string, retention: value.retention, secret_material: value.secret_material };
}

function makeBrainBatchCheckpoint(input: { jobId: string; requestDigests: readonly string[]; batchInputDigest: string; semanticRoutingPolicyDigest: string | null; completed: readonly { index: number; item: AutonomousBrainBatchItem }[]; maxParallelism: number; stopOnError: boolean; status: "running" | "partial" | "completed" }): AutonomousBrainBatchCheckpointJSON {
  const payload = { schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, job_id: input.jobId, mode: "brain" as const, batch_input_digest: input.batchInputDigest, ...(input.semanticRoutingPolicyDigest === null ? {} : { semantic_routing_policy_digest: input.semanticRoutingPolicyDigest }), request_digests: [...input.requestDigests], completed_indices: input.completed.map((entry) => entry.index), completed_result_digests: input.completed.map((entry) => batchItemDigest(entry.item)), max_parallelism: input.maxParallelism, stop_on_error: input.stopOnError, status: input.status };
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

  /** Compile routing and workflow metadata without contacting a provider or connector. */
  async plan(input: AutonomousBrainRequest): Promise<AutonomousBrainPlan> {
    const request = validateRequest(input);
    const route = await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true });
    return this.buildPlanForRoute(request, route, null);
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
   * Execute the complete reviewed brain boundary while retaining a caller-owned trace of plan,
   * connector, provider, and terminal transitions. The trace never receives transient values.
   */
  async executeWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainTraceOptions): Promise<AutonomousBrainTracedExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeWithTrace options must be an object");
    const prepared = await this.prepare(request, selectBrainSemanticRouting(options.semanticRouting, options.run?.semanticRouting), options.run, options.approveProviderCall);
    return this.executePreparedWithTrace(prepared, options);
  }

  /** Recompile and verify a persisted metadata-only plan before supplying transient task values. */
  async executePlanned(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainExecuteOptions = {}): Promise<AutonomousBrainExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlanned requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain plan does not match the transient request");
    return this.executePrepared(prepared, options);
  }

  /** Rehydrate a reviewed plan, then execute it through the same full traced facade boundary. */
  async executePlannedWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainTraceOptions): Promise<AutonomousBrainTracedExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain traced plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain traced plan does not match the transient request");
    return this.executePreparedWithTrace(prepared, options);
  }

  /** Execute the closed-loop route -> invoke -> evaluate -> learn cycle behind the same plan boundary. */
  async executeCycle(input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    const prepared = await this.prepare(input, options.semanticRouting, options.cycle, options.approveProviderCall);
    return this.executeCyclePrepared(prepared, options);
  }

  /** Rehydrate a persisted brain plan, then run the closed-loop evaluator/learning cycle. */
  async executePlannedCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedCycle requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain cycle plan does not match the transient request");
    return this.executeCyclePrepared(prepared, options);
  }

  /** Execute a closed-loop cycle while tracing planning, connectors, provider turns, evaluation, and learning. */
  async executeCycleWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainCycleTraceOptions): Promise<AutonomousBrainTracedCycleExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeCycleWithTrace options must be an object");
    const prepared = await this.prepare(request, options.semanticRouting, options.cycle, options.approveProviderCall);
    return this.executeCyclePreparedWithTrace(prepared, options);
  }

  /** Rehydrate a reviewed plan, then execute its closed-loop cycle through the trace boundary. */
  async executePlannedCycleWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainCycleTraceOptions): Promise<AutonomousBrainTracedCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedCycleWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain traced cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain traced cycle plan does not match the transient request");
    return this.executeCyclePreparedWithTrace(prepared, options);
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

  /** Rehydrate a persisted metadata-only plan, then run the bounded adaptive loop. */
  async executePlannedAdaptiveCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedAdaptiveCycle requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain adaptive cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain adaptive cycle plan does not match the transient request");
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Execute an evaluator-guided loop while tracing every bounded attempt and learning transition. */
  async executeAdaptiveCycleWithTrace(input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleTraceOptions): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    const request = validateRequest(input);
    if (!options || typeof options !== "object") throw new ArgumentError("autonomous brain executeAdaptiveCycleWithTrace options must be an object");
    const prepared = await this.prepare(request, options.semanticRouting, options.adaptive, options.approveProviderCall);
    return this.executeAdaptiveCyclePreparedWithTrace(prepared, options);
  }

  /** Rehydrate a reviewed plan, then execute its evaluator-guided loop through the trace boundary. */
  async executePlannedAdaptiveCycleWithTrace(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleTraceOptions): Promise<AutonomousBrainTracedAdaptiveCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedAdaptiveCycleWithTrace requires a typed plan");
    const request = validateRequest(input);
    assertBrainPlanTask(plan, request, "autonomous brain traced adaptive cycle plan does not match the transient request");
    const prepared = await this.prepare(request, undefined, undefined, undefined, plan.route, plan.semantic_route);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain traced adaptive cycle plan does not match the transient request");
    return this.executeAdaptiveCyclePreparedWithTrace(prepared, options);
  }

  /** Return the redacted provider/model/tool posture needed to render onboarding UI. */
  async readiness(options: AutonomousBrainReadinessOptions = {}): Promise<AutonomousBrainReadinessReport> {
    return this.agent.readiness(options);
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
   * Compose every provider-free launch gate into one digest-bound, review-only handoff.
   * The projection covers all twelve domains and cannot authorize provider, source, tool,
   * credential, learner, queue, or effect dispatch.
   */
  async launchPreflight(options: AutonomousLaunchPreflightOptions = {}): Promise<AutonomousLaunchPreflightReport> {
    return auditAutonomousBrainLaunchPreflight(this, options);
  }

  /** Project a portfolio-wide admission image before provider/tool/source dispatch. */
  async admitWorkflowPortfolio(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioAdmissionOptions = {},
  ): Promise<AutonomousWorkflowPortfolioAdmission> {
    return this.agent.admitWorkflowPortfolio(requests, options);
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
    const runOptions = {
      ...(options.run ?? {}),
      domain: request.domain,
      capability: options.run?.capability ?? request.capability,
      context: options.run?.context ?? request.context,
    } as AutonomousApprovedModelSelectionOptions;
    const run = await this.agent.runApprovedModelSelection(request.task, preview, runOptions);
    return {
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status: run.status,
      plan: prepared.plan.toJSON(),
      run,
      connector: null,
      error: null,
      retention: "plan_metadata_only;run_and_connector_values_transient_to_caller",
      secret_material: "never_returned",
    };
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
  async saveActivation(store: AutonomousBrainActivationSnapshotStore): Promise<void> {
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
      if (restored.job_id !== jobId || JSON.stringify(restored.request_digests) !== JSON.stringify(requestDigests)) throw new ArgumentError("autonomous brain batch checkpoint does not match the current requests");
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
        ? await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true })
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

  constructor(readonly brain: AutonomousBrainFacade, readonly persistence: AutonomousBrainBatchCheckpointStore) {
    if (!(brain instanceof AutonomousBrainFacade)) throw new ArgumentError("autonomous brain batch controller requires an AutonomousBrainFacade");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous brain batch checkpoint store is malformed");
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
    this.running = true;
    try {
      const batch = await this.brain.executeBatchResumable(inputs, {
        ...options,
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
