import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import type {
  AutonomousAgent,
  AutonomousEvidenceBackedRunOptions,
  AutonomousEvidenceBackedRunPreflight,
  AutonomousEvidenceBackedRunResult,
  AutonomousEvidenceBackedRunStatus,
  AutonomousPromptChunk,
  AutonomousRunResult,
} from "./autonomous.js";
import type {
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionResult,
} from "./autonomous-evidence-execution.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only restart boundary for one reviewed evidence-to-provider operation. */
export const AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-checkpoint/0.1" as const;
export const AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-resumable-result/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES = 64_000;

export type AutonomousEvidenceBackedCheckpointStatus =
  | "evidence_review_required"
  | "evidence_blocked"
  | "evidence_incomplete"
  | "provider_pending"
  | "provider_reconciliation_required"
  | "completed";

export interface AutonomousEvidenceBackedCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA;
  job_id: string;
  task_digest: string;
  request_digest: string;
  run_policy_digest: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  evidence_result_digest: string | null;
  prompt_projection_digest: string | null;
  provider_result_digest: string | null;
  provider_status: AutonomousRunResult["status"] | null;
  status: AutonomousEvidenceBackedCheckpointStatus;
  checkpoint_digest: string;
  retention: "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceBackedCheckpointStore {
  read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> | AutonomousEvidenceBackedCheckpointJSON | null;
  write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> | void;
  /** Optional atomic fence; false means another worker committed after this controller restored. */
  writeIfUnchanged?(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceBackedCheckpointTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceBackedTransactionalCheckpointTextStore extends AutonomousEvidenceBackedCheckpointTextStore {
  writeIfUnchanged(expectedCheckpointDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceBackedProviderRehydrationContext {
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  executionPlan: AutonomousEvidenceExecutionPlan;
  evidence: AutonomousEvidenceExecutionResult;
  promptContext: readonly AutonomousPromptChunk[];
}

export type AutonomousEvidenceBackedProviderRehydrator = (
  context: AutonomousEvidenceBackedProviderRehydrationContext,
) => AutonomousRunResult | null | Promise<AutonomousRunResult | null>;

export interface AutonomousEvidenceBackedResumableExecutionOptions extends Omit<AutonomousEvidenceBackedRunOptions, "beforeProviderRun" | "providerRunOverride"> {
  jobId: string;
  checkpoint?: AutonomousEvidenceBackedCheckpointJSON;
  checkpointSink: (checkpoint: AutonomousEvidenceBackedCheckpointJSON) => Promise<void> | void;
  /** Rehydrate a prior provider result by its caller-owned digest; returning null requires reconciliation. */
  rehydrateProviderRun?: AutonomousEvidenceBackedProviderRehydrator;
  /** Provider dispatch after a provider_pending checkpoint is always an explicit resume decision. */
  resumeProvider?: boolean;
}

export interface AutonomousEvidenceBackedResumableRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA;
  status: AutonomousEvidenceBackedResumableStatus;
  job_id: string;
  checkpoint_digest: string;
  result_status: AutonomousEvidenceBackedRunStatus;
  provider_rehydrated: boolean;
  retention: "metadata_only;raw_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export type AutonomousEvidenceBackedResumableStatus =
  | AutonomousEvidenceBackedRunStatus
  | "provider_pending"
  | "provider_reconciliation_required";

export interface AutonomousEvidenceBackedResumableRun {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA;
  status: AutonomousEvidenceBackedResumableStatus;
  job_id: string;
  result: AutonomousEvidenceBackedRunResult;
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  provider_rehydrated: boolean;
  toJSON(): AutonomousEvidenceBackedResumableRunProjection;
}

export interface AutonomousEvidenceBackedControllerProjection extends JsonObject {
  schema: "bioprism-typescript-autonomous-evidence-backed-controller/0.1";
  status: "empty" | "restored" | "flushed" | "completed" | "provider_pending" | "provider_reconciliation_required" | "evidence_incomplete";
  job_id: string;
  checkpoint_digest: string | null;
  persisted: true;
  retention: "metadata_only_task_request_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceBackedControllerRun {
  controller: AutonomousEvidenceBackedControllerProjection;
  run: AutonomousEvidenceBackedResumableRun;
}

export type AutonomousEvidenceBackedControllerRunOptions = Omit<
  AutonomousEvidenceBackedResumableExecutionOptions,
  "jobId" | "checkpoint" | "checkpointSink"
>;

const RETENTION = "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;
const RESULT_RETENTION = "metadata_only;raw_evidence_and_provider_payloads_caller_owned" as const;
const CONTROLLER_RETENTION = "metadata_only_task_request_evidence_and_provider_payloads_caller_owned" as const;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function optionalDigest(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return digest(name, value);
}

function allowedKeys(value: Record<string, unknown>, allowed: readonly string[], name: string): void {
  const set = new Set(allowed);
  if (Object.keys(value).some((key) => !set.has(key))) throw new ArgumentError(`${name} contains unsupported fields`);
}

async function requestDigest(requests: AutonomousEvidenceBackedResumableExecutionOptions["requests"]): Promise<string> {
  return digestJson({ schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA, requests });
}

async function runPolicyDigest(options: AutonomousEvidenceBackedResumableExecutionOptions): Promise<string> {
  const run = options.run ?? {};
  return digestJson({
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    domain: run.domain ?? null,
    capability: run.capability ?? null,
    candidates_digest: run.candidates === undefined ? null : await digestJson(run.candidates),
    context_digest: run.context === undefined ? null : await digestJson(run.context),
    content_parts_digest: run.contentParts === undefined ? null : await digestJson(run.contentParts),
    tools: run.tools?.map((tool) => tool.name).sort() ?? null,
    max_input_tokens: run.maxInputTokens ?? null,
    max_output_tokens: run.maxOutputTokens ?? null,
    max_cost_per_million_tokens: run.maxCostPerMillionTokens ?? null,
    max_latency_ms: run.maxLatencyMs ?? null,
    min_quality: run.minQuality ?? null,
    min_selection_confidence: run.minSelectionConfidence ?? null,
    approve_effects: run.approveEffects ?? false,
    tool_read_only: run.toolReadOnly ?? true,
    max_provider_failovers: run.maxProviderFailovers ?? null,
    max_total_cost_units: run.maxTotalCostUnits ?? null,
    require_json: run.requireJson ?? false,
    response_schema_digest: run.responseSchema === undefined ? null : await digestJson(run.responseSchema),
    structured_domain_response: run.structuredDomainResponse ?? false,
    temperature: run.temperature ?? null,
    authorize_and_execute: run.authorizeAndExecute !== undefined,
    evidence_checkpointed: options.evidenceCheckpointStore !== undefined,
    evidence_job_id: options.evidenceJobId ?? null,
  });
}

function providerResultWasObserved(status: AutonomousRunResult["status"]): boolean {
  return !["approval_required", "route_review_required", "abstained"].includes(status);
}

function checkpointStatusForResult(result: AutonomousEvidenceBackedRunResult): AutonomousEvidenceBackedCheckpointStatus {
  if (result.status === "evidence_review_required") return "evidence_review_required";
  if (result.status === "evidence_blocked") return "evidence_blocked";
  if (result.evidence && result.evidence.status !== "completed") return "evidence_incomplete";
  if (result.run?.status === "completed") return "completed";
  if (result.run && providerResultWasObserved(result.run.status)) return "provider_reconciliation_required";
  return "provider_pending";
}

async function checkpointForResult(input: {
  jobId: string;
  requestDigest: string;
  runPolicyDigest: string;
  result: AutonomousEvidenceBackedRunResult;
  status?: AutonomousEvidenceBackedCheckpointStatus;
}): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  const status = input.status ?? checkpointStatusForResult(input.result);
  const providerResultDigest = input.result.run && providerResultWasObserved(input.result.run.status) ? await digestJson(input.result.run) : null;
  const payload = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    task_digest: input.result.task_digest,
    request_digest: input.requestDigest,
    run_policy_digest: input.runPolicyDigest,
    evidence_plan_digest: input.result.execution_plan.evidence_plan_digest,
    execution_plan_digest: input.result.execution_plan.plan_digest,
    evidence_result_digest: input.result.evidence?.result_digest ?? null,
    prompt_projection_digest: input.result.prompt_context.length ? await digestJson(input.result.prompt_context) : null,
    provider_result_digest: providerResultDigest,
    provider_status: input.result.run?.status ?? null,
    status,
  };
  const encoded = JSON.stringify(payload);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ProviderRuntimeError("evidence-backed checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: RETENTION, secret_material: SECRET_MATERIAL };
}

async function checkpointForPreflight(input: {
  jobId: string;
  taskDigest: string;
  requestDigest: string;
  runPolicyDigest: string;
  preflight: AutonomousEvidenceBackedRunPreflight;
}): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  const payload = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    task_digest: input.taskDigest,
    request_digest: input.requestDigest,
    run_policy_digest: input.runPolicyDigest,
    evidence_plan_digest: input.preflight.executionPlan.evidence_plan_digest,
    execution_plan_digest: input.preflight.executionPlan.plan_digest,
    evidence_result_digest: input.preflight.evidence.result_digest,
    prompt_projection_digest: input.preflight.promptContext.length ? await digestJson(input.preflight.promptContext) : null,
    provider_result_digest: null,
    provider_status: null,
    status: "provider_pending" as const,
  };
  const encoded = JSON.stringify(payload);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ProviderRuntimeError("evidence-backed preflight checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: RETENTION, secret_material: SECRET_MATERIAL };
}

/** Validate checkpoint structure, retention, and the content digest before any dispatch. */
export async function validateAutonomousEvidenceBackedCheckpoint(value: unknown): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA) throw new ArgumentError("evidence-backed checkpoint schema is invalid");
  allowedKeys(value, ["schema", "job_id", "task_digest", "request_digest", "run_policy_digest", "evidence_plan_digest", "execution_plan_digest", "evidence_result_digest", "prompt_projection_digest", "provider_result_digest", "provider_status", "status", "checkpoint_digest", "retention", "secret_material"], "evidence-backed checkpoint");
  const jobId = boundedIdentifier("evidence-backed checkpoint job_id", value.job_id);
  const taskDigest = digest("evidence-backed checkpoint task_digest", value.task_digest);
  const requestDigestValue = digest("evidence-backed checkpoint request_digest", value.request_digest);
  const runPolicyDigest = digest("evidence-backed checkpoint run_policy_digest", value.run_policy_digest);
  const evidencePlanDigest = digest("evidence-backed checkpoint evidence_plan_digest", value.evidence_plan_digest);
  const executionPlanDigest = digest("evidence-backed checkpoint execution_plan_digest", value.execution_plan_digest);
  const evidenceResultDigest = optionalDigest("evidence-backed checkpoint evidence_result_digest", value.evidence_result_digest);
  const promptProjectionDigest = optionalDigest("evidence-backed checkpoint prompt_projection_digest", value.prompt_projection_digest);
  const providerResultDigest = optionalDigest("evidence-backed checkpoint provider_result_digest", value.provider_result_digest);
  const providerStatus = value.provider_status === null ? null : value.provider_status as AutonomousRunResult["status"];
  if (providerStatus !== null && !["completed", "route_review_required", "approval_required", "reconciliation_required", "turn_limit_reached", "abstained", "cross_domain_partial", "child_failed"].includes(providerStatus)) throw new ArgumentError("evidence-backed checkpoint provider_status is invalid");
  const status = value.status as AutonomousEvidenceBackedCheckpointStatus;
  if (!["evidence_review_required", "evidence_blocked", "evidence_incomplete", "provider_pending", "provider_reconciliation_required", "completed"].includes(status)) throw new ArgumentError("evidence-backed checkpoint status is invalid");
  if (status === "completed" && (providerResultDigest === null || providerStatus !== "completed")) throw new ArgumentError("completed evidence-backed checkpoint requires a completed provider digest");
  if (status === "provider_reconciliation_required" && (providerResultDigest === null || providerStatus === null)) throw new ArgumentError("provider reconciliation checkpoint requires a provider result digest");
  if (["evidence_review_required", "evidence_blocked", "evidence_incomplete"].includes(status) && (providerResultDigest !== null || providerStatus !== null)) throw new ArgumentError("evidence-only checkpoint cannot contain provider result metadata");
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evidence-backed checkpoint retention contract is invalid");
  const payload = { schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA, job_id: jobId, task_digest: taskDigest, request_digest: requestDigestValue, run_policy_digest: runPolicyDigest, evidence_plan_digest: evidencePlanDigest, execution_plan_digest: executionPlanDigest, evidence_result_digest: evidenceResultDigest, prompt_projection_digest: promptProjectionDigest, provider_result_digest: providerResultDigest, provider_status: providerStatus, status };
  if (await digestJson(payload) !== value.checkpoint_digest) throw new ArgumentError("evidence-backed checkpoint digest is invalid");
  const encoded = JSON.stringify(value);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint exceeds its bounded size");
  return structuredClone({ ...payload, checkpoint_digest: value.checkpoint_digest as string, retention: RETENTION, secret_material: SECRET_MATERIAL });
}

