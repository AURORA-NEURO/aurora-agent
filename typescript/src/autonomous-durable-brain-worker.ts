import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import { AutonomousBrainFacade, AutonomousBrainPlan, type AutonomousBrainAdaptiveCycleExecution, type AutonomousBrainAdaptiveCycleOptions, type AutonomousBrainCycleExecution, type AutonomousBrainCycleOptions, type AutonomousBrainExecution, type AutonomousBrainExecuteOptions, type AutonomousBrainRequest } from "./autonomous-brain-facade.js";
import { AutonomousBrainJobProtectedRehydrator, autonomousBrainJobSpecDigest, type AutonomousBrainJobExecutionMode, type AutonomousBrainJobResolution } from "./autonomous-brain-worker.js";
import type { ApiClient } from "./client.js";
import { digestJsonSync } from "./tooling.js";
import type { AutonomousRunTraceStore, AutonomousRunTraceSummary } from "./autonomous-run-trace.js";
import type { AutonomousCredentialBinding, AutonomousCredentialScope } from "./autonomous-credential-scope.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import type {
  BrainControlEvent,
  BrainJobApprovalAction,
  BrainJobApprovalResult,
  BrainJobCancelResult,
  BrainJobClaimNextResult,
  BrainJobRecord,
  BrainJobReconcileOutcome,
  BrainJobStatusResult,
  BrainJobSubmitResult,
  BrainJobLifecycleResult,
  JsonObject,
  JsonValue,
  RestToolResponse,
} from "./types.js";

export const AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA = "bioprism-typescript-autonomous-durable-brain-job-worker/0.1" as const;
export const MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_LEASE_MS = 86_400_000;
export const MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_HEARTBEAT_MS = 300_000;
export const MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_BATCH = 64;
export const MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_EVENT_PAGES = 8;

export type AutonomousDurableBrainJobWorkerStatus = "succeeded" | "waiting_approval" | "retry_scheduled" | "reconciliation_required" | "failed" | "already_terminal";

export type AutonomousDurableBrainJobApi = Pick<ApiClient, "brainJobSubmit" | "brainJobStatus" | "brainJobEvents" | "brainJobApproval" | "brainJobClaim" | "brainJobClaimNext" | "brainJobRenew" | "brainJobCheckpoint" | "brainJobComplete" | "brainJobFail" | "brainJobReconcile" | "brainJobCancel">;

export interface AutonomousDurableBrainJobSubmitOptions {
  idempotencyKey: string;
  request: AutonomousBrainRequest;
  mode: AutonomousBrainJobExecutionMode;
  policyDigest?: string | null;
  priority?: number;
  maxAttempts?: number;
  checkpointDigest?: string | null;
}

export interface AutonomousDurableBrainJobSubmission {
  schema: typeof AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA;
  status: "submitted" | "route_review_required" | "connector_review_required";
  plan: AutonomousBrainPlan;
  job: BrainJobRecord | null;
  spec_digest: string | null;
  private_spec: "caller_owned;request_policy_credentials_and_evaluator_values_not_sent_to_control_plane";
  secret_material: "never_returned";
}

export interface AutonomousDurableBrainJobResolverContext {
  job: BrainJobRecord;
  approvalReleased: boolean;
  attempt: number;
}

/** Caller-owned request, provider, credential, connector, and evaluator state. */
export type AutonomousDurableBrainJobResolution = AutonomousBrainJobResolution;

export type AutonomousDurableBrainJobResolver = (
  context: AutonomousDurableBrainJobResolverContext,
) => Promise<AutonomousDurableBrainJobResolution> | AutonomousDurableBrainJobResolution;

export interface AutonomousDurableBrainJobWorkerOptions {
  brain: AutonomousBrainFacade;
  apiClient: AutonomousDurableBrainJobApi;
  workerId: string;
  /** Explicit resolver remains authoritative when both resolution paths are configured. */
  resolve?: AutonomousDurableBrainJobResolver;
  /** Optional protected receipt fallback for restart-safe caller-owned private specs. */
  protectedRehydration?: AutonomousBrainJobProtectedRehydrator;
  traceStore?: AutonomousRunTraceStore;
  leaseMs?: number;
  heartbeatMs?: number;
  /** Retry only typed failures that occur before the facade dispatch boundary. */
  retryPreflightFailures?: boolean;
  /** Open an opaque credential session only after approval is released; never part of job state. */
  credentialScope?: AutonomousCredentialScope;
}

