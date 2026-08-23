import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_EXECUTION_POLICY_SCHEMA = "bioprism-typescript-autonomous-execution-policy/0.1" as const;
export const AUTONOMOUS_EXECUTION_STATE_SCHEMA = "bioprism-typescript-autonomous-execution-state/0.1" as const;
export const AUTONOMOUS_EXECUTION_EVENT_SCHEMA = "bioprism-typescript-autonomous-execution-event/0.1" as const;
export const AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-execution-journal/0.1" as const;
export const AUTONOMOUS_EXECUTION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-execution-snapshot/0.1" as const;

export const AUTONOMOUS_EXECUTION_MAX_STEPS = 4_096;
export const AUTONOMOUS_EXECUTION_MAX_PROVIDER_CALLS = 1_024;
export const AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS = 8;
export const AUTONOMOUS_EXECUTION_MAX_TOOL_CALLS = 8_192;
export const AUTONOMOUS_EXECUTION_MAX_EFFECTFUL_CALLS = 512;
export const AUTONOMOUS_EXECUTION_MAX_REPLANS = 64;
export const AUTONOMOUS_EXECUTION_MAX_COST_UNITS = 1_000_000;
export const AUTONOMOUS_EXECUTION_MAX_JOURNAL_EVENTS = 32_768;
export const AUTONOMOUS_EXECUTION_MAX_JOURNAL_BYTES = 64_000_000;
export const AUTONOMOUS_EXECUTION_MAX_EVENT_BYTES = 256_000;
export const AUTONOMOUS_EXECUTION_MAX_METADATA_DEPTH = 32;

export const AUTONOMOUS_EXECUTION_TERMINAL_STATUSES = ["completed", "failed", "cancelled", "reconciliation_required"] as const;
export const AUTONOMOUS_EXECUTION_EVENT_KINDS = ["started", "resumed", "provider_call", "tool_intent", "tool_outcome", "effect_reconciliation", "evaluation", "checkpoint", "replan", "completed", "failed"] as const;

export type AutonomousExecutionTerminalStatus = typeof AUTONOMOUS_EXECUTION_TERMINAL_STATUSES[number];
export type AutonomousExecutionEventKind = typeof AUTONOMOUS_EXECUTION_EVENT_KINDS[number];

export class AutonomousExecutionError extends ArgumentError {
  override readonly name: string = "AutonomousExecutionError";
}

export class AutonomousExecutionPolicyError extends AutonomousExecutionError {
  override readonly name: string = "AutonomousExecutionPolicyError";
}

export interface AutonomousExecutionPolicyInput {
  schema?: string;
  max_steps?: number;
  max_provider_calls?: number;
  max_provider_failovers?: number;
  max_tool_calls?: number;
  max_effectful_calls?: number;
  max_replans?: number;
  max_cost_units?: number;
  allow_side_effects?: boolean;
  stop_on_error?: boolean;
  pause_on_approval?: boolean;
}

export interface AutonomousExecutionPolicyProjection {
  schema: typeof AUTONOMOUS_EXECUTION_POLICY_SCHEMA;
  max_steps: number;
  max_provider_calls: number;
  max_provider_failovers: number;
  max_tool_calls: number;
  max_effectful_calls: number;
  max_replans: number;
  max_cost_units: number;
  allow_side_effects: boolean;
  stop_on_error: boolean;
  pause_on_approval: boolean;
  authorization: "caller_owned_policy";
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) throw new AutonomousExecutionError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum = 512): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousExecutionError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (value === undefined && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousExecutionError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedHeadDigest(name: string, value: unknown): string {
  if (value === "") return "";
  return boundedDigest(name, value)!;
}

function boundedInteger(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) throw new AutonomousExecutionError(`${name} must be an integer within [0, ${maximum}]`);
  return value as number;
}

function boundedCost(name: string, value: unknown, maximum = AUTONOMOUS_EXECUTION_MAX_COST_UNITS): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) throw new AutonomousExecutionError(`${name} must be finite and within [0, ${maximum}]`);
  return value;
}

function boundedBoolean(name: string, value: unknown): boolean {
  if (typeof value !== "boolean") throw new AutonomousExecutionError(`${name} must be boolean`);
  return value;
}

function assertSafeMetadata(value: unknown, depth = 0): void {
  if (depth > AUTONOMOUS_EXECUTION_MAX_METADATA_DEPTH) throw new AutonomousExecutionError("autonomous execution metadata is too deeply nested");
  if (Array.isArray(value)) {
    for (const child of value) assertSafeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "password", "secret", "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response", "rawpayload", "arguments", "output", "task", "messages"].includes(normalized)) throw new AutonomousExecutionError("autonomous execution metadata contains transient or secret-shaped fields");
      assertSafeMetadata(child, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new AutonomousExecutionError("autonomous execution metadata contains a non-finite number");
}

function assertMetadataBytes(name: string, value: unknown, maximum: number): void {
  assertSafeMetadata(value);
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new AutonomousExecutionError(`${name} must be JSON serializable`);
  }
  if (new TextEncoder().encode(encoded).byteLength > maximum) throw new AutonomousExecutionError(`${name} exceeds its bounded byte size`);
}

export class AutonomousExecutionPolicy {
  readonly max_steps: number;
  readonly max_provider_calls: number;
  readonly max_provider_failovers: number;
  readonly max_tool_calls: number;
  readonly max_effectful_calls: number;
  readonly max_replans: number;
  readonly max_cost_units: number;
  readonly allow_side_effects: boolean;
  readonly stop_on_error: boolean;
  readonly pause_on_approval: boolean;

  constructor(input: AutonomousExecutionPolicyInput = {}) {
    if (!isObject(input)) throw new AutonomousExecutionError("execution policy must be an object");
    const allowed = new Set(["schema", "max_steps", "max_provider_calls", "max_provider_failovers", "max_tool_calls", "max_effectful_calls", "max_replans", "max_cost_units", "allow_side_effects", "stop_on_error", "pause_on_approval"]);
    if (Object.keys(input).some((key) => !allowed.has(key))) throw new AutonomousExecutionError("execution policy contains unsupported fields");
    if (input.schema !== undefined && input.schema !== AUTONOMOUS_EXECUTION_POLICY_SCHEMA) throw new AutonomousExecutionError("execution policy schema is unsupported");
    this.max_steps = boundedInteger("max_steps", input.max_steps ?? 32, AUTONOMOUS_EXECUTION_MAX_STEPS);
    this.max_provider_calls = boundedInteger("max_provider_calls", input.max_provider_calls ?? 16, AUTONOMOUS_EXECUTION_MAX_PROVIDER_CALLS);
    this.max_provider_failovers = boundedInteger("max_provider_failovers", input.max_provider_failovers ?? 2, AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS);
    this.max_tool_calls = boundedInteger("max_tool_calls", input.max_tool_calls ?? 128, AUTONOMOUS_EXECUTION_MAX_TOOL_CALLS);
    this.max_effectful_calls = boundedInteger("max_effectful_calls", input.max_effectful_calls ?? 0, AUTONOMOUS_EXECUTION_MAX_EFFECTFUL_CALLS);
    this.max_replans = boundedInteger("max_replans", input.max_replans ?? 2, AUTONOMOUS_EXECUTION_MAX_REPLANS);
    this.max_cost_units = boundedCost("max_cost_units", input.max_cost_units ?? 100);
    this.allow_side_effects = boundedBoolean("allow_side_effects", input.allow_side_effects ?? false);
    this.stop_on_error = boundedBoolean("stop_on_error", input.stop_on_error ?? true);
    this.pause_on_approval = boundedBoolean("pause_on_approval", input.pause_on_approval ?? true);
    if (this.allow_side_effects && this.max_effectful_calls === 0) throw new AutonomousExecutionError("allow_side_effects requires max_effectful_calls greater than zero");
  }

