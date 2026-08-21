import { ArgumentError, isObject } from "./errors.js";
import type {
  AutonomousCapabilityExecutionRecord,
  AutonomousCapabilityObservation,
} from "./autonomous-capabilities.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only, caller-owned persistence for reviewed capability executions. */
export const AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-capability-journal/0.1" as const;
export const AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-capability-journal-snapshot/0.1" as const;
export const AUTONOMOUS_CAPABILITY_JOURNAL_MAX_ENTRIES = 4_096;
export const AUTONOMOUS_CAPABILITY_JOURNAL_MAX_SNAPSHOT_BYTES = 64_000_000;

export class AutonomousCapabilityPersistenceError extends ArgumentError {
  override readonly name = "AutonomousCapabilityPersistenceError";
}

export interface AutonomousCapabilityJournalEntry extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA;
  sequence: number;
  previous_entry_digest: string | null;
  record: AutonomousCapabilityExecutionRecord;
  entry_digest: string;
  retention: "metadata_only_hash_chained_no_private_payloads";
  secret_material: "never_returned";
}

export interface AutonomousCapabilityJournalSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA;
  entries: AutonomousCapabilityJournalEntry[];
  head_digest: string | null;
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousCapabilityJournalStore {
  append(record: AutonomousCapabilityExecutionRecord): Promise<AutonomousCapabilityJournalEntry> | AutonomousCapabilityJournalEntry;
  find(requestDigest: string): Promise<AutonomousCapabilityExecutionRecord | null> | AutonomousCapabilityExecutionRecord | null;
  records(): Promise<readonly AutonomousCapabilityExecutionRecord[]> | readonly AutonomousCapabilityExecutionRecord[];
}

export interface AutonomousCapabilityJournalSnapshotStore extends AutonomousCapabilityJournalStore {
  snapshot(): Promise<AutonomousCapabilityJournalSnapshot>;
  restore(snapshot: AutonomousCapabilityJournalSnapshot): Promise<void> | void;
}

export interface AutonomousCapabilityJournalSnapshotPersistence {
  read(): Promise<AutonomousCapabilityJournalSnapshot | null> | AutonomousCapabilityJournalSnapshot | null;
  write(snapshot: AutonomousCapabilityJournalSnapshot): Promise<void> | void;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function jsonBytes(value: unknown): number {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new AutonomousCapabilityPersistenceError("capability journal metadata must be JSON serializable");
  }
  if (typeof encoded !== "string") throw new AutonomousCapabilityPersistenceError("capability journal metadata must be JSON serializable");
  return new TextEncoder().encode(encoded).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || /[\u0000-\u001F\u007F]/.test(value)) throw new AutonomousCapabilityPersistenceError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousCapabilityPersistenceError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousCapabilityPersistenceError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new AutonomousCapabilityPersistenceError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function assertKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const keys = new Set(allowed);
  if (Object.keys(value).some((key) => !keys.has(key))) throw new AutonomousCapabilityPersistenceError(`${name} contains unsupported fields`);
}

function assertRequired(name: string, value: Record<string, unknown>, required: readonly string[]): void {
  if (required.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) throw new AutonomousCapabilityPersistenceError(`${name} is missing required metadata fields`);
}