export interface AutonomousDurableBrainJobWorkerRun {
  schema: typeof AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA;
  worker_id: string;
  job_id: string;
  status: AutonomousDurableBrainJobWorkerStatus;
  job: BrainJobRecord;
  mode: AutonomousBrainJobExecutionMode | null;
  execution: AutonomousBrainExecution | null;
  cycle: AutonomousBrainCycleExecution | null;
  adaptive: AutonomousBrainAdaptiveCycleExecution | null;
  trace: AutonomousRunTraceSummary | null;
  error_class: string | null;
  failure_code: string | null;
  error_retryable: boolean | null;
  retention: "remote_job_metadata_only;private_brain_values_transient_to_caller";
  secret_material: "never_returned";
}

export interface AutonomousDurableBrainJobWorkerBatch {
  schema: typeof AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA;
  worker_id: string;
  status: "empty" | "completed" | "partial" | "failed";
  runs: AutonomousDurableBrainJobWorkerRun[];
  claimed_count: number;
  succeeded_count: number;
  waiting_count: number;
  reconciliation_count: number;
  retry_scheduled_count: number;
  failed_count: number;
  already_terminal_count: number;
  batch_digest: string;
  retention: "remote_job_metadata_only;private_brain_values_transient_to_caller";
  secret_material: "never_returned";
}

