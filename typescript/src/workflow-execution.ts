import { ArgumentError, CredentialError, ProviderRuntimeError } from "./errors.js";
import type { ProviderErrorCode } from "./errors.js";
import type { ApiClient } from "./client.js";
import { AUTONOMOUS_DOMAIN_NAMES, validateAutonomousRouteOverride } from "./autonomous.js";
import type { AutonomousLearningController } from "./autonomous-learning.js";
import type {
  AutonomousAgent,
  AutonomousDomainName,
  AutonomousRunOptions,
  AutonomousRunResult,
  AutonomousRouteProposal,
  AutonomousTaskBlueprint,
  AutonomousWorkflow,
  AutonomousWorkflowToolContext,
  AutonomousWorkflowStage,
} from "./autonomous.js";
import { semanticRouteAutonomousTask } from "./autonomous-routing.js";
import type { AutonomousSemanticRouteOptions, AutonomousSemanticRouteResult } from "./autonomous-routing.js";
import { AutonomousCostBudget } from "./llm.js";
import { digestJson } from "./tooling.js";
import type {
  BrainJobApprovalResult,
  BrainJobEventsResult,
  BrainJobRecord,
  BrainJobStatusResult,
  JsonObject,
  JsonValue,
  RestToolResponse,
  AutonomousPlanRefinementResult,
} from "./types.js";

export const AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-workflow-execution/0.1" as const;
export const AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-workflow-checkpoint/0.1" as const;
export const AUTONOMOUS_WORKFLOW_EVENT_SCHEMA = "bioprism-typescript-autonomous-workflow-event/0.1" as const;
export const AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-workflow-snapshot/0.1" as const;
export const AUTONOMOUS_DURABLE_JOB_SCHEMA = "bioprism-typescript-autonomous-durable-job/0.1" as const;
export const AUTONOMOUS_WORKFLOW_EXECUTION_CONTRACT_SCHEMA = "bioprism-typescript-autonomous-workflow-execution-contract/0.1" as const;
export const AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL = 32;
export const AUTONOMOUS_WORKFLOW_MAX_EVENTS = 256;
export const AUTONOMOUS_WORKFLOW_MAX_JOBS = 1_024;
export const AUTONOMOUS_WORKFLOW_MAX_SNAPSHOT_BYTES = 4 * 1024 * 1024;

export type AutonomousWorkflowCheckpointStatus = "running" | "paused" | "completed" | "failed";
export type AutonomousWorkflowExecutionStatus = "completed" | "paused" | "approval_required" | "failed" | "stage_blocked" | "stage_proposed" | "stage_not_attempted" | "route_review_required";
export type AutonomousWorkflowSemanticRouteStatus = AutonomousSemanticRouteResult["status"];
export type AutonomousWorkflowEventType = "started" | "stage_completed" | "checkpointed" | "approval_required" | "stage_failed" | "completed";
export const AUTONOMOUS_WORKFLOW_STAGE_STATUSES = ["completed", "proposed", "blocked", "not_attempted"] as const;
export type AutonomousWorkflowStageStatus = typeof AUTONOMOUS_WORKFLOW_STAGE_STATUSES[number];
export const AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE = 32;
export const AUTONOMOUS_WORKFLOW_MAX_STAGE_TEXT_BYTES = 16_000;

export interface AutonomousWorkflowStageOutcome {
  stage_id: string;
  status: "completed" | "approval_required" | "failed";
  run_status: string;
  selection_digest: string | null;
  response_digest: string | null;
  output_bytes: number;
  error_class: string | null;
  error_code?: ProviderErrorCode | null;
  retryable?: boolean | null;
  status_code?: number | null;
  learning_episode_id: string | null;
}

/** Metadata-only restart checkpoint. Task text, prompts, credentials, and provider responses are never persisted here. */
export interface AutonomousWorkflowCheckpoint {
  schema: typeof AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA;
  job_id: string;
  task_digest: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  plan_digest: string;
  /** Digest of the route actually approved for this workflow, when one was supplied. */
  route_digest?: string | null;
  completed_stage_ids: string[];
  next_stage_id: string | null;
  stage_outcomes: AutonomousWorkflowStageOutcome[];
  generation: number;
  status: AutonomousWorkflowCheckpointStatus;
  /** Digest of the explicitly accepted provider planning proposal, if one shaped scheduling. */
  plan_refinement_digest?: string | null;
  /** Digest of the caller-owned model-selection/output contract; null only for legacy unbound checkpoints. */
  execution_contract_digest?: string | null;
  previous_checkpoint_digest: string | null;
  checkpoint_digest: string;
  retention: "metadata_only;task_prompt_response_and_credentials_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowEvent {
  schema: typeof AUTONOMOUS_WORKFLOW_EVENT_SCHEMA;
  sequence: number;
  job_id: string;
  event_type: AutonomousWorkflowEventType;
  stage_id: string | null;
  checkpoint_digest: string;
  previous_event_digest: string | null;
  event_digest: string;
  retention: "metadata_only;provider_payloads_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowCheckpointStore {
  load(jobId: string): Promise<AutonomousWorkflowCheckpoint | null> | AutonomousWorkflowCheckpoint | null;
  save(checkpoint: AutonomousWorkflowCheckpoint): Promise<void> | void;
  appendEvent(event: AutonomousWorkflowEvent): Promise<void> | void;
  events(jobId: string, after?: number, limit?: number): Promise<AutonomousWorkflowEvent[]> | AutonomousWorkflowEvent[];
}

export interface AutonomousWorkflowCheckpointStoreSnapshot {
  schema: typeof AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA;
  checkpoints: AutonomousWorkflowCheckpoint[];
  event_rows: Array<{ job_id: string; events: AutonomousWorkflowEvent[] }>;
  retention: "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousWorkflowSnapshotStore extends AutonomousWorkflowCheckpointStore {
  snapshot(): Promise<AutonomousWorkflowCheckpointStoreSnapshot>;
  restore(snapshot: AutonomousWorkflowCheckpointStoreSnapshot): Promise<void>;
  verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA; verified: true; jobs: number; events: number; snapshot_digest: string; retention: "metadata_only" }>;
}

export interface AutonomousWorkflowSnapshotPersistence {
  read(): Promise<AutonomousWorkflowCheckpointStoreSnapshot | null> | AutonomousWorkflowCheckpointStoreSnapshot | null;
  write(snapshot: AutonomousWorkflowCheckpointStoreSnapshot): Promise<void> | void;
}

export interface AutonomousWorkflowStageResult {
  stage: AutonomousWorkflowStage;
  run: AutonomousRunResult | null;
  output_digest: string | null;
  output_bytes: number;
  learning_episode_id: string | null;
  declared_status: AutonomousWorkflowStageStatus | null;
  evidence: string[];
  uncertainty: string[];
  notes: string | null;
  next_actions: string[];
  validation_errors: string[];
}

export interface AutonomousWorkflowExecutionResult {
  schema: typeof AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA;
  status: AutonomousWorkflowExecutionStatus;
  job_id: string | null;
  blueprint: AutonomousTaskBlueprint | null;
  checkpoint: AutonomousWorkflowCheckpoint | null;
  route: AutonomousRouteProposal | null;
  semantic_route_status: AutonomousWorkflowSemanticRouteStatus | null;
  events: AutonomousWorkflowEvent[];
  stage_results: AutonomousWorkflowStageResult[];
  completed_stage_count: number;
  total_stage_count: number;
  plan_refinement_digest: string | null;
  learning_episode_ids: string[];
  recovery: "caller_rehydrates_task_and_credentials";
  retention: "provider_responses_local;checkpoint_metadata_only";
}

export interface AutonomousWorkflowExecuteOptions extends AutonomousRunOptions {
  jobId?: string;
  maxStages?: number;
  /** Explicitly permit re-dispatch of a stage whose provider declared it blocked/proposed/not_attempted. */
  retryBlocked?: boolean;
  /** Caller-owned raw JSON responses for completed stages; each is digest-checked before reuse. */
  stageOutputs?: Readonly<Record<string, string>>;
  /** A completed, non-review provider proposal that may reorder only existing workflow stages. */
  acceptedPlanRefinement?: AutonomousPlanRefinementResult;
  /** Explicitly bind a legacy checkpoint that predates execution-contract digests before continuing it. */
  rebindLegacyExecutionContract?: boolean;
  /** Optional provider-assisted routing policy. Routing remains review-only and is separately approved. */
  semanticRouting?: AutonomousWorkflowSemanticRoutingOptions;
}

export interface AutonomousWorkflowSemanticRoutingOptions extends Pick<AutonomousSemanticRouteOptions, "approveProviderCall" | "minSemanticConfidence" | "maxDomains" | "allowCrossDomain" | "maxOutputTokens" | "maxProviderFailovers"> {
  enabled?: boolean;
}

export interface AutonomousWorkflowExecutorOptions {
  learning?: AutonomousLearningController;
}

