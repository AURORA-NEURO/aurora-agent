import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousAgent,
  type AutonomousDomainName,
  type AutonomousPromptChunk,
  type AutonomousRunOptions,
  type AutonomousRunResult,
} from "./autonomous.js";
import {
  MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS,
  planAutonomousWorkflowPortfolio,
  validateAutonomousWorkflowPortfolioPlan,
  verifyAutonomousWorkflowPortfolio,
  type AutonomousWorkflowPortfolioItemRequest,
  type AutonomousWorkflowPortfolioPlan,
  type AutonomousWorkflowPortfolioPlanOptions,
} from "./autonomous-workflow-portfolio.js";
import type {
  AutonomousEvaluatorRewardInput,
  AutonomousLearningController,
  AutonomousLearningOutboxSettlementOptions,
} from "./autonomous-learning.js";
import { validateAutonomousWorkflowPortfolioAdmission, type AutonomousWorkflowPortfolioAdmission } from "./autonomous-workflow-portfolio-admission.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Digest-bound execution result for a portfolio whose plan was reviewed separately. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-execution/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM = 8;
export const DEFAULT_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES = 16_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES = 64_000;

export type AutonomousWorkflowPortfolioExecutionItemStatus =
  | "succeeded"
  | "failed"
  | "blocked"
  | "approval_required"
  | "route_review_required"
  | "reconciliation_required"
  | "turn_limit_reached"
  | "child_failed"
  | "omitted";

export type AutonomousWorkflowPortfolioExecutionStatus = "completed" | "partial" | "failed" | "approval_required" | "blocked";

/** Metadata-only lifecycle phases for one dependency-aware portfolio execution. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-execution-trace/0.1" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-event/0.1" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENTS = 100_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_BYTES = 16_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES = 5_000_000;
export type AutonomousWorkflowPortfolioExecutionTracePhase = "started" | "plan_verified" | "item_started" | "item_decided" | "progress" | "completed" | "paused" | "failed" | "blocked";
export type AutonomousWorkflowPortfolioExecutionTraceStatus = AutonomousWorkflowPortfolioExecutionStatus | AutonomousWorkflowPortfolioExecutionItemStatus | "running";

export interface AutonomousWorkflowPortfolioExecutionTraceEvent extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_SCHEMA;
  trace_id: string;
  sequence: number;
  plan_digest: string;
  admission_digest: string | null;
  phase: AutonomousWorkflowPortfolioExecutionTracePhase;
  status: AutonomousWorkflowPortfolioExecutionTraceStatus;
  item_id: string | null;
  domain: AutonomousDomainName | null;
  wave_index: number | null;
  provider_call: AutonomousWorkflowPortfolioExecutionItemJSON["provider_call"] | null;
  result_digest: string | null;
  failure_code: string | null;
  learning_status: AutonomousWorkflowPortfolioLearningStatus | null;
  detail_digest: string | null;
  previous_digest: string;
  event_digest: string;
  retention: "metadata_only_no_tasks_prompts_outputs_credentials_or_evidence";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionTraceEventInput {
  phase: AutonomousWorkflowPortfolioExecutionTracePhase;
  status: AutonomousWorkflowPortfolioExecutionTraceStatus;
  item_id?: string | null;
  domain?: AutonomousDomainName | null;
  wave_index?: number | null;
  provider_call?: AutonomousWorkflowPortfolioExecutionItemJSON["provider_call"] | null;
  result_digest?: string | null;
  failure_code?: string | null;
  learning_status?: AutonomousWorkflowPortfolioLearningStatus | null;
  detail_digest?: string | null;
}

export type AutonomousWorkflowPortfolioExecutionTraceSink = (event: AutonomousWorkflowPortfolioExecutionTraceEvent) => Promise<void> | void;

export interface AutonomousWorkflowPortfolioExecutionTraceEmitter {
  emit(input: AutonomousWorkflowPortfolioExecutionTraceEventInput): Promise<AutonomousWorkflowPortfolioExecutionTraceEvent>;
  flush(): Promise<void>;
  readonly traceId: string;
  readonly headDigest: string;
}

export interface AutonomousWorkflowPortfolioExecutionTraceSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_SCHEMA;
  trace_id: string;
  plan_digest: string;
  admission_digest: string | null;
  sequence: number;
  head_digest: string;
  events: AutonomousWorkflowPortfolioExecutionTraceEvent[];
  snapshot_digest: string;
  retention: "metadata_only_hash_chained_no_tasks_prompts_outputs_credentials_or_evidence";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionTracePersistence {
  read(): Promise<AutonomousWorkflowPortfolioExecutionTraceSnapshot | null> | AutonomousWorkflowPortfolioExecutionTraceSnapshot | null;
  write(snapshot: AutonomousWorkflowPortfolioExecutionTraceSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioExecutionTraceSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioExecutionTraceTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioExecutionTraceTransactionalTextStore extends AutonomousWorkflowPortfolioExecutionTraceTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

/** Explicit state of the optional evaluator-to-bandit handoff for one portfolio item. */
export type AutonomousWorkflowPortfolioLearningStatus =
  | "disabled"
  | "not_eligible"
  | "pending_evaluation"
  | "preparation_failed"
  | "evaluation_failed"
  | "settled"
  | "settlement_failed";

/** Shared provider/learning controls for each item; portfolio identity controls are not caller-overridable. */
export type AutonomousWorkflowPortfolioRunOptions = Omit<
  AutonomousRunOptions,
  "domain" | "routeOverride" | "capability" | "workflowContext" | "allowCrossDomain"
>;

export interface AutonomousWorkflowPortfolioExecutionOptions {
  /** Reuse a previously reviewed metadata-only plan. When omitted, a provider-free plan is compiled. */
  plan?: AutonomousWorkflowPortfolioPlan;
  /** Optional provider-free admission image. When present, only its eligible items may dispatch. */
  admission?: AutonomousWorkflowPortfolioAdmission;
  planOptions?: AutonomousWorkflowPortfolioPlanOptions;
  /** Recompute the provider-free plan identity before any provider or tool dispatch; defaults to true. */
  verifyPlan?: boolean;
  /** Controls shared by every item, including candidates, credentialFor, memory, learning, tools, and budgets. */
  run?: AutonomousWorkflowPortfolioRunOptions;
  /** Optional controller used to settle explicit evaluator rewards for completed portfolio items. */
  learning?: AutonomousLearningController;
  /** Digest of the caller-owned evaluator policy; binds resumable feedback to one evaluator contract. */
  learningPolicyDigest?: string;
  /** Build one explicit evaluator reward packet from a completed item; null defers settlement. */
  evaluateItem?: (context: AutonomousWorkflowPortfolioLearningEvaluationContext) => AutonomousEvaluatorRewardInput | null | Promise<AutonomousEvaluatorRewardInput | null>;
  /** Direct, remote, and restart-safe outbox controls for item settlement. */
  learningSettlement?: AutonomousWorkflowPortfolioLearningSettlementOptions;
  /** Explicit approval for every provider invocation; defaults to false. */
  approveProviderCall?: boolean;
  /** Maximum number of same-wave item runs in flight. */
  maxParallelism?: number;
  /** Stop dispatching later dependency waves after a hard item failure. */
  stopOnError?: boolean;
  /** Include transient predecessor summaries in child prompts. Defaults to true. */
  includeDependencyOutputs?: boolean;
  /** Aggregate byte budget for direct predecessor prompt handoffs. */
  maxDependencyHandoffBytes?: number;
  /** Caller-owned metadata sink for the portfolio decision trace; no transient values are emitted. */
  traceSink?: AutonomousWorkflowPortfolioExecutionTraceSink;
  /** Stable caller-owned trace identity, required when traceSink is supplied. */
  traceId?: string;
}

