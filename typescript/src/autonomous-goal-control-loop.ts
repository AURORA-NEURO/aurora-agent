import { ArgumentError, isObject } from "./errors.js";
import { AutonomousGoalWorker, type AutonomousGoalWorkerBatch } from "./autonomous-goal-worker.js";
import type { AutonomousGoalRecord, InMemoryAutonomousGoalLedger } from "./autonomous-goals.js";
import type { AutonomousGoalSchedulingSignal } from "./autonomous-goal-scheduler.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import {
  sealAutonomousGoalControlLoopSnapshot,
  validateAutonomousGoalControlLoopSnapshot,
  type AutonomousGoalControlLoopCheckpoint,
} from "./autonomous-goal-control-persistence.js";

/** Bounded autonomous continuation over scheduler/worker cycles. */
export const AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA = "bioprism-autonomous-goal-control-loop/0.1" as const;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION = "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES = 128;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS = 8_192;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_BATCH_PREFIX_BYTES = 128;
export const AUTONOMOUS_GOAL_CONTROL_EVALUATION_SCHEMA = "bioprism-autonomous-goal-control-evaluation/0.1" as const;
export const AUTONOMOUS_GOAL_CONTROL_BANDIT_SCHEMA = "bioprism-autonomous-goal-control-bandit/0.1" as const;
export const AUTONOMOUS_GOAL_CONTROL_MAX_EVALUATIONS = 128;
export const AUTONOMOUS_GOAL_CONTROL_MAX_SIGNALS = 4_096;

export type AutonomousGoalControlLoopStopReason = "all_terminal" | "no_admissible_work" | "cycle_budget_exhausted" | "run_budget_exhausted";

export interface AutonomousGoalControlLoopContext extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA;
  cycle: number;
  previous_cycle: AutonomousGoalControlLoopCycleJSON | null;
  ledger_stats: JsonObject;
  retention: typeof AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION;
  secret_material: "never_returned";
}

export type AutonomousGoalControlLoopOptionsFactory = (context: AutonomousGoalControlLoopContext) => Record<string, unknown> | Promise<Record<string, unknown>>;

export interface AutonomousGoalEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_CONTROL_EVALUATION_SCHEMA;
  goal_id: string;
  domain: string;
  attempt: number;
  outcome_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  evidence_digest: string | null;
  failure_class: string | null;
  feedback_digest: string;
  retention: "metadata_only_explicit_evaluator_credit";
  secret_material: "never_returned";
}

export type AutonomousGoalControlLoopEvaluator = (cycle: AutonomousGoalControlLoopCycle) => unknown | Promise<unknown>;
export type AutonomousGoalControlLoopLearner = (evaluations: readonly AutonomousGoalEvaluation[], goals: readonly AutonomousGoalRecord[]) => Record<string, unknown> | Promise<Record<string, unknown>>;

export interface AutonomousGoalControlLoopCycleJSON extends JsonObject {
  cycle: number;
  schedule_digest: string;
  claim_digest: string | null;
  worker_digest: string;
  selected: number;
  claimed: number;
  runs: number;
  counts: {
    selected: number;
    claimed: number;
    settled: number;
    completed: number;
    paused: number;
    blocked: number;
    failed: number;
  };
  selected_domains: string[];
  missing_domains: string[];
  evaluated?: number;
  evaluation_digest?: string;
  learning_state_digest?: string;
  signals_digest?: string;
  retention: typeof AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousGoalControlLoopJSON extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA;
  cycles: AutonomousGoalControlLoopCycleJSON[];
  stop_reason: AutonomousGoalControlLoopStopReason;
  total_selected: number;
  total_claimed: number;
  total_runs: number;
  status_counts: Record<string, number>;
  domain_counts: Record<string, number>;
  evaluation_count?: number;
  evaluation_digest?: string;
  learning_state_digest?: string;
  restored_cycle_count?: number;
  cycle_history_digest?: string | null;
  retention: typeof AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION;
  secret_material: "never_returned";
  loop_digest: string;
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal control loop ${message}`);
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bounds`);
  return value;
}

