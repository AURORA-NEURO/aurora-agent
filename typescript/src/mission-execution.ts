import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, validateAutonomousRouteOverride, type AutonomousAgent, type AutonomousDomainName, type AutonomousRunOptions, type AutonomousRunResult, type AutonomousRouteProposal } from "./autonomous.js";
import { AutonomousEffectReconciliationRequiredError, type AutonomousEffectBoundary } from "./autonomous-effects.js";
import { semanticRouteAutonomousTask } from "./autonomous-routing.js";
import type { AutonomousSemanticRouteOptions, AutonomousSemanticRouteResult } from "./autonomous-routing.js";
import { canonicalJson, digestJson, ToolCatalogue } from "./tooling.js";
import { preflightMission } from "./mission.js";
import { AutonomousCostBudget } from "./llm.js";
import type {
  AgentMissionArgs,
  AgentMissionPolicy,
  AgentMissionStep,
  JsonObject,
  JsonValue,
  MissionPreflightResult,
} from "./types.js";
import type { ProviderTool, ProviderToolCall, ProviderToolResult } from "./llm.js";
import {
  AutonomousPromptAdaptiveSelection,
  type AutonomousPromptAdaptiveSelectionJSON,
} from "./autonomous-prompt-registry.js";
import type {
  AutonomousEvaluatorRewardInput,
  AutonomousLearningEpisode,
  AutonomousLearningTrajectory,
  AutonomousTrajectorySettlement,
  AutonomousLearningOutboxSettlementOptions,
} from "./autonomous-learning.js";

/**
 * Local mission execution is deliberately separate from the remote `agent_mission` transport.
 * The remote server remains authoritative for its own queue, while this module gives an
 * embedding application a restart-safe executor that composes the same preflight contract with
 * a caller-owned step adapter. Raw arguments and outputs never enter the durable checkpoint.
 */
export const AUTONOMOUS_MISSION_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-mission-execution/0.3" as const;
export const AUTONOMOUS_MISSION_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-mission-checkpoint/0.3" as const;
export const AUTONOMOUS_MISSION_EVENT_SCHEMA = "bioprism-typescript-autonomous-mission-event/0.1" as const;
export const AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-mission-snapshot/0.3" as const;
export const AUTONOMOUS_MISSION_TRACE_SCHEMA_VERSION = "bioprism-typescript-autonomous-mission-trace/0.1" as const;
export const AUTONOMOUS_MISSION_STEP_QUALITY_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-mission-step-quality-evaluation/0.1" as const;

export const AUTONOMOUS_MISSION_EVENT_TYPES = [
  "mission.started",
  "wave.started",
  "step.started",
  "step.completed",
  "step.refused",
  "step.failed",
  "step.blocked",
  "step.cancelled",
  "approval.required",
  "reconciliation.required",
  "wave.completed",
  "checkpointed",
  "mission.cancelled",
  "mission.completed",
] as const;
export const AUTONOMOUS_MISSION_STATUSES = [
  "planned",
  "route_review_required",
  "policy_review_required",
  "policy_blocked",
  "running",
  "approval_required",
  "reconciliation_required",
  "recovery_required",
  "succeeded",
  "partial",
  "failed",
  "cancelled",
] as const;
export const AUTONOMOUS_MISSION_STEP_STATUSES = [
  "pending",
  "running",
  "succeeded",
  "approval_required",
  "reconciliation_required",
  "recovery_required",
  "refused",
  "failed",
  "blocked",
  "cancelled",
] as const;

export const AUTONOMOUS_MISSION_MAX_EVENTS = 32_768;
export const AUTONOMOUS_MISSION_MAX_JOBS = 4_096;
export const AUTONOMOUS_MISSION_MAX_SNAPSHOT_BYTES = 64_000_000;
export const AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL = 128;
export const AUTONOMOUS_MISSION_MAX_RESULT_BYTES = 20_000_000;
export const AUTONOMOUS_MISSION_MAX_ERROR_BYTES = 2_048;

export type AutonomousMissionEventType = typeof AUTONOMOUS_MISSION_EVENT_TYPES[number];
export type AutonomousMissionStatus = typeof AUTONOMOUS_MISSION_STATUSES[number];
export type AutonomousMissionStepStatus = typeof AUTONOMOUS_MISSION_STEP_STATUSES[number];
export type AutonomousMissionSemanticRouteStatus = AutonomousSemanticRouteResult["status"];

/** Digest-only receipt of the planning and model-selection decisions for one mission step. */
export interface AutonomousMissionStepDecision extends JsonObject {
  selection_digest: string | null;
  provider: string | null;
  model: string | null;
  route_digest: string | null;
  plan_digest: string | null;
  prompt_digest: string | null;
  /** Exact value-only prompt-arm receipt; rendered messages remain transient. */
  adaptive_selection?: AutonomousPromptAdaptiveSelectionJSON | null;
}

export class AutonomousMissionExecutionError extends ArgumentError {
  override readonly name: string = "AutonomousMissionExecutionError";
}

export class AutonomousMissionRecoveryError extends AutonomousMissionExecutionError {
  override readonly name: string = "AutonomousMissionRecoveryError";
  readonly missionId: string;
  readonly stepId: string;

  constructor(missionId: string, stepId: string, message: string) {
    super(`mission ${missionId} step ${stepId} requires caller-owned recovery: ${message}`);
    this.missionId = missionId;
    this.stepId = stepId;
  }
}

export class AutonomousMissionPolicyError extends AutonomousMissionExecutionError {
  override readonly name: string = "AutonomousMissionPolicyError";
}

export interface AutonomousMissionStepExecutionContext {
  mission_id: string;
  goal: string;
  wave: number;
  step: AgentMissionStep;
  arguments: JsonObject;
  dependency_outputs: Record<string, JsonValue>;
  execution_attempt: number;
  resumed: boolean;
  /** Shared caller-owned budget for provider-assisted step adapters; never persisted. */
  cost_budget?: AutonomousCostBudget;
  signal?: AbortSignal;
}

export interface AutonomousMissionStepExecutionResult {
  status: "succeeded" | "approval_required" | "reconciliation_required" | "refused" | "failed";
  value?: JsonValue;
  error_class?: string | null;
  detail?: string | null;
  effect_ids?: string[];
  run_status?: string | null;
  /** Value-only learning linkage; the episode itself is stored by the learning adapter. */
  learning_episode_id?: string | null;
  /** Digest-only route/plan/prompt/model receipt; raw provider material is never returned here. */
  decision?: AutonomousMissionStepDecision | null;
}

/**
 * Value-only quality decision used at the mission continuation boundary.
 *
 * The evaluator is caller-owned: this projection can score structural quality or connect to a
 * separately reviewed evaluator, but it never establishes external truth.  The raw step result
 * is deliberately available only to the transient callback and is represented durably by its
 * digest.  A failed decision is a hard continuation gate and requires an explicit retry.
 */
export interface AutonomousMissionStepQualityEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_MISSION_STEP_QUALITY_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: string;
  domain: AutonomousDomainName;
  mission_id: string;
  goal_digest: string;
  step_id: string;
  step_digest: string;
  result_digest: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  failure_class: string | null;
  feedback_digest: string;
  evidence_digest: string | null;
  evaluator_authority: "caller_declared_signal_scoring_only";
  retention: "value_only;step_result_not_retained";
  secret_material: "never_returned";
  evaluation_digest: string;
}

export interface AutonomousMissionStepQualityContext {
  mission_id: string;
  goal_digest: string;
  wave: number;
  attempt: number;
  step: AgentMissionStep;
  /** The raw value is transient and must never be copied into a quality projection. */
  result: JsonValue;
  result_digest: string;
}

export type AutonomousMissionStepQualityEvaluator = (
  context: AutonomousMissionStepQualityContext,
) => Promise<AutonomousEvaluatorRewardInput> | AutonomousEvaluatorRewardInput;

export type AutonomousMissionStepExecutor = (
  context: AutonomousMissionStepExecutionContext,
) => Promise<AutonomousMissionStepExecutionResult> | AutonomousMissionStepExecutionResult;

/** Raw results are retained only by this caller-owned adapter, never by the checkpoint store. */
export interface AutonomousMissionResultStore {
  save(missionId: string, stepId: string, value: JsonValue, resultDigest: string): Promise<void> | void;
  load(missionId: string, stepId: string, resultDigest: string): Promise<JsonValue | null> | JsonValue | null;
}

export class InMemoryAutonomousMissionResultStore implements AutonomousMissionResultStore {
  private readonly values = new Map<string, { value: JsonValue; result_digest: string }>();

  save(missionId: string, stepId: string, value: JsonValue, resultDigest: string): void {
    this.values.set(`${missionId}:${stepId}`, { value: structuredClone(value), result_digest: resultDigest });
  }

  async load(missionId: string, stepId: string, resultDigest: string): Promise<JsonValue | null> {
    const entry = this.values.get(`${missionId}:${stepId}`);
    if (!entry || entry.result_digest !== resultDigest) return null;
    return structuredClone(entry.value);
  }
}

export interface AutonomousMissionStepCheckpoint {
  status: AutonomousMissionStepStatus;
  result_digest: string | null;
  output_bytes: number;
  error_class: string | null;
  run_status: string | null;
  learning_episode_id: string | null;
  decision: AutonomousMissionStepDecision | null;
  /** Optional for backwards-compatible checkpoints created before mission quality gates. */
  quality?: AutonomousMissionStepQualityEvaluation | null;
  attempt: number;
  last_event_sequence: number;
}

export interface AutonomousMissionCheckpoint {
  schema: typeof AUTONOMOUS_MISSION_CHECKPOINT_SCHEMA;
  mission_id: string;
  request_digest: string;
  policy_digest: string;
  catalogue_digest: string;
  ordered_steps: string[];
  waves: string[][];
  step_states: Record<string, AutonomousMissionStepCheckpoint>;
  completed_step_ids: string[];
  next_wave: number | null;
  /** Digest of the approved top-level route, when this mission was route-bound. */
  route_digest?: string | null;
  output_bytes: number;
  generation: number;
  status: AutonomousMissionStatus;
  previous_checkpoint_digest: string | null;
  checkpoint_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
}

export interface AutonomousMissionEvent {
  schema: typeof AUTONOMOUS_MISSION_EVENT_SCHEMA;
  sequence: number;
  mission_id: string;
  event_type: AutonomousMissionEventType;
  wave: number | null;
  step_id: string | null;
  tool: string | null;
  status: string | null;
  arguments_digest: string | null;
  output_bytes: number;
  detail: string | null;
  checkpoint_digest: string;
  previous_event_digest: string | null;
  event_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
}

export interface AutonomousMissionCheckpointStore {
  load(missionId: string): Promise<AutonomousMissionCheckpoint | null> | AutonomousMissionCheckpoint | null;
  save(checkpoint: AutonomousMissionCheckpoint): Promise<void> | void;
  appendEvent(event: AutonomousMissionEvent): Promise<void> | void;
  events(missionId: string, after?: number, limit?: number): Promise<AutonomousMissionEvent[]> | AutonomousMissionEvent[];
}

