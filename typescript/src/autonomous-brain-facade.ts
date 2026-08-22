import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousAgent,
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
  type AutonomousTaskBlueprint,
} from "./autonomous.js";
import {
  AutonomousConnectorOperationFacade,
  AutonomousConnectorOperationPlan,
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
  type AutonomousCrossDomainReplanCycleOptions,
  type AutonomousCrossDomainReplanCycleResult,
  type AutonomousDecisionCycleOptions,
  type AutonomousDecisionCycleResult,
  type AutonomousReplanCycleOptions,
  type AutonomousReplanCycleResult,
} from "./autonomous-cycle.js";
import type { AutonomousCapabilityActivationSnapshotStore } from "./autonomous-activation.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

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
export const AUTONOMOUS_BRAIN_CYCLE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-cycle-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_ADAPTIVE_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-adaptive-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_SUMMARY_SCHEMA = "bioprism-typescript-autonomous-brain-plan-summary/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_BATCH = 64;
export const MAX_AUTONOMOUS_BRAIN_PARALLELISM = 8;
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
  run: AutonomousRunResult | AutonomousCrossDomainRunResult | null;
  connector: AutonomousConnectorOperationExecution | null;
  error: { error_class: string; failure_code: string } | null;
  retention: "plan_metadata_only;run_and_connector_values_transient_to_caller";
  secret_material: "never_returned";
}

export interface AutonomousBrainExecuteOptions {
  /** Explicit provider approval; defaults to false even when a model is registered. */
  approveProviderCall?: boolean;
  /** Run the optional connector operation before invoking the provider; defaults to true. */
  connectorFirst?: boolean;
  /** Include the connector's transient bounded observation in the provider context. */
  includeConnectorObservation?: boolean;
  /** Lower-level provider, tool, memory, learning, and effect controls. */
  run?: Omit<AutonomousRunOptions, "domain" | "routeOverride" | "capability" | "context" | "hints" | "allowCrossDomain">;
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
  /** Evaluator, memory, learning, provider-planning, persistence, and budget controls. */
  cycle?: AutonomousBrainSingleCycleOptions | AutonomousBrainCrossDomainCycleOptions;
}

export type AutonomousBrainCycleResult = AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult;
export type AutonomousBrainCycleStatus = AutonomousBrainCycleResult["status"] | "connector_blocked";

export interface AutonomousBrainCycleExecution {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainCycleStatus;
  plan: AutonomousBrainPlanJSON;
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
  /** Evaluator, bounded replan, learning, persistence, memory, and budget controls. */
  adaptive: AutonomousBrainSingleAdaptiveCycleOptions | AutonomousBrainCrossDomainAdaptiveCycleOptions;
}

export type AutonomousBrainAdaptiveCycleResult = AutonomousReplanCycleResult | AutonomousCrossDomainReplanCycleResult;
export type AutonomousBrainAdaptiveCycleStatus = AutonomousBrainAdaptiveCycleResult["status"] | "connector_blocked";

export interface AutonomousBrainAdaptiveCycleExecution {
  schema: typeof AUTONOMOUS_BRAIN_FACADE_SCHEMA;
  status: AutonomousBrainAdaptiveCycleStatus;
  plan: AutonomousBrainPlanJSON;
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

interface PreparedBrainRequest {
  readonly request: AutonomousBrainRequest;
  readonly route: AutonomousRouteProposal;
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
  return digestJsonSync(items.map((item) => ({ index: item.index, status: item.status, task_digest: item.task_digest, error_class: item.error_class ?? null, failure_code: item.failure_code ?? null, plan_digest: item.execution?.plan.plan_digest ?? null, execution_status: item.execution?.status ?? null })));
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
  readonly domain_plan: AutonomousBrainDomainPlanSummary | null;
  readonly cross_domain_plan: AutonomousBrainCrossDomainPlanSummary | null;
  readonly connector_plan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null;
  readonly selected_domains: AutonomousDomainName[];
  readonly task_digest: string;
  readonly plan_digest: string;