function prefix(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_BATCH_PREFIX_BYTES) fail("batch_id_prefix is outside its bounded contract");
  return value.trim();
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function hasEligibleWork(ledger: InMemoryAutonomousGoalLedger, includePaused: boolean, allowFailedRetry: boolean): boolean {
  const rawCounts = ledger.stats().statuses;
  const counts = isObject(rawCounts) ? rawCounts : {};
  if (typeof counts.ready === "number" && counts.ready > 0) return true;
  if (includePaused && typeof counts.paused === "number" && counts.paused > 0) return true;
  // Aggregate status does not expose each failed record's remaining attempt budget.  A failed
  // row is conservatively treated as potentially retryable; exhausted failures remain an
  // explicit no_admissible_work stop rather than being mislabeled as completion.
  return allowFailedRetry && typeof counts.failed === "number" && counts.failed > 0;
}

function allTerminal(ledger: InMemoryAutonomousGoalLedger): boolean {
  const rawCounts = ledger.stats().statuses;
  if (!isObject(rawCounts)) return false;
  const keys = Object.keys(rawCounts);
  return keys.length > 0 && keys.every((key) => key === "completed" || key === "cancelled");
}

function identifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) fail(`${name} is outside its bounded identifier contract`);
  return value.trim();
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function normalizeEvaluation(value: unknown, run: AutonomousGoalWorkerBatch["runs"][number], goal: AutonomousGoalRecord): AutonomousGoalEvaluation {
  if (!isObject(value)) fail("evaluator output must be an object");
  const allowed = new Set(["goal_id", "evaluator_id", "evaluator_version", "reward", "passed", "evidence_digest", "failure_class", "feedback_digest"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) fail("evaluator output contains unsupported fields");
  const goalId = identifier("goal_id", value.goal_id ?? run.goal_id);
  if (goalId !== run.goal_id || goalId !== goal.goal_id) fail(`evaluator output goal_id does not match ${run.goal_id}`);
  const reward = finite("evaluator reward", value.reward, -1, 1);
  if (typeof value.passed !== "boolean") fail("evaluator passed must be boolean");
  const evaluatorId = identifier("evaluator_id", value.evaluator_id, 128);
  const evaluatorVersion = identifier("evaluator_version", value.evaluator_version, 128);
  const evidenceDigest = digest("evidence_digest", value.evidence_digest ?? null, true);
  const failureClass = value.failure_class === undefined || value.failure_class === null ? null : identifier("failure_class", value.failure_class, 128);
  const body = {
    schema: AUTONOMOUS_GOAL_CONTROL_EVALUATION_SCHEMA,
    goal_id: goalId,
    domain: goal.domain,
    attempt: run.attempt,
    outcome_digest: digest("outcome_digest", run.outcome_digest)!,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    evidence_digest: evidenceDigest,
    failure_class: failureClass,
  };
  const feedbackDigest = value.feedback_digest === undefined || value.feedback_digest === null
    ? digestJsonSync(body)
    : digest("feedback_digest", value.feedback_digest)!;
  return {
    ...body,
    feedback_digest: feedbackDigest,
    retention: "metadata_only_explicit_evaluator_credit",
    secret_material: "never_returned",
  };
}

function normalizeLearningSignal(value: unknown, index: number): AutonomousGoalSchedulingSignal {
  if (!isObject(value)) fail(`learner signal ${index} is malformed`);
  const dependencies = value.dependencies ?? [];
  if (!Array.isArray(dependencies) || dependencies.length > 64) fail(`learner signal ${index}.dependencies is outside its bounds`);
  return {
    goal_id: identifier(`learner signal ${index}.goal_id`, value.goal_id),
    priority: finite(`learner signal ${index}.priority`, value.priority ?? 0.5, 0, 1),
    urgency: finite(`learner signal ${index}.urgency`, value.urgency ?? 0, 0, 1),
    deadline_ns: value.deadline_ns === undefined || value.deadline_ns === null ? null : integer(`learner signal ${index}.deadline_ns`, value.deadline_ns, 0, Number.MAX_SAFE_INTEGER),
    estimated_cost: integer(`learner signal ${index}.estimated_cost`, value.estimated_cost ?? 1, 1, 1_000_000),
    dependencies: [...new Set(dependencies.map((item, dependencyIndex) => identifier(`learner signal ${index}.dependencies[${dependencyIndex}]`, item)))].sort(),
  };
}

type BanditArm = { pulls: number; failures: number; reward_sum: number };

/** Value-only UCB-style domain adaptation driven only by explicit evaluator rewards. */
export class AutonomousGoalBanditLearner {
  private generationValue = 0;
  private readonly arms = new Map<string, BanditArm>();
  exploration: number;

  constructor(options: { state?: JsonObject; exploration?: number } = {}) {
    this.exploration = finite("bandit exploration", options.exploration ?? 0.35, 0, 2);
    if (options.state !== undefined) this.restore(options.state);
  }

  restore(state: JsonObject): void {
    if (state.schema !== AUTONOMOUS_GOAL_CONTROL_BANDIT_SCHEMA) fail("bandit state schema is invalid");
    this.exploration = finite("bandit state exploration", state.exploration ?? this.exploration, 0, 2);
    this.generationValue = integer("bandit generation", state.generation, 0, 2_147_483_647);
    if (!Array.isArray(state.arms) || state.arms.length > 128) fail("bandit arms are outside their bounds");
    this.arms.clear();
    for (const raw of state.arms) {
      if (!isObject(raw)) fail("bandit arm is malformed");
      const domain = identifier("bandit arm domain", raw.domain, 128);
      if (this.arms.has(domain)) fail("bandit state contains duplicate domains");
      const pulls = integer("bandit arm pulls", raw.pulls, 0, 2_147_483_647);
      const failures = integer("bandit arm failures", raw.failures, 0, 2_147_483_647);
      if (failures > pulls) fail("bandit arm failures exceed pulls");
      const rewardSum = finite("bandit arm reward_sum", raw.reward_sum, -pulls, pulls);
      this.arms.set(domain, { pulls, failures, reward_sum: rewardSum });
    }
  }

  snapshot(): JsonObject {
    const body: JsonObject = {
      schema: AUTONOMOUS_GOAL_CONTROL_BANDIT_SCHEMA,
      generation: this.generationValue,
      arms: [...this.arms.keys()].sort().map((domain) => ({ domain, ...this.arms.get(domain)! })),
      exploration: this.exploration,
      retention: "value_only_goal_domain_bandit_state",
      secret_material: "never_returned",
    };
    return { ...body, state_digest: digestJsonSync(body) };
  }

  update(evaluations: readonly AutonomousGoalEvaluation[], goals: readonly AutonomousGoalRecord[]): Record<string, unknown> {
    if (!Array.isArray(evaluations) || evaluations.length > AUTONOMOUS_GOAL_CONTROL_MAX_EVALUATIONS) fail("bandit evaluations are outside their bounds");
    for (const evaluation of evaluations) {
      const domain = identifier("bandit evaluation domain", evaluation.domain, 128);
      const reward = finite("bandit evaluation reward", evaluation.reward, -1, 1);
      const arm = this.arms.get(domain) ?? { pulls: 0, failures: 0, reward_sum: 0 };
      arm.pulls += 1;
      arm.reward_sum += reward;
      if (!evaluation.passed) arm.failures += 1;
      this.arms.set(domain, arm);
    }
    if (this.generationValue >= 2_147_483_647) fail("bandit generation is exhausted");
    this.generationValue += 1;
    const totalPulls = Math.max(1, [...this.arms.values()].reduce((total, arm) => total + arm.pulls, 0));
    const signals: AutonomousGoalSchedulingSignal[] = [];
    for (const goal of goals) {
      if (!(["ready", "paused", "failed"] as readonly string[]).includes(goal.status)) continue;
      const arm = this.arms.get(goal.domain) ?? { pulls: 0, failures: 0, reward_sum: 0 };
      const mean = arm.pulls === 0 ? 1 : (arm.reward_sum / arm.pulls + 1) / 2;
      const score = arm.pulls === 0 ? 1 : Math.min(1, Math.max(0, mean + this.exploration * Math.sqrt(Math.log(totalPulls + 1) / arm.pulls)));
      const urgency = Math.min(1, arm.failures / Math.max(1, arm.pulls));
      signals.push({ goal_id: identifier("bandit goal_id", goal.goal_id), priority: Math.round(score * 10_000) / 10_000, urgency: Math.round(urgency * 10_000) / 10_000, estimated_cost: 1, dependencies: [] });
      if (signals.length >= AUTONOMOUS_GOAL_CONTROL_MAX_SIGNALS) break;
    }
    signals.sort((left, right) => (right.priority ?? 0) - (left.priority ?? 0) || (right.urgency ?? 0) - (left.urgency ?? 0) || left.goal_id.localeCompare(right.goal_id));
    const state = this.snapshot();
    return {
      schema: AUTONOMOUS_GOAL_CONTROL_BANDIT_SCHEMA,
      generation: this.generationValue,
      learning_state_digest: state.state_digest,
      signals,
      signals_digest: digestJsonSync(signals),
      retention: "value_only_goal_bandit_update",
      secret_material: "never_returned",
    };
  }
}

function cycleMetadata(cycle: number, batch: AutonomousGoalWorkerBatch): AutonomousGoalControlLoopCycleJSON {
  const publicBatch = batch.toJSON();
  return {
    cycle,
    schedule_digest: batch.schedule.schedule_digest,
    claim_digest: batch.claim?.claim_digest ?? null,
    worker_digest: batch.worker_digest,
    selected: batch.schedule.selected_goal_ids.length,
    claimed: batch.claim?.claims.length ?? 0,
    runs: batch.runs.length,
    counts: clone(publicBatch.counts),
    selected_domains: [...batch.schedule.coverage.selected_domains],
    missing_domains: [...batch.schedule.coverage.missing_domains],
    retention: AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION,
    secret_material: "never_returned",
  };
}

export class AutonomousGoalControlLoopCycle {
  constructor(
    readonly cycle: number,
    readonly batch: AutonomousGoalWorkerBatch,
    readonly evaluations: readonly AutonomousGoalEvaluation[] = [],
    readonly learning_state_digest: string | null = null,
    readonly next_signals: readonly AutonomousGoalSchedulingSignal[] = [],
  ) {}

  toJSON(): AutonomousGoalControlLoopCycleJSON {
    const body = cycleMetadata(this.cycle, this.batch);
    if (this.evaluations.length > 0) {
      body.evaluated = this.evaluations.length;
      body.evaluation_digest = digestJsonSync(this.evaluations);
    }
    if (this.learning_state_digest !== null) {
      body.learning_state_digest = this.learning_state_digest;
      body.signals_digest = digestJsonSync(this.next_signals);
    }
    return body;
  }

  get live_results(): unknown[] {
    return this.batch.live_results;
  }
}

export class AutonomousGoalControlLoopResult {
  constructor(
    readonly cycles: readonly AutonomousGoalControlLoopCycle[],
    readonly stop_reason: AutonomousGoalControlLoopStopReason,
    readonly total_selected: number,
    readonly total_claimed: number,
    readonly total_runs: number,
    readonly status_counts: Readonly<Record<string, number>>,
    readonly domain_counts: Readonly<Record<string, number>>,
    readonly loop_digest: string,
    readonly evaluation_count = 0,
    readonly evaluation_digest: string | null = null,
    readonly learning_state_digest: string | null = null,
    readonly restored_cycle_count = 0,
    readonly cycle_history_digest: string | null = null,
  ) {}

  get live_results(): unknown[] {
    return this.cycles.flatMap((cycle) => cycle.live_results);
  }

  toJSON(): AutonomousGoalControlLoopJSON {
    const body = {
      schema: AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA,
      cycles: this.cycles.map((cycle) => cycle.toJSON()),
      stop_reason: this.stop_reason,
      total_selected: this.total_selected,
      total_claimed: this.total_claimed,
      total_runs: this.total_runs,
      status_counts: { ...this.status_counts },
      domain_counts: { ...this.domain_counts },
      retention: AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION,
      secret_material: "never_returned" as const,
      loop_digest: this.loop_digest,
      ...(this.evaluation_digest === null ? {} : { evaluation_count: this.evaluation_count, evaluation_digest: this.evaluation_digest }),
      ...(this.learning_state_digest === null ? {} : { learning_state_digest: this.learning_state_digest }),
      ...(this.restored_cycle_count === 0 ? {} : { restored_cycle_count: this.restored_cycle_count, cycle_history_digest: this.cycle_history_digest }),
    } satisfies AutonomousGoalControlLoopJSON;
    return clone(body);
  }
}

export class AutonomousGoalControlLoop {
  readonly worker: AutonomousGoalWorker;
  readonly batch_id_prefix: string;
  readonly evaluator: AutonomousGoalControlLoopEvaluator | null;
  readonly learner: AutonomousGoalControlLoopLearner | AutonomousGoalBanditLearner | null;

  constructor(options: { worker: AutonomousGoalWorker; batch_id_prefix?: string; evaluator?: AutonomousGoalControlLoopEvaluator; learner?: AutonomousGoalControlLoopLearner | AutonomousGoalBanditLearner | null }) {
    if (!(options?.worker instanceof AutonomousGoalWorker)) fail("worker must be an AutonomousGoalWorker");
    this.worker = options.worker;
    this.batch_id_prefix = prefix(options.batch_id_prefix ?? "autonomous-goal-loop");
    if (options.evaluator !== undefined && typeof options.evaluator !== "function") fail("evaluator must be callable or undefined");
    if (options.learner !== undefined && options.learner !== null && typeof options.learner !== "function" && !(options.learner instanceof AutonomousGoalBanditLearner)) fail("learner must be callable, an AutonomousGoalBanditLearner, or null");
    if (options.learner !== undefined && options.learner !== null && options.evaluator === undefined) fail("learner requires an explicit evaluator");
    this.evaluator = options.evaluator ?? null;
    this.learner = this.evaluator === null ? null : options.learner ?? new AutonomousGoalBanditLearner();
  }

  async run(options: {
    schedule_options?: Record<string, unknown>;
    options_factory?: AutonomousGoalControlLoopOptionsFactory;
    max_cycles?: number;
    max_total_runs?: number;
    run_id?: string;
    resume_snapshot?: AutonomousGoalControlLoopCheckpoint | null;
    checkpoint?: (snapshot: AutonomousGoalControlLoopCheckpoint) => unknown | Promise<unknown>;
  } = {}): Promise<AutonomousGoalControlLoopResult> {
    if (options.schedule_options !== undefined && !isObject(options.schedule_options)) fail("schedule_options must be an object");
    if (options.options_factory !== undefined && typeof options.options_factory !== "function") fail("options_factory must be callable or undefined");
    if (options.run_id !== undefined && typeof options.run_id !== "string") fail("run_id must be a string or undefined");
    if (options.resume_snapshot !== undefined && options.resume_snapshot !== null && !isObject(options.resume_snapshot)) fail("resume_snapshot must be an object or null");
    if (options.checkpoint !== undefined && typeof options.checkpoint !== "function") fail("checkpoint must be callable or undefined");
    const maxCycles = integer("max_cycles", options.max_cycles ?? AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES, 1, AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES);
    const maxTotalRuns = integer("max_total_runs", options.max_total_runs ?? AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS, 1, AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS);
    const baseOptions = options.schedule_options ? { ...options.schedule_options } : {};
    const cycles: AutonomousGoalControlLoopCycle[] = [];
    const history: JsonObject[] = [];
    let previous: AutonomousGoalControlLoopCycleJSON | null = null;
    let totalSelected = 0;
    let totalClaimed = 0;
    let totalRuns = 0;
    const statusCounts: Record<string, number> = {};
    const domainCounts: Record<string, number> = {};
    const evaluationDigests: string[] = [];
    let evaluationCount = 0;
    let learningStateDigest: string | null = null;
    let learnedSignals: readonly AutonomousGoalSchedulingSignal[] | null = null;
    let stopReason: AutonomousGoalControlLoopStopReason = "cycle_budget_exhausted";
    let previousCheckpoint: AutonomousGoalControlLoopCheckpoint | null = null;
    let restoredCycleCount = 0;

    const hasResumeSnapshot = options.resume_snapshot !== undefined && options.resume_snapshot !== null;
    if (hasResumeSnapshot) {
      const restored = validateAutonomousGoalControlLoopSnapshot(options.resume_snapshot as AutonomousGoalControlLoopCheckpoint);
      restoredCycleCount = restored.completed_cycles;
      history.push(...restored.cycle_summaries.map((item) => clone(item)));
      previous = restored.previous_cycle === null ? null : clone(restored.previous_cycle) as AutonomousGoalControlLoopCycleJSON;
      totalSelected = restored.total_selected;
      totalClaimed = restored.total_claimed;
      totalRuns = restored.total_runs;
      Object.assign(statusCounts, restored.status_counts);
      Object.assign(domainCounts, restored.domain_counts);
      evaluationCount = restored.evaluation_count;
      evaluationDigests.push(...restored.evaluation_digests);
      learningStateDigest = restored.learning_state_digest;
      learnedSignals = restored.learned_signals.map((item, index) => normalizeLearningSignal(item, index));
      previousCheckpoint = restored;
      if (options.run_id !== undefined && options.run_id !== restored.run_id) fail("run_id does not match the resume snapshot");
      if (restored.learner_state !== null) {
        if (!(this.learner instanceof AutonomousGoalBanditLearner)) fail("resume snapshot contains built-in learner state but this loop has no compatible bandit");
        this.learner.restore(restored.learner_state);
      }
    }
    const checkpointRunId = options.checkpoint !== undefined || hasResumeSnapshot
      ? identifier("run_id", options.run_id ?? previousCheckpoint?.run_id ?? this.batch_id_prefix)
      : (options.run_id ?? this.batch_id_prefix);
    const startCycle = previousCheckpoint?.next_cycle ?? 1;
    const emitCheckpoint = async (currentStopReason: AutonomousGoalControlLoopStopReason): Promise<void> => {
      if (options.checkpoint === undefined) return;
      const learnerState = this.learner instanceof AutonomousGoalBanditLearner ? this.learner.snapshot() : null;
      const descriptor: JsonObject = {
        schema: "bioprism-autonomous-goal-control-checkpoint/0.1",
        run_id: checkpointRunId,
        next_cycle: history.length + 1,
        cycle_summaries: history,
        previous_cycle: previous,
        completed_cycles: history.length,
        total_selected: totalSelected,
        total_claimed: totalClaimed,
        total_runs: totalRuns,
        status_counts: Object.fromEntries(Object.entries(statusCounts).sort(([left], [right]) => left.localeCompare(right))),
        domain_counts: Object.fromEntries(Object.entries(domainCounts).sort(([left], [right]) => left.localeCompare(right))),
        evaluation_count: evaluationCount,
        evaluation_digests: [...evaluationDigests],
        learning_state_digest: learningStateDigest,
        learned_signals: learnedSignals === null ? [] : learnedSignals.map((signal) => ({
          goal_id: signal.goal_id,
          priority: signal.priority ?? 0.5,
          urgency: signal.urgency ?? 0,
          deadline_ns: signal.deadline_ns ?? null,
          estimated_cost: signal.estimated_cost ?? 1,
          dependencies: [...(signal.dependencies ?? [])],
        })),
        learner_state: learnerState,
        stop_reason: currentStopReason,
        generation: previousCheckpoint === null ? 1 : previousCheckpoint.generation + 1,
        previous_snapshot_digest: previousCheckpoint?.snapshot_digest ?? null,
        retention: "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
        secret_material: "never_returned",
      };
      const snapshot = sealAutonomousGoalControlLoopSnapshot(descriptor);
      await options.checkpoint(snapshot);
      previousCheckpoint = snapshot;
    };

    for (let cycleNumber = startCycle; cycleNumber <= maxCycles; cycleNumber += 1) {
      const remainingRuns = maxTotalRuns - totalRuns;
      if (remainingRuns <= 0) {
        stopReason = "run_budget_exhausted";
        break;
      }
      const context: AutonomousGoalControlLoopContext = {
        schema: AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA,
        cycle: cycleNumber,
        previous_cycle: previous === null ? null : clone(previous),
        ledger_stats: clone(this.worker.ledger.stats()),
        retention: AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION,
        secret_material: "never_returned",
      };
      const scheduleOptions = { ...baseOptions };
      if (learnedSignals !== null) scheduleOptions.signals = learnedSignals;
      if (options.options_factory) {
        const supplied = await options.options_factory(context);
        if (!isObject(supplied)) fail("options_factory must return an object");
        Object.assign(scheduleOptions, supplied);
      }
      const requestedSelected = integer("schedule_options.max_selected", scheduleOptions.max_selected ?? 1, 1, 128);
      const effectiveSelected = Math.min(requestedSelected, remainingRuns);
      scheduleOptions.max_selected = effectiveSelected;
      const requestedConcurrent = integer("schedule_options.max_concurrent", scheduleOptions.max_concurrent ?? effectiveSelected, 1, 128);
      scheduleOptions.max_concurrent = Math.min(requestedConcurrent, effectiveSelected);
      const batchId = `${this.batch_id_prefix}:cycle-${cycleNumber}`;
      if (new TextEncoder().encode(batchId).byteLength > 256) fail("generated batch_id exceeds its worker bound");
      const batch = await this.worker.run({ schedule_options: scheduleOptions, batch_id: batchId });
      let evaluations: AutonomousGoalEvaluation[] = [];
      let nextSignals: readonly AutonomousGoalSchedulingSignal[] = [];
      if (this.evaluator !== null && batch.runs.length > 0) {
        const rawEvaluations = await this.evaluator(new AutonomousGoalControlLoopCycle(cycleNumber, batch));
        if (!Array.isArray(rawEvaluations) || rawEvaluations.length !== batch.runs.length || rawEvaluations.length > AUTONOMOUS_GOAL_CONTROL_MAX_EVALUATIONS) fail("evaluator must return exactly one evaluation for every worker run");
        const runsByGoal = new Map(batch.runs.map((run) => [run.goal_id, run]));
        const seen = new Set<string>();
        for (const raw of rawEvaluations) {
          const rawGoalId = isObject(raw) ? raw.goal_id : undefined;
          const run = runsByGoal.get(rawGoalId as string);
          if (run === undefined) fail("evaluator output references an unknown goal");
          const goal = this.worker.ledger.get(run.goal_id);
          if (goal === null) fail(`evaluated goal ${run.goal_id} disappeared`);
          const evaluation = normalizeEvaluation(raw, run, goal);
          if (seen.has(evaluation.goal_id)) fail("evaluator returned duplicate goal evaluations");
          seen.add(evaluation.goal_id);
          evaluations.push(evaluation);
        }
        evaluationDigests.push(digestJsonSync(evaluations));
        evaluationCount += evaluations.length;
        const goalsForLearning = this.worker.ledger.list({ limit: 512 });
        if (this.learner !== null) {
          const update = this.learner instanceof AutonomousGoalBanditLearner
            ? this.learner.update(evaluations, goalsForLearning)
            : await this.learner(evaluations, goalsForLearning);
          if (!isObject(update)) fail("learner must return an object");
          learningStateDigest = update.learning_state_digest === undefined || update.learning_state_digest === null
            ? digestJsonSync(update)
            : digest("learning_state_digest", update.learning_state_digest)!;
          const rawSignals = update.signals === undefined ? [] : update.signals;
          if (!Array.isArray(rawSignals) || rawSignals.length > AUTONOMOUS_GOAL_CONTROL_MAX_SIGNALS) fail("learner signals are outside their bounds");
          nextSignals = rawSignals.map(normalizeLearningSignal);
          learnedSignals = nextSignals;
        }
        for (const evaluation of evaluations) {
          const current = this.worker.ledger.get(evaluation.goal_id);
          if (current === null) fail(`evaluated goal ${evaluation.goal_id} disappeared before feedback settlement`);
          this.worker.ledger.transition(evaluation.goal_id, current.status, {
            expected_revision: current.revision,
            blockers: current.blockers,
            next_action_digest: current.next_action_digest,
            evaluator_digest: evaluation.feedback_digest,
            learning_state_digest: learningStateDigest,
          });
        }
      }
      const cycle = new AutonomousGoalControlLoopCycle(cycleNumber, batch, evaluations, learningStateDigest, nextSignals);
      cycles.push(cycle);
      const publicCycle = cycle.toJSON();
      previous = publicCycle;
      history.push(publicCycle);
      totalSelected += publicCycle.selected;
      totalClaimed += publicCycle.claimed;
      totalRuns += publicCycle.runs;
      for (const run of batch.runs) {
        statusCounts[run.goal_status] = (statusCounts[run.goal_status] ?? 0) + 1;
        domainCounts[run.domain] = (domainCounts[run.domain] ?? 0) + 1;
      }
      const includePaused = scheduleOptions.include_paused ?? true;
      const allowFailedRetry = scheduleOptions.allow_failed_retry ?? false;
      if (typeof includePaused !== "boolean" || typeof allowFailedRetry !== "boolean") fail("schedule retry and pause policies must be boolean");
      let shouldBreak = false;
      if (batch.schedule.selected_goal_ids.length === 0) {
        stopReason = allTerminal(this.worker.ledger) ? "all_terminal" : "no_admissible_work";
        shouldBreak = true;
      } else if (batch.runs.length === 0) {
        stopReason = "no_admissible_work";
        shouldBreak = true;
      } else if (!hasEligibleWork(this.worker.ledger, includePaused, allowFailedRetry)) {
        stopReason = allTerminal(this.worker.ledger) ? "all_terminal" : "no_admissible_work";
        shouldBreak = true;
      }
      await emitCheckpoint(stopReason);
      if (shouldBreak) break;
    }

    const summaries = cycles.map((cycle) => cycle.toJSON());
    const normalizedStatusCounts = Object.fromEntries(Object.entries(statusCounts).sort(([left], [right]) => left.localeCompare(right)));
    const normalizedDomainCounts = Object.fromEntries(Object.entries(domainCounts).sort(([left], [right]) => left.localeCompare(right)));
    const evaluationDigest = evaluationDigests.length > 0 ? digestJsonSync(evaluationDigests) : null;
    const cycleHistoryDigest = restoredCycleCount > 0 ? digestJsonSync(history) : null;
    const loopDigest = digestJsonSync({
      schema: AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA,
      cycles: restoredCycleCount > 0 ? history : summaries,
      stop_reason: stopReason,
      total_selected: totalSelected,
      total_claimed: totalClaimed,
      total_runs: totalRuns,
      status_counts: normalizedStatusCounts,
      domain_counts: normalizedDomainCounts,
      ...(evaluationDigest === null ? {} : { evaluation_digest: evaluationDigest }),
      ...(learningStateDigest === null ? {} : { learning_state_digest: learningStateDigest }),
      ...(restoredCycleCount === 0 ? {} : { restored_cycle_count: restoredCycleCount, cycle_history_digest: cycleHistoryDigest }),
      retention: AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION,
      secret_material: "never_returned",
    });
    return new AutonomousGoalControlLoopResult(cycles, stopReason, totalSelected, totalClaimed, totalRuns, normalizedStatusCounts, normalizedDomainCounts, loopDigest, evaluationCount, evaluationDigest, learningStateDigest, restoredCycleCount, cycleHistoryDigest);
  }
}
