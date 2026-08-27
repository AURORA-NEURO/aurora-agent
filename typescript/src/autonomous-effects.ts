import { ArgumentError, isObject } from "./errors.js";
import { AutonomousProtectedRehydrationAdapter } from "./autonomous-protected-rehydration.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import type { JsonObject, JsonValue } from "./types.js";
import type { ProviderToolCall } from "./llm.js";

const utf8Bytes = (value: string): number => new TextEncoder().encode(value).byteLength;

/** Metadata-only effect state; raw arguments, results, credentials, and provider bodies never enter this ledger. */
export const AUTONOMOUS_EFFECT_SCHEMA = "bioprism-typescript-autonomous-effect/0.1" as const;
export const AUTONOMOUS_EFFECT_EVENT_SCHEMA = "bioprism-typescript-autonomous-effect-event/0.1" as const;
export const AUTONOMOUS_EFFECT_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-effect-journal/0.1" as const;
export const AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-effect-snapshot/0.1" as const;
export const AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA = "bioprism-typescript-provider-effect-reconciliation/0.1" as const;
export const AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_ADMISSION_SCHEMA = "bioprism-typescript-provider-effect-reconciliation-admission/0.1" as const;

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

/**
 * Adapt a caller-owned provider status lookup to the metadata-only effect resolver.
 * The idempotency key is supplied transiently to the lookup and is never written to the ledger;
 * only its digest is retained by the effect boundary.
 */
export class AutonomousProviderEffectResolver implements AutonomousEffectResolver {
  private readonly lookup: (provider: string, operation: "invoke" | "stream", idempotencyKey: string, record: AutonomousEffectRecord) => Promise<AutonomousEffectResolution> | AutonomousEffectResolution;

  constructor(lookup: (provider: string, operation: "invoke" | "stream", idempotencyKey: string, record: AutonomousEffectRecord) => Promise<AutonomousEffectResolution> | AutonomousEffectResolution) {
    if (typeof lookup !== "function") throw new AutonomousEffectError("provider effect lookup must be callable");
    this.lookup = lookup;
  }

  resolve(record: AutonomousEffectRecord): Promise<AutonomousEffectResolution> | AutonomousEffectResolution {
    return this.resolveWithKey(record, `aurora-effect-${record.effect_id}`);
  }

  resolveWithKey(record: AutonomousEffectRecord, idempotencyKey: string): Promise<AutonomousEffectResolution> | AutonomousEffectResolution {
    const parts = record.tool.split(".");
    if (parts.length !== 3 || parts[0] !== "provider" || !parts[1] || (parts[2] !== "invoke" && parts[2] !== "stream")) throw new AutonomousEffectPolicyError("provider effect resolver received a non-provider effect");
    if (typeof idempotencyKey !== "string" || !idempotencyKey.trim() || utf8Bytes(idempotencyKey) > 512) throw new AutonomousEffectError("provider effect idempotency key is outside its bounded contract");
    return this.lookup(parts[1], parts[2], idempotencyKey, record);
  }
}

export const AUTONOMOUS_PROTECTED_PROVIDER_EFFECT_REHYDRATION_SCHEMA = "bioprism-typescript-autonomous-protected-provider-effect-rehydration/0.1" as const;

/** Identity supplied to a protected provider-status receipt lookup. The raw key is transient. */
export interface AutonomousProviderEffectProtectedRehydrationContext {
  effectId: string;
  executionId: string | null;
  tool: string;
  callId: string;
  riskClass: string;
  argumentsDigest: string;
  idempotencyKeyDigest: string;
  dispatchAttempt: number;
  provider: string;
  operation: "invoke" | "stream";
  idempotencyKey: string;
  domain: AutonomousDomainName | null;
}

export type AutonomousProviderEffectProtectedReceiptResolver = (
  context: AutonomousProviderEffectProtectedRehydrationContext,
) => unknown | Promise<unknown>;

function providerEffectParts(record: AutonomousEffectRecord): { provider: string; operation: "invoke" | "stream" } {
  const parts = record.tool.split(".");
  if (parts.length !== 3 || parts[0] !== "provider" || !parts[1] || (parts[2] !== "invoke" && parts[2] !== "stream")) throw new AutonomousEffectPolicyError("protected provider effect resolver received a non-provider effect");
  return { provider: parts[1], operation: parts[2] };
}