function assertCheckpointBinding(
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  jobId: string,
  taskDigest: string,
  requestDigestValue: string,
  runPolicyDigestValue: string,
): void {
  if (checkpoint.job_id !== jobId || checkpoint.task_digest !== taskDigest || checkpoint.request_digest !== requestDigestValue || checkpoint.run_policy_digest !== runPolicyDigestValue) throw new ArgumentError("evidence-backed checkpoint does not match the current task, requests, run policy, or job");
}

async function makeResumableResult(input: {
  jobId: string;
  status: AutonomousEvidenceBackedResumableStatus;
  result: AutonomousEvidenceBackedRunResult;
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  providerRehydrated: boolean;
}): Promise<AutonomousEvidenceBackedResumableRun> {
  const projection = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
    status: input.status,
    job_id: input.jobId,
    checkpoint_digest: input.checkpoint.checkpoint_digest,
    result_status: input.result.status,
    provider_rehydrated: input.providerRehydrated,
    retention: RESULT_RETENTION,
    secret_material: SECRET_MATERIAL,
  } satisfies AutonomousEvidenceBackedResumableRunProjection;
  return {
    schema: AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
    status: input.status,
    job_id: input.jobId,
    result: input.result,
    checkpoint: structuredClone(input.checkpoint),
    provider_rehydrated: input.providerRehydrated,
    toJSON: () => structuredClone(projection),
  };
}

