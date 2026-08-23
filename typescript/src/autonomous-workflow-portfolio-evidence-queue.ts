import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousDomainName,
} from "./autonomous.js";
import {
  AutonomousWorkflowPortfolioExecutionResult,
  type AutonomousWorkflowPortfolioExecutionItemStatus,
} from "./autonomous-workflow-portfolio-execution.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only multi-worker admission and lease state for one portfolio's evidence items. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-evidence-work-queue/0.2" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-evidence-work-item/0.2" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS = 64;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS = 300_000;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES = 256_000;

export type AutonomousWorkflowPortfolioEvidenceWorkStatus =
  | "queued"
  | "leased"
  | "completed"
  | "awaiting_evaluation"
  | "failed"
  | "reconciliation_required"
  | "cancelled";

export type AutonomousWorkflowPortfolioEvidenceWorkFailureClass =
  | "dependency_failed"
  | "provider_execution_not_succeeded"
  | "lease_expired"
  | "approval_required"
  | "rehydration_missing"
  | "identity_conflict"
  | "evaluator_pending"
  | "executor_error"
  | "transport_error"
  | "unknown";

export interface AutonomousWorkflowPortfolioEvidenceWorkItem extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA;
  work_id: string;
  job_id: string;
  item_id: string;
  domain: AutonomousDomainName;
  wave_index: number;
  dependency_item_ids: string[];
  provider_status: AutonomousWorkflowPortfolioExecutionItemStatus;
  portfolio_plan_digest: string;
  admission_digest: string | null;
  provider_execution_digest: string;
  evidence_plan_digest: string;
  request_digest: string;
  checkpoint_digest: string | null;
  max_attempts: number;
  attempts: number;
  status: AutonomousWorkflowPortfolioEvidenceWorkStatus;
  available_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  result_digest: string | null;
  failure_class: AutonomousWorkflowPortfolioEvidenceWorkFailureClass | null;
  last_error_class: AutonomousWorkflowPortfolioEvidenceWorkFailureClass | null;
  created_at: number;
  updated_at: number;
  item_digest: string;
  retention: "metadata_only_task_sources_values_and_provider_payloads_never_persisted";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA;
  items: AutonomousWorkflowPortfolioEvidenceWorkItem[];
  retention: "metadata_only_task_sources_values_and_provider_payloads_never_persisted";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousWorkflowPortfolioEvidenceWorkQueuePersistence {
  read(): Promise<AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot | null> | AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot | null;
  write(snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): Promise<boolean> | boolean;
}

/** Minimal text store used by the portable JSON queue persistence adapters. */
export interface AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

/** Text store with an atomic digest fence for multi-worker queue snapshots. */
export interface AutonomousWorkflowPortfolioEvidenceWorkQueueTransactionalSnapshotTextStore extends AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioEvidenceWorkExecution {
  status: "completed" | "awaiting_evaluation" | "failed" | "reconciliation_required";
  result_digest: string | null;
  error_class?: AutonomousWorkflowPortfolioEvidenceWorkFailureClass | null;
  retryable?: boolean;
}

export interface AutonomousWorkflowPortfolioEvidenceWorkWorkerRow extends JsonObject {
  work_id: string;
  item_id: string;
  domain: AutonomousDomainName;
  outcome: "completed" | "awaiting_evaluation" | "retry_scheduled" | "failed" | "reconciliation_required" | "leased_elsewhere";
  attempts: number;
  result_digest: string | null;
  error_class: AutonomousWorkflowPortfolioEvidenceWorkFailureClass | null;
  lease_retained: false;
}

export interface AutonomousWorkflowPortfolioEvidenceWorkWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA;
  worker_id: string;
  inspected: number;
  completed: number;
  awaiting_evaluation: number;
  retried: number;
  failed: number;
  reconciled: number;
  leased_elsewhere: number;
  rows: AutonomousWorkflowPortfolioEvidenceWorkWorkerRow[];
  retention: "metadata_only_receipts_and_digests_no_values";
  secret_material: "never_returned";
}

const RETENTION = "metadata_only_task_sources_values_and_provider_payloads_never_persisted" as const;
const SECRET_MATERIAL = "never_returned" as const;
const STATUSES: readonly AutonomousWorkflowPortfolioEvidenceWorkStatus[] = ["queued", "leased", "completed", "awaiting_evaluation", "failed", "reconciliation_required", "cancelled"];
const FAILURES: readonly AutonomousWorkflowPortfolioEvidenceWorkFailureClass[] = ["dependency_failed", "provider_execution_not_succeeded", "lease_expired", "approval_required", "rehydration_missing", "identity_conflict", "evaluator_pending", "executor_error", "transport_error", "unknown"];
const PROVIDER_STATUSES: readonly AutonomousWorkflowPortfolioExecutionItemStatus[] = ["succeeded", "failed", "blocked", "approval_required", "route_review_required", "reconciliation_required", "turn_limit_reached", "child_failed", "omitted"];

function clone<T>(value: T): T {
  return structuredClone(value);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function timestamp(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER) throw new ArgumentError(`${name} must be a finite non-negative timestamp`);
  return value;
}

function failure(name: string, value: unknown): AutonomousWorkflowPortfolioEvidenceWorkFailureClass {
  if (!FAILURES.includes(value as AutonomousWorkflowPortfolioEvidenceWorkFailureClass)) throw new ArgumentError(`${name} is not a recognized portfolio evidence work failure`);
  return value as AutonomousWorkflowPortfolioEvidenceWorkFailureClass;
}

function itemDescriptor(item: AutonomousWorkflowPortfolioEvidenceWorkItem): JsonObject {
  const { item_digest: _itemDigest, retention: _retention, secret_material: _secretMaterial, ...descriptor } = item;
  return descriptor;
}

function itemDigest(item: AutonomousWorkflowPortfolioEvidenceWorkItem): string {
  return digestJsonSync(itemDescriptor(item));
}