  toJSON(): AutonomousExecutionPolicyProjection {
    return {
      schema: AUTONOMOUS_EXECUTION_POLICY_SCHEMA,
      max_steps: this.max_steps,
      max_provider_calls: this.max_provider_calls,
      max_provider_failovers: this.max_provider_failovers,
      max_tool_calls: this.max_tool_calls,
      max_effectful_calls: this.max_effectful_calls,
      max_replans: this.max_replans,
      max_cost_units: this.max_cost_units,
      allow_side_effects: this.allow_side_effects,
      stop_on_error: this.stop_on_error,
      pause_on_approval: this.pause_on_approval,
      authorization: "caller_owned_policy",
    };
  }

  async digest(): Promise<string> {
    return digestJson(this.toJSON());
  }
}

export function normalizeAutonomousExecutionPolicy(input?: AutonomousExecutionPolicy | AutonomousExecutionPolicyInput): AutonomousExecutionPolicy {
  return input instanceof AutonomousExecutionPolicy ? input : new AutonomousExecutionPolicy(input ?? {});
}

export interface AutonomousExecutionState {
  schema: typeof AUTONOMOUS_EXECUTION_STATE_SCHEMA;
  execution_id: string;
  domain: string;
  capability: string;
  risk_class: string;
  policy_digest: string;
  step_index: number;
  provider_calls: number;
  provider_failovers: number;
  tool_calls: number;
  effectful_calls: number;
  cost_units: number;
  replans: number;
  status: string;
  last_event_kind: string;
  last_tool: string | null;
  last_call_id: string | null;
  last_outcome_digest: string | null;
  last_evaluation_digest: string | null;
  checkpoint_digest: string | null;
  journal_sequence: number;
  retention: "metadata_only_no_task_prompt_credentials_or_payloads";
}

export interface AutonomousExecutionEvent {
  schema: typeof AUTONOMOUS_EXECUTION_EVENT_SCHEMA;
  execution_id: string;
  kind: AutonomousExecutionEventKind;
  domain: string;
  capability: string;
  risk_class: string;
  status: string;
  policy_digest: string;
  state: AutonomousExecutionState;
  [key: string]: unknown;
}

export interface AutonomousExecutionJournalRow {
  schema: typeof AUTONOMOUS_EXECUTION_EVENT_SCHEMA;
  sequence: number;
  event: AutonomousExecutionEvent;
  previous_digest: string;
  created_at: number;
  event_digest: string;
}

export interface AutonomousExecutionJournalReceipt {
  schema: typeof AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA;
  sequence: number;
  event_digest: string;
  head_digest: string;
  execution_id: string;
  kind: AutonomousExecutionEventKind;
  retention: "metadata_only_hash_chained";
}

