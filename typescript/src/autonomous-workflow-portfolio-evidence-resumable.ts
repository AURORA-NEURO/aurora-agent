import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousAgent, AutonomousDomainName } from "./autonomous.js";
import { AutonomousEvidencePlan } from "./autonomous-evidence.js";
import {
  executeAutonomousWorkflowPortfolioEvidence,
  type AutonomousWorkflowPortfolioEvidenceExecutionResult,
  type AutonomousWorkflowPortfolioEvidenceItemJSON,
  type AutonomousWorkflowPortfolioEvidenceItemRequest,
  type AutonomousWorkflowPortfolioEvidenceItemStatus,
  type AutonomousWorkflowPortfolioEvidenceProgress,
  type AutonomousWorkflowPortfolioEvidenceStatus,
  type AutonomousWorkflowPortfolioEvidenceSupervisorOptions,
} from "./autonomous-workflow-portfolio-evidence.js";
import {
  AutonomousWorkflowPortfolioExecutionResult,
} from "./autonomous-workflow-portfolio-execution.js";
import {
  validateAutonomousWorkflowPortfolioPlan,
  type AutonomousWorkflowPortfolioPlan,
} from "./autonomous-workflow-portfolio.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Digest-bound metadata-only restart checkpoint for portfolio evidence waves. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-evidence-checkpoint/0.2" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES = 256_000;

export type AutonomousWorkflowPortfolioEvidenceCheckpointStatus =
  | "running"
  | "partial"
  | "completed"
  | "awaiting_evaluation"
  | "failed"
  | "reconciliation_required";

