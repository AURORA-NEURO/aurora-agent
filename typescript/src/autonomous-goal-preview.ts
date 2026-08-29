/** Durable, metadata-only operator decisions for autonomous goal previews. */

import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type { AutonomousGoalControlLoopPreview } from "./autonomous-goal-control-loop.js";

export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA = "bioprism-autonomous-goal-preview-admission-record/0.1" as const;
export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-preview-admission-snapshot/0.1" as const;
export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION = "metadata_only_goal_preview_approval;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL = "never_returned" as const;
export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY = "caller_operator_review_only;does_not_authenticate_or_authorize_provider_source_tool_effect_or_credentials" as const;
export const AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION = "approval_only;execution_requires_current_preview_digest_and_downstream_policy_gates" as const;
export const MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS = 4_096;
export const MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES = 4_000_000;
export const MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_ID_BYTES = 256;
export const MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES = 4_096;
export const MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_TTL_NS = 7 * 24 * 60 * 60 * 1_000_000_000;

export type AutonomousGoalPreviewAdmissionStatus = "pending_review" | "approved" | "rejected";
export type AutonomousGoalPreviewAdmissionDecision = "submitted" | "approved" | "rejected";

export interface AutonomousGoalPreviewAdmissionRecord extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA;
  admission_id: string;
  revision: number;
  status: AutonomousGoalPreviewAdmissionStatus;
  decision: AutonomousGoalPreviewAdmissionDecision;
  preview: JsonObject;
  preview_digest: string;
  requested_by_digest: string | null;
  reviewer_digest: string | null;
  issued_at_ns: number;
  expires_at_ns: number;
  reason_digest: string | null;
  previous_record_digest: string | null;
  authority: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY;
  retention: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION;
  execution: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION;
  secret_material: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL;
  record_digest: string;
}

export interface AutonomousGoalPreviewAdmissionSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA;
  generation: number;
  records: AutonomousGoalPreviewAdmissionRecord[];
  previous_snapshot_digest: string | null;
  retention: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION;
  secret_material: typeof AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL;
  snapshot_digest: string;
}

export interface AutonomousGoalPreviewAdmissionRecordCreateOptions {
  admission_id: string;
  issued_at_ns: number;
  expires_at_ns: number;
  requested_by_digest?: string | null;
  reason?: string | null;
  previous_record_digest?: string | null;
}

export interface AutonomousGoalPreviewAdmissionReviewOptions {
  approved: boolean;
  reviewer_digest: string;
  reason?: string | null;
  expected_record_digest?: string | null;
}

export interface AutonomousGoalPreviewAdmissionSnapshotTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface TransactionalAutonomousGoalPreviewAdmissionSnapshotTextStore extends AutonomousGoalPreviewAdmissionSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface AutonomousGoalPreviewAdmissionSnapshotPersistence {
  read(): AutonomousGoalPreviewAdmissionSnapshot | null | Promise<AutonomousGoalPreviewAdmissionSnapshot | null>;
  write(snapshot: AutonomousGoalPreviewAdmissionSnapshot): void | Promise<void>;
}

export interface TransactionalAutonomousGoalPreviewAdmissionSnapshotPersistence extends AutonomousGoalPreviewAdmissionSnapshotPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalPreviewAdmissionSnapshot): boolean | Promise<boolean>;
}

