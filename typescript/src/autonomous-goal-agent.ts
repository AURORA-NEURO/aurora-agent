/** Connect metadata-only goal control to the real model-selection/provider facade. */
import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainSubtask,
  type AutonomousDomainName,
  type AutonomousRunOptions,
} from "./autonomous.js";
import {
  AutonomousGoalControlLoop,
  AutonomousGoalBanditLearner,
  type AutonomousGoalControlLoopEvaluator,
  type AutonomousGoalControlLoopLearner,
  type AutonomousGoalControlLoopOptionsFactory,
  type AutonomousGoalControlLoopResult,
} from "./autonomous-goal-control-loop.js";
import {
  AutonomousGoalWorker,
  type AutonomousGoalExecutionRequest,
} from "./autonomous-goal-worker.js";
import type { AutonomousGoalWorkerJournal } from "./autonomous-goal-worker-journal.js";
import {
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalRecord,
} from "./autonomous-goals.js";
import type { AutonomousGoalScheduleRow } from "./autonomous-goal-scheduler.js";

export const AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA = "bioprism-autonomous-goal-agent-runtime/0.1" as const;
export const AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION = "metadata_only_goal_agent_bridge;tasks_prompts_parameters_credentials_and_results_not_retained" as const;

export type AutonomousGoalAgentTaskResolver = (goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) => string | Promise<string>;
export type AutonomousGoalAgentRunOptionsFactory = (goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) => Record<string, unknown> | Promise<Record<string, unknown>>;

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal agent runtime ${message}`);
}

function task(value: unknown, goalId: string): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 32_000) fail(`task_resolver returned an invalid task for goal ${goalId}`);
  return value;
}

function runOptions(value: unknown): Record<string, unknown> {
  if (value === undefined || value === null) return {};
  if (!isObject(value)) fail("run_options_factory must return an object");
  const forbidden = Object.keys(value).filter((key) => key === "task" || key === "domain");
  if (forbidden.length > 0) fail(`run options cannot override goal ${forbidden.join(", ")}`);
  if (Object.keys(value).length > 128) fail("run options contain too many fields");
  if (Object.keys(value).some((key) => !key.trim() || key.includes("\u0000"))) fail("run options contain an invalid key");
  // Do not structuredClone this object: credential handles, AbortSignals, effect boundaries,
  // observers, and tool callbacks are intentionally process-local execution values.
  return { ...value };
}

function subtasks(value: unknown): readonly AutonomousCrossDomainSubtask[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > 64 || value.some((item) => !isObject(item))) fail("cross-domain run options require 1..64 subtasks");
  return value as unknown as readonly AutonomousCrossDomainSubtask[];
}

/**
 * Long-horizon agent bridge. Task text and provider options are rehydrated only at execution;
 * scheduler, worker, evaluator, and bandit projections remain metadata-only.
 */
export class AutonomousGoalAgentRuntime {
  readonly agent: AutonomousAgent;
  readonly ledger: InMemoryAutonomousGoalLedger;
  readonly task_resolver: AutonomousGoalAgentTaskResolver;
  readonly run_options_factory: AutonomousGoalAgentRunOptionsFactory | undefined;
  readonly worker: AutonomousGoalWorker;
  readonly loop: AutonomousGoalControlLoop;
  readonly batch_id_prefix: string;

  constructor(options: {
    agent: AutonomousAgent;
    ledger: InMemoryAutonomousGoalLedger;
    task_resolver: AutonomousGoalAgentTaskResolver;
    run_options_factory?: AutonomousGoalAgentRunOptionsFactory;
    evaluator?: AutonomousGoalControlLoopEvaluator;
    learner?: AutonomousGoalControlLoopLearner | AutonomousGoalBanditLearner | null;
    journal?: AutonomousGoalWorkerJournal;
    batch_id_prefix?: string;
  }) {
    if (!(options?.agent instanceof AutonomousAgent)) fail("agent must be an AutonomousAgent");
    if (!(options.ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (typeof options.task_resolver !== "function") fail("task_resolver must be callable");
    if (options.run_options_factory !== undefined && typeof options.run_options_factory !== "function") fail("run_options_factory must be callable or undefined");
    const batchIdPrefix = options.batch_id_prefix ?? "autonomous-goal-agent";
    if (typeof batchIdPrefix !== "string" || !batchIdPrefix.trim() || batchIdPrefix.includes("\u0000") || new TextEncoder().encode(batchIdPrefix).byteLength > 128) fail("batch_id_prefix is outside its bounded contract");
    this.agent = options.agent;
    this.ledger = options.ledger;
    this.task_resolver = options.task_resolver;
    this.run_options_factory = options.run_options_factory;
    this.batch_id_prefix = batchIdPrefix.trim();
    this.worker = new AutonomousGoalWorker({ ledger: this.ledger, resolver: async (goal, row) => ({ task: task(await this.task_resolver(goal, row), goal.goal_id), parameters: {} }), executor: (request) => this.execute(request), journal: options.journal });
    this.loop = new AutonomousGoalControlLoop({ worker: this.worker, batch_id_prefix: this.batch_id_prefix, evaluator: options.evaluator, learner: options.learner });
  }

  private async executionOptions(goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow): Promise<Record<string, unknown>> {
    const value = this.run_options_factory === undefined ? {} : await this.run_options_factory(goal, row);
    const options = runOptions(value);
    if (goal.domain === "cross_domain") options.subtasks = subtasks(options.subtasks);
    else if ("subtasks" in options) fail("single-domain run options cannot contain subtasks");
    return options;
  }

  private async execute(request: AutonomousGoalExecutionRequest): Promise<unknown> {
    const options = await this.executionOptions(request.goal, request.schedule_row);
    if (request.goal.domain === "cross_domain") {
      const { subtasks: childSubtasks, ...rest } = options;
      return this.agent.runCrossDomain(request.task, { ...rest, subtasks: childSubtasks } as unknown as AutonomousCrossDomainRunOptions);
    }
    const domain = request.goal.domain as AutonomousDomainName;
    if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) fail(`goal ${request.goal.goal_id} has an unsupported autonomous domain`);
    return this.agent.run(request.task, { ...options, domain } as unknown as AutonomousRunOptions);
  }

  metadata(): Record<string, unknown> {
    return { schema: AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA, batch_id_prefix: this.batch_id_prefix, domain_count: AUTONOMOUS_DOMAIN_NAMES.length, domains: [...AUTONOMOUS_DOMAIN_NAMES], execution_surface: "autonomous_agent_facade", retention: AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION, secret_material: "never_returned" };
  }

  run(options: { schedule_options?: Record<string, unknown>; options_factory?: AutonomousGoalControlLoopOptionsFactory; max_cycles?: number; max_total_runs?: number } = {}): Promise<AutonomousGoalControlLoopResult> {
    return this.loop.run(options);
  }
}
