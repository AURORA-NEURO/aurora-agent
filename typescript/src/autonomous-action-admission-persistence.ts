import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousActionAdmission,
  AutonomousActionPlan,
  admitAutonomousActionPlan,
  type AutonomousActionAdmissionJSON,
  type AutonomousActionPlanApproval,
  type AutonomousActionPlanJSON,
} from "./autonomous-action-plan.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Durable operator handoff for one action plan and its explicit admission.
 *
 * The plan and admission are already metadata-only. This envelope adds the caller's review
 * identity, a monotonic revision, and a predecessor digest so an application can persist an
 * auditable approval process without putting task text, credentials, prompts, provider values,
 * or effect authority into its durable store.
 */
export const AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA = "bioprism-typescript-autonomous-action-admission-record/0.1" as const;
export const AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-action-admission-snapshot/0.1" as const;
export const AUTONOMOUS_ACTION_ADMISSION_RETENTION = "metadata_only;plan_admission_and_review_digests;task_prompt_provider_connector_credential_and_effect_values_not_retained" as const;
export const AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL = "never_returned" as const;
export const AUTONOMOUS_ACTION_ADMISSION_AUTHORITY = "caller_review_record_only;does_not_authorize_provider_source_tool_effect_or_credentials" as const;
export const AUTONOMOUS_ACTION_ADMISSION_EXECUTION = "admission_only;downstream_provider_source_tool_effect_and_credential_gates_remain_required" as const;
export const MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS = 4_096;
export const MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES = 4_000_000;
export const MAX_AUTONOMOUS_ACTION_ADMISSION_ACTION_ID_BYTES = 256;

export type AutonomousActionAdmissionRecordStatus = "pending_review" | "admitted" | "blocked";
export type AutonomousActionAdmissionRecordDecision = "submitted" | "reviewed";

export interface AutonomousActionAdmissionRecord extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA;
  action_id: string;
  revision: number;
  status: AutonomousActionAdmissionRecordStatus;
  decision: AutonomousActionAdmissionRecordDecision;
  plan: AutonomousActionPlanJSON;
  admission: AutonomousActionAdmissionJSON;
  reviewer_digest: string | null;
  reason_digest: string | null;
  previous_record_digest: string | null;
  authority: typeof AUTONOMOUS_ACTION_ADMISSION_AUTHORITY;
  retention: typeof AUTONOMOUS_ACTION_ADMISSION_RETENTION;
  execution: typeof AUTONOMOUS_ACTION_ADMISSION_EXECUTION;
  secret_material: typeof AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL;
  record_digest: string;
}

export interface AutonomousActionAdmissionSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA;
  generation: number;
  records: AutonomousActionAdmissionRecord[];
  previous_snapshot_digest: string | null;
  retention: typeof AUTONOMOUS_ACTION_ADMISSION_RETENTION;
  secret_material: typeof AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL;
  snapshot_digest: string;
}

export interface AutonomousActionAdmissionRecordCreateOptions {
  actionId: string;
  reviewerDigest?: string | null;
  reason?: string | null;
  previousRecordDigest?: string | null;
}

export interface AutonomousActionAdmissionReviewOptions {
  approvals?: Partial<Record<AutonomousActionPlanApproval, boolean>>;
  reviewed?: boolean;
  reviewerDigest: string;
  reason?: string | null;
  expectedRecordDigest?: string | null;
}

export interface AutonomousActionAdmissionSnapshotTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface TransactionalAutonomousActionAdmissionSnapshotTextStore extends AutonomousActionAdmissionSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface AutonomousActionAdmissionSnapshotPersistence {
  read(): AutonomousActionAdmissionSnapshot | null | Promise<AutonomousActionAdmissionSnapshot | null>;
  write(snapshot: AutonomousActionAdmissionSnapshot): void | Promise<void>;
}

export interface TransactionalAutonomousActionAdmissionSnapshotPersistence extends AutonomousActionAdmissionSnapshotPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousActionAdmissionSnapshot): boolean | Promise<boolean>;
}

const RECORD_STATUSES: readonly AutonomousActionAdmissionRecordStatus[] = ["pending_review", "admitted", "blocked"];
const DECISIONS: readonly AutonomousActionAdmissionRecordDecision[] = ["submitted", "reviewed"];
const RECORD_KEYS = ["schema", "action_id", "revision", "status", "decision", "plan", "admission", "reviewer_digest", "reason_digest", "previous_record_digest", "authority", "retention", "execution", "secret_material"] as const;
const SNAPSHOT_KEYS = ["schema", "generation", "records", "previous_snapshot_digest", "retention", "secret_material"] as const;

