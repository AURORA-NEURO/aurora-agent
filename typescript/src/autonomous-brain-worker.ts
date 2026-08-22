import { ArgumentError, isObject, ProviderRuntimeError } from "./errors.js";
import {
  AutonomousBrainFacade,
  type AutonomousBrainAdaptiveCycleOptions,
  type AutonomousBrainAdaptiveCycleExecution,
  type AutonomousBrainCycleExecution,
  type AutonomousBrainCycleOptions,
  type AutonomousBrainCycleTraceOptions,
  type AutonomousBrainExecuteOptions,
  type AutonomousBrainExecution,
  type AutonomousBrainPlan,
  type AutonomousBrainRequest,
} from "./autonomous-brain-facade.js";
import {
  InMemoryAutonomousBrainJobScheduler,
  type AutonomousBrainJob,
} from "./autonomous-brain-jobs.js";
import type { AutonomousRunTraceStore, AutonomousRunTraceSummary } from "./autonomous-run-trace.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only worker lifecycle for one rehydrated autonomous brain job. */
export const AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA = "bioprism-typescript-autonomous-brain-job-worker/0.2" as const;
export const AUTONOMOUS_BRAIN_JOB_SPEC_SCHEMA = "bioprism-typescript-autonomous-brain-job-spec/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_WORKER_HEARTBEAT_MS = 300_000;
export const MAX_AUTONOMOUS_BRAIN_WORKER_BATCH = 64;

export type AutonomousBrainJobExecutionMode = "execute" | "cycle" | "adaptive";
export type AutonomousBrainJobWorkerStatus = "succeeded" | "waiting_approval" | "retry_scheduled" | "reconciliation_required" | "failed" | "already_terminal";

export interface AutonomousBrainJobSpecDigestInput {
  request: AutonomousBrainRequest;
  mode: AutonomousBrainJobExecutionMode;
  /** Digest of caller-owned provider/evaluator/connector policy; private policy values stay transient. */
  policyDigest?: string | null;
}

export interface AutonomousBrainJobResolution {
  specDigest: string;
  policyDigest?: string | null;
  request: AutonomousBrainRequest;
  mode: AutonomousBrainJobExecutionMode;
  execute?: AutonomousBrainExecuteOptions;
  cycle?: AutonomousBrainCycleOptions;
  adaptive?: AutonomousBrainAdaptiveCycleOptions;
}

export interface AutonomousBrainJobResolverContext {
  job: AutonomousBrainJob;
  approvalReleased: boolean;
  attempt: number;
}

export type AutonomousBrainJobResolver = (
  context: AutonomousBrainJobResolverContext,
) => AutonomousBrainJobResolution | Promise<AutonomousBrainJobResolution>;

export interface AutonomousBrainJobWorkerOptions {
  brain: AutonomousBrainFacade;
  scheduler: InMemoryAutonomousBrainJobScheduler;
  workerId: string;
  resolve: AutonomousBrainJobResolver;
  traceStore?: AutonomousRunTraceStore;
  leaseMs?: number;
  heartbeatMs?: number;
  /** Retry only typed, retryable failures that occurred before the facade was invoked. */
  retryPreflightFailures?: boolean;
}

export interface AutonomousBrainJobWorkerRun {
  schema: typeof AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA;
  worker_id: string;
  job_id: string;
  status: AutonomousBrainJobWorkerStatus;
  job: AutonomousBrainJob;
  mode: AutonomousBrainJobExecutionMode | null;
  execution: AutonomousBrainExecution | null;
  cycle: AutonomousBrainCycleExecution | null;
  adaptive: AutonomousBrainAdaptiveCycleExecution | null;
  trace: AutonomousRunTraceSummary | null;
  error_class: string | null;
  failure_code: string | null;
  error_retryable: boolean | null;
  retention: "metadata_only_job_and_trace;private_task_policy_provider_and_evaluator_values_transient";
  secret_material: "never_returned";
}

