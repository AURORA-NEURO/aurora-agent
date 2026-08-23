import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import { AutonomousMissionExecutor } from "./mission-execution.js";
import {
  AutonomousMissionReplanContractError,
  runAutonomousMissionReplanCycle,
  type AutonomousMissionReplanOptions,
  type AutonomousMissionReplanResult,
  type AutonomousMissionReplanStatus,
} from "./mission-replan.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type { AgentMissionArgs, JsonObject } from "./types.js";

/** Metadata-only remote handoff queue for one caller-owned mission replan cycle. */
export const AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA = "bioprism-typescript-autonomous-mission-replan-job-queue/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_JOB_SCHEMA = "bioprism-typescript-autonomous-mission-replan-job/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_REMOTE_WORKER_SCHEMA = "bioprism-typescript-autonomous-mission-replan-remote-worker/0.1" as const;
export const MAX_AUTONOMOUS_MISSION_REPLAN_JOBS = 4_096;
export const MAX_AUTONOMOUS_MISSION_REPLAN_JOB_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_MISSION_REPLAN_JOB_LEASE_MS = 300_000;
export const MAX_AUTONOMOUS_MISSION_REPLAN_WORKER_HEARTBEAT_MS = 120_000;
export const MAX_AUTONOMOUS_MISSION_REPLAN_JOB_SNAPSHOT_BYTES = 512_000;

export type AutonomousMissionReplanRemoteJobStatus =
  | "queued"
  | "leased"
  | "completed"
  | "plan_review_required"
  | "approval_required"
  | "reconciliation_required"
  | "failed"
  | "dead_lettered"
  | "cancelled";

export type AutonomousMissionReplanRemoteJobFailureClass =
  | "resolver_missing"
  | "contract_mismatch"
  | "plan_mismatch"
  | "approval_required"
  | "reconciliation_required"
  | "lease_expired"
  | "provider_error"
  | "execution_error"
  | "transport_error"
  | "unknown";

export interface AutonomousMissionReplanRemoteJob extends JsonObject {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_JOB_SCHEMA;
  job_id: string;
  root_mission_id: string;
  protected_contract_digest: string;
  planning_status: "unknown" | "disabled" | "approval_required" | "plan_review_required" | "provider_invalid" | "provider_disagreement" | "accepted";
  plan_refinement_digest: string | null;
  status: AutonomousMissionReplanRemoteJobStatus;
  max_attempts: number;
  attempts: number;
  available_at: number;
  updated_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  result_digest: string | null;
  planner_learning_settlement_digest: string | null;
  failure_class: AutonomousMissionReplanRemoteJobFailureClass | null;
  failure_code: string | null;
  job_digest: string;
  retention: "metadata_only_mission_and_plan_digests;mission_payloads_prompts_credentials_outputs_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousMissionReplanRemoteJobQueueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA;
  jobs: AutonomousMissionReplanRemoteJob[];
  snapshot_digest: string;
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
}