function fail(message: string): never {
  throw new ArgumentError(`autonomous action admission persistence ${message}`);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: unknown): number {
  return new TextEncoder().encode(canonicalJson(value)).byteLength;
}

function text(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) fail(`${name} is outside its text bound`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const result = text(name, value, MAX_AUTONOMOUS_ACTION_ADMISSION_ACTION_ID_BYTES);
  if (!/^[A-Za-z0-9_.:+/-]+$/.test(result)) fail(`${name} contains unsupported identifier characters`);
  return result;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bound`);
  return value;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function exactKeys(name: string, value: Record<string, unknown>, expected: readonly string[]): void {
  const allowed = new Set(expected);
  if (Object.keys(value).some((key) => !allowed.has(key)) || expected.some((key) => !(key in value))) fail(`${name} has unsupported or missing fields`);
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 16) fail("metadata nesting exceeds its bound");
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) fail("metadata contains a non-finite number");
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > 4_096) fail("metadata sequence exceeds its bound");
    for (const child of value) safeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 4_096) fail("metadata object exceeds its bound");
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.replaceAll("_", "").replaceAll("-", "").toLowerCase();
      if (["task", "prompt", "credential", "credentials", "secret", "token", "password", "response", "messages", "body", "headers"].includes(normalized)) fail("metadata contains transient or secret-shaped material");
      safeMetadata(child, depth + 1);
    }
    return;
  }
  fail("metadata contains an unsupported value");
}

function normalizePlan(value: AutonomousActionPlan | AutonomousActionPlanJSON): AutonomousActionPlan {
  try {
    return value instanceof AutonomousActionPlan ? value : AutonomousActionPlan.fromJSON(value);
  } catch (error) {
    const wrapped = new ArgumentError("autonomous action admission record plan is invalid");
    (wrapped as Error & { cause?: unknown }).cause = error;
    throw wrapped;
  }
}

function normalizeAdmission(value: AutonomousActionAdmission | AutonomousActionAdmissionJSON): AutonomousActionAdmission {
  try {
    return value instanceof AutonomousActionAdmission ? value : AutonomousActionAdmission.fromJSON(value);
  } catch (error) {
    const wrapped = new ArgumentError("autonomous action admission record admission is invalid");
    (wrapped as Error & { cause?: unknown }).cause = error;
    throw wrapped;
  }
}

function recordStatus(admission: AutonomousActionAdmission): AutonomousActionAdmissionRecordStatus {
  if (admission.status === "admitted") return "admitted";
  if (admission.status === "blocked") return "blocked";
  return "pending_review";
}

function recordBody(input: {
  actionId: string;
  revision: number;
  plan: AutonomousActionPlan;
  admission: AutonomousActionAdmission;
  reviewerDigest: string | null;
  reasonDigest: string | null;
  previousRecordDigest: string | null;
}): Omit<AutonomousActionAdmissionRecord, "record_digest"> {
  const status = recordStatus(input.admission);
  const decision: AutonomousActionAdmissionRecordDecision = input.reviewerDigest === null ? "submitted" : "reviewed";
  if (status === "admitted" && input.reviewerDigest === null) fail("an admitted record requires a reviewer digest");
  return {
    schema: AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA,
    action_id: identifier("action_id", input.actionId),
    revision: integer("revision", input.revision, 1, 2_147_483_647),
    status,
    decision,
    plan: input.plan.toJSON(),
    admission: input.admission.toJSON(),
    reviewer_digest: digest("reviewer_digest", input.reviewerDigest, true),
    reason_digest: digest("reason_digest", input.reasonDigest, true),
    previous_record_digest: digest("previous_record_digest", input.previousRecordDigest, true),
    authority: AUTONOMOUS_ACTION_ADMISSION_AUTHORITY,
    retention: AUTONOMOUS_ACTION_ADMISSION_RETENTION,
    execution: AUTONOMOUS_ACTION_ADMISSION_EXECUTION,
    secret_material: AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
  };
}

function normalizedRecord(value: unknown, requireDigest: boolean): AutonomousActionAdmissionRecord {
  if (!isObject(value)) fail("record must be an object");
  exactKeys("record", value, requireDigest ? [...RECORD_KEYS, "record_digest"] : RECORD_KEYS);
  if (value.schema !== AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA || value.authority !== AUTONOMOUS_ACTION_ADMISSION_AUTHORITY || value.retention !== AUTONOMOUS_ACTION_ADMISSION_RETENTION || value.execution !== AUTONOMOUS_ACTION_ADMISSION_EXECUTION || value.secret_material !== AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL) fail("record markers are invalid");
  const plan = normalizePlan(value.plan as AutonomousActionPlanJSON);
  const admission = normalizeAdmission(value.admission as AutonomousActionAdmissionJSON);
  if (plan.plan_digest !== admission.plan_digest) fail("record admission is bound to a different plan");
  if (recordStatus(admission) !== value.status || !RECORD_STATUSES.includes(value.status as AutonomousActionAdmissionRecordStatus) || !DECISIONS.includes(value.decision as AutonomousActionAdmissionRecordDecision)) fail("record status or decision is inconsistent");
  const body = recordBody({ actionId: value.action_id as string, revision: value.revision as number, plan, admission, reviewerDigest: value.reviewer_digest as string | null, reasonDigest: value.reason_digest as string | null, previousRecordDigest: value.previous_record_digest as string | null });
  if (canonicalJson(body) !== canonicalJson(Object.fromEntries(RECORD_KEYS.map((key) => [key, value[key]])))) fail("record metadata is not canonical");
  if (requireDigest && digest("record_digest", value.record_digest) !== digestJsonSync(body)) fail("record digest does not match metadata");
  safeMetadata(value);
  const result = { ...body, record_digest: digestJsonSync(body) } as AutonomousActionAdmissionRecord;
  if (requireDigest && result.record_digest !== value.record_digest) fail("record digest is invalid");
  return clone(result);
}

function normalizedSnapshot(value: unknown, requireDigest: boolean): AutonomousActionAdmissionSnapshot {
  if (!isObject(value)) fail("snapshot must be an object");
  exactKeys("snapshot", value, requireDigest ? [...SNAPSHOT_KEYS, "snapshot_digest"] : SNAPSHOT_KEYS);
  if (value.schema !== AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA || value.retention !== AUTONOMOUS_ACTION_ADMISSION_RETENTION || value.secret_material !== AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL) fail("snapshot markers are invalid");
  const generation = integer("snapshot generation", value.generation, 0, 2_147_483_647);
  if (!Array.isArray(value.records) || value.records.length > MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS) fail("snapshot records exceed their bound");
  const records = value.records.map((record, index) => normalizedRecord(record, true));
  const ids = new Set<string>();
  for (const record of records) {
    if (ids.has(record.action_id)) fail("snapshot contains duplicate action ids");
    ids.add(record.action_id);
  }
  records.sort((left, right) => left.action_id.localeCompare(right.action_id));
  const body = {
    schema: AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA,
    generation,
    records,
    previous_snapshot_digest: digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest, true),
    retention: AUTONOMOUS_ACTION_ADMISSION_RETENTION,
    secret_material: AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
  } as Omit<AutonomousActionAdmissionSnapshot, "snapshot_digest">;
  if (requireDigest && digest("snapshot_digest", value.snapshot_digest) !== digestJsonSync(body)) fail("snapshot digest does not match metadata");
  const result = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousActionAdmissionSnapshot;
  if (requireDigest && result.snapshot_digest !== value.snapshot_digest) fail("snapshot digest is invalid");
  if (bytes(result) > MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return clone(result);
}

export function createAutonomousActionAdmissionRecord(
  planSource: AutonomousActionPlan | AutonomousActionPlanJSON,
  admissionSource: AutonomousActionAdmission | AutonomousActionAdmissionJSON,
  options: AutonomousActionAdmissionRecordCreateOptions,
): AutonomousActionAdmissionRecord {
  if (!options || typeof options !== "object") fail("record options are malformed");
  const plan = normalizePlan(planSource);
  const admission = normalizeAdmission(admissionSource);
  if (plan.plan_digest !== admission.plan_digest) fail("admission is bound to a different plan");
  const reviewerDigest = digest("reviewerDigest", options.reviewerDigest ?? null, true);
  const reasonDigest = options.reason === undefined || options.reason === null ? null : digestJsonSync(text("reason", options.reason, 4_096));
  const body = recordBody({ actionId: options.actionId, revision: 1, plan, admission, reviewerDigest, reasonDigest, previousRecordDigest: options.previousRecordDigest ?? null });
  return clone({ ...body, record_digest: digestJsonSync(body) } as AutonomousActionAdmissionRecord);
}

export function reviewAutonomousActionAdmissionRecord(
  source: AutonomousActionAdmissionRecord,
  options: AutonomousActionAdmissionReviewOptions,
): AutonomousActionAdmissionRecord {
  const current = normalizedRecord(source, true);
  if (!options || typeof options !== "object") fail("review options are malformed");
  if (options.expectedRecordDigest !== undefined && options.expectedRecordDigest !== null && options.expectedRecordDigest !== current.record_digest) fail("review expectedRecordDigest does not match the current record");
  const reviewerDigest = digest("reviewerDigest", options.reviewerDigest, false) as string;
  const admission = admitAutonomousActionPlan(current.plan, { approvals: options.approvals, reviewed: options.reviewed ?? false });
  const reasonDigest = options.reason === undefined || options.reason === null ? null : digestJsonSync(text("reason", options.reason, 4_096));
  const body = recordBody({ actionId: current.action_id, revision: current.revision + 1, plan: normalizePlan(current.plan), admission, reviewerDigest, reasonDigest, previousRecordDigest: current.record_digest });
  return clone({ ...body, record_digest: digestJsonSync(body) } as AutonomousActionAdmissionRecord);
}

export function validateAutonomousActionAdmissionRecord(value: unknown): AutonomousActionAdmissionRecord {
  return normalizedRecord(value, true);
}

export function sealAutonomousActionAdmissionSnapshot(input: {
  generation: number;
  records: readonly AutonomousActionAdmissionRecord[];
  previousSnapshotDigest?: string | null;
}): AutonomousActionAdmissionSnapshot {
  if (!input || typeof input !== "object") fail("snapshot input is malformed");
  const body = normalizedSnapshot({
    schema: AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA,
    generation: input.generation,
    records: [...input.records],
    previous_snapshot_digest: input.previousSnapshotDigest ?? null,
    retention: AUTONOMOUS_ACTION_ADMISSION_RETENTION,
    secret_material: AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
  }, false);
  return clone(body);
}

export function validateAutonomousActionAdmissionSnapshot(value: unknown): AutonomousActionAdmissionSnapshot {
  return normalizedSnapshot(value, true);
}

export class InMemoryAutonomousActionAdmissionLedger {
  readonly maxRecords: number;
  private readonly recordsById = new Map<string, AutonomousActionAdmissionRecord>();

  constructor(options: { maxRecords?: number } = {}) {
    this.maxRecords = integer("maxRecords", options.maxRecords ?? MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS, 1, MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS);
  }

  put(record: AutonomousActionAdmissionRecord): AutonomousActionAdmissionRecord {
    const normalized = validateAutonomousActionAdmissionRecord(record);
    const existing = this.recordsById.get(normalized.action_id);
    if (existing !== undefined && existing.record_digest !== normalized.previous_record_digest && existing.record_digest !== normalized.record_digest) fail("record predecessor conflicts with the current action record");
    if (existing === undefined && normalized.revision !== 1) fail("new action records must begin at revision one");
    if (existing !== undefined && normalized.revision !== existing.revision + 1 && normalized.record_digest !== existing.record_digest) fail("action record revision is not contiguous");
    if (existing === undefined && this.recordsById.size >= this.maxRecords) fail("ledger capacity is exhausted");
    this.recordsById.set(normalized.action_id, normalized);
    return clone(normalized);
  }

  submit(plan: AutonomousActionPlan | AutonomousActionPlanJSON, admission: AutonomousActionAdmission | AutonomousActionAdmissionJSON, options: AutonomousActionAdmissionRecordCreateOptions): AutonomousActionAdmissionRecord {
    return this.put(createAutonomousActionAdmissionRecord(plan, admission, options));
  }

  review(actionId: string, options: AutonomousActionAdmissionReviewOptions): AutonomousActionAdmissionRecord {
    const current = this.get(actionId);
    if (current === null) fail("cannot review an unknown action record");
    return this.put(reviewAutonomousActionAdmissionRecord(current, options));
  }

  get(actionId: string): AutonomousActionAdmissionRecord | null {
    const normalized = identifier("actionId", actionId);
    const value = this.recordsById.get(normalized);
    return value === undefined ? null : clone(value);
  }

  list(): AutonomousActionAdmissionRecord[] {
    return [...this.recordsById.values()].sort((left, right) => left.action_id.localeCompare(right.action_id)).map(clone);
  }

  restore(snapshot: AutonomousActionAdmissionSnapshot): void {
    const normalized = validateAutonomousActionAdmissionSnapshot(snapshot);
    if (normalized.records.length > this.maxRecords) fail("snapshot exceeds ledger capacity");
    this.recordsById.clear();
    for (const record of normalized.records) this.recordsById.set(record.action_id, record);
  }
}

export class JsonAutonomousActionAdmissionSnapshotPersistence implements AutonomousActionAdmissionSnapshotPersistence {
  readonly store: AutonomousActionAdmissionSnapshotTextStore;
  readonly maxBytes: number;

  constructor(store: AutonomousActionAdmissionSnapshotTextStore, maxBytes = MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES) {
    if (typeof store?.read !== "function" || typeof store?.write !== "function") fail("JSON persistence requires a text store");
    this.store = store;
    this.maxBytes = integer("maxBytes", maxBytes, 1, MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES);
  }

  async read(): Promise<AutonomousActionAdmissionSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("stored JSON exceeds its byte bound");
    let raw: unknown;
    try { raw = JSON.parse(encoded); } catch { fail("stored JSON is invalid"); }
    const normalized = validateAutonomousActionAdmissionSnapshot(raw);
    if (canonicalJson(normalized) !== encoded) fail("stored JSON is not canonical");
    return normalized;
  }

  async write(snapshot: AutonomousActionAdmissionSnapshot): Promise<void> {
    const normalized = validateAutonomousActionAdmissionSnapshot(snapshot);
    const encoded = canonicalJson(normalized);
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("snapshot exceeds the configured byte bound");
    await this.store.write(encoded);
  }
}

export class TransactionalJsonAutonomousActionAdmissionSnapshotPersistence extends JsonAutonomousActionAdmissionSnapshotPersistence implements TransactionalAutonomousActionAdmissionSnapshotPersistence {
  override readonly store: TransactionalAutonomousActionAdmissionSnapshotTextStore;

  constructor(store: TransactionalAutonomousActionAdmissionSnapshotTextStore, maxBytes = MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES) {
    super(store, maxBytes);
    if (typeof store.writeIfUnchanged !== "function") fail("transactional JSON persistence requires writeIfUnchanged");
    this.store = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousActionAdmissionSnapshot): Promise<boolean> {
    digest("expectedSnapshotDigest", expectedSnapshotDigest, true);
    return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validateAutonomousActionAdmissionSnapshot(snapshot)));
  }
}

export class AutonomousActionAdmissionPersistenceCoordinator {
  private expectedSnapshotDigestValue: string | null = null;
  private expectedGeneration = 0;
  readonly ledger: InMemoryAutonomousActionAdmissionLedger;
  readonly persistence: AutonomousActionAdmissionSnapshotPersistence;

  constructor(ledger: InMemoryAutonomousActionAdmissionLedger, persistence: AutonomousActionAdmissionSnapshotPersistence) {
    if (!(ledger instanceof InMemoryAutonomousActionAdmissionLedger)) fail("coordinator requires a typed ledger");
    if (typeof persistence?.read !== "function" || typeof persistence?.write !== "function") fail("coordinator persistence is malformed");
    this.ledger = ledger;
    this.persistence = persistence;
  }

  get expectedSnapshotDigest(): string | null { return this.expectedSnapshotDigestValue; }

  async restore(): Promise<AutonomousActionAdmissionSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) {
      this.expectedSnapshotDigestValue = null;
      this.expectedGeneration = 0;
      return null;
    }
    const normalized = validateAutonomousActionAdmissionSnapshot(snapshot);
    this.ledger.restore(normalized);
    this.expectedSnapshotDigestValue = normalized.snapshot_digest;
    this.expectedGeneration = normalized.generation;
    return normalized;
  }

  async flush(): Promise<AutonomousActionAdmissionSnapshot> {
    const snapshot = sealAutonomousActionAdmissionSnapshot({ generation: this.expectedGeneration + 1, records: this.ledger.list(), previousSnapshotDigest: this.expectedSnapshotDigestValue });
    const transactional = this.persistence as Partial<TransactionalAutonomousActionAdmissionSnapshotPersistence>;
    if (typeof transactional.writeIfUnchanged === "function") {
      if (!await transactional.writeIfUnchanged(this.expectedSnapshotDigestValue, snapshot)) fail("persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigestValue = snapshot.snapshot_digest;
    this.expectedGeneration = snapshot.generation;
    return snapshot;
  }
}
