import { ArgumentError, isObject, ProviderRuntimeError } from "./errors.js";
import type { AutonomousPromptChunk } from "./autonomous.js";
import {
  AutonomousWorkflowEvaluator,
  type AutonomousLearningController,
  type AutonomousWorkflowLearningSettlement,
  type AutonomousWorkflowEvaluation,
  type AutonomousWorkflowEvaluationInput,
} from "./autonomous-learning.js";
import {
  type AutonomousWorkflowExecuteOptions,
  type AutonomousWorkflowExecutionResult,
  type AutonomousWorkflowExecutor,
} from "./workflow-execution.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";
import {
  type AutonomousWorkflowCyclePersistencePhase,
  type AutonomousWorkflowCycleRehydrationContext,
  type AutonomousWorkflowCycleAttemptState,
  type AutonomousWorkflowCycleState,
  type AutonomousWorkflowCycleStateStore,
  sealAutonomousWorkflowCycleState,
  validateAutonomousWorkflowCycleState,
} from "./autonomous-workflow-cycle-persistence.js";

/** A bounded supervisor around the durable stage executor and explicit workflow evaluator. */
export const AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA = "bioprism-typescript-autonomous-workflow-cycle/0.1" as const;
export const AUTONOMOUS_WORKFLOW_REPLAN_CONTEXT_SCHEMA = "bioprism-typescript-autonomous-workflow-replan-context/0.1" as const;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_REPLANS = 3;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_INSTRUCTION_BYTES = 8_192;

export type AutonomousWorkflowCycleStatus =
  | "completed"
  | "completed_without_replan"
  | "replan_limit_reached"
  | "approval_required"
  | "paused"
  | "stage_blocked"
  | "stage_proposed"
  | "stage_not_attempted"
  | "failed"
  | "route_review_required";

/** Explicit caller/evaluator evidence plus optional bounded guidance for a new attempt. */
export interface AutonomousWorkflowCycleEvaluationInput extends JsonObject {
  evidence: AutonomousWorkflowEvaluationInput;
  replan_requested?: boolean;
  replan_instruction?: string | null;
  feedback_digest?: string | null;
  failure_class?: string | null;
}

