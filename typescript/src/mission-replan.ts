import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_MISSION_STATUSES,
  AutonomousMissionExecutor,
  settleAutonomousMissionLearning,
  type AutonomousMissionExecuteOptions,
  type AutonomousMissionExecutionResult,
  type AutonomousMissionLearningAdapter,
  type AutonomousMissionLearningSettlement,
  type AutonomousMissionStatus,
} from "./mission-execution.js";
import { AutonomousCostBudget, type AutonomousCostBudgetSnapshot } from "./llm.js";
import type { AutonomousRouteProposal } from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { AgentMissionArgs, AgentMissionStep, JsonObject } from "./types.js";

/** Durable mission-level evaluator/replanning metadata. Raw evaluator instructions stay transient. */
export const AUTONOMOUS_MISSION_REPLAN_SCHEMA = "bioprism-typescript-autonomous-mission-replan/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-mission-replan-checkpoint/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA = "bioprism-typescript-autonomous-mission-replan-state/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-mission-replan-snapshot/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS = 3;
export const AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES = 8_192;
export const AUTONOMOUS_MISSION_REPLAN_MAX_STATES = 4_096;
export const AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS = AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS + 1;
export const AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES = 64_000_000;

export type AutonomousMissionReplanStatus =
  | "completed"
  | "completed_without_replan"
  | "replan_limit_reached"
  | AutonomousMissionStatus;

export class AutonomousMissionReplanError extends ArgumentError {
  override readonly name: string = "AutonomousMissionReplanError";
}

export class AutonomousMissionReplanContractError extends AutonomousMissionReplanError {
  override readonly name: string = "AutonomousMissionReplanContractError";
}

export interface AutonomousMissionReplanEvaluation extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  replan_requested: boolean;
  replan_instruction?: string | null;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
  /** Exact per-episode rewards; these are required when mission steps emit learning IDs. */
  rewards?: Record<string, AutonomousEvaluatorRewardInput>;
}

export interface AutonomousMissionReplanEvaluationProjection extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  replan_requested: boolean;
  replan_instruction_digest: string | null;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
  rewards_digest: string;
  evaluation_digest: string;
  retention: "evaluator_values_and_digests_only";
  secret_material: "never_returned";
}

export interface AutonomousMissionReplanAttempt extends JsonObject {
  attempt: number;
  mission_id: string;
  status: AutonomousMissionStatus;
  next_wave: number | null;
  completed_steps: number;
  succeeded_steps: number;
  failed_steps: number;
  evaluation_digest: string | null;
  learning_trajectory_id: string | null;
  replan_instruction_digest: string | null;
  route_digest?: string | null;
}

export interface AutonomousMissionReplanCheckpoint extends JsonObject {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA;
  root_mission_id: string;
  attempt: number;
  mission_id: string;
  protected_contract_digest: string;
  mission_request_digest: string | null;
  phase: "execution_pending" | "evaluation_recorded" | "replan_scheduled" | "terminal";
  mission_status: AutonomousMissionStatus;
  evaluation_digest: string | null;
  replan_instruction_digest: string | null;
  learning_trajectory_id: string | null;
  route_digest?: string | null;
  cost_budget?: AutonomousCostBudgetSnapshot | null;
  checkpoint_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
}

export interface AutonomousMissionReplanState {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA;
  root_mission_id: string;
  protected_contract_digest: string;
  max_replans: number;
  attempt: number;
  current_mission_id: string;
  mission_request_digest: string | null;
  phase: "execution_pending" | "evaluation_pending" | "replan_handoff" | "terminal";
  replan_instruction_digest: string | null;
  terminal_status: AutonomousMissionReplanStatus | null;
  attempts: AutonomousMissionReplanAttempt[];
  evaluations: AutonomousMissionReplanEvaluationProjection[];
  learning_settlements: AutonomousMissionLearningSettlement[];
  /** Digest of the approved route shared across all attempts; route material is caller-owned. */
  route_digest?: string | null;
  /** Metadata-only aggregate accounting; provider payloads and credentials are excluded. */
  cost_budget?: AutonomousCostBudgetSnapshot | null;
  last_mission_checkpoint_digest: string | null;
  generation: number;
  previous_state_digest: string | null;
  state_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_provider_material_or_raw_instructions";
  secret_material: "never_returned";
}

export interface AutonomousMissionReplanStateStore {
  load(rootMissionId: string): Promise<AutonomousMissionReplanState | null> | AutonomousMissionReplanState | null;
  save(state: AutonomousMissionReplanState): Promise<void> | void;
}

export interface AutonomousMissionReplanSnapshot {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA;
  states: AutonomousMissionReplanState[];
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousMissionReplanSnapshotStore extends AutonomousMissionReplanStateStore {
  snapshot(): Promise<AutonomousMissionReplanSnapshot>;
  restore(snapshot: AutonomousMissionReplanSnapshot): Promise<void> | void;
}

export interface AutonomousMissionReplanSnapshotPersistence {
  read(): Promise<AutonomousMissionReplanSnapshot | null> | AutonomousMissionReplanSnapshot | null;
  write(snapshot: AutonomousMissionReplanSnapshot): Promise<void> | void;
}

export interface AutonomousMissionReplanMissionRehydrator {
  (context: { root_mission_id: string; mission_id: string; attempt: number; protected_contract_digest: string }): Promise<AgentMissionArgs> | AgentMissionArgs;
}

export interface AutonomousMissionReplanInstructionRehydrator {
  (context: { root_mission_id: string; mission_id: string; attempt: number; instruction_digest: string; evaluation: AutonomousMissionReplanEvaluationProjection }): Promise<string> | string;
}

export interface AutonomousMissionReplanContext {
  mission: AgentMissionArgs;
  execution: AutonomousMissionExecutionResult;
  evaluation: AutonomousMissionReplanEvaluationProjection;
  /** Transient evaluator guidance; never included in returned projections or checkpoints. */
  instruction: string | null;
  attempt: number;
}

export type AutonomousMissionReplanEvaluator = (
  execution: AutonomousMissionExecutionResult,
) => Promise<AutonomousMissionReplanEvaluation> | AutonomousMissionReplanEvaluation;

export type AutonomousMissionReplanner = (
  context: AutonomousMissionReplanContext,
) => Promise<AgentMissionArgs> | AgentMissionArgs;

export interface AutonomousMissionReplanOptions {
  maxReplans?: number;
  evaluate: AutonomousMissionReplanEvaluator;
  /** Optional proposal callback. If omitted, bounded evaluator guidance is appended to objectives. */
  replan?: AutonomousMissionReplanner;
  learning?: {
    adapter: AutonomousMissionLearningAdapter;
    trajectoryIdPrefix?: string;
    discount?: number;
    remote?: boolean;
  };
  execute?: Omit<AutonomousMissionExecuteOptions, "signal" | "execution_attempt">;
  signal?: AbortSignal;
  /** Caller-owned durable handoff for metadata-only attempt checkpoints. */
  checkpointSink?: (checkpoint: AutonomousMissionReplanCheckpoint) => Promise<void> | void;
  /** Optional restart-safe orchestration state; no mission payloads or raw instructions are stored. */
  stateStore?: AutonomousMissionReplanStateStore;
  /** Rehydrate a non-root attempt mission from caller-owned storage after a process restart. */
  rehydrateMission?: AutonomousMissionReplanMissionRehydrator;
  /** Rehydrate transient evaluator guidance from caller-owned storage for a replan handoff. */
  rehydrateReplanInstruction?: AutonomousMissionReplanInstructionRehydrator;
}

export interface AutonomousMissionReplanResult {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_SCHEMA;
  status: AutonomousMissionReplanStatus;
  root_mission_id: string;
  protected_contract_digest: string;
  attempts: AutonomousMissionReplanAttempt[];
  evaluations: AutonomousMissionReplanEvaluationProjection[];
  learning_settlements: AutonomousMissionLearningSettlement[];
  route_digest: string | null;
  cost_budget: AutonomousCostBudgetSnapshot | null;
  replan_count: number;
  final_execution: AutonomousMissionExecutionResult;
  retention: "provider_responses_local;replan_instructions_transient;value_only_evaluation_and_learning_projection";
  secret_material: "never_returned";
}

const RETENTION = "provider_responses_local;replan_instructions_transient;value_only_evaluation_and_learning_projection" as const;
const CHECKPOINT_RETENTION = "metadata_only_no_arguments_outputs_credentials_or_provider_material" as const;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) throw new AutonomousMissionReplanError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum = 256): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousMissionReplanError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousMissionReplanError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCostBudgetSnapshot(name: string, value: unknown, allowNull = true): AutonomousCostBudgetSnapshot | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (!isObject(value)) throw new AutonomousMissionReplanError(`${name} must be a cost budget snapshot`);
  assertKnownKeys(name, value, ["max_cost_units", "consumed_cost_units", "remaining_cost_units"]);
  try {
    const budget = AutonomousCostBudget.fromSnapshot(value as unknown as AutonomousCostBudgetSnapshot);
    const normalized = budget.snapshot();
    if (canonicalJson(normalized) !== canonicalJson(value)) throw new Error("snapshot normalization mismatch");
    return normalized;
  } catch {
    throw new AutonomousMissionReplanError(`${name} is malformed`);
  }
}

