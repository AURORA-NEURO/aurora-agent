import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import type { JsonObject, JsonValue } from "./types.js";
import type { ProviderToolCall } from "./llm.js";

/** Metadata-only effect state; raw arguments, results, credentials, and provider bodies never enter this ledger. */
export const AUTONOMOUS_EFFECT_SCHEMA = "bioprism-typescript-autonomous-effect/0.1" as const;
export const AUTONOMOUS_EFFECT_EVENT_SCHEMA = "bioprism-typescript-autonomous-effect-event/0.1" as const;
export const AUTONOMOUS_EFFECT_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-effect-journal/0.1" as const;
export const AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-effect-snapshot/0.1" as const;

export const AUTONOMOUS_EFFECT_STATUSES = [
  "prepared",
  "dispatching",
  "dispatched",
  "completed",
  "uncertain",
  "reconciled",
  "failed",
] as const;
export const AUTONOMOUS_EFFECT_MAX_EVENTS = 32_768;
export const AUTONOMOUS_EFFECT_MAX_JOURNAL_BYTES = 64_000_000;
export const AUTONOMOUS_EFFECT_MAX_EVENT_BYTES = 64_000;
export const AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES = 2_000_000;
export const AUTONOMOUS_EFFECT_MAX_REASON_BYTES = 2_048;

export type AutonomousEffectStatus = typeof AUTONOMOUS_EFFECT_STATUSES[number];

export class AutonomousEffectError extends ArgumentError {
  override readonly name: string = "AutonomousEffectError";
}

export class AutonomousEffectPolicyError extends AutonomousEffectError {
  override readonly name = "AutonomousEffectPolicyError";
}

/** Raised when a previous dispatch may have reached the external system and a resolver is required. */
export class AutonomousEffectReconciliationRequiredError extends AutonomousEffectError {
  override readonly name = "AutonomousEffectReconciliationRequiredError";
  readonly effectId: string;
  readonly idempotencyKey: string;
  readonly status: AutonomousEffectStatus;

  constructor(effectId: string, idempotencyKey: string, status: AutonomousEffectStatus) {
    super(`effect ${effectId} is ${status}; caller-owned reconciliation is required before retry`);
    this.effectId = effectId;
    this.idempotencyKey = idempotencyKey;
    this.status = status;
  }
}

/** A bounded error used after a definite external failure; raw error text is intentionally discarded. */
export class AutonomousEffectExecutionError extends AutonomousEffectError {
  override readonly name = "AutonomousEffectExecutionError";
  readonly effectId: string;
  readonly failureClass: string;

  constructor(effectId: string, failureClass: string) {
    super(`effect ${effectId} failed at the external boundary (${failureClass})`);
    this.effectId = effectId;
    this.failureClass = failureClass;
  }
}

export interface AutonomousEffectRecord {
  schema: typeof AUTONOMOUS_EFFECT_SCHEMA;
  effect_id: string;
  execution_id: string | null;
  tool: string;
  call_id: string;
  risk_class: string;
  arguments_digest: string;
  idempotency_key_digest: string;
  status: AutonomousEffectStatus;
  dispatch_attempt: number;
  result_digest: string | null;
  failure_class: string | null;
  reason: string | null;
  last_sequence: number;
  last_event_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
}

export interface AutonomousEffectEvent {
  schema: typeof AUTONOMOUS_EFFECT_EVENT_SCHEMA;
  effect_id: string;
  execution_id: string | null;
  tool: string;
  call_id: string;
  risk_class: string;
  arguments_digest: string;
  idempotency_key_digest: string;
  status: AutonomousEffectStatus;
  dispatch_attempt: number;
  result_digest?: string | null;
  failure_class?: string | null;
  reason?: string | null;
  metadata?: JsonObject;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
}

export interface AutonomousEffectJournalRow {
  schema: typeof AUTONOMOUS_EFFECT_EVENT_SCHEMA;
  sequence: number;
  event: AutonomousEffectEvent;
  previous_digest: string;
  created_at: number;
  event_digest: string;
}

export interface AutonomousEffectJournalReceipt {
  schema: typeof AUTONOMOUS_EFFECT_JOURNAL_SCHEMA;
  sequence: number;
  event_digest: string;
  head_digest: string;
  effect_id: string;
  status: AutonomousEffectStatus;
  retention: "metadata_only_hash_chained";
}

