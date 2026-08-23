import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import type {
  AutonomousDomainToolBinding,
  AutonomousDomainToolExecutionReceipt,
  AutonomousDomainToolRuntime,
  AutonomousWorkflowToolContext,
} from "./autonomous.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import type { ProviderToolCall, ProviderToolResult } from "./llm.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { BrainBanditContext, BrainBanditState, JsonObject, JsonValue } from "./types.js";
import type {
  AutonomousCapabilityJournalStore,
} from "./autonomous-capability-persistence.js";
import { validateAutonomousCapabilityExecutionRecord } from "./autonomous-capability-persistence.js";

/** Stable schema for the adapter-to-evaluator capability boundary. */
export const AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-capability-execution/0.1" as const;
export const AUTONOMOUS_CAPABILITY_BATCH_SCHEMA = "bioprism-typescript-autonomous-capability-batch/0.1" as const;
export const AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA = "bioprism-typescript-autonomous-capability-observation/0.1" as const;
export const MAX_AUTONOMOUS_CAPABILITY_BATCH = 64;
export const MAX_AUTONOMOUS_CAPABILITY_HISTORY = 512;
export const MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS = 128;
export const AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA = "bioprism-typescript-autonomous-capability-learning-settlement/0.1" as const;
export const AUTONOMOUS_CAPABILITY_LEARNING_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-capability-learning-receipt/0.1" as const;
const LEGACY_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-capability-learning-snapshot/0.1" as const;
export const AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-capability-learning-snapshot/0.2" as const;
export const MAX_AUTONOMOUS_CAPABILITY_LEARNING_EVIDENCE_BYTES = 256_000;
export const MAX_AUTONOMOUS_CAPABILITY_LEARNING_RECEIPTS = 8_192;
export const MAX_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_BYTES = 4_000_000;

export type AutonomousCapabilityExecutionStatus = "completed" | "approval_required" | "reconciliation_required" | "refused" | "failed";
export type AutonomousCapabilityReplayStatus = "fresh" | "replayed";
export type AutonomousCapabilityEvidenceStatus = "not_evaluated" | "missing_required_outputs" | "declared_for_evaluator" | "projection_failed";
export type AutonomousCapabilityObservationKind = "fact" | "measurement" | "provenance" | "limitation" | "warning";
export type AutonomousCapabilityObservationStatus = "observed" | "inferred" | "missing";

/**
 * An adapter observation is intentionally metadata-only. The observation points at a value by
 * digest; it never stores the value, a prompt, a credential, or an external-world conclusion.
 */
export interface AutonomousCapabilityObservationInput {
  id: string;
  label: string;
  kind: AutonomousCapabilityObservationKind;
  status: AutonomousCapabilityObservationStatus;
  value_digest?: string | null;
  source_digest?: string | null;
  confidence?: number | null;
  limitations?: string[];
}

export interface AutonomousCapabilityObservation extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA;
  id: string;
  label: string;
  kind: AutonomousCapabilityObservationKind;
  status: AutonomousCapabilityObservationStatus;
  value_digest: string | null;
  source_digest: string | null;
  confidence: number | null;
  limitations: string[];
}

export interface AutonomousCapabilityExecutionRequest extends JsonObject {
  schema?: typeof AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA;
  call_id: string;
  tool: string;
  arguments: JsonObject;
  workflow_context: AutonomousWorkflowToolContext;
  /** Digest of the semantic task/input the adapter is being asked to inspect. */
  input_digest: string;
  /** Optional digest of a caller-owned subject, record, artifact, or workspace. */
  subject_digest?: string | null;
  /** Ordered digests of prior evidence that this capability is consuming. */
  parent_evidence_digests?: string[];
  /** Optional caller-owned idempotency label; its plaintext is never retained. */
  replay_key?: string;
  execution_id?: string | null;
}

export interface AutonomousCapabilityExecutionRecord extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA;
  record_kind: "capability_execution_record";
  request_digest: string;
  execution_id: string | null;
  call_id: string;
  domain: string;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  stage_contract_digest: string | null;
  tool: string;
  capability: string | null;
  risk_class: string | null;
  schema_digest: string | null;
  input_digest: string;
  subject_digest: string | null;
  parent_evidence_digests: string[];
  arguments_digest: string;
  replay_key_digest: string | null;
  status: AutonomousCapabilityExecutionStatus;
  replay: AutonomousCapabilityReplayStatus;
  output_digest: string | null;
  output_bytes: number;
  observations: AutonomousCapabilityObservation[];
  evidence_digest: string | null;
  evidence_status: AutonomousCapabilityEvidenceStatus;
  required_evidence_outputs: string[];
  missing_evidence_outputs: string[];
  limitations: string[];
  effect: string | null;
  effect_id: string | null;
  error_class: string | null;
  duration_ms: number;
  does_not_claim: string[];
  secret_material: "never_returned";
}

/** The value is intentionally transient; persist only `record` in a durable store. */
export interface AutonomousCapabilityExecutionResult extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA;
  record: AutonomousCapabilityExecutionRecord;
  value: JsonValue | null;
  value_retention: "transient_caller_value_only";
  secret_material: "never_returned";
}

/** Metadata-only evaluator input. The transient execution value is intentionally absent. */
export interface AutonomousCapabilityEvaluationInput extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA;
  request_digest: string;
  execution_record_digest: string;
  capability_status: AutonomousCapabilityExecutionStatus;
  replay: AutonomousCapabilityReplayStatus;
  execution_id: string | null;
  call_id: string;
  domain: string;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  stage_contract_digest: string | null;
  tool: string;
  capability: string | null;
  risk_class: string | null;
  schema_digest: string | null;
  input_digest: string;
  subject_digest: string | null;
  parent_evidence_digests: string[];
  arguments_digest: string;
  output_digest: string | null;
  output_bytes: number;
  observations: AutonomousCapabilityObservation[];
  evidence_digest: string | null;
  evidence_status: AutonomousCapabilityEvidenceStatus;
  required_evidence_outputs: string[];
  missing_evidence_outputs: string[];
  limitations: string[];
  effect: string | null;
  effect_id: string | null;
  error_class: string | null;
  duration_ms: number;
  caller_evidence: JsonObject;
  retention: "metadata_only;transient_value_excluded";
}

export interface AutonomousCapabilityEvaluatorAssessment extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
}

export interface AutonomousCapabilityEvaluator {
  evaluator_id: string;
  evaluator_version: string;
  evaluate: (input: AutonomousCapabilityEvaluationInput) => AutonomousCapabilityEvaluatorAssessment | Promise<AutonomousCapabilityEvaluatorAssessment>;
}

export interface AutonomousCapabilityLearningSettlement extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA;
  status: "settled";
  request_digest: string;
  execution_record_digest: string;
  settlement_key: string;
  settlement_digest: string;
  arm_id: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  caller_evidence_digest: string;
  outcome_digest: string;
  next_state: BrainBanditState;
  next_state_digest: string;
  idempotent_replay: boolean;
  retention: "value_only;capability_payloads_excluded";
  secret_material: "never_returned";
}

export interface AutonomousCapabilityLearningSettlementReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_LEARNING_RECEIPT_SCHEMA;
  settlement_key: string;
  request_digest: string;
  execution_record_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  settlement_digest: string;
  settlement: AutonomousCapabilityLearningSettlement;
  retention: "value_only;capability_payloads_excluded";
  secret_material: "never_returned";
}

/** Durable, caller-owned replay barrier for direct capability learning settlements. */
export interface AutonomousCapabilityLearningSettlementStore {
  load(settlementKey: string): Promise<AutonomousCapabilityLearningSettlementReceipt | null> | AutonomousCapabilityLearningSettlementReceipt | null;
  save(receipt: AutonomousCapabilityLearningSettlementReceipt): Promise<void> | void;
}

/** Digest-bound restart image for the metadata-only capability learning receipt journal. */
export interface AutonomousCapabilityLearningSnapshot extends JsonObject {
  /** 0.1 remains readable; current images carry independent snapshot lineage in 0.2. */
  schema: typeof AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA | typeof LEGACY_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA;
  snapshot_generation?: number;
  previous_snapshot_digest?: string | null;
  receipts: AutonomousCapabilityLearningSettlementReceipt[];
  retention: "value_only;capability_payloads_excluded";
  secret_material: "never_returned";
  snapshot_digest: string;
}

