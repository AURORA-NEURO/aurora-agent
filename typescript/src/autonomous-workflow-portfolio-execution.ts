import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
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
import { digestJson } from "./tooling.js";
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

/** Shared provider/learning controls for each item; portfolio identity controls are not caller-overridable. */
export type AutonomousWorkflowPortfolioRunOptions = Omit<
  AutonomousRunOptions,
  "domain" | "routeOverride" | "capability" | "workflowContext" | "allowCrossDomain"
>;

export interface AutonomousWorkflowPortfolioExecutionOptions {
  /** Reuse a previously reviewed metadata-only plan. When omitted, a provider-free plan is compiled. */
  plan?: AutonomousWorkflowPortfolioPlan;
  planOptions?: AutonomousWorkflowPortfolioPlanOptions;
  /** Recompute the provider-free plan identity before any provider or tool dispatch; defaults to true. */
  verifyPlan?: boolean;
  /** Controls shared by every item, including candidates, credentialFor, memory, learning, tools, and budgets. */
  run?: AutonomousWorkflowPortfolioRunOptions;
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
}

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
  retention: "metadata_only_task_and_provider_output_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA;
  status: AutonomousWorkflowPortfolioExecutionStatus;
  plan_digest: string;
  execution_digest: string;
  wave_count: number;
  completed_count: number;
  failed_count: number;
  blocked_count: number;
  approval_required_count: number;
  omitted_count: number;
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

