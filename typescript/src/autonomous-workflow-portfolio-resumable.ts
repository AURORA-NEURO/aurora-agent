import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousAgent, AutonomousDomainName } from "./autonomous.js";
import {
  DEFAULT_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES,
  MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES,
  MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM,
  autonomousWorkflowPortfolioTransientOutput,
  digestAutonomousWorkflowPortfolioExecutionItem,
  executeAutonomousWorkflowPortfolioWithInitialItems,
  isAutonomousWorkflowPortfolioHardFailure,
  AutonomousWorkflowPortfolioExecutionResult,
  AutonomousWorkflowPortfolioItemExecutionResult,
  type AutonomousWorkflowPortfolioExecutionItemStatus,
  type AutonomousWorkflowPortfolioExecutionOptions,
  type AutonomousWorkflowPortfolioExecutionProgress,
  type AutonomousWorkflowPortfolioExecutionStatus,
} from "./autonomous-workflow-portfolio-execution.js";
import {
  planAutonomousWorkflowPortfolio,
  validateAutonomousWorkflowPortfolioPlan,
  type AutonomousWorkflowPortfolioItemRequest,
  type AutonomousWorkflowPortfolioPlan,
} from "./autonomous-workflow-portfolio.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only restart checkpoint for a verified workflow portfolio. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-execution-checkpoint/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_BYTES = 256_000;

export type AutonomousWorkflowPortfolioCheckpointStatus = "running" | "partial" | "completed" | "blocked";

export interface AutonomousWorkflowPortfolioExecutionCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA;
  job_id: string;
  plan_digest: string;
  portfolio_input_digest: string;
  item_ids: string[];
  request_digests: string[];
  task_digests: string[];
  settled_item_ids: string[];
  settled_item_statuses: AutonomousWorkflowPortfolioExecutionItemStatus[];
  settled_result_digests: string[];
  max_parallelism: number;
  stop_on_error: boolean;
  include_dependency_outputs: boolean;
  max_dependency_handoff_bytes: number;
  status: AutonomousWorkflowPortfolioCheckpointStatus;
  checkpoint_digest: string;
  retention: "request_and_result_digests_only;tasks_prompts_credentials_and_provider_payloads_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionRehydrationContext {
  job_id: string;
  item_id: string;
  domain: AutonomousDomainName;
  request_digest: string;
  task_digest: string;
  expected_status: AutonomousWorkflowPortfolioExecutionItemStatus;
  expected_result_digest: string;
}

