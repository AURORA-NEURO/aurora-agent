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
import type { JsonObject, JsonValue } from "./types.js";
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
      this.rehydratedByRequest.set(record.request_digest, cloneRecord(record));
      if (record.replay_key_digest !== null) {
        const priorKey = this.rehydratedByReplayKey.get(record.replay_key_digest);
        if (priorKey && priorKey.request_digest !== record.request_digest) throw new ProviderRuntimeError("capability journal contains a replay-key collision");
        this.rehydratedByReplayKey.set(record.replay_key_digest, cloneRecord(record));
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
