import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type { AutonomousAgent } from "./autonomous.js";
import {
  AutonomousWorkflowPortfolioExecutionResult,
  type AutonomousWorkflowPortfolioExecutionStatus,
} from "./autonomous-workflow-portfolio-execution.js";
import {
  validateAutonomousWorkflowPortfolioAdmission,
  type AutonomousWorkflowPortfolioAdmission,
} from "./autonomous-workflow-portfolio-admission.js";
import {
  validateAutonomousWorkflowPortfolioPlan,
  type AutonomousWorkflowPortfolioItemRequest,
  type AutonomousWorkflowPortfolioPlan,
} from "./autonomous-workflow-portfolio.js";
import type { AutonomousWorkflowPortfolioResumableExecutionOptions } from "./autonomous-workflow-portfolio-resumable.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only remote handoff queue for one reviewed autonomous portfolio. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-job-queue/0.3" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-job/0.3" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_REMOTE_WORKER_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-remote-worker/0.3" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOBS = 4_096;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ITEMS = 64;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_LEASE_MS = 300_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SNAPSHOT_BYTES = 512_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_REMOTE_WORKER_HEARTBEAT_MS = 300_000;

export type AutonomousWorkflowPortfolioRemoteJobStatus =
  | "queued"
  | "leased"
  | "completed"
  | "partial"
  | "blocked"
  | "approval_required"
  | "failed"
  | "reconciliation_required"
  | "cancelled";

export type AutonomousWorkflowPortfolioRemoteJobFailureClass =
  | "resolver_missing"
  | "plan_mismatch"
  | "admission_mismatch"
  | "request_mismatch"
  | "checkpoint_mismatch"
  | "lease_expired"
  | "provider_execution_failed"
  | "approval_required"
  | "transport_error"
  | "rehydration_missing"
  | "executor_error"
  | "reconciliation_required"
  | "unknown";

export type AutonomousWorkflowPortfolioRemoteJobExecutionPhase = "not_started" | "running" | "settled";
export type AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome = "succeeded" | "failed" | "not_executed" | "unknown";