export interface AutonomousDurableJobSubmitOptions extends AutonomousWorkflowExecuteOptions {
  idempotencyKey: string;
  priority?: number;
  maxAttempts?: number;
  checkpointDigest?: string | null;
}

export interface AutonomousDurableJobSubmission {
  schema: typeof AUTONOMOUS_DURABLE_JOB_SCHEMA;
  status: "submitted" | "route_review_required";
  route: AutonomousRouteProposal;
  blueprint: AutonomousTaskBlueprint | null;
  job: BrainJobRecord | null;
  spec_digest: string | null;
  execution: "not_started";
  private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane";
}

export interface AutonomousDurableJobExecutionResult {
  schema: typeof AUTONOMOUS_DURABLE_JOB_SCHEMA;
  job: BrainJobRecord;
  local: AutonomousWorkflowExecutionResult;
  server_job_posture: "control_plane_projection;completion_requires_external_worker_reconciliation";
  private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane";
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, allowed: readonly string[], label: string): void {
  const allowedKeys = new Set(allowed);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError(`${label} contains unsupported fields`);
}

function workflowDigest(value: unknown, label: string, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${label} must be a lowercase SHA-256 digest`);
  return value;
}

function workflowLabel(value: unknown, label: string, maximum = 512): string {
  if (typeof value !== "string" || !value || value.length > maximum || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${label} must be a bounded identifier`);
  return value;
}

