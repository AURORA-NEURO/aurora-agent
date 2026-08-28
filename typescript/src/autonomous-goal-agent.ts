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
  AutonomousGoalControlLoopPreview,
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
import { AutonomousProtectedRehydrationAdapter } from "./autonomous-protected-rehydration.js";
import { digestJsonSync } from "./tooling.js";
import {
  autonomousRunTraceStatus,
  AutonomousRunTraceSession,
  type AutonomousRunTraceStatus,
  type AutonomousRunTraceStore,
  type AutonomousRunTraceSummary,
} from "./autonomous-run-trace.js";
import type { ProviderInvocationObserver, AutonomousModelSelectionTraceEventCallback } from "./llm.js";
import {
  AutonomousRunTraceRegistry,
  publishAutonomousRunTraceRegistrySnapshot,
  type AutonomousRunTraceRegistryPublication,
} from "./autonomous-run-trace-registry.js";

export const AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA = "bioprism-autonomous-goal-agent-runtime/0.1" as const;
export const AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION = "metadata_only_goal_agent_bridge;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA = "bioprism-autonomous-goal-agent-trace/0.1" as const;
export const AUTONOMOUS_GOAL_AGENT_TRACE_RETENTION = "metadata_only_goal_control_trace;goal_task_prompts_parameters_credentials_and_results_not_retained" as const;

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

export interface AutonomousGoalAgentLoopRunOptions {
  schedule_options?: Record<string, unknown>;
  options_factory?: AutonomousGoalControlLoopOptionsFactory;
  max_cycles?: number;
  max_total_runs?: number;
  run_id?: string;
  resume_snapshot?: AutonomousGoalControlLoopCheckpoint | null;
  checkpoint?: (snapshot: AutonomousGoalControlLoopCheckpoint) => unknown | Promise<unknown>;
  expected_preview_digest?: string;
}

export interface AutonomousGoalAgentTraceOptions extends Omit<AutonomousGoalAgentLoopRunOptions, "run_id"> {
  traceStore: AutonomousRunTraceStore;
  runId: string;
  /** Optional metadata-only projection for operator queries and bounded retention. */
  traceRegistry?: AutonomousRunTraceRegistry;
}

export interface AutonomousGoalAgentTracedRunResult {
  schema: typeof AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA;
  /** The live loop result is available to the initiating caller only. */
  result: AutonomousGoalControlLoopResult;
  trace: AutonomousRunTraceSummary;
  traceRegistry?: AutonomousRunTraceRegistryPublication;
  retention: typeof AUTONOMOUS_GOAL_AGENT_TRACE_RETENTION;
  secret_material: "never_returned";
}

/**
 * Construction boundary for the long-horizon goal runtime.
 *
 * The agent and optional brain are deliberately explicit in this type instead of being hidden
 * inside a global singleton. Applications can therefore own the ledger, evaluator, learner,
 * recovery journal, and protected task rehydration boundary while the brain facade supplies the
 * already-bound routing/provider composition. Task text, provider options, credentials, and live
 * results remain process-local values and are never part of the runtime metadata projection.
 */
