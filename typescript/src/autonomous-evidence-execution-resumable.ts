import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousEvidencePlan,
} from "./autonomous-evidence.js";
import {
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionResult,
  type AutonomousEvidenceExecutionOptions,
} from "./autonomous-evidence-execution.js";
import type { AutonomousEvidenceAcquisitionRequest } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Restart-safe metadata handoff for reviewed evidence source execution. */
export const AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-checkpoint/0.1" as const;
export const AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-resumable-result/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES = 128_000;

export type AutonomousEvidenceExecutionCheckpointStatus =
  | "approval_required"
  | "blocked"
  | "dispatch_pending"
  | "awaiting_evaluation"
  | "partial"
  | "failed"
  | "reconciliation_required"
  | "completed";

export interface AutonomousEvidenceExecutionCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA;
  job_id: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_digest: string;
  readiness_report_digest: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  runtime_status: "completed" | "partial" | "awaiting_evaluation" | "failed" | "reconciliation_required" | null;
  runtime_result_digest: string | null;
  completed_request_count: number;
  pending_request_count: number;
  accepted_request_count: number;
  checkpoint_digest: string;
  retention: "metadata_only;requests_readiness_and_source_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceExecutionCheckpointStore {
  read(): Promise<AutonomousEvidenceExecutionCheckpointJSON | null> | AutonomousEvidenceExecutionCheckpointJSON | null;
  write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> | void;
  writeIfUnchanged?(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceExecutionCheckpointTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceExecutionTransactionalCheckpointTextStore extends AutonomousEvidenceExecutionCheckpointTextStore {
  writeIfUnchanged(expectedCheckpointDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceExecutionResumableRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA;
  job_id: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  checkpoint_digest: string;
  execution_plan_digest: string;
  evidence_result_digest: string | null;
  replayed: boolean;
  retention: "metadata_only;source_values_and_runtime_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceExecutionResumableRun {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA;
  job_id: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  checkpoint: AutonomousEvidenceExecutionCheckpointJSON;
  result: AutonomousEvidenceExecutionResult | null;
  replayed: boolean;
  toJSON(): AutonomousEvidenceExecutionResumableRunProjection;
}

const RETENTION = "metadata_only;requests_readiness_and_source_values_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;
const RESULT_RETENTION = "metadata_only;source_values_and_runtime_payloads_caller_owned" as const;
const STATUSES: readonly AutonomousEvidenceExecutionCheckpointStatus[] = [
  "approval_required",
  "blocked",
  "dispatch_pending",
  "awaiting_evaluation",
  "partial",
  "failed",
  "reconciliation_required",
  "completed",
];
const RUNTIME_STATUSES = ["completed", "partial", "awaiting_evaluation", "failed", "reconciliation_required"] as const;

interface AutonomousEvidenceExecutionCheckpointPayload {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA;
  job_id: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_digest: string;
  readiness_report_digest: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  runtime_status: (typeof RUNTIME_STATUSES)[number] | null;
  runtime_result_digest: string | null;
  completed_request_count: number;
  pending_request_count: number;
  accepted_request_count: number;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(name: string, value: unknown, nullable = false): string | null {
  if (nullable && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function allowedKeys(value: Record<string, unknown>, allowed: readonly string[], name: string): void {
  const accepted = new Set(allowed);
  if (Object.keys(value).some((key) => !accepted.has(key))) throw new ArgumentError(`${name} contains unsupported fields`);
}

function normalizeRequests(requests: readonly AutonomousEvidenceAcquisitionRequest[]): JsonObject[] {
  if (!Array.isArray(requests) || requests.length < 1 || requests.length > 128) throw new ArgumentError("evidence execution checkpoint requests are outside their bound");
  return requests.map((request, index) => {
    if (!isObject(request)) throw new ArgumentError(`evidence execution checkpoint request ${index} is malformed`);
    return {
      requirement_id: identifier(`evidence execution checkpoint request ${index} requirement_id`, request.requirement_id),
      source_id: identifier(`evidence execution checkpoint request ${index} source_id`, request.source_id),
      source_digest: digest(`evidence execution checkpoint request ${index} source_digest`, request.source_digest, true),
      request_id: request.request_id === undefined || request.request_id === null ? null : identifier(`evidence execution checkpoint request ${index} request_id`, request.request_id),
      metadata_digest: digestJsonSync(request.metadata ?? {}),
    };
  });
}

function requestsDigest(requests: readonly AutonomousEvidenceAcquisitionRequest[]): string {
  return digestJsonSync({ schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA, requests: normalizeRequests(requests) });
}

function checkpointPayload(input: AutonomousEvidenceExecutionCheckpointPayload): AutonomousEvidenceExecutionCheckpointPayload {
  return input;
}

function checkpointDigest(input: AutonomousEvidenceExecutionCheckpointPayload): string {
  return digestJsonSync(checkpointPayload(input));
}

function checkpointFor(input: {
  jobId: string;
  executionPlan: AutonomousEvidenceExecutionPlan;
  requestDigest: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  result?: AutonomousEvidenceExecutionResult | null;
}): AutonomousEvidenceExecutionCheckpointJSON {
  const runtime = input.result?.runtime.toJSON() ?? null;
  const payload = checkpointPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: identifier("evidence execution checkpoint job_id", input.jobId),
    evidence_plan_digest: input.executionPlan.evidence_plan_digest,
    execution_plan_digest: input.executionPlan.plan_digest,
    request_digest: input.requestDigest,
    readiness_report_digest: input.executionPlan.readiness.report_digest,
    status: input.status,
    runtime_status: runtime?.status ?? null,
    runtime_result_digest: runtime?.result_digest ?? null,
    completed_request_count: runtime?.completed_requirement_ids.length ?? 0,
    pending_request_count: (runtime?.pending_evaluation_requirement_ids.length ?? 0) + (runtime?.missing_requirement_ids.length ?? 0),
    accepted_request_count: runtime?.assessments.filter((assessment) => assessment.verdict === "accepted").length ?? 0,
  });
  return {
    ...payload,
    checkpoint_digest: checkpointDigest(payload),
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  };
}

function statusForResult(result: AutonomousEvidenceExecutionResult): AutonomousEvidenceExecutionCheckpointStatus {
  switch (result.status) {
    case "completed": return "completed";
    case "awaiting_evaluation": return "awaiting_evaluation";
    case "partial": return "partial";
    case "failed": return "failed";
    case "reconciliation_required": return "reconciliation_required";
    default: return "reconciliation_required";
  }
}

function validateCheckpoint(value: unknown): AutonomousEvidenceExecutionCheckpointJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA) throw new ArgumentError("evidence execution checkpoint schema is invalid");
  allowedKeys(value, ["schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest", "readiness_report_digest", "status", "runtime_status", "runtime_result_digest", "completed_request_count", "pending_request_count", "accepted_request_count", "checkpoint_digest", "retention", "secret_material"], "evidence execution checkpoint");
  const status = value.status as AutonomousEvidenceExecutionCheckpointStatus;
  if (!STATUSES.includes(status)) throw new ArgumentError("evidence execution checkpoint status is invalid");
  const runtimeStatus = value.runtime_status === null ? null : value.runtime_status as (typeof RUNTIME_STATUSES)[number];
  if (runtimeStatus !== null && !RUNTIME_STATUSES.includes(runtimeStatus)) throw new ArgumentError("evidence execution checkpoint runtime status is invalid");
  const runtimeResultDigest = digest("evidence execution checkpoint runtime_result_digest", value.runtime_result_digest, true);
  const normalized = checkpointPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: identifier("evidence execution checkpoint job_id", value.job_id),
    evidence_plan_digest: digest("evidence execution checkpoint evidence_plan_digest", value.evidence_plan_digest) as string,
    execution_plan_digest: digest("evidence execution checkpoint execution_plan_digest", value.execution_plan_digest) as string,
    request_digest: digest("evidence execution checkpoint request_digest", value.request_digest) as string,
    readiness_report_digest: digest("evidence execution checkpoint readiness_report_digest", value.readiness_report_digest) as string,
    status,
    runtime_status: runtimeStatus,
    runtime_result_digest: runtimeResultDigest,
    completed_request_count: integer("evidence execution checkpoint completed_request_count", value.completed_request_count, 0, 128),
    pending_request_count: integer("evidence execution checkpoint pending_request_count", value.pending_request_count, 0, 256),
    accepted_request_count: integer("evidence execution checkpoint accepted_request_count", value.accepted_request_count, 0, 128),
  });
  const hasRuntime = normalized.runtime_status !== null || normalized.runtime_result_digest !== null || normalized.completed_request_count > 0 || normalized.pending_request_count > 0 || normalized.accepted_request_count > 0;
  if (["approval_required", "blocked", "dispatch_pending"].includes(status) && hasRuntime) throw new ArgumentError("pre-dispatch evidence execution checkpoint cannot contain runtime state");
  if (status === "completed" && (normalized.runtime_status !== "completed" || normalized.runtime_result_digest === null)) throw new ArgumentError("completed evidence execution checkpoint requires a completed runtime digest");
  if (status !== "completed" && status !== "approval_required" && status !== "blocked" && status !== "dispatch_pending" && normalized.runtime_result_digest === null) throw new ArgumentError("post-dispatch evidence execution checkpoint requires a runtime digest");
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evidence execution checkpoint retention contract is invalid");
  const observedDigest = digest("evidence execution checkpoint checkpoint_digest", value.checkpoint_digest) as string;
  if (checkpointDigest(normalized) !== observedDigest) throw new ArgumentError("evidence execution checkpoint digest is invalid");
  const result = { ...normalized, checkpoint_digest: observedDigest, retention: RETENTION, secret_material: SECRET_MATERIAL };
  if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES) throw new ArgumentError("evidence execution checkpoint exceeds its bound");
  return clone(result);
}