export interface AutonomousEffectJournalSnapshot {
  schema: typeof AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA;
  rows: AutonomousEffectJournalRow[];
  head_digest: string;
  retention: "metadata_only_hash_chained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousEffectJournal {
  append(event: AutonomousEffectEvent): Promise<AutonomousEffectJournalReceipt> | AutonomousEffectJournalReceipt;
  get(effectId: string): Promise<AutonomousEffectRecord | null> | AutonomousEffectRecord | null;
  events(options?: { effectId?: string; afterSequence?: number; limit?: number }): Promise<AutonomousEffectJournalRow[]> | AutonomousEffectJournalRow[];
  verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_EFFECT_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> | { schema: typeof AUTONOMOUS_EFFECT_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" };
}

export interface AutonomousEffectSnapshotJournal extends AutonomousEffectJournal {
  snapshot(): Promise<AutonomousEffectJournalSnapshot>;
  restore(snapshot: AutonomousEffectJournalSnapshot): Promise<void>;
}

export interface AutonomousEffectSnapshotPersistence {
  read(): Promise<AutonomousEffectJournalSnapshot | null> | AutonomousEffectJournalSnapshot | null;
  write(snapshot: AutonomousEffectJournalSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousEffectJournalSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousEffectSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEffectTransactionalSnapshotTextStore extends AutonomousEffectSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousEffectResolution {
  status: "completed" | "failed" | "not_found" | "unknown";
  result?: JsonValue;
  failure_class?: string;
  reason?: string;
  /** A not_found response may authorize a fresh dispatch only when the resolver proves it is safe. */
  retry_safe?: boolean;
}

export interface AutonomousEffectResolver {
  resolve(record: AutonomousEffectRecord): Promise<AutonomousEffectResolution> | AutonomousEffectResolution;
}

export interface AutonomousEffectExecutionContext {
  effect_id: string;
  execution_id: string | null;
  tool: string;
  call_id: string;
  risk_class: string;
  idempotency_key: string;
  dispatch_attempt: number;
}

export interface AutonomousEffectRequest {
  execution_id?: string | null;
  tool: string;
  call_id: string;
  risk_class: string;
  arguments: JsonObject;
}

export interface AutonomousEffectBoundaryOptions {
  journal?: AutonomousEffectSnapshotJournal;
  resolver?: AutonomousEffectResolver;
  execution?: AutonomousExecutionController;
  clock?: () => number;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) throw new AutonomousEffectError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum = 512): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousEffectError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === undefined || value === null)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousEffectError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedInteger(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) throw new AutonomousEffectError(`${name} must be an integer within [0, ${maximum}]`);
  return value as number;
}

function assertJsonMetadata(value: unknown, depth = 0): void {
  if (depth > 24) throw new AutonomousEffectError("effect metadata is too deeply nested");
  if (value === undefined || typeof value === "function" || typeof value === "symbol" || typeof value === "bigint") throw new AutonomousEffectError("effect metadata contains a non-JSON value");
  if (Array.isArray(value)) {
    for (const child of value) assertJsonMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "password", "secret", "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response", "rawpayload", "arguments", "output", "task", "messages"].includes(normalized)) throw new AutonomousEffectError("effect metadata contains transient or secret-shaped fields");
      assertJsonMetadata(child, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new AutonomousEffectError("effect metadata contains a non-finite number");
}

function assertMetadataBytes(name: string, value: unknown, maximum: number): void {
  assertJsonMetadata(value);
  let encoded: string | undefined;
  try { encoded = JSON.stringify(value); } catch { throw new AutonomousEffectError(`${name} must be JSON serializable`); }
  if (encoded === undefined) throw new AutonomousEffectError(`${name} must be JSON serializable`);
  if (new TextEncoder().encode(encoded).byteLength > maximum) throw new AutonomousEffectError(`${name} exceeds its bounded byte size`);
}

function normalizeStatus(value: unknown): AutonomousEffectStatus {
  if (typeof value !== "string" || !(AUTONOMOUS_EFFECT_STATUSES as readonly string[]).includes(value)) throw new AutonomousEffectError("effect status is unsupported");
  return value as AutonomousEffectStatus;
}