export interface AutonomousMissionReplanRemoteJobQueuePersistence {
  read(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot | null> | AutonomousMissionReplanRemoteJobQueueSnapshot | null;
  write(snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousMissionReplanRemoteJobQueueTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousMissionReplanRemoteJobQueueTransactionalTextStore extends AutonomousMissionReplanRemoteJobQueueTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousMissionReplanRemoteJobAdmission {
  jobId: string;
  rootMissionId: string;
  protectedContractDigest: string;
  planningStatus?: AutonomousMissionReplanRemoteJob["planning_status"];
  planRefinementDigest?: string | null;
  maxAttempts?: number;
  availableAt?: number;
}

export interface AutonomousMissionReplanRemoteJobRequeueOptions {
  planningStatus?: Exclude<AutonomousMissionReplanRemoteJob["planning_status"], "unknown">;
  planRefinementDigest?: string | null;
  availableAt?: number;
}

export interface AutonomousMissionReplanRemoteJobQueueHandle {
  enqueue(input: AutonomousMissionReplanRemoteJobAdmission): Promise<AutonomousMissionReplanRemoteJob>;
  load(jobId: string): Promise<AutonomousMissionReplanRemoteJob | null>;
  claimNext(workerId: string, leaseMs?: number, now?: number): Promise<AutonomousMissionReplanRemoteJob | null>;
  renew(jobId: string, workerId: string, leaseMs?: number, now?: number): Promise<AutonomousMissionReplanRemoteJob>;
  complete(jobId: string, workerId: string, result: AutonomousMissionReplanResult, now?: number): Promise<AutonomousMissionReplanRemoteJob>;
  fail(jobId: string, workerId: string, failureClass: AutonomousMissionReplanRemoteJobFailureClass, failureCode: string, retryable: boolean, now?: number): Promise<AutonomousMissionReplanRemoteJob>;
  cancel(jobId: string, now?: number): Promise<AutonomousMissionReplanRemoteJob>;
  requeue(jobId: string, options?: AutonomousMissionReplanRemoteJobRequeueOptions, now?: number): Promise<AutonomousMissionReplanRemoteJob>;
  snapshot(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot>;
}

export interface AutonomousMissionReplanRemoteWorkerRow extends JsonObject {
  job_id: string;
  outcome: "completed" | "plan_review_required" | "approval_required" | "reconciliation_required" | "retry_scheduled" | "failed" | "leased_elsewhere";
  attempts: number;
  result_digest: string | null;
  plan_refinement_digest: string | null;
  planner_learning_settlement_digest: string | null;
  failure_class: AutonomousMissionReplanRemoteJobFailureClass | null;
  lease_retained: false;
}

export interface AutonomousMissionReplanRemoteWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_REMOTE_WORKER_SCHEMA;
  worker_id: string;
  inspected: number;
  completed: number;
  plan_review_required: number;
  approval_required: number;
  reconciliations: number;
  retried: number;
  failed: number;
  leased_elsewhere: number;
  rows: AutonomousMissionReplanRemoteWorkerRow[];
  retention: "metadata_only_job_receipts_and_digests_no_private_values";
  secret_material: "never_returned";
}

const JOB_RETENTION = "metadata_only_mission_and_plan_digests;mission_payloads_prompts_credentials_outputs_never_persisted" as const;
const JOB_STATUSES: readonly AutonomousMissionReplanRemoteJobStatus[] = ["queued", "leased", "completed", "plan_review_required", "approval_required", "reconciliation_required", "failed", "dead_lettered", "cancelled"];
const PLANNING_STATUSES: readonly AutonomousMissionReplanRemoteJob["planning_status"][] = ["unknown", "disabled", "approval_required", "plan_review_required", "provider_invalid", "provider_disagreement", "accepted"];
const FAILURE_CLASSES: readonly AutonomousMissionReplanRemoteJobFailureClass[] = ["resolver_missing", "contract_mismatch", "plan_mismatch", "approval_required", "reconciliation_required", "lease_expired", "provider_error", "execution_error", "transport_error", "unknown"];

function clone<T>(value: T): T { return structuredClone(value); }

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number, fallback?: number): number {
  if (value === undefined && fallback !== undefined) return fallback;
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function timestamp(name: string, value: unknown, fallback?: number): number {
  const candidate = value === undefined && fallback !== undefined ? fallback : value;
  if (typeof candidate !== "number" || !Number.isFinite(candidate) || candidate < 0 || candidate > Number.MAX_SAFE_INTEGER) throw new ArgumentError(`${name} must be a finite non-negative timestamp`);
  return candidate;
}

function planningStatus(value: unknown): AutonomousMissionReplanRemoteJob["planning_status"] {
  if (!PLANNING_STATUSES.includes(value as AutonomousMissionReplanRemoteJob["planning_status"])) throw new ArgumentError("mission remote planning_status is invalid");
  return value as AutonomousMissionReplanRemoteJob["planning_status"];
}

function jobDescriptor(job: AutonomousMissionReplanRemoteJob): JsonObject {
  const { job_digest: _jobDigest, ...descriptor } = job;
  return descriptor;
}

function jobDigest(job: AutonomousMissionReplanRemoteJob): string { return digestJsonSync(jobDescriptor(job)); }

function resultDigest(result: AutonomousMissionReplanResult): string {
  return digestJsonSync({
    schema: result.schema,
    status: result.status,
    root_mission_id: result.root_mission_id,
    protected_contract_digest: result.protected_contract_digest,
    planning_status: result.planning_status,
    plan_refinement_digest: result.plan_refinement ? digestJsonSync(result.plan_refinement) : null,
    planner_learning_status: result.planner_learning_status,
    planner_learning_settlement_digest: result.planner_learning_settlement ? digestJsonSync(result.planner_learning_settlement) : null,
    replan_count: result.replan_count,
  });
}

function failureProjection(error: unknown): { failureClass: AutonomousMissionReplanRemoteJobFailureClass; failureCode: string } {
  if (error instanceof ProviderRuntimeError) return { failureClass: "provider_error", failureCode: error.code };
  if (error instanceof AutonomousMissionReplanContractError) return { failureClass: "contract_mismatch", failureCode: "mission_contract" };
  if (error instanceof ArgumentError) return { failureClass: "contract_mismatch", failureCode: "configuration" };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { failureClass: "execution_error", failureCode: error.constructor.name };
  return { failureClass: "unknown", failureCode: "unknown" };
}

function classifyResult(result: AutonomousMissionReplanResult): AutonomousMissionReplanRemoteJobStatus {
  if (["completed", "completed_without_replan", "replan_limit_reached", "succeeded", "partial", "failed", "cancelled"].includes(result.status)) return result.status === "failed" ? "failed" : "completed";
  if (result.status === "plan_review_required" || result.status === "planning_provider_invalid" || result.status === "planning_provider_disagreement") return "plan_review_required";
  if (result.status === "planning_approval_required" || result.status === "approval_required") return "approval_required";
  if (result.status === "reconciliation_required") return "reconciliation_required";
  return "failed";
}

function validateJob(value: unknown): AutonomousMissionReplanRemoteJob {
  if (!isObject(value)) throw new ArgumentError("mission remote job must be an object");
  const job = value as unknown as AutonomousMissionReplanRemoteJob;
  if (job.schema !== AUTONOMOUS_MISSION_REPLAN_JOB_SCHEMA || job.retention !== JOB_RETENTION || job.secret_material !== "never_returned") throw new ArgumentError("mission remote job retention markers are invalid");
  identifier("mission remote job_id", job.job_id);
  identifier("mission remote root_mission_id", job.root_mission_id);
  digest("mission remote protected_contract_digest", job.protected_contract_digest);
  planningStatus(job.planning_status);
  if (!JOB_STATUSES.includes(job.status)) throw new ArgumentError("mission remote job status is invalid");
  digest("mission remote plan_refinement_digest", job.plan_refinement_digest, true);
  integer("mission remote max_attempts", job.max_attempts, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_ATTEMPTS);
  integer("mission remote attempts", job.attempts, 0, job.max_attempts);
  timestamp("mission remote available_at", job.available_at);
  timestamp("mission remote updated_at", job.updated_at);
  if (job.lease_owner !== null) identifier("mission remote lease_owner", job.lease_owner);
  if (job.lease_until !== null) timestamp("mission remote lease_until", job.lease_until);
  digest("mission remote result_digest", job.result_digest, true);
  digest("mission remote planner_learning_settlement_digest", job.planner_learning_settlement_digest, true);
  if (job.failure_class !== null && !FAILURE_CLASSES.includes(job.failure_class)) throw new ArgumentError("mission remote failure_class is invalid");
  if (job.failure_code !== null) identifier("mission remote failure_code", job.failure_code);
  digest("mission remote job_digest", job.job_digest);
  if (jobDigest(job) !== job.job_digest) throw new ArgumentError("mission remote job digest does not match metadata");
  return clone(job);
}

function validateSnapshot(value: unknown): AutonomousMissionReplanRemoteJobQueueSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA || value.retention !== "metadata_only_hash_bound" || value.secret_material !== "never_returned" || !Array.isArray(value.jobs)) throw new ArgumentError("mission remote queue snapshot is malformed");
  if (value.jobs.length > MAX_AUTONOMOUS_MISSION_REPLAN_JOBS) throw new ArgumentError("mission remote queue exceeds its capacity");
  const jobs = value.jobs.map(validateJob);
  if (new Set(jobs.map((job) => job.job_id)).size !== jobs.length) throw new ArgumentError("mission remote queue contains duplicate jobs");
  digest("mission remote snapshot_digest", value.snapshot_digest);
  const descriptor = { schema: value.schema, jobs: jobs.sort((left, right) => left.job_id.localeCompare(right.job_id)), retention: value.retention, secret_material: value.secret_material };
  if (digestJsonSync(descriptor) !== value.snapshot_digest) throw new ArgumentError("mission remote queue snapshot digest does not match metadata");
  if (new TextEncoder().encode(JSON.stringify(value)).byteLength > MAX_AUTONOMOUS_MISSION_REPLAN_JOB_SNAPSHOT_BYTES) throw new ArgumentError("mission remote queue snapshot exceeds its byte bound");
  return { schema: value.schema as typeof AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA, jobs, snapshot_digest: value.snapshot_digest as string, retention: "metadata_only_hash_bound", secret_material: "never_returned" };
}

function refresh(job: AutonomousMissionReplanRemoteJob, patch: Partial<AutonomousMissionReplanRemoteJob>, now: number): AutonomousMissionReplanRemoteJob {
  const next = { ...job, ...patch, updated_at: timestamp("mission remote updatedAt", now) } as AutonomousMissionReplanRemoteJob;
  next.job_digest = jobDigest(next);
  return next;
}

/** In-memory reference queue; callers can replace it with a transactional persistence adapter. */
export class InMemoryAutonomousMissionReplanRemoteJobQueue implements AutonomousMissionReplanRemoteJobQueueHandle {
  private readonly jobs = new Map<string, AutonomousMissionReplanRemoteJob>();