function boundedReward(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new AutonomousMissionReplanError(`${name} must be within [0, 1]`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) throw new AutonomousMissionReplanError(`${name} must be an integer within [0, ${maximum}]`);
  return value as number;
}

function safeLabel(name: string, value: unknown, allowNull = true): string | null {
  if (value === null || value === undefined) {
    if (allowNull) return null;
    throw new AutonomousMissionReplanError(`${name} is required`);
  }
  if (typeof value !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/.test(value)) throw new AutonomousMissionReplanError(`${name} must be a bounded label`);
  return value;
}

function screenInstruction(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  const instruction = boundedText("mission replan instruction", value, AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES);
  if (new TextEncoder().encode(instruction).byteLength > AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES) throw new AutonomousMissionReplanError("mission replan instruction exceeds its byte budget");
  if (/(api[_-]?key|authorization|bearer|credential|password|secret|access[_-]?token|refresh[_-]?token|private[_-]?key|gsk_|sk-)/i.test(instruction)) throw new AutonomousMissionReplanError("mission replan instruction contains credential-shaped material");
  return instruction;
}

function normalizeReward(episodeId: string, value: unknown): AutonomousEvaluatorRewardInput {
  if (!isObject(value)) throw new AutonomousMissionReplanError(`mission reward ${episodeId} must be an object`);
  const evaluatorId = safeLabel(`mission reward ${episodeId}.evaluator_id`, value.evaluator_id, false)!;
  const evaluatorVersion = safeLabel(`mission reward ${episodeId}.evaluator_version`, value.evaluator_version, false)!;
  const reward = boundedReward(`mission reward ${episodeId}.reward`, value.reward);
  if (typeof value.passed !== "boolean") throw new AutonomousMissionReplanError(`mission reward ${episodeId}.passed must be boolean`);
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new AutonomousMissionReplanError(`mission reward ${episodeId}.failed must be boolean`);
  const failureClass = value.failure_class === undefined || value.failure_class === null ? null : safeLabel(`mission reward ${episodeId}.failure_class`, value.failure_class, false);
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    ...(value.failed === undefined ? {} : { failed: value.failed }),
    feedback_digest: boundedDigest(`mission reward ${episodeId}.feedback_digest`, value.feedback_digest, true),
    failure_class: failureClass,
    evidence_digest: boundedDigest(`mission reward ${episodeId}.evidence_digest`, value.evidence_digest, true),
  };
}

function normalizeRewards(value: unknown): Record<string, AutonomousEvaluatorRewardInput> {
  if (value === undefined || value === null) return {};
  if (!isObject(value)) throw new AutonomousMissionReplanError("mission replan rewards must be an object keyed by episode ID");
  const entries = Object.entries(value);
  if (entries.length > 128) throw new AutonomousMissionReplanError("mission replan rewards exceed the bounded episode limit");
  return Object.fromEntries(entries.map(([episodeId, reward]) => {
    const id = boundedIdentifier("mission reward episodeId", episodeId);
    return [id, normalizeReward(id, reward)];
  }));
}

function normalizeEvaluation(value: unknown): AutonomousMissionReplanEvaluation {
  if (!isObject(value)) throw new AutonomousMissionReplanError("mission evaluator must return an object");
  const evaluatorId = safeLabel("mission evaluator_id", value.evaluator_id, false)!;
  const evaluatorVersion = safeLabel("mission evaluator_version", value.evaluator_version, false)!;
  const reward = boundedReward("mission evaluator reward", value.reward);
  if (typeof value.passed !== "boolean") throw new AutonomousMissionReplanError("mission evaluator passed must be boolean");
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new AutonomousMissionReplanError("mission evaluator failed must be boolean");
  if (typeof value.replan_requested !== "boolean") throw new AutonomousMissionReplanError("mission evaluator replan_requested must be boolean");
  const instruction = screenInstruction(value.replan_instruction);
  if (value.replan_requested && !instruction) throw new AutonomousMissionReplanError("mission evaluator must provide an instruction when replan_requested is true");
  if (!value.replan_requested && instruction) throw new AutonomousMissionReplanError("mission evaluator supplied an instruction without requesting a replan");
  const failed = value.failed ?? !value.passed;
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    failed,
    replan_requested: value.replan_requested,
    replan_instruction: instruction,
    feedback_digest: boundedDigest("mission evaluator feedback_digest", value.feedback_digest, true),
    failure_class: value.failure_class === undefined || value.failure_class === null ? null : safeLabel("mission evaluator failure_class", value.failure_class, false),
    evidence_digest: boundedDigest("mission evaluator evidence_digest", value.evidence_digest, true),
    rewards: normalizeRewards(value.rewards),
  };
}

function protectedStep(step: AgentMissionStep): JsonObject {
  return {
    id: step.id,
    domain: step.domain,
    capability: step.capability,
    tool: step.tool,
    arguments: step.arguments ?? {},
    depends_on: [...(step.depends_on ?? [])].sort(),
    bindings: [...(step.bindings ?? [])].map((binding) => ({ ...binding })).sort((left, right) => `${left.from_step}/${left.source_pointer}/${left.target_pointer}`.localeCompare(`${right.from_step}/${right.source_pointer}/${right.target_pointer}`)),
    required: step.required ?? true,
  };
}

async function protectedMissionDigest(mission: AgentMissionArgs): Promise<string> {
  const descriptor = {
    goal: mission.goal,
    policy: mission.policy ?? {},
    operations_gate_acceptance: mission.operations_gate_acceptance ?? null,
    claim_requests: mission.claim_requests ?? [],
    evaluator_review: mission.evaluator_review ?? null,
    workflow_binding: mission.workflow_binding ?? null,
    route_review: mission.route_review ?? null,
    steps: [...mission.steps].map(protectedStep).sort((left, right) => String(left.id).localeCompare(String(right.id))),
  };
  return digestJson(descriptor);
}

