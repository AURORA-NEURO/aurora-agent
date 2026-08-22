import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { ArgumentError, isObject } from "./errors.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Local scheduler contract for autonomous brain work.
 *
 * The scheduler is deliberately provider-neutral. It stores only identities, routing labels,
 * leases, checkpoints, and digests; the embedding application rehydrates the task, prompt,
 * credentials, model catalogue, evaluator, and connector values when it owns a lease. This makes
 * the same brain facade usable in a browser worker, a Node worker, or behind the durable Python
 * control plane without silently turning a local snapshot into a secret store.
 */
export const AUTONOMOUS_BRAIN_JOB_SCHEMA = "bioprism-typescript-autonomous-brain-job/0.1" as const;
export const AUTONOMOUS_BRAIN_JOB_EVENT_SCHEMA = "bioprism-typescript-autonomous-brain-job-event/0.1" as const;
export const AUTONOMOUS_BRAIN_JOB_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-brain-job-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_JOBS = 1_024;
export const MAX_AUTONOMOUS_BRAIN_JOB_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_BRAIN_JOB_PRIORITY = 255;
export const MAX_AUTONOMOUS_BRAIN_JOB_LEASE_MS = 600_000;
export const MAX_AUTONOMOUS_BRAIN_JOB_CHECKPOINT_BYTES = 4_096;
export const MAX_AUTONOMOUS_BRAIN_JOB_SNAPSHOT_BYTES = 4 * 1024 * 1024;
export const MAX_AUTONOMOUS_BRAIN_JOB_EVENTS = 16_384;
export const AUTONOMOUS_BRAIN_JOB_AGING_INTERVAL_MS = 60_000;
export const AUTONOMOUS_BRAIN_JOB_MAX_AGING_BONUS = 64;

export type AutonomousBrainJobState =
  | "queued"
  | "leased"
  | "running"
  | "waiting_approval"
  | "succeeded"
  | "failed"
  | "dead_lettered"
  | "cancelled"
  | "reconciliation_required";
export type AutonomousBrainJobBoundary = "not_started" | "preflight" | "dispatched" | "unknown";
export type AutonomousBrainJobReconciliationOutcome = "succeeded" | "failed" | "not_executed" | "unknown";

const JOB_STATES: readonly AutonomousBrainJobState[] = [
  "queued", "leased", "running", "waiting_approval", "succeeded", "failed", "dead_lettered",
  "cancelled", "reconciliation_required",
];
const JOB_BOUNDARIES: readonly AutonomousBrainJobBoundary[] = ["not_started", "preflight", "dispatched", "unknown"];
const BOUNDARY_ORDER: Readonly<Record<AutonomousBrainJobBoundary, number>> = {
  not_started: 0,
  preflight: 1,
  dispatched: 2,
  unknown: 3,
};
const TERMINAL_STATES = new Set<AutonomousBrainJobState>(["succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"]);
let generatedJobSequence = 0;

export interface AutonomousBrainJobSubmission {
  jobId?: string;
  idempotencyKey: string;
  specDigest: string;
  domain: AutonomousDomainName;
  capability: string;
  riskClass: string;
  priority?: number;
  maxAttempts?: number;
  checkpointDigest?: string | null;
}

export interface AutonomousBrainJob extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_JOB_SCHEMA;
  job_id: string;
  /** Digest only; the raw idempotency key is never retained or returned. */
  idempotency_key_digest: string;
  spec_digest: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  priority: number;
  max_attempts: number;
  state: AutonomousBrainJobState;
  attempts: number;
  lease_owner: string | null;
  lease_expires_at: number | null;
  checkpoint_digest: string | null;
  reconciliation_digest: string | null;
  result_digest: string | null;
  side_effect_boundary: AutonomousBrainJobBoundary;
  recovered_after_restart: boolean;
  reason_digest: string | null;
  created_at: number;
  updated_at: number;
  job_digest: string;
  retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousBrainJobEvent extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_JOB_EVENT_SCHEMA;
  sequence: number;
  event_type: string;
  job_id: string;
  metadata: JsonObject;
  previous_digest: string;
  event_digest: string;
  created_at: number;
  retention: "metadata_only_hash_chained";
  secret_material: "never_returned";
}

export interface AutonomousBrainJobSubmissionResult extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_JOB_SCHEMA;
  created: boolean;
  idempotent: boolean;
  job: AutonomousBrainJob;
  event: AutonomousBrainJobEvent;
  retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousBrainJobSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_JOB_SNAPSHOT_SCHEMA;
  jobs: AutonomousBrainJob[];
  events: AutonomousBrainJobEvent[];
  retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousBrainJobSchedulerPersistence {
  read(): Promise<AutonomousBrainJobSnapshot | null> | AutonomousBrainJobSnapshot | null;
  write(snapshot: AutonomousBrainJobSnapshot): Promise<void> | void;
  /**
   * Optional atomic snapshot fence for shared stores. Returning false means another scheduler
   * committed a newer snapshot after this coordinator restored its expected digest.
   */
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousBrainJobSnapshot): Promise<boolean> | boolean;
}