const PREVIEW_KEYS = ["schema", "schedule", "status", "eligible_goal_count", "decision_counts", "reason_counts", "status_counts", "dependency_blocked_goal_ids", "learning_state_digest", "retention", "secret_material", "preview_digest"] as const;
const RECORD_KEYS = ["schema", "admission_id", "revision", "status", "decision", "preview", "preview_digest", "requested_by_digest", "reviewer_digest", "issued_at_ns", "expires_at_ns", "reason_digest", "previous_record_digest", "authority", "retention", "execution", "secret_material"] as const;
const SNAPSHOT_KEYS = ["schema", "generation", "records", "previous_snapshot_digest", "retention", "secret_material"] as const;

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal preview admission ${message}`);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: unknown): number {
  return new TextEncoder().encode(canonicalJson(value)).byteLength;
}

function text(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) fail(`${name} is outside its text bound`);
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_ID_BYTES): string {
  const result = text(name, value, maximum);
  if (!/^[A-Za-z0-9_.:/-]+$/.test(result)) fail(`${name} contains unsupported identifier characters`);
  return result;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bound`);
  return value;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function exactKeys(name: string, value: Record<string, unknown>, expected: readonly string[]): void {
  const allowed = new Set(expected);
  if (Object.keys(value).some((key) => !allowed.has(key)) || expected.some((key) => !(key in value))) fail(`${name} contains unsupported or missing fields`);
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 18) fail("metadata nesting exceeds its bound");
  if (value === null || typeof value === "string" || typeof value === "boolean" || typeof value === "number") {
    if (typeof value === "number" && !Number.isFinite(value)) fail("metadata contains a non-finite number");
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > 4_096) fail("metadata sequence exceeds its bound");
    value.forEach((child) => safeMetadata(child, depth + 1));
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 4_096) fail("metadata object exceeds its bound");
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.replace(/[_-]/g, "").toLowerCase();
      if (["task", "prompt", "credential", "credentials", "secret", "token", "password", "messages", "body", "headers", "response", "result"].includes(normalized)) fail("metadata contains transient or secret-shaped material");
      safeMetadata(child, depth + 1);
    }
    return;
  }
  fail("metadata contains an unsupported value");
}

function counts(name: string, value: unknown): JsonObject {
  if (!isObject(value) || Object.keys(value).length > 256) fail(`${name} is malformed`);
  const result: Record<string, number> = {};
  for (const [key, raw] of Object.entries(value)) result[identifier(`${name} key`, key, 128)] = integer(`${name} value`, raw, 0, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS);
  return Object.fromEntries(Object.entries(result).sort(([left], [right]) => left.localeCompare(right)));
}

function sequence(name: string, value: unknown, maximum: number): unknown[] {
  if (!Array.isArray(value) || value.length > maximum) fail(`${name} is outside its sequence bound`);
  return [...value];
}

function normalizePreview(value: unknown): JsonObject {
  if (isObject(value) && typeof value.toJSON === "function") value = value.toJSON();
  if (!isObject(value)) fail("preview projection must be an object");
  exactKeys("preview", value, PREVIEW_KEYS);
  if (value.schema !== "bioprism-autonomous-goal-control-preview/0.1") fail("preview schema is invalid");
  if (!["admissible_work", "all_terminal", "no_admissible_work"].includes(String(value.status))) fail("preview status is invalid");
  if (value.retention !== "metadata_only_goal_control_preview;tasks_prompts_parameters_credentials_and_results_not_retained" || value.secret_material !== "never_returned") fail("preview retention markers are invalid");
  integer("preview eligible_goal_count", value.eligible_goal_count, 0, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS);
  const decisionCounts = counts("preview decision_counts", value.decision_counts);
  const reasonCounts = counts("preview reason_counts", value.reason_counts);
  const statusCounts = counts("preview status_counts", value.status_counts);
  const blocked = sequence("preview dependency_blocked_goal_ids", value.dependency_blocked_goal_ids, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS).map((item) => identifier("preview dependency_blocked_goal_id", item));
  const learning = digest("preview learning_state_digest", value.learning_state_digest, true);
  if (!isObject(value.schedule)) fail("preview schedule is malformed");
  digest("preview schedule_digest", value.schedule.schedule_digest);
  sequence("preview selected_goal_ids", value.schedule.selected_goal_ids, 128).forEach((item) => identifier("preview selected_goal_id", item));
  if (!isObject(value.schedule.coverage)) fail("preview schedule coverage is malformed");
  for (const field of ["required_domains", "selected_domains", "missing_domains"] as const) sequence(`preview coverage ${field}`, value.schedule.coverage[field], 128).forEach((item) => identifier(`preview coverage ${field}`, item, 128));
  safeMetadata(value);
  const body: JsonObject = {
    schema: value.schema,
    schedule: clone(value.schedule) as unknown as JsonObject,
    status: value.status as string,
    eligible_goal_count: value.eligible_goal_count as number,
    decision_counts: decisionCounts,
    reason_counts: reasonCounts,
    status_counts: statusCounts,
    dependency_blocked_goal_ids: [...new Set(blocked)].sort(),
    learning_state_digest: learning,
    retention: value.retention,
    secret_material: value.secret_material,
  };
  const supplied = digest("preview_digest", value.preview_digest)!;
  if (supplied !== digestJsonSync(body)) fail("preview digest does not match its projection");
  return { ...body, preview_digest: supplied };
}