async function persist(
  sink: (checkpoint: AutonomousEvidenceBackedCheckpointJSON) => Promise<void> | void,
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  await sink(checkpoint);
  return checkpoint;
}

/**
 * Execute or resume one evidence-backed run. Evidence journals replay completed source work;
 * provider results are never replayed implicitly. A provider_pending checkpoint requires either
 * a caller-rehydrated result or the explicit resumeProvider=true decision.
 */
export async function runAutonomousEvidenceBackedResumable(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousEvidenceBackedResumableExecutionOptions,
): Promise<AutonomousEvidenceBackedResumableRun> {
  if (!agent || typeof agent.runWithReviewedEvidence !== "function") throw new ArgumentError("evidence-backed resumable execution requires an AutonomousAgent");
  if (!options || typeof options !== "object") throw new ArgumentError("evidence-backed resumable options are malformed");
  const jobId = boundedIdentifier("evidence-backed resumable jobId", options.jobId);
  if (typeof options.checkpointSink !== "function") throw new ArgumentError("evidence-backed resumable execution requires checkpointSink");
  const taskDigest = await digestJson({ task });
  const requestDigestValue = await requestDigest(options.requests);
  const runPolicyDigestValue = await runPolicyDigest(options);
  const restored = options.checkpoint === undefined ? null : await validateAutonomousEvidenceBackedCheckpoint(options.checkpoint);
  if (restored !== null) {
    assertCheckpointBinding(restored, jobId, taskDigest, requestDigestValue, runPolicyDigestValue);
    if (!options.execute?.journal) throw new ArgumentError("evidence-backed resume requires the caller-owned evidence journal");
  }

  const { jobId: _jobId, checkpoint: _checkpoint, checkpointSink: _checkpointSink, rehydrateProviderRun, resumeProvider, ...baseOptions } = options;
  const providerRun = baseOptions.run?.approveProviderCall === true;
  const probe = async (): Promise<AutonomousEvidenceBackedRunResult> => agent.runWithReviewedEvidence(task, {
    ...baseOptions,
    run: { ...(baseOptions.run ?? {}), approveProviderCall: false },
  });

  const finishRestoredProvider = async (
    probeResult: AutonomousEvidenceBackedRunResult,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  ): Promise<AutonomousEvidenceBackedResumableRun> => {
    if (!probeResult.evidence || probeResult.evidence.status !== "completed") {
      const next = await checkpointForResult({ jobId, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, result: probeResult });
      await persist(options.checkpointSink, next);
      return makeResumableResult({ jobId, status: next.status === "evidence_incomplete" ? "evidence_incomplete" : probeResult.status, result: probeResult, checkpoint: next, providerRehydrated: false });
    }
    if (rehydrateProviderRun) {
      const recovered = await rehydrateProviderRun({ checkpoint, executionPlan: probeResult.execution_plan, evidence: probeResult.evidence, promptContext: probeResult.prompt_context });
      if (recovered !== null) {
        if (!isObject(recovered) || recovered.schema !== "bioprism-typescript-autonomous-run/0.1") throw new ArgumentError("rehydrated provider run is malformed");
        if (checkpoint.provider_result_digest === null || await digestJson(recovered) !== checkpoint.provider_result_digest) throw new ProviderRuntimeError("rehydrated provider run does not match its checkpoint digest");
        const finalResult = await agent.runWithReviewedEvidence(task, {
          ...baseOptions,
          run: { ...(baseOptions.run ?? {}), approveProviderCall: true },
          providerRunOverride: recovered,
        });
        const next = await checkpointForResult({ jobId, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, result: finalResult, status: "completed" });
        await persist(options.checkpointSink, next);
        return makeResumableResult({ jobId, status: "completed", result: finalResult, checkpoint: next, providerRehydrated: true });
      }
    }
    const next = await checkpointForResult({ jobId, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, result: probeResult, status: "provider_reconciliation_required" });
    await persist(options.checkpointSink, next);
    return makeResumableResult({ jobId, status: "provider_reconciliation_required", result: probeResult, checkpoint: next, providerRehydrated: false });
  };

  if (restored && ["completed", "provider_reconciliation_required"].includes(restored.status)) {
    return finishRestoredProvider(await probe(), restored);
  }

  if (restored && restored.status === "provider_pending" && !resumeProvider) {
    const probeResult = await probe();
    if (rehydrateProviderRun && restored.provider_result_digest !== null) return finishRestoredProvider(probeResult, restored);
    const next = await checkpointForResult({ jobId, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, result: probeResult, status: "provider_pending" });
    await persist(options.checkpointSink, next);
    return makeResumableResult({ jobId, status: "provider_pending", result: probeResult, checkpoint: next, providerRehydrated: false });
  }

  const beforeProviderRun = async (preflight: AutonomousEvidenceBackedRunPreflight): Promise<void> => {
    const next = await checkpointForPreflight({ jobId, taskDigest, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, preflight });
    await persist(options.checkpointSink, next);
  };
  const result = await agent.runWithReviewedEvidence(task, {
    ...baseOptions,
    beforeProviderRun,
  });
  const finalCheckpoint = await checkpointForResult({ jobId, requestDigest: requestDigestValue, runPolicyDigest: runPolicyDigestValue, result });
  await persist(options.checkpointSink, finalCheckpoint);
  const status: AutonomousEvidenceBackedResumableStatus = finalCheckpoint.status === "provider_pending" ? "provider_pending" : finalCheckpoint.status === "provider_reconciliation_required" ? "provider_reconciliation_required" : result.status;
  return makeResumableResult({ jobId, status, result, checkpoint: finalCheckpoint, providerRehydrated: false });
}