export interface AutonomousWorkflowPortfolioRemoteJob extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA;
  job_id: string;
  plan_digest: string;
  admission_digest: string | null;
  require_admission: boolean;
  trace_id: string | null;
  item_ids: string[];
  request_digests: string[];
  checkpoint_digest: string | null;
  result_digest: string | null;
  trace_digest: string | null;
  execution_phase: AutonomousWorkflowPortfolioRemoteJobExecutionPhase;
  reconciliation_digest: string | null;
  reconciliation_observed_job_digest: string | null;
  reconciliation_outcome: AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome | null;
  reconciliation_evidence_digest: string | null;
  reconciliation_evidence_kind: string | null;
  reconciliation_operator: string | null;
  reconciliation_effect_absent: boolean | null;
  reconciliation_history: string[];
  status: AutonomousWorkflowPortfolioRemoteJobStatus;
  max_attempts: number;
  attempts: number;
  available_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  failure_class: AutonomousWorkflowPortfolioRemoteJobFailureClass | null;
  failure_code: string | null;
  created_at: number;
  updated_at: number;
  job_digest: string;
  retention: "metadata_only_plan_admission_request_and_result_digests;tasks_prompts_credentials_outputs_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioRemoteJobQueueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA;
  jobs: AutonomousWorkflowPortfolioRemoteJob[];
  snapshot_digest: string;
  retention: "metadata_only_plan_admission_request_and_result_digests;tasks_prompts_credentials_outputs_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioRemoteJobQueuePersistence {
  read(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot | null> | AutonomousWorkflowPortfolioRemoteJobQueueSnapshot | null;
  write(snapshot: AutonomousWorkflowPortfolioRemoteJobQueueSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioRemoteJobQueueSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioRemoteJobQueueTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioRemoteJobQueueTransactionalTextStore extends AutonomousWorkflowPortfolioRemoteJobQueueTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioRemoteJobRequeueOptions {
  reconciliationDigest?: string;
}

export interface AutonomousWorkflowPortfolioRemoteJobReconciliationOptions extends JsonObject {
  outcome: AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome;
  evidenceDigest: string;
  evidenceKind?: string;
  operator?: string;
  effectAbsent?: boolean;
}

export interface AutonomousWorkflowPortfolioRemoteWorkerRow extends JsonObject {
  job_id: string;
  outcome: "completed" | "partial" | "blocked" | "approval_required" | "retry_scheduled" | "failed" | "reconciliation_required" | "leased_elsewhere";
  attempts: number;
  result_digest: string | null;
  trace_digest: string | null;
  reconciliation_digest: string | null;
  failure_class: AutonomousWorkflowPortfolioRemoteJobFailureClass | null;
  lease_retained: false;
}

export interface AutonomousWorkflowPortfolioRemoteWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_REMOTE_WORKER_SCHEMA;
  worker_id: string;
  inspected: number;
  completed: number;
  partial: number;
  blocked: number;
  approval_required: number;
  retried: number;
  failed: number;
  reconciled: number;
  leased_elsewhere: number;
  rows: AutonomousWorkflowPortfolioRemoteWorkerRow[];
  retention: "metadata_only_job_receipts_and_digests_no_private_values";
  secret_material: "never_returned";
}

const JOB_RETENTION = "metadata_only_plan_admission_request_and_result_digests;tasks_prompts_credentials_outputs_never_persisted" as const;
const JOB_SECRET_MATERIAL = "never_returned" as const;
const JOB_STATUSES: readonly AutonomousWorkflowPortfolioRemoteJobStatus[] = ["queued", "leased", "completed", "partial", "blocked", "approval_required", "failed", "reconciliation_required", "cancelled"];
const FAILURE_CLASSES: readonly AutonomousWorkflowPortfolioRemoteJobFailureClass[] = ["resolver_missing", "plan_mismatch", "admission_mismatch", "request_mismatch", "checkpoint_mismatch", "lease_expired", "provider_execution_failed", "approval_required", "transport_error", "rehydration_missing", "executor_error", "reconciliation_required", "unknown"];
const EXECUTION_PHASES: readonly AutonomousWorkflowPortfolioRemoteJobExecutionPhase[] = ["not_started", "running", "settled"];
const RECONCILIATION_OUTCOMES: readonly AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome[] = ["succeeded", "failed", "not_executed", "unknown"];

function clone<T>(value: T): T {
  return structuredClone(value);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function optionalDigest(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return digest(name, value);
}

function digestHistory(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS) throw new ArgumentError(`${name} must contain at most ${MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS} entries`);
  const result = value.map((entry, index) => digest(`${name}[${index}]`, entry));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return result;
}

function timestamp(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER) throw new ArgumentError(`${name} must be a finite non-negative timestamp`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number, fallback?: number): number {
  if (value === undefined && fallback !== undefined) return fallback;
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function reconciliationOutcome(value: unknown): AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome {
  if (!RECONCILIATION_OUTCOMES.includes(value as AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome)) throw new ArgumentError("portfolio remote reconciliation outcome is invalid");
  return value as AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome;
}

function reconciliationReceiptDigest(job: AutonomousWorkflowPortfolioRemoteJob, options: {
  outcome: AutonomousWorkflowPortfolioRemoteJobReconciliationOutcome;
  evidenceDigest: string;
  evidenceKind: string;
  operator: string;
  effectAbsent: boolean | null;
}): string {
  return digestJsonSync({
    schema: `${AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA}/reconciliation-receipt`,
    job_id: job.job_id,
    plan_digest: job.plan_digest,
    observed_job_digest: job.job_digest,
    outcome: options.outcome,
    evidence_digest: options.evidenceDigest,
    evidence_kind: options.evidenceKind,
    operator: options.operator,
    effect_absent: options.effectAbsent,
  });
}

function isPortfolioRemoteJobQueueHandle(value: unknown): value is AutonomousWorkflowPortfolioRemoteJobQueueHandle {
  if (!isObject(value)) return false;
  return ["get", "pending", "claim", "renew", "checkpoint", "beginExecution", "complete", "fail", "reconcile", "settleReconciliation", "reclaimExpired", "requeue", "cancel", "snapshot"].every((method) => typeof (value as Record<string, unknown>)[method] === "function") && typeof (value as Record<string, unknown>).maxJobs === "number";
}

async function computePrivateRequestDigests(requests: readonly AutonomousWorkflowPortfolioItemRequest[]): Promise<Array<{ itemId: string; digest: string }>> {
  if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ITEMS) throw new ArgumentError("portfolio remote private requests are outside their bounds");
  return Promise.all(requests.map((request, index) => {
    if (!isObject(request)) throw new ArgumentError(`portfolio remote private request ${index} is malformed`);
    const id = typeof request.id === "string" && request.id.length > 0 ? request.id : `item-${index + 1}`;
    return digestJson({
      schema: "bioprism-typescript-autonomous-workflow-portfolio-request/0.1",
      item_id: id,
      task: request.task,
      domain: request.domain,
      capability: request.capability ?? null,
      depends_on: Array.isArray(request.dependsOn) ? [...request.dependsOn] : [],
      hints: Array.isArray(request.hints) ? [...request.hints] : [],
      context: Array.isArray(request.context) ? request.context : [],
    }).then((requestDigest) => ({ itemId: id, digest: requestDigest }));
  }));
}

function jobDescriptor(job: AutonomousWorkflowPortfolioRemoteJob): JsonObject {
  const { job_digest: _jobDigest, ...descriptor } = job;
  return descriptor;
}

function jobDigest(job: AutonomousWorkflowPortfolioRemoteJob): string {
  return digestJsonSync(jobDescriptor(job));
}

function refresh(job: AutonomousWorkflowPortfolioRemoteJob, patch: Partial<AutonomousWorkflowPortfolioRemoteJob>, now: number): AutonomousWorkflowPortfolioRemoteJob {
  const next = { ...job, ...patch, updated_at: timestamp("portfolio remote job updated_at", now) } as AutonomousWorkflowPortfolioRemoteJob;
  next.job_digest = jobDigest(next);
  return next;
}

function validateJob(raw: unknown): AutonomousWorkflowPortfolioRemoteJob {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA) throw new ArgumentError("portfolio remote job schema is invalid");
  const allowed = new Set(["schema", "job_id", "plan_digest", "admission_digest", "require_admission", "trace_id", "item_ids", "request_digests", "checkpoint_digest", "result_digest", "trace_digest", "execution_phase", "reconciliation_digest", "reconciliation_observed_job_digest", "reconciliation_outcome", "reconciliation_evidence_digest", "reconciliation_evidence_kind", "reconciliation_operator", "reconciliation_effect_absent", "reconciliation_history", "status", "max_attempts", "attempts", "available_at", "lease_owner", "lease_until", "failure_class", "failure_code", "created_at", "updated_at", "job_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("portfolio remote job contains unsupported fields");
  const value = raw as unknown as AutonomousWorkflowPortfolioRemoteJob;
  const itemIds = Array.isArray(value.item_ids) ? value.item_ids.map((item, index) => identifier(`portfolio remote job item_ids[${index}]`, item)) : [];
  const requestDigests = Array.isArray(value.request_digests) ? value.request_digests.map((item, index) => digest(`portfolio remote job request_digests[${index}]`, item)) : [];
  const normalized = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA,
    job_id: identifier("portfolio remote job job_id", value.job_id),
    plan_digest: digest("portfolio remote job plan_digest", value.plan_digest),
    admission_digest: optionalDigest("portfolio remote job admission_digest", value.admission_digest),
    require_admission: value.require_admission,
    trace_id: value.trace_id === null ? null : identifier("portfolio remote job trace_id", value.trace_id),
    item_ids: itemIds,
    request_digests: requestDigests,
    checkpoint_digest: optionalDigest("portfolio remote job checkpoint_digest", value.checkpoint_digest),
    result_digest: optionalDigest("portfolio remote job result_digest", value.result_digest),
    trace_digest: optionalDigest("portfolio remote job trace_digest", value.trace_digest),
    execution_phase: value.execution_phase,
    reconciliation_digest: optionalDigest("portfolio remote job reconciliation_digest", value.reconciliation_digest),
    reconciliation_observed_job_digest: optionalDigest("portfolio remote job reconciliation_observed_job_digest", value.reconciliation_observed_job_digest),
    reconciliation_outcome: value.reconciliation_outcome === null ? null : reconciliationOutcome(value.reconciliation_outcome),
    reconciliation_evidence_digest: optionalDigest("portfolio remote job reconciliation_evidence_digest", value.reconciliation_evidence_digest),
    reconciliation_evidence_kind: value.reconciliation_evidence_kind === null ? null : identifier("portfolio remote job reconciliation_evidence_kind", value.reconciliation_evidence_kind),
    reconciliation_operator: value.reconciliation_operator === null ? null : identifier("portfolio remote job reconciliation_operator", value.reconciliation_operator),
    reconciliation_effect_absent: value.reconciliation_effect_absent === null ? null : value.reconciliation_effect_absent,
    reconciliation_history: digestHistory("portfolio remote job reconciliation_history", value.reconciliation_history),
    status: value.status,
    max_attempts: boundedInteger("portfolio remote job max_attempts", value.max_attempts, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS),
    attempts: boundedInteger("portfolio remote job attempts", value.attempts, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS),
    available_at: timestamp("portfolio remote job available_at", value.available_at),
    lease_owner: value.lease_owner === null ? null : identifier("portfolio remote job lease_owner", value.lease_owner),
    lease_until: value.lease_until === null ? null : timestamp("portfolio remote job lease_until", value.lease_until),
    failure_class: value.failure_class === null ? null : value.failure_class,
    failure_code: value.failure_code === null ? null : identifier("portfolio remote job failure_code", value.failure_code),
    created_at: timestamp("portfolio remote job created_at", value.created_at),
    updated_at: timestamp("portfolio remote job updated_at", value.updated_at),
    job_digest: digest("portfolio remote job job_digest", value.job_digest),
    retention: value.retention,
    secret_material: value.secret_material,
  } satisfies AutonomousWorkflowPortfolioRemoteJob;
  if (typeof normalized.require_admission !== "boolean") throw new ArgumentError("portfolio remote job require_admission must be boolean");
  if (normalized.item_ids.length < 1 || normalized.item_ids.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ITEMS || normalized.item_ids.length !== normalized.request_digests.length || new Set(normalized.item_ids).size !== normalized.item_ids.length) throw new ArgumentError("portfolio remote job item identity is invalid");
  if (!EXECUTION_PHASES.includes(normalized.execution_phase)) throw new ArgumentError("portfolio remote job execution_phase is invalid");
  if (!JOB_STATUSES.includes(normalized.status)) throw new ArgumentError("portfolio remote job status is invalid");
  if (normalized.status === "queued" && normalized.execution_phase !== "not_started") throw new ArgumentError("queued portfolio remote job must not have started execution");
  if (normalized.status === "reconciliation_required" && normalized.execution_phase !== "running") throw new ArgumentError("reconciliation-required portfolio remote job must retain its running execution phase");
  if (["completed", "partial", "blocked", "approval_required", "cancelled"].includes(normalized.status) && normalized.execution_phase !== "settled") throw new ArgumentError("settled portfolio remote job status requires a settled execution phase");
  if (normalized.status === "leased" && normalized.execution_phase === "settled") throw new ArgumentError("leased portfolio remote job cannot have a settled execution phase");
  if (normalized.require_admission && normalized.admission_digest === null) throw new ArgumentError("required portfolio remote job admission is missing");
  if (normalized.failure_class !== null && !FAILURE_CLASSES.includes(normalized.failure_class)) throw new ArgumentError("portfolio remote job failure_class is invalid");
  if (normalized.status === "approval_required" && normalized.failure_class !== "approval_required") throw new ArgumentError("approval-required portfolio remote job must retain its approval failure class");
  if (normalized.status !== "approval_required" && normalized.failure_class === "approval_required") throw new ArgumentError("approval failure class is only valid for approval-required jobs");
  const reconciliationFields = [normalized.reconciliation_observed_job_digest, normalized.reconciliation_outcome, normalized.reconciliation_evidence_digest, normalized.reconciliation_evidence_kind, normalized.reconciliation_operator, normalized.reconciliation_effect_absent];
  if (normalized.reconciliation_digest === null && reconciliationFields.some((field) => field !== null)) throw new ArgumentError("portfolio remote reconciliation metadata requires a reconciliation digest");
  if (normalized.reconciliation_digest !== null && reconciliationFields.some((field) => field === null)) throw new ArgumentError("portfolio remote reconciliation digest requires complete receipt metadata");
  if (normalized.reconciliation_digest !== null && normalized.reconciliation_outcome === "not_executed" && normalized.reconciliation_effect_absent !== true) throw new ArgumentError("portfolio remote not_executed receipt must assert effect absence");
  if (normalized.reconciliation_digest !== null && (normalized.reconciliation_outcome === "succeeded" || normalized.reconciliation_outcome === "unknown") && normalized.reconciliation_effect_absent === true) throw new ArgumentError("portfolio remote reconciliation outcome contradicts effect absence");
  if (normalized.reconciliation_digest !== null) {
    const expectedReceipt = digestJsonSync({ schema: `${AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA}/reconciliation-receipt`, job_id: normalized.job_id, plan_digest: normalized.plan_digest, observed_job_digest: normalized.reconciliation_observed_job_digest, outcome: normalized.reconciliation_outcome, evidence_digest: normalized.reconciliation_evidence_digest, evidence_kind: normalized.reconciliation_evidence_kind, operator: normalized.reconciliation_operator, effect_absent: normalized.reconciliation_effect_absent });
    if (expectedReceipt !== normalized.reconciliation_digest) throw new ArgumentError("portfolio remote reconciliation digest does not match its receipt metadata");
  }
  if (normalized.reconciliation_effect_absent !== null && typeof normalized.reconciliation_effect_absent !== "boolean") throw new ArgumentError("portfolio remote reconciliation_effect_absent must be boolean or null");
  if (normalized.status === "leased" && (normalized.lease_owner === null || normalized.lease_until === null)) throw new ArgumentError("leased portfolio remote job requires a lease");
  if (normalized.status !== "leased" && (normalized.lease_owner !== null || normalized.lease_until !== null)) throw new ArgumentError("non-leased portfolio remote job cannot retain a lease");
  if (normalized.retention !== JOB_RETENTION || normalized.secret_material !== JOB_SECRET_MATERIAL) throw new ArgumentError("portfolio remote job retention contract is invalid");
  if (jobDigest(normalized) !== normalized.job_digest) throw new ArgumentError("portfolio remote job digest is invalid");
  return clone(normalized);
}

export function validateAutonomousWorkflowPortfolioRemoteJobQueueSnapshot(raw: unknown, maxJobs = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOBS): AutonomousWorkflowPortfolioRemoteJobQueueSnapshot {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA || !Array.isArray(raw.jobs)) throw new ArgumentError("portfolio remote job queue snapshot is malformed");
  if (raw.retention !== JOB_RETENTION || raw.secret_material !== JOB_SECRET_MATERIAL) throw new ArgumentError("portfolio remote job queue retention contract is invalid");
  if (raw.jobs.length > maxJobs || new Set(raw.jobs.map((job) => isObject(job) && job.job_id)).size !== raw.jobs.length) throw new ArgumentError("portfolio remote job queue jobs are invalid");
  const jobs = raw.jobs.map(validateJob).sort((left, right) => left.created_at - right.created_at || left.job_id.localeCompare(right.job_id));
  const body = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA, jobs, retention: JOB_RETENTION, secret_material: JOB_SECRET_MATERIAL };
  if (typeof raw.snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(raw.snapshot_digest) || digestJsonSync(body) !== raw.snapshot_digest) throw new ArgumentError("portfolio remote job queue snapshot digest is invalid");
  const snapshot = { ...body, snapshot_digest: raw.snapshot_digest } as AutonomousWorkflowPortfolioRemoteJobQueueSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SNAPSHOT_BYTES) throw new ArgumentError("portfolio remote job queue snapshot exceeds its byte bound");
  return clone(snapshot);
}

