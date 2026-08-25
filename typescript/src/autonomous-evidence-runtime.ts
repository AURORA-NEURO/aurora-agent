import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AutonomousEvidencePlan,
  type AutonomousEvidencePlanJSON,
  type AutonomousEvidenceRequirement,
} from "./autonomous-evidence.js";
import { AutonomousProtectedRehydrationAdapter } from "./autonomous-protected-rehydration.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Runtime boundary for caller-owned acquisition, projection, and evaluation. */
export const AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA = "bioprism-typescript-autonomous-evidence-runtime/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-evidence-receipt/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA = "bioprism-typescript-autonomous-evidence-assessment/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-evidence-runtime-journal/0.1" as const;
const LEGACY_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-evidence-runtime-snapshot/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-evidence-runtime-snapshot/0.2" as const;
export const MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS = 128;
export const MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS = 4_096;
export const MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES = 64_000;
export const MAX_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_BYTES = 512_000;

export type AutonomousEvidenceRuntimeStatus = "completed" | "partial" | "awaiting_evaluation" | "failed" | "reconciliation_required";
export type AutonomousEvidenceAcquisitionStatus = "observed" | "partial" | "failed" | "reconciliation_required";
export type AutonomousEvidenceEvaluatorStatus = "not_evaluated" | "accepted" | "rejected" | "indeterminate" | "failed";
export type AutonomousEvidenceVerdict = "accepted" | "rejected" | "indeterminate";

const SECRET_KEYS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "privatekey", "refreshtoken"]);