function recordBody(input: {
  admissionId: unknown;
  revision: unknown;
  status: AutonomousGoalPreviewAdmissionStatus;
  decision: AutonomousGoalPreviewAdmissionDecision;
  preview: JsonObject;
  requestedByDigest: unknown;
  reviewerDigest: unknown;
  issuedAtNs: unknown;
  expiresAtNs: unknown;
  reasonDigest: unknown;
  previousRecordDigest: unknown;
}): Omit<AutonomousGoalPreviewAdmissionRecord, "record_digest"> {
  const issued = integer("issued_at_ns", input.issuedAtNs, 0, Number.MAX_SAFE_INTEGER);
  const expires = integer("expires_at_ns", input.expiresAtNs, 1, Number.MAX_SAFE_INTEGER);
  if (expires <= issued || expires - issued > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_TTL_NS) fail("approval expiry is outside its bounded lifetime");
  const requested = digest("requested_by_digest", input.requestedByDigest, true);
  const reviewer = digest("reviewer_digest", input.reviewerDigest, true);
  if (input.status === "pending_review" && reviewer !== null) fail("pending review cannot contain a reviewer");
  if (input.status !== "pending_review" && reviewer === null) fail("reviewed approval records require a reviewer digest");
  return {
    schema: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA,
    admission_id: identifier("admission_id", input.admissionId),
    revision: integer("revision", input.revision, 1, 2_147_483_647),
    status: input.status,
    decision: input.decision,
    preview: clone(input.preview),
    preview_digest: digest("preview_digest", input.preview.preview_digest)!,
    requested_by_digest: requested,
    reviewer_digest: reviewer,
    issued_at_ns: issued,
    expires_at_ns: expires,
    reason_digest: digest("reason_digest", input.reasonDigest, true),
    previous_record_digest: digest("previous_record_digest", input.previousRecordDigest, true),
    authority: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY,
    retention: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
    execution: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION,
    secret_material: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
  };
}

export function validateAutonomousGoalPreviewAdmissionRecord(value: unknown): AutonomousGoalPreviewAdmissionRecord {
  if (!isObject(value)) fail("record must be an object");
  exactKeys("record", value, [...RECORD_KEYS, "record_digest"]);
  safeMetadata(value);
  if (value.schema !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA || value.authority !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY || value.retention !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION || value.execution !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION || value.secret_material !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL) fail("record markers are invalid");
  const preview = normalizePreview(value.preview);
  if (value.preview_digest !== preview.preview_digest) fail("record preview digest does not match the preview");
  const status = value.status as AutonomousGoalPreviewAdmissionStatus;
  const decision = value.decision as AutonomousGoalPreviewAdmissionDecision;
  if (!["pending_review", "approved", "rejected"].includes(status) || !["submitted", "approved", "rejected"].includes(decision) || (status === "pending_review" && decision !== "submitted") || (status === "approved" && decision !== "approved") || (status === "rejected" && decision !== "rejected")) fail("record status or decision is invalid");
  const body = recordBody({ admissionId: value.admission_id, revision: value.revision, status, decision, preview, requestedByDigest: value.requested_by_digest, reviewerDigest: value.reviewer_digest, issuedAtNs: value.issued_at_ns, expiresAtNs: value.expires_at_ns, reasonDigest: value.reason_digest, previousRecordDigest: value.previous_record_digest });
  const supplied = digest("record_digest", value.record_digest)!;
  if (supplied !== digestJsonSync(body)) fail("record digest does not match metadata");
  return clone({ ...body, record_digest: supplied } as AutonomousGoalPreviewAdmissionRecord);
}