function validateEvent(event: AutonomousEffectEvent): AutonomousEffectEvent {
  if (!isObject(event) || event.schema !== AUTONOMOUS_EFFECT_EVENT_SCHEMA) throw new AutonomousEffectError("effect event schema is unsupported");
  const allowed = new Set(["schema", "effect_id", "execution_id", "tool", "call_id", "risk_class", "arguments_digest", "idempotency_key_digest", "status", "dispatch_attempt", "result_digest", "failure_class", "reason", "metadata", "retention"]);
  if (Object.keys(event).some((key) => !allowed.has(key))) throw new AutonomousEffectError("effect event contains unsupported fields");
  boundedIdentifier("effect_id", event.effect_id, 128);
  if (event.execution_id !== null) boundedIdentifier("execution_id", event.execution_id, 256);
  boundedIdentifier("effect tool", event.tool, 512);
  boundedIdentifier("effect call_id", event.call_id, 512);
  boundedIdentifier("effect risk_class", event.risk_class, 256);
  boundedDigest("effect arguments_digest", event.arguments_digest);
  boundedDigest("effect idempotency_key_digest", event.idempotency_key_digest);
  normalizeStatus(event.status);
  boundedInteger("effect dispatch_attempt", event.dispatch_attempt, 64);
  if (event.result_digest !== undefined) boundedDigest("effect result_digest", event.result_digest, true);
  if (event.failure_class !== undefined && event.failure_class !== null) boundedIdentifier("effect failure_class", event.failure_class, 256);
  if (event.reason !== undefined && event.reason !== null) boundedText("effect reason", event.reason, AUTONOMOUS_EFFECT_MAX_REASON_BYTES);
  if (event.metadata !== undefined) assertMetadataBytes("effect metadata", event.metadata, 8_192);
  if (event.retention !== "metadata_only_no_arguments_outputs_credentials_or_provider_material") throw new AutonomousEffectError("effect event retention declaration is invalid");
  assertMetadataBytes("effect event", event, AUTONOMOUS_EFFECT_MAX_EVENT_BYTES);
  return clone(event);
}

/** Validate the full effect hash chain before a snapshot is restored or persisted. */
export async function validateAutonomousEffectJournalSnapshot(value: unknown): Promise<AutonomousEffectJournalSnapshot> {
  if (!isObject(value) || Object.keys(value).sort().join(",") !== "head_digest,retention,rows,schema,secret_material,snapshot_digest" || value.schema !== AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA || !Array.isArray(value.rows) || value.retention !== "metadata_only_hash_chained" || value.secret_material !== "never_returned") throw new AutonomousEffectError("effect journal snapshot is malformed");
  if (value.rows.length > AUTONOMOUS_EFFECT_MAX_EVENTS) throw new AutonomousEffectError("effect journal snapshot event count exceeds its capacity");
  if (value.head_digest !== "") boundedDigest("effect snapshot head_digest", value.head_digest);
  boundedDigest("effect snapshot snapshot_digest", value.snapshot_digest);
  const { snapshot_digest: observed, ...descriptor } = value;
  if (await digestJson(descriptor) !== observed) throw new AutonomousEffectError("effect journal snapshot digest does not match");
  let previous = "";
  let totalBytes = 0;
  for (let index = 0; index < value.rows.length; index += 1) {
    const row = value.rows[index] as unknown as AutonomousEffectJournalRow;
    if (!isObject(row) || Object.keys(row).sort().join(",") !== "created_at,event,event_digest,previous_digest,schema,sequence" || row.schema !== AUTONOMOUS_EFFECT_EVENT_SCHEMA || row.sequence !== index + 1 || row.previous_digest !== previous || typeof row.event_digest !== "string" || !/^[0-9a-f]{64}$/.test(row.event_digest) || !Number.isSafeInteger(row.created_at) || row.created_at < 0) throw new AutonomousEffectError("effect journal hash chain sequence is invalid");
    const event = validateEvent(row.event);
    const rowDescriptor = { schema: AUTONOMOUS_EFFECT_EVENT_SCHEMA, sequence: row.sequence, event, previous_digest: row.previous_digest, created_at: row.created_at };
    if (await digestJson(rowDescriptor) !== row.event_digest) throw new AutonomousEffectError("effect journal hash chain digest is invalid");
    totalBytes += new TextEncoder().encode(JSON.stringify(row)).byteLength;
    if (totalBytes > AUTONOMOUS_EFFECT_MAX_JOURNAL_BYTES) throw new AutonomousEffectError("effect journal snapshot exceeds its byte capacity");
    previous = row.event_digest;
  }
  if (value.head_digest !== previous) throw new AutonomousEffectError("effect journal snapshot head does not match its rows");
  return clone(value) as unknown as AutonomousEffectJournalSnapshot;
}

function recordFromRow(row: AutonomousEffectJournalRow): AutonomousEffectRecord {
  const event = row.event;
  return {
    schema: AUTONOMOUS_EFFECT_SCHEMA,
    effect_id: event.effect_id,
    execution_id: event.execution_id,
    tool: event.tool,
    call_id: event.call_id,
    risk_class: event.risk_class,
    arguments_digest: event.arguments_digest,
    idempotency_key_digest: event.idempotency_key_digest,
    status: event.status,
    dispatch_attempt: event.dispatch_attempt,
    result_digest: event.result_digest ?? null,
    failure_class: event.failure_class ?? null,
    reason: event.reason ?? null,
    last_sequence: row.sequence,
    last_event_digest: row.event_digest,
    retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material",
  };
}