/** A lease-fenced, metadata-only queue for remote portfolio provider execution. */
export class InMemoryAutonomousWorkflowPortfolioRemoteJobQueue {
  private readonly jobs = new Map<string, AutonomousWorkflowPortfolioRemoteJob>();

  constructor(readonly maxJobs = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOBS) {
    boundedInteger("portfolio remote job queue maxJobs", maxJobs, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOBS);
  }

  enqueue(input: {
    jobId: string;
    planDigest: string;
    admissionDigest?: string | null;
    requireAdmission?: boolean;
    traceId?: string | null;
    itemIds: readonly string[];
    requestDigests: readonly string[];
    maxAttempts?: number;
    now?: number;
  }): AutonomousWorkflowPortfolioRemoteJob {
    const jobId = identifier("portfolio remote job jobId", input.jobId);
    const planDigest = digest("portfolio remote job planDigest", input.planDigest);
    const admissionDigest = optionalDigest("portfolio remote job admissionDigest", input.admissionDigest);
    const requireAdmission = input.requireAdmission ?? true;
    if (typeof requireAdmission !== "boolean") throw new ArgumentError("portfolio remote job requireAdmission must be boolean");
    if (requireAdmission && admissionDigest === null) throw new ArgumentError("portfolio remote job admission is required");
    const traceId = input.traceId === undefined || input.traceId === null ? null : identifier("portfolio remote job traceId", input.traceId);
    const itemIds = input.itemIds.map((item, index) => identifier(`portfolio remote job itemIds[${index}]`, item));
    const requestDigests = input.requestDigests.map((item, index) => digest(`portfolio remote job requestDigests[${index}]`, item));
    if (itemIds.length < 1 || itemIds.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ITEMS || itemIds.length !== requestDigests.length || new Set(itemIds).size !== itemIds.length) throw new ArgumentError("portfolio remote job items are outside their bounds");
    const maxAttempts = boundedInteger("portfolio remote job maxAttempts", input.maxAttempts, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_ATTEMPTS, 3);
    const now = timestamp("portfolio remote job now", input.now ?? Date.now());
    const existing = this.jobs.get(jobId);
    if (existing) {
      if (existing.plan_digest !== planDigest || existing.admission_digest !== admissionDigest || existing.require_admission !== requireAdmission || existing.trace_id !== traceId || canonicalJson(existing.item_ids) !== canonicalJson(itemIds) || canonicalJson(existing.request_digests) !== canonicalJson(requestDigests) || existing.max_attempts !== maxAttempts) throw new ArgumentError("portfolio remote job idempotency identity conflicts");
      return clone(existing);
    }
    if (this.jobs.size >= this.maxJobs) throw new ArgumentError("portfolio remote job queue is full");
    const job = {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SCHEMA,
      job_id: jobId,
      plan_digest: planDigest,
      admission_digest: admissionDigest,
      require_admission: requireAdmission,
      trace_id: traceId,
      item_ids: [...itemIds],
      request_digests: [...requestDigests],
      checkpoint_digest: null,
      result_digest: null,
      trace_digest: null,
      execution_phase: "not_started",
      reconciliation_digest: null,
      reconciliation_observed_job_digest: null,
      reconciliation_outcome: null,
      reconciliation_evidence_digest: null,
      reconciliation_evidence_kind: null,
      reconciliation_operator: null,
      reconciliation_effect_absent: null,
      reconciliation_history: [],
      status: "queued" as const,
      max_attempts: maxAttempts,
      attempts: 0,
      available_at: now,
      lease_owner: null,
      lease_until: null,
      failure_class: null,
      failure_code: null,
      created_at: now,
      updated_at: now,
      job_digest: "0".repeat(64),
      retention: JOB_RETENTION,
      secret_material: JOB_SECRET_MATERIAL,
    } satisfies AutonomousWorkflowPortfolioRemoteJob;
    job.job_digest = jobDigest(job);
    const validated = validateJob(job);
    this.jobs.set(jobId, validated);
    return clone(validated);
  }