export interface AutonomousExecutionJournalSnapshot {
  schema: typeof AUTONOMOUS_EXECUTION_SNAPSHOT_SCHEMA;
  rows: AutonomousExecutionJournalRow[];
  head_digest: string;
  retention: "metadata_only_hash_chained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

/** Adapter contract for durable SQLite, IndexedDB, object-store, or database persistence. */
export interface AutonomousExecutionSnapshotPersistence {
  read(): Promise<AutonomousExecutionJournalSnapshot | null> | AutonomousExecutionJournalSnapshot | null;
  write(snapshot: AutonomousExecutionJournalSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousExecutionJournalSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousExecutionSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousExecutionTransactionalSnapshotTextStore extends AutonomousExecutionSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousExecutionSnapshotJournal extends AutonomousExecutionJournal {
  snapshot(): Promise<AutonomousExecutionJournalSnapshot>;
  restore(snapshot: AutonomousExecutionJournalSnapshot): Promise<void>;
}

export interface AutonomousExecutionJournal {
  append(event: AutonomousExecutionEvent): Promise<AutonomousExecutionJournalReceipt> | AutonomousExecutionJournalReceipt;
  state(executionId: string): Promise<AutonomousExecutionState | null> | AutonomousExecutionState | null;
  events(options?: { executionId?: string; afterSequence?: number; limit?: number }): Promise<AutonomousExecutionJournalRow[]> | AutonomousExecutionJournalRow[];
  verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> | { schema: typeof AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" };
}

export interface AutonomousExecutionControllerOptions {
  executionId: string;
  domain: string;
  capability: string;
  riskClass: string;
  policy?: AutonomousExecutionPolicy | AutonomousExecutionPolicyInput;
  journal?: AutonomousExecutionJournal;
  resume?: boolean;
}

function validateState(state: AutonomousExecutionState, policy: AutonomousExecutionPolicy): AutonomousExecutionState {
  if (!isObject(state) || state.schema !== AUTONOMOUS_EXECUTION_STATE_SCHEMA) throw new AutonomousExecutionError("autonomous execution state schema is unsupported");
  boundedIdentifier("execution_id", state.execution_id, 256);
  boundedIdentifier("execution domain", state.domain);
  boundedIdentifier("execution capability", state.capability);
  boundedIdentifier("execution risk_class", state.risk_class);
  boundedDigest("execution policy_digest", state.policy_digest);
  boundedInteger("execution step_index", state.step_index, policy.max_steps);
  boundedInteger("execution provider_calls", state.provider_calls, policy.max_provider_calls);
  boundedInteger("execution provider_failovers", state.provider_failovers, policy.max_provider_failovers);
  boundedInteger("execution tool_calls", state.tool_calls, policy.max_tool_calls);
  boundedInteger("execution effectful_calls", state.effectful_calls, policy.max_effectful_calls);
  boundedCost("execution cost_units", state.cost_units);
  boundedInteger("execution replans", state.replans, policy.max_replans);
  boundedIdentifier("execution status", state.status);
  boundedIdentifier("execution last_event_kind", state.last_event_kind);
  if (state.last_tool !== null) boundedIdentifier("execution last_tool", state.last_tool);
  if (state.last_call_id !== null) boundedIdentifier("execution last_call_id", state.last_call_id);
  boundedDigest("execution last_outcome_digest", state.last_outcome_digest, true);
  boundedDigest("execution last_evaluation_digest", state.last_evaluation_digest, true);
  boundedDigest("execution checkpoint_digest", state.checkpoint_digest, true);
  boundedInteger("execution journal_sequence", state.journal_sequence, AUTONOMOUS_EXECUTION_MAX_JOURNAL_EVENTS);
  return clone(state);
}

function validateEvent(event: AutonomousExecutionEvent, policy: AutonomousExecutionPolicy): AutonomousExecutionEvent {
  if (!isObject(event) || event.schema !== AUTONOMOUS_EXECUTION_EVENT_SCHEMA || !AUTONOMOUS_EXECUTION_EVENT_KINDS.includes(event.kind)) throw new AutonomousExecutionError("autonomous execution event is malformed");
  const allowed = new Set(["schema", "execution_id", "kind", "domain", "capability", "risk_class", "status", "policy_digest", "state", "step_index", "provider_calls", "provider_failovers", "tool_calls", "effectful_calls", "replans", "attempt", "turn", "input_tokens", "output_tokens", "status_code", "latency_ms", "cost_units", "estimated_cost_units", "actual_cost_units", "selection_digest", "arguments_digest", "output_digest", "outcome_digest", "evaluation_digest", "request_id_digest", "instruction_digest", "effect_id", "effect_status", "idempotency_key_digest", "dispatch_attempt", "reconciliation_digest", "provider", "model", "invocation_kind", "provider_outcome", "tool", "call_id", "evaluator_id", "evaluator_version", "failure_class", "reason", "read_only", "approval_required", "passed", "failover", "retryable", "reward", "metadata"]);
  if (Object.keys(event).some((key) => !allowed.has(key))) throw new AutonomousExecutionError("autonomous execution event contains unsupported fields");
  boundedIdentifier("event execution_id", event.execution_id, 256);
  boundedIdentifier("event domain", event.domain);
  boundedIdentifier("event capability", event.capability);
  boundedIdentifier("event risk_class", event.risk_class);
  boundedIdentifier("event status", event.status);
  boundedDigest("event policy_digest", event.policy_digest);
  validateState(event.state, policy);
  const knownOptional = ["step_index", "provider_calls", "provider_failovers", "tool_calls", "effectful_calls", "replans", "attempt", "turn", "input_tokens", "output_tokens", "status_code"] as const;
  for (const name of knownOptional) {
    if (event[name] !== undefined && !(name === "status_code" && event[name] === null)) boundedInteger(`event ${name}`, event[name], name === "step_index" ? policy.max_steps : name === "provider_calls" ? policy.max_provider_calls : name === "provider_failovers" ? policy.max_provider_failovers : name === "tool_calls" ? policy.max_tool_calls : name === "effectful_calls" ? policy.max_effectful_calls : name === "replans" ? policy.max_replans : name === "attempt" ? 64 : name === "turn" ? 64 : name === "status_code" ? 999 : 100_000_000);
  }
  for (const name of ["cost_units", "estimated_cost_units", "actual_cost_units", "latency_ms"] as const) if (event[name] !== undefined) boundedCost(`event ${name}`, event[name]);
  for (const name of ["selection_digest", "arguments_digest", "output_digest", "outcome_digest", "evaluation_digest", "request_id_digest", "instruction_digest"] as const) if (event[name] !== undefined && event[name] !== null) boundedDigest(`event ${name}`, event[name]);
  for (const name of ["provider", "model", "invocation_kind", "provider_outcome", "tool", "call_id", "evaluator_id", "evaluator_version", "failure_class", "reason"] as const) if (event[name] !== undefined && event[name] !== null) boundedText(`event ${name}`, event[name], name === "reason" ? 2_048 : 512);
  for (const name of ["read_only", "approval_required", "passed", "failover", "retryable"] as const) if (event[name] !== undefined) boundedBoolean(`event ${name}`, event[name]);
  if (event.reward !== undefined) boundedCost("event reward", event.reward, 1);
  if (event.metadata !== undefined) assertMetadataBytes("event metadata", event.metadata, 64_000);
  assertMetadataBytes("autonomous execution event", event, AUTONOMOUS_EXECUTION_MAX_EVENT_BYTES);
  return clone(event);
}

/** Validate a complete execution journal snapshot before it reaches a live journal or store. */
export async function validateAutonomousExecutionJournalSnapshot(value: unknown): Promise<AutonomousExecutionJournalSnapshot> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EXECUTION_SNAPSHOT_SCHEMA || !Array.isArray(value.rows) || value.retention !== "metadata_only_hash_chained" || value.secret_material !== "never_returned") throw new AutonomousExecutionError("execution journal snapshot is malformed");
  if (value.rows.length > AUTONOMOUS_EXECUTION_MAX_JOURNAL_EVENTS) throw new AutonomousExecutionError("execution journal snapshot event count exceeds its capacity");
  boundedHeadDigest("snapshot head_digest", value.head_digest);
  boundedDigest("snapshot snapshot_digest", value.snapshot_digest);
  const { snapshot_digest: observed, ...descriptor } = value;
  if (await digestJson(descriptor) !== observed) throw new AutonomousExecutionError("execution journal snapshot digest does not match");
  const policy = new AutonomousExecutionPolicy({
    max_steps: AUTONOMOUS_EXECUTION_MAX_STEPS,
    max_provider_calls: AUTONOMOUS_EXECUTION_MAX_PROVIDER_CALLS,
    max_provider_failovers: AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS,
    max_tool_calls: AUTONOMOUS_EXECUTION_MAX_TOOL_CALLS,
    max_effectful_calls: AUTONOMOUS_EXECUTION_MAX_EFFECTFUL_CALLS,
    max_replans: AUTONOMOUS_EXECUTION_MAX_REPLANS,
    max_cost_units: AUTONOMOUS_EXECUTION_MAX_COST_UNITS,
    allow_side_effects: true,
  });
  let previous = "";
  let totalBytes = 0;
  for (let index = 0; index < value.rows.length; index += 1) {
    const row = value.rows[index] as unknown as AutonomousExecutionJournalRow;
    if (!isObject(row) || row.schema !== AUTONOMOUS_EXECUTION_EVENT_SCHEMA || row.sequence !== index + 1 || row.previous_digest !== previous || typeof row.event_digest !== "string" || !/^[0-9a-f]{64}$/.test(row.event_digest) || !Number.isSafeInteger(row.created_at) || row.created_at < 0) throw new AutonomousExecutionError("execution journal hash chain sequence is invalid");
    const event = validateEvent(row.event as AutonomousExecutionEvent, policy);
    const rowDescriptor = { schema: AUTONOMOUS_EXECUTION_EVENT_SCHEMA, sequence: row.sequence, event, previous_digest: row.previous_digest, created_at: row.created_at };
    if (await digestJson(rowDescriptor) !== row.event_digest) throw new AutonomousExecutionError("execution journal hash chain digest is invalid");
    totalBytes += new TextEncoder().encode(JSON.stringify(row)).byteLength;
    if (totalBytes > AUTONOMOUS_EXECUTION_MAX_JOURNAL_BYTES) throw new AutonomousExecutionError("execution journal snapshot exceeds its byte capacity");
    previous = row.event_digest;
  }
  if (value.head_digest !== previous) throw new AutonomousExecutionError("execution journal snapshot head does not match its rows");
  return clone(value) as unknown as AutonomousExecutionJournalSnapshot;
}

/** In-memory reference journal; applications can supply SQLite, IndexedDB, or object-store adapters through the journal interface. */
export class InMemoryAutonomousExecutionJournal implements AutonomousExecutionSnapshotJournal {
  private readonly rows: AutonomousExecutionJournalRow[] = [];
  private readonly maxEvents: number;
  private readonly maxBytes: number;
  private readonly clock: () => number;
  private readonly validationPolicy: AutonomousExecutionPolicy;
  private totalBytes = 0;
  private appendOperation: Promise<void> = Promise.resolve();