/** Minimal text store used by the concrete JSON persistence adapters. */
export interface AutonomousBrainJobSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

/** Text store contract with an atomic digest fence for shared workers. */
export interface AutonomousBrainJobTransactionalSnapshotTextStore extends AutonomousBrainJobSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

/**
 * Concrete metadata-only JSON persistence for browser, Node, or embedded text stores.
 * The store owns durability; this adapter owns bounded serialization and fail-closed parsing.
 */
export class JsonAutonomousBrainJobSchedulerPersistence implements AutonomousBrainJobSchedulerPersistence {
  protected readonly store: AutonomousBrainJobSnapshotTextStore;

  constructor(store: AutonomousBrainJobSnapshotTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("brain job JSON persistence requires a text store");
    this.store = store;
  }

  async read(): Promise<AutonomousBrainJobSnapshot | null> {
    return this.decode(await this.store.read());
  }

  async write(snapshot: AutonomousBrainJobSnapshot): Promise<void> {
    await this.store.write(this.encode(snapshot));
  }

  protected encode(snapshot: AutonomousBrainJobSnapshot): string {
    if (!snapshot || typeof snapshot !== "object" || Array.isArray(snapshot)) throw new ArgumentError("brain job JSON persistence snapshot is malformed");
    const encoded = JSON.stringify(snapshot);
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_BRAIN_JOB_SNAPSHOT_BYTES) throw new ArgumentError("brain job JSON persistence snapshot exceeds its bound");
    return encoded;
  }

  protected decode(encoded: string | null): AutonomousBrainJobSnapshot | null {
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_BRAIN_JOB_SNAPSHOT_BYTES) throw new ArgumentError("brain job JSON persistence text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("brain job JSON persistence text is invalid JSON");
    }
    if (!isObject(parsed) || Array.isArray(parsed)) throw new ArgumentError("brain job JSON persistence value is malformed");
    return parsed as unknown as AutonomousBrainJobSnapshot;
  }
}

/** JSON persistence variant that refuses to operate without an atomic snapshot fence. */
export class TransactionalJsonAutonomousBrainJobSchedulerPersistence extends JsonAutonomousBrainJobSchedulerPersistence {
  private readonly transactionalStore: AutonomousBrainJobTransactionalSnapshotTextStore;

  constructor(store: AutonomousBrainJobTransactionalSnapshotTextStore) {
    super(store);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional brain job JSON persistence requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousBrainJobSnapshot): Promise<boolean> {
    const committed = await this.transactionalStore.writeIfUnchanged(expectedSnapshotDigest, this.encode(snapshot));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional brain job JSON persistence returned a non-boolean commit result");
    return committed;
  }
}

/** Browser-compatible single-writer text store for localStorage/sessionStorage-like objects. */
export class WebStorageAutonomousBrainJobSnapshotTextStore implements AutonomousBrainJobSnapshotTextStore {
  readonly storage: Pick<Storage, "getItem" | "setItem">;
  readonly key: string;

  constructor(storage: Pick<Storage, "getItem" | "setItem">, key = "aurora.autonomous.brain.jobs") {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("brain job web storage adapter requires getItem and setItem");
    if (typeof key !== "string" || !key.trim() || key.length > 256 || key.includes("\u0000")) throw new ArgumentError("brain job web storage key is outside its bounds");
    this.storage = storage;
    this.key = key;
  }

  read(): string | null {
    return this.storage.getItem(this.key);
  }

  write(value: string): void {
    if (typeof value !== "string" || bytes(value) > MAX_AUTONOMOUS_BRAIN_JOB_SNAPSHOT_BYTES) throw new ArgumentError("brain job web storage value exceeds its bound");
    this.storage.setItem(this.key, value);
  }
}

export interface AutonomousBrainJobSchedulerOptions {
  maxJobs?: number;
  clock?: () => number;
}

export interface AutonomousBrainJobCheckpointOptions {
  phase: string;
  checkpointDigest?: string | null;
  sideEffectBoundary?: AutonomousBrainJobBoundary;
  waitingForApproval?: boolean;
  now?: number;
}

export interface AutonomousBrainJobFailureOptions {
  reason: string;
  retryable?: boolean;
  now?: number;
}

