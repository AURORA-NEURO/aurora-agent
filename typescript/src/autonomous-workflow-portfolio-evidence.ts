import { ArgumentError } from "./errors.js";
import {
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceRuntimeJournal,
  type AutonomousEvidenceRuntimeResult,
  type AutonomousEvidenceRuntimeExecuteOptions,
} from "./autonomous-evidence-runtime.js";
import { AutonomousEvidencePlan } from "./autonomous-evidence.js";
import {
  AutonomousEvidenceRuntime,
} from "./autonomous-evidence-runtime.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousAgent, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousWorkflowPortfolioExecutionResult,
  type AutonomousWorkflowPortfolioExecutionItemStatus,
  type AutonomousWorkflowPortfolioItemExecutionResult,
} from "./autonomous-workflow-portfolio-execution.js";
import { validateAutonomousWorkflowPortfolioPlan, type AutonomousWorkflowPortfolioPlan } from "./autonomous-workflow-portfolio.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-evidence/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS = 64;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS = 128;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM = 8;

export type AutonomousWorkflowPortfolioEvidenceItemStatus =
  | "completed"
  | "partial"
  | "awaiting_evaluation"
  | "failed"
  | "reconciliation_required"
  | "not_requested"
  | "omitted";

export type AutonomousWorkflowPortfolioEvidenceStatus = "completed" | "partial" | "awaiting_evaluation" | "failed" | "reconciliation_required";

export interface AutonomousWorkflowPortfolioEvidenceItemRequest extends JsonObject {
  item_id: string;
  requests: AutonomousEvidenceAcquisitionRequest[];
}

export type AutonomousWorkflowPortfolioEvidenceRuntimeOptions = Omit<AutonomousEvidenceRuntimeExecuteOptions, "parentEvidenceDigests">;

export interface AutonomousWorkflowPortfolioEvidenceSupervisorOptions {
  /** Reuse a previously reviewed plan; omitted plans are compiled without provider calls. */
  plan?: AutonomousWorkflowPortfolioPlan;
  /** Evidence requests grouped by portfolio item. Raw acquisition values remain caller-owned. */
  items: readonly AutonomousWorkflowPortfolioEvidenceItemRequest[];
  /** Caller-owned acquisition/projection/evaluator boundary used for every evidence item. */
  runtime: AutonomousWorkflowPortfolioEvidenceRuntimeOptions;
  /** Optional evidence plan. When omitted, the current agent catalogue compiles one for all plan domains. */
  evidencePlan?: AutonomousEvidencePlan;
  /** Caller-owned journal per item; journals must be isolated or concurrency-safe. */
  journalFor?: (context: { itemId: string; domain: AutonomousDomainName; evidencePlanDigest: string }) => AutonomousEvidenceRuntimeJournal | undefined;
  maxParallelism?: number;
  stopOnFailure?: boolean;
  /** Metadata-only wave progress for a restart checkpoint or operator projection. */
  progressSink?: AutonomousWorkflowPortfolioEvidenceProgressSink;
}

export interface AutonomousWorkflowPortfolioEvidenceItemJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA;
  item_id: string;
  domain: AutonomousDomainName;
  provider_status: AutonomousWorkflowPortfolioExecutionItemStatus;
  status: AutonomousWorkflowPortfolioEvidenceItemStatus;
  request_count: number;
  completed_requirement_count: number;
  pending_evaluation_count: number;
  missing_requirement_count: number;
  result_digest: string | null;
  receipt_digests: string[];
  assessment_digests: string[];
  error_class: string | null;
  retention: "metadata_only;raw_evidence_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioEvidenceJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA;
  status: AutonomousWorkflowPortfolioEvidenceStatus;
  portfolio_plan_digest: string;
  evidence_plan_digest: string;
  completed_count: number;
  partial_count: number;
  awaiting_evaluation_count: number;
  failed_count: number;
  omitted_count: number;
  not_requested_count: number;
  items: AutonomousWorkflowPortfolioEvidenceItemJSON[];
  result_digest: string;
  retention: "metadata_only;raw_evidence_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioEvidenceItemTransient {
  itemId: string;
  domain: AutonomousDomainName;
  providerStatus: AutonomousWorkflowPortfolioExecutionItemStatus;
  status: AutonomousWorkflowPortfolioEvidenceItemStatus;
  runtime: AutonomousEvidenceRuntimeResult | null;
  errorClass: string | null;
}