function refresh(
  item: AutonomousWorkflowPortfolioEvidenceWorkItem,
  patch: Partial<AutonomousWorkflowPortfolioEvidenceWorkItem>,
  now: number,
): AutonomousWorkflowPortfolioEvidenceWorkItem {
  const next = { ...item, ...patch, updated_at: timestamp("portfolio evidence work updated_at", now) } as AutonomousWorkflowPortfolioEvidenceWorkItem;
  next.item_digest = itemDigest(next);
  return next;
}

function validateItem(raw: unknown): AutonomousWorkflowPortfolioEvidenceWorkItem {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA) throw new ArgumentError("portfolio evidence work item schema is invalid");
  const item = raw as unknown as AutonomousWorkflowPortfolioEvidenceWorkItem;
  const allowed = new Set(["schema", "work_id", "job_id", "item_id", "domain", "wave_index", "dependency_item_ids", "provider_status", "portfolio_plan_digest", "admission_digest", "provider_execution_digest", "evidence_plan_digest", "request_digest", "checkpoint_digest", "max_attempts", "attempts", "status", "available_at", "lease_owner", "lease_until", "result_digest", "failure_class", "last_error_class", "created_at", "updated_at", "item_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("portfolio evidence work item contains unsupported fields");
  const normalized = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA,
    work_id: identifier("portfolio evidence work_id", item.work_id),
    job_id: identifier("portfolio evidence work job_id", item.job_id),
    item_id: identifier("portfolio evidence work item_id", item.item_id),
    domain: item.domain,
    wave_index: boundedInteger("portfolio evidence work wave_index", item.wave_index, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS),
    dependency_item_ids: Array.isArray(item.dependency_item_ids) ? item.dependency_item_ids.map((value, index) => identifier(`portfolio evidence work dependency_item_ids[${index}]`, value)) : [],
    provider_status: item.provider_status,
    portfolio_plan_digest: digest("portfolio evidence work portfolio_plan_digest", item.portfolio_plan_digest),
    admission_digest: item.admission_digest === null ? null : digest("portfolio evidence work admission_digest", item.admission_digest),
    provider_execution_digest: digest("portfolio evidence work provider_execution_digest", item.provider_execution_digest),
    evidence_plan_digest: digest("portfolio evidence work evidence_plan_digest", item.evidence_plan_digest),
    request_digest: digest("portfolio evidence work request_digest", item.request_digest),
    checkpoint_digest: item.checkpoint_digest === null ? null : digest("portfolio evidence work checkpoint_digest", item.checkpoint_digest),
    max_attempts: boundedInteger("portfolio evidence work max_attempts", item.max_attempts, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS),
    attempts: boundedInteger("portfolio evidence work attempts", item.attempts, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS),
    status: item.status,
    available_at: timestamp("portfolio evidence work available_at", item.available_at),
    lease_owner: item.lease_owner === null ? null : identifier("portfolio evidence work lease_owner", item.lease_owner),
    lease_until: item.lease_until === null ? null : timestamp("portfolio evidence work lease_until", item.lease_until),
    result_digest: item.result_digest === null ? null : digest("portfolio evidence work result_digest", item.result_digest),
    failure_class: item.failure_class === null ? null : failure("portfolio evidence work failure_class", item.failure_class),
    last_error_class: item.last_error_class === null ? null : failure("portfolio evidence work last_error_class", item.last_error_class),
    created_at: timestamp("portfolio evidence work created_at", item.created_at),
    updated_at: timestamp("portfolio evidence work updated_at", item.updated_at),
    item_digest: digest("portfolio evidence work item_digest", item.item_digest),
    retention: item.retention,
    secret_material: item.secret_material,
  } satisfies AutonomousWorkflowPortfolioEvidenceWorkItem;
  if (!STATUSES.includes(normalized.status)) throw new ArgumentError("portfolio evidence work status is unsupported");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(normalized.domain as AutonomousDomainName)) throw new ArgumentError("portfolio evidence work domain is unsupported");
  if (!Array.isArray(item.dependency_item_ids) || normalized.dependency_item_ids.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS || new Set(normalized.dependency_item_ids).size !== normalized.dependency_item_ids.length) throw new ArgumentError("portfolio evidence work dependencies are invalid");
  if (!PROVIDER_STATUSES.includes(normalized.provider_status)) throw new ArgumentError("portfolio evidence work provider status is invalid");
  if (normalized.attempts > normalized.max_attempts) throw new ArgumentError("portfolio evidence work attempts exceed max_attempts");
  if (normalized.retention !== RETENTION || normalized.secret_material !== SECRET_MATERIAL) throw new ArgumentError("portfolio evidence work retention contract is invalid");
  if (normalized.status === "leased" && (normalized.lease_owner === null || normalized.lease_until === null)) throw new ArgumentError("leased portfolio evidence work must have a lease");
  if (normalized.status !== "leased" && (normalized.lease_owner !== null || normalized.lease_until !== null)) throw new ArgumentError("non-leased portfolio evidence work cannot retain a lease");
  if (["completed", "awaiting_evaluation"].includes(normalized.status) && normalized.result_digest === null) throw new ArgumentError("terminal portfolio evidence work requires a result digest");
  if (itemDigest(normalized) !== normalized.item_digest) throw new ArgumentError("portfolio evidence work item digest is invalid");
  return clone(normalized);
}

export function validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(raw: unknown, maxItems = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS): AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA || !Array.isArray(raw.items)) throw new ArgumentError("portfolio evidence work queue snapshot is malformed");
  const snapshot = raw as unknown as AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot;
  if (snapshot.retention !== RETENTION || snapshot.secret_material !== SECRET_MATERIAL) throw new ArgumentError("portfolio evidence work queue snapshot retention contract is invalid");
  const { snapshot_digest: observed, ...descriptor } = snapshot;
  if (typeof observed !== "string" || digestJsonSync(descriptor) !== observed) throw new ArgumentError("portfolio evidence work queue snapshot digest is invalid");
  if (snapshot.items.length > maxItems) throw new ArgumentError("portfolio evidence work queue snapshot exceeds its bound");
  const items = snapshot.items.map(validateItem);
  if (new Set(items.map((item) => item.work_id)).size !== items.length) throw new ArgumentError("portfolio evidence work queue snapshot contains duplicate work ids");
  return clone({ ...snapshot, items });
}

function queueSnapshotBytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

/** JSON persistence for browser, Node, or embedded text stores. */
export class JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence implements AutonomousWorkflowPortfolioEvidenceWorkQueuePersistence {
  protected readonly store: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore;

  constructor(
    store: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore,
    readonly maxItems = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
  ) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("portfolio evidence work JSON persistence requires a text store");
    boundedInteger("portfolio evidence work JSON persistence maxItems", maxItems, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS);
    this.store = store;
  }

  async read(): Promise<AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || queueSnapshotBytes(encoded) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("portfolio evidence work JSON persistence text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("portfolio evidence work JSON persistence text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("portfolio evidence work JSON persistence text is not canonical");
    return validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(parsed, this.maxItems);
  }

  async write(snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): Promise<void> {
    await this.store.write(this.encode(snapshot));
  }

  protected encode(snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): string {
    const validated = validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(snapshot, this.maxItems);
    const encoded = canonicalJson(validated);
    if (queueSnapshotBytes(encoded) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("portfolio evidence work JSON persistence snapshot exceeds its bound");
    return encoded;
  }
}

/** JSON queue persistence variant that requires an atomic compare-and-swap text store. */
export class TransactionalJsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence extends JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence {
  private readonly transactionalStore: AutonomousWorkflowPortfolioEvidenceWorkQueueTransactionalSnapshotTextStore;

  constructor(
    store: AutonomousWorkflowPortfolioEvidenceWorkQueueTransactionalSnapshotTextStore,
    maxItems = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS,
  ) {
    super(store, maxItems);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional portfolio evidence work JSON persistence requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): Promise<boolean> {
    const committed = await this.transactionalStore.writeIfUnchanged(expectedSnapshotDigest, this.encode(snapshot));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional portfolio evidence work JSON persistence returned a non-boolean commit result");
    return committed;
  }
}

/** Browser-compatible single-writer text store for localStorage/sessionStorage-like objects. */
export class WebStorageAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore implements AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore {
  readonly storage: Pick<Storage, "getItem" | "setItem">;
  readonly key: string;

  constructor(storage: Pick<Storage, "getItem" | "setItem">, key = "aurora.autonomous.portfolio.evidence.work") {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("portfolio evidence work web storage adapter requires getItem and setItem");
    if (typeof key !== "string" || !key.trim() || key.length > 256 || key.includes("\u0000")) throw new ArgumentError("portfolio evidence work web storage key is outside its bounds");
    this.storage = storage;
    this.key = key;
  }

  read(): string | null {
    return this.storage.getItem(this.key);
  }

  write(value: string): void {
    if (typeof value !== "string" || queueSnapshotBytes(value) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("portfolio evidence work web storage value exceeds its bound");
    this.storage.setItem(this.key, value);
  }
}

function nowMs(value: number | undefined): number {
  return timestamp("portfolio evidence work now", value ?? Date.now());
}

/** Dependency-aware, lease-fenced portfolio evidence work queue. */
export class InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue {
  private readonly items = new Map<string, AutonomousWorkflowPortfolioEvidenceWorkItem>();

  constructor(readonly maxItems = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS) {
    boundedInteger("portfolio evidence work queue maxItems", maxItems, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS);
  }

  admit(input: {
    workId: string;
    jobId: string;
    itemId: string;
    domain: AutonomousDomainName;
    waveIndex: number;
    dependencyItemIds?: readonly string[];
    providerStatus: AutonomousWorkflowPortfolioExecutionItemStatus;
    portfolioPlanDigest: string;
    admissionDigest?: string | null;
    providerExecutionDigest: string;
    evidencePlanDigest: string;
    requestDigest: string;
    checkpointDigest?: string | null;
    maxAttempts?: number;
    availableAt?: number;
    now?: number;
  }): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const workId = identifier("portfolio evidence work_id", input.workId);
    const jobId = identifier("portfolio evidence work job_id", input.jobId);
    const itemId = identifier("portfolio evidence work item_id", input.itemId);
    const dependencies = [...new Set((input.dependencyItemIds ?? []).map((value) => identifier("portfolio evidence work dependency", value)))].sort();
    const maxAttempts = boundedInteger("portfolio evidence work maxAttempts", input.maxAttempts ?? 3, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ATTEMPTS);
    const time = nowMs(input.now);
    const existing = this.items.get(workId);
    if (existing) {
      const admissionDigest = input.admissionDigest === undefined || input.admissionDigest === null ? null : digest("portfolio evidence work admissionDigest", input.admissionDigest);
      if (existing.job_id !== jobId || existing.item_id !== itemId || existing.domain !== input.domain || existing.wave_index !== input.waveIndex || JSON.stringify(existing.dependency_item_ids) !== JSON.stringify(dependencies) || existing.provider_status !== input.providerStatus || existing.portfolio_plan_digest !== input.portfolioPlanDigest || existing.admission_digest !== admissionDigest || existing.provider_execution_digest !== input.providerExecutionDigest || existing.evidence_plan_digest !== input.evidencePlanDigest || existing.request_digest !== input.requestDigest || existing.max_attempts !== maxAttempts) throw new ArgumentError("portfolio evidence work identity conflicts with an existing item");
      return clone(existing);
    }
    if (this.items.size >= this.maxItems) throw new ArgumentError("portfolio evidence work queue is full");
    const item = {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEM_SCHEMA,
      work_id: workId,
      job_id: jobId,
      item_id: itemId,
      domain: input.domain,
      wave_index: boundedInteger("portfolio evidence work waveIndex", input.waveIndex, 0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS),
      dependency_item_ids: dependencies,
      provider_status: input.providerStatus,
      portfolio_plan_digest: digest("portfolio evidence work portfolioPlanDigest", input.portfolioPlanDigest),
      admission_digest: input.admissionDigest === undefined || input.admissionDigest === null ? null : digest("portfolio evidence work admissionDigest", input.admissionDigest),
      provider_execution_digest: digest("portfolio evidence work providerExecutionDigest", input.providerExecutionDigest),
      evidence_plan_digest: digest("portfolio evidence work evidencePlanDigest", input.evidencePlanDigest),
      request_digest: digest("portfolio evidence work requestDigest", input.requestDigest),
      checkpoint_digest: input.checkpointDigest === undefined || input.checkpointDigest === null ? null : digest("portfolio evidence work checkpointDigest", input.checkpointDigest),
      max_attempts: maxAttempts,
      attempts: 0,
      status: "queued" as const,
      available_at: timestamp("portfolio evidence work availableAt", input.availableAt ?? time),
      lease_owner: null,
      lease_until: null,
      result_digest: null,
      failure_class: null,
      last_error_class: null,
      created_at: time,
      updated_at: time,
      item_digest: "0".repeat(64),
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    } satisfies AutonomousWorkflowPortfolioEvidenceWorkItem;
    item.item_digest = itemDigest(item);
    const validated = validateItem(item);
    this.items.set(workId, validated);
    return clone(validated);
  }

