import { ArgumentError, CredentialError, ProviderRuntimeError, type ProviderErrorCode } from "./errors.js";
import {
  acceptedCrossDomainPlan,
  AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN,
  AUTONOMOUS_CROSS_DOMAIN_SCHEMA,
  type AutonomousAcceptedCrossDomainPlan,
  type AutonomousAgent,
  type AutonomousCrossDomainBlueprint,
  type AutonomousCrossDomainChildRun,
  type AutonomousCrossDomainRunOptions,
  type AutonomousDomainName,
  type AutonomousPromptChunk,
  type AutonomousRouteProposal,
  type AutonomousRunResult,
} from "./autonomous.js";
import type { AutonomousLearningController } from "./autonomous-learning.js";
import { AutonomousCostBudget, type AutonomousModelCandidate } from "./llm.js";
import { digestJson } from "./tooling.js";
import type { AutonomousCrossDomainPlanRefinementResult, JsonObject } from "./types.js";

/** Durable cross-domain execution is deliberately separate from the one-shot fan-out API. */
export const AUTONOMOUS_CROSS_DOMAIN_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-cross-domain-execution/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-checkpoint/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_EVENT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-event/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-snapshot/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_EXECUTION_CONTRACT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-execution-contract/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_MAX_STEPS_PER_CALL = AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN + 1;
export const AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS = 256;
export const AUTONOMOUS_CROSS_DOMAIN_MAX_JOBS = 1_024;
export const AUTONOMOUS_CROSS_DOMAIN_MAX_SNAPSHOT_BYTES = 4 * 1024 * 1024;

export type AutonomousCrossDomainCheckpointStatus = "children_pending" | "synthesis_pending" | "paused" | "completed" | "failed";
export type AutonomousCrossDomainExecutionStatus = "completed" | "paused" | "approval_required" | "reconciliation_required" | "failed" | "route_review_required";
export type AutonomousCrossDomainEventType = "started" | "child_completed" | "checkpointed" | "approval_required" | "reconciliation_required" | "synthesis_completed" | "failed" | "completed";

export interface AutonomousCrossDomainCheckpoint {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA;
  job_id: string;
  task_digest: string;
  route_digest: string;
  base_plan_digest: string;
  execution_child_ids: string[];
  completed_child_ids: string[];
  child_result_digests: Record<string, string>;
  next_child_id: string | null;
  plan_refinement_digest: string | null;
  execution_contract_digest?: string | null;
  synthesis_result_digest: string | null;
  generation: number;
  status: AutonomousCrossDomainCheckpointStatus;
  previous_checkpoint_digest: string | null;
  checkpoint_digest: string;
  retention: "metadata_only;task_prompt_response_and_credentials_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousCrossDomainEvent {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_EVENT_SCHEMA;
  sequence: number;
  job_id: string;
  event_type: AutonomousCrossDomainEventType;
  item_id: string | null;
  phase: "child" | "synthesis" | "lifecycle";
  checkpoint_digest: string;
  previous_event_digest: string | null;
  event_digest: string;
  retention: "metadata_only;provider_payloads_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousCrossDomainCheckpointStore {
  load(jobId: string): Promise<AutonomousCrossDomainCheckpoint | null> | AutonomousCrossDomainCheckpoint | null;
  save(checkpoint: AutonomousCrossDomainCheckpoint): Promise<void> | void;
  appendEvent(event: AutonomousCrossDomainEvent): Promise<void> | void;
  events(jobId: string, after?: number, limit?: number): Promise<AutonomousCrossDomainEvent[]> | AutonomousCrossDomainEvent[];
}

export interface AutonomousCrossDomainCheckpointStoreSnapshot {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA;
  checkpoints: AutonomousCrossDomainCheckpoint[];
  event_rows: Array<{ job_id: string; events: AutonomousCrossDomainEvent[] }>;
  retention: "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousCrossDomainSnapshotStore extends AutonomousCrossDomainCheckpointStore {
  snapshot(): Promise<AutonomousCrossDomainCheckpointStoreSnapshot>;
  restore(snapshot: AutonomousCrossDomainCheckpointStoreSnapshot): Promise<void>;
  verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA; verified: true; jobs: number; events: number; snapshot_digest: string; retention: "metadata_only" }>;
}

export interface AutonomousCrossDomainSnapshotPersistence {
  read(): Promise<AutonomousCrossDomainCheckpointStoreSnapshot | null> | AutonomousCrossDomainCheckpointStoreSnapshot | null;
  write(snapshot: AutonomousCrossDomainCheckpointStoreSnapshot): Promise<void> | void;
}

/** A result or child envelope retained by the caller and rehydrated before another step. */
export type AutonomousCrossDomainRehydratableChild = AutonomousRunResult | AutonomousCrossDomainChildRun;

export interface AutonomousCrossDomainChildResultResolver {
  (childId: string, checkpoint: AutonomousCrossDomainCheckpoint): Promise<AutonomousCrossDomainRehydratableChild | null> | AutonomousCrossDomainRehydratableChild | null;
}

export interface AutonomousCrossDomainStepResult {
  phase: "child" | "synthesis";
  item_id: string;
  run: AutonomousRunResult;
  output_digest: string | null;
  output_bytes: number;
  execution_child_ids: string[];
  completed_child_ids: string[];
  child_result_digests: Record<string, string>;
  plan_refinement_digest: string | null;
  learning_episode_id: string | null;
}

export interface AutonomousCrossDomainExecutionResult {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_EXECUTION_SCHEMA;
  status: AutonomousCrossDomainExecutionStatus;
  job_id: string | null;
  route: AutonomousRouteProposal | null;
  blueprint: AutonomousCrossDomainBlueprint | null;
  checkpoint: AutonomousCrossDomainCheckpoint | null;
  events: AutonomousCrossDomainEvent[];
  step_results: AutonomousCrossDomainStepResult[];
  synthesis: AutonomousRunResult | null;
  completed_children: number;
  total_children: number;
  plan_refinement_digest: string | null;
  error: AutonomousCrossDomainErrorMetadata | null;
  learning_episode_ids: string[];
  recovery: "caller_rehydrates_task_credentials_and_completed_child_results";
  retention: "provider_responses_local;checkpoint_metadata_and_outcome_digests_only";
}

export interface AutonomousCrossDomainErrorMetadata {
  error_class: string;
  error_code: ProviderErrorCode | null;
  retryable: boolean | null;
  status_code: number | null;
}

export interface AutonomousCrossDomainExecuteOptions extends AutonomousCrossDomainRunOptions {
  jobId?: string;
  maxSteps?: number;
  blueprint?: AutonomousCrossDomainBlueprint;
  /** Alias matching the single-domain workflow executor. */
  acceptedPlanRefinement?: AutonomousCrossDomainPlanRefinementResult;
  /** Explicitly bind a legacy checkpoint that predates execution-contract digests. */
  rebindLegacyExecutionContract?: boolean;
  /** Rehydrate raw child results from caller-owned durable storage before the next step. */
  resolveChildResult?: AutonomousCrossDomainChildResultResolver;
}

export interface AutonomousCrossDomainExecutorOptions {
  learning?: AutonomousLearningController;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, allowed: readonly string[], label: string): void {
  const allowedKeys = new Set(allowed);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError(`${label} contains unsupported fields`);
}