/** Caller-owned durable storage for one portfolio checkpoint. */
export interface AutonomousWorkflowPortfolioExecutionCheckpointStore {
  read(): Promise<AutonomousWorkflowPortfolioExecutionCheckpointJSON | null> | AutonomousWorkflowPortfolioExecutionCheckpointJSON | null;
  write(checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioResumableExecutionOptions extends AutonomousWorkflowPortfolioExecutionOptions {
  jobId: string;
  checkpoint?: AutonomousWorkflowPortfolioExecutionCheckpointJSON;
  checkpointSink?: (checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON) => Promise<void> | void;
  rehydrateItem?: (context: AutonomousWorkflowPortfolioExecutionRehydrationContext) => Promise<AutonomousWorkflowPortfolioItemExecutionResult> | AutonomousWorkflowPortfolioItemExecutionResult;
}

export type AutonomousWorkflowPortfolioExecutionControllerStatus = "empty" | "restored" | "flushed" | "completed" | "partial" | "failed";

export interface AutonomousWorkflowPortfolioExecutionControllerProjection extends JsonObject {
  schema: "bioprism-typescript-autonomous-workflow-portfolio-execution-controller/0.1";
  status: AutonomousWorkflowPortfolioExecutionControllerStatus;
  job_id: string;
  checkpoint_digest: string | null;
  settled_items: number;
  total_items: number | null;
  persisted: true;
  retention: "metadata_only_request_and_result_digests;task_prompt_provider_values_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioExecutionControllerRun {
  controller: AutonomousWorkflowPortfolioExecutionControllerProjection;
  execution: AutonomousWorkflowPortfolioExecutionResult;
}

export type AutonomousWorkflowPortfolioExecutionControllerRunOptions = Omit<
  AutonomousWorkflowPortfolioResumableExecutionOptions,
  "checkpoint" | "checkpointSink" | "jobId"
>;

const CHECKPOINT_RETENTION = "request_and_result_digests_only;tasks_prompts_credentials_and_provider_payloads_never_persisted" as const;
const CHECKPOINT_SECRET_MATERIAL = "never_returned" as const;

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

function boundedControls(options: AutonomousWorkflowPortfolioResumableExecutionOptions): { maxParallelism: number; stopOnError: boolean; includeDependencyOutputs: boolean; maxDependencyHandoffBytes: number } {
  const maxParallelism = options.maxParallelism ?? 4;
  if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM) throw new ArgumentError("workflow portfolio resumable maxParallelism is outside its bound");
  const stopOnError = options.stopOnError ?? false;
  if (typeof stopOnError !== "boolean") throw new ArgumentError("workflow portfolio resumable stopOnError must be boolean");
  const includeDependencyOutputs = options.includeDependencyOutputs !== false;
  const maxDependencyHandoffBytes = options.maxDependencyHandoffBytes ?? DEFAULT_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES;
  if (!Number.isSafeInteger(maxDependencyHandoffBytes) || maxDependencyHandoffBytes < 512 || maxDependencyHandoffBytes > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES) throw new ArgumentError("workflow portfolio resumable maxDependencyHandoffBytes is outside its bound");
  return { maxParallelism, stopOnError, includeDependencyOutputs, maxDependencyHandoffBytes };
}

function checkpointable(status: AutonomousWorkflowPortfolioExecutionItemStatus): boolean {
  return status === "succeeded" || isAutonomousWorkflowPortfolioHardFailure(status);
}

function checkpointStatusFor(status: AutonomousWorkflowPortfolioExecutionStatus): AutonomousWorkflowPortfolioCheckpointStatus {
  if (status === "completed") return "completed";
  if (status === "blocked") return "blocked";
  return "partial";
}

async function portfolioInputDigest(plan: AutonomousWorkflowPortfolioPlan): Promise<string> {
  return digestJson({
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA,
    plan_digest: plan.portfolio_digest,
    items: plan.items.map((item) => ({
      item_id: item.item_id,
      domain: item.domain,
      depends_on: [...item.depends_on],
      task_digest: item.task_digest,
      request_digest: item.request_digest,
      status: item.status,
    })),
  });
}

async function makeCheckpoint(input: {
  jobId: string;
  plan: AutonomousWorkflowPortfolioPlan;
  progress: AutonomousWorkflowPortfolioExecutionProgress;
  maxParallelism: number;
  stopOnError: boolean;
  includeDependencyOutputs: boolean;
  maxDependencyHandoffBytes: number;
}): Promise<AutonomousWorkflowPortfolioExecutionCheckpointJSON> {
  const ready = new Set(input.plan.items.filter((item) => item.status === "ready").map((item) => item.item_id));
  const byId = new Map(input.progress.items.map((item) => [item.itemId, item]));
  const settled = input.plan.items
    .filter((item) => ready.has(item.item_id))
    .map((item) => byId.get(item.item_id))
    .filter((item): item is AutonomousWorkflowPortfolioItemExecutionResult => item !== undefined && checkpointable(item.status));
  const payload = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    plan_digest: input.plan.portfolio_digest,
    portfolio_input_digest: await portfolioInputDigest(input.plan),
    item_ids: input.plan.items.map((item) => item.item_id),
    request_digests: input.plan.items.map((item) => item.request_digest),
    task_digests: input.plan.items.map((item) => item.task_digest),
    settled_item_ids: settled.map((item) => item.itemId),
    settled_item_statuses: settled.map((item) => item.status),
    settled_result_digests: await Promise.all(settled.map((item) => digestAutonomousWorkflowPortfolioExecutionItem(item))),
    max_parallelism: input.maxParallelism,
    stop_on_error: input.stopOnError,
    include_dependency_outputs: input.includeDependencyOutputs,
    max_dependency_handoff_bytes: input.maxDependencyHandoffBytes,
    status: checkpointStatusFor(input.progress.status),
  };
  const encoded = JSON.stringify(payload);
  if (bytes(encoded) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_BYTES) throw new ArgumentError("workflow portfolio execution checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: CHECKPOINT_RETENTION, secret_material: CHECKPOINT_SECRET_MATERIAL };
}

/** Validate checkpoint structure, identity, digest, and retention markers before reuse. */
export async function validateAutonomousWorkflowPortfolioExecutionCheckpoint(value: unknown): Promise<AutonomousWorkflowPortfolioExecutionCheckpointJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA) throw new ArgumentError("workflow portfolio execution checkpoint schema is invalid");
  const allowed = new Set(["schema", "job_id", "plan_digest", "portfolio_input_digest", "item_ids", "request_digests", "task_digests", "settled_item_ids", "settled_item_statuses", "settled_result_digests", "max_parallelism", "stop_on_error", "include_dependency_outputs", "max_dependency_handoff_bytes", "status", "checkpoint_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("workflow portfolio execution checkpoint contains unsupported fields");
  const jobId = boundedIdentifier("workflow portfolio execution checkpoint job_id", value.job_id);
  const planDigest = digest(value.plan_digest, "workflow portfolio execution checkpoint plan_digest");
  const inputDigest = digest(value.portfolio_input_digest, "workflow portfolio execution checkpoint portfolio_input_digest");
  const itemIds = value.item_ids;
  const requestDigests = value.request_digests;
  const taskDigests = value.task_digests;
  if (!Array.isArray(itemIds) || itemIds.length < 1 || itemIds.length > 64 || itemIds.some((item) => typeof item !== "string")) throw new ArgumentError("workflow portfolio execution checkpoint item_ids are invalid");
  const normalizedItemIds = itemIds.map((item, index) => boundedIdentifier(`workflow portfolio execution checkpoint item_ids[${index}]`, item));
  if (new Set(normalizedItemIds).size !== normalizedItemIds.length) throw new ArgumentError("workflow portfolio execution checkpoint item_ids must be unique");
  if (!Array.isArray(requestDigests) || requestDigests.length !== normalizedItemIds.length || requestDigests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) throw new ArgumentError("workflow portfolio execution checkpoint request_digests are invalid");
  if (!Array.isArray(taskDigests) || taskDigests.length !== normalizedItemIds.length || taskDigests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) throw new ArgumentError("workflow portfolio execution checkpoint task_digests are invalid");
  const settledIds = value.settled_item_ids;
  const settledStatuses = value.settled_item_statuses;
  const settledDigests = value.settled_result_digests;
  if (!Array.isArray(settledIds) || !Array.isArray(settledStatuses) || !Array.isArray(settledDigests) || settledIds.length !== settledStatuses.length || settledIds.length !== settledDigests.length || settledIds.length > normalizedItemIds.length) throw new ArgumentError("workflow portfolio execution checkpoint settled rows are invalid");
  const normalizedSettledIds = settledIds.map((item, index) => boundedIdentifier(`workflow portfolio execution checkpoint settled_item_ids[${index}]`, item));
  if (new Set(normalizedSettledIds).size !== normalizedSettledIds.length || normalizedSettledIds.some((item) => !normalizedItemIds.includes(item)) || normalizedSettledIds.some((item, index) => index > 0 && item.localeCompare(normalizedSettledIds[index - 1]!) < 0)) throw new ArgumentError("workflow portfolio execution checkpoint settled item ids are invalid");
  const normalizedSettledStatuses = settledStatuses.map((status) => status as AutonomousWorkflowPortfolioExecutionItemStatus);
  if (normalizedSettledStatuses.some((status) => !checkpointable(status))) throw new ArgumentError("workflow portfolio execution checkpoint settled statuses are invalid");
  if (settledDigests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) throw new ArgumentError("workflow portfolio execution checkpoint settled result digests are invalid");
  if (!Number.isSafeInteger(value.max_parallelism) || (value.max_parallelism as number) < 1 || (value.max_parallelism as number) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_PARALLELISM || typeof value.stop_on_error !== "boolean" || typeof value.include_dependency_outputs !== "boolean" || !Number.isSafeInteger(value.max_dependency_handoff_bytes) || (value.max_dependency_handoff_bytes as number) < 512 || (value.max_dependency_handoff_bytes as number) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HANDOFF_BYTES) throw new ArgumentError("workflow portfolio execution checkpoint controls are invalid");
  if (!["running", "partial", "completed", "blocked"].includes(value.status as string)) throw new ArgumentError("workflow portfolio execution checkpoint status is invalid");
  if (value.retention !== CHECKPOINT_RETENTION || value.secret_material !== CHECKPOINT_SECRET_MATERIAL) throw new ArgumentError("workflow portfolio execution checkpoint retention contract is invalid");
  const payload = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: jobId,
    plan_digest: planDigest,
    portfolio_input_digest: inputDigest,
    item_ids: normalizedItemIds,
    request_digests: [...requestDigests as string[]],
    task_digests: [...taskDigests as string[]],
    settled_item_ids: normalizedSettledIds,
    settled_item_statuses: normalizedSettledStatuses,
    settled_result_digests: [...settledDigests as string[]],
    max_parallelism: value.max_parallelism as number,
    stop_on_error: value.stop_on_error as boolean,
    include_dependency_outputs: value.include_dependency_outputs as boolean,
    max_dependency_handoff_bytes: value.max_dependency_handoff_bytes as number,
    status: value.status as AutonomousWorkflowPortfolioCheckpointStatus,
  };
  if (await digestJson(payload) !== value.checkpoint_digest) throw new ArgumentError("workflow portfolio execution checkpoint digest is invalid");
  return structuredClone({ ...payload, checkpoint_digest: value.checkpoint_digest as string, retention: value.retention, secret_material: value.secret_material }) as AutonomousWorkflowPortfolioExecutionCheckpointJSON;
}