async function evaluationProjection(value: AutonomousMissionReplanEvaluation): Promise<AutonomousMissionReplanEvaluationProjection> {
  const withoutDigest = {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed ?? !value.passed,
    replan_requested: value.replan_requested,
    replan_instruction_digest: value.replan_instruction ? await digestJson(value.replan_instruction) : null,
    feedback_digest: value.feedback_digest ?? null,
    failure_class: value.failure_class ?? null,
    evidence_digest: value.evidence_digest ?? null,
    rewards_digest: await digestJson(value.rewards ?? {}),
    retention: "evaluator_values_and_digests_only" as const,
    secret_material: "never_returned" as const,
  };
  return { ...withoutDigest, evaluation_digest: await digestJson(withoutDigest) };
}

async function checkpoint(
  rootMissionId: string,
  protectedContractDigest: string,
  attempt: number,
  mission: AgentMissionArgs,
  execution: AutonomousMissionExecutionResult,
  phase: AutonomousMissionReplanCheckpoint["phase"],
  evaluationDigest: string | null,
  instructionDigest: string | null,
  learningTrajectoryId: string | null,
  routeDigest: string | null,
  costBudget: AutonomousCostBudgetSnapshot | null,
): Promise<AutonomousMissionReplanCheckpoint> {
  const descriptor = {
    schema: AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA,
    root_mission_id: rootMissionId,
    attempt,
    mission_id: mission.mission_id,
    protected_contract_digest: protectedContractDigest,
    mission_request_digest: execution.preflight.request_digest ?? null,
    phase,
    mission_status: execution.status,
    evaluation_digest: evaluationDigest,
    replan_instruction_digest: instructionDigest,
    learning_trajectory_id: learningTrajectoryId,
    route_digest: routeDigest,
    cost_budget: costBudget,
    retention: CHECKPOINT_RETENTION,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
}

function isTerminalMission(status: AutonomousMissionStatus): boolean {
  return ["succeeded", "partial", "failed", "cancelled"].includes(status);
}

function missionAttemptId(rootMissionId: string, attempt: number): string {
  return boundedIdentifier("mission attempt id", `${rootMissionId}:attempt-${attempt}`);
}

function expectedMissionId(rootMissionId: string, attempt: number): string {
  return attempt === 1 ? rootMissionId : missionAttemptId(rootMissionId, attempt);
}

async function defaultReplan(rootMissionId: string, mission: AgentMissionArgs, instruction: string, attempt: number): Promise<AgentMissionArgs> {
  const nextMission: AgentMissionArgs = {
    ...structuredClone(mission),
    mission_id: missionAttemptId(rootMissionId, attempt + 1),
    steps: mission.steps.map((step) => ({
      ...structuredClone(step),
      objective: `${step.objective}\n\nBounded evaluator guidance (not authorization): ${instruction}`,
    })),
  };
  return nextMission;
}

async function validateProposal(rootMissionId: string, protectedDigest: string, current: AgentMissionArgs, proposal: AgentMissionArgs, expectedAttempt: number, executor: AutonomousMissionExecutor): Promise<AgentMissionArgs> {
  if (!isObject(proposal)) throw new AutonomousMissionReplanContractError("mission replanner must return a mission object");
  const expectedId = missionAttemptId(rootMissionId, expectedAttempt);
  if (proposal.mission_id !== expectedId) throw new AutonomousMissionReplanContractError(`mission replanner must return mission_id ${expectedId}`);
  if (await protectedMissionDigest(proposal) !== protectedDigest) throw new AutonomousMissionReplanContractError("mission replanner changed the protected mission contract");
  const preflight = await executor.preflight(proposal);
  if (!preflight.ok || preflight.execution !== "authorized") throw new AutonomousMissionReplanContractError("mission replanner returned a mission that fails preflight authorization");
  if (proposal.goal !== current.goal) throw new AutonomousMissionReplanContractError("mission replanner changed the mission goal");
  return structuredClone(proposal);
}

function result(
  status: AutonomousMissionReplanStatus,
  rootMissionId: string,
  protectedContractDigest: string,
  attempts: AutonomousMissionReplanAttempt[],
  evaluations: AutonomousMissionReplanEvaluationProjection[],
  settlements: AutonomousMissionLearningSettlement[],
  finalExecution: AutonomousMissionExecutionResult,
  routeDigest: string | null,
  costBudget: AutonomousCostBudgetSnapshot | null,
): AutonomousMissionReplanResult {
  return {
    schema: AUTONOMOUS_MISSION_REPLAN_SCHEMA,
    status,
    root_mission_id: rootMissionId,
    protected_contract_digest: protectedContractDigest,
    attempts,
    evaluations,
    learning_settlements: settlements,
    route_digest: routeDigest,
    cost_budget: costBudget,
    replan_count: Math.max(0, attempts.length - 1),
    final_execution: finalExecution,
    retention: RETENTION,
    secret_material: "never_returned",
  };
}

/** Validate a metadata-only replan checkpoint before a caller persists or restores it. */
export async function validateAutonomousMissionReplanCheckpoint(value: unknown): Promise<AutonomousMissionReplanCheckpoint> {
  if (!isObject(value)) throw new AutonomousMissionReplanError("mission replan checkpoint must be an object");
  const checkpoint = value as unknown as AutonomousMissionReplanCheckpoint;
  if (checkpoint.schema !== AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA || checkpoint.retention !== CHECKPOINT_RETENTION || checkpoint.secret_material !== "never_returned") throw new AutonomousMissionReplanError("mission replan checkpoint retention markers are invalid");
  boundedIdentifier("replan checkpoint root_mission_id", checkpoint.root_mission_id);
  boundedCount("replan checkpoint attempt", checkpoint.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS + 1);
  boundedIdentifier("replan checkpoint mission_id", checkpoint.mission_id);
  boundedDigest("replan checkpoint protected_contract_digest", checkpoint.protected_contract_digest);
  if (checkpoint.mission_request_digest === undefined || checkpoint.evaluation_digest === undefined || checkpoint.replan_instruction_digest === undefined || checkpoint.learning_trajectory_id === undefined) throw new AutonomousMissionReplanError("replan checkpoint nullable metadata fields are missing");
  boundedDigest("replan checkpoint mission_request_digest", checkpoint.mission_request_digest, true);
  if (!["execution_pending", "evaluation_recorded", "replan_scheduled", "terminal"].includes(checkpoint.phase)) throw new AutonomousMissionReplanError("replan checkpoint phase is invalid");
  if (!AUTONOMOUS_MISSION_STATUSES.includes(checkpoint.mission_status)) throw new AutonomousMissionReplanError("replan checkpoint mission status is invalid");
  boundedDigest("replan checkpoint evaluation_digest", checkpoint.evaluation_digest, true);
  boundedDigest("replan checkpoint replan_instruction_digest", checkpoint.replan_instruction_digest, true);
  if (checkpoint.learning_trajectory_id !== null) boundedIdentifier("replan checkpoint learning_trajectory_id", checkpoint.learning_trajectory_id);
  if (checkpoint.route_digest !== undefined) boundedDigest("replan checkpoint route_digest", checkpoint.route_digest, true);
  if (checkpoint.cost_budget !== undefined) boundedCostBudgetSnapshot("replan checkpoint cost_budget", checkpoint.cost_budget, true);
  boundedDigest("replan checkpoint checkpoint_digest", checkpoint.checkpoint_digest);
  const descriptor = {
    schema: checkpoint.schema,
    root_mission_id: checkpoint.root_mission_id,
    attempt: checkpoint.attempt,
    mission_id: checkpoint.mission_id,
    protected_contract_digest: checkpoint.protected_contract_digest,
    mission_request_digest: checkpoint.mission_request_digest,
    phase: checkpoint.phase,
    mission_status: checkpoint.mission_status,
    evaluation_digest: checkpoint.evaluation_digest,
    replan_instruction_digest: checkpoint.replan_instruction_digest,
    learning_trajectory_id: checkpoint.learning_trajectory_id,
    ...(checkpoint.route_digest === undefined ? {} : { route_digest: checkpoint.route_digest }),
    ...(checkpoint.cost_budget === undefined ? {} : { cost_budget: checkpoint.cost_budget }),
    retention: checkpoint.retention,
    secret_material: checkpoint.secret_material,
  };
  if (await digestJson(descriptor) !== checkpoint.checkpoint_digest) throw new AutonomousMissionReplanError("mission replan checkpoint digest does not match its metadata");
  return structuredClone(checkpoint);
}

function jsonBytes(value: unknown): number {
  let serialized: string;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new AutonomousMissionReplanError("mission replan metadata must be JSON serializable");
  }
  if (typeof serialized !== "string") throw new AutonomousMissionReplanError("mission replan metadata must be JSON serializable");
  return new TextEncoder().encode(serialized).byteLength;
}