  constructor(options: { maxEvents?: number; maxBytes?: number; clock?: () => number } = {}) {
    this.maxEvents = boundedInteger("journal maxEvents", options.maxEvents ?? AUTONOMOUS_EXECUTION_MAX_JOURNAL_EVENTS, AUTONOMOUS_EXECUTION_MAX_JOURNAL_EVENTS);
    if (this.maxEvents < 1) throw new AutonomousExecutionError("journal maxEvents must be at least one");
    this.maxBytes = boundedInteger("journal maxBytes", options.maxBytes ?? AUTONOMOUS_EXECUTION_MAX_JOURNAL_BYTES, AUTONOMOUS_EXECUTION_MAX_JOURNAL_BYTES);
    if (this.maxBytes < AUTONOMOUS_EXECUTION_MAX_EVENT_BYTES) throw new AutonomousExecutionError("journal maxBytes is below one event capacity");
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new AutonomousExecutionError("journal clock must be callable");
    this.validationPolicy = new AutonomousExecutionPolicy({ max_steps: AUTONOMOUS_EXECUTION_MAX_STEPS, max_provider_calls: AUTONOMOUS_EXECUTION_MAX_PROVIDER_CALLS, max_provider_failovers: AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS, max_tool_calls: AUTONOMOUS_EXECUTION_MAX_TOOL_CALLS, max_effectful_calls: AUTONOMOUS_EXECUTION_MAX_EFFECTFUL_CALLS, max_replans: AUTONOMOUS_EXECUTION_MAX_REPLANS, max_cost_units: AUTONOMOUS_EXECUTION_MAX_COST_UNITS, allow_side_effects: true });
  }

  async append(event: AutonomousExecutionEvent): Promise<AutonomousExecutionJournalReceipt> {
    return this.enqueueJournal(() => this.appendUnlocked(event));
  }

  private async appendUnlocked(event: AutonomousExecutionEvent): Promise<AutonomousExecutionJournalReceipt> {
    const normalized = validateEvent(event, this.validationPolicy);
    if (this.rows.length >= this.maxEvents) throw new AutonomousExecutionError("execution journal event capacity is exhausted");
    const createdAt = this.clock();
    if (!Number.isFinite(createdAt) || createdAt < 0) throw new AutonomousExecutionError("journal clock returned an invalid timestamp");
    const sequence = this.rows.length + 1;
    const previousDigest = this.rows.at(-1)?.event_digest ?? "";
    const descriptor = { schema: AUTONOMOUS_EXECUTION_EVENT_SCHEMA, sequence, event: normalized, previous_digest: previousDigest, created_at: Math.floor(createdAt) };
    const eventDigest = await digestJson(descriptor);
    const row: AutonomousExecutionJournalRow = { ...descriptor, event_digest: eventDigest };
    const size = new TextEncoder().encode(JSON.stringify(row)).byteLength;
    if (this.totalBytes + size > this.maxBytes) throw new AutonomousExecutionError("execution journal byte capacity is exhausted");
    this.rows.push(clone(row));
    this.totalBytes += size;
    return { schema: AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA, sequence, event_digest: eventDigest, head_digest: eventDigest, execution_id: normalized.execution_id, kind: normalized.kind, retention: "metadata_only_hash_chained" };
  }

  state(executionId: string): AutonomousExecutionState | null {
    boundedIdentifier("execution_id", executionId, 256);
    const row = [...this.rows].reverse().find((candidate) => candidate.event.execution_id === executionId);
    if (!row) return null;
    return { ...clone(row.event.state), journal_sequence: row.sequence, checkpoint_digest: row.event_digest };
  }

  events(options: { executionId?: string; afterSequence?: number; limit?: number } = {}): AutonomousExecutionJournalRow[] {
    if (options.executionId !== undefined) boundedIdentifier("execution_id", options.executionId, 256);
    const after = options.afterSequence ?? 0;
    if (!Number.isSafeInteger(after) || after < 0) throw new AutonomousExecutionError("journal afterSequence must be a non-negative integer");
    const limit = options.limit ?? Math.min(256, this.maxEvents);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > this.maxEvents) throw new AutonomousExecutionError("journal limit is outside its bounds");
    return this.rows.filter((row) => row.sequence > after && (options.executionId === undefined || row.event.execution_id === options.executionId)).slice(0, limit).map(clone);
  }

  async verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> {
    return this.enqueueJournal(() => this.verifyIntegrityUnlocked());
  }

  snapshotRows(): AutonomousExecutionJournalRow[] {
    return this.rows.map(clone);
  }

  async snapshot(): Promise<AutonomousExecutionJournalSnapshot> {
    return this.enqueueJournal(() => this.snapshotUnlocked());
  }