const RETENTION = "remote_job_metadata_only;private_brain_values_transient_to_caller" as const;
const SECRET_MATERIAL = "never_returned" as const;
const PRIVATE_KEYS = new Set(["task", "prompt", "credentials", "credential", "password", "secret", "token", "response", "content", "provider_response", "tool_arguments", "tool_output"]);
const APPROVAL_STATUSES = new Set(["approval_required", "route_review_required", "plan_review_required", "connector_blocked"]);
const TERMINAL_JOB_STATES = new Set(["succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"]);

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, allowed: readonly string[], label: string): void {
  const known = new Set(allowed);
  if (Object.keys(value).some((key) => !known.has(key))) throw new ArgumentError(`${label} contains unsupported fields`);
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z][A-Za-z0-9_.:+-]*$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be within [${minimum}, ${maximum}]`);
  return value as number;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function mode(value: unknown): AutonomousBrainJobExecutionMode {
  if (value !== "execute" && value !== "cycle" && value !== "adaptive") throw new ArgumentError("durable brain job execution mode is invalid");
  return value;
}

function rejectPrivateFields(value: unknown, depth = 0): void {
  if (depth > 8) throw new ProviderRuntimeError("durable brain control-plane projection is too deeply nested", { code: "protocol" });
  if (Array.isArray(value)) {
    for (const item of value) rejectPrivateFields(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (PRIVATE_KEYS.has(key.toLowerCase())) throw new ProviderRuntimeError("durable brain control-plane projection contains private material", { code: "protocol" });
    rejectPrivateFields(child, depth + 1);
  }
}

function project<T extends JsonValue>(response: RestToolResponse<T>, operation: string): T {
  if (!response || response.ok !== true || response.mcp?.error || response.mcp?.result?.isError) throw new ProviderRuntimeError(`${operation} returned a control-plane refusal`, { code: "protocol" });
  const value = response.mcp?.result?.structuredContent;
  if (!value || typeof value !== "object") throw new ProviderRuntimeError(`${operation} returned no structured projection`, { code: "protocol" });
  rejectPrivateFields(value);
  return value as T;
}

function validateJob(value: unknown): BrainJobRecord {
  if (!isObject(value)) throw new ProviderRuntimeError("durable brain control plane returned a malformed job", { code: "protocol" });
  boundedIdentifier("durable brain job_id", value.job_id);
  digest("durable brain spec_digest", value.spec_digest);
  if (typeof value.domain !== "string" || !value.domain.trim() || typeof value.capability !== "string" || !value.capability.trim() || typeof value.risk_class !== "string" || !value.risk_class.trim()) throw new ProviderRuntimeError("durable brain job metadata is malformed", { code: "protocol" });
  if (typeof value.state !== "string" || !value.state.trim() || !Number.isSafeInteger(value.attempts) || (value.attempts as number) < 0) throw new ProviderRuntimeError("durable brain job lifecycle metadata is malformed", { code: "protocol" });
  if (value.record_digest !== undefined) digest("durable brain record_digest", value.record_digest);
  return value as BrainJobRecord;
}

function errorProjection(error: unknown): { errorClass: string; failureCode: string; retryable: boolean | null } {
  if (error instanceof ProviderRuntimeError) return { errorClass: error.name, failureCode: error.code, retryable: error.retryable };
  if (error instanceof ArgumentError) return { errorClass: error.name, failureCode: "configuration", retryable: false };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { errorClass: error.constructor.name, failureCode: "error", retryable: null };
  return { errorClass: "AutonomousDurableBrainJobWorkerError", failureCode: "unknown", retryable: null };
}

function requestDomainCovered(plan: AutonomousBrainPlan, domain: string): boolean {
  if (domain === "cross_domain") return plan.route.cross_domain && plan.cross_domain_plan !== null;
  return !plan.route.cross_domain && plan.domain_plan?.domain === domain;
}

function metadataForPlan(plan: AutonomousBrainPlan): { domain: string; capability: string; riskClass: string } {
  if (plan.route.cross_domain) {
    if (!plan.cross_domain_plan) throw new ArgumentError("cross-domain brain plan is missing its synthesis metadata");
    return { domain: "cross_domain", capability: plan.cross_domain_plan.synthesis.capability, riskClass: plan.cross_domain_plan.synthesis.risk_class };
  }
  if (!plan.domain_plan) throw new ArgumentError("single-domain brain plan is missing its domain metadata");
  return { domain: plan.domain_plan.domain, capability: plan.domain_plan.capability, riskClass: plan.domain_plan.risk_class };
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

function approvalBoundary(result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution): "preflight" | "dispatched" {
  if ("connector" in result && result.connector !== null) return "dispatched";
  if ("run" in result && result.run !== null) return "dispatched";
  if ("cycle" in result && result.cycle !== null) return "dispatched";
  if ("adaptive" in result && result.adaptive !== null) return "dispatched";
  return "preflight";
}

/**
 * Remote counterpart of the local autonomous brain worker.
 *
 * The remote queue stores only job metadata. The resolver rehydrates the complete request and
 * private execution policy, while the worker verifies the composite spec digest before compiling
 * a plan. It supports execute, evaluator/learning cycle, adaptive replan cycle, all built-in
 * single-domain profiles, and cross-domain fan-out/synthesis through one claim-next boundary.
 */
export class AutonomousDurableBrainJobWorker {
  readonly brain: AutonomousBrainFacade;
  readonly apiClient: AutonomousDurableBrainJobApi;
  readonly workerId: string;
  readonly resolve?: AutonomousDurableBrainJobResolver;
  readonly protectedRehydration?: AutonomousBrainJobProtectedRehydrator;
  readonly traceStore?: AutonomousRunTraceStore;
  readonly leaseMs: number;
  readonly heartbeatMs: number;
  readonly retryPreflightFailures: boolean;
  readonly credentialScope?: AutonomousCredentialScope;

  constructor(options: AutonomousDurableBrainJobWorkerOptions) {
    if (!options || !(options.brain instanceof AutonomousBrainFacade)) throw new ArgumentError("durable brain worker requires an AutonomousBrainFacade");
    if (!options.apiClient || ["brainJobSubmit", "brainJobStatus", "brainJobEvents", "brainJobApproval", "brainJobClaim", "brainJobClaimNext", "brainJobRenew", "brainJobCheckpoint", "brainJobComplete", "brainJobFail", "brainJobReconcile", "brainJobCancel"].some((name) => typeof (options.apiClient as unknown as Record<string, unknown>)[name] !== "function")) throw new ArgumentError("durable brain worker requires the complete brain job ApiClient surface");
    if (options.resolve !== undefined && typeof options.resolve !== "function") throw new ArgumentError("durable brain worker resolver must be callable");
    if (options.protectedRehydration !== undefined && !(options.protectedRehydration instanceof AutonomousBrainJobProtectedRehydrator)) throw new ArgumentError("durable brain worker protectedRehydration is malformed");
    if (options.resolve === undefined && options.protectedRehydration === undefined) throw new ArgumentError("durable brain worker requires resolve or protectedRehydration");
    this.brain = options.brain;
    this.apiClient = options.apiClient;
    this.workerId = boundedIdentifier("durable brain workerId", options.workerId);
    this.resolve = options.resolve;
    this.protectedRehydration = options.protectedRehydration;
    if (options.traceStore !== undefined && (typeof options.traceStore.append !== "function" || typeof options.traceStore.events !== "function")) throw new ArgumentError("durable brain worker traceStore is malformed");
    this.traceStore = options.traceStore;
    this.leaseMs = boundedInteger("durable brain worker leaseMs", options.leaseMs ?? 300_000, 100, MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_LEASE_MS);
    this.heartbeatMs = boundedInteger("durable brain worker heartbeatMs", options.heartbeatMs ?? Math.min(30_000, Math.floor(this.leaseMs / 3)), 1, MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_HEARTBEAT_MS);
    if (this.heartbeatMs >= this.leaseMs) throw new ArgumentError("durable brain worker heartbeatMs must be less than leaseMs");
    if (options.retryPreflightFailures !== undefined && typeof options.retryPreflightFailures !== "boolean") throw new ArgumentError("durable brain worker retryPreflightFailures must be boolean");
    this.retryPreflightFailures = options.retryPreflightFailures ?? true;
    if (options.credentialScope !== undefined && typeof options.credentialScope.open !== "function") throw new ArgumentError("durable brain worker credentialScope is malformed");
    this.credentialScope = options.credentialScope;
  }

  /** Plan and admit a metadata-only remote job; the request itself remains caller-owned. */
  async submit(options: AutonomousDurableBrainJobSubmitOptions): Promise<AutonomousDurableBrainJobSubmission> {
    if (!options || typeof options !== "object") throw new ArgumentError("durable brain job submission options must be an object");
    if (typeof options.idempotencyKey !== "string" || !options.idempotencyKey.trim() || options.idempotencyKey.length > 512 || options.idempotencyKey.includes("\u0000")) throw new ArgumentError("durable brain idempotencyKey is outside its bounded contract");
    const selectedMode = mode(options.mode);
    const specDigest = autonomousBrainJobSpecDigest({ request: options.request, mode: selectedMode, policyDigest: options.policyDigest ?? null });
    const plan = await this.brain.plan(options.request);
    if (plan.status === "route_review_required") return { schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA, status: "route_review_required", plan, job: null, spec_digest: specDigest, private_spec: "caller_owned;request_policy_credentials_and_evaluator_values_not_sent_to_control_plane", secret_material: SECRET_MATERIAL };
    if (plan.status === "connector_review_required") return { schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA, status: "connector_review_required", plan, job: null, spec_digest: specDigest, private_spec: "caller_owned;request_policy_credentials_and_evaluator_values_not_sent_to_control_plane", secret_material: SECRET_MATERIAL };
    const metadata = metadataForPlan(plan);
    const submitted = project<BrainJobSubmitResult>(await this.apiClient.brainJobSubmit({ idempotency_key: options.idempotencyKey, spec_digest: specDigest, domain: metadata.domain, capability: metadata.capability, risk_class: metadata.riskClass, priority: options.priority, max_attempts: options.maxAttempts, checkpoint_digest: options.checkpointDigest ?? null }), "brain job submit");
    return { schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA, status: "submitted", plan, job: validateJob(submitted.job), spec_digest: specDigest, private_spec: "caller_owned;request_policy_credentials_and_evaluator_values_not_sent_to_control_plane", secret_material: SECRET_MATERIAL };
  }

  async status(jobId: string): Promise<BrainJobStatusResult> {
    const result = project<BrainJobStatusResult>(await this.apiClient.brainJobStatus({ job_id: boundedIdentifier("durable brain job_id", jobId) }), "brain job status");
    return { ...result, job: validateJob(result.job) };
  }

  async approval(jobId: string, action: BrainJobApprovalAction, options: { reason?: string; authorizationDigest?: string } = {}): Promise<BrainJobApprovalResult> {
    return project<BrainJobApprovalResult>(await this.apiClient.brainJobApproval({ job_id: boundedIdentifier("durable brain job_id", jobId), action, reason: options.reason, authorization_digest: options.authorizationDigest }), "brain job approval");
  }

  async reconcile(jobId: string, outcome: BrainJobReconcileOutcome, evidenceDigest: string, options: { evidenceKind?: string; operator?: string; reason?: string; effectAbsent?: boolean } = {}): Promise<BrainJobLifecycleResult> {
    return project<BrainJobLifecycleResult>(await this.apiClient.brainJobReconcile({ job_id: boundedIdentifier("durable brain job_id", jobId), outcome, evidence_digest: digest("durable brain evidenceDigest", evidenceDigest)!, evidence_kind: options.evidenceKind, operator: options.operator, reason: options.reason, effect_absent: options.effectAbsent }), "brain job reconcile");
  }

  async cancel(jobId: string, reason = "cancelled by caller"): Promise<BrainJobCancelResult> {
    return project<BrainJobCancelResult>(await this.apiClient.brainJobCancel({ job_id: boundedIdentifier("durable brain job_id", jobId), reason }), "brain job cancel");
  }

  async runOnce(jobId?: string): Promise<AutonomousDurableBrainJobWorkerRun | null> {
    const claimed = jobId === undefined
      ? project<BrainJobClaimNextResult>(await this.apiClient.brainJobClaimNext({ worker_id: this.workerId, lease_ms: this.leaseMs }), "brain job claim next")
      : project<BrainJobLifecycleResult>(await this.apiClient.brainJobClaim({ job_id: boundedIdentifier("durable brain job_id", jobId), worker_id: this.workerId, lease_ms: this.leaseMs }), "brain job claim");
    if (jobId === undefined) {
      const next = claimed as BrainJobClaimNextResult;
      if (!next.claimed || next.job === null) return null;
    }
    const job = validateJob(claimed.job);
    if (TERMINAL_JOB_STATES.has(job.state)) return this.envelope(job, "already_terminal", null, null, null, null, null, null);
    if (!job.lease_owner || job.lease_owner !== this.workerId || !["leased", "running"].includes(job.state)) throw new ProviderRuntimeError("durable brain control plane returned a job without an executable lease", { code: "protocol" });

    let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
    let heartbeatRunning = false;
    let heartbeatError: unknown = null;
    const heartbeat = async (): Promise<void> => {
      if (heartbeatRunning || heartbeatError !== null) return;
      heartbeatRunning = true;
      try {
        const renewed = project<BrainJobLifecycleResult>(await this.apiClient.brainJobRenew({ job_id: job.job_id, worker_id: this.workerId, lease_ms: this.leaseMs }), "brain job renew");
        validateJob(renewed.job);
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
    let resolution: AutonomousDurableBrainJobResolution | null = null;
    let credentialBinding: AutonomousCredentialBinding | null = null;
    let trace: AutonomousRunTraceSummary | null = null;
    try {
      const approvalReleased = await this.approvalReleased(job.job_id);
      await this.checkpoint(job.job_id, { phase: "resolving_private_spec", checkpointDigest: digestJsonSync({ schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA, job_id: job.job_id, spec_digest: job.spec_digest, attempt: job.attempts }), sideEffectBoundary: "not_started" });
      resolution = this.resolve !== undefined
        ? await this.resolve({ job, approvalReleased, attempt: job.attempts })
        : await this.protectedRehydration!.resolve({
          jobId: job.job_id,
          specDigest: job.spec_digest,
          domain: job.domain as AutonomousDomainName,
          capability: job.capability,
          attempt: job.attempts,
          approvalReleased,
        });
      this.validateResolution(job, resolution);
      const plan = await this.brain.plan(resolution.request);
      if (!requestDomainCovered(plan, job.domain)) throw new ArgumentError("rehydrated brain request is outside the durable job domain");
      if (plan.status === "route_review_required" || plan.status === "connector_review_required") {
        await this.checkpoint(job.job_id, { phase: plan.status, checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, route_digest: plan.route.route_digest }), sideEffectBoundary: "preflight", waitingForApproval: true });
        return this.envelope(validateJob((await this.status(job.job_id)).job), "waiting_approval", resolution.mode, null, null, null, null, null);
      }
      planCompiled = true;
      await this.checkpoint(job.job_id, { phase: "plan_compiled", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, route_digest: plan.route.route_digest, mode: resolution.mode }), sideEffectBoundary: "preflight" });
      if (!approvalReleased) {
        await this.checkpoint(job.job_id, { phase: "provider_approval_required", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, mode: resolution.mode }), sideEffectBoundary: "preflight", waitingForApproval: true });
        return this.envelope(validateJob((await this.status(job.job_id)).job), "waiting_approval", resolution.mode, null, null, null, null, null);
      }
      if (heartbeatError !== null) throw new ProviderRuntimeError("durable brain worker lease heartbeat failed before dispatch", { code: "transport" });
      let approved = this.approvalBoundResolution(resolution);
      if (this.credentialScope) {
        this.assertCredentialFreeResolution(approved);
        credentialBinding = await this.credentialScope.open({ jobId: job.job_id, attempt: job.attempts, approvalReleased: true });
        approved = this.bindCredentialResolution(approved, credentialBinding);
      }
      await this.checkpoint(job.job_id, { phase: "dispatch_started", checkpointDigest: digestJsonSync({ plan_digest: plan.plan_digest, attempt: job.attempts }), sideEffectBoundary: "unknown" });
      executionStarted = true;
      let result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution;
      if (resolution.mode === "execute") {
        if (this.traceStore) {
          const traced = await this.brain.executePlannedWithTrace(plan, resolution.request, { ...(approved.execute ?? {}), traceStore: this.traceStore, runId: `${job.job_id}:attempt-${job.attempts}` });
          result = traced.execution;
          trace = traced.trace;
        } else result = await this.brain.executePlanned(plan, resolution.request, approved.execute ?? {});
      } else if (resolution.mode === "cycle") {
        if (this.traceStore) {
          const traced = await this.brain.executePlannedCycleWithTrace(plan, resolution.request, { ...(approved.cycle ?? {}), traceStore: this.traceStore, runId: `${job.job_id}:attempt-${job.attempts}` });
          result = traced.execution;
          trace = traced.trace;
        } else result = await this.brain.executePlannedCycle(plan, resolution.request, approved.cycle ?? {});
      } else if (this.traceStore) {
        if (!approved.adaptive) throw new ArgumentError("adaptive durable brain job policy disappeared during approval binding");
        const traced = await this.brain.executePlannedAdaptiveCycleWithTrace(plan, resolution.request, { ...approved.adaptive, traceStore: this.traceStore, runId: `${job.job_id}:attempt-${job.attempts}` });
        result = traced.execution;
        trace = traced.trace;
      } else {
        if (!approved.adaptive) throw new ArgumentError("adaptive durable brain job policy disappeared during approval binding");
        result = await this.brain.executePlannedAdaptiveCycle(plan, resolution.request, approved.adaptive);
      }
      if (heartbeatError !== null) throw new ProviderRuntimeError("durable brain worker lease heartbeat failed after dispatch", { code: "transport" });
      const status = result.status;
      if (APPROVAL_STATUSES.has(status)) {
        // The worker admitted the facade behind an unknown boundary before invocation. Even a
        // provider-free review result must not lower that durable boundary back to preflight.
        await this.checkpoint(job.job_id, { phase: status, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: executionStarted ? "unknown" : approvalBoundary(result), waitingForApproval: true });
        return this.envelope(validateJob((await this.status(job.job_id)).job), "waiting_approval", resolution.mode, result, null, null, trace, null);
      }
      if (status === "reconciliation_required") {
        await this.checkpoint(job.job_id, { phase: status, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: "unknown" });
        const quarantined = await this.fail(job, "durable brain execution requires caller reconciliation", false);
        return this.envelope(quarantined, "reconciliation_required", resolution.mode, result, null, null, trace, null);
      }
      if (status !== "completed") {
        await this.checkpoint(job.job_id, { phase: `terminal_${status}`, checkpointDigest: resultDigest(result, trace), sideEffectBoundary: "unknown" });
        const failed = await this.fail(job, `durable brain execution ended with ${status}`, false);
        return this.envelope(failed, failed.state === "reconciliation_required" ? "reconciliation_required" : "failed", resolution.mode, result, null, null, trace, null);
      }
      const completed = await this.complete(job, resultDigest(result, trace));
      return this.envelope(completed, "succeeded", resolution.mode, result, null, null, trace, null);
    } catch (error) {
      const projection = errorProjection(error);
      try {
        const boundary = executionStarted ? "unknown" : planCompiled ? "preflight" : "not_started";
        await this.checkpoint(job.job_id, { phase: "worker_execution_error", checkpointDigest: digestJsonSync({ error_class: projection.errorClass, failure_code: projection.failureCode }), sideEffectBoundary: boundary });
        const retryable = !executionStarted && this.retryPreflightFailures && error instanceof ProviderRuntimeError && error.retryable;
        const failed = await this.fail(job, executionStarted ? "durable brain execution outcome is uncertain; reconciliation required" : retryable ? "durable brain preflight retry scheduled" : "durable brain execution failed before dispatch", retryable);
        return this.envelope(failed, failed.state === "reconciliation_required" ? "reconciliation_required" : failed.state === "queued" ? "retry_scheduled" : "failed", resolution?.mode ?? null, null, null, null, trace, projection);
      } catch (settlementError) {
        const wrapped = new ProviderRuntimeError("durable brain worker failure could not be settled", { code: "configuration" });
        (wrapped as Error & { cause?: unknown }).cause = settlementError;
        throw wrapped;
      }
    } finally {
      if (heartbeatTimer !== null) clearInterval(heartbeatTimer);
      credentialBinding?.close();
    }
  }

  async run(options: { limit?: number } = {}): Promise<AutonomousDurableBrainJobWorkerBatch> {
    const limit = boundedInteger("durable brain worker limit", options.limit ?? 1, 1, MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_BATCH);
    const runs: AutonomousDurableBrainJobWorkerRun[] = [];
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
    const terminal = runs.filter((run) => run.status === "already_terminal").length;
    return {
      schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA,
      worker_id: this.workerId,
      status: runs.length === 0 ? "empty" : failed > 0 && succeeded === 0 && waiting === 0 && reconciliation === 0 && retryScheduled === 0 ? "failed" : waiting > 0 || reconciliation > 0 || retryScheduled > 0 || failed > 0 ? "partial" : "completed",
      runs,
      claimed_count: runs.length - terminal,
      succeeded_count: succeeded,
      waiting_count: waiting,
      reconciliation_count: reconciliation,
      retry_scheduled_count: retryScheduled,
      failed_count: failed,
      already_terminal_count: terminal,
      batch_digest: digestJsonSync(runs.map((run) => ({ job_id: run.job_id, status: run.status, record_digest: run.job.record_digest, trace_digest: run.trace?.trace_digest ?? null }))),
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }

  private async approvalReleased(jobId: string): Promise<boolean> {
    let after = 0;
    for (let page = 0; page < MAX_AUTONOMOUS_DURABLE_BRAIN_WORKER_EVENT_PAGES; page += 1) {
      const result = project<{ events: BrainControlEvent[]; next_after: number }>(await this.apiClient.brainJobEvents({ job_id: jobId, after, limit: 256 }), "brain job events");
      if (result.events.some((event) => event.event_type === "job_approval_granted" || event.event_type === "job_approval_released")) return true;
      if (!Number.isSafeInteger(result.next_after) || result.next_after <= after || result.events.length === 0) return false;
      after = result.next_after;
    }
    return false;
  }

  private async checkpoint(jobId: string, options: { phase: string; checkpointDigest: string; sideEffectBoundary: "not_started" | "preflight" | "dispatched" | "unknown"; waitingForApproval?: boolean }): Promise<BrainJobRecord> {
    const result = project<BrainJobLifecycleResult>(await this.apiClient.brainJobCheckpoint({ job_id: jobId, worker_id: this.workerId, phase: options.phase, checkpoint_digest: options.checkpointDigest, side_effect_boundary: options.sideEffectBoundary, waiting_for_approval: options.waitingForApproval }), "brain job checkpoint");
    return validateJob(result.job);
  }

  private async complete(expected: BrainJobRecord, resultDigestValue: string): Promise<BrainJobRecord> {
    const result = project<BrainJobLifecycleResult>(await this.apiClient.brainJobComplete({ job_id: expected.job_id, worker_id: this.workerId, result_digest: resultDigestValue }), "brain job complete");
    const settled = this.assertSettlement(expected, result.job, "complete");
    if (settled.state !== "succeeded" || settled.result_digest !== resultDigestValue) throw new ProviderRuntimeError("durable brain completion did not persist the exact successful result digest", { code: "protocol" });
    return settled;
  }

  private async fail(expected: BrainJobRecord, reason: string, retryable: boolean): Promise<BrainJobRecord> {
    const result = project<BrainJobLifecycleResult>(await this.apiClient.brainJobFail({ job_id: expected.job_id, worker_id: this.workerId, reason, retryable }), "brain job fail");
    const settled = this.assertSettlement(expected, result.job, "fail");
    if (!["queued", "failed", "dead_lettered", "reconciliation_required"].includes(settled.state)) throw new ProviderRuntimeError("durable brain failure settlement returned a non-terminal or non-queued state", { code: "protocol" });
    return settled;
  }

  private assertSettlement(expected: BrainJobRecord, candidate: unknown, operation: string): BrainJobRecord {
    const settled = validateJob(candidate);
    if (settled.job_id !== expected.job_id || settled.spec_digest !== expected.spec_digest) throw new ProviderRuntimeError(`durable brain ${operation} settlement changed the leased job identity`, { code: "protocol" });
    return settled;
  }

  private assertCredentialFreeResolution(value: AutonomousDurableBrainJobResolution): void {
    const seen = new Set<object>();
    const scan = (candidate: unknown, depth = 0): void => {
      if (!candidate || typeof candidate !== "object") return;
      if (seen.has(candidate)) return;
      if (depth > 16) throw new ArgumentError("durable brain credential policy is too deeply nested");
      seen.add(candidate);
      const row = candidate as Record<string, unknown>;
      if (Object.keys(row).some((key) => key.replaceAll("_", "").toLowerCase() === "credential" || key.replaceAll("_", "").toLowerCase() === "credentials" || key.replaceAll("_", "").toLowerCase() === "credentialfor")) throw new ArgumentError("durable brain credentialScope owns credentials; resolver policy must omit credential fields");
      for (const child of Object.values(row)) scan(child, depth + 1);
    };
    scan(value.execute);
    scan(value.cycle);
    scan(value.adaptive);
  }

  private bindCredentialResolution(value: AutonomousDurableBrainJobResolution, binding: AutonomousCredentialBinding): AutonomousDurableBrainJobResolution {
    const credentialFor = (provider: string) => binding.credentialFor(provider);
    const bindProviderPolicy = (policy: unknown): Record<string, unknown> => {
      const row = isObject(policy) ? { ...policy } : {};
      const planning = isObject(row.providerPlanning) ? { ...row.providerPlanning, credentialFor } : row.providerPlanning;
      return { ...row, credentialFor, ...(planning === undefined ? {} : { providerPlanning: planning }) };
    };
    if (value.mode === "execute") {
      const execute = isObject(value.execute) ? { ...value.execute } : {};
      const run = isObject(execute.run) ? { ...execute.run, credentialFor } : { credentialFor };
      return { ...value, execute: { ...execute, run } as AutonomousBrainJobResolution["execute"] };
    }
    if (value.mode === "cycle") {
      const cycle = isObject(value.cycle) ? { ...value.cycle } : {};
      return { ...value, cycle: { ...cycle, cycle: bindProviderPolicy(cycle.cycle) } as AutonomousBrainJobResolution["cycle"] };
    }
    const adaptive: Record<string, unknown> = isObject(value.adaptive) ? { ...value.adaptive } : {};
    const adaptivePolicy = isObject(adaptive.adaptive) ? adaptive.adaptive : {};
    return { ...value, adaptive: { ...adaptive, adaptive: bindProviderPolicy(adaptivePolicy) } as AutonomousBrainJobResolution["adaptive"] };
  }

  private validateResolution(job: BrainJobRecord, value: AutonomousDurableBrainJobResolution): void {
    if (!isObject(value)) throw new ArgumentError("durable brain job resolver must return an object");
    exactKeys(value, ["specDigest", "policyDigest", "request", "mode", "execute", "cycle", "adaptive"], "durable brain job resolver result");
    const selectedMode = mode(value.mode);
    if (digest("durable brain resolution specDigest", value.specDigest) !== job.spec_digest) throw new ArgumentError("durable brain resolver specDigest does not match the durable job");
    if (autonomousBrainJobSpecDigest({ request: value.request, mode: selectedMode, policyDigest: value.policyDigest ?? null }) !== job.spec_digest) throw new ArgumentError("durable brain request, mode, and policy digest do not match the durable spec");
    if (!isObject(value.request) || typeof value.request.task !== "string" || !value.request.task.trim()) throw new ArgumentError("durable brain resolver request is invalid");
    if (selectedMode === "adaptive" && (!isObject(value.adaptive) || !isObject(value.adaptive.adaptive) || typeof value.adaptive.adaptive.evaluate !== "function")) throw new ArgumentError("adaptive durable brain job requires an evaluator policy");
  }

  private approvalBoundResolution(value: AutonomousDurableBrainJobResolution): AutonomousDurableBrainJobResolution {
    if (value.mode === "execute") return { ...value, execute: { ...(value.execute ?? {}), approveProviderCall: true, run: { ...(value.execute?.run ?? {}), approveProviderCall: true } } };
    if (value.mode === "cycle") return { ...value, cycle: { ...(value.cycle ?? {}), approveProviderCall: true, cycle: { ...(value.cycle?.cycle ?? {}), approveProviderCall: true } } };
    if (!value.adaptive) throw new ArgumentError("adaptive durable brain job policy is missing");
    return { ...value, adaptive: { ...value.adaptive, approveProviderCall: true, adaptive: { ...value.adaptive.adaptive, approveProviderCall: true } } };
  }

  private envelope(job: BrainJobRecord, status: AutonomousDurableBrainJobWorkerStatus, modeValue: AutonomousBrainJobExecutionMode | null, result: AutonomousBrainExecution | AutonomousBrainCycleExecution | AutonomousBrainAdaptiveCycleExecution | null, execution: AutonomousBrainExecution | null, cycle: AutonomousBrainCycleExecution | null, trace: AutonomousRunTraceSummary | null, error: { errorClass: string; failureCode: string; retryable: boolean | null } | null): AutonomousDurableBrainJobWorkerRun {
    const resolvedExecution = execution ?? (result && "run" in result ? result : null);
    const resolvedCycle = cycle ?? (result && "cycle" in result ? result : null);
    const resolvedAdaptive = result && "adaptive" in result ? result : null;
    return { schema: AUTONOMOUS_DURABLE_BRAIN_JOB_WORKER_SCHEMA, worker_id: this.workerId, job_id: job.job_id, status, job, mode: modeValue, execution: resolvedExecution, cycle: resolvedCycle, adaptive: resolvedAdaptive, trace, error_class: error?.errorClass ?? null, failure_code: error?.failureCode ?? null, error_retryable: error?.retryable ?? null, retention: RETENTION, secret_material: SECRET_MATERIAL };
  }
}
