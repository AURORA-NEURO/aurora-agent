import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type { ProviderInvocationMetadata, ProviderInvocationObserver, ProviderInvocationOutcome } from "./llm.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** A metadata-only, append-only execution trace for one or more autonomous invocations. */
export const AUTONOMOUS_RUN_TRACE_SCHEMA = "bioprism-typescript-autonomous-run-trace/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA = "bioprism-typescript-autonomous-run-trace-event/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-run-trace-snapshot/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_PHASES = [
  "started",
  "plan_compiled",
  "connector_started",
  "connector_finished",
  "provider_invocation_started",
  "provider_invocation_finished",
  "evaluation_settled",
  "learning_prepared",
  "completed",
  "paused",
  "refused",
  "failed",
] as const;
export const AUTONOMOUS_RUN_TRACE_STATUSES = ["running", "completed", "partial", "paused", "refused", "failed", "unknown"] as const;
export const MAX_AUTONOMOUS_RUN_TRACE_EVENTS = 100_000;
export const MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES = 16_000;
export const MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES = 50_000_000;
export const MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT = 10_000;

/** Kept in lockstep with the authoritative autonomous catalogue; cross_domain is a valid run domain. */
const TRACE_DOMAIN_NAMES: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];

export type AutonomousRunTracePhase = typeof AUTONOMOUS_RUN_TRACE_PHASES[number];
export type AutonomousRunTraceStatus = typeof AUTONOMOUS_RUN_TRACE_STATUSES[number];

export interface AutonomousRunTraceEvent extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA;
  sequence: number;
  run_id: string;
  task_digest: string;
  domains: AutonomousDomainName[];
  phase: AutonomousRunTracePhase;
  status: AutonomousRunTraceStatus;
  route_digest: string | null;
  plan_digest: string | null;
  selection_digest: string | null;
  provider: string | null;
  model: string | null;
  attempt: number | null;
  turn: number | null;
  latency_ms: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  tool_count: number | null;
  status_code: number | null;
  failure_class: string | null;
  failure_code: string | null;
  retryable: boolean | null;
  detail_digest: string | null;
  recorded_at: number;
  previous_digest: string;
  event_digest: string;
  retention: "metadata_only_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

export interface AutonomousRunTraceEventInput {
  run_id: string;
  task_digest: string;
  domains: readonly AutonomousDomainName[];
  phase: AutonomousRunTracePhase;
  status: AutonomousRunTraceStatus;
  route_digest?: string | null;
  plan_digest?: string | null;
  selection_digest?: string | null;
  provider?: string | null;
  model?: string | null;
  attempt?: number | null;
  turn?: number | null;
  latency_ms?: number | null;
  input_tokens?: number | null;
  output_tokens?: number | null;
  tool_count?: number | null;
  status_code?: number | null;
  failure_class?: string | null;
  failure_code?: string | null;
  retryable?: boolean | null;
  detail_digest?: string | null;
}

export interface AutonomousRunTraceQuery {
  run_id?: string;
  domain?: AutonomousDomainName;
  phase?: AutonomousRunTracePhase;
  status?: AutonomousRunTraceStatus;
  provider?: string;
  model?: string;
  after_sequence?: number;
  limit?: number;
}

export interface AutonomousRunTraceSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  events: AutonomousRunTraceEvent[];
  snapshot_digest: string;
  retention: "metadata_only_hash_chained_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

export interface AutonomousRunTraceStore {
  append(input: AutonomousRunTraceEventInput): Promise<AutonomousRunTraceEvent> | AutonomousRunTraceEvent;
  events(query?: AutonomousRunTraceQuery): Promise<AutonomousRunTraceEvent[]> | AutonomousRunTraceEvent[];
  snapshot(): Promise<AutonomousRunTraceSnapshot> | AutonomousRunTraceSnapshot;
  restore(snapshot: unknown): Promise<void> | void;
  verifyIntegrity(): Promise<{ verified: true; events: number; head_digest: string }> | { verified: true; events: number; head_digest: string };
}

export interface AutonomousRunTracePersistence {
  read(): Promise<AutonomousRunTraceSnapshot | null> | AutonomousRunTraceSnapshot | null;
  write(snapshot: AutonomousRunTraceSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousRunTraceSnapshot): Promise<boolean> | boolean;
}