  async restore(snapshot: AutonomousExecutionJournalSnapshot): Promise<void> {
    await this.enqueueJournal(async () => {
      if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_EXECUTION_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.rows) || snapshot.retention !== "metadata_only_hash_chained" || snapshot.secret_material !== "never_returned") throw new AutonomousExecutionError("execution journal snapshot is malformed");
      boundedHeadDigest("snapshot head_digest", snapshot.head_digest);
      boundedDigest("snapshot snapshot_digest", snapshot.snapshot_digest);
      const { snapshot_digest: observed, ...descriptor } = snapshot;
      if (await digestJson(descriptor) !== observed) throw new AutonomousExecutionError("execution journal snapshot digest does not match");
      const validated = await this.validateRows(snapshot.rows);
      if (validated.headDigest !== snapshot.head_digest) throw new AutonomousExecutionError("execution journal snapshot head does not match its rows");
      this.rows.splice(0, this.rows.length, ...snapshot.rows.map(clone));
      this.totalBytes = validated.totalBytes;
    });
  }

  private async snapshotUnlocked(): Promise<AutonomousExecutionJournalSnapshot> {
    const rows = this.rows.map(clone);
    const descriptor = { schema: AUTONOMOUS_EXECUTION_SNAPSHOT_SCHEMA, rows, head_digest: rows.at(-1)?.event_digest ?? "", retention: "metadata_only_hash_chained" as const, secret_material: "never_returned" as const };
    return clone({ ...descriptor, snapshot_digest: await digestJson(descriptor) });
  }

  private async verifyIntegrityUnlocked(): Promise<{ schema: typeof AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> {
    const validated = await this.validateRows(this.rows);
    if (validated.totalBytes !== this.totalBytes) throw new AutonomousExecutionError("execution journal byte accounting is inconsistent");
    return { schema: AUTONOMOUS_EXECUTION_JOURNAL_SCHEMA, verified: true, events: this.rows.length, head_digest: validated.headDigest, retention: "metadata_only" };
  }

  private async validateRows(rows: readonly AutonomousExecutionJournalRow[]): Promise<{ headDigest: string; totalBytes: number }> {
    if (rows.length > this.maxEvents) throw new AutonomousExecutionError("execution journal snapshot event count exceeds its capacity");
    let previous = "";
    let totalBytes = 0;
    for (let index = 0; index < rows.length; index += 1) {
      const row = rows[index]!;
      if (!isObject(row) || row.schema !== AUTONOMOUS_EXECUTION_EVENT_SCHEMA || row.sequence !== index + 1 || row.previous_digest !== previous || typeof row.event_digest !== "string" || !/^([0-9a-f]{64})$/.test(row.event_digest) || !Number.isSafeInteger(row.created_at) || row.created_at < 0) throw new AutonomousExecutionError("execution journal hash chain sequence is invalid");
      const event = validateEvent(row.event, this.validationPolicy);
      const descriptor = { schema: AUTONOMOUS_EXECUTION_EVENT_SCHEMA, sequence: row.sequence, event, previous_digest: row.previous_digest, created_at: row.created_at };
      if (await digestJson(descriptor) !== row.event_digest) throw new AutonomousExecutionError("execution journal hash chain digest is invalid");
      totalBytes += new TextEncoder().encode(JSON.stringify(row)).byteLength;
      if (totalBytes > this.maxBytes) throw new AutonomousExecutionError("execution journal snapshot exceeds its byte capacity");
      previous = row.event_digest;
    }
    return { headDigest: previous, totalBytes };
  }

  private enqueueJournal<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.appendOperation.then(operation);
    this.appendOperation = run.then(() => undefined, () => undefined);
    return run;
  }
}