export function createAutonomousGoalPreviewAdmissionRecord(preview: AutonomousGoalControlLoopPreview | JsonObject, options: AutonomousGoalPreviewAdmissionRecordCreateOptions): AutonomousGoalPreviewAdmissionRecord {
  if (!options || typeof options !== "object") fail("record options are malformed");
  const normalizedPreview = normalizePreview(preview);
  const reasonDigest = options.reason === undefined || options.reason === null ? null : digestJsonSync(text("reason", options.reason, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES));
  const body = recordBody({ admissionId: options.admission_id, revision: 1, status: "pending_review", decision: "submitted", preview: normalizedPreview, requestedByDigest: options.requested_by_digest ?? null, reviewerDigest: null, issuedAtNs: options.issued_at_ns, expiresAtNs: options.expires_at_ns, reasonDigest, previousRecordDigest: options.previous_record_digest ?? null });
  return clone({ ...body, record_digest: digestJsonSync(body) } as AutonomousGoalPreviewAdmissionRecord);
}

export function reviewAutonomousGoalPreviewAdmissionRecord(source: AutonomousGoalPreviewAdmissionRecord, options: AutonomousGoalPreviewAdmissionReviewOptions): AutonomousGoalPreviewAdmissionRecord {
  const current = validateAutonomousGoalPreviewAdmissionRecord(source);
  if (current.status !== "pending_review") fail("only a pending preview admission can be reviewed");
  if (!options || typeof options !== "object") fail("review options are malformed");
  if (typeof options.approved !== "boolean") fail("approved must be boolean");
  if (options.expected_record_digest !== undefined && options.expected_record_digest !== null && options.expected_record_digest !== current.record_digest) fail("review expected_record_digest does not match the current record");
  const decision = options.approved ? "approved" : "rejected" as const;
  const reasonDigest = options.reason === undefined || options.reason === null ? null : digestJsonSync(text("reason", options.reason, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES));
  const body = recordBody({ admissionId: current.admission_id, revision: current.revision + 1, status: decision, decision, preview: current.preview, requestedByDigest: current.requested_by_digest, reviewerDigest: options.reviewer_digest, issuedAtNs: current.issued_at_ns, expiresAtNs: current.expires_at_ns, reasonDigest, previousRecordDigest: current.record_digest });
  return clone({ ...body, record_digest: digestJsonSync(body) } as AutonomousGoalPreviewAdmissionRecord);
}

export function verifyAutonomousGoalPreviewApproval(source: AutonomousGoalPreviewAdmissionRecord, options: { current_preview_digest: string; now_ns: number; reviewer_digest?: string | null }): AutonomousGoalPreviewAdmissionRecord {
  const record = validateAutonomousGoalPreviewAdmissionRecord(source);
  if (record.status !== "approved") fail("preview admission is not approved");
  const currentDigest = digest("current_preview_digest", options.current_preview_digest)!;
  const now = integer("now_ns", options.now_ns, 0, Number.MAX_SAFE_INTEGER);
  if (now >= record.expires_at_ns) fail("preview admission has expired");
  if (record.preview_digest !== currentDigest) fail("preview admission does not match the current preview");
  if (options.reviewer_digest !== undefined && options.reviewer_digest !== null && record.reviewer_digest !== digest("reviewer_digest", options.reviewer_digest)) fail("preview admission reviewer does not match");
  return record;
}

export function validateAutonomousGoalPreviewAdmissionSnapshot(value: unknown): AutonomousGoalPreviewAdmissionSnapshot {
  if (!isObject(value)) fail("snapshot must be an object");
  exactKeys("snapshot", value, [...SNAPSHOT_KEYS, "snapshot_digest"]);
  safeMetadata(value);
  if (value.schema !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA || value.retention !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION || value.secret_material !== AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL) fail("snapshot markers are invalid");
  const records = value.records;
  if (!Array.isArray(records) || records.length > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS) fail("snapshot records exceed their bound");
  const normalized = records.map((record) => validateAutonomousGoalPreviewAdmissionRecord(record));
  const ids = new Set<string>();
  for (const record of normalized) {
    if (ids.has(record.admission_id)) fail("snapshot contains duplicate admission ids");
    ids.add(record.admission_id);
  }
  const body = {
    schema: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA,
    generation: integer("snapshot generation", value.generation, 0, 2_147_483_647),
    records: normalized.sort((left, right) => left.admission_id.localeCompare(right.admission_id)),
    previous_snapshot_digest: digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest, true),
    retention: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
    secret_material: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
  } satisfies Omit<AutonomousGoalPreviewAdmissionSnapshot, "snapshot_digest">;
  const supplied = digest("snapshot_digest", value.snapshot_digest)!;
  if (supplied !== digestJsonSync(body)) fail("snapshot digest does not match metadata");
  const result = clone({ ...body, snapshot_digest: supplied });
  if (bytes(result) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return result;
}