/** A settlement store with caller-owned restart snapshots. */
export interface AutonomousCapabilityLearningSnapshotStore extends AutonomousCapabilityLearningSettlementStore {
  snapshot(): Promise<AutonomousCapabilityLearningSnapshot>;
  restore(snapshot: AutonomousCapabilityLearningSnapshot): Promise<void> | void;
}

/** Adapter contract for SQLite, Postgres, IndexedDB, object storage, or another durable owner. */
export interface AutonomousCapabilityLearningSnapshotPersistence {
  read(): Promise<AutonomousCapabilityLearningSnapshot | null> | AutonomousCapabilityLearningSnapshot | null;
  write(snapshot: AutonomousCapabilityLearningSnapshot): Promise<void> | void;
}

export interface AutonomousCapabilityLearningRewardUpdate {
  failed: boolean;
  outcomeDigest: string;
  contractDigest: string | null;
  contextDigest?: string | null;
  context?: BrainBanditContext;
}

export interface AutonomousCapabilityLearningOptions {
  evaluator: AutonomousCapabilityEvaluator;
  armId?: string;
  callerEvidence?: JsonObject;
  allowReconciliation?: boolean;
  idempotencyKey?: string;
  contextDigest?: string | null;
  context?: BrainBanditContext;
  settlementStore?: AutonomousCapabilityLearningSettlementStore;
  recordEvaluatorReward: (armId: string, reward: number, update: AutonomousCapabilityLearningRewardUpdate) => Promise<BrainBanditState> | BrainBanditState;
}

export interface AutonomousCapabilityLearningBatchOptions extends Omit<AutonomousCapabilityLearningOptions, "callerEvidence" | "armId" | "idempotencyKey"> {
  evidence?: Readonly<Record<string, JsonObject>>;
  armIdFor?: (record: AutonomousCapabilityExecutionRecord) => string;
}

export interface AutonomousCapabilityLearningBatchResult extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA;
  status: "settled";
  settlements: AutonomousCapabilityLearningSettlement[];
  batch_digest: string;
  retention: "value_only;capability_payloads_excluded";
  secret_material: "never_returned";
}

export interface AutonomousCapabilityExecutionOptions {
  approveEffects?: boolean;
  execution?: AutonomousExecutionController;
  effectBoundary?: import("./autonomous-effects.js").AutonomousEffectBoundary;
  /** Projects a raw adapter value into bounded metadata observations before it is discarded. */
  projectObservations?: (value: JsonValue, request: AutonomousCapabilityExecutionRequest) => readonly AutonomousCapabilityObservationInput[] | Promise<readonly AutonomousCapabilityObservationInput[]>;
}

export interface AutonomousCapabilityBatchOptions extends AutonomousCapabilityExecutionOptions {
  /** Ordered execution is deliberate: stage dependencies and effect reconciliation stay visible. */
  maxConcurrency?: 1;
  stopOnFailure?: boolean;
}

export interface AutonomousCapabilityBatchItem extends JsonObject {
  index: number;
  request_digest: string;
  result: AutonomousCapabilityExecutionResult | null;
  omission_reason: "stopped_after_failure" | null;
}

export interface AutonomousCapabilityBatchResult extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_BATCH_SCHEMA;
  batch_digest: string;
  status: "completed" | "partial";
  items: AutonomousCapabilityBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  execution: "ordered_serial";
  durable_projection: "records_and_digests_only";
  secret_material: "never_returned";
}

type NormalizedRequest = Required<Pick<AutonomousCapabilityExecutionRequest, "call_id" | "tool" | "arguments" | "workflow_context" | "input_digest">> & {
  subject_digest: string | null;
  parent_evidence_digests: string[];
  replay_key: string | null;
  execution_id: string | null;
};

interface CachedExecution {
  request_digest: string;
  result: AutonomousCapabilityExecutionResult;
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digestOrNull(name: string, value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
  return value;
}

function normalizeWorkflowContext(value: unknown): AutonomousWorkflowToolContext {
  if (!isObject(value)) throw new ArgumentError("capability workflow_context must be an object");
  return {
    domain: boundedIdentifier("capability workflow_context domain", value.domain) as AutonomousWorkflowToolContext["domain"],
    workflow_id: boundedIdentifier("capability workflow_context workflow_id", value.workflow_id),
    workflow_digest: digestOrNull("capability workflow_context workflow_digest", value.workflow_digest) as string,
    stage_id: boundedIdentifier("capability workflow_context stage_id", value.stage_id),
  };
}

function normalizeRequest(value: AutonomousCapabilityExecutionRequest): NormalizedRequest {
  if (!isObject(value)) throw new ArgumentError("capability execution request must be an object");
  if (value.schema !== undefined && value.schema !== AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA) throw new ArgumentError("capability execution request schema is unsupported");
  if (!isObject(value.arguments)) throw new ArgumentError("capability execution arguments must be a JSON object");
  const parentEvidenceDigests = value.parent_evidence_digests === undefined ? [] : value.parent_evidence_digests;
  if (!Array.isArray(parentEvidenceDigests) || parentEvidenceDigests.length > 64) throw new ArgumentError("parent_evidence_digests must contain at most 64 entries");
  const normalizedParents = parentEvidenceDigests.map((digest, index) => digestOrNull(`parent_evidence_digests[${index}]`, digest) as string);
  if (new Set(normalizedParents).size !== normalizedParents.length) throw new ArgumentError("parent_evidence_digests must not contain duplicates");
  const workflowContext = normalizeWorkflowContext(value.workflow_context);
  const inputDigest = digestOrNull("capability input_digest", value.input_digest);
  if (!inputDigest) throw new ArgumentError("capability input_digest is required");
  return {
    call_id: boundedText("capability call_id", value.call_id, 256),
    tool: boundedIdentifier("capability tool", value.tool),
    arguments: value.arguments,
    workflow_context: workflowContext,
    input_digest: inputDigest,
    subject_digest: digestOrNull("capability subject_digest", value.subject_digest),
    parent_evidence_digests: normalizedParents,
    replay_key: value.replay_key === undefined ? null : boundedText("capability replay_key", value.replay_key, 256),
    execution_id: value.execution_id === undefined || value.execution_id === null ? null : boundedText("capability execution_id", value.execution_id, 256),
  };
}

function normalizeObservation(value: AutonomousCapabilityObservationInput, index: number): AutonomousCapabilityObservation {
  if (!isObject(value)) throw new ArgumentError(`capability observation ${index} must be an object`);
  const kind = value.kind;
  const status = value.status;
  if (!["fact", "measurement", "provenance", "limitation", "warning"].includes(kind as string)) throw new ArgumentError(`capability observation ${index} kind is unsupported`);
  if (!["observed", "inferred", "missing"].includes(status as string)) throw new ArgumentError(`capability observation ${index} status is unsupported`);
  const confidence = value.confidence === undefined || value.confidence === null ? null : value.confidence;
  if (confidence !== null && (typeof confidence !== "number" || !Number.isFinite(confidence) || confidence < 0 || confidence > 1)) throw new ArgumentError(`capability observation ${index} confidence must be within [0, 1]`);
  const limitations = value.limitations === undefined ? [] : value.limitations;
  if (!Array.isArray(limitations) || limitations.length > 32 || limitations.some((item) => typeof item !== "string" || bytes(item) > 2048)) throw new ArgumentError(`capability observation ${index} limitations are outside their bounds`);
  return {
    schema: AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA,
    id: boundedIdentifier(`capability observation ${index} id`, value.id),
    label: boundedText(`capability observation ${index} label`, value.label, 256),
    kind: kind as AutonomousCapabilityObservationKind,
    status: status as AutonomousCapabilityObservationStatus,
    value_digest: digestOrNull(`capability observation ${index} value_digest`, value.value_digest),
    source_digest: digestOrNull(`capability observation ${index} source_digest`, value.source_digest),
    confidence,
    limitations: [...limitations] as string[],
  };
}

function resultStatus(response: ProviderToolResult, receipt: AutonomousDomainToolExecutionReceipt | undefined): AutonomousCapabilityExecutionStatus {
  const status = isObject(response.content) && typeof response.content.status === "string" ? response.content.status : receipt?.status;
  if (status === "approval_required") return "approval_required";
  if (status === "reconciliation_required") return "reconciliation_required";
  if (status === "activation_required" || status === "authorization_required") return "refused";
  return response.approved && response.isError !== true ? "completed" : "failed";
}

function copyResult(result: AutonomousCapabilityExecutionResult, replay: AutonomousCapabilityReplayStatus): AutonomousCapabilityExecutionResult {
  return {
    ...result,
    record: { ...result.record, replay, observations: result.record.observations.map((observation) => ({ ...observation, limitations: [...observation.limitations] })), required_evidence_outputs: [...result.record.required_evidence_outputs], missing_evidence_outputs: [...result.record.missing_evidence_outputs], parent_evidence_digests: [...result.record.parent_evidence_digests], limitations: [...result.record.limitations], does_not_claim: [...result.record.does_not_claim] },
    value: result.value,
  };
}

function cloneRecord(record: AutonomousCapabilityExecutionRecord): AutonomousCapabilityExecutionRecord {
  return structuredClone(record);
}

function commonDoesNotClaim(): string[] {
  return [
    "capability execution is not proof that the overall task succeeded",
    "an adapter output digest is not a claim about external-world truth",
    "declared observations require evaluator and provenance review",
    "a complete evidence label set does not authorize effects or certify correctness",
  ];
}

const CAPABILITY_LEARNING_FORBIDDEN_KEYS = new Set([
  "apikey", "authorization", "bearer", "credential", "password", "secret", "accesstoken",
  "refreshtoken", "token", "privatekey", "prompt", "response", "rawpayload", "arguments",
  "output", "task", "messages", "value", "result",
]);

function assertCapabilityLearningMetadata(value: unknown, path = "capability learning metadata", depth = 0): void {
  if (depth > 32) throw new ArgumentError(`${path} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 4_096) throw new ArgumentError(`${path} contains too many values`);
    value.forEach((child, index) => assertCapabilityLearningMetadata(child, `${path}[${index}]`, depth + 1));
    return;
  }
  if (!isObject(value)) {
    if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${path} contains a non-finite number`);
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
    if (CAPABILITY_LEARNING_FORBIDDEN_KEYS.has(normalized)) throw new ArgumentError(`${path} contains transient or secret-shaped field ${key}`);
    assertCapabilityLearningMetadata(child, `${path}.${key}`, depth + 1);
  }
}