async function validatePlanBinding(plan: AutonomousWorkflowPortfolioPlan, checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON, controls: ReturnType<typeof boundedControls>): Promise<void> {
  const inputDigest = await portfolioInputDigest(plan);
  if (checkpoint.plan_digest !== plan.portfolio_digest || checkpoint.portfolio_input_digest !== inputDigest || JSON.stringify(checkpoint.item_ids) !== JSON.stringify(plan.items.map((item) => item.item_id)) || JSON.stringify(checkpoint.request_digests) !== JSON.stringify(plan.items.map((item) => item.request_digest)) || JSON.stringify(checkpoint.task_digests) !== JSON.stringify(plan.items.map((item) => item.task_digest))) throw new ArgumentError("workflow portfolio execution checkpoint does not match the current reviewed plan");
  if (checkpoint.max_parallelism !== controls.maxParallelism || checkpoint.stop_on_error !== controls.stopOnError || checkpoint.include_dependency_outputs !== controls.includeDependencyOutputs || checkpoint.max_dependency_handoff_bytes !== controls.maxDependencyHandoffBytes) throw new ArgumentError("workflow portfolio execution checkpoint controls do not match");
  const readyItemIds = plan.items.filter((item) => item.status === "ready").map((item) => item.item_id);
  if (checkpoint.settled_item_ids.some((itemId) => !readyItemIds.includes(itemId))) throw new ArgumentError("workflow portfolio execution checkpoint settles an item that was not executable in the reviewed plan");
  if (checkpoint.status === "completed" && (plan.status === "blocked" || JSON.stringify(checkpoint.settled_item_ids) !== JSON.stringify(readyItemIds) || checkpoint.settled_item_statuses.some((status) => status !== "succeeded"))) throw new ArgumentError("completed workflow portfolio execution checkpoint is not complete");
  if (checkpoint.status === "blocked" && plan.status !== "blocked") throw new ArgumentError("blocked workflow portfolio execution checkpoint does not match the reviewed plan status");
}