/** In-memory hash-chained effect journal. Production callers can persist its snapshot in SQLite, IndexedDB, or an object store. */
export class InMemoryAutonomousEffectJournal implements AutonomousEffectSnapshotJournal {
  private readonly rows: AutonomousEffectJournalRow[] = [];
  private readonly maxEvents: number;
  private readonly maxBytes: number;
  private readonly clock: () => number;
  private totalBytes = 0;
  private operation: Promise<void> = Promise.resolve();

  constructor(options: { maxEvents?: number; maxBytes?: number; clock?: () => number } = {}) {
    this.maxEvents = boundedInteger("effect journal maxEvents", options.maxEvents ?? AUTONOMOUS_EFFECT_MAX_EVENTS, AUTONOMOUS_EFFECT_MAX_EVENTS);
    if (this.maxEvents < 1) throw new AutonomousEffectError("effect journal maxEvents must be at least one");
    this.maxBytes = boundedInteger("effect journal maxBytes", options.maxBytes ?? AUTONOMOUS_EFFECT_MAX_JOURNAL_BYTES, AUTONOMOUS_EFFECT_MAX_JOURNAL_BYTES);
    if (this.maxBytes < AUTONOMOUS_EFFECT_MAX_EVENT_BYTES) throw new AutonomousEffectError("effect journal maxBytes is below one event capacity");
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new AutonomousEffectError("effect journal clock must be callable");
  }

  async append(event: AutonomousEffectEvent): Promise<AutonomousEffectJournalReceipt> {
    return this.enqueue(() => this.appendUnlocked(event));
  }

  private async appendUnlocked(event: AutonomousEffectEvent): Promise<AutonomousEffectJournalReceipt> {
    const normalized = validateEvent(event);
    if (this.rows.length >= this.maxEvents) throw new AutonomousEffectError("effect journal event capacity is exhausted");
    const createdAt = this.clock();
    if (!Number.isFinite(createdAt) || createdAt < 0) throw new AutonomousEffectError("effect journal clock returned an invalid timestamp");
    const sequence = this.rows.length + 1;
    const previousDigest = this.rows.at(-1)?.event_digest ?? "";
    const descriptor = { schema: AUTONOMOUS_EFFECT_EVENT_SCHEMA, sequence, event: normalized, previous_digest: previousDigest, created_at: Math.floor(createdAt) };
    const eventDigest = await digestJson(descriptor);
    const row: AutonomousEffectJournalRow = { ...descriptor, event_digest: eventDigest };
    const size = new TextEncoder().encode(JSON.stringify(row)).byteLength;
    if (this.totalBytes + size > this.maxBytes) throw new AutonomousEffectError("effect journal byte capacity is exhausted");
    this.rows.push(clone(row));
    this.totalBytes += size;
    return { schema: AUTONOMOUS_EFFECT_JOURNAL_SCHEMA, sequence, event_digest: eventDigest, head_digest: eventDigest, effect_id: normalized.effect_id, status: normalized.status, retention: "metadata_only_hash_chained" };
  }

  get(effectId: string): AutonomousEffectRecord | null {
    boundedIdentifier("effect_id", effectId, 128);
    const row = [...this.rows].reverse().find((candidate) => candidate.event.effect_id === effectId);
    return row ? recordFromRow(clone(row)) : null;
  }