  async enqueue(input: AutonomousMissionReplanRemoteJobAdmission): Promise<AutonomousMissionReplanRemoteJob> {
    const jobId = identifier("mission remote jobId", input.jobId);
    if (this.jobs.has(jobId)) throw new ArgumentError("mission remote jobId is already queued");
    if (this.jobs.size >= MAX_AUTONOMOUS_MISSION_REPLAN_JOBS) throw new ArgumentError("mission remote queue capacity is exhausted");
    const now = Date.now();
    const job: AutonomousMissionReplanRemoteJob = {
      schema: AUTONOMOUS_MISSION_REPLAN_JOB_SCHEMA,
      job_id: jobId,
      root_mission_id: identifier("mission remote rootMissionId", input.rootMissionId),
      protected_contract_digest: digest("mission remote protectedContractDigest", input.protectedContractDigest)!,
      planning_status: planningStatus(input.planningStatus ?? "unknown"),
      plan_refinement_digest: digest("mission remote planRefinementDigest", input.planRefinementDigest ?? null, true),
      status: "queued",
      max_attempts: integer("mission remote maxAttempts", input.maxAttempts, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_ATTEMPTS, 3),
      attempts: 0,
      available_at: timestamp("mission remote availableAt", input.availableAt, now),
      updated_at: now,
      lease_owner: null,
      lease_until: null,
      result_digest: null,
      planner_learning_settlement_digest: null,
      failure_class: null,
      failure_code: null,
      job_digest: "0".repeat(64),
      retention: JOB_RETENTION,
      secret_material: "never_returned",
    };
    job.job_digest = jobDigest(job);
    this.jobs.set(jobId, clone(job));
    return clone(job);
  }