async function boundedCapabilityLearningMetadata(value: unknown, name: string): Promise<JsonObject> {
  if (!isObject(value)) throw new ArgumentError(`${name} must be a JSON object`);
  assertCapabilityLearningMetadata(value, name);
  const encoded = JSON.stringify(value);
  if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_CAPABILITY_LEARNING_EVIDENCE_BYTES) throw new ArgumentError(`${name} exceeds its bounded size`);
  await digestJson(value);
  return structuredClone(value);
}

function capabilityRecordFromResult(value: AutonomousCapabilityExecutionResult | AutonomousCapabilityExecutionRecord): unknown {
  return isObject(value) && isObject(value.record) ? value.record : value;
}

async function capabilityEvaluationInput(record: AutonomousCapabilityExecutionRecord, callerEvidence: JsonObject): Promise<AutonomousCapabilityEvaluationInput> {
  const executionRecordDigest = await digestJson(record);
  return {
    schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA,
    request_digest: record.request_digest,
    execution_record_digest: executionRecordDigest,
    capability_status: record.status,
    replay: record.replay,
    execution_id: record.execution_id,
    call_id: record.call_id,
    domain: record.domain,
    workflow_id: record.workflow_id,
    workflow_digest: record.workflow_digest,
    stage_id: record.stage_id,
    stage_contract_digest: record.stage_contract_digest,
    tool: record.tool,
    capability: record.capability,
    risk_class: record.risk_class,
    schema_digest: record.schema_digest,
    input_digest: record.input_digest,
    subject_digest: record.subject_digest,
    parent_evidence_digests: [...record.parent_evidence_digests],
    arguments_digest: record.arguments_digest,
    output_digest: record.output_digest,
    output_bytes: record.output_bytes,
    observations: record.observations.map((observation) => ({ ...observation, limitations: [...observation.limitations] })),
    evidence_digest: record.evidence_digest,
    evidence_status: record.evidence_status,
    required_evidence_outputs: [...record.required_evidence_outputs],
    missing_evidence_outputs: [...record.missing_evidence_outputs],
    limitations: [...record.limitations],
    effect: record.effect,
    effect_id: record.effect_id,
    error_class: record.error_class,
    duration_ms: record.duration_ms,
    caller_evidence: callerEvidence,
    retention: "metadata_only;transient_value_excluded",
  };
}

function validateCapabilityAssessment(raw: unknown, evaluator: AutonomousCapabilityEvaluator): AutonomousCapabilityEvaluatorAssessment {
  if (!isObject(raw)) throw new ArgumentError("capability evaluator must return an object");
  const evaluatorId = boundedIdentifier("capability evaluator_id", raw.evaluator_id);
  const evaluatorVersion = boundedIdentifier("capability evaluator_version", raw.evaluator_version);
  if (evaluatorId !== boundedIdentifier("configured capability evaluator_id", evaluator.evaluator_id) || evaluatorVersion !== boundedIdentifier("configured capability evaluator_version", evaluator.evaluator_version)) throw new ArgumentError("capability evaluator decision identity does not match the configured evaluator");
  if (typeof raw.reward !== "number" || !Number.isFinite(raw.reward) || raw.reward < -1 || raw.reward > 1) throw new ArgumentError("capability evaluator reward must be finite and within [-1, 1]");
  if (typeof raw.passed !== "boolean") throw new ArgumentError("capability evaluator passed must be boolean");
  const failed = raw.failed === undefined ? !raw.passed : raw.failed;
  if (typeof failed !== "boolean" || (raw.passed && failed)) throw new ArgumentError("capability evaluator passed and failed are contradictory");
  const feedbackDigest = digestOrNull("capability evaluator feedback_digest", raw.feedback_digest);
  const evidenceDigest = digestOrNull("capability evaluator evidence_digest", raw.evidence_digest);
  const failureClass = raw.failure_class === undefined || raw.failure_class === null ? null : boundedIdentifier("capability evaluator failure_class", raw.failure_class);
  return { evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, reward: raw.reward, passed: raw.passed, failed, feedback_digest: feedbackDigest, failure_class: failureClass, evidence_digest: evidenceDigest };
}

function cloneCapabilitySettlement(value: AutonomousCapabilityLearningSettlement, idempotentReplay: boolean): AutonomousCapabilityLearningSettlement {
  return { ...structuredClone(value), idempotent_replay: idempotentReplay };
}

const CAPABILITY_LEARNING_SETTLEMENT_KEYS = [
  "schema", "status", "request_digest", "execution_record_digest", "settlement_key", "settlement_digest", "arm_id",
  "evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class",
  "caller_evidence_digest", "outcome_digest", "next_state", "next_state_digest", "idempotent_replay", "retention", "secret_material",
] as const;

const CAPABILITY_LEARNING_RECEIPT_KEYS = [
  "schema", "settlement_key", "request_digest", "execution_record_digest", "evaluator_id", "evaluator_version",
  "settlement_digest", "settlement", "retention", "secret_material",
] as const;

const LEGACY_CAPABILITY_LEARNING_SNAPSHOT_KEYS = ["schema", "receipts", "retention", "secret_material", "snapshot_digest"] as const;
const CAPABILITY_LEARNING_SNAPSHOT_KEYS = ["schema", "snapshot_generation", "previous_snapshot_digest", "receipts", "retention", "secret_material", "snapshot_digest"] as const;

