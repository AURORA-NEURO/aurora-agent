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
import { digestJson } from "./tooling.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { AgentMissionArgs, AgentMissionStep, JsonObject } from "./types.js";

/** Durable mission-level evaluator/replanning metadata. Raw evaluator instructions stay transient. */
export const AUTONOMOUS_MISSION_REPLAN_SCHEMA = "bioprism-typescript-autonomous-mission-replan/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-mission-replan-checkpoint/0.1" as const;
export const AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS = 3;
export const AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES = 8_192;

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
  checkpoint_digest: string;
  retention: "metadata_only_no_arguments_outputs_credentials_or_provider_material";
  secret_material: "never_returned";
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
}

export interface AutonomousMissionReplanResult {
  schema: typeof AUTONOMOUS_MISSION_REPLAN_SCHEMA;
  status: AutonomousMissionReplanStatus;
  root_mission_id: string;
  protected_contract_digest: string;
  attempts: AutonomousMissionReplanAttempt[];
  evaluations: AutonomousMissionReplanEvaluationProjection[];
  learning_settlements: AutonomousMissionLearningSettlement[];
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
): AutonomousMissionReplanResult {
  return {
    schema: AUTONOMOUS_MISSION_REPLAN_SCHEMA,
    status,
    root_mission_id: rootMissionId,
    protected_contract_digest: protectedContractDigest,
    attempts,
    evaluations,
    learning_settlements: settlements,
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
    retention: checkpoint.retention,
    secret_material: checkpoint.secret_material,
  };
  if (await digestJson(descriptor) !== checkpoint.checkpoint_digest) throw new AutonomousMissionReplanError("mission replan checkpoint digest does not match its metadata");
  return structuredClone(checkpoint);
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
  let current = structuredClone(mission);
  const attempts: AutonomousMissionReplanAttempt[] = [];
  const evaluations: AutonomousMissionReplanEvaluationProjection[] = [];
  const settlements: AutonomousMissionLearningSettlement[] = [];
  let finalExecution: AutonomousMissionExecutionResult | null = null;

  for (let attempt = 1; attempt <= maxReplans + 1; attempt += 1) {
    const execution = await executor.start(current, {
      ...(options.execute ?? {}),
      execution_attempt: attempt,
      signal: options.signal,
    });
    finalExecution = execution;
    if (!isTerminalMission(execution.status)) {
      attempts.push({ attempt, mission_id: current.mission_id, status: execution.status, next_wave: execution.next_wave, completed_steps: execution.completed_steps, succeeded_steps: execution.succeeded_steps, failed_steps: execution.failed_steps, evaluation_digest: null, learning_trajectory_id: null, replan_instruction_digest: null });
      if (options.checkpointSink) await options.checkpointSink(await checkpoint(rootMissionId, protectedContractDigest, attempt, current, execution, "execution_pending", null, null, null));
      return result(execution.status, rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution);
    }

    const evaluation = normalizeEvaluation(await options.evaluate(execution));
    const projection = await evaluationProjection(evaluation);
    evaluations.push(projection);
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

    attempts.push({ attempt, mission_id: current.mission_id, status: execution.status, next_wave: execution.next_wave, completed_steps: execution.completed_steps, succeeded_steps: execution.succeeded_steps, failed_steps: execution.failed_steps, evaluation_digest: projection.evaluation_digest, learning_trajectory_id: trajectoryId, replan_instruction_digest: projection.replan_instruction_digest });
    if (!evaluation.replan_requested) {
      if (options.checkpointSink) await options.checkpointSink(await checkpoint(rootMissionId, protectedContractDigest, attempt, current, execution, "terminal", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId));
      return result(evaluation.passed ? "completed" : "completed_without_replan", rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution);
    }
    if (attempt > maxReplans) {
      if (options.checkpointSink) await options.checkpointSink(await checkpoint(rootMissionId, protectedContractDigest, attempt, current, execution, "terminal", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId));
      return result("replan_limit_reached", rootMissionId, protectedContractDigest, attempts, evaluations, settlements, execution);
    }
    if (current.policy?.allow_side_effects === true && !options.replan) throw new AutonomousMissionReplanError("default mission replanning refuses side-effect-enabled missions; supply an explicit idempotency-aware replanner");

    const proposal = options.replan
      ? await options.replan({ mission: structuredClone(current), execution, evaluation: projection, instruction: evaluation.replan_instruction ?? null, attempt })
      : await defaultReplan(rootMissionId, current, evaluation.replan_instruction as string, attempt);
    const next = await validateProposal(rootMissionId, protectedContractDigest, current, proposal, attempt + 1, executor);
    if (options.checkpointSink) await options.checkpointSink(await checkpoint(rootMissionId, protectedContractDigest, attempt, current, execution, "replan_scheduled", projection.evaluation_digest, projection.replan_instruction_digest, trajectoryId));
    current = next;
  }
  throw new AutonomousMissionReplanError("mission replan cycle exited without a terminal result");
}