export interface AutonomousWorkflowPortfolioLearningEvaluationContext {
  itemId: string;
  domain: AutonomousDomainName;
  request: AutonomousWorkflowPortfolioItemRequest;
  planItem: AutonomousWorkflowPortfolioPlan["items"][number];
  run: AutonomousRunResult;
  outputText: string;
  outputDigest: string | null;
}

export interface AutonomousWorkflowPortfolioLearningSettlementOptions {
  remote?: boolean;
  outbox?: AutonomousLearningOutboxSettlementOptions;
}

/** Internal resume hook used by the metadata-only durable portfolio controller. */
export interface AutonomousWorkflowPortfolioExecutionProgress {
  plan: AutonomousWorkflowPortfolioPlan;
  items: readonly AutonomousWorkflowPortfolioItemExecutionResult[];
  status: AutonomousWorkflowPortfolioExecutionStatus;
}

export type AutonomousWorkflowPortfolioExecutionProgressSink = (
  progress: AutonomousWorkflowPortfolioExecutionProgress,
) => Promise<void> | void;

export interface AutonomousWorkflowPortfolioExecutionItemJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA;
  item_id: string;
  domain: AutonomousDomainName;
  depends_on: string[];
  status: AutonomousWorkflowPortfolioExecutionItemStatus;
  run_status: AutonomousRunResult["status"] | null;
  provider_call: "not_started" | "approval_required" | "may_have_started";
  dependency_statuses: Record<string, AutonomousWorkflowPortfolioExecutionItemStatus>;
  output_digest: string | null;
  output_bytes: number;
  error_class: string | null;
  failure_code: string | null;
  learning_status: AutonomousWorkflowPortfolioLearningStatus;
  learning_episode_id: string | null;
  evaluation_digest: string | null;
  settlement_digest: string | null;
  learning_error_class: string | null;
  retention: "metadata_only_task_and_provider_output_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA;
  status: AutonomousWorkflowPortfolioExecutionStatus;
  plan_digest: string;
  admission_digest: string | null;
  trace_digest: string | null;
  execution_digest: string;
  wave_count: number;
  completed_count: number;
  failed_count: number;
  blocked_count: number;
  approval_required_count: number;
  omitted_count: number;
  learning_settled_count: number;
  learning_pending_count: number;
  learning_not_eligible_count: number;
  learning_failed_count: number;
  items: AutonomousWorkflowPortfolioExecutionItemJSON[];
  execution: "provider_and_tool_calls_are_per_item_approved;outputs_transient_to_caller";
  authorization: "portfolio_plan_verification_does_not_authorize_provider_or_effects";
  retention: "metadata_only_task_and_provider_output_not_retained";
  secret_material: "never_returned";
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedInteger(label: string, value: unknown, minimum: number, maximum: number, fallback: number): number {
  if (value === undefined) return fallback;
  const numeric = typeof value === "number" ? value : Number.NaN;
  if (!Number.isSafeInteger(numeric) || numeric < minimum || numeric > maximum) throw new ArgumentError(`${label} must be an integer in [${minimum}, ${maximum}]`);
  return numeric;
}

function truncateBytes(value: string, maximum: number): string {
  if (bytes(value) <= maximum) return value;
  let truncated = value.slice(0, Math.max(0, maximum - 32));
  while (truncated && bytes(`${truncated}\n[handoff truncated]`) > maximum) truncated = truncated.slice(0, -1);
  return `${truncated}\n[handoff truncated]`;
}

export function autonomousWorkflowPortfolioTransientOutput(run: AutonomousRunResult): { text: string; bytes: number } {
  let text = "";
  if (run.response?.text) return { text: run.response.text, bytes: bytes(run.response.text) };
  if (run.response?.structured === null || run.response?.structured === undefined) return { text, bytes: 0 };
  try {
    text = JSON.stringify(run.response.structured) ?? "";
  } catch {
    text = "";
  }
  return { text, bytes: bytes(text) };
}

export async function digestAutonomousWorkflowPortfolioExecutionItem(item: AutonomousWorkflowPortfolioItemExecutionResult): Promise<string> {
  return digestJson({
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
    item_id: item.itemId,
    domain: item.domain,
    depends_on: [...item.dependsOn],
    status: item.status,
    run_status: item.run?.status ?? null,
    provider_call: item.run ? providerCallStatus(item.run) : "not_started",
    output_digest: item.outputDigest,
    output_bytes: item.outputBytes,
    error_class: item.errorClass,
    failure_code: item.failureCode,
    learning_status: item.learningStatus,
    learning_episode_id: item.learningEpisodeId,
    evaluation_digest: item.evaluationDigest,
    settlement_digest: item.settlementDigest,
    learning_error_class: item.learningErrorClass,
  });
}

function errorMetadata(error: unknown): { errorClass: string; failureCode: string } {
  const record = isObject(error) ? error : null;
  const errorClass = error instanceof Error && error.constructor.name ? error.constructor.name : "portfolio_item_execution_failed";
  const failureCode = record && typeof record.code === "string" && record.code.length <= 128 ? record.code : "portfolio_item_execution_failed";
  return { errorClass, failureCode };
}

function executionStatusForRun(run: AutonomousRunResult): AutonomousWorkflowPortfolioExecutionItemStatus {
  switch (run.status) {
    case "completed": return "succeeded";
    case "approval_required": return "approval_required";
    case "route_review_required":
    case "abstained": return "route_review_required";
    case "reconciliation_required": return "reconciliation_required";
    case "turn_limit_reached": return "turn_limit_reached";
    case "child_failed":
    case "cross_domain_partial": return "child_failed";
    default: return "failed";
  }
}

function providerCallStatus(run: AutonomousRunResult): AutonomousWorkflowPortfolioExecutionItemJSON["provider_call"] {
  if (run.status === "approval_required") return "approval_required";
  return run.response || run.selection ? "may_have_started" : "not_started";
}

export function isAutonomousWorkflowPortfolioHardFailure(status: AutonomousWorkflowPortfolioExecutionItemStatus): boolean {
  return status === "failed" || status === "route_review_required" || status === "reconciliation_required" || status === "turn_limit_reached" || status === "child_failed";
}

const hardFailure = isAutonomousWorkflowPortfolioHardFailure;

function executionCounts(items: readonly AutonomousWorkflowPortfolioItemExecutionResult[]): {
  completed: number;
  failed: number;
  blocked: number;
  approvalRequired: number;
  omitted: number;
  learningSettled: number;
  learningPending: number;
  learningNotEligible: number;
  learningFailed: number;
} {
  return {
    completed: items.filter((item) => item.status === "succeeded").length,
    failed: items.filter((item) => hardFailure(item.status)).length,
    blocked: items.filter((item) => item.status === "blocked").length,
    approvalRequired: items.filter((item) => item.status === "approval_required").length,
    omitted: items.filter((item) => item.status === "omitted").length,
    learningSettled: items.filter((item) => item.learningStatus === "settled").length,
    learningPending: items.filter((item) => item.learningStatus === "pending_evaluation").length,
    learningNotEligible: items.filter((item) => item.learningStatus === "not_eligible").length,
    learningFailed: items.filter((item) => item.learningStatus === "preparation_failed" || item.learningStatus === "evaluation_failed" || item.learningStatus === "settlement_failed").length,
  };
}

function learningControllerFor(options: AutonomousWorkflowPortfolioExecutionOptions): AutonomousLearningController | undefined {
  return options.learning ?? options.run?.learning;
}

function learningPolicyDigestFor(options: AutonomousWorkflowPortfolioExecutionOptions): string | null {
  if (options.learningPolicyDigest === undefined) return null;
  if (typeof options.learningPolicyDigest !== "string" || !/^[0-9a-f]{64}$/.test(options.learningPolicyDigest)) throw new ArgumentError("workflow portfolio learningPolicyDigest must be a lowercase SHA-256 digest");
  return options.learningPolicyDigest;
}