  get(jobId: string): AutonomousWorkflowPortfolioRemoteJob | null {
    const job = this.jobs.get(identifier("portfolio remote job jobId", jobId));
    return job ? clone(job) : null;
  }

  pending(limit = 1, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob[] {
    const boundedLimit = boundedInteger("portfolio remote job pending limit", limit, 1, this.maxJobs);
    const time = timestamp("portfolio remote job pending now", now);
    return [...this.jobs.values()].filter((job) => job.status === "queued" && job.available_at <= time && job.attempts < job.max_attempts).sort((left, right) => left.available_at - right.available_at || left.created_at - right.created_at || left.job_id.localeCompare(right.job_id)).slice(0, boundedLimit).map(clone);
  }

  claim(jobId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob | null {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const lease = boundedInteger("portfolio remote job leaseMs", leaseMs, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_LEASE_MS);
    const time = timestamp("portfolio remote job claim now", now);
    let current = this.jobs.get(id);
    if (!current || ["completed", "partial", "blocked", "approval_required", "failed", "cancelled", "reconciliation_required"].includes(current.status)) return null;
    if (current.status === "leased") {
      if (current.lease_until !== null && current.lease_until > time) return null;
      if (current.execution_phase === "running") {
        this.jobs.set(id, refresh(current, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: "reconciliation_required", failure_code: "execution_in_flight" }, time));
        return null;
      }
      current = refresh(current, { status: "queued", execution_phase: "not_started", lease_owner: null, lease_until: null, failure_class: null, failure_code: null }, time);
      this.jobs.set(id, current);
    }
    if (current.available_at > time || current.attempts >= current.max_attempts) return null;
    const next = refresh(current, { status: "leased", execution_phase: "not_started", attempts: current.attempts + 1, lease_owner: worker, lease_until: time + lease, failure_class: null, failure_code: null }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  renew(jobId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const lease = boundedInteger("portfolio remote job leaseMs", leaseMs, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_LEASE_MS);
    const time = timestamp("portfolio remote job renew now", now);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job lease cannot be renewed by this worker");
    const next = refresh(current, { lease_until: time + lease }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  beginExecution(jobId: string, workerId: string, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const time = timestamp("portfolio remote job execution start now", now);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job execution start is fenced by its lease");
    const next = refresh(current, { execution_phase: "running" }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  checkpoint(jobId: string, workerId: string, checkpointDigest: string, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const checkpoint = digest("portfolio remote job checkpointDigest", checkpointDigest);
    const time = timestamp("portfolio remote job checkpoint now", now);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job checkpoint is fenced by its lease");
    const next = refresh(current, { checkpoint_digest: checkpoint }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  complete(jobId: string, workerId: string, input: { status: "completed" | "partial" | "blocked" | "approval_required"; resultDigest: string; traceDigest?: string | null }, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const time = timestamp("portfolio remote job completion now", now);
    const resultDigest = digest("portfolio remote job resultDigest", input.resultDigest);
    const traceDigest = optionalDigest("portfolio remote job traceDigest", input.traceDigest);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job completion is fenced by its lease");
    if (current.execution_phase !== "running") throw new ArgumentError("portfolio remote job completion requires the execution phase to be running");
    const next = refresh(current, {
      status: input.status,
      execution_phase: "settled",
      result_digest: resultDigest,
      trace_digest: traceDigest,
      lease_owner: null,
      lease_until: null,
      failure_class: input.status === "approval_required" ? "approval_required" : null,
      failure_code: input.status === "approval_required" ? "approval_required" : null,
    }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  fail(jobId: string, workerId: string, failureClass: AutonomousWorkflowPortfolioRemoteJobFailureClass, retryable: boolean, failureCode: string = failureClass, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const normalizedFailure = FAILURE_CLASSES.includes(failureClass) ? failureClass : (() => { throw new ArgumentError("portfolio remote job failureClass is invalid"); })();
    const code = identifier("portfolio remote job failureCode", failureCode);
    const time = timestamp("portfolio remote job failure now", now);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job failure is fenced by its lease");
    const uncertainExecution = current.execution_phase === "running";
    const canRetry = !uncertainExecution && retryable && current.attempts < current.max_attempts;
    const next = refresh(current, { status: uncertainExecution ? "reconciliation_required" : canRetry ? "queued" : "failed", execution_phase: uncertainExecution ? "running" : "not_started", available_at: canRetry ? time + Math.min(3_600_000, 1_000 * (2 ** Math.max(0, current.attempts - 1))) : current.available_at, lease_owner: null, lease_until: null, failure_class: uncertainExecution ? "reconciliation_required" : canRetry ? null : normalizedFailure, failure_code: uncertainExecution ? "execution_in_flight" : code }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  reconcile(jobId: string, workerId: string, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const worker = identifier("portfolio remote job workerId", workerId);
    const time = timestamp("portfolio remote job reconciliation now", now);
    const current = this.jobs.get(id);
    if (!current || current.status !== "leased" || current.lease_owner !== worker || current.lease_until === null || current.lease_until <= time) throw new ArgumentError("portfolio remote job reconciliation is fenced by its lease");
    const next = refresh(current, { status: "reconciliation_required", execution_phase: "running", lease_owner: null, lease_until: null, failure_class: "reconciliation_required", failure_code: "lease_reconciliation_required" }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  /** Record caller-owned evidence for a quarantined portfolio execution without retaining raw values. */
  settleReconciliation(jobId: string, options: AutonomousWorkflowPortfolioRemoteJobReconciliationOptions, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const time = timestamp("portfolio remote reconciliation settle now", now);
    const current = this.jobs.get(id);
    if (!current) throw new ArgumentError("portfolio remote job was not found");
    const outcome = reconciliationOutcome(options.outcome);
    const evidenceDigest = digest("portfolio remote reconciliation evidenceDigest", options.evidenceDigest);
    const evidenceKind = identifier("portfolio remote reconciliation evidenceKind", options.evidenceKind ?? "caller_observation");
    const operator = identifier("portfolio remote reconciliation operator", options.operator ?? "caller");
    const effectAbsent = options.effectAbsent === undefined ? (outcome === "not_executed" ? true : null) : options.effectAbsent;
    if (typeof effectAbsent !== "boolean" && effectAbsent !== null) throw new ArgumentError("portfolio remote reconciliation effectAbsent must be boolean or omitted");
    if (outcome === "not_executed" && effectAbsent !== true) throw new ArgumentError("not_executed portfolio reconciliation requires effectAbsent=true");
    if ((outcome === "succeeded" || outcome === "unknown") && effectAbsent === true) throw new ArgumentError("portfolio reconciliation effectAbsent contradicts the selected outcome");
    if (current.reconciliation_digest !== null && current.reconciliation_outcome === outcome && current.reconciliation_evidence_digest === evidenceDigest && current.reconciliation_evidence_kind === evidenceKind && current.reconciliation_operator === operator && current.reconciliation_effect_absent === effectAbsent) return clone(current);
    if (current.status !== "reconciliation_required") throw new ArgumentError("portfolio remote job is not awaiting reconciliation");
    const receipt = reconciliationReceiptDigest(current, { outcome, evidenceDigest, evidenceKind, operator, effectAbsent });
    const next = refresh(current, {
      status: outcome === "succeeded" ? "completed" : outcome === "failed" ? "failed" : "reconciliation_required",
      execution_phase: outcome === "succeeded" || outcome === "failed" ? "settled" : "running",
      result_digest: outcome === "succeeded" ? receipt : current.result_digest,
      reconciliation_digest: receipt,
      reconciliation_observed_job_digest: current.job_digest,
      reconciliation_outcome: outcome,
      reconciliation_evidence_digest: evidenceDigest,
      reconciliation_evidence_kind: evidenceKind,
      reconciliation_operator: operator,
      reconciliation_effect_absent: effectAbsent,
      failure_class: outcome === "succeeded" ? null : "reconciliation_required",
      failure_code: outcome === "succeeded" ? null : outcome === "failed" ? "reconciled_failure" : "execution_in_flight",
      lease_owner: null,
      lease_until: null,
    }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  reclaimExpired(now = Date.now(), limit = this.maxJobs): AutonomousWorkflowPortfolioRemoteJob[] {
    const time = timestamp("portfolio remote job reclaim now", now);
    const boundedLimit = boundedInteger("portfolio remote job reclaim limit", limit, 1, this.maxJobs);
    const expired = [...this.jobs.values()].filter((job) => job.status === "leased" && job.lease_until !== null && job.lease_until <= time).sort((left, right) => left.lease_until! - right.lease_until!).slice(0, boundedLimit);
    return expired.map((job) => {
      const next = refresh(job, job.execution_phase === "running"
        ? { status: "reconciliation_required", execution_phase: "running", lease_owner: null, lease_until: null, failure_class: "reconciliation_required", failure_code: "execution_in_flight" }
        : { status: "queued", execution_phase: "not_started", lease_owner: null, lease_until: null, failure_class: null, failure_code: null }, time);
      this.jobs.set(job.job_id, next);
      return clone(next);
    });
  }

  requeue(jobId: string, now = Date.now(), options: AutonomousWorkflowPortfolioRemoteJobRequeueOptions = {}): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const time = timestamp("portfolio remote job requeue now", now);
    const current = this.jobs.get(id);
    if (!current || !["reconciliation_required", "approval_required"].includes(current.status) || current.attempts >= current.max_attempts) throw new ArgumentError("portfolio remote job is not eligible for requeue");
    if (current.status === "reconciliation_required") {
      if (current.reconciliation_digest === null || current.reconciliation_outcome !== "not_executed" || current.reconciliation_effect_absent !== true) throw new ArgumentError("portfolio remote requeue requires a matching no-effect reconciliation receipt");
      if (options.reconciliationDigest !== current.reconciliation_digest) throw new ArgumentError("portfolio remote requeue requires the matching reconciliation digest");
    }
    const history = current.status === "reconciliation_required" ? [...current.reconciliation_history, current.reconciliation_digest!] : current.reconciliation_history;
    const next = refresh(current, {
      status: "queued",
      execution_phase: "not_started",
      available_at: time,
      failure_class: null,
      failure_code: null,
      reconciliation_digest: null,
      reconciliation_observed_job_digest: null,
      reconciliation_outcome: null,
      reconciliation_evidence_digest: null,
      reconciliation_evidence_kind: null,
      reconciliation_operator: null,
      reconciliation_effect_absent: null,
      reconciliation_history: history,
    }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  cancel(jobId: string, now = Date.now()): AutonomousWorkflowPortfolioRemoteJob {
    const id = identifier("portfolio remote job jobId", jobId);
    const time = timestamp("portfolio remote job cancel now", now);
    const current = this.jobs.get(id);
    if (!current || ["completed", "partial", "blocked", "failed", "cancelled", "reconciliation_required"].includes(current.status) || current.execution_phase === "running") throw new ArgumentError("portfolio remote job cannot be cancelled across an active or uncertain execution boundary");
    const next = refresh(current, { status: "cancelled", execution_phase: "settled", lease_owner: null, lease_until: null, failure_class: "unknown", failure_code: "cancelled" }, time);
    this.jobs.set(id, next);
    return clone(next);
  }

  jobsSnapshot(): AutonomousWorkflowPortfolioRemoteJob[] {
    return [...this.jobs.values()].sort((left, right) => left.created_at - right.created_at || left.job_id.localeCompare(right.job_id)).map(clone);
  }

  snapshot(): AutonomousWorkflowPortfolioRemoteJobQueueSnapshot {
    const body = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA, jobs: this.jobsSnapshot(), retention: JOB_RETENTION, secret_material: JOB_SECRET_MATERIAL };
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousWorkflowPortfolioRemoteJobQueueSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SNAPSHOT_BYTES) throw new ArgumentError("portfolio remote job queue snapshot exceeds its byte bound");
    return clone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateAutonomousWorkflowPortfolioRemoteJobQueueSnapshot(raw, this.maxJobs);
    this.jobs.clear();
    for (const job of snapshot.jobs) this.jobs.set(job.job_id, job);
  }

  verifyIntegrity(): { verified: true; jobs: number; snapshot_digest: string } {
    const snapshot = this.snapshot();
    return { verified: true, jobs: snapshot.jobs.length, snapshot_digest: snapshot.snapshot_digest };
  }
}

/** Validate a plan/admission handoff before it is placed on the remote queue. */
export async function admitAutonomousWorkflowPortfolioRemoteJob(
  queue: InMemoryAutonomousWorkflowPortfolioRemoteJobQueue,
  input: { jobId: string; plan: AutonomousWorkflowPortfolioPlan; admission?: AutonomousWorkflowPortfolioAdmission | null; requireAdmission?: boolean; traceId?: string | null; maxAttempts?: number; now?: number },
): Promise<AutonomousWorkflowPortfolioRemoteJob> {
  if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioRemoteJobQueue)) throw new ArgumentError("portfolio remote job admission requires a typed queue");
  const plan = await validateAutonomousWorkflowPortfolioPlan(input.plan);
  const admission = input.admission === undefined || input.admission === null ? null : await validateAutonomousWorkflowPortfolioAdmission(input.admission);
  const requireAdmission = input.requireAdmission ?? true;
  if (typeof requireAdmission !== "boolean") throw new ArgumentError("portfolio remote job requireAdmission must be boolean");
  if (requireAdmission && admission === null) throw new ArgumentError("portfolio remote job admission is required before enqueue");
  if (admission !== null && admission.plan.portfolio_digest !== plan.portfolio_digest) throw new ArgumentError("portfolio remote job admission does not match the plan");
  return queue.enqueue({
    jobId: input.jobId,
    planDigest: plan.portfolio_digest,
    admissionDigest: admission?.admission_digest ?? null,
    requireAdmission,
    traceId: input.traceId,
    itemIds: plan.items.map((item) => item.item_id),
    requestDigests: plan.items.map((item) => item.request_digest),
    maxAttempts: input.maxAttempts,
    now: input.now,
  });
}

export interface AutonomousWorkflowPortfolioRemoteJobResolution {
  requests: readonly AutonomousWorkflowPortfolioItemRequest[];
  plan: AutonomousWorkflowPortfolioPlan;
  admission?: AutonomousWorkflowPortfolioAdmission | null;
  executionOptions?: Omit<AutonomousWorkflowPortfolioResumableExecutionOptions, "jobId" | "plan" | "admission" | "requireAdmission" | "checkpointSink"> & { checkpoint?: AutonomousWorkflowPortfolioResumableExecutionOptions["checkpoint"]; checkpointSink?: never };
}

export interface AutonomousWorkflowPortfolioRemoteJobQueueHandle {
  readonly maxJobs: number;
  get(jobId: string): Promise<AutonomousWorkflowPortfolioRemoteJob | null> | AutonomousWorkflowPortfolioRemoteJob | null;
  pending(limit?: number, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob[]> | AutonomousWorkflowPortfolioRemoteJob[];
  claim(jobId: string, workerId: string, leaseMs?: number, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob | null> | AutonomousWorkflowPortfolioRemoteJob | null;
  renew(jobId: string, workerId: string, leaseMs?: number, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  checkpoint(jobId: string, workerId: string, checkpointDigest: string, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  beginExecution(jobId: string, workerId: string, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  complete(jobId: string, workerId: string, input: { status: "completed" | "partial" | "blocked" | "approval_required"; resultDigest: string; traceDigest?: string | null }, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  fail(jobId: string, workerId: string, failureClass: AutonomousWorkflowPortfolioRemoteJobFailureClass, retryable: boolean, failureCode?: string, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  reconcile(jobId: string, workerId: string, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  settleReconciliation(jobId: string, options: AutonomousWorkflowPortfolioRemoteJobReconciliationOptions, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  reclaimExpired(now?: number, limit?: number): Promise<AutonomousWorkflowPortfolioRemoteJob[]> | AutonomousWorkflowPortfolioRemoteJob[];
  requeue(jobId: string, now?: number, options?: AutonomousWorkflowPortfolioRemoteJobRequeueOptions): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  cancel(jobId: string, now?: number): Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob;
  snapshot(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot> | AutonomousWorkflowPortfolioRemoteJobQueueSnapshot;
}
export type AutonomousWorkflowPortfolioRemoteJobResolver = (job: AutonomousWorkflowPortfolioRemoteJob, context: { workerId: string; renew: (leaseMs?: number, now?: number) => Promise<AutonomousWorkflowPortfolioRemoteJob> | AutonomousWorkflowPortfolioRemoteJob }) => Promise<AutonomousWorkflowPortfolioRemoteJobResolution> | AutonomousWorkflowPortfolioRemoteJobResolution;

function executionStatus(status: AutonomousWorkflowPortfolioExecutionStatus): "completed" | "partial" | "blocked" | "approval_required" {
  if (status === "completed") return "completed";
  if (status === "blocked") return "blocked";
  if (status === "approval_required") return "approval_required";
  return "partial";
}

function errorForWorker(error: unknown): { failureClass: AutonomousWorkflowPortfolioRemoteJobFailureClass; failureCode: string; retryable: boolean } {
  if (error instanceof ProviderRuntimeError) {
    const code = typeof error.code === "string" && /^[A-Za-z0-9_.:+-]+$/.test(error.code) ? error.code : "provider_runtime_error";
    return { failureClass: error.code === "transport" || error.retryable ? "transport_error" : "provider_execution_failed", failureCode: code, retryable: error.retryable === true || error.code === "transport" };
  }
  const record = isObject(error) ? error : null;
  const code = record && typeof record.code === "string" && /^[A-Za-z0-9_.:+-]+$/.test(record.code) ? record.code : "portfolio_remote_worker_error";
  return { failureClass: "executor_error", failureCode: code, retryable: false };
}

/** Pull-based remote portfolio worker; private requests and credentials remain resolver-owned. */
export class AutonomousWorkflowPortfolioRemoteWorker {
  constructor(
    readonly agent: AutonomousAgent,
    readonly queue: AutonomousWorkflowPortfolioRemoteJobQueueHandle,
    readonly resolver: AutonomousWorkflowPortfolioRemoteJobResolver,
    readonly workerId: string,
  ) {
    if (!agent || typeof agent.executeWorkflowPortfolioResumable !== "function") throw new ArgumentError("portfolio remote worker requires an AutonomousAgent");
    if (!isPortfolioRemoteJobQueueHandle(queue)) throw new ArgumentError("portfolio remote worker requires a queue handle implementing the remote portfolio contract");
    if (typeof resolver !== "function") throw new ArgumentError("portfolio remote worker resolver must be callable");
    identifier("portfolio remote worker workerId", workerId);
  }

  async run(options: { limit?: number; leaseMs?: number; heartbeatMs?: number; now?: number; clock?: () => number; signal?: { readonly aborted: boolean } } = {}): Promise<AutonomousWorkflowPortfolioRemoteWorkerRun> {
    const limit = boundedInteger("portfolio remote worker limit", options.limit, 1, this.queue.maxJobs, 1);
    const leaseMs = boundedInteger("portfolio remote worker leaseMs", options.leaseMs, 100, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_LEASE_MS, 30_000);
    const heartbeatMs = boundedInteger("portfolio remote worker heartbeatMs", options.heartbeatMs ?? Math.min(30_000, Math.floor(leaseMs / 3)), 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_REMOTE_WORKER_HEARTBEAT_MS);
    if (heartbeatMs >= leaseMs) throw new ArgumentError("portfolio remote worker heartbeatMs must be less than leaseMs");
    if (options.clock !== undefined && typeof options.clock !== "function") throw new ArgumentError("portfolio remote worker clock must be callable");
    const requestedNow = options.now;
    const time = timestamp("portfolio remote worker now", requestedNow ?? (options.clock ?? (() => Date.now()))());
    const clock = options.clock ?? (requestedNow === undefined ? (() => Date.now()) : (() => time));
    const rows: AutonomousWorkflowPortfolioRemoteWorkerRow[] = [];
    for (const expired of await this.queue.reclaimExpired(time, limit)) {
      if (expired.status === "reconciliation_required") rows.push({ job_id: expired.job_id, outcome: "reconciliation_required", attempts: expired.attempts, result_digest: expired.result_digest, trace_digest: expired.trace_digest, reconciliation_digest: expired.reconciliation_digest, failure_class: expired.failure_class, lease_retained: false });
    }
    const remaining = Math.max(0, limit - rows.length);
    const candidates = remaining > 0 ? await this.queue.pending(remaining, time) : [];
    for (const candidate of candidates) {
      if (options.signal?.aborted) break;
      const claimed = await this.queue.claim(candidate.job_id, this.workerId, leaseMs, time);
      if (!claimed) {
        const current = await this.queue.get(candidate.job_id);
        rows.push({ job_id: candidate.job_id, outcome: current?.status === "reconciliation_required" ? "reconciliation_required" : "leased_elsewhere", attempts: current?.attempts ?? candidate.attempts, result_digest: current?.result_digest ?? null, trace_digest: current?.trace_digest ?? null, reconciliation_digest: current?.reconciliation_digest ?? null, failure_class: current?.failure_class ?? null, lease_retained: false });
        continue;
      }
      let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
      let heartbeatRunning = false;
      let heartbeatError: unknown = null;
      const heartbeat = async (): Promise<void> => {
        if (heartbeatRunning || heartbeatError !== null) return;
        heartbeatRunning = true;
        try {
          await this.queue.renew(claimed.job_id, this.workerId, leaseMs, clock());
        } catch (error) {
          heartbeatError = error;
        } finally {
          heartbeatRunning = false;
        }
      };
      heartbeatTimer = setInterval(() => { void heartbeat(); }, heartbeatMs);
      const unref = (heartbeatTimer as unknown as { unref?: () => void }).unref;
      if (typeof unref === "function") unref.call(heartbeatTimer);
      try {
        const resolved = await this.resolver(claimed, { workerId: this.workerId, renew: (renewLeaseMs = leaseMs, renewAt = clock()) => this.queue.renew(claimed.job_id, this.workerId, renewLeaseMs, renewAt) });
        if (!resolved || !Array.isArray(resolved.requests) || !resolved.plan) throw new ProviderRuntimeError("portfolio remote worker resolver returned no private execution binding", { code: "configuration", retryable: false });
        const plan = await validateAutonomousWorkflowPortfolioPlan(resolved.plan);
        if (plan.portfolio_digest !== claimed.plan_digest || canonicalJson(plan.items.map((item) => item.item_id)) !== canonicalJson(claimed.item_ids) || canonicalJson(plan.items.map((item) => item.request_digest)) !== canonicalJson(claimed.request_digests)) throw new ProviderRuntimeError("portfolio remote worker plan or request identity drifted", { code: "protocol", retryable: false });
        const observedRequestIdentities = await computePrivateRequestDigests(resolved.requests);
        const observedById = new Map(observedRequestIdentities.map((entry) => [entry.itemId, entry.digest]));
        const observedRequestDigests = claimed.item_ids.map((itemId) => observedById.get(itemId) ?? null);
        if (observedRequestDigests.some((entry) => entry === null) || canonicalJson(observedRequestDigests) !== canonicalJson(claimed.request_digests)) throw new ProviderRuntimeError("portfolio remote worker private request identity drifted", { code: "protocol", retryable: false });
        const admission = resolved.admission === undefined || resolved.admission === null ? null : await validateAutonomousWorkflowPortfolioAdmission(resolved.admission);
        if (claimed.require_admission && admission === null) throw new ProviderRuntimeError("portfolio remote worker requires a reviewed admission", { code: "protocol", retryable: false });
        if ((admission?.admission_digest ?? null) !== claimed.admission_digest) throw new ProviderRuntimeError("portfolio remote worker admission identity drifted", { code: "protocol", retryable: false });
        const executionOptions = {
          ...(resolved.executionOptions ?? {}),
          plan,
          ...(admission === null ? {} : { admission }),
          jobId: claimed.job_id,
          requireAdmission: claimed.require_admission,
          checkpointSink: async (checkpoint: { checkpoint_digest: string }) => { await this.queue.checkpoint(claimed.job_id, this.workerId, checkpoint.checkpoint_digest, clock()); },
        } as AutonomousWorkflowPortfolioResumableExecutionOptions;
        if (claimed.trace_id !== null && executionOptions.traceId !== claimed.trace_id) throw new ProviderRuntimeError("portfolio remote worker trace identity drifted", { code: "protocol", retryable: false });
        if (heartbeatError !== null) throw new ProviderRuntimeError("portfolio remote worker lease heartbeat failed before dispatch", { code: "transport", retryable: true });
        await this.queue.beginExecution(claimed.job_id, this.workerId, clock());
        const execution = await this.agent.executeWorkflowPortfolioResumable(resolved.requests, executionOptions);
        if (!(execution instanceof AutonomousWorkflowPortfolioExecutionResult)) throw new ProviderRuntimeError("portfolio remote worker execution result is malformed", { code: "protocol", retryable: false });
        if (heartbeatError !== null) throw new ProviderRuntimeError("portfolio remote worker lease heartbeat failed after dispatch", { code: "transport", retryable: true });
        const completed = await this.queue.complete(claimed.job_id, this.workerId, { status: executionStatus(execution.status), resultDigest: execution.executionDigest, traceDigest: execution.traceDigest }, clock());
        rows.push({ job_id: completed.job_id, outcome: completed.status as "completed" | "partial" | "blocked" | "approval_required", attempts: completed.attempts, result_digest: completed.result_digest, trace_digest: completed.trace_digest, reconciliation_digest: completed.reconciliation_digest, failure_class: completed.failure_class, lease_retained: false });
      } catch (error) {
        const failure = errorForWorker(error);
        try {
          const failed = await this.queue.fail(claimed.job_id, this.workerId, failure.failureClass, failure.retryable, failure.failureCode, clock());
          rows.push({ job_id: failed.job_id, outcome: failed.status === "queued" ? "retry_scheduled" : failed.status === "reconciliation_required" ? "reconciliation_required" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, trace_digest: failed.trace_digest, reconciliation_digest: failed.reconciliation_digest, failure_class: failed.failure_class ?? failed.failure_class, lease_retained: false });
        } catch {
          const current = await this.queue.get(claimed.job_id);
          rows.push({ job_id: claimed.job_id, outcome: current?.status === "reconciliation_required" ? "reconciliation_required" : "leased_elsewhere", attempts: current?.attempts ?? claimed.attempts, result_digest: current?.result_digest ?? null, trace_digest: current?.trace_digest ?? null, reconciliation_digest: current?.reconciliation_digest ?? null, failure_class: current?.failure_class ?? failure.failureClass, lease_retained: false });
        }
      } finally {
        if (heartbeatTimer !== null) clearInterval(heartbeatTimer);
      }
    }
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_REMOTE_WORKER_SCHEMA,
      worker_id: this.workerId,
      inspected: rows.length,
      completed: rows.filter((row) => row.outcome === "completed").length,
      partial: rows.filter((row) => row.outcome === "partial").length,
      blocked: rows.filter((row) => row.outcome === "blocked").length,
      approval_required: rows.filter((row) => row.outcome === "approval_required").length,
      retried: rows.filter((row) => row.outcome === "retry_scheduled").length,
      failed: rows.filter((row) => row.outcome === "failed").length,
      reconciled: rows.filter((row) => row.outcome === "reconciliation_required").length,
      leased_elsewhere: rows.filter((row) => row.outcome === "leased_elsewhere").length,
      rows,
      retention: "metadata_only_job_receipts_and_digests_no_private_values",
      secret_material: "never_returned",
    };
  }
}

/** CAS-fenced persistence coordinator for remote portfolio jobs. */
export class AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly queue: InMemoryAutonomousWorkflowPortfolioRemoteJobQueue, readonly persistence: AutonomousWorkflowPortfolioRemoteJobQueuePersistence) {
    if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioRemoteJobQueue)) throw new ArgumentError("portfolio remote job persistence requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("portfolio remote job persistence adapter is malformed");
  }

  get maxJobs(): number { return this.queue.maxJobs; }

  async restore(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot | null> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        const emptyBody = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA, jobs: [], retention: JOB_RETENTION, secret_material: JOB_SECRET_MATERIAL } as const;
        this.queue.restore({ ...emptyBody, snapshot_digest: digestJsonSync(emptyBody) });
        this.expectedSnapshotDigest = null;
        return null;
      }
      this.queue.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.queue.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("portfolio remote job persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async get(jobId: string): Promise<AutonomousWorkflowPortfolioRemoteJob | null> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.get(jobId);
    });
  }

  async pending(limit = 1, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob[]> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.pending(limit, now);
    });
  }

  async reclaimExpired(now = Date.now(), limit = this.maxJobs): Promise<AutonomousWorkflowPortfolioRemoteJob[]> {
    return this.transact((queue) => queue.reclaimExpired(now, limit));
  }

  async claim(jobId: string, workerId: string, leaseMs = 30_000, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob | null> {
    return this.transact((queue) => queue.claim(jobId, workerId, leaseMs, now));
  }

  async renew(jobId: string, workerId: string, leaseMs = 30_000, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.renew(jobId, workerId, leaseMs, now));
  }

  async beginExecution(jobId: string, workerId: string, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.beginExecution(jobId, workerId, now));
  }

  async checkpoint(jobId: string, workerId: string, checkpointDigest: string, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.checkpoint(jobId, workerId, checkpointDigest, now));
  }

  async complete(jobId: string, workerId: string, input: { status: "completed" | "partial" | "blocked" | "approval_required"; resultDigest: string; traceDigest?: string | null }, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.complete(jobId, workerId, input, now));
  }

  async fail(jobId: string, workerId: string, failureClass: AutonomousWorkflowPortfolioRemoteJobFailureClass, retryable: boolean, failureCode: string = failureClass, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.fail(jobId, workerId, failureClass, retryable, failureCode, now));
  }

  async reconcile(jobId: string, workerId: string, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.reconcile(jobId, workerId, now));
  }

  async settleReconciliation(jobId: string, options: AutonomousWorkflowPortfolioRemoteJobReconciliationOptions, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.settleReconciliation(jobId, options, now));
  }

  async requeue(jobId: string, now = Date.now(), options: AutonomousWorkflowPortfolioRemoteJobRequeueOptions = {}): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.requeue(jobId, now, options));
  }

  async cancel(jobId: string, now = Date.now()): Promise<AutonomousWorkflowPortfolioRemoteJob> {
    return this.transact((queue) => queue.cancel(jobId, now));
  }

  async snapshot(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.snapshot();
    });
  }

  private async loadLatest(): Promise<string | null> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) {
      const emptyBody = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_QUEUE_SCHEMA, jobs: [], retention: JOB_RETENTION, secret_material: JOB_SECRET_MATERIAL } as const;
      this.queue.restore({ ...emptyBody, snapshot_digest: digestJsonSync(emptyBody) });
      this.expectedSnapshotDigest = null;
      return null;
    }
    this.queue.restore(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot.snapshot_digest;
  }

  private async transact<T>(operation: (queue: InMemoryAutonomousWorkflowPortfolioRemoteJobQueue) => T): Promise<T> {
    return this.enqueue(async () => {
      for (let attempt = 0; attempt < 4; attempt += 1) {
        const expected = await this.loadLatest();
        const before = this.queue.snapshot();
        const result = operation(this.queue);
        const after = this.queue.snapshot();
        if (after.snapshot_digest === before.snapshot_digest) return result;
        if (typeof this.persistence.writeIfUnchanged === "function") {
          if (await this.persistence.writeIfUnchanged(expected, after)) {
            this.expectedSnapshotDigest = after.snapshot_digest;
            return result;
          }
        } else {
          await this.persistence.write(after);
          this.expectedSnapshotDigest = after.snapshot_digest;
          return result;
        }
      }
      throw new ArgumentError("portfolio remote job persistence compare-and-swap conflicted repeatedly");
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence implements AutonomousWorkflowPortfolioRemoteJobQueuePersistence {
  constructor(readonly textStore: AutonomousWorkflowPortfolioRemoteJobQueueTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("portfolio remote job text store is malformed");
  }

  async read(): Promise<AutonomousWorkflowPortfolioRemoteJobQueueSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_JOB_SNAPSHOT_BYTES) throw new ArgumentError("portfolio remote job JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("portfolio remote job JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("portfolio remote job JSON is not canonical");
    return validateAutonomousWorkflowPortfolioRemoteJobQueueSnapshot(parsed);
  }

  async write(snapshot: AutonomousWorkflowPortfolioRemoteJobQueueSnapshot): Promise<void> {
    const validated = validateAutonomousWorkflowPortfolioRemoteJobQueueSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

export class TransactionalJsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence extends JsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence {
  declare readonly textStore: AutonomousWorkflowPortfolioRemoteJobQueueTransactionalTextStore;

  constructor(textStore: AutonomousWorkflowPortfolioRemoteJobQueueTransactionalTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("portfolio remote job text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioRemoteJobQueueSnapshot): Promise<boolean> {
    const validated = validateAutonomousWorkflowPortfolioRemoteJobQueueSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

export class WebStorageAutonomousWorkflowPortfolioRemoteJobQueueTextStore implements AutonomousWorkflowPortfolioRemoteJobQueueTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("portfolio remote job Web Storage adapter is malformed");
    identifier("portfolio remote job storage key", key);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}
