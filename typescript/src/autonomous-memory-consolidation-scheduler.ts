/**
 * Durable evaluator-observation scheduling for autonomous memory consolidation.
 *
 * The queue is intentionally provider-free. It accepts only explicit evaluator observations,
 * leases work to bounded workers, retries failures without persisting raw errors, and stores
 * canonical metadata snapshots with no prompt text, provider output, credentials, or tool args.
 */

import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousMemoryConsolidationError,
  AutonomousMemoryConsolidationObservation,
  AutonomousMemoryConsolidator,
} from "./autonomous-memory-consolidation.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";

export const AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation-scheduler/0.1" as const;
export const AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation-scheduler-job/0.1" as const;
export const AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation-scheduler-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS = 4_096;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB = 1_024;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES = 8_000_000;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS = 86_400;

const DOMAINS = [...AUTONOMOUS_DOMAIN_NAMES] as AutonomousDomainName[];
const ID = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$/;
const RETENTION = "metadata_only_evaluator_observations_no_text_payloads_or_provider_values" as const;
const SECRET_MATERIAL = "never_returned" as const;
const STATUSES = ["queued", "leased", "completed", "quarantined"] as const;
type SchedulerStatus = typeof STATUSES[number];

export class AutonomousMemoryConsolidationSchedulerError extends ArgumentError {}

function fail(message: string): never {
  throw new AutonomousMemoryConsolidationSchedulerError(`memory consolidation scheduler ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || !ID.test(value) || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is not a bounded identifier`);
  return value;
}

function digest(name: string, value: unknown, optional = false): string | null {
  if (optional && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function numberBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function integerBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}

function domain(value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !DOMAINS.includes(value as AutonomousDomainName)) fail("domain is not a supported built-in autonomous domain");
  return value as AutonomousDomainName;
}

function orderedDomains(values: unknown): AutonomousDomainName[] {
  if (!Array.isArray(values) || values.some((value) => typeof value !== "string") || new Set(values).size !== values.length) fail("job domains are malformed");
  const normalized = values.map(domain);
  return DOMAINS.filter((item) => normalized.includes(item));
}

function normalizeObservation(value: AutonomousMemoryConsolidationObservation): AutonomousMemoryConsolidationObservation {
  if (!isObject(value)) fail("observation must be a value object");
  const normalized: AutonomousMemoryConsolidationObservation = {
    episode_id: identifier("observation episode_id", value.episode_id), lesson_id: identifier("observation lesson_id", value.lesson_id),
    concept_id: identifier("observation concept_id", value.concept_id), variant_id: identifier("observation variant_id", value.variant_id),
    domain: domain(value.domain), capability: identifier("observation capability", value.capability), risk_class: identifier("observation risk_class", value.risk_class),
    evaluator_id: identifier("observation evaluator_id", value.evaluator_id), evaluator_version: identifier("observation evaluator_version", value.evaluator_version),
    reward: numberBound("observation reward", value.reward, -1, 1), passed: typeof value.passed === "boolean" ? value.passed : fail("observation passed must be boolean"),
    evidence_digest: digest("observation evidence_digest", value.evidence_digest)!, lesson_digest: digest("observation lesson_digest", value.lesson_digest)!,
    decision_digest: digest("observation decision_digest", value.decision_digest ?? null, true), observed_at: numberBound("observation observed_at", value.observed_at ?? 0, 0, 9_223_372_036_854_775),
    transferable: typeof value.transferable === "boolean" ? value.transferable : fail("observation transferable must be boolean"),
  };
  if (Object.keys(value).some((key) => !Object.keys(normalized).includes(key))) fail("observation contains unsupported fields");
  return normalized;
}

function observationProjection(value: AutonomousMemoryConsolidationObservation): AutonomousMemoryConsolidationObservation {
  return { ...value };
}

function leaseDigest(jobDigest: string, jobId: string, workerId: string, attempt: number, expiresAt: number): string {
  return digestJsonSync({ schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SCHEMA, job_digest: jobDigest, job_id: jobId, worker_id: workerId, attempt, lease_expires_at: expiresAt });
}

