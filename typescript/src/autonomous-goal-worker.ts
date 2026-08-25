import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_RETENTION,
  goalStatusForResult,
  goalTaskDigest,
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalRecord,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import {
  AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED,
  AutonomousGoalScheduler,
  type AutonomousGoalSchedule,
  type AutonomousGoalScheduleRow,
  type AutonomousGoalClaimResult,
} from "./autonomous-goal-scheduler.js";
import { AutonomousGoalWorkerJournal } from "./autonomous-goal-worker-journal.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** The transient rehydration bridge between a metadata-only goal and a caller-owned executor. */
export const AUTONOMOUS_GOAL_WORKER_SCHEMA = "bioprism-autonomous-goal-worker/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_RETENTION = "metadata_only_goal_execution;task_and_execution_values_not_retained" as const;
export const AUTONOMOUS_GOAL_WORKER_MAX_RUNS = AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED;
export const AUTONOMOUS_GOAL_WORKER_MAX_TASK_BYTES = 32_000;

export type AutonomousGoalWorkerRunStatus = "completed" | "paused" | "blocked" | "failed";

export interface AutonomousGoalExecutionRequest {
  goal: AutonomousGoalRecord;
  schedule_row: AutonomousGoalScheduleRow;
  task: string;
  parameters: JsonObject;
  schedule_digest: string;
  task_digest: string;
  execution_binding_digest: string;
}

export interface AutonomousGoalWorkerResolution extends JsonObject {
  task: string;
  domain?: string;
  parameters?: JsonObject;
}

export interface AutonomousGoalWorkerOutcome extends JsonObject {
  status: string;
  criterion_updates?: JsonObject[];
  settlement_metadata?: {
    evaluator_digest?: string | null;
    learning_state_digest?: string | null;
    progress_digest?: string | null;
  };
}

export type AutonomousGoalResolver = (goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) => AutonomousGoalWorkerResolution | Promise<AutonomousGoalWorkerResolution>;
export type AutonomousGoalExecutor = (request: AutonomousGoalExecutionRequest) => unknown | Promise<unknown>;

export interface AutonomousGoalWorkerRun extends JsonObject {
  goal_id: string;
  domain: string;
  attempt: number;
  execution_status: AutonomousGoalWorkerRunStatus;
  goal_status: AutonomousGoalStatus;
  outcome_digest: string;
  schedule_digest: string;
  claim_digest: string;
  error_class: string | null;
  error_digest: string | null;
}

type LiveGoalWorkerRun = AutonomousGoalWorkerRun & { live_result?: any };

export interface AutonomousGoalWorkerBatchJSON extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_WORKER_SCHEMA;
  schedule: AutonomousGoalSchedule;
  claim: AutonomousGoalClaimResult | null;
  runs: AutonomousGoalWorkerRun[];
  counts: {
    selected: number;
    claimed: number;
    settled: number;
    completed: number;
    paused: number;
    blocked: number;
    failed: number;
  };
  worker_digest: string;
  retention: typeof AUTONOMOUS_GOAL_WORKER_RETENTION;
  goal_retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

export class AutonomousGoalWorkerBatch {
  constructor(
    readonly schedule: AutonomousGoalSchedule,
    readonly claim: AutonomousGoalClaimResult | null,
    readonly runs: readonly LiveGoalWorkerRun[],
    readonly worker_digest: string,
  ) {}

  get live_results(): unknown[] {
    return this.runs.map((run) => run.live_result);
  }