function learningIncomplete(status: AutonomousWorkflowPortfolioLearningStatus): boolean {
  return status === "pending_evaluation" || status === "preparation_failed" || status === "evaluation_failed" || status === "settlement_failed";
}

function dependencyStatuses(item: AutonomousWorkflowPortfolioItemExecutionResult, results: ReadonlyMap<string, AutonomousWorkflowPortfolioItemExecutionResult>): Record<string, AutonomousWorkflowPortfolioExecutionItemStatus> {
  return Object.fromEntries(item.dependsOn.map((dependency) => [dependency, results.get(dependency)?.status ?? "blocked"]));
}

function runOptionsFor(
  request: AutonomousWorkflowPortfolioItemRequest,
  itemId: string,
  planDigest: string,
  options: AutonomousWorkflowPortfolioExecutionOptions,
  context: readonly AutonomousPromptChunk[],
): AutonomousRunOptions {
  const base = options.run ?? {};
  const approval = options.approveProviderCall ?? base.approveProviderCall ?? false;
  const learning = options.learning ?? base.learning;
  const learningEpisodeId = base.learningEpisodeId
    ? `${base.learningEpisodeId}:${itemId}`
    : learning
      ? `portfolio:${planDigest.slice(0, 24)}:${itemId}`
      : undefined;
  return {
    ...base,
    domain: request.domain,
    capability: request.capability,
    context,
    hints: request.hints ?? [],
    allowCrossDomain: false,
    approveProviderCall: approval,
    ...(learning ? { learning } : {}),
    ...(base.memoryRunId ? { memoryRunId: `${base.memoryRunId}:${itemId}` } : {}),
    ...(learningEpisodeId ? { learningEpisodeId } : {}),
  };
}

function dependencyHandoffs(
  item: AutonomousWorkflowPortfolioItemExecutionResult,
  results: ReadonlyMap<string, AutonomousWorkflowPortfolioItemExecutionResult>,
  maximumBytes: number,
): AutonomousPromptChunk[] {
  if (!item.includeDependencyOutputs || item.dependsOn.length === 0) return [];
  const perDependency = Math.max(256, Math.floor(maximumBytes / item.dependsOn.length));
  return item.dependsOn.flatMap((dependency) => {
    const predecessor = results.get(dependency);
    if (!predecessor) return [];
    const output = predecessor.transientOutputText;
    const content = truncateBytes(JSON.stringify({
      dependency_item_id: dependency,
      status: predecessor.status,
      output_digest: predecessor.outputDigest,
      output: output || null,
      retention: "transient_prompt_handoff_only",
    }), perDependency);
    return [{ id: `portfolio-dependency-${dependency}`, content, required: true, priority: 90 }];
  });
}

type AutonomousWorkflowPortfolioLearningProjection = {
  status: AutonomousWorkflowPortfolioLearningStatus;
  episodeId: string | null;
  evaluationDigest: string | null;
  settlementDigest: string | null;
  errorClass: string | null;
};

function noLearningProjection(status: AutonomousWorkflowPortfolioLearningStatus = "disabled"): AutonomousWorkflowPortfolioLearningProjection {
  return { status, episodeId: null, evaluationDigest: null, settlementDigest: null, errorClass: null };
}

function learningSettlementKey(planDigest: string, itemId: string): string {
  return `portfolio:${planDigest.slice(0, 24)}:${itemId}`;
}

async function settlePortfolioItemLearning(
  options: AutonomousWorkflowPortfolioExecutionOptions,
  planDigest: string,
  request: AutonomousWorkflowPortfolioItemRequest,
  planItem: AutonomousWorkflowPortfolioPlan["items"][number],
  itemId: string,
  run: AutonomousRunResult,
  outputText: string,
  outputDigest: string | null,
): Promise<AutonomousWorkflowPortfolioLearningProjection> {
  const learning = learningControllerFor(options);
  if (!learning) return noLearningProjection();
  const episodeId = run.learning_episode_id ?? null;
  if (run.learning_episode_status === "failed") {
    return { ...noLearningProjection("preparation_failed"), errorClass: run.learning_error_class ?? "learning_episode_preparation_failed" };
  }
  if (run.status !== "completed" || run.learning_episode_status !== "prepared" || !episodeId) return noLearningProjection("not_eligible");
  if (!options.evaluateItem) return { ...noLearningProjection("pending_evaluation"), episodeId };

  let reward: AutonomousEvaluatorRewardInput | null;
  try {
    reward = await options.evaluateItem({ itemId, domain: planItem.domain, request, planItem, run, outputText, outputDigest });
    if (reward === null) return { ...noLearningProjection("pending_evaluation"), episodeId };
    if (!isObject(reward)) throw new ArgumentError("portfolio evaluator returned a malformed reward packet");
  } catch (error) {
    return { ...noLearningProjection("evaluation_failed"), episodeId, errorClass: errorMetadata(error).errorClass };
  }

  try {
    const settlement = await learning.settleRun(episodeId, reward, {
      idempotencyKey: learningSettlementKey(planDigest, itemId),
      remote: options.learningSettlement?.remote,
      outbox: options.learningSettlement?.outbox,
    });
    return {
      status: "settled",
      episodeId,
      evaluationDigest: await digestJson(settlement.assessment),
      settlementDigest: settlement.episode.settlement?.settlement_digest ?? null,
      errorClass: null,
    };
  } catch (error) {
    return { ...noLearningProjection("settlement_failed"), episodeId, errorClass: errorMetadata(error).errorClass };
  }
}

async function applyRehydratedLearningSettlement(
  item: AutonomousWorkflowPortfolioItemExecutionResult,
  options: AutonomousWorkflowPortfolioExecutionOptions,
  planDigest: string,
  request: AutonomousWorkflowPortfolioItemRequest | undefined,
  planItem: AutonomousWorkflowPortfolioPlan["items"][number],
): Promise<AutonomousWorkflowPortfolioItemExecutionResult> {
  if (!request || !item.run || !["pending_evaluation", "evaluation_failed", "settlement_failed"].includes(item.learningStatus)) return item;
  const projection = await settlePortfolioItemLearning(options, planDigest, request, planItem, item.itemId, item.run, item.transientOutputText, item.outputDigest);
  if (projection.status === item.learningStatus && projection.episodeId === item.learningEpisodeId && projection.evaluationDigest === item.evaluationDigest && projection.settlementDigest === item.settlementDigest && projection.errorClass === item.learningErrorClass) return item;
  return new AutonomousWorkflowPortfolioItemExecutionResult(
    item.itemId,
    item.domain,
    [...item.dependsOn],
    item.status,
    item.run,
    item.outputDigest,
    item.outputBytes,
    item.errorClass,
    item.failureCode,
    item.includeDependencyOutputs,
    item.transientOutputText,
    projection.status,
    projection.episodeId,
    projection.evaluationDigest,
    projection.settlementDigest,
    projection.errorClass,
  );
}

/** Transient execution record. Its JSON projection intentionally excludes the run and output. */
export class AutonomousWorkflowPortfolioItemExecutionResult {
  readonly schema = AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA;

  constructor(
    readonly itemId: string,
    readonly domain: AutonomousDomainName,
    readonly dependsOn: string[],
    readonly status: AutonomousWorkflowPortfolioExecutionItemStatus,
    readonly run: AutonomousRunResult | null,
    readonly outputDigest: string | null,
    readonly outputBytes: number,
    readonly errorClass: string | null,
    readonly failureCode: string | null,
    readonly includeDependencyOutputs = false,
    readonly transientOutputText: string = "",
    readonly learningStatus: AutonomousWorkflowPortfolioLearningStatus = "disabled",
    readonly learningEpisodeId: string | null = null,
    readonly evaluationDigest: string | null = null,
    readonly settlementDigest: string | null = null,
    readonly learningErrorClass: string | null = null,
  ) {}