function assertKnownKeys(name: string, value: Record<string, unknown>, keys: readonly string[]): void {
  const allowed = new Set(keys);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousMissionReplanError(`${name} contains unsupported or non-metadata fields`);
}

function assertValueOnlySettlement(value: unknown, index: number): asserts value is AutonomousMissionLearningSettlement {
  if (!isObject(value)) throw new AutonomousMissionReplanError(`mission learning settlement ${index} must be an object`);
  const settlement = value as unknown as AutonomousMissionLearningSettlement;
  assertKnownKeys(`mission learning settlement ${index}`, settlement, ["schema", "mission_id", "trajectory_id", "episode_ids", "settlement", "retention", "secret_material"]);
  if (settlement.schema !== "bioprism-typescript-autonomous-mission-learning-settlement/0.1" || settlement.retention !== "value_only_learning_projection" || settlement.secret_material !== "never_returned") throw new AutonomousMissionReplanError(`mission learning settlement ${index} retention markers are invalid`);
  boundedIdentifier(`mission learning settlement ${index}.mission_id`, settlement.mission_id);
  boundedIdentifier(`mission learning settlement ${index}.trajectory_id`, settlement.trajectory_id);
  if (!Array.isArray(settlement.episode_ids) || settlement.episode_ids.length > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS * 128) throw new AutonomousMissionReplanError(`mission learning settlement ${index}.episode_ids are malformed`);
  for (const episodeId of settlement.episode_ids) boundedIdentifier(`mission learning settlement ${index}.episode_id`, episodeId);
  if (!isObject(settlement.settlement)) throw new AutonomousMissionReplanError(`mission learning settlement ${index}.settlement is malformed`);
  if (jsonBytes(settlement) > 4_000_000) throw new AutonomousMissionReplanError(`mission learning settlement ${index} exceeds its metadata budget`);
  const inspectMetadata = (candidate: unknown, depth: number): void => {
    if (depth > 16) throw new AutonomousMissionReplanError(`mission learning settlement ${index} is too deeply nested`);
    if (Array.isArray(candidate)) {
      if (candidate.length > 8_192) throw new AutonomousMissionReplanError(`mission learning settlement ${index} contains too many metadata rows`);
      for (const child of candidate) inspectMetadata(child, depth + 1);
      return;
    }
    if (!isObject(candidate)) return;
    for (const [key, child] of Object.entries(candidate)) {
      if (/^(arguments?|output|response|instruction|content|credential|password|secret|token)$/i.test(key)) throw new AutonomousMissionReplanError(`mission learning settlement ${index} contains raw payload field ${key}`);
      inspectMetadata(child, depth + 1);
    }
  };
  inspectMetadata(settlement.settlement, 0);
  const serialized = JSON.stringify(settlement);
  if (typeof serialized === "string" && /(api[_-]?key|authorization|bearer|password|private[_-]?key|access[_-]?token|refresh[_-]?token|gsk_|sk-)/i.test(serialized)) throw new AutonomousMissionReplanError(`mission learning settlement ${index} contains credential-shaped material`);
}

function evaluationProjectionDescriptor(value: AutonomousMissionReplanEvaluationProjection): JsonObject {
  return {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed,
    replan_requested: value.replan_requested,
    replan_instruction_digest: value.replan_instruction_digest,
    feedback_digest: value.feedback_digest,
    failure_class: value.failure_class,
    evidence_digest: value.evidence_digest,
    rewards_digest: value.rewards_digest,
    retention: value.retention,
    secret_material: value.secret_material,
  };
}

async function validateEvaluationProjection(value: unknown, index: number): Promise<AutonomousMissionReplanEvaluationProjection> {
  if (!isObject(value)) throw new AutonomousMissionReplanError(`mission evaluation projection ${index} must be an object`);
  const projection = value as unknown as AutonomousMissionReplanEvaluationProjection;
  assertKnownKeys(`mission evaluation projection ${index}`, projection, ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "replan_requested", "replan_instruction_digest", "feedback_digest", "failure_class", "evidence_digest", "rewards_digest", "evaluation_digest", "retention", "secret_material"]);
  if (projection.retention !== "evaluator_values_and_digests_only" || projection.secret_material !== "never_returned") throw new AutonomousMissionReplanError(`mission evaluation projection ${index} retention markers are invalid`);
  safeLabel(`mission evaluation projection ${index}.evaluator_id`, projection.evaluator_id, false);
  safeLabel(`mission evaluation projection ${index}.evaluator_version`, projection.evaluator_version, false);
  boundedReward(`mission evaluation projection ${index}.reward`, projection.reward);
  if (typeof projection.passed !== "boolean" || typeof projection.failed !== "boolean" || typeof projection.replan_requested !== "boolean") throw new AutonomousMissionReplanError(`mission evaluation projection ${index} boolean fields are malformed`);
  if (projection.replan_requested !== (projection.replan_instruction_digest !== null)) throw new AutonomousMissionReplanError(`mission evaluation projection ${index} instruction state is inconsistent`);
  if (projection.replan_instruction_digest === undefined || projection.feedback_digest === undefined || projection.failure_class === undefined || projection.evidence_digest === undefined) throw new AutonomousMissionReplanError(`mission evaluation projection ${index} nullable fields are missing`);
  boundedDigest(`mission evaluation projection ${index}.replan_instruction_digest`, projection.replan_instruction_digest, true);
  boundedDigest(`mission evaluation projection ${index}.feedback_digest`, projection.feedback_digest, true);
  if (projection.failure_class !== null) safeLabel(`mission evaluation projection ${index}.failure_class`, projection.failure_class, false);
  boundedDigest(`mission evaluation projection ${index}.evidence_digest`, projection.evidence_digest, true);
  boundedDigest(`mission evaluation projection ${index}.rewards_digest`, projection.rewards_digest);
  boundedDigest(`mission evaluation projection ${index}.evaluation_digest`, projection.evaluation_digest);
  if (await digestJson(evaluationProjectionDescriptor(projection)) !== projection.evaluation_digest) throw new AutonomousMissionReplanError(`mission evaluation projection ${index} digest does not match its metadata`);
  return structuredClone(projection);
}