  events(options: { effectId?: string; afterSequence?: number; limit?: number } = {}): AutonomousEffectJournalRow[] {
    if (options.effectId !== undefined) boundedIdentifier("effect_id", options.effectId, 128);
    const after = options.afterSequence ?? 0;
    if (!Number.isSafeInteger(after) || after < 0) throw new AutonomousEffectError("effect journal afterSequence must be a non-negative integer");
    const limit = options.limit ?? Math.min(256, this.maxEvents);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > this.maxEvents) throw new AutonomousEffectError("effect journal limit is outside its bounds");
    return this.rows.filter((row) => row.sequence > after && (options.effectId === undefined || row.event.effect_id === options.effectId)).slice(0, limit).map(clone);
  }

  async verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_EFFECT_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> {
    return this.enqueue(() => this.verifyIntegrityUnlocked());
  }

  async snapshot(): Promise<AutonomousEffectJournalSnapshot> {
    return this.enqueue(async () => {
      const rows = this.rows.map(clone);
      const descriptor = { schema: AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA, rows, head_digest: rows.at(-1)?.event_digest ?? "", retention: "metadata_only_hash_chained" as const, secret_material: "never_returned" as const };
      return clone({ ...descriptor, snapshot_digest: await digestJson(descriptor) });
    });
  }

  async restore(snapshot: AutonomousEffectJournalSnapshot): Promise<void> {
    await this.enqueue(async () => {
      if (!isObject(snapshot) || Object.keys(snapshot).sort().join(",") !== "head_digest,retention,rows,schema,secret_material,snapshot_digest" || snapshot.schema !== AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.rows) || snapshot.retention !== "metadata_only_hash_chained" || snapshot.secret_material !== "never_returned") throw new AutonomousEffectError("effect journal snapshot is malformed");
      if (snapshot.head_digest !== "") boundedDigest("effect snapshot head_digest", snapshot.head_digest);
      boundedDigest("effect snapshot snapshot_digest", snapshot.snapshot_digest);
      const { snapshot_digest: observed, ...descriptor } = snapshot;
      if (await digestJson(descriptor) !== observed) throw new AutonomousEffectError("effect journal snapshot digest does not match");
      const validated = await this.validateRows(snapshot.rows);
      if (validated.headDigest !== snapshot.head_digest) throw new AutonomousEffectError("effect journal snapshot head does not match its rows");
      this.rows.splice(0, this.rows.length, ...snapshot.rows.map(clone));
      this.totalBytes = validated.totalBytes;
    });
  }

  private async verifyIntegrityUnlocked(): Promise<{ schema: typeof AUTONOMOUS_EFFECT_JOURNAL_SCHEMA; verified: true; events: number; head_digest: string; retention: "metadata_only" }> {
    const validated = await this.validateRows(this.rows);
    if (validated.totalBytes !== this.totalBytes) throw new AutonomousEffectError("effect journal byte accounting is inconsistent");
    return { schema: AUTONOMOUS_EFFECT_JOURNAL_SCHEMA, verified: true, events: this.rows.length, head_digest: validated.headDigest, retention: "metadata_only" };
  }

  private async validateRows(rows: readonly AutonomousEffectJournalRow[]): Promise<{ headDigest: string; totalBytes: number }> {
    if (rows.length > this.maxEvents) throw new AutonomousEffectError("effect journal snapshot event count exceeds its capacity");
    let previous = "";
    let totalBytes = 0;
    for (let index = 0; index < rows.length; index += 1) {
      const row = rows[index]!;
      if (!isObject(row) || Object.keys(row).sort().join(",") !== "created_at,event,event_digest,previous_digest,schema,sequence" || row.schema !== AUTONOMOUS_EFFECT_EVENT_SCHEMA || row.sequence !== index + 1 || row.previous_digest !== previous || typeof row.event_digest !== "string" || !/^[0-9a-f]{64}$/.test(row.event_digest) || !Number.isSafeInteger(row.created_at) || row.created_at < 0) throw new AutonomousEffectError("effect journal hash chain sequence is invalid");
      const event = validateEvent(row.event);
      const descriptor = { schema: AUTONOMOUS_EFFECT_EVENT_SCHEMA, sequence: row.sequence, event, previous_digest: row.previous_digest, created_at: row.created_at };
      if (await digestJson(descriptor) !== row.event_digest) throw new AutonomousEffectError("effect journal hash chain digest is invalid");
      totalBytes += new TextEncoder().encode(JSON.stringify(row)).byteLength;
      if (totalBytes > this.maxBytes) throw new AutonomousEffectError("effect journal snapshot exceeds its byte capacity");
      previous = row.event_digest;
    }
    return { headDigest: previous, totalBytes };
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.operation.then(operation);
    this.operation = run.then(() => undefined, () => undefined);
    return run;
  }
}

