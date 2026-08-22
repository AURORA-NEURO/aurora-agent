import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type {
  BrainControlEvent,
  BrainJobApprovalAction,
  BrainJobApprovalResult,
  BrainJobEventsResult,
  BrainJobRecord,
  BrainJobStatusResult,
  JsonObject,
  JsonValue,
  RestToolResponse,
} from "./types.js";

/** Metadata-only supervision for jobs admitted to the durable brain control plane. */
export const AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA = "bioprism-typescript-autonomous-brain-control-plane-monitor/0.1" as const;
export const MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS = 60_000;
export const MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS = 300_000;
export const MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS = 256;
export const MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS = 256;

const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:+-]+$/;
const TERMINAL_STATES = new Set(["succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"]);
const SECRET_KEYS = new Set(["apikey", "bearer", "credential", "credentials", "password", "privatekey", "prompt", "response", "secret", "token", "messages", "content", "body", "headers"]);

export interface AutonomousBrainControlPlaneClient {
  brainJobStatus(args: { job_id: string }): Promise<RestToolResponse>;
  brainJobEvents(args: { job_id?: string; after?: number; limit?: number }): Promise<RestToolResponse>;
  brainJobApproval(args: { job_id: string; action: BrainJobApprovalAction; reason?: string; authorization_digest?: string }): Promise<RestToolResponse>;
}

export interface AutonomousBrainControlPlaneMonitorOptions {
  client: AutonomousBrainControlPlaneClient;
  clock?: () => number;
  sleep?: (milliseconds: number) => Promise<void>;
}

export interface AutonomousBrainControlPlaneStatus extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA;
  status: BrainJobStatusResult;
  retention: "metadata_only_control_plane_projection";
  secret_material: "never_returned";
}

export interface AutonomousBrainControlPlaneEvents extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA;
  events: BrainJobEventsResult;
  retention: "metadata_only_control_plane_projection";
  secret_material: "never_returned";
}

export interface AutonomousBrainControlPlaneApproval extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA;
  approval: BrainJobApprovalResult;
  retention: "metadata_only_control_plane_projection";
  secret_material: "never_returned";
}

export interface AutonomousBrainControlPlaneWaitOptions {
  until?: readonly string[];
  timeoutMs?: number;
  pollMs?: number;
  maxPolls?: number;
  eventLimit?: number;
  afterEvent?: number;
}

export interface AutonomousBrainControlPlaneWaitResult extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA;
  status: "reached" | "timed_out";
  job_id: string;
  terminal_state: string;
  job: BrainJobRecord;
  events: BrainControlEvent[];
  event_cursor: number;
  polls: number;
  elapsed_ms: number;
  retention: "metadata_only_control_plane_projection";
  secret_material: "never_returned";
}

export interface AutonomousBrainControlPlaneAllStatusResult extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA;
  status: "completed";
  jobs: BrainJobStatusResult[];
  domains: AutonomousDomainName[];
  max_parallel: number;
  retention: "metadata_only_control_plane_projection";
  secret_material: "never_returned";
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !IDENTIFIER.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function positiveInteger(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1 || (value as number) > maximum) throw new ArgumentError(`${name} must be within [1, ${maximum}]`);
  return value as number;
}

function nonnegativeInteger(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) throw new ArgumentError(`${name} must be within [0, ${maximum}]`);
  return value as number;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST.test(value)) throw new ProviderRuntimeError(`${name} returned a malformed digest`, { code: "protocol" });
  return value;
}