function workflowBoundedInteger(value: unknown, label: string, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${label} is outside its bounded integer contract`);
  return value as number;
}

async function validateWorkflowCheckpoint(value: unknown): Promise<AutonomousWorkflowCheckpoint> {
  if (!isObject(value)) throw new ArgumentError("workflow checkpoint must be an object");
  exactKeys(value, ["schema", "job_id", "task_digest", "domain", "workflow_id", "workflow_digest", "plan_digest", "route_digest", "completed_stage_ids", "next_stage_id", "stage_outcomes", "generation", "status", "plan_refinement_digest", "execution_contract_digest", "previous_checkpoint_digest", "checkpoint_digest", "retention", "secret_material"], "workflow checkpoint");
  if (value.schema !== AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA || value.retention !== "metadata_only;task_prompt_response_and_credentials_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("workflow checkpoint metadata markers are invalid");
  const jobId = boundedJobId(value.job_id);
  const taskDigest = workflowDigest(value.task_digest, "workflow task_digest")!;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain as AutonomousDomainName)) throw new ArgumentError("workflow checkpoint domain is not supported");
  const workflowId = workflowLabel(value.workflow_id, "workflow workflow_id");
  const workflowDigestValue = workflowDigest(value.workflow_digest, "workflow workflow_digest")!;
  const planDigest = workflowDigest(value.plan_digest, "workflow plan_digest")!;
  const hasRouteDigest = Object.prototype.hasOwnProperty.call(value, "route_digest");
  const routeDigest = hasRouteDigest ? workflowDigest(value.route_digest, "workflow route_digest", true) : null;
  if (!Array.isArray(value.completed_stage_ids) || value.completed_stage_ids.length > AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL) throw new ArgumentError("workflow completed_stage_ids exceed their bound");
  const completedStageIds = value.completed_stage_ids.map((stageId) => workflowLabel(stageId, "workflow completed stage id", 256));
  if (new Set(completedStageIds).size !== completedStageIds.length) throw new ArgumentError("workflow completed_stage_ids must be unique");
  const nextStageId = value.next_stage_id === null ? null : workflowLabel(value.next_stage_id, "workflow next_stage_id", 256);
  if (!Array.isArray(value.stage_outcomes) || value.stage_outcomes.length > AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL) throw new ArgumentError("workflow stage_outcomes exceed their bound");
  const stageOutcomes: AutonomousWorkflowStageOutcome[] = value.stage_outcomes.map((candidate) => {
    if (!isObject(candidate)) throw new ArgumentError("workflow stage outcome must be an object");
    exactKeys(candidate, ["stage_id", "status", "run_status", "selection_digest", "response_digest", "output_bytes", "error_class", "error_code", "retryable", "status_code", "learning_episode_id"], "workflow stage outcome");
    const status = candidate.status;
    if (status !== "completed" && status !== "approval_required" && status !== "failed") throw new ArgumentError("workflow stage outcome status is invalid");
    const errorCode = candidate.error_code === undefined || candidate.error_code === null ? null : workflowLabel(candidate.error_code, "workflow error_code", 128) as ProviderErrorCode;
    if (candidate.retryable !== undefined && candidate.retryable !== null && typeof candidate.retryable !== "boolean") throw new ArgumentError("workflow retryable must be boolean or null");
    if (candidate.status_code !== undefined && candidate.status_code !== null) workflowBoundedInteger(candidate.status_code, "workflow status_code", 999, 100);
    const hasErrorCode = Object.prototype.hasOwnProperty.call(candidate, "error_code");
    return {
      stage_id: workflowLabel(candidate.stage_id, "workflow stage_id", 256),
      status,
      run_status: workflowLabel(candidate.run_status, "workflow run_status", 128),
      selection_digest: workflowDigest(candidate.selection_digest, "workflow selection_digest", true),
      response_digest: workflowDigest(candidate.response_digest, "workflow response_digest", true),
      output_bytes: workflowBoundedInteger(candidate.output_bytes, "workflow output_bytes", 64 * 1024 * 1024),
      error_class: candidate.error_class === undefined || candidate.error_class === null ? null : workflowLabel(candidate.error_class, "workflow error_class", 128),
      ...(hasErrorCode ? { error_code: errorCode } : {}),
      ...(candidate.retryable === undefined ? {} : { retryable: candidate.retryable as boolean | null }),
      ...(candidate.status_code === undefined ? {} : { status_code: candidate.status_code as number | null }),
      learning_episode_id: candidate.learning_episode_id === null ? null : workflowLabel(candidate.learning_episode_id, "workflow learning_episode_id", 512),
    };
  });
  const generation = workflowBoundedInteger(value.generation, "workflow generation", Number.MAX_SAFE_INTEGER, 1);
  const status = value.status;
  if (status !== "running" && status !== "paused" && status !== "completed" && status !== "failed") throw new ArgumentError("workflow checkpoint status is invalid");
  const hasExecutionContractDigest = Object.prototype.hasOwnProperty.call(value, "execution_contract_digest");
  const executionContractDigest = hasExecutionContractDigest
    ? workflowDigest(value.execution_contract_digest, "workflow execution_contract_digest", true)
    : null;
  const hasPlanRefinementDigest = Object.prototype.hasOwnProperty.call(value, "plan_refinement_digest");
  const planRefinementDigest = hasPlanRefinementDigest
    ? workflowDigest(value.plan_refinement_digest, "workflow plan_refinement_digest", true)
    : null;
  const previousCheckpointDigest = workflowDigest(value.previous_checkpoint_digest, "workflow previous_checkpoint_digest", true);
  const checkpointDigest = workflowDigest(value.checkpoint_digest, "workflow checkpoint_digest")!;
  const descriptor = {
    schema: value.schema,
    job_id: jobId,
    task_digest: taskDigest,
    domain: value.domain,
    workflow_id: workflowId,
    workflow_digest: workflowDigestValue,
    plan_digest: planDigest,
    ...(hasRouteDigest ? { route_digest: routeDigest } : {}),
    completed_stage_ids: completedStageIds,
    next_stage_id: nextStageId,
    stage_outcomes: stageOutcomes,
    generation,
    status,
    ...(hasPlanRefinementDigest ? { plan_refinement_digest: planRefinementDigest } : {}),
    ...(hasExecutionContractDigest ? { execution_contract_digest: executionContractDigest } : {}),
    previous_checkpoint_digest: previousCheckpointDigest,
    retention: value.retention,
    secret_material: value.secret_material,
  };
  if (await digestJson(descriptor) !== checkpointDigest) throw new ArgumentError("workflow checkpoint digest does not match its metadata");
  return (hasExecutionContractDigest
    ? { ...descriptor, execution_contract_digest: executionContractDigest, checkpoint_digest: checkpointDigest }
    : { ...descriptor, checkpoint_digest: checkpointDigest }) as AutonomousWorkflowCheckpoint;
}

async function validateWorkflowEvent(value: unknown): Promise<AutonomousWorkflowEvent> {
  if (!isObject(value)) throw new ArgumentError("workflow event must be an object");
  exactKeys(value, ["schema", "sequence", "job_id", "event_type", "stage_id", "checkpoint_digest", "previous_event_digest", "event_digest", "retention", "secret_material"], "workflow event");
  if (value.schema !== AUTONOMOUS_WORKFLOW_EVENT_SCHEMA || value.retention !== "metadata_only;provider_payloads_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("workflow event metadata markers are invalid");
  const sequence = workflowBoundedInteger(value.sequence, "workflow event sequence", Number.MAX_SAFE_INTEGER, 1);
  const jobId = boundedJobId(value.job_id);
  const eventType = value.event_type;
  if (eventType !== "started" && eventType !== "stage_completed" && eventType !== "checkpointed" && eventType !== "approval_required" && eventType !== "stage_failed" && eventType !== "completed") throw new ArgumentError("workflow event type is invalid");
  const stageId = value.stage_id === null ? null : workflowLabel(value.stage_id, "workflow event stage_id", 256);
  const checkpointDigest = workflowDigest(value.checkpoint_digest, "workflow event checkpoint_digest")!;
  const previousEventDigest = workflowDigest(value.previous_event_digest, "workflow event previous_event_digest", true);
  const eventDigest = workflowDigest(value.event_digest, "workflow event event_digest")!;
  const descriptor = { schema: value.schema, sequence, job_id: jobId, event_type: eventType, stage_id: stageId, checkpoint_digest: checkpointDigest, previous_event_digest: previousEventDigest, retention: value.retention, secret_material: value.secret_material };
  if (await digestJson(descriptor) !== eventDigest) throw new ArgumentError("workflow event digest does not match its metadata");
  return { ...descriptor, event_digest: eventDigest } as AutonomousWorkflowEvent;
}

async function validateWorkflowSnapshot(value: unknown): Promise<{ snapshot: AutonomousWorkflowCheckpointStoreSnapshot; eventCount: number }> {
  if (!isObject(value)) throw new ArgumentError("workflow snapshot must be an object");
  exactKeys(value, ["schema", "checkpoints", "event_rows", "retention", "secret_material", "snapshot_digest"], "workflow snapshot");
  if (value.schema !== AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA || value.retention !== "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("workflow snapshot metadata markers are invalid");
  if (!Array.isArray(value.checkpoints) || !Array.isArray(value.event_rows) || value.checkpoints.length > AUTONOMOUS_WORKFLOW_MAX_JOBS || value.event_rows.length > AUTONOMOUS_WORKFLOW_MAX_JOBS) throw new ArgumentError("workflow snapshot job capacity is exhausted");
  const snapshotDigest = workflowDigest(value.snapshot_digest, "workflow snapshot_digest")!;
  const { snapshot_digest: observed, ...descriptor } = value;
  if (await digestJson(descriptor) !== observed) throw new ArgumentError("workflow snapshot digest does not match");
  const checkpoints: AutonomousWorkflowCheckpoint[] = [];
  const checkpointIds = new Set<string>();
  for (const candidate of value.checkpoints) {
    const checkpoint = await validateWorkflowCheckpoint(candidate);
    if (checkpointIds.has(checkpoint.job_id)) throw new ArgumentError("workflow snapshot contains duplicate checkpoints");
    checkpointIds.add(checkpoint.job_id);
    checkpoints.push(checkpoint);
  }
  const eventRows: Array<{ job_id: string; events: AutonomousWorkflowEvent[] }> = [];
  const eventJobIds = new Set<string>();
  let eventCount = 0;
  for (const candidate of value.event_rows) {
    if (!isObject(candidate)) throw new ArgumentError("workflow snapshot event row must be an object");
    exactKeys(candidate, ["job_id", "events"], "workflow snapshot event row");
    const jobId = boundedJobId(candidate.job_id);
    if (eventJobIds.has(jobId)) throw new ArgumentError("workflow snapshot contains duplicate event rows");
    if (!checkpointIds.has(jobId)) throw new ArgumentError("workflow snapshot event row has no checkpoint");
    if (!Array.isArray(candidate.events) || candidate.events.length > AUTONOMOUS_WORKFLOW_MAX_EVENTS) throw new ArgumentError("workflow snapshot event capacity is exhausted");
    const events: AutonomousWorkflowEvent[] = [];
    let prior: AutonomousWorkflowEvent | null = null;
    for (const eventCandidate of candidate.events) {
      const event = await validateWorkflowEvent(eventCandidate);
      if (event.job_id !== jobId) throw new ArgumentError("workflow event job_id does not match its row");
      if (prior === null) {
        if (event.sequence === 1 && event.previous_event_digest !== null) throw new ArgumentError("workflow first event must not have a predecessor");
        if (event.sequence > 1 && event.previous_event_digest === null) throw new ArgumentError("workflow truncated event history must retain its predecessor digest");
      } else if (event.sequence !== prior.sequence + 1 || event.previous_event_digest !== prior.event_digest) {
        throw new ArgumentError("workflow event hash chain is not contiguous");
      }
      events.push(event);
      prior = event;
      eventCount += 1;
    }
    eventJobIds.add(jobId);
    eventRows.push({ job_id: jobId, events });
  }
  const bytes = new TextEncoder().encode(JSON.stringify(value)).byteLength;
  if (bytes > AUTONOMOUS_WORKFLOW_MAX_SNAPSHOT_BYTES) throw new ArgumentError("workflow snapshot exceeds its byte capacity");
  return { snapshot: { ...descriptor, snapshot_digest: snapshotDigest, checkpoints, event_rows: eventRows } as AutonomousWorkflowCheckpointStoreSnapshot, eventCount };
}

/** A bounded process-local store useful for tests and small workers; production callers can replace it with SQLite/Redis/etc. */
export class InMemoryAutonomousWorkflowCheckpointStore implements AutonomousWorkflowSnapshotStore {
  private readonly checkpoints = new Map<string, AutonomousWorkflowCheckpoint>();
  private readonly eventRows = new Map<string, AutonomousWorkflowEvent[]>();

  load(jobId: string): AutonomousWorkflowCheckpoint | null {
    const checkpoint = this.checkpoints.get(boundedJobId(jobId));
    return checkpoint ? structuredClone(checkpoint) : null;
  }

  async save(checkpoint: AutonomousWorkflowCheckpoint): Promise<void> {
    const normalized = await validateWorkflowCheckpoint(checkpoint);
    const previous = this.checkpoints.get(normalized.job_id);
    if (!previous) {
      if (normalized.generation !== 1 || normalized.previous_checkpoint_digest !== null) throw new ArgumentError("workflow initial checkpoint must start at generation one");
    } else if (previous.checkpoint_digest !== normalized.checkpoint_digest && (normalized.generation !== previous.generation + 1 || normalized.previous_checkpoint_digest !== previous.checkpoint_digest)) {
      throw new ArgumentError("workflow checkpoint generation is not contiguous");
    }
    if (this.checkpoints.size >= AUTONOMOUS_WORKFLOW_MAX_JOBS && !previous) throw new ArgumentError("workflow job capacity is exhausted");
    this.checkpoints.set(normalized.job_id, structuredClone(normalized));
  }

  async appendEvent(event: AutonomousWorkflowEvent): Promise<void> {
    const normalized = await validateWorkflowEvent(event);
    if (!this.checkpoints.has(normalized.job_id)) throw new ArgumentError("workflow event requires an existing checkpoint");
    const rows = this.eventRows.get(normalized.job_id) ?? [];
    const prior = rows.at(-1);
    if (prior && normalized.sequence === prior.sequence && normalized.event_digest === prior.event_digest) return;
    if (prior && (normalized.sequence !== prior.sequence + 1 || normalized.previous_event_digest !== prior.event_digest)) throw new ArgumentError("workflow event sequence or predecessor digest is invalid");
    if (!prior && normalized.sequence !== 1) throw new ArgumentError("workflow event sequence must start at one");
    rows.push(structuredClone(normalized));
    if (rows.length > AUTONOMOUS_WORKFLOW_MAX_EVENTS) rows.splice(0, rows.length - AUTONOMOUS_WORKFLOW_MAX_EVENTS);
    this.eventRows.set(normalized.job_id, rows);
  }

  events(jobId: string, after = 0, limit = AUTONOMOUS_WORKFLOW_MAX_EVENTS): AutonomousWorkflowEvent[] {
    const normalizedJobId = boundedJobId(jobId);
    if (!Number.isSafeInteger(after) || after < 0) throw new ArgumentError("workflow event after must be a non-negative integer");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_WORKFLOW_MAX_EVENTS) throw new ArgumentError("workflow event limit is outside its bounds");
    return (this.eventRows.get(normalizedJobId) ?? []).filter((event) => event.sequence > after).slice(0, limit).map((event) => structuredClone(event));
  }

  async snapshot(): Promise<AutonomousWorkflowCheckpointStoreSnapshot> {
    const descriptor = {
      schema: AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA,
      checkpoints: [...this.checkpoints.values()].sort((left, right) => left.job_id.localeCompare(right.job_id)).map((checkpoint) => structuredClone(checkpoint)),
      event_rows: [...this.eventRows.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([job_id, events]) => ({ job_id, events: events.map((event) => structuredClone(event)) })),
      retention: "metadata_only;task_prompt_response_credentials_and_provider_payloads_not_retained" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...descriptor, snapshot_digest: await digestJson(descriptor) };
    return (await validateWorkflowSnapshot(snapshot)).snapshot;
  }

  async restore(snapshot: AutonomousWorkflowCheckpointStoreSnapshot): Promise<void> {
    const validated = (await validateWorkflowSnapshot(snapshot)).snapshot;
    this.checkpoints.clear();
    this.eventRows.clear();
    for (const checkpoint of validated.checkpoints) this.checkpoints.set(checkpoint.job_id, structuredClone(checkpoint));
    for (const row of validated.event_rows) this.eventRows.set(row.job_id, row.events.map((event) => structuredClone(event)));
  }

  async verifyIntegrity(): Promise<{ schema: typeof AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA; verified: true; jobs: number; events: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await this.snapshot();
    return { schema: AUTONOMOUS_WORKFLOW_SNAPSHOT_SCHEMA, verified: true, jobs: snapshot.checkpoints.length, events: snapshot.event_rows.reduce((total, row) => total + row.events.length, 0), snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }
}

/** Coordinates workflow checkpoint snapshots with a caller-owned durable adapter. */
export class AutonomousWorkflowPersistenceCoordinator {
  constructor(readonly store: AutonomousWorkflowSnapshotStore, readonly persistence: AutonomousWorkflowSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("workflow persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("workflow persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousWorkflowCheckpointStoreSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousWorkflowCheckpointStoreSnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

function boundedJobId(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError("workflow jobId must be a bounded identifier");
  return value;
}

function boundedStageCount(value: unknown): number {
  if (value === undefined) return AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL;
  if (!Number.isSafeInteger(value) || (value as number) < 1 || (value as number) > AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL) throw new ArgumentError("workflow maxStages must be between 1 and 32");
  return value as number;
}

function boundedRetryBlocked(value: unknown): boolean {
  if (value === undefined) return false;
  if (typeof value !== "boolean") throw new ArgumentError("workflow retryBlocked must be boolean");
  return value;
}

function responseText(run: AutonomousRunResult | null): string {
  if (!run?.response) return "";
  if (run.response.text) return run.response.text;
  return run.response.structured === null || run.response.structured === undefined ? "" : JSON.stringify(run.response.structured);
}

function workflowStageResponseSchema(stage: AutonomousWorkflowStage): JsonObject {
  return {
    type: "object",
    additionalProperties: false,
    properties: {
      stage_id: { type: "string", enum: [stage.id] },
      status: { type: "string", enum: [...AUTONOMOUS_WORKFLOW_STAGE_STATUSES] },
      evidence: { type: "array", maxItems: AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE, items: { type: "string", maxLength: 4_096 } },
      uncertainty: { type: "array", maxItems: AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE, items: { type: "string", maxLength: 4_096 } },
      notes: { type: "string", maxLength: AUTONOMOUS_WORKFLOW_MAX_STAGE_TEXT_BYTES },
      next_actions: { type: "array", maxItems: AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE, items: { type: "string", maxLength: 4_096 } },
    },
    required: ["stage_id", "status", "evidence", "uncertainty", "notes", "next_actions"],
  };
}

interface ValidatedWorkflowStageOutput {
  declaredStatus: AutonomousWorkflowStageStatus | null;
  evidence: string[];
  uncertainty: string[];
  notes: string | null;
  nextActions: string[];
  errors: string[];
}

function workflowStageStringArray(value: unknown, field: string, errors: string[]): string[] {
  if (!Array.isArray(value)) {
    errors.push(`${field} must be an array`);
    return [];
  }
  if (value.length > AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE) errors.push(`${field} exceeds ${AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE} items`);
  const values: string[] = [];
  for (const [index, candidate] of value.slice(0, AUTONOMOUS_WORKFLOW_MAX_STAGE_EVIDENCE).entries()) {
    if (typeof candidate !== "string" || new TextEncoder().encode(candidate).byteLength > 4_096 || candidate.includes("\u0000")) {
      errors.push(`${field}[${index}] is outside its bounded text contract`);
    } else {
      values.push(candidate);
    }
  }
  return values;
}

function workflowTaskText(task: unknown): string {
  if (typeof task !== "string" || !task.trim() || task.includes("\u0000") || new TextEncoder().encode(task).byteLength > 32_000) throw new ArgumentError("workflow task is outside its bounded text contract");
  return task;
}

function validateWorkflowStageOutput(stage: AutonomousWorkflowStage, value: unknown): ValidatedWorkflowStageOutput {
  const empty: ValidatedWorkflowStageOutput = { declaredStatus: null, evidence: [], uncertainty: [], notes: null, nextActions: [], errors: [] };
  if (!isObject(value)) return { ...empty, errors: ["provider returned no structured workflow stage object"] };
  const errors: string[] = [];
  const allowed = new Set(["stage_id", "status", "evidence", "uncertainty", "notes", "next_actions"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) errors.push("structured workflow stage output contains unsupported fields");
  if (value.stage_id !== stage.id) errors.push(`stage_id must equal ${stage.id}`);
  const declaredStatus = AUTONOMOUS_WORKFLOW_STAGE_STATUSES.includes(value.status as AutonomousWorkflowStageStatus)
    ? value.status as AutonomousWorkflowStageStatus
    : null;
  if (declaredStatus === null) errors.push("status is not a supported workflow stage status");
  const evidence = workflowStageStringArray(value.evidence, "evidence", errors);
  const uncertainty = workflowStageStringArray(value.uncertainty, "uncertainty", errors);
  const nextActions = workflowStageStringArray(value.next_actions, "next_actions", errors);
  const notes = typeof value.notes === "string" && !value.notes.includes("\u0000") && new TextEncoder().encode(value.notes).byteLength <= AUTONOMOUS_WORKFLOW_MAX_STAGE_TEXT_BYTES
    ? value.notes
    : null;
  if (notes === null) errors.push("notes is outside its bounded text contract");
  return { declaredStatus, evidence, uncertainty, notes, nextActions, errors };
}

function blockedWorkflowExecutionStatus(errorClass: string | null): Extract<AutonomousWorkflowExecutionStatus, "stage_blocked" | "stage_proposed" | "stage_not_attempted"> | null {
  if (errorClass === "stage_blocked") return "stage_blocked";
  if (errorClass === "stage_proposed") return "stage_proposed";
  if (errorClass === "stage_not_attempted") return "stage_not_attempted";
  return null;
}

function blockedWorkflowErrorClass(status: AutonomousWorkflowStageStatus | null): string | null {
  if (status === "blocked") return "stage_blocked";
  if (status === "proposed") return "stage_proposed";
  if (status === "not_attempted") return "stage_not_attempted";
  return null;
}

function safeErrorClass(error: unknown): string {
  const candidate = error instanceof Error && typeof error.constructor?.name === "string" ? error.constructor.name : "UnknownError";
  return /^[A-Za-z0-9_.-]{1,128}$/.test(candidate) ? candidate : "UnknownError";
}

function stageFailure(error: unknown): { error_class: string; error_code: ProviderErrorCode | null; retryable: boolean | null; status_code: number | null } {
  if (error instanceof ProviderRuntimeError) return { error_class: error.name, error_code: error.code, retryable: error.retryable, status_code: error.statusCode ?? null };
  if (error instanceof CredentialError) return { error_class: error.name, error_code: "credential", retryable: false, status_code: null };
  return { error_class: safeErrorClass(error), error_code: null, retryable: null, status_code: null };
}

function workflowCandidateContract(candidate: NonNullable<AutonomousRunOptions["candidates"]>[number]): Record<string, unknown> {
  return {
    provider: candidate.provider,
    model: candidate.model,
    capabilities: [...(candidate.capabilities ?? [])],
    context_window_tokens: candidate.context_window_tokens,
    max_output_tokens: candidate.max_output_tokens,
    quality: candidate.quality,
    latency_ms: candidate.latency_ms,
    cost_per_million_tokens: candidate.cost_per_million_tokens,
    reliability: candidate.reliability,
    requires_credential: candidate.requires_credential ?? null,
    enabled: candidate.enabled ?? null,
  };
}

async function workflowExecutionContractDigest(agent: AutonomousAgent, options: AutonomousWorkflowExecuteOptions): Promise<string> {
  const candidates = options.candidates ? [...options.candidates] : agent.models();
  const toolsDigest = options.tools === undefined ? null : await digestJson(options.tools.map((tool) => ({ name: tool.name, description: tool.description, parameters: tool.parameters })));
  const executionPolicyDigest = options.execution ? await options.execution.policy.digest() : null;
  return digestJson({
    schema: AUTONOMOUS_WORKFLOW_EXECUTION_CONTRACT_SCHEMA,
    candidates_digest: await digestJson(candidates.map((candidate) => workflowCandidateContract(candidate))),
    max_input_tokens: options.maxInputTokens ?? null,
    max_output_tokens: options.maxOutputTokens ?? null,
    max_cost_per_million_tokens: options.maxCostPerMillionTokens ?? null,
    max_latency_ms: options.maxLatencyMs ?? null,
    min_quality: options.minQuality ?? null,
    aggregate_cost_limit: options.costBudget?.maxCostUnits ?? options.maxTotalCostUnits ?? null,
    // A workflow owns its stage contract. Caller schemas cannot weaken or replace it,
    // so custom responseSchema/requireJson values do not create replay identities.
    require_json: true,
    response_schema_digest: "builtin-workflow-stage-contract-v1",
    temperature: options.temperature ?? null,
    tools_digest: toolsDigest,
    approve_effects: options.approveEffects === true,
    max_provider_failovers: options.maxProviderFailovers ?? null,
    execution_policy_digest: executionPolicyDigest,
  });
}

function normalizeWorkflowCostOptions<T extends AutonomousWorkflowExecuteOptions>(options: T): T {
  if (options.costBudget !== undefined && !(options.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("costBudget must be an AutonomousCostBudget");
  if (options.costBudget !== undefined && options.maxTotalCostUnits !== undefined) throw new ArgumentError("costBudget and maxTotalCostUnits cannot both be supplied");
  if (options.costBudget !== undefined || options.maxTotalCostUnits === undefined) return options;
  return { ...options, maxTotalCostUnits: undefined, costBudget: new AutonomousCostBudget(options.maxTotalCostUnits) } as T;
}

interface AcceptedWorkflowPlan {
  priority_stage_ids: string[];
  focus_stage_ids: string[];
  refinement_digest: string;
}

interface WorkflowRouteResolution {
  route: AutonomousRouteProposal;
  semantic_status: AutonomousWorkflowSemanticRouteStatus | null;
}

/** Validate an accepted provider proposal before it can affect scheduling or checkpoint state. */
async function acceptedWorkflowPlan(
  blueprint: AutonomousTaskBlueprint,
  refinement: AutonomousPlanRefinementResult | undefined,
): Promise<AcceptedWorkflowPlan | null> {
  if (refinement === undefined) return null;
  if (!isObject(refinement) || refinement.status !== "completed" || refinement.review_required !== false) {
    throw new ProviderRuntimeError("only a completed, non-review plan refinement may be accepted");
  }
  const basePlanDigest = await digestJson(blueprint.plan);
  if (refinement.task_digest !== blueprint.task_digest) throw new ProviderRuntimeError("accepted plan refinement task does not match the workflow blueprint");
  if (refinement.base_plan_digest !== basePlanDigest) throw new ProviderRuntimeError("accepted plan refinement base plan does not match the workflow blueprint");
  if (refinement.workflow_digest !== blueprint.workflow.workflow_digest) throw new ProviderRuntimeError("accepted plan refinement workflow does not match the workflow blueprint");
  if (!Array.isArray(refinement.priority_stage_ids) || !Array.isArray(refinement.focus_stage_ids)) throw new ProviderRuntimeError("accepted plan refinement stage identifiers are malformed");
  const stageIds = blueprint.workflow.stages.map((stage) => stage.id);
  const priority = refinement.priority_stage_ids.filter((stageId): stageId is string => typeof stageId === "string");
  const focus = refinement.focus_stage_ids.filter((stageId): stageId is string => typeof stageId === "string");
  if (priority.length !== refinement.priority_stage_ids.length || focus.length !== refinement.focus_stage_ids.length || priority.length !== stageIds.length || new Set(priority).size !== priority.length || new Set(focus).size !== focus.length || priority.some((stageId) => !stageIds.includes(stageId)) || focus.some((stageId) => !stageIds.includes(stageId))) {
    throw new ProviderRuntimeError("accepted plan refinement must contain an exact stage permutation and valid focus subset");
  }
  const positions = new Map(priority.map((stageId, index) => [stageId, index]));
  for (const stage of blueprint.workflow.stages) {
    if (stage.depends_on.some((dependency) => (positions.get(dependency) ?? -1) > (positions.get(stage.id) ?? -1))) throw new ProviderRuntimeError("accepted plan refinement violates workflow dependencies");
  }
  return { priority_stage_ids: [...priority], focus_stage_ids: [...focus], refinement_digest: await digestJson(refinement) };
}

function runOptions(options: AutonomousWorkflowExecuteOptions, stage: AutonomousWorkflowStage, workflow: AutonomousWorkflow, context: AutonomousRunOptions["context"]): AutonomousRunOptions {
  const workflowContext: AutonomousWorkflowToolContext = {
    domain: workflow.domain,
    workflow_id: workflow.workflow_id,
    workflow_digest: workflow.workflow_digest,
    stage_id: stage.id,
  };
  return {
    domain: workflow.domain,
    workflowContext,
    capability: stage.required_capabilities[0],
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context,
    hints: [],
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
    maxCostPerMillionTokens: options.maxCostPerMillionTokens,
    maxLatencyMs: options.maxLatencyMs,
    minQuality: options.minQuality,
    maxTotalCostUnits: options.costBudget ? undefined : options.maxTotalCostUnits,
    costBudget: options.costBudget,
    requireJson: true,
    responseSchema: workflowStageResponseSchema(stage),
    temperature: options.temperature,
    tools: options.tools,
    authorizeAndExecute: options.authorizeAndExecute,
    toolReadOnly: options.toolReadOnly,
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
    execution: options.execution,
    executionAttempt: options.executionAttempt,
    maxProviderFailovers: options.maxProviderFailovers,
    // The workflow owns the enclosing lifecycle; each stage contributes observations and
    // accounting without completing the shared controller independently.
    executionLifecycle: "observe_only",
    signal: options.signal,
    observer: options.observer,
  };
}

export class AutonomousWorkflowExecutor {
  readonly agent: AutonomousAgent;
  readonly store: AutonomousWorkflowCheckpointStore;
  readonly learning?: AutonomousLearningController;

  constructor(agent: AutonomousAgent, store: AutonomousWorkflowCheckpointStore, options: AutonomousWorkflowExecutorOptions = {}) {
    if (!agent || typeof agent.blueprint !== "function" || typeof agent.run !== "function") throw new ArgumentError("workflow executor requires an AutonomousAgent");
    if (!store || typeof store.load !== "function" || typeof store.save !== "function" || typeof store.appendEvent !== "function" || typeof store.events !== "function") throw new ArgumentError("workflow executor requires a checkpoint store");
    this.agent = agent;
    this.store = store;
    this.learning = options.learning;
  }

  async start(task: string, rawOptions: AutonomousWorkflowExecuteOptions = {}): Promise<AutonomousWorkflowExecutionResult> {
    const options = normalizeWorkflowCostOptions(rawOptions);
    const taskText = workflowTaskText(task);
    const taskDigest = await digestJson({ task: taskText });
    const jobId = boundedJobId(options.jobId ?? `workflow-${taskDigest.slice(0, 24)}`);
    const existing = await this.store.load(jobId);
    const routeResolution = existing
      ? await this.resolveExistingRoute(taskText, existing, options)
      : await this.resolveStartRoute(taskText, options);
    const route = routeResolution.route;
    if (routeResolution.semantic_status !== null && routeResolution.semantic_status !== "completed") return this.routeReviewResult(route, routeResolution.semantic_status);
    if (route.abstained || !route.primary_domain || route.cross_domain) return this.routeReviewResult(route, routeResolution.semantic_status);
    const blueprintEnvelope = await this.agent.blueprint(taskText, { domain: route.primary_domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name) });
    const blueprint = blueprintEnvelope.blueprint;
    if (!blueprint) return this.routeReviewResult(route, routeResolution.semantic_status);
    const acceptedPlan = await acceptedWorkflowPlan(blueprint, options.acceptedPlanRefinement);
    const contractDigest = await workflowExecutionContractDigest(this.agent, options);
    if (existing) {
      if (existing.task_digest !== blueprint.task_digest || existing.workflow_digest !== blueprint.workflow.workflow_digest) throw new ArgumentError("workflow job already exists with a different task or workflow");
      const bound = await this.bindExecutionContract(existing, blueprint, options, contractDigest, acceptedPlan);
      const planBound = await this.bindPlanRefinement(bound, blueprint, acceptedPlan, contractDigest);
      return this.drive(taskText, blueprint, planBound, options, contractDigest, acceptedPlan, route, routeResolution.semantic_status);
    }
    const initial = await this.makeCheckpoint(jobId, blueprint, [], [], "running", contractDigest, null, acceptedPlan?.priority_stage_ids, acceptedPlan?.refinement_digest ?? null, route.route_digest);
    await this.store.save(initial);
    await this.appendEvent(jobId, "started", null, initial);
    return this.drive(taskText, blueprint, initial, options, contractDigest, acceptedPlan, route, routeResolution.semantic_status);
  }

  async resume(jobId: string, task: string, rawOptions: Omit<AutonomousWorkflowExecuteOptions, "jobId"> = {}): Promise<AutonomousWorkflowExecutionResult> {
    const options = normalizeWorkflowCostOptions(rawOptions);
    const normalizedJobId = boundedJobId(jobId);
    const checkpoint = await this.store.load(normalizedJobId);
    if (!checkpoint) throw new ArgumentError(`workflow job ${normalizedJobId} was not found; caller must rehydrate from its durable store`);
    const taskText = workflowTaskText(task);
    const route = (await this.resolveExistingRoute(taskText, checkpoint, options)).route;
    const blueprintEnvelope = await this.agent.blueprint(taskText, { domain: checkpoint.domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name) });
    const blueprint = blueprintEnvelope.blueprint;
    if (!blueprint || blueprint.workflow.workflow_digest !== checkpoint.workflow_digest || blueprint.plan.plan_digest !== checkpoint.plan_digest) throw new ProviderRuntimeError("workflow rehydration blueprint digest does not match the checkpoint");
    const acceptedPlan = await acceptedWorkflowPlan(blueprint, options.acceptedPlanRefinement);
    const contractOptions = { ...options, jobId: normalizedJobId };
    const contractDigest = await workflowExecutionContractDigest(this.agent, contractOptions);
    const bound = await this.bindExecutionContract(checkpoint, blueprint, contractOptions, contractDigest, acceptedPlan);
    const planBound = await this.bindPlanRefinement(bound, blueprint, acceptedPlan, contractDigest);
    return this.drive(taskText, blueprint, planBound, contractOptions, contractDigest, acceptedPlan, route, null);
  }

  async events(jobId: string, after = 0, limit = AUTONOMOUS_WORKFLOW_MAX_EVENTS): Promise<AutonomousWorkflowEvent[]> {
    return this.store.events(boundedJobId(jobId), after, limit);
  }

  private async resolveStartRoute(task: string, options: AutonomousWorkflowExecuteOptions): Promise<WorkflowRouteResolution> {
    if (options.routeOverride) return { route: await validateAutonomousRouteOverride(task, options.routeOverride), semantic_status: null };
    if (!options.semanticRouting?.enabled) return { route: await this.agent.route(task, { domain: options.domain, hints: options.hints }), semantic_status: null };
    if (options.domain !== undefined) throw new ArgumentError("semantic workflow routing cannot replace an explicit caller domain");
    const semantic = await semanticRouteAutonomousTask(this.agent, task, {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains,
      allowCrossDomain: options.semanticRouting.allowCrossDomain ?? false,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      maxTotalCostUnits: options.costBudget ? undefined : options.maxTotalCostUnits,
      costBudget: options.costBudget,
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers ?? options.maxProviderFailovers,
      executionLifecycle: options.executionLifecycle,
      signal: options.signal,
      observer: options.observer,
    });
    return { route: semantic.route, semantic_status: semantic.status };
  }

  private async resolveExistingRoute(task: string, checkpoint: AutonomousWorkflowCheckpoint, options: AutonomousWorkflowExecuteOptions): Promise<WorkflowRouteResolution> {
    if (options.semanticRouting?.enabled && !options.routeOverride) throw new ArgumentError("existing workflow checkpoints require routeOverride to change provider-assisted routing; semantic routing is never replayed implicitly");
    const route = options.routeOverride
      ? await validateAutonomousRouteOverride(task, options.routeOverride)
      : await this.agent.route(task, { domain: checkpoint.domain, hints: options.hints });
    if (route.abstained || !route.primary_domain || route.primary_domain !== checkpoint.domain || route.cross_domain) throw new ProviderRuntimeError("workflow rehydration route does not match the checkpoint domain");
    if (route.task_digest !== checkpoint.task_digest) throw new ProviderRuntimeError("workflow rehydration task digest does not match the checkpoint");
    if (options.routeOverride && checkpoint.route_digest !== undefined && checkpoint.route_digest !== null && route.route_digest !== checkpoint.route_digest) throw new ProviderRuntimeError("workflow route override does not match the persisted route digest");
    return { route, semantic_status: null };
  }

  private routeReviewResult(route: AutonomousRouteProposal | null = null, semanticStatus: AutonomousWorkflowSemanticRouteStatus | null = null): AutonomousWorkflowExecutionResult {
    return { schema: AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA, status: "route_review_required", job_id: null, blueprint: null, checkpoint: null, route, semantic_route_status: semanticStatus, events: [], stage_results: [], completed_stage_count: 0, total_stage_count: 0, plan_refinement_digest: null, learning_episode_ids: [], recovery: "caller_rehydrates_task_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only" };
  }

  private async makeCheckpoint(jobId: string, blueprint: AutonomousTaskBlueprint, completed: string[], outcomes: AutonomousWorkflowStageOutcome[], status: AutonomousWorkflowCheckpointStatus, executionContractDigest: string, previous: AutonomousWorkflowCheckpoint | null, stageOrder: readonly string[] = blueprint.workflow.stages.map((stage) => stage.id), planRefinementDigest: string | null = previous?.plan_refinement_digest ?? null, routeDigest: string | null = previous?.route_digest ?? null): Promise<AutonomousWorkflowCheckpoint> {
    const next = stageOrder.find((stageId) => !completed.includes(stageId)) ?? null;
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA, job_id: jobId, task_digest: blueprint.task_digest, domain: blueprint.domain_profile.domain, workflow_id: blueprint.workflow.workflow_id, workflow_digest: blueprint.workflow.workflow_digest, plan_digest: blueprint.plan.plan_digest, route_digest: routeDigest, completed_stage_ids: completed, next_stage_id: next, stage_outcomes: outcomes, generation: (previous?.generation ?? 0) + 1, status, plan_refinement_digest: planRefinementDigest, execution_contract_digest: executionContractDigest, previous_checkpoint_digest: previous?.checkpoint_digest ?? null, retention: "metadata_only;task_prompt_response_and_credentials_not_retained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
  }

  private async bindExecutionContract(checkpoint: AutonomousWorkflowCheckpoint, blueprint: AutonomousTaskBlueprint, options: AutonomousWorkflowExecuteOptions, contractDigest: string, acceptedPlan: AcceptedWorkflowPlan | null): Promise<AutonomousWorkflowCheckpoint> {
    if (checkpoint.execution_contract_digest === contractDigest) return checkpoint;
    if (checkpoint.execution_contract_digest !== undefined && checkpoint.execution_contract_digest !== null) throw new ProviderRuntimeError("workflow execution contract does not match the checkpoint");
    if (options.rebindLegacyExecutionContract !== true) throw new ProviderRuntimeError("workflow checkpoint predates execution-contract binding; set rebindLegacyExecutionContract: true for an explicit migration");
    const migrated = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, checkpoint.status, contractDigest, checkpoint, acceptedPlan?.priority_stage_ids, acceptedPlan?.refinement_digest ?? checkpoint.plan_refinement_digest ?? null);
    await this.store.save(migrated);
    await this.appendEvent(migrated.job_id, "checkpointed", migrated.next_stage_id, migrated);
    return migrated;
  }

  private async bindPlanRefinement(checkpoint: AutonomousWorkflowCheckpoint, blueprint: AutonomousTaskBlueprint, acceptedPlan: AcceptedWorkflowPlan | null, contractDigest: string): Promise<AutonomousWorkflowCheckpoint> {
    const existingDigest = checkpoint.plan_refinement_digest ?? null;
    const acceptedDigest = acceptedPlan?.refinement_digest ?? null;
    if (existingDigest === acceptedDigest) return checkpoint;
    if (existingDigest !== null) throw new ProviderRuntimeError("workflow plan refinement does not match the checkpoint");
    if (!acceptedPlan) throw new ProviderRuntimeError("workflow checkpoint requires its accepted plan refinement for resume");
    if (checkpoint.completed_stage_ids.length > 0) throw new ProviderRuntimeError("workflow cannot bind a new plan refinement after stage execution has started");
    const migrated = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, checkpoint.status, checkpoint.execution_contract_digest ?? contractDigest, checkpoint, acceptedPlan.priority_stage_ids, acceptedPlan.refinement_digest);
    await this.store.save(migrated);
    await this.appendEvent(migrated.job_id, "checkpointed", migrated.next_stage_id, migrated);
    return migrated;
  }

  private async appendEvent(jobId: string, eventType: AutonomousWorkflowEventType, stageId: string | null, checkpoint: AutonomousWorkflowCheckpoint): Promise<AutonomousWorkflowEvent> {
    const prior = await this.store.events(jobId, 0, AUTONOMOUS_WORKFLOW_MAX_EVENTS);
    const previousEventDigest = prior.at(-1)?.event_digest ?? null;
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_EVENT_SCHEMA, sequence: (prior.at(-1)?.sequence ?? 0) + 1, job_id: jobId, event_type: eventType, stage_id: stageId, checkpoint_digest: checkpoint.checkpoint_digest, previous_event_digest: previousEventDigest, retention: "metadata_only;provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
    const event = { ...descriptor, event_digest: await digestJson(descriptor) };
    await this.store.appendEvent(event);
    return event;
  }

  private async priorOutputs(
    checkpoint: AutonomousWorkflowCheckpoint,
    stageResults: readonly AutonomousWorkflowStageResult[],
    stages: readonly AutonomousWorkflowStage[],
    options: AutonomousWorkflowExecuteOptions,
  ): Promise<Array<Record<string, unknown>>> {
    const outputs = new Map<string, Record<string, unknown>>();
    for (const entry of stageResults) {
      outputs.set(entry.stage.id, {
        stage_id: entry.stage.id,
        output_digest: entry.output_digest,
        output_bytes: entry.output_bytes,
        structured_output: {
          stage_id: entry.stage.id,
          status: entry.declared_status,
          evidence: entry.evidence,
          uncertainty: entry.uncertainty,
          notes: entry.notes,
          next_actions: entry.next_actions,
        },
      });
    }
    const stageById = new Map(stages.map((stage) => [stage.id, stage]));
    const completedOutcomes = new Map<string, AutonomousWorkflowStageOutcome>();
    for (const outcome of checkpoint.stage_outcomes) {
      if (outcome.status === "completed") completedOutcomes.set(outcome.stage_id, outcome);
    }
    const supplied = options.stageOutputs ?? {};
    if (!isObject(supplied) || Object.keys(supplied).length > AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL) throw new ArgumentError("workflow stageOutputs exceed their bound");
    for (const [stageId, raw] of Object.entries(supplied)) {
      const stage = stageById.get(stageId);
      if (!stage || !checkpoint.completed_stage_ids.includes(stageId)) throw new ArgumentError(`workflow stageOutputs contains a non-completed stage ${stageId}`);
      if (typeof raw !== "string" || !raw || raw.includes("\u0000") || new TextEncoder().encode(raw).byteLength > 256 * 1024) throw new ArgumentError(`workflow stageOutputs.${stageId} is outside its bounded text contract`);
      const outcome = completedOutcomes.get(stageId);
      if (!outcome?.response_digest || await digestJson({ stage_id: stageId, output: raw }) !== outcome.response_digest) throw new ProviderRuntimeError(`rehydrated workflow stage output digest does not match checkpoint for ${stageId}`);
      let structured: unknown;
      try { structured = JSON.parse(raw); } catch { throw new ProviderRuntimeError(`rehydrated workflow stage output for ${stageId} is not valid JSON`); }
      const validation = validateWorkflowStageOutput(stage, structured);
      if (validation.errors.length > 0 || validation.declaredStatus !== "completed") throw new ProviderRuntimeError(`rehydrated workflow stage output for ${stageId} fails its stage contract`);
      outputs.set(stageId, {
        stage_id: stageId,
        output_digest: outcome.response_digest,
        output_bytes: new TextEncoder().encode(raw).byteLength,
        structured_output: {
          stage_id: stageId,
          status: validation.declaredStatus,
          evidence: validation.evidence,
          uncertainty: validation.uncertainty,
          notes: validation.notes,
          next_actions: validation.nextActions,
        },
      });
    }
    for (const stageId of checkpoint.completed_stage_ids) {
      if (outputs.has(stageId)) continue;
      const outcome = completedOutcomes.get(stageId);
      outputs.set(stageId, { stage_id: stageId, output_digest: outcome?.response_digest ?? null, output_bytes: outcome?.output_bytes ?? 0, structured_output: null });
    }
    return [...outputs.values()];
  }

  private async drive(task: string, blueprint: AutonomousTaskBlueprint, initial: AutonomousWorkflowCheckpoint, options: AutonomousWorkflowExecuteOptions, contractDigest: string, acceptedPlan: AcceptedWorkflowPlan | null, route: AutonomousRouteProposal, semanticStatus: AutonomousWorkflowSemanticRouteStatus | null): Promise<AutonomousWorkflowExecutionResult> {
    const maxStages = boundedStageCount(options.maxStages);
    const retryBlocked = boundedRetryBlocked(options.retryBlocked);
    let checkpoint = initial;
    const stageResults: AutonomousWorkflowStageResult[] = [];
    const stages = blueprint.workflow.stages;
    const stageOrder = acceptedPlan?.priority_stage_ids ?? stages.map((stage) => stage.id);
    const planRefinementDigest = acceptedPlan?.refinement_digest ?? checkpoint.plan_refinement_digest ?? null;
    let consumed = 0;
    if (checkpoint.status === "completed") return this.result("completed", checkpoint, blueprint, stageResults, route, semanticStatus);
    const previousBlockedStatus = blockedWorkflowExecutionStatus(checkpoint.stage_outcomes.at(-1)?.error_class ?? null);
    if (previousBlockedStatus && !retryBlocked) return this.result(previousBlockedStatus, checkpoint, blueprint, stageResults, route, semanticStatus);
    if (previousBlockedStatus && retryBlocked) {
      // Preserve the prior blocked checkpoint/event chain, but remove its active outcome from
      // the new attempt so a successful retry can complete without duplicating one stage row.
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes.slice(0, -1), "running", contractDigest, checkpoint, stageOrder, planRefinementDigest);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "checkpointed", checkpoint.next_stage_id, checkpoint);
    }
    if (options.approveProviderCall !== true) {
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, "paused", contractDigest, checkpoint, stageOrder, planRefinementDigest);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "approval_required", checkpoint.next_stage_id, checkpoint);
      return this.result("approval_required", checkpoint, blueprint, stageResults, route, semanticStatus);
    }
    while (checkpoint.next_stage_id && consumed < maxStages) {
      const stage = stages.find((candidate) => candidate.id === checkpoint.next_stage_id);
      if (!stage) throw new ProviderRuntimeError(`workflow checkpoint references unknown stage ${checkpoint.next_stage_id}`);
      if (stage.depends_on.some((dependency) => !checkpoint.completed_stage_ids.includes(dependency))) throw new ProviderRuntimeError(`workflow stage ${stage.id} has incomplete dependencies`);
      consumed += 1;
      const priorOutputs = await this.priorOutputs(checkpoint, stageResults, stages, options);
      const context = [
        ...(options.context ?? []),
        { id: "workflow-checkpoint", content: JSON.stringify({ job_id: checkpoint.job_id, workflow_digest: checkpoint.workflow_digest, completed_stage_ids: checkpoint.completed_stage_ids, stage_outcomes: checkpoint.stage_outcomes, prior_outputs: priorOutputs }), required: true, priority: 100 },
        { id: "workflow-stage-contract", content: JSON.stringify({ stage_id: stage.id, objective: stage.objective, required_capabilities: stage.required_capabilities, evidence_outputs: stage.evidence_outputs, evaluator_signals: stage.evaluator_signals }), required: true, priority: 90 },
        ...(acceptedPlan ? [{ id: "workflow-plan-refinement", content: JSON.stringify({ refinement_digest: acceptedPlan.refinement_digest, priority_rank: stageOrder.indexOf(stage.id), focus: acceptedPlan.focus_stage_ids.includes(stage.id) }), required: true, priority: 95 }] : []),
      ];
      let run: AutonomousRunResult;
      try {
        run = await this.agent.run(`Execute workflow stage ${stage.id} for task: ${task}`, runOptions(options, stage, blueprint.workflow, context));
      } catch (error) {
        const failure = stageFailure(error);
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "failed", run_status: "exception", selection_digest: null, response_digest: null, output_bytes: 0, error_class: failure.error_class, error_code: failure.error_code, retryable: failure.retryable, status_code: failure.status_code, learning_episode_id: null }], "failed", contractDigest, checkpoint, stageOrder, planRefinementDigest);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "stage_failed", stage.id, checkpoint);
        return this.result("failed", checkpoint, blueprint, stageResults, route, semanticStatus);
      }
      const text = responseText(run);
      const outputDigest = text ? await digestJson({ stage_id: stage.id, output: text }) : null;
      const selectionDigest = run.selection ? await digestJson(run.selection) : null;
      const outputBytes = new TextEncoder().encode(text).byteLength;
      const validation = validateWorkflowStageOutput(stage, run.response?.structured);
      let learningEpisodeId: string | null = null;
      if (run.status === "completed" && validation.errors.length === 0 && validation.declaredStatus === "completed" && this.learning) {
        const episodeId = `workflow:${checkpoint.job_id}:${stage.id}:g${checkpoint.generation + 1}`;
        const episode = await this.learning.prepareRun(run, { episodeId, runId: episodeId, stageId: stage.id, parentJobId: checkpoint.job_id, planRefinementDigest });
        learningEpisodeId = episode.episode_id;
      }
      stageResults.push({
        stage,
        run,
        output_digest: outputDigest,
        output_bytes: outputBytes,
        learning_episode_id: learningEpisodeId,
        declared_status: validation.declaredStatus,
        evidence: validation.evidence,
        uncertainty: validation.uncertainty,
        notes: validation.notes,
        next_actions: validation.nextActions,
        validation_errors: validation.errors,
      });
      if (run.status === "approval_required") {
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "approval_required", run_status: run.status, selection_digest: selectionDigest, response_digest: null, output_bytes: 0, error_class: null, learning_episode_id: null }], "paused", contractDigest, checkpoint, stageOrder, planRefinementDigest);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "approval_required", stage.id, checkpoint);
        return this.result("approval_required", checkpoint, blueprint, stageResults, route, semanticStatus);
      }
      if (run.status !== "completed") {
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "failed", run_status: run.status, selection_digest: selectionDigest, response_digest: outputDigest, output_bytes: outputBytes, error_class: null, learning_episode_id: null }], "failed", contractDigest, checkpoint, stageOrder, planRefinementDigest);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "stage_failed", stage.id, checkpoint);
        return this.result("failed", checkpoint, blueprint, stageResults, route, semanticStatus);
      }
      if (validation.errors.length > 0 || validation.declaredStatus !== "completed") {
        const errorClass = validation.errors.length > 0 ? "stage_output_invalid" : blockedWorkflowErrorClass(validation.declaredStatus) ?? "stage_not_completed";
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "failed", run_status: run.status, selection_digest: selectionDigest, response_digest: outputDigest, output_bytes: outputBytes, error_class: errorClass, error_code: "invalid_response", retryable: false, status_code: null, learning_episode_id: null }], "failed", contractDigest, checkpoint, stageOrder, planRefinementDigest);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "stage_failed", stage.id, checkpoint);
        return this.result(blockedWorkflowExecutionStatus(errorClass) ?? "failed", checkpoint, blueprint, stageResults, route, semanticStatus);
      }
      const completed = [...checkpoint.completed_stage_ids, stage.id];
      const outcomes = [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "completed" as const, run_status: run.status, selection_digest: selectionDigest, response_digest: outputDigest, output_bytes: outputBytes, error_class: null, learning_episode_id: learningEpisodeId }];
      const nextStatus: AutonomousWorkflowCheckpointStatus = completed.length === stages.length ? "completed" : "running";
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, completed, outcomes, nextStatus, contractDigest, checkpoint, stageOrder, planRefinementDigest);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, nextStatus === "completed" ? "completed" : "stage_completed", stage.id, checkpoint);
    }
    if (checkpoint.status === "completed") return this.result("completed", checkpoint, blueprint, stageResults, route, semanticStatus);
    checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, "paused", contractDigest, checkpoint, stageOrder, planRefinementDigest);
    await this.store.save(checkpoint);
    await this.appendEvent(checkpoint.job_id, "checkpointed", checkpoint.next_stage_id, checkpoint);
    return this.result("paused", checkpoint, blueprint, stageResults, route, semanticStatus);
  }

  private async result(status: AutonomousWorkflowExecutionStatus, checkpoint: AutonomousWorkflowCheckpoint, blueprint: AutonomousTaskBlueprint, stageResults: AutonomousWorkflowStageResult[], route: AutonomousRouteProposal, semanticStatus: AutonomousWorkflowSemanticRouteStatus | null): Promise<AutonomousWorkflowExecutionResult> {
    return { schema: AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA, status, job_id: checkpoint.job_id, blueprint, checkpoint, route, semantic_route_status: semanticStatus, events: await this.store.events(checkpoint.job_id, 0, AUTONOMOUS_WORKFLOW_MAX_EVENTS), stage_results: stageResults, completed_stage_count: checkpoint.completed_stage_ids.length, total_stage_count: blueprint.workflow.stages.length, plan_refinement_digest: checkpoint.plan_refinement_digest ?? null, learning_episode_ids: checkpoint.stage_outcomes.flatMap((outcome) => outcome.learning_episode_id ? [outcome.learning_episode_id] : []), recovery: "caller_rehydrates_task_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only" };
  }
}

function projectControlPlane<T extends JsonValue>(response: RestToolResponse<T>, operation: string): T {
  if (!response.ok || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError(`${operation} returned a control-plane refusal`);
  const value = response.mcp.result?.structuredContent;
  if (!value || typeof value !== "object") throw new ProviderRuntimeError(`${operation} returned no structured projection`);
  return value as T;
}

function boundedIdempotencyKey(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 512 || value.includes("\u0000")) throw new ArgumentError("durable job idempotencyKey is outside its bounded contract");
  return value;
}

/** Bridge local private execution to the value-only brain job control plane. */
export class AutonomousDurableJobController {
  readonly agent: AutonomousAgent;
  readonly apiClient: ApiClient;
  readonly executor: AutonomousWorkflowExecutor;

  constructor(agent: AutonomousAgent, apiClient: ApiClient, store: AutonomousWorkflowCheckpointStore) {
    if (!apiClient || typeof apiClient.brainJobSubmit !== "function" || typeof apiClient.brainJobStatus !== "function" || typeof apiClient.brainJobEvents !== "function" || typeof apiClient.brainJobApproval !== "function") throw new ArgumentError("durable job controller requires brain job ApiClient methods");
    this.agent = agent;
    this.apiClient = apiClient;
    this.executor = new AutonomousWorkflowExecutor(agent, store);
  }

  async submit(task: string, options: AutonomousDurableJobSubmitOptions): Promise<AutonomousDurableJobSubmission> {
    const route = await this.agent.route(task, { domain: options.domain, hints: options.hints });
    if (route.abstained || !route.primary_domain || route.cross_domain) return { schema: AUTONOMOUS_DURABLE_JOB_SCHEMA, status: "route_review_required", route, blueprint: null, job: null, spec_digest: null, execution: "not_started", private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane" };
    const envelope = await this.agent.blueprint(task, { domain: route.primary_domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name) });
    const blueprint = envelope.blueprint;
    if (!blueprint) return { schema: AUTONOMOUS_DURABLE_JOB_SCHEMA, status: "route_review_required", route, blueprint: null, job: null, spec_digest: null, execution: "not_started", private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane" };
    const projection = projectControlPlane(await this.apiClient.brainJobSubmit({
      idempotency_key: boundedIdempotencyKey(options.idempotencyKey),
      spec_digest: route.task_digest,
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      risk_class: blueprint.domain_profile.risk_class,
      priority: options.priority,
      max_attempts: options.maxAttempts,
      checkpoint_digest: options.checkpointDigest ?? null,
    }), "brain job submit");
    return { schema: AUTONOMOUS_DURABLE_JOB_SCHEMA, status: "submitted", route, blueprint, job: projection.job, spec_digest: route.task_digest, execution: "not_started", private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane" };
  }

  async status(jobId: string): Promise<BrainJobStatusResult> {
    return projectControlPlane(await this.apiClient.brainJobStatus({ job_id: boundedJobId(jobId) }), "brain job status");
  }

  async events(jobId?: string, after = 0, limit = AUTONOMOUS_WORKFLOW_MAX_EVENTS): Promise<BrainJobEventsResult> {
    return projectControlPlane(await this.apiClient.brainJobEvents({ job_id: jobId === undefined ? undefined : boundedJobId(jobId), after, limit }), "brain job events");
  }

  async approval(jobId: string, action: "request" | "approve" | "deny", options: { reason?: string; authorizationDigest?: string } = {}): Promise<BrainJobApprovalResult> {
    return projectControlPlane(await this.apiClient.brainJobApproval({ job_id: boundedJobId(jobId), action, reason: options.reason, authorization_digest: options.authorizationDigest }), "brain job approval");
  }

  async execute(jobId: string, task: string, options: Omit<AutonomousWorkflowExecuteOptions, "jobId"> = {}): Promise<AutonomousDurableJobExecutionResult> {
    const normalizedJobId = boundedJobId(jobId);
    const server = await this.status(normalizedJobId);
    if (server.job.state === "waiting_approval") {
      return { schema: AUTONOMOUS_DURABLE_JOB_SCHEMA, job: server.job, local: { schema: AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA, status: "approval_required", job_id: normalizedJobId, blueprint: null, checkpoint: null, route: null, semantic_route_status: null, events: [], stage_results: [], completed_stage_count: 0, total_stage_count: 0, plan_refinement_digest: null, learning_episode_ids: [], recovery: "caller_rehydrates_task_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only" }, server_job_posture: "control_plane_projection;completion_requires_external_worker_reconciliation", private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane" };
    }
    if (server.job.state !== "queued") throw new ProviderRuntimeError(`brain job ${normalizedJobId} is not executable in state ${server.job.state}`);
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(server.job.domain as AutonomousDomainName)) throw new ProviderRuntimeError(`brain job ${normalizedJobId} has an unsupported autonomous domain`);
    const local = await this.executor.start(task, { ...options, domain: server.job.domain as AutonomousDomainName, jobId: normalizedJobId });
    const refreshed = await this.status(normalizedJobId);
    return { schema: AUTONOMOUS_DURABLE_JOB_SCHEMA, job: refreshed.job, local, server_job_posture: "control_plane_projection;completion_requires_external_worker_reconciliation", private_spec: "caller_owned;task_prompt_response_and_credentials_not_sent_to_control_plane" };
  }
}