function boundedId(value: unknown, label: string, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${label} must be a bounded identifier`);
  return value;
}

function boundedTask(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 32_000) throw new ArgumentError("cross-domain task must be a bounded non-empty string");
  return value;
}

function digest(value: unknown, label: string, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${label} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedInteger(value: unknown, label: string, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${label} is outside its bounded integer contract`);
  return value as number;
}

function childIds(value: unknown, label: string): string[] {
  if (!Array.isArray(value) || value.length < 2 || value.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN) throw new ArgumentError(`${label} must contain between 2 and ${AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN} children`);
  const ids = value.map((item) => boundedId(item, `${label} child id`, 256));
  if (new Set(ids).size !== ids.length) throw new ArgumentError(`${label} must contain unique child IDs`);
  return ids;
}

function checkpointDescriptor(value: Omit<AutonomousCrossDomainCheckpoint, "checkpoint_digest">): Omit<AutonomousCrossDomainCheckpoint, "checkpoint_digest"> {
  return {
    ...value,
    execution_child_ids: [...value.execution_child_ids],
    completed_child_ids: [...value.completed_child_ids],
    child_result_digests: { ...value.child_result_digests },
  };
}

async function validateCheckpoint(value: unknown): Promise<AutonomousCrossDomainCheckpoint> {
  if (!isObject(value)) throw new ArgumentError("cross-domain checkpoint must be an object");
  exactKeys(value, ["schema", "job_id", "task_digest", "route_digest", "base_plan_digest", "execution_child_ids", "completed_child_ids", "child_result_digests", "next_child_id", "plan_refinement_digest", "execution_contract_digest", "synthesis_result_digest", "generation", "status", "previous_checkpoint_digest", "checkpoint_digest", "retention", "secret_material"], "cross-domain checkpoint");
  if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA || value.retention !== "metadata_only;task_prompt_response_and_credentials_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("cross-domain checkpoint metadata markers are invalid");
  const jobId = boundedId(value.job_id, "cross-domain checkpoint job_id");
  const taskDigest = digest(value.task_digest, "cross-domain checkpoint task_digest")!;
  const routeDigest = digest(value.route_digest, "cross-domain checkpoint route_digest")!;
  const basePlanDigest = digest(value.base_plan_digest, "cross-domain checkpoint base_plan_digest")!;
  const execution = childIds(value.execution_child_ids, "cross-domain checkpoint execution_child_ids");
  if (!Array.isArray(value.completed_child_ids) || value.completed_child_ids.length > execution.length) throw new ArgumentError("cross-domain checkpoint completed_child_ids exceed their bound");
  const completed = value.completed_child_ids.map((item) => boundedId(item, "cross-domain checkpoint completed child id"));
  if (new Set(completed).size !== completed.length || completed.some((id, index) => id !== execution[index])) throw new ArgumentError("cross-domain checkpoint completed children must be the ordered execution prefix");
  if (!isObject(value.child_result_digests)) throw new ArgumentError("cross-domain checkpoint child_result_digests must be an object");
  const resultDigests: Record<string, string> = {};
  for (const [childId, resultDigest] of Object.entries(value.child_result_digests)) {
    if (!completed.includes(childId)) throw new ArgumentError("cross-domain checkpoint result digests contain an unknown or incomplete child");
    resultDigests[childId] = digest(resultDigest, `cross-domain checkpoint result digest for ${childId}`)!;
  }
  if (Object.keys(resultDigests).length !== completed.length) throw new ArgumentError("cross-domain checkpoint result digests must match completed children");
  const nextChildId = value.next_child_id === null ? null : boundedId(value.next_child_id, "cross-domain checkpoint next_child_id");
  const expectedNext = completed.length < execution.length ? execution[completed.length]! : null;
  if (nextChildId !== expectedNext) throw new ArgumentError("cross-domain checkpoint next_child_id is not the next ordered child");
  const planRefinementDigest = digest(value.plan_refinement_digest, "cross-domain checkpoint plan_refinement_digest", true);
  const executionContractDigest = value.execution_contract_digest === undefined ? null : digest(value.execution_contract_digest, "cross-domain checkpoint execution_contract_digest", true);
  const synthesisResultDigest = digest(value.synthesis_result_digest, "cross-domain checkpoint synthesis_result_digest", true);
  if (synthesisResultDigest !== null && completed.length !== execution.length) throw new ArgumentError("cross-domain checkpoint cannot contain synthesis before all children");
  const generation = boundedInteger(value.generation, "cross-domain checkpoint generation", Number.MAX_SAFE_INTEGER, 1);
  const status = value.status;
  if (status !== "children_pending" && status !== "synthesis_pending" && status !== "paused" && status !== "completed" && status !== "failed") throw new ArgumentError("cross-domain checkpoint status is invalid");
  if (status === "completed" && synthesisResultDigest === null) throw new ArgumentError("completed cross-domain checkpoint must contain synthesis digest");
  if (status === "completed" && nextChildId !== null) throw new ArgumentError("completed cross-domain checkpoint cannot have a next child");
  if (status === "synthesis_pending" && (completed.length !== execution.length || nextChildId !== null)) throw new ArgumentError("synthesis_pending checkpoint must have all children and no next child");
  const previous = digest(value.previous_checkpoint_digest, "cross-domain checkpoint previous_checkpoint_digest", true);
  const descriptor = checkpointDescriptor({
    schema: AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA,
    job_id: jobId,
    task_digest: taskDigest,
    route_digest: routeDigest,
    base_plan_digest: basePlanDigest,
    execution_child_ids: execution,
    completed_child_ids: completed,
    child_result_digests: resultDigests,
    next_child_id: nextChildId,
    plan_refinement_digest: planRefinementDigest,
    execution_contract_digest: executionContractDigest,
    synthesis_result_digest: synthesisResultDigest,
    generation,
    status,
    previous_checkpoint_digest: previous,
    retention: "metadata_only;task_prompt_response_and_credentials_not_retained",
    secret_material: "never_returned",
  });
  const checkpointDigest = digest(value.checkpoint_digest, "cross-domain checkpoint checkpoint_digest")!;
  if (checkpointDigest !== await digestJson(descriptor)) throw new ArgumentError("cross-domain checkpoint digest does not match its contents");
  return { ...descriptor, checkpoint_digest: checkpointDigest };
}

