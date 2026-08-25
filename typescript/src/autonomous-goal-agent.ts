/** Connect metadata-only goal control to the real model-selection/provider facade. */
import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousBrainFacade,
  type AutonomousActionHandoffExecutionOptions,
  type AutonomousBrainRequest,
} from "./autonomous-brain-facade.js";
import {
  validateAutonomousActionDispatchHandoff,
  type AutonomousActionDispatchHandoff,
} from "./autonomous-action-admission-controller.js";
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
  AutonomousGoalRecoveryCoordinator,
  type AutonomousGoalRecoveryReport,
} from "./autonomous-goal-recovery.js";
import type { AutonomousGoalControlLoopCheckpoint } from "./autonomous-goal-control-persistence.js";
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
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA = "bioprism-autonomous-goal-agent-runtime/0.1" as const;
export const AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION = "metadata_only_goal_agent_bridge;tasks_prompts_parameters_credentials_and_results_not_retained" as const;

export type AutonomousGoalAgentTaskResolver = (goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) => string | Promise<string>;
export type AutonomousGoalAgentRunOptionsFactory = (goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) => Record<string, unknown> | Promise<Record<string, unknown>>;
export type AutonomousGoalAgentActionHandoffRequest = Omit<AutonomousBrainRequest, "task">;
export interface AutonomousGoalAgentActionHandoffBinding {
  handoff: AutonomousActionDispatchHandoff | JsonObject;
  request?: AutonomousGoalAgentActionHandoffRequest;
}
export type AutonomousGoalAgentActionHandoffResolver = (
  goal: AutonomousGoalRecord,
  row: AutonomousGoalScheduleRow,
  task: string,
) => AutonomousActionDispatchHandoff | AutonomousGoalAgentActionHandoffBinding | null | undefined | Promise<AutonomousActionDispatchHandoff | AutonomousGoalAgentActionHandoffBinding | null | undefined>;

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

const ACTION_HANDOFF_REQUEST_KEYS = new Set(["domain", "capability", "hints", "allow_cross_domain", "context", "connector"]);

type NormalizedActionHandoffBinding = {
  handoff: AutonomousActionDispatchHandoff;
  request: AutonomousGoalAgentActionHandoffRequest;
};