function validateAttempt(value: unknown, index: number): AutonomousMissionReplanAttempt {
  if (!isObject(value)) throw new AutonomousMissionReplanError(`mission replan attempt ${index} must be an object`);
  const attempt = value as unknown as AutonomousMissionReplanAttempt;
  assertKnownKeys(`mission replan attempt ${index}`, attempt, ["attempt", "mission_id", "status", "next_wave", "completed_steps", "succeeded_steps", "failed_steps", "evaluation_digest", "learning_trajectory_id", "replan_instruction_digest", "route_digest"]);
  boundedCount(`mission replan attempt ${index}.attempt`, attempt.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS);
  boundedIdentifier(`mission replan attempt ${index}.mission_id`, attempt.mission_id);
  if (!AUTONOMOUS_MISSION_STATUSES.includes(attempt.status)) throw new AutonomousMissionReplanError(`mission replan attempt ${index}.status is invalid`);
  if (attempt.next_wave !== null) boundedCount(`mission replan attempt ${index}.next_wave`, attempt.next_wave, 128);
  boundedCount(`mission replan attempt ${index}.completed_steps`, attempt.completed_steps, 128);
  boundedCount(`mission replan attempt ${index}.succeeded_steps`, attempt.succeeded_steps, 128);
  boundedCount(`mission replan attempt ${index}.failed_steps`, attempt.failed_steps, 128);
  if (attempt.evaluation_digest === undefined || attempt.learning_trajectory_id === undefined || attempt.replan_instruction_digest === undefined) throw new AutonomousMissionReplanError(`mission replan attempt ${index} nullable fields are missing`);
  boundedDigest(`mission replan attempt ${index}.evaluation_digest`, attempt.evaluation_digest, true);
  if (attempt.learning_trajectory_id !== null) boundedIdentifier(`mission replan attempt ${index}.learning_trajectory_id`, attempt.learning_trajectory_id);
  boundedDigest(`mission replan attempt ${index}.replan_instruction_digest`, attempt.replan_instruction_digest, true);
  if (attempt.route_digest !== undefined) boundedDigest(`mission replan attempt ${index}.route_digest`, attempt.route_digest, true);
  if (attempt.succeeded_steps + attempt.failed_steps > attempt.completed_steps) throw new AutonomousMissionReplanError(`mission replan attempt ${index} step counts are inconsistent`);
  return structuredClone(attempt);
}

function stateDescriptor(state: AutonomousMissionReplanState): JsonObject {
  const { state_digest: _stateDigest, ...descriptor } = state;
  return descriptor;
}

/** Validate a restart-safe, metadata-only orchestration state. */
export async function validateAutonomousMissionReplanState(value: unknown): Promise<AutonomousMissionReplanState> {
  if (!isObject(value)) throw new AutonomousMissionReplanError("mission replan state must be an object");
  const state = value as unknown as AutonomousMissionReplanState;
  assertKnownKeys("mission replan state", state as unknown as Record<string, unknown>, ["schema", "root_mission_id", "protected_contract_digest", "max_replans", "attempt", "current_mission_id", "mission_request_digest", "phase", "replan_instruction_digest", "terminal_status", "attempts", "evaluations", "learning_settlements", "route_digest", "cost_budget", "last_mission_checkpoint_digest", "generation", "previous_state_digest", "state_digest", "retention", "secret_material"]);
  if (state.schema !== AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA || state.retention !== "metadata_only_no_arguments_outputs_credentials_provider_material_or_raw_instructions" || state.secret_material !== "never_returned") throw new AutonomousMissionReplanError("mission replan state retention markers are invalid");
  boundedIdentifier("mission replan state root_mission_id", state.root_mission_id);
  boundedDigest("mission replan state protected_contract_digest", state.protected_contract_digest);
  boundedCount("mission replan state max_replans", state.max_replans, AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS);
  boundedCount("mission replan state attempt", state.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS);
  if (state.attempt < 1) throw new AutonomousMissionReplanError("mission replan state attempt must be positive");
  boundedIdentifier("mission replan state current_mission_id", state.current_mission_id);
  if (state.mission_request_digest === undefined || state.replan_instruction_digest === undefined || state.terminal_status === undefined || state.last_mission_checkpoint_digest === undefined || state.previous_state_digest === undefined) throw new AutonomousMissionReplanError("mission replan state nullable fields are missing");
  boundedDigest("mission replan state mission_request_digest", state.mission_request_digest, true);
  if (!["execution_pending", "evaluation_pending", "replan_handoff", "terminal"].includes(state.phase)) throw new AutonomousMissionReplanError("mission replan state phase is invalid");
  boundedDigest("mission replan state replan_instruction_digest", state.replan_instruction_digest, true);
  if (state.route_digest !== undefined) boundedDigest("mission replan state route_digest", state.route_digest, true);
  if (state.cost_budget !== undefined) boundedCostBudgetSnapshot("mission replan state cost_budget", state.cost_budget, true);
  if (state.terminal_status !== null && !["completed", "completed_without_replan", "replan_limit_reached", ...AUTONOMOUS_MISSION_STATUSES].includes(state.terminal_status)) throw new AutonomousMissionReplanError("mission replan state terminal status is invalid");
  boundedDigest("mission replan state last_mission_checkpoint_digest", state.last_mission_checkpoint_digest, true);
  boundedCount("mission replan state generation", state.generation, Number.MAX_SAFE_INTEGER);
  if (state.generation < 1) throw new AutonomousMissionReplanError("mission replan state generation must be positive");
  boundedDigest("mission replan state previous_state_digest", state.previous_state_digest, true);
  if (state.current_mission_id !== expectedMissionId(state.root_mission_id, state.attempt)) throw new AutonomousMissionReplanError("mission replan state current mission identity is inconsistent with its attempt");
  if (!Array.isArray(state.attempts) || state.attempts.length > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS) throw new AutonomousMissionReplanError("mission replan state attempts exceed capacity");
  if (!Array.isArray(state.evaluations) || state.evaluations.length > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS) throw new AutonomousMissionReplanError("mission replan state evaluations exceed capacity");
  if (!Array.isArray(state.learning_settlements) || state.learning_settlements.length > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS) throw new AutonomousMissionReplanError("mission replan state settlements exceed capacity");
  const attempts = state.attempts.map(validateAttempt);
  const attemptIds = new Set<number>();
  for (const attempt of attempts) {
    if (attemptIds.has(attempt.attempt) || attempt.attempt > state.attempt) throw new AutonomousMissionReplanError("mission replan state contains duplicate or future attempts");
    attemptIds.add(attempt.attempt);
    if (attempt.mission_id !== expectedMissionId(state.root_mission_id, attempt.attempt)) throw new AutonomousMissionReplanError("mission replan state attempt identity is inconsistent with its root");
  }
  const attemptRouteDigests = [...new Set(attempts.map((attempt) => attempt.route_digest ?? null).filter((digest): digest is string => digest !== null))];
  if (attemptRouteDigests.length > 1 || (state.route_digest !== undefined && state.route_digest !== null && attemptRouteDigests.some((digest) => digest !== state.route_digest))) throw new AutonomousMissionReplanError("mission replan state attempts do not share one route identity");
  if (attempts.some((attempt, index) => attempt.attempt !== index + 1)) throw new AutonomousMissionReplanError("mission replan state attempts are not contiguous");
  const evaluations: AutonomousMissionReplanEvaluationProjection[] = [];
  for (let index = 0; index < state.evaluations.length; index += 1) evaluations.push(await validateEvaluationProjection(state.evaluations[index], index));
  for (let index = 0; index < state.learning_settlements.length; index += 1) assertValueOnlySettlement(state.learning_settlements[index], index);
  if (state.phase === "replan_handoff" && (!state.evaluations.length || !state.replan_instruction_digest)) throw new AutonomousMissionReplanError("mission replan handoff requires the last evaluation and instruction digest");
  if (state.phase === "terminal" && state.terminal_status === null) throw new AutonomousMissionReplanError("terminal mission replan state requires a terminal status");
  if (state.attempts.length !== state.evaluations.length && state.phase !== "execution_pending") throw new AutonomousMissionReplanError("mission replan state attempt and evaluation counts are inconsistent");
  if (await digestJson(stateDescriptor(state)) !== state.state_digest) throw new AutonomousMissionReplanError("mission replan state digest does not match its metadata");
  return { ...structuredClone(state), attempts, evaluations };
}