export function validateAutonomousEvidenceExecutionCheckpoint(value: unknown): AutonomousEvidenceExecutionCheckpointJSON {
  return validateCheckpoint(value);
}

function encodeCheckpoint(value: AutonomousEvidenceExecutionCheckpointJSON): string {
  return canonicalJson(validateCheckpoint(value));
}

export class InMemoryAutonomousEvidenceExecutionCheckpointStore implements AutonomousEvidenceExecutionCheckpointStore {
  private checkpoint: AutonomousEvidenceExecutionCheckpointJSON | null = null;

  read(): AutonomousEvidenceExecutionCheckpointJSON | null {
    return this.checkpoint === null ? null : clone(this.checkpoint);
  }

  write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): void {
    this.checkpoint = clone(validateCheckpoint(checkpoint));
  }

  writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): boolean {
    const current = this.checkpoint?.checkpoint_digest ?? null;
    if (current !== expectedCheckpointDigest) return false;
    this.checkpoint = clone(validateCheckpoint(checkpoint));
    return true;
  }
}

export class JsonAutonomousEvidenceExecutionCheckpointStore implements AutonomousEvidenceExecutionCheckpointStore {
  constructor(protected readonly store: AutonomousEvidenceExecutionCheckpointTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evidence execution checkpoint JSON store is malformed");
  }