  constructor(input: {
    status: AutonomousBrainPlanStatus;
    route: AutonomousRouteProposal;
    domain_plan: AutonomousBrainDomainPlanSummary | null;
    cross_domain_plan: AutonomousBrainCrossDomainPlanSummary | null;
    connector_plan: ReturnType<AutonomousConnectorOperationPlan["toJSON"]> | null;
  }) {
    if (input.status !== "ready" && input.status !== "route_review_required" && input.status !== "connector_review_required") throw new ArgumentError("autonomous brain plan status is invalid");
    if (!isObject(input.route) || typeof input.route.route_digest !== "string") throw new ArgumentError("autonomous brain plan route is malformed");
    this.status = input.status;
    this.route = structuredClone(input.route);
    this.domain_plan = input.domain_plan === null ? null : structuredClone(input.domain_plan);
    this.cross_domain_plan = input.cross_domain_plan === null ? null : structuredClone(input.cross_domain_plan);
    this.connector_plan = input.connector_plan === null ? null : structuredClone(input.connector_plan);
    this.selected_domains = [...this.route.selected_domains];
    this.task_digest = digest("autonomous brain plan task_digest", this.route.task_digest);
    this.plan_digest = digestJsonSync(this.descriptor());
  }

  private descriptor(): Omit<AutonomousBrainPlanJSON, "plan_digest"> {
    return {
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status: this.status,
      route: structuredClone(this.route),
      domain_plan: this.domain_plan === null ? null : structuredClone(this.domain_plan),
      cross_domain_plan: this.cross_domain_plan === null ? null : structuredClone(this.cross_domain_plan),
      connector_plan: this.connector_plan === null ? null : structuredClone(this.connector_plan),
      selected_domains: [...this.selected_domains],
      task_digest: this.task_digest,
      retention: PLAN_RETENTION,
      secret_material: "never_returned",
    };
  }

  toJSON(): AutonomousBrainPlanJSON {
    return { ...this.descriptor(), plan_digest: this.plan_digest };
  }

  static fromJSON(value: unknown): AutonomousBrainPlan {
    if (!isObject(value) || value.schema !== AUTONOMOUS_BRAIN_FACADE_SCHEMA || value.retention !== PLAN_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous brain plan is malformed");
    const plan = new AutonomousBrainPlan({
      status: value.status as AutonomousBrainPlanStatus,
      route: value.route as AutonomousRouteProposal,
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

  constructor(options: { agent: AutonomousAgent; connectorOperations?: AutonomousConnectorOperationFacade }) {
    if (!options || !options.agent || typeof options.agent.route !== "function" || typeof options.agent.blueprint !== "function" || typeof options.agent.run !== "function" || typeof options.agent.runCrossDomain !== "function" || typeof options.agent.readiness !== "function" || typeof options.agent.refreshActivation !== "function") throw new ArgumentError("autonomous brain facade requires an AutonomousAgent");
    if (options.connectorOperations !== undefined && !(options.connectorOperations instanceof AutonomousConnectorOperationFacade)) throw new ArgumentError("autonomous brain connectorOperations is invalid");
    this.agent = options.agent;
    this.connectorOperations = options.connectorOperations;
  }

  /** Compile routing and workflow metadata without contacting a provider or connector. */
  async plan(input: AutonomousBrainRequest): Promise<AutonomousBrainPlan> {
    const request = validateRequest(input);
    const route = await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true });
    let domainPlan: AutonomousBrainDomainPlanSummary | null = null;
    let crossDomainPlan: AutonomousBrainCrossDomainPlanSummary | null = null;
    if (!route.abstained && route.primary_domain !== null) {
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
    return new AutonomousBrainPlan({ status, route, domain_plan: domainPlan, cross_domain_plan: crossDomainPlan, connector_plan: connectorPlan });
  }

  /** Execute a fresh request after compiling its request-free plan. */
  async execute(input: AutonomousBrainRequest, options: AutonomousBrainExecuteOptions = {}): Promise<AutonomousBrainExecution> {
    const prepared = await this.prepare(input);
    return this.executePrepared(prepared, options);
  }

  /** Recompile and verify a persisted metadata-only plan before supplying transient task values. */
  async executePlanned(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainExecuteOptions = {}): Promise<AutonomousBrainExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlanned requires a typed plan");
    const prepared = await this.prepare(input);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain plan does not match the transient request");
    return this.executePrepared(prepared, options);
  }

  /** Execute the closed-loop route -> invoke -> evaluate -> learn cycle behind the same plan boundary. */
  async executeCycle(input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    const prepared = await this.prepare(input);
    return this.executeCyclePrepared(prepared, options);
  }

  /** Rehydrate a persisted brain plan, then run the closed-loop evaluator/learning cycle. */
  async executePlannedCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainCycleOptions = {}): Promise<AutonomousBrainCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedCycle requires a typed plan");
    const prepared = await this.prepare(input);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain cycle plan does not match the transient request");
    return this.executeCyclePrepared(prepared, options);
  }