async function validateRehydratedItem(
  planItem: AutonomousWorkflowPortfolioPlan["items"][number],
  expectedStatus: AutonomousWorkflowPortfolioExecutionItemStatus,
  expectedDigest: string,
  item: AutonomousWorkflowPortfolioItemExecutionResult,
): Promise<void> {
  if (!item || item.itemId !== planItem.item_id || item.domain !== planItem.domain || JSON.stringify(item.dependsOn) !== JSON.stringify(planItem.depends_on) || item.status !== expectedStatus || !checkpointable(item.status)) throw new ArgumentError(`rehydrated workflow portfolio item ${planItem.item_id} does not match its checkpoint`);
  if (item.run) {
    const output = autonomousWorkflowPortfolioTransientOutput(item.run);
    const outputDigest = output.text ? await digestJson({ item_id: item.itemId, output: output.text }) : null;
    if (item.outputBytes !== output.bytes || item.outputDigest !== outputDigest) throw new ArgumentError(`rehydrated workflow portfolio item ${planItem.item_id} output digest does not match its checkpoint`);
  } else if (item.outputBytes !== 0 || item.outputDigest !== null) {
    throw new ArgumentError(`rehydrated workflow portfolio item ${planItem.item_id} has an output without a run`);
  }
  if (item.status === "succeeded" && item.run?.status !== "completed") throw new ArgumentError(`rehydrated workflow portfolio item ${planItem.item_id} is not a completed run`);
  if (await digestAutonomousWorkflowPortfolioExecutionItem(item) !== expectedDigest) throw new ArgumentError(`rehydrated workflow portfolio item ${planItem.item_id} result digest does not match its checkpoint`);
}