function exactCapabilityLearningKeys(value: JsonObject, allowed: readonly string[], name: string): void {
  const expected = new Set(allowed);
  if (Object.keys(value).some((key) => !expected.has(key)) || allowed.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function capabilityLearningJsonBytes(value: unknown, name: string, maximum: number): void {
  let encoded: string | undefined;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new ArgumentError(`${name} must be JSON serializable`);
  }
  if (typeof encoded !== "string" || bytes(encoded) > maximum) throw new ArgumentError(`${name} exceeds its byte capacity`);
}

async function validateCapabilityLearningSettlement(value: unknown): Promise<AutonomousCapabilityLearningSettlement> {
  if (!isObject(value)) throw new ArgumentError("capability learning settlement must be an object");
  exactCapabilityLearningKeys(value, CAPABILITY_LEARNING_SETTLEMENT_KEYS, "capability learning settlement");
  if (value.schema !== AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA || value.status !== "settled") throw new ArgumentError("capability learning settlement schema is invalid");
  if (value.retention !== "value_only;capability_payloads_excluded" || value.secret_material !== "never_returned") throw new ArgumentError("capability learning settlement retention markers are invalid");
  const requestDigest = digestOrNull("capability learning settlement request_digest", value.request_digest);
  const executionRecordDigest = digestOrNull("capability learning settlement execution_record_digest", value.execution_record_digest);
  const settlementKey = boundedText("capability learning settlement settlement_key", value.settlement_key, 256);
  const settlementDigest = digestOrNull("capability learning settlement settlement_digest", value.settlement_digest);
  const armId = boundedText("capability learning settlement arm_id", value.arm_id, 512);
  const evaluatorId = boundedIdentifier("capability learning settlement evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedIdentifier("capability learning settlement evaluator_version", value.evaluator_version);
  if (typeof value.reward !== "number" || !Number.isFinite(value.reward) || value.reward < -1 || value.reward > 1) throw new ArgumentError("capability learning settlement reward is outside [-1, 1]");
  if (typeof value.passed !== "boolean" || typeof value.failed !== "boolean" || (value.passed && value.failed)) throw new ArgumentError("capability learning settlement outcome flags are contradictory");
  const feedbackDigest = digestOrNull("capability learning settlement feedback_digest", value.feedback_digest);
  const failureClass = value.failure_class === null ? null : boundedIdentifier("capability learning settlement failure_class", value.failure_class);
  const callerEvidenceDigest = digestOrNull("capability learning settlement caller_evidence_digest", value.caller_evidence_digest);
  const outcomeDigest = digestOrNull("capability learning settlement outcome_digest", value.outcome_digest);
  const nextState = await boundedCapabilityLearningMetadata(value.next_state, "capability learning settlement next_state") as BrainBanditState;
  const nextStateDigest = digestOrNull("capability learning settlement next_state_digest", value.next_state_digest);
  if (!nextStateDigest || await digestJson(nextState) !== nextStateDigest) throw new ArgumentError("capability learning settlement next_state_digest does not match next_state");
  if (typeof value.idempotent_replay !== "boolean") throw new ArgumentError("capability learning settlement idempotent_replay must be boolean");
  const { settlement_digest: observed, ...descriptor } = value;
  if (!settlementDigest || await digestJson(descriptor) !== observed) throw new ArgumentError("capability learning settlement digest does not match");
  const normalized = {
    schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA,
    status: "settled" as const,
    request_digest: requestDigest!,
    execution_record_digest: executionRecordDigest!,
    settlement_key: settlementKey,
    settlement_digest: settlementDigest,
    arm_id: armId,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed,
    feedback_digest: feedbackDigest,
    failure_class: failureClass,
    caller_evidence_digest: callerEvidenceDigest!,
    outcome_digest: outcomeDigest!,
    next_state: structuredClone(nextState),
    next_state_digest: nextStateDigest!,
    idempotent_replay: value.idempotent_replay,
    retention: "value_only;capability_payloads_excluded" as const,
    secret_material: "never_returned" as const,
  } satisfies AutonomousCapabilityLearningSettlement;
  capabilityLearningJsonBytes(normalized, "capability learning settlement", MAX_AUTONOMOUS_CAPABILITY_LEARNING_EVIDENCE_BYTES);
  return normalized;
}

/** Validate a persisted capability-learning receipt, including its nested settlement digest. */
export async function validateAutonomousCapabilityLearningSettlementReceipt(value: unknown): Promise<AutonomousCapabilityLearningSettlementReceipt> {
  if (!isObject(value)) throw new ArgumentError("capability learning settlement receipt must be an object");
  exactCapabilityLearningKeys(value, CAPABILITY_LEARNING_RECEIPT_KEYS, "capability learning settlement receipt");
  if (value.schema !== AUTONOMOUS_CAPABILITY_LEARNING_RECEIPT_SCHEMA || value.retention !== "value_only;capability_payloads_excluded" || value.secret_material !== "never_returned") throw new ArgumentError("capability learning settlement receipt markers are invalid");
  const settlementKey = boundedText("capability learning receipt settlement_key", value.settlement_key, 256);
  const requestDigest = digestOrNull("capability learning receipt request_digest", value.request_digest);
  const executionRecordDigest = digestOrNull("capability learning receipt execution_record_digest", value.execution_record_digest);
  const evaluatorId = boundedIdentifier("capability learning receipt evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedIdentifier("capability learning receipt evaluator_version", value.evaluator_version);
  const settlementDigest = digestOrNull("capability learning receipt settlement_digest", value.settlement_digest);
  const settlement = await validateCapabilityLearningSettlement(value.settlement);
  if (settlement.idempotent_replay || settlement.settlement_key !== settlementKey || settlement.request_digest !== requestDigest || settlement.execution_record_digest !== executionRecordDigest || settlement.evaluator_id !== evaluatorId || settlement.evaluator_version !== evaluatorVersion || settlement.settlement_digest !== settlementDigest) throw new ArgumentError("capability learning receipt identity does not match its settlement");
  const normalized = {
    schema: AUTONOMOUS_CAPABILITY_LEARNING_RECEIPT_SCHEMA,
    settlement_key: settlementKey,
    request_digest: requestDigest!,
    execution_record_digest: executionRecordDigest!,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    settlement_digest: settlementDigest!,
    settlement,
    retention: "value_only;capability_payloads_excluded" as const,
    secret_material: "never_returned" as const,
  } satisfies AutonomousCapabilityLearningSettlementReceipt;
  capabilityLearningJsonBytes(normalized, "capability learning settlement receipt", MAX_AUTONOMOUS_CAPABILITY_LEARNING_EVIDENCE_BYTES);
  return normalized;
}

/** Validate and re-hash a complete capability-learning restart image. */
export async function validateAutonomousCapabilityLearningSnapshot(value: unknown): Promise<AutonomousCapabilityLearningSnapshot> {
  if (!isObject(value)) throw new ArgumentError("capability learning snapshot must be an object");
  const legacy = value.schema === LEGACY_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA;
  exactCapabilityLearningKeys(value, legacy ? LEGACY_CAPABILITY_LEARNING_SNAPSHOT_KEYS : CAPABILITY_LEARNING_SNAPSHOT_KEYS, "capability learning snapshot");
  if (value.schema !== AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA && !legacy) throw new ArgumentError("capability learning snapshot schema is unsupported");
  if (value.retention !== "value_only;capability_payloads_excluded" || value.secret_material !== "never_returned") throw new ArgumentError("capability learning snapshot markers are invalid");
  if (!legacy) {
    if (!Number.isSafeInteger(value.snapshot_generation) || (value.snapshot_generation as number) < 1) throw new ArgumentError("capability learning snapshot generation is outside its bound");
    if (value.previous_snapshot_digest !== null && digestOrNull("capability learning previous_snapshot_digest", value.previous_snapshot_digest) === null) throw new ArgumentError("capability learning previous_snapshot_digest is invalid");
    if (((value.snapshot_generation as number) === 1) !== (value.previous_snapshot_digest === null)) throw new ArgumentError("capability learning snapshot generation and previous_snapshot_digest are inconsistent");
  }
  const snapshotDigest = digestOrNull("capability learning snapshot snapshot_digest", value.snapshot_digest);
  if (!Array.isArray(value.receipts) || value.receipts.length > MAX_AUTONOMOUS_CAPABILITY_LEARNING_RECEIPTS) throw new ArgumentError("capability learning snapshot receipt capacity is exhausted");
  const receipts: AutonomousCapabilityLearningSettlementReceipt[] = [];
  const keys = new Set<string>();
  for (const candidate of value.receipts) {
    const receipt = await validateAutonomousCapabilityLearningSettlementReceipt(candidate);
    if (keys.has(receipt.settlement_key)) throw new ArgumentError("capability learning snapshot contains duplicate settlement keys");
    keys.add(receipt.settlement_key);
    receipts.push(receipt);
  }
  const descriptor = legacy
    ? { schema: LEGACY_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA, receipts, retention: "value_only;capability_payloads_excluded" as const, secret_material: "never_returned" as const }
    : { schema: AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA, snapshot_generation: value.snapshot_generation as number, previous_snapshot_digest: value.previous_snapshot_digest as string | null, receipts, retention: "value_only;capability_payloads_excluded" as const, secret_material: "never_returned" as const };
  if (!snapshotDigest || await digestJson(descriptor) !== snapshotDigest) throw new ArgumentError("capability learning snapshot digest does not match");
  const normalized = { ...descriptor, snapshot_digest: snapshotDigest } satisfies AutonomousCapabilityLearningSnapshot;
  capabilityLearningJsonBytes(normalized, "capability learning snapshot", MAX_AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_BYTES);
  return normalized;
}

/** Process-local default store; production callers should provide a durable implementation. */
export class InMemoryAutonomousCapabilityLearningSettlementStore implements AutonomousCapabilityLearningSnapshotStore {
  private readonly receipts = new Map<string, AutonomousCapabilityLearningSettlementReceipt>();
  private snapshotGeneration = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: AutonomousCapabilityLearningSnapshot | null = null;
  private cachedReceiptSignature: string | null = null;

  async load(settlementKey: string): Promise<AutonomousCapabilityLearningSettlementReceipt | null> {
    const key = boundedText("capability settlement key", settlementKey, 256);
    const receipt = this.receipts.get(key);
    return receipt ? validateAutonomousCapabilityLearningSettlementReceipt(structuredClone(receipt)) : null;
  }

  async save(receipt: AutonomousCapabilityLearningSettlementReceipt): Promise<void> {
    const normalized = await validateAutonomousCapabilityLearningSettlementReceipt(receipt);
    const key = boundedText("capability settlement key", normalized.settlement_key, 256);
    const prior = this.receipts.get(key);
    if (prior && (prior.request_digest !== normalized.request_digest || prior.execution_record_digest !== normalized.execution_record_digest || prior.settlement_digest !== normalized.settlement_digest)) throw new ArgumentError(`capability settlement ${key} conflicts with an existing identity`);
    if (!prior && this.receipts.size >= MAX_AUTONOMOUS_CAPABILITY_LEARNING_RECEIPTS) throw new ArgumentError("capability learning settlement store is full");
    if (!prior) {
      this.cachedSnapshot = null;
      this.cachedReceiptSignature = null;
    }
    this.receipts.set(key, structuredClone(normalized));
  }

  async snapshot(): Promise<AutonomousCapabilityLearningSnapshot> {
    const receipts = [] as AutonomousCapabilityLearningSettlementReceipt[];
    for (const receipt of [...this.receipts.values()].sort((left, right) => left.settlement_key.localeCompare(right.settlement_key))) receipts.push(await validateAutonomousCapabilityLearningSettlementReceipt(structuredClone(receipt)));
    const signature = receipts.map((receipt) => `${receipt.settlement_key}:${receipt.settlement_digest}`).join("|");
    if (this.cachedSnapshot !== null && this.cachedReceiptSignature === signature) return structuredClone(this.cachedSnapshot);
    const descriptor = { schema: AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA, snapshot_generation: this.snapshotGeneration + 1, previous_snapshot_digest: this.snapshotGeneration === 0 ? null : this.previousSnapshotDigest, receipts, retention: "value_only;capability_payloads_excluded" as const, secret_material: "never_returned" as const };
    const snapshot = await validateAutonomousCapabilityLearningSnapshot({ ...descriptor, snapshot_digest: await digestJson(descriptor) });
    this.snapshotGeneration = snapshot.snapshot_generation!;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    this.cachedSnapshot = structuredClone(snapshot);
    this.cachedReceiptSignature = signature;
    return structuredClone(snapshot);
  }

  async restore(snapshot: AutonomousCapabilityLearningSnapshot): Promise<void> {
    const validated = await validateAutonomousCapabilityLearningSnapshot(snapshot);
    this.receipts.clear();
    for (const receipt of validated.receipts) this.receipts.set(receipt.settlement_key, structuredClone(receipt));
    this.snapshotGeneration = validated.snapshot_generation ?? 0;
    this.previousSnapshotDigest = this.snapshotGeneration === 0 ? null : validated.snapshot_digest;
    this.cachedSnapshot = validated.schema === AUTONOMOUS_CAPABILITY_LEARNING_SNAPSHOT_SCHEMA ? structuredClone(validated) : null;
    this.cachedReceiptSignature = this.cachedSnapshot === null ? null : validated.receipts.map((receipt) => `${receipt.settlement_key}:${receipt.settlement_digest}`).join("|");
  }
}

/** Coordinates capability-learning restart images with caller-owned durable storage. */
export class AutonomousCapabilityLearningPersistenceCoordinator {
  constructor(readonly store: AutonomousCapabilityLearningSnapshotStore, readonly persistence: AutonomousCapabilityLearningSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("capability learning persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("capability learning persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousCapabilityLearningSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousCapabilityLearningSnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

/** Settle one reviewed capability result using explicit evaluator credit and a caller-owned learner. */
export async function settleAutonomousCapabilityLearning(
  result: AutonomousCapabilityExecutionResult | AutonomousCapabilityExecutionRecord,
  options: AutonomousCapabilityLearningOptions,
): Promise<AutonomousCapabilityLearningSettlement> {
  if (!options || typeof options !== "object") throw new ArgumentError("capability learning options are required");
  if (!options.evaluator || typeof options.evaluator.evaluate !== "function") throw new ArgumentError("capability learning requires an evaluator");
  if (typeof options.recordEvaluatorReward !== "function") throw new ArgumentError("capability learning requires a learner callback");
  const record = await validateAutonomousCapabilityExecutionRecord(capabilityRecordFromResult(result));
  if (record.status === "reconciliation_required" && options.allowReconciliation !== true) throw new ArgumentError("reconciliation_required capability results cannot receive learning credit without explicit reconciliation");
  const callerEvidence = await boundedCapabilityLearningMetadata(options.callerEvidence ?? {}, "capability evaluator caller_evidence");
  const input = await capabilityEvaluationInput(record, callerEvidence);
  const inputDigest = await digestJson(input);
  const settlementKey = boundedText("capability settlement key", options.idempotencyKey ?? await digestJson({ schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA, request_digest: record.request_digest, execution_record_digest: input.execution_record_digest, evaluator_id: options.evaluator.evaluator_id, evaluator_version: options.evaluator.evaluator_version, input_digest: inputDigest }), 256);
  const store = options.settlementStore ?? new InMemoryAutonomousCapabilityLearningSettlementStore();
  if (!store || typeof store.load !== "function" || typeof store.save !== "function") throw new ArgumentError("capability learning settlement store is malformed");
  const loadedPrior = await store.load(settlementKey);
  const prior = loadedPrior ? await validateAutonomousCapabilityLearningSettlementReceipt(loadedPrior) : null;
  if (prior) {
    if (prior.request_digest !== record.request_digest || prior.execution_record_digest !== input.execution_record_digest || prior.evaluator_id !== options.evaluator.evaluator_id || prior.evaluator_version !== options.evaluator.evaluator_version) throw new ArgumentError(`capability settlement ${settlementKey} conflicts with a different identity`);
    return cloneCapabilitySettlement(prior.settlement, true);
  }

  let rawAssessment: AutonomousCapabilityEvaluatorAssessment;
  try {
    rawAssessment = await options.evaluator.evaluate(input);
  } catch (error) {
    void error;
    throw new ArgumentError("capability evaluator callback failed");
  }
  const assessment = validateCapabilityAssessment(rawAssessment, options.evaluator);
  const callerEvidenceDigest = await digestJson(callerEvidence);
  const armId = boundedText("capability learning armId", options.armId ?? `capability:${record.domain}:${record.tool}`, 512);
  const outcomeDigest = await digestJson({ schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA, request_digest: record.request_digest, execution_record_digest: input.execution_record_digest, input_digest: inputDigest, caller_evidence_digest: callerEvidenceDigest, evaluator_id: assessment.evaluator_id, evaluator_version: assessment.evaluator_version, reward: assessment.reward, passed: assessment.passed, failed: assessment.failed, feedback_digest: assessment.feedback_digest ?? null, failure_class: assessment.failure_class ?? null });
  const nextState = await options.recordEvaluatorReward(armId, assessment.reward, { failed: assessment.failed ?? !assessment.passed, outcomeDigest, contractDigest: record.stage_contract_digest, contextDigest: options.contextDigest ?? null, context: options.context });
  if (!isObject(nextState) || !Array.isArray(nextState.arms)) throw new ProviderRuntimeError("capability learning callback returned an invalid bandit state");
  const nextStateDigest = await digestJson(nextState);
  const base = { schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA, status: "settled" as const, request_digest: record.request_digest, execution_record_digest: input.execution_record_digest, settlement_key: settlementKey, arm_id: armId, evaluator_id: assessment.evaluator_id, evaluator_version: assessment.evaluator_version, reward: assessment.reward, passed: assessment.passed, failed: assessment.failed ?? !assessment.passed, feedback_digest: assessment.feedback_digest ?? null, failure_class: assessment.failure_class ?? null, caller_evidence_digest: callerEvidenceDigest, outcome_digest: outcomeDigest, next_state: structuredClone(nextState), next_state_digest: nextStateDigest, idempotent_replay: false, retention: "value_only;capability_payloads_excluded" as const, secret_material: "never_returned" as const };
  const settlement: AutonomousCapabilityLearningSettlement = { ...base, settlement_digest: await digestJson(base) };
  const receipt: AutonomousCapabilityLearningSettlementReceipt = { schema: AUTONOMOUS_CAPABILITY_LEARNING_RECEIPT_SCHEMA, settlement_key: settlementKey, request_digest: record.request_digest, execution_record_digest: input.execution_record_digest, evaluator_id: assessment.evaluator_id, evaluator_version: assessment.evaluator_version, settlement_digest: settlement.settlement_digest, settlement, retention: "value_only;capability_payloads_excluded", secret_material: "never_returned" };
  try {
    await store.save(receipt);
  } catch (error) {
    const loadedObserved = await store.load(settlementKey);
    const observed = loadedObserved ? await validateAutonomousCapabilityLearningSettlementReceipt(loadedObserved) : null;
    if (!observed || observed.settlement_digest !== settlement.settlement_digest) throw error;
    return cloneCapabilitySettlement(observed.settlement, true);
  }
  return structuredClone(settlement);
}

/** Settle capability results in input order; evidence is keyed by request digest or unique call ID. */
export async function settleAutonomousCapabilityLearningBatch(
  results: readonly (AutonomousCapabilityExecutionResult | AutonomousCapabilityExecutionRecord)[],
  options: AutonomousCapabilityLearningBatchOptions,
): Promise<AutonomousCapabilityLearningBatchResult> {
  if (!Array.isArray(results) || results.length < 1 || results.length > MAX_AUTONOMOUS_CAPABILITY_BATCH) throw new ArgumentError(`capability learning batch must contain 1..=${MAX_AUTONOMOUS_CAPABILITY_BATCH} results`);
  const callIds = results.map((item) => (capabilityRecordFromResult(item) as JsonObject).call_id);
  const duplicateCallIds = new Set(callIds).size !== callIds.length;
  const settlements: AutonomousCapabilityLearningSettlement[] = [];
  for (const item of results) {
    const rawRecord = capabilityRecordFromResult(item);
    if (!isObject(rawRecord)) throw new ArgumentError("capability learning batch contains a malformed result");
    const requestDigest = typeof rawRecord.request_digest === "string" ? rawRecord.request_digest : null;
    const callId = typeof rawRecord.call_id === "string" ? rawRecord.call_id : null;
    const evidence = options.evidence?.[requestDigest ?? ""] ?? (!duplicateCallIds && callId ? options.evidence?.[callId] : undefined);
    const armId = options.armIdFor?.(rawRecord as unknown as AutonomousCapabilityExecutionRecord);
    settlements.push(await settleAutonomousCapabilityLearning(item, { ...options, callerEvidence: evidence, ...(armId === undefined ? {} : { armId }) }));
  }
  return { schema: AUTONOMOUS_CAPABILITY_LEARNING_SETTLEMENT_SCHEMA, status: "settled", settlements, batch_digest: await digestJson(settlements.map((settlement) => settlement.settlement_digest)), retention: "value_only;capability_payloads_excluded", secret_material: "never_returned" };
}

/**
 * Structured execution above the reviewed-stage tool runtime.
 *
 * The underlying runtime remains the only dispatch authority. This layer adds identity,
 * replay, observation projection, and evaluator-facing evidence without pretending that a
 * successful function call is an autonomous conclusion.
 */
export class AutonomousCapabilityRuntime {
  private readonly runtime: AutonomousDomainToolRuntime;
  private readonly admitTool?: (tool: string) => boolean | string;
  private readonly journal?: AutonomousCapabilityJournalStore;
  private readonly cache = new Map<string, CachedExecution>();
  private readonly inFlight = new Map<string, Promise<AutonomousCapabilityExecutionResult>>();
  private readonly rehydratedByRequest = new Map<string, AutonomousCapabilityExecutionRecord>();
  private readonly rehydratedByReplayKey = new Map<string, AutonomousCapabilityExecutionRecord>();
  private readonly history: AutonomousCapabilityExecutionRecord[] = [];

  constructor(runtime: AutonomousDomainToolRuntime, options: { admitTool?: (tool: string) => boolean | string; journal?: AutonomousCapabilityJournalStore } = {}) {
    if (!runtime || typeof runtime.authorizeAndExecute !== "function" || !runtime.registry) throw new ProviderRuntimeError("autonomous capability runtime requires a domain tool runtime");
    if (options.journal !== undefined && (typeof options.journal.append !== "function" || typeof options.journal.find !== "function" || typeof options.journal.records !== "function")) throw new ProviderRuntimeError("autonomous capability journal is malformed");
    this.runtime = runtime;
    this.admitTool = options.admitTool;
    this.journal = options.journal;
  }

  static async refusal(request: AutonomousCapabilityExecutionRequest, reason: string): Promise<AutonomousCapabilityExecutionResult> {
    const normalized = normalizeRequest(request);
    return makeRefusal(normalized, reason);
  }

  async execute(request: AutonomousCapabilityExecutionRequest, options: AutonomousCapabilityExecutionOptions = {}): Promise<AutonomousCapabilityExecutionResult> {
    const normalized = normalizeRequest(request);
    const requestDigest = await this.requestDigest(normalized);
    const replayKeyDigest = normalized.replay_key === null ? null : await digestJson(normalized.replay_key);
    const cacheKey = replayKeyDigest ?? requestDigest;
    const pending = this.inFlight.get(cacheKey);
    if (pending) {
      const result = await pending;
      if (result.record.request_digest !== requestDigest) throw new ProviderRuntimeError("capability replay key collides with different in-flight request metadata");
      return copyResult(result, "replayed");
    }
    const execution = this.executeFresh(request, options);
    this.inFlight.set(cacheKey, execution);
    try {
      return await execution;
    } finally {
      if (this.inFlight.get(cacheKey) === execution) this.inFlight.delete(cacheKey);
    }
  }

  private async executeFresh(request: AutonomousCapabilityExecutionRequest, options: AutonomousCapabilityExecutionOptions = {}): Promise<AutonomousCapabilityExecutionResult> {
    const normalized = normalizeRequest(request);
    const argumentsDigest = await digestJson(normalized.arguments);
    const replayKeyDigest = normalized.replay_key === null ? null : await digestJson(normalized.replay_key);
    const requestDigest = await digestJson({
      schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
      call_id: normalized.call_id,
      tool: normalized.tool,
      arguments_digest: argumentsDigest,
      workflow_context: normalized.workflow_context,
      input_digest: normalized.input_digest,
      subject_digest: normalized.subject_digest,
      parent_evidence_digests: normalized.parent_evidence_digests,
      replay_key_digest: replayKeyDigest,
      execution_id: normalized.execution_id,
    });
    const cacheKey = replayKeyDigest ?? requestDigest;
    const cached = this.cache.get(cacheKey);
    if (cached) {
      if (cached.request_digest !== requestDigest) throw new ProviderRuntimeError("capability replay key collides with different request metadata");
      return copyResult(cached.result, "replayed");
    }
    const rehydrated = (replayKeyDigest === null ? undefined : this.rehydratedByReplayKey.get(replayKeyDigest)) ?? this.rehydratedByRequest.get(requestDigest);
    if (rehydrated) {
      if (rehydrated.request_digest !== requestDigest) throw new ProviderRuntimeError("rehydrated capability replay key collides with different request metadata");
      const result: AutonomousCapabilityExecutionResult = {
        schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
        record: rehydrated,
        value: null,
        value_retention: "transient_caller_value_only",
        secret_material: "never_returned",
      };
      return this.recordResult(copyResult(result, "replayed"), false);
    }

    const started = Date.now();
    const admission = this.admitTool?.(normalized.tool);
    if (admission !== undefined && admission !== true) return this.recordResult(await makeRefusal(normalized, typeof admission === "string" ? admission : "activation_required", requestDigest, argumentsDigest, replayKeyDigest, started));

    let planned: ReturnType<AutonomousDomainToolRuntime["registry"]["stagePlan"]>;
    try {
      planned = this.runtime.registry.stagePlan(normalized.tool, normalized.arguments, normalized.workflow_context);
    } catch (error) {
      const reason = error instanceof Error ? error.constructor.name : "capability_admission_refused";
      return this.recordResult(await makeRefusal(normalized, reason, requestDigest, argumentsDigest, replayKeyDigest, started));
    }

    const call: ProviderToolCall = { id: normalized.call_id, name: normalized.tool, arguments: normalized.arguments };
    const beforeReceipts = this.runtime.receiptsSnapshot().length;
    let response: ProviderToolResult;
    try {
      const responses = await this.runtime.authorizeAndExecute([call], {
        domains: [normalized.workflow_context.domain],
        approveEffects: options.approveEffects,
        execution: options.execution,
        effectBoundary: options.effectBoundary,
        workflowContext: normalized.workflow_context,
      });
      response = responses[0] ?? { callId: normalized.call_id, approved: false, isError: true, content: { status: "execution_failed", reason: "missing_tool_result" } };
    } catch (error) {
      return this.recordResult(await makeRefusal(normalized, error instanceof Error ? error.constructor.name : "capability_runtime_failure", requestDigest, argumentsDigest, replayKeyDigest, started, planned));
    }
    const receipt = this.runtime.receiptsSnapshot().slice(beforeReceipts)[0];
    const status = resultStatus(response, receipt);
    if (status !== "completed") {
      const record = this.recordFromFailure(normalized, requestDigest, argumentsDigest, replayKeyDigest, planned.binding, receipt, status, started);
      return this.recordResult({ schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, record, value: null, value_retention: "transient_caller_value_only", secret_material: "never_returned" });
    }

    const value = response.content as JsonValue;
    const outputEncoded = canonicalJson(value);
    const outputDigest = await digestJson(value);
    let observations: AutonomousCapabilityObservation[] = [];
    let evidenceStatus: AutonomousCapabilityEvidenceStatus = "missing_required_outputs";
    let projectionFailure: string | null = null;
    if (options.projectObservations) {
      try {
        const projected = await options.projectObservations(value, request);
        if (!Array.isArray(projected) || projected.length > MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS) throw new ArgumentError(`capability observations must contain at most ${MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS} entries`);
        observations = projected.map((observation, index) => normalizeObservation(observation, index));
        const labels = new Set(observations.map((observation) => observation.label));
        const missing = planned.stage.evidence_outputs.filter((label) => !labels.has(label));
        evidenceStatus = missing.length ? "missing_required_outputs" : "declared_for_evaluator";
      } catch (error) {
        projectionFailure = error instanceof Error ? error.constructor.name : "observation_projection_failed";
        evidenceStatus = "projection_failed";
      }
    }
    const missingEvidenceOutputs = evidenceStatus === "projection_failed" || !options.projectObservations
      ? [...planned.stage.evidence_outputs]
      : planned.stage.evidence_outputs.filter((label) => !new Set(observations.map((observation) => observation.label)).has(label));
    const evidenceDigest = await digestJson({
      schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
      request_digest: requestDigest,
      input_digest: normalized.input_digest,
      arguments_digest: argumentsDigest,
      output_digest: outputDigest,
      required_evidence_outputs: planned.stage.evidence_outputs,
      observations,
      evidence_status: evidenceStatus,
    });
    const record: AutonomousCapabilityExecutionRecord = {
      schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
      record_kind: "capability_execution_record",
      request_digest: requestDigest,
      execution_id: normalized.execution_id,
      call_id: normalized.call_id,
      domain: normalized.workflow_context.domain,
      workflow_id: normalized.workflow_context.workflow_id,
      workflow_digest: normalized.workflow_context.workflow_digest,
      stage_id: normalized.workflow_context.stage_id,
      stage_contract_digest: receipt?.stage_contract_digest ?? null,
      tool: normalized.tool,
      capability: planned.binding.capability,
      risk_class: planned.binding.risk_class,
      schema_digest: receipt?.schema_digest ?? planned.schemaDigest,
      input_digest: normalized.input_digest,
      subject_digest: normalized.subject_digest,
      parent_evidence_digests: [...normalized.parent_evidence_digests],
      arguments_digest: argumentsDigest,
      replay_key_digest: replayKeyDigest,
      status: "completed",
      replay: "fresh",
      output_digest: outputDigest,
      output_bytes: bytes(outputEncoded),
      observations,
      evidence_digest: evidenceDigest,
      evidence_status: evidenceStatus,
      required_evidence_outputs: [...planned.stage.evidence_outputs],
      missing_evidence_outputs: [...missingEvidenceOutputs],
      limitations: projectionFailure ? ["observation projection failed", projectionFailure] : ["raw adapter output is transient and not part of the durable record"],
      effect: receipt?.effect ?? planned.binding.risk_class,
      effect_id: receipt?.effect_id ?? null,
      error_class: null,
      duration_ms: Math.max(0, Date.now() - started),
      does_not_claim: commonDoesNotClaim(),
      secret_material: "never_returned",
    };
    const result: AutonomousCapabilityExecutionResult = { schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, record, value, value_retention: "transient_caller_value_only", secret_material: "never_returned" };
    this.cache.set(cacheKey, { request_digest: requestDigest, result });
    this.trimCache();
    return this.recordResult(result);
  }

  async executeBatch(requests: readonly AutonomousCapabilityExecutionRequest[], options: AutonomousCapabilityBatchOptions = {}): Promise<AutonomousCapabilityBatchResult> {
    if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_CAPABILITY_BATCH) throw new ArgumentError(`capability batch must contain 1..=${MAX_AUTONOMOUS_CAPABILITY_BATCH} requests`);
    if (options.maxConcurrency !== undefined && options.maxConcurrency !== 1) throw new ArgumentError("capability batches currently require maxConcurrency: 1 to preserve reviewed stage ordering and effect visibility");
    const items: AutonomousCapabilityBatchItem[] = [];
    let failed = false;
    for (let index = 0; index < requests.length; index += 1) {
      const request = requests[index]!;
      const normalized = normalizeRequest(request);
      const requestDigest = await this.requestDigest(normalized);
      if (failed && options.stopOnFailure === true) {
        items.push({ index, request_digest: requestDigest, result: null, omission_reason: "stopped_after_failure" });
        continue;
      }
      const result = await this.execute(request, options);
      items.push({ index, request_digest: result.record.request_digest, result, omission_reason: null });
      if (["failed", "refused", "reconciliation_required", "approval_required"].includes(result.record.status)) failed = true;
    }
    const completedCount = items.filter((item) => item.result?.record.status === "completed").length;
    const failedCount = items.filter((item) => item.result !== null && item.result.record.status !== "completed").length;
    const omittedCount = items.filter((item) => item.omission_reason !== null).length;
    const descriptor = { schema: AUTONOMOUS_CAPABILITY_BATCH_SCHEMA, item_records: items.map((item) => ({ index: item.index, request_digest: item.request_digest, record: item.result?.record ?? null, omission_reason: item.omission_reason })), completed_count: completedCount, failed_count: failedCount, omitted_count: omittedCount, execution: "ordered_serial" as const };
    return { schema: AUTONOMOUS_CAPABILITY_BATCH_SCHEMA, batch_digest: await digestJson(descriptor), status: failed || omittedCount ? "partial" : "completed", items, completed_count: completedCount, failed_count: failedCount, omitted_count: omittedCount, execution: "ordered_serial", durable_projection: "records_and_digests_only", secret_material: "never_returned" };
  }

  executionEvidence(): AutonomousCapabilityExecutionRecord[] {
    return this.history.map((record) => ({ ...record, observations: record.observations.map((observation) => ({ ...observation, limitations: [...observation.limitations] })), required_evidence_outputs: [...record.required_evidence_outputs], missing_evidence_outputs: [...record.missing_evidence_outputs], parent_evidence_digests: [...record.parent_evidence_digests], limitations: [...record.limitations], does_not_claim: [...record.does_not_claim] }));
  }

  /** Rebuild metadata-only replay indexes after a process restart without redispatching tools. */
  async rehydrate(): Promise<{ restored: number; replayable: number; value_retention: "transient_caller_value_only" }> {
    if (!this.journal) return { restored: 0, replayable: 0, value_retention: "transient_caller_value_only" };
    const rawRecords = await this.journal.records();
    if (!Array.isArray(rawRecords) || rawRecords.length > 4_096) throw new ProviderRuntimeError("capability journal returned too many records");
    const records = await Promise.all(rawRecords.map((record) => validateAutonomousCapabilityExecutionRecord(record)));
    this.rehydratedByRequest.clear();
    this.rehydratedByReplayKey.clear();
    this.cache.clear();
    this.history.length = 0;
    for (const record of records) {
      if (record.replay !== "fresh") throw new ProviderRuntimeError("capability journal returned a replayed record");
      const prior = this.rehydratedByRequest.get(record.request_digest);
      if (prior && await digestJson(prior) !== await digestJson(record)) throw new ProviderRuntimeError("capability journal contains conflicting request metadata");
      this.rehydratedByRequest.delete(record.request_digest);
      if (record.replay_key_digest !== null) this.rehydratedByReplayKey.delete(record.replay_key_digest);
      if (["completed", "reconciliation_required"].includes(record.status)) {
        this.rehydratedByRequest.set(record.request_digest, cloneRecord(record));
        if (record.replay_key_digest !== null) {
          const priorKey = this.rehydratedByReplayKey.get(record.replay_key_digest);
          if (priorKey && priorKey.request_digest !== record.request_digest) throw new ProviderRuntimeError("capability journal contains a replay-key collision");
          this.rehydratedByReplayKey.set(record.replay_key_digest, cloneRecord(record));
        }
      }
      this.history.push(cloneRecord(record));
    }
    while (this.history.length > MAX_AUTONOMOUS_CAPABILITY_HISTORY) this.history.shift();
    return { restored: records.length, replayable: this.rehydratedByRequest.size, value_retention: "transient_caller_value_only" };
  }

  private async requestDigest(request: NormalizedRequest): Promise<string> {
    return digestJson({ schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, call_id: request.call_id, tool: request.tool, arguments_digest: await digestJson(request.arguments), workflow_context: request.workflow_context, input_digest: request.input_digest, subject_digest: request.subject_digest, parent_evidence_digests: request.parent_evidence_digests, replay_key_digest: request.replay_key === null ? null : await digestJson(request.replay_key), execution_id: request.execution_id });
  }

  private async recordResult(result: AutonomousCapabilityExecutionResult, persist = true): Promise<AutonomousCapabilityExecutionResult> {
    if (persist && this.journal && result.record.replay === "fresh") await this.journal.append(result.record);
    this.history.push(result.record);
    while (this.history.length > MAX_AUTONOMOUS_CAPABILITY_HISTORY) this.history.shift();
    return result;
  }

  private trimCache(): void {
    while (this.cache.size > MAX_AUTONOMOUS_CAPABILITY_HISTORY) {
      const first = this.cache.keys().next().value as string | undefined;
      if (first === undefined) break;
      this.cache.delete(first);
    }
  }

  private recordFromFailure(request: NormalizedRequest, requestDigest: string, argumentsDigest: string, replayKeyDigest: string | null, binding: AutonomousDomainToolBinding, receipt: AutonomousDomainToolExecutionReceipt | undefined, status: Exclude<AutonomousCapabilityExecutionStatus, "completed">, started: number): AutonomousCapabilityExecutionRecord {
    return {
      schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
      record_kind: "capability_execution_record",
      request_digest: requestDigest,
      execution_id: request.execution_id,
      call_id: request.call_id,
      domain: request.workflow_context.domain,
      workflow_id: request.workflow_context.workflow_id,
      workflow_digest: request.workflow_context.workflow_digest,
      stage_id: request.workflow_context.stage_id,
      stage_contract_digest: receipt?.stage_contract_digest ?? null,
      tool: request.tool,
      capability: binding.capability,
      risk_class: binding.risk_class,
      schema_digest: receipt?.schema_digest ?? null,
      input_digest: request.input_digest,
      subject_digest: request.subject_digest,
      parent_evidence_digests: [...request.parent_evidence_digests],
      arguments_digest: argumentsDigest,
      replay_key_digest: replayKeyDigest,
      status,
      replay: "fresh",
      output_digest: null,
      output_bytes: 0,
      observations: [],
      evidence_digest: null,
      evidence_status: "not_evaluated",
      required_evidence_outputs: receipt?.required_evidence_outputs ?? [],
      missing_evidence_outputs: receipt?.required_evidence_outputs ?? [],
      limitations: ["capability did not produce a durable success observation"],
      effect: receipt?.effect ?? binding.risk_class,
      effect_id: receipt?.effect_id ?? null,
      error_class: receipt?.error_class ?? null,
      duration_ms: Math.max(0, Date.now() - started),
      does_not_claim: commonDoesNotClaim(),
      secret_material: "never_returned",
    };
  }
}

async function makeRefusal(request: NormalizedRequest, reason: string, requestDigest?: string, argumentsDigest?: string, replayKeyDigest?: string | null, started = Date.now(), planned?: ReturnType<AutonomousDomainToolRuntime["registry"]["stagePlan"]>): Promise<AutonomousCapabilityExecutionResult> {
  const argumentDigest = argumentsDigest ?? await digestJson(request.arguments);
  const normalizedReplayKeyDigest = replayKeyDigest === undefined ? request.replay_key === null ? null : await digestJson(request.replay_key) : replayKeyDigest;
  const normalizedRequestDigest = requestDigest ?? await digestJson({ schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, call_id: request.call_id, tool: request.tool, arguments_digest: argumentDigest, workflow_context: request.workflow_context, input_digest: request.input_digest, subject_digest: request.subject_digest, parent_evidence_digests: request.parent_evidence_digests, replay_key_digest: normalizedReplayKeyDigest, execution_id: request.execution_id });
  const record: AutonomousCapabilityExecutionRecord = {
    schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
    record_kind: "capability_execution_record",
    request_digest: normalizedRequestDigest,
    execution_id: request.execution_id,
    call_id: request.call_id,
    domain: request.workflow_context.domain,
    workflow_id: request.workflow_context.workflow_id,
    workflow_digest: request.workflow_context.workflow_digest,
    stage_id: request.workflow_context.stage_id,
    stage_contract_digest: null,
    tool: request.tool,
    capability: planned?.binding.capability ?? null,
    risk_class: planned?.binding.risk_class ?? null,
    schema_digest: planned?.schemaDigest ?? null,
    input_digest: request.input_digest,
    subject_digest: request.subject_digest,
    parent_evidence_digests: [...request.parent_evidence_digests],
    arguments_digest: argumentDigest,
    replay_key_digest: normalizedReplayKeyDigest,
    status: "refused",
    replay: "fresh",
    output_digest: null,
    output_bytes: 0,
    observations: [],
    evidence_digest: null,
    evidence_status: "not_evaluated",
    required_evidence_outputs: planned?.stage.evidence_outputs ? [...planned.stage.evidence_outputs] : [],
    missing_evidence_outputs: planned?.stage.evidence_outputs ? [...planned.stage.evidence_outputs] : [],
    limitations: [reason],
    effect: planned?.binding.risk_class ?? null,
    effect_id: null,
    error_class: reason,
    duration_ms: Math.max(0, Date.now() - started),
    does_not_claim: commonDoesNotClaim(),
    secret_material: "never_returned",
  };
  return { schema: AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, record, value: null, value_retention: "transient_caller_value_only", secret_material: "never_returned" };
}

/** Create a structured refusal when an agent has no live catalogue/runtime configured. */
export async function autonomousCapabilityRefusal(request: AutonomousCapabilityExecutionRequest, reason: string): Promise<AutonomousCapabilityExecutionResult> {
  return makeRefusal(normalizeRequest(request), boundedText("capability refusal reason", reason, 256));
}