  toJSON(): AutonomousGoalWorkerBatchJSON {
    const runs = this.runs.map((run) => {
      const { live_result: _liveResult, ...metadata } = run;
      return metadata;
    });
    return {
      schema: AUTONOMOUS_GOAL_WORKER_SCHEMA,
      schedule: this.schedule,
      claim: this.claim,
      runs,
      counts: {
        selected: this.schedule.selected_goal_ids.length,
        claimed: this.claim?.claims.length ?? 0,
        settled: runs.length,
        completed: runs.filter((run) => run.goal_status === "completed").length,
        paused: runs.filter((run) => run.goal_status === "paused").length,
        blocked: runs.filter((run) => run.goal_status === "blocked").length,
        failed: runs.filter((run) => run.goal_status === "failed").length,
      },
      worker_digest: this.worker_digest,
      retention: AUTONOMOUS_GOAL_WORKER_RETENTION,
      goal_retention: AUTONOMOUS_GOAL_RETENTION,
      secret_material: "never_returned",
    };
  }
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal worker ${message}`);
}

function digest(value: unknown): string {
  return digestJsonSync(value);
}

function task(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > AUTONOMOUS_GOAL_WORKER_MAX_TASK_BYTES) fail("resolved task is outside its bounded contract");
  return value;
}

function status(value: unknown): string {
  const candidate = isObject(value) ? value.status : (value as { status?: unknown } | null)?.status;
  if (typeof candidate !== "string" || !candidate.trim() || candidate.includes("\u0000") || new TextEncoder().encode(candidate).byteLength > 128) fail("executor result status is outside its bounded contract");
  return candidate.trim();
}

function field(value: unknown, name: string, fallback: unknown): unknown {
  return isObject(value) && name in value ? value[name] : (value as Record<string, unknown> | null)?.[name] ?? fallback;
}

function criterionUpdates(value: unknown): JsonObject[] {
  const raw = field(value, "criterion_updates", []);
  if (raw === null || raw === undefined) return [];
  if (!Array.isArray(raw) || raw.length > 64 || raw.some((item) => !isObject(item))) fail("executor criterion_updates are outside their bounds");
  return raw as JsonObject[];
}

function settlementMetadata(value: unknown): Record<string, string | null> {
  const raw = field(value, "settlement_metadata", {});
  if (raw === null || raw === undefined) return {};
  if (!isObject(raw)) fail("executor settlement_metadata must be an object");
  const allowed = new Set(["evaluator_digest", "learning_state_digest", "progress_digest"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) fail("executor settlement_metadata contains unsupported fields");
  const normalized: Record<string, string | null> = {};
  for (const [key, item] of Object.entries(raw)) {
    if (item !== null && (typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item))) fail(`executor settlement_metadata.${key} must be a lowercase SHA-256 digest or null`);
    normalized[key] = item as string | null;
  }
  return normalized;
}

export class AutonomousGoalWorker {
  readonly ledger: InMemoryAutonomousGoalLedger;
  readonly resolver: AutonomousGoalResolver;
  readonly executor: AutonomousGoalExecutor;
  readonly scheduler: AutonomousGoalScheduler;
  readonly journal: AutonomousGoalWorkerJournal | undefined;

  constructor(options: { ledger: InMemoryAutonomousGoalLedger; resolver: AutonomousGoalResolver; executor: AutonomousGoalExecutor; scheduler?: AutonomousGoalScheduler; journal?: AutonomousGoalWorkerJournal }) {
    if (!(options?.ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (typeof options.resolver !== "function") fail("resolver must be callable");
    if (typeof options.executor !== "function") fail("executor must be callable");
    if (options.scheduler !== undefined && !(options.scheduler instanceof AutonomousGoalScheduler)) fail("scheduler must be an AutonomousGoalScheduler");
    if (options.journal !== undefined && !(options.journal instanceof AutonomousGoalWorkerJournal)) fail("journal must be an AutonomousGoalWorkerJournal");
    this.ledger = options.ledger;
    this.resolver = options.resolver;
    this.executor = options.executor;
    this.scheduler = options.scheduler ?? new AutonomousGoalScheduler();
    this.journal = options.journal;
  }

  async run(options: { schedule_options?: Record<string, unknown>; batch_id?: string } = {}): Promise<AutonomousGoalWorkerBatch> {
    if (options.schedule_options !== undefined && !isObject(options.schedule_options)) fail("schedule_options must be an object");
    if (this.journal !== undefined && (typeof options.batch_id !== "string" || !options.batch_id.trim() || options.batch_id.includes("\u0000") || new TextEncoder().encode(options.batch_id).byteLength > 256)) fail("batch_id is required and bounded when a journal is configured");
    const scheduleOptions = options.schedule_options ?? {};
    // The ledger intentionally caps one bounded listing at 512 rows.  The scheduler's
    // admission cap is smaller, so this is enough to make one worker pass deterministic.
    const schedule = this.scheduler.plan(this.ledger.list({ limit: 512 }), scheduleOptions);
    const rows = new Map(schedule.rows.filter((row) => row.decision === "admit").map((row) => [row.goal_id, row]));
    const prepared = new Map<string, AutonomousGoalExecutionRequest>();
    for (const goalId of schedule.selected_goal_ids) {
      const goal = this.ledger.get(goalId);
      const row = rows.get(goalId);
      if (!goal || !row) fail(`schedule admission disappeared for goal ${goalId}`);
      this.journal?.assertNoActive(goalId);
      const resolved = await this.resolver(goal, row);
      if (!isObject(resolved)) fail(`resolver returned a non-object for goal ${goalId}`);
      const resolvedDomain = resolved.domain ?? goal.domain;
      if (resolvedDomain !== goal.domain) fail(`resolver domain does not match goal ${goalId}`);
      const parameters = resolved.parameters ?? {};
      if (!isObject(parameters)) fail(`resolver parameters must be an object for goal ${goalId}`);
      const resolvedTask = task(resolved.task);
      if (goalTaskDigest(resolvedTask) !== goal.task_digest) fail(`resolver task digest does not match goal ${goalId}`);
      const clonedParameters = structuredClone(parameters);
      prepared.set(goalId, { goal, schedule_row: row, task: resolvedTask, parameters: clonedParameters, schedule_digest: schedule.schedule_digest, task_digest: goal.task_digest, execution_binding_digest: digest({ parameters: clonedParameters }) });
    }
    if (this.journal !== undefined && options.batch_id !== undefined) {
      for (const request of prepared.values()) this.journal.record({ batch_id: options.batch_id, goal_id: request.goal.goal_id, phase: "prepared", attempt: request.goal.attempt, revision: request.goal.revision, schedule_digest: schedule.schedule_digest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
    }
    const claim = schedule.selected_goal_ids.length === 0 ? null : this.scheduler.claim(this.ledger, schedule, { now_ns: typeof scheduleOptions.now_ns === "number" ? scheduleOptions.now_ns : undefined });
    const runs: LiveGoalWorkerRun[] = [];
    if (claim) {
      const claimById = new Map(claim.claims.map((item) => [item.goal_id, item]));
      if (this.journal !== undefined && options.batch_id !== undefined) {
        for (const item of claim.claims) {
          const current = this.ledger.get(item.goal_id);
          if (!current) fail(`claimed goal ${item.goal_id} disappeared before journaling`);
          const request = prepared.get(item.goal_id);
          if (!request) fail(`prepared request for goal ${item.goal_id} disappeared before journaling`);
          this.journal.record({ batch_id: options.batch_id, goal_id: item.goal_id, phase: "claimed", attempt: current.attempt, revision: current.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
        }
      }
      for (const goalId of schedule.selected_goal_ids) {
        const claimRow = claimById.get(goalId)!;
        const request = prepared.get(goalId)!;
        const current = this.ledger.get(goalId);
        if (!current || current.status !== "running" || current.revision !== claimRow.running_revision) fail(`claimed goal ${goalId} changed before execution`);
        try {
          if (this.journal !== undefined && options.batch_id !== undefined) this.journal.record({ batch_id: options.batch_id, goal_id: goalId, phase: "dispatch_started", attempt: current.attempt, revision: current.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
          const liveResult = await this.executor(request);
          const resultStatus = status(liveResult);
          const updated = this.settle(current, resultStatus, liveResult);
          const outcomeDigest = digest({ goal_id: goalId, attempt: current.attempt, result_status: resultStatus });
          if (this.journal !== undefined && options.batch_id !== undefined) this.journal.record({ batch_id: options.batch_id, goal_id: goalId, phase: "settled", attempt: current.attempt, revision: updated.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, outcome_digest: outcomeDigest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
          runs.push({ goal_id: goalId, domain: current.domain, attempt: current.attempt, execution_status: updated.status as AutonomousGoalWorkerRunStatus, goal_status: updated.status, outcome_digest: outcomeDigest, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, error_class: null, error_digest: null, live_result: liveResult as any });
        } catch (error) {
          const errorClass = error instanceof Error ? error.constructor.name : "UnknownError";
          const outcomeDigest = digest({ goal_id: goalId, attempt: current.attempt, result_status: `exception:${errorClass}` });
          let updated;
          try {
            updated = this.ledger.transition(goalId, "failed", { expected_revision: current.revision, blockers: [`exception:${errorClass}`], next_action_digest: goalTaskDigest("goal-retry"), outcome_digest: outcomeDigest });
          } catch (transitionError) {
            const wrapped = new ArgumentError(`goal ${goalId} failed without a durable failure transition`);
            (wrapped as Error & { cause?: unknown }).cause = transitionError;
            throw wrapped;
          }
          if (this.journal !== undefined && options.batch_id !== undefined) this.journal.record({ batch_id: options.batch_id, goal_id: goalId, phase: "failed", attempt: current.attempt, revision: updated.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, outcome_digest: outcomeDigest, error_digest: digest({ error_class: errorClass }), task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
          runs.push({ goal_id: goalId, domain: current.domain, attempt: current.attempt, execution_status: "failed", goal_status: updated.status, outcome_digest: outcomeDigest, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, error_class: errorClass, error_digest: digest({ error_class: errorClass }) });
        }
      }
    }
    const metadataRuns = runs.map(({ live_result: _liveResult, ...metadata }) => metadata);
    const workerDigest = digest({ schema: AUTONOMOUS_GOAL_WORKER_SCHEMA, schedule_digest: schedule.schedule_digest, claim_digest: claim?.claim_digest ?? null, runs: metadataRuns, retention: AUTONOMOUS_GOAL_WORKER_RETENTION, goal_retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" });
    return new AutonomousGoalWorkerBatch(schedule, claim, runs, workerDigest);
  }

  private settle(current: AutonomousGoalRecord, resultStatus: string, result: unknown): AutonomousGoalRecord {
    const outcomeDigest = digest({ goal_id: current.goal_id, attempt: current.attempt, result_status: resultStatus });
    const updates = criterionUpdates(result);
    const settled = updates.length === 0 ? current : this.ledger.updateCriteria(current.goal_id, updates, { expected_revision: current.revision });
    const criteriaComplete = settled.criteria.every((criterion) => !criterion.required || ["satisfied", "waived"].includes(criterion.status));
    const target = goalStatusForResult(resultStatus, criteriaComplete);
    const metadata = settlementMetadata(result);
    return this.ledger.transition(current.goal_id, target, { expected_revision: settled.revision, blockers: target === "completed" ? [] : [`result:${resultStatus}`], next_action_digest: target === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`), outcome_digest: outcomeDigest, ...metadata });
  }
}
