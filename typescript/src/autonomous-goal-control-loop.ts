import { ArgumentError, isObject } from "./errors.js";
import { AutonomousGoalWorker, type AutonomousGoalWorkerBatch } from "./autonomous-goal-worker.js";
import type { InMemoryAutonomousGoalLedger } from "./autonomous-goals.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Bounded autonomous continuation over scheduler/worker cycles. */
export const AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA = "bioprism-autonomous-goal-control-loop/0.1" as const;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION = "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES = 128;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS = 8_192;
export const AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_BATCH_PREFIX_BYTES = 128;

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
  constructor(readonly cycle: number, readonly batch: AutonomousGoalWorkerBatch) {}

  toJSON(): AutonomousGoalControlLoopCycleJSON {
    return cycleMetadata(this.cycle, this.batch);
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
    } satisfies AutonomousGoalControlLoopJSON;
    return clone(body);
  }
}

export class AutonomousGoalControlLoop {
  readonly worker: AutonomousGoalWorker;
  readonly batch_id_prefix: string;

  constructor(options: { worker: AutonomousGoalWorker; batch_id_prefix?: string }) {
    if (!(options?.worker instanceof AutonomousGoalWorker)) fail("worker must be an AutonomousGoalWorker");
    this.worker = options.worker;
    this.batch_id_prefix = prefix(options.batch_id_prefix ?? "autonomous-goal-loop");
  }

  async run(options: {
    schedule_options?: Record<string, unknown>;
    options_factory?: AutonomousGoalControlLoopOptionsFactory;
    max_cycles?: number;
    max_total_runs?: number;
  } = {}): Promise<AutonomousGoalControlLoopResult> {
    if (options.schedule_options !== undefined && !isObject(options.schedule_options)) fail("schedule_options must be an object");
    if (options.options_factory !== undefined && typeof options.options_factory !== "function") fail("options_factory must be callable or undefined");
    const maxCycles = integer("max_cycles", options.max_cycles ?? AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES, 1, AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_CYCLES);
    const maxTotalRuns = integer("max_total_runs", options.max_total_runs ?? AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS, 1, AUTONOMOUS_GOAL_CONTROL_LOOP_MAX_RUNS);
    const baseOptions = options.schedule_options ? { ...options.schedule_options } : {};
    const cycles: AutonomousGoalControlLoopCycle[] = [];
    let previous: AutonomousGoalControlLoopCycleJSON | null = null;
    let totalSelected = 0;
    let totalClaimed = 0;
    let totalRuns = 0;
    const statusCounts: Record<string, number> = {};
    const domainCounts: Record<string, number> = {};
    let stopReason: AutonomousGoalControlLoopStopReason = "cycle_budget_exhausted";

    for (let cycleNumber = 1; cycleNumber <= maxCycles; cycleNumber += 1) {
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
      const cycle = new AutonomousGoalControlLoopCycle(cycleNumber, batch);
      cycles.push(cycle);
      previous = cycle.toJSON();
      totalSelected += previous.selected;
      totalClaimed += previous.claimed;
      totalRuns += previous.runs;
      for (const run of batch.runs) {
        statusCounts[run.goal_status] = (statusCounts[run.goal_status] ?? 0) + 1;
        domainCounts[run.domain] = (domainCounts[run.domain] ?? 0) + 1;
      }
      const includePaused = scheduleOptions.include_paused ?? true;
      const allowFailedRetry = scheduleOptions.allow_failed_retry ?? false;
      if (typeof includePaused !== "boolean" || typeof allowFailedRetry !== "boolean") fail("schedule retry and pause policies must be boolean");
      if (batch.schedule.selected_goal_ids.length === 0) {
        stopReason = allTerminal(this.worker.ledger) ? "all_terminal" : "no_admissible_work";
        break;
      }
      if (batch.runs.length === 0) {
        stopReason = "no_admissible_work";
        break;
      }
      if (!hasEligibleWork(this.worker.ledger, includePaused, allowFailedRetry)) {
        stopReason = allTerminal(this.worker.ledger) ? "all_terminal" : "no_admissible_work";
        break;
      }
    }

    const summaries = cycles.map((cycle) => cycle.toJSON());
    const normalizedStatusCounts = Object.fromEntries(Object.entries(statusCounts).sort(([left], [right]) => left.localeCompare(right)));
    const normalizedDomainCounts = Object.fromEntries(Object.entries(domainCounts).sort(([left], [right]) => left.localeCompare(right)));
    const loopDigest = digestJsonSync({ schema: AUTONOMOUS_GOAL_CONTROL_LOOP_SCHEMA, cycles: summaries, stop_reason: stopReason, total_selected: totalSelected, total_claimed: totalClaimed, total_runs: totalRuns, status_counts: normalizedStatusCounts, domain_counts: normalizedDomainCounts, retention: AUTONOMOUS_GOAL_CONTROL_LOOP_RETENTION, secret_material: "never_returned" });
    return new AutonomousGoalControlLoopResult(cycles, stopReason, totalSelected, totalClaimed, totalRuns, normalizedStatusCounts, normalizedDomainCounts, loopDigest);
  }
}