  async load(jobId: string): Promise<AutonomousMissionReplanRemoteJob | null> { return clone(this.jobs.get(identifier("mission remote jobId", jobId)) ?? null); }

  async claimNext(workerId: string, leaseMs = 60_000, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob | null> {
    const owner = identifier("mission remote workerId", workerId);
    const duration = integer("mission remote leaseMs", leaseMs, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_LEASE_MS);
    const candidate = [...this.jobs.values()].filter((job) => (job.status === "queued" && job.available_at <= now) || (job.status === "leased" && (job.lease_until ?? 0) <= now)).sort((left, right) => left.available_at - right.available_at || left.job_id.localeCompare(right.job_id))[0];
    if (!candidate) return null;
    if (candidate.attempts >= candidate.max_attempts) {
      const dead = refresh(candidate, { status: "dead_lettered", failure_class: "lease_expired", failure_code: "attempt_limit" }, now);
      this.jobs.set(dead.job_id, dead);
      return null;
    }
    const claimed = refresh(candidate, { status: "leased", attempts: candidate.attempts + 1, lease_owner: owner, lease_until: now + duration, failure_class: null, failure_code: null }, now);
    this.jobs.set(claimed.job_id, claimed);
    return clone(claimed);
  }

  async renew(jobId: string, workerId: string, leaseMs = 60_000, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> {
    const job = this.jobs.get(identifier("mission remote jobId", jobId));
    if (!job || job.status !== "leased" || job.lease_owner !== identifier("mission remote workerId", workerId) || (job.lease_until ?? 0) < now) throw new ArgumentError("mission remote lease cannot be renewed");
    const renewed = refresh(job, { lease_until: now + integer("mission remote leaseMs", leaseMs, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_LEASE_MS) }, now);
    this.jobs.set(renewed.job_id, renewed);
    return clone(renewed);
  }

  async complete(jobId: string, workerId: string, result: AutonomousMissionReplanResult, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> {
    const job = this.assertLease(jobId, workerId, now);
    const planDigest = result.plan_refinement ? digestJsonSync(result.plan_refinement) : null;
    if (job.protected_contract_digest !== result.protected_contract_digest) throw new ArgumentError("mission remote result protected contract does not match the job");
    if (job.plan_refinement_digest !== null && job.plan_refinement_digest !== planDigest) throw new ArgumentError("mission remote result plan digest does not match the job");
    const plannerSettlementDigest = result.planner_learning_settlement ? digestJsonSync(result.planner_learning_settlement) : null;
    const next = refresh(job, { status: classifyResult(result), planning_status: result.planning_status, plan_refinement_digest: planDigest, planner_learning_settlement_digest: plannerSettlementDigest, result_digest: resultDigest(result), lease_owner: null, lease_until: null, failure_class: null, failure_code: null }, now);
    this.jobs.set(next.job_id, next);
    return clone(next);
  }

  async fail(jobId: string, workerId: string, failureClass: AutonomousMissionReplanRemoteJobFailureClass, failureCode: string, retryable: boolean, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> {
    const job = this.assertLease(jobId, workerId, now);
    const terminal = !retryable || job.attempts >= job.max_attempts;
    const next = refresh(job, { status: terminal ? "failed" : "queued", available_at: terminal ? job.available_at : now + Math.min(60_000, 1_000 * 2 ** Math.max(0, job.attempts - 1)), lease_owner: null, lease_until: null, failure_class: failureClass, failure_code: identifier("mission remote failureCode", failureCode) }, now);
    this.jobs.set(next.job_id, next);
    return clone(next);
  }

  async cancel(jobId: string, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> {
    const id = identifier("mission remote jobId", jobId);
    const job = this.jobs.get(id);
    if (!job) throw new ArgumentError("mission remote job was not found");
    const next = refresh(job, { status: "cancelled", lease_owner: null, lease_until: null }, now);
    this.jobs.set(id, next);
    return clone(next);
  }

  /** Reopen an explicit review/approval boundary after the caller has rehydrated private state. */
  async requeue(jobId: string, options: AutonomousMissionReplanRemoteJobRequeueOptions = {}, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> {
    const id = identifier("mission remote jobId", jobId);
    const job = this.jobs.get(id);
    if (!job || !["plan_review_required", "approval_required", "reconciliation_required", "failed"].includes(job.status)) throw new ArgumentError("mission remote job is not requeueable");
    const nextStatus = options.planningStatus === undefined ? job.planning_status : planningStatus(options.planningStatus);
    const nextPlanDigest = digest("mission remote requeue planRefinementDigest", options.planRefinementDigest ?? job.plan_refinement_digest, true);
    if (nextStatus === "accepted" && nextPlanDigest === null) throw new ArgumentError("accepted mission remote requeue requires a plan refinement digest");
    const next = refresh(job, { status: "queued", planning_status: nextStatus, plan_refinement_digest: nextPlanDigest, planner_learning_settlement_digest: null, result_digest: null, failure_class: null, failure_code: null, lease_owner: null, lease_until: null, available_at: timestamp("mission remote requeue availableAt", options.availableAt, now) }, now);
    this.jobs.set(id, next);
    return clone(next);
  }

  async snapshot(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot> {
    const jobs = [...this.jobs.values()].sort((left, right) => left.job_id.localeCompare(right.job_id)).map(clone);
    const descriptor = { schema: AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA, jobs, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
  }

  async restore(snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<void> {
    const validated = validateSnapshot(snapshot);
    this.jobs.clear();
    for (const job of validated.jobs) this.jobs.set(job.job_id, clone(job));
  }

  private assertLease(jobId: string, workerId: string, now: number): AutonomousMissionReplanRemoteJob {
    const job = this.jobs.get(identifier("mission remote jobId", jobId));
    if (!job || job.status !== "leased" || job.lease_owner !== identifier("mission remote workerId", workerId) || (job.lease_until ?? 0) < now) throw new ArgumentError("mission remote job lease is not owned by this worker");
    return job;
  }
}

export class JsonAutonomousMissionReplanRemoteJobQueuePersistence {
  readonly store: AutonomousMissionReplanRemoteJobQueuePersistence;
  constructor(store: AutonomousMissionReplanRemoteJobQueuePersistence) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("mission remote queue persistence is malformed");
    this.store = store;
  }
  async flush(queue: InMemoryAutonomousMissionReplanRemoteJobQueue): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot> {
    const snapshot = await queue.snapshot();
    await this.store.write(snapshot);
    return snapshot;
  }
  async restore(queue: InMemoryAutonomousMissionReplanRemoteJobQueue): Promise<boolean> {
    const snapshot = await this.store.read();
    if (snapshot === null) return false;
    await queue.restore(validateSnapshot(snapshot));
    return true;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<boolean> {
    if (typeof this.store.writeIfUnchanged !== "function") {
      await this.store.write(validateSnapshot(snapshot));
      return true;
    }
    return this.store.writeIfUnchanged(expectedSnapshotDigest, validateSnapshot(snapshot));
  }
}

export class JsonAutonomousMissionReplanRemoteJobQueueTextStore implements AutonomousMissionReplanRemoteJobQueuePersistence {
  readonly textStore: AutonomousMissionReplanRemoteJobQueueTextStore;
  constructor(textStore: AutonomousMissionReplanRemoteJobQueueTextStore) { if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("mission remote queue text store is malformed"); this.textStore = textStore; }
  async read(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot | null> {
    const value = await this.textStore.read();
    if (value === null) return null;
    let parsed: unknown;
    try { parsed = JSON.parse(value) as unknown; } catch { throw new ArgumentError("mission remote queue text is not valid JSON"); }
    if (canonicalJson(parsed) !== value) throw new ArgumentError("mission remote queue text is not canonical");
    return validateSnapshot(parsed);
  }
  async write(snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<void> { await this.textStore.write(canonicalJson(validateSnapshot(snapshot))); }
  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot): Promise<boolean> {
    if (typeof (this.textStore as Partial<AutonomousMissionReplanRemoteJobQueueTransactionalTextStore>).writeIfUnchanged !== "function") {
      await this.write(snapshot);
      return true;
    }
    return (this.textStore as AutonomousMissionReplanRemoteJobQueueTransactionalTextStore).writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validateSnapshot(snapshot)));
  }
}

/** CAS-fenced persistence facade for multi-process queue adapters. */
export class AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator implements AutonomousMissionReplanRemoteJobQueueHandle {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly queue: InMemoryAutonomousMissionReplanRemoteJobQueue, readonly persistence: AutonomousMissionReplanRemoteJobQueuePersistence) {
    if (!(queue instanceof InMemoryAutonomousMissionReplanRemoteJobQueue)) throw new ArgumentError("mission remote persistence requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("mission remote persistence adapter is malformed");
  }

  async restore(): Promise<boolean> {
    return this.serialize(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        await this.queue.restore(emptyQueueSnapshot());
        this.expectedSnapshotDigest = null;
        return false;
      }
      await this.queue.restore(validateSnapshot(snapshot));
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return true;
    });
  }

  async flush(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot> {
    return this.serialize(async () => {
      const snapshot = await this.queue.snapshot();
      await this.persist(snapshot, this.expectedSnapshotDigest);
      return clone(snapshot);
    });
  }

  async enqueue(input: AutonomousMissionReplanRemoteJobAdmission): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.enqueue(input)); }
  async load(jobId: string): Promise<AutonomousMissionReplanRemoteJob | null> { return this.serialize(async () => { await this.loadLatest(); return this.queue.load(jobId); }); }
  async claimNext(workerId: string, leaseMs = 60_000, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob | null> { return this.transact((queue) => queue.claimNext(workerId, leaseMs, now)); }
  async renew(jobId: string, workerId: string, leaseMs = 60_000, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.renew(jobId, workerId, leaseMs, now)); }
  async complete(jobId: string, workerId: string, result: AutonomousMissionReplanResult, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.complete(jobId, workerId, result, now)); }
  async fail(jobId: string, workerId: string, failureClass: AutonomousMissionReplanRemoteJobFailureClass, failureCode: string, retryable: boolean, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.fail(jobId, workerId, failureClass, failureCode, retryable, now)); }
  async cancel(jobId: string, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.cancel(jobId, now)); }
  async requeue(jobId: string, options: AutonomousMissionReplanRemoteJobRequeueOptions = {}, now = Date.now()): Promise<AutonomousMissionReplanRemoteJob> { return this.transact((queue) => queue.requeue(jobId, options, now)); }
  async snapshot(): Promise<AutonomousMissionReplanRemoteJobQueueSnapshot> { return this.serialize(async () => { await this.loadLatest(); return this.queue.snapshot(); }); }

  private async loadLatest(): Promise<string | null> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) {
      await this.queue.restore(emptyQueueSnapshot());
      this.expectedSnapshotDigest = null;
      return null;
    }
    await this.queue.restore(validateSnapshot(snapshot));
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot.snapshot_digest;
  }