async function validateEvent(value: unknown): Promise<AutonomousCrossDomainEvent> {
  if (!isObject(value)) throw new ArgumentError("cross-domain event must be an object");
  exactKeys(value, ["schema", "sequence", "job_id", "event_type", "item_id", "phase", "checkpoint_digest", "previous_event_digest", "event_digest", "retention", "secret_material"], "cross-domain event");
  if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_EVENT_SCHEMA || value.retention !== "metadata_only;provider_payloads_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("cross-domain event metadata markers are invalid");
  const sequence = boundedInteger(value.sequence, "cross-domain event sequence", Number.MAX_SAFE_INTEGER, 1);
  const jobId = boundedId(value.job_id, "cross-domain event job_id");
  const eventType = value.event_type;
  if (!["started", "child_completed", "checkpointed", "approval_required", "reconciliation_required", "synthesis_completed", "failed", "completed"].includes(String(eventType))) throw new ArgumentError("cross-domain event type is invalid");
  const itemId = value.item_id === null ? null : boundedId(value.item_id, "cross-domain event item_id");
  const phase = value.phase;
  if (phase !== "child" && phase !== "synthesis" && phase !== "lifecycle") throw new ArgumentError("cross-domain event phase is invalid");
  const checkpointDigest = digest(value.checkpoint_digest, "cross-domain event checkpoint_digest")!;
  const previousEventDigest = digest(value.previous_event_digest, "cross-domain event previous_event_digest", true);
  const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_EVENT_SCHEMA, sequence, job_id: jobId, event_type: eventType, item_id: itemId, phase: phase as "child" | "synthesis" | "lifecycle", checkpoint_digest: checkpointDigest, previous_event_digest: previousEventDigest, retention: "metadata_only;provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
  const eventDigest = digest(value.event_digest, "cross-domain event event_digest")!;
  if (eventDigest !== await digestJson(descriptor)) throw new ArgumentError("cross-domain event digest does not match its contents");
  return { ...descriptor, event_type: eventType as AutonomousCrossDomainEventType, event_digest: eventDigest };
}

async function validateSnapshot(value: unknown): Promise<AutonomousCrossDomainCheckpointStoreSnapshot> {
  if (!isObject(value)) throw new ArgumentError("cross-domain snapshot must be an object");
  exactKeys(value, ["schema", "checkpoints", "event_rows", "retention", "secret_material", "snapshot_digest"], "cross-domain snapshot");
  if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA || value.retention !== "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("cross-domain snapshot metadata markers are invalid");
  if (!Array.isArray(value.checkpoints) || value.checkpoints.length > AUTONOMOUS_CROSS_DOMAIN_MAX_JOBS) throw new ArgumentError("cross-domain snapshot checkpoints exceed their bound");
  const checkpoints: AutonomousCrossDomainCheckpoint[] = [];
  const jobIds = new Set<string>();
  for (const candidate of value.checkpoints) {
    const checkpoint = await validateCheckpoint(candidate);
    if (!jobIds.add(checkpoint.job_id)) throw new ArgumentError("cross-domain snapshot contains duplicate jobs");
    checkpoints.push(checkpoint);
  }
  if (!Array.isArray(value.event_rows) || value.event_rows.length > checkpoints.length) throw new ArgumentError("cross-domain snapshot event rows are malformed");
  const eventRows: Array<{ job_id: string; events: AutonomousCrossDomainEvent[] }> = [];
  const eventJobs = new Set<string>();
  for (const rawRow of value.event_rows) {
    if (!isObject(rawRow)) throw new ArgumentError("cross-domain snapshot event row must be an object");
    exactKeys(rawRow, ["job_id", "events"], "cross-domain snapshot event row");
    const jobId = boundedId(rawRow.job_id, "cross-domain snapshot event row job_id");
    if (!jobIds.has(jobId) || !eventJobs.add(jobId) || !Array.isArray(rawRow.events) || rawRow.events.length > AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS) throw new ArgumentError("cross-domain snapshot event row is invalid");
    const events: AutonomousCrossDomainEvent[] = [];
    let previous: AutonomousCrossDomainEvent | undefined;
    for (const candidate of rawRow.events) {
      const event = await validateEvent(candidate);
      if (event.job_id !== jobId || (previous && (event.sequence !== previous.sequence + 1 || event.previous_event_digest !== previous.event_digest)) || (!previous && event.sequence !== 1 && event.previous_event_digest === null)) throw new ArgumentError("cross-domain snapshot event chain is not contiguous");
      events.push(event);
      previous = event;
    }
    eventRows.push({ job_id: jobId, events });
  }
  const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA, checkpoints: checkpoints.map((checkpoint) => structuredClone(checkpoint)), event_rows: eventRows.map((row) => ({ job_id: row.job_id, events: row.events.map((event) => structuredClone(event)) })), retention: "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
  const snapshotDigest = digest(value.snapshot_digest, "cross-domain snapshot snapshot_digest")!;
  if (snapshotDigest !== await digestJson(descriptor)) throw new ArgumentError("cross-domain snapshot digest does not match its contents");
  if (new TextEncoder().encode(JSON.stringify(value)).byteLength > AUTONOMOUS_CROSS_DOMAIN_MAX_SNAPSHOT_BYTES) throw new ArgumentError("cross-domain snapshot exceeds its byte capacity");
  return { ...descriptor, snapshot_digest: snapshotDigest };
}

/** A bounded process-local store; production callers can replace it with SQLite, Redis, or a database adapter. */
export class InMemoryAutonomousCrossDomainCheckpointStore implements AutonomousCrossDomainSnapshotStore {
  private readonly checkpoints = new Map<string, AutonomousCrossDomainCheckpoint>();
  private readonly eventRows = new Map<string, AutonomousCrossDomainEvent[]>();

  load(jobId: string): AutonomousCrossDomainCheckpoint | null {
    const checkpoint = this.checkpoints.get(boundedId(jobId, "cross-domain jobId"));
    return checkpoint ? structuredClone(checkpoint) : null;
  }

  async save(checkpoint: AutonomousCrossDomainCheckpoint): Promise<void> {
    const normalized = await validateCheckpoint(checkpoint);
    const previous = this.checkpoints.get(normalized.job_id);
    if (!previous) {
      if (normalized.generation !== 1 || normalized.previous_checkpoint_digest !== null) throw new ArgumentError("cross-domain initial checkpoint must start at generation one");
    } else if (previous.checkpoint_digest !== normalized.checkpoint_digest && (normalized.generation !== previous.generation + 1 || normalized.previous_checkpoint_digest !== previous.checkpoint_digest)) {
      throw new ArgumentError("cross-domain checkpoint generation is not contiguous");
    }
    if (this.checkpoints.size >= AUTONOMOUS_CROSS_DOMAIN_MAX_JOBS && !previous) throw new ArgumentError("cross-domain job capacity is exhausted");
    this.checkpoints.set(normalized.job_id, structuredClone(normalized));
  }

  async appendEvent(event: AutonomousCrossDomainEvent): Promise<void> {
    const normalized = await validateEvent(event);
    if (!this.checkpoints.has(normalized.job_id)) throw new ArgumentError("cross-domain event requires an existing checkpoint");
    const rows = this.eventRows.get(normalized.job_id) ?? [];
    const prior = rows.at(-1);
    if (prior && normalized.sequence === prior.sequence && normalized.event_digest === prior.event_digest) return;
    if (prior && (normalized.sequence !== prior.sequence + 1 || normalized.previous_event_digest !== prior.event_digest)) throw new ArgumentError("cross-domain event sequence or predecessor digest is invalid");
    if (!prior && normalized.sequence !== 1) throw new ArgumentError("cross-domain event sequence must start at one");
    rows.push(structuredClone(normalized));
    if (rows.length > AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS) rows.splice(0, rows.length - AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS);
    this.eventRows.set(normalized.job_id, rows);
  }

  events(jobId: string, after = 0, limit = AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS): AutonomousCrossDomainEvent[] {
    const normalizedJobId = boundedId(jobId, "cross-domain jobId");
    boundedInteger(after, "cross-domain event after", Number.MAX_SAFE_INTEGER);
    boundedInteger(limit, "cross-domain event limit", AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS, 1);
    return (this.eventRows.get(normalizedJobId) ?? []).filter((event) => event.sequence > after).slice(0, limit).map((event) => structuredClone(event));
  }