/** Portable text contract implemented by browser storage, HTTP stores, databases, or files. */
export interface AutonomousRunTraceTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

/** Text contract with an atomic snapshot-digest fence for multi-worker trace writers. */
export interface AutonomousRunTraceTransactionalTextStore extends AutonomousRunTraceTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousRunTraceSummary extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_SCHEMA;
  run_id: string;
  task_digest: string;
  domains: AutonomousDomainName[];
  status: AutonomousRunTraceStatus;
  first_sequence: number | null;
  last_sequence: number | null;
  event_count: number;
  provider_invocations: number;
  provider_failures: number;
  input_tokens: number;
  output_tokens: number;
  tool_calls: number;
  route_digest: string | null;
  plan_digest: string | null;
  selection_digests: string[];
  failure_codes: string[];
  trace_digest: string;
  retention: "metadata_only_no_prompts_responses_or_tool_payloads";
  secret_material: "never_returned";
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function sha256Digest(name: string, value: unknown, allowNull = true): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function nonnegativeInteger(name: string, value: unknown, allowNull = true): number | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new ArgumentError(`${name} must be a non-negative safe integer`);
  return value;
}

function boundedNullableText(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return boundedText(name, value, 256);
}

function boundedStatusCode(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 100 || value > 599) throw new ArgumentError("autonomous run trace status_code is invalid");
  return value;
}

function boundedLatency(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 86_400_000) throw new ArgumentError("autonomous run trace latency_ms is invalid");
  return value;
}

function normalizeDomains(value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > TRACE_DOMAIN_NAMES.length) throw new ArgumentError(`autonomous run trace domains must contain 1..=${TRACE_DOMAIN_NAMES.length} entries`);
  const domains = value.map((domain) => {
    if (typeof domain !== "string" || !TRACE_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous run trace contains an unsupported domain");
    return domain as AutonomousDomainName;
  });
  if (new Set(domains).size !== domains.length) throw new ArgumentError("autonomous run trace domains must be unique");
  return domains;
}

function normalizeEventInput(input: AutonomousRunTraceEventInput, sequence: number, previousDigest: string, recordedAt: number): Omit<AutonomousRunTraceEvent, "event_digest"> {
  if (!isObject(input)) throw new ArgumentError("autonomous run trace event must be an object");
  if (!Number.isSafeInteger(sequence) || sequence < 1) throw new ArgumentError("autonomous run trace sequence is invalid");
  if (!Number.isSafeInteger(recordedAt) || recordedAt < 0) throw new ArgumentError("autonomous run trace recorded_at is invalid");
  const phase = input.phase;
  const status = input.status;
  if (!AUTONOMOUS_RUN_TRACE_PHASES.includes(phase)) throw new ArgumentError("autonomous run trace phase is invalid");
  if (!AUTONOMOUS_RUN_TRACE_STATUSES.includes(status)) throw new ArgumentError("autonomous run trace status is invalid");
  const domains = normalizeDomains(input.domains);
  const retryable = input.retryable === null || input.retryable === undefined ? null : input.retryable;
  if (retryable !== null && typeof retryable !== "boolean") throw new ArgumentError("autonomous run trace retryable must be boolean or null");
  return {
    schema: AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA,
    sequence,
    run_id: identifier("autonomous run trace run_id", input.run_id),
    task_digest: sha256Digest("autonomous run trace task_digest", input.task_digest, false) as string,
    domains,
    phase,
    status,
    route_digest: sha256Digest("autonomous run trace route_digest", input.route_digest),
    plan_digest: sha256Digest("autonomous run trace plan_digest", input.plan_digest),
    selection_digest: sha256Digest("autonomous run trace selection_digest", input.selection_digest),
    provider: boundedNullableText("autonomous run trace provider", input.provider),
    model: boundedNullableText("autonomous run trace model", input.model),
    attempt: nonnegativeInteger("autonomous run trace attempt", input.attempt),
    turn: nonnegativeInteger("autonomous run trace turn", input.turn),
    latency_ms: boundedLatency(input.latency_ms),
    input_tokens: nonnegativeInteger("autonomous run trace input_tokens", input.input_tokens),
    output_tokens: nonnegativeInteger("autonomous run trace output_tokens", input.output_tokens),
    tool_count: nonnegativeInteger("autonomous run trace tool_count", input.tool_count),
    status_code: boundedStatusCode(input.status_code),
    failure_class: boundedNullableText("autonomous run trace failure_class", input.failure_class),
    failure_code: boundedNullableText("autonomous run trace failure_code", input.failure_code),
    retryable,
    detail_digest: sha256Digest("autonomous run trace detail_digest", input.detail_digest),
    recorded_at: recordedAt,
    previous_digest: previousDigest,
    retention: "metadata_only_no_prompts_responses_or_tool_payloads",
    secret_material: "never_returned",
  };
}