function assertProtectedProviderEffectReceipt(receipt: unknown, context: AutonomousProviderEffectProtectedRehydrationContext): asserts receipt is Record<string, unknown> {
  if (!isObject(receipt)) throw new AutonomousEffectPolicyError("protected provider effect receipt must be a metadata object");
  const forbidden = new Set(["idempotencykey", "apikey", "credential", "credentials", "secret", "token", "authorization", "password"]);
  if (Object.keys(receipt).some((key) => forbidden.has(key.toLowerCase().replace(/[^a-z0-9]/g, "")))) throw new AutonomousEffectPolicyError("protected provider effect receipt contains transient or secret-shaped material");
  const expected: Record<string, unknown> = {
    effect_id: context.effectId,
    execution_id: context.executionId,
    tool: context.tool,
    call_id: context.callId,
    risk_class: context.riskClass,
    arguments_digest: context.argumentsDigest,
    idempotency_key_digest: context.idempotencyKeyDigest,
    dispatch_attempt: context.dispatchAttempt,
    provider: context.provider,
    operation: context.operation,
  };
  for (const [key, value] of Object.entries(expected)) if (receipt[key] !== value) throw new AutonomousEffectPolicyError(`protected provider effect receipt ${key} does not match the effect record`);
  if (context.domain !== null && receipt.domain !== context.domain) throw new AutonomousEffectPolicyError("protected provider effect receipt domain does not match the configured scope");
  if (typeof receipt.domain !== "string" || !receipt.domain.trim()) throw new AutonomousEffectPolicyError("protected provider effect receipt must declare a domain scope");
}