  async snapshot(): Promise<AutonomousCrossDomainCheckpointStoreSnapshot> {
    const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA, checkpoints: [...this.checkpoints.values()].sort((left, right) => left.job_id.localeCompare(right.job_id)).map((checkpoint) => structuredClone(checkpoint)), event_rows: [...this.eventRows.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([job_id, events]) => ({ job_id, events: events.map((event) => structuredClone(event)) })), retention: "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
    const snapshot = { ...descriptor, snapshot_digest: await digestJson(descriptor) };
    return (await validateSnapshot(snapshot));
  }

  async restore(snapshot: AutonomousCrossDomainCheckpointStoreSnapshot): Promise<void> {
    const validated = await validateSnapshot(snapshot);
    this.checkpoints.clear();
    this.eventRows.clear();
    for (const checkpoint of validated.checkpoints) this.checkpoints.set(checkpoint.job_id, structuredClone(checkpoint));
    for (const row of validated.event_rows) this.eventRows.set(row.job_id, row.events.map((event) => structuredClone(event)));
  }

  async verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA; verified: true; jobs: number; events: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await this.snapshot();
    return { schema: AUTONOMOUS_CROSS_DOMAIN_SNAPSHOT_SCHEMA, verified: true, jobs: snapshot.checkpoints.length, events: snapshot.event_rows.reduce((total, row) => total + row.events.length, 0), snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }
}