function secretFree(value: unknown, depth = 0): void {
  if (depth > 8) throw new ProviderRuntimeError("brain control-plane metadata nesting exceeds its bound", { code: "protocol" });
  if (Array.isArray(value)) {
    if (value.length > 256) throw new ProviderRuntimeError("brain control-plane metadata array exceeds its bound", { code: "protocol" });
    for (const item of value) secretFree(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (SECRET_KEYS.has(key.toLowerCase().replace(/[^a-z0-9]/g, ""))) throw new ProviderRuntimeError("brain control-plane projection contains transient or secret-shaped metadata", { code: "protocol" });
    secretFree(child, depth + 1);
  }
}

function project<T extends JsonValue>(response: RestToolResponse, operation: string): T {
  if (!response || response.ok !== true || !isObject(response.mcp) || response.mcp.error !== undefined || !isObject(response.mcp.result) || response.mcp.result.isError === true) throw new ProviderRuntimeError(`${operation} returned a control-plane refusal`, { code: "protocol" });
  const structured = response.mcp.result.structuredContent;
  if (!isObject(structured)) throw new ProviderRuntimeError(`${operation} returned no structured projection`, { code: "protocol" });
  secretFree(structured);
  return structured as unknown as T;
}

function validateJob(value: unknown): BrainJobRecord {
  if (!isObject(value)) throw new ProviderRuntimeError("brain control-plane job projection is malformed", { code: "protocol" });
  const job = value as unknown as BrainJobRecord;
  identifier("brain control-plane job_id", job.job_id);
  digest("brain control-plane spec_digest", job.spec_digest);
  if (typeof job.domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(job.domain as AutonomousDomainName)) throw new ProviderRuntimeError("brain control-plane job domain is unsupported", { code: "protocol" });
  identifier("brain control-plane capability", job.capability);
  identifier("brain control-plane risk_class", job.risk_class);
  if (typeof job.state !== "string" || !job.state.trim() || job.state.length > 128) throw new ProviderRuntimeError("brain control-plane job state is malformed", { code: "protocol" });
  nonnegativeInteger("brain control-plane attempts", job.attempts, 8);
  positiveInteger("brain control-plane max_attempts", job.max_attempts, 8);
  if (job.attempts > job.max_attempts) throw new ProviderRuntimeError("brain control-plane job attempts exceed its ceiling", { code: "protocol" });
  if (typeof job.side_effect_boundary !== "string" || !job.side_effect_boundary.trim()) throw new ProviderRuntimeError("brain control-plane job side_effect_boundary is malformed", { code: "protocol" });
  if (typeof job.recovered_after_restart !== "boolean") throw new ProviderRuntimeError("brain control-plane job recovery flag is malformed", { code: "protocol" });
  if (job.record_digest !== undefined) digest("brain control-plane record_digest", job.record_digest);
  return job;
}

function validateStatus(value: BrainJobStatusResult): BrainJobStatusResult {
  if (!isObject(value)) throw new ProviderRuntimeError("brain control-plane status projection is malformed", { code: "protocol" });
  validateJob(value.job);
  digest("brain control-plane head_digest", value.head_digest);
  return value;
}

function validateEvents(value: BrainJobEventsResult, limit: number): BrainJobEventsResult {
  if (!isObject(value) || !Array.isArray(value.events) || value.events.length > limit) throw new ProviderRuntimeError("brain control-plane events projection is malformed", { code: "protocol" });
  nonnegativeInteger("brain control-plane event after", value.after, Number.MAX_SAFE_INTEGER);
  nonnegativeInteger("brain control-plane event next_after", value.next_after, Number.MAX_SAFE_INTEGER);
  digest("brain control-plane event head_digest", value.head_digest);
  for (const event of value.events) {
    if (!isObject(event) || !Number.isSafeInteger(event.sequence) || event.sequence < 1 || typeof event.event_type !== "string" || typeof event.job_id !== "string") throw new ProviderRuntimeError("brain control-plane event row is malformed", { code: "protocol" });
    digest("brain control-plane event digest", event.event_digest);
    if (event.previous_digest !== "") digest("brain control-plane event previous_digest", event.previous_digest);
  }
  return value;
}

/**
 * Observes and controls the public brain job API without claiming it is a provider worker.
 * Tasks, prompts, credentials, and provider responses never cross this boundary.
 */
export class AutonomousBrainControlPlaneMonitor {
  readonly client: AutonomousBrainControlPlaneClient;
  readonly clock: () => number;
  readonly sleep: (milliseconds: number) => Promise<void>;

  constructor(options: AutonomousBrainControlPlaneMonitorOptions) {
    if (!options || !options.client || typeof options.client.brainJobStatus !== "function" || typeof options.client.brainJobEvents !== "function" || typeof options.client.brainJobApproval !== "function") throw new ArgumentError("brain control-plane monitor requires status, events, and approval client methods");
    this.client = options.client;
    this.clock = options.clock ?? (() => Date.now());
    this.sleep = options.sleep ?? ((milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)));
  }

  async status(jobId: string): Promise<AutonomousBrainControlPlaneStatus> {
    const normalizedJobId = identifier("brain control-plane jobId", jobId);
    const value = validateStatus(project<BrainJobStatusResult>(await this.client.brainJobStatus({ job_id: normalizedJobId }), "brain_job_status"));
    return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, status: value, retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
  }

  async events(jobId: string | undefined = undefined, after = 0, limit = 100): Promise<AutonomousBrainControlPlaneEvents> {
    const normalizedJobId = jobId === undefined ? undefined : identifier("brain control-plane jobId", jobId);
    const boundedAfter = nonnegativeInteger("brain control-plane after", after, Number.MAX_SAFE_INTEGER);
    const boundedLimit = positiveInteger("brain control-plane event limit", limit, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS);
    const value = validateEvents(project<BrainJobEventsResult>(await this.client.brainJobEvents({ job_id: normalizedJobId, after: boundedAfter, limit: boundedLimit }), "brain_job_events"), boundedLimit);
    return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, events: value, retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
  }

  async approval(jobId: string, action: BrainJobApprovalAction, options: { reason?: string; authorizationDigest?: string } = {}): Promise<AutonomousBrainControlPlaneApproval> {
    const normalizedJobId = identifier("brain control-plane jobId", jobId);
    if (!(action === "request" || action === "approve" || action === "deny")) throw new ArgumentError("brain control-plane approval action is invalid");
    if ((action === "approve" || action === "deny") && (typeof options.authorizationDigest !== "string" || !DIGEST.test(options.authorizationDigest))) throw new ArgumentError("brain control-plane approval decision requires authorizationDigest");
    if (options.reason !== undefined && (typeof options.reason !== "string" || !options.reason.trim() || options.reason.length > 2_048)) throw new ArgumentError("brain control-plane approval reason is outside its bounds");
    const value = project<BrainJobApprovalResult>(await this.client.brainJobApproval({ job_id: normalizedJobId, action, reason: options.reason, authorization_digest: options.authorizationDigest }), "brain_job_approval");
    if (!isObject(value) || !isObject(value.job)) throw new ProviderRuntimeError("brain_job_approval returned a malformed job", { code: "protocol" });
    validateJob(value.job);
    return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, approval: value, retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
  }

  async statusAll(jobIds: readonly string[], options: { maxParallel?: number } = {}): Promise<AutonomousBrainControlPlaneAllStatusResult> {
    if (!Array.isArray(jobIds) || jobIds.length < 1 || jobIds.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("brain control-plane statusAll requires a bounded non-empty job list");
    const normalized = jobIds.map((jobId) => identifier("brain control-plane jobId", jobId));
    if (new Set(normalized).size !== normalized.length) throw new ArgumentError("brain control-plane statusAll job IDs must be unique");
    const maxParallel = positiveInteger("brain control-plane maxParallel", options.maxParallel ?? 4, AUTONOMOUS_DOMAIN_NAMES.length);
    const statuses: BrainJobStatusResult[] = [];
    for (let offset = 0; offset < normalized.length; offset += maxParallel) {
      const page = await Promise.all(normalized.slice(offset, offset + maxParallel).map(async (jobId) => (await this.status(jobId)).status));
      statuses.push(...page);
    }
    const domains = statuses.map((status) => status.job.domain as AutonomousDomainName);
    return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, status: "completed", jobs: statuses, domains, max_parallel: maxParallel, retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
  }

  async wait(jobId: string, options: AutonomousBrainControlPlaneWaitOptions = {}): Promise<AutonomousBrainControlPlaneWaitResult> {
    const normalizedJobId = identifier("brain control-plane jobId", jobId);
    const timeoutMs = positiveInteger("brain control-plane timeoutMs", options.timeoutMs ?? MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS);
    const pollMs = positiveInteger("brain control-plane pollMs", options.pollMs ?? 1_000, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS);
    const maxPolls = positiveInteger("brain control-plane maxPolls", options.maxPolls ?? Math.min(MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS, Math.ceil(timeoutMs / pollMs) + 1), MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS);
    const eventLimit = positiveInteger("brain control-plane eventLimit", options.eventLimit ?? 100, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS);
    const after = nonnegativeInteger("brain control-plane afterEvent", options.afterEvent ?? 0, Number.MAX_SAFE_INTEGER);
    const targets = new Set(options.until ?? []);
    for (const target of targets) if (typeof target !== "string" || !target.trim() || target.length > 128) throw new ArgumentError("brain control-plane wait target state is invalid");
    const started = this.clock();
    let cursor = after;
    const collected = new Map<number, BrainControlEvent>();
    let latest: BrainJobRecord | null = null;
    let polls = 0;
    for (; polls < maxPolls; polls += 1) {
      const status = (await this.status(normalizedJobId)).status;
      latest = status.job;
      const page = (await this.events(normalizedJobId, cursor, eventLimit)).events;
      cursor = page.next_after;
      for (const event of page.events) collected.set(event.sequence, event);
      if (targets.has(latest.state) || (targets.size === 0 && TERMINAL_STATES.has(latest.state))) {
        return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, status: "reached", job_id: normalizedJobId, terminal_state: latest.state, job: latest, events: [...collected.values()].sort((left, right) => left.sequence - right.sequence), event_cursor: cursor, polls: polls + 1, elapsed_ms: Math.max(0, this.clock() - started), retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
      }
      if (this.clock() - started >= timeoutMs) break;
      await this.sleep(pollMs);
    }
    if (latest === null) throw new ProviderRuntimeError("brain control-plane wait ended without a status projection", { code: "protocol" });
    return { schema: AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA, status: "timed_out", job_id: normalizedJobId, terminal_state: latest.state, job: latest, events: [...collected.values()].sort((left, right) => left.sequence - right.sequence), event_cursor: cursor, polls, elapsed_ms: Math.max(0, this.clock() - started), retention: "metadata_only_control_plane_projection", secret_material: "never_returned" };
  }
}