export async function runAutonomousEvidenceBackedResumableWithCheckpoint(
  agent: AutonomousAgent,
  task: string,
  options: Omit<AutonomousEvidenceBackedResumableExecutionOptions, "checkpoint"> & { checkpoint: AutonomousEvidenceBackedCheckpointJSON },
): Promise<AutonomousEvidenceBackedResumableRun> {
  return runAutonomousEvidenceBackedResumable(agent, task, options);
}

/** In-memory checkpoint adapter for local workers and tests. */
export class InMemoryAutonomousEvidenceBackedCheckpointStore implements AutonomousEvidenceBackedCheckpointStore {
  private checkpoint: AutonomousEvidenceBackedCheckpointJSON | null;

  constructor(initial?: AutonomousEvidenceBackedCheckpointJSON | null) {
    this.checkpoint = initial === undefined || initial === null ? null : structuredClone(initial);
  }

  async read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> {
    return this.checkpoint === null ? null : validateAutonomousEvidenceBackedCheckpoint(this.checkpoint);
  }

  async write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> {
    this.checkpoint = structuredClone(await validateAutonomousEvidenceBackedCheckpoint(checkpoint));
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> {
    if ((this.checkpoint?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
    await this.write(checkpoint);
    return true;
  }
}

/** Browser/Node text adapter with strict JSON and byte bounds. */
export class JsonAutonomousEvidenceBackedCheckpointStore implements AutonomousEvidenceBackedCheckpointStore {
  constructor(protected readonly store: AutonomousEvidenceBackedCheckpointTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evidence-backed JSON checkpoint store is malformed");
  }

  async read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("evidence-backed checkpoint text is invalid JSON");
    }
    return validateAutonomousEvidenceBackedCheckpoint(parsed);
  }

  async write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> {
    const validated = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
    const encoded = JSON.stringify(validated);
    if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint text exceeds its bound");
    await this.store.write(encoded);
  }
}

/** Text adapter that exposes compare-and-swap rather than pretending ordinary writes are atomic. */
export class TransactionalJsonAutonomousEvidenceBackedCheckpointStore extends JsonAutonomousEvidenceBackedCheckpointStore {
  constructor(private readonly transactionalStore: AutonomousEvidenceBackedTransactionalCheckpointTextStore) {
    super(transactionalStore);
    if (typeof transactionalStore.writeIfUnchanged !== "function") throw new ArgumentError("transactional evidence-backed checkpoint store requires writeIfUnchanged");
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> {
    const validated = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
    const encoded = JSON.stringify(validated);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedCheckpointDigest, encoded);
    if (typeof committed !== "boolean") throw new ArgumentError("transactional evidence-backed checkpoint store returned a non-boolean result");
    return committed;
  }
}

/** Restart-aware controller with serialized local operations and optional CAS fencing. */
export class AutonomousEvidenceBackedController {
  private checkpoint: AutonomousEvidenceBackedCheckpointJSON | null = null;
  private expectedCheckpointDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();
  private controllerStatus: AutonomousEvidenceBackedControllerProjection["status"] = "empty";