/** Execute a portfolio with restart-safe, digest-bound settled-item rehydration. */
export async function executeAutonomousWorkflowPortfolioResumable(
  agent: AutonomousAgent,
  requests: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioResumableExecutionOptions,
): Promise<AutonomousWorkflowPortfolioExecutionResult> {
  if (!options || options.jobId === undefined) throw new ArgumentError("workflow portfolio resumable execution requires jobId");
  const jobId = boundedIdentifier("workflow portfolio resumable execution jobId", options.jobId);
  if (options.checkpointSink !== undefined && typeof options.checkpointSink !== "function") throw new ArgumentError("workflow portfolio resumable checkpointSink must be callable");
  if (options.rehydrateItem !== undefined && typeof options.rehydrateItem !== "function") throw new ArgumentError("workflow portfolio resumable rehydrateItem must be callable");
  const controls = boundedControls(options);
  const plan = options.plan ? await validateAutonomousWorkflowPortfolioPlan(options.plan) : await planAutonomousWorkflowPortfolio(agent, requests, options.planOptions);
  const restored = options.checkpoint === undefined ? null : await validateAutonomousWorkflowPortfolioExecutionCheckpoint(options.checkpoint);
  if (restored !== null) {
    if (restored.job_id !== jobId) throw new ArgumentError("workflow portfolio execution checkpoint job id does not match");
    await validatePlanBinding(plan, restored, controls);
    if (restored.settled_item_ids.length > 0 && options.rehydrateItem === undefined) throw new ArgumentError("resuming a workflow portfolio requires rehydrateItem for settled items");
  }

  const rehydrated: AutonomousWorkflowPortfolioItemExecutionResult[] = [];
  if (restored && options.rehydrateItem) {
    const planItems = new Map(plan.items.map((item) => [item.item_id, item]));
    for (let index = 0; index < restored.settled_item_ids.length; index += 1) {
      const itemId = restored.settled_item_ids[index]!;
      const planItem = planItems.get(itemId);
      if (!planItem) throw new ArgumentError(`workflow portfolio checkpoint references unknown item ${itemId}`);
      const context: AutonomousWorkflowPortfolioExecutionRehydrationContext = {
        job_id: jobId,
        item_id: itemId,
        domain: planItem.domain,
        request_digest: planItem.request_digest,
        task_digest: planItem.task_digest,
        expected_status: restored.settled_item_statuses[index]!,
        expected_result_digest: restored.settled_result_digests[index]!,
      };
      let item: AutonomousWorkflowPortfolioItemExecutionResult;
      try {
        item = await options.rehydrateItem(context);
      } catch {
        throw new ArgumentError(`rehydrated workflow portfolio item ${itemId} could not be loaded`);
      }
      await validateRehydratedItem(planItem, context.expected_status, context.expected_result_digest, item);
      rehydrated.push(item);
    }
  }

  const executionOptions: AutonomousWorkflowPortfolioExecutionOptions = {
    ...options,
    plan,
    maxParallelism: controls.maxParallelism,
    stopOnError: controls.stopOnError,
    includeDependencyOutputs: controls.includeDependencyOutputs,
    maxDependencyHandoffBytes: controls.maxDependencyHandoffBytes,
  };
  delete (executionOptions as Record<string, unknown>).checkpoint;
  delete (executionOptions as Record<string, unknown>).checkpointSink;
  delete (executionOptions as Record<string, unknown>).rehydrateItem;
  delete (executionOptions as Record<string, unknown>).jobId;
  const progressSink = options.checkpointSink
    ? async (progress: AutonomousWorkflowPortfolioExecutionProgress): Promise<void> => {
      const checkpoint = await makeCheckpoint({ jobId, plan, progress, ...controls });
      await options.checkpointSink!(checkpoint);
    }
    : undefined;
  return executeAutonomousWorkflowPortfolioWithInitialItems(agent, requests, executionOptions, rehydrated, progressSink);
}