  toJSON(results?: ReadonlyMap<string, AutonomousWorkflowPortfolioItemExecutionResult>): AutonomousWorkflowPortfolioExecutionItemJSON {
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
      item_id: this.itemId,
      domain: this.domain,
      depends_on: [...this.dependsOn],
      status: this.status,
      run_status: this.run?.status ?? null,
      provider_call: this.run ? providerCallStatus(this.run) : "not_started",
      dependency_statuses: dependencyStatuses(this, results ?? new Map()),
      output_digest: this.outputDigest,
      output_bytes: this.outputBytes,
      error_class: this.errorClass,
      failure_code: this.failureCode,
      learning_status: this.learningStatus,
      learning_episode_id: this.learningEpisodeId,
      evaluation_digest: this.evaluationDigest,
      settlement_digest: this.settlementDigest,
      learning_error_class: this.learningErrorClass,
      retention: "metadata_only_task_and_provider_output_not_retained",
      secret_material: "never_returned",
    };
  }
}

/** Caller-visible result; raw provider output remains available only through transient run objects. */
export class AutonomousWorkflowPortfolioExecutionResult {
  constructor(
    readonly plan: AutonomousWorkflowPortfolioPlan,
    readonly status: AutonomousWorkflowPortfolioExecutionStatus,
    readonly items: AutonomousWorkflowPortfolioItemExecutionResult[],
    readonly waveCount: number,
    readonly executionDigest: string,
    readonly admissionDigest: string | null = null,
    readonly traceDigest: string | null = null,
  ) {}

  toJSON(): AutonomousWorkflowPortfolioExecutionJSON {
    const itemJson = this.items.map((item) => item.toJSON(new Map(this.items.map((candidate) => [candidate.itemId, candidate]))));
    const counts = executionCounts(this.items);
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
      status: this.status,
      plan_digest: this.plan.portfolio_digest,
      admission_digest: this.admissionDigest,
      trace_digest: this.traceDigest,
      execution_digest: this.executionDigest,
      wave_count: this.waveCount,
      completed_count: counts.completed,
      failed_count: counts.failed,
      blocked_count: counts.blocked,
      approval_required_count: counts.approvalRequired,
    omitted_count: counts.omitted,
      learning_settled_count: counts.learningSettled,
      learning_pending_count: counts.learningPending,
      learning_not_eligible_count: counts.learningNotEligible,
      learning_failed_count: counts.learningFailed,
      items: itemJson,
      execution: "provider_and_tool_calls_are_per_item_approved;outputs_transient_to_caller",
      authorization: "portfolio_plan_verification_does_not_authorize_provider_or_effects",
      retention: "metadata_only_task_and_provider_output_not_retained",
      secret_material: "never_returned",
    };
  }
}

function statusForPlanItem(item: AutonomousWorkflowPortfolioPlan["items"][number]): AutonomousWorkflowPortfolioExecutionItemStatus {
  if (item.status === "ready") return "blocked";
  if (item.status === "route_review_required") return "route_review_required";
  if (item.status === "failed") return "failed";
  return "blocked";
}

async function runBounded<T>(values: readonly string[], maxParallelism: number, run: (value: string) => Promise<T>): Promise<T[]> {
  const output = new Map<string, T>();
  let cursor = 0;
  const worker = async (): Promise<void> => {
    while (cursor < values.length) {
      const index = cursor++;
      const value = values[index]!;
      output.set(value, await run(value));
    }
  };
  await Promise.all(Array.from({ length: Math.min(maxParallelism, values.length) }, () => worker()));
  return values.map((value) => output.get(value)!);
}

function requestMap(values: readonly AutonomousWorkflowPortfolioItemRequest[]): Map<string, AutonomousWorkflowPortfolioItemRequest> {
  if (!Array.isArray(values) || values.length < 1 || values.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS) throw new ArgumentError("workflow portfolio execution requests are outside their bounds");
  const map = new Map<string, AutonomousWorkflowPortfolioItemRequest>();
  values.forEach((request, index) => {
    const id = request.id ?? `item-${index + 1}`;
    if (typeof id !== "string" || !id || map.has(id)) throw new ArgumentError("workflow portfolio execution request ids must be unique and non-empty");
    map.set(id, request);
  });
  return map;
}

function overallStatus(plan: AutonomousWorkflowPortfolioPlan, items: readonly AutonomousWorkflowPortfolioItemExecutionResult[], admissionStatus?: AutonomousWorkflowPortfolioAdmission["status"]): AutonomousWorkflowPortfolioExecutionStatus {
  if (admissionStatus === "blocked") return "blocked";
  if (plan.status === "blocked") return "blocked";
  if (items.length > 0 && items.every((item) => item.status === "succeeded")) {
    return admissionStatus === "partial" || items.some((item) => learningIncomplete(item.learningStatus)) ? "partial" : "completed";
  }
  if (items.some((item) => item.status === "approval_required") && !items.some((item) => hardFailure(item.status) || item.status === "succeeded")) return "approval_required";
  if (items.some((item) => hardFailure(item.status)) && !items.some((item) => item.status === "succeeded")) return "failed";
  return "partial";
}

const PORTFOLIO_TRACE_RETENTION = "metadata_only_no_tasks_prompts_outputs_credentials_or_evidence" as const;
const PORTFOLIO_TRACE_SECRET_MATERIAL = "never_returned" as const;
const PORTFOLIO_TRACE_PHASES: readonly AutonomousWorkflowPortfolioExecutionTracePhase[] = ["started", "plan_verified", "item_started", "item_decided", "progress", "completed", "paused", "failed", "blocked"];
const PORTFOLIO_TRACE_STATUSES: readonly AutonomousWorkflowPortfolioExecutionTraceStatus[] = ["completed", "partial", "failed", "approval_required", "blocked", "running", "succeeded", "route_review_required", "reconciliation_required", "turn_limit_reached", "child_failed", "omitted"];
const PORTFOLIO_TRACE_PROVIDER_CALLS: readonly NonNullable<AutonomousWorkflowPortfolioExecutionItemJSON["provider_call"]>[] = ["not_started", "approval_required", "may_have_started"];
const PORTFOLIO_TRACE_LEARNING_STATUSES: readonly AutonomousWorkflowPortfolioLearningStatus[] = ["disabled", "not_eligible", "pending_evaluation", "preparation_failed", "evaluation_failed", "settled", "settlement_failed"];

function traceIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function traceOptionalDigest(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function traceOptionalText(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "string" || value.length === 0 || value.length > 256 || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

/** Create a serialized, hash-chained metadata trace for a portfolio execution. */
export function createAutonomousWorkflowPortfolioExecutionTraceEmitter(input: {
  traceId: string;
  planDigest: string;
  admissionDigest?: string | null;
  sink: AutonomousWorkflowPortfolioExecutionTraceSink;
}): AutonomousWorkflowPortfolioExecutionTraceEmitter {
  const traceId = traceIdentifier("workflow portfolio traceId", input?.traceId);
  const planDigest = traceOptionalDigest("workflow portfolio trace planDigest", input?.planDigest);
  if (planDigest === null) throw new ArgumentError("workflow portfolio trace planDigest is required");
  const admissionDigest = traceOptionalDigest("workflow portfolio trace admissionDigest", input.admissionDigest);
  if (typeof input.sink !== "function") throw new ArgumentError("workflow portfolio trace sink must be callable");
  let sequence = 0;
  let previousDigest = "";
  let tail: Promise<void> = Promise.resolve();
  const emit = (eventInput: AutonomousWorkflowPortfolioExecutionTraceEventInput): Promise<AutonomousWorkflowPortfolioExecutionTraceEvent> => {
    let result!: Promise<AutonomousWorkflowPortfolioExecutionTraceEvent>;
    const operation = tail.then(async () => {
      if (!eventInput || !PORTFOLIO_TRACE_PHASES.includes(eventInput.phase)) throw new ArgumentError("workflow portfolio trace phase is invalid");
      if (!PORTFOLIO_TRACE_STATUSES.includes(eventInput.status)) throw new ArgumentError("workflow portfolio trace status is invalid");
      const itemId = eventInput.item_id === null || eventInput.item_id === undefined ? null : traceIdentifier("workflow portfolio trace item_id", eventInput.item_id);
      const domain = eventInput.domain === null || eventInput.domain === undefined ? null : (AUTONOMOUS_DOMAIN_NAMES.includes(eventInput.domain) ? eventInput.domain : (() => { throw new ArgumentError("workflow portfolio trace domain is invalid"); })());
      const waveIndex = eventInput.wave_index === null || eventInput.wave_index === undefined ? null : boundedInteger("workflow portfolio trace wave_index", eventInput.wave_index, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS, 0);
      const providerCall = eventInput.provider_call === null || eventInput.provider_call === undefined ? null : (PORTFOLIO_TRACE_PROVIDER_CALLS.includes(eventInput.provider_call) ? eventInput.provider_call : (() => { throw new ArgumentError("workflow portfolio trace provider_call is invalid"); })());
      const learningStatus = eventInput.learning_status === null || eventInput.learning_status === undefined ? null : (PORTFOLIO_TRACE_LEARNING_STATUSES.includes(eventInput.learning_status) ? eventInput.learning_status : (() => { throw new ArgumentError("workflow portfolio trace learning_status is invalid"); })());
      const body = {
        schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_SCHEMA,
        trace_id: traceId,
        sequence: sequence + 1,
        plan_digest: planDigest,
        admission_digest: admissionDigest,
        phase: eventInput.phase,
        status: eventInput.status,
        item_id: itemId,
        domain,
        wave_index: waveIndex,
        provider_call: providerCall,
        result_digest: traceOptionalDigest("workflow portfolio trace result_digest", eventInput.result_digest),
        failure_code: traceOptionalText("workflow portfolio trace failure_code", eventInput.failure_code),
        learning_status: learningStatus,
        detail_digest: traceOptionalDigest("workflow portfolio trace detail_digest", eventInput.detail_digest),
        previous_digest: previousDigest,
        retention: PORTFOLIO_TRACE_RETENTION,
        secret_material: PORTFOLIO_TRACE_SECRET_MATERIAL,
      } satisfies Omit<AutonomousWorkflowPortfolioExecutionTraceEvent, "event_digest">;
      const event = { ...body, event_digest: digestJsonSync(body) } as AutonomousWorkflowPortfolioExecutionTraceEvent;
      await input.sink(event);
      sequence += 1;
      previousDigest = event.event_digest;
      return structuredClone(event);
    });
    result = operation;
    tail = operation.then(() => undefined, () => undefined);
    return result;
  };
  return {
    emit,
    flush: async () => { await tail; },
    get traceId() { return traceId; },
    get headDigest() { return previousDigest; },
  };
}

function traceEventBody(event: AutonomousWorkflowPortfolioExecutionTraceEvent): Omit<AutonomousWorkflowPortfolioExecutionTraceEvent, "event_digest"> {
  const { event_digest: _eventDigest, ...body } = event;
  return body;
}

function validateStoredTraceEvent(
  raw: unknown,
  expectedTraceId: string,
  expectedPlanDigest: string,
  expectedAdmissionDigest: string | null,
  expectedSequence: number,
  expectedPreviousDigest: string,
): AutonomousWorkflowPortfolioExecutionTraceEvent {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_SCHEMA) throw new ArgumentError("workflow portfolio trace event schema is invalid");
  const allowed = new Set(["schema", "trace_id", "sequence", "plan_digest", "admission_digest", "phase", "status", "item_id", "domain", "wave_index", "provider_call", "result_digest", "failure_code", "learning_status", "detail_digest", "previous_digest", "event_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("workflow portfolio trace event contains unsupported fields");
  const event = raw as unknown as AutonomousWorkflowPortfolioExecutionTraceEvent;
  const traceId = traceIdentifier("workflow portfolio trace event trace_id", event.trace_id);
  const planDigest = traceOptionalDigest("workflow portfolio trace event plan_digest", event.plan_digest);
  const admissionDigest = traceOptionalDigest("workflow portfolio trace event admission_digest", event.admission_digest);
  if (traceId !== expectedTraceId || planDigest !== expectedPlanDigest || admissionDigest !== expectedAdmissionDigest) throw new ArgumentError("workflow portfolio trace event identity does not match the trace");
  if (!Number.isSafeInteger(event.sequence) || event.sequence !== expectedSequence) throw new ArgumentError("workflow portfolio trace event sequence is invalid");
  if (!PORTFOLIO_TRACE_PHASES.includes(event.phase)) throw new ArgumentError("workflow portfolio trace event phase is invalid");
  if (!PORTFOLIO_TRACE_STATUSES.includes(event.status)) throw new ArgumentError("workflow portfolio trace event status is invalid");
  const itemId = event.item_id === null ? null : traceIdentifier("workflow portfolio trace event item_id", event.item_id);
  const domain = event.domain === null ? null : (AUTONOMOUS_DOMAIN_NAMES.includes(event.domain) ? event.domain : (() => { throw new ArgumentError("workflow portfolio trace event domain is invalid"); })());
  const waveIndex = event.wave_index === null ? null : boundedInteger("workflow portfolio trace event wave_index", event.wave_index, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS, 0);
  const providerCall = event.provider_call === null ? null : (PORTFOLIO_TRACE_PROVIDER_CALLS.includes(event.provider_call) ? event.provider_call : (() => { throw new ArgumentError("workflow portfolio trace event provider_call is invalid"); })());
  const learningStatus = event.learning_status === null ? null : (PORTFOLIO_TRACE_LEARNING_STATUSES.includes(event.learning_status) ? event.learning_status : (() => { throw new ArgumentError("workflow portfolio trace event learning_status is invalid"); })());
  const resultDigest = traceOptionalDigest("workflow portfolio trace event result_digest", event.result_digest);
  const failureCode = traceOptionalText("workflow portfolio trace event failure_code", event.failure_code);
  const detailDigest = traceOptionalDigest("workflow portfolio trace event detail_digest", event.detail_digest);
  if (typeof event.previous_digest !== "string" || !/^$|^[0-9a-f]{64}$/.test(event.previous_digest) || event.previous_digest !== expectedPreviousDigest) throw new ArgumentError("workflow portfolio trace event previous digest is invalid");
  if (event.retention !== PORTFOLIO_TRACE_RETENTION || event.secret_material !== PORTFOLIO_TRACE_SECRET_MATERIAL) throw new ArgumentError("workflow portfolio trace event retention contract is invalid");
  const body = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_SCHEMA,
    trace_id: traceId,
    sequence: expectedSequence,
    plan_digest: planDigest,
    admission_digest: admissionDigest,
    phase: event.phase,
    status: event.status,
    item_id: itemId,
    domain,
    wave_index: waveIndex,
    provider_call: providerCall,
    result_digest: resultDigest,
    failure_code: failureCode,
    learning_status: learningStatus,
    detail_digest: detailDigest,
    previous_digest: expectedPreviousDigest,
    retention: PORTFOLIO_TRACE_RETENTION,
    secret_material: PORTFOLIO_TRACE_SECRET_MATERIAL,
  } satisfies Omit<AutonomousWorkflowPortfolioExecutionTraceEvent, "event_digest">;
  if (digestJsonSync(body) !== event.event_digest) throw new ArgumentError("workflow portfolio trace event digest is invalid");
  return structuredClone({ ...body, event_digest: event.event_digest }) as AutonomousWorkflowPortfolioExecutionTraceEvent;
}

/** Validate a portable portfolio trace snapshot before it is restored or persisted. */
export function validateAutonomousWorkflowPortfolioExecutionTraceSnapshot(value: unknown): AutonomousWorkflowPortfolioExecutionTraceSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_SCHEMA || !Array.isArray(value.events)) throw new ArgumentError("workflow portfolio trace snapshot is malformed");
  const traceId = traceIdentifier("workflow portfolio trace snapshot trace_id", value.trace_id);
  const planDigest = traceOptionalDigest("workflow portfolio trace snapshot plan_digest", value.plan_digest);
  if (planDigest === null) throw new ArgumentError("workflow portfolio trace snapshot plan_digest is required");
  const admissionDigest = traceOptionalDigest("workflow portfolio trace snapshot admission_digest", value.admission_digest);
  const sequenceValue = value.sequence;
  if (!Number.isSafeInteger(sequenceValue) || (sequenceValue as number) < 0 || (sequenceValue as number) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENTS) throw new ArgumentError("workflow portfolio trace snapshot sequence is invalid");
  const sequence = sequenceValue as number;
  if (!Array.isArray(value.events) || value.events.length !== sequence) throw new ArgumentError("workflow portfolio trace snapshot sequence does not match events");
  let previousDigest = "";
  const events = value.events.map((event, index) => {
    const validated = validateStoredTraceEvent(event, traceId, planDigest, admissionDigest, index + 1, previousDigest);
    previousDigest = validated.event_digest;
    return validated;
  });
  if (typeof value.head_digest !== "string" || value.head_digest !== previousDigest) throw new ArgumentError("workflow portfolio trace snapshot head digest is invalid");
  if (value.retention !== "metadata_only_hash_chained_no_tasks_prompts_outputs_credentials_or_evidence" || value.secret_material !== "never_returned") throw new ArgumentError("workflow portfolio trace snapshot retention contract is invalid");
  const body = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_SCHEMA,
    trace_id: traceId,
    plan_digest: planDigest,
    admission_digest: admissionDigest,
    sequence: events.length,
    head_digest: previousDigest,
    events,
    retention: "metadata_only_hash_chained_no_tasks_prompts_outputs_credentials_or_evidence" as const,
    secret_material: "never_returned" as const,
  };
  if (typeof value.snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.snapshot_digest) || digestJsonSync(body) !== value.snapshot_digest) throw new ArgumentError("workflow portfolio trace snapshot digest is invalid");
  const snapshot = { ...body, snapshot_digest: value.snapshot_digest } as AutonomousWorkflowPortfolioExecutionTraceSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("workflow portfolio trace snapshot exceeds its byte bound");
  return structuredClone(snapshot);
}