export interface AutonomousWorkflowCycleEvaluationProjection extends JsonObject {
  evaluation_digest: string;
  evidence_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  status: AutonomousWorkflowEvaluation["status"];
  reward: number;
  passed: boolean;
  missing_signals: string[];
  rejected_signals: string[];
  replan_requested: boolean;
  replan_instruction_digest: string | null;
  feedback_digest: string | null;
  failure_class: string | null;
  retention: "evaluator_values_and_digests_only";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowCycleAttempt extends JsonObject {
  attempt: number;
  job_id: string;
  execution_status: AutonomousWorkflowExecutionResult["status"];
  workflow_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evidence_digest: string | null;
  settlement_digest: string | null;
  learning_episode_ids: string[];
  replan_instruction_digest: string | null;
}

export interface AutonomousWorkflowCycleLearningOptions {
  /** Defaults to the learning controller attached to the workflow executor. */
  controller?: AutonomousLearningController;
  trajectoryIdPrefix?: string;
  discount?: number;
  remote?: boolean;
}

export interface AutonomousWorkflowCycleOptions extends Omit<AutonomousWorkflowExecuteOptions, "jobId" | "context"> {
  /** Stable root identity. Each evaluator-guided retry receives a bounded child job id. */
  cycleId?: string;
  /** Root workflow checkpoint identity; defaults to cycleId or a task digest-derived id. */
  jobId?: string;
  maxReplans?: number;
  context?: readonly AutonomousPromptChunk[];
  learning?: AutonomousWorkflowCycleLearningOptions;
  /** Optional metadata-only restart ledger for evaluator and settlement boundaries. */
  stateStore?: AutonomousWorkflowCycleStateStore;
  /** Rehydrate a full local execution when state persisted after provider work but before evaluation. */
  rehydrateExecution?: (context: AutonomousWorkflowCycleRehydrationContext) => AutonomousWorkflowExecutionResult | Promise<AutonomousWorkflowExecutionResult>;
  /** Rehydrate the exact evaluator packet after a settlement interruption. */
  rehydrateEvaluation?: (context: AutonomousWorkflowCycleRehydrationContext) => AutonomousWorkflowCycleEvaluationInput | Promise<AutonomousWorkflowCycleEvaluationInput>;
  /** Rehydrate transient evaluator guidance after a process restart. */
  rehydrateReplanInstruction?: (context: AutonomousWorkflowCycleRehydrationContext) => string | Promise<string>;
  evaluate: (execution: AutonomousWorkflowExecutionResult) => AutonomousWorkflowCycleEvaluationInput | Promise<AutonomousWorkflowCycleEvaluationInput>;
}

export interface AutonomousWorkflowCycleResult {
  schema: typeof AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA;
  status: AutonomousWorkflowCycleStatus;
  final: AutonomousWorkflowExecutionResult | null;
  attempts: AutonomousWorkflowCycleAttempt[];
  evaluations: AutonomousWorkflowCycleEvaluationProjection[];
  settlements: AutonomousWorkflowLearningSettlement[];
  learning_episode_ids: string[];
  replan_count: number;
  retention: "provider_responses_local;workflow_checkpoints_metadata_only;value_only_evaluation_and_learning_projection";
  authorization: "workflow_stage_and_provider_approval_remain_caller_controlled";
}

const RETENTION = "provider_responses_local;workflow_checkpoints_metadata_only;value_only_evaluation_and_learning_projection" as const;
const AUTHORIZATION = "workflow_stage_and_provider_approval_remain_caller_controlled" as const;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedTrajectoryPrefix(value: string): string {
  const bounded = value.length > 220 ? value.slice(0, 220) : value;
  return boundedIdentifier("workflow cycle trajectoryIdPrefix", bounded);
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === undefined || value === null)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function boundedReward(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function safeOptionalLabel(name: string, value: unknown): string | null {
  if (value === undefined || value === null) return null;
  const label = boundedText(name, value, 128);
  if (!/^[A-Za-z0-9_.:-]+$/.test(label)) throw new ArgumentError(`${name} must be a bounded label`);
  return label;
}

function screenReplanInstruction(value: unknown): string {
  const instruction = boundedText("workflow replan instruction", value, AUTONOMOUS_WORKFLOW_CYCLE_MAX_INSTRUCTION_BYTES);
  if (new TextEncoder().encode(instruction).byteLength > AUTONOMOUS_WORKFLOW_CYCLE_MAX_INSTRUCTION_BYTES) throw new ArgumentError("workflow replan instruction exceeds its byte budget");
  if (/(api[_-]?key|authorization|bearer|credential|password|secret|access[_-]?token|refresh[_-]?token|private[_-]?key|gsk_|sk-)/i.test(instruction)) throw new ArgumentError("workflow replan instruction contains credential-shaped material");
  return instruction;
}

function executionStatus(status: AutonomousWorkflowExecutionResult["status"]): AutonomousWorkflowCycleStatus {
  if (status === "route_review_required") return "route_review_required";
  if (status === "approval_required") return "approval_required";
  if (status === "paused") return "paused";
  if (status === "stage_blocked") return "stage_blocked";
  if (status === "stage_proposed") return "stage_proposed";
  if (status === "stage_not_attempted") return "stage_not_attempted";
  return "failed";
}

function workflowJobId(root: string, attempt: number): string {
  const id = attempt === 1 ? root : `${root}:attempt-${attempt}`;
  return boundedIdentifier("workflow cycle job id", id);
}

function normalizeCycleInput(value: unknown): AutonomousWorkflowCycleEvaluationInput {
  if (!isObject(value) || !isObject(value.evidence) || !Array.isArray(value.evidence.stages)) throw new ArgumentError("workflow cycle evaluator must return an evidence packet");
  if (value.replan_requested !== undefined && typeof value.replan_requested !== "boolean") throw new ArgumentError("workflow cycle replan_requested must be boolean");
  const replanRequested = value.replan_requested === true;
  const instruction = value.replan_instruction === undefined || value.replan_instruction === null ? null : screenReplanInstruction(value.replan_instruction);
  if (replanRequested && instruction === null) throw new ArgumentError("workflow cycle replan_requested requires replan_instruction");
  if (!replanRequested && instruction !== null) throw new ArgumentError("workflow cycle replan_instruction requires replan_requested");
  const feedbackDigest = boundedDigest("workflow cycle feedback_digest", value.feedback_digest, true);
  const failureClass = safeOptionalLabel("workflow cycle failure_class", value.failure_class);
  return { evidence: value.evidence as AutonomousWorkflowEvaluationInput, replan_requested: replanRequested, replan_instruction: instruction, feedback_digest: feedbackDigest, failure_class: failureClass };
}

async function cycleProjection(evaluation: AutonomousWorkflowEvaluation, input: AutonomousWorkflowCycleEvaluationInput): Promise<AutonomousWorkflowCycleEvaluationProjection> {
  return {
    evaluation_digest: evaluation.evaluation_digest,
    evidence_digest: evaluation.evidence_digest,
    evaluator_id: evaluation.evaluator_id,
    evaluator_version: evaluation.evaluator_version,
    status: evaluation.status,
    reward: boundedReward("workflow cycle reward", evaluation.reward),
    passed: evaluation.passed,
    missing_signals: [...evaluation.missing_signals],
    rejected_signals: [...evaluation.rejected_signals],
    replan_requested: input.replan_requested === true,
    replan_instruction_digest: input.replan_instruction ? await digestJson(input.replan_instruction) : null,
    feedback_digest: input.feedback_digest ?? null,
    failure_class: input.failure_class ?? null,
    retention: "evaluator_values_and_digests_only",
    secret_material: "never_returned",
  };
}

async function replanContext(attempt: number, execution: AutonomousWorkflowExecutionResult, evaluation: AutonomousWorkflowEvaluation, input: AutonomousWorkflowCycleEvaluationInput): Promise<AutonomousPromptChunk> {
  if (!input.replan_instruction) throw new ArgumentError("workflow replan context requires an instruction");
  const content = JSON.stringify({
    schema: AUTONOMOUS_WORKFLOW_REPLAN_CONTEXT_SCHEMA,
    attempt,
    prior: {
      job_id: execution.job_id,
      workflow_digest: execution.checkpoint?.workflow_digest ?? null,
      evaluation_digest: evaluation.evaluation_digest,
      evidence_digest: evaluation.evidence_digest,
    },
    evaluator: {
      status: evaluation.status,
      reward: evaluation.reward,
      passed: evaluation.passed,
      missing_signals: evaluation.missing_signals,
      rejected_signals: evaluation.rejected_signals,
      feedback_digest: input.feedback_digest ?? null,
      failure_class: input.failure_class ?? null,
    },
    instruction: input.replan_instruction,
    guardrails: [
      "This is bounded evaluator feedback, not a new authorization.",
      "Preserve the reviewed domain, workflow stages, dependencies, tool allow-list, budgets, and approval gates.",
      "Do not treat the prior provider response or evaluator signal as verified external truth.",
    ],
  });
  return { id: `autonomous-workflow-replan-${attempt}`, content, required: true, priority: 95 };
}

function result(status: AutonomousWorkflowCycleStatus, final: AutonomousWorkflowExecutionResult | null, attempts: AutonomousWorkflowCycleAttempt[], evaluations: AutonomousWorkflowCycleEvaluationProjection[], settlements: AutonomousWorkflowLearningSettlement[], learningEpisodeIds: string[]): AutonomousWorkflowCycleResult {
  return {
    schema: AUTONOMOUS_WORKFLOW_CYCLE_SCHEMA,
    status,
    final,
    attempts,
    evaluations,
    settlements,
    learning_episode_ids: learningEpisodeIds,
    replan_count: Math.max(0, attempts.length - 1),
    retention: RETENTION,
    authorization: AUTHORIZATION,
  };
}

interface CyclePersistenceRuntime {
  readonly store: AutonomousWorkflowCycleStateStore;
  readonly cycleId: string;
  readonly taskDigest: string;
  readonly rootJobId: string;
  readonly maxReplans: number;
  state: AutonomousWorkflowCycleState;
}

function rehydrationContext(runtime: CyclePersistenceRuntime): AutonomousWorkflowCycleRehydrationContext {
  const state = runtime.state;
  return {
    cycle_id: runtime.cycleId,
    task_digest: runtime.taskDigest,
    root_job_id: runtime.rootJobId,
    current_job_id: state.current_job_id,
    attempt: state.attempt,
    phase: state.phase,
    workflow_digest: state.workflow_digest,
    outcome_digest: state.outcome_digest,
    evaluation_digest: state.evaluation_digest,
    evidence_digest: state.evidence_digest,
    replan_instruction_digest: state.replan_instruction_digest,
  };
}

async function openCyclePersistence(options: AutonomousWorkflowCycleOptions, task: string, rootJobId: string, maxReplans: number): Promise<CyclePersistenceRuntime | null> {
  if (!options.stateStore) {
    if (options.cycleId !== undefined) boundedIdentifier("workflow cycle cycleId", options.cycleId);
    return null;
  }
  if (options.cycleId === undefined) throw new ArgumentError("workflow cycle stateStore requires cycleId");
  const cycleId = boundedIdentifier("workflow cycle cycleId", options.cycleId);
  const taskDigest = await digestJson(task);
  const loaded = await options.stateStore.load(cycleId);
  if (loaded) {
    const state = await validateAutonomousWorkflowCycleState(loaded);
    if (state.cycle_id !== cycleId || state.task_digest !== taskDigest || state.root_job_id !== rootJobId || state.max_replans !== maxReplans || (options.domain !== undefined && state.domain !== null && state.domain !== options.domain)) throw new ArgumentError("persisted workflow cycle state does not match the requested cycle contract");
    return { store: options.stateStore, cycleId, taskDigest, rootJobId, maxReplans, state };
  }
  const initial = await sealAutonomousWorkflowCycleState({
    schema: "bioprism-typescript-autonomous-workflow-cycle-state/0.1",
    cycle_id: cycleId,
    task_digest: taskDigest,
    domain: options.domain ?? null,
    root_job_id: rootJobId,
    current_job_id: rootJobId,
    max_replans: maxReplans,
    attempt: 1,
    phase: "execution_pending",
    execution_status: null,
    workflow_digest: null,
    outcome_digest: null,
    evaluation_digest: null,
    evidence_digest: null,
    replan_instruction_digest: null,
    terminal_status: null,
    attempts: [],
    evaluations: [],
    learning_episode_ids: [],
    settlement_digests: [],
    trajectory_ids: [],
    context_digests: [],
    generation: 1,
    previous_state_digest: null,
    retention: "metadata_only_hash_chained_no_private_payloads",
    secret_material: "never_returned",
  });
  await options.stateStore.save(initial);
  return { store: options.stateStore, cycleId, taskDigest, rootJobId, maxReplans, state: initial };
}

async function commitCyclePersistence(runtime: CyclePersistenceRuntime | null, changes: Partial<Omit<AutonomousWorkflowCycleState, "state_digest" | "generation" | "previous_state_digest">>): Promise<void> {
  if (!runtime) return;
  const { state_digest: _stateDigest, generation: priorGeneration, previous_state_digest: _previous, ...descriptor } = runtime.state;
  const next = await sealAutonomousWorkflowCycleState({
    ...descriptor,
    ...changes,
    generation: priorGeneration + 1,
    previous_state_digest: runtime.state.state_digest,
  });
  await runtime.store.save(next);
  runtime.state = next;
}

async function workflowExecutionDigest(execution: AutonomousWorkflowExecutionResult): Promise<string> {
  const stageResults = await Promise.all(execution.stage_results.map(async (stage) => ({
    stage_id: stage.stage.id,
    output_digest: stage.output_digest,
    output_bytes: stage.output_bytes,
    declared_status: stage.declared_status,
    evidence_digest: await digestJson(stage.evidence),
    uncertainty_digest: await digestJson(stage.uncertainty),
    notes_digest: await digestJson(stage.notes),
    next_actions_digest: await digestJson(stage.next_actions),
    validation_errors: [...stage.validation_errors],
  })));
  return digestJson({
    schema: "bioprism-typescript-autonomous-workflow-cycle-execution-digest/0.1",
    status: execution.status,
    job_id: execution.job_id,
    checkpoint_digest: execution.checkpoint?.checkpoint_digest ?? null,
    workflow_digest: execution.checkpoint?.workflow_digest ?? execution.blueprint?.workflow.workflow_digest ?? null,
    stage_results: stageResults,
    learning_episode_ids: [...execution.learning_episode_ids],
  });
}

function persistedAttempt(value: AutonomousWorkflowCycleAttempt): AutonomousWorkflowCycleAttemptState {
  return {
    attempt: value.attempt,
    job_id: value.job_id,
    execution_status: value.execution_status,
    workflow_digest: value.workflow_digest,
    outcome_digest: value.outcome_digest,
    evaluation_digest: value.evaluation_digest,
    evidence_digest: value.evidence_digest,
    settlement_digest: value.settlement_digest,
    learning_episode_ids: [...value.learning_episode_ids],
    replan_instruction_digest: value.replan_instruction_digest,
  };
}

function persistedAttempts(state: AutonomousWorkflowCycleState): AutonomousWorkflowCycleAttempt[] {
  return state.attempts.map((attempt) => ({
    attempt: attempt.attempt,
    job_id: attempt.job_id,
    execution_status: attempt.execution_status as AutonomousWorkflowExecutionResult["status"],
    workflow_digest: attempt.workflow_digest,
    evaluation_digest: attempt.evaluation_digest,
    evidence_digest: attempt.evidence_digest,
    outcome_digest: attempt.outcome_digest,
    settlement_digest: attempt.settlement_digest,
    learning_episode_ids: [...attempt.learning_episode_ids],
    replan_instruction_digest: attempt.replan_instruction_digest,
  }));
}

function persistedEvaluations(state: AutonomousWorkflowCycleState): AutonomousWorkflowCycleEvaluationProjection[] {
  return state.evaluations.map((evaluation) => ({ ...evaluation })) as unknown as AutonomousWorkflowCycleEvaluationProjection[];
}

function persistedResult(state: AutonomousWorkflowCycleState): AutonomousWorkflowCycleResult {
  const status = state.terminal_status && ["completed", "completed_without_replan", "replan_limit_reached", "approval_required", "paused", "stage_blocked", "stage_proposed", "stage_not_attempted", "failed", "route_review_required"].includes(state.terminal_status)
    ? state.terminal_status as AutonomousWorkflowCycleStatus
    : "failed";
  return result(status, null, persistedAttempts(state), persistedEvaluations(state), [], [...state.learning_episode_ids]);
}

async function rehydrateInstruction(runtime: CyclePersistenceRuntime, options: AutonomousWorkflowCycleOptions): Promise<string> {
  if (!options.rehydrateReplanInstruction || runtime.state.replan_instruction_digest === null) throw new ArgumentError("workflow cycle restart requires rehydrateReplanInstruction for the transient evaluator handoff");
  const instruction = screenReplanInstruction(await options.rehydrateReplanInstruction(rehydrationContext(runtime)));
  if (await digestJson(instruction) !== runtime.state.replan_instruction_digest) throw new ArgumentError("rehydrated workflow cycle instruction does not match its persisted digest");
  return instruction;
}

function replanContextFromProjection(attempt: number, runtime: CyclePersistenceRuntime, projection: AutonomousWorkflowCycleEvaluationProjection, instruction: string): AutonomousPromptChunk {
  const content = JSON.stringify({
    schema: AUTONOMOUS_WORKFLOW_REPLAN_CONTEXT_SCHEMA,
    attempt,
    prior: { job_id: runtime.state.current_job_id, workflow_digest: runtime.state.workflow_digest, evaluation_digest: projection.evaluation_digest, evidence_digest: projection.evidence_digest },
    evaluator: { status: projection.status, reward: projection.reward, passed: projection.passed, missing_signals: projection.missing_signals, rejected_signals: projection.rejected_signals, feedback_digest: projection.feedback_digest, failure_class: projection.failure_class },
    instruction,
    guardrails: [
      "This is bounded evaluator feedback, not a new authorization.",
      "Preserve the reviewed domain, workflow stages, dependencies, tool allow-list, budgets, and approval gates.",
      "Do not treat the prior provider response or evaluator signal as verified external truth.",
    ],
  });
  return { id: `autonomous-workflow-replan-${attempt}`, content, required: true, priority: 95 };
}

/**
 * Execute a durable workflow under an explicit evaluator and optional online-learning
 * settlement. Replans create fresh, bounded workflow checkpoints so the previous attempt's
 * stage contract cannot be silently mutated. Only the evaluator may request a retry, and its
 * instruction is transient, screened, and never written to workflow checkpoint metadata.
 */
export async function runAutonomousWorkflowCycle(task: string, executor: AutonomousWorkflowExecutor, options: AutonomousWorkflowCycleOptions): Promise<AutonomousWorkflowCycleResult> {
  if (!executor || typeof executor.start !== "function") throw new ArgumentError("workflow cycle requires an AutonomousWorkflowExecutor");
  if (!options || typeof options.evaluate !== "function") throw new ArgumentError("workflow cycle requires an evaluator callback");
  const taskText = boundedText("workflow cycle task", task, 32_000);
  const maxReplans = boundedCount("workflow cycle maxReplans", options.maxReplans ?? 0, AUTONOMOUS_WORKFLOW_CYCLE_MAX_REPLANS);
  const rootJobId = boundedIdentifier("workflow cycle jobId", options.jobId ?? options.cycleId ?? `workflow-cycle-${(await digestJson(taskText)).slice(0, 24)}`);
  if (maxReplans > 0 && rootJobId.length > 240) throw new ArgumentError("workflow cycle jobId is too long for bounded retry identities");
  const learning = options.learning?.controller ?? executor.learning;
  const trajectoryPrefix = boundedTrajectoryPrefix(options.learning?.trajectoryIdPrefix ?? `workflow-cycle:${rootJobId}`);
  const persistence = await openCyclePersistence(options, taskText, rootJobId, maxReplans);
  if (persistence?.state.phase === "terminal") return persistedResult(persistence.state);
  const attempts: AutonomousWorkflowCycleAttempt[] = persistence ? persistedAttempts(persistence.state) : [];
  const evaluations: AutonomousWorkflowCycleEvaluationProjection[] = persistence ? persistedEvaluations(persistence.state) : [];
  const settlements: AutonomousWorkflowLearningSettlement[] = [];
  const learningEpisodeIds: string[] = persistence ? [...persistence.state.learning_episode_ids] : [];
  let context = [...(options.context ?? [])];
  let final: AutonomousWorkflowExecutionResult | null = null;
  let domain = options.domain;
  let startAttempt = 1;
  if (persistence && persistence.state.domain !== null && domain === undefined) domain = persistence.state.domain as AutonomousWorkflowCycleOptions["domain"];
  if (persistence?.state.phase === "replan_handoff") {
    if (persistence.state.attempt >= maxReplans + 1) throw new ArgumentError("persisted workflow cycle replan handoff exceeds its attempt limit");
    const projection = evaluations.at(-1);
    if (!projection) throw new ArgumentError("persisted workflow cycle handoff is missing its evaluation projection");
    const instruction = await rehydrateInstruction(persistence, options);
    context = [...context, replanContextFromProjection(persistence.state.attempt + 1, persistence, projection, instruction)];
    startAttempt = persistence.state.attempt + 1;
  } else if (persistence) {
    startAttempt = persistence.state.attempt;
    if (persistence.state.phase === "execution_pending" && persistence.state.replan_instruction_digest !== null) {
      const projection = evaluations.at(-1);
      if (!projection) throw new ArgumentError("persisted workflow cycle execution handoff is missing its evaluation projection");
      const instruction = await rehydrateInstruction(persistence, options);
      context = [...context, replanContextFromProjection(persistence.state.attempt, persistence, projection, instruction)];
    }
    if (persistence.state.phase === "evaluation_pending" || persistence.state.phase === "settlement_pending") {
      if (!options.rehydrateExecution) throw new ArgumentError("workflow cycle restart requires rehydrateExecution for the persisted provider outcome");
    }
  }
  const {
    cycleId: _cycleId,
    jobId: _jobId,
    context: _context,
    learning: _learning,
    evaluate: _evaluate,
    maxReplans: _maxReplans,
    stateStore: _stateStore,
    rehydrateExecution: _rehydrateExecution,
    rehydrateEvaluation: _rehydrateEvaluation,
    rehydrateReplanInstruction: _rehydrateReplanInstruction,
    ...workflowBaseOptions
  } = options;

  for (let attemptNumber = startAttempt; attemptNumber <= maxReplans + 1; attemptNumber += 1) {
    const jobId = workflowJobId(rootJobId, attemptNumber);
    const phaseAtEntry: AutonomousWorkflowCyclePersistencePhase | null = persistence && persistence.state.attempt === attemptNumber ? persistence.state.phase : null;
    if (persistence && persistence.state.phase === "replan_handoff" && persistence.state.attempt + 1 === attemptNumber) {
      await commitCyclePersistence(persistence, {
        attempt: attemptNumber,
        current_job_id: jobId,
        phase: "execution_pending",
        execution_status: null,
        workflow_digest: null,
        outcome_digest: null,
        evaluation_digest: null,
        evidence_digest: null,
        terminal_status: null,
      });
    }
    const executionOptions: AutonomousWorkflowExecuteOptions = {
      ...workflowBaseOptions,
      ...(domain === undefined ? {} : { domain }),
      jobId,
      context,
    };
    let execution: AutonomousWorkflowExecutionResult;
    if (persistence && (phaseAtEntry === "evaluation_pending" || phaseAtEntry === "settlement_pending")) {
      if (persistence.state.current_job_id !== jobId || !options.rehydrateExecution) throw new ArgumentError("persisted workflow cycle job identity is not rehydratable");
      execution = await options.rehydrateExecution(rehydrationContext(persistence));
      const observedDigest = await workflowExecutionDigest(execution);
      if (execution.job_id !== jobId || observedDigest !== persistence.state.outcome_digest || (execution.checkpoint?.workflow_digest ?? null) !== persistence.state.workflow_digest) throw new ArgumentError("rehydrated workflow execution does not match the persisted cycle outcome");
    } else {
      execution = await executor.start(taskText, executionOptions);
      if (persistence && phaseAtEntry === "execution_pending" && execution.status === "completed" && execution.checkpoint?.status === "completed" && execution.stage_results.length === 0) {
        if (!options.rehydrateExecution) throw new ArgumentError("workflow cycle restart requires rehydrateExecution when a completed checkpoint has no local stage results");
        execution = await options.rehydrateExecution(rehydrationContext(persistence));
        if (execution.job_id !== jobId) throw new ArgumentError("rehydrated workflow execution job identity does not match the pending cycle attempt");
      } else if (persistence && execution.status === "completed" && execution.checkpoint?.status === "completed" && execution.stage_results.length === 0) {
        throw new ArgumentError("workflow cycle restart requires rehydrateExecution when a completed checkpoint has no local stage results");
      }
    }
    final = execution;
    const executionAttempt: AutonomousWorkflowCycleAttempt = {
      attempt: attemptNumber,
      job_id: jobId,
      execution_status: execution.status,
      workflow_digest: execution.checkpoint?.workflow_digest ?? null,
      outcome_digest: null,
      evaluation_digest: null,
      evidence_digest: null,
      settlement_digest: null,
      learning_episode_ids: [...execution.learning_episode_ids],
      replan_instruction_digest: null,
    };
    for (const episodeId of execution.learning_episode_ids) if (!learningEpisodeIds.includes(episodeId)) learningEpisodeIds.push(episodeId);

    if (!execution.blueprint || !execution.stage_results.length || execution.status === "approval_required" || execution.status === "route_review_required") {
      const stateAttempts = persistence ? [...persistence.state.attempts.filter((attempt) => attempt.attempt !== attemptNumber), persistedAttempt(executionAttempt)] : [];
      if (persistence) {
        await commitCyclePersistence(persistence, {
          current_job_id: jobId,
          attempt: attemptNumber,
          phase: "terminal",
          execution_status: execution.status,
          workflow_digest: executionAttempt.workflow_digest,
          outcome_digest: await workflowExecutionDigest(execution),
          evaluation_digest: null,
          evidence_digest: null,
          replan_instruction_digest: null,
          terminal_status: executionStatus(execution.status),
          attempts: stateAttempts,
        });
      }
      const index = attempts.findIndex((attempt) => attempt.attempt === attemptNumber);
      if (index >= 0) attempts[index] = executionAttempt;
      else attempts.push(executionAttempt);
      return result(executionStatus(execution.status), final, attempts, evaluations, settlements, learningEpisodeIds);
    }

    const outcomeDigest = await workflowExecutionDigest(execution);
    executionAttempt.outcome_digest = outcomeDigest;
    if (persistence && phaseAtEntry !== "evaluation_pending" && phaseAtEntry !== "settlement_pending") {
      const stateAttempts = [...persistence.state.attempts.filter((attempt) => attempt.attempt !== attemptNumber), persistedAttempt(executionAttempt)];
      await commitCyclePersistence(persistence, {
        current_job_id: jobId,
        attempt: attemptNumber,
        phase: "evaluation_pending",
        execution_status: execution.status,
        workflow_digest: executionAttempt.workflow_digest,
        outcome_digest: outcomeDigest,
        evaluation_digest: null,
        evidence_digest: null,
        replan_instruction_digest: null,
        terminal_status: null,
        attempts: stateAttempts,
        domain: domain ?? execution.blueprint.domain_profile.domain,
      });
      if (domain === undefined) domain = execution.blueprint.domain_profile.domain;
    } else if (persistence && (persistence.state.outcome_digest !== outcomeDigest || persistence.state.current_job_id !== jobId)) {
      throw new ArgumentError("workflow cycle execution digest changed during restart rehydration");
    }

    const resumedSettlement = phaseAtEntry === "settlement_pending";
    const input = normalizeCycleInput(resumedSettlement
      ? await (options.rehydrateEvaluation ? options.rehydrateEvaluation(rehydrationContext(persistence!)) : Promise.reject(new ArgumentError("workflow cycle restart requires rehydrateEvaluation after settlement interruption")))
      : await options.evaluate(execution));
    const evaluator = learning?.evaluator ?? new AutonomousWorkflowEvaluator();
    const evaluation = await evaluator.evaluate(execution, input.evidence);
    const projection = await cycleProjection(evaluation, input);
    if (resumedSettlement && persistence) {
      const priorProjection = evaluations.at(-1);
      if (persistence.state.evaluation_digest !== projection.evaluation_digest || persistence.state.evidence_digest !== projection.evidence_digest || persistence.state.replan_instruction_digest !== projection.replan_instruction_digest || (priorProjection && priorProjection.evaluation_digest !== projection.evaluation_digest)) throw new ArgumentError("rehydrated workflow evaluator packet does not match the persisted cycle evaluation");
    }
    executionAttempt.evaluation_digest = evaluation.evaluation_digest;
    executionAttempt.evidence_digest = evaluation.evidence_digest;
    executionAttempt.replan_instruction_digest = projection.replan_instruction_digest;
    const stateEvaluations = persistence
      ? [...persistence.state.evaluations.slice(0, Math.max(0, attemptNumber - 1)), projection]
      : [];
    const stateAttemptAfterEvaluation = persistedAttempt(executionAttempt);
    if (persistence && !resumedSettlement) {
      await commitCyclePersistence(persistence, {
        phase: "settlement_pending",
        execution_status: execution.status,
        workflow_digest: executionAttempt.workflow_digest,
        outcome_digest: outcomeDigest,
        evaluation_digest: evaluation.evaluation_digest,
        evidence_digest: evaluation.evidence_digest,
        replan_instruction_digest: projection.replan_instruction_digest,
        attempts: [...persistence.state.attempts.filter((attempt) => attempt.attempt !== attemptNumber), stateAttemptAfterEvaluation],
        evaluations: stateEvaluations,
      });
    }
    if (learning && execution.learning_episode_ids.length > 0) {
      const trajectoryId = `${trajectoryPrefix}:attempt-${attemptNumber}`;
      const settlement = await learning.settleWorkflow(execution, input.evidence, {
        trajectoryId: boundedIdentifier("workflow cycle trajectory id", trajectoryId),
        discount: options.learning?.discount,
        remote: options.learning?.remote,
        idempotencyKey: boundedIdentifier("workflow cycle settlement idempotency key", trajectoryId),
      });
      executionAttempt.settlement_digest = await digestJson(settlement);
      settlements.push(settlement);
    }
    const attemptIndex = attempts.findIndex((attempt) => attempt.attempt === attemptNumber);
    if (attemptIndex >= 0) attempts[attemptIndex] = executionAttempt;
    else attempts.push(executionAttempt);
    if (evaluations.length >= attemptNumber) evaluations[attemptNumber - 1] = projection;
    else evaluations.push(projection);

    const wantsReplan = input.replan_requested === true;
    const terminal = execution.status === "completed" ? (evaluation.passed ? "completed" : "completed_without_replan") : executionStatus(execution.status);
    const shouldReplan = wantsReplan && attemptNumber <= maxReplans;
    if (persistence) {
      const stateLearningEpisodeIds = [...new Set([...persistence.state.learning_episode_ids, ...execution.learning_episode_ids])];
      const settlementDigest = executionAttempt.settlement_digest;
      const stateSettlementDigests = settlementDigest && !persistence.state.settlement_digests.includes(settlementDigest) ? [...persistence.state.settlement_digests, settlementDigest] : [...persistence.state.settlement_digests];
      const trajectoryId = execution.learning_episode_ids.length > 0 ? boundedIdentifier("workflow cycle trajectory id", `${trajectoryPrefix}:attempt-${attemptNumber}`) : null;
      const stateTrajectoryIds = trajectoryId && !persistence.state.trajectory_ids.includes(trajectoryId) ? [...persistence.state.trajectory_ids, trajectoryId] : [...persistence.state.trajectory_ids];
      const stateAttempts = [...persistence.state.attempts.filter((attempt) => attempt.attempt !== attemptNumber), persistedAttempt(executionAttempt)];
      if (shouldReplan) {
        const nextContext = await replanContext(attemptNumber + 1, execution, evaluation, input);
        const nextContextDigest = await digestJson({ id: nextContext.id, content: nextContext.content, priority: nextContext.priority ?? null, required: nextContext.required ?? false });
        await commitCyclePersistence(persistence, {
          attempt: attemptNumber,
          phase: "replan_handoff",
          current_job_id: jobId,
          execution_status: execution.status,
          workflow_digest: executionAttempt.workflow_digest,
          outcome_digest: outcomeDigest,
          evaluation_digest: evaluation.evaluation_digest,
          evidence_digest: evaluation.evidence_digest,
          replan_instruction_digest: projection.replan_instruction_digest,
          terminal_status: null,
          attempts: stateAttempts,
          evaluations: stateEvaluations.length ? stateEvaluations : [...persistence.state.evaluations, projection],
          learning_episode_ids: stateLearningEpisodeIds,
          settlement_digests: stateSettlementDigests,
          trajectory_ids: stateTrajectoryIds,
          context_digests: [...persistence.state.context_digests, nextContextDigest],
          domain: domain ?? execution.blueprint.domain_profile.domain,
        });
        context = [...context, nextContext];
      } else {
        await commitCyclePersistence(persistence, {
          attempt: attemptNumber,
          phase: "terminal",
          current_job_id: jobId,
          execution_status: execution.status,
          workflow_digest: executionAttempt.workflow_digest,
          outcome_digest: outcomeDigest,
          evaluation_digest: evaluation.evaluation_digest,
          evidence_digest: evaluation.evidence_digest,
          replan_instruction_digest: projection.replan_instruction_digest,
          terminal_status: wantsReplan ? "replan_limit_reached" : terminal,
          attempts: stateAttempts,
          evaluations: stateEvaluations.length ? stateEvaluations : [...persistence.state.evaluations, projection],
          learning_episode_ids: stateLearningEpisodeIds,
          settlement_digests: stateSettlementDigests,
          trajectory_ids: stateTrajectoryIds,
          domain: domain ?? execution.blueprint.domain_profile.domain,
        });
      }
    }
    if (!wantsReplan) {
      return result(terminal, final, attempts, evaluations, settlements, learningEpisodeIds);
    }
    if (attemptNumber > maxReplans) return result("replan_limit_reached", final, attempts, evaluations, settlements, learningEpisodeIds);

    if (domain === undefined && execution.blueprint.domain_profile.domain) domain = execution.blueprint.domain_profile.domain;
    if (!persistence) context = [...context, await replanContext(attemptNumber + 1, execution, evaluation, input)];
  }

  throw new ProviderRuntimeError("workflow cycle exited without a terminal result");
}