function actionHandoff(value: unknown, goal: AutonomousGoalRecord): NormalizedActionHandoffBinding | undefined {
  if (value === undefined || value === null) return undefined;
  let handoffSource: unknown = value;
  let request: Record<string, unknown> = {};
  if (isObject(value) && "handoff" in value) {
    handoffSource = value.handoff as unknown;
    if (value.request !== undefined) {
      if (!isObject(value.request)) fail("action handoff request must be an object");
      request = { ...value.request };
    }
  }
  if (!isObject(handoffSource)) fail("action handoff must be an object");
  const handoff = validateAutonomousActionDispatchHandoff(handoffSource);
  if (Object.keys(request).some((key) => !ACTION_HANDOFF_REQUEST_KEYS.has(key) || key === "task")) fail("action handoff request contains unsupported fields");
  if (goal.domain === "cross_domain") {
    if (request.domain !== undefined && request.domain !== "cross_domain") fail("cross-domain goal action handoffs cannot select a single-domain request");
    if (!handoff.cross_domain && !handoff.selected_domains.includes("cross_domain")) fail("cross-domain goal action handoff is not cross-domain");
    if (request.domain === undefined && !handoff.cross_domain) request.domain = "cross_domain";
  } else {
    if (!handoff.selected_domains.includes(goal.domain as AutonomousDomainName)) fail(`action handoff does not cover goal domain ${goal.domain}`);
    if (request.domain !== undefined && request.domain !== goal.domain) fail(`action handoff request domain does not match goal ${goal.domain}`);
    request.domain ??= goal.domain;
  }
  return { handoff, request: request as AutonomousGoalAgentActionHandoffRequest };
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
  readonly action_handoff_resolver: AutonomousGoalAgentActionHandoffResolver | undefined;
  readonly brain: AutonomousBrainFacade | undefined;
  readonly worker: AutonomousGoalWorker;
  readonly loop: AutonomousGoalControlLoop;
  readonly recovery: AutonomousGoalRecoveryCoordinator | undefined;
  readonly batch_id_prefix: string;

  constructor(options: {
    agent: AutonomousAgent;
    ledger: InMemoryAutonomousGoalLedger;
    task_resolver: AutonomousGoalAgentTaskResolver;
    run_options_factory?: AutonomousGoalAgentRunOptionsFactory;
    action_handoff_resolver?: AutonomousGoalAgentActionHandoffResolver;
    brain?: AutonomousBrainFacade;
    evaluator?: AutonomousGoalControlLoopEvaluator;
    learner?: AutonomousGoalControlLoopLearner | AutonomousGoalBanditLearner | null;
    journal?: AutonomousGoalWorkerJournal;
    recovery?: AutonomousGoalRecoveryCoordinator;
    batch_id_prefix?: string;
  }) {
    if (!(options?.agent instanceof AutonomousAgent)) fail("agent must be an AutonomousAgent");
    if (!(options.ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (typeof options.task_resolver !== "function") fail("task_resolver must be callable");
    if (options.run_options_factory !== undefined && typeof options.run_options_factory !== "function") fail("run_options_factory must be callable or undefined");
    if (options.action_handoff_resolver !== undefined && typeof options.action_handoff_resolver !== "function") fail("action_handoff_resolver must be callable or undefined");
    if (options.brain !== undefined && !(options.brain instanceof AutonomousBrainFacade)) fail("brain must be an AutonomousBrainFacade or undefined");
    if (options.brain !== undefined && options.brain.agent !== options.agent) fail("brain must be bound to the supplied agent");
    if (options.action_handoff_resolver !== undefined && options.brain === undefined) fail("action_handoff_resolver requires a brain facade");
    const batchIdPrefix = options.batch_id_prefix ?? "autonomous-goal-agent";
    if (typeof batchIdPrefix !== "string" || !batchIdPrefix.trim() || batchIdPrefix.includes("\u0000") || new TextEncoder().encode(batchIdPrefix).byteLength > 128) fail("batch_id_prefix is outside its bounded contract");
    this.agent = options.agent;
    this.ledger = options.ledger;
    this.task_resolver = options.task_resolver;
    this.run_options_factory = options.run_options_factory;
    this.action_handoff_resolver = options.action_handoff_resolver;
    this.brain = options.brain;
    if (options.recovery !== undefined && options.recovery.ledger !== this.ledger) fail("recovery coordinator must own the supplied ledger");
    if (options.recovery !== undefined && (options.journal === undefined || options.recovery.journal.journal !== options.journal)) fail("recovery coordinator must own the supplied worker journal");
    this.recovery = options.recovery;
    this.batch_id_prefix = batchIdPrefix.trim();
    this.worker = new AutonomousGoalWorker({
      ledger: this.ledger,
      resolver: async (goal, row) => {
        const resolvedTask = task(await this.task_resolver(goal, row), goal.goal_id);
        const resolvedHandoff = this.action_handoff_resolver === undefined ? undefined : await this.action_handoff_resolver(goal, row, resolvedTask);
        const binding = actionHandoff(resolvedHandoff, goal);
        return { task: resolvedTask, parameters: binding === undefined ? {} : { action_handoff: binding as unknown as JsonObject } };
      },
      executor: (request) => this.execute(request),
      journal: options.journal,
    });
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
    const binding = actionHandoff(request.parameters.action_handoff, request.goal);
    if (binding !== undefined) {
      if (this.brain === undefined) fail("action handoff execution requires a brain facade");
      return this.brain.executeActionHandoff({ ...binding.request, task: request.task }, binding.handoff, options as AutonomousActionHandoffExecutionOptions);
    }
    if (request.goal.domain === "cross_domain") {
      const { subtasks: childSubtasks, ...rest } = options;
      return this.agent.runCrossDomain(request.task, { ...rest, subtasks: childSubtasks } as unknown as AutonomousCrossDomainRunOptions);
    }
    const domain = request.goal.domain as AutonomousDomainName;
    if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) fail(`goal ${request.goal.goal_id} has an unsupported autonomous domain`);
    return this.agent.run(request.task, { ...options, domain } as unknown as AutonomousRunOptions);
  }

  metadata(): Record<string, unknown> {
    return { schema: AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA, batch_id_prefix: this.batch_id_prefix, domain_count: AUTONOMOUS_DOMAIN_NAMES.length, domains: [...AUTONOMOUS_DOMAIN_NAMES], execution_surface: this.action_handoff_resolver === undefined ? "autonomous_agent_facade" : "autonomous_goal_action_handoff_facade", action_handoff_execution: this.action_handoff_resolver === undefined ? "not_configured" : "verified_handoff_replay_before_run_boundary", recovery_execution: this.recovery === undefined ? "caller_composed" : "ordered_journal_then_control_checkpoint", retention: AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION, secret_material: "never_returned" };
  }

  async restore(options: { now_ns?: number } = {}): Promise<AutonomousGoalRecoveryReport> {
    if (this.recovery === undefined) fail("restore requires a recovery coordinator");
    return this.recovery.restore(options);
  }

  run(options: { schedule_options?: Record<string, unknown>; options_factory?: AutonomousGoalControlLoopOptionsFactory; max_cycles?: number; max_total_runs?: number; checkpoint?: (snapshot: AutonomousGoalControlLoopCheckpoint) => unknown | Promise<unknown> } = {}): Promise<AutonomousGoalControlLoopResult> {
    if (this.recovery === undefined) return this.loop.run(options);
    if (options.checkpoint !== undefined) fail("checkpoint is owned by the recovery coordinator");
    return this.recovery.resume(this.loop, {
      ...options,
      checkpoint: (snapshot: AutonomousGoalControlLoopCheckpoint) => this.recovery!.checkpoint(snapshot),
    });
  }
}