export class AutonomousCrossDomainPersistenceCoordinator {
  constructor(readonly store: AutonomousCrossDomainSnapshotStore, readonly persistence: AutonomousCrossDomainSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("cross-domain persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("cross-domain persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousCrossDomainCheckpointStoreSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousCrossDomainCheckpointStoreSnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

function responseText(run: AutonomousRunResult | null): string {
  if (!run?.response) return "";
  return run.response.text || (run.response.structured === null || run.response.structured === undefined ? "" : JSON.stringify(run.response.structured));
}

function safeErrorClass(error: unknown): string {
  const name = error instanceof Error && typeof error.constructor?.name === "string" ? error.constructor.name : "UnknownError";
  return /^[A-Za-z0-9_.-]{1,128}$/.test(name) ? name : "UnknownError";
}

function errorMetadata(error: unknown): AutonomousCrossDomainErrorMetadata {
  if (error instanceof ProviderRuntimeError) return { error_class: error.name, error_code: error.code, retryable: error.retryable, status_code: error.statusCode ?? null };
  if (error instanceof CredentialError) return { error_class: error.name, error_code: "credential", retryable: false, status_code: null };
  return { error_class: safeErrorClass(error), error_code: null, retryable: null, status_code: null };
}

function boundedSteps(value: unknown, total: number): number {
  if (value === undefined) return 1;
  if (!Number.isSafeInteger(value) || (value as number) < 1 || (value as number) > Math.min(AUTONOMOUS_CROSS_DOMAIN_MAX_STEPS_PER_CALL, total + 1)) throw new ArgumentError(`cross-domain maxSteps must be between 1 and ${Math.min(AUTONOMOUS_CROSS_DOMAIN_MAX_STEPS_PER_CALL, total + 1)}`);
  return value as number;
}

function validateDurableOptions(options: AutonomousCrossDomainExecuteOptions): void {
  if (options.maxParallelChildren !== undefined && options.maxParallelChildren !== 1) throw new ArgumentError("durable cross-domain execution is sequential; maxParallelChildren must be 1 or omitted");
  if (options.allowPartial === true) throw new ArgumentError("durable cross-domain execution does not synthesize partial children; use synthesize: false and resume the failed child explicitly");
}

function crossPlanRefinement(options: AutonomousCrossDomainExecuteOptions): AutonomousCrossDomainPlanRefinementResult | undefined {
  if (options.acceptedPlanRefinement !== undefined && options.acceptedCrossDomainPlanRefinement !== undefined) throw new ArgumentError("acceptedPlanRefinement and acceptedCrossDomainPlanRefinement cannot both be supplied");
  return options.acceptedPlanRefinement ?? options.acceptedCrossDomainPlanRefinement;
}

function candidateContract(candidate: AutonomousModelCandidate): JsonObject {
  return { provider: candidate.provider, model: candidate.model, capabilities: candidate.capabilities ? [...candidate.capabilities] : null, context_window_tokens: candidate.context_window_tokens ?? null, max_output_tokens: candidate.max_output_tokens ?? null, quality: candidate.quality ?? null, latency_ms: candidate.latency_ms ?? null, cost_per_million_tokens: candidate.cost_per_million_tokens ?? null, reliability: candidate.reliability ?? null };
}

async function executionContractDigest(agent: AutonomousAgent, options: AutonomousCrossDomainExecuteOptions): Promise<string> {
  const candidates = options.candidates ? [...options.candidates] : agent.models();
  const responseSchemaDigest = options.responseSchema === undefined ? null : await digestJson(options.responseSchema);
  const toolsDigest = options.tools === undefined ? null : await digestJson(options.tools.map((tool) => ({ name: tool.name, description: tool.description, parameters: tool.parameters })));
  const executionPolicyDigest = options.execution ? await options.execution.policy.digest() : null;
  return digestJson({ schema: AUTONOMOUS_CROSS_DOMAIN_EXECUTION_CONTRACT_SCHEMA, candidates_digest: await digestJson(candidates.map(candidateContract)), max_input_tokens: options.maxInputTokens ?? null, max_output_tokens: options.maxOutputTokens ?? null, max_cost_per_million_tokens: options.maxCostPerMillionTokens ?? null, max_latency_ms: options.maxLatencyMs ?? null, min_quality: options.minQuality ?? null, max_total_cost_units: options.maxTotalCostUnits ?? null, cost_budget_max: options.costBudget?.maxCostUnits ?? null, require_json: options.requireJson === true, response_schema_digest: responseSchemaDigest, tools_digest: toolsDigest, approve_effects: options.approveEffects === true, max_provider_failovers: options.maxProviderFailovers ?? null, execution_policy_digest: executionPolicyDigest });
}

function validCrossBlueprint(value: unknown): value is AutonomousCrossDomainBlueprint {
  if (!isObject(value) || value.schema !== AUTONOMOUS_CROSS_DOMAIN_SCHEMA || typeof value.task_digest !== "string" || typeof value.plan_digest !== "string" || !Array.isArray(value.child_ids) || !Array.isArray(value.child_blueprints) || !isObject(value.synthesis_blueprint)) return false;
  if (value.child_ids.length < 2 || value.child_ids.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN || value.child_ids.some((id) => typeof id !== "string" || !id.trim()) || new Set(value.child_ids).size !== value.child_ids.length) return false;
  return value.child_blueprints.length === value.child_ids.length;
}

function childTaskMessage(blueprint: AutonomousCrossDomainBlueprint, childId: string): { task: string; domain: AutonomousDomainName; capability: string } {
  const index = blueprint.child_ids.indexOf(childId);
  const child = blueprint.child_blueprints[index];
  if (!child) throw new ProviderRuntimeError(`cross-domain checkpoint references missing child ${childId}`);
  const message = child.prompt.messages.find((candidate) => candidate.source_id === "task");
  if (!message) throw new ProviderRuntimeError(`cross-domain child ${childId} has no bounded task message`);
  return { task: message.content, domain: child.domain_profile.domain, capability: child.selection_context.capability };
}

function childOutput(childId: string, child: AutonomousRunResult, domain: AutonomousDomainName): { id: string; domain: AutonomousDomainName; status: string; output: string } {
  const raw = responseText(child);
  const output = (raw.length > 48_000 ? `${raw.slice(0, 48_000)}\n[child output bounded locally]` : raw).trim() || "[child returned no textual or structured output]";
  return { id: childId, domain, status: child.status, output };
}

/**
 * Restart-safe cross-domain executor. It dispatches at most `maxSteps` provider calls per
 * invocation, persists after every completed child, and requires caller-owned rehydration before
 * any later child or synthesis call can use prior outputs.
 */
export class AutonomousCrossDomainExecutor {
  readonly agent: AutonomousAgent;
  readonly store: AutonomousCrossDomainCheckpointStore;
  readonly learning?: AutonomousLearningController;
  private readonly resultCache = new Map<string, Map<string, AutonomousRunResult>>();

  constructor(agent: AutonomousAgent, store: AutonomousCrossDomainCheckpointStore, options: AutonomousCrossDomainExecutorOptions = {}) {
    if (!agent || typeof agent.route !== "function" || typeof agent.blueprint !== "function" || typeof agent.run !== "function") throw new ArgumentError("cross-domain executor requires an AutonomousAgent");
    if (!store || typeof store.load !== "function" || typeof store.save !== "function" || typeof store.appendEvent !== "function" || typeof store.events !== "function") throw new ArgumentError("cross-domain executor requires a checkpoint store");
    this.agent = agent;
    this.store = store;
    this.learning = options.learning;
  }

  async start(task: string, options: AutonomousCrossDomainExecuteOptions = {}): Promise<AutonomousCrossDomainExecutionResult> {
    validateDurableOptions(options);
    const taskText = boundedTask(task);
    const expectedTaskDigest = await digestJson({ task: taskText });
    if (options.routeOverride && options.routeOverride.task_digest !== expectedTaskDigest) throw new ArgumentError("cross-domain route override does not match the task digest");
    const route = options.routeOverride ? options.routeOverride : await this.agent.route(taskText, { domain: options.domain, hints: options.hints, allowCrossDomain: true });
    if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) return this.routeReviewResult(route);
    const blueprint = await this.resolveBlueprint(taskText, route, options);
    const refinement = crossPlanRefinement(options);
    const acceptedPlan = await acceptedCrossDomainPlan(blueprint, refinement);
    const contractDigest = await executionContractDigest(this.agent, options);
    const jobId = boundedId(options.jobId ?? `cross-${route.task_digest.slice(0, 24)}`, "cross-domain jobId");
    const existing = await this.store.load(jobId);
    if (existing) {
      this.assertCheckpointIdentity(existing, route, blueprint);
      const bound = await this.bindExecutionContract(existing, contractDigest, options.rebindLegacyExecutionContract === true);
      const planBound = await this.bindPlanRefinement(bound, acceptedPlan, blueprint, contractDigest);
      return this.drive(taskText, route, blueprint, planBound, options, contractDigest, acceptedPlan);
    }
    const order = acceptedPlan?.priority_child_ids ?? [...blueprint.child_ids];
    const initial = await this.makeCheckpoint(jobId, route, blueprint, order, [], {}, "children_pending", null, contractDigest, acceptedPlan?.refinement_digest ?? null, null);
    await this.store.save(initial);
    await this.appendEvent(jobId, "started", null, "lifecycle", initial);
    return this.drive(taskText, route, blueprint, initial, options, contractDigest, acceptedPlan);
  }

  async resume(jobId: string, task: string, options: Omit<AutonomousCrossDomainExecuteOptions, "jobId"> = {}): Promise<AutonomousCrossDomainExecutionResult> {
    validateDurableOptions(options);
    const taskText = boundedTask(task);
    const normalizedJobId = boundedId(jobId, "cross-domain jobId");
    const checkpoint = await this.store.load(normalizedJobId);
    if (!checkpoint) throw new ArgumentError(`cross-domain job ${normalizedJobId} was not found; caller must rehydrate from its durable store`);
    if (options.routeOverride && options.routeOverride.task_digest !== checkpoint.task_digest) throw new ArgumentError("cross-domain route override does not match the checkpoint task digest");
    const route = options.routeOverride ? options.routeOverride : await this.agent.route(taskText, { domain: options.domain, hints: options.hints, allowCrossDomain: true });
    if (route.abstained || !route.cross_domain || route.task_digest !== checkpoint.task_digest || route.route_digest !== checkpoint.route_digest) throw new ProviderRuntimeError("cross-domain rehydration route does not match the checkpoint");
    const blueprint = await this.resolveBlueprint(taskText, route, options, checkpoint);
    this.assertCheckpointIdentity(checkpoint, route, blueprint);
    const refinement = crossPlanRefinement(options);
    const acceptedPlan = await acceptedCrossDomainPlan(blueprint, refinement);
    const contractOptions = { ...options, jobId: normalizedJobId };
    const contractDigest = await executionContractDigest(this.agent, contractOptions);
    const bound = await this.bindExecutionContract(checkpoint, contractDigest, options.rebindLegacyExecutionContract === true);
    const planBound = await this.bindPlanRefinement(bound, acceptedPlan, blueprint, contractDigest);
    return this.drive(taskText, route, blueprint, planBound, contractOptions, contractDigest, acceptedPlan);
  }

  async events(jobId: string, after = 0, limit = AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS): Promise<AutonomousCrossDomainEvent[]> {
    return this.store.events(boundedId(jobId, "cross-domain jobId"), after, limit);
  }

  private async resolveBlueprint(task: string, route: AutonomousRouteProposal, options: AutonomousCrossDomainExecuteOptions, checkpoint?: AutonomousCrossDomainCheckpoint): Promise<AutonomousCrossDomainBlueprint> {
    if (options.blueprint !== undefined) {
      if (!validCrossBlueprint(options.blueprint)) throw new ArgumentError("cross-domain execution blueprint is malformed");
      if (options.blueprint.task_digest !== route.task_digest) throw new ProviderRuntimeError("cross-domain execution blueprint task does not match the route");
      return options.blueprint;
    }
    const envelope = await this.agent.blueprint(task, { domain: options.domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name), subtasks: options.subtasks });
    const blueprint = envelope.cross_domain_blueprint;
    if (!blueprint || !validCrossBlueprint(blueprint)) throw new ProviderRuntimeError("cross-domain blueprint could not be prepared");
    if (blueprint.task_digest !== route.task_digest || (checkpoint && blueprint.plan_digest !== checkpoint.base_plan_digest)) throw new ProviderRuntimeError("cross-domain rehydration blueprint digest does not match the checkpoint");
    return blueprint;
  }

  private assertCheckpointIdentity(checkpoint: AutonomousCrossDomainCheckpoint, route: AutonomousRouteProposal, blueprint: AutonomousCrossDomainBlueprint): void {
    if (checkpoint.task_digest !== route.task_digest || checkpoint.route_digest !== route.route_digest || checkpoint.base_plan_digest !== blueprint.plan_digest) throw new ProviderRuntimeError("cross-domain checkpoint identity does not match the prepared route and blueprint");
    if (checkpoint.execution_child_ids.length !== blueprint.child_ids.length || checkpoint.execution_child_ids.some((id) => !blueprint.child_ids.includes(id))) throw new ProviderRuntimeError("cross-domain checkpoint contains a child outside the prepared blueprint");
  }

  private async bindExecutionContract(checkpoint: AutonomousCrossDomainCheckpoint, contractDigest: string, explicitLegacyRebind: boolean): Promise<AutonomousCrossDomainCheckpoint> {
    if (checkpoint.execution_contract_digest === contractDigest) return checkpoint;
    if (checkpoint.execution_contract_digest !== undefined && checkpoint.execution_contract_digest !== null) throw new ProviderRuntimeError("cross-domain execution contract does not match the checkpoint");
    if (!explicitLegacyRebind) throw new ProviderRuntimeError("cross-domain checkpoint predates execution-contract binding; set rebindLegacyExecutionContract: true for an explicit migration");
    const migrated = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, checkpoint.plan_refinement_digest, checkpoint.synthesis_result_digest);
    await this.store.save(migrated);
    await this.appendEvent(migrated.job_id, "checkpointed", migrated.next_child_id, migrated.next_child_id === null ? "synthesis" : "child", migrated);
    return migrated;
  }