  private async persist(snapshot: AutonomousMissionReplanRemoteJobQueueSnapshot, expected: string | null): Promise<void> {
    if (typeof this.persistence.writeIfUnchanged === "function") {
      if (!await this.persistence.writeIfUnchanged(expected, snapshot)) throw new ArgumentError("mission remote persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
  }

  private async transact<T>(operation: (queue: InMemoryAutonomousMissionReplanRemoteJobQueue) => Promise<T>): Promise<T> {
    return this.serialize(async () => {
      for (let attempt = 0; attempt < 4; attempt += 1) {
        const expected = await this.loadLatest();
        const before = await this.queue.snapshot();
        const value = await operation(this.queue);
        const after = await this.queue.snapshot();
        if (after.snapshot_digest === before.snapshot_digest) return value;
        try {
          await this.persist(after, expected);
          return value;
        } catch (error) {
          if (attempt === 3) throw error;
        }
      }
      throw new ArgumentError("mission remote persistence compare-and-swap conflicted repeatedly");
    });
  }

  private serialize<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

function emptyQueueSnapshot(): AutonomousMissionReplanRemoteJobQueueSnapshot {
  const descriptor = { schema: AUTONOMOUS_MISSION_REPLAN_JOB_QUEUE_SCHEMA, jobs: [] as AutonomousMissionReplanRemoteJob[], retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
  return { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
}

export interface AutonomousMissionReplanRemoteJobResolution {
  executor: AutonomousMissionExecutor;
  mission: AgentMissionArgs;
  options: AutonomousMissionReplanOptions;
}

export interface AutonomousMissionReplanRemoteJobResolverContext {
  job: AutonomousMissionReplanRemoteJob;
  attempt: number;
  worker_id: string;
  renew: (leaseMs?: number, now?: number) => Promise<AutonomousMissionReplanRemoteJob>;
}

export type AutonomousMissionReplanRemoteJobResolver = (context: AutonomousMissionReplanRemoteJobResolverContext) => AutonomousMissionReplanRemoteJobResolution | Promise<AutonomousMissionReplanRemoteJobResolution>;

export interface AutonomousMissionReplanRemoteWorkerOptions {
  queue: AutonomousMissionReplanRemoteJobQueueHandle;
  workerId: string;
  resolve: AutonomousMissionReplanRemoteJobResolver;
  leaseMs?: number;
}

export interface AutonomousMissionReplanRemoteWorkerRunOptions {
  limit?: number;
  leaseMs?: number;
  heartbeatMs?: number;
  now?: number;
  clock?: () => number;
  signal?: { readonly aborted: boolean };
}

/**
 * Claim-next worker for mission replans. The resolver owns all private mission/provider material;
 * the queue carries only protected-contract, planner, lease, and result digests.
 */
export class AutonomousMissionReplanRemoteWorker {
  readonly queue: AutonomousMissionReplanRemoteJobQueueHandle;
  readonly workerId: string;
  readonly resolve: AutonomousMissionReplanRemoteJobResolver;
  readonly leaseMs: number;

  constructor(options: AutonomousMissionReplanRemoteWorkerOptions) {
    if (!options || (!(options.queue instanceof InMemoryAutonomousMissionReplanRemoteJobQueue) && !(options.queue instanceof AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator))) throw new ArgumentError("mission remote worker requires a typed queue or CAS coordinator");
    this.queue = options.queue;
    this.workerId = identifier("mission remote workerId", options.workerId);
    if (typeof options.resolve !== "function") throw new ArgumentError("mission remote worker resolver must be callable");
    this.resolve = options.resolve;
    this.leaseMs = integer("mission remote worker leaseMs", options.leaseMs, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_LEASE_MS, 60_000);
  }

  async run(limitOrOptions: number | AutonomousMissionReplanRemoteWorkerRunOptions = 1): Promise<AutonomousMissionReplanRemoteWorkerRun> {
    const options = typeof limitOrOptions === "number" ? { limit: limitOrOptions } : (limitOrOptions ?? {});
    const max = integer("mission remote worker limit", options.limit, 1, 64, 1);
    const leaseMs = integer("mission remote worker leaseMs", options.leaseMs, 1, MAX_AUTONOMOUS_MISSION_REPLAN_JOB_LEASE_MS, this.leaseMs);
    const heartbeatMs = integer("mission remote worker heartbeatMs", options.heartbeatMs, 1, MAX_AUTONOMOUS_MISSION_REPLAN_WORKER_HEARTBEAT_MS, Math.min(30_000, Math.floor(leaseMs / 3)));
    if (heartbeatMs >= leaseMs) throw new ArgumentError("mission remote worker heartbeatMs must be less than leaseMs");
    if (options.clock !== undefined && typeof options.clock !== "function") throw new ArgumentError("mission remote worker clock must be callable");
    const requestedNow = options.now;
    const initialNow = timestamp("mission remote worker now", requestedNow ?? (options.clock ?? (() => Date.now()))());
    const clock = options.clock ?? (requestedNow === undefined ? (() => Date.now()) : (() => initialNow));
    const rows: AutonomousMissionReplanRemoteWorkerRow[] = [];
    for (let index = 0; index < max; index += 1) {
      if (options.signal?.aborted) break;
      const job = await this.queue.claimNext(this.workerId, leaseMs, clock());
      if (!job) break;
      let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
      let heartbeatRunning = false;
      let heartbeatError: unknown = null;
      const heartbeat = async (): Promise<void> => {
        if (heartbeatRunning || heartbeatError !== null) return;
        heartbeatRunning = true;
        try {
          await this.queue.renew(job.job_id, this.workerId, leaseMs, clock());
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
        const resolved = await this.resolve({ job: clone(job), attempt: job.attempts, worker_id: this.workerId, renew: (renewLeaseMs = leaseMs, renewAt = clock()) => this.queue.renew(job.job_id, this.workerId, renewLeaseMs, renewAt) });
        if (!(resolved.executor instanceof AutonomousMissionExecutor) || !isObject(resolved.mission) || !isObject(resolved.options)) throw new ArgumentError("mission remote resolver returned malformed private execution state");
        if (heartbeatError !== null) throw new ProviderRuntimeError("mission remote worker lease heartbeat failed before execution", { code: "transport", retryable: true });
        const result = await runAutonomousMissionReplanCycle(resolved.executor, resolved.mission, resolved.options);
        if (heartbeatError !== null) throw new ProviderRuntimeError("mission remote worker lease heartbeat failed after execution", { code: "transport", retryable: true });
        const completed = await this.queue.complete(job.job_id, this.workerId, result, clock());
        rows.push({ job_id: job.job_id, outcome: completed.status === "completed" ? "completed" : completed.status === "plan_review_required" ? "plan_review_required" : completed.status === "approval_required" ? "approval_required" : completed.status === "reconciliation_required" ? "reconciliation_required" : "failed", attempts: completed.attempts, result_digest: completed.result_digest, plan_refinement_digest: completed.plan_refinement_digest, planner_learning_settlement_digest: completed.planner_learning_settlement_digest, failure_class: completed.failure_class, lease_retained: false });
      } catch (error) {
        const projection = failureProjection(error);
        const retryable = error instanceof ProviderRuntimeError ? error.retryable : false;
        try {
          const failed = await this.queue.fail(job.job_id, this.workerId, projection.failureClass, projection.failureCode, retryable, clock());
          rows.push({ job_id: job.job_id, outcome: failed.status === "queued" ? "retry_scheduled" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, plan_refinement_digest: failed.plan_refinement_digest, planner_learning_settlement_digest: failed.planner_learning_settlement_digest, failure_class: failed.failure_class, lease_retained: false });
        } catch {
          const current = await this.queue.load(job.job_id);
          rows.push({ job_id: job.job_id, outcome: "leased_elsewhere", attempts: current?.attempts ?? job.attempts, result_digest: current?.result_digest ?? null, plan_refinement_digest: current?.plan_refinement_digest ?? null, planner_learning_settlement_digest: current?.planner_learning_settlement_digest ?? null, failure_class: current?.failure_class ?? projection.failureClass, lease_retained: false });
        }
      } finally {
        if (heartbeatTimer !== null) clearInterval(heartbeatTimer);
      }
    }
    const counts = {
      completed: rows.filter((row) => row.outcome === "completed").length,
      plan_review_required: rows.filter((row) => row.outcome === "plan_review_required").length,
      approval_required: rows.filter((row) => row.outcome === "approval_required").length,
      reconciliations: rows.filter((row) => row.outcome === "reconciliation_required").length,
      retried: rows.filter((row) => row.outcome === "retry_scheduled").length,
      failed: rows.filter((row) => row.outcome === "failed").length,
      leased_elsewhere: rows.filter((row) => row.outcome === "leased_elsewhere").length,
    };
    return { schema: AUTONOMOUS_MISSION_REPLAN_REMOTE_WORKER_SCHEMA, worker_id: this.workerId, inspected: rows.length, ...counts, rows, retention: "metadata_only_job_receipts_and_digests_no_private_values", secret_material: "never_returned" };
  }
}

export function validateAutonomousMissionReplanRemoteJobQueueSnapshot(value: unknown): AutonomousMissionReplanRemoteJobQueueSnapshot { return validateSnapshot(value); }