export interface AutonomousMissionSnapshot {
  schema: typeof AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA;
  checkpoints: AutonomousMissionCheckpoint[];
  event_rows: AutonomousMissionEvent[];
  retention: "metadata_only_hash_chained";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousMissionSnapshotStore extends AutonomousMissionCheckpointStore {
  snapshot(): Promise<AutonomousMissionSnapshot>;
  restore(snapshot: AutonomousMissionSnapshot): Promise<void> | void;
}

export interface AutonomousMissionPersistence {
  read(): Promise<AutonomousMissionSnapshot | null> | AutonomousMissionSnapshot | null;
  write(snapshot: AutonomousMissionSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousMissionSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousMissionSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousMissionTransactionalSnapshotTextStore extends AutonomousMissionSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousMissionExecutionResult {
  schema: typeof AUTONOMOUS_MISSION_EXECUTION_SCHEMA;
  status: AutonomousMissionStatus;
  mission_id: string;
  preflight: MissionPreflightResult;
  checkpoint: AutonomousMissionCheckpoint | null;
  route: AutonomousRouteProposal | null;
  semantic_route_status: AutonomousMissionSemanticRouteStatus | null;
  events: AutonomousMissionEvent[];
  results: AutonomousMissionStepResult[];
  completed_steps: number;
  total_steps: number;
  succeeded_steps: number;
  refused_steps: number;
  blocked_steps: number;
  failed_steps: number;
  cancelled_steps: number;
  returned_bytes: number;
  next_wave: number | null;
  recovery: "caller_rehydrates_raw_results_and_credentials";
  retention: "provider_responses_local;checkpoint_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousMissionStepResult {
  step: AgentMissionStep;
  status: AutonomousMissionStepStatus;
  value: JsonValue | null;
  result_digest: string | null;
  output_bytes: number;
  error_class: string | null;
  run_status: string | null;
  learning_episode_id: string | null;
  decision: AutonomousMissionStepDecision | null;
  quality: AutonomousMissionStepQualityEvaluation | null;
  attempt: number;
}

/**
 * Narrow adapter contract for connecting mission outcomes to the existing online learner.
 * Implementations may be local, remote, or backed by a durable database; this module never
 * stores provider responses or evaluator evidence in a mission checkpoint.
 */
export interface AutonomousMissionLearningAdapter {
  prepareRun(
    result: AutonomousRunResult,
    options: { episodeId: string; runId?: string; stageId?: string; parentJobId?: string; planRefinementDigest?: string | null },
  ): Promise<AutonomousLearningEpisode> | AutonomousLearningEpisode;
  prepareTrajectory(
    episodeIds: readonly string[],
    options: { trajectoryId: string; discount?: number },
  ): Promise<AutonomousLearningTrajectory> | AutonomousLearningTrajectory;
  settleTrajectory(
    trajectoryId: string,
    rewards: Record<string, AutonomousEvaluatorRewardInput>,
    options?: { remote?: boolean; outbox?: AutonomousLearningOutboxSettlementOptions },
  ): Promise<AutonomousTrajectorySettlement> | AutonomousTrajectorySettlement;
}

export interface AutonomousMissionLearningSettlement extends JsonObject {
  schema: "bioprism-typescript-autonomous-mission-learning-settlement/0.1";
  mission_id: string;
  trajectory_id: string;
  episode_ids: string[];
  settlement: AutonomousTrajectorySettlement;
  retention: "value_only_learning_projection";
  secret_material: "never_returned";
}

export interface AutonomousMissionExecuteOptions {
  /** Maximum dependency waves to consume in this call; omitted means drive to a terminal state. */
  max_waves?: number;
  /** Stable logical retry/dispatch number supplied by the caller and passed to step adapters. */
  execution_attempt?: number;
  /** Re-run steps held by the mission quality gate; omitted means hold for operator review. */
  retryBlocked?: boolean;
  signal?: AbortSignal;
  /** The caller must explicitly approve provider invocation for this local executor. */
  approveProviderCall?: boolean;
  /** Reuse a caller-reviewed route; required to recover a route-bound checkpoint. */
  routeOverride?: AutonomousRouteProposal;
  /** Optional provider-assisted semantic route review for the mission goal. */
  semanticRouting?: AutonomousMissionSemanticRoutingOptions;
  /** Aggregate provider cost ceiling shared by routing and provider-backed step adapters. */
  maxTotalCostUnits?: number;
  /** Caller-owned aggregate budget shared across routing and nested step provider calls. */
  costBudget?: AutonomousCostBudget;
  /** Strict provider-free policy admission controls for semantic routing. */
  domainPolicyMode?: AutonomousSemanticRouteOptions["domainPolicyMode"];
  domainPolicyEvidenceReady?: boolean;
  domainPolicyEvaluatorConfigured?: boolean;
  domainPolicyEffectsRequested?: boolean;
  domainPolicyEffectsApproved?: boolean;
}

export interface AutonomousMissionSemanticRoutingOptions extends Pick<AutonomousSemanticRouteOptions, "candidates" | "credential" | "credentialFor" | "hints" | "approveProviderCall" | "minSemanticConfidence" | "maxDomains" | "allowCrossDomain" | "maxOutputTokens" | "maxCostPerMillionTokens" | "maxLatencyMs" | "minQuality" | "maxProviderFailovers" | "domainPolicyMode" | "domainPolicyEvidenceReady" | "domainPolicyEvaluatorConfigured" | "domainPolicyEffectsRequested" | "domainPolicyEffectsApproved"> {
  enabled?: boolean;
}

export interface AutonomousMissionExecutorOptions {
  catalogue: import("./tooling.js").ToolCatalogue;
  executeStep: AutonomousMissionStepExecutor;
  /** Optional agent used only when semanticRouting is enabled for a mission call. */
  agent?: AutonomousAgent;
  checkpointStore?: AutonomousMissionCheckpointStore;
  resultStore?: AutonomousMissionResultStore;
  /** Optional caller-owned quality gate invoked after a bounded result is produced and before it is persisted. */
  evaluateStep?: AutonomousMissionStepQualityEvaluator;
  /** Optional learning/evaluation projection; rewards are caller supplied, never inferred. */
  onStepOutcome?: (outcome: AutonomousMissionStepResult, context: { mission_id: string; wave: number }) => Promise<void> | void;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) throw new AutonomousMissionExecutionError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum = 512): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousMissionExecutionError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousMissionExecutionError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedInteger(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new AutonomousMissionExecutionError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function boundedDetail(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string") return null;
  if (/(api[_-]?key|authorization|bearer|credential|password|secret|access[_-]?token|refresh[_-]?token|private[_-]?key|gsk_|sk-)/i.test(value)) return "redacted_diagnostic";
  return value.replace(/[\r\n\u0000]/g, " ").slice(0, AUTONOMOUS_MISSION_MAX_ERROR_BYTES);
}

function safeLabel(value: unknown, fallback: string): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/.test(value)) return fallback;
  return value;
}

function jsonBytes(value: unknown): number {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new AutonomousMissionExecutionError("mission result is not JSON serializable");
  }
  if (typeof encoded !== "string") throw new AutonomousMissionExecutionError("mission result must be a JSON value");
  return new TextEncoder().encode(encoded).byteLength;
}

function safeFailureClass(error: unknown): string {
  return error instanceof Error && error.constructor.name ? error.constructor.name.slice(0, 128) : "ExecutionError";
}

function validPointer(pointer: string, allowEmpty: boolean): boolean {
  if (pointer === "") return allowEmpty;
  if (!pointer.startsWith("/") || /[\u0000-\u001f]/.test(pointer)) return false;
  for (let index = 0; index < pointer.length; index += 1) {
    if (pointer[index] === "~" && pointer[index + 1] !== "0" && pointer[index + 1] !== "1") return false;
    if (pointer[index] === "~") index += 1;
  }
  return true;
}

function pointerTokens(pointer: string): string[] {
  return pointer.slice(1).split("/").map((token) => token.replaceAll("~1", "/").replaceAll("~0", "~"));
}

function pointerGet(value: unknown, pointer: string): JsonValue | undefined {
  if (pointer === "") return value as JsonValue;
  if (!validPointer(pointer, false)) return undefined;
  let current: unknown = value;
  for (const token of pointerTokens(pointer)) {
    if (isObject(current) && token in current) current = current[token];
    else if (Array.isArray(current) && /^\d+$/.test(token) && Number(token) < current.length) current = current[Number(token)];
    else return undefined;
  }
  return current as JsonValue;
}

function pointerSet(root: JsonObject, pointer: string, value: JsonValue): JsonObject {
  if (!validPointer(pointer, false)) throw new AutonomousMissionExecutionError(`invalid binding target pointer: ${pointer}`);
  const tokens = pointerTokens(pointer);
  if (!tokens.length) throw new AutonomousMissionExecutionError("binding target pointer cannot address the root object");
  const result = clone(root) as JsonObject;
  let current: JsonValue = result;
  for (let index = 0; index < tokens.length - 1; index += 1) {
    const token = tokens[index] as string;
    if (isObject(current) && token in current) current = current[token] as JsonValue;
    else if (Array.isArray(current) && /^\d+$/.test(token) && Number(token) < current.length) current = current[Number(token)] as JsonValue;
    else throw new AutonomousMissionExecutionError(`binding target path disappeared: ${pointer}`);
  }
  const finalToken = tokens.at(-1) as string;
  if (isObject(current) && finalToken in current) current[finalToken] = value;
  else if (Array.isArray(current) && /^\d+$/.test(finalToken) && Number(finalToken) < current.length) current[Number(finalToken)] = value;
  else throw new AutonomousMissionExecutionError(`binding target path disappeared: ${pointer}`);
  return result;
}

function normalizeDecision(value: unknown): AutonomousMissionStepDecision | null {
  if (value === undefined || value === null) return null;
  if (!isObject(value)) throw new AutonomousMissionExecutionError("step decision metadata must be an object");
  const normalized: AutonomousMissionStepDecision = {
    selection_digest: boundedDigest("step decision selection_digest", value.selection_digest, true),
    provider: safeLabel(value.provider, "unknown"),
    model: safeLabel(value.model, "unknown"),
    route_digest: boundedDigest("step decision route_digest", value.route_digest, true),
    plan_digest: boundedDigest("step decision plan_digest", value.plan_digest, true),
    prompt_digest: boundedDigest("step decision prompt_digest", value.prompt_digest, true),
  };
  if (value.adaptive_selection !== undefined) {
    if (value.adaptive_selection === null) normalized.adaptive_selection = null;
    else {
      try {
        normalized.adaptive_selection = AutonomousPromptAdaptiveSelection.fromJSON(value.adaptive_selection as JsonObject).toJSON();
      } catch (error) {
        throw new AutonomousMissionExecutionError(`step decision adaptive_selection is malformed: ${error instanceof Error ? error.message : "invalid selection"}`);
      }
    }
  }
  return normalized;
}

function normalizeStepResult(value: unknown): AutonomousMissionStepExecutionResult {
  if (!isObject(value)) throw new AutonomousMissionExecutionError("step executor must return an object");
  const status = value.status;
  if (status !== "succeeded" && status !== "approval_required" && status !== "reconciliation_required" && status !== "refused" && status !== "failed") throw new AutonomousMissionExecutionError("step executor returned an unsupported status");
  return {
    status,
    ...(value.value === undefined ? {} : { value: value.value as JsonValue }),
    error_class: safeLabel(value.error_class, "UnclassifiedStepFailure"),
    detail: boundedDetail(value.detail),
    effect_ids: Array.isArray(value.effect_ids) ? value.effect_ids.filter((id): id is string => typeof id === "string").slice(0, 32) : [],
    run_status: safeLabel(value.run_status, "unknown"),
    learning_episode_id: value.learning_episode_id === undefined || value.learning_episode_id === null
      ? null
      : boundedIdentifier("learning_episode_id", value.learning_episode_id),
    decision: normalizeDecision(value.decision),
  };
}