export interface AutonomousBrainJobReconciliationOptions {
  outcome: AutonomousBrainJobReconciliationOutcome;
  evidenceDigest: string;
  evidenceKind?: string;
  operator?: string;
  reason?: string;
  now?: number;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function nowMs(clock: () => number, value?: number): number {
  const result = value ?? clock();
  if (!Number.isSafeInteger(result) || result < 0) throw new ArgumentError("job timestamp is outside its bounded integer contract");
  return result;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be within [${minimum}, ${maximum}]`);
  return value as number;
}

function jobPayload(job: AutonomousBrainJob): JsonObject {
  const { job_digest: _jobDigest, ...payload } = job;
  return payload;
}

function eventPayload(event: AutonomousBrainJobEvent): JsonObject {
  const { event_digest: _eventDigest, ...payload } = event;
  return payload;
}

function ensureDomain(value: unknown): AutonomousDomainName {
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value as AutonomousDomainName)) throw new ArgumentError("job domain is not a supported autonomous domain");
  return value as AutonomousDomainName;
}

function ensureState(value: unknown): AutonomousBrainJobState {
  if (!JOB_STATES.includes(value as AutonomousBrainJobState)) throw new ArgumentError("job state is invalid");
  return value as AutonomousBrainJobState;
}

function ensureBoundary(value: unknown): AutonomousBrainJobBoundary {
  if (!JOB_BOUNDARIES.includes(value as AutonomousBrainJobBoundary)) throw new ArgumentError("job side_effect_boundary is invalid");
  return value as AutonomousBrainJobBoundary;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort();
  const allowed = [...expected].sort();
  if (actual.length !== allowed.length || actual.some((key, index) => key !== allowed[index])) throw new ArgumentError(`${label} contains unsupported or missing fields`);
}

function normalizedJob(value: unknown): AutonomousBrainJob {
  if (!isObject(value)) throw new ArgumentError("job snapshot row must be an object");
  exactKeys(value, ["schema", "job_id", "idempotency_key_digest", "spec_digest", "domain", "capability", "risk_class", "priority", "max_attempts", "state", "attempts", "lease_owner", "lease_expires_at", "checkpoint_digest", "reconciliation_digest", "result_digest", "side_effect_boundary", "recovered_after_restart", "reason_digest", "created_at", "updated_at", "job_digest", "retention", "secret_material"], "job snapshot row");
  if (value.schema !== AUTONOMOUS_BRAIN_JOB_SCHEMA || value.retention !== "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("job snapshot retention markers are invalid");
  const job = { ...value } as AutonomousBrainJob;
  identifier("job_id", job.job_id);
  digest("idempotency_key_digest", job.idempotency_key_digest);
  digest("spec_digest", job.spec_digest);
  ensureDomain(job.domain);
  identifier("job capability", job.capability);
  identifier("job risk_class", job.risk_class);
  boundedInteger("job priority", job.priority, 0, MAX_AUTONOMOUS_BRAIN_JOB_PRIORITY);
  boundedInteger("job max_attempts", job.max_attempts, 1, MAX_AUTONOMOUS_BRAIN_JOB_ATTEMPTS);
  ensureState(job.state);
  boundedInteger("job attempts", job.attempts, 0, MAX_AUTONOMOUS_BRAIN_JOB_ATTEMPTS);
  if (job.attempts > job.max_attempts) throw new ArgumentError("job attempts exceed max_attempts");
  if (job.lease_owner !== null) identifier("job lease_owner", job.lease_owner);
  if (job.lease_expires_at !== null) nowMs(() => 0, job.lease_expires_at);
  digest("job checkpoint_digest", job.checkpoint_digest, true);
  digest("job reconciliation_digest", job.reconciliation_digest, true);
  digest("job result_digest", job.result_digest, true);
  ensureBoundary(job.side_effect_boundary);
  if (typeof job.recovered_after_restart !== "boolean") throw new ArgumentError("job recovered_after_restart must be boolean");
  digest("job reason_digest", job.reason_digest, true);
  nowMs(() => 0, job.created_at);
  nowMs(() => 0, job.updated_at);
  digest("job_digest", job.job_digest);
  if ((job.state === "leased" || job.state === "running") && (job.lease_owner === null || job.lease_expires_at === null)) throw new ArgumentError("active job requires an owner and expiry");
  if (job.state !== "leased" && job.state !== "running" && (job.lease_owner !== null || job.lease_expires_at !== null)) throw new ArgumentError("unleased job cannot retain lease metadata");
  if (job.job_digest !== digestJsonSync(jobPayload(job))) throw new ArgumentError("job digest is invalid");
  return job;
}

function normalizedEvent(value: unknown): AutonomousBrainJobEvent {
  if (!isObject(value)) throw new ArgumentError("job snapshot event must be an object");
  exactKeys(value, ["schema", "sequence", "event_type", "job_id", "metadata", "previous_digest", "event_digest", "created_at", "retention", "secret_material"], "job snapshot event");
  if (value.schema !== AUTONOMOUS_BRAIN_JOB_EVENT_SCHEMA || value.retention !== "metadata_only_hash_chained" || value.secret_material !== "never_returned") throw new ArgumentError("job event retention markers are invalid");
  const event = { ...value } as AutonomousBrainJobEvent;
  boundedInteger("job event sequence", event.sequence, 1, MAX_AUTONOMOUS_BRAIN_JOB_EVENTS);
  identifier("job event type", event.event_type);
  identifier("job event job_id", event.job_id);
  if (!isObject(event.metadata)) throw new ArgumentError("job event metadata must be an object");
  if (event.previous_digest !== "") digest("job event previous_digest", event.previous_digest);
  digest("job event_digest", event.event_digest);
  nowMs(() => 0, event.created_at);
  if (event.event_digest !== digestJsonSync(eventPayload(event))) throw new ArgumentError("job event digest is invalid");
  return event;
}

/** A bounded, deterministic, restartable local scheduler for autonomous brain work. */
export class InMemoryAutonomousBrainJobScheduler {
  readonly maxJobs: number;
  private readonly clock: () => number;
  private readonly jobs = new Map<string, AutonomousBrainJob>();
  private readonly idempotency = new Map<string, string>();
  private readonly events: AutonomousBrainJobEvent[] = [];

  constructor(options: AutonomousBrainJobSchedulerOptions = {}) {
    this.maxJobs = boundedInteger("brain job maxJobs", options.maxJobs ?? MAX_AUTONOMOUS_BRAIN_JOBS, 1, MAX_AUTONOMOUS_BRAIN_JOBS);
    if (options.clock !== undefined && typeof options.clock !== "function") throw new ArgumentError("brain job clock must be callable");
    this.clock = options.clock ?? (() => Date.now());
  }

  submit(input: AutonomousBrainJobSubmission, now?: number): AutonomousBrainJobSubmissionResult {
    if (!isObject(input)) throw new ArgumentError("brain job submission must be an object");
    const idempotencyKey = boundedText("idempotencyKey", input.idempotencyKey);
    const idempotencyKeyDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_JOB_SCHEMA, idempotency_key: idempotencyKey });
    const specDigest = digest("specDigest", input.specDigest)!;
    const domain = ensureDomain(input.domain);
    const capability = identifier("capability", input.capability);
    const riskClass = identifier("riskClass", input.riskClass);
    const priority = boundedInteger("priority", input.priority ?? 0, 0, MAX_AUTONOMOUS_BRAIN_JOB_PRIORITY);
    const maxAttempts = boundedInteger("maxAttempts", input.maxAttempts ?? 3, 1, MAX_AUTONOMOUS_BRAIN_JOB_ATTEMPTS);
    const checkpointDigest = digest("checkpointDigest", input.checkpointDigest ?? null, true);
    const current = nowMs(this.clock, now);
    const existingId = this.idempotency.get(idempotencyKeyDigest);
    if (existingId !== undefined) {
      const existing = this.jobs.get(existingId);
      if (!existing || existing.spec_digest !== specDigest) throw new ArgumentError("idempotency key is bound to a different specDigest");
      const event = this.events.find((candidate) => candidate.job_id === existing.job_id && candidate.event_type === "job_submitted");
      if (!event) throw new ArgumentError("idempotent job is missing its submission event");
      return this.submissionResult(existing, event, false, true);
    }
    if (this.jobs.size >= this.maxJobs) throw new ArgumentError("brain job scheduler is full");
    const jobId = identifier("jobId", input.jobId ?? `brain-job-${current}-${++generatedJobSequence}`);
    if (this.jobs.has(jobId)) throw new ArgumentError("jobId already exists");
    const job = this.refresh({
      schema: AUTONOMOUS_BRAIN_JOB_SCHEMA,
      job_id: jobId,
      idempotency_key_digest: idempotencyKeyDigest,
      spec_digest: specDigest,
      domain,
      capability,
      risk_class: riskClass,
      priority,
      max_attempts: maxAttempts,
      state: "queued",
      attempts: 0,
      lease_owner: null,
      lease_expires_at: null,
      checkpoint_digest: checkpointDigest,
      reconciliation_digest: null,
      result_digest: null,
      side_effect_boundary: "not_started",
      recovered_after_restart: false,
      reason_digest: null,
      created_at: current,
      updated_at: current,
      job_digest: "",
      retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained",
      secret_material: "never_returned",
    }, current);
    this.jobs.set(job.job_id, job);
    this.idempotency.set(job.idempotency_key_digest, job.job_id);
    const event = this.append("job_submitted", job, current, { state: job.state, priority: job.priority });
    return this.submissionResult(job, event, true, false);
  }

  get(jobId: string): AutonomousBrainJob | null {
    return this.jobs.get(identifier("jobId", jobId)) ?? null;
  }

  /** Return the hash-chained lifecycle for one job without exposing private task or provider values. */
  eventsFor(jobId: string): readonly AutonomousBrainJobEvent[] {
    const id = identifier("jobId", jobId);
    if (!this.jobs.has(id)) throw new ArgumentError("unknown brain job");
    return this.events.filter((event) => event.job_id === id).map((event) => ({ ...event, metadata: { ...event.metadata } }));
  }

  inventory(options: { limit?: number; state?: AutonomousBrainJobState; now?: number } = {}): readonly AutonomousBrainJob[] {
    const limit = boundedInteger("job inventory limit", options.limit ?? 100, 1, Math.min(this.maxJobs, MAX_AUTONOMOUS_BRAIN_JOBS));
    const current = nowMs(this.clock, options.now);
    this.recoverExpired(current);
    if (options.state !== undefined) ensureState(options.state);
    return [...this.jobs.values()]
      .filter((job) => options.state === undefined || job.state === options.state)
      .sort((left, right) => this.compare(left, right, current))
      .slice(0, limit);
  }

  pending(limit = 100, now?: number): readonly AutonomousBrainJob[] {
    return this.inventory({ limit, state: "queued", now });
  }

  claim(jobId: string, workerId: string, leaseMs = 60_000, now?: number): AutonomousBrainJob {
    const id = identifier("jobId", jobId);
    const owner = identifier("workerId", workerId);
    const lease = boundedInteger("leaseMs", leaseMs, 1, MAX_AUTONOMOUS_BRAIN_JOB_LEASE_MS);
    const current = nowMs(this.clock, now);
    this.recoverExpired(current);
    const job = this.require(id);
    if (TERMINAL_STATES.has(job.state)) return job;
    if (job.state === "waiting_approval") throw new ArgumentError("brain job is waiting for approval");
    if (job.state === "reconciliation_required") throw new ArgumentError("brain job requires reconciliation");
    if (job.state !== "queued") {
      if ((job.state === "leased" || job.state === "running") && job.lease_owner === owner) return job;
      throw new ArgumentError("brain job is already leased by another worker");
    }
    if (job.attempts >= job.max_attempts) return this.transition(job, "job_dead_lettered", { state: "dead_lettered", reason_digest: digestJsonSync("maximum attempts exhausted") }, current);
    return this.transition(job, "job_claimed", { state: "leased", attempts: job.attempts + 1, lease_owner: owner, lease_expires_at: current + lease }, current);
  }

  /** Atomically chooses the highest effective priority runnable job for one worker. */
  claimNext(workerId: string, leaseMs = 60_000, now?: number): AutonomousBrainJob | null {
    const current = nowMs(this.clock, now);
    this.recoverExpired(current);
    const next = [...this.jobs.values()]
      .filter((job) => job.state === "queued")
      .sort((left, right) => this.compare(left, right, current))[0];
    return next ? this.claim(next.job_id, workerId, leaseMs, current) : null;
  }

  renew(jobId: string, workerId: string, leaseMs = 60_000, now?: number): AutonomousBrainJob {
    const current = nowMs(this.clock, now);
    const job = this.requireOwned(jobId, workerId, current);
    const lease = boundedInteger("leaseMs", leaseMs, 1, MAX_AUTONOMOUS_BRAIN_JOB_LEASE_MS);
    return this.transition(job, "job_lease_renewed", { lease_expires_at: current + lease }, current);
  }

  checkpoint(jobId: string, workerId: string, options: AutonomousBrainJobCheckpointOptions): AutonomousBrainJob {
    const current = nowMs(this.clock, options.now);
    const job = this.requireOwned(jobId, workerId, current);
    const phase = boundedText("checkpoint phase", options.phase);
    const checkpointDigest = digest("checkpointDigest", options.checkpointDigest ?? null, true);
    const boundary = ensureBoundary(options.sideEffectBoundary ?? job.side_effect_boundary);
    if (BOUNDARY_ORDER[boundary] < BOUNDARY_ORDER[job.side_effect_boundary]) throw new ArgumentError("job side_effect_boundary cannot move backwards");
    if (options.waitingForApproval !== undefined && typeof options.waitingForApproval !== "boolean") throw new ArgumentError("waitingForApproval must be boolean");
    const nextState: AutonomousBrainJobState = options.waitingForApproval ? "waiting_approval" : "running";
    return this.transition(job, options.waitingForApproval ? "job_waiting_approval" : "job_checkpointed", { state: nextState, checkpoint_digest: checkpointDigest, side_effect_boundary: boundary, reason_digest: digestJsonSync(phase), lease_owner: options.waitingForApproval ? null : job.lease_owner, lease_expires_at: options.waitingForApproval ? null : job.lease_expires_at }, current);
  }

  resumeApproval(jobId: string, approver: string, reason = "caller approval granted", now?: number): AutonomousBrainJob {
    const current = nowMs(this.clock, now);
    const job = this.require(jobId);
    if (job.state !== "waiting_approval") throw new ArgumentError("brain job is not waiting for approval");
    identifier("approver", approver);
    const text = boundedText("approval reason", reason, 2_048);
    return this.transition(job, "job_approval_released", { state: "queued", reason_digest: digestJsonSync(text) }, current);
  }

  release(jobId: string, workerId: string, reason = "checkpoint persisted; worker released lease", now?: number): AutonomousBrainJob {
    const current = nowMs(this.clock, now);
    const job = this.requireOwned(jobId, workerId, current);
    if (job.side_effect_boundary === "dispatched" || job.side_effect_boundary === "unknown") throw new ArgumentError("job cannot be released after external dispatch");
    const text = boundedText("release reason", reason, 2_048);
    return this.transition(job, "job_released", { state: "queued", lease_owner: null, lease_expires_at: null, reason_digest: digestJsonSync(text) }, current);
  }

  complete(jobId: string, workerId: string, resultDigest: string | null = null, now?: number): AutonomousBrainJob {
    const current = nowMs(this.clock, now);
    const job = this.requireOwned(jobId, workerId, current);
    const result = digest("resultDigest", resultDigest, true);
    return this.transition(job, "job_completed", { state: "succeeded", result_digest: result, lease_owner: null, lease_expires_at: null, reason_digest: null }, current);
  }

  fail(jobId: string, workerId: string, options: AutonomousBrainJobFailureOptions): AutonomousBrainJob {
    const current = nowMs(this.clock, options.now);
    const job = this.requireOwned(jobId, workerId, current);
    const reason = boundedText("failure reason", options.reason, 2_048);
    if (options.retryable !== undefined && typeof options.retryable !== "boolean") throw new ArgumentError("retryable must be boolean");
    const retryable = options.retryable ?? false;
    const external = job.side_effect_boundary === "dispatched" || job.side_effect_boundary === "unknown";
    const state: AutonomousBrainJobState = external ? "reconciliation_required" : retryable && job.attempts < job.max_attempts ? "queued" : job.attempts >= job.max_attempts ? "dead_lettered" : "failed";
    return this.transition(job, external ? "job_reconciliation_required" : state === "queued" ? "job_retry_queued" : state === "dead_lettered" ? "job_dead_lettered" : "job_failed", { state, lease_owner: null, lease_expires_at: null, reason_digest: digestJsonSync(reason) }, current);
  }

  cancel(jobId: string, reason = "cancelled by caller", now?: number): AutonomousBrainJob {
    const current = nowMs(this.clock, now);
    const job = this.require(jobId);
    if (job.state === "succeeded" || job.state === "failed" || job.state === "dead_lettered" || job.state === "cancelled") return job;
    const text = boundedText("cancellation reason", reason, 2_048);
    return this.transition(job, "job_cancelled", { state: "cancelled", lease_owner: null, lease_expires_at: null, reason_digest: digestJsonSync(text) }, current);
  }

  reconcile(jobId: string, options: AutonomousBrainJobReconciliationOptions): AutonomousBrainJob {
    const current = nowMs(this.clock, options.now);
    const job = this.require(jobId);
    if (job.state !== "reconciliation_required") throw new ArgumentError("brain job is not awaiting reconciliation");
    if (!(options.outcome === "succeeded" || options.outcome === "failed" || options.outcome === "not_executed" || options.outcome === "unknown")) throw new ArgumentError("reconciliation outcome is invalid");
    const evidenceDigest = digest("evidenceDigest", options.evidenceDigest)!;
    const evidenceKind = identifier("evidenceKind", options.evidenceKind ?? "caller_observation");
    const operator = identifier("operator", options.operator ?? "caller");
    const reason = boundedText("reconciliation reason", options.reason ?? "caller reconciled uncertain external state", 2_048);
    const reconciliationDigest = digestJsonSync({ schema: AUTONOMOUS_BRAIN_JOB_SCHEMA, outcome: options.outcome, evidence_digest: evidenceDigest, evidence_kind: evidenceKind, operator, reason_digest: digestJsonSync(reason) });
    if (job.reconciliation_digest === reconciliationDigest) return job;
    const state: AutonomousBrainJobState = options.outcome === "succeeded" ? "succeeded" : options.outcome === "failed" ? "failed" : options.outcome === "not_executed" ? "queued" : "reconciliation_required";
    if (options.outcome === "not_executed" && job.attempts >= job.max_attempts) throw new ArgumentError("reconciliation retry is unavailable after maximum attempts");
    return this.transition(job, `job_reconciled_${options.outcome}`, { state, reconciliation_digest: reconciliationDigest, lease_owner: null, lease_expires_at: null, side_effect_boundary: options.outcome === "not_executed" ? "not_started" : job.side_effect_boundary, reason_digest: digestJsonSync(reason) }, current);
  }

  snapshot(): AutonomousBrainJobSnapshot {
    this.verifyIntegrity();
    const descriptor = { schema: AUTONOMOUS_BRAIN_JOB_SNAPSHOT_SCHEMA, jobs: [...this.jobs.values()].sort((left, right) => left.job_id.localeCompare(right.job_id)), events: [...this.events], retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousBrainJobSnapshot;
    if (bytes(JSON.stringify(snapshot)) > MAX_AUTONOMOUS_BRAIN_JOB_SNAPSHOT_BYTES) throw new ArgumentError("brain job snapshot exceeds its bound");
    return snapshot;
  }

  restore(snapshot: AutonomousBrainJobSnapshot): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_BRAIN_JOB_SNAPSHOT_SCHEMA || snapshot.retention !== "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained" || snapshot.secret_material !== "never_returned" || !Array.isArray(snapshot.jobs) || !Array.isArray(snapshot.events)) throw new ArgumentError("brain job snapshot is malformed");
    const descriptor = { schema: snapshot.schema, jobs: snapshot.jobs, events: snapshot.events, retention: snapshot.retention, secret_material: snapshot.secret_material };
    if (digest("snapshot_digest", snapshot.snapshot_digest) !== digestJsonSync(descriptor)) throw new ArgumentError("brain job snapshot digest is invalid");
    if (snapshot.jobs.length > this.maxJobs || snapshot.events.length > MAX_AUTONOMOUS_BRAIN_JOB_EVENTS) throw new ArgumentError("brain job snapshot exceeds its bound");
    const jobs = snapshot.jobs.map(normalizedJob);
    const events = snapshot.events.map(normalizedEvent).sort((left, right) => left.sequence - right.sequence);
    let previous = "";
    for (const event of events) {
      if (event.sequence !== events.indexOf(event) + 1 || event.previous_digest !== previous || !jobs.some((job) => job.job_id === event.job_id)) throw new ArgumentError("brain job snapshot event chain is invalid");
      previous = event.event_digest;
    }
    const ids = new Set<string>();
    const idempotency = new Map<string, string>();
    for (const job of jobs) {
      if (ids.has(job.job_id) || idempotency.has(job.idempotency_key_digest)) throw new ArgumentError("brain job snapshot contains duplicate identity");
      ids.add(job.job_id);
      idempotency.set(job.idempotency_key_digest, job.job_id);
    }
    this.jobs.clear();
    this.idempotency.clear();
    this.events.splice(0, this.events.length);
    for (const job of jobs) {
      const recovered = job.state === "leased" || job.state === "running" ? true : job.recovered_after_restart;
      const restored = recovered === job.recovered_after_restart ? job : this.refresh({ ...job, recovered_after_restart: recovered }, job.updated_at);
      this.jobs.set(restored.job_id, restored);
    }
    for (const [key, jobId] of idempotency) this.idempotency.set(key, jobId);
    this.events.push(...events);
    this.recoverExpired(this.clock());
  }

  verifyIntegrity(): { schema: typeof AUTONOMOUS_BRAIN_JOB_SCHEMA; verified: true; jobs: number; events: number; head_digest: string } {
    let previous = "";
    for (let index = 0; index < this.events.length; index += 1) {
      const event = normalizedEvent(this.events[index]);
      if (event.sequence !== index + 1 || event.previous_digest !== previous) throw new ArgumentError("brain job event chain is invalid");
      previous = event.event_digest;
    }
    for (const job of this.jobs.values()) normalizedJob(job);
    return { schema: AUTONOMOUS_BRAIN_JOB_SCHEMA, verified: true, jobs: this.jobs.size, events: this.events.length, head_digest: previous };
  }

  private require(jobId: string): AutonomousBrainJob {
    const job = this.jobs.get(identifier("jobId", jobId));
    if (!job) throw new ArgumentError("unknown brain job");
    return job;
  }

  private requireOwned(jobId: string, workerId: string, now: number): AutonomousBrainJob {
    const job = this.require(jobId);
    const owner = identifier("workerId", workerId);
    if ((job.state !== "leased" && job.state !== "running") || job.lease_owner !== owner || job.lease_expires_at === null || job.lease_expires_at <= now) throw new ArgumentError("worker does not own an active brain job lease");
    return job;
  }

  private refresh(job: AutonomousBrainJob, now: number): AutonomousBrainJob {
    const candidate = { ...job, updated_at: now, job_digest: "" } as AutonomousBrainJob;
    return { ...candidate, job_digest: digestJsonSync(jobPayload(candidate)) };
  }

  private transition(job: AutonomousBrainJob, eventType: string, changes: Partial<AutonomousBrainJob>, now: number): AutonomousBrainJob {
    const next = this.refresh({ ...job, ...changes }, now);
    this.jobs.set(job.job_id, next);
    this.append(eventType, next, now, { state: next.state, attempts: next.attempts, reason_digest: next.reason_digest, checkpoint_digest: next.checkpoint_digest, side_effect_boundary: next.side_effect_boundary });
    return next;
  }

  private append(eventType: string, job: AutonomousBrainJob, current: number, metadata: JsonObject): AutonomousBrainJobEvent {
    if (this.events.length >= MAX_AUTONOMOUS_BRAIN_JOB_EVENTS) throw new ArgumentError("brain job event capacity is exhausted");
    const eventBase = { schema: AUTONOMOUS_BRAIN_JOB_EVENT_SCHEMA, sequence: this.events.length + 1, event_type: identifier("event_type", eventType), job_id: job.job_id, metadata, previous_digest: this.events[this.events.length - 1]?.event_digest ?? "", created_at: current, retention: "metadata_only_hash_chained" as const, secret_material: "never_returned" as const };
    const event = { ...eventBase, event_digest: digestJsonSync(eventBase) } satisfies AutonomousBrainJobEvent;
    this.events.push(event);
    return event;
  }

  private submissionResult(job: AutonomousBrainJob, event: AutonomousBrainJobEvent, created: boolean, idempotent: boolean): AutonomousBrainJobSubmissionResult {
    return { schema: AUTONOMOUS_BRAIN_JOB_SCHEMA, created, idempotent, job, event, retention: "metadata_only;task_prompt_credentials_and_provider_payloads_not_retained", secret_material: "never_returned" };
  }

  private compare(left: AutonomousBrainJob, right: AutonomousBrainJob, current: number): number {
    const effective = (job: AutonomousBrainJob): number => job.priority + Math.min(AUTONOMOUS_BRAIN_JOB_MAX_AGING_BONUS, Math.floor(Math.max(0, current - job.created_at) / AUTONOMOUS_BRAIN_JOB_AGING_INTERVAL_MS));
    return effective(right) - effective(left) || right.priority - left.priority || left.created_at - right.created_at || left.job_id.localeCompare(right.job_id);
  }

  private recoverExpired(current: number): void {
    for (const job of [...this.jobs.values()]) {
      if ((job.state !== "leased" && job.state !== "running") || job.lease_expires_at === null || job.lease_expires_at > current) continue;
      if (job.side_effect_boundary === "not_started" || job.side_effect_boundary === "preflight") {
        this.transition(job, "job_lease_expired_requeued", { state: "queued", lease_owner: null, lease_expires_at: null, recovered_after_restart: true, reason_digest: digestJsonSync("lease expired before external dispatch") }, current);
      } else {
        this.transition(job, "job_lease_expired_quarantined", { state: "reconciliation_required", lease_owner: null, lease_expires_at: null, recovered_after_restart: true, reason_digest: digestJsonSync("lease expired after external dispatch") }, current);
      }
    }
  }
}

/** Caller-owned persistence lifecycle around the in-memory scheduler. */
export class AutonomousBrainJobSchedulerPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly scheduler: InMemoryAutonomousBrainJobScheduler, readonly persistence: AutonomousBrainJobSchedulerPersistence) {
    if (!(scheduler instanceof InMemoryAutonomousBrainJobScheduler)) throw new ArgumentError("brain job persistence requires a typed scheduler");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("brain job persistence is malformed");
  }

  async restore(): Promise<AutonomousBrainJobSnapshot | null> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot !== null) this.scheduler.restore(snapshot);
      this.expectedSnapshotDigest = snapshot?.snapshot_digest ?? null;
      return snapshot;
    });
  }

  async flush(): Promise<AutonomousBrainJobSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.scheduler.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        const committed = await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot);
        if (!committed) throw new ArgumentError("brain job persistence compare-and-swap conflict; reload the scheduler before continuing");
      } else {
        await this.persistence.write(snapshot);
      }
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class InMemoryAutonomousBrainJobSchedulerPersistence implements AutonomousBrainJobSchedulerPersistence {
  private snapshotValue: AutonomousBrainJobSnapshot | null = null;

  read(): AutonomousBrainJobSnapshot | null {
    return this.snapshotValue === null ? null : JSON.parse(JSON.stringify(this.snapshotValue)) as AutonomousBrainJobSnapshot;
  }

  write(snapshot: AutonomousBrainJobSnapshot): void {
    this.snapshotValue = JSON.parse(JSON.stringify(snapshot)) as AutonomousBrainJobSnapshot;
  }

  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousBrainJobSnapshot): boolean {
    const currentSnapshotDigest = this.snapshotValue?.snapshot_digest ?? null;
    if (currentSnapshotDigest !== expectedSnapshotDigest) return false;
    this.snapshotValue = JSON.parse(JSON.stringify(snapshot)) as AutonomousBrainJobSnapshot;
    return true;
  }
}