/** In-memory validated trace store; pass `event => store.append(event)` as the execution trace sink. */
export class InMemoryAutonomousWorkflowPortfolioExecutionTraceStore {
  private readonly eventsValue: AutonomousWorkflowPortfolioExecutionTraceEvent[] = [];
  readonly traceId: string;
  readonly planDigest: string;
  readonly admissionDigest: string | null;
  readonly maxEvents: number;
  readonly maxEventBytes: number;
  readonly maxSnapshotBytes: number;

  constructor(options: { traceId: string; planDigest: string; admissionDigest?: string | null; maxEvents?: number; maxEventBytes?: number; maxSnapshotBytes?: number }) {
    this.traceId = traceIdentifier("workflow portfolio trace store traceId", options?.traceId);
    const planDigest = traceOptionalDigest("workflow portfolio trace store planDigest", options?.planDigest);
    if (planDigest === null) throw new ArgumentError("workflow portfolio trace store planDigest is required");
    this.planDigest = planDigest;
    this.admissionDigest = traceOptionalDigest("workflow portfolio trace store admissionDigest", options.admissionDigest);
    this.maxEvents = options.maxEvents ?? MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENTS;
    this.maxEventBytes = options.maxEventBytes ?? MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_BYTES;
    this.maxSnapshotBytes = options.maxSnapshotBytes ?? MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES;
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENTS) throw new ArgumentError("workflow portfolio trace store maxEvents is outside its bounds");
    if (!Number.isSafeInteger(this.maxEventBytes) || this.maxEventBytes < 512 || this.maxEventBytes > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_EVENT_BYTES) throw new ArgumentError("workflow portfolio trace store maxEventBytes is outside its bounds");
    if (!Number.isSafeInteger(this.maxSnapshotBytes) || this.maxSnapshotBytes < this.maxEventBytes || this.maxSnapshotBytes > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("workflow portfolio trace store maxSnapshotBytes is outside its bounds");
  }

  append(raw: AutonomousWorkflowPortfolioExecutionTraceEvent): AutonomousWorkflowPortfolioExecutionTraceEvent {
    if (this.eventsValue.length >= this.maxEvents) throw new ArgumentError("workflow portfolio trace store event capacity is exhausted");
    const event = validateStoredTraceEvent(raw, this.traceId, this.planDigest, this.admissionDigest, this.eventsValue.length + 1, this.eventsValue.at(-1)?.event_digest ?? "");
    if (new TextEncoder().encode(canonicalJson(event)).byteLength > this.maxEventBytes) throw new ArgumentError("workflow portfolio trace event exceeds its byte bound");
    this.eventsValue.push(event);
    return structuredClone(event);
  }

  events(options: { afterSequence?: number; limit?: number } = {}): AutonomousWorkflowPortfolioExecutionTraceEvent[] {
    const afterSequence = options.afterSequence ?? 0;
    const limit = options.limit ?? Math.min(this.maxEvents, 10_000);
    if (!Number.isSafeInteger(afterSequence) || afterSequence < 0) throw new ArgumentError("workflow portfolio trace afterSequence is invalid");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 10_000) throw new ArgumentError("workflow portfolio trace query limit is invalid");
    return this.eventsValue.filter((event) => event.sequence > afterSequence).slice(0, limit).map((event) => structuredClone(event));
  }

  snapshot(): AutonomousWorkflowPortfolioExecutionTraceSnapshot {
    const body = {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_SCHEMA,
      trace_id: this.traceId,
      plan_digest: this.planDigest,
      admission_digest: this.admissionDigest,
      sequence: this.eventsValue.length,
      head_digest: this.eventsValue.at(-1)?.event_digest ?? "",
      events: this.eventsValue.map((event) => structuredClone(event)),
      retention: "metadata_only_hash_chained_no_tasks_prompts_outputs_credentials_or_evidence" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousWorkflowPortfolioExecutionTraceSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > this.maxSnapshotBytes) throw new ArgumentError("workflow portfolio trace snapshot exceeds its byte bound");
    return structuredClone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateAutonomousWorkflowPortfolioExecutionTraceSnapshot(raw);
    if (snapshot.trace_id !== this.traceId || snapshot.plan_digest !== this.planDigest || snapshot.admission_digest !== this.admissionDigest) throw new ArgumentError("workflow portfolio trace snapshot identity does not match the store");
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > this.maxSnapshotBytes || snapshot.events.length > this.maxEvents) throw new ArgumentError("workflow portfolio trace snapshot exceeds the store bounds");
    this.eventsValue.splice(0, this.eventsValue.length, ...snapshot.events.map((event) => structuredClone(event)));
  }

  verifyIntegrity(): { verified: true; events: number; head_digest: string } {
    let previousDigest = "";
    for (const [index, event] of this.eventsValue.entries()) {
      const validated = validateStoredTraceEvent(event, this.traceId, this.planDigest, this.admissionDigest, index + 1, previousDigest);
      previousDigest = validated.event_digest;
    }
    return { verified: true, events: this.eventsValue.length, head_digest: previousDigest };
  }
}