function validateProtectedEffectResolution(value: unknown): AutonomousEffectResolution {
  if (!isObject(value)) throw new AutonomousEffectError("protected provider effect value must be a metadata object");
  const allowed = new Set(["status", "result", "failure_class", "reason", "retry_safe"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousEffectError("protected provider effect value contains unsupported fields");
  if (typeof value.status !== "string" || !["completed", "failed", "not_found", "unknown"].includes(value.status)) throw new AutonomousEffectError("protected provider effect value has an unsupported status");
  if (value.status === "completed" && !Object.prototype.hasOwnProperty.call(value, "result")) throw new AutonomousEffectError("completed protected provider effect value must include a result");
  if (value.failure_class !== undefined && (typeof value.failure_class !== "string" || !/^[A-Za-z0-9_.:-]{1,256}$/.test(value.failure_class))) throw new AutonomousEffectError("protected provider effect failure_class is malformed");
  if (value.reason !== undefined && (typeof value.reason !== "string" || !/^[A-Za-z0-9_.:-]{1,256}$/.test(value.reason))) throw new AutonomousEffectError("protected provider effect reason is malformed");
  if (value.retry_safe !== undefined && typeof value.retry_safe !== "boolean") throw new AutonomousEffectError("protected provider effect retry_safe must be boolean");
  return value as unknown as AutonomousEffectResolution;
}

/**
 * Rehydrates provider-status outcomes from a caller-owned protected receipt.
 *
 * The effect journal never stores the idempotency key or provider response. This resolver binds
 * the receipt to every durable effect identity field, gives the transient key only to the caller
 * lookup, and delegates tenant/authorization/expiry/replay/digest enforcement to the shared
 * protected boundary. A domain is required on the receipt because generic effect records do not
 * themselves own a domain authority.
 */
export class AutonomousProtectedProviderEffectResolver implements AutonomousEffectResolver {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: AutonomousProviderEffectProtectedReceiptResolver;
  readonly valueDecoder?: (value: unknown) => unknown | Promise<unknown>;
  readonly domain: AutonomousDomainName | undefined;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: AutonomousProviderEffectProtectedReceiptResolver;
    valueDecoder?: (value: unknown) => unknown | Promise<unknown>;
    domain?: AutonomousDomainName;
    purpose?: string;
    valueKind?: string;
    oneTime?: boolean;
    digestScheme?: string;
  }) {
    if (!options || !(options.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new AutonomousEffectError("protected provider effect resolver requires a protected receipt adapter");
    if (typeof options.receiptResolver !== "function") throw new AutonomousEffectError("protected provider effect receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new AutonomousEffectError("protected provider effect valueDecoder must be callable");
    if (options.domain !== undefined && (typeof options.domain !== "string" || !options.domain.trim())) throw new AutonomousEffectError("protected provider effect domain is malformed");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = boundedIdentifier("protected provider effect purpose", options.purpose ?? "autonomous_provider_effect_resolution", 256);
    this.valueKind = boundedIdentifier("protected provider effect valueKind", options.valueKind ?? "autonomous_provider_effect_resolution", 256);
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new AutonomousEffectError("protected provider effect oneTime must be boolean");
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
    if (this.digestScheme !== "canonical_json" && this.digestScheme !== "utf8_sha256") throw new AutonomousEffectError("protected provider effect digestScheme is unsupported");
  }

  resolve(record: AutonomousEffectRecord): Promise<AutonomousEffectResolution> {
    return this.resolveWithKey(record, `aurora-effect-${record.effect_id}`);
  }

  async resolveWithKey(record: AutonomousEffectRecord, idempotencyKey: string): Promise<AutonomousEffectResolution> {
    if (!record || typeof record !== "object") throw new AutonomousEffectError("protected provider effect record is malformed");
    if (typeof idempotencyKey !== "string" || !idempotencyKey.trim() || utf8Bytes(idempotencyKey) > 512) throw new AutonomousEffectError("protected provider effect idempotency key is outside its bounded contract");
    const { provider, operation } = providerEffectParts(record);
    const context: AutonomousProviderEffectProtectedRehydrationContext = {
      effectId: record.effect_id,
      executionId: record.execution_id,
      tool: record.tool,
      callId: record.call_id,
      riskClass: record.risk_class,
      argumentsDigest: record.arguments_digest,
      idempotencyKeyDigest: record.idempotency_key_digest,
      dispatchAttempt: record.dispatch_attempt,
      provider,
      operation,
      idempotencyKey,
      domain: this.domain ?? null,
    };
    try {
      const receipt = await this.receiptResolver(context);
      assertProtectedProviderEffectReceipt(receipt, context);
      const protectedValue = this.adapter.resolveReceipt(receipt, { domain: this.domain, purpose: this.purpose, valueKind: this.valueKind, oneTime: this.oneTime, digestScheme: this.digestScheme });
      const decoded = this.valueDecoder ? await this.valueDecoder(protectedValue) : protectedValue;
      return validateProtectedEffectResolution(decoded);
    } catch (error) {
      if (error instanceof AutonomousEffectError) throw error;
      const wrapped = new AutonomousEffectError("protected provider effect receipt could not be resolved");
      (wrapped as Error & { cause?: unknown }).cause = error;
      throw wrapped;
    }
  }
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
    if (canonicalJson(parsed) !== encoded) throw new AutonomousEffectError("effect JSON is not canonical");
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

  async pendingRecords(options: { toolPrefix?: string; maximum?: number } = {}): Promise<AutonomousEffectRecord[]> {
    const toolPrefix = options.toolPrefix;
    const maximum = options.maximum ?? 128;
    if (toolPrefix !== undefined && (typeof toolPrefix !== "string" || !toolPrefix || toolPrefix.length > 128)) throw new AutonomousEffectError("effect pending toolPrefix is outside its bounds");
    if (!Number.isSafeInteger(maximum) || maximum < 1 || maximum > 1_024) throw new AutonomousEffectError("effect pending maximum is outside its bounds");
    const latest = new Map<string, AutonomousEffectJournalRow>();
    let afterSequence = 0;
    while (true) {
      const rows = await this.journal.events({ afterSequence, limit: 256 });
      if (rows.length === 0) break;
      for (const row of rows) {
        latest.set(row.event.effect_id, row);
        afterSequence = Math.max(afterSequence, row.sequence);
      }
      if (rows.length < 256) break;
    }
    const pending: AutonomousEffectRecord[] = [];
    for (const [effectId, row] of [...latest.entries()].sort((left, right) => left[1].sequence - right[1].sequence || left[0].localeCompare(right[0]))) {
      if (!["dispatching", "dispatched", "uncertain"].includes(row.event.status)) continue;
      if (toolPrefix !== undefined && !row.event.tool.startsWith(toolPrefix)) continue;
      const record = await this.journal.get(effectId);
      if (record) pending.push(record);
      if (pending.length >= maximum) break;
    }
    return pending;
  }

  async execute<T>(
    request: AutonomousEffectRequest,
    executor: (context: AutonomousEffectExecutionContext) => T | Promise<T>,
    options: {
      execution?: AutonomousExecutionController;
      resultProjector?: (result: T) => JsonValue | Promise<JsonValue>;
      cacheResult?: boolean;
      definiteFailure?: (error: unknown) => boolean | Promise<boolean>;
    } = {},
  ): Promise<T> {
    if (typeof executor !== "function") throw new AutonomousEffectError("effect executor must be callable");
    if (options.resultProjector !== undefined && typeof options.resultProjector !== "function") throw new AutonomousEffectError("effect resultProjector must be callable or undefined");
    if (options.cacheResult !== undefined && typeof options.cacheResult !== "boolean") throw new AutonomousEffectError("effect cacheResult must be a boolean");
    if (options.definiteFailure !== undefined && typeof options.definiteFailure !== "function") throw new AutonomousEffectError("effect definiteFailure must be callable or undefined");
    const normalized = this.normalizeRequest(request);
    const effectId = await this.effectId(normalized);
    return this.exclusive(effectId, async () => this.executeExclusive(normalized, effectId, executor, options.execution ?? this.execution, options.resultProjector, options.cacheResult ?? true, options.definiteFailure));
  }

  async *executeStream<T>(
    request: AutonomousEffectRequest,
    producer: (context: AutonomousEffectExecutionContext) => AsyncIterable<T> | Iterable<T> | Promise<AsyncIterable<T> | Iterable<T>>,
    options: {
      execution?: AutonomousExecutionController;
      summaryProjector?: (summary: JsonObject) => JsonValue | Promise<JsonValue>;
      observe?: (item: T, eventCount: number) => void | Promise<void>;
      definiteFailure?: (error: unknown) => boolean | Promise<boolean>;
    } = {},
  ): AsyncIterable<T> {
    if (typeof producer !== "function") throw new AutonomousEffectError("effect stream producer must be callable");
    if (options.summaryProjector !== undefined && typeof options.summaryProjector !== "function") throw new AutonomousEffectError("effect stream summaryProjector must be callable or undefined");
    if (options.observe !== undefined && typeof options.observe !== "function") throw new AutonomousEffectError("effect stream observe must be callable or undefined");
    if (options.definiteFailure !== undefined && typeof options.definiteFailure !== "function") throw new AutonomousEffectError("effect stream definiteFailure must be callable or undefined");
    const normalized = this.normalizeRequest(request);
    const effectId = await this.effectId(normalized);
    const release = await this.acquireExclusive(effectId);
    try {
      for await (const item of this.executeStreamExclusive(normalized, effectId, producer, options.execution ?? this.execution, options.summaryProjector, options.observe, options.definiteFailure)) yield item;
    } finally {
      release();
    }
  }

  async reconcile(effectId: string, resolver: AutonomousEffectResolver = this.resolver as AutonomousEffectResolver, options: { idempotencyKey?: string } = {}): Promise<AutonomousEffectRecord> {
    boundedIdentifier("effect_id", effectId, 128);
    if (options.idempotencyKey !== undefined && (typeof options.idempotencyKey !== "string" || !options.idempotencyKey.trim() || utf8Bytes(options.idempotencyKey) > 512)) throw new AutonomousEffectError("effect idempotencyKey is outside its bounded contract");
    if (!resolver || typeof resolver.resolve !== "function") throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), "uncertain");
    const record = await this.journal.get(effectId);
    if (!record) throw new AutonomousEffectError(`effect ${effectId} is not present in the effect ledger`);
    return this.exclusive(effectId, async () => {
      const current = await this.journal.get(effectId);
      if (!current) throw new AutonomousEffectError(`effect ${effectId} disappeared from the effect ledger`);
      // Refresh under the per-effect queue so concurrent restart workers do not resolve or append
      // a second transition from a stale pre-lock record.
      if (!["dispatching", "dispatched", "uncertain"].includes(current.status)) return current;
      return this.reconcileExclusive(current, resolver, options.idempotencyKey);
    });
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

  private async executeExclusive<T>(
    request: AutonomousEffectRequest,
    effectId: string,
    executor: (context: AutonomousEffectExecutionContext) => T | Promise<T>,
    execution: AutonomousExecutionController | undefined,
    resultProjector: ((result: T) => JsonValue | Promise<JsonValue>) | undefined,
    cacheResult: boolean,
    definiteFailure: ((error: unknown) => boolean | Promise<boolean>) | undefined,
  ): Promise<T> {
    let record = await this.journal.get(effectId);
    if (record) {
      if (record.status === "completed" || record.status === "reconciled") {
        const cached = this.resultCache.get(effectId);
        if (cacheResult && cached !== undefined) return clone(cached) as T;
        if (!this.resolver) throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
        record = await this.reconcileExclusive(record, this.resolver);
        if (record.status === "reconciled" || record.status === "completed") {
          const resolved = this.resultCache.get(effectId);
          if (cacheResult && resolved !== undefined) return clone(resolved) as T;
        }
      }
      if (["dispatching", "dispatched", "uncertain"].includes(record.status)) {
        if (!this.resolver) throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
        record = await this.reconcileExclusive(record, this.resolver);
        if (record.status === "reconciled" || record.status === "completed") {
          const resolved = this.resultCache.get(effectId);
          if (cacheResult && resolved !== undefined) return clone(resolved) as T;
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
      const projected = resultProjector ? await resultProjector(result) : result;
      assertMetadataBytes("effect result", projected, AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES);
      const resultDigest = await digestJson(projected);
      if (cacheResult) this.resultCache.set(effectId, clone(result) as JsonValue);
      await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "completed", dispatch_attempt: attempt, result_digest: resultDigest }, execution);
      return (cacheResult ? clone(result) : result) as T;
    } catch (unknownError) {
      let isDefiniteFailure = false;
      if (definiteFailure) {
        try { isDefiniteFailure = await definiteFailure(unknownError); } catch { isDefiniteFailure = false; }
      }
      if (isDefiniteFailure) {
        await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "failed", dispatch_attempt: attempt, failure_class: normalizedFailureClass(unknownError), reason: normalizedReason(unknownError) }, execution);
        throw unknownError;
      }
      const failureClass = normalizedFailureClass(unknownError);
      await this.transition({ ...request, effect_id: effectId, arguments_digest: argumentsDigest, idempotency_key_digest: await digestJson(idempotencyKey), status: "uncertain", dispatch_attempt: attempt, failure_class: failureClass, reason: normalizedReason(unknownError) }, execution);
      if (unknownError instanceof AutonomousEffectError) throw unknownError;
      throw new AutonomousEffectReconciliationRequiredError(effectId, idempotencyKey, "uncertain");
    }
  }

  private async reconcileExclusive(record: AutonomousEffectRecord, resolver: AutonomousEffectResolver, idempotencyKey?: string): Promise<AutonomousEffectRecord> {
    const resolveWithKey = (resolver as AutonomousEffectResolver & { resolveWithKey?: (record: AutonomousEffectRecord, idempotencyKey: string) => Promise<AutonomousEffectResolution> | AutonomousEffectResolution }).resolveWithKey;
    const resolution = await (typeof resolveWithKey === "function" ? resolveWithKey.call(resolver, clone(record), idempotencyKey ?? this.idempotencyKey(record.effect_id)) : resolver.resolve(clone(record)));
    if (!isObject(resolution) || !["completed", "failed", "not_found", "unknown"].includes(resolution.status)) throw new AutonomousEffectError("effect resolver returned malformed status");
    const fallbackKey = this.idempotencyKey(record.effect_id);
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
    throw new AutonomousEffectReconciliationRequiredError(record.effect_id, fallbackKey, record.status);
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

  private async *executeStreamExclusive<T>(
    request: AutonomousEffectRequest,
    effectId: string,
    producer: (context: AutonomousEffectExecutionContext) => AsyncIterable<T> | Iterable<T> | Promise<AsyncIterable<T> | Iterable<T>>,
    execution: AutonomousExecutionController | undefined,
    summaryProjector: ((summary: JsonObject) => JsonValue | Promise<JsonValue>) | undefined,
    observe: ((item: T, eventCount: number) => void | Promise<void>) | undefined,
    definiteFailure: ((error: unknown) => boolean | Promise<boolean>) | undefined,
  ): AsyncIterable<T> {
    let record = await this.journal.get(effectId);
    const argumentsDigest = await digestJson(request.arguments);
    if (record) {
      if (record.tool !== request.tool || record.call_id !== request.call_id || record.arguments_digest !== argumentsDigest) throw new AutonomousEffectPolicyError("effect id collides with different call metadata");
      // Stream deltas are intentionally never cached. A completed stream therefore cannot be
      // replayed, even if a resolver can confirm that the provider finished it remotely.
      if (record.status === "completed" || record.status === "reconciled") throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
      if (["dispatching", "dispatched", "uncertain"].includes(record.status)) {
        if (!this.resolver) throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
        record = await this.reconcileExclusive(record, this.resolver);
        if (record.status !== "prepared") throw new AutonomousEffectReconciliationRequiredError(effectId, this.idempotencyKey(effectId), record.status);
      }
      if (record.status === "failed") throw new AutonomousEffectExecutionError(effectId, record.failure_class ?? "previous_effect_failure");
    }
    const idempotencyKey = this.idempotencyKey(effectId);
    const base = {
      ...request,
      effect_id: effectId,
      arguments_digest: argumentsDigest,
      idempotency_key_digest: await digestJson(idempotencyKey),
      dispatch_attempt: (record?.dispatch_attempt ?? 0) + 1,
    };
    await this.transition({ ...base, status: "prepared", reason: null }, execution);
    await this.transition({ ...base, status: "dispatching" }, execution);
    await this.transition({ ...base, status: "dispatched" }, execution);
    const context: AutonomousEffectExecutionContext = { effect_id: effectId, execution_id: request.execution_id ?? null, tool: request.tool, call_id: request.call_id, risk_class: request.risk_class, idempotency_key: idempotencyKey, dispatch_attempt: base.dispatch_attempt };
    let eventCount = 0;
    let terminalRecorded = false;
    try {
      const stream = await producer(context);
      if (!stream || (typeof (stream as AsyncIterable<T>)[Symbol.asyncIterator] !== "function" && typeof (stream as Iterable<T>)[Symbol.iterator] !== "function")) throw new AutonomousEffectError("effect stream producer must return an iterable");
      for await (const item of stream) {
        eventCount += 1;
        await observe?.(item, eventCount);
        yield item;
      }
      const summaryInput: JsonObject = { event_count: eventCount, completed: true };
      const projected = summaryProjector ? await summaryProjector(summaryInput) : summaryInput;
      assertMetadataBytes("effect stream summary", projected, AUTONOMOUS_EFFECT_MAX_ARGUMENT_BYTES);
      await this.transition({ ...base, status: "completed", result_digest: await digestJson(projected) }, execution);
      terminalRecorded = true;
    } catch (unknownError) {
      let isDefiniteFailure = false;
      if (definiteFailure) {
        try { isDefiniteFailure = await definiteFailure(unknownError); } catch { isDefiniteFailure = false; }
      }
      if (isDefiniteFailure) {
        await this.transition({ ...base, status: "failed", failure_class: normalizedFailureClass(unknownError), reason: normalizedReason(unknownError) }, execution);
        terminalRecorded = true;
        throw unknownError;
      }
      await this.transition({ ...base, status: "uncertain", failure_class: normalizedFailureClass(unknownError), reason: normalizedReason(unknownError) }, execution);
      terminalRecorded = true;
      if (unknownError instanceof AutonomousEffectError) throw unknownError;
      throw new AutonomousEffectReconciliationRequiredError(effectId, idempotencyKey, "uncertain");
    } finally {
      // AsyncIterator.return() closes a consumer without throwing through the producer. A
      // dispatched stream that was not exhausted is still externally ambiguous.
      if (!terminalRecorded) {
        await this.transition({ ...base, status: "uncertain", failure_class: "stream_abandoned", reason: "consumer_closed_stream" }, execution);
      }
    }
  }

  private async acquireExclusive(effectId: string): Promise<() => void> {
    const previous = this.operations.get(effectId) ?? Promise.resolve();
    const predecessor = previous.catch(() => undefined);
    let releaseGate: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => { releaseGate = resolve; });
    const queued = predecessor.then(() => gate);
    this.operations.set(effectId, queued);
    await predecessor;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      releaseGate?.();
      if (this.operations.get(effectId) === queued) this.operations.delete(effectId);
    };
  }
}