export function sealAutonomousGoalPreviewAdmissionSnapshot(input: { generation: number; records: readonly AutonomousGoalPreviewAdmissionRecord[]; previous_snapshot_digest?: string | null }): AutonomousGoalPreviewAdmissionSnapshot {
  if (!input || typeof input !== "object") fail("snapshot input is malformed");
  const body = {
    schema: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA,
    generation: integer("snapshot generation", input.generation, 0, 2_147_483_647),
    records: [...input.records].map((record) => validateAutonomousGoalPreviewAdmissionRecord(record)).sort((left, right) => left.admission_id.localeCompare(right.admission_id)),
    previous_snapshot_digest: digest("snapshot previous_snapshot_digest", input.previous_snapshot_digest ?? null, true),
    retention: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
    secret_material: AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
  } satisfies Omit<AutonomousGoalPreviewAdmissionSnapshot, "snapshot_digest">;
  if (body.records.length > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS) fail("snapshot records exceed their bound");
  const result = clone({ ...body, snapshot_digest: digestJsonSync(body) });
  if (bytes(result) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return result;
}

export class InMemoryAutonomousGoalPreviewAdmissionLedger {
  readonly max_records: number;
  private readonly records = new Map<string, AutonomousGoalPreviewAdmissionRecord>();

  constructor(options: { max_records?: number } = {}) {
    this.max_records = integer("max_records", options.max_records ?? MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS, 1, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS);
  }

  put(source: AutonomousGoalPreviewAdmissionRecord): AutonomousGoalPreviewAdmissionRecord {
    const record = validateAutonomousGoalPreviewAdmissionRecord(source);
    const existing = this.records.get(record.admission_id);
    if (existing !== undefined && existing.record_digest !== record.previous_record_digest && existing.record_digest !== record.record_digest) fail("record predecessor conflicts with the current admission");
    if (existing === undefined && record.revision !== 1) fail("new preview admissions must begin at revision one");
    if (existing !== undefined && record.revision !== existing.revision + 1 && record.record_digest !== existing.record_digest) fail("preview admission revision is not contiguous");
    if (existing === undefined && this.records.size >= this.max_records) fail("ledger capacity is exhausted");
    this.records.set(record.admission_id, record);
    return clone(record);
  }

  submit(preview: AutonomousGoalControlLoopPreview | JsonObject, options: AutonomousGoalPreviewAdmissionRecordCreateOptions): AutonomousGoalPreviewAdmissionRecord {
    return this.put(createAutonomousGoalPreviewAdmissionRecord(preview, options));
  }

  review(admissionId: string, options: AutonomousGoalPreviewAdmissionReviewOptions): AutonomousGoalPreviewAdmissionRecord {
    const current = this.get(admissionId);
    if (current === null) fail("cannot review an unknown preview admission");
    return this.put(reviewAutonomousGoalPreviewAdmissionRecord(current, options));
  }

  get(admissionId: string): AutonomousGoalPreviewAdmissionRecord | null {
    const value = this.records.get(identifier("admission_id", admissionId));
    return value === undefined ? null : clone(value);
  }

  list(): AutonomousGoalPreviewAdmissionRecord[] {
    return [...this.records.values()].sort((left, right) => left.admission_id.localeCompare(right.admission_id)).map(clone);
  }

  restore(snapshot: AutonomousGoalPreviewAdmissionSnapshot): void {
    const normalized = validateAutonomousGoalPreviewAdmissionSnapshot(snapshot);
    if (normalized.records.length > this.max_records) fail("snapshot exceeds ledger capacity");
    this.records.clear();
    for (const record of normalized.records) this.records.set(record.admission_id, record);
  }
}

export class JsonAutonomousGoalPreviewAdmissionSnapshotPersistence implements AutonomousGoalPreviewAdmissionSnapshotPersistence {
  readonly store: AutonomousGoalPreviewAdmissionSnapshotTextStore;
  readonly max_bytes: number;

  constructor(store: AutonomousGoalPreviewAdmissionSnapshotTextStore, maxBytes = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) {
    if (typeof store?.read !== "function" || typeof store?.write !== "function") fail("JSON persistence requires a text store");
    this.store = store;
    this.max_bytes = integer("max_bytes", maxBytes, 1, MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES);
  }

  async read(): Promise<AutonomousGoalPreviewAdmissionSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > this.max_bytes) fail("stored JSON exceeds its byte bound");
    let raw: unknown;
    try { raw = JSON.parse(encoded); } catch { fail("stored JSON is invalid"); }
    const normalized = validateAutonomousGoalPreviewAdmissionSnapshot(raw);
    if (canonicalJson(normalized) !== encoded) fail("stored JSON is not canonical");
    return normalized;
  }

  async write(snapshot: AutonomousGoalPreviewAdmissionSnapshot): Promise<void> {
    const normalized = validateAutonomousGoalPreviewAdmissionSnapshot(snapshot);
    const encoded = canonicalJson(normalized);
    if (new TextEncoder().encode(encoded).byteLength > this.max_bytes) fail("snapshot exceeds the configured byte bound");
    await this.store.write(encoded);
  }
}

