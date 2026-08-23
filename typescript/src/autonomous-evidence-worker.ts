import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousDomainName,
} from "./autonomous.js";
import {
  AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
  AutonomousEvidenceRuntime,
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceRuntimeExecuteOptions,
  type AutonomousEvidenceRuntimeResult,
} from "./autonomous-evidence-runtime.js";
import { AutonomousEvidencePlan, type AutonomousEvidencePlanJSON } from "./autonomous-evidence.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Durable metadata-only orchestration for caller-owned evidence runtimes. */
export const AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA = "bioprism-typescript-autonomous-evidence-work-item/0.2" as const;
export const AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA = "bioprism-typescript-autonomous-evidence-work-queue/0.2" as const;
export const AUTONOMOUS_EVIDENCE_WORKER_SCHEMA = "bioprism-typescript-autonomous-evidence-worker/0.2" as const;
const LEGACY_AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA = "bioprism-typescript-autonomous-evidence-work-item/0.1" as const;
const LEGACY_AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA = "bioprism-typescript-autonomous-evidence-work-queue/0.1" as const;
const AUTONOMOUS_EVIDENCE_WORK_ACCEPTANCE_SCHEMA = "bioprism-typescript-autonomous-evidence-work-acceptance/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS = 4_096;
export const MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH = 128;
export const MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS = 600_000;
export const MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES = 8_000_000;

export type AutonomousEvidenceWorkStatus =
  | "queued"
  | "leased"
  | "completed"
  | "failed"
  | "awaiting_evaluation"
  | "reconciliation_required"
  | "cancelled";

export type AutonomousEvidenceWorkFailureClass =
  | "rehydration_missing"
  | "rehydration_invalid"
  | "identity_conflict"
  | "lease_expired"
  | "acquisition_failed"
  | "projection_failed"
  | "evaluation_pending"
  | "evaluation_rejected"
  | "result_reconciliation_required"
  | "result_invalid"
  | "executor_error"
  | "transport_error"
  | "unknown";

const WORK_FAILURE_CLASSES: readonly (AutonomousEvidenceWorkFailureClass | null)[] = [
  null,
  "rehydration_missing",
  "rehydration_invalid",
  "identity_conflict",
  "lease_expired",
  "acquisition_failed",
  "projection_failed",
  "evaluation_pending",
  "evaluation_rejected",
  "result_reconciliation_required",
  "result_invalid",
  "executor_error",
  "transport_error",
  "unknown",
];

const WORK_STATUSES: readonly AutonomousEvidenceWorkStatus[] = [
  "queued",
  "leased",
  "completed",
  "failed",
  "awaiting_evaluation",
  "reconciliation_required",
  "cancelled",
];

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+\- /]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function timestamp(name: string, value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 8_640_000_000_000_000) throw new ArgumentError(`${name} must be a bounded epoch millisecond timestamp`);
  return value as number;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer between ${minimum} and ${maximum}`);
  return value as number;
}

function domain(name: string, value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(value)) throw new ArgumentError(`${name} is not a supported autonomous domain`);
  return value as AutonomousDomainName;
}

function digests(name: string, value: unknown, maximum = 128): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must contain at most ${maximum} entries`);
  const result = value.map((entry, index) => digest(`${name}[${index}]`, entry) as string);
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
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "credentials", "password", "privatekey", "refreshtoken", "secret", "token"].includes(normalized)) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function normalizeRequest(request: AutonomousEvidenceAcquisitionRequest): AutonomousEvidenceAcquisitionRequest {
  if (!isObject(request)) throw new ArgumentError("evidence work request must be an object");
  const requirementId = identifier("evidence work request requirement_id", request.requirement_id);
  const sourceId = identifier("evidence work request source_id", request.source_id);
  const sourceDigest = digest("evidence work request source_digest", request.source_digest, true);
  const requestId = request.request_id === undefined || request.request_id === null ? null : identifier("evidence work request request_id", request.request_id);
  const metadata = request.metadata === undefined ? {} : request.metadata;
  if (!isObject(metadata)) throw new ArgumentError("evidence work request metadata must be an object");
  assertMetadata(metadata, "evidence work request metadata");
  if (bytes(canonicalJson(metadata)) > 64_000) throw new ArgumentError("evidence work request metadata exceeds its byte bound");
  return { requirement_id: requirementId, source_id: sourceId, source_digest: sourceDigest, request_id: requestId, metadata: clone(metadata) };
}

function requestDigest(planDigest: string, request: AutonomousEvidenceAcquisitionRequest): string {
  const normalized = normalizeRequest(request);
  return digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    plan_digest: planDigest,
    requirement_id: normalized.requirement_id,
    source_id: normalized.source_id,
    source_digest: normalized.source_digest ?? null,
    request_id: normalized.request_id ?? null,
    metadata: normalized.metadata ?? {},
  });
}