/** In-memory checkpoint adapter useful for local tests and single-process applications. */
export class InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore implements AutonomousWorkflowPortfolioExecutionCheckpointStore {
  private checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON | null;

  constructor(initial?: AutonomousWorkflowPortfolioExecutionCheckpointJSON | null) {
    this.checkpoint = initial === undefined || initial === null ? null : structuredClone(initial);
  }

  async read(): Promise<AutonomousWorkflowPortfolioExecutionCheckpointJSON | null> {
    return this.checkpoint === null ? null : await validateAutonomousWorkflowPortfolioExecutionCheckpoint(this.checkpoint);
  }

  async write(checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON): Promise<void> {
    this.checkpoint = structuredClone(await validateAutonomousWorkflowPortfolioExecutionCheckpoint(checkpoint));
  }
}

/** Restart-aware controller that owns checkpoint restore/flush while the caller owns private rehydration. */
export class AutonomousWorkflowPortfolioExecutionController {
  private checkpoint: AutonomousWorkflowPortfolioExecutionCheckpointJSON | null = null;
  private controllerStatus: AutonomousWorkflowPortfolioExecutionControllerStatus = "empty";
  private totalItems: number | null = null;

  constructor(
    readonly agent: AutonomousAgent,
    readonly jobId: string,
    readonly persistence: AutonomousWorkflowPortfolioExecutionCheckpointStore,
  ) {
    boundedIdentifier("workflow portfolio controller jobId", jobId);
  }

  async restore(): Promise<AutonomousWorkflowPortfolioExecutionControllerProjection> {
    const stored = await this.persistence.read();
    this.checkpoint = stored === null ? null : await validateAutonomousWorkflowPortfolioExecutionCheckpoint(stored);
    this.controllerStatus = this.checkpoint === null ? "empty" : "restored";
    this.totalItems = this.checkpoint?.item_ids.length ?? null;
    return this.projection();
  }

  async run(
    requests: readonly AutonomousWorkflowPortfolioItemRequest[],
    options: AutonomousWorkflowPortfolioExecutionControllerRunOptions = {},
  ): Promise<AutonomousWorkflowPortfolioExecutionControllerRun> {
    const restored = this.checkpoint ?? await this.persistence.read();
    this.checkpoint = restored === null ? null : await validateAutonomousWorkflowPortfolioExecutionCheckpoint(restored);
    const result = await executeAutonomousWorkflowPortfolioResumable(this.agent, requests, {
      ...options,
      jobId: this.jobId,
      checkpoint: this.checkpoint ?? undefined,
      checkpointSink: async (checkpoint) => {
        this.checkpoint = checkpoint;
        this.controllerStatus = "flushed";
        this.totalItems = checkpoint.item_ids.length;
        await this.persistence.write(checkpoint);
      },
    });
    this.controllerStatus = result.status === "completed" ? "completed" : result.status === "failed" ? "failed" : "partial";
    this.totalItems = result.items.length;
    return { controller: this.projection(), execution: result };
  }

  projection(): AutonomousWorkflowPortfolioExecutionControllerProjection {
    return {
      schema: "bioprism-typescript-autonomous-workflow-portfolio-execution-controller/0.1",
      status: this.controllerStatus,
      job_id: this.jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      settled_items: this.checkpoint?.settled_item_ids.length ?? 0,
      total_items: this.totalItems,
      persisted: true,
      retention: "metadata_only_request_and_result_digests;task_prompt_provider_values_never_persisted",
      secret_material: "never_returned",
    };
  }
}