/** Coordinates an integrity-checked execution journal with a caller-owned durable adapter. */
export class AutonomousExecutionPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly journal: AutonomousExecutionSnapshotJournal, readonly persistence: AutonomousExecutionSnapshotPersistence) {
    if (!journal || typeof journal.snapshot !== "function" || typeof journal.restore !== "function") throw new AutonomousExecutionError("execution persistence requires a snapshot-capable journal");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousExecutionError("execution persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousExecutionJournalSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = await validateAutonomousExecutionJournalSnapshot(raw);
      await this.journal.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  async flush(): Promise<AutonomousExecutionJournalSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await validateAutonomousExecutionJournalSnapshot(await this.journal.snapshot());
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new AutonomousExecutionError("execution persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
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

export class JsonAutonomousExecutionSnapshotPersistence implements AutonomousExecutionSnapshotPersistence {
  constructor(readonly textStore: AutonomousExecutionSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new AutonomousExecutionError("execution text store is malformed");
  }

  async read(): Promise<AutonomousExecutionJournalSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_EXECUTION_MAX_JOURNAL_BYTES) throw new AutonomousExecutionError("execution JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new AutonomousExecutionError("execution JSON is invalid"); }
    return validateAutonomousExecutionJournalSnapshot(parsed);
  }

  async write(raw: AutonomousExecutionJournalSnapshot): Promise<void> {
    const snapshot = await validateAutonomousExecutionJournalSnapshot(raw);
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousExecutionSnapshotPersistence extends JsonAutonomousExecutionSnapshotPersistence {
  declare readonly textStore: AutonomousExecutionTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousExecutionTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new AutonomousExecutionError("execution text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, raw: AutonomousExecutionJournalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new AutonomousExecutionError("execution expected snapshot digest is invalid");
    const snapshot = await validateAutonomousExecutionJournalSnapshot(raw);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}

export class AutonomousExecutionController {
  readonly policy: AutonomousExecutionPolicy;
  readonly journal?: AutonomousExecutionJournal;
  private stateValue: AutonomousExecutionState;
  private terminal = false;
  private operation: Promise<void> = Promise.resolve();

  private constructor(policy: AutonomousExecutionPolicy, state: AutonomousExecutionState, journal?: AutonomousExecutionJournal) {
    this.policy = policy;
    this.stateValue = state;
    this.journal = journal;
    this.terminal = AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(state.status as AutonomousExecutionTerminalStatus);
  }

  static async create(options: AutonomousExecutionControllerOptions): Promise<AutonomousExecutionController> {
    if (!isObject(options)) throw new AutonomousExecutionError("execution controller options must be an object");
    const policy = normalizeAutonomousExecutionPolicy(options.policy);
    const executionId = boundedIdentifier("executionId", options.executionId, 256);
    const domain = boundedIdentifier("execution domain", options.domain);
    const capability = boundedIdentifier("execution capability", options.capability);
    const riskClass = boundedIdentifier("execution riskClass", options.riskClass);
    if (options.journal !== undefined && (!options.journal || typeof options.journal.append !== "function" || typeof options.journal.state !== "function")) throw new AutonomousExecutionError("execution journal is malformed");
    const policyDigest = await policy.digest();
    const prior = options.journal ? await options.journal.state(executionId) : null;
    let state: AutonomousExecutionState;
    let kind: AutonomousExecutionEventKind;
    if (prior) {
      if (options.resume !== true) throw new AutonomousExecutionPolicyError("execution id already exists; resume must be explicit");
      if (prior.policy_digest !== policyDigest) throw new AutonomousExecutionPolicyError("resume policy digest does not match persisted execution");
      if ((AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(prior.status as AutonomousExecutionTerminalStatus) && prior.status !== "reconciliation_required") || prior.last_event_kind === "completed" || prior.last_event_kind === "failed") throw new AutonomousExecutionPolicyError("terminal execution cannot be resumed");
      state = { ...validateState(prior, policy), status: prior.status === "reconciliation_required" ? "reconciliation_required" : "resumed", last_event_kind: "resumed" };
      kind = "resumed";
    } else {
      if (options.resume === true && !options.journal) throw new AutonomousExecutionPolicyError("resume requires a journal");
      state = {
        schema: AUTONOMOUS_EXECUTION_STATE_SCHEMA,
        execution_id: executionId,
        domain,
        capability,
        risk_class: riskClass,
        policy_digest: policyDigest,
        step_index: 0,
        provider_calls: 0,
        provider_failovers: 0,
        tool_calls: 0,
        effectful_calls: 0,
        cost_units: 0,
        replans: 0,
        status: "started",
        last_event_kind: "started",
        last_tool: null,
        last_call_id: null,
        last_outcome_digest: null,
        last_evaluation_digest: null,
        checkpoint_digest: null,
        journal_sequence: 0,
        retention: "metadata_only_no_task_prompt_credentials_or_payloads",
      };
      kind = "started";
    }
    const controller = new AutonomousExecutionController(policy, state, options.journal);
    await controller.persist(kind, state.status);
    return controller;
  }

  get state(): AutonomousExecutionState {
    return clone(this.stateValue);
  }

  async admitProviderCall(options: { costUnits?: number; provider?: string; model?: string; invocationKind?: string; attempt?: number; turn?: number; selectionDigest?: string | null; estimatedCostUnits?: number; failover?: boolean } = {}): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.admitProviderCallUnlocked(options));
  }

  private async admitProviderCallUnlocked(options: { costUnits?: number; provider?: string; model?: string; invocationKind?: string; attempt?: number; turn?: number; selectionDigest?: string | null; estimatedCostUnits?: number; failover?: boolean } = {}): Promise<AutonomousExecutionState> {
    this.ensureActive();
    this.ensureStep();
    if (this.stateValue.provider_calls >= this.policy.max_provider_calls) throw new AutonomousExecutionPolicyError("max_provider_calls exceeded");
    const failover = options.failover === true;
    if (failover && this.stateValue.provider_failovers >= this.policy.max_provider_failovers) throw new AutonomousExecutionPolicyError("max_provider_failovers exceeded");
    const costUnits = boundedCost("provider cost_units", options.costUnits ?? 0);
    this.ensureCost(costUnits);
    const next: AutonomousExecutionState = { ...this.stateValue, step_index: this.stateValue.step_index + 1, provider_calls: this.stateValue.provider_calls + 1, provider_failovers: this.stateValue.provider_failovers + (failover ? 1 : 0), cost_units: this.stateValue.cost_units + costUnits, status: "running", last_event_kind: "provider_call" };
    this.stateValue = next;
    return this.persist("provider_call", "running", {
      provider: options.provider,
      model: options.model,
      invocation_kind: options.invocationKind,
      attempt: options.attempt,
      turn: options.turn,
      selection_digest: options.selectionDigest,
      estimated_cost_units: options.estimatedCostUnits,
      cost_units: costUnits,
      failover,
    });
  }

  async recordProviderOutcome(options: { provider: string; model: string; invocationKind: string; attempt: number; turn: number; status: "completed" | "provider_refused"; outcome: "success" | "failure"; latencyMs: number; inputTokens: number; outputTokens: number; estimatedCostUnits: number; actualCostUnits: number; selectionDigest?: string | null; outcomeDigest: string; requestIdDigest?: string | null; failureClass?: string | null; statusCode?: number | null; retryable?: boolean }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.recordProviderOutcomeUnlocked(options));
  }

  private async recordProviderOutcomeUnlocked(options: { provider: string; model: string; invocationKind: string; attempt: number; turn: number; status: "completed" | "provider_refused"; outcome: "success" | "failure"; latencyMs: number; inputTokens: number; outputTokens: number; estimatedCostUnits: number; actualCostUnits: number; selectionDigest?: string | null; outcomeDigest: string; requestIdDigest?: string | null; failureClass?: string | null; statusCode?: number | null; retryable?: boolean }): Promise<AutonomousExecutionState> {
    this.ensureActive();
    if (options.outcome !== "success" && options.outcome !== "failure") throw new AutonomousExecutionError("provider outcome must be success or failure");
    if (options.status !== "completed" && options.status !== "provider_refused") throw new AutonomousExecutionError("provider outcome status is unsupported");
    boundedText("provider", options.provider);
    boundedText("model", options.model);
    boundedIdentifier("invocationKind", options.invocationKind, 128);
    boundedInteger("provider attempt", options.attempt, 64);
    boundedInteger("provider turn", options.turn, 64);
    boundedCost("provider latencyMs", options.latencyMs, Number.MAX_SAFE_INTEGER);
    boundedInteger("provider inputTokens", options.inputTokens, 100_000_000);
    boundedInteger("provider outputTokens", options.outputTokens, 100_000_000);
    boundedCost("provider estimatedCostUnits", options.estimatedCostUnits);
    boundedCost("provider actualCostUnits", options.actualCostUnits);
    boundedDigest("provider selectionDigest", options.selectionDigest, true);
    boundedDigest("provider outcomeDigest", options.outcomeDigest);
    boundedDigest("provider requestIdDigest", options.requestIdDigest, true);
    if (options.failureClass !== undefined && options.failureClass !== null) boundedIdentifier("provider failureClass", options.failureClass);
    if (options.statusCode !== undefined && options.statusCode !== null) boundedInteger("provider statusCode", options.statusCode, 999);
    if (options.retryable !== undefined) boundedBoolean("provider retryable", options.retryable);
    // The provider outcome label is an event-level result, not the lifecycle of the
    // enclosing execution. A successful turn must not make later tool/provider turns
    // or an explicit resume look terminal.
    const lifecycleStatus = options.outcome === "failure" && options.retryable !== true && this.policy.stop_on_error ? "error" : "running";
    this.stateValue = { ...this.stateValue, last_event_kind: "provider_call", last_outcome_digest: options.outcomeDigest, status: lifecycleStatus };
    return this.persist("provider_call", options.status, { provider: options.provider, model: options.model, invocation_kind: options.invocationKind, attempt: options.attempt, turn: options.turn, provider_outcome: options.outcome, latency_ms: options.latencyMs, input_tokens: options.inputTokens, output_tokens: options.outputTokens, estimated_cost_units: options.estimatedCostUnits, actual_cost_units: options.actualCostUnits, selection_digest: options.selectionDigest, outcome_digest: options.outcomeDigest, request_id_digest: options.requestIdDigest, failure_class: options.failureClass, status_code: options.statusCode, retryable: options.retryable });
  }

  async admitToolCall(options: { tool: string; callId: string; readOnly: boolean; approvalRequired: boolean; costUnits?: number }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.admitToolCallUnlocked(options));
  }

  private async admitToolCallUnlocked(options: { tool: string; callId: string; readOnly: boolean; approvalRequired: boolean; costUnits?: number }): Promise<AutonomousExecutionState> {
    this.ensureActive();
    this.ensureStep();
    if (this.stateValue.tool_calls >= this.policy.max_tool_calls) throw new AutonomousExecutionPolicyError("max_tool_calls exceeded");
    const readOnly = boundedBoolean("tool readOnly", options.readOnly);
    const approvalRequired = boundedBoolean("tool approvalRequired", options.approvalRequired);
    if (!readOnly && !this.policy.allow_side_effects) throw new AutonomousExecutionPolicyError("side effects are disabled by the execution policy");
    if (!readOnly && this.stateValue.effectful_calls >= this.policy.max_effectful_calls) throw new AutonomousExecutionPolicyError("max_effectful_calls exceeded");
    const tool = boundedIdentifier("tool", options.tool);
    const callId = boundedIdentifier("callId", options.callId, 512);
    const costUnits = boundedCost("tool cost_units", options.costUnits ?? 0);
    this.ensureCost(costUnits);
    const lifecycleStatus = approvalRequired && this.policy.pause_on_approval ? "approval_required" : "running";
    this.stateValue = { ...this.stateValue, step_index: this.stateValue.step_index + 1, tool_calls: this.stateValue.tool_calls + 1, effectful_calls: this.stateValue.effectful_calls + (readOnly ? 0 : 1), cost_units: this.stateValue.cost_units + costUnits, last_tool: tool, last_call_id: callId, last_event_kind: "tool_intent", status: lifecycleStatus };
    return this.persist("tool_intent", this.stateValue.status, { tool, call_id: callId, read_only: readOnly, approval_required: approvalRequired, cost_units: costUnits });
  }

  async recordToolOutcome(options: { tool: string; callId: string; status: string; outcomeDigest?: string | null; reason?: string | null }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.recordToolOutcomeUnlocked(options));
  }