export class TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence extends JsonAutonomousGoalPreviewAdmissionSnapshotPersistence implements TransactionalAutonomousGoalPreviewAdmissionSnapshotPersistence {
  override readonly store: TransactionalAutonomousGoalPreviewAdmissionSnapshotTextStore;

  constructor(store: TransactionalAutonomousGoalPreviewAdmissionSnapshotTextStore, maxBytes = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) {
    super(store, maxBytes);
    if (typeof store.writeIfUnchanged !== "function") fail("transactional JSON persistence requires writeIfUnchanged");
    this.store = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalPreviewAdmissionSnapshot): Promise<boolean> {
    digest("expectedSnapshotDigest", expectedSnapshotDigest, true);
    return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validateAutonomousGoalPreviewAdmissionSnapshot(snapshot)));
  }
}

export class AutonomousGoalPreviewAdmissionPersistenceCoordinator {
  private expectedSnapshotDigestValue: string | null = null;
  private expectedGeneration = 0;
  readonly ledger: InMemoryAutonomousGoalPreviewAdmissionLedger;
  readonly persistence: AutonomousGoalPreviewAdmissionSnapshotPersistence;

  constructor(ledger: InMemoryAutonomousGoalPreviewAdmissionLedger, persistence: AutonomousGoalPreviewAdmissionSnapshotPersistence) {
    if (!(ledger instanceof InMemoryAutonomousGoalPreviewAdmissionLedger)) fail("coordinator requires a typed ledger");
    if (typeof persistence?.read !== "function" || typeof persistence?.write !== "function") fail("coordinator persistence is malformed");
    this.ledger = ledger;
    this.persistence = persistence;
  }

  get expected_snapshot_digest(): string | null { return this.expectedSnapshotDigestValue; }

  async restore(): Promise<AutonomousGoalPreviewAdmissionSnapshot | null> {
    const raw = await this.persistence.read();
    if (raw === null) {
      this.expectedSnapshotDigestValue = null;
      this.expectedGeneration = 0;
      return null;
    }
    const snapshot = validateAutonomousGoalPreviewAdmissionSnapshot(raw);
    this.ledger.restore(snapshot);
    this.expectedSnapshotDigestValue = snapshot.snapshot_digest;
    this.expectedGeneration = snapshot.generation;
    return snapshot;
  }

  async flush(): Promise<AutonomousGoalPreviewAdmissionSnapshot> {
    const snapshot = sealAutonomousGoalPreviewAdmissionSnapshot({ generation: this.expectedGeneration + 1, records: this.ledger.list(), previous_snapshot_digest: this.expectedSnapshotDigestValue });
    const transactional = this.persistence as Partial<TransactionalAutonomousGoalPreviewAdmissionSnapshotPersistence>;
    if (typeof transactional.writeIfUnchanged === "function") {
      if (!await transactional.writeIfUnchanged(this.expectedSnapshotDigestValue, snapshot)) fail("persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigestValue = snapshot.snapshot_digest;
    this.expectedGeneration = snapshot.generation;
    return snapshot;
  }
}