/** Coordinates atomic caller-owned persistence for a portfolio trace snapshot. */
export class AutonomousWorkflowPortfolioExecutionTracePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: InMemoryAutonomousWorkflowPortfolioExecutionTraceStore, readonly persistence: AutonomousWorkflowPortfolioExecutionTracePersistence) {
    if (!(store instanceof InMemoryAutonomousWorkflowPortfolioExecutionTraceStore)) throw new ArgumentError("workflow portfolio trace persistence requires a typed store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("workflow portfolio trace persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousWorkflowPortfolioExecutionTraceSnapshot | null> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  async flush(): Promise<AutonomousWorkflowPortfolioExecutionTraceSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.store.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        const committed = await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot);
        if (!committed) throw new ArgumentError("workflow portfolio trace persistence compare-and-swap conflict");
      } else {
        await this.persistence.write(snapshot);
      }
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

/** Strict bounded JSON adapter for a caller-owned portfolio trace text store. */
export class JsonAutonomousWorkflowPortfolioExecutionTracePersistence implements AutonomousWorkflowPortfolioExecutionTracePersistence {
  constructor(readonly textStore: AutonomousWorkflowPortfolioExecutionTraceTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("workflow portfolio trace text store is malformed");
  }

  async read(): Promise<AutonomousWorkflowPortfolioExecutionTraceSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("workflow portfolio trace JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("workflow portfolio trace JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("workflow portfolio trace JSON is not canonical");
    return validateAutonomousWorkflowPortfolioExecutionTraceSnapshot(parsed);
  }

  async write(snapshot: AutonomousWorkflowPortfolioExecutionTraceSnapshot): Promise<void> {
    const validated = validateAutonomousWorkflowPortfolioExecutionTraceSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("workflow portfolio trace JSON exceeds its byte bound");
    await this.textStore.write(encoded);
  }
}

/** JSON adapter with an atomic text-store compare-and-swap fence. */
export class TransactionalJsonAutonomousWorkflowPortfolioExecutionTracePersistence extends JsonAutonomousWorkflowPortfolioExecutionTracePersistence {
  declare readonly textStore: AutonomousWorkflowPortfolioExecutionTraceTransactionalTextStore;

  constructor(textStore: AutonomousWorkflowPortfolioExecutionTraceTransactionalTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("workflow portfolio trace text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioExecutionTraceSnapshot): Promise<boolean> {
    const validated = validateAutonomousWorkflowPortfolioExecutionTraceSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, encoded);
  }
}

/** Browser-compatible key/value adapter for a portfolio trace snapshot. */
export class WebStorageAutonomousWorkflowPortfolioExecutionTraceTextStore implements AutonomousWorkflowPortfolioExecutionTraceTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("workflow portfolio trace Web Storage adapter is malformed");
    traceIdentifier("workflow portfolio trace storage key", key);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/** Execute a verified portfolio in deterministic dependency waves with bounded transient handoffs. */
export async function executeAutonomousWorkflowPortfolio(
  agent: AutonomousAgent,
  requests: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioExecutionOptions = {},
): Promise<AutonomousWorkflowPortfolioExecutionResult> {
  return executeAutonomousWorkflowPortfolioWithInitialItems(agent, requests, options);
}

/**
 * Execute a portfolio while reusing caller-rehydrated terminal item results. This is the narrow
 * restart seam used by the durable controller; callers should use its resumable wrapper so that
 * checkpoint identity and result digests are validated before this function is reached.
 */
