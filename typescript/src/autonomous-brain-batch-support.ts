import { ArgumentError, isObject } from "./errors.js";
import { boundedIdentifier, digest } from "./autonomous-brain-facade-utils.js";
import { digestJsonSync } from "./tooling.js";

/** Stable schemas and bounds shared by the brain batch engine and checkpoint adapters. */
export const AUTONOMOUS_BRAIN_BATCH_SCHEMA = "bioprism-typescript-autonomous-brain-batch/0.1" as const;
export const AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-brain-batch-checkpoint/0.1" as const;
export const AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-batch-controller/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_BATCH = 64;
export const MAX_AUTONOMOUS_BRAIN_PARALLELISM = 8;
export const MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES = 128_000;

export type AutonomousBrainBatchMode = "brain" | "automatic" | "automatic_cycle" | "automatic_replan";

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

type BatchItemDigestInput = {
  index: number;
  status: string;
  task_digest: string | null;
  error_class?: string;
  failure_code?: string;
  execution?: unknown;
};

function batchItemProjection(item: BatchItemDigestInput): Record<string, unknown> {
  const execution = item.execution as { plan: { plan_digest: string }; status: string } | undefined;
  return {
    index: item.index,
    status: item.status,
    task_digest: item.task_digest,
    error_class: item.error_class ?? null,
    failure_code: item.failure_code ?? null,
    plan_digest: execution?.plan.plan_digest ?? null,
    execution_status: execution?.status ?? null,
  };
}

export function batchStatus(completed: number, failed: number, omitted: number): "completed" | "partial" | "failed" {
  return failed === 0 && omitted === 0 ? "completed" : completed > 0 ? "partial" : "failed";
}

export function batchDigest(items: readonly BatchItemDigestInput[]): string {
  return digestJsonSync(items.map(batchItemProjection));
}

export function automaticCycleBatchDigest(items: readonly {
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

export function batchItemDigest(item: BatchItemDigestInput): string {
  return digestJsonSync(batchItemProjection(item));
}

export function automaticCycleBatchItemDigest(item: BatchItemDigestInput): string {
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

function checkpointBatchItemDigest(item: BatchItemDigestInput): string {
  const execution = isObject(item.execution) ? item.execution : null;
  return execution !== null && isObject(execution.plan) ? batchItemDigest(item) : automaticCycleBatchItemDigest(item);
}

export function brainBatchTaskDigest(input: { task: string }): string {
  return digestJsonSync({ task: input.task });
}

export function brainBatchRequestDigest(input: {
  task: string;
  domain?: string;
  capability?: string;
  hints?: readonly string[];
  allow_cross_domain?: boolean;
  context?: readonly unknown[];
  connector?: unknown;
}, index: number, mode: AutonomousBrainBatchMode = "brain"): string {
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

export function checkpointText(name: string, value: unknown): string {
  return boundedIdentifier(name, value);
}

export function validateBrainBatchCheckpoint(value: unknown): AutonomousBrainBatchCheckpointJSON {
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

export function makeBrainBatchCheckpoint(input: {
  jobId: string;
  mode?: AutonomousBrainBatchMode;
  requestDigests: readonly string[];
  batchInputDigest: string;
  semanticRoutingPolicyDigest: string | null;
  automaticExecutionPolicyDigest?: string | null;
  completed: readonly { index: number; item: BatchItemDigestInput }[];
  maxParallelism: number;
  stopOnError: boolean;
  status: "running" | "partial" | "completed";
}): AutonomousBrainBatchCheckpointJSON {
  const mode = input.mode ?? "brain";
  if (mode === "automatic" && input.automaticExecutionPolicyDigest === undefined) throw new ArgumentError("automatic brain batch checkpoint requires an automatic execution policy digest");
  const payload = { schema: AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_SCHEMA, job_id: input.jobId, mode, batch_input_digest: input.batchInputDigest, ...(input.semanticRoutingPolicyDigest === null ? {} : { semantic_routing_policy_digest: input.semanticRoutingPolicyDigest }), ...(input.automaticExecutionPolicyDigest === undefined || input.automaticExecutionPolicyDigest === null ? {} : { automatic_execution_policy_digest: input.automaticExecutionPolicyDigest }), request_digests: [...input.requestDigests], completed_indices: input.completed.map((entry) => entry.index), completed_result_digests: input.completed.map((entry) => checkpointBatchItemDigest(entry.item)), max_parallelism: input.maxParallelism, stop_on_error: input.stopOnError, status: input.status };
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > MAX_AUTONOMOUS_BRAIN_BATCH_CHECKPOINT_BYTES) throw new ArgumentError("autonomous brain batch checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: digestJsonSync(payload), retention: "request_and_result_digests_only;tasks_prompts_credentials_and_payloads_never_persisted", secret_material: "never_returned" };
}