  async read(): Promise<AutonomousEvidenceExecutionCheckpointJSON | null> {
    const value = await this.store.read();
    if (value === null) return null;
    if (typeof value !== "string" || bytes(value) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES) throw new ArgumentError("evidence execution checkpoint JSON exceeds its bound");
    let parsed: unknown;
    try { parsed = JSON.parse(value); } catch { throw new ArgumentError("evidence execution checkpoint JSON is invalid"); }
    return validateCheckpoint(parsed);
  }

  async write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> {
    await this.store.write(encodeCheckpoint(checkpoint));
  }
}

export class TransactionalJsonAutonomousEvidenceExecutionCheckpointStore extends JsonAutonomousEvidenceExecutionCheckpointStore {
  private readonly transactionalStore: AutonomousEvidenceExecutionTransactionalCheckpointTextStore;

  constructor(store: AutonomousEvidenceExecutionTransactionalCheckpointTextStore) {
    super(store);
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<boolean> {
    return this.transactionalStore.writeIfUnchanged(expectedCheckpointDigest, encodeCheckpoint(checkpoint));
  }
}

export class WebStorageAutonomousEvidenceExecutionCheckpointTextStore implements AutonomousEvidenceExecutionCheckpointTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("evidence execution checkpoint web storage is malformed");
    if (!key || key.length > 256) throw new ArgumentError("evidence execution checkpoint web storage key is malformed");
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

export class AutonomousEvidenceExecutionResumableController {
  private expectedCheckpointDigest: string | null = null;
  private restored = false;
  private mutation: Promise<void> = Promise.resolve();
  private checkpoint: AutonomousEvidenceExecutionCheckpointJSON | null = null;

  constructor(readonly controller: AutonomousEvidenceExecutionController, readonly persistence: AutonomousEvidenceExecutionCheckpointStore, readonly jobId: string) {
    if (!(controller instanceof AutonomousEvidenceExecutionController)) throw new ArgumentError("evidence execution resumable controller requires a typed execution controller");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("evidence execution resumable controller persistence is malformed");
    this.jobId = identifier("evidence execution resumable job_id", jobId);
  }

  async restore(): Promise<{ status: "empty" | "restored"; checkpoint_digest: string | null }> {
    return this.serial(() => this.restoreInternal());
  }