/** Reject suspicious payload-shaped keys even if a future schema extension accidentally permits them. */
function inspectMetadata(value: unknown, path: string, depth = 0): void {
  if (depth > 16) throw new AutonomousCapabilityPersistenceError(`${path} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 8_192) throw new AutonomousCapabilityPersistenceError(`${path} contains too many rows`);
    value.forEach((child, index) => inspectMetadata(child, `${path}[${index}]`, depth + 1));
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (/^(task|prompt|response|content|instruction|evidence|output|arguments?|credential|password|secret|token|payload|transcript|value)$/i.test(key)) {
      throw new AutonomousCapabilityPersistenceError(`${path}.${key} is not allowed in metadata-only capability records`);
    }
    inspectMetadata(child, `${path}.${key}`, depth + 1);
  }
}

function validateStringList(name: string, value: unknown, maximum: number, textMaximum = 512): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new AutonomousCapabilityPersistenceError(`${name} is malformed`);
  return value.map((item, index) => boundedText(`${name}[${index}]`, item, textMaximum));
}

function validateDigestList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new AutonomousCapabilityPersistenceError(`${name} is malformed`);
  return value.map((item, index) => boundedDigest(`${name}[${index}]`, item)!);
}

function validateObservation(value: unknown, index: number): AutonomousCapabilityObservation {
  if (!isObject(value)) throw new AutonomousCapabilityPersistenceError(`capability journal observation ${index} must be an object`);
  const keys = ["schema", "id", "label", "kind", "status", "value_digest", "source_digest", "confidence", "limitations"] as const;
  assertKeys(`capability journal observation ${index}`, value, keys);
  assertRequired(`capability journal observation ${index}`, value, keys);
  if (value.schema !== "bioprism-typescript-autonomous-capability-observation/0.1") throw new AutonomousCapabilityPersistenceError(`capability journal observation ${index} schema is invalid`);
  const kind = value.kind;
  const status = value.status;
  if (!["fact", "measurement", "provenance", "limitation", "warning"].includes(kind as string)) throw new AutonomousCapabilityPersistenceError(`capability journal observation ${index} kind is invalid`);
  if (!["observed", "inferred", "missing"].includes(status as string)) throw new AutonomousCapabilityPersistenceError(`capability journal observation ${index} status is invalid`);
  const confidence = value.confidence;
  if (confidence !== null && (typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1)) throw new AutonomousCapabilityPersistenceError(`capability journal observation ${index} confidence is invalid`);
  const limitations = validateStringList(`capability journal observation ${index}.limitations`, value.limitations, 32, 2_048);
  return {
    schema: value.schema,
    id: boundedIdentifier(`capability journal observation ${index}.id`, value.id),
    label: boundedText(`capability journal observation ${index}.label`, value.label, 256),
    kind: kind as AutonomousCapabilityObservation["kind"],
    status: status as AutonomousCapabilityObservation["status"],
    value_digest: boundedDigest(`capability journal observation ${index}.value_digest`, value.value_digest, true),
    source_digest: boundedDigest(`capability journal observation ${index}.source_digest`, value.source_digest, true),
    confidence,
    limitations,
  };
}

const RECORD_KEYS = [
  "schema", "record_kind", "request_digest", "execution_id", "call_id", "domain", "workflow_id", "workflow_digest", "stage_id",
  "stage_contract_digest", "tool", "capability", "risk_class", "schema_digest", "input_digest", "subject_digest", "parent_evidence_digests",
  "arguments_digest", "replay_key_digest", "status", "replay", "output_digest", "output_bytes", "observations", "evidence_digest",
  "evidence_status", "required_evidence_outputs", "missing_evidence_outputs", "limitations", "effect", "effect_id", "error_class",
  "duration_ms", "does_not_claim", "secret_material",
] as const;

function recordEvidenceDescriptor(record: AutonomousCapabilityExecutionRecord): JsonObject {
  return {
    schema: record.schema,
    request_digest: record.request_digest,
    input_digest: record.input_digest,
    arguments_digest: record.arguments_digest,
    output_digest: record.output_digest,
    required_evidence_outputs: record.required_evidence_outputs,
    observations: record.observations,
    evidence_status: record.evidence_status,
  };
}

/** Validate one metadata-only record before it enters a journal or is rehydrated. */
export async function validateAutonomousCapabilityExecutionRecord(value: unknown): Promise<AutonomousCapabilityExecutionRecord> {
  if (!isObject(value)) throw new AutonomousCapabilityPersistenceError("capability journal record must be an object");
  assertKeys("capability journal record", value, RECORD_KEYS);
  assertRequired("capability journal record", value, RECORD_KEYS);
  if (value.schema !== "bioprism-typescript-autonomous-capability-execution/0.1" || value.record_kind !== "capability_execution_record") throw new AutonomousCapabilityPersistenceError("capability journal record schema is invalid");
  if (value.secret_material !== "never_returned") throw new AutonomousCapabilityPersistenceError("capability journal record secret marker is invalid");
  if (value.replay !== "fresh") throw new AutonomousCapabilityPersistenceError("only fresh execution records may be persisted");
  const status = value.status;
  if (!["completed", "approval_required", "reconciliation_required", "refused", "failed"].includes(status as string)) throw new AutonomousCapabilityPersistenceError("capability journal record status is invalid");
  const evidenceStatus = value.evidence_status;
  if (!["not_evaluated", "missing_required_outputs", "declared_for_evaluator", "projection_failed"].includes(evidenceStatus as string)) throw new AutonomousCapabilityPersistenceError("capability journal evidence status is invalid");
  const record: AutonomousCapabilityExecutionRecord = {
    schema: value.schema,
    record_kind: value.record_kind,
    request_digest: boundedDigest("capability journal record request_digest", value.request_digest)!,
    execution_id: value.execution_id === null ? null : boundedText("capability journal record execution_id", value.execution_id, 256),
    call_id: boundedText("capability journal record call_id", value.call_id, 256),
    domain: boundedIdentifier("capability journal record domain", value.domain),
    workflow_id: boundedIdentifier("capability journal record workflow_id", value.workflow_id),
    workflow_digest: boundedDigest("capability journal record workflow_digest", value.workflow_digest)!,
    stage_id: boundedIdentifier("capability journal record stage_id", value.stage_id),
    stage_contract_digest: boundedDigest("capability journal record stage_contract_digest", value.stage_contract_digest, true),
    tool: boundedIdentifier("capability journal record tool", value.tool),
    capability: value.capability === null ? null : boundedText("capability journal record capability", value.capability, 256),
    risk_class: value.risk_class === null ? null : boundedText("capability journal record risk_class", value.risk_class, 256),
    schema_digest: boundedDigest("capability journal record schema_digest", value.schema_digest, true),
    input_digest: boundedDigest("capability journal record input_digest", value.input_digest)!,
    subject_digest: boundedDigest("capability journal record subject_digest", value.subject_digest, true),
    parent_evidence_digests: validateDigestList("capability journal record parent_evidence_digests", value.parent_evidence_digests, 64),
    arguments_digest: boundedDigest("capability journal record arguments_digest", value.arguments_digest)!,
    replay_key_digest: boundedDigest("capability journal record replay_key_digest", value.replay_key_digest, true),
    status: status as AutonomousCapabilityExecutionRecord["status"],
    replay: "fresh",
    output_digest: boundedDigest("capability journal record output_digest", value.output_digest, true),
    output_bytes: boundedCount("capability journal record output_bytes", value.output_bytes, 64_000_000),
    observations: Array.isArray(value.observations) && value.observations.length <= 128 ? value.observations.map(validateObservation) : (() => { throw new AutonomousCapabilityPersistenceError("capability journal observations are malformed"); })(),
    evidence_digest: boundedDigest("capability journal record evidence_digest", value.evidence_digest, true),
    evidence_status: evidenceStatus as AutonomousCapabilityExecutionRecord["evidence_status"],
    required_evidence_outputs: validateStringList("capability journal record required_evidence_outputs", value.required_evidence_outputs, 128),
    missing_evidence_outputs: validateStringList("capability journal record missing_evidence_outputs", value.missing_evidence_outputs, 128),
    limitations: validateStringList("capability journal record limitations", value.limitations, 64, 2_048),
    effect: value.effect === null ? null : boundedText("capability journal record effect", value.effect, 256),
    effect_id: value.effect_id === null ? null : boundedText("capability journal record effect_id", value.effect_id, 256),
    error_class: value.error_class === null ? null : boundedIdentifier("capability journal record error_class", value.error_class),
    duration_ms: boundedCount("capability journal record duration_ms", value.duration_ms, 86_400_000),
    does_not_claim: validateStringList("capability journal record does_not_claim", value.does_not_claim, 32, 1_024),
    secret_material: "never_returned",
  };
  if (record.status === "completed" && record.output_digest === null) throw new AutonomousCapabilityPersistenceError("completed capability records require an output digest");
  if (record.status !== "completed" && record.output_digest !== null) throw new AutonomousCapabilityPersistenceError("non-completed capability records cannot contain an output digest");
  if (record.evidence_digest !== null) {
    if (record.output_digest === null) throw new AutonomousCapabilityPersistenceError("evidence digest requires an output digest");
    if (await digestJson(recordEvidenceDescriptor(record)) !== record.evidence_digest) throw new AutonomousCapabilityPersistenceError("capability evidence digest does not match its metadata");
  }
  inspectMetadata(record, "capability journal record");
  if (jsonBytes(record) > 8_000_000) throw new AutonomousCapabilityPersistenceError("capability journal record exceeds its byte capacity");
  return clone(record);
}

function entryDescriptor(entry: Omit<AutonomousCapabilityJournalEntry, "entry_digest">): JsonObject {
  return entry;
}

async function sealEntry(sequence: number, previousEntryDigest: string | null, record: AutonomousCapabilityExecutionRecord): Promise<AutonomousCapabilityJournalEntry> {
  const descriptor = {
    schema: AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA,
    sequence,
    previous_entry_digest: previousEntryDigest,
    record,
    retention: "metadata_only_hash_chained_no_private_payloads" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, entry_digest: await digestJson(entryDescriptor(descriptor)) };
}

/** Validate a complete journal entry, including its row hash. */
export async function validateAutonomousCapabilityJournalEntry(value: unknown): Promise<AutonomousCapabilityJournalEntry> {
  if (!isObject(value)) throw new AutonomousCapabilityPersistenceError("capability journal entry must be an object");
  const keys = ["schema", "sequence", "previous_entry_digest", "record", "entry_digest", "retention", "secret_material"] as const;
  assertKeys("capability journal entry", value, keys);
  assertRequired("capability journal entry", value, keys);
  if (value.schema !== AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA || value.retention !== "metadata_only_hash_chained_no_private_payloads" || value.secret_material !== "never_returned") throw new AutonomousCapabilityPersistenceError("capability journal entry retention markers are invalid");
  const sequence = boundedCount("capability journal entry sequence", value.sequence, AUTONOMOUS_CAPABILITY_JOURNAL_MAX_ENTRIES, 1);
  const previousEntryDigest = boundedDigest("capability journal entry previous_entry_digest", value.previous_entry_digest, true);
  const record = await validateAutonomousCapabilityExecutionRecord(value.record);
  const entryDigest = boundedDigest("capability journal entry entry_digest", value.entry_digest)!;
  const descriptor = { schema: AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA, sequence, previous_entry_digest: previousEntryDigest, record, retention: "metadata_only_hash_chained_no_private_payloads" as const, secret_material: "never_returned" as const };
  if (await digestJson(descriptor) !== entryDigest) throw new AutonomousCapabilityPersistenceError("capability journal entry digest does not match its metadata");
  return clone({ ...descriptor, entry_digest: entryDigest });
}

/** Validate a journal snapshot, including every row and the snapshot root digest. */
export async function validateAutonomousCapabilityJournalSnapshot(value: unknown): Promise<AutonomousCapabilityJournalSnapshot> {
  if (!isObject(value)) throw new AutonomousCapabilityPersistenceError("capability journal snapshot must be an object");
  const keys = ["schema", "entries", "head_digest", "retention", "secret_material", "snapshot_digest"] as const;
  assertKeys("capability journal snapshot", value, keys);
  assertRequired("capability journal snapshot", value, keys);
  if (value.schema !== AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA || value.retention !== "metadata_only_hash_bound" || value.secret_material !== "never_returned") throw new AutonomousCapabilityPersistenceError("capability journal snapshot retention markers are invalid");
  if (!Array.isArray(value.entries) || value.entries.length > AUTONOMOUS_CAPABILITY_JOURNAL_MAX_ENTRIES) throw new AutonomousCapabilityPersistenceError("capability journal snapshot exceeds its entry capacity");
  const entries = await Promise.all(value.entries.map((entry) => validateAutonomousCapabilityJournalEntry(entry)));
  const requestDigests = new Set<string>();
  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index]!;
    if (entry.sequence !== index + 1) throw new AutonomousCapabilityPersistenceError("capability journal sequences must be contiguous from one");
    if (!requestDigests.add(entry.record.request_digest)) throw new AutonomousCapabilityPersistenceError("capability journal contains duplicate request digests");
    const expectedPrevious = index === 0 ? null : entries[index - 1]!.entry_digest;
    if (entry.previous_entry_digest !== expectedPrevious) throw new AutonomousCapabilityPersistenceError("capability journal hash-chain continuity check failed");
  }
  const headDigest = boundedDigest("capability journal snapshot head_digest", value.head_digest, true);
  if (headDigest !== (entries.length ? entries[entries.length - 1]!.entry_digest : null)) throw new AutonomousCapabilityPersistenceError("capability journal snapshot head digest does not match its entries");
  const snapshotDigest = boundedDigest("capability journal snapshot snapshot_digest", value.snapshot_digest)!;
  const descriptor = { schema: AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA, entries, head_digest: headDigest, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
  if (await digestJson(descriptor) !== snapshotDigest) throw new AutonomousCapabilityPersistenceError("capability journal snapshot digest does not match its metadata");
  if (jsonBytes(value) > AUTONOMOUS_CAPABILITY_JOURNAL_MAX_SNAPSHOT_BYTES) throw new AutonomousCapabilityPersistenceError("capability journal snapshot exceeds its byte capacity");
  return clone({ ...descriptor, snapshot_digest: snapshotDigest });
}

/** Bounded in-memory journal for tests, desktop workers, and small caller-owned deployments. */
export class InMemoryAutonomousCapabilityJournalStore implements AutonomousCapabilityJournalSnapshotStore {
  private readonly entries: AutonomousCapabilityJournalEntry[] = [];

  async append(rawRecord: AutonomousCapabilityExecutionRecord): Promise<AutonomousCapabilityJournalEntry> {
    const record = await validateAutonomousCapabilityExecutionRecord(rawRecord);
    const existing = this.entries.find((entry) => entry.record.request_digest === record.request_digest);
    if (existing) {
      if (await digestJson(existing.record) !== await digestJson(record)) throw new AutonomousCapabilityPersistenceError("capability journal request digest conflicts with an existing record");
      return clone(existing);
    }
    if (this.entries.length >= AUTONOMOUS_CAPABILITY_JOURNAL_MAX_ENTRIES) throw new AutonomousCapabilityPersistenceError("capability journal capacity exhausted");
    const entry = await sealEntry(this.entries.length + 1, this.entries.at(-1)?.entry_digest ?? null, record);
    this.entries.push(entry);
    return clone(entry);
  }

  async find(requestDigest: string): Promise<AutonomousCapabilityExecutionRecord | null> {
    const digest = boundedDigest("capability journal request_digest", requestDigest)!;
    const entry = this.entries.find((candidate) => candidate.record.request_digest === digest);
    return entry ? clone(entry.record) : null;
  }

  async records(): Promise<readonly AutonomousCapabilityExecutionRecord[]> {
    return this.entries.map((entry) => clone(entry.record));
  }

  async snapshot(): Promise<AutonomousCapabilityJournalSnapshot> {
    const descriptor = {
      schema: AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
      entries: this.entries.map(clone),
      head_digest: this.entries.at(-1)?.entry_digest ?? null,
      retention: "metadata_only_hash_bound" as const,
      secret_material: "never_returned" as const,
    };
    return { ...descriptor, snapshot_digest: await digestJson(descriptor) };
  }

  async restore(rawSnapshot: AutonomousCapabilityJournalSnapshot): Promise<void> {
    const snapshot = await validateAutonomousCapabilityJournalSnapshot(rawSnapshot);
    this.entries.length = 0;
    this.entries.push(...snapshot.entries.map(clone));
  }
}

/** Flushes or restores a journal through caller-owned durable storage. */
export class AutonomousCapabilityJournalPersistenceCoordinator {
  constructor(readonly store: AutonomousCapabilityJournalSnapshotStore, readonly persistence: AutonomousCapabilityJournalSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new AutonomousCapabilityPersistenceError("capability journal persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousCapabilityPersistenceError("capability journal persistence requires readable and writable storage");
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA; bytes: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return { schema: snapshot.schema, bytes: jsonBytes(snapshot), snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }

  async restore(): Promise<{ restored: boolean; entry_count: number; snapshot_digest: string | null }> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) return { restored: false, entry_count: 0, snapshot_digest: null };
    const validated = await validateAutonomousCapabilityJournalSnapshot(snapshot);
    await this.store.restore(validated);
    return { restored: true, entry_count: validated.entries.length, snapshot_digest: validated.snapshot_digest };
  }
}