  /**
   * Execute the bounded evaluator -> learn -> optional replan loop behind the same route,
   * connector, approval, and metadata-only plan boundary. Replanning is always delegated to
   * the lower-level capped loop, so evaluator feedback cannot silently widen authority.
   */
  async executeAdaptiveCycle(input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    const prepared = await this.prepare(input);
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Rehydrate a persisted metadata-only plan, then run the bounded adaptive loop. */
  async executePlannedAdaptiveCycle(plan: AutonomousBrainPlan, input: AutonomousBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (!(plan instanceof AutonomousBrainPlan)) throw new ArgumentError("autonomous brain executePlannedAdaptiveCycle requires a typed plan");
    const prepared = await this.prepare(input);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous brain adaptive cycle plan does not match the transient request");
    return this.executeAdaptiveCyclePrepared(prepared, options);
  }

  /** Return the redacted provider/model/tool posture needed to render onboarding UI. */
  async readiness(options: AutonomousBrainReadinessOptions = {}): Promise<AutonomousBrainReadinessReport> {
    return this.agent.readiness(options);
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

  private async prepare(input: AutonomousBrainRequest): Promise<PreparedBrainRequest> {
    const request = validateRequest(input);
    const route = await this.agent.route(request.task, { domain: request.domain, hints: request.hints, allowCrossDomain: request.allow_cross_domain ?? true });
    const plan = await this.plan(request);
    let connectorPlan: AutonomousConnectorOperationPlan | null = null;
    if (request.connector !== undefined) {
      if (!this.connectorOperations) throw new ArgumentError("autonomous brain connector input requires connectorOperations");
      connectorPlan = this.connectorOperations.plan(request.connector);
    }
    // Requiring the route digest to agree here catches an accidental route recomputation change
    // between plan construction and the returned prepared request without retaining task text.
    if (plan.route.route_digest !== route.route_digest) throw new ProviderRuntimeError("autonomous brain route changed while preparing execution", { code: "configuration" });
    return { request, route, plan, connectorPlan };
  }

  private async executePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainExecuteOptions): Promise<AutonomousBrainExecution> {
    const { request, route, plan } = prepared;
    if (plan.status === "route_review_required") return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "route_review_required", plan: plan.toJSON(), run: null, connector: null, error: null, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    if (plan.status === "connector_review_required" || (prepared.connectorPlan && prepared.connectorPlan.status !== "ready")) return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "connector_blocked", plan: plan.toJSON(), run: null, connector: null, error: { error_class: "ConnectorOperationError", failure_code: "configuration" }, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    let connector: AutonomousConnectorOperationExecution | null = null;
    if (request.connector !== undefined && options.connectorFirst !== false) {
      if (!this.connectorOperations || !prepared.connectorPlan) throw new ArgumentError("autonomous brain connector plan is unavailable");
      connector = await this.connectorOperations.executePlanned(prepared.connectorPlan, request.connector);
      if (!connectorSucceeded(connector.status)) return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: "connector_blocked", plan: plan.toJSON(), run: null, connector, error: { error_class: "ConnectorOperationError", failure_code: connector.status }, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const approved = options.approveProviderCall ?? options.run?.approveProviderCall ?? false;
    const runOptions = { ...(options.run ?? {}), routeOverride: route, capability: request.capability, context, hints: request.hints, allowCrossDomain: request.allow_cross_domain, approveProviderCall: approved } as AutonomousRunOptions;
    const run = route.cross_domain
      ? await this.agent.runCrossDomain(request.task, runOptions as AutonomousCrossDomainRunOptions)
      : await this.agent.run(request.task, { ...runOptions, domain: route.primary_domain ?? undefined });
    return { schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA, status: run.status, plan: plan.toJSON(), run, connector, error: null, retention: "plan_metadata_only;run_and_connector_values_transient_to_caller", secret_material: "never_returned" };
  }

  private async executeCyclePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainCycleOptions): Promise<AutonomousBrainCycleExecution> {
    const { request, route, plan } = prepared;
    const base = (status: AutonomousBrainCycleStatus, cycle: AutonomousBrainCycleResult | null, connector: AutonomousConnectorOperationExecution | null, error: { error_class: string; failure_code: string } | null): AutonomousBrainCycleExecution => ({
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status,
      plan: plan.toJSON(),
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
      connector = await this.connectorOperations.executePlanned(prepared.connectorPlan, request.connector);
      if (!connectorSucceeded(connector.status)) return base("connector_blocked", null, connector, { error_class: "ConnectorOperationError", failure_code: connector.status });
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const cycleOptions = {
      ...(options.cycle ?? {}),
      routeOverride: route,
      domain: route.primary_domain ?? undefined,
      capability: request.capability,
      context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
      approveProviderCall: options.approveProviderCall ?? options.cycle?.approveProviderCall ?? false,
    };
    const cycle = route.cross_domain
      ? await runAutonomousCrossDomainDecisionCycle(this.agent, request.task, cycleOptions as AutonomousCrossDomainDecisionCycleOptions)
      : await runAutonomousDecisionCycle(this.agent, request.task, cycleOptions as AutonomousDecisionCycleOptions);
    return base(cycle.status, cycle, connector, null);
  }

  private async executeAdaptiveCyclePrepared(prepared: PreparedBrainRequest, options: AutonomousBrainAdaptiveCycleOptions): Promise<AutonomousBrainAdaptiveCycleExecution> {
    if (!options || !isObject(options.adaptive) || typeof options.adaptive.evaluate !== "function") throw new ArgumentError("autonomous brain adaptive cycle requires an evaluator callback");
    const { request, route, plan } = prepared;
    const base = (status: AutonomousBrainAdaptiveCycleStatus, adaptive: AutonomousBrainAdaptiveCycleResult | null, connector: AutonomousConnectorOperationExecution | null, error: { error_class: string; failure_code: string } | null): AutonomousBrainAdaptiveCycleExecution => ({
      schema: AUTONOMOUS_BRAIN_FACADE_SCHEMA,
      status,
      plan: plan.toJSON(),
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
      connector = await this.connectorOperations.executePlanned(prepared.connectorPlan, request.connector);
      if (!connectorSucceeded(connector.status)) return base("connector_blocked", null, connector, { error_class: "ConnectorOperationError", failure_code: connector.status });
    }
    const context = [
      ...(request.context ?? []),
      ...(connector && options.includeConnectorObservation !== false ? [observationChunk(connector)] : []),
    ];
    const adaptiveOptions = {
      ...(options.adaptive ?? {}),
      routeOverride: route,
      domain: route.primary_domain ?? undefined,
      capability: request.capability,
      context,
      hints: request.hints,
      allowCrossDomain: request.allow_cross_domain,
      approveProviderCall: options.approveProviderCall ?? options.adaptive.approveProviderCall ?? false,
    };
    const adaptive = route.cross_domain
      ? await runAutonomousCrossDomainReplanCycle(this.agent, request.task, adaptiveOptions as AutonomousCrossDomainReplanCycleOptions)
      : await runAutonomousReplanCycle(this.agent, request.task, adaptiveOptions as AutonomousReplanCycleOptions);
    return base(adaptive.status, adaptive, connector, null);
  }
}

export function createAutonomousBrainFacade(options: { agent: AutonomousAgent; connectorOperations?: AutonomousConnectorOperationFacade }): AutonomousBrainFacade {
  return new AutonomousBrainFacade(options);
}