  private async recordToolOutcomeUnlocked(options: { tool: string; callId: string; status: string; outcomeDigest?: string | null; reason?: string | null }): Promise<AutonomousExecutionState> {
    const isReconciliationOutcome = this.stateValue.status === "reconciliation_required" && options.status === "reconciliation_required";
    if (!isReconciliationOutcome) this.ensureActive();
    const tool = boundedIdentifier("tool", options.tool);
    const callId = boundedIdentifier("callId", options.callId, 512);
    const outcomeDigest = boundedDigest("tool outcomeDigest", options.outcomeDigest, true);
    const reason = options.reason === undefined || options.reason === null ? null : boundedIdentifier("tool reason", options.reason, 2_048);
    const outcomeStatus = boundedIdentifier("tool outcome status", options.status);
    const lifecycleStatus = outcomeStatus === "reconciliation_required"
      ? "reconciliation_required"
      : outcomeStatus === "authorization_required"
      ? (this.policy.pause_on_approval ? "approval_required" : "running")
      : outcomeStatus === "failed" && this.policy.stop_on_error ? "error" : "running";
    this.stateValue = { ...this.stateValue, last_tool: tool, last_call_id: callId, last_outcome_digest: outcomeDigest, last_event_kind: "tool_outcome", status: lifecycleStatus };
    return this.persist("tool_outcome", outcomeStatus, { tool, call_id: callId, outcome_digest: outcomeDigest, reason });
  }