export async function executeAutonomousWorkflowPortfolioWithInitialItems(
  agent: AutonomousAgent,
  requests: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioExecutionOptions = {},
  initialItems: readonly AutonomousWorkflowPortfolioItemExecutionResult[] = [],
  progressSink?: AutonomousWorkflowPortfolioExecutionProgressSink,
): Promise<AutonomousWorkflowPortfolioExecutionResult> {
  if (!agent || typeof agent.run !== "function" || typeof agent.blueprint !== "function") throw new ArgumentError("workflow portfolio execution requires an AutonomousAgent");
  if (options.verifyPlan !== undefined && typeof options.verifyPlan !== "boolean") throw new ArgumentError("workflow portfolio verifyPlan must be boolean");
  learningPolicyDigestFor(options);
  if (options.learningSettlement?.remote !== undefined && typeof options.learningSettlement.remote !== "boolean") throw new ArgumentError("workflow portfolio learningSettlement.remote must be boolean");
  if (options.evaluateItem !== undefined && typeof options.evaluateItem !== "function") throw new ArgumentError("workflow portfolio evaluateItem must be callable");
  const maxParallelism = boundedInteger("workflow portfolio maxParallelism", options.maxParallelism, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM, 4);
  const maxHandoffBytes = boundedInteger("workflow portfolio maxDependencyHandoffBytes", options.maxDependencyHandoffBytes, 512, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES, DEFAULT_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES);
  const requestById = requestMap(requests);
  const plan = options.plan
    ? await validateAutonomousWorkflowPortfolioPlan(options.plan)
    : await planAutonomousWorkflowPortfolio(agent, requests, options.planOptions);
  const admission = options.admission === undefined ? null : await validateAutonomousWorkflowPortfolioAdmission(options.admission);
  if (admission !== null && admission.plan.portfolio_digest !== plan.portfolio_digest) throw new ProviderRuntimeError("workflow portfolio admission does not match the reviewed plan", { code: "protocol", retryable: false, operation: "workflow_portfolio_admission" });
  if (options.traceId !== undefined && options.traceSink === undefined) throw new ArgumentError("workflow portfolio traceId requires traceSink");
  const trace = options.traceSink === undefined ? null : createAutonomousWorkflowPortfolioExecutionTraceEmitter({ traceId: options.traceId ?? `portfolio-trace:${plan.portfolio_digest.slice(0, 24)}`, planDigest: plan.portfolio_digest, admissionDigest: admission?.admission_digest ?? null, sink: options.traceSink });
  await trace?.emit({ phase: "started", status: "running" });
  const admissionItems = admission === null ? null : new Map(admission.items.map((item) => [item.item_id, item]));
  if (options.plan && options.verifyPlan !== false) {
    const verification = await verifyAutonomousWorkflowPortfolio(agent, plan, requests, options.planOptions);
    if (verification.status !== "verified") throw new ProviderRuntimeError("workflow portfolio plan verification failed before dispatch", { code: "protocol", retryable: false, operation: "workflow_portfolio_verify" });
  }
  await trace?.emit({ phase: "plan_verified", status: "running" });

  const executions = new Map<string, AutonomousWorkflowPortfolioItemExecutionResult>();
  const planItemsById = new Map(plan.items.map((item) => [item.item_id, item]));
  for (const initial of initialItems) {
    const planItem = planItemsById.get(initial.itemId);
    if (!planItem || planItem.status !== "ready" || executions.has(initial.itemId) || initial.domain !== planItem.domain || canonicalJson(initial.dependsOn) !== canonicalJson(planItem.depends_on)) {
      throw new ProviderRuntimeError(`rehydrated workflow portfolio item ${initial.itemId} does not match the reviewed plan`, { code: "protocol", retryable: false, operation: "workflow_portfolio_rehydrate" });
    }
    const rehydrated = await applyRehydratedLearningSettlement(initial, options, plan.portfolio_digest, requestById.get(initial.itemId), planItem);
    executions.set(initial.itemId, rehydrated);
  }
  for (const item of plan.items) {
    const admitted = admissionItems?.get(item.item_id);
    if (item.status !== "ready") executions.set(item.item_id, new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], statusForPlanItem(item), null, null, 0, item.error_class, item.error_class));
    else if (admitted && admitted.status !== "eligible") executions.set(item.item_id, new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, admitted.blockers[0] ?? "portfolio_admission_blocked", "portfolio_admission_blocked", options.includeDependencyOutputs !== false));
  }
  const snapshotItems = (): AutonomousWorkflowPortfolioItemExecutionResult[] => plan.items.map((item) => executions.get(item.item_id) ?? new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, "portfolio_item_pending", "portfolio_item_pending"));
  const reportProgress = async (): Promise<void> => {
    const snapshot = snapshotItems();
    const status = overallStatus(plan, snapshot, admission?.status);
    await trace?.emit({ phase: "progress", status, detail_digest: await digestJson(snapshot.map((item) => item.toJSON(new Map(snapshot.map((candidate) => [candidate.itemId, candidate]))))) });
    await progressSink?.({ plan, items: snapshot, status });
  };
  await reportProgress();
  if (plan.status === "blocked") {
    for (const item of plan.items) {
      if (!executions.has(item.item_id)) {
        const blocked = new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, "portfolio_plan_blocked", "portfolio_plan_blocked");
        executions.set(item.item_id, blocked);
        await trace?.emit({ phase: "blocked", status: blocked.status, item_id: item.item_id, domain: item.domain, failure_code: blocked.failureCode });
      }
    }
  } else {
    const includeDependencyOutputs = options.includeDependencyOutputs !== false;
    let stopped = initialItems.some((item) => options.stopOnError === true && hardFailure(item.status));
    for (const wave of plan.dependency_graph.waves) {
      const waveIds = wave.filter((itemId) => planItemsById.get(itemId)?.status === "ready" && !executions.has(itemId)).sort();
      if (stopped) {
        for (const itemId of waveIds) {
          const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
          const omitted = new AutonomousWorkflowPortfolioItemExecutionResult(itemId, item.domain, [...item.depends_on], "omitted", null, null, 0, "portfolio_stopped_after_failure", "portfolio_stopped_after_failure", includeDependencyOutputs);
          executions.set(itemId, omitted);
          await trace?.emit({ phase: "item_decided", status: omitted.status, item_id: itemId, domain: item.domain, failure_code: omitted.failureCode });
        }
        continue;
      }
      const runnable = waveIds.filter((itemId) => {
        const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
        return item.depends_on.every((dependency) => executions.get(dependency)?.status === "succeeded");
      });
      for (const itemId of waveIds.filter((itemId) => !runnable.includes(itemId))) {
        const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
        const blocked = new AutonomousWorkflowPortfolioItemExecutionResult(itemId, item.domain, [...item.depends_on], "blocked", null, null, 0, "dependency_not_succeeded", "dependency_not_succeeded", includeDependencyOutputs);
        executions.set(itemId, blocked);
        await trace?.emit({ phase: "blocked", status: blocked.status, item_id: itemId, domain: item.domain, failure_code: blocked.failureCode });
      }
      const waveResults = await runBounded(runnable, maxParallelism, async (itemId) => {
        const planItem = plan.items.find((item) => item.item_id === itemId)!;
        const request = requestById.get(itemId);
        if (!request) throw new ProviderRuntimeError("workflow portfolio execution request is missing for the reviewed plan", { code: "protocol", retryable: false, operation: "workflow_portfolio_dispatch" });
        const waveIndex = plan.dependency_graph.waves.findIndex((wave) => wave.includes(itemId));
        await trace?.emit({ phase: "item_started", status: "running", item_id: itemId, domain: planItem.domain, wave_index: waveIndex < 0 ? 0 : waveIndex, provider_call: "not_started" });
        const context = [
          ...(options.run?.context ?? []),
          ...(request.context ?? []),
          ...dependencyHandoffs(new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], "succeeded", null, null, 0, null, null, includeDependencyOutputs), executions, maxHandoffBytes),
        ];
        try {
          const run = await agent.run(request.task, runOptionsFor({ ...request, domain: planItem.domain }, itemId, plan.portfolio_digest, options, context));
          const status = executionStatusForRun(run);
          const outputProjection = autonomousWorkflowPortfolioTransientOutput(run);
          const output = outputProjection.text;
          const outputDigest = output ? await digestJson({ item_id: itemId, output }) : null;
          const learning = await settlePortfolioItemLearning(options, plan.portfolio_digest, request, planItem, itemId, run, output, outputDigest);
          const result = new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], status, run, outputDigest, outputProjection.bytes, null, status === "succeeded" ? null : status, includeDependencyOutputs, output, learning.status, learning.episodeId, learning.evaluationDigest, learning.settlementDigest, learning.errorClass);
          await trace?.emit({ phase: "item_decided", status: result.status, item_id: itemId, domain: result.domain, wave_index: waveIndex < 0 ? 0 : waveIndex, provider_call: result.toJSON().provider_call, result_digest: result.outputDigest, failure_code: result.failureCode, learning_status: result.learningStatus });
          return result;
        } catch (error) {
          const metadata = errorMetadata(error);
          const result = new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], "failed", null, null, 0, metadata.errorClass, metadata.failureCode, includeDependencyOutputs);
          await trace?.emit({ phase: "item_decided", status: result.status, item_id: itemId, domain: result.domain, wave_index: waveIndex < 0 ? 0 : waveIndex, provider_call: result.toJSON().provider_call, failure_code: result.failureCode, learning_status: result.learningStatus });
          return result;
        }
      });
      for (const result of waveResults) executions.set(result.itemId, result);
      await reportProgress();
      if (options.stopOnError === true && waveResults.some((item) => hardFailure(item.status))) stopped = true;
    }
  }

  const items = plan.items.map((item) => executions.get(item.item_id) ?? new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, "portfolio_item_not_scheduled", "portfolio_item_not_scheduled"));
  const status = overallStatus(plan, items, admission?.status);
  await trace?.emit({ phase: status === "blocked" ? "blocked" : status === "approval_required" ? "paused" : status === "failed" ? "failed" : "completed", status, detail_digest: await digestJson(items.map((item) => item.toJSON(new Map(items.map((candidate) => [candidate.itemId, candidate]))))) });
  await progressSink?.({ plan, items, status });
  await trace?.flush();
  const traceDigest = trace?.headDigest ?? null;
  const counts = executionCounts(items);
  const metadata = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
    status,
    plan_digest: plan.portfolio_digest,
    admission_digest: admission?.admission_digest ?? null,
    trace_digest: traceDigest,
    wave_count: plan.dependency_graph.waves.length,
    completed_count: counts.completed,
    failed_count: counts.failed,
    blocked_count: counts.blocked,
    approval_required_count: counts.approvalRequired,
      omitted_count: counts.omitted,
    learning_settled_count: counts.learningSettled,
    learning_pending_count: counts.learningPending,
    learning_not_eligible_count: counts.learningNotEligible,
    learning_failed_count: counts.learningFailed,
    items: items.map((item) => item.toJSON(new Map(items.map((candidate) => [candidate.itemId, candidate])))),
    execution: "provider_and_tool_calls_are_per_item_approved;outputs_transient_to_caller" as const,
    authorization: "portfolio_plan_verification_does_not_authorize_provider_or_effects" as const,
    retention: "metadata_only_task_and_provider_output_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  const executionDigest = await digestJson(metadata);
  return new AutonomousWorkflowPortfolioExecutionResult(plan, status, items, plan.dependency_graph.waves.length, executionDigest, admission?.admission_digest ?? null, traceDigest);
}