  get(workId: string): AutonomousWorkflowPortfolioEvidenceWorkItem | null {
    const item = this.items.get(identifier("portfolio evidence work_id", workId));
    return item ? clone(item) : null;
  }

  dependencyStatuses(item: AutonomousWorkflowPortfolioEvidenceWorkItem): Record<string, AutonomousWorkflowPortfolioEvidenceWorkStatus | "missing"> {
    return Object.fromEntries(item.dependency_item_ids.map((dependency) => [dependency, this.items.get(dependency)?.status ?? "missing"]));
  }

  private dependencyReady(item: AutonomousWorkflowPortfolioEvidenceWorkItem): boolean {
    return item.dependency_item_ids.every((dependency) => this.items.get(dependency)?.status === "completed");
  }

  private dependencyFailed(item: AutonomousWorkflowPortfolioEvidenceWorkItem): boolean {
    return item.dependency_item_ids.some((dependency) => ["failed", "reconciliation_required", "cancelled"].includes(this.items.get(dependency)?.status ?? "missing"));
  }

  pending(limit = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem[] {
    const time = nowMs(now);
    const boundedLimit = boundedInteger("portfolio evidence work pending limit", limit, 1, Math.min(MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, this.maxItems));
    return [...this.items.values()]
      .filter((item) => item.status === "queued" && item.available_at <= time && item.attempts < item.max_attempts && (this.dependencyReady(item) || this.dependencyFailed(item)))
      .sort((left, right) => left.wave_index - right.wave_index || left.available_at - right.available_at || left.created_at - right.created_at || left.work_id.localeCompare(right.work_id))
      .slice(0, boundedLimit)
      .map(clone);
  }

  /** Reconcile expired leases without requiring a replacement worker to claim each id first. */
  reclaimExpired(now = Date.now(), limit = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS): AutonomousWorkflowPortfolioEvidenceWorkItem[] {
    const time = nowMs(now);
    const boundedLimit = boundedInteger("portfolio evidence work reclaim limit", limit, 1, Math.min(MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, this.maxItems));
    const expired = [...this.items.values()]
      .filter((item) => item.status === "leased" && item.lease_until !== null && item.lease_until <= time)
      .sort((left, right) => left.lease_until! - right.lease_until! || left.work_id.localeCompare(right.work_id))
      .slice(0, boundedLimit);
    const reconciled: AutonomousWorkflowPortfolioEvidenceWorkItem[] = [];
    for (const item of expired) {
      const next = refresh(item, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: "lease_expired", last_error_class: "lease_expired" }, time);
      this.items.set(item.work_id, next);
      reconciled.push(clone(next));
    }
    return reconciled;
  }