  private async bindPlanRefinement(checkpoint: AutonomousCrossDomainCheckpoint, acceptedPlan: AutonomousAcceptedCrossDomainPlan | null, blueprint: AutonomousCrossDomainBlueprint, contractDigest: string): Promise<AutonomousCrossDomainCheckpoint> {
    const current = checkpoint.plan_refinement_digest;
    const requested = acceptedPlan?.refinement_digest ?? null;
    if (current === requested) return checkpoint;
    if (current !== null) throw new ProviderRuntimeError("cross-domain plan refinement does not match the checkpoint");
    if (!acceptedPlan) throw new ProviderRuntimeError("cross-domain checkpoint requires its accepted plan refinement for resume");
    if (checkpoint.completed_child_ids.length > 0 || checkpoint.synthesis_result_digest !== null) throw new ProviderRuntimeError("cross-domain execution cannot bind a new plan after child execution has started");
    const migrated = await this.makeCheckpoint(checkpoint.job_id, { route_digest: checkpoint.route_digest }, blueprint, acceptedPlan.priority_child_ids, [], {}, checkpoint.status, checkpoint, contractDigest, requested, null);
    await this.store.save(migrated);
    await this.appendEvent(migrated.job_id, "checkpointed", migrated.next_child_id, migrated.next_child_id === null ? "synthesis" : "child", migrated);
    return migrated;
  }

  private async makeCheckpoint(jobId: string, route: Pick<AutonomousRouteProposal, "route_digest">, blueprint: AutonomousCrossDomainBlueprint, executionOrder: readonly string[], completed: readonly string[], childResultDigests: Record<string, string>, status: AutonomousCrossDomainCheckpointStatus, previous: AutonomousCrossDomainCheckpoint | null, contractDigest: string | null, planRefinementDigest: string | null, synthesisResultDigest: string | null): Promise<AutonomousCrossDomainCheckpoint> {
    const next = completed.length < executionOrder.length ? executionOrder[completed.length]! : null;
    const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA, job_id: jobId, task_digest: blueprint.task_digest, route_digest: route.route_digest, base_plan_digest: blueprint.plan_digest, execution_child_ids: [...executionOrder], completed_child_ids: [...completed], child_result_digests: { ...childResultDigests }, next_child_id: next, plan_refinement_digest: planRefinementDigest, execution_contract_digest: contractDigest, synthesis_result_digest: synthesisResultDigest, generation: (previous?.generation ?? 0) + 1, status, previous_checkpoint_digest: previous?.checkpoint_digest ?? null, retention: "metadata_only;task_prompt_response_and_credentials_not_retained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
  }

  private async makeCheckpointFromExisting(checkpoint: AutonomousCrossDomainCheckpoint, status: AutonomousCrossDomainCheckpointStatus, contractDigest: string, planRefinementDigest: string | null, synthesisResultDigest: string | null): Promise<AutonomousCrossDomainCheckpoint> {
    const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_CHECKPOINT_SCHEMA, job_id: checkpoint.job_id, task_digest: checkpoint.task_digest, route_digest: checkpoint.route_digest, base_plan_digest: checkpoint.base_plan_digest, execution_child_ids: [...checkpoint.execution_child_ids], completed_child_ids: [...checkpoint.completed_child_ids], child_result_digests: { ...checkpoint.child_result_digests }, next_child_id: checkpoint.next_child_id, plan_refinement_digest: planRefinementDigest, execution_contract_digest: contractDigest, synthesis_result_digest: synthesisResultDigest, generation: checkpoint.generation + 1, status, previous_checkpoint_digest: checkpoint.checkpoint_digest, retention: "metadata_only;task_prompt_response_and_credentials_not_retained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
  }