function eventDigestBody(event: Omit<AutonomousRunTraceEvent, "event_digest">): JsonObject {
  return { ...event };
}

function eventDigest(event: Omit<AutonomousRunTraceEvent, "event_digest">): string {
  return digestJsonSync(eventDigestBody(event));
}

function cloneEvent(event: AutonomousRunTraceEvent): AutonomousRunTraceEvent {
  return structuredClone(event);
}

function verifyEventChain(events: readonly AutonomousRunTraceEvent[], maximum: number): { verified: true; events: number; head_digest: string } {
  if (!Array.isArray(events) || events.length > maximum) throw new ArgumentError("autonomous run trace event capacity is exceeded");
  let previous = "";
  for (let index = 0; index < events.length; index += 1) {
    const event = events[index];
    if (!event || event.sequence !== index + 1 || event.previous_digest !== previous) throw new ArgumentError(`autonomous run trace hash chain breaks at sequence ${index + 1}`);
    if (event.schema !== AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA || event.retention !== "metadata_only_no_prompts_responses_or_tool_payloads" || event.secret_material !== "never_returned") throw new ArgumentError(`autonomous run trace event ${index + 1} has invalid retention`);
    const { event_digest: supplied, ...body } = event;
    if (eventDigest(body) !== supplied) throw new ArgumentError(`autonomous run trace event digest mismatch at sequence ${index + 1}`);
    normalizeEventInput(body as unknown as AutonomousRunTraceEventInput, event.sequence, event.previous_digest, event.recorded_at);
    previous = supplied;
  }
  return { verified: true, events: events.length, head_digest: previous };
}

function snapshotBody(events: readonly AutonomousRunTraceEvent[]): Omit<AutonomousRunTraceSnapshot, "snapshot_digest"> {
  return {
    schema: AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
    sequence: events.length,
    head_digest: events.at(-1)?.event_digest ?? "",
    events: events.map(cloneEvent),
    retention: "metadata_only_hash_chained_no_prompts_responses_or_tool_payloads",
    secret_material: "never_returned",
  };
}

function validateSnapshot(value: unknown, maximumEvents: number, maximumBytes: number): AutonomousRunTraceSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA || !Array.isArray(value.events)) throw new ArgumentError("autonomous run trace snapshot is malformed");
  if (value.retention !== "metadata_only_hash_chained_no_prompts_responses_or_tool_payloads" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous run trace snapshot retention is invalid");
  const snapshotValue = value as Record<string, unknown>;
  const events = (snapshotValue.events as unknown[]).map((event) => {
    if (!isObject(event)) throw new ArgumentError("autonomous run trace snapshot contains a malformed event");
    return structuredClone(event) as unknown as AutonomousRunTraceEvent;
  });
  const verified = verifyEventChain(events, maximumEvents);
  const body = snapshotBody(events);
  const suppliedSequence = snapshotValue.sequence;
  const suppliedHead = snapshotValue.head_digest;
  const suppliedDigest = snapshotValue.snapshot_digest;
  if (suppliedSequence !== body.sequence || suppliedHead !== body.head_digest || typeof suppliedDigest !== "string" || digestJsonSync(body) !== suppliedDigest) throw new ArgumentError("autonomous run trace snapshot digest is invalid");
  const snapshot = { ...body, snapshot_digest: suppliedDigest } as AutonomousRunTraceSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > maximumBytes) throw new ArgumentError("autonomous run trace snapshot exceeds its byte capacity");
  if (verified.head_digest !== snapshot.head_digest) throw new ArgumentError("autonomous run trace snapshot head is inconsistent");
  return structuredClone(snapshot);
}