export interface AutonomousWorkflowPortfolioEvidenceProgress {
  plan: AutonomousWorkflowPortfolioPlan;
  evidencePlan: AutonomousEvidencePlan;
  items: readonly AutonomousWorkflowPortfolioEvidenceItemJSON[];
  status: AutonomousWorkflowPortfolioEvidenceStatus;
  resultDigest: string;
}

export type AutonomousWorkflowPortfolioEvidenceProgressSink = (
  progress: AutonomousWorkflowPortfolioEvidenceProgress,
) => Promise<void> | void;

type EvidenceItemTransient = AutonomousWorkflowPortfolioEvidenceItemTransient;

async function scopedEvidencePlan(
  agent: AutonomousAgent,
  domain: AutonomousDomainName,
  evidencePlan: AutonomousEvidencePlan,
): Promise<AutonomousEvidencePlan> {
  // A portfolio evidence plan is global for review and digesting, but each runtime
  // must be scoped to one item domain. Otherwise a perfectly evaluated coding item
  // would remain "awaiting_evaluation" for the eleven unrelated domains.
  return agent.evidencePlan([domain], { availableEvidence: evidencePlan.available_evidence });
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number, fallback: number): number {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function boundedItemId(value: unknown): string {
  if (typeof value !== "string" || !value || value.length > 128 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError("portfolio evidence item_id is outside its identifier contract");
  return value;
}

function errorClass(error: unknown): string {
  return error instanceof Error && error.constructor.name.trim() ? error.constructor.name : "portfolio_evidence_failed";
}

function statusForRuntime(status: AutonomousEvidenceRuntimeResult["json"]["status"]): AutonomousWorkflowPortfolioEvidenceItemStatus {
  switch (status) {
    case "completed": return "completed";
    case "awaiting_evaluation": return "awaiting_evaluation";
    case "reconciliation_required": return "reconciliation_required";
    case "failed": return "failed";
    default: return "partial";
  }
}

function overallStatus(items: readonly EvidenceItemTransient[]): AutonomousWorkflowPortfolioEvidenceStatus {
  const requested = items.filter((item) => item.status !== "not_requested" && item.status !== "omitted");
  if (requested.length > 0 && requested.every((item) => item.status === "completed")) return "completed";
  if (items.some((item) => item.status === "reconciliation_required")) return "reconciliation_required";
  if (items.some((item) => item.status === "awaiting_evaluation")) return "awaiting_evaluation";
  if (requested.length > 0 && requested.every((item) => item.status === "failed")) return "failed";
  return "partial";
}

function itemJSON(item: EvidenceItemTransient): AutonomousWorkflowPortfolioEvidenceItemJSON {
  const json = item.runtime?.json;
  return {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
    item_id: item.itemId,
    domain: item.domain,
    provider_status: item.providerStatus,
    status: item.status,
    request_count: json?.receipts.length ?? 0,
    completed_requirement_count: json?.completed_requirement_ids.length ?? 0,
    pending_evaluation_count: json?.pending_evaluation_requirement_ids.length ?? 0,
    missing_requirement_count: json?.missing_requirement_ids.length ?? 0,
    result_digest: json?.result_digest ?? null,
    receipt_digests: json?.receipts.map((receipt) => receipt.receipt_digest) ?? [],
    assessment_digests: json?.assessments.map((assessment) => assessment.assessment_digest) ?? [],
    error_class: item.errorClass,
    retention: "metadata_only;raw_evidence_values_caller_owned",
    secret_material: "never_returned",
  };
}

async function metadataDigest(
  plan: AutonomousWorkflowPortfolioPlan,
  evidencePlan: AutonomousEvidencePlan,
  items: readonly EvidenceItemTransient[],
  status: AutonomousWorkflowPortfolioEvidenceStatus,
): Promise<string> {
  return digestJson({
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
    status,
    portfolio_plan_digest: plan.portfolio_digest,
    evidence_plan_digest: evidencePlan.plan_digest,
    items: items.map(itemJSON),
    retention: "metadata_only;raw_evidence_values_caller_owned" as const,
    secret_material: "never_returned" as const,
  });
}

function injectItemMetadata(request: AutonomousEvidenceAcquisitionRequest, item: AutonomousWorkflowPortfolioItemExecutionResult): AutonomousEvidenceAcquisitionRequest {
  const metadata = { ...(request.metadata ?? {}) } as Record<string, unknown>;
  const reserved = {
    portfolio_item_id: item.itemId,
    portfolio_item_domain: item.domain,
    portfolio_provider_status: item.status,
    portfolio_provider_output_digest: item.outputDigest,
  } as const;
  for (const key of Object.keys(reserved)) if (key in metadata) throw new ArgumentError(`evidence request metadata reserves ${key}`);
  return { ...request, metadata: { ...metadata, ...reserved } as JsonObject };
}

function dependencyEvidenceDigests(item: AutonomousWorkflowPortfolioItemExecutionResult, evidence: ReadonlyMap<string, EvidenceItemTransient>): string[] {
  return item.dependsOn.flatMap((dependency) => {
    const result = evidence.get(dependency)?.runtime?.json.result_digest;
    return result ? [result] : [];
  });
}

async function runBounded<T>(values: readonly string[], maximum: number, run: (value: string) => Promise<T>): Promise<T[]> {
  const results = new Map<string, T>();
  let cursor = 0;
  const worker = async (): Promise<void> => {
    while (cursor < values.length) {
      const value = values[cursor++];
      if (value === undefined) return;
      results.set(value, await run(value));
    }
  };
  await Promise.all(Array.from({ length: Math.min(maximum, values.length) }, () => worker()));
  return values.map((value) => results.get(value)!);
}

/** Metadata-only result; runtime evidence values remain available only through transient accessors. */
export class AutonomousWorkflowPortfolioEvidenceExecutionResult {
  constructor(
    readonly plan: AutonomousWorkflowPortfolioPlan,
    readonly evidencePlan: AutonomousEvidencePlan,
    readonly items: readonly AutonomousWorkflowPortfolioEvidenceItemTransient[],
    readonly status: AutonomousWorkflowPortfolioEvidenceStatus,
    readonly resultDigest: string,
  ) {}

  runtimeFor(itemId: string): AutonomousEvidenceRuntimeResult | null {
    boundedItemId(itemId);
    return this.items.find((item) => item.itemId === itemId)?.runtime ?? null;
  }

  toJSON(): AutonomousWorkflowPortfolioEvidenceJSON {
    const jsonItems = this.items.map(itemJSON);
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
      status: this.status,
      portfolio_plan_digest: this.plan.portfolio_digest,
      evidence_plan_digest: this.evidencePlan.plan_digest,
      completed_count: this.items.filter((item) => item.status === "completed").length,
      partial_count: this.items.filter((item) => item.status === "partial").length,
      awaiting_evaluation_count: this.items.filter((item) => item.status === "awaiting_evaluation").length,
      failed_count: this.items.filter((item) => item.status === "failed" || item.status === "reconciliation_required").length,
      omitted_count: this.items.filter((item) => item.status === "omitted").length,
      not_requested_count: this.items.filter((item) => item.status === "not_requested").length,
      items: jsonItems,
      result_digest: this.resultDigest,
      retention: "metadata_only;raw_evidence_values_caller_owned",
      secret_material: "never_returned",
    };
  }
}

/**
 * Acquire and evaluate evidence for a verified portfolio without replaying provider work.
 * Requests are executed in the same dependency waves as the portfolio and direct predecessor
 * evidence result digests are supplied to the caller-owned acquisition adapter.
 */
export async function executeAutonomousWorkflowPortfolioEvidence(
  agent: AutonomousAgent,
  execution: AutonomousWorkflowPortfolioExecutionResult,
  options: AutonomousWorkflowPortfolioEvidenceSupervisorOptions,
): Promise<AutonomousWorkflowPortfolioEvidenceExecutionResult> {
  if (!agent || typeof agent.evidencePlan !== "function") throw new ArgumentError("portfolio evidence execution requires an AutonomousAgent");
  if (!(execution instanceof AutonomousWorkflowPortfolioExecutionResult)) throw new ArgumentError("portfolio evidence execution requires a typed portfolio execution result");
  if (!options || !Array.isArray(options.items)) throw new ArgumentError("portfolio evidence execution items are required");
  const plan = options.plan ? await validateAutonomousWorkflowPortfolioPlan(options.plan) : execution.plan;
  if (plan.portfolio_digest !== execution.plan.portfolio_digest) throw new ArgumentError("portfolio evidence plan does not match the provider execution plan");
  const maxParallelism = boundedInteger("portfolio evidence maxParallelism", options.maxParallelism, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM, 4);
  if (options.stopOnFailure !== undefined && typeof options.stopOnFailure !== "boolean") throw new ArgumentError("portfolio evidence stopOnFailure must be boolean");
  if (!options.runtime || !options.runtime.acquirer || typeof options.runtime.acquirer.acquire !== "function") throw new ArgumentError("portfolio evidence runtime requires an acquirer");
  const domains = [...new Set(plan.items.map((item) => item.domain))];
  const evidencePlan = options.evidencePlan ?? await agent.evidencePlan(domains);
  if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("portfolio evidence plan is malformed");
  const planItems = new Map(plan.items.map((item) => [item.item_id, item]));
  const executionItems = new Map(execution.items.map((item) => [item.itemId, item]));
  if (executionItems.size !== execution.items.length) throw new ArgumentError("portfolio provider execution item ids must be unique");
  if (!Array.isArray(options.items) || options.items.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS) throw new ArgumentError("portfolio evidence item count is outside its bound");
  const requestsByItem = new Map<string, AutonomousWorkflowPortfolioEvidenceItemRequest>();
  for (const entry of options.items) {
    const itemId = boundedItemId(entry?.item_id);
    if (requestsByItem.has(itemId)) throw new ArgumentError(`portfolio evidence item ${itemId} is duplicated`);
    const planItem = planItems.get(itemId);
    if (!planItem) throw new ArgumentError(`portfolio evidence item ${itemId} is not in the reviewed plan`);
    if (!Array.isArray(entry.requests) || entry.requests.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS) throw new ArgumentError(`portfolio evidence requests for ${itemId} are outside their bound`);
    for (const request of entry.requests) {
      const requirement = evidencePlan.requirements.find((candidate) => candidate.requirement_id === request.requirement_id);
      if (!requirement) throw new ArgumentError(`portfolio evidence request ${request.requirement_id} is not in the evidence plan`);
      if (requirement.domain !== planItem.domain) throw new ArgumentError(`portfolio evidence request ${request.requirement_id} crosses item domain ${planItem.domain}`);
    }
    requestsByItem.set(itemId, { item_id: itemId, requests: [...entry.requests] });
  }
  const transient = new Map<string, EvidenceItemTransient>();
  const initial = plan.items.map((item) => {
    const provider = executionItems.get(item.item_id);
    if (!provider || provider.status !== "succeeded") return { itemId: item.item_id, domain: item.domain, providerStatus: provider?.status ?? "omitted", status: "omitted" as const, runtime: null, errorClass: "provider_execution_not_succeeded" };
    if (!requestsByItem.has(item.item_id) || requestsByItem.get(item.item_id)!.requests.length === 0) return { itemId: item.item_id, domain: item.domain, providerStatus: provider.status, status: "not_requested" as const, runtime: null, errorClass: null };
    return null;
  });
  for (const item of initial) if (item) transient.set(item.itemId, item);
  const snapshotItems = (): EvidenceItemTransient[] => plan.items.map((item) => transient.get(item.item_id) ?? {
    itemId: item.item_id,
    domain: item.domain,
    providerStatus: executionItems.get(item.item_id)?.status ?? "omitted",
    status: "omitted" as const,
    runtime: null,
    errorClass: "portfolio_evidence_not_scheduled",
  });
  const reportProgress = async (): Promise<void> => {
    if (!options.progressSink) return;
    const items = snapshotItems();
    const status = overallStatus(items);
    const resultDigest = await metadataDigest(plan, evidencePlan, items, status);
    await options.progressSink({ plan, evidencePlan, items: items.map(itemJSON), status, resultDigest });
  };
  let stopped = false;
  for (const wave of plan.dependency_graph.waves) {
    const waveIds = wave.filter((itemId) => !transient.has(itemId));
    if (stopped) {
      for (const itemId of waveIds) {
        const item = planItems.get(itemId)!;
        const provider = executionItems.get(itemId)!;
        transient.set(itemId, { itemId, domain: item.domain, providerStatus: provider.status, status: "omitted", runtime: null, errorClass: "portfolio_evidence_stopped_after_failure" });
      }
      await reportProgress();
      continue;
    }
    const runnable = waveIds.filter((itemId) => planItems.get(itemId)!.depends_on.every((dependency) => transient.get(dependency)?.status === "completed" || transient.get(dependency)?.status === "not_requested"));
    for (const itemId of waveIds.filter((candidate) => !runnable.includes(candidate))) {
      const item = planItems.get(itemId)!;
      const provider = executionItems.get(itemId)!;
      transient.set(itemId, { itemId, domain: item.domain, providerStatus: provider.status, status: "omitted", runtime: null, errorClass: "evidence_dependency_not_completed" });
    }
    const results = await runBounded(runnable, maxParallelism, async (itemId) => {
      const item = planItems.get(itemId)!;
      const provider = executionItems.get(itemId)!;
      const requestEntry = requestsByItem.get(itemId)!;
      try {
        const itemEvidencePlan = await scopedEvidencePlan(agent, item.domain, evidencePlan);
        const journal = options.journalFor?.({ itemId, domain: item.domain, evidencePlanDigest: itemEvidencePlan.plan_digest });
        const runtime = new AutonomousEvidenceRuntime({ plan: itemEvidencePlan, journal });
        await runtime.rehydrate();
        const requests = requestEntry.requests.map((request) => injectItemMetadata(request, provider));
        const result = await runtime.execute(requests, { ...options.runtime, parentEvidenceDigests: dependencyEvidenceDigests(provider, transient) });
        return { itemId, domain: item.domain, providerStatus: provider.status, status: statusForRuntime(result.json.status), runtime: result, errorClass: null } satisfies EvidenceItemTransient;
      } catch (error) {
        return { itemId, domain: item.domain, providerStatus: provider.status, status: "failed" as const, runtime: null, errorClass: errorClass(error) } satisfies EvidenceItemTransient;
      }
    });
    for (const result of results) transient.set(result.itemId, result);
    await reportProgress();
    if (options.stopOnFailure === true && results.some((item) => item.status === "failed" || item.status === "reconciliation_required")) stopped = true;
  }
  const items = snapshotItems();
  const status = overallStatus(items);
  const resultDigest = await metadataDigest(plan, evidencePlan, items, status);
  return new AutonomousWorkflowPortfolioEvidenceExecutionResult(plan, evidencePlan, items, status, resultDigest);
}