export interface AutonomousGoalAgentRuntimeOptions {
  agent: AutonomousAgent;
  ledger: InMemoryAutonomousGoalLedger;
  task_resolver?: AutonomousGoalAgentTaskResolver;
  protected_rehydration?: AutonomousProtectedRehydrationAdapter;
  run_options_factory?: AutonomousGoalAgentRunOptionsFactory;
  action_handoff_resolver?: AutonomousGoalAgentActionHandoffResolver;
  brain?: AutonomousBrainFacade;
  evaluator?: AutonomousGoalControlLoopEvaluator;
  learner?: AutonomousGoalControlLoopLearner | AutonomousGoalBanditLearner | null;
  journal?: AutonomousGoalWorkerJournal;
  recovery?: AutonomousGoalRecoveryCoordinator;
  batch_id_prefix?: string;
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal agent runtime ${message}`);
}

function task(value: unknown, goalId: string): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 32_000) fail(`resolved task is invalid for goal ${goalId}`);
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

function composeGoalInvocationObservers(...observers: readonly (ProviderInvocationObserver | undefined)[]): ProviderInvocationObserver | undefined {
  const active = observers.filter((observer): observer is ProviderInvocationObserver => observer !== undefined);
  if (!active.length) return undefined;
  return {
    before: async (metadata) => {
      for (const observer of active) await observer.before?.(metadata);
    },
    after: async (metadata, outcome) => {
      for (const observer of active) await observer.after?.(metadata, outcome);
    },
  };
}

function composeGoalSelectionCallbacks(...callbacks: readonly (AutonomousModelSelectionTraceEventCallback | undefined)[]): AutonomousModelSelectionTraceEventCallback | undefined {
  const active = callbacks.filter((callback): callback is AutonomousModelSelectionTraceEventCallback => callback !== undefined);
  if (!active.length) return undefined;
  return async (event) => {
    for (const callback of active) await callback(event);
  };
}

function goalTraceDomains(goal: AutonomousGoalRecord, options?: Record<string, unknown>): AutonomousDomainName[] {
  const domains: AutonomousDomainName[] = [goal.domain as AutonomousDomainName];
  if (goal.domain === "cross_domain" && Array.isArray(options?.subtasks)) {
    for (const item of options.subtasks) {
      if (!isObject(item) || typeof item.domain !== "string") continue;
      if ((AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(item.domain)) domains.push(item.domain as AutonomousDomainName);
    }
  }
  return [...new Set(domains)];
}

function goalTraceStatus(value: unknown): AutonomousRunTraceStatus {
  const status = isObject(value) && typeof value.status === "string" ? value.status : "unknown";
  return autonomousRunTraceStatus(status);
}

function controlLoopTraceStatus(result: AutonomousGoalControlLoopResult): AutonomousRunTraceStatus {
  const count = (name: string): number => {
    const value = result.status_counts[name];
    return typeof value === "number" && Number.isFinite(value) ? value : 0;
  };
  const completed = count("completed");
  const paused = count("paused");
  const blocked = count("blocked");
  const failed = count("failed");
  if (failed > 0 && completed === 0 && paused === 0 && blocked === 0) return "failed";
  if (paused > 0 || blocked > 0) return completed > 0 ? "partial" : "paused";
  if (failed > 0) return completed > 0 ? "partial" : "failed";
  if (result.stop_reason === "all_terminal") return "completed";
  if (completed > 0) return "partial";
  return result.stop_reason === "no_admissible_work" ? "paused" : "unknown";
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
  readonly task_rehydration_configured: boolean;
  readonly protected_rehydration: AutonomousProtectedRehydrationAdapter | undefined;
  readonly run_options_factory: AutonomousGoalAgentRunOptionsFactory | undefined;
  readonly action_handoff_resolver: AutonomousGoalAgentActionHandoffResolver | undefined;
  readonly brain: AutonomousBrainFacade | undefined;
  readonly worker: AutonomousGoalWorker;
  readonly loop: AutonomousGoalControlLoop;
  readonly recovery: AutonomousGoalRecoveryCoordinator | undefined;
  readonly batch_id_prefix: string;
  private trace_context: {
    session: AutonomousRunTraceSession;
    observer: ProviderInvocationObserver;
    selection_event_callback: AutonomousModelSelectionTraceEventCallback;
  } | undefined;

  constructor(options: AutonomousGoalAgentRuntimeOptions) {
    if (!(options?.agent instanceof AutonomousAgent)) fail("agent must be an AutonomousAgent");
    if (!(options.ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (options.task_resolver !== undefined && typeof options.task_resolver !== "function") fail("task_resolver must be callable or undefined");
    if (options.protected_rehydration !== undefined && !(options.protected_rehydration instanceof AutonomousProtectedRehydrationAdapter)) fail("protected_rehydration must be an AutonomousProtectedRehydrationAdapter or undefined");
    if (options.run_options_factory !== undefined && typeof options.run_options_factory !== "function") fail("run_options_factory must be callable or undefined");
    if (options.action_handoff_resolver !== undefined && typeof options.action_handoff_resolver !== "function") fail("action_handoff_resolver must be callable or undefined");
    if (options.brain !== undefined && !(options.brain instanceof AutonomousBrainFacade)) fail("brain must be an AutonomousBrainFacade or undefined");
    if (options.brain !== undefined && options.brain.agent !== options.agent) fail("brain must be bound to the supplied agent");
    if (options.action_handoff_resolver !== undefined && options.brain === undefined) fail("action_handoff_resolver requires a brain facade");
    const batchIdPrefix = options.batch_id_prefix ?? "autonomous-goal-agent";
    if (typeof batchIdPrefix !== "string" || !batchIdPrefix.trim() || batchIdPrefix.includes("\u0000") || new TextEncoder().encode(batchIdPrefix).byteLength > 128) fail("batch_id_prefix is outside its bounded contract");
    this.agent = options.agent;
    this.ledger = options.ledger;
    this.task_resolver = options.task_resolver ?? (async () => { throw new ArgumentError("protected task resolver was not initialized"); });
    this.task_rehydration_configured = options.task_resolver !== undefined || options.protected_rehydration !== undefined;
    this.protected_rehydration = options.protected_rehydration;
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
        if (options.task_resolver === undefined && this.protected_rehydration === undefined) fail("task rehydration is not configured");
        const resolvedTask = task(
          options.task_resolver === undefined
            ? this.protected_rehydration!.resolveReceipt(
                {
                  goal_id: goal.goal_id,
                  task_digest: goal.task_digest,
                  value_digest: goal.task_digest,
                  domain: goal.domain,
                  attempt: goal.attempt,
                  revision: goal.revision,
                  request_digest: digestJsonSync(row),
                },
                { domain: goal.domain as AutonomousDomainName, purpose: "goal_task", valueKind: "goal_task", oneTime: false, digestScheme: "utf8_sha256" },
              )
            : await this.task_resolver(goal, row),
          goal.goal_id,
        );
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
    const traceContext = this.trace_context;
    const traceDomains = goalTraceDomains(request.goal, options);
    const planDigest = traceContext === undefined ? null : digestJsonSync({
      schema: AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA,
      goal_id: request.goal.goal_id,
      task_digest: request.goal.task_digest,
      domain: request.goal.domain,
      capability: request.goal.capability,
      risk_class: request.goal.risk_class,
      attempt: request.goal.attempt,
      max_attempts: request.goal.max_attempts,
      revision: request.goal.revision,
      schedule_digest: request.schedule_digest,
    });
    if (traceContext !== undefined) {
      await traceContext.session.record({
        phase: "plan_compiled",
        status: "running",
        domains: traceDomains,
        plan_digest: planDigest,
        detail_digest: digestJsonSync({
          goal_id: request.goal.goal_id,
          attempt: request.goal.attempt,
          revision: request.goal.revision,
          execution_binding_digest: request.execution_binding_digest,
        }),
      });
    }
    const tracedOptions = traceContext === undefined
      ? options
      : {
        ...options,
        observer: composeGoalInvocationObservers(options.observer as ProviderInvocationObserver | undefined, traceContext.observer),
        selectionEventCallback: composeGoalSelectionCallbacks(options.selectionEventCallback as AutonomousModelSelectionTraceEventCallback | undefined, traceContext.selection_event_callback),
    };
    const binding = actionHandoff(request.parameters.action_handoff, request.goal);
    let result: unknown;
    if (binding !== undefined) {
      if (this.brain === undefined) fail("action handoff execution requires a brain facade");
      result = await this.brain.executeActionHandoff({ ...binding.request, task: request.task }, binding.handoff, tracedOptions as AutonomousActionHandoffExecutionOptions);
    } else if (request.goal.domain === "cross_domain") {
      const { subtasks: childSubtasks, ...rest } = tracedOptions;
      result = await this.agent.runCrossDomain(request.task, { ...rest, subtasks: childSubtasks } as unknown as AutonomousCrossDomainRunOptions);
    } else {
      const domain = request.goal.domain as AutonomousDomainName;
      if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) fail(`goal ${request.goal.goal_id} has an unsupported autonomous domain`);
      result = await this.agent.run(request.task, { ...tracedOptions, domain } as unknown as AutonomousRunOptions);
    }
    if (traceContext !== undefined) {
      const resultStatus = isObject(result) && typeof result.status === "string" ? result.status : "unknown";
      await traceContext.session.record({
        phase: "evaluation_settled",
        status: goalTraceStatus(result),
        domains: traceDomains,
        plan_digest: planDigest,
        detail_digest: digestJsonSync({ goal_id: request.goal.goal_id, attempt: request.goal.attempt, result_status: resultStatus }),
      });
    }
    return result;
  }

  metadata(): Record<string, unknown> {
    return { schema: AUTONOMOUS_GOAL_AGENT_RUNTIME_SCHEMA, batch_id_prefix: this.batch_id_prefix, domain_count: AUTONOMOUS_DOMAIN_NAMES.length, domains: [...AUTONOMOUS_DOMAIN_NAMES], execution_surface: this.action_handoff_resolver === undefined ? "autonomous_agent_facade" : "autonomous_goal_action_handoff_facade", action_handoff_execution: this.action_handoff_resolver === undefined ? "not_configured" : "verified_handoff_replay_before_run_boundary", task_rehydration: !this.task_rehydration_configured ? "not_configured_preview_only" : (this.protected_rehydration === undefined ? "caller_task_resolver_precedence" : "protected_receipt_adapter_fallback"), recovery_execution: this.recovery === undefined ? "caller_composed" : "ordered_journal_then_control_checkpoint", trace_execution: "metadata_only_goal_control_trace", retention: AUTONOMOUS_GOAL_AGENT_RUNTIME_RETENTION, secret_material: "never_returned" };
  }

  async restore(options: { now_ns?: number } = {}): Promise<AutonomousGoalRecoveryReport> {
    if (this.recovery === undefined) fail("restore requires a recovery coordinator");
    return this.recovery.restore(options);
  }

  run(options: AutonomousGoalAgentLoopRunOptions = {}): Promise<AutonomousGoalControlLoopResult> {
    if (this.recovery === undefined) return this.loop.run(options);
    if (options.expected_preview_digest !== undefined) fail("expected_preview_digest cannot be combined with recovery-owned resume");
    if (options.checkpoint !== undefined) fail("checkpoint is owned by the recovery coordinator");
    return this.recovery.resume(this.loop, {
      ...options,
      checkpoint: (snapshot: AutonomousGoalControlLoopCheckpoint) => this.recovery!.checkpoint(snapshot),
    });
  }

  /** Return the next goal admission explanation without rehydrating or dispatching work. */
  preview(options: { schedule_options?: Record<string, unknown> } = {}): AutonomousGoalControlLoopPreview {
    return this.loop.preview(options);
  }

  /**
   * Run the complete scheduler/worker/evaluator/learner loop under one metadata-only trace.
   * Goal task text, transient run options, provider payloads, credentials, and live results
   * remain caller-owned and are intentionally absent from the serialized envelope.
   */
  async runWithTrace(options: AutonomousGoalAgentTraceOptions): Promise<AutonomousGoalAgentTracedRunResult> {
    if (!options || typeof options !== "object") fail("runWithTrace options must be an object");
    if (!options.traceStore || typeof options.traceStore.append !== "function" || typeof options.traceStore.events !== "function") fail("runWithTrace requires a trace store");
    if (options.traceRegistry !== undefined && !(options.traceRegistry instanceof AutonomousRunTraceRegistry)) fail("runWithTrace traceRegistry must be an AutonomousRunTraceRegistry");
    if (this.trace_context !== undefined) fail("runWithTrace cannot be re-entered while another trace is active");
    const goals = this.ledger.list({ limit: 512 });
    const unsupported = goals.filter((goal) => !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(goal.domain));
    if (unsupported.length > 0) fail(`runWithTrace found unsupported goal domains: ${unsupported.map((goal) => goal.domain).join(", ")}`);
    const domains = goals.length
      ? [...new Set(goals.map((goal) => goal.domain as AutonomousDomainName))]
      : ["cross_domain" as AutonomousDomainName];
    const goalMetadata = goals.map((goal) => ({
      goal_id: goal.goal_id,
      task_digest: goal.task_digest,
      domain: goal.domain,
      capability: goal.capability,
      risk_class: goal.risk_class,
      status: goal.status,
      attempt: goal.attempt,
      max_attempts: goal.max_attempts,
      revision: goal.revision,
    }));
    const taskDigest = digestJsonSync({ schema: AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA, run_id: options.runId, goals: goalMetadata });
    const planDigest = digestJsonSync({ schema: AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA, batch_id_prefix: this.batch_id_prefix, goals: goalMetadata });
    const session = new AutonomousRunTraceSession(options.traceStore, { run_id: options.runId, task_digest: taskDigest, domains });
    await session.started(digestJsonSync({ goal_count: goalMetadata.length, domain_count: domains.length }));
    await session.record({ phase: "plan_compiled", status: "running", domains, plan_digest: planDigest, detail_digest: digestJsonSync({ goal_count: goalMetadata.length, domain_count: domains.length }) });
    this.trace_context = {
      session,
      observer: session.providerObserver(),
      selection_event_callback: session.selectionEventCallback(),
    };
    try {
      const { traceStore: _traceStore, runId: _runId, ...loopOptions } = options;
      const result = await this.run({ ...loopOptions, run_id: options.runId });
      await session.record({
        phase: "learning_prepared",
        status: "running",
        domains,
        plan_digest: planDigest,
        detail_digest: digestJsonSync({
          total_selected: result.total_selected,
          total_claimed: result.total_claimed,
          total_runs: result.total_runs,
          evaluation_count: result.evaluation_count,
          evaluation_digest: result.evaluation_digest,
          learning_state_digest: result.learning_state_digest,
          stop_reason: result.stop_reason,
        }),
      });
      await session.complete({ status: controlLoopTraceStatus(result), domains, plan_digest: planDigest, detail_digest: digestJsonSync(result.toJSON()) });
      const traceRegistry = options.traceRegistry === undefined
        ? undefined
        : await publishAutonomousRunTraceRegistrySnapshot(options.traceRegistry, options.traceStore, options.runId);
      return {
        schema: AUTONOMOUS_GOAL_AGENT_TRACE_SCHEMA,
        result,
        trace: await session.summary(),
        ...(traceRegistry === undefined ? {} : { traceRegistry }),
        retention: AUTONOMOUS_GOAL_AGENT_TRACE_RETENTION,
        secret_material: "never_returned",
      };
    } catch (error) {
      const failureClass = error instanceof Error ? error.constructor.name : "UnknownError";
      await session.fail({ failure_class: failureClass, failure_code: "goal_control_loop_error", detail_digest: digestJsonSync({ failure_class: failureClass }) }).catch(() => undefined);
      if (options.traceRegistry !== undefined) await publishAutonomousRunTraceRegistrySnapshot(options.traceRegistry, options.traceStore, options.runId);
      throw error;
    } finally {
      this.trace_context = undefined;
    }
  }
}