export interface AutonomousWorkflowPortfolioEvidenceCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA;
  job_id: string;
  portfolio_plan_digest: string;
  admission_digest: string | null;
  provider_execution_digest: string;
  evidence_plan_digest: string;
  evidence_input_digest: string;
  item_ids: string[];
  item_request_digests: string[];
  settled_item_ids: string[];
  settled_item_statuses: AutonomousWorkflowPortfolioEvidenceItemStatus[];
  settled_result_digests: string[];
  max_parallelism: number;
  stop_on_failure: boolean;
  reevaluate_pending: boolean;
  evaluator_id: string | null;
  evaluator_version: string | null;
  runtime_policy_digest: string | null;
  status: AutonomousWorkflowPortfolioEvidenceCheckpointStatus;
  checkpoint_digest: string;
  retention: "request_and_result_digests_only;raw_evidence_values_and_sources_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioEvidenceCheckpointStore {
  read(): Promise<AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null> | AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null;
  write(checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<void> | void;
  /** Optional atomic fence; false means another worker committed after this coordinator restored. */
  writeIfUnchanged?(expectedCheckpointDigest: string | null, checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioEvidenceCheckpointTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioEvidenceTransactionalCheckpointTextStore extends AutonomousWorkflowPortfolioEvidenceCheckpointTextStore {
  writeIfUnchanged(expectedCheckpointDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions extends AutonomousWorkflowPortfolioEvidenceSupervisorOptions {
  jobId: string;
  /** Require the provider execution to carry a separately reviewed portfolio admission. */
  requireAdmission?: boolean;
  checkpoint?: AutonomousWorkflowPortfolioEvidenceCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON) => Promise<void> | void;
  /** Digest of caller-owned source/evaluator policy not represented by the adapter identity. */
  runtimePolicyDigest?: string;
}

export interface AutonomousWorkflowPortfolioEvidenceControllerProjection extends JsonObject {
  schema: "bioprism-typescript-autonomous-workflow-portfolio-evidence-controller/0.1";
  status: "empty" | "restored" | "flushed" | "completed" | "partial" | "failed" | "awaiting_evaluation" | "reconciliation_required";
  job_id: string;
  checkpoint_digest: string | null;
  settled_items: number;
  total_items: number | null;
  persisted: true;
  retention: "metadata_only_request_and_result_digests;raw_evidence_values_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioEvidenceControllerRun {
  controller: AutonomousWorkflowPortfolioEvidenceControllerProjection;
  evidence: AutonomousWorkflowPortfolioEvidenceExecutionResult;
}

export type AutonomousWorkflowPortfolioEvidenceControllerRunOptions = Omit<
  AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions,
  "checkpoint" | "checkpointSink" | "jobId"
>;

const CHECKPOINT_RETENTION = "request_and_result_digests_only;raw_evidence_values_and_sources_never_persisted" as const;
const CHECKPOINT_SECRET_MATERIAL = "never_returned" as const;
const MAX_ITEMS = 64;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(value: unknown, name: string): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function optionalDigest(name: string, value: unknown): string | null {
  if (value === undefined || value === null) return null;
  return digest(value, name);
}

function optionalText(name: string, value: unknown): string | null {
  if (value === undefined || value === null) return null;
  return boundedIdentifier(name, value);
}

function checkpointable(status: AutonomousWorkflowPortfolioEvidenceItemStatus): boolean {
  return status === "completed" || status === "failed" || status === "omitted" || status === "not_requested";
}

function settledProgressItem(item: AutonomousWorkflowPortfolioEvidenceItemJSON): boolean {
  return checkpointable(item.status) && !(item.status === "omitted" && item.error_class === "portfolio_evidence_not_scheduled");
}

function checkpointStatusFor(status: AutonomousWorkflowPortfolioEvidenceStatus): AutonomousWorkflowPortfolioEvidenceCheckpointStatus {
  return status;
}

function controls(options: AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions): {
  maxParallelism: number;
  stopOnFailure: boolean;
  reevaluatePending: boolean;
  evaluatorId: string | null;
  evaluatorVersion: string | null;
  runtimePolicyDigest: string | null;
} {
  const maxParallelism = options.maxParallelism ?? 4;
  if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > 8) throw new ArgumentError("portfolio evidence resumable maxParallelism is outside its bound");
  const stopOnFailure = options.stopOnFailure ?? false;
  if (typeof stopOnFailure !== "boolean") throw new ArgumentError("portfolio evidence resumable stopOnFailure must be boolean");
  const reevaluatePending = options.runtime.reevaluatePending ?? false;
  if (typeof reevaluatePending !== "boolean") throw new ArgumentError("portfolio evidence resumable reevaluatePending must be boolean");
  const evaluatorId = options.runtime.evaluator?.evaluator_id === undefined ? null : boundedIdentifier("portfolio evidence evaluator_id", options.runtime.evaluator.evaluator_id);
  const evaluatorVersion = options.runtime.evaluator?.evaluator_version === undefined ? null : boundedIdentifier("portfolio evidence evaluator_version", options.runtime.evaluator.evaluator_version);
  if ((evaluatorId === null) !== (evaluatorVersion === null)) throw new ArgumentError("portfolio evidence evaluator identity must be complete");
  const runtimePolicyDigest = options.runtimePolicyDigest === undefined ? null : digest(options.runtimePolicyDigest, "portfolio evidence runtimePolicyDigest");
  return { maxParallelism, stopOnFailure, reevaluatePending, evaluatorId, evaluatorVersion, runtimePolicyDigest };
}

async function requestDigest(entry: AutonomousWorkflowPortfolioEvidenceItemRequest): Promise<string> {
  return digestJson({ schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA, item_id: entry.item_id, requests: entry.requests });
}

async function inputBinding(plan: AutonomousWorkflowPortfolioPlan, entries: readonly AutonomousWorkflowPortfolioEvidenceItemRequest[]): Promise<{ itemIds: string[]; itemRequestDigests: string[]; evidenceInputDigest: string }> {
  if (!Array.isArray(entries) || entries.length > MAX_ITEMS) throw new ArgumentError("portfolio evidence checkpoint items are outside their bound");
  const byId = new Map<string, AutonomousWorkflowPortfolioEvidenceItemRequest>();
  for (const entry of entries) {
    const id = boundedIdentifier("portfolio evidence checkpoint item_id", entry?.item_id);
    if (byId.has(id)) throw new ArgumentError(`portfolio evidence checkpoint item ${id} is duplicated`);
    byId.set(id, entry);
  }
  const itemIds = plan.items.map((item) => item.item_id);
  const itemRequestDigests = await Promise.all(itemIds.map((itemId) => requestDigest(byId.get(itemId) ?? { item_id: itemId, requests: [] })));
  const evidenceInputDigest = await digestJson({ schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA, items: itemIds.map((itemId, index) => ({ item_id: itemId, request_digest: itemRequestDigests[index] })) });
  return { itemIds, itemRequestDigests, evidenceInputDigest };
}

async function itemResultDigest(item: AutonomousWorkflowPortfolioEvidenceItemJSON): Promise<string> {
  return digestJson(item);
}

async function makeCheckpoint(input: {
  jobId: string;
  execution: AutonomousWorkflowPortfolioExecutionResult;
  progress: AutonomousWorkflowPortfolioEvidenceProgress;
  itemIds: readonly string[];
  itemRequestDigests: readonly string[];
  evidenceInputDigest: string;
  configuration: ReturnType<typeof controls>;
}): Promise<AutonomousWorkflowPortfolioEvidenceCheckpointJSON> {
  const settled = input.progress.items.filter(settledProgressItem);
  const settledItemIds = input.itemIds.filter((itemId) => settled.some((item) => item.item_id === itemId));
  const settledItems = settledItemIds.map((itemId) => input.progress.items.find((item) => item.item_id === itemId)!);
  const payload = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    portfolio_plan_digest: input.execution.plan.portfolio_digest,
    admission_digest: input.execution.admissionDigest,
    provider_execution_digest: input.execution.executionDigest,
    evidence_plan_digest: input.progress.evidencePlan.plan_digest,
    evidence_input_digest: input.evidenceInputDigest,
    item_ids: [...input.itemIds],
    item_request_digests: [...input.itemRequestDigests],
    settled_item_ids: settledItemIds,
    settled_item_statuses: settledItems.map((item) => item.status),
    settled_result_digests: await Promise.all(settledItems.map(itemResultDigest)),
    max_parallelism: input.configuration.maxParallelism,
    stop_on_failure: input.configuration.stopOnFailure,
    reevaluate_pending: input.configuration.reevaluatePending,
    evaluator_id: input.configuration.evaluatorId,
    evaluator_version: input.configuration.evaluatorVersion,
    runtime_policy_digest: input.configuration.runtimePolicyDigest,
    status: checkpointStatusFor(input.progress.status),
  };
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES) throw new ArgumentError("portfolio evidence checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: CHECKPOINT_RETENTION, secret_material: CHECKPOINT_SECRET_MATERIAL };
}

/** Validate checkpoint fields, retention, ordering, and its content digest. */
export async function validateAutonomousWorkflowPortfolioEvidenceCheckpoint(value: unknown): Promise<AutonomousWorkflowPortfolioEvidenceCheckpointJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA) throw new ArgumentError("portfolio evidence checkpoint schema is invalid");
  const allowed = new Set(["schema", "job_id", "portfolio_plan_digest", "admission_digest", "provider_execution_digest", "evidence_plan_digest", "evidence_input_digest", "item_ids", "item_request_digests", "settled_item_ids", "settled_item_statuses", "settled_result_digests", "max_parallelism", "stop_on_failure", "reevaluate_pending", "evaluator_id", "evaluator_version", "runtime_policy_digest", "status", "checkpoint_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("portfolio evidence checkpoint contains unsupported fields");
  const jobId = boundedIdentifier("portfolio evidence checkpoint job_id", value.job_id);
  const portfolioPlanDigest = digest(value.portfolio_plan_digest, "portfolio evidence checkpoint portfolio_plan_digest");
  const admissionDigest = optionalDigest("portfolio evidence checkpoint admission_digest", value.admission_digest);
  const providerExecutionDigest = digest(value.provider_execution_digest, "portfolio evidence checkpoint provider_execution_digest");
  const evidencePlanDigest = digest(value.evidence_plan_digest, "portfolio evidence checkpoint evidence_plan_digest");
  const evidenceInputDigest = digest(value.evidence_input_digest, "portfolio evidence checkpoint evidence_input_digest");
  const itemIds = value.item_ids;
  const itemRequestDigests = value.item_request_digests;
  if (!Array.isArray(itemIds) || itemIds.length < 1 || itemIds.length > MAX_ITEMS || itemIds.some((item) => typeof item !== "string")) throw new ArgumentError("portfolio evidence checkpoint item_ids are invalid");
  const normalizedItemIds = itemIds.map((item, index) => boundedIdentifier(`portfolio evidence checkpoint item_ids[${index}]`, item));
  if (new Set(normalizedItemIds).size !== normalizedItemIds.length) throw new ArgumentError("portfolio evidence checkpoint item_ids must be unique");
  if (!Array.isArray(itemRequestDigests) || itemRequestDigests.length !== normalizedItemIds.length || itemRequestDigests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) throw new ArgumentError("portfolio evidence checkpoint item_request_digests are invalid");
  const settledIds = value.settled_item_ids;
  const settledStatuses = value.settled_item_statuses;
  const settledDigests = value.settled_result_digests;
  if (!Array.isArray(settledIds) || !Array.isArray(settledStatuses) || !Array.isArray(settledDigests) || settledIds.length !== settledStatuses.length || settledIds.length !== settledDigests.length || settledIds.length > normalizedItemIds.length) throw new ArgumentError("portfolio evidence checkpoint settled rows are invalid");
  const normalizedSettledIds = settledIds.map((item, index) => boundedIdentifier(`portfolio evidence checkpoint settled_item_ids[${index}]`, item));
  if (new Set(normalizedSettledIds).size !== normalizedSettledIds.length || normalizedSettledIds.some((item) => !normalizedItemIds.includes(item)) || normalizedSettledIds.some((item, index) => index > 0 && normalizedItemIds.indexOf(item) < normalizedItemIds.indexOf(normalizedSettledIds[index - 1]!))) throw new ArgumentError("portfolio evidence checkpoint settled item ids are invalid");
  const normalizedSettledStatuses = settledStatuses.map((status) => status as AutonomousWorkflowPortfolioEvidenceItemStatus);
  if (normalizedSettledStatuses.some((status) => !checkpointable(status))) throw new ArgumentError("portfolio evidence checkpoint settled statuses are invalid");
  if (settledDigests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) throw new ArgumentError("portfolio evidence checkpoint settled result digests are invalid");
  if (!Number.isSafeInteger(value.max_parallelism) || (value.max_parallelism as number) < 1 || (value.max_parallelism as number) > 8 || typeof value.stop_on_failure !== "boolean" || typeof value.reevaluate_pending !== "boolean") throw new ArgumentError("portfolio evidence checkpoint controls are invalid");
  const evaluatorId = optionalText("portfolio evidence checkpoint evaluator_id", value.evaluator_id);
  const evaluatorVersion = optionalText("portfolio evidence checkpoint evaluator_version", value.evaluator_version);
  if ((evaluatorId === null) !== (evaluatorVersion === null)) throw new ArgumentError("portfolio evidence checkpoint evaluator identity is incomplete");
  const runtimePolicyDigest = optionalDigest("portfolio evidence checkpoint runtime_policy_digest", value.runtime_policy_digest);
  const status = value.status as AutonomousWorkflowPortfolioEvidenceCheckpointStatus;
  if (!["running", "partial", "completed", "awaiting_evaluation", "failed", "reconciliation_required"].includes(status)) throw new ArgumentError("portfolio evidence checkpoint status is invalid");
  if (value.retention !== CHECKPOINT_RETENTION || value.secret_material !== CHECKPOINT_SECRET_MATERIAL) throw new ArgumentError("portfolio evidence checkpoint retention contract is invalid");
  const payload = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
    job_id: jobId,
    portfolio_plan_digest: portfolioPlanDigest,
    admission_digest: admissionDigest,
    provider_execution_digest: providerExecutionDigest,
    evidence_plan_digest: evidencePlanDigest,
    evidence_input_digest: evidenceInputDigest,
    item_ids: normalizedItemIds,
    item_request_digests: [...itemRequestDigests as string[]],
    settled_item_ids: normalizedSettledIds,
    settled_item_statuses: normalizedSettledStatuses,
    settled_result_digests: [...settledDigests as string[]],
    max_parallelism: value.max_parallelism as number,
    stop_on_failure: value.stop_on_failure as boolean,
    reevaluate_pending: value.reevaluate_pending as boolean,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    runtime_policy_digest: runtimePolicyDigest,
    status,
  };
  if (await digestJson(payload) !== value.checkpoint_digest) throw new ArgumentError("portfolio evidence checkpoint digest is invalid");
  return structuredClone({ ...payload, checkpoint_digest: value.checkpoint_digest as string, retention: value.retention, secret_material: value.secret_material }) as AutonomousWorkflowPortfolioEvidenceCheckpointJSON;
}