function bytes(value: string): number { return new TextEncoder().encode(value).byteLength; }

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+\- /]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digestOrNull(name: string, value: unknown, required = false): string | null {
  if (value === undefined || value === null) {
    if (required) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    return null;
  }
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded array`);
  const result = value.map((item, index) => boundedIdentifier(`${name}[${index}]`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return result;
}

function assertMetadata(value: unknown, name: string, depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((item, index) => assertMetadata(item, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      if (SECRET_KEYS.has(key.toLowerCase().replace(/[^a-z0-9]/g, ""))) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function jsonBytes(value: unknown, name: string): number {
  try {
    const encoded = canonicalJson(value);
    const size = bytes(encoded);
    if (size > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES) throw new ArgumentError(`${name} exceeds its metadata byte bound`);
    return size;
  } catch (error) {
    if (error instanceof ArgumentError) throw error;
    throw new ArgumentError(`${name} must be JSON-safe`);
  }
}

export interface AutonomousEvidenceAcquisitionRequest extends JsonObject {
  requirement_id: string;
  source_id: string;
  source_digest?: string | null;
  request_id?: string | null;
  metadata?: JsonObject;
}

export interface AutonomousEvidenceAcquisitionContext extends JsonObject {
  plan_digest: string;
  requirement: AutonomousEvidenceRequirement;
  request: AutonomousEvidenceAcquisitionRequest;
  attempt: number;
  parent_evidence_digests: string[];
  execution: "caller_owned_adapter;raw_value_transient";
}

export interface AutonomousEvidenceObservationInput extends JsonObject {
  label: string;
  kind?: "fact" | "measurement" | "provenance" | "limitation" | "warning";
  status?: "observed" | "inferred" | "missing";
  value_digest?: string | null;
  source_digest?: string | null;
  confidence?: number | null;
  limitations?: string[];
}

export interface AutonomousEvidenceObservation extends JsonObject {
  schema: "bioprism-typescript-autonomous-evidence-observation/0.1";
  label: string;
  kind: "fact" | "measurement" | "provenance" | "limitation" | "warning";
  status: "observed" | "inferred" | "missing";
  value_digest: string | null;
  source_digest: string | null;
  confidence: number | null;
  limitations: string[];
}

export interface AutonomousEvidenceAcquirer {
  acquire(context: AutonomousEvidenceAcquisitionContext): JsonValue | Promise<JsonValue>;
}

export interface AutonomousEvidenceProjector {
  project(value: JsonValue, context: AutonomousEvidenceAcquisitionContext): readonly AutonomousEvidenceObservationInput[] | Promise<readonly AutonomousEvidenceObservationInput[]>;
}

export interface AutonomousEvidenceEvaluationInput extends JsonObject {
  requirement: AutonomousEvidenceRequirement;
  receipt: AutonomousEvidenceReceiptJSON;
  observations: AutonomousEvidenceObservation[];
  value: JsonValue;
}

export interface AutonomousEvidenceEvaluatorAssessmentInput extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  verdict: AutonomousEvidenceVerdict;
  score: number;
  feedback_digest?: string | null;
  evidence_digest?: string | null;
  failure_class?: string | null;
}

export interface AutonomousEvidenceEvaluator {
  evaluator_id: string;
  evaluator_version: string;
  evaluate(input: AutonomousEvidenceEvaluationInput): AutonomousEvidenceEvaluatorAssessmentInput | Promise<AutonomousEvidenceEvaluatorAssessmentInput>;
}

export interface AutonomousEvidenceReceiptJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA;
  request_digest: string;
  plan_digest: string;
  requirement_id: string;
  domain: string;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  source_id: string;
  source_digest: string | null;
  attempt: number;
  status: AutonomousEvidenceAcquisitionStatus;
  replay: "fresh" | "replayed";
  value_digest: string | null;
  value_bytes: number;
  observations: AutonomousEvidenceObservation[];
  observed_requirement_ids: string[];
  missing_requirement_ids: string[];
  evidence_status: "not_evaluated" | "missing_required_outputs" | "declared_for_evaluator" | "projection_failed";
  evaluator_status: AutonomousEvidenceEvaluatorStatus;
  assessment_digest: string | null;
  limitations: string[];
  error_class: string | null;
  duration_ms: number;
  receipt_digest: string;
  retention: "metadata_only;raw_acquisition_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAssessmentJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA;
  receipt_digest: string;
  requirement_id: string;
  evaluator_id: string;
  evaluator_version: string;
  verdict: AutonomousEvidenceVerdict;
  score: number;
  feedback_digest: string | null;
  evidence_digest: string | null;
  failure_class: string | null;
  assessment_digest: string;
  retention: "value_only;evaluator_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRuntimeJournalEntry extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA;
  sequence: number;
  previous_entry_digest: string | null;
  receipt: AutonomousEvidenceReceiptJSON;
  assessment: AutonomousEvidenceAssessmentJSON | null;
  entry_digest: string;
  retention: "metadata_only;raw_acquisition_and_evaluator_values_excluded";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRuntimeSnapshot extends JsonObject {
  /** 0.1 remains readable; current snapshots carry independent image lineage in 0.2. */
  schema: typeof AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA | typeof LEGACY_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA;
  snapshot_generation?: number;
  previous_snapshot_digest?: string | null;
  plan_digest: string;
  entries: AutonomousEvidenceRuntimeJournalEntry[];
  head_digest: string | null;
  snapshot_digest: string;
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRuntimeJournal {
  /** Append-only journals may contain later assessment revisions for one request digest. */
  append(entry: AutonomousEvidenceRuntimeJournalEntry): Promise<AutonomousEvidenceRuntimeJournalEntry> | AutonomousEvidenceRuntimeJournalEntry;
  records(): Promise<readonly AutonomousEvidenceRuntimeJournalEntry[]> | readonly AutonomousEvidenceRuntimeJournalEntry[];
}

export interface AutonomousEvidenceRuntimeExecuteOptions {
  acquirer: AutonomousEvidenceAcquirer;
  projector?: AutonomousEvidenceProjector;
  evaluator?: AutonomousEvidenceEvaluator;
  rehydrateValue?: (receipt: AutonomousEvidenceReceiptJSON) => JsonValue | null | Promise<JsonValue | null>;
  parentEvidenceDigests?: readonly string[];
  stopOnFailure?: boolean;
  /** Re-run an evaluator for a journaled observed receipt whose prior verdict is unresolved. */
  reevaluatePending?: boolean;
}

export interface AutonomousEvidenceRuntimeResultJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA;
  status: AutonomousEvidenceRuntimeStatus;
  plan: AutonomousEvidencePlanJSON;
  receipts: AutonomousEvidenceReceiptJSON[];
  assessments: AutonomousEvidenceAssessmentJSON[];
  completed_requirement_ids: string[];
  pending_evaluation_requirement_ids: string[];
  missing_requirement_ids: string[];
  next_stage_ids: string[];
  omitted_request_digests: string[];
  result_digest: string;
  retention: "metadata_only;raw_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRuntimeResult {
  readonly json: AutonomousEvidenceRuntimeResultJSON;
  readonly values: Readonly<Record<string, JsonValue | null>>;
  toJSON(): AutonomousEvidenceRuntimeResultJSON;
}

function requirementFor(plan: AutonomousEvidencePlan, requirementId: string): AutonomousEvidenceRequirement {
  const normalized = boundedIdentifier("evidence runtime requirement_id", requirementId);
  const requirement = plan.requirements.find((item) => item.requirement_id === normalized);
  if (!requirement) throw new ArgumentError(`evidence runtime requirement is not in the plan: ${normalized}`);
  return requirement;
}

function normalizeRequest(value: AutonomousEvidenceAcquisitionRequest): AutonomousEvidenceAcquisitionRequest {
  if (!isObject(value)) throw new ArgumentError("evidence runtime acquisition request must be an object");
  const requirementId = boundedIdentifier("evidence runtime requirement_id", value.requirement_id);
  const sourceId = boundedIdentifier("evidence runtime source_id", value.source_id);
  const sourceDigest = digestOrNull("evidence runtime source_digest", value.source_digest);
  const requestId = value.request_id === undefined || value.request_id === null ? null : boundedIdentifier("evidence runtime request_id", value.request_id);
  const metadata = value.metadata === undefined ? {} : value.metadata;
  if (!isObject(metadata)) throw new ArgumentError("evidence runtime request metadata must be an object");
  assertMetadata(metadata, "evidence runtime request metadata");
  jsonBytes(metadata, "evidence runtime request metadata");
  return { requirement_id: requirementId, source_id: sourceId, source_digest: sourceDigest, request_id: requestId, metadata: structuredClone(metadata) as JsonObject };
}

function normalizeObservation(value: AutonomousEvidenceObservationInput, index: number): AutonomousEvidenceObservation {
  if (!isObject(value)) throw new ArgumentError(`evidence runtime observation ${index} must be an object`);
  const label = boundedIdentifier(`evidence runtime observation ${index}.label`, value.label);
  const kind = value.kind ?? "fact";
  const status = value.status ?? "observed";
  if (!["fact", "measurement", "provenance", "limitation", "warning"].includes(kind)) throw new ArgumentError(`evidence runtime observation ${index}.kind is invalid`);
  if (!["observed", "inferred", "missing"].includes(status)) throw new ArgumentError(`evidence runtime observation ${index}.status is invalid`);
  const confidence = value.confidence === undefined || value.confidence === null ? null : value.confidence;
  if (confidence !== null && (typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1)) throw new ArgumentError(`evidence runtime observation ${index}.confidence is invalid`);
  const limitations = value.limitations === undefined ? [] : boundedList(`evidence runtime observation ${index}.limitations`, value.limitations, 32);
  return {
    schema: "bioprism-typescript-autonomous-evidence-observation/0.1",
    label,
    kind: kind as AutonomousEvidenceObservation["kind"],
    status: status as AutonomousEvidenceObservation["status"],
    value_digest: digestOrNull(`evidence runtime observation ${index}.value_digest`, value.value_digest),
    source_digest: digestOrNull(`evidence runtime observation ${index}.source_digest`, value.source_digest),
    confidence,
    limitations,
  };
}

function receiptDescriptor(receipt: AutonomousEvidenceReceiptJSON): JsonObject {
  const { receipt_digest: _receiptDigest, ...descriptor } = receipt;
  return descriptor;
}

async function validateReceipt(value: unknown): Promise<AutonomousEvidenceReceiptJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA) throw new ArgumentError("evidence runtime receipt is malformed");
  const receipt = value as unknown as AutonomousEvidenceReceiptJSON;
  if (!/^[0-9a-f]{64}$/.test(String(receipt.request_digest)) || !/^[0-9a-f]{64}$/.test(String(receipt.plan_digest)) || !/^[0-9a-f]{64}$/.test(String(receipt.workflow_digest)) || !/^[0-9a-f]{64}$/.test(String(receipt.receipt_digest))) throw new ArgumentError("evidence runtime receipt digest fields are invalid");
  if (!(["observed", "partial", "failed", "reconciliation_required"] as string[]).includes(receipt.status) || !(["fresh", "replayed"] as string[]).includes(receipt.replay)) throw new ArgumentError("evidence runtime receipt status is invalid");
  boundedIdentifier("evidence runtime receipt requirement_id", receipt.requirement_id);
  boundedIdentifier("evidence runtime receipt source_id", receipt.source_id);
  if (!Array.isArray(receipt.observations) || !Array.isArray(receipt.observed_requirement_ids) || !Array.isArray(receipt.missing_requirement_ids) || !Array.isArray(receipt.limitations)) throw new ArgumentError("evidence runtime receipt arrays are invalid");
  if (receipt.retention !== "metadata_only;raw_acquisition_values_caller_owned" || receipt.secret_material !== "never_returned") throw new ArgumentError("evidence runtime receipt retention is invalid");
  if (await digestJson(receiptDescriptor(receipt)) !== receipt.receipt_digest) throw new ArgumentError("evidence runtime receipt digest is invalid");
  return structuredClone(receipt);
}

function assessmentDescriptor(assessment: AutonomousEvidenceAssessmentJSON): JsonObject {
  const { assessment_digest: _assessmentDigest, ...descriptor } = assessment;
  return descriptor;
}

async function validateAssessment(value: unknown): Promise<AutonomousEvidenceAssessmentJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA) throw new ArgumentError("evidence runtime assessment is malformed");
  const assessment = value as unknown as AutonomousEvidenceAssessmentJSON;
  boundedIdentifier("evidence runtime assessment evaluator_id", assessment.evaluator_id);
  boundedIdentifier("evidence runtime assessment evaluator_version", assessment.evaluator_version);
  if (!["accepted", "rejected", "indeterminate"].includes(assessment.verdict) || typeof assessment.score !== "number" || !Number.isFinite(assessment.score) || assessment.score < 0 || assessment.score > 1) throw new ArgumentError("evidence runtime assessment verdict or score is invalid");
  if (assessment.retention !== "value_only;evaluator_payloads_caller_owned" || assessment.secret_material !== "never_returned") throw new ArgumentError("evidence runtime assessment retention is invalid");
  if (await digestJson(assessmentDescriptor(assessment)) !== assessment.assessment_digest) throw new ArgumentError("evidence runtime assessment digest is invalid");
  return structuredClone(assessment);
}

function journalDescriptor(entry: AutonomousEvidenceRuntimeJournalEntry): JsonObject {
  const { entry_digest: _entryDigest, ...descriptor } = entry;
  return descriptor;
}

export class InMemoryAutonomousEvidenceRuntimeJournal implements AutonomousEvidenceRuntimeJournal {
  private entries: AutonomousEvidenceRuntimeJournalEntry[] = [];
  private snapshotGeneration = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: AutonomousEvidenceRuntimeSnapshot | null = null;
  private cachedEntrySignature: string | null = null;

  async append(entry: AutonomousEvidenceRuntimeJournalEntry): Promise<AutonomousEvidenceRuntimeJournalEntry> {
    const validatedReceipt = await validateReceipt(entry.receipt);
    const validatedAssessment = entry.assessment === null ? null : await validateAssessment(entry.assessment);
    const normalized = structuredClone({ ...entry, receipt: validatedReceipt, assessment: validatedAssessment });
    const prior = this.entries.find((candidate) => candidate.receipt.request_digest === validatedReceipt.request_digest);
    if (prior && await digestJson(prior) === await digestJson(normalized)) return structuredClone(prior);
    if (normalized.sequence !== this.entries.length + 1 || normalized.previous_entry_digest !== (this.entries.at(-1)?.entry_digest ?? null)) throw new ArgumentError("evidence runtime journal chain position is invalid");
    if (await digestJson(journalDescriptor(normalized)) !== normalized.entry_digest) throw new ArgumentError("evidence runtime journal entry digest is invalid");
    if (this.entries.length >= MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS) throw new ArgumentError("evidence runtime journal capacity is exhausted");
    this.entries.push(normalized);
    this.cachedSnapshot = null;
    this.cachedEntrySignature = null;
    return structuredClone(normalized);
  }

  records(): readonly AutonomousEvidenceRuntimeJournalEntry[] { return this.entries.map((entry) => structuredClone(entry)); }

  async snapshot(planDigest: string): Promise<AutonomousEvidenceRuntimeSnapshot> {
    const plan = digestOrNull("evidence runtime snapshot plan_digest", planDigest, true)!;
    const signature = this.entries.map((entry) => entry.entry_digest).join(":");
    if (this.cachedSnapshot !== null && this.cachedEntrySignature === signature && this.cachedSnapshot.plan_digest === plan) return structuredClone(this.cachedSnapshot);
    const descriptor = {
      schema: AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
      snapshot_generation: this.snapshotGeneration + 1,
      previous_snapshot_digest: this.snapshotGeneration === 0 ? null : this.previousSnapshotDigest,
      plan_digest: plan,
      entries: this.records(),
      head_digest: this.entries.at(-1)?.entry_digest ?? null,
      retention: "metadata_only_hash_bound" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...descriptor, snapshot_digest: await digestJson(descriptor) } as AutonomousEvidenceRuntimeSnapshot;
    if (bytes(canonicalJson(snapshot)) > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_BYTES) throw new ArgumentError("evidence runtime snapshot exceeds its byte bound");
    this.snapshotGeneration = snapshot.snapshot_generation!;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    this.cachedSnapshot = structuredClone(snapshot);
    this.cachedEntrySignature = signature;
    return structuredClone(snapshot);
  }

  async restore(snapshot: AutonomousEvidenceRuntimeSnapshot, planDigest: string): Promise<void> {
    if (!isObject(snapshot)) throw new ArgumentError("evidence runtime snapshot metadata is invalid");
    const snapshotValue = snapshot as unknown as JsonObject;
    const legacy = snapshotValue.schema === LEGACY_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA;
    if (snapshotValue.schema !== AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA && !legacy || snapshotValue.plan_digest !== planDigest || snapshotValue.retention !== "metadata_only_hash_bound" || snapshotValue.secret_material !== "never_returned") throw new ArgumentError("evidence runtime snapshot metadata is invalid");
    const allowedKeys = legacy
      ? ["schema", "plan_digest", "entries", "head_digest", "snapshot_digest", "retention", "secret_material"]
      : ["schema", "snapshot_generation", "previous_snapshot_digest", "plan_digest", "entries", "head_digest", "snapshot_digest", "retention", "secret_material"];
    if (Object.keys(snapshotValue).some((key) => !allowedKeys.includes(key))) throw new ArgumentError("evidence runtime snapshot contains unsupported metadata");
    if (!legacy) {
      if (!Number.isSafeInteger(snapshotValue.snapshot_generation) || (snapshotValue.snapshot_generation as number) < 1) throw new ArgumentError("evidence runtime snapshot generation is outside its bounds");
      if (snapshotValue.previous_snapshot_digest !== null && (typeof snapshotValue.previous_snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(snapshotValue.previous_snapshot_digest))) throw new ArgumentError("evidence runtime previous_snapshot_digest is malformed");
      if (((snapshotValue.snapshot_generation as number) === 1) !== (snapshotValue.previous_snapshot_digest === null)) throw new ArgumentError("evidence runtime snapshot generation and previous_snapshot_digest are inconsistent");
    }
    if (!Array.isArray(snapshot.entries) || snapshot.entries.length > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS) throw new ArgumentError("evidence runtime snapshot entries are invalid");
    const descriptor = legacy
      ? { schema: LEGACY_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA, plan_digest: snapshot.plan_digest, entries: snapshot.entries, head_digest: snapshot.head_digest, retention: snapshot.retention, secret_material: snapshot.secret_material }
      : { schema: AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA, snapshot_generation: snapshot.snapshot_generation as number, previous_snapshot_digest: snapshot.previous_snapshot_digest as string | null, plan_digest: snapshot.plan_digest, entries: snapshot.entries, head_digest: snapshot.head_digest, retention: snapshot.retention, secret_material: snapshot.secret_material };
    if (await digestJson(descriptor) !== snapshot.snapshot_digest) throw new ArgumentError("evidence runtime snapshot digest is invalid");
    const restored: AutonomousEvidenceRuntimeJournalEntry[] = [];
    for (const raw of snapshot.entries) {
      const receipt = await validateReceipt(raw.receipt);
      const assessment = raw.assessment === null ? null : await validateAssessment(raw.assessment);
      const entry = structuredClone({ ...raw, receipt, assessment }) as AutonomousEvidenceRuntimeJournalEntry;
      if (entry.sequence !== restored.length + 1 || entry.previous_entry_digest !== (restored.at(-1)?.entry_digest ?? null) || await digestJson(journalDescriptor(entry)) !== entry.entry_digest) throw new ArgumentError("evidence runtime snapshot journal chain is invalid");
      restored.push(entry);
    }
    if (snapshot.head_digest !== (restored.at(-1)?.entry_digest ?? null)) throw new ArgumentError("evidence runtime snapshot head digest is invalid");
    this.entries = restored;
    this.snapshotGeneration = legacy ? 0 : snapshot.snapshot_generation!;
    this.previousSnapshotDigest = legacy ? null : snapshot.snapshot_digest;
    this.cachedSnapshot = legacy ? null : structuredClone(snapshot);
    this.cachedEntrySignature = legacy ? null : this.entries.map((entry) => entry.entry_digest).join(":");
  }
}

function makeResult(json: AutonomousEvidenceRuntimeResultJSON, values: Record<string, JsonValue | null>): AutonomousEvidenceRuntimeResult {
  return { json, values: Object.freeze({ ...values }), toJSON: () => structuredClone(json) };
}

/** Execute evidence acquisition through an explicit caller-owned adapter boundary. */
export class AutonomousEvidenceRuntime {
  readonly plan: AutonomousEvidencePlan;
  readonly journal: AutonomousEvidenceRuntimeJournal | null;
  readonly protectedRehydration: AutonomousProtectedRehydrationAdapter | null;
  private readonly recordsByRequest = new Map<string, AutonomousEvidenceRuntimeJournalEntry>();
  private readonly valuesByRequest = new Map<string, JsonValue>();

  constructor(options: { plan: AutonomousEvidencePlan; journal?: AutonomousEvidenceRuntimeJournal; protectedRehydration?: AutonomousProtectedRehydrationAdapter }) {
    if (!(options?.plan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence runtime requires an AutonomousEvidencePlan");
    if (options.journal !== undefined && (!options.journal || typeof options.journal.append !== "function" || typeof options.journal.records !== "function")) throw new ArgumentError("evidence runtime journal is malformed");
    if (options.protectedRehydration !== undefined && !(options.protectedRehydration instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("evidence runtime protectedRehydration adapter is malformed");
    this.plan = options.plan;
    this.journal = options.journal ?? null;
    this.protectedRehydration = options.protectedRehydration ?? null;
  }

  async rehydrate(): Promise<{ restored: number; replayable: number; value_retention: "transient_caller_value_only" }> {
    if (!this.journal) return { restored: 0, replayable: 0, value_retention: "transient_caller_value_only" };
    const records = await this.journal.records();
    if (!Array.isArray(records) || records.length > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS) throw new ProviderRuntimeError("evidence runtime journal returned too many records");
    this.recordsByRequest.clear();
    this.valuesByRequest.clear();
    let previousEntryDigest: string | null = null;
    for (const [index, raw] of records.entries()) {
      if (!isObject(raw) || raw.schema !== AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA || raw.sequence !== index + 1 || raw.previous_entry_digest !== previousEntryDigest || typeof raw.entry_digest !== "string" || !/^[0-9a-f]{64}$/.test(raw.entry_digest) || raw.retention !== "metadata_only;raw_acquisition_and_evaluator_values_excluded" || raw.secret_material !== "never_returned") throw new ProviderRuntimeError("evidence runtime journal chain metadata is invalid");
      const receipt = await validateReceipt(raw.receipt);
      if (receipt.plan_digest !== this.plan.plan_digest) throw new ProviderRuntimeError("evidence runtime journal belongs to a different evidence plan");
      const assessment = raw.assessment === null ? null : await validateAssessment(raw.assessment);
      const entry = structuredClone({ ...raw, receipt, assessment }) as AutonomousEvidenceRuntimeJournalEntry;
      if (await digestJson(journalDescriptor(entry)) !== entry.entry_digest) throw new ProviderRuntimeError("evidence runtime journal entry digest is invalid");
      // Journals may contain later evaluator revisions for the same request. The chain is
      // validated above; the latest revision becomes the replay source for this runtime.
      this.recordsByRequest.set(receipt.request_digest, entry);
      previousEntryDigest = entry.entry_digest;
    }
    return { restored: records.length, replayable: records.length, value_retention: "transient_caller_value_only" };
  }

  private async requestDigest(request: AutonomousEvidenceAcquisitionRequest): Promise<string> {
    return digestJson({ schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA, plan_digest: this.plan.plan_digest, requirement_id: request.requirement_id, source_id: request.source_id, source_digest: request.source_digest ?? null, request_id: request.request_id ?? null, metadata: request.metadata ?? {} });
  }

  private async append(receipt: AutonomousEvidenceReceiptJSON, assessment: AutonomousEvidenceAssessmentJSON | null): Promise<AutonomousEvidenceRuntimeJournalEntry> {
    const previous = [...this.recordsByRequest.values()].sort((left, right) => left.sequence - right.sequence).at(-1);
    const base = { schema: AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA, sequence: (previous?.sequence ?? 0) + 1, previous_entry_digest: previous?.entry_digest ?? null, receipt, assessment, retention: "metadata_only;raw_acquisition_and_evaluator_values_excluded" as const, secret_material: "never_returned" as const };
    const entry = { ...base, entry_digest: await digestJson(base) } as AutonomousEvidenceRuntimeJournalEntry;
    const persisted = this.journal ? await this.journal.append(entry) : entry;
    this.recordsByRequest.set(receipt.request_digest, persisted);
    return persisted;
  }

  private async receipt(input: Omit<AutonomousEvidenceReceiptJSON, "receipt_digest">): Promise<AutonomousEvidenceReceiptJSON> {
    const { receipt_digest: _receiptDigest, ...descriptor } = input as Omit<AutonomousEvidenceReceiptJSON, "receipt_digest"> & { receipt_digest?: string };
    return { ...descriptor, receipt_digest: await digestJson(descriptor) } as AutonomousEvidenceReceiptJSON;
  }

  private async assessment(input: Omit<AutonomousEvidenceAssessmentJSON, "assessment_digest">): Promise<AutonomousEvidenceAssessmentJSON> {
    const { assessment_digest: _assessmentDigest, ...descriptor } = input as Omit<AutonomousEvidenceAssessmentJSON, "assessment_digest"> & { assessment_digest?: string };
    return { ...descriptor, assessment_digest: await digestJson(descriptor) } as AutonomousEvidenceAssessmentJSON;
  }

  private async replayPrior(
    entry: AutonomousEvidenceRuntimeJournalEntry,
    request: AutonomousEvidenceAcquisitionRequest,
    options: AutonomousEvidenceRuntimeExecuteOptions,
  ): Promise<{ receipt: AutonomousEvidenceReceiptJSON; assessment: AutonomousEvidenceAssessmentJSON | null; value: JsonValue | null }> {
    const prior = entry.receipt;
    let value = this.valuesByRequest.get(prior.request_digest) ?? null;
    if (value === null && options.rehydrateValue) {
      const restored = await options.rehydrateValue(prior);
      if (restored !== null) {
        if (prior.value_digest === null || await digestJson(restored) !== prior.value_digest) throw new ProviderRuntimeError("rehydrated evidence value does not match its receipt digest");
        value = restored;
      }
    } else if (value === null && this.protectedRehydration && prior.value_digest !== null) {
      const restored = this.protectedRehydration.resolveReceipt(prior, { purpose: "evidence_runtime_value", valueKind: "evidence_value", oneTime: false }) as JsonValue | null;
      if (restored !== null && await digestJson(restored) !== prior.value_digest) throw new ProviderRuntimeError("protected evidence value does not match its receipt digest");
      value = restored;
    }
    if (value === null && prior.value_digest !== null) {
      const { receipt_digest: _receiptDigest, ...descriptor } = prior;
      const receipt = await this.receipt({ ...descriptor, status: "reconciliation_required", replay: "replayed", limitations: [...prior.limitations, "caller-owned evidence value requires rehydration"] });
      return { receipt, assessment: entry.assessment, value: null };
    }
    return { receipt: { ...prior, replay: "replayed" }, assessment: entry.assessment, value };
  }

  private async reconcilePrior(
    entry: AutonomousEvidenceRuntimeJournalEntry,
    requirement: AutonomousEvidenceRequirement,
    value: JsonValue,
    options: AutonomousEvidenceRuntimeExecuteOptions,
  ): Promise<{ receipt: AutonomousEvidenceReceiptJSON; assessment: AutonomousEvidenceAssessmentJSON | null } | null> {
    if (!options.evaluator || !["observed", "partial"].includes(entry.receipt.status) || !entry.receipt.observed_requirement_ids.includes(requirement.requirement_id) || !["not_evaluated", "indeterminate", "failed"].includes(entry.receipt.evaluator_status)) return null;
    const replayBase = await this.receipt({
      ...entry.receipt,
      replay: "replayed",
      evaluator_status: "not_evaluated",
      assessment_digest: null,
    });
    try {
      const decision = await options.evaluator.evaluate({
        requirement,
        receipt: replayBase,
        observations: replayBase.observations,
        value,
      });
      const evaluatorId = boundedIdentifier("evidence runtime evaluator_id", decision.evaluator_id);
      const evaluatorVersion = boundedIdentifier("evidence runtime evaluator_version", decision.evaluator_version);
      if (evaluatorId !== boundedIdentifier("configured evidence runtime evaluator_id", options.evaluator.evaluator_id) || evaluatorVersion !== boundedIdentifier("configured evidence runtime evaluator_version", options.evaluator.evaluator_version)) throw new ArgumentError("evidence runtime evaluator identity does not match configured evaluator");
      if (!["accepted", "rejected", "indeterminate"].includes(decision.verdict) || typeof decision.score !== "number" || !Number.isFinite(decision.score) || decision.score < 0 || decision.score > 1) throw new ArgumentError("evidence runtime evaluator verdict is malformed");
      const assessment = await this.assessment({
        schema: AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA,
        receipt_digest: replayBase.receipt_digest,
        requirement_id: requirement.requirement_id,
        evaluator_id: evaluatorId,
        evaluator_version: evaluatorVersion,
        verdict: decision.verdict,
        score: decision.score,
        feedback_digest: digestOrNull("evidence runtime feedback_digest", decision.feedback_digest),
        evidence_digest: digestOrNull("evidence runtime evidence_digest", decision.evidence_digest),
        failure_class: decision.failure_class === undefined || decision.failure_class === null ? null : boundedIdentifier("evidence runtime failure_class", decision.failure_class),
        retention: "value_only;evaluator_payloads_caller_owned",
        secret_material: "never_returned",
      });
      const receipt = await this.receipt({ ...replayBase, evaluator_status: assessment.verdict, assessment_digest: assessment.assessment_digest });
      await this.append(receipt, assessment);
      return { receipt, assessment };
    } catch (error) {
      const receipt = await this.receipt({
        ...replayBase,
        evaluator_status: "failed",
        limitations: [...replayBase.limitations, "caller-owned evaluator failed", error instanceof Error ? error.constructor.name : "evaluator_failed"],
      });
      await this.append(receipt, null);
      return { receipt, assessment: null };
    }
  }

  async execute(requests: readonly AutonomousEvidenceAcquisitionRequest[], options: AutonomousEvidenceRuntimeExecuteOptions): Promise<AutonomousEvidenceRuntimeResult> {
    if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS) throw new ArgumentError(`evidence runtime requests must contain 1..${MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS} entries`);
    if (!options || !options.acquirer || typeof options.acquirer.acquire !== "function") throw new ArgumentError("evidence runtime requires a caller-owned acquirer");
    if (options.projector !== undefined && typeof options.projector.project !== "function") throw new ArgumentError("evidence runtime projector is malformed");
    if (options.evaluator !== undefined && (typeof options.evaluator.evaluate !== "function" || !options.evaluator.evaluator_id || !options.evaluator.evaluator_version)) throw new ArgumentError("evidence runtime evaluator is malformed");
    const parentEvidenceDigests = boundedList("evidence runtime parent_evidence_digests", options.parentEvidenceDigests ?? [], 64);
    const normalized = requests.map(normalizeRequest);
    const receipts: AutonomousEvidenceReceiptJSON[] = [];
    const assessments: AutonomousEvidenceAssessmentJSON[] = [];
    const values: Record<string, JsonValue | null> = {};
    const available = new Set(this.plan.available_evidence);
    const completed = new Set<string>();
    const pendingEvaluation = new Set<string>();
    const omitted: string[] = [];
    let sawFailure = false;
    let sawReconciliation = false;
    let sawPendingEvaluation = false;
    for (let index = 0; index < normalized.length; index += 1) {
      const request = normalized[index]!;
      const requirement = requirementFor(this.plan, request.requirement_id);
      const requestDigest = await this.requestDigest(request);
      const prior = this.recordsByRequest.get(requestDigest);
      if (prior) {
        let replay = await this.replayPrior(prior, request, options);
        if (options.reevaluatePending === true && replay.value !== null) {
          const reconciled = await this.reconcilePrior(prior, requirement, replay.value, options);
          if (reconciled) replay = { ...reconciled, value: replay.value };
        }
        receipts.push(replay.receipt);
        if (replay.assessment) assessments.push(replay.assessment);
        values[requestDigest] = replay.value;
        if (replay.value !== null) this.valuesByRequest.set(requestDigest, replay.value);
        for (const id of replay.receipt.observed_requirement_ids) available.add(id);
        if (replay.receipt.status === "reconciliation_required") sawReconciliation = true;
        if (replay.assessment?.verdict === "accepted") completed.add(replay.receipt.requirement_id);
        else if (replay.receipt.evaluator_status === "not_evaluated" || replay.receipt.evaluator_status === "indeterminate" || replay.receipt.evaluator_status === "failed") pendingEvaluation.add(replay.receipt.requirement_id);
        continue;
      }
      if (sawFailure && options.stopOnFailure === true) {
        omitted.push(requestDigest);
        continue;
      }
      const started = Date.now();
      const context: AutonomousEvidenceAcquisitionContext = { plan_digest: this.plan.plan_digest, requirement, request, attempt: 1, parent_evidence_digests: [...parentEvidenceDigests], execution: "caller_owned_adapter;raw_value_transient" };
      let raw: JsonValue;
      try {
        raw = await options.acquirer.acquire(context);
        jsonBytes(raw, "evidence runtime acquisition value");
      } catch (error) {
        sawFailure = true;
        const failure = await this.receipt({ schema: AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA, request_digest: requestDigest, plan_digest: this.plan.plan_digest, requirement_id: requirement.requirement_id, domain: requirement.domain, workflow_id: requirement.workflow_id, workflow_digest: requirement.workflow_digest, stage_id: requirement.stage_id, source_id: request.source_id, source_digest: request.source_digest ?? null, attempt: 1, status: "failed", replay: "fresh", value_digest: null, value_bytes: 0, observations: [], observed_requirement_ids: [], missing_requirement_ids: [requirement.requirement_id], evidence_status: "not_evaluated", evaluator_status: "not_evaluated", assessment_digest: null, limitations: ["caller-owned acquisition failed"], error_class: error instanceof Error ? error.constructor.name : "acquisition_failed", duration_ms: Math.max(0, Date.now() - started), retention: "metadata_only;raw_acquisition_values_caller_owned", secret_material: "never_returned" });
        await this.append(failure, null);
        receipts.push(failure);
        values[requestDigest] = null;
        continue;
      }
      const valueDigest = await digestJson(raw);
      const valueBytes = bytes(canonicalJson(raw));
      this.valuesByRequest.set(requestDigest, raw);
      values[requestDigest] = raw;
      let observations: AutonomousEvidenceObservation[] = [];
      let evidenceStatus: AutonomousEvidenceReceiptJSON["evidence_status"] = "missing_required_outputs";
      let projectionFailure: string | null = null;
      if (options.projector) {
        try {
          observations = (await options.projector.project(raw, context)).map(normalizeObservation);
          const matches = observations.some((observation) => observation.label === requirement.requirement_id || observation.label === requirement.label);
          if (matches) evidenceStatus = "declared_for_evaluator";
        } catch (error) {
          projectionFailure = error instanceof Error ? error.constructor.name : "projection_failed";
          evidenceStatus = "projection_failed";
        }
      }
      const observedIds = evidenceStatus === "declared_for_evaluator" ? [requirement.requirement_id] : [];
      if (observedIds.length) available.add(requirement.requirement_id);
      const missingIds = observedIds.length ? [] : [requirement.requirement_id];
      const baseReceipt = await this.receipt({ schema: AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA, request_digest: requestDigest, plan_digest: this.plan.plan_digest, requirement_id: requirement.requirement_id, domain: requirement.domain, workflow_id: requirement.workflow_id, workflow_digest: requirement.workflow_digest, stage_id: requirement.stage_id, source_id: request.source_id, source_digest: request.source_digest ?? null, attempt: 1, status: observedIds.length ? "observed" : "partial", replay: "fresh", value_digest: valueDigest, value_bytes: valueBytes, observations, observed_requirement_ids: observedIds, missing_requirement_ids: missingIds, evidence_status: evidenceStatus, evaluator_status: "not_evaluated", assessment_digest: null, limitations: ["raw acquisition value is transient and caller-owned", ...(projectionFailure ? ["observation projection failed", projectionFailure] : [])], error_class: null, duration_ms: Math.max(0, Date.now() - started), retention: "metadata_only;raw_acquisition_values_caller_owned", secret_material: "never_returned" });
      let receipt = baseReceipt;
      let assessment: AutonomousEvidenceAssessmentJSON | null = null;
      if (options.evaluator && observedIds.length) {
        try {
          const decision = await options.evaluator.evaluate({ requirement, receipt: baseReceipt, observations, value: raw });
          const evaluatorId = boundedIdentifier("evidence runtime evaluator_id", decision.evaluator_id);
          const evaluatorVersion = boundedIdentifier("evidence runtime evaluator_version", decision.evaluator_version);
          if (evaluatorId !== boundedIdentifier("configured evidence runtime evaluator_id", options.evaluator.evaluator_id) || evaluatorVersion !== boundedIdentifier("configured evidence runtime evaluator_version", options.evaluator.evaluator_version)) throw new ArgumentError("evidence runtime evaluator identity does not match configured evaluator");
          if (!["accepted", "rejected", "indeterminate"].includes(decision.verdict) || typeof decision.score !== "number" || !Number.isFinite(decision.score) || decision.score < 0 || decision.score > 1) throw new ArgumentError("evidence runtime evaluator verdict is malformed");
          assessment = await this.assessment({ schema: AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA, receipt_digest: baseReceipt.receipt_digest, requirement_id: requirement.requirement_id, evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, verdict: decision.verdict, score: decision.score, feedback_digest: digestOrNull("evidence runtime feedback_digest", decision.feedback_digest), evidence_digest: digestOrNull("evidence runtime evidence_digest", decision.evidence_digest), failure_class: decision.failure_class === undefined || decision.failure_class === null ? null : boundedIdentifier("evidence runtime failure_class", decision.failure_class), retention: "value_only;evaluator_payloads_caller_owned", secret_material: "never_returned" });
          receipt = await this.receipt({ ...baseReceipt, evaluator_status: assessment.verdict, assessment_digest: assessment.assessment_digest });
          if (assessment.verdict === "accepted") completed.add(requirement.requirement_id);
          else pendingEvaluation.add(requirement.requirement_id);
        } catch (error) {
          sawPendingEvaluation = true;
          pendingEvaluation.add(requirement.requirement_id);
          receipt = await this.receipt({ ...baseReceipt, evaluator_status: "failed", limitations: [...baseReceipt.limitations, "caller-owned evaluator failed", error instanceof Error ? error.constructor.name : "evaluator_failed"] });
        }
      } else if (observedIds.length) {
        sawPendingEvaluation = true;
        pendingEvaluation.add(requirement.requirement_id);
      }
      if (assessment) assessments.push(assessment);
      await this.append(receipt, assessment);
      receipts.push(receipt);
    }
    const nextPlan = await this.plan.withAvailableEvidence([...available]);
    const accepted = new Set(assessments.filter((assessment) => assessment.verdict === "accepted").map((assessment) => assessment.requirement_id));
    for (const id of accepted) completed.add(id);
    const allCovered = nextPlan.missing_requirement_ids.length === 0;
    const allAccepted = nextPlan.requirements.every((requirement) => accepted.has(requirement.requirement_id));
    const status: AutonomousEvidenceRuntimeStatus = sawReconciliation ? "reconciliation_required" : sawFailure && receipts.every((receipt) => receipt.status === "failed") ? "failed" : allCovered && allAccepted ? "completed" : sawPendingEvaluation || (allCovered && !allAccepted) ? "awaiting_evaluation" : "partial";
    const descriptor = { schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA, status, plan_digest: nextPlan.plan_digest, receipt_digests: receipts.map((receipt) => receipt.receipt_digest), assessment_digests: assessments.map((assessment) => assessment.assessment_digest), completed_requirement_ids: [...completed].sort(), pending_evaluation_requirement_ids: [...pendingEvaluation].sort(), missing_requirement_ids: [...nextPlan.missing_requirement_ids].sort(), next_stage_ids: [...nextPlan.next_stage_ids].sort(), omitted_request_digests: [...omitted].sort(), retention: "metadata_only;raw_values_caller_owned" as const, secret_material: "never_returned" as const };
    const json = { schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA, status, plan: nextPlan.toJSON(), receipts, assessments, completed_requirement_ids: [...completed].sort(), pending_evaluation_requirement_ids: [...pendingEvaluation].sort(), missing_requirement_ids: [...nextPlan.missing_requirement_ids].sort(), next_stage_ids: [...nextPlan.next_stage_ids].sort(), omitted_request_digests: [...omitted].sort(), result_digest: await digestJson(descriptor), retention: "metadata_only;raw_values_caller_owned" as const, secret_material: "never_returned" as const } as AutonomousEvidenceRuntimeResultJSON;
    return makeResult(json, values);
  }
}