  claim(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem | null {
    const id = identifier("portfolio evidence work_id", workId);
    const worker = identifier("portfolio evidence worker_id", workerId);
    const lease = boundedInteger("portfolio evidence work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || ["completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"].includes(item.status)) return null;
    if (item.status === "leased") {
      if (item.lease_until !== null && item.lease_until > time) return null;
      this.items.set(id, refresh(item, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: "lease_expired", last_error_class: "lease_expired" }, time));
      return null;
    }
    if (item.provider_status !== "succeeded") {
      this.items.set(id, refresh(item, { status: "reconciliation_required", failure_class: "provider_execution_not_succeeded", last_error_class: "provider_execution_not_succeeded" }, time));
      return null;
    }
    if (this.dependencyFailed(item)) {
      this.items.set(id, refresh(item, { status: "reconciliation_required", failure_class: "dependency_failed", last_error_class: "dependency_failed" }, time));
      return null;
    }
    if (!this.dependencyReady(item) || item.available_at > time || item.attempts >= item.max_attempts) return null;
    const next = refresh(item, { status: "leased", attempts: item.attempts + 1, lease_owner: worker, lease_until: time + lease, failure_class: null, last_error_class: null }, time);
    this.items.set(id, next);
    return clone(next);
  }

  renew(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const worker = identifier("portfolio evidence worker_id", workerId);
    const lease = boundedInteger("portfolio evidence work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("portfolio evidence work lease cannot be renewed by this worker");
    const next = refresh(item, { lease_until: time + lease }, time);
    this.items.set(id, next);
    return clone(next);
  }

  complete(workId: string, workerId: string, result: { status: "completed" | "awaiting_evaluation"; resultDigest: string }, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const worker = identifier("portfolio evidence worker_id", workerId);
    const time = nowMs(now);
    const resultDigest = digest("portfolio evidence work resultDigest", result.resultDigest);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("portfolio evidence work completion is fenced by an expired or foreign lease");
    const next = refresh(item, { status: result.status, lease_owner: null, lease_until: null, result_digest: resultDigest, failure_class: result.status === "awaiting_evaluation" ? "evaluator_pending" : null, last_error_class: result.status === "awaiting_evaluation" ? "evaluator_pending" : null }, time);
    this.items.set(id, next);
    return clone(next);
  }

  fail(workId: string, workerId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass, retryable: boolean, resultDigest: string | null = null, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const worker = identifier("portfolio evidence worker_id", workerId);
    const failureClass = failure("portfolio evidence work failure", errorClass);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("portfolio evidence work failure is fenced by an expired or foreign lease");
    const canRetry = retryable && item.attempts < item.max_attempts;
    const delay = Math.min(3_600_000, 1_000 * (2 ** Math.max(0, item.attempts - 1)));
    const next = refresh(item, { status: canRetry ? "queued" : "failed", available_at: canRetry ? time + delay : item.available_at, lease_owner: null, lease_until: null, result_digest: resultDigest === null ? item.result_digest : digest("portfolio evidence work failure resultDigest", resultDigest), failure_class: canRetry ? null : failureClass, last_error_class: failureClass }, time);
    this.items.set(id, next);
    return clone(next);
  }

  reconcile(workId: string, workerId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass = "rehydration_missing", now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const worker = identifier("portfolio evidence worker_id", workerId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("portfolio evidence work reconciliation is fenced by an expired or foreign lease");
    const next = refresh(item, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: failure("portfolio evidence work reconciliation failure", errorClass), last_error_class: failure("portfolio evidence work reconciliation failure", errorClass) }, time);
    this.items.set(id, next);
    return clone(next);
  }

  requeue(workId: string, now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || !["awaiting_evaluation", "reconciliation_required"].includes(item.status)) throw new ArgumentError("portfolio evidence work is not awaiting explicit requeue");
    if (item.attempts >= item.max_attempts) throw new ArgumentError("portfolio evidence work has exhausted its attempts");
    const next = refresh(item, { status: "queued", available_at: time, failure_class: null, last_error_class: item.last_error_class }, time);
    this.items.set(id, next);
    return clone(next);
  }

  cancel(workId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass = "unknown", now = Date.now()): AutonomousWorkflowPortfolioEvidenceWorkItem {
    const id = identifier("portfolio evidence work_id", workId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || ["completed", "failed", "awaiting_evaluation", "reconciliation_required", "cancelled"].includes(item.status)) throw new ArgumentError("portfolio evidence work cannot be cancelled in its current state");
    const next = refresh(item, { status: "cancelled", lease_owner: null, lease_until: null, failure_class: failure("portfolio evidence cancellation failure", errorClass), last_error_class: failure("portfolio evidence cancellation failure", errorClass) }, time);
    this.items.set(id, next);
    return clone(next);
  }

  bindCheckpointDigest(jobId: string, checkpoint: string | null, now = Date.now()): number {
    const job = identifier("portfolio evidence checkpoint job_id", jobId);
    const checkpointDigest = checkpoint === null ? null : digest("portfolio evidence checkpoint digest", checkpoint);
    const time = nowMs(now);
    let count = 0;
    for (const [workId, item] of this.items) {
      if (item.job_id !== job) continue;
      this.items.set(workId, refresh(item, { checkpoint_digest: checkpointDigest }, time));
      count += 1;
    }
    return count;
  }

  rows(): AutonomousWorkflowPortfolioEvidenceWorkItem[] {
    return [...this.items.values()].sort((left, right) => left.wave_index - right.wave_index || left.created_at - right.created_at || left.work_id.localeCompare(right.work_id)).map(clone);
  }

  snapshot(): AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot {
    this.verifyIntegrity();
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA, items: this.rows(), retention: RETENTION, secret_material: SECRET_MATERIAL } as const;
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_SNAPSHOT_BYTES) throw new ArgumentError("portfolio evidence work queue snapshot exceeds its bound");
    return clone(snapshot);
  }

  restore(snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): void {
    const restored = validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(snapshot, this.maxItems);
    this.items.clear();
    for (const item of restored.items) this.items.set(item.work_id, item);
  }

  verifyIntegrity(): { verified: true; items: number; schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA } {
    for (const item of this.items.values()) validateItem(item);
    return { verified: true, items: this.items.size, schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA };
  }
}

/** Admit all reviewed portfolio items with the exact provider/evidence/request identities. */
export function admitAutonomousWorkflowPortfolioEvidenceWorkItems(
  queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
  input: {
    jobId: string;
    execution: AutonomousWorkflowPortfolioExecutionResult;
    evidencePlanDigest: string;
    itemRequestDigests: readonly string[];
    checkpointDigest?: string | null;
    maxAttempts?: number;
    now?: number;
  },
): AutonomousWorkflowPortfolioEvidenceWorkItem[] {
  if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue)) throw new ArgumentError("portfolio evidence work admission requires a typed queue");
  if (!(input.execution instanceof AutonomousWorkflowPortfolioExecutionResult)) throw new ArgumentError("portfolio evidence work admission requires a typed provider execution");
  const items = input.execution.plan.items;
  if (!Array.isArray(input.itemRequestDigests) || input.itemRequestDigests.length !== items.length) throw new ArgumentError("portfolio evidence work admission request digests must align with the reviewed plan");
  const waves = new Map<string, number>();
  input.execution.plan.dependency_graph.waves.forEach((wave, index) => wave.forEach((itemId) => waves.set(itemId, index)));
  return items.map((item, index) => queue.admit({
    workId: `${input.jobId}:${item.item_id}`,
    jobId: input.jobId,
    itemId: item.item_id,
    domain: item.domain,
    waveIndex: waves.get(item.item_id) ?? 0,
    dependencyItemIds: item.depends_on.map((dependency) => `${input.jobId}:${dependency}`),
    providerStatus: input.execution.items.find((candidate) => candidate.itemId === item.item_id)?.status ?? "omitted",
    portfolioPlanDigest: input.execution.plan.portfolio_digest,
    admissionDigest: input.execution.admissionDigest,
    providerExecutionDigest: input.execution.executionDigest,
    evidencePlanDigest: input.evidencePlanDigest,
    requestDigest: digest("portfolio evidence admission request digest", input.itemRequestDigests[index]),
    checkpointDigest: input.checkpointDigest,
    maxAttempts: input.maxAttempts,
    now: input.now,
  }));
}