  constructor(readonly agent: AutonomousAgent, readonly jobId: string, readonly persistence: AutonomousEvidenceBackedCheckpointStore) {
    if (!agent || typeof agent.runWithReviewedEvidence !== "function") throw new ArgumentError("evidence-backed controller requires an AutonomousAgent");
    boundedIdentifier("evidence-backed controller jobId", jobId);
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("evidence-backed controller persistence is malformed");
  }

  async restore(): Promise<AutonomousEvidenceBackedControllerProjection> {
    return this.enqueue(async () => {
      const stored = await this.persistence.read();
      this.checkpoint = stored === null ? null : await validateAutonomousEvidenceBackedCheckpoint(stored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      this.controllerStatus = this.checkpoint === null ? "empty" : "restored";
      return this.projection();
    });
  }

  async run(task: string, options: AutonomousEvidenceBackedControllerRunOptions): Promise<AutonomousEvidenceBackedControllerRun> {
    return this.enqueue(async () => {
      const stored = this.checkpoint ?? await this.persistence.read();
      this.checkpoint = stored === null ? null : await validateAutonomousEvidenceBackedCheckpoint(stored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      const result = await runAutonomousEvidenceBackedResumable(this.agent, task, {
        ...options,
        jobId: this.jobId,
        ...(this.checkpoint === null ? {} : { checkpoint: this.checkpoint }),
        checkpointSink: async (checkpoint) => {
          if (typeof this.persistence.writeIfUnchanged === "function") {
            const committed = await this.persistence.writeIfUnchanged(this.expectedCheckpointDigest, checkpoint);
            if (!committed) throw new ArgumentError("evidence-backed checkpoint compare-and-swap conflict; reload before continuing");
          } else {
            await this.persistence.write(checkpoint);
          }
          this.checkpoint = checkpoint;
          this.expectedCheckpointDigest = checkpoint.checkpoint_digest;
          this.controllerStatus = checkpoint.status === "completed" ? "completed" : checkpoint.status === "provider_pending" ? "provider_pending" : checkpoint.status === "provider_reconciliation_required" ? "provider_reconciliation_required" : checkpoint.status === "evidence_incomplete" ? "evidence_incomplete" : "flushed";
        },
      });
      this.checkpoint = result.checkpoint;
      this.expectedCheckpointDigest = result.checkpoint.checkpoint_digest;
      this.controllerStatus = result.status === "completed" ? "completed" : result.status === "provider_pending" ? "provider_pending" : result.status === "provider_reconciliation_required" ? "provider_reconciliation_required" : result.status === "evidence_incomplete" ? "evidence_incomplete" : "flushed";
      return { controller: this.projection(), run: result };
    });
  }

  projection(): AutonomousEvidenceBackedControllerProjection {
    return {
      schema: "bioprism-typescript-autonomous-evidence-backed-controller/0.1",
      status: this.controllerStatus,
      job_id: this.jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      persisted: true,
      retention: CONTROLLER_RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