function safeOutputText(run: AutonomousRunResult): string {
  if (run.response?.text) return run.response.text;
  if (run.response?.structured === null || run.response?.structured === undefined) return "";
  try {
    return JSON.stringify(run.response.structured) ?? "";
  } catch {
    return "";
  }
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

function hardFailure(status: AutonomousWorkflowPortfolioExecutionItemStatus): boolean {
  return status === "failed" || status === "route_review_required" || status === "reconciliation_required" || status === "turn_limit_reached" || status === "child_failed";
}

function executionCounts(items: readonly AutonomousWorkflowPortfolioItemExecutionResult[]): {
  completed: number;
  failed: number;
  blocked: number;
  approvalRequired: number;
  omitted: number;
} {
  return {
    completed: items.filter((item) => item.status === "succeeded").length,
    failed: items.filter((item) => hardFailure(item.status)).length,
    blocked: items.filter((item) => item.status === "blocked").length,
    approvalRequired: items.filter((item) => item.status === "approval_required").length,
    omitted: items.filter((item) => item.status === "omitted").length,
  };
}

function dependencyStatuses(item: AutonomousWorkflowPortfolioItemExecutionResult, results: ReadonlyMap<string, AutonomousWorkflowPortfolioItemExecutionResult>): Record<string, AutonomousWorkflowPortfolioExecutionItemStatus> {
  return Object.fromEntries(item.dependsOn.map((dependency) => [dependency, results.get(dependency)?.status ?? "blocked"]));
}

function runOptionsFor(
  request: AutonomousWorkflowPortfolioItemRequest,
  itemId: string,
  options: AutonomousWorkflowPortfolioExecutionOptions,
  context: readonly AutonomousPromptChunk[],
): AutonomousRunOptions {
  const base = options.run ?? {};
  const approval = options.approveProviderCall ?? base.approveProviderCall ?? false;
  return {
    ...base,
    domain: request.domain,
    capability: request.capability,
    context,
    hints: request.hints ?? [],
    allowCrossDomain: false,
    approveProviderCall: approval,
    ...(base.memoryRunId ? { memoryRunId: `${base.memoryRunId}:${itemId}` } : {}),
    ...(base.learningEpisodeId ? { learningEpisodeId: `${base.learningEpisodeId}:${itemId}` } : {}),
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
  ) {}

  toJSON(): AutonomousWorkflowPortfolioExecutionJSON {
    const itemJson = this.items.map((item) => item.toJSON(new Map(this.items.map((candidate) => [candidate.itemId, candidate]))));
    const counts = executionCounts(this.items);
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
      status: this.status,
      plan_digest: this.plan.portfolio_digest,
      execution_digest: this.executionDigest,
      wave_count: this.waveCount,
      completed_count: counts.completed,
      failed_count: counts.failed,
      blocked_count: counts.blocked,
      approval_required_count: counts.approvalRequired,
      omitted_count: counts.omitted,
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

function overallStatus(plan: AutonomousWorkflowPortfolioPlan, items: readonly AutonomousWorkflowPortfolioItemExecutionResult[]): AutonomousWorkflowPortfolioExecutionStatus {
  if (plan.status === "blocked") return "blocked";
  if (items.length > 0 && items.every((item) => item.status === "succeeded")) return "completed";
  if (items.some((item) => item.status === "approval_required") && !items.some((item) => hardFailure(item.status) || item.status === "succeeded")) return "approval_required";
  if (items.some((item) => hardFailure(item.status)) && !items.some((item) => item.status === "succeeded")) return "failed";
  return "partial";
}

/** Execute a verified portfolio in deterministic dependency waves with bounded transient handoffs. */
export async function executeAutonomousWorkflowPortfolio(
  agent: AutonomousAgent,
  requests: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioExecutionOptions = {},
): Promise<AutonomousWorkflowPortfolioExecutionResult> {
  if (!agent || typeof agent.run !== "function" || typeof agent.blueprint !== "function") throw new ArgumentError("workflow portfolio execution requires an AutonomousAgent");
  if (options.verifyPlan !== undefined && typeof options.verifyPlan !== "boolean") throw new ArgumentError("workflow portfolio verifyPlan must be boolean");
  const maxParallelism = boundedInteger("workflow portfolio maxParallelism", options.maxParallelism, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM, 4);
  const maxHandoffBytes = boundedInteger("workflow portfolio maxDependencyHandoffBytes", options.maxDependencyHandoffBytes, 512, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES, DEFAULT_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES);
  const requestById = requestMap(requests);
  const plan = options.plan
    ? await validateAutonomousWorkflowPortfolioPlan(options.plan)
    : await planAutonomousWorkflowPortfolio(agent, requests, options.planOptions);
  if (options.plan && options.verifyPlan !== false) {
    const verification = await verifyAutonomousWorkflowPortfolio(agent, plan, requests, options.planOptions);
    if (verification.status !== "verified") throw new ProviderRuntimeError("workflow portfolio plan verification failed before dispatch", { code: "protocol", retryable: false, operation: "workflow_portfolio_verify" });
  }

  const executions = new Map<string, AutonomousWorkflowPortfolioItemExecutionResult>();
  for (const item of plan.items) {
    if (item.status !== "ready") executions.set(item.item_id, new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], statusForPlanItem(item), null, null, 0, item.error_class, item.error_class));
  }
  if (plan.status === "blocked") {
    for (const item of plan.items) {
      if (!executions.has(item.item_id)) executions.set(item.item_id, new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, "portfolio_plan_blocked", "portfolio_plan_blocked"));
    }
  } else {
    const includeDependencyOutputs = options.includeDependencyOutputs !== false;
    let stopped = false;
    for (const wave of plan.dependency_graph.waves) {
      const waveIds = wave.filter((itemId) => plan.items.find((item) => item.item_id === itemId)?.status === "ready").sort();
      if (stopped) {
        for (const itemId of waveIds) {
          const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
          executions.set(itemId, new AutonomousWorkflowPortfolioItemExecutionResult(itemId, item.domain, [...item.depends_on], "omitted", null, null, 0, "portfolio_stopped_after_failure", "portfolio_stopped_after_failure", includeDependencyOutputs));
        }
        continue;
      }
      const runnable = waveIds.filter((itemId) => {
        const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
        return item.depends_on.every((dependency) => executions.get(dependency)?.status === "succeeded");
      });
      for (const itemId of waveIds.filter((itemId) => !runnable.includes(itemId))) {
        const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
        executions.set(itemId, new AutonomousWorkflowPortfolioItemExecutionResult(itemId, item.domain, [...item.depends_on], "blocked", null, null, 0, "dependency_not_succeeded", "dependency_not_succeeded", includeDependencyOutputs));
      }
      const waveResults = await runBounded(runnable, maxParallelism, async (itemId) => {
        const planItem = plan.items.find((item) => item.item_id === itemId)!;
        const request = requestById.get(itemId);
        if (!request) throw new ProviderRuntimeError("workflow portfolio execution request is missing for the reviewed plan", { code: "protocol", retryable: false, operation: "workflow_portfolio_dispatch" });
        const context = [
          ...(options.run?.context ?? []),
          ...(request.context ?? []),
          ...dependencyHandoffs(new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], "succeeded", null, null, 0, null, null, includeDependencyOutputs), executions, maxHandoffBytes),
        ];
        try {
          const run = await agent.run(request.task, runOptionsFor({ ...request, domain: planItem.domain }, itemId, options, context));
          const status = executionStatusForRun(run);
          const output = safeOutputText(run);
          return new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], status, run, output ? await digestJson({ item_id: itemId, output }) : null, bytes(output), null, status === "succeeded" ? null : status, includeDependencyOutputs, output);
        } catch (error) {
          const metadata = errorMetadata(error);
          return new AutonomousWorkflowPortfolioItemExecutionResult(itemId, planItem.domain, [...planItem.depends_on], "failed", null, null, 0, metadata.errorClass, metadata.failureCode, includeDependencyOutputs);
        }
      });
      for (const result of waveResults) executions.set(result.itemId, result);
      if (options.stopOnError === true && waveResults.some((item) => hardFailure(item.status))) stopped = true;
    }
  }

  const items = plan.items.map((item) => executions.get(item.item_id) ?? new AutonomousWorkflowPortfolioItemExecutionResult(item.item_id, item.domain, [...item.depends_on], "blocked", null, null, 0, "portfolio_item_not_scheduled", "portfolio_item_not_scheduled"));
  const status = overallStatus(plan, items);
  const counts = executionCounts(items);
  const metadata = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_SCHEMA,
    status,
    plan_digest: plan.portfolio_digest,
    wave_count: plan.dependency_graph.waves.length,
    completed_count: counts.completed,
    failed_count: counts.failed,
    blocked_count: counts.blocked,
    approval_required_count: counts.approvalRequired,
    omitted_count: counts.omitted,
    items: items.map((item) => item.toJSON(new Map(items.map((candidate) => [candidate.itemId, candidate])))),
    execution: "provider_and_tool_calls_are_per_item_approved;outputs_transient_to_caller" as const,
    authorization: "portfolio_plan_verification_does_not_authorize_provider_or_effects" as const,
    retention: "metadata_only_task_and_provider_output_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  const executionDigest = await digestJson(metadata);
  return new AutonomousWorkflowPortfolioExecutionResult(plan, status, items, plan.dependency_graph.waves.length, executionDigest);
}