function planFor(value: AutonomousEvidencePlan | AutonomousEvidencePlanJSON): AutonomousEvidencePlan {
  if (value instanceof AutonomousEvidencePlan) return value;
  if (!isObject(value) || value.schema !== "bioprism-typescript-autonomous-evidence-plan/0.1") throw new ArgumentError("evidence work plan is malformed");
  return new AutonomousEvidencePlan({
    domains: value.domains as AutonomousDomainName[],
    workflow_ids: value.workflow_ids as string[],
    workflow_digests: value.workflow_digests as string[],
    requirements: value.requirements as never[],
    available_evidence: value.available_evidence as string[],
    covered_requirement_ids: value.covered_requirement_ids as string[],
    missing_requirement_ids: value.missing_requirement_ids as string[],
    next_stage_ids: value.next_stage_ids as string[],
    coverage_status: value.coverage_status as "not_evaluated" | "missing" | "partial" | "complete",
    plan_digest: value.plan_digest as string,
  });
}

function requirement(plan: AutonomousEvidencePlan, requirementId: string) {
  const row = plan.requirements.find((candidate) => candidate.requirement_id === requirementId);
  if (!row) throw new ArgumentError(`evidence work requirement is not in the plan: ${requirementId}`);
  return row;
}

export interface AutonomousEvidenceWorkItem extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA;
  work_id: string;
  plan_digest: string;
  requirement_id: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  source_id: string;
  source_digest: string | null;
  request_digest: string;
  parent_evidence_digests: string[];
  max_attempts: number;
  attempts: number;
  status: AutonomousEvidenceWorkStatus;
  available_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  receipt_digest: string | null;
  assessment_digest: string | null;
  result_digest: string | null;
  acceptance_digest: string | null;
  failure_class: AutonomousEvidenceWorkFailureClass | null;
  last_error_class: AutonomousEvidenceWorkFailureClass | null;
  created_at: number;
  updated_at: number;
  item_digest: string;
  retention: "metadata_only_request_and_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceWorkQueueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA;
  items: AutonomousEvidenceWorkItem[];
  snapshot_digest: string;
  retention: "metadata_only_request_and_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceWorkQueuePersistence {
  read(): Promise<AutonomousEvidenceWorkQueueSnapshot | null> | AutonomousEvidenceWorkQueueSnapshot | null;
  write(snapshot: AutonomousEvidenceWorkQueueSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousEvidenceWorkQueueSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceWorkQueueSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceWorkQueueTransactionalSnapshotTextStore extends AutonomousEvidenceWorkQueueSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

function itemPayload(item: AutonomousEvidenceWorkItem): JsonObject {
  const { item_digest: _itemDigest, ...payload } = item;
  return payload;
}

function itemDigest(item: AutonomousEvidenceWorkItem): string {
  return digestJsonSync(itemPayload(item));
}

function workFailure(value: unknown): AutonomousEvidenceWorkFailureClass {
  return (WORK_FAILURE_CLASSES as readonly unknown[]).includes(value) && value !== null ? value as AutonomousEvidenceWorkFailureClass : "unknown";
}

function validateItem(raw: unknown): AutonomousEvidenceWorkItem {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA) throw new ArgumentError("autonomous evidence work item is malformed");
  if (raw.retention !== "metadata_only_request_and_values_caller_owned" || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous evidence work item retention is invalid");
  if (!(WORK_STATUSES as readonly unknown[]).includes(raw.status)) throw new ArgumentError("autonomous evidence work item status is invalid");
  const item = {
    schema: AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA,
    work_id: identifier("autonomous evidence work_id", raw.work_id),
    plan_digest: digest("autonomous evidence work plan_digest", raw.plan_digest) as string,
    requirement_id: identifier("autonomous evidence work requirement_id", raw.requirement_id),
    domain: domain("autonomous evidence work domain", raw.domain),
    workflow_id: identifier("autonomous evidence work workflow_id", raw.workflow_id),
    workflow_digest: digest("autonomous evidence work workflow_digest", raw.workflow_digest) as string,
    stage_id: identifier("autonomous evidence work stage_id", raw.stage_id),
    source_id: identifier("autonomous evidence work source_id", raw.source_id),
    source_digest: digest("autonomous evidence work source_digest", raw.source_digest, true),
    request_digest: digest("autonomous evidence work request_digest", raw.request_digest) as string,
    parent_evidence_digests: digests("autonomous evidence work parent_evidence_digests", raw.parent_evidence_digests, 64),
    max_attempts: boundedInteger("autonomous evidence work max_attempts", raw.max_attempts, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS),
    attempts: boundedInteger("autonomous evidence work attempts", raw.attempts, 0, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS),
    status: raw.status as AutonomousEvidenceWorkStatus,
    available_at: timestamp("autonomous evidence work available_at", raw.available_at),
    lease_owner: raw.lease_owner === null ? null : identifier("autonomous evidence work lease_owner", raw.lease_owner),
    lease_until: raw.lease_until === null ? null : timestamp("autonomous evidence work lease_until", raw.lease_until),
    receipt_digest: digest("autonomous evidence work receipt_digest", raw.receipt_digest, true),
    assessment_digest: digest("autonomous evidence work assessment_digest", raw.assessment_digest, true),
    result_digest: digest("autonomous evidence work result_digest", raw.result_digest, true),
    acceptance_digest: digest("autonomous evidence work acceptance_digest", raw.acceptance_digest, true),
    failure_class: raw.failure_class === null ? null : workFailure(raw.failure_class),
    last_error_class: raw.last_error_class === null ? null : workFailure(raw.last_error_class),
    created_at: timestamp("autonomous evidence work created_at", raw.created_at),
    updated_at: timestamp("autonomous evidence work updated_at", raw.updated_at),
    item_digest: digest("autonomous evidence work item_digest", raw.item_digest) as string,
    retention: "metadata_only_request_and_values_caller_owned" as const,
    secret_material: "never_returned" as const,
  } satisfies AutonomousEvidenceWorkItem;
  if (item.attempts > item.max_attempts || (item.status === "leased") !== (item.lease_owner !== null && item.lease_until !== null)) throw new ArgumentError("autonomous evidence work lease state is inconsistent");
  if (item.status !== "awaiting_evaluation" && item.status !== "reconciliation_required" && item.status !== "failed" && item.status !== "cancelled" && item.failure_class !== null) throw new ArgumentError("autonomous evidence work active item cannot retain a terminal failure class");
  if ((item.status === "completed") !== (item.acceptance_digest !== null)) throw new ArgumentError("autonomous evidence work completion must retain an acceptance digest");
  if (item.item_digest !== itemDigest(item)) throw new ArgumentError("autonomous evidence work item digest is invalid");
  return item;
}

function migrateLegacyItem(raw: unknown): AutonomousEvidenceWorkItem {
  if (!isObject(raw) || raw.schema !== LEGACY_AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA) throw new ArgumentError("autonomous evidence legacy work item is malformed");
  const { item_digest: observed, ...legacyPayload } = raw;
  if (typeof observed !== "string" || digestJsonSync(legacyPayload) !== observed) throw new ArgumentError("autonomous evidence legacy work item digest is invalid");
  const upgraded = { ...raw, schema: AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA, acceptance_digest: null, item_digest: "" } as unknown as AutonomousEvidenceWorkItem;
  if (upgraded.status === "completed") {
    upgraded.status = "reconciliation_required";
    upgraded.failure_class = "result_reconciliation_required";
    upgraded.last_error_class = "result_reconciliation_required";
    upgraded.lease_owner = null;
    upgraded.lease_until = null;
  }
  upgraded.item_digest = itemDigest(upgraded);
  return validateItem(upgraded);
}

function refresh(item: AutonomousEvidenceWorkItem, updates: Partial<AutonomousEvidenceWorkItem>, now: number): AutonomousEvidenceWorkItem {
  const next = { ...item, ...updates, updated_at: now, item_digest: "" } as AutonomousEvidenceWorkItem;
  next.item_digest = itemDigest(next);
  return next;
}

function nowMs(value: number | undefined): number {
  return timestamp("time", value ?? Date.now());
}

function resultMetadata(item: AutonomousEvidenceWorkItem, result: AutonomousEvidenceRuntimeResult): { resultDigest: string; receiptDigest: string; assessmentDigest: string | null; receiptStatus: string; evaluatorStatus: string; replay: "fresh" | "replayed"; acceptanceDigest: string | null } {
  if (!result || typeof result.toJSON !== "function") throw new ArgumentError("evidence work result must be a typed runtime result");
  const json = result.toJSON();
  if (!isObject(json) || json.schema !== "bioprism-typescript-autonomous-evidence-runtime/0.1") throw new ArgumentError("evidence work result schema is invalid");
  if (!isObject(json.plan) || !Array.isArray(json.receipts)) throw new ArgumentError("evidence work result plan identity is stale");
  const receipt = json.receipts.find((candidate) => isObject(candidate) && candidate.request_digest === item.request_digest);
  if (!isObject(receipt) || typeof receipt.receipt_digest !== "string") throw new ArgumentError("evidence work result does not contain the queued request");
  if (receipt.plan_digest !== item.plan_digest || receipt.requirement_id !== item.requirement_id || receipt.domain !== item.domain || receipt.workflow_id !== item.workflow_id || receipt.workflow_digest !== item.workflow_digest || receipt.stage_id !== item.stage_id || receipt.source_id !== item.source_id || receipt.source_digest !== item.source_digest) throw new ArgumentError("evidence work result receipt identity conflicts with the queued request");
  if (receipt.replay !== "fresh" && receipt.replay !== "replayed") throw new ArgumentError("evidence work result receipt replay state is invalid");
  const resultDigest = digest("evidence work result result_digest", json.result_digest) as string;
  const resultDescriptor = {
    schema: json.schema,
    status: json.status,
    plan_digest: json.plan.plan_digest,
    receipt_digests: json.receipts.map((candidate) => isObject(candidate) ? candidate.receipt_digest : null),
    assessment_digests: Array.isArray(json.assessments) ? json.assessments.map((candidate) => isObject(candidate) ? candidate.assessment_digest : null) : [],
    completed_requirement_ids: json.completed_requirement_ids,
    pending_evaluation_requirement_ids: json.pending_evaluation_requirement_ids,
    missing_requirement_ids: json.missing_requirement_ids,
    next_stage_ids: json.next_stage_ids,
    omitted_request_digests: json.omitted_request_digests,
    retention: "metadata_only;raw_values_caller_owned",
    secret_material: "never_returned",
  };
  if (digestJsonSync(resultDescriptor) !== resultDigest) throw new ArgumentError("evidence work result digest is invalid");
  const assessment = Array.isArray(json.assessments) ? json.assessments.find((candidate) => isObject(candidate) && candidate.requirement_id === item.requirement_id) : undefined;
  const receiptDigest = digest("evidence work result receipt_digest", receipt.receipt_digest) as string;
  const { receipt_digest: _receiptDigest, ...receiptDescriptor } = receipt;
  if (digestJsonSync(receiptDescriptor) !== receiptDigest) throw new ArgumentError("evidence work result receipt digest is invalid");
  const assessmentDigest = assessment && typeof assessment.assessment_digest === "string" ? digest("evidence work result assessment_digest", assessment.assessment_digest) : null;
  if (assessment && assessmentDigest !== null) {
    const { assessment_digest: _assessmentDigest, ...assessmentDescriptor } = assessment;
    if (digestJsonSync(assessmentDescriptor) !== assessmentDigest) throw new ArgumentError("evidence work result assessment digest is invalid");
  }
  const evaluatorStatus = String(receipt.evaluator_status);
  const { receipt_digest: _finalReceiptDigest, ...baseReceiptDescriptor } = receipt;
  const baseReceiptDigest = digestJsonSync({ ...baseReceiptDescriptor, evaluator_status: "not_evaluated", assessment_digest: null });
  const accepted = receipt.status !== "failed" && (receipt.status === "observed" || receipt.status === "partial") && evaluatorStatus === "accepted" && assessmentDigest !== null && isObject(assessment) && assessment.receipt_digest === baseReceiptDigest && assessment.requirement_id === item.requirement_id && assessment.verdict === "accepted" && Array.isArray(json.completed_requirement_ids) && json.completed_requirement_ids.includes(item.requirement_id);
  const acceptanceDigest = accepted ? digestJsonSync({ schema: AUTONOMOUS_EVIDENCE_WORK_ACCEPTANCE_SCHEMA, work_id: item.work_id, item_digest: item.item_digest, plan_digest: item.plan_digest, request_digest: item.request_digest, requirement_id: item.requirement_id, source_id: item.source_id, source_digest: item.source_digest, receipt_digest: receiptDigest, assessment_digest: assessmentDigest, result_digest: resultDigest, replay: receipt.replay, status: "accepted" }) : null;
  return { resultDigest, receiptDigest, assessmentDigest, receiptStatus: String(receipt.status), evaluatorStatus, replay: receipt.replay, acceptanceDigest };
}

/** Thread-safe in-memory queue suitable for tests and single-process deployments. */
export class InMemoryAutonomousEvidenceWorkQueue {
  private readonly items = new Map<string, AutonomousEvidenceWorkItem>();

  constructor(readonly maxItems = MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS) {
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > MAX_AUTONOMOUS_EVIDENCE_WORK_ITEMS) throw new ArgumentError("autonomous evidence work queue maxItems is outside its bound");
  }

  enqueue(input: { workId: string; plan: AutonomousEvidencePlan; request: AutonomousEvidenceAcquisitionRequest; parentEvidenceDigests?: readonly string[]; maxAttempts?: number; availableAt?: number; now?: number }): AutonomousEvidenceWorkItem {
    if (!(input.plan instanceof AutonomousEvidencePlan)) throw new ArgumentError("autonomous evidence work enqueue requires a typed plan");
    const workId = identifier("autonomous evidence work_id", input.workId);
    const request = normalizeRequest(input.request);
    const requirementRow = requirement(input.plan, request.requirement_id);
    const requestDigestValue = requestDigest(input.plan.plan_digest, request);
    const parents = digests("autonomous evidence work parent_evidence_digests", input.parentEvidenceDigests ?? [], 64);
    const maxAttempts = boundedInteger("autonomous evidence work maxAttempts", input.maxAttempts ?? 3, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_ATTEMPTS);
    const time = nowMs(input.now);
    const existing = this.items.get(workId);
    if (existing) {
      if (existing.plan_digest !== input.plan.plan_digest || existing.request_digest !== requestDigestValue || existing.requirement_id !== requirementRow.requirement_id) throw new ArgumentError("autonomous evidence work identity conflicts with an existing work item");
      return clone(existing);
    }
    if (this.items.size >= this.maxItems) throw new ArgumentError("autonomous evidence work queue is full");
    const availableAt = timestamp("autonomous evidence work availableAt", input.availableAt ?? time);
    const item = {
      schema: AUTONOMOUS_EVIDENCE_WORK_ITEM_SCHEMA,
      work_id: workId,
      plan_digest: input.plan.plan_digest,
      requirement_id: requirementRow.requirement_id,
      domain: requirementRow.domain,
      workflow_id: requirementRow.workflow_id,
      workflow_digest: requirementRow.workflow_digest,
      stage_id: requirementRow.stage_id,
      source_id: request.source_id,
      source_digest: request.source_digest ?? null,
      request_digest: requestDigestValue,
      parent_evidence_digests: parents,
      max_attempts: maxAttempts,
      attempts: 0,
      status: "queued" as const,
      available_at: availableAt,
      lease_owner: null,
      lease_until: null,
      receipt_digest: null,
      assessment_digest: null,
      result_digest: null,
      acceptance_digest: null,
      failure_class: null,
      last_error_class: null,
      created_at: time,
      updated_at: time,
      item_digest: "",
      retention: "metadata_only_request_and_values_caller_owned" as const,
      secret_material: "never_returned" as const,
    } satisfies AutonomousEvidenceWorkItem;
    item.item_digest = itemDigest(item);
    this.items.set(workId, item);
    return clone(item);
  }

  get(workId: string): AutonomousEvidenceWorkItem | null {
    const item = this.items.get(identifier("autonomous evidence work_id", workId));
    return item ? clone(item) : null;
  }

  pending(limit = 64, now = Date.now()): AutonomousEvidenceWorkItem[] {
    const time = nowMs(now);
    const boundedLimit = boundedInteger("autonomous evidence work pending limit", limit, 1, Math.min(MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH, this.maxItems));
    return [...this.items.values()]
      .filter((item) => (item.status === "queued" && item.available_at <= time && item.attempts < item.max_attempts) || (item.status === "leased" && item.lease_until !== null && item.lease_until <= time && item.attempts < item.max_attempts))
      .sort((left, right) => left.available_at - right.available_at || left.created_at - right.created_at || left.work_id.localeCompare(right.work_id))
      .slice(0, boundedLimit)
      .map((item) => clone(item));
  }

  claim(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousEvidenceWorkItem | null {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const lease = boundedInteger("autonomous evidence work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || ["completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"].includes(item.status)) return null;
    if (item.status === "leased" && item.lease_until !== null && item.lease_until > time) return null;
    if (item.attempts >= item.max_attempts) {
      this.items.set(id, refresh(item, { status: "reconciliation_required", failure_class: "lease_expired", last_error_class: "lease_expired", lease_owner: null, lease_until: null }, time));
      return null;
    }
    const next = refresh(item, { status: "leased", attempts: item.attempts + 1, lease_owner: worker, lease_until: time + lease, failure_class: null, last_error_class: null }, time);
    this.items.set(id, next);
    return clone(next);
  }

  renew(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const lease = boundedInteger("autonomous evidence work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous evidence work lease cannot be renewed by this worker");
    const next = refresh(item, { lease_until: time + lease }, time);
    this.items.set(id, next);
    return clone(next);
  }

  complete(workId: string, workerId: string, result: AutonomousEvidenceRuntimeResult, now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous evidence work completion is fenced by an expired or foreign lease");
    const json = result.toJSON();
    const metadata = resultMetadata(item, result);
    if (json.status !== "completed" && !(json.status === "awaiting_evaluation" && metadata.evaluatorStatus === "accepted")) throw new ArgumentError("autonomous evidence work completion requires an accepted queued requirement");
    if (metadata.acceptanceDigest === null || metadata.evaluatorStatus !== "accepted") throw new ArgumentError("autonomous evidence work completion requires a digest-bound accepted assessment");
    const next = refresh(item, { status: "completed", lease_owner: null, lease_until: null, receipt_digest: metadata.receiptDigest, assessment_digest: metadata.assessmentDigest, result_digest: metadata.resultDigest, acceptance_digest: metadata.acceptanceDigest, failure_class: null, last_error_class: null }, time);
    this.items.set(id, next);
    return clone(next);
  }

  awaitEvaluation(workId: string, workerId: string, result: AutonomousEvidenceRuntimeResult, now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous evidence evaluation handoff is fenced by an expired or foreign lease");
    if (result.toJSON().status !== "awaiting_evaluation") throw new ArgumentError("autonomous evidence evaluation handoff requires an awaiting_evaluation runtime result");
    const metadata = resultMetadata(item, result);
    const next = refresh(item, { status: "awaiting_evaluation", lease_owner: null, lease_until: null, receipt_digest: metadata.receiptDigest, assessment_digest: metadata.assessmentDigest, result_digest: metadata.resultDigest, acceptance_digest: null, failure_class: "evaluation_pending", last_error_class: "evaluation_pending" }, time);
    this.items.set(id, next);
    return clone(next);
  }

  fail(workId: string, workerId: string, errorClass: AutonomousEvidenceWorkFailureClass, retryable: boolean, now = Date.now(), result: AutonomousEvidenceRuntimeResult | null = null): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const failure = workFailure(errorClass);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous evidence work failure is fenced by an expired or foreign lease");
    const metadata = result === null ? null : resultMetadata(item, result);
    const canRetry = retryable && item.attempts < item.max_attempts;
    const delay = Math.min(3_600_000, 1_000 * (2 ** Math.max(0, item.attempts - 1)));
    const next = refresh(item, {
      status: canRetry ? "queued" : "failed",
      available_at: canRetry ? time + delay : item.available_at,
      lease_owner: null,
      lease_until: null,
      receipt_digest: metadata?.receiptDigest ?? item.receipt_digest,
      assessment_digest: metadata?.assessmentDigest ?? item.assessment_digest,
      result_digest: metadata?.resultDigest ?? item.result_digest,
      acceptance_digest: null,
      failure_class: canRetry ? null : failure,
      last_error_class: failure,
    }, time);
    this.items.set(id, next);
    return clone(next);
  }

  reconcile(workId: string, workerId: string, errorClass: AutonomousEvidenceWorkFailureClass = "rehydration_missing", now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const worker = identifier("autonomous evidence worker_id", workerId);
    const failure = workFailure(errorClass);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous evidence reconciliation is fenced by an expired or foreign lease");
    const next = refresh(item, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: failure, last_error_class: failure }, time);
    this.items.set(id, next);
    return clone(next);
  }

  requeue(workId: string, now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || !["awaiting_evaluation", "reconciliation_required"].includes(item.status)) throw new ArgumentError("autonomous evidence work is not waiting for explicit requeue");
    if (item.attempts >= item.max_attempts) throw new ArgumentError("autonomous evidence work has exhausted its attempts");
    const next = refresh(item, { status: "queued", available_at: time, failure_class: null, last_error_class: item.last_error_class }, time);
    this.items.set(id, next);
    return clone(next);
  }

  cancel(workId: string, reason: AutonomousEvidenceWorkFailureClass = "unknown", now = Date.now()): AutonomousEvidenceWorkItem {
    const id = identifier("autonomous evidence work_id", workId);
    const failure = workFailure(reason);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || ["completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"].includes(item.status)) throw new ArgumentError("autonomous evidence work cannot be cancelled in its current state");
    const next = refresh(item, { status: "cancelled", lease_owner: null, lease_until: null, failure_class: failure, last_error_class: failure }, time);
    this.items.set(id, next);
    return clone(next);
  }

  rows(): AutonomousEvidenceWorkItem[] {
    return [...this.items.values()].sort((left, right) => left.created_at - right.created_at || left.work_id.localeCompare(right.work_id)).map((item) => clone(item));
  }

  verifyIntegrity(): { schema: typeof AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA; verified: true; items: number; retention: "metadata_only_request_and_values_caller_owned"; secret_material: "never_returned" } {
    for (const item of this.items.values()) validateItem(item);
    return { schema: AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA, verified: true, items: this.items.size, retention: "metadata_only_request_and_values_caller_owned", secret_material: "never_returned" };
  }

  snapshot(): AutonomousEvidenceWorkQueueSnapshot {
    this.verifyIntegrity();
    const descriptor = { schema: AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA, items: this.rows(), retention: "metadata_only_request_and_values_caller_owned" as const, secret_material: "never_returned" as const };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousEvidenceWorkQueueSnapshot;
    if (bytes(canonicalJson(snapshot)) > MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("autonomous evidence work queue snapshot exceeds its bound");
    return snapshot;
  }

  restore(snapshot: AutonomousEvidenceWorkQueueSnapshot): void {
    const snapshotSchema: string | null = isObject(snapshot) && typeof snapshot.schema === "string" ? String(snapshot.schema) : null;
    if (!isObject(snapshot) || (snapshotSchema !== AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA && snapshotSchema !== LEGACY_AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA) || !Array.isArray(snapshot.items)) throw new ArgumentError("autonomous evidence work queue snapshot is malformed");
    if (snapshot.retention !== "metadata_only_request_and_values_caller_owned" || snapshot.secret_material !== "never_returned") throw new ArgumentError("autonomous evidence work queue snapshot retention is invalid");
    const { snapshot_digest: observed, ...descriptor } = snapshot;
    if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("autonomous evidence work queue snapshot digest is invalid");
    if (snapshot.items.length > this.maxItems) throw new ArgumentError("autonomous evidence work queue snapshot exceeds maxItems");
    const restored = new Map<string, AutonomousEvidenceWorkItem>();
    for (const raw of snapshot.items) {
      const item = snapshotSchema === LEGACY_AUTONOMOUS_EVIDENCE_WORK_QUEUE_SCHEMA ? migrateLegacyItem(raw) : validateItem(raw);
      if (restored.has(item.work_id)) throw new ArgumentError("autonomous evidence work queue snapshot contains duplicate work ids");
      restored.set(item.work_id, item);
    }
    this.items.clear();
    for (const [workId, item] of restored) this.items.set(workId, item);
  }
}

export class AutonomousEvidenceWorkQueuePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly queue: InMemoryAutonomousEvidenceWorkQueue, readonly persistence: AutonomousEvidenceWorkQueuePersistence) {
    if (!(queue instanceof InMemoryAutonomousEvidenceWorkQueue)) throw new ArgumentError("autonomous evidence work persistence requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous evidence work persistence adapter is malformed");
  }

  async restore(): Promise<{ status: "empty" | "restored"; snapshot_digest: string | null; items: number }> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        this.expectedSnapshotDigest = null;
        return { status: "empty", snapshot_digest: null, items: 0 };
      }
      this.queue.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return { status: "restored", snapshot_digest: snapshot.snapshot_digest, items: this.queue.verifyIntegrity().items };
    });
  }

  async flush(): Promise<AutonomousEvidenceWorkQueueSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.queue.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("evidence work persistence compare-and-swap conflict");
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

export class JsonAutonomousEvidenceWorkQueueSnapshotPersistence implements AutonomousEvidenceWorkQueuePersistence {
  constructor(readonly textStore: AutonomousEvidenceWorkQueueSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("evidence work text store is malformed");
  }

  async read(): Promise<AutonomousEvidenceWorkQueueSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("evidence work JSON is invalid"); }
    if (!isObject(parsed)) throw new ArgumentError("evidence work JSON must be an object");
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("evidence work JSON is not canonical");
    if (bytes(canonicalJson(parsed)) > MAX_AUTONOMOUS_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("evidence work JSON exceeds its byte bound");
    return parsed as unknown as AutonomousEvidenceWorkQueueSnapshot;
  }

  async write(snapshot: AutonomousEvidenceWorkQueueSnapshot): Promise<void> {
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousEvidenceWorkQueueSnapshotPersistence extends JsonAutonomousEvidenceWorkQueueSnapshotPersistence {
  declare readonly textStore: AutonomousEvidenceWorkQueueTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousEvidenceWorkQueueTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("evidence work text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousEvidenceWorkQueueSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ArgumentError("evidence work expected snapshot digest is invalid");
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}

export interface AutonomousEvidenceWorkRehydration {
  plan: AutonomousEvidencePlan | AutonomousEvidencePlanJSON;
  runtime: AutonomousEvidenceRuntime;
  request: AutonomousEvidenceAcquisitionRequest;
  execute: AutonomousEvidenceRuntimeExecuteOptions;
}

export type AutonomousEvidenceWorkRehydrator = (item: AutonomousEvidenceWorkItem) => AutonomousEvidenceWorkRehydration | Promise<AutonomousEvidenceWorkRehydration>;

export interface AutonomousEvidenceWorkerRow extends JsonObject {
  work_id: string;
  outcome: "completed" | "replayed" | "retry_scheduled" | "awaiting_evaluation" | "failed" | "reconciliation_required" | "leased_elsewhere";
  attempts: number;
  receipt_digest: string | null;
  assessment_digest: string | null;
  result_digest: string | null;
  value_retained: false;
  error_class: AutonomousEvidenceWorkFailureClass | null;
}

export interface AutonomousEvidenceWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_WORKER_SCHEMA;
  worker_id: string;
  inspected: number;
  completed: number;
  retried: number;
  awaiting_evaluation: number;
  failed: number;
  reconciled: number;
  leased_elsewhere: number;
  rows: AutonomousEvidenceWorkerRow[];
  retention: "metadata_only_receipts_and_digests_no_values";
  secret_material: "never_returned";
}

function row(item: AutonomousEvidenceWorkItem, outcome: AutonomousEvidenceWorkerRow["outcome"], errorClass: AutonomousEvidenceWorkFailureClass | null = null): AutonomousEvidenceWorkerRow {
  return { work_id: item.work_id, outcome, attempts: item.attempts, receipt_digest: item.receipt_digest, assessment_digest: item.assessment_digest, result_digest: item.result_digest, acceptance_digest: item.acceptance_digest, value_retained: false, error_class: errorClass };
}

/** Runs one queued evidence request through a caller-owned runtime and rehydration boundary. */
export class AutonomousEvidenceWorker {
  constructor(readonly queue: InMemoryAutonomousEvidenceWorkQueue, readonly rehydrate: AutonomousEvidenceWorkRehydrator) {
    if (!(queue instanceof InMemoryAutonomousEvidenceWorkQueue)) throw new ArgumentError("autonomous evidence worker requires a typed work queue");
    if (typeof rehydrate !== "function") throw new ArgumentError("autonomous evidence worker requires a rehydrator");
  }

  async run(options: { workerId?: string; limit?: number; leaseMs?: number; now?: number; signal?: { readonly aborted: boolean }; workIds?: readonly string[] } = {}): Promise<AutonomousEvidenceWorkerRun> {
    const workerId = identifier("autonomous evidence worker_id", options.workerId ?? "evidence-worker");
    const limit = boundedInteger("autonomous evidence worker limit", options.limit ?? 64, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH);
    const leaseMs = boundedInteger("autonomous evidence worker lease_ms", options.leaseMs ?? 30_000, 1, MAX_AUTONOMOUS_EVIDENCE_WORK_LEASE_MS);
    const workIds = options.workIds === undefined ? null : options.workIds.map((workId) => identifier("autonomous evidence worker work_id", workId));
    if (workIds !== null && (workIds.length < 1 || workIds.length > MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH || new Set(workIds).size !== workIds.length)) throw new ArgumentError("autonomous evidence worker workIds are outside their bound");
    const time = nowMs(options.now);
    const currentTime = () => options.now === undefined ? Date.now() : time;
    const pending = this.queue.pending(workIds === null ? limit : MAX_AUTONOMOUS_EVIDENCE_WORK_BATCH, time).filter((item) => workIds === null || workIds.includes(item.work_id)).slice(0, limit);
    const rows: AutonomousEvidenceWorkerRow[] = [];
    for (const candidate of pending) {
      if (options.signal?.aborted) break;
      const claimed = this.queue.claim(candidate.work_id, workerId, leaseMs, time);
      if (!claimed) { rows.push(row(candidate, "leased_elsewhere")); continue; }
      try {
        const hydrated = await this.rehydrate(claimed);
        if (!hydrated || !(hydrated.runtime instanceof AutonomousEvidenceRuntime) || !hydrated.request || !hydrated.execute || typeof hydrated.execute.acquirer !== "object" && typeof hydrated.execute.acquirer !== "function") {
          const reconciled = this.queue.reconcile(claimed.work_id, workerId, "rehydration_missing", currentTime());
          rows.push(row(reconciled, "reconciliation_required", "rehydration_missing"));
          continue;
        }
        const plan = planFor(hydrated.plan);
        const request = normalizeRequest(hydrated.request);
        const requirementRow = requirement(plan, request.requirement_id);
        if (plan.plan_digest !== claimed.plan_digest || hydrated.runtime.plan.plan_digest !== claimed.plan_digest || requestDigest(plan.plan_digest, request) !== claimed.request_digest || requirementRow.domain !== claimed.domain || requirementRow.workflow_digest !== claimed.workflow_digest || request.source_id !== claimed.source_id || request.source_digest !== claimed.source_digest) throw new ArgumentError("autonomous evidence worker rehydrated identity conflicts with the work item");
        const executeOptions = { ...hydrated.execute, parentEvidenceDigests: claimed.parent_evidence_digests };
        const result = await hydrated.runtime.execute([request], executeOptions);
        const queuedReceipt = result.json.receipts.find((receipt) => receipt.request_digest === claimed.request_digest);
        if (result.json.status === "completed" || (result.json.status === "awaiting_evaluation" && queuedReceipt?.evaluator_status === "accepted")) {
          const finished = this.queue.complete(claimed.work_id, workerId, result, currentTime());
          rows.push(row(finished, result.json.receipts.some((receipt) => receipt.replay === "replayed") ? "replayed" : "completed"));
        } else if (result.json.status === "awaiting_evaluation") {
          const waiting = this.queue.awaitEvaluation(claimed.work_id, workerId, result, currentTime());
          rows.push(row(waiting, "awaiting_evaluation", "evaluation_pending"));
        } else if (result.json.status === "reconciliation_required") {
          const reconciled = this.queue.reconcile(claimed.work_id, workerId, "result_reconciliation_required", currentTime());
          rows.push(row(reconciled, "reconciliation_required", "result_reconciliation_required"));
        } else {
          const failure = result.json.receipts.some((receipt) => receipt.evidence_status === "projection_failed") ? "projection_failed" : "acquisition_failed";
          const failed = this.queue.fail(claimed.work_id, workerId, failure, true, currentTime(), result);
          rows.push(row(failed, failed.status === "queued" ? "retry_scheduled" : "failed", failure));
        }
      } catch (error) {
        const failure = this.classify(error);
        if (["rehydration_missing", "rehydration_invalid", "identity_conflict", "result_reconciliation_required", "result_invalid"].includes(failure)) {
          const reconciled = this.queue.reconcile(claimed.work_id, workerId, failure, currentTime());
          rows.push(row(reconciled, "reconciliation_required", failure));
        } else {
          const failed = this.queue.fail(claimed.work_id, workerId, failure, ["executor_error", "transport_error", "unknown"].includes(failure), currentTime());
          rows.push(row(failed, failed.status === "queued" ? "retry_scheduled" : "failed", failure));
        }
      }
    }
    return {
      schema: AUTONOMOUS_EVIDENCE_WORKER_SCHEMA,
      worker_id: workerId,
      inspected: pending.length,
      completed: rows.filter((entry) => entry.outcome === "completed" || entry.outcome === "replayed").length,
      retried: rows.filter((entry) => entry.outcome === "retry_scheduled").length,
      awaiting_evaluation: rows.filter((entry) => entry.outcome === "awaiting_evaluation").length,
      failed: rows.filter((entry) => entry.outcome === "failed").length,
      reconciled: rows.filter((entry) => entry.outcome === "reconciliation_required").length,
      leased_elsewhere: rows.filter((entry) => entry.outcome === "leased_elsewhere").length,
      rows,
      retention: "metadata_only_receipts_and_digests_no_values",
      secret_material: "never_returned",
    };
  }

  private classify(error: unknown): AutonomousEvidenceWorkFailureClass {
    const message = error instanceof Error ? error.message.toLowerCase() : "";
    if (message.includes("rehydrat") || message.includes("runtime plan") || message.includes("request identity") || message.includes("work item")) return message.includes("missing") ? "rehydration_missing" : message.includes("identity") || message.includes("conflicts") ? "identity_conflict" : "rehydration_invalid";
    if (message.includes("projection")) return "projection_failed";
    if (message.includes("transport")) return "transport_error";
    if (message.includes("acquisition")) return "acquisition_failed";
    if (message.includes("completion requires") || message.includes("acceptance") || message.includes("receipt identity") || message.includes("result digest")) return "result_invalid";
    if (message.includes("reconciliation_required") || message.includes("requires rehydration")) return "result_reconciliation_required";
    if (message.includes("evaluator")) return "evaluation_rejected";
    if (message.includes("executor")) return "executor_error";
    return "unknown";
  }
}