/** Validate an external run-trace snapshot before it can alter a live trace journal. */
export function validateAutonomousRunTraceSnapshot(raw: unknown, options: { maxEvents?: number; maxBytes?: number } = {}): AutonomousRunTraceSnapshot {
  const maxEvents = options.maxEvents ?? MAX_AUTONOMOUS_RUN_TRACE_EVENTS;
  const maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES;
  if (!Number.isSafeInteger(maxEvents) || maxEvents < 1 || maxEvents > MAX_AUTONOMOUS_RUN_TRACE_EVENTS) throw new ArgumentError("autonomous run trace validation maxEvents is outside its bounds");
  if (!Number.isSafeInteger(maxBytes) || maxBytes < MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES || maxBytes > MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("autonomous run trace validation maxBytes is outside its bounds");
  return validateSnapshot(raw, maxEvents, maxBytes);
}

/** JSON persistence for any bounded text store, including the HTTP snapshot transport. */
export class JsonAutonomousRunTracePersistence implements AutonomousRunTracePersistence {
  protected readonly store: AutonomousRunTraceTextStore;
  readonly maxEvents: number;
  readonly maxBytes: number;

  constructor(store: AutonomousRunTraceTextStore, options: { maxEvents?: number; maxBytes?: number } = {}) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("autonomous run trace JSON persistence requires a text store");
    this.store = store;
    this.maxEvents = options.maxEvents ?? MAX_AUTONOMOUS_RUN_TRACE_EVENTS;
    this.maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES;
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > MAX_AUTONOMOUS_RUN_TRACE_EVENTS) throw new ArgumentError("autonomous run trace JSON persistence maxEvents is outside its bounds");
    if (!Number.isSafeInteger(this.maxBytes) || this.maxBytes < MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES || this.maxBytes > MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("autonomous run trace JSON persistence maxBytes is outside its bounds");
  }

  async read(): Promise<AutonomousRunTraceSnapshot | null> {
    const text = await this.store.read();
    if (text === null) return null;
    if (new TextEncoder().encode(text).byteLength > this.maxBytes) throw new ArgumentError("autonomous run trace JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch { throw new ArgumentError("autonomous run trace JSON is invalid"); }
    if (canonicalJson(parsed) !== text) throw new ArgumentError("autonomous run trace JSON is not canonical");
    return validateSnapshot(parsed, this.maxEvents, this.maxBytes);
  }

  async write(snapshot: AutonomousRunTraceSnapshot): Promise<void> {
    const validated = validateSnapshot(snapshot, this.maxEvents, this.maxBytes);
    await this.store.write(canonicalJson(validated));
  }
}

/** JSON persistence variant that carries the trace head through an atomic compare-and-swap. */
export class TransactionalJsonAutonomousRunTracePersistence extends JsonAutonomousRunTracePersistence implements AutonomousRunTracePersistence {
  declare protected readonly store: AutonomousRunTraceTransactionalTextStore;

  constructor(store: AutonomousRunTraceTransactionalTextStore, options: { maxEvents?: number; maxBytes?: number } = {}) {
    super(store, options);
    this.store = store;
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("autonomous run trace transactional persistence requires writeIfUnchanged");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousRunTraceSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ArgumentError("autonomous run trace expected snapshot digest is invalid");
    const validated = validateSnapshot(snapshot, this.maxEvents, this.maxBytes);
    return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

/** Browser-compatible local text storage for trace snapshots. */
export class WebStorageAutonomousRunTraceTextStore implements AutonomousRunTraceTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("autonomous run trace Web Storage adapter is malformed");
    boundedText("autonomous run trace storage key", key, 256);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/** Bounded in-memory journal suitable for local workers and caller-owned persistence adapters. */
export class InMemoryAutonomousRunTraceStore implements AutonomousRunTraceStore {
  private readonly eventsValue: AutonomousRunTraceEvent[] = [];
  private readonly maxEvents: number;
  private readonly maxEventBytes: number;
  private readonly maxSnapshotBytes: number;
  private readonly clock: () => number;

  constructor(options: { maxEvents?: number; maxEventBytes?: number; maxSnapshotBytes?: number; clock?: () => number } = {}) {
    this.maxEvents = options.maxEvents ?? MAX_AUTONOMOUS_RUN_TRACE_EVENTS;
    this.maxEventBytes = options.maxEventBytes ?? MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES;
    this.maxSnapshotBytes = options.maxSnapshotBytes ?? MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES;
    this.clock = options.clock ?? (() => Date.now());
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > MAX_AUTONOMOUS_RUN_TRACE_EVENTS) throw new ArgumentError("autonomous run trace maxEvents is outside its bounds");
    if (!Number.isSafeInteger(this.maxEventBytes) || this.maxEventBytes < 512 || this.maxEventBytes > MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES) throw new ArgumentError("autonomous run trace maxEventBytes is outside its bounds");
    if (!Number.isSafeInteger(this.maxSnapshotBytes) || this.maxSnapshotBytes < this.maxEventBytes || this.maxSnapshotBytes > MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) throw new ArgumentError("autonomous run trace maxSnapshotBytes is outside its bounds");
  }

  append(input: AutonomousRunTraceEventInput): AutonomousRunTraceEvent {
    if (this.eventsValue.length >= this.maxEvents) throw new ArgumentError("autonomous run trace event capacity is exhausted");
    const eventBody = normalizeEventInput(input, this.eventsValue.length + 1, this.eventsValue.at(-1)?.event_digest ?? "", this.clock());
    const event = { ...eventBody, event_digest: eventDigest(eventBody) } as AutonomousRunTraceEvent;
    if (new TextEncoder().encode(canonicalJson(event)).byteLength > this.maxEventBytes) throw new ArgumentError("autonomous run trace event exceeds its byte capacity");
    this.eventsValue.push(event);
    return cloneEvent(event);
  }

  events(query: AutonomousRunTraceQuery = {}): AutonomousRunTraceEvent[] {
    if (!query || typeof query !== "object" || Array.isArray(query)) throw new ArgumentError("autonomous run trace query must be an object");
    const normalizedQuery = query as AutonomousRunTraceQuery;
    const after = normalizedQuery.after_sequence === undefined ? 0 : nonnegativeInteger("autonomous run trace after_sequence", normalizedQuery.after_sequence, false) as number;
    const limit = normalizedQuery.limit ?? MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT;
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT) throw new ArgumentError("autonomous run trace query limit is outside its bounds");
    if (normalizedQuery.run_id !== undefined) identifier("autonomous run trace query run_id", normalizedQuery.run_id);
    if (normalizedQuery.provider !== undefined) boundedText("autonomous run trace query provider", normalizedQuery.provider, 256);
    if (normalizedQuery.model !== undefined) boundedText("autonomous run trace query model", normalizedQuery.model, 256);
    if (normalizedQuery.domain !== undefined && !TRACE_DOMAIN_NAMES.includes(normalizedQuery.domain)) throw new ArgumentError("autonomous run trace query domain is unsupported");
    if (normalizedQuery.phase !== undefined && !AUTONOMOUS_RUN_TRACE_PHASES.includes(normalizedQuery.phase)) throw new ArgumentError("autonomous run trace query phase is unsupported");
    if (normalizedQuery.status !== undefined && !AUTONOMOUS_RUN_TRACE_STATUSES.includes(normalizedQuery.status)) throw new ArgumentError("autonomous run trace query status is unsupported");
    return this.eventsValue
      .filter((event) => event.sequence > after)
      .filter((event) => normalizedQuery.run_id === undefined || event.run_id === normalizedQuery.run_id)
      .filter((event) => normalizedQuery.domain === undefined || event.domains.includes(normalizedQuery.domain))
      .filter((event) => normalizedQuery.phase === undefined || event.phase === normalizedQuery.phase)
      .filter((event) => normalizedQuery.status === undefined || event.status === normalizedQuery.status)
      .filter((event) => normalizedQuery.provider === undefined || event.provider === normalizedQuery.provider)
      .filter((event) => normalizedQuery.model === undefined || event.model === normalizedQuery.model)
      .slice(0, limit)
      .map(cloneEvent);
  }

  snapshot(): AutonomousRunTraceSnapshot {
    const body = snapshotBody(this.eventsValue);
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousRunTraceSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > this.maxSnapshotBytes) throw new ArgumentError("autonomous run trace snapshot exceeds its byte capacity");
    return structuredClone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateSnapshot(raw, this.maxEvents, this.maxSnapshotBytes);
    const next = snapshot.events.map(cloneEvent);
    this.eventsValue.splice(0, this.eventsValue.length, ...next);
  }

  verifyIntegrity(): { verified: true; events: number; head_digest: string } {
    return verifyEventChain(this.eventsValue, this.maxEvents);
  }
}