async function validateBinding(
  plan: AutonomousWorkflowPortfolioPlan,
  execution: AutonomousWorkflowPortfolioExecutionResult,
  evidencePlan: AutonomousEvidencePlan,
  checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON,
  binding: Awaited<ReturnType<typeof inputBinding>>,
  configuration: ReturnType<typeof controls>,
): Promise<void> {
  if (checkpoint.portfolio_plan_digest !== plan.portfolio_digest || checkpoint.admission_digest !== execution.admissionDigest || checkpoint.provider_execution_digest !== execution.executionDigest || checkpoint.evidence_plan_digest !== evidencePlan.plan_digest || checkpoint.evidence_input_digest !== binding.evidenceInputDigest || JSON.stringify(checkpoint.item_ids) !== JSON.stringify(binding.itemIds) || JSON.stringify(checkpoint.item_request_digests) !== JSON.stringify(binding.itemRequestDigests)) throw new ArgumentError("portfolio evidence checkpoint does not match the current reviewed execution or evidence input");
  if (checkpoint.max_parallelism !== configuration.maxParallelism || checkpoint.stop_on_failure !== configuration.stopOnFailure || checkpoint.reevaluate_pending !== configuration.reevaluatePending || checkpoint.evaluator_id !== configuration.evaluatorId || checkpoint.evaluator_version !== configuration.evaluatorVersion || checkpoint.runtime_policy_digest !== configuration.runtimePolicyDigest) throw new ArgumentError("portfolio evidence checkpoint controls do not match");
  if (checkpoint.status === "completed" && (checkpoint.settled_item_ids.length !== plan.items.length || checkpoint.settled_item_statuses.some((status) => status !== "completed"))) throw new ArgumentError("completed portfolio evidence checkpoint is not complete");
}