/** A bounded report from a restart reconciliation pass. It contains no provider key or payload. */
export interface AutonomousProviderEffectReconciliationReport extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA;
  inspected: number;
  reconciled: number;
  failed: number;
  retry_ready: number;
  uncertain: number;
  errors: number;
  outcomes: JsonValue[];
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
}

export interface AutonomousProviderEffectReconciliationAdmission extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_ADMISSION_SCHEMA;
  status: "allowed" | "blocked";
  reason: "no_pending_effects" | "pending_effects_reconciled" | "uncertain_effect_state" | "reconciliation_errors";
  report: AutonomousProviderEffectReconciliationReport;
  admission_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
}

/**
 * Scan restored provider effects and ask a caller-owned resolver about each one.
 * The worker never retries a provider call itself; `prepared` is reported as retry-ready and
 * remains subject to the normal runtime admission gates on a later fresh dispatch.
 */
export class AutonomousProviderEffectReconciliationWorker {
  readonly boundary: AutonomousEffectBoundary;
  readonly resolver: AutonomousEffectResolver;
  readonly keyResolver?: (record: AutonomousEffectRecord) => string | null | Promise<string | null>;
  readonly maximumRecords: number;

  constructor(
    boundary: AutonomousEffectBoundary,
    resolver: AutonomousEffectResolver,
    options: { keyResolver?: (record: AutonomousEffectRecord) => string | null | Promise<string | null>; maximumRecords?: number } = {},
  ) {
    if (!(boundary instanceof AutonomousEffectBoundary)) throw new AutonomousEffectError("provider reconciliation boundary is malformed");
    if (!resolver || typeof resolver.resolve !== "function") throw new AutonomousEffectError("provider reconciliation resolver is malformed");
    if (options.keyResolver !== undefined && typeof options.keyResolver !== "function") throw new AutonomousEffectError("provider reconciliation keyResolver must be callable or undefined");
    const maximumRecords = options.maximumRecords ?? 128;
    if (!Number.isSafeInteger(maximumRecords) || maximumRecords < 1 || maximumRecords > 1_024) throw new AutonomousEffectError("provider reconciliation maximumRecords is outside its bounds");
    this.boundary = boundary;
    this.resolver = resolver;
    this.keyResolver = options.keyResolver;
    this.maximumRecords = maximumRecords;
  }