/** Coordinates atomic caller-owned snapshot persistence without putting storage in the brain. */
export class AutonomousRunTracePersistenceCoordinator {
  readonly store: AutonomousRunTraceStore;
  readonly persistence: AutonomousRunTracePersistence;
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(store: AutonomousRunTraceStore, persistence: AutonomousRunTracePersistence) {
    if (!store || typeof store.append !== "function" || typeof store.events !== "function" || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("autonomous run trace persistence requires a complete trace store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous run trace persistence adapter is malformed");
    this.store = store;
    this.persistence = persistence;
  }

  async restore(): Promise<AutonomousRunTraceSnapshot | null> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      await this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  async flush(): Promise<AutonomousRunTraceSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await this.store.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("autonomous run trace persistence compare-and-swap conflict");
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

export interface AutonomousRunTraceSessionInput {
  run_id: string;
  task_digest: string;
  domains: readonly AutonomousDomainName[];
}

export interface AutonomousRunTraceCompletion {
  status: AutonomousRunTraceStatus;
  route_digest?: string | null;
  plan_digest?: string | null;
  selection_digest?: string | null;
  domains?: readonly AutonomousDomainName[];
  detail_digest?: string | null;
  failure_class?: string | null;
  failure_code?: string | null;
}

function terminalPhase(status: AutonomousRunTraceStatus): AutonomousRunTracePhase {
  if (status === "completed" || status === "partial") return "completed";
  if (status === "paused") return "paused";
  if (status === "refused") return "refused";
  if (status === "failed") return "failed";
  return "failed";
}

/** Coordinates one trace's lifecycle and creates a provider observer without retaining payloads. */
export class AutonomousRunTraceSession {
  readonly store: AutonomousRunTraceStore;
  readonly run_id: string;
  readonly task_digest: string;
  readonly domains: AutonomousDomainName[];
  private terminal = false;

  constructor(store: AutonomousRunTraceStore, input: AutonomousRunTraceSessionInput) {
    if (!store || typeof store.append !== "function" || typeof store.events !== "function") throw new ArgumentError("autonomous run trace session requires a trace store");
    this.store = store;
    this.run_id = identifier("autonomous run trace run_id", input.run_id);
    this.task_digest = sha256Digest("autonomous run trace task_digest", input.task_digest, false) as string;
    this.domains = normalizeDomains(input.domains);
  }

  async started(detailDigest: string | null = null): Promise<AutonomousRunTraceEvent> {
    const existing = await this.store.events({ run_id: this.run_id, limit: MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT });
    if (existing.length > 0) throw new ArgumentError("autonomous run trace run_id already has events");
    return this.record({ phase: "started", status: "running", detail_digest: detailDigest });
  }

  async record(input: Omit<AutonomousRunTraceEventInput, "run_id" | "task_digest" | "domains"> & { domains?: readonly AutonomousDomainName[] }): Promise<AutonomousRunTraceEvent> {
    if (this.terminal) throw new ArgumentError("autonomous run trace session is already terminal");
    const event = await this.store.append({
      ...input,
      run_id: this.run_id,
      task_digest: this.task_digest,
      domains: input.domains ?? this.domains,
    });
    return event;
  }

  providerObserver(): ProviderInvocationObserver {
    return {
      before: async (metadata: ProviderInvocationMetadata): Promise<void> => {
        await this.record({ phase: "provider_invocation_started", status: "running", provider: metadata.provider, model: metadata.model, input_tokens: metadata.inputTokens, output_tokens: null, tool_count: metadata.toolCount });
      },
      after: async (metadata: ProviderInvocationMetadata, outcome: ProviderInvocationOutcome): Promise<void> => {
        await this.record({
          phase: "provider_invocation_finished",
          status: "running",
          provider: metadata.provider,
          model: metadata.model,
          latency_ms: outcome.latencyMs,
          input_tokens: outcome.inputTokens,
          output_tokens: outcome.outputTokens,
          tool_count: metadata.toolCount,
          status_code: outcome.statusCode ?? null,
          failure_class: outcome.failureClass ?? null,
          failure_code: outcome.failureCode ?? null,
          retryable: outcome.retryable ?? null,
        });
      },
    };
  }

  async complete(completion: AutonomousRunTraceCompletion): Promise<AutonomousRunTraceEvent> {
    if (this.terminal) throw new ArgumentError("autonomous run trace session is already terminal");
    if (!AUTONOMOUS_RUN_TRACE_STATUSES.includes(completion.status)) throw new ArgumentError("autonomous run trace completion status is invalid");
    this.terminal = true;
    return this.store.append({
      run_id: this.run_id,
      task_digest: this.task_digest,
      domains: completion.domains ?? this.domains,
      phase: terminalPhase(completion.status),
      status: completion.status,
      route_digest: completion.route_digest ?? null,
      plan_digest: completion.plan_digest ?? null,
      selection_digest: completion.selection_digest ?? null,
      detail_digest: completion.detail_digest ?? null,
      failure_class: completion.failure_class ?? null,
      failure_code: completion.failure_code ?? null,
    });
  }

  async fail(input: { failure_class?: string | null; failure_code?: string | null; detail_digest?: string | null } = {}): Promise<AutonomousRunTraceEvent> {
    return this.complete({ status: "failed", ...input });
  }

  async summary(): Promise<AutonomousRunTraceSummary> {
    const events = await this.store.events({ run_id: this.run_id, limit: MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT });
    if (!events.length) throw new ArgumentError("autonomous run trace has no events");
    const last = events.at(-1)!;
    const domains = [...new Set(events.flatMap((event) => event.domains))].sort() as AutonomousDomainName[];
    const selectionDigests = [...new Set(events.map((event) => event.selection_digest).filter((value): value is string => value !== null))].sort();
    const failureCodes = [...new Set(events.map((event) => event.failure_code).filter((value): value is string => value !== null))].sort();
    const routeDigest = [...events].reverse().find((event) => event.route_digest !== null)?.route_digest ?? null;
    const planDigest = [...events].reverse().find((event) => event.plan_digest !== null)?.plan_digest ?? null;
    const completedInvocations = events.filter((event) => event.phase === "provider_invocation_finished");
    const summaryBody = {
      schema: AUTONOMOUS_RUN_TRACE_SCHEMA,
      run_id: this.run_id,
      task_digest: this.task_digest,
      domains,
      status: last.status,
      first_sequence: events[0]?.sequence ?? null,
      last_sequence: last.sequence,
      event_count: events.length,
      provider_invocations: events.filter((event) => event.phase === "provider_invocation_finished").length,
      provider_failures: events.filter((event) => event.phase === "provider_invocation_finished" && event.failure_code !== null).length,
      input_tokens: completedInvocations.reduce((total, event) => total + (event.input_tokens ?? 0), 0),
      output_tokens: completedInvocations.reduce((total, event) => total + (event.output_tokens ?? 0), 0),
      tool_calls: completedInvocations.reduce((total, event) => total + (event.tool_count ?? 0), 0),
      route_digest: routeDigest,
      plan_digest: planDigest,
      selection_digests: selectionDigests,
      failure_codes: failureCodes,
      retention: "metadata_only_no_prompts_responses_or_tool_payloads" as const,
      secret_material: "never_returned" as const,
    };
    return { ...summaryBody, trace_digest: digestJsonSync(summaryBody) };
  }
}

/** Map a provider/run status into the explicit trace state without inventing success. */
export function autonomousRunTraceStatus(status: string): AutonomousRunTraceStatus {
  if (status === "completed") return "completed";
  if (status === "cross_domain_partial" || status === "children_partial" || status === "children_completed" || status === "completed_without_replan" || status === "replan_limit_reached") return "partial";
  if (status === "route_review_required" || status === "approval_required" || status === "reconciliation_required" || status === "turn_limit_reached" || status === "plan_review_required" || status === "connector_blocked" || status === "paused" || status === "stage_blocked" || status === "stage_proposed" || status === "stage_not_attempted") return "paused";
  if (status === "abstained" || status === "provider_abstained" || status === "provider_invalid" || status === "provider_disagreement") return "refused";
  if (status === "child_failed" || status === "execution_failed" || status === "stage_failed" || status === "provider_failed") return "failed";
  return "unknown";
}