function scopedPlanForItem(
  evidencePlan: AutonomousEvidencePlan,
  domain: AutonomousDomainName,
  agent: AutonomousAgent,
): Promise<AutonomousEvidencePlan> {
  return agent.evidencePlan([domain], { availableEvidence: evidencePlan.available_evidence });
}

async function requireReplayBoundary(
  agent: AutonomousAgent,
  plan: AutonomousWorkflowPortfolioPlan,
  evidencePlan: AutonomousEvidencePlan,
  checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON,
  journalFor: NonNullable<AutonomousWorkflowPortfolioEvidenceSupervisorOptions["journalFor"]>,
): Promise<void> {
  for (const itemId of checkpoint.settled_item_ids) {
    const item = plan.items.find((candidate) => candidate.item_id === itemId)!;
    if (checkpoint.settled_item_statuses[checkpoint.settled_item_ids.indexOf(itemId)] !== "completed") continue;
    const scoped = await scopedPlanForItem(evidencePlan, item.domain, agent);
    const journal = journalFor({ itemId, domain: item.domain, evidencePlanDigest: scoped.plan_digest });
    if (!journal || typeof journal.append !== "function" || typeof journal.records !== "function") throw new ArgumentError(`portfolio evidence resume requires a journal for completed item ${itemId}`);
  }
}