/** In-memory reference implementation for a caller-owned durable mission state table. */
export class InMemoryAutonomousMissionReplanStateStore implements AutonomousMissionReplanSnapshotStore {
  private readonly states = new Map<string, AutonomousMissionReplanState>();

  async load(rootMissionId: string): Promise<AutonomousMissionReplanState | null> {
    return structuredClone(this.states.get(boundedIdentifier("root_mission_id", rootMissionId)) ?? null);
  }

  async save(state: AutonomousMissionReplanState): Promise<void> {
    const normalized = await validateAutonomousMissionReplanState(state);
    const previous = this.states.get(normalized.root_mission_id);
    if (!previous) {
      if (normalized.generation !== 1 || normalized.previous_state_digest !== null) throw new AutonomousMissionReplanError("initial mission replan state must start at generation one");
    } else if (previous.state_digest !== normalized.state_digest && (normalized.generation !== previous.generation + 1 || normalized.previous_state_digest !== previous.state_digest)) {
      throw new AutonomousMissionReplanError("mission replan state generation is not contiguous");
    }
    if (!previous && this.states.size >= AUTONOMOUS_MISSION_REPLAN_MAX_STATES) throw new AutonomousMissionReplanError("mission replan state capacity is exhausted");
    this.states.set(normalized.root_mission_id, structuredClone(normalized));
  }

  async snapshot(): Promise<AutonomousMissionReplanSnapshot> {
    const states = [...this.states.values()].sort((left, right) => left.root_mission_id.localeCompare(right.root_mission_id)).map((state) => structuredClone(state));
    const descriptor = { schema: AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA, states, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: await digestJson(descriptor) };
  }

  async restore(snapshot: AutonomousMissionReplanSnapshot): Promise<void> {
    const validated = await validateAutonomousMissionReplanSnapshot(snapshot);
    this.states.clear();
    for (const state of validated.states) this.states.set(state.root_mission_id, structuredClone(state));
  }
}

/** Validate a hash-bound collection of mission orchestration states before restoration. */
export async function validateAutonomousMissionReplanSnapshot(value: unknown): Promise<AutonomousMissionReplanSnapshot> {
  if (!isObject(value)) throw new AutonomousMissionReplanError("mission replan snapshot must be an object");
  const snapshot = value as unknown as AutonomousMissionReplanSnapshot;
  if (snapshot.schema !== AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA || snapshot.retention !== "metadata_only_hash_bound" || snapshot.secret_material !== "never_returned") throw new AutonomousMissionReplanError("mission replan snapshot retention markers are invalid");
  if (!Array.isArray(snapshot.states) || snapshot.states.length > AUTONOMOUS_MISSION_REPLAN_MAX_STATES) throw new AutonomousMissionReplanError("mission replan snapshot capacity is exhausted");
  const states: AutonomousMissionReplanState[] = [];
  const ids = new Set<string>();
  for (const raw of snapshot.states) {
    const state = await validateAutonomousMissionReplanState(raw);
    if (ids.has(state.root_mission_id)) throw new AutonomousMissionReplanError("mission replan snapshot contains duplicate roots");
    ids.add(state.root_mission_id);
    states.push(state);
  }
  states.sort((left, right) => left.root_mission_id.localeCompare(right.root_mission_id));
  boundedDigest("mission replan snapshot snapshot_digest", snapshot.snapshot_digest);
  const descriptor = { schema: snapshot.schema, states, retention: snapshot.retention, secret_material: snapshot.secret_material };
  if (await digestJson(descriptor) !== snapshot.snapshot_digest) throw new AutonomousMissionReplanError("mission replan snapshot digest does not match its metadata");
  if (jsonBytes(snapshot) > AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES) throw new AutonomousMissionReplanError("mission replan snapshot exceeds its bounded size");
  return { ...structuredClone(snapshot), states };
}

/** Coordinates a metadata-only replan state table with caller-owned persistence. */
export class AutonomousMissionReplanPersistenceCoordinator {
  readonly store: AutonomousMissionReplanSnapshotStore;
  readonly persistence: AutonomousMissionReplanSnapshotPersistence;