  async runOnce(options: { maximumRecords?: number } = {}): Promise<AutonomousProviderEffectReconciliationReport> {
    const maximumRecords = options.maximumRecords ?? this.maximumRecords;
    if (!Number.isSafeInteger(maximumRecords) || maximumRecords < 1 || maximumRecords > this.maximumRecords) throw new AutonomousEffectError("provider reconciliation run limit is outside its bounds");
    const pending = await this.boundary.pendingRecords({ toolPrefix: "provider.", maximum: maximumRecords });
    const outcomes: JsonValue[] = [];
    const counts = { reconciled: 0, failed: 0, retry_ready: 0, uncertain: 0, errors: 0 };
    for (const record of pending) {
      try {
        const key = this.keyResolver ? await this.keyResolver(record) : undefined;
        const updated = await this.boundary.reconcile(record.effect_id, this.resolver, { idempotencyKey: key ?? undefined });
        if (updated.status === "reconciled") counts.reconciled += 1;
        else if (updated.status === "failed") counts.failed += 1;
        else if (updated.status === "prepared") counts.retry_ready += 1;
        else counts.uncertain += 1;
        outcomes.push({ effect_id: record.effect_id, status: updated.status, dispatch_attempt: updated.dispatch_attempt });
      } catch (error) {
        if (error instanceof AutonomousEffectReconciliationRequiredError) {
          counts.uncertain += 1;
          outcomes.push({ effect_id: record.effect_id, status: "uncertain", dispatch_attempt: record.dispatch_attempt, reason: error.status });
        } else if (error instanceof AutonomousEffectError) {
          counts.errors += 1;
          outcomes.push({ effect_id: record.effect_id, status: "worker_error", dispatch_attempt: record.dispatch_attempt, error_class: "effect_error" });
        } else {
          counts.errors += 1;
          outcomes.push({ effect_id: record.effect_id, status: "worker_error", dispatch_attempt: record.dispatch_attempt, error_class: "resolver_error" });
        }
      }
    }
    return {
      schema: AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA,
      inspected: pending.length,
      ...counts,
      outcomes,
      retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material",
      secret_material: "never_returned",
    };
  }
}