/** Execute portfolio evidence with digest-bound wave checkpoints and journal-backed replay. */
export async function executeAutonomousWorkflowPortfolioEvidenceResumable(
  agent: AutonomousAgent,
  execution: AutonomousWorkflowPortfolioExecutionResult,
  options: AutonomousWorkflowPortfolioEvidenceResumableExecutionOptions,
): Promise<AutonomousWorkflowPortfolioEvidenceExecutionResult> {
  if (!options || options.jobId === undefined) throw new ArgumentError("portfolio evidence resumable execution requires jobId");
  const jobId = boundedIdentifier("portfolio evidence resumable jobId", options.jobId);
  if (!execution || !(execution instanceof AutonomousWorkflowPortfolioExecutionResult)) throw new ArgumentError("portfolio evidence resumable execution requires a typed provider execution result");
  if (typeof options.checkpointSink !== "function") throw new ArgumentError("portfolio evidence resumable execution requires checkpointSink");
  if (options.requireAdmission !== undefined && typeof options.requireAdmission !== "boolean") throw new ArgumentError("portfolio evidence resumable requireAdmission must be boolean");
  if (execution.admissionDigest !== null) digest(execution.admissionDigest, "portfolio evidence execution admission_digest");
  if (options.requireAdmission === true && execution.admissionDigest === null) throw new ArgumentError("portfolio evidence resumable execution requires a reviewed portfolio admission");
  const configuration = controls(options);
  const plan = options.plan ? await validateAutonomousWorkflowPortfolioPlan(options.plan) : execution.plan;
  if (plan.portfolio_digest !== execution.plan.portfolio_digest) throw new ArgumentError("portfolio evidence resumable plan does not match provider execution");
  const domains = [...new Set(plan.items.map((item) => item.domain))];
  const evidencePlan = options.evidencePlan ?? await agent.evidencePlan(domains);
  if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("portfolio evidence resumable evidence plan is malformed");
  const binding = await inputBinding(plan, options.items);
  const restored = options.checkpoint === undefined ? null : await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(options.checkpoint);
  if (restored !== null) {
    if (restored.job_id !== jobId) throw new ArgumentError("portfolio evidence checkpoint job id does not match");
    await validateBinding(plan, execution, evidencePlan, restored, binding, configuration);
    if (restored.settled_item_statuses.includes("completed")) {
      if (typeof options.journalFor !== "function" || typeof options.runtime.rehydrateValue !== "function") throw new ArgumentError("portfolio evidence resume requires journalFor and rehydrateValue for completed items");
      await requireReplayBoundary(agent, plan, evidencePlan, restored, options.journalFor);
    }
  }
  const { checkpoint: _checkpoint, checkpointSink: _checkpointSink, jobId: _jobId, requireAdmission: _requireAdmission, runtimePolicyDigest: _runtimePolicyDigest, progressSink: callerProgressSink, ...supervisorOptions } = options;
  const progressSink = async (progress: AutonomousWorkflowPortfolioEvidenceProgress): Promise<void> => {
    const checkpoint = await makeCheckpoint({ jobId, execution, progress, itemIds: binding.itemIds, itemRequestDigests: binding.itemRequestDigests, evidenceInputDigest: binding.evidenceInputDigest, configuration });
    await options.checkpointSink!(checkpoint);
    await callerProgressSink?.(progress);
  };
  return executeAutonomousWorkflowPortfolioEvidence(agent, execution, {
    ...supervisorOptions,
    plan,
    evidencePlan,
    progressSink,
  });
}