function assertQualitySafeValue(value: unknown, depth = 0): void {
  if (depth > 8) throw new AutonomousMissionExecutionError("mission step quality evaluation is too deeply nested");
  if (typeof value === "string") {
    if (/\b(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})/i.test(value)) throw new AutonomousMissionExecutionError("mission step quality evaluation contains credential-shaped material");
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) assertQualitySafeValue(item, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "accesstoken", "refreshtoken", "privatekey"].includes(normalized)) throw new AutonomousMissionExecutionError("mission step quality evaluation contains credential-shaped fields");
      assertQualitySafeValue(child, depth + 1);
    }
  }
}

/** Normalize a caller reward into a digest-bound, metadata-only mission quality projection. */
async function normalizeMissionStepQuality(
  value: unknown,
  expected?: Pick<AutonomousMissionStepQualityContext, "mission_id" | "goal_digest" | "step" | "result_digest">,
): Promise<AutonomousMissionStepQualityEvaluation> {
  if (!isObject(value)) throw new AutonomousMissionExecutionError("mission step quality evaluator must return an object");
  assertQualitySafeValue(value);
  const projection = value.schema === AUTONOMOUS_MISSION_STEP_QUALITY_EVALUATION_SCHEMA;
  const allowed = [
    "schema", "evaluator_id", "evaluator_version", "domain", "mission_id", "goal_digest", "step_id", "step_digest",
    "result_digest", "reward", "passed", "failed", "failure_class", "feedback_digest", "evidence_digest",
    "evaluator_authority", "retention", "secret_material", "evaluation_digest",
  ] as const;
  if (projection && (Object.keys(value).length !== allowed.length || Object.keys(value).some((key) => !allowed.includes(key as typeof allowed[number])))) throw new AutonomousMissionExecutionError("mission step quality evaluation contains unsupported or missing fields");
  if (!projection) {
    const inputAllowed = new Set(["evaluator_id", "evaluator_version", "reward", "passed", "failed", "failure_class", "feedback_digest", "evidence_digest"]);
    if (Object.keys(value).some((key) => !inputAllowed.has(key))) throw new AutonomousMissionExecutionError("mission step quality evaluator returned unsupported fields");
  }
  const evaluatorId = boundedIdentifier("mission step quality evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedIdentifier("mission step quality evaluator_version", value.evaluator_version);
  const domain = boundedIdentifier("mission step quality domain", value.domain ?? expected?.step.domain);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new AutonomousMissionExecutionError("mission step quality domain is not supported");
  const missionId = boundedIdentifier("mission step quality mission_id", value.mission_id ?? expected?.mission_id);
  const stepId = boundedIdentifier("mission step quality step_id", value.step_id ?? expected?.step.id);
  const goalDigest = boundedDigest("mission step quality goal_digest", value.goal_digest ?? expected?.goal_digest);
  const stepDigest = boundedDigest("mission step quality step_digest", value.step_digest ?? (expected ? await digestJson(expected.step) : undefined));
  const resultDigest = boundedDigest("mission step quality result_digest", value.result_digest ?? expected?.result_digest);
  if (expected && (missionId !== expected.mission_id || domain !== expected.step.domain || stepId !== expected.step.id || goalDigest !== expected.goal_digest || stepDigest !== await digestJson(expected.step) || resultDigest !== expected.result_digest)) throw new AutonomousMissionExecutionError("mission step quality evaluation is not bound to the scheduled result");
  if (typeof value.reward !== "number" || !Number.isFinite(value.reward) || value.reward < 0 || value.reward > 1 || typeof value.passed !== "boolean") throw new AutonomousMissionExecutionError("mission step quality reward or passed flag is invalid");
  const failed = value.failed === undefined ? !value.passed : value.failed;
  if (typeof failed !== "boolean" || failed === value.passed) throw new AutonomousMissionExecutionError("mission step quality passed and failed flags are inconsistent");
  const failureClass = value.failure_class === undefined || value.failure_class === null
    ? (failed ? "MissionStepQualityGateRejected" : null)
    : boundedIdentifier("mission step quality failure_class", value.failure_class);
  if (value.passed && failureClass !== null) throw new AutonomousMissionExecutionError("passed mission step quality evaluations cannot contain failure_class");
  const evidenceDigest = boundedDigest("mission step quality evidence_digest", value.evidence_digest, true);
  const feedbackDigest = value.feedback_digest === undefined || value.feedback_digest === null
    ? await digestJson({ schema: AUTONOMOUS_MISSION_STEP_QUALITY_EVALUATION_SCHEMA, evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, mission_id: missionId, step_id: stepId, result_digest: resultDigest, reward: value.reward, passed: value.passed, failed, failure_class: failureClass, evidence_digest: evidenceDigest })
    : boundedDigest("mission step quality feedback_digest", value.feedback_digest) as string;
  const descriptor = {
    schema: AUTONOMOUS_MISSION_STEP_QUALITY_EVALUATION_SCHEMA,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    domain: domain as AutonomousDomainName,
    mission_id: missionId,
    goal_digest: goalDigest as string,
    step_id: stepId,
    step_digest: stepDigest as string,
    result_digest: resultDigest as string,
    reward: value.reward,
    passed: value.passed,
    failed,
    failure_class: failureClass,
    feedback_digest: feedbackDigest,
    evidence_digest: evidenceDigest,
    evaluator_authority: "caller_declared_signal_scoring_only" as const,
    retention: "value_only;step_result_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  const evaluationDigest = await digestJson(descriptor);
  if (projection && value.evaluator_authority !== descriptor.evaluator_authority || projection && value.retention !== descriptor.retention || projection && value.secret_material !== descriptor.secret_material || projection && value.evaluation_digest !== evaluationDigest) throw new AutonomousMissionExecutionError("mission step quality evaluation authority, retention, or digest is invalid");
  return { ...descriptor, evaluation_digest: evaluationDigest };
}

function stepState(status: AutonomousMissionStepStatus = "pending"): AutonomousMissionStepCheckpoint {
  return { status, result_digest: null, output_bytes: 0, error_class: null, run_status: null, learning_episode_id: null, decision: null, quality: null, attempt: 0, last_event_sequence: 0 };
}

function policyOf(mission: AgentMissionArgs): AgentMissionPolicy {
  return mission.policy ?? {};
}

function normalizeMissionCostOptions<T extends AutonomousMissionExecuteOptions>(options: T): T {
  if (options.costBudget !== undefined && !(options.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("costBudget must be an AutonomousCostBudget");
  if (options.costBudget !== undefined && options.maxTotalCostUnits !== undefined) throw new ArgumentError("costBudget and maxTotalCostUnits cannot both be supplied");
  if (options.costBudget !== undefined || options.maxTotalCostUnits === undefined) return options;
  return { ...options, maxTotalCostUnits: undefined, costBudget: new AutonomousCostBudget(options.maxTotalCostUnits) } as T;
}

interface MissionRouteResolution {
  route: AutonomousRouteProposal | null;
  semantic_status: AutonomousMissionSemanticRouteStatus | null;
}

function missionDomains(mission: AgentMissionArgs): AutonomousDomainName[] {
  const domains: AutonomousDomainName[] = [];
  for (const step of mission.steps) {
    const domain = step.domain as AutonomousDomainName;
    if (AUTONOMOUS_DOMAIN_NAMES.includes(domain) && !domains.includes(domain)) domains.push(domain);
  }
  return domains;
}

function routeMatchesMission(route: AutonomousRouteProposal, mission: AgentMissionArgs): boolean {
  const expected = missionDomains(mission).sort();
  const selected = [...route.selected_domains].sort();
  return !route.abstained
    && route.primary_domain !== null
    && selected.length === expected.length
    && selected.every((domain, index) => domain === expected[index])
    && route.cross_domain === (expected.length > 1);
}

function isTerminalStep(status: AutonomousMissionStepStatus): boolean {
  return status === "succeeded" || status === "refused" || status === "failed" || status === "blocked" || status === "cancelled";
}

function requiredFailure(step: AgentMissionStep, status: AutonomousMissionStepStatus): boolean {
  return step.required !== false && status !== "succeeded";
}

async function validateCheckpoint(value: unknown): Promise<AutonomousMissionCheckpoint> {
  if (!isObject(value)) throw new AutonomousMissionExecutionError("mission checkpoint must be an object");
  const checkpoint = value as unknown as AutonomousMissionCheckpoint;
  if (checkpoint.schema !== AUTONOMOUS_MISSION_CHECKPOINT_SCHEMA || checkpoint.retention !== "metadata_only_no_arguments_outputs_credentials_or_provider_material" || checkpoint.secret_material !== "never_returned") throw new AutonomousMissionExecutionError("mission checkpoint retention markers are invalid");
  boundedIdentifier("mission checkpoint mission_id", checkpoint.mission_id);
  boundedDigest("mission checkpoint request_digest", checkpoint.request_digest);
  boundedDigest("mission checkpoint policy_digest", checkpoint.policy_digest);
  boundedDigest("mission checkpoint catalogue_digest", checkpoint.catalogue_digest);
  if (checkpoint.route_digest !== undefined) boundedDigest("mission checkpoint route_digest", checkpoint.route_digest, true);
  if (!Array.isArray(checkpoint.ordered_steps) || !Array.isArray(checkpoint.waves) || !isObject(checkpoint.step_states)) throw new AutonomousMissionExecutionError("mission checkpoint graph metadata is malformed");
  if (checkpoint.ordered_steps.length > AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL || checkpoint.waves.length > AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL) throw new AutonomousMissionExecutionError("mission checkpoint exceeds its step capacity");
  const ids = new Set<string>();
  for (const id of checkpoint.ordered_steps) { boundedIdentifier("mission checkpoint step id", id); if (ids.has(id)) throw new AutonomousMissionExecutionError("mission checkpoint contains duplicate step ids"); ids.add(id); }
  if (Object.keys(checkpoint.step_states).some((id) => !ids.has(id))) throw new AutonomousMissionExecutionError("mission checkpoint contains an unknown step state");
  if (!Array.isArray(checkpoint.completed_step_ids) || checkpoint.completed_step_ids.some((id) => !ids.has(id))) throw new AutonomousMissionExecutionError("mission checkpoint completed steps are malformed");
  if (checkpoint.next_wave !== null) boundedInteger("mission checkpoint next_wave", checkpoint.next_wave, AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL);
  boundedInteger("mission checkpoint output_bytes", checkpoint.output_bytes, AUTONOMOUS_MISSION_MAX_RESULT_BYTES);
  boundedInteger("mission checkpoint generation", checkpoint.generation, Number.MAX_SAFE_INTEGER, 1);
  if (!AUTONOMOUS_MISSION_STATUSES.includes(checkpoint.status)) throw new AutonomousMissionExecutionError("mission checkpoint status is invalid");
  boundedDigest("mission checkpoint previous_checkpoint_digest", checkpoint.previous_checkpoint_digest, true);
  boundedDigest("mission checkpoint checkpoint_digest", checkpoint.checkpoint_digest);
  const descriptor = { schema: checkpoint.schema, mission_id: checkpoint.mission_id, request_digest: checkpoint.request_digest, policy_digest: checkpoint.policy_digest, catalogue_digest: checkpoint.catalogue_digest, ...(checkpoint.route_digest === undefined ? {} : { route_digest: checkpoint.route_digest }), ordered_steps: checkpoint.ordered_steps, waves: checkpoint.waves, step_states: checkpoint.step_states, completed_step_ids: checkpoint.completed_step_ids, next_wave: checkpoint.next_wave, output_bytes: checkpoint.output_bytes, generation: checkpoint.generation, status: checkpoint.status, previous_checkpoint_digest: checkpoint.previous_checkpoint_digest, retention: checkpoint.retention, secret_material: checkpoint.secret_material };
  if (await digestJson(descriptor) !== checkpoint.checkpoint_digest) throw new AutonomousMissionExecutionError("mission checkpoint digest does not match its metadata");
  for (const id of checkpoint.ordered_steps) {
    const state = checkpoint.step_states[id];
    if (!state || !AUTONOMOUS_MISSION_STEP_STATUSES.includes(state.status)) throw new AutonomousMissionExecutionError(`mission checkpoint step state is invalid for ${id}`);
    boundedDigest(`mission checkpoint ${id}.result_digest`, state.result_digest, true);
    boundedInteger(`mission checkpoint ${id}.output_bytes`, state.output_bytes, AUTONOMOUS_MISSION_MAX_RESULT_BYTES);
    boundedInteger(`mission checkpoint ${id}.attempt`, state.attempt, AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL);
    boundedInteger(`mission checkpoint ${id}.last_event_sequence`, state.last_event_sequence, AUTONOMOUS_MISSION_MAX_EVENTS);
    if (state.error_class !== null && typeof state.error_class !== "string") throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.error_class is malformed`);
    if (state.run_status !== null && typeof state.run_status !== "string") throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.run_status is malformed`);
    if (state.learning_episode_id !== null) boundedIdentifier(`mission checkpoint ${id}.learning_episode_id`, state.learning_episode_id);
    if (state.decision !== null && await digestJson(normalizeDecision(state.decision)) !== await digestJson(state.decision)) throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.decision is malformed`);
    if (state.quality !== undefined && state.quality !== null) {
      const quality = await normalizeMissionStepQuality(state.quality);
      if (quality.mission_id !== checkpoint.mission_id || quality.step_id !== id) throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.quality is bound to another mission step`);
    }
    if (state.error_class !== null && safeLabel(state.error_class, "UnclassifiedStepFailure") !== state.error_class) throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.error_class is not a safe label`);
    if (state.run_status !== null && safeLabel(state.run_status, "unknown") !== state.run_status) throw new AutonomousMissionExecutionError(`mission checkpoint ${id}.run_status is not a safe label`);
  }
  return clone(checkpoint);
}

async function validateEvent(value: unknown): Promise<AutonomousMissionEvent> {
  if (!isObject(value)) throw new AutonomousMissionExecutionError("mission event must be an object");
  const event = value as unknown as AutonomousMissionEvent;
  if (event.schema !== AUTONOMOUS_MISSION_EVENT_SCHEMA || event.retention !== "metadata_only_no_arguments_outputs_credentials_or_provider_material" || event.secret_material !== "never_returned") throw new AutonomousMissionExecutionError("mission event retention markers are invalid");
  boundedInteger("mission event sequence", event.sequence, AUTONOMOUS_MISSION_MAX_EVENTS);
  boundedIdentifier("mission event mission_id", event.mission_id);
  if (!AUTONOMOUS_MISSION_EVENT_TYPES.includes(event.event_type)) throw new AutonomousMissionExecutionError("mission event type is invalid");
  if (event.wave !== null) boundedInteger("mission event wave", event.wave, AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL);
  if (event.step_id !== null) boundedIdentifier("mission event step_id", event.step_id);
  if (event.tool !== null) boundedIdentifier("mission event tool", event.tool);
  if (event.arguments_digest !== null) boundedDigest("mission event arguments_digest", event.arguments_digest);
  boundedInteger("mission event output_bytes", event.output_bytes, AUTONOMOUS_MISSION_MAX_RESULT_BYTES);
  if (boundedDetail(event.detail) !== event.detail) throw new AutonomousMissionExecutionError("mission event detail is unbounded or secret-shaped");
  boundedDigest("mission event checkpoint_digest", event.checkpoint_digest);
  boundedDigest("mission event previous_event_digest", event.previous_event_digest, true);
  boundedDigest("mission event event_digest", event.event_digest);
  const descriptor = { schema: event.schema, sequence: event.sequence, mission_id: event.mission_id, event_type: event.event_type, wave: event.wave, step_id: event.step_id, tool: event.tool, status: event.status, arguments_digest: event.arguments_digest, output_bytes: event.output_bytes, detail: event.detail, checkpoint_digest: event.checkpoint_digest, previous_event_digest: event.previous_event_digest, retention: event.retention, secret_material: event.secret_material };
  if (await digestJson(descriptor) !== event.event_digest) throw new AutonomousMissionExecutionError("mission event digest does not match its metadata");
  return clone(event);
}

export async function validateAutonomousMissionSnapshot(value: unknown): Promise<{ snapshot: AutonomousMissionSnapshot; eventCount: number }> {
  if (!isObject(value)) throw new AutonomousMissionExecutionError("mission snapshot must be an object");
  const snapshot = value as unknown as AutonomousMissionSnapshot;
  if (snapshot.schema !== AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA || snapshot.retention !== "metadata_only_hash_chained" || snapshot.secret_material !== "never_returned") throw new AutonomousMissionExecutionError("mission snapshot retention markers are invalid");
  if (!Array.isArray(snapshot.checkpoints) || !Array.isArray(snapshot.event_rows) || snapshot.checkpoints.length > AUTONOMOUS_MISSION_MAX_JOBS || snapshot.event_rows.length > AUTONOMOUS_MISSION_MAX_EVENTS) throw new AutonomousMissionExecutionError("mission snapshot capacity is exhausted");
  const checkpoints: AutonomousMissionCheckpoint[] = [];
  const ids = new Set<string>();
  for (const raw of snapshot.checkpoints) { const checkpoint = await validateCheckpoint(raw); if (ids.has(checkpoint.mission_id)) throw new AutonomousMissionExecutionError("mission snapshot contains duplicate checkpoints"); ids.add(checkpoint.mission_id); checkpoints.push(checkpoint); }
  const events: AutonomousMissionEvent[] = [];
  for (const raw of snapshot.event_rows) { const event = await validateEvent(raw); if (!ids.has(event.mission_id)) throw new AutonomousMissionExecutionError("mission snapshot event has no checkpoint"); events.push(event); }
  const priorByMission = new Map<string, AutonomousMissionEvent>();
  for (const event of events.sort((left, right) => left.mission_id.localeCompare(right.mission_id) || left.sequence - right.sequence)) {
    const prior = priorByMission.get(event.mission_id);
    if (event.sequence !== (prior?.sequence ?? 0) + 1 || event.previous_event_digest !== (prior?.event_digest ?? null)) throw new AutonomousMissionExecutionError(`mission snapshot event chain is not contiguous for ${event.mission_id}`);
    priorByMission.set(event.mission_id, event);
  }
  const descriptor = { schema: snapshot.schema, checkpoints, event_rows: events, retention: snapshot.retention, secret_material: snapshot.secret_material };
  const snapshotDigest = await digestJson(descriptor);
  if (snapshotDigest !== snapshot.snapshot_digest) throw new AutonomousMissionExecutionError("mission snapshot digest does not match its metadata");
  return { snapshot: { ...clone(snapshot), checkpoints, event_rows: events }, eventCount: events.length };
}

export class InMemoryAutonomousMissionCheckpointStore implements AutonomousMissionSnapshotStore {
  private readonly checkpoints = new Map<string, AutonomousMissionCheckpoint>();
  private readonly eventRows = new Map<string, AutonomousMissionEvent[]>();

  async load(missionId: string): Promise<AutonomousMissionCheckpoint | null> {
    return clone(this.checkpoints.get(boundedIdentifier("mission_id", missionId)) ?? null);
  }

  async save(checkpoint: AutonomousMissionCheckpoint): Promise<void> {
    const normalized = await validateCheckpoint(checkpoint);
    const previous = this.checkpoints.get(normalized.mission_id);
    if (!previous) {
      if (normalized.generation !== 1 || normalized.previous_checkpoint_digest !== null) throw new AutonomousMissionExecutionError("initial mission checkpoint must start at generation one");
    } else if (previous.checkpoint_digest !== normalized.checkpoint_digest && (normalized.generation !== previous.generation + 1 || normalized.previous_checkpoint_digest !== previous.checkpoint_digest)) {
      throw new AutonomousMissionExecutionError("mission checkpoint generation is not contiguous");
    }
    if (!previous && this.checkpoints.size >= AUTONOMOUS_MISSION_MAX_JOBS) throw new AutonomousMissionExecutionError("mission checkpoint capacity is exhausted");
    this.checkpoints.set(normalized.mission_id, clone(normalized));
  }

  async appendEvent(event: AutonomousMissionEvent): Promise<void> {
    const normalized = await validateEvent(event);
    if (!this.checkpoints.has(normalized.mission_id)) throw new AutonomousMissionExecutionError("mission event requires an existing checkpoint");
    const rows = this.eventRows.get(normalized.mission_id) ?? [];
    const previous = rows.at(-1);
    if (normalized.sequence !== (previous?.sequence ?? 0) + 1 || normalized.previous_event_digest !== (previous?.event_digest ?? null)) throw new AutonomousMissionExecutionError("mission event sequence or hash chain is not contiguous");
    if (rows.length >= AUTONOMOUS_MISSION_MAX_EVENTS) throw new AutonomousMissionExecutionError("mission event capacity is exhausted");
    rows.push(clone(normalized));
    this.eventRows.set(normalized.mission_id, rows);
  }

  async events(missionId: string, after = 0, limit = AUTONOMOUS_MISSION_MAX_EVENTS): Promise<AutonomousMissionEvent[]> {
    boundedIdentifier("mission_id", missionId);
    boundedInteger("event after", after, AUTONOMOUS_MISSION_MAX_EVENTS);
    boundedInteger("event limit", limit, AUTONOMOUS_MISSION_MAX_EVENTS, 1);
    return clone((this.eventRows.get(missionId) ?? []).filter((event) => event.sequence > after).slice(0, limit));
  }

  async snapshot(): Promise<AutonomousMissionSnapshot> {
    const checkpoints = [...this.checkpoints.values()].sort((a, b) => a.mission_id.localeCompare(b.mission_id)).map(clone);
    const event_rows = [...this.eventRows.values()].flat().sort((a, b) => a.mission_id.localeCompare(b.mission_id) || a.sequence - b.sequence).map(clone);
    const descriptor = { schema: AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA, checkpoints, event_rows, retention: "metadata_only_hash_chained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: await digestJson(descriptor) };
  }

  async restore(snapshot: AutonomousMissionSnapshot): Promise<void> {
    const validated = await validateAutonomousMissionSnapshot(snapshot);
    this.checkpoints.clear();
    this.eventRows.clear();
    for (const checkpoint of validated.snapshot.checkpoints) this.checkpoints.set(checkpoint.mission_id, clone(checkpoint));
    for (const event of validated.snapshot.event_rows) this.eventRows.set(event.mission_id, [...(this.eventRows.get(event.mission_id) ?? []), clone(event)]);
  }
}

/** Coordinates a metadata-only mission snapshot with a caller-owned durable adapter. */
export class AutonomousMissionPersistenceCoordinator {
  readonly store: AutonomousMissionSnapshotStore;
  readonly persistence: AutonomousMissionPersistence;
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(store: AutonomousMissionSnapshotStore, persistence: AutonomousMissionPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("mission persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("mission persistence adapter is malformed");
    this.store = store;
    this.persistence = persistence;
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA; bytes: number; snapshot_digest: string; retention: "metadata_only" }> {
    return this.enqueue(async () => {
      const snapshot = (await validateAutonomousMissionSnapshot(await this.store.snapshot())).snapshot;
      const bytes = jsonBytes(snapshot);
      if (bytes > AUTONOMOUS_MISSION_MAX_SNAPSHOT_BYTES) throw new AutonomousMissionExecutionError("mission snapshot exceeds its bounded size");
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new AutonomousMissionExecutionError("mission persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return { schema: AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA, bytes, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
    });
  }

  async restore(): Promise<{ schema: typeof AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA; restored: boolean; missions: number; events: number; snapshot_digest: string | null; retention: "metadata_only" }> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return { schema: AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA, restored: false, missions: 0, events: 0, snapshot_digest: null, retention: "metadata_only" };
      }
      const validated = await validateAutonomousMissionSnapshot(raw);
      await this.store.restore(validated.snapshot);
      this.expectedSnapshotDigest = validated.snapshot.snapshot_digest;
      return { schema: AUTONOMOUS_MISSION_SNAPSHOT_SCHEMA, restored: true, missions: validated.snapshot.checkpoints.length, events: validated.eventCount, snapshot_digest: validated.snapshot.snapshot_digest, retention: "metadata_only" };
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousMissionSnapshotPersistence implements AutonomousMissionPersistence {
  constructor(readonly textStore: AutonomousMissionSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new AutonomousMissionExecutionError("mission text store is malformed");
  }

  async read(): Promise<AutonomousMissionSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (jsonBytes(encoded) > AUTONOMOUS_MISSION_MAX_SNAPSHOT_BYTES) throw new AutonomousMissionExecutionError("mission JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new AutonomousMissionExecutionError("mission JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new AutonomousMissionExecutionError("mission JSON is not canonical");
    return (await validateAutonomousMissionSnapshot(parsed)).snapshot;
  }

  async write(raw: AutonomousMissionSnapshot): Promise<void> {
    const snapshot = (await validateAutonomousMissionSnapshot(raw)).snapshot;
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousMissionSnapshotPersistence extends JsonAutonomousMissionSnapshotPersistence {
  declare readonly textStore: AutonomousMissionTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousMissionTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new AutonomousMissionExecutionError("mission text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, raw: AutonomousMissionSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new AutonomousMissionExecutionError("mission expected snapshot digest is invalid");
    const snapshot = (await validateAutonomousMissionSnapshot(raw)).snapshot;
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}

function classifyStatus(result: AutonomousMissionStepExecutionResult): AutonomousMissionStepStatus {
  return result.status;
}

function missionStatus(checkpoint: AutonomousMissionCheckpoint, steps: readonly AgentMissionStep[]): AutonomousMissionStatus {
  const states = steps.map((step) => checkpoint.step_states[step.id]?.status ?? "pending");
  if (states.some((status) => status === "reconciliation_required")) return "reconciliation_required";
  if (states.some((status) => status === "recovery_required")) return "recovery_required";
  if (states.some((status) => status === "approval_required")) return "approval_required";
  if (states.every((status) => status === "succeeded" || status === "refused" || status === "failed" || status === "blocked" || status === "cancelled")) {
    if (states.some((status, index) => requiredFailure(steps[index] as AgentMissionStep, status))) return states.every((status) => status === "cancelled") ? "cancelled" : "failed";
    return states.some((status) => status !== "succeeded") ? "partial" : "succeeded";
  }
  return checkpoint.status === "cancelled" ? "cancelled" : "running";
}

export class AutonomousMissionExecutor {
  readonly catalogue: ToolCatalogue;
  readonly executeStep: AutonomousMissionStepExecutor;
  readonly agent?: AutonomousAgent;
  readonly store: AutonomousMissionCheckpointStore;
  readonly resultStore: AutonomousMissionResultStore;
  readonly evaluateStep?: AutonomousMissionStepQualityEvaluator;
  readonly onStepOutcome?: AutonomousMissionExecutorOptions["onStepOutcome"];
  private eventQueue: Promise<void> = Promise.resolve();

  constructor(options: AutonomousMissionExecutorOptions) {
    if (!options || !isObject(options)) throw new ArgumentError("mission executor options are required");
    if (!(options.catalogue instanceof ToolCatalogue)) throw new ArgumentError("mission executor requires a ToolCatalogue");
    if (typeof options.executeStep !== "function") throw new ArgumentError("mission executor requires an executeStep callback");
    this.catalogue = options.catalogue;
    this.executeStep = options.executeStep;
    this.agent = options.agent;
    this.store = options.checkpointStore ?? new InMemoryAutonomousMissionCheckpointStore();
    this.resultStore = options.resultStore ?? new InMemoryAutonomousMissionResultStore();
    this.evaluateStep = options.evaluateStep;
    this.onStepOutcome = options.onStepOutcome;
    if (this.evaluateStep !== undefined && typeof this.evaluateStep !== "function") throw new ArgumentError("mission executor evaluateStep must be callable");
    if (!this.store || typeof this.store.load !== "function" || typeof this.store.save !== "function" || typeof this.store.appendEvent !== "function" || typeof this.store.events !== "function") throw new ArgumentError("mission executor checkpoint store is malformed");
  }

  async preflight(mission: AgentMissionArgs): Promise<MissionPreflightResult> {
    const unsupported = Array.isArray(mission.steps)
      ? mission.steps.filter((step) => !AUTONOMOUS_DOMAIN_NAMES.includes(step.domain as AutonomousDomainName)).map((step) => `${step.id}: unsupported autonomous domain ${step.domain}`)
      : [];
    if (unsupported.length) throw new AutonomousMissionExecutionError(unsupported.join("; "));
    return preflightMission(mission, this.catalogue);
  }

  async start(mission: AgentMissionArgs, options: AutonomousMissionExecuteOptions = {}): Promise<AutonomousMissionExecutionResult> {
    if (!isObject(mission)) throw new ArgumentError("mission must be a JSON object");
    const normalizedOptions = normalizeMissionCostOptions(options);
    const preflight = await this.preflight(mission);
    const existing = await this.store.load(preflight.mission_id);
    if (!preflight.ok) return this.result("planned", preflight, existing, [], null, null);
    if (preflight.execution !== "authorized") return this.result("planned", preflight, existing, [], null, null);
    const routeResolution = existing
      ? await this.resolveExistingRoute(mission, existing, normalizedOptions)
      : await this.resolveStartRoute(mission, normalizedOptions);
    const route = routeResolution.route;
    if (routeResolution.semantic_status !== null && routeResolution.semantic_status !== "completed") {
      const status = routeResolution.semantic_status === "policy_blocked" ? "policy_blocked" : routeResolution.semantic_status === "policy_review_required" ? "policy_review_required" : "route_review_required";
      return this.result(status, preflight, existing, [], route, routeResolution.semantic_status);
    }
    const policy = policyOf(mission);
    if (policy.execute !== true) return this.result("planned", preflight, existing, [], route, routeResolution.semantic_status);
    if (normalizedOptions.approveProviderCall !== undefined && normalizedOptions.approveProviderCall !== true) return this.result("approval_required", preflight, existing, [], route, routeResolution.semantic_status);
    const requestDigest = preflight.request_digest || await digestJson(mission);
    const policyDigest = await digestJson(policy);
    const orderedSteps = preflight.ordered_steps;
    const waves = preflight.waves;
    let checkpoint = existing
      ? await this.assertExisting(existing, requestDigest, policyDigest, preflight.catalogue_digest, orderedSteps, waves, route?.route_digest ?? null)
      : await this.makeCheckpoint(preflight.mission_id, requestDigest, policyDigest, preflight.catalogue_digest, orderedSteps, waves, Object.fromEntries(orderedSteps.map((id) => [id, stepState()])), [], 0, 0, "running", null, route?.route_digest ?? null);
    if (!existing) {
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint, "mission.started", null, null, null, "mission execution started");
    } else if (normalizedOptions.retryBlocked === true) {
      checkpoint = await this.resetQualityBlocked(checkpoint, mission);
    }
    return this.drive(mission, preflight, checkpoint, normalizedOptions, route, routeResolution.semantic_status);
  }

  async resume(mission: AgentMissionArgs, options: AutonomousMissionExecuteOptions = {}): Promise<AutonomousMissionExecutionResult> {
    return this.start(mission, { ...options, approveProviderCall: options.approveProviderCall ?? true });
  }

  async events(missionId: string, after = 0, limit = AUTONOMOUS_MISSION_MAX_EVENTS): Promise<AutonomousMissionEvent[]> {
    return this.store.events(boundedIdentifier("mission_id", missionId), after, limit);
  }

  private async resolveStartRoute(mission: AgentMissionArgs, options: AutonomousMissionExecuteOptions): Promise<MissionRouteResolution> {
    if (options.routeOverride) {
      const route = await validateAutonomousRouteOverride(mission.goal, options.routeOverride);
      if (!routeMatchesMission(route, mission)) throw new AutonomousMissionExecutionError("mission route override does not exactly cover the mission step domains");
      return { route, semantic_status: null };
    }
    if (options.semanticRouting?.enabled !== true) return { route: null, semantic_status: null };
    if (!this.agent || typeof this.agent.route !== "function" || !this.agent.runtime) throw new AutonomousMissionExecutionError("semantic mission routing requires an AutonomousAgent on the mission executor");
    const domains = missionDomains(mission);
    const semantic = await semanticRouteAutonomousTask(this.agent, mission.goal, {
      candidates: options.semanticRouting.candidates,
      credential: options.semanticRouting.credential,
      credentialFor: options.semanticRouting.credentialFor,
      hints: options.semanticRouting.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains ?? Math.min(Math.max(domains.length, 1), 8),
      allowCrossDomain: options.semanticRouting.allowCrossDomain ?? domains.length > 1,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.semanticRouting.maxCostPerMillionTokens,
      maxLatencyMs: options.semanticRouting.maxLatencyMs,
      minQuality: options.semanticRouting.minQuality,
      maxTotalCostUnits: options.costBudget ? undefined : options.maxTotalCostUnits,
      costBudget: options.costBudget,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers,
      domainPolicyMode: options.semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: options.semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: options.semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
      signal: options.signal,
    });
    if (semantic.status === "completed" && !routeMatchesMission(semantic.route, mission)) return { route: semantic.route, semantic_status: "provider_disagreement" };
    return { route: semantic.route, semantic_status: semantic.status };
  }

  private async resolveExistingRoute(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, options: AutonomousMissionExecuteOptions): Promise<MissionRouteResolution> {
    if (options.semanticRouting?.enabled === true && !options.routeOverride) throw new AutonomousMissionExecutionError("existing mission checkpoints require routeOverride to change provider-assisted routing; semantic routing is never replayed implicitly");
    if (options.routeOverride) {
      const route = await validateAutonomousRouteOverride(mission.goal, options.routeOverride);
      if (!routeMatchesMission(route, mission)) throw new AutonomousMissionExecutionError("mission route override does not exactly cover the mission step domains");
      if (checkpoint.route_digest !== route.route_digest) throw new AutonomousMissionExecutionError("mission route override does not match the persisted route digest");
      return { route, semantic_status: null };
    }
    if (checkpoint.route_digest !== undefined && checkpoint.route_digest !== null) throw new AutonomousMissionExecutionError("existing route-bound mission checkpoints require routeOverride for caller-owned route recovery");
    return { route: null, semantic_status: null };
  }

  private async assertExisting(checkpoint: AutonomousMissionCheckpoint, requestDigest: string, policyDigest: string, catalogueDigest: string, orderedSteps: readonly string[], waves: readonly string[][], routeDigest: string | null): Promise<AutonomousMissionCheckpoint> {
    const normalized = await validateCheckpoint(checkpoint);
    if (normalized.request_digest !== requestDigest || normalized.policy_digest !== policyDigest || normalized.catalogue_digest !== catalogueDigest || canonicalJson(normalized.ordered_steps) !== canonicalJson(orderedSteps) || canonicalJson(normalized.waves) !== canonicalJson(waves)) throw new AutonomousMissionExecutionError("mission checkpoint does not match the supplied mission contract");
    if (routeDigest !== null && normalized.route_digest !== routeDigest) throw new AutonomousMissionExecutionError("mission route override does not match the persisted route digest");
    return normalized;
  }

  private async makeCheckpoint(missionId: string, requestDigest: string, policyDigest: string, catalogueDigest: string, orderedSteps: readonly string[], waves: readonly string[][], states: Record<string, AutonomousMissionStepCheckpoint>, completed: readonly string[], nextWave: number | null, outputBytes: number, status: AutonomousMissionStatus, previous: AutonomousMissionCheckpoint | null, routeDigest: string | null = previous?.route_digest ?? null): Promise<AutonomousMissionCheckpoint> {
    const descriptor = { schema: AUTONOMOUS_MISSION_CHECKPOINT_SCHEMA, mission_id: missionId, request_digest: requestDigest, policy_digest: policyDigest, catalogue_digest: catalogueDigest, route_digest: routeDigest, ordered_steps: [...orderedSteps], waves: waves.map((wave) => [...wave]), step_states: states, completed_step_ids: [...completed], next_wave: nextWave, output_bytes: outputBytes, generation: (previous?.generation ?? 0) + 1, status, previous_checkpoint_digest: previous?.checkpoint_digest ?? null, retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material" as const, secret_material: "never_returned" as const };
    return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
  }

  private async resetQualityBlocked(checkpoint: AutonomousMissionCheckpoint, mission: AgentMissionArgs): Promise<AutonomousMissionCheckpoint> {
    const states = clone(checkpoint.step_states);
    let reset = false;
    for (const step of mission.steps) {
      const state = states[step.id];
      if (state?.status !== "blocked" || (state.quality === null || state.quality === undefined) && state.error_class !== "QualityEvaluatorError") continue;
      states[step.id] = { ...stepState("pending"), attempt: state.attempt, last_event_sequence: state.last_event_sequence };
      reset = true;
    }
    if (!reset) return checkpoint;
    const completed = checkpoint.completed_step_ids.filter((id) => states[id]?.status === "succeeded");
    const nextWave = this.nextPendingWave({ ...checkpoint, step_states: states, completed_step_ids: completed });
    const retried = await this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, states, completed, nextWave, checkpoint.output_bytes, "running", checkpoint);
    await this.store.save(retried);
    await this.appendEvent(retried, "checkpointed", nextWave, null, "running", "quality-gated steps reset for explicit retry");
    return retried;
  }

  private async appendEvent(checkpoint: AutonomousMissionCheckpoint, eventType: AutonomousMissionEventType, wave: number | null, step: AgentMissionStep | null, status: string | null, detail: string | null, outputBytes = 0, argumentsDigest: string | null = null): Promise<AutonomousMissionEvent> {
    const operation = this.eventQueue.then(async () => {
      const prior = await this.store.events(checkpoint.mission_id, 0, AUTONOMOUS_MISSION_MAX_EVENTS);
      const descriptor = { schema: AUTONOMOUS_MISSION_EVENT_SCHEMA, sequence: (prior.at(-1)?.sequence ?? 0) + 1, mission_id: checkpoint.mission_id, event_type: eventType, wave, step_id: step?.id ?? null, tool: step?.tool ?? null, status, arguments_digest: argumentsDigest, output_bytes: outputBytes, detail: boundedDetail(detail), checkpoint_digest: checkpoint.checkpoint_digest, previous_event_digest: prior.at(-1)?.event_digest ?? null, retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material" as const, secret_material: "never_returned" as const };
      const event = { ...descriptor, event_digest: await digestJson(descriptor) };
      await this.store.appendEvent(event);
      return event;
    });
    this.eventQueue = operation.then(() => undefined, () => undefined);
    return operation;
  }

  private async drive(mission: AgentMissionArgs, preflight: MissionPreflightResult, initial: AutonomousMissionCheckpoint, options: AutonomousMissionExecuteOptions, route: AutonomousRouteProposal | null, semanticRouteStatus: AutonomousMissionSemanticRouteStatus | null): Promise<AutonomousMissionExecutionResult> {
    const policy = policyOf(mission);
    const stepsById = new Map(mission.steps.map((step) => [step.id, step]));
    const maxWaves = options.max_waves === undefined ? AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL : boundedInteger("max_waves", options.max_waves, AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL, 1);
    const attempt = options.execution_attempt === undefined ? 1 : boundedInteger("execution_attempt", options.execution_attempt, AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL, 1);
    let checkpoint = initial;
    const localResults: AutonomousMissionStepResult[] = [];
    let wavesConsumed = 0;
    while (checkpoint.next_wave !== null && wavesConsumed < maxWaves) {
      const waveIndex = checkpoint.next_wave;
      const waveIds = checkpoint.waves[waveIndex];
      if (!waveIds) throw new AutonomousMissionExecutionError(`mission checkpoint references unknown wave ${waveIndex}`);
      if (options.signal?.aborted) {
        checkpoint = await this.transitionPending(mission, checkpoint, "cancelled", "operator cancellation");
        await this.appendEvent(checkpoint, "mission.cancelled", waveIndex, null, "cancelled", "mission signal was aborted");
        break;
      }
      checkpoint = await this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, checkpoint.step_states, checkpoint.completed_step_ids, checkpoint.next_wave, checkpoint.output_bytes, "running", checkpoint);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint, "wave.started", waveIndex, null, "running", `wave ${waveIndex} started`);
      const eligible: AgentMissionStep[] = [];
      for (const id of waveIds) {
        const step = stepsById.get(id);
        if (!step) throw new AutonomousMissionExecutionError(`mission step ${id} is missing from the supplied mission`);
        const state = checkpoint.step_states[id] ?? stepState();
        if (isTerminalStep(state.status)) continue;
        const dependencies = step.depends_on ?? [];
        const dependencyStates = dependencies.map((dependency) => checkpoint.step_states[dependency]?.status ?? "pending");
        const failedDependency = dependencyStates.some((status) => status !== "succeeded");
        if (failedDependency) {
          checkpoint = await this.setStepState(checkpoint, step, "blocked", null, 0, "dependency_not_succeeded", null, attempt);
          await this.store.save(checkpoint);
          localResults.push(this.localResult(step, checkpoint.step_states[id] as AutonomousMissionStepCheckpoint, null));
          continue;
        }
        eligible.push(step);
      }
      if (eligible.length) {
        const completed = await this.executeWave(mission, checkpoint, eligible, waveIndex, policy, options, attempt, localResults);
        checkpoint = completed.checkpoint;
        await this.store.save(checkpoint);
      }
      const requiredFailureInWave = waveIds.some((id) => {
        const step = stepsById.get(id) as AgentMissionStep;
        const status = checkpoint.step_states[id]?.status ?? "pending";
        return requiredFailure(step, status) && status !== "approval_required" && status !== "reconciliation_required" && status !== "recovery_required";
      });
      const retryRequiredInWave = waveIds.some((id) => ["approval_required", "reconciliation_required", "recovery_required"].includes(checkpoint.step_states[id]?.status ?? "pending"));
      const qualityFailureInWave = waveIds.some((id) => checkpoint.step_states[id]?.status === "blocked" && (checkpoint.step_states[id]?.quality !== null && checkpoint.step_states[id]?.quality !== undefined || checkpoint.step_states[id]?.error_class === "QualityEvaluatorError"));
      wavesConsumed += 1;
      // A quality rejection keeps the current wave addressable for an explicit retry.  Leaving
      // dependents pending is important: they must not be marked as ordinary stop-on-error
      // blocks before the caller has had a chance to repair or review the result.
      const nextWave = retryRequiredInWave ? waveIndex : qualityFailureInWave ? waveIndex : requiredFailureInWave && policy.stop_on_error !== false ? null : this.nextPendingWave(checkpoint);
      const statusAfterWave = retryRequiredInWave ? missionStatus({ ...checkpoint, next_wave: nextWave }, mission.steps) : qualityFailureInWave ? "failed" : nextWave === null ? missionStatus({ ...checkpoint, next_wave: null }, mission.steps) : "running";
      checkpoint = await this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, checkpoint.step_states, checkpoint.completed_step_ids, nextWave, checkpoint.output_bytes, statusAfterWave, checkpoint);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint, "wave.completed", waveIndex, null, statusAfterWave, `wave ${waveIndex} completed`);
      if (retryRequiredInWave) break;
      if (qualityFailureInWave) break;
      if (requiredFailureInWave && policy.stop_on_error !== false && nextWave === null) {
        checkpoint = await this.blockRemaining(mission, checkpoint, "stop_on_error halted later waves");
        break;
      }
    }
    if (checkpoint.next_wave !== null && wavesConsumed >= maxWaves && checkpoint.status === "running") {
      checkpoint = await this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, checkpoint.step_states, checkpoint.completed_step_ids, checkpoint.next_wave, checkpoint.output_bytes, "running", checkpoint);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint, "checkpointed", checkpoint.next_wave, null, "running", "bounded continuation checkpoint");
    } else if (checkpoint.next_wave === null && !["cancelled", "approval_required", "reconciliation_required", "recovery_required"].includes(checkpoint.status)) {
      const finalStatus = missionStatus(checkpoint, mission.steps);
      if (checkpoint.status !== finalStatus) {
        checkpoint = await this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, checkpoint.step_states, checkpoint.completed_step_ids, null, checkpoint.output_bytes, finalStatus, checkpoint);
        await this.store.save(checkpoint);
      }
      const priorEvents = await this.store.events(checkpoint.mission_id, 0, AUTONOMOUS_MISSION_MAX_EVENTS);
      if (priorEvents.at(-1)?.event_type !== "mission.completed") await this.appendEvent(checkpoint, "mission.completed", null, null, finalStatus, "mission reached a terminal state", checkpoint.output_bytes);
    }
    return this.result(checkpoint.status, preflight, checkpoint, localResults, route, semanticRouteStatus);
  }

  private async executeWave(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, steps: readonly AgentMissionStep[], wave: number, policy: AgentMissionPolicy, options: AutonomousMissionExecuteOptions, attempt: number, localResults: AutonomousMissionStepResult[]): Promise<{ checkpoint: AutonomousMissionCheckpoint }> {
    const maxParallelism = policy.execution_mode === "parallel_waves" ? Math.max(1, Math.min(Number.isSafeInteger(policy.max_parallelism) ? policy.max_parallelism as number : 1, 16)) : 1;
    const base = checkpoint;
    let nextIndex = 0;
    let haltDispatch = false;
    const executions: Array<{ step: AgentMissionStep; result: AutonomousMissionStepResult; checkpoint: AutonomousMissionCheckpoint }> = [];
    const worker = async (): Promise<void> => {
      while (true) {
        if (haltDispatch) return;
        const index = nextIndex;
        nextIndex += 1;
        const step = steps[index];
        if (!step) return;
        const result = await this.executeOne(mission, base, step, wave, options, attempt);
        executions.push({ step, result: result.result, checkpoint: result.checkpoint });
        if (result.result.status === "approval_required" || result.result.status === "reconciliation_required" || result.result.status === "recovery_required") haltDispatch = true;
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, steps.length) }, () => worker()));
    // Every worker starts from the same immutable wave checkpoint. Merge by declaration order so
    // completion timing cannot cause one parallel step to erase another step's state or change
    // the digest of the durable continuation.
    let current = base;
    for (const step of steps) {
      const execution = executions.find((candidate) => candidate.step.id === step.id);
      if (!execution) continue;
      const candidateState = execution.checkpoint.step_states[step.id] as AutonomousMissionStepCheckpoint;
      const outputDelta = candidateState.status === "succeeded" ? candidateState.output_bytes : 0;
      const states = clone(current.step_states);
      states[step.id] = clone(candidateState);
      const completed = [...current.completed_step_ids, ...(candidateState.status === "succeeded" && !current.completed_step_ids.includes(step.id) ? [step.id] : [])];
      const mergedStatus = candidateState.status === "reconciliation_required" ? "reconciliation_required" : candidateState.status === "recovery_required" ? "recovery_required" : candidateState.status === "approval_required" ? "approval_required" : candidateState.status === "blocked" && (candidateState.quality !== null && candidateState.quality !== undefined || candidateState.error_class === "QualityEvaluatorError") ? "failed" : "running";
      current = await this.makeCheckpoint(current.mission_id, current.request_digest, current.policy_digest, current.catalogue_digest, current.ordered_steps, current.waves, states, completed, this.nextPendingWave({ ...current, step_states: states, completed_step_ids: completed }), current.output_bytes + outputDelta, mergedStatus, current);
      await this.store.save(current);
      localResults.push(execution.result);
      if (this.onStepOutcome) await this.onStepOutcome(execution.result, { mission_id: mission.mission_id, wave });
    }
    return { checkpoint: current };
  }

  private async executeOne(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, step: AgentMissionStep, wave: number, options: AutonomousMissionExecuteOptions, attempt: number): Promise<{ checkpoint: AutonomousMissionCheckpoint; result: AutonomousMissionStepResult }> {
    let args: JsonObject = clone(step.arguments ?? {});
    const dependencyOutputs: Record<string, JsonValue> = {};
    for (const binding of step.bindings ?? []) {
      const dependencyState = checkpoint.step_states[binding.from_step];
      if (!dependencyState?.result_digest) return this.markTransient(mission, checkpoint, step, "recovery_required", "dependency result is not available", attempt, wave);
      const dependencyOutput = await this.resultStore.load(mission.mission_id, binding.from_step, dependencyState.result_digest);
      if (dependencyOutput === null) return this.markTransient(mission, checkpoint, step, "recovery_required", "caller-owned dependency result must be rehydrated", attempt, wave);
      const selected = pointerGet(dependencyOutput, binding.source_pointer);
      if (selected === undefined) return this.markTransient(mission, checkpoint, step, "failed", `binding source pointer is unavailable: ${binding.source_pointer}`, attempt, wave);
      dependencyOutputs[binding.from_step] = dependencyOutput;
      args = pointerSet(args, binding.target_pointer, selected);
    }
    const argumentsDigest = await digestJson(args);
    const started = await this.setStepState(checkpoint, step, "running", null, 0, null, null, attempt);
    await this.appendEvent(started, "step.started", wave, step, "running", "step dispatch started", 0, argumentsDigest);
    let execution: AutonomousMissionStepExecutionResult;
    try {
      execution = normalizeStepResult(await this.executeStep({ mission_id: mission.mission_id, goal: mission.goal, wave, step: clone(step), arguments: args, dependency_outputs: dependencyOutputs, execution_attempt: attempt, resumed: checkpoint.step_states[step.id]?.status !== "pending", cost_budget: options.costBudget, signal: options.signal }));
    } catch (error) {
      execution = { status: error instanceof AutonomousEffectReconciliationRequiredError ? "reconciliation_required" : "failed", error_class: safeFailureClass(error), detail: null };
    }
    const status = classifyStatus(execution);
    if (status !== "succeeded") {
      const eventType: AutonomousMissionEventType = status === "approval_required" ? "approval.required" : status === "reconciliation_required" ? "reconciliation.required" : status === "refused" ? "step.refused" : "step.failed";
      const transitioned = await this.setStepState(started, step, status, execution.error_class ?? null, 0, execution.detail ?? null, execution.run_status ?? null, attempt);
      await this.appendEvent(transitioned, eventType, wave, step, status, execution.detail ?? execution.error_class ?? "step did not complete", 0, argumentsDigest);
      return { checkpoint: transitioned, result: this.localResult(step, transitioned.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
    }
    if (execution.value === undefined) execution.value = null;
    const outputBytes = jsonBytes(execution.value);
    const maxStepBytes = Number.isSafeInteger(policyOf(mission).max_step_output_bytes) ? policyOf(mission).max_step_output_bytes as number : 2_000_000;
    if (outputBytes > Math.min(maxStepBytes, AUTONOMOUS_MISSION_MAX_RESULT_BYTES)) {
      const failed = await this.setStepState(started, step, "failed", "StepOutputBudgetExceeded", outputBytes, "step output budget exceeded", null, attempt);
      await this.appendEvent(failed, "step.failed", wave, step, "failed", "step output budget exceeded", outputBytes, argumentsDigest);
      return { checkpoint: failed, result: this.localResult(step, failed.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
    }
    const currentTotal = checkpoint.output_bytes + outputBytes;
    const maxTotal = Number.isSafeInteger(policyOf(mission).max_total_output_bytes) ? policyOf(mission).max_total_output_bytes as number : 10_000_000;
    if (currentTotal > maxTotal) {
      const failed = await this.setStepState(started, step, "failed", "OutputBudgetExceeded", outputBytes, "mission output budget exceeded", null, attempt);
      await this.appendEvent(failed, "step.failed", wave, step, "failed", "mission output budget exceeded", outputBytes, argumentsDigest);
      return { checkpoint: failed, result: this.localResult(step, failed.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
    }
    const resultDigest = await digestJson(execution.value);
    let quality: AutonomousMissionStepQualityEvaluation | null = null;
    if (this.evaluateStep) {
      try {
        const goalDigest = await digestJson({ goal: mission.goal });
        quality = await normalizeMissionStepQuality(await this.evaluateStep({
          mission_id: mission.mission_id,
          goal_digest: goalDigest,
          wave,
          attempt,
          step: clone(step),
          result: clone(execution.value),
          result_digest: resultDigest,
        }), {
          mission_id: mission.mission_id,
          goal_digest: goalDigest,
          step: clone(step),
          result_digest: resultDigest,
        });
      } catch (error) {
        const blocked = await this.setStepState(started, step, "blocked", "QualityEvaluatorError", 0, error instanceof Error ? error.message : "mission step quality evaluator failed", execution.run_status ?? null, attempt, null, checkpoint.output_bytes, null, execution.decision ?? null);
        await this.appendEvent(blocked, "step.blocked", wave, step, "blocked", error instanceof Error ? error.message : "mission step quality evaluator failed", 0, argumentsDigest);
        return { checkpoint: blocked, result: this.localResult(step, blocked.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
      }
      if (!quality.passed) {
        const blocked = await this.setStepState(started, step, "blocked", quality.failure_class ?? "MissionStepQualityGateRejected", 0, "mission step quality gate rejected the result", execution.run_status ?? null, attempt, null, checkpoint.output_bytes, null, execution.decision ?? null, quality);
        await this.appendEvent(blocked, "step.blocked", wave, step, "blocked", quality.failure_class ?? "mission step quality gate rejected the result", 0, argumentsDigest);
        return { checkpoint: blocked, result: this.localResult(step, blocked.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
      }
    }
    await this.resultStore.save(mission.mission_id, step.id, execution.value, resultDigest);
    const completed = await this.setStepState(started, step, "succeeded", null, outputBytes, null, execution.run_status ?? null, attempt, resultDigest, currentTotal, execution.learning_episode_id ?? null, execution.decision ?? null, quality);
    await this.appendEvent(completed, "step.completed", wave, step, "succeeded", null, outputBytes, argumentsDigest);
    return { checkpoint: completed, result: this.localResult(step, completed.step_states[step.id] as AutonomousMissionStepCheckpoint, execution.value) };
  }

  private async markTransient(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, step: AgentMissionStep, status: "recovery_required" | "failed", detail: string, attempt: number, wave: number): Promise<{ checkpoint: AutonomousMissionCheckpoint; result: AutonomousMissionStepResult }> {
    const updated = await this.setStepState(checkpoint, step, status, status === "recovery_required" ? "ResultRehydrationRequired" : "BindingResolutionError", 0, detail, null, attempt);
    await this.appendEvent(updated, status === "recovery_required" ? "checkpointed" : "step.failed", wave, step, status, detail);
    return { checkpoint: updated, result: this.localResult(step, updated.step_states[step.id] as AutonomousMissionStepCheckpoint, null) };
  }

  private async setStepState(checkpoint: AutonomousMissionCheckpoint, step: AgentMissionStep, status: AutonomousMissionStepStatus, errorClass: string | null, outputBytes: number, detail: string | null, runStatus: string | null, attempt: number, resultDigest: string | null = null, totalOutputBytes = checkpoint.output_bytes, learningEpisodeId: string | null = null, decision: AutonomousMissionStepDecision | null = null, quality: AutonomousMissionStepQualityEvaluation | null = null): Promise<AutonomousMissionCheckpoint> {
    const states = clone(checkpoint.step_states);
    const previous = states[step.id] ?? stepState();
    states[step.id] = { status, result_digest: resultDigest ?? previous.result_digest, output_bytes: outputBytes, error_class: errorClass, run_status: runStatus, learning_episode_id: learningEpisodeId ?? previous.learning_episode_id, decision: decision ?? previous.decision, quality: quality ?? (status === "running" ? null : previous.quality ?? null), attempt, last_event_sequence: previous.last_event_sequence };
    const completed = Object.entries(states).filter(([, state]) => state.status === "succeeded").map(([id]) => id).filter((id) => !checkpoint.completed_step_ids.includes(id));
    const next = [...checkpoint.completed_step_ids, ...completed];
    const nextWave = this.nextPendingWave({ ...checkpoint, step_states: states, completed_step_ids: next });
    return this.makeCheckpoint(checkpoint.mission_id, checkpoint.request_digest, checkpoint.policy_digest, checkpoint.catalogue_digest, checkpoint.ordered_steps, checkpoint.waves, states, next, nextWave, totalOutputBytes, status === "reconciliation_required" ? "reconciliation_required" : status === "recovery_required" ? "recovery_required" : status === "approval_required" ? "approval_required" : "running", checkpoint);
  }

  private nextPendingWave(checkpoint: AutonomousMissionCheckpoint): number | null {
    for (let index = 0; index < checkpoint.waves.length; index += 1) if (checkpoint.waves[index]?.some((id) => !isTerminalStep(checkpoint.step_states[id]?.status ?? "pending"))) return index;
    return null;
  }

  private async transitionPending(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, status: "cancelled", detail: string): Promise<AutonomousMissionCheckpoint> {
    let current = checkpoint;
    for (const step of mission.steps) if (!isTerminalStep(current.step_states[step.id]?.status ?? "pending")) {
      current = await this.setStepState(current, step, status, "Cancelled", 0, detail, null, 0);
      await this.store.save(current);
    }
    const final = await this.makeCheckpoint(current.mission_id, current.request_digest, current.policy_digest, current.catalogue_digest, current.ordered_steps, current.waves, current.step_states, current.completed_step_ids, null, current.output_bytes, "cancelled", current);
    await this.store.save(final);
    return final;
  }

  private async blockRemaining(mission: AgentMissionArgs, checkpoint: AutonomousMissionCheckpoint, detail: string): Promise<AutonomousMissionCheckpoint> {
    let current = checkpoint;
    for (const step of mission.steps) if (!isTerminalStep(current.step_states[step.id]?.status ?? "pending")) {
      current = await this.setStepState(current, step, "blocked", "StopOnError", 0, detail, null, 0);
      await this.store.save(current);
    }
    const final = await this.makeCheckpoint(current.mission_id, current.request_digest, current.policy_digest, current.catalogue_digest, current.ordered_steps, current.waves, current.step_states, current.completed_step_ids, null, current.output_bytes, missionStatus({ ...current, next_wave: null }, mission.steps), current);
    await this.store.save(final);
    return final;
  }

  private localResult(step: AgentMissionStep, state: AutonomousMissionStepCheckpoint, value: JsonValue | null): AutonomousMissionStepResult {
    return { step: clone(step), status: state.status, value: value === null ? null : clone(value), result_digest: state.result_digest, output_bytes: state.output_bytes, error_class: state.error_class, run_status: state.run_status, learning_episode_id: state.learning_episode_id, decision: state.decision, quality: state.quality ?? null, attempt: state.attempt };
  }

  private async result(status: AutonomousMissionStatus, preflight: MissionPreflightResult, checkpoint: AutonomousMissionCheckpoint | null, localResults: AutonomousMissionStepResult[], route: AutonomousRouteProposal | null = null, semanticRouteStatus: AutonomousMissionSemanticRouteStatus | null = null): Promise<AutonomousMissionExecutionResult> {
    const events = checkpoint ? await this.store.events(preflight.mission_id, 0, AUTONOMOUS_MISSION_MAX_EVENTS) : [];
    const states = checkpoint ? Object.values(checkpoint.step_states) : [];
    return { schema: AUTONOMOUS_MISSION_EXECUTION_SCHEMA, status, mission_id: preflight.mission_id, preflight, checkpoint, route, semantic_route_status: semanticRouteStatus, events, results: localResults, completed_steps: checkpoint?.completed_step_ids.length ?? 0, total_steps: preflight.ordered_steps.length, succeeded_steps: states.filter((state) => state.status === "succeeded").length, refused_steps: states.filter((state) => state.status === "refused").length, blocked_steps: states.filter((state) => state.status === "blocked").length, failed_steps: states.filter((state) => state.status === "failed").length, cancelled_steps: states.filter((state) => state.status === "cancelled").length, returned_bytes: checkpoint?.output_bytes ?? 0, next_wave: checkpoint?.next_wave ?? null, recovery: "caller_rehydrates_raw_results_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only", secret_material: "never_returned" };
  }
}

/**
 * Prepare and settle exactly the successful learning episodes present in a mission checkpoint.
 * This helper deliberately accepts evaluator rewards from the caller; it never treats execution
 * success, provider confidence, or a tool response as a reward signal.
 */
export async function settleAutonomousMissionLearning(
  execution: AutonomousMissionExecutionResult,
  learning: AutonomousMissionLearningAdapter,
  options: {
    trajectoryId: string;
    discount?: number;
    rewards: Record<string, AutonomousEvaluatorRewardInput>;
    remote?: boolean;
    outbox?: AutonomousLearningOutboxSettlementOptions;
  },
): Promise<AutonomousMissionLearningSettlement> {
  if (!execution || !isObject(execution)) throw new ArgumentError("mission learning settlement requires an execution result");
  if (!learning || typeof learning.prepareTrajectory !== "function" || typeof learning.settleTrajectory !== "function") throw new ArgumentError("mission learning adapter is malformed");
  if (!options || !isObject(options)) throw new ArgumentError("mission learning settlement options are required");
  const trajectoryId = boundedIdentifier("mission trajectoryId", options.trajectoryId);
  if (!isObject(options.rewards)) throw new ArgumentError("mission learning rewards must be an object keyed by episode ID");
  const checkpoint = execution.checkpoint;
  const episodeIds: string[] = [];
  for (const stepId of checkpoint?.ordered_steps ?? []) {
    const state = checkpoint?.step_states[stepId];
    if (state?.status === "succeeded" && state.learning_episode_id !== null) {
      const episodeId = boundedIdentifier(`mission ${stepId}.learning_episode_id`, state.learning_episode_id);
      if (!episodeIds.includes(episodeId)) episodeIds.push(episodeId);
    }
  }
  for (const result of execution.results) {
    if (result.status === "succeeded" && result.learning_episode_id !== null) {
      const episodeId = boundedIdentifier(`mission ${result.step.id}.learning_episode_id`, result.learning_episode_id);
      if (!episodeIds.includes(episodeId)) episodeIds.push(episodeId);
    }
  }
  if (!episodeIds.length) throw new ArgumentError("mission execution has no successful learning episodes");
  const trajectory = await learning.prepareTrajectory(episodeIds, { trajectoryId, discount: options.discount });
  if (trajectory.trajectory_id !== trajectoryId) throw new AutonomousMissionExecutionError("learning adapter returned a mismatched trajectory identity");
  const trajectoryEpisodeIds = trajectory.steps.map((step) => boundedIdentifier("trajectory episodeId", step.episode_id));
  if (trajectoryEpisodeIds.length !== episodeIds.length || trajectoryEpisodeIds.some((id, index) => id !== episodeIds[index])) throw new AutonomousMissionExecutionError("learning adapter changed mission episode order");
  const rewardKeys = Object.keys(options.rewards);
  if (rewardKeys.length !== episodeIds.length || rewardKeys.some((id) => !episodeIds.includes(id))) throw new ArgumentError("mission learning rewards must cover exactly every successful learning episode");
  const settlement = await learning.settleTrajectory(trajectoryId, options.rewards, { remote: options.remote, outbox: options.outbox });
  return {
    schema: "bioprism-typescript-autonomous-mission-learning-settlement/0.1",
    mission_id: boundedIdentifier("mission_id", execution.mission_id),
    trajectory_id: trajectoryId,
    episode_ids: [...episodeIds],
    settlement,
    retention: "value_only_learning_projection",
    secret_material: "never_returned",
  };
}

/**
 * Adapter that lets the full autonomous brain execute an explicit mission step. The provider is
 * still responsible for model selection and tool invocation, but the adapter rejects any tool
 * other than the step's exact tool and rejects argument drift from the resolved dependency graph.
 */
export function agentMissionStepExecutor(agent: AutonomousAgent, options: {
  toolsForStep?: (step: AgentMissionStep) => readonly ProviderTool[] | undefined;
  run?: Omit<AutonomousRunOptions, "domain" | "capability" | "tools" | "authorizeAndExecute" | "context" | "approveProviderCall" | "signal">;
  approveEffects?: boolean;
  signal?: AbortSignal;
  /** Receives only the validated prompt-arm receipt, never the provider response or rendered prompt. */
  onPromptSelection?: (selection: AutonomousPromptAdaptiveSelectionJSON, context: AutonomousMissionStepExecutionContext) => Promise<void> | void;
  learning?: {
    adapter: AutonomousMissionLearningAdapter;
    episodeId?: (context: AutonomousMissionStepExecutionContext) => string;
    runId?: (context: AutonomousMissionStepExecutionContext) => string;
    planRefinementDigest?: string | null;
  };
} = {}): AutonomousMissionStepExecutor {
  if (!agent || typeof agent.run !== "function" || typeof agent.executeToolCalls !== "function") throw new ArgumentError("agent mission adapter requires an AutonomousAgent");
  return async (context) => {
    const expectedDigest = await digestJson(context.arguments);
    let sawExpectedCall = false;
    let toolOutput: JsonValue | null = null;
    const authorizeAndExecute = async (calls: ProviderToolCall[]): Promise<ProviderToolResult[]> => {
      if (calls.length !== 1 || calls[0]?.name !== context.step.tool || await digestJson(calls[0]?.arguments ?? {}) !== expectedDigest) {
        return calls.map((call) => ({ callId: call.id, approved: false, isError: true, content: { status: "mission_tool_contract_violation", expected_tool: context.step.tool, expected_arguments_digest: expectedDigest, received_tool: call.name, secret_material: "never_returned" } }));
      }
      sawExpectedCall = true;
      const results = await agent.executeToolCalls(calls, { domains: [context.step.domain], approveEffects: options.approveEffects, effectBoundary: (options.run as { effectBoundary?: AutonomousEffectBoundary } | undefined)?.effectBoundary });
      const result = results[0];
      if (result?.approved === true && !result.isError) toolOutput = result.content as JsonValue;
      return results;
    };
    const run = await agent.run(context.step.objective, {
      ...options.run,
      ...(context.cost_budget === undefined ? {} : { costBudget: context.cost_budget, maxTotalCostUnits: undefined }),
      domain: context.step.domain as AutonomousDomainName,
      capability: context.step.capability,
      context: [{ id: "mission-step-contract", content: JSON.stringify({ mission_id: context.mission_id, step_id: context.step.id, tool: context.step.tool, arguments_digest: expectedDigest, wave: context.wave }), required: true, priority: 100 }],
      tools: options.toolsForStep?.(context.step),
      authorizeAndExecute,
      approveProviderCall: true,
      approveEffects: options.approveEffects,
      signal: options.signal ?? context.signal,
    });
    if (run.prompt?.adaptive_selection !== undefined && run.prompt.adaptive_selection !== null) {
      await options.onPromptSelection?.(structuredClone(run.prompt.adaptive_selection), context);
    }
    if (run.status === "reconciliation_required" || run.tool_loop?.status === "reconciliation_required") return { status: "reconciliation_required", run_status: run.status };
    if (run.status === "approval_required" || run.tool_loop?.status === "authorization_required") return { status: "approval_required", run_status: run.status };
    if (!sawExpectedCall) return { status: "refused", run_status: run.status, detail: "provider did not invoke the mission step's exact tool contract" };
    if (run.status !== "completed") return { status: "failed", run_status: run.status, detail: "provider run did not complete" };
    const decision: AutonomousMissionStepDecision = {
      selection_digest: run.selection ? await digestJson(run.selection) : null,
      provider: safeLabel(run.selection?.selected_model?.provider, "unknown"),
      model: safeLabel(run.selection?.selected_model?.model, "unknown"),
      route_digest: typeof run.route?.route_digest === "string" ? boundedDigest("mission route_digest", run.route.route_digest) : null,
      plan_digest: typeof run.blueprint?.plan?.plan_digest === "string" ? boundedDigest("mission plan_digest", run.blueprint.plan.plan_digest) : null,
      prompt_digest: typeof run.blueprint?.prompt?.prompt_digest === "string" ? boundedDigest("mission prompt_digest", run.blueprint.prompt.prompt_digest) : null,
      ...(run.prompt?.adaptive_selection === undefined || run.prompt.adaptive_selection === null
        ? {}
        : { adaptive_selection: structuredClone(run.prompt.adaptive_selection) }),
    };
    let learningEpisodeId: string | null = null;
    if (options.learning) {
      if (!options.learning.adapter || typeof options.learning.adapter.prepareRun !== "function") throw new ArgumentError("mission learning adapter is malformed");
      const episodeId = boundedIdentifier("mission learning episodeId", options.learning.episodeId?.(context) ?? `mission:${context.mission_id}:${context.step.id}`);
      const episode = await options.learning.adapter.prepareRun(run, {
        episodeId,
        runId: options.learning.runId?.(context) ?? episodeId,
        stageId: context.step.id,
        parentJobId: context.mission_id,
        planRefinementDigest: options.learning.planRefinementDigest,
      });
      if (!episode || boundedIdentifier("prepared learning episodeId", episode.episode_id) !== episodeId) throw new AutonomousMissionExecutionError("learning adapter returned a mismatched episode identity");
      learningEpisodeId = episodeId;
    }
    return { status: "succeeded", value: toolOutput ?? run.response?.structured ?? run.response?.text ?? null, run_status: run.status, learning_episode_id: learningEpisodeId, decision };
  };
}