export interface AutonomousBrainJobWorkerBatch {
  schema: typeof AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA;
  worker_id: string;
  status: "empty" | "completed" | "partial" | "failed";
  runs: AutonomousBrainJobWorkerRun[];
  claimed_count: number;
  succeeded_count: number;
  waiting_count: number;
  reconciliation_count: number;
  retry_scheduled_count: number;
  failed_count: number;
  batch_digest: string;
  retention: "metadata_only_job_and_trace;private_task_policy_provider_and_evaluator_values_transient";
  secret_material: "never_returned";
}

const RETENTION = "metadata_only_job_and_trace;private_task_policy_provider_and_evaluator_values_transient" as const;
const SECRET_MATERIAL = "never_returned" as const;
const APPROVAL_STATUSES = new Set(["approval_required", "route_review_required", "plan_review_required", "connector_blocked"]);

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be within [${minimum}, ${maximum}]`);
  return value as number;
}

function mode(value: unknown): AutonomousBrainJobExecutionMode {
  if (value !== "execute" && value !== "cycle" && value !== "adaptive") throw new ArgumentError("autonomous brain job execution mode is invalid");
  return value;
}

function errorProjection(error: unknown): { errorClass: string; failureCode: string; retryable: boolean | null } {
  if (error instanceof ProviderRuntimeError) return { errorClass: error.constructor.name, failureCode: error.code, retryable: error.retryable };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { errorClass: error.constructor.name, failureCode: "error", retryable: null };
  return { errorClass: "AutonomousBrainJobWorkerError", failureCode: "unknown", retryable: null };
}

function statusOf(result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution): string {
  return result.status;
}

function resultDigest(result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution, trace: AutonomousRunTraceSummary | null): string {
  const base: JsonObject = {
    schema: result.schema,
    status: result.status,
    plan_digest: result.plan.plan_digest,
    route_digest: result.plan.route.route_digest,
    trace_digest: trace?.trace_digest ?? null,
  };
  if ("run" in result && result.run) base.run_status = result.run.status;
  if ("cycle" in result && result.cycle) base.cycle_status = result.cycle.status;
  if ("adaptive" in result && result.adaptive) base.adaptive_status = result.adaptive.status;
  return digestJsonSync(base);
}

function requestDomainCovered(plan: AutonomousBrainPlan, domain: AutonomousBrainJob["domain"]): boolean {
  return plan.selected_domains.includes(domain) || plan.route.primary_domain === domain;
}

function approvalBoundary(result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution): "preflight" | "dispatched" {
  if ("connector" in result && result.connector !== null) return "dispatched";
  if ("run" in result && result.run !== null) return "dispatched";
  if ("cycle" in result && result.cycle !== null) {
    const cycle = result.cycle as unknown as Record<string, unknown>;
    if (isObject(cycle.run) && cycle.run !== null) return "dispatched";
  }
  if ("adaptive" in result && result.adaptive !== null) {
    const adaptive = result.adaptive as unknown as Record<string, unknown>;
    const final = isObject(adaptive.final) ? adaptive.final : null;
    if (final && isObject(final.run) && final.run !== null) return "dispatched";
  }
  return "preflight";
}

/**
 * Compute the digest that binds a private rehydrated request to a durable job identity.
 * The request is hashed transiently; it is never copied into the scheduler checkpoint.
 */
export function autonomousBrainJobSpecDigest(input: AutonomousBrainJobSpecDigestInput): string {
  if (!input || typeof input !== "object") throw new ArgumentError("autonomous brain job spec digest input is malformed");
  const selectedMode = mode(input.mode);
  const policyDigest = digest("job policyDigest", input.policyDigest ?? null, true);
  if (!input.request || typeof input.request !== "object") throw new ArgumentError("autonomous brain job spec digest request is malformed");
  return digestJsonSync({ schema: AUTONOMOUS_BRAIN_JOB_SPEC_SCHEMA, mode: selectedMode, request: input.request, policy_digest: policyDigest });
}

/**
 * Rehydrates private job material behind a fenced metadata-only scheduler and invokes the
 * reviewed brain facade. A worker never persists the task, prompt, credentials, provider
 * response, evaluator evidence, or connector payload; only bounded digests and lifecycle state
 * cross the scheduler boundary.
 */
export class AutonomousBrainJobWorker {
  readonly brain: AutonomousBrainFacade;
  readonly scheduler: InMemoryAutonomousBrainJobScheduler;
  readonly workerId: string;
  readonly resolve: AutonomousBrainJobResolver;
  readonly traceStore?: AutonomousRunTraceStore;
  readonly leaseMs: number;
  readonly heartbeatMs: number;
  readonly retryPreflightFailures: boolean;

  constructor(options: AutonomousBrainJobWorkerOptions) {
    if (!options || !(options.brain instanceof AutonomousBrainFacade)) throw new ArgumentError("autonomous brain job worker requires an AutonomousBrainFacade");
    if (!(options.scheduler instanceof InMemoryAutonomousBrainJobScheduler)) throw new ArgumentError("autonomous brain job worker requires a typed job scheduler");
    this.brain = options.brain;
    this.scheduler = options.scheduler;
    this.workerId = identifier("autonomous brain workerId", options.workerId);
    if (typeof options.resolve !== "function") throw new ArgumentError("autonomous brain job worker resolver must be callable");
    this.resolve = options.resolve;
    if (options.traceStore !== undefined && (typeof options.traceStore.append !== "function" || typeof options.traceStore.events !== "function")) throw new ArgumentError("autonomous brain job worker traceStore is malformed");
    this.traceStore = options.traceStore;
    this.leaseMs = boundedInteger("autonomous brain worker leaseMs", options.leaseMs ?? 60_000, 1, 600_000);
    this.heartbeatMs = boundedInteger("autonomous brain worker heartbeatMs", options.heartbeatMs ?? Math.min(30_000, Math.floor(this.leaseMs / 3)), 1, MAX_AUTONOMOUS_BRAIN_WORKER_HEARTBEAT_MS);
    if (this.heartbeatMs >= this.leaseMs) throw new ArgumentError("autonomous brain worker heartbeatMs must be less than leaseMs");
    if (options.retryPreflightFailures !== undefined && typeof options.retryPreflightFailures !== "boolean") throw new ArgumentError("autonomous brain worker retryPreflightFailures must be boolean");
    this.retryPreflightFailures = options.retryPreflightFailures ?? true;
  }

  async runOnce(jobId?: string, now?: number): Promise<AutonomousBrainJobWorkerRun | null> {
    const claimed = jobId === undefined
      ? this.scheduler.claimNext(this.workerId, this.leaseMs, now)
      : this.scheduler.claim(jobId, this.workerId, this.leaseMs, now);
    if (claimed === null) return null;
    if (["succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"].includes(claimed.state)) {
      return this.envelope(claimed, "already_terminal", null, null, null, null, null, null);
    }

    let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
    let heartbeatRunning = false;
    let heartbeatError: unknown = null;
    const heartbeat = async (): Promise<void> => {
      if (heartbeatRunning || heartbeatError !== null) return;
      heartbeatRunning = true;
      try {
        this.scheduler.renew(claimed.job_id, this.workerId, this.leaseMs);
      } catch (error) {
        heartbeatError = error;
      } finally {
        heartbeatRunning = false;
      }
    };
    heartbeatTimer = setInterval(() => { void heartbeat(); }, this.heartbeatMs);
    const unref = (heartbeatTimer as unknown as { unref?: () => void }).unref;
    if (typeof unref === "function") unref.call(heartbeatTimer);

    let executionStarted = false;
    let planCompiled = false;
    let resolution: AutonomousBrainJobResolution | null = null;
    let trace: AutonomousRunTraceSummary | null = null;
    try {
      const approvalReleased = this.scheduler.eventsFor(claimed.job_id).some((event) => event.event_type === "job_approval_released");
      this.scheduler.checkpoint(claimed.job_id, this.workerId, {
        phase: "resolving_private_spec",
        checkpointDigest: digestJsonSync({ schema: AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA, job_id: claimed.job_id, spec_digest: claimed.spec_digest, attempt: claimed.attempts }),
        sideEffectBoundary: claimed.side_effect_boundary === "not_started" ? "not_started" : "preflight",
      });
      resolution = await this.resolve({ job: claimed, approvalReleased, attempt: claimed.attempts });
      this.validateResolution(claimed, resolution);
      const plan = await this.brain.plan(resolution.request);
      if (!requestDomainCovered(plan, claimed.domain)) throw new ArgumentError("rehydrated brain request is outside the durable job domain");
      if (plan.status === "route_review_required") {
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: "route_review_required", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, route_digest: plan.route.route_digest }), sideEffectBoundary: "preflight", waitingForApproval: true });
        return this.envelope(this.scheduler.get(claimed.job_id)!, "waiting_approval", resolution.mode, null, null, null, null, null);
      }
      planCompiled = true;
      this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: "plan_compiled", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, route_digest: plan.route.route_digest, mode: resolution.mode }), sideEffectBoundary: "preflight" });
      // The durable worker owns a second approval gate around the entire rehydrated dispatch.
      // Do not even invoke the facade before the scheduler records an explicit release; this
      // prevents a connector or provider-planning callback from doing work during a pause.
      if (!approvalReleased) {
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: "provider_approval_required", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, mode: resolution.mode }), sideEffectBoundary: "preflight", waitingForApproval: true });
        return this.envelope(this.scheduler.get(claimed.job_id)!, "waiting_approval", resolution.mode, null, null, null, null, null);
      }
      const approvedResolution = this.approvalBoundResolution(resolution, approvalReleased);
      const shouldTrace = this.traceStore !== undefined;
      this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: "dispatch_started", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, attempt: claimed.attempts }), sideEffectBoundary: "dispatched" });
      executionStarted = true;
      if (heartbeatError !== null) throw heartbeatError;
      let result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution;
      if (resolution.mode === "execute") {
        if (shouldTrace) {
          const traced = await this.brain.executePlannedWithTrace(plan, resolution.request, { ...(approvedResolution.execute ?? {}), traceStore: this.traceStore!, runId: `${claimed.job_id}:attempt-${claimed.attempts}` });
          result = traced.execution;
          trace = traced.trace;
        } else result = await this.brain.executePlanned(plan, resolution.request, approvedResolution.execute ?? {});
      } else if (resolution.mode === "cycle") {
        if (shouldTrace) {
          const traced = await this.brain.executePlannedCycleWithTrace(plan, resolution.request, { ...(approvedResolution.cycle ?? {}), traceStore: this.traceStore!, runId: `${claimed.job_id}:attempt-${claimed.attempts}` });
          result = traced.execution;
          trace = traced.trace;
        } else result = await this.brain.executePlannedCycle(plan, resolution.request, approvedResolution.cycle ?? {});
      } else if (shouldTrace) {
        const adaptiveOptions = approvedResolution.adaptive;
        if (!adaptiveOptions) throw new ArgumentError("adaptive brain job policy disappeared during approval binding");
        const traced = await this.brain.executePlannedAdaptiveCycleWithTrace(plan, resolution.request, { ...adaptiveOptions, traceStore: this.traceStore!, runId: `${claimed.job_id}:attempt-${claimed.attempts}` });
        result = traced.execution;
        trace = traced.trace;
      } else result = await this.brain.executePlannedAdaptiveCycle(plan, resolution.request, approvedResolution.adaptive!);

      if (heartbeatError !== null) throw heartbeatError;
      const status = statusOf(result);
      if (APPROVAL_STATUSES.has(status)) {
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: status, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: approvalBoundary(result), waitingForApproval: true });
        return this.envelope(this.scheduler.get(claimed.job_id)!, "waiting_approval", resolution.mode, result, null, null, trace, null);
      }
      if (status === "reconciliation_required") {
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: status, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: "unknown" });
        const quarantined = this.scheduler.fail(claimed.job_id, this.workerId, { reason: "brain execution requires caller reconciliation", retryable: false });
        return this.envelope(quarantined, "reconciliation_required", resolution.mode, result, null, null, trace, null);
      }
      if (status !== "completed") {
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: `terminal_${status}`, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: "dispatched" });
        const failed = this.scheduler.fail(claimed.job_id, this.workerId, { reason: `brain execution ended with ${status}`, retryable: false });
        return this.envelope(failed, failed.state === "reconciliation_required" ? "reconciliation_required" : "failed", resolution.mode, result, null, null, trace, null);
      }
      const completed = this.scheduler.complete(claimed.job_id, this.workerId, resultDigest(result, trace));
      return this.envelope(completed, "succeeded", resolution.mode, result, null, null, trace, null);
    } catch (error) {
      const projection = errorProjection(error);
      const current = this.scheduler.get(claimed.job_id);
      if (!current || current.lease_owner !== this.workerId || !["leased", "running"].includes(current.state)) throw error;
      try {
        const boundary = executionStarted ? "unknown" : planCompiled ? "preflight" : "not_started";
        this.scheduler.checkpoint(claimed.job_id, this.workerId, { phase: "worker_execution_error", checkpointDigest: digestJsonSync({ error_class: projection.errorClass, failure_code: projection.failureCode }), sideEffectBoundary: boundary });
        const retryablePreflight = !executionStarted && this.retryPreflightFailures && error instanceof ProviderRuntimeError && error.retryable;
        const failed = this.scheduler.fail(claimed.job_id, this.workerId, {
          reason: executionStarted
            ? "worker execution outcome is uncertain; reconciliation required"
            : retryablePreflight
              ? "retryable worker preflight failure; scheduler retry policy applied"
              : "worker execution failed before provider dispatch",
          retryable: retryablePreflight,
        });
        const workerStatus = failed.state === "reconciliation_required"
          ? "reconciliation_required"
          : failed.state === "queued"
            ? "retry_scheduled"
            : "failed";
        return this.envelope(failed, workerStatus, resolution?.mode ?? null, null, null, null, trace, projection);
      } catch (persistenceError) {
        const wrapped = new ProviderRuntimeError("autonomous brain worker failure could not be durably recorded", { code: "configuration" });
        (wrapped as Error & { cause?: unknown }).cause = persistenceError;
        throw wrapped;
      }
    } finally {
      if (heartbeatTimer !== null) clearInterval(heartbeatTimer);
    }
  }

  async run(options: { limit?: number } = {}): Promise<AutonomousBrainJobWorkerBatch> {
    const limit = boundedInteger("autonomous brain worker limit", options.limit ?? 1, 1, MAX_AUTONOMOUS_BRAIN_WORKER_BATCH);
    const runs: AutonomousBrainJobWorkerRun[] = [];
    for (let index = 0; index < limit; index += 1) {
      const result = await this.runOnce();
      if (result === null) break;
      runs.push(result);
      if (result.status === "waiting_approval" || result.status === "retry_scheduled" || result.status === "reconciliation_required") break;
    }
    const succeeded = runs.filter((run) => run.status === "succeeded").length;
    const waiting = runs.filter((run) => run.status === "waiting_approval").length;
    const reconciliation = runs.filter((run) => run.status === "reconciliation_required").length;
    const retryScheduled = runs.filter((run) => run.status === "retry_scheduled").length;
    const failed = runs.filter((run) => run.status === "failed").length;
    return {
      schema: AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA,
      worker_id: this.workerId,
      status: runs.length === 0 ? "empty" : failed > 0 && succeeded === 0 && waiting === 0 && reconciliation === 0 && retryScheduled === 0 ? "failed" : waiting > 0 || reconciliation > 0 || retryScheduled > 0 || failed > 0 ? "partial" : "completed",
      runs,
      claimed_count: runs.filter((run) => run.status !== "already_terminal").length,
      succeeded_count: succeeded,
      waiting_count: waiting,
      reconciliation_count: reconciliation,
      retry_scheduled_count: retryScheduled,
      failed_count: failed,
      batch_digest: digestJsonSync(runs.map((run) => ({ job_id: run.job_id, status: run.status, job_digest: run.job.job_digest, trace_digest: run.trace?.trace_digest ?? null }))),
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }

  private validateResolution(job: AutonomousBrainJob, value: AutonomousBrainJobResolution): void {
    if (!value || typeof value !== "object") throw new ArgumentError("brain job resolver must return an object");
    const selectedMode = mode(value.mode);
    if (digest("job resolution specDigest", value.specDigest) !== job.spec_digest) throw new ArgumentError("brain job resolver specDigest does not match the durable job");
    if (autonomousBrainJobSpecDigest({ request: value.request, mode: selectedMode, policyDigest: value.policyDigest ?? null }) !== job.spec_digest) throw new ArgumentError("brain job request, mode, and policy digest do not match the durable spec");
    if (!value.request || typeof value.request !== "object" || typeof value.request.task !== "string" || !value.request.task.trim()) throw new ArgumentError("brain job resolver request is invalid");
    if (selectedMode === "adaptive" && (!value.adaptive || typeof value.adaptive !== "object" || typeof value.adaptive.adaptive?.evaluate !== "function")) throw new ArgumentError("adaptive brain job requires an evaluator policy");
  }

  private approvalBoundResolution(value: AutonomousBrainJobResolution, approvalReleased: boolean): AutonomousBrainJobResolution {
    if (value.mode === "execute") {
      return { ...value, execute: { ...(value.execute ?? {}), approveProviderCall: approvalReleased, run: { ...(value.execute?.run ?? {}), approveProviderCall: approvalReleased } } };
    }
    if (value.mode === "cycle") {
      return { ...value, cycle: { ...(value.cycle ?? {}), approveProviderCall: approvalReleased, cycle: { ...(value.cycle?.cycle ?? {}), approveProviderCall: approvalReleased } } };
    }
    const adaptive = value.adaptive;
    if (!adaptive) throw new ArgumentError("adaptive brain job policy is missing");
    return { ...value, adaptive: { ...adaptive, approveProviderCall: approvalReleased, adaptive: { ...adaptive.adaptive, approveProviderCall: approvalReleased } } };
  }

  private envelope(
    job: AutonomousBrainJob,
    status: AutonomousBrainJobWorkerStatus,
    selectedMode: AutonomousBrainJobExecutionMode | null,
    result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution | null,
    execution: AutonomousBrainExecution | null,
    cycle: AutonomousBrainCycleExecution | null,
    trace: AutonomousRunTraceSummary | null,
    error: { errorClass: string; failureCode: string; retryable: boolean | null } | null,
  ): AutonomousBrainJobWorkerRun {
    const resolvedExecution = execution ?? (result && "run" in result ? result as AutonomousBrainExecution : null);
    const resolvedCycle = cycle ?? (result && "cycle" in result ? result as AutonomousBrainCycleExecution : null);
    const resolvedAdaptive = result && "adaptive" in result ? result as AutonomousBrainAdaptiveCycleExecution : null;
    return { schema: AUTONOMOUS_BRAIN_JOB_WORKER_SCHEMA, worker_id: this.workerId, job_id: job.job_id, status, job, mode: selectedMode, execution: resolvedExecution, cycle: resolvedCycle, adaptive: resolvedAdaptive, trace, error_class: error?.errorClass ?? null, failure_code: error?.failureCode ?? null, error_retryable: error?.retryable ?? null, retention: RETENTION, secret_material: SECRET_MATERIAL };
  }
}