/** In-memory checkpoint adapter useful for tests and local desktop workers. */
export class InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore implements AutonomousWorkflowPortfolioEvidenceCheckpointStore {
  private checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null;

  constructor(initial?: AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null) {
    this.checkpoint = initial === undefined || initial === null ? null : structuredClone(initial);
  }

  async read(): Promise<AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null> {
    return this.checkpoint === null ? null : await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(this.checkpoint);
  }

  async write(checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<void> {
    this.checkpoint = structuredClone(await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(checkpoint));
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<boolean> {
    const current = this.checkpoint?.checkpoint_digest ?? null;
    if (current !== expectedCheckpointDigest) return false;
    await this.write(checkpoint);
    return true;
  }
}

/** JSON adapter for browser, Node, or embedded text stores. */
export class JsonAutonomousWorkflowPortfolioEvidenceCheckpointStore implements AutonomousWorkflowPortfolioEvidenceCheckpointStore {
  protected readonly store: AutonomousWorkflowPortfolioEvidenceCheckpointTextStore;

  constructor(store: AutonomousWorkflowPortfolioEvidenceCheckpointTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("portfolio evidence JSON checkpoint store is malformed");
    this.store = store;
  }

  async read(): Promise<AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES) throw new ArgumentError("portfolio evidence JSON checkpoint text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("portfolio evidence JSON checkpoint text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("portfolio evidence JSON checkpoint text is not canonical");
    return validateAutonomousWorkflowPortfolioEvidenceCheckpoint(parsed);
  }

  async write(checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<void> {
    await this.store.write(this.encode(await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(checkpoint)));
  }

  protected encode(checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): string {
    const encoded = canonicalJson(checkpoint);
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES) throw new ArgumentError("portfolio evidence JSON checkpoint exceeds its bound");
    return encoded;
  }
}

/** JSON checkpoint adapter that refuses to operate without an atomic compare-and-swap fence. */
export class TransactionalJsonAutonomousWorkflowPortfolioEvidenceCheckpointStore extends JsonAutonomousWorkflowPortfolioEvidenceCheckpointStore {
  private readonly transactionalStore: AutonomousWorkflowPortfolioEvidenceTransactionalCheckpointTextStore;