  constructor(store: AutonomousMissionReplanSnapshotStore, persistence: AutonomousMissionReplanSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("mission replan persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("mission replan persistence adapter is malformed");
    this.store = store;
    this.persistence = persistence;
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA; bytes: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await this.store.snapshot();
    const bytes = jsonBytes(snapshot);
    if (bytes > AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES) throw new AutonomousMissionReplanError("mission replan snapshot exceeds its bounded size");
    await this.persistence.write(snapshot);
    return { schema: AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA, bytes, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }

  async restore(): Promise<{ schema: typeof AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA; restored: boolean; states: number; snapshot_digest: string | null; retention: "metadata_only" }> {
    const raw = await this.persistence.read();
    if (raw === null) return { schema: AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA, restored: false, states: 0, snapshot_digest: null, retention: "metadata_only" };
    const snapshot = await validateAutonomousMissionReplanSnapshot(raw);
    await this.store.restore(snapshot);
    return { schema: AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA, restored: true, states: snapshot.states.length, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }
}

/**
 * Execute a bounded evaluator-guided mission loop. A replan may refine objectives or reorder
 * independent steps, but cannot change tools, arguments, domains, dependencies, policy, claims,
 * route review, credentials, or effect authority. Each attempt uses a new durable mission ID.
 */
export async function runAutonomousMissionReplanCycle(
  executor: AutonomousMissionExecutor,
  mission: AgentMissionArgs,
  options: AutonomousMissionReplanOptions,
): Promise<AutonomousMissionReplanResult> {
  if (!executor || typeof executor.start !== "function" || typeof executor.preflight !== "function") throw new ArgumentError("mission replan cycle requires an AutonomousMissionExecutor");
  if (!isObject(options) || typeof options.evaluate !== "function") throw new ArgumentError("mission replan cycle requires an evaluator callback");
  if (!isObject(mission)) throw new ArgumentError("mission replan cycle requires a mission object");
  const maxReplans = boundedCount("mission maxReplans", options.maxReplans ?? 1, AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS);
  const rootMissionId = boundedIdentifier("root mission_id", mission.mission_id);
  const protectedContractDigest = await protectedMissionDigest(mission);
  if (options.learning && (!options.learning.adapter || typeof options.learning.adapter.prepareTrajectory !== "function" || typeof options.learning.adapter.settleTrajectory !== "function")) throw new ArgumentError("mission replan learning adapter is malformed");
  const trajectoryPrefix = options.learning ? boundedIdentifier("mission trajectoryIdPrefix", options.learning.trajectoryIdPrefix ?? "mission-replan") : null;
  const stateStore = options.stateStore;
  const loaded = stateStore ? await stateStore.load(rootMissionId) : null;
  const persisted = loaded === null ? null : await validateAutonomousMissionReplanState(loaded);
  if (persisted && (persisted.root_mission_id !== rootMissionId || persisted.protected_contract_digest !== protectedContractDigest)) throw new AutonomousMissionReplanContractError("stored mission replan state does not match the protected mission contract");
  if (persisted && persisted.max_replans !== maxReplans) throw new AutonomousMissionReplanError("stored mission replan state was created with a different replan limit");

  let current = structuredClone(mission);
  let phase: AutonomousMissionReplanState["phase"] = persisted?.phase ?? "execution_pending";
  let attempt = persisted?.attempt ?? 1;
  const attempts: AutonomousMissionReplanAttempt[] = structuredClone(persisted?.attempts ?? []);
  const evaluations: AutonomousMissionReplanEvaluationProjection[] = structuredClone(persisted?.evaluations ?? []);
  const settlements: AutonomousMissionLearningSettlement[] = structuredClone(persisted?.learning_settlements ?? []);
  let missionRequestDigest = persisted?.mission_request_digest ?? null;
  let replanInstructionDigest = persisted?.replan_instruction_digest ?? null;
  let lastMissionCheckpointDigest = persisted?.last_mission_checkpoint_digest ?? null;
  let terminalStatus = persisted?.terminal_status ?? null;
  let finalExecution: AutonomousMissionExecutionResult | null = null;
  let persistedState: AutonomousMissionReplanState | null = persisted;
  let routeDigest = persisted?.route_digest ?? null;
  let routeOverride: AutonomousRouteProposal | null = options.execute?.routeOverride ?? null;
  if (routeDigest !== null && routeOverride !== null && routeOverride.route_digest !== routeDigest) throw new AutonomousMissionReplanContractError("supplied mission route override does not match the persisted route digest");
  if (routeDigest !== null && routeOverride === null) throw new AutonomousMissionReplanError("stored mission replan state requires caller-owned routeOverride for route recovery; semantic routing is never replayed implicitly");

  const executeTemplate = options.execute ?? {};
  if (executeTemplate.costBudget !== undefined && executeTemplate.maxTotalCostUnits !== undefined) throw new AutonomousMissionReplanError("mission replan execute options cannot combine costBudget and maxTotalCostUnits");
  let sharedCostBudget = executeTemplate.costBudget;
  if (persisted?.cost_budget !== undefined && persisted.cost_budget !== null) {
    const persistedBudget = AutonomousCostBudget.fromSnapshot(boundedCostBudgetSnapshot("persisted mission replan cost_budget", persisted.cost_budget, false)!);
    if (sharedCostBudget !== undefined) {
      if (canonicalJson(sharedCostBudget.snapshot()) !== canonicalJson(persistedBudget.snapshot())) throw new AutonomousMissionReplanContractError("supplied mission cost budget does not match the persisted accounting");
    } else if (executeTemplate.maxTotalCostUnits !== undefined && executeTemplate.maxTotalCostUnits !== persistedBudget.maxCostUnits) {
      throw new AutonomousMissionReplanContractError("supplied mission maxTotalCostUnits does not match the persisted accounting");
    } else {
      sharedCostBudget = persistedBudget;
    }
  } else if (sharedCostBudget === undefined && executeTemplate.maxTotalCostUnits !== undefined) {
    sharedCostBudget = new AutonomousCostBudget(executeTemplate.maxTotalCostUnits);
  }
  const executeBase: Omit<AutonomousMissionExecuteOptions, "signal" | "execution_attempt"> = sharedCostBudget === undefined
    ? executeTemplate
    : { ...executeTemplate, costBudget: sharedCostBudget, maxTotalCostUnits: undefined };
  const executeOptions = (attemptNumber: number): AutonomousMissionExecuteOptions => ({
    ...executeBase,
    ...(routeOverride === null ? {} : { routeOverride, semanticRouting: undefined }),
    execution_attempt: attemptNumber,
    signal: options.signal,
  });
  const observeExecutionRoute = (execution: AutonomousMissionExecutionResult): void => {
    const observed = execution.route?.route_digest ?? null;
    if (observed === null) return;
    if (routeDigest !== null && routeDigest !== observed) throw new AutonomousMissionReplanContractError("mission execution changed the persisted route identity");
    routeDigest = observed;
    if (routeOverride === null && execution.semantic_route_status === "completed" && execution.route !== null) routeOverride = execution.route;
  };

  if (persisted && persisted.current_mission_id !== rootMissionId) {
    if (!options.rehydrateMission) throw new AutonomousMissionReplanError("resume requires caller-owned mission rehydration for the current non-root attempt");
    current = await options.rehydrateMission({ root_mission_id: rootMissionId, mission_id: persisted.current_mission_id, attempt, protected_contract_digest: protectedContractDigest });
    if (!isObject(current) || current.mission_id !== persisted.current_mission_id || await protectedMissionDigest(current) !== protectedContractDigest) throw new AutonomousMissionReplanContractError("rehydrated mission does not match the protected mission contract");
    const rehydratedPreflight = await executor.preflight(current);
    if (!rehydratedPreflight.ok || rehydratedPreflight.execution !== "authorized") throw new AutonomousMissionReplanContractError("rehydrated mission fails preflight authorization");
  } else if (persisted && persisted.attempt !== 1) {
    throw new AutonomousMissionReplanContractError("stored non-root attempt is missing its caller-owned mission rehydration");
  }

  const upsertAttempt = (entry: AutonomousMissionReplanAttempt): void => {
    const index = attempts.findIndex((existing) => existing.attempt === entry.attempt);
    if (index === -1) attempts.push(entry);
    else attempts[index] = entry;
  };

  const persistState = async (next: {
    phase: AutonomousMissionReplanState["phase"];
    terminalStatus: AutonomousMissionReplanStatus | null;
    missionRequestDigest: string | null;
    replanInstructionDigest: string | null;
    lastMissionCheckpointDigest: string | null;
    routeDigest?: string | null;
    costBudget?: AutonomousCostBudgetSnapshot | null;
  }): Promise<void> => {
    if (!stateStore) return;
    const nextRouteDigest = Object.prototype.hasOwnProperty.call(next, "routeDigest") ? next.routeDigest ?? null : routeDigest;
    const nextCostBudget = Object.prototype.hasOwnProperty.call(next, "costBudget") ? next.costBudget ?? null : sharedCostBudget?.snapshot() ?? null;
    const descriptor = {
      schema: AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA,
      root_mission_id: rootMissionId,
      protected_contract_digest: protectedContractDigest,
      max_replans: maxReplans,
      attempt,
      current_mission_id: current.mission_id,
      mission_request_digest: next.missionRequestDigest,
      phase: next.phase,
      replan_instruction_digest: next.replanInstructionDigest,
      terminal_status: next.terminalStatus,
      attempts: structuredClone(attempts),
      evaluations: structuredClone(evaluations),
      learning_settlements: structuredClone(settlements),
      route_digest: nextRouteDigest,
      cost_budget: nextCostBudget,
      last_mission_checkpoint_digest: next.lastMissionCheckpointDigest,
      generation: (persistedState?.generation ?? 0) + 1,
      previous_state_digest: persistedState?.state_digest ?? null,
      retention: "metadata_only_no_arguments_outputs_credentials_provider_material_or_raw_instructions" as const,
      secret_material: "never_returned" as const,
    };
    const state = { ...descriptor, state_digest: await digestJson(descriptor) };
    await stateStore.save(state);
    persistedState = state;
    phase = next.phase;
    terminalStatus = next.terminalStatus;
    missionRequestDigest = next.missionRequestDigest;
    replanInstructionDigest = next.replanInstructionDigest;
    routeDigest = nextRouteDigest;
    lastMissionCheckpointDigest = next.lastMissionCheckpointDigest;
  };

  const writeCheckpoint = async (
    execution: AutonomousMissionExecutionResult,
    checkpointPhase: AutonomousMissionReplanCheckpoint["phase"],
    evaluationDigest: string | null,
    instructionDigest: string | null,
    trajectoryId: string | null,
  ): Promise<AutonomousMissionReplanCheckpoint> => {
    const row = await checkpoint(rootMissionId, protectedContractDigest, attempt, current, execution, checkpointPhase, evaluationDigest, instructionDigest, trajectoryId, routeDigest, sharedCostBudget?.snapshot() ?? null);
    if (options.checkpointSink) await options.checkpointSink(row);
    return row;
  };

  while (true) {
    if (phase === "terminal") {
      if (!terminalStatus) throw new AutonomousMissionReplanError("terminal mission replan state is missing its result status");
      finalExecution ??= await executor.start(current, executeOptions(attempt));
      observeExecutionRoute(finalExecution);
      return result(terminalStatus, rootMissionId, protectedContractDigest, attempts, evaluations, settlements, finalExecution, routeDigest, sharedCostBudget?.snapshot() ?? null);
    }

    if (phase === "replan_handoff") {
      const projection = evaluations.at(-1);
      if (!projection || !replanInstructionDigest) throw new AutonomousMissionReplanError("stored replan handoff is missing its evaluator projection");
      if (!options.rehydrateReplanInstruction) throw new AutonomousMissionReplanError("resume requires caller-owned replan instruction rehydration; raw evaluator guidance is never persisted");
      const instruction = screenInstruction(await options.rehydrateReplanInstruction({ root_mission_id: rootMissionId, mission_id: current.mission_id, attempt, instruction_digest: replanInstructionDigest, evaluation: structuredClone(projection) }));
      if (!instruction || await digestJson(instruction) !== replanInstructionDigest) throw new AutonomousMissionReplanContractError("rehydrated replan instruction does not match its stored digest");
      finalExecution = await executor.start(current, executeOptions(attempt));
      observeExecutionRoute(finalExecution);
      if (!isTerminalMission(finalExecution.status)) throw new AutonomousMissionReplanError("stored replan handoff does not point to a terminal mission attempt");
      if (current.policy?.allow_side_effects === true && !options.replan) throw new AutonomousMissionReplanError("default mission replanning refuses side-effect-enabled missions; supply an explicit idempotency-aware replanner");
      const proposal = options.replan
        ? await options.replan({ mission: structuredClone(current), execution: finalExecution, evaluation: structuredClone(projection), instruction, attempt })
        : await defaultReplan(rootMissionId, current, instruction, attempt);
      current = await validateProposal(rootMissionId, protectedContractDigest, current, proposal, attempt + 1, executor);
      attempt += 1;
      await persistState({ phase: "execution_pending", terminalStatus: null, missionRequestDigest: null, replanInstructionDigest: null, lastMissionCheckpointDigest });
      continue;
    }

    if (phase === "execution_pending") await persistState({ phase: "execution_pending", terminalStatus: null, missionRequestDigest, replanInstructionDigest, lastMissionCheckpointDigest });
    const execution = await executor.start(current, executeOptions(attempt));
    finalExecution = execution;
    observeExecutionRoute(execution);
    missionRequestDigest = execution.preflight.request_digest ?? null;
    if (!isTerminalMission(execution.status)) {
      const pendingAttempt = { attempt, mission_id: current.mission_id, status: execution.status, next_wave: execution.next_wave, completed_steps: execution.completed_steps, succeeded_steps: execution.succeeded_steps, failed_steps: execution.failed_steps, evaluation_digest: null, learning_trajectory_id: null, replan_instruction_digest: null, route_digest: routeDigest } satisfies AutonomousMissionReplanAttempt;
      upsertAttempt(pendingAttempt);
      const row = await writeCheckpoint(execution, "execution_pending", null, null, null);
      await persistState({ phase: "execution_pending", terminalStatus: null, missionRequestDigest, replanInstructionDigest: null, lastMissionCheckpointDigest: row.checkpoint_digest });
      return result(execution.status, rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution, routeDigest, sharedCostBudget?.snapshot() ?? null);
    }
    if (phase === "execution_pending") {
      const pendingIndex = attempts.findIndex((existing) => existing.attempt === attempt && existing.evaluation_digest === null);
      if (pendingIndex !== -1) attempts.splice(pendingIndex, 1);
      await persistState({ phase: "evaluation_pending", terminalStatus: null, missionRequestDigest, replanInstructionDigest: null, lastMissionCheckpointDigest: execution.checkpoint?.checkpoint_digest ?? lastMissionCheckpointDigest });
      phase = "evaluation_pending";
    }

    const evaluation = normalizeEvaluation(await options.evaluate(execution));
    const projection = await evaluationProjection(evaluation);
    let trajectoryId: string | null = null;
    const rewardKeys = Object.keys(evaluation.rewards ?? {});
    if (options.learning) {
      const hasEpisode = Object.values(execution.checkpoint?.step_states ?? {}).some((state) => state.status === "succeeded" && state.learning_episode_id !== null);
      if (hasEpisode || rewardKeys.length) {
        trajectoryId = boundedIdentifier("mission learning trajectory", `${trajectoryPrefix}:${rootMissionId}:attempt-${attempt}`);
        settlements.push(await settleAutonomousMissionLearning(execution, options.learning.adapter, {
          trajectoryId,
          discount: options.learning.discount,
          rewards: evaluation.rewards ?? {},
          remote: options.learning.remote,
        }));
      }
    } else if (rewardKeys.length) {
      throw new AutonomousMissionReplanError("mission evaluator returned learning rewards without a learning adapter");
    }

    upsertAttempt({ attempt, mission_id: current.mission_id, status: execution.status, next_wave: execution.next_wave, completed_steps: execution.completed_steps, succeeded_steps: execution.succeeded_steps, failed_steps: execution.failed_steps, evaluation_digest: projection.evaluation_digest, learning_trajectory_id: trajectoryId, replan_instruction_digest: projection.replan_instruction_digest, route_digest: routeDigest });
    evaluations.push(projection);
    replanInstructionDigest = projection.replan_instruction_digest;
    if (!evaluation.replan_requested) {
      const row = await writeCheckpoint(execution, "terminal", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId);
      await persistState({ phase: "terminal", terminalStatus: evaluation.passed ? "completed" : "completed_without_replan", missionRequestDigest, replanInstructionDigest, lastMissionCheckpointDigest: row.checkpoint_digest });
      return result(evaluation.passed ? "completed" : "completed_without_replan", rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution, routeDigest, sharedCostBudget?.snapshot() ?? null);
    }
    if (attempt > maxReplans) {
      const row = await writeCheckpoint(execution, "terminal", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId);
      await persistState({ phase: "terminal", terminalStatus: "replan_limit_reached", missionRequestDigest, replanInstructionDigest, lastMissionCheckpointDigest: row.checkpoint_digest });
      return result("replan_limit_reached", rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution, routeDigest, sharedCostBudget?.snapshot() ?? null);
    }
    const row = await writeCheckpoint(execution, "replan_scheduled", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId);
    await persistState({ phase: "replan_handoff", terminalStatus: null, missionRequestDigest, replanInstructionDigest: projection.replan_instruction_digest, lastMissionCheckpointDigest: row.checkpoint_digest });
    if (current.policy?.allow_side_effects === true && !options.replan) throw new AutonomousMissionReplanError("default mission replanning refuses side-effect-enabled missions; supply an explicit idempotency-aware replanner");
    const instruction = evaluation.replan_instruction;
    if (!instruction) throw new AutonomousMissionReplanError("mission evaluator replan instruction disappeared before handoff");
    const proposal = options.replan
      ? await options.replan({ mission: structuredClone(current), execution, evaluation: projection, instruction, attempt })
      : await defaultReplan(rootMissionId, current, instruction, attempt);
    current = await validateProposal(rootMissionId, protectedContractDigest, current, proposal, attempt + 1, executor);
    attempt += 1;
    await persistState({ phase: "execution_pending", terminalStatus: null, missionRequestDigest: null, replanInstructionDigest: null, lastMissionCheckpointDigest: row.checkpoint_digest });
  }
}