  /** Record the external-effect state machine without retaining effect arguments or outputs. */
  async recordEffectReconciliation(options: { effectId: string; tool: string; callId: string; status: "prepared" | "dispatching" | "dispatched" | "completed" | "uncertain" | "reconciled" | "failed"; dispatchAttempt: number; resultDigest?: string | null; failureClass?: string | null; reason?: string | null }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.recordEffectReconciliationUnlocked(options));
  }

  private async recordEffectReconciliationUnlocked(options: { effectId: string; tool: string; callId: string; status: "prepared" | "dispatching" | "dispatched" | "completed" | "uncertain" | "reconciled" | "failed"; dispatchAttempt: number; resultDigest?: string | null; failureClass?: string | null; reason?: string | null }): Promise<AutonomousExecutionState> {
    // An uncertain effect is a recoverable terminal boundary. It must be possible to
    // reconcile it with the same controller after a restart, but no other terminal state
    // may be reopened by an effect callback.
    if (this.terminal && this.stateValue.status !== "reconciliation_required") throw new AutonomousExecutionPolicyError("execution is terminal");
    if (AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(this.stateValue.status as AutonomousExecutionTerminalStatus) && this.stateValue.status !== "reconciliation_required") throw new AutonomousExecutionPolicyError("execution cannot record an effect after terminal completion");
    const effectId = boundedIdentifier("effectId", options.effectId, 128);
    const tool = boundedIdentifier("effect tool", options.tool);
    const callId = boundedIdentifier("effect callId", options.callId, 512);
    boundedInteger("effect dispatchAttempt", options.dispatchAttempt, 64);
    if (!["prepared", "dispatching", "dispatched", "completed", "uncertain", "reconciled", "failed"].includes(options.status)) throw new AutonomousExecutionError("effect reconciliation status is unsupported");
    const resultDigest = boundedDigest("effect resultDigest", options.resultDigest, true);
    if (options.failureClass !== undefined && options.failureClass !== null) boundedIdentifier("effect failureClass", options.failureClass, 256);
    const reason = options.reason === undefined || options.reason === null ? null : boundedIdentifier("effect reason", options.reason, 2_048);
    const lifecycleStatus = options.status === "uncertain"
      ? "reconciliation_required"
      : options.status === "failed" && this.policy.stop_on_error
        ? "error"
        : "running";
    this.stateValue = { ...this.stateValue, last_tool: tool, last_call_id: callId, last_outcome_digest: resultDigest, last_event_kind: "effect_reconciliation", status: lifecycleStatus };
    if (options.status === "reconciled" || options.status === "completed" || options.status === "failed") this.terminal = false;
    const reconciliationDigest = await digestJson({ effect_id: effectId, status: options.status, dispatch_attempt: options.dispatchAttempt, result_digest: resultDigest, failure_class: options.failureClass ?? null, reason });
    return this.persist("effect_reconciliation", options.status, { effect_id: effectId, effect_status: options.status, tool, call_id: callId, dispatch_attempt: options.dispatchAttempt, reconciliation_digest: reconciliationDigest, outcome_digest: resultDigest, failure_class: options.failureClass, reason });
  }

  async recordEvaluation(options: { evaluatorId: string; evaluatorVersion: string; reward: number; passed: boolean; evaluationDigest: string; failureClass?: string | null }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.recordEvaluationUnlocked(options));
  }

  private async recordEvaluationUnlocked(options: { evaluatorId: string; evaluatorVersion: string; reward: number; passed: boolean; evaluationDigest: string; failureClass?: string | null }): Promise<AutonomousExecutionState> {
    this.ensureActive();
    boundedIdentifier("evaluatorId", options.evaluatorId);
    boundedIdentifier("evaluatorVersion", options.evaluatorVersion);
    boundedCost("evaluation reward", options.reward, 1);
    boundedBoolean("evaluation passed", options.passed);
    const evaluationDigest = boundedDigest("evaluationDigest", options.evaluationDigest)!;
    if (options.failureClass !== undefined && options.failureClass !== null) boundedIdentifier("evaluation failureClass", options.failureClass);
    this.stateValue = { ...this.stateValue, last_evaluation_digest: evaluationDigest, last_event_kind: "evaluation", status: "evaluated" };
    return this.persist("evaluation", "evaluated", { evaluator_id: options.evaluatorId, evaluator_version: options.evaluatorVersion, reward: options.reward, passed: options.passed, evaluation_digest: evaluationDigest, failure_class: options.failureClass });
  }

  async replan(options: { instructionDigest?: string | null; reason?: string | null; attempt?: number }): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.replanUnlocked(options));
  }

  private async replanUnlocked(options: { instructionDigest?: string | null; reason?: string | null; attempt?: number }): Promise<AutonomousExecutionState> {
    this.ensureActive();
    if (this.stateValue.replans >= this.policy.max_replans) throw new AutonomousExecutionPolicyError("max_replans exceeded");
    const instructionDigest = boundedDigest("replan instructionDigest", options.instructionDigest, true);
    const reason = options.reason === undefined || options.reason === null ? null : boundedIdentifier("replan reason", options.reason, 2_048);
    if (options.attempt !== undefined) boundedInteger("replan attempt", options.attempt, 64);
    this.stateValue = { ...this.stateValue, replans: this.stateValue.replans + 1, last_event_kind: "replan", status: "replanning" };
    return this.persist("replan", "replanning", { instruction_digest: instructionDigest, reason, attempt: options.attempt });
  }

  async checkpoint(options: { status?: string; reason?: string | null } = {}): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.checkpointUnlocked(options));
  }

  private async checkpointUnlocked(options: { status?: string; reason?: string | null } = {}): Promise<AutonomousExecutionState> {
    this.ensureActive();
    const status = boundedIdentifier("checkpoint status", options.status ?? "paused");
    if (AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(status as AutonomousExecutionTerminalStatus)) throw new AutonomousExecutionPolicyError("checkpoint status cannot be terminal");
    const reason = options.reason === undefined || options.reason === null ? null : boundedIdentifier("checkpoint reason", options.reason, 2_048);
    this.stateValue = { ...this.stateValue, status, last_event_kind: "checkpoint" };
    return this.persist("checkpoint", status, { reason });
  }

  async complete(status = "completed"): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.completeUnlocked(status));
  }

  private async completeUnlocked(status = "completed"): Promise<AutonomousExecutionState> {
    this.ensureActive();
    const normalized = boundedIdentifier("completion status", status);
    this.stateValue = { ...this.stateValue, status: normalized, last_event_kind: "completed" };
    const result = await this.persist("completed", normalized);
    this.terminal = true;
    return result;
  }

  async fail(reason: string, status = "failed"): Promise<AutonomousExecutionState> {
    return this.enqueue(() => this.failUnlocked(reason, status));
  }

  private async failUnlocked(reason: string, status = "failed"): Promise<AutonomousExecutionState> {
    if (this.terminal || AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(this.stateValue.status as AutonomousExecutionTerminalStatus)) throw new AutonomousExecutionPolicyError("execution is terminal");
    const normalizedReason = boundedIdentifier("failure reason", reason, 2_048);
    const normalized = boundedIdentifier("failure status", status);
    this.stateValue = { ...this.stateValue, status: normalized, last_event_kind: "failed" };
    const result = await this.persist("failed", normalized, { reason: normalizedReason });
    this.terminal = true;
    return result;
  }

  toJSON(): { schema: typeof AUTONOMOUS_EXECUTION_STATE_SCHEMA; policy: AutonomousExecutionPolicyProjection; state: AutonomousExecutionState; retention: "metadata_only" } {
    return { schema: AUTONOMOUS_EXECUTION_STATE_SCHEMA, policy: this.policy.toJSON(), state: this.state, retention: "metadata_only" };
  }

  private ensureActive(): void {
    if (this.terminal || AUTONOMOUS_EXECUTION_TERMINAL_STATUSES.includes(this.stateValue.status as AutonomousExecutionTerminalStatus) || (this.policy.stop_on_error && this.stateValue.status === "error")) throw new AutonomousExecutionPolicyError("execution is terminal or halted");
  }

  private ensureStep(): void {
    if (this.stateValue.step_index >= this.policy.max_steps) throw new AutonomousExecutionPolicyError("max_steps exceeded");
  }

  private ensureCost(costUnits: number): void {
    if (this.stateValue.cost_units + costUnits > this.policy.max_cost_units) throw new AutonomousExecutionPolicyError("max_cost_units exceeded");
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.operation.then(operation);
    this.operation = run.then(() => undefined, () => undefined);
    return run;
  }

  private async persist(kind: AutonomousExecutionEventKind, status: string, fields: Record<string, unknown> = {}): Promise<AutonomousExecutionState> {
    const presentFields = Object.fromEntries(Object.entries(fields).filter(([, value]) => value !== undefined));
    const event: AutonomousExecutionEvent = { schema: AUTONOMOUS_EXECUTION_EVENT_SCHEMA, execution_id: this.stateValue.execution_id, kind, domain: this.stateValue.domain, capability: this.stateValue.capability, risk_class: this.stateValue.risk_class, status, policy_digest: this.stateValue.policy_digest, state: this.stateValue, ...presentFields };
    if (this.journal) {
      const receipt = await this.journal.append(event);
      this.stateValue = { ...this.stateValue, journal_sequence: receipt.sequence, checkpoint_digest: receipt.event_digest };
    }
    return this.state;
  }
}