  private async appendEvent(jobId: string, eventType: AutonomousCrossDomainEventType, itemId: string | null, phase: "child" | "synthesis" | "lifecycle", checkpoint: AutonomousCrossDomainCheckpoint): Promise<void> {
    const prior = await this.store.events(jobId, 0, AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS);
    const descriptor = { schema: AUTONOMOUS_CROSS_DOMAIN_EVENT_SCHEMA, sequence: (prior.at(-1)?.sequence ?? 0) + 1, job_id: jobId, event_type: eventType, item_id: itemId, phase, checkpoint_digest: checkpoint.checkpoint_digest, previous_event_digest: prior.at(-1)?.event_digest ?? null, retention: "metadata_only;provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
    await this.store.appendEvent({ ...descriptor, event_digest: await digestJson(descriptor) });
  }

  private routeReviewResult(route: AutonomousRouteProposal): AutonomousCrossDomainExecutionResult {
    return { schema: AUTONOMOUS_CROSS_DOMAIN_EXECUTION_SCHEMA, status: "route_review_required", job_id: null, route, blueprint: null, checkpoint: null, events: [], step_results: [], synthesis: null, completed_children: 0, total_children: route.selected_domains.length, plan_refinement_digest: null, error: null, learning_episode_ids: [], recovery: "caller_rehydrates_task_credentials_and_completed_child_results", retention: "provider_responses_local;checkpoint_metadata_and_outcome_digests_only" };
  }

  private async hydrateChildren(jobId: string, checkpoint: AutonomousCrossDomainCheckpoint, blueprint: AutonomousCrossDomainBlueprint, options: AutonomousCrossDomainExecuteOptions): Promise<Map<string, AutonomousRunResult>> {
    const local = this.resultCache.get(jobId) ?? new Map<string, AutonomousRunResult>();
    const results = new Map<string, AutonomousRunResult>();
    for (const childId of checkpoint.completed_child_ids) {
      const candidate = options.resolveChildResult ? await options.resolveChildResult(childId, checkpoint) : local.get(childId) ?? null;
      if (!candidate) throw new ProviderRuntimeError(`cross-domain child ${childId} is checkpointed but its caller-owned result was not rehydrated`);
      const result = "result" in candidate ? candidate.result : candidate;
      if (!isObject(result) || result.status !== "completed") throw new ProviderRuntimeError(`cross-domain child ${childId} rehydrated an incomplete result`);
      const expectedDigest = checkpoint.child_result_digests[childId];
      if (expectedDigest !== await digestJson(result)) throw new ProviderRuntimeError(`cross-domain child ${childId} result digest does not match the checkpoint`);
      const index = blueprint.child_ids.indexOf(childId);
      const expectedTaskDigest = blueprint.child_blueprints[index]?.task_digest;
      if ("task_digest" in candidate && candidate.task_digest !== expectedTaskDigest) throw new ProviderRuntimeError(`cross-domain child ${childId} task digest does not match the blueprint`);
      if ("output_digest" in candidate && candidate.output_digest !== null) {
        const actualOutputDigest = await digestJson({ output: responseText(result) });
        if (candidate.output_digest !== actualOutputDigest) throw new ProviderRuntimeError(`cross-domain child ${childId} output digest does not match its result`);
      }
      results.set(childId, result);
      local.set(childId, result);
    }
    this.resultCache.set(jobId, local);
    return results;
  }

  private async drive(task: string, route: AutonomousRouteProposal, blueprint: AutonomousCrossDomainBlueprint, initial: AutonomousCrossDomainCheckpoint, options: AutonomousCrossDomainExecuteOptions, contractDigest: string, acceptedPlan: AutonomousAcceptedCrossDomainPlan | null): Promise<AutonomousCrossDomainExecutionResult> {
    let checkpoint = initial;
    const order = checkpoint.execution_child_ids;
    const maxSteps = boundedSteps(options.maxSteps, order.length);
    const stepResults: AutonomousCrossDomainStepResult[] = [];
    const learningEpisodeIds: string[] = [];
    const localResults = await this.hydrateChildren(checkpoint.job_id, checkpoint, blueprint, options);
    const planRefinementDigest = acceptedPlan?.refinement_digest ?? checkpoint.plan_refinement_digest ?? null;
    if (checkpoint.status === "completed") return this.result("completed", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
    if (options.approveProviderCall !== true) {
      checkpoint = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, planRefinementDigest, checkpoint.synthesis_result_digest);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "approval_required", checkpoint.next_child_id, checkpoint.next_child_id === null ? "synthesis" : "child", checkpoint);
      return this.result("approval_required", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
    }
    const costBudget = options.costBudget ?? (options.maxTotalCostUnits === undefined ? undefined : new AutonomousCostBudget(options.maxTotalCostUnits));
    const learning = this.learning ?? options.learning;
    for (let step = 0; step < maxSteps; step += 1) {
      if (checkpoint.next_child_id !== null) {
        const childId = checkpoint.next_child_id;
        const childSpec = childTaskMessage(blueprint, childId);
        const childContext: AutonomousPromptChunk[] = [
          ...(options.context ?? []),
          { id: "cross-domain-parent", content: JSON.stringify({ route_digest: route.route_digest, task_digest: blueprint.task_digest, child_id: childId }), required: true, priority: 100 },
          ...(acceptedPlan ? [{ id: "accepted-cross-domain-plan", content: JSON.stringify({ refinement_digest: acceptedPlan.refinement_digest, child_id: childId, priority_rank: order.indexOf(childId), focus: acceptedPlan.focus_child_ids.includes(childId) }), required: true, priority: 95 }] : []),
        ];
        let run: AutonomousRunResult;
        try {
          run = await this.agent.run(childSpec.task, { domain: childSpec.domain, capability: childSpec.capability, candidates: options.candidates, credential: options.credential, credentialFor: options.credentialFor, context: childContext, hints: [], allowCrossDomain: false, maxInputTokens: options.maxInputTokens, maxOutputTokens: options.maxOutputTokens, maxCostPerMillionTokens: options.maxCostPerMillionTokens, maxLatencyMs: options.maxLatencyMs, minQuality: options.minQuality, requireJson: options.requireJson, responseSchema: options.responseSchema, temperature: options.temperature, tools: options.tools, authorizeAndExecute: options.authorizeAndExecute, toolReadOnly: options.toolReadOnly, approveProviderCall: true, approveEffects: options.approveEffects, execution: options.execution, effectBoundary: options.effectBoundary, costBudget, executionAttempt: checkpoint.generation, maxProviderFailovers: options.maxProviderFailovers, executionLifecycle: "observe_only", signal: options.signal, observer: options.observer });
        } catch (error) {
          const metadata = errorMetadata(error);
          checkpoint = await this.makeCheckpointFromExisting(checkpoint, "failed", contractDigest, planRefinementDigest, null);
          await this.store.save(checkpoint);
          await this.appendEvent(checkpoint.job_id, "failed", childId, "child", checkpoint);
          return this.result("failed", route, blueprint, checkpoint, stepResults, learningEpisodeIds, metadata);
        }
        const text = responseText(run);
        const outputDigest = text ? await digestJson({ output: text }) : null;
        const outputBytes = new TextEncoder().encode(text).byteLength;
        const resultDigest = await digestJson(run);
        const learningEpisodeId = run.status === "completed" && learning ? (await learning.prepareRun(run, { episodeId: `cross:${checkpoint.job_id}:${childId}`, runId: `cross:${checkpoint.job_id}:${childId}`, stageId: childId, parentJobId: checkpoint.job_id, planRefinementDigest })).episode_id : null;
        if (learningEpisodeId) learningEpisodeIds.push(learningEpisodeId);
        stepResults.push({ phase: "child", item_id: childId, run, output_digest: outputDigest, output_bytes: outputBytes, execution_child_ids: [...order], completed_child_ids: [...checkpoint.completed_child_ids], child_result_digests: { ...checkpoint.child_result_digests }, plan_refinement_digest: planRefinementDigest, learning_episode_id: learningEpisodeId });
        if (run.status === "approval_required") {
          checkpoint = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, planRefinementDigest, null);
          await this.store.save(checkpoint);
          await this.appendEvent(checkpoint.job_id, "approval_required", childId, "child", checkpoint);
          return this.result("approval_required", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
        }
        if (run.status === "reconciliation_required") {
          checkpoint = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, planRefinementDigest, null);
          await this.store.save(checkpoint);
          await this.appendEvent(checkpoint.job_id, "reconciliation_required", childId, "child", checkpoint);
          return this.result("reconciliation_required", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
        }
        if (run.status !== "completed") {
          checkpoint = await this.makeCheckpointFromExisting(checkpoint, "failed", contractDigest, planRefinementDigest, null);
          await this.store.save(checkpoint);
          await this.appendEvent(checkpoint.job_id, "failed", childId, "child", checkpoint);
          return this.result("failed", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
        }
        localResults.set(childId, run);
        this.resultCache.set(checkpoint.job_id, localResults);
        const completed = [...checkpoint.completed_child_ids, childId];
        const digests = { ...checkpoint.child_result_digests, [childId]: resultDigest };
        const nextStatus: AutonomousCrossDomainCheckpointStatus = completed.length === order.length ? "synthesis_pending" : "children_pending";
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, route, blueprint, order, completed, digests, nextStatus, checkpoint, contractDigest, planRefinementDigest, null);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "child_completed", childId, "child", checkpoint);
        stepResults[step] = { ...stepResults[step]!, completed_child_ids: [...completed], child_result_digests: { ...digests } };
        continue;
      }
      if (options.synthesize === false) return this.result("paused", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
      const synthesisMessage = blueprint.synthesis_blueprint.prompt.messages.find((candidate) => candidate.source_id === "task");
      if (!synthesisMessage) throw new ProviderRuntimeError("cross-domain synthesis has no bounded task message");
      const synthesisContext: AutonomousPromptChunk[] = [
        ...(options.context ?? []),
        { id: "cross-domain-parent", content: JSON.stringify({ route_digest: route.route_digest, task_digest: blueprint.task_digest }), required: true, priority: 100 },
        ...(acceptedPlan ? [{ id: "accepted-cross-domain-plan", content: JSON.stringify({ refinement_digest: acceptedPlan.refinement_digest, priority_child_ids: order, focus_child_ids: acceptedPlan.focus_child_ids }), required: true, priority: 95 }] : []),
      ];
      for (const childId of order) {
        const child = localResults.get(childId);
        if (!child) throw new ProviderRuntimeError(`cross-domain synthesis cannot rehydrate child ${childId}`);
        const index = blueprint.child_ids.indexOf(childId);
        const metadata = { ...childOutput(childId, child, blueprint.child_blueprints[index]!.domain_profile.domain), output_digest: responseText(child) ? await digestJson({ output: responseText(child) }) : null };
        synthesisContext.push({ id: `cross-domain-output-${childId}`, content: JSON.stringify(metadata), priority: 90 });
      }
      let synthesis: AutonomousRunResult;
      try {
        synthesis = await this.agent.run(synthesisMessage.content, { domain: "cross_domain", capability: "cross_domain_synthesis", candidates: options.candidates, credential: options.credential, credentialFor: options.credentialFor, context: synthesisContext, hints: [], allowCrossDomain: false, maxInputTokens: options.maxInputTokens, maxOutputTokens: options.maxOutputTokens, maxCostPerMillionTokens: options.maxCostPerMillionTokens, maxLatencyMs: options.maxLatencyMs, minQuality: options.minQuality, requireJson: options.requireJson, responseSchema: options.responseSchema, temperature: options.temperature, tools: options.tools, authorizeAndExecute: options.authorizeAndExecute, toolReadOnly: options.toolReadOnly, approveProviderCall: true, approveEffects: options.approveEffects, execution: options.execution, effectBoundary: options.effectBoundary, costBudget, executionAttempt: order.length + 1, maxProviderFailovers: options.maxProviderFailovers, executionLifecycle: "observe_only", signal: options.signal, observer: options.observer });
      } catch (error) {
        checkpoint = await this.makeCheckpointFromExisting(checkpoint, "failed", contractDigest, planRefinementDigest, null);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "failed", "synthesis", "synthesis", checkpoint);
        return this.result("failed", route, blueprint, checkpoint, stepResults, learningEpisodeIds, errorMetadata(error));
      }
      const synthesisText = responseText(synthesis);
      const synthesisOutputDigest = synthesisText ? await digestJson({ output: synthesisText }) : null;
      const synthesisEpisodeId = synthesis.status === "completed" && learning ? (await learning.prepareRun(synthesis, { episodeId: `cross:${checkpoint.job_id}:synthesis`, runId: `cross:${checkpoint.job_id}:synthesis`, stageId: "synthesis", parentJobId: checkpoint.job_id, planRefinementDigest })).episode_id : null;
      if (synthesisEpisodeId) learningEpisodeIds.push(synthesisEpisodeId);
      stepResults.push({ phase: "synthesis", item_id: "synthesis", run: synthesis, output_digest: synthesisOutputDigest, output_bytes: new TextEncoder().encode(synthesisText).byteLength, execution_child_ids: [...order], completed_child_ids: [...checkpoint.completed_child_ids], child_result_digests: { ...checkpoint.child_result_digests }, plan_refinement_digest: planRefinementDigest, learning_episode_id: synthesisEpisodeId });
      if (synthesis.status === "approval_required") {
        checkpoint = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, planRefinementDigest, null);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "approval_required", "synthesis", "synthesis", checkpoint);
        return this.result("approval_required", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
      }
      if (synthesis.status === "reconciliation_required") {
        checkpoint = await this.makeCheckpointFromExisting(checkpoint, "paused", contractDigest, planRefinementDigest, null);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "reconciliation_required", "synthesis", "synthesis", checkpoint);
        return this.result("reconciliation_required", route, blueprint, checkpoint, stepResults, learningEpisodeIds, undefined, synthesis);
      }
      if (synthesis.status !== "completed") {
        checkpoint = await this.makeCheckpointFromExisting(checkpoint, "failed", contractDigest, planRefinementDigest, null);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "failed", "synthesis", "synthesis", checkpoint);
        return this.result("failed", route, blueprint, checkpoint, stepResults, learningEpisodeIds);
      }
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, route, blueprint, order, checkpoint.completed_child_ids, checkpoint.child_result_digests, "completed", checkpoint, contractDigest, planRefinementDigest, await digestJson(synthesis));
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "synthesis_completed", "synthesis", "synthesis", checkpoint);
      await this.appendEvent(checkpoint.job_id, "completed", null, "lifecycle", checkpoint);
      return this.result("completed", route, blueprint, checkpoint, stepResults, learningEpisodeIds, undefined, synthesis);
    }
    const status: AutonomousCrossDomainExecutionStatus = checkpoint.status === "completed" ? "completed" : "paused";
    if (checkpoint.status !== "completed") {
      checkpoint = await this.makeCheckpointFromExisting(checkpoint, checkpoint.status === "synthesis_pending" ? "synthesis_pending" : "paused", contractDigest, planRefinementDigest, checkpoint.synthesis_result_digest);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "checkpointed", checkpoint.next_child_id, checkpoint.next_child_id === null ? "synthesis" : "child", checkpoint);
    }
    return this.result(status, route, blueprint, checkpoint, stepResults, learningEpisodeIds);
  }

  private async result(status: AutonomousCrossDomainExecutionStatus, route: AutonomousRouteProposal, blueprint: AutonomousCrossDomainBlueprint, checkpoint: AutonomousCrossDomainCheckpoint, stepResults: AutonomousCrossDomainStepResult[], learningEpisodeIds: string[], error: AutonomousCrossDomainErrorMetadata | undefined = undefined, synthesis: AutonomousRunResult | null = null): Promise<AutonomousCrossDomainExecutionResult> {
    return { schema: AUTONOMOUS_CROSS_DOMAIN_EXECUTION_SCHEMA, status, job_id: checkpoint.job_id, route, blueprint, checkpoint, events: await this.store.events(checkpoint.job_id, 0, AUTONOMOUS_CROSS_DOMAIN_MAX_EVENTS), step_results: stepResults, synthesis, completed_children: checkpoint.completed_child_ids.length, total_children: checkpoint.execution_child_ids.length, plan_refinement_digest: checkpoint.plan_refinement_digest, error: error ?? null, learning_episode_ids: [...learningEpisodeIds], recovery: "caller_rehydrates_task_credentials_and_completed_child_results", retention: "provider_responses_local;checkpoint_metadata_and_outcome_digests_only" };
  }
}