  constructor(store: AutonomousWorkflowPortfolioEvidenceTransactionalCheckpointTextStore) {
    super(store);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional portfolio evidence JSON checkpoint store requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON): Promise<boolean> {
    const committed = await this.transactionalStore.writeIfUnchanged(expectedCheckpointDigest, this.encode(await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(checkpoint)));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional portfolio evidence JSON checkpoint store returned a non-boolean commit result");
    return committed;
  }
}

/** Restart-aware evidence controller; the caller owns journals, values, and durable storage. */
export class AutonomousWorkflowPortfolioEvidenceController {
  private checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpointJSON | null = null;
  private expectedCheckpointDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();
  private controllerStatus: AutonomousWorkflowPortfolioEvidenceControllerProjection["status"] = "empty";
  private totalItems: number | null = null;

  constructor(
    readonly agent: AutonomousAgent,
    readonly jobId: string,
    readonly persistence: AutonomousWorkflowPortfolioEvidenceCheckpointStore,
  ) {
    boundedIdentifier("portfolio evidence controller jobId", jobId);
  }

  async restore(): Promise<AutonomousWorkflowPortfolioEvidenceControllerProjection> {
    return this.enqueue(async () => {
      const stored = await this.persistence.read();
      this.checkpoint = stored === null ? null : await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(stored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      this.controllerStatus = this.checkpoint === null ? "empty" : "restored";
      this.totalItems = this.checkpoint?.item_ids.length ?? null;
      return this.projection();
    });
  }

  async run(
    execution: AutonomousWorkflowPortfolioExecutionResult,
    options: AutonomousWorkflowPortfolioEvidenceControllerRunOptions,
  ): Promise<AutonomousWorkflowPortfolioEvidenceControllerRun> {
    return this.enqueue(async () => {
      const restored = this.checkpoint ?? await this.persistence.read();
      this.checkpoint = restored === null ? null : await validateAutonomousWorkflowPortfolioEvidenceCheckpoint(restored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      const evidence = await executeAutonomousWorkflowPortfolioEvidenceResumable(this.agent, execution, {
        ...options,
        jobId: this.jobId,
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          if (typeof this.persistence.writeIfUnchanged === "function") {
            const committed = await this.persistence.writeIfUnchanged(this.expectedCheckpointDigest, checkpoint);
            if (!committed) throw new ArgumentError("portfolio evidence checkpoint compare-and-swap conflict; reload before continuing");
          } else {
            await this.persistence.write(checkpoint);
          }
          this.checkpoint = checkpoint;
          this.expectedCheckpointDigest = checkpoint.checkpoint_digest;
          this.controllerStatus = "flushed";
          this.totalItems = checkpoint.item_ids.length;
        },
      });
      this.controllerStatus = evidence.status === "completed" ? "completed" : evidence.status === "failed" ? "failed" : evidence.status === "awaiting_evaluation" ? "awaiting_evaluation" : evidence.status === "reconciliation_required" ? "reconciliation_required" : "partial";
      this.totalItems = evidence.items.length;
      return { controller: this.projection(), evidence };
    });
  }

  projection(): AutonomousWorkflowPortfolioEvidenceControllerProjection {
    return {
      schema: "bioprism-typescript-autonomous-workflow-portfolio-evidence-controller/0.1",
      status: this.controllerStatus,
      job_id: this.jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      settled_items: this.checkpoint?.settled_item_ids.length ?? 0,
      total_items: this.totalItems,
      persisted: true,
      retention: "metadata_only_request_and_result_digests;raw_evidence_values_never_persisted",
      secret_material: "never_returned",
    };
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