export interface AutonomousMemoryConsolidationScheduledJob {
  schema: typeof AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA;
  job_id: string;
  observations: AutonomousMemoryConsolidationObservation[];
  observation_count: number;
  domains: AutonomousDomainName[];
  priority: number;
  submitted_at: number;
  attempts: number;
  max_attempts: number;
  status: SchedulerStatus;
  lease_owner: string | null;
  lease_expires_at: number | null;
  lease_digest: string | null;
  report_digest: string | null;
  last_error_class: string | null;
  job_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousMemoryConsolidationClaim {
  job_id: string;
  job_digest: string;
  worker_id: string;
  attempt: number;
  lease_expires_at: number;
  lease_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousMemoryConsolidationSchedulerSnapshot {
  schema: typeof AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA;
  generation: number;
  previous_snapshot_digest: string | null;
  policy: { max_jobs: number; max_observations_per_job: number; default_max_attempts: number; lease_seconds: number };
  jobs: AutonomousMemoryConsolidationScheduledJob[];
  coverage: AutonomousMemoryConsolidationSchedulerCoverage[];
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  snapshot_digest: string;
}

export interface AutonomousMemoryConsolidationSchedulerCoverage {
  domain: AutonomousDomainName;
  job_count: number;
  observation_count: number;
  queued_job_count: number;
  leased_job_count: number;
  completed_job_count: number;
  quarantined_job_count: number;
}

export interface AutonomousMemoryConsolidationSchedulerTextStore {
  read(): string | null;
  write(value: string): void;
}

export interface AutonomousMemoryConsolidationSchedulerTransactionalTextStore extends AutonomousMemoryConsolidationSchedulerTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean;
}

function immutableProjection(job: Pick<AutonomousMemoryConsolidationScheduledJob, "job_id" | "observations" | "domains" | "priority" | "submitted_at" | "max_attempts">): Record<string, unknown> {
  return { schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA, job_id: job.job_id, observations: job.observations.map(observationProjection), domains: [...job.domains], priority: job.priority, submitted_at: job.submitted_at, max_attempts: job.max_attempts };
}

function publicJob(job: AutonomousMemoryConsolidationScheduledJob): Omit<AutonomousMemoryConsolidationScheduledJob, "observations"> & { observation_count: number } {
  const { observations: _observations, ...rest } = job;
  return { ...rest, observation_count: job.observation_count };
}

function coverageFor(jobs: readonly AutonomousMemoryConsolidationScheduledJob[]): AutonomousMemoryConsolidationSchedulerCoverage[] {
  return DOMAINS.map((item) => {
    const selected = jobs.filter((job) => job.domains.includes(item));
    return {
      domain: item, job_count: selected.length,
      observation_count: selected.reduce((sum, job) => sum + job.observations.filter((observation) => observation.domain === item).length, 0),
      queued_job_count: selected.filter((job) => job.status === "queued").length,
      leased_job_count: selected.filter((job) => job.status === "leased").length,
      completed_job_count: selected.filter((job) => job.status === "completed").length,
      quarantined_job_count: selected.filter((job) => job.status === "quarantined").length,
    };
  });
}

function validateJob(value: unknown, maxObservations: number): AutonomousMemoryConsolidationScheduledJob {
  const expectedJobKeys = ["attempts", "domains", "job_digest", "job_id", "last_error_class", "lease_digest", "lease_expires_at", "lease_owner", "max_attempts", "observation_count", "observations", "priority", "report_digest", "retention", "schema", "secret_material", "status", "submitted_at"];
  if (!isObject(value) || Object.keys(value).sort().join(",") !== expectedJobKeys.join(",") || value.schema !== AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("snapshot job is malformed");
  const observations = Array.isArray(value.observations) ? value.observations.map((item) => normalizeObservation(item as AutonomousMemoryConsolidationObservation)) : fail("snapshot job observations are malformed");
  if (observations.length === 0 || observations.length > maxObservations || value.observation_count !== observations.length) fail("snapshot job observation_count is malformed");
  const domains = orderedDomains(value.domains);
  if (domains.length === 0 || observations.some((observation) => !domains.includes(observation.domain))) fail("snapshot job domains do not cover observations");
  const job: AutonomousMemoryConsolidationScheduledJob = {
    schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA, job_id: identifier("snapshot job_id", value.job_id), observations, observation_count: observations.length,
    domains, priority: numberBound("snapshot job priority", value.priority, 0, 1), submitted_at: numberBound("snapshot job submitted_at", value.submitted_at, 0, 9_223_372_036_854_775),
    attempts: integerBound("snapshot job attempts", value.attempts, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS), max_attempts: integerBound("snapshot job max_attempts", value.max_attempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS),
    status: STATUSES.includes(value.status as SchedulerStatus) ? value.status as SchedulerStatus : fail("snapshot job status is unsupported"),
    lease_owner: value.lease_owner === null ? null : identifier("snapshot job lease_owner", value.lease_owner), lease_expires_at: value.lease_expires_at === null ? null : numberBound("snapshot job lease_expires_at", value.lease_expires_at, 0, 9_223_372_036_854_775),
    lease_digest: digest("snapshot job lease_digest", value.lease_digest, true), report_digest: digest("snapshot job report_digest", value.report_digest, true), last_error_class: value.last_error_class === null ? null : identifier("snapshot job last_error_class", value.last_error_class),
    job_digest: digest("snapshot job job_digest", value.job_digest)!, retention: RETENTION, secret_material: SECRET_MATERIAL,
  };
  if (job.attempts > job.max_attempts) fail("snapshot job attempts exceed max_attempts");
  if (job.status === "leased" && (job.lease_owner === null || job.lease_expires_at === null || job.lease_digest === null || job.report_digest !== null)) fail("snapshot leased job state is malformed");
  if (job.status === "queued" && (job.lease_owner !== null || job.lease_expires_at !== null || job.lease_digest !== null || job.report_digest !== null)) fail("snapshot queued job state is malformed");
  if (job.status === "completed" && (job.lease_owner !== null || job.lease_expires_at !== null || job.lease_digest !== null || job.report_digest === null)) fail("snapshot completed job state is malformed");
  if (job.status === "quarantined" && (job.lease_owner !== null || job.lease_expires_at !== null || job.lease_digest !== null || job.report_digest !== null)) fail("snapshot quarantined job state is malformed");
  if (digestJsonSync(immutableProjection(job)) !== job.job_digest) fail("snapshot job digest does not match its immutable projection");
  return job;
}

export function validateAutonomousMemoryConsolidationSchedulerSnapshot(value: unknown): AutonomousMemoryConsolidationSchedulerSnapshot {
  const expectedSnapshotKeys = ["coverage", "generation", "jobs", "policy", "previous_snapshot_digest", "retention", "schema", "secret_material", "snapshot_digest"];
  if (!isObject(value) || Object.keys(value).sort().join(",") !== expectedSnapshotKeys.join(",") || value.schema !== AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("snapshot is malformed");
  const generation = integerBound("snapshot generation", value.generation, 1, 2_147_483_647);
  const previous = digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest ?? null, true);
  if (!isObject(value.policy) || Object.keys(value.policy).sort().join(",") !== "default_max_attempts,lease_seconds,max_jobs,max_observations_per_job") fail("snapshot policy is malformed");
  const policy = {
    max_jobs: integerBound("snapshot policy max_jobs", value.policy.max_jobs, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS),
    max_observations_per_job: integerBound("snapshot policy max_observations_per_job", value.policy.max_observations_per_job, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB),
    default_max_attempts: integerBound("snapshot policy default_max_attempts", value.policy.default_max_attempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS),
    lease_seconds: numberBound("snapshot policy lease_seconds", value.policy.lease_seconds, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS),
  };
  if (!Array.isArray(value.jobs) || value.jobs.length > policy.max_jobs) fail("snapshot jobs are malformed");
  const jobs = value.jobs.map((item) => validateJob(item, policy.max_observations_per_job));
  if (new Set(jobs.map((job) => job.job_id)).size !== jobs.length) fail("snapshot contains duplicate job identifiers");
  if (!Array.isArray(value.coverage) || canonicalJson(value.coverage) !== canonicalJson(coverageFor(jobs))) fail("snapshot domain coverage does not match jobs");
  const snapshotDigest = digest("snapshot snapshot_digest", value.snapshot_digest)!;
  const descriptor = { schema: value.schema, generation, previous_snapshot_digest: previous, policy, jobs: [...jobs].sort((left, right) => left.job_id.localeCompare(right.job_id)), coverage: value.coverage, retention: RETENTION, secret_material: SECRET_MATERIAL };
  if (digestJsonSync(descriptor) !== snapshotDigest) fail("snapshot digest does not match its canonical projection");
  return { schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA, generation, previous_snapshot_digest: previous, policy, jobs, coverage: value.coverage as AutonomousMemoryConsolidationSchedulerCoverage[], retention: RETENTION, secret_material: SECRET_MATERIAL, snapshot_digest: snapshotDigest };
}

export class AutonomousMemoryConsolidationScheduler {
  readonly consolidator: AutonomousMemoryConsolidator;
  readonly maxJobs: number;
  readonly maxObservationsPerJob: number;
  readonly defaultMaxAttempts: number;
  readonly leaseSeconds: number;
  private readonly jobs = new Map<string, AutonomousMemoryConsolidationScheduledJob>();
  private generation = 0;
  private previousSnapshotDigest: string | null = null;

  constructor(consolidator: AutonomousMemoryConsolidator, options: { maxJobs?: number; maxObservationsPerJob?: number; defaultMaxAttempts?: number; leaseSeconds?: number } = {}) {
    if (!(consolidator instanceof AutonomousMemoryConsolidator)) fail("consolidator is malformed");
    this.consolidator = consolidator;
    this.maxJobs = integerBound("maxJobs", options.maxJobs ?? MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOBS);
    this.maxObservationsPerJob = integerBound("maxObservationsPerJob", options.maxObservationsPerJob ?? MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_OBSERVATIONS_PER_JOB);
    this.defaultMaxAttempts = integerBound("defaultMaxAttempts", options.defaultMaxAttempts ?? 3, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS);
    this.leaseSeconds = numberBound("leaseSeconds", options.leaseSeconds ?? 300, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS);
  }

  get policy(): AutonomousMemoryConsolidationSchedulerSnapshot["policy"] {
    return { max_jobs: this.maxJobs, max_observations_per_job: this.maxObservationsPerJob, default_max_attempts: this.defaultMaxAttempts, lease_seconds: this.leaseSeconds };
  }

  private replace(job: AutonomousMemoryConsolidationScheduledJob, changes: Partial<AutonomousMemoryConsolidationScheduledJob>): AutonomousMemoryConsolidationScheduledJob {
    return { ...job, ...changes };
  }

  submit(jobId: string, observations: readonly AutonomousMemoryConsolidationObservation[], options: { priority?: number; submittedAt?: number; maxAttempts?: number } = {}): Omit<AutonomousMemoryConsolidationScheduledJob, "observations"> & { observation_count: number } {
    const normalizedJobId = identifier("jobId", jobId);
    if (!Array.isArray(observations) || observations.length === 0 || observations.length > this.maxObservationsPerJob) fail("observations exceed their bound");
    const normalized = observations.map(normalizeObservation);
    const domains = DOMAINS.filter((item) => normalized.some((observation) => observation.domain === item));
    const job: AutonomousMemoryConsolidationScheduledJob = {
      schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_JOB_SCHEMA, job_id: normalizedJobId, observations: normalized, observation_count: normalized.length, domains,
      priority: numberBound("priority", options.priority ?? 0.5, 0, 1), submitted_at: numberBound("submittedAt", options.submittedAt ?? Date.now() / 1000, 0, 9_223_372_036_854_775), attempts: 0,
      max_attempts: integerBound("maxAttempts", options.maxAttempts ?? this.defaultMaxAttempts, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_ATTEMPTS), status: "queued", lease_owner: null, lease_expires_at: null, lease_digest: null, report_digest: null, last_error_class: null,
      job_digest: "0".repeat(64), retention: RETENTION, secret_material: SECRET_MATERIAL,
    };
    const immutableDigest = digestJsonSync(immutableProjection(job));
    const withDigest = this.replace(job, { job_digest: immutableDigest });
    const existing = this.jobs.get(normalizedJobId);
    if (existing) {
      if (existing.job_digest !== immutableDigest) fail("job identifier already exists with a different immutable payload");
      return publicJob(existing);
    }
    if (this.jobs.size >= this.maxJobs) fail("job queue is full");
    this.jobs.set(normalizedJobId, withDigest);
    return publicJob(withDigest);
  }

  get(jobId: string): (Omit<AutonomousMemoryConsolidationScheduledJob, "observations"> & { observation_count: number }) | null {
    const job = this.jobs.get(identifier("jobId", jobId));
    return job ? publicJob(job) : null;
  }

  listJobs(limit = 128): Array<Omit<AutonomousMemoryConsolidationScheduledJob, "observations"> & { observation_count: number }> {
    const boundedLimit = integerBound("list limit", limit, 1, this.maxJobs);
    return [...this.jobs.values()].sort((left, right) => Number(left.status !== "queued") - Number(right.status !== "queued") || right.priority - left.priority || left.submitted_at - right.submitted_at || left.job_id.localeCompare(right.job_id)).slice(0, boundedLimit).map(publicJob);
  }

  private reclaimExpired(now: number): void {
    for (const job of this.jobs.values()) {
      if (job.status !== "leased" || job.lease_expires_at === null || job.lease_expires_at > now) continue;
      const status: SchedulerStatus = job.attempts >= job.max_attempts ? "quarantined" : "queued";
      this.jobs.set(job.job_id, this.replace(job, { status, lease_owner: null, lease_expires_at: null, lease_digest: null, last_error_class: "lease_expired" }));
    }
  }

  claimNext(workerId: string, options: { now?: number; leaseSeconds?: number } = {}): AutonomousMemoryConsolidationClaim | null {
    const normalizedWorkerId = identifier("workerId", workerId);
    const now = numberBound("claim now", options.now ?? Date.now() / 1000, 0, 9_223_372_036_854_775);
    const duration = numberBound("claim leaseSeconds", options.leaseSeconds ?? this.leaseSeconds, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_LEASE_SECONDS);
    this.reclaimExpired(now);
    const queued = [...this.jobs.values()].filter((job) => job.status === "queued" && job.attempts < job.max_attempts).sort((left, right) => right.priority - left.priority || -(Math.max(0, now - left.submitted_at) - Math.max(0, now - right.submitted_at)) || left.submitted_at - right.submitted_at || left.job_id.localeCompare(right.job_id));
    const job = queued[0];
    if (!job) return null;
    const attempt = job.attempts + 1;
    const expiresAt = now + duration;
    const lease = leaseDigest(job.job_digest, job.job_id, normalizedWorkerId, attempt, expiresAt);
    this.jobs.set(job.job_id, this.replace(job, { attempts: attempt, status: "leased", lease_owner: normalizedWorkerId, lease_expires_at: expiresAt, lease_digest: lease, last_error_class: null }));
    return { job_id: job.job_id, job_digest: job.job_digest, worker_id: normalizedWorkerId, attempt, lease_expires_at: expiresAt, lease_digest: lease, retention: RETENTION, secret_material: SECRET_MATERIAL };
  }

  private leased(jobId: string, workerId: string, lease: string, now: number): AutonomousMemoryConsolidationScheduledJob {
    const job = this.jobs.get(identifier("jobId", jobId));
    const normalizedWorkerId = identifier("workerId", workerId);
    digest("leaseDigest", lease);
    if (!job || job.status !== "leased" || job.lease_owner !== normalizedWorkerId || job.lease_digest !== lease) fail("lease is invalid or no longer owned by the worker");
    if (job.lease_expires_at === null || job.lease_expires_at <= now) fail("lease has expired");
    return job;
  }

  complete(jobId: string, workerId: string, lease: string, reportDigest: string, options: { now?: number } = {}): ReturnType<AutonomousMemoryConsolidationScheduler["get"]> {
    const now = numberBound("complete now", options.now ?? Date.now() / 1000, 0, 9_223_372_036_854_775);
    const normalizedReport = digest("reportDigest", reportDigest)!;
    const job = this.leased(jobId, workerId, lease, now);
    const completed = this.replace(job, { status: "completed", lease_owner: null, lease_expires_at: null, lease_digest: null, report_digest: normalizedReport, last_error_class: null });
    this.jobs.set(job.job_id, completed);
    return publicJob(completed);
  }

  fail(jobId: string, workerId: string, lease: string, errorClass: string, options: { now?: number } = {}): ReturnType<AutonomousMemoryConsolidationScheduler["get"]> {
    const now = numberBound("fail now", options.now ?? Date.now() / 1000, 0, 9_223_372_036_854_775);
    const normalizedError = identifier("errorClass", errorClass);
    const job = this.leased(jobId, workerId, lease, now);
    const failed = this.replace(job, { status: job.attempts < job.max_attempts ? "queued" : "quarantined", lease_owner: null, lease_expires_at: null, lease_digest: null, last_error_class: normalizedError });
    this.jobs.set(job.job_id, failed);
    return publicJob(failed);
  }

  runNext(workerId: string, options: { now?: number } = {}): Record<string, unknown> | null {
    const now = options.now ?? Date.now() / 1000;
    const claim = this.claimNext(workerId, { now });
    if (!claim) return null;
    const job = this.jobs.get(claim.job_id)!;
    try {
      const report = this.consolidator.consolidate(job.observations);
      this.complete(claim.job_id, claim.worker_id, claim.lease_digest, report.report_digest, { now });
      return { job_id: claim.job_id, status: "completed", attempt: claim.attempt, report_digest: report.report_digest, observation_count: job.observation_count, domains: [...job.domains], retention: RETENTION, secret_material: SECRET_MATERIAL };
    } catch (_error) {
      const row = this.fail(claim.job_id, claim.worker_id, claim.lease_digest, "memory_consolidation_failure", { now });
      return { job_id: claim.job_id, status: row?.status, attempt: claim.attempt, error_class: "memory_consolidation_failure", retention: RETENTION, secret_material: SECRET_MATERIAL };
    }
  }

  runUntilIdle(workerId: string, options: { maxCycles?: number; now?: number } = {}): { worker_id: string; cycles: number; idle: boolean; results: Record<string, unknown>[]; retention: typeof RETENTION; secret_material: typeof SECRET_MATERIAL } {
    const normalizedWorkerId = identifier("workerId", workerId);
    const maxCycles = integerBound("maxCycles", options.maxCycles ?? 64, 1, 1_024);
    const now = numberBound("runUntilIdle now", options.now ?? Date.now() / 1000, 0, 9_223_372_036_854_775);
    const results: Record<string, unknown>[] = [];
    for (let index = 0; index < maxCycles; index += 1) {
      const result = this.runNext(normalizedWorkerId, { now });
      if (!result) break;
      results.push(result);
    }
    return { worker_id: normalizedWorkerId, cycles: results.length, idle: results.length < maxCycles && ![...this.jobs.values()].some((job) => job.status === "queued"), results, retention: RETENTION, secret_material: SECRET_MATERIAL };
  }

  snapshot(): AutonomousMemoryConsolidationSchedulerSnapshot {
    this.generation += 1;
    const descriptor = { schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_SCHEMA, generation: this.generation, previous_snapshot_digest: this.previousSnapshotDigest, policy: this.policy, jobs: [...this.jobs.values()].sort((left, right) => left.job_id.localeCompare(right.job_id)), coverage: coverageFor([...this.jobs.values()]), retention: RETENTION, secret_material: SECRET_MATERIAL };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousMemoryConsolidationSchedulerSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    return JSON.parse(canonicalJson(snapshot)) as AutonomousMemoryConsolidationSchedulerSnapshot;
  }

  restore(snapshot: AutonomousMemoryConsolidationSchedulerSnapshot): AutonomousMemoryConsolidationSchedulerSnapshot {
    const validated = validateAutonomousMemoryConsolidationSchedulerSnapshot(snapshot);
    if (canonicalJson(validated.policy) !== canonicalJson(this.policy)) fail("restored policy conflicts with the configured scheduler");
    this.jobs.clear();
    for (const job of validated.jobs) this.jobs.set(job.job_id, job);
    this.generation = validated.generation;
    this.previousSnapshotDigest = validated.snapshot_digest;
    return JSON.parse(canonicalJson(validated)) as AutonomousMemoryConsolidationSchedulerSnapshot;
  }
}

export class JsonAutonomousMemoryConsolidationSchedulerPersistence {
  readonly textStore: AutonomousMemoryConsolidationSchedulerTextStore;
  readonly maxBytes: number;
  constructor(textStore: AutonomousMemoryConsolidationSchedulerTextStore, maxBytes = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") fail("JSON text store is malformed");
    this.textStore = textStore;
    this.maxBytes = integerBound("JSON maxBytes", maxBytes, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEDULER_SNAPSHOT_BYTES);
  }
  read(): AutonomousMemoryConsolidationSchedulerSnapshot | null {
    const encoded = this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch (error) { throw new AutonomousMemoryConsolidationError("memory consolidation scheduler JSON is invalid", { cause: error }); }
    if (canonicalJson(parsed) !== encoded) fail("JSON snapshot is not canonical");
    return validateAutonomousMemoryConsolidationSchedulerSnapshot(parsed);
  }
  write(snapshot: AutonomousMemoryConsolidationSchedulerSnapshot): void {
    const encoded = canonicalJson(validateAutonomousMemoryConsolidationSchedulerSnapshot(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    this.textStore.write(encoded);
  }
}

export class TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence extends JsonAutonomousMemoryConsolidationSchedulerPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousMemoryConsolidationSchedulerSnapshot): boolean {
    digest("expectedSnapshotDigest", expectedSnapshotDigest, true);
    if (typeof (this.textStore as Partial<AutonomousMemoryConsolidationSchedulerTransactionalTextStore>).writeIfUnchanged !== "function") fail("transactional JSON text store lacks compare-and-swap");
    const encoded = canonicalJson(validateAutonomousMemoryConsolidationSchedulerSnapshot(snapshot));
    return Boolean((this.textStore as AutonomousMemoryConsolidationSchedulerTransactionalTextStore).writeIfUnchanged(expectedSnapshotDigest, encoded));
  }
}

export class AutonomousMemoryConsolidationSchedulerPersistenceCoordinator {
  readonly scheduler: AutonomousMemoryConsolidationScheduler;
  readonly persistence: JsonAutonomousMemoryConsolidationSchedulerPersistence;
  private expectedSnapshotDigest: string | null = null;
  constructor(scheduler: AutonomousMemoryConsolidationScheduler, persistence: JsonAutonomousMemoryConsolidationSchedulerPersistence) {
    if (!(scheduler instanceof AutonomousMemoryConsolidationScheduler) || !persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") fail("persistence coordinator inputs are malformed");
    this.scheduler = scheduler;
    this.persistence = persistence;
  }
  restore(): AutonomousMemoryConsolidationSchedulerSnapshot | null {
    const snapshot = this.persistence.read();
    if (snapshot === null) return null;
    this.scheduler.restore(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
  flush(): AutonomousMemoryConsolidationSchedulerSnapshot {
    const snapshot = this.scheduler.snapshot();
    if (this.persistence instanceof TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence && !this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) fail("persistence compare-and-swap conflict");
    if (!(this.persistence instanceof TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence)) this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
}