  async run(executionPlan: AutonomousEvidenceExecutionPlan, evidencePlan: AutonomousEvidencePlan, requests: readonly AutonomousEvidenceAcquisitionRequest[], options: AutonomousEvidenceExecutionOptions & { resumeAfterReconciliation?: boolean } = {}): Promise<AutonomousEvidenceExecutionResumableRun> {
    return this.serial(async () => {
      await this.restoreInternal();
      if (!(executionPlan instanceof AutonomousEvidenceExecutionPlan) || !(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution resumable run requires typed plans");
      const requestDigest = requestsDigest(requests);
      const current = this.checkpoint;
      if (current !== null && (current.evidence_plan_digest !== executionPlan.evidence_plan_digest || current.execution_plan_digest !== executionPlan.plan_digest || current.request_digest !== requestDigest || current.readiness_report_digest !== executionPlan.readiness.report_digest)) throw new ArgumentError("evidence execution checkpoint is bound to a different plan, request set, or readiness report");
      if (["completed", "awaiting_evaluation", "partial", "failed"].includes(current?.status ?? "") && options.journal === undefined) return this.resultFromCheckpoint(current!);
      if (["dispatch_pending", "reconciliation_required"].includes(current?.status ?? "") && options.resumeAfterReconciliation !== true) return this.resultFromCheckpoint(current!);
      if (options.approveSourceDispatch !== true) {
        const gated = checkpointFor({ jobId: this.jobId, executionPlan, requestDigest, status: executionPlan.status === "ready_for_review" ? "approval_required" : "blocked" });
        await this.commit(gated);
        return this.resultFromCheckpoint(gated);
      }
      if (executionPlan.status !== "ready_for_review") {
        const blocked = checkpointFor({ jobId: this.jobId, executionPlan, requestDigest, status: "blocked" });
        await this.commit(blocked);
        return this.resultFromCheckpoint(blocked);
      }
      const pending = checkpointFor({ jobId: this.jobId, executionPlan, requestDigest, status: "dispatch_pending" });
      await this.commit(pending);
      const { resumeAfterReconciliation: _resumeAfterReconciliation, ...executeOptions } = options;
      try {
        const result = await this.controller.execute(executionPlan, evidencePlan, requests, executeOptions);
        const settled = checkpointFor({ jobId: this.jobId, executionPlan, requestDigest, status: statusForResult(result), result });
        await this.commit(settled);
        return this.resultFromCheckpoint(settled, result, result.runtime.json.receipts.some((receipt) => receipt.replay === "replayed"));
      } catch (error) {
        const reconciliation = checkpointFor({ jobId: this.jobId, executionPlan, requestDigest, status: "reconciliation_required" });
        await this.commit(reconciliation);
        throw error;
      }
    });
  }

  private resultFromCheckpoint(checkpoint: AutonomousEvidenceExecutionCheckpointJSON, result: AutonomousEvidenceExecutionResult | null = null, replayed = false): AutonomousEvidenceExecutionResumableRun {
    const projection = {
      schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA,
      job_id: this.jobId,
      status: checkpoint.status,
      checkpoint_digest: checkpoint.checkpoint_digest,
      execution_plan_digest: checkpoint.execution_plan_digest,
      evidence_result_digest: result?.result_digest ?? null,
      replayed,
      retention: RESULT_RETENTION,
      secret_material: SECRET_MATERIAL,
    } satisfies AutonomousEvidenceExecutionResumableRunProjection;
    return {
      schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA,
      job_id: this.jobId,
      status: checkpoint.status,
      checkpoint: clone(checkpoint),
      result,
      replayed,
      toJSON: () => clone(projection),
    };
  }

  private async commit(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> {
    const validated = validateCheckpoint(checkpoint);
    if (this.persistence.writeIfUnchanged !== undefined) {
      const committed = await this.persistence.writeIfUnchanged(this.expectedCheckpointDigest, validated);
      if (!committed) throw new ArgumentError("evidence execution checkpoint is stale; another worker committed after restore");
    } else {
      await this.persistence.write(validated);
    }
    this.checkpoint = clone(validated);
    this.expectedCheckpointDigest = validated.checkpoint_digest;
  }

  private async restoreInternal(): Promise<{ status: "empty" | "restored"; checkpoint_digest: string | null }> {
    if (this.restored) return { status: this.checkpoint === null ? "empty" : "restored", checkpoint_digest: this.expectedCheckpointDigest } as const;
    const restored = await this.persistence.read();
    this.checkpoint = restored === null ? null : validateCheckpoint(restored);
    if (this.checkpoint !== null && this.checkpoint.job_id !== this.jobId) throw new ArgumentError("evidence execution checkpoint belongs to a different job");
    this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
    this.restored = true;
    return { status: this.checkpoint === null ? "empty" : "restored", checkpoint_digest: this.expectedCheckpointDigest } as const;
  }

  private async serial<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.mutation.then(operation, operation);
    this.mutation = next.then(() => undefined, () => undefined);
    return next;
  }
}