/** Flushes/restores the effect ledger through caller-owned durable storage after integrity checks. */
export class AutonomousEffectPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly journal: AutonomousEffectSnapshotJournal, readonly persistence: AutonomousEffectSnapshotPersistence) {
    if (!journal || typeof journal.snapshot !== "function" || typeof journal.restore !== "function") throw new AutonomousEffectError("effect persistence requires a snapshot-capable journal");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousEffectError("effect persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousEffectJournalSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = await validateAutonomousEffectJournalSnapshot(raw);
      await this.journal.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  async flush(): Promise<AutonomousEffectJournalSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await validateAutonomousEffectJournalSnapshot(await this.journal.snapshot());
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new AutonomousEffectError("effect persistence compare-and-swap conflict");
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

export class JsonAutonomousEffectSnapshotPersistence implements AutonomousEffectSnapshotPersistence {
  constructor(readonly textStore: AutonomousEffectSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new AutonomousEffectError("effect text store is malformed");
  }

  async read(): Promise<AutonomousEffectJournalSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_EFFECT_MAX_JOURNAL_BYTES) throw new AutonomousEffectError("effect JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new AutonomousEffectError("effect JSON is invalid"); }
    return validateAutonomousEffectJournalSnapshot(parsed);
  }

  async write(raw: AutonomousEffectJournalSnapshot): Promise<void> {
    const snapshot = await validateAutonomousEffectJournalSnapshot(raw);
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousEffectSnapshotPersistence extends JsonAutonomousEffectSnapshotPersistence {
  declare readonly textStore: AutonomousEffectTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousEffectTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new AutonomousEffectError("effect text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, raw: AutonomousEffectJournalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new AutonomousEffectError("effect expected snapshot digest is invalid");
    const snapshot = await validateAutonomousEffectJournalSnapshot(raw);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}

function normalizedFailureClass(error: unknown): string {
  const candidate = error instanceof Error ? error.name : "effect_execution_error";
  return /^[A-Za-z0-9_.:-]{1,256}$/.test(candidate) ? candidate : "effect_execution_error";
}

function normalizedReason(error: unknown): string {
  const candidate = error instanceof Error ? error.constructor.name : "effect_execution_error";
  return /^[A-Za-z0-9_.:-]{1,256}$/.test(candidate) ? candidate : "effect_execution_error";
}

/**
 * Exactly-once is not claimed: external systems must honor the supplied idempotency key.
 * This boundary gives the caller a conservative at-least-once protocol with durable uncertainty
 * and an explicit resolver before a possibly duplicated retry is allowed.
 */
export class AutonomousEffectBoundary {
  readonly journal: AutonomousEffectSnapshotJournal;
  readonly resolver?: AutonomousEffectResolver;
  readonly execution?: AutonomousExecutionController;
  private readonly clock: () => number;
  private readonly resultCache = new Map<string, JsonValue>();
  private readonly operations = new Map<string, Promise<unknown>>();

  constructor(options: AutonomousEffectBoundaryOptions = {}) {
    this.journal = options.journal ?? new InMemoryAutonomousEffectJournal();
    if (typeof this.journal.append !== "function" || typeof this.journal.get !== "function") throw new AutonomousEffectError("effect boundary journal is malformed");
    if (options.resolver !== undefined && typeof options.resolver.resolve !== "function") throw new AutonomousEffectError("effect boundary resolver is malformed");
    this.resolver = options.resolver;
    this.execution = options.execution;
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new AutonomousEffectError("effect boundary clock must be callable");
  }

  async effectId(request: AutonomousEffectRequest): Promise<string> {
    const normalized = this.normalizeRequest(request);
    const argumentsDigest = await digestJson(normalized.arguments);
    return (await digestJson({ schema: AUTONOMOUS_EFFECT_SCHEMA, execution_id: normalized.execution_id, tool: normalized.tool, call_id: normalized.call_id, arguments_digest: argumentsDigest })).slice(0, 64);
  }

  async execute(request: AutonomousEffectRequest, executor: (context: AutonomousEffectExecutionContext) => JsonValue | Promise<JsonValue>, options: { execution?: AutonomousExecutionController } = {}): Promise<JsonValue> {
    if (typeof executor !== "function") throw new AutonomousEffectError("effect executor must be callable");
    const normalized = this.normalizeRequest(request);
    const effectId = await this.effectId(normalized);
    return this.exclusive(effectId, async () => this.executeExclusive(normalized, effectId, executor, options.execution ?? this.execution));
  }

  async reconcile(effectId: string, resolver: AutonomousEffectResolver = this.resolver as AutonomousEffectResolver): Promise<AutonomousEffectRecord> {
    boundedIdentifier("effect_id", effectId, 128);
    if (!resolver || typeof resolver.resolve !== "function") throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), "uncertain");
    const record = await this.journal.get(effectId);
    if (!record) throw new AutonomousEffectError(`effect ${effectId} is not present in the effect ledger`);
    return this.exclusive(effectId, async () => this.reconcileExclusive(record, resolver));
  }

  async authorizeAndExecute(
    calls: readonly ProviderToolCall[],
    options: { approve: (call: ProviderToolCall) => boolean | Promise<boolean>; execute: (call: ProviderToolCall, context?: AutonomousEffectExecutionContext) => JsonValue | Promise<JsonValue>; executionId?: string | null; execution?: AutonomousExecutionController; isReadOnly?: (call: ProviderToolCall) => boolean | Promise<boolean>; riskClass?: (call: ProviderToolCall) => string | Promise<string> },
  ): Promise<ProviderToolResultLike[]> {
    if (!Array.isArray(calls) || calls.length > 128) throw new AutonomousEffectError("effect tool call count is outside its bounds");
    if (typeof options.approve !== "function" || typeof options.execute !== "function") throw new AutonomousEffectError("effect approval and executor callbacks must be callable");
    const results: ProviderToolResultLike[] = [];
    for (const call of calls) {
      const approved = await options.approve(call);
      if (!approved) {
        results.push({ callId: call.id, approved: false, isError: true, content: { status: "authorization_required", tool: call.name, secret_material: "never_returned" } });
        continue;
      }
      const readOnly = options.isReadOnly ? await options.isReadOnly(call) : false;
      if (readOnly) {
        results.push({ callId: call.id, approved: true, content: await options.execute(call) });
        continue;
      }
      const riskClass = options.riskClass ? await options.riskClass(call) : "external_effect";
      try {
        const result = await this.execute({ execution_id: options.executionId ?? null, tool: call.name, call_id: call.id, risk_class: riskClass, arguments: call.arguments }, async (context) => options.execute(call, context), { execution: options.execution });
        results.push({ callId: call.id, approved: true, content: result });
      } catch (error) {
        if (!(error instanceof AutonomousEffectReconciliationRequiredError)) throw error;
        results.push({ callId: call.id, approved: false, isError: true, content: { status: "reconciliation_required", tool: call.name, effect_id: error.effectId, idempotency_key: error.idempotencyKey, secret_material: "never_returned" } });
      }
    }
    return results;
  }

  private normalizeRequest(request: AutonomousEffectRequest): AutonomousEffectRequest {
    if (!isObject(request)) throw new AutonomousEffectError("effect request must be an object");
    const executionId = request.execution_id === undefined || request.execution_id === null ? null : boundedIdentifier("execution_id", request.execution_id, 256);
    const tool = boundedIdentifier("effect tool", request.tool, 512);
    const callId = boundedIdentifier("effect call_id", request.call_id, 512);
    const riskClass = boundedIdentifier("effect risk_class", request.risk_class, 256);
    if (!isObject(request.arguments)) throw new AutonomousEffectError("effect arguments must be a JSON object");
    assertMetadataBytes("effect arguments", request.arguments, AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES);
    return { execution_id: executionId, tool, call_id: callId, risk_class: riskClass, arguments: clone(request.arguments) as JsonObject };
  }

  private async executeExclusive(request: AutonomousEffectRequest, effectId: string, executor: (context: AutonomousEffectExecutionContext) => JsonValue | Promise<JsonValue>, execution?: AutonomousExecutionController): Promise<JsonValue> {
    let record = await this.journal.get(effectId);
    if (record) {
      if (record.status === "completed" || record.status === "reconciled") {
        const cached = this.resultCache.get(effectId);
        if (cached !== undefined) return clone(cached);
        if (!this.resolver) throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
        record = await this.reconcileExclusive(record, this.resolver);
        if (record.status === "reconciled" || record.status === "completed") {
          const resolved = this.resultCache.get(effectId);
          if (resolved !== undefined) return clone(resolved);
        }
      }
      if (["dispatching", "dispatched", "uncertain"].includes(record.status)) {
        if (!this.resolver) throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
        record = await this.reconcileExclusive(record, this.resolver);
        if (record.status === "reconciled" || record.status === "completed") {
          const resolved = this.resultCache.get(effectId);
          if (resolved !== undefined) return clone(resolved);
        }
        if (record.status !== "prepared") throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
      }
      if (record.status === "failed") throw new AutonomousEffectExecutionError(effectId, record.failure_class ?? "previous_effect_failure");
      if (record.arguments_digest !== await digestJson(request.arguments) || record.tool !== request.tool || record.call_id !== request.call_id) throw new AutonomousEffectPolicyError("effect id collides with different call metadata");
    }
    const argumentsDigest = await digestJson(request.arguments);
    const idempotencyKey = this.idempotencyKey(effectId);
    const attempt = (record?.dispatch_attempt ?? 0) + 1;
    await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "prepared", dispatch_attempt: attempt, reason: null }, execution);
    await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "dispatching", dispatch_attempt: attempt }, execution);
    // Persist dispatched before entering user code. A crash after this point is conservatively uncertain.
    await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "dispatched", dispatch_attempt: attempt }, execution);
    const context: AutonomousEffectExecutionContext = { effect_id: effectId, execution_id: request.execution_id ?? null, tool: request.tool, call_id: request.call_id, risk_class: request.risk_class, idempotency_key: idempotencyKey, dispatch_attempt: attempt };
    try {
      const result = await executor(context);
      assertMetadataBytes("effect result", result, AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES);
      const resultDigest = await digestJson(result);
      this.resultCache.set(effectId, clone(result));
      await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "completed", dispatch_attempt: attempt, result_digest: resultDigest }, execution);
      return clone(result);
    } catch (unknownError) {
      const failureClass = normalizedFailureClass(unknownError);
      await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "uncertain", dispatch_attempt: attempt, failure_class: failureClass, reason: normalizedReason(unknownError) }, execution);
      if (unknownError instanceof AutonomousEffectError) throw unknownError;
      throw new AutonomousEffectReconciliationRequiredError(effectId, idempotencyKey, "uncertain");
    }
  }

  private async reconcileExclusive(record: AutonomousEffectRecord, resolver: AutonomousEffectResolver): Promise<AutonomousEffectRecord> {
    const resolution = await resolver.resolve(clone(record));
    if (!isObject(resolution) || !["completed", "failed", "not_found", "unknown"].includes(resolution.status)) throw new AutonomousEffectError("effect resolver returned malformed status");
    const idempotencyKey = this.idempotencyKey(record.effect_id);
    const base: AutonomousEffectRequest = { execution_id: record.execution_id, tool: record.tool, call_id: record.call_id, risk_class: record.risk_class, arguments: {} };
    if (resolution.status === "completed") {
      if (resolution.result === undefined) throw new AutonomousEffectError("completed effect resolution must include a result");
      assertMetadataBytes("resolved effect result", resolution.result, AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES);
      const resultDigest = await digestJson(resolution.result);
      this.resultCache.set(record.effect_id, clone(resolution.result));
      await this.transition({ ...base, effect_id: record.effect_id, arguments_digest: record.arguments_digest, idempotency_key_digest: record.idempotency_key_digest, status: "reconciled", dispatch_attempt: record.dispatch_attempt, result_digest: resultDigest, reason: "resolver_confirmed_completion" });
      return (await this.journal.get(record.effect_id))!;
    }
    if (resolution.status === "failed") {
      const failureClass = resolution.failure_class && /^[A-Za-z0-9_.:-]{1,256}$/.test(resolution.failure_class) ? resolution.failure_class : "resolved_effect_failure";
      const reason = resolution.reason && /^[A-Za-z0-9_.:-]{1,256}$/.test(resolution.reason) ? resolution.reason : "resolver_confirmed_failure";
      await this.transition({ ...base, effect_id: record.effect_id, arguments_digest: record.arguments_digest, idempotency_key_digest: record.idempotency_key_digest, status: "failed", dispatch_attempt: record.dispatch_attempt, failure_class: failureClass, reason });
      return (await this.journal.get(record.effect_id))!;
    }
    if (resolution.status === "not_found" && resolution.retry_safe === true) {
      await this.transition({ ...base, effect_id: record.effect_id, arguments_digest: record.arguments_digest, idempotency_key_digest: record.idempotency_key_digest, status: "prepared", dispatch_attempt: record.dispatch_attempt, reason: "resolver_confirmed_not_found_retry_safe" });
      return (await this.journal.get(record.effect_id))!;
    }
    throw new AutonomousEffectReconciliationRequiredError(record.effect_id, idempotencyKey, record.status);
  }

  private async transition(input: AutonomousEffectRequest & { effect_id: string; arguments_digest: string; idempotency_key_digest: string; status: AutonomousEffectStatus; dispatch_attempt: number; result_digest?: string | null; failure_class?: string | null; reason?: string | null }, execution?: AutonomousExecutionController): Promise<void> {
    const event: AutonomousEffectEvent = {
      schema: AUTONOMOUS_EFFECT_EVENT_SCHEMA,
      effect_id: input.effect_id,
      execution_id: input.execution_id ?? null,
      tool: input.tool,
      call_id: input.call_id,
      risk_class: input.risk_class,
      arguments_digest: input.arguments_digest,
      idempotency_key_digest: input.idempotency_key_digest,
      status: input.status,
      dispatch_attempt: input.dispatch_attempt,
      ...(input.result_digest !== undefined ? { result_digest: input.result_digest } : {}),
      ...(input.failure_class !== undefined ? { failure_class: input.failure_class } : {}),
      ...(input.reason !== undefined ? { reason: input.reason } : {}),
      retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material",
    };
    const receipt = await this.journal.append(event);
    await (execution ?? this.execution)?.recordEffectReconciliation({ effectId: input.effect_id, tool: input.tool, callId: input.call_id, status: input.status, dispatchAttempt: input.dispatch_attempt, resultDigest: input.result_digest ?? null, failureClass: input.failure_class ?? null, reason: input.reason ?? null });
    void receipt;
  }

  private idempotencyKey(effectId: string): string {
    return `aurora-effect-${effectId}`;
  }

  private async exclusive<T>(effectId: string, operation: () => Promise<T>): Promise<T> {
    const previous = this.operations.get(effectId) ?? Promise.resolve();
    const run = previous.then(operation, operation);
    this.operations.set(effectId, run);
    try { return await run; } finally { if (this.operations.get(effectId) === run) this.operations.delete(effectId); }
  }
}

/** Minimal structural result used to avoid coupling this ledger to a specific provider runtime implementation. */
export interface ProviderToolResultLike {
  callId: string;
  approved: boolean;
  content: string | JsonValue;
  isError?: boolean;
}