/** Snapshot persistence coordinator with an optional atomic snapshot fence. */
export class AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue, readonly persistence: AutonomousWorkflowPortfolioEvidenceWorkQueuePersistence) {
    if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue)) throw new ArgumentError("portfolio evidence work persistence requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("portfolio evidence work persistence is malformed");
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
      return { status: "restored", snapshot_digest: snapshot.snapshot_digest, items: snapshot.items.length };
    });
  }

  async flush(): Promise<AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.queue.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        const committed = await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot);
        if (!committed) throw new ArgumentError("portfolio evidence work queue compare-and-swap conflict; reload before continuing");
      } else {
        await this.persistence.write(snapshot);
      }
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

/**
 * CAS-backed state-transition coordinator for workers that share one queue snapshot.
 *
 * A normal persistence coordinator is useful for one process: it serializes local flushes but
 * intentionally leaves claim/complete operations in memory until the caller flushes. This
 * coordinator reloads the latest snapshot before every mutation and commits the resulting
 * transition through `writeIfUnchanged`. A concurrent claim therefore either wins the fence or
 * retries against the newer lease; it cannot execute the same item merely because two workers
 * restored the same snapshot at the same time.
 */
export class AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator {
  private operationTail: Promise<void> = Promise.resolve();

  constructor(
    readonly queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
    readonly persistence: AutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
    readonly maxConflictRetries = 4,
  ) {
    if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue)) throw new ArgumentError("portfolio evidence atomic coordinator requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.writeIfUnchanged !== "function") throw new ArgumentError("portfolio evidence atomic coordinator requires compare-and-swap persistence");
    boundedInteger("portfolio evidence atomic coordinator maxConflictRetries", maxConflictRetries, 1, 16);
  }

  async restore(): Promise<{ status: "empty" | "restored"; snapshot_digest: string | null; items: number }> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        this.restoreEmpty();
        return { status: "empty", snapshot_digest: null, items: 0 };
      }
      this.queue.restore(snapshot);
      return { status: "restored", snapshot_digest: snapshot.snapshot_digest, items: snapshot.items.length };
    });
  }

  async snapshot(): Promise<AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.snapshot();
    });
  }

  async get(workId: string): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem | null> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.get(workId);
    });
  }

  async pending(limit = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem[]> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.pending(limit, now);
    });
  }

  async rows(): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem[]> {
    return this.enqueue(async () => {
      await this.loadLatest();
      return this.queue.rows();
    });
  }

  async admit(input: Parameters<InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue["admit"]>[0]): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.admit(input));
  }

  async claim(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem | null> {
    return this.transact((queue) => queue.claim(workId, workerId, leaseMs, now));
  }

  async renew(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.renew(workId, workerId, leaseMs, now));
  }

  async complete(workId: string, workerId: string, result: { status: "completed" | "awaiting_evaluation"; resultDigest: string }, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.complete(workId, workerId, result, now));
  }

  async fail(workId: string, workerId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass, retryable: boolean, resultDigest: string | null = null, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.fail(workId, workerId, errorClass, retryable, resultDigest, now));
  }

  async reconcile(workId: string, workerId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass = "rehydration_missing", now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.reconcile(workId, workerId, errorClass, now));
  }

  async requeue(workId: string, now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.requeue(workId, now));
  }

  async cancel(workId: string, errorClass: AutonomousWorkflowPortfolioEvidenceWorkFailureClass = "unknown", now = Date.now()): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> {
    return this.transact((queue) => queue.cancel(workId, errorClass, now));
  }

  async bindCheckpointDigest(jobId: string, checkpoint: string | null, now = Date.now()): Promise<number> {
    return this.transact((queue) => queue.bindCheckpointDigest(jobId, checkpoint, now));
  }

  async reclaimExpired(now = Date.now(), limit = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS): Promise<AutonomousWorkflowPortfolioEvidenceWorkItem[]> {
    return this.transact((queue) => queue.reclaimExpired(now, limit));
  }

  private restoreEmpty(): void {
    this.queue.restore(new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue(this.queue.maxItems).snapshot());
  }

  private async loadLatest(): Promise<string | null> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) {
      this.restoreEmpty();
      return null;
    }
    this.queue.restore(snapshot);
    return snapshot.snapshot_digest;
  }

  private async transact<T>(operation: (queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue) => T): Promise<T> {
    return this.enqueue(async () => {
      for (let attempt = 0; attempt < this.maxConflictRetries; attempt += 1) {
        const expected = await this.loadLatest();
        const before = this.queue.snapshot();
        const result = operation(this.queue);
        const after = this.queue.snapshot();
        if (after.snapshot_digest === before.snapshot_digest) return result;
        const committed = await this.persistence.writeIfUnchanged!(expected, after);
        if (committed) return result;
      }
      throw new ArgumentError("portfolio evidence work queue atomic transition conflicted repeatedly; reload before continuing");
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

/** In-memory fenced persistence adapter for local workers and tests. */
export class InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence implements AutonomousWorkflowPortfolioEvidenceWorkQueuePersistence {
  private snapshotValue: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot | null = null;

  read(): AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot | null {
    return this.snapshotValue === null ? null : clone(this.snapshotValue);
  }

  write(snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): void {
    this.snapshotValue = clone(validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(snapshot));
  }

  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot): boolean {
    const current = this.snapshotValue?.snapshot_digest ?? null;
    if (current !== expectedSnapshotDigest) return false;
    this.snapshotValue = clone(validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(snapshot));
    return true;
  }
}

/** Runs caller-owned item execution through the fenced queue without retaining its values. */
export class AutonomousWorkflowPortfolioEvidenceWorkWorker {
  constructor(
    readonly queue: InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
    readonly execute: (item: AutonomousWorkflowPortfolioEvidenceWorkItem, context: { renew: (leaseMs?: number, now?: number) => AutonomousWorkflowPortfolioEvidenceWorkItem }) => Promise<AutonomousWorkflowPortfolioEvidenceWorkExecution> | AutonomousWorkflowPortfolioEvidenceWorkExecution,
  ) {
    if (!(queue instanceof InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue)) throw new ArgumentError("portfolio evidence work worker requires a typed queue");
    if (typeof execute !== "function") throw new ArgumentError("portfolio evidence work worker requires an executor");
  }

  async run(options: { workerId?: string; limit?: number; leaseMs?: number; now?: number; signal?: { readonly aborted: boolean } } = {}): Promise<AutonomousWorkflowPortfolioEvidenceWorkWorkerRun> {
    const workerId = identifier("portfolio evidence work worker_id", options.workerId ?? "portfolio-evidence-worker");
    const limit = boundedInteger("portfolio evidence work worker limit", options.limit ?? MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS);
    const leaseMs = boundedInteger("portfolio evidence work worker lease_ms", options.leaseMs ?? 30_000, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(options.now);
    const currentTime = () => options.now === undefined ? Date.now() : time;
    const rows: AutonomousWorkflowPortfolioEvidenceWorkWorkerRow[] = [];
    const expiredLeases = this.queue.reclaimExpired(time, limit);
    for (const expired of expiredLeases) rows.push({ work_id: expired.work_id, item_id: expired.item_id, domain: expired.domain, outcome: "reconciliation_required", attempts: expired.attempts, result_digest: expired.result_digest, error_class: expired.failure_class, lease_retained: false });
    const remaining = Math.max(0, limit - rows.length);
    const activeLeases = this.queue.rows().filter((item) => item.status === "leased" && item.lease_until !== null && item.lease_until > time).slice(0, remaining);
    const candidates = [...activeLeases, ...this.queue.pending(Math.max(1, remaining - activeLeases.length), time)].slice(0, remaining);
    for (const candidate of candidates) {
      if (options.signal?.aborted) break;
      const claimed = this.queue.claim(candidate.work_id, workerId, leaseMs, time);
      if (!claimed) {
        const current = this.queue.get(candidate.work_id);
        rows.push({ work_id: candidate.work_id, item_id: candidate.item_id, domain: candidate.domain, outcome: current?.status === "reconciliation_required" ? "reconciliation_required" : "leased_elsewhere", attempts: current?.attempts ?? candidate.attempts, result_digest: current?.result_digest ?? null, error_class: current?.failure_class ?? null, lease_retained: false });
        continue;
      }
      try {
        const outcome = await this.execute(claimed, { renew: (renewLeaseMs = leaseMs, renewAt = currentTime()) => this.queue.renew(claimed.work_id, workerId, renewLeaseMs, renewAt) });
        if (!outcome || !["completed", "awaiting_evaluation", "failed", "reconciliation_required"].includes(outcome.status)) throw new ArgumentError("portfolio evidence work executor returned a malformed status");
        if (outcome.status === "completed" || outcome.status === "awaiting_evaluation") {
          const finished = this.queue.complete(claimed.work_id, workerId, { status: outcome.status, resultDigest: outcome.result_digest ?? "" }, currentTime());
          rows.push({ work_id: finished.work_id, item_id: finished.item_id, domain: finished.domain, outcome: outcome.status, attempts: finished.attempts, result_digest: finished.result_digest, error_class: finished.failure_class, lease_retained: false });
        } else if (outcome.status === "reconciliation_required") {
          const reconciled = this.queue.reconcile(claimed.work_id, workerId, outcome.error_class ?? "rehydration_missing", currentTime());
          rows.push({ work_id: reconciled.work_id, item_id: reconciled.item_id, domain: reconciled.domain, outcome: "reconciliation_required", attempts: reconciled.attempts, result_digest: reconciled.result_digest, error_class: reconciled.failure_class, lease_retained: false });
        } else {
          const failed = this.queue.fail(claimed.work_id, workerId, outcome.error_class ?? "executor_error", outcome.retryable === true, outcome.result_digest, currentTime());
          rows.push({ work_id: failed.work_id, item_id: failed.item_id, domain: failed.domain, outcome: failed.status === "queued" ? "retry_scheduled" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, error_class: failed.last_error_class, lease_retained: false });
        }
      } catch {
        const failed = this.queue.fail(claimed.work_id, workerId, "executor_error", true, null, currentTime());
        rows.push({ work_id: failed.work_id, item_id: failed.item_id, domain: failed.domain, outcome: failed.status === "queued" ? "retry_scheduled" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, error_class: failed.last_error_class, lease_retained: false });
      }
    }
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
      worker_id: workerId,
      inspected: rows.length,
      completed: rows.filter((row) => row.outcome === "completed").length,
      awaiting_evaluation: rows.filter((row) => row.outcome === "awaiting_evaluation").length,
      retried: rows.filter((row) => row.outcome === "retry_scheduled").length,
      failed: rows.filter((row) => row.outcome === "failed").length,
      reconciled: rows.filter((row) => row.outcome === "reconciliation_required").length,
      leased_elsewhere: rows.filter((row) => row.outcome === "leased_elsewhere").length,
      rows,
      retention: "metadata_only_receipts_and_digests_no_values",
      secret_material: "never_returned",
    };
  }
}

/**
 * Distributed-safe worker that performs every queue transition through a CAS coordinator.
 * The executor still receives only one transient work identity and must rehydrate private task,
 * source, evaluator, and provider state from the embedding application.
 */
export class AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker {
  constructor(
    readonly coordinator: AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator,
    readonly execute: (item: AutonomousWorkflowPortfolioEvidenceWorkItem, context: { renew: (leaseMs?: number, now?: number) => Promise<AutonomousWorkflowPortfolioEvidenceWorkItem> }) => Promise<AutonomousWorkflowPortfolioEvidenceWorkExecution> | AutonomousWorkflowPortfolioEvidenceWorkExecution,
  ) {
    if (!(coordinator instanceof AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator)) throw new ArgumentError("portfolio evidence atomic worker requires a CAS coordinator");
    if (typeof execute !== "function") throw new ArgumentError("portfolio evidence atomic worker requires an executor");
  }

  async run(options: { workerId?: string; limit?: number; leaseMs?: number; now?: number; signal?: { readonly aborted: boolean } } = {}): Promise<AutonomousWorkflowPortfolioEvidenceWorkWorkerRun> {
    const workerId = identifier("portfolio evidence atomic worker_id", options.workerId ?? "portfolio-evidence-atomic-worker");
    const limit = boundedInteger("portfolio evidence atomic worker limit", options.limit ?? MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_ITEMS);
    const leaseMs = boundedInteger("portfolio evidence atomic worker lease_ms", options.leaseMs ?? 30_000, 1, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_LEASE_MS);
    const time = nowMs(options.now);
    const currentTime = () => options.now === undefined ? Date.now() : time;
    const rows: AutonomousWorkflowPortfolioEvidenceWorkWorkerRow[] = [];
    const expiredLeases = await this.coordinator.reclaimExpired(time, limit);
    for (const expired of expiredLeases) rows.push({ work_id: expired.work_id, item_id: expired.item_id, domain: expired.domain, outcome: "reconciliation_required", attempts: expired.attempts, result_digest: expired.result_digest, error_class: expired.failure_class, lease_retained: false });
    const remaining = Math.max(0, limit - rows.length);
    const currentRows = await this.coordinator.rows();
    const activeLeases = currentRows.filter((item) => item.status === "leased" && item.lease_until !== null && item.lease_until > time).slice(0, remaining);
    const pending = remaining > 0 ? await this.coordinator.pending(Math.max(1, remaining - activeLeases.length), time) : [];
    const candidates = [...activeLeases, ...pending].slice(0, remaining);
    for (const candidate of candidates) {
      if (options.signal?.aborted) break;
      const claimed = await this.coordinator.claim(candidate.work_id, workerId, leaseMs, time);
      if (!claimed) {
        const current = await this.coordinator.get(candidate.work_id);
        rows.push({ work_id: candidate.work_id, item_id: candidate.item_id, domain: candidate.domain, outcome: current?.status === "reconciliation_required" ? "reconciliation_required" : "leased_elsewhere", attempts: current?.attempts ?? candidate.attempts, result_digest: current?.result_digest ?? null, error_class: current?.failure_class ?? null, lease_retained: false });
        continue;
      }
      try {
        const outcome = await this.execute(claimed, { renew: (renewLeaseMs = leaseMs, renewAt = currentTime()) => this.coordinator.renew(claimed.work_id, workerId, renewLeaseMs, renewAt) });
        if (!outcome || !["completed", "awaiting_evaluation", "failed", "reconciliation_required"].includes(outcome.status)) throw new ArgumentError("portfolio evidence atomic executor returned a malformed status");
        if (outcome.status === "completed" || outcome.status === "awaiting_evaluation") {
          const finished = await this.coordinator.complete(claimed.work_id, workerId, { status: outcome.status, resultDigest: outcome.result_digest ?? "" }, currentTime());
          rows.push({ work_id: finished.work_id, item_id: finished.item_id, domain: finished.domain, outcome: outcome.status, attempts: finished.attempts, result_digest: finished.result_digest, error_class: finished.failure_class, lease_retained: false });
        } else if (outcome.status === "reconciliation_required") {
          const reconciled = await this.coordinator.reconcile(claimed.work_id, workerId, outcome.error_class ?? "rehydration_missing", currentTime());
          rows.push({ work_id: reconciled.work_id, item_id: reconciled.item_id, domain: reconciled.domain, outcome: "reconciliation_required", attempts: reconciled.attempts, result_digest: reconciled.result_digest, error_class: reconciled.failure_class, lease_retained: false });
        } else {
          const failed = await this.coordinator.fail(claimed.work_id, workerId, outcome.error_class ?? "executor_error", outcome.retryable === true, outcome.result_digest, currentTime());
          rows.push({ work_id: failed.work_id, item_id: failed.item_id, domain: failed.domain, outcome: failed.status === "queued" ? "retry_scheduled" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, error_class: failed.last_error_class, lease_retained: false });
        }
      } catch {
        try {
          const failed = await this.coordinator.fail(claimed.work_id, workerId, "executor_error", true, null, currentTime());
          rows.push({ work_id: failed.work_id, item_id: failed.item_id, domain: failed.domain, outcome: failed.status === "queued" ? "retry_scheduled" : "failed", attempts: failed.attempts, result_digest: failed.result_digest, error_class: failed.last_error_class, lease_retained: false });
        } catch {
          const current = await this.coordinator.get(claimed.work_id);
          rows.push({ work_id: claimed.work_id, item_id: claimed.item_id, domain: claimed.domain, outcome: current?.status === "reconciliation_required" ? "reconciliation_required" : "leased_elsewhere", attempts: current?.attempts ?? claimed.attempts, result_digest: current?.result_digest ?? null, error_class: current?.failure_class ?? "executor_error", lease_retained: false });
        }
      }
    }
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_WORK_QUEUE_SCHEMA,
      worker_id: workerId,
      inspected: rows.length,
      completed: rows.filter((row) => row.outcome === "completed").length,
      awaiting_evaluation: rows.filter((row) => row.outcome === "awaiting_evaluation").length,
      retried: rows.filter((row) => row.outcome === "retry_scheduled").length,
      failed: rows.filter((row) => row.outcome === "failed").length,
      reconciled: rows.filter((row) => row.outcome === "reconciliation_required").length,
      leased_elsewhere: rows.filter((row) => row.outcome === "leased_elsewhere").length,
      rows,
      retention: "metadata_only_receipts_and_digests_no_values",
      secret_material: "never_returned",
    };
  }
}