/**
 * Owns one restart-time reconciliation pass and turns its result into an execution admission.
 *
 * A worker process should create one coordinator for its lifecycle and invoke `admit()` before
 * claiming or dispatching new brain work. The result is cached so concurrent callers cannot run
 * duplicate provider-status lookups. A caller that has resolved the outstanding external state
 * may explicitly call `reset()` before requesting another pass. This class never invents external
 * truth and never performs a fresh provider request.
 */
export class AutonomousProviderEffectReconciliationCoordinator {
  readonly worker: AutonomousProviderEffectReconciliationWorker;
  private admissionPromise: Promise<AutonomousProviderEffectReconciliationAdmission> | null = null;
  private running = false;

  constructor(worker: AutonomousProviderEffectReconciliationWorker) {
    if (!(worker instanceof AutonomousProviderEffectReconciliationWorker)) throw new AutonomousEffectError("provider reconciliation coordinator requires a reconciliation worker");
    this.worker = worker;
  }

  async admit(): Promise<AutonomousProviderEffectReconciliationAdmission> {
    if (this.admissionPromise !== null) return this.admissionPromise;
    this.running = true;
    const promise = (async () => {
      let report: AutonomousProviderEffectReconciliationReport;
      try {
        report = await this.worker.runOnce();
      } catch (_error) {
        report = {
          schema: AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA,
          inspected: 0,
          reconciled: 0,
          failed: 0,
          retry_ready: 0,
          uncertain: 0,
          errors: 1,
          outcomes: [{ status: "coordinator_error", error_class: "reconciliation_error" }],
          retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material",
          secret_material: "never_returned",
        };
      }
      const blocked = report.uncertain > 0 || report.errors > 0;
      const reason: AutonomousProviderEffectReconciliationAdmission["reason"] = report.errors > 0
        ? "reconciliation_errors"
        : report.uncertain > 0
          ? "uncertain_effect_state"
          : report.inspected === 0
            ? "no_pending_effects"
            : "pending_effects_reconciled";
      const status: AutonomousProviderEffectReconciliationAdmission["status"] = blocked ? "blocked" : "allowed";
      const admission: AutonomousProviderEffectReconciliationAdmission = {
        schema: AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_ADMISSION_SCHEMA,
        status,
        reason,
        report,
        admission_digest: await digestJson({
          schema: AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_ADMISSION_SCHEMA,
          status,
          reason,
          inspected: report.inspected,
          reconciled: report.reconciled,
          failed: report.failed,
          retry_ready: report.retry_ready,
          uncertain: report.uncertain,
          errors: report.errors,
          outcomes: report.outcomes,
        }),
        retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material",
        secret_material: "never_returned",
      };
      this.running = false;
      return admission;
    })();
    this.admissionPromise = promise;
    return promise;
  }

  /** Clear a completed pass after the caller has resolved the reported external state. */
  reset(): void {
    if (this.running) throw new AutonomousEffectError("provider reconciliation coordinator cannot reset while a pass is running");
    this.admissionPromise = null;
  }
}

/** Minimal structural result used to avoid coupling this ledger to a specific provider runtime implementation. */
export interface ProviderToolResultLike {
  callId: string;
  approved: boolean;
  content: string | JsonValue;
  isError?: boolean;
}
