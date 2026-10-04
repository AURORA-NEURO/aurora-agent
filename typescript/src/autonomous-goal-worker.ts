import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_MAX_BLOCKERS,
  AUTONOMOUS_GOAL_MAX_GOALS,
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
  type AutonomousGoalScheduleRowSnapshot,
  type AutonomousGoalScheduleSnapshot,
  type AutonomousGoalClaim,
  type AutonomousGoalClaimResult,
  type AutonomousGoalClaimResultSnapshot,
} from "./autonomous-goal-scheduler.js";
import { AutonomousGoalWorkerJournal, type AutonomousGoalWorkerEvent } from "./autonomous-goal-worker-journal.js";
import { digestJsonSync, isUnicodeScalarString } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** The transient rehydration bridge between a metadata-only goal and a caller-owned executor. */
export const AUTONOMOUS_GOAL_WORKER_SCHEMA = "bioprism-autonomous-goal-worker/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_RETENTION = "metadata_only_goal_execution;task_and_execution_values_not_retained" as const;
export const AUTONOMOUS_GOAL_WORKER_MAX_RUNS = AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED;
export const AUTONOMOUS_GOAL_WORKER_MAX_TASK_BYTES = 32_000;

export type AutonomousGoalWorkerRunStatus = "completed" | "paused" | "blocked" | "failed";

export type AutonomousGoalWorkerReadonlyJsonValue =
  | string
  | number
  | boolean
  | null
  | AutonomousGoalWorkerReadonlyJsonObject
  | readonly AutonomousGoalWorkerReadonlyJsonValue[];

export interface AutonomousGoalWorkerReadonlyJsonObject {
  readonly [key: string]: AutonomousGoalWorkerReadonlyJsonValue | undefined;
}

/** Immutable callback view of a durable goal record. */
export type AutonomousGoalWorkerGoal = Pick<Readonly<AutonomousGoalRecord>,
  | "schema" | "goal_id" | "task_digest" | "domain" | "capability" | "risk_class" | "status"
  | "attempt" | "max_attempts" | "revision" | "next_action_digest" | "outcome_digest"
  | "evaluator_digest" | "learning_state_digest" | "progress_digest" | "created_ns" | "updated_ns"
  | "state_digest" | "retention" | "secret_material"
> & {
  readonly criteria: readonly Readonly<AutonomousGoalRecord["criteria"][number]>[];
  readonly blockers: readonly string[];
};

/** Immutable callback view of a schedule row, including its sealed dependency set. */
export type AutonomousGoalWorkerScheduleRow = Pick<Readonly<AutonomousGoalScheduleRow>,
  | "goal_id" | "domain" | "status" | "revision" | "attempt" | "max_attempts" | "priority" | "urgency"
  | "deadline_ns" | "estimated_cost" | "age_score" | "deadline_score" | "retry_pressure" | "score"
  | "efficiency" | "decision" | "reason" | "expected_revision"
> & {
  readonly dependencies: readonly string[];
  readonly unmet_dependencies: readonly string[];
};

export interface AutonomousGoalExecutionRequest {
  readonly goal: AutonomousGoalWorkerGoal;
  readonly schedule_row: AutonomousGoalWorkerScheduleRow;
  readonly task: string;
  readonly parameters: AutonomousGoalWorkerReadonlyJsonObject;
  readonly schedule_digest: string;
  readonly task_digest: string;
  readonly execution_binding_digest: string;
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

export type AutonomousGoalResolver = (goal: AutonomousGoalWorkerGoal, row: AutonomousGoalWorkerScheduleRow) => AutonomousGoalWorkerResolution | Promise<AutonomousGoalWorkerResolution>;
export type AutonomousGoalExecutor = (request: AutonomousGoalExecutionRequest) => unknown | Promise<unknown>;
/** Caller-owned durable commit that must finish before an executor can cross the dispatch boundary. */
export type AutonomousGoalDispatchIntentPersister = (event: Readonly<AutonomousGoalWorkerEvent>) => void | Promise<void>;

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

type LiveGoalWorkerRun = AutonomousGoalWorkerRun & { live_result?: any; dispatched: boolean };

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
    readonly schedule: AutonomousGoalScheduleSnapshot,
    readonly claim: AutonomousGoalClaimResultSnapshot | null,
    readonly runs: readonly LiveGoalWorkerRun[],
    readonly worker_digest: string,
  ) {}

  get live_results(): unknown[] {
    return this.runs.map((run) => run.live_result);
  }

  toJSON(): AutonomousGoalWorkerBatchJSON {
    const runs = this.runs.map((run) => {
      const { live_result: _liveResult, dispatched: _dispatched, ...metadata } = run;
      return metadata;
    });
    return {
      schema: AUTONOMOUS_GOAL_WORKER_SCHEMA,
      schedule: structuredClone(this.schedule) as AutonomousGoalSchedule,
      claim: this.claim === null ? null : structuredClone(this.claim) as AutonomousGoalClaimResult,
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

function isGoalWorkerWhitespace(codePoint: number): boolean {
  return (codePoint >= 0x09 && codePoint <= 0x0d)
    || (codePoint >= 0x1c && codePoint <= 0x20)
    || codePoint === 0x85
    || codePoint === 0xa0
    || codePoint === 0x1680
    || (codePoint >= 0x2000 && codePoint <= 0x200a)
    || codePoint === 0x2028
    || codePoint === 0x2029
    || codePoint === 0x202f
    || codePoint === 0x205f
    || codePoint === 0x3000;
}

function trimGoalWorkerWhitespace(value: string): string {
  let start = 0;
  let end = value.length;
  while (start < end && isGoalWorkerWhitespace(value.charCodeAt(start))) start += 1;
  while (end > start && isGoalWorkerWhitespace(value.charCodeAt(end - 1))) end -= 1;
  return value.slice(start, end);
}

function snapshotGoalRecord(goal: AutonomousGoalRecord): AutonomousGoalWorkerGoal {
  const snapshot = structuredClone(goal);
  snapshot.criteria.forEach((criterion) => Object.freeze(criterion));
  Object.freeze(snapshot.criteria);
  Object.freeze(snapshot.blockers);
  return Object.freeze(snapshot) as AutonomousGoalWorkerGoal;
}

function snapshotScheduleRow(row: AutonomousGoalScheduleRowSnapshot): AutonomousGoalWorkerScheduleRow {
  const snapshot = structuredClone(row);
  Object.freeze(snapshot.dependencies);
  Object.freeze(snapshot.unmet_dependencies);
  return Object.freeze(snapshot) as AutonomousGoalWorkerScheduleRow;
}

function snapshotExecutionParameters(parameters: JsonObject): AutonomousGoalWorkerReadonlyJsonObject {
  const active = new WeakSet<object>();
  const clone = (value: unknown, depth: number): AutonomousGoalWorkerReadonlyJsonValue => {
    if (depth > 100) fail("resolver parameters exceed the JSON nesting limit");
    if (value === null || typeof value === "boolean") return value;
    if (typeof value === "string") {
      if (!isUnicodeScalarString(value)) fail("resolver parameters contain invalid Unicode");
      return value;
    }
    if (typeof value === "number") {
      if (!Number.isFinite(value)) fail("resolver parameters contain a non-finite number");
      return value;
    }
    if (typeof value !== "object" || value === null) fail("resolver parameters must contain only JSON values");
    if (active.has(value)) fail("resolver parameters must not contain cycles");
    active.add(value);
    try {
      if (Array.isArray(value)) {
        if (Object.getPrototypeOf(value) !== Array.prototype || Reflect.ownKeys(value).length !== value.length + 1) {
          fail("resolver parameter arrays must be dense JSON arrays");
        }
        const entries: AutonomousGoalWorkerReadonlyJsonValue[] = [];
        for (let index = 0; index < value.length; index += 1) {
          if (!Object.prototype.hasOwnProperty.call(value, index)) fail("resolver parameter arrays must not contain holes");
          entries.push(clone(value[index], depth + 1));
        }
        return Object.freeze(entries);
      }
      const prototype = Object.getPrototypeOf(value);
      if (prototype !== Object.prototype && prototype !== null) fail("resolver parameters must use plain JSON objects");
      const keys = Reflect.ownKeys(value);
      if (keys.some((key) => typeof key !== "string")) fail("resolver parameter keys must be strings");
      const entries: Record<string, AutonomousGoalWorkerReadonlyJsonValue> = {};
      for (const key of keys as string[]) {
        const descriptor = Object.getOwnPropertyDescriptor(value, key);
        if (!descriptor?.enumerable || !("value" in descriptor)) fail("resolver parameters must contain plain data properties");
        if (!isUnicodeScalarString(key)) fail("resolver parameter keys contain invalid Unicode");
        // defineProperty preserves an own JSON `__proto__` key without changing the prototype.
        Object.defineProperty(entries, key, {
          value: clone(descriptor.value, depth + 1),
          enumerable: true,
          configurable: true,
          writable: true,
        });
      }
      return Object.freeze(entries);
    } finally {
      active.delete(value);
    }
  };
  const snapshot = clone(parameters, 0);
  if (!isObject(snapshot)) fail("resolver parameters must be a JSON object");
  return snapshot;
}

function task(value: unknown): string {
  if (typeof value !== "string" || !isUnicodeScalarString(value) || !trimGoalWorkerWhitespace(value) || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > AUTONOMOUS_GOAL_WORKER_MAX_TASK_BYTES) fail("resolved task is outside its bounded contract");
  return value;
}

function status(value: unknown): string {
  const candidate = isObject(value) ? value.status : (value as { status?: unknown } | null)?.status;
  if (typeof candidate !== "string" || !isUnicodeScalarString(candidate) || !trimGoalWorkerWhitespace(candidate) || candidate.includes("\u0000") || new TextEncoder().encode(candidate).byteLength > 128) fail("executor result status is outside its bounded contract");
  return trimGoalWorkerWhitespace(candidate);
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
  readonly persist_dispatch_intent: AutonomousGoalDispatchIntentPersister | undefined;
  private persistDispatchIntentTail: Promise<void> = Promise.resolve();

  constructor(options: { ledger: InMemoryAutonomousGoalLedger; resolver: AutonomousGoalResolver; executor: AutonomousGoalExecutor; scheduler?: AutonomousGoalScheduler; journal?: AutonomousGoalWorkerJournal; persist_dispatch_intent?: AutonomousGoalDispatchIntentPersister }) {
    if (!(options?.ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (typeof options.resolver !== "function") fail("resolver must be callable");
    if (typeof options.executor !== "function") fail("executor must be callable");
    if (options.scheduler !== undefined && !(options.scheduler instanceof AutonomousGoalScheduler)) fail("scheduler must be an AutonomousGoalScheduler");
    if (options.journal !== undefined && !(options.journal instanceof AutonomousGoalWorkerJournal)) fail("journal must be an AutonomousGoalWorkerJournal");
    if (options.persist_dispatch_intent !== undefined && typeof options.persist_dispatch_intent !== "function") fail("persist_dispatch_intent must be callable");
    if (options.persist_dispatch_intent !== undefined && options.journal === undefined) fail("persist_dispatch_intent requires a worker journal");
    this.ledger = options.ledger;
    this.resolver = options.resolver;
    this.executor = options.executor;
    this.scheduler = options.scheduler ?? new AutonomousGoalScheduler();
    this.journal = options.journal;
    this.persist_dispatch_intent = options.persist_dispatch_intent;
  }

  async run(options: { schedule_options?: Record<string, unknown>; batch_id?: string } = {}): Promise<AutonomousGoalWorkerBatch> {
    const release = this.journal?.beginWorkerRun();
    try {
      return await this.runAdmitted(options);
    } finally {
      release?.();
    }
  }

  private async runAdmitted(options: { schedule_options?: Record<string, unknown>; batch_id?: string }): Promise<AutonomousGoalWorkerBatch> {
    if (options.schedule_options !== undefined && !isObject(options.schedule_options)) fail("schedule_options must be an object");
    if (this.journal !== undefined && (typeof options.batch_id !== "string" || !isUnicodeScalarString(options.batch_id) || !trimGoalWorkerWhitespace(options.batch_id) || options.batch_id.includes("\u0000") || new TextEncoder().encode(options.batch_id).byteLength > 256)) fail("batch_id is required and bounded when a journal is configured");
    const scheduleOptions = options.schedule_options ?? {};
    // Dependency closure requires reading the full bounded ledger, including older prerequisites.
    const schedule = this.scheduler.plan(this.ledger.list({ limit: AUTONOMOUS_GOAL_MAX_GOALS }), scheduleOptions);
    if (this.journal !== undefined) this.journal.requireCapacity(6 * schedule.selected_goal_ids.length);
    const rows = new Map(schedule.rows.filter((row) => row.decision === "admit").map((row) => [row.goal_id, row]));
    const prepared = new Map<string, AutonomousGoalExecutionRequest>();
    for (const goalId of schedule.selected_goal_ids) {
      const ledgerGoal = this.ledger.get(goalId);
      const row = rows.get(goalId);
      if (!ledgerGoal || !row) fail(`schedule admission disappeared for goal ${goalId}`);
      const goal = snapshotGoalRecord(ledgerGoal);
      this.journal?.assertNoActive(goalId);
      const resolverRow = snapshotScheduleRow(row);
      const resolved = await this.resolver(goal, resolverRow);
      if (!isObject(resolved)) fail(`resolver returned a non-object for goal ${goalId}`);
      const resolvedDomain = resolved.domain ?? goal.domain;
      if (resolvedDomain !== goal.domain) fail(`resolver domain does not match goal ${goalId}`);
      const parameters = resolved.parameters ?? {};
      if (!isObject(parameters)) fail(`resolver parameters must be an object for goal ${goalId}`);
      const resolvedTask = task(resolved.task);
      if (goalTaskDigest(resolvedTask) !== goal.task_digest) fail(`resolver task digest does not match goal ${goalId}`);
      const clonedParameters = snapshotExecutionParameters(parameters);
      prepared.set(goalId, Object.freeze({
        goal,
        schedule_row: resolverRow,
        task: resolvedTask,
        parameters: clonedParameters,
        schedule_digest: schedule.schedule_digest,
        task_digest: goal.task_digest,
        execution_binding_digest: digest({ parameters: clonedParameters }),
      }));
    }
    const selected = [...schedule.selected_goal_ids];
    const selectedSet = new Set(selected);
    const completedWaves = new Set<string>();
    const executionWaves: string[][] = [];
    let remaining = selected;
    while (remaining.length > 0) {
      const wave = remaining.filter((goalId) => (prepared.get(goalId)?.schedule_row.dependencies ?? []).every((dependencyId) => !selectedSet.has(dependencyId) || completedWaves.has(dependencyId)));
      if (wave.length === 0) fail("schedule cannot be divided into dependency-closed execution waves");
      executionWaves.push(wave);
      wave.forEach((goalId) => completedWaves.add(goalId));
      remaining = remaining.filter((goalId) => !completedWaves.has(goalId));
    }
    if (this.journal !== undefined && options.batch_id !== undefined) {
      for (const request of prepared.values()) this.journal.record({ batch_id: options.batch_id, goal_id: request.goal.goal_id, phase: "prepared", attempt: request.goal.attempt, revision: request.goal.revision, schedule_digest: schedule.schedule_digest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
    }
    const claim = schedule.selected_goal_ids.length === 0 ? null : this.scheduler.claim(this.ledger, schedule, { now_ns: schedule.now_ns });
    const runsById = new Map<string, LiveGoalWorkerRun>();
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
      for (const wave of executionWaves) {
        for (let offset = 0; offset < wave.length; offset += schedule.max_concurrent) {
          const boundedWave = wave.slice(offset, offset + schedule.max_concurrent);
          const waveResults = await Promise.allSettled(boundedWave.map((goalId) => this.executeClaimed(goalId, claimById.get(goalId)!, prepared.get(goalId)!, claim, schedule, options.batch_id)));
          // Drain every execution in the wave before surfacing an unexpected worker failure.
          // Promise.all rejects as soon as one task fails, which would release the worker-run
          // fence while siblings were still settling ledger and journal state.
          const firstFailure = waveResults.find((result): result is PromiseRejectedResult => result.status === "rejected");
          if (firstFailure !== undefined) throw firstFailure.reason;
          const waveRuns = waveResults.map((result) => (result as PromiseFulfilledResult<LiveGoalWorkerRun>).value);
          waveRuns.forEach((run) => runsById.set(run.goal_id, run));
        }
      }
    }
    const runs = schedule.selected_goal_ids.flatMap((goalId) => {
      const run = runsById.get(goalId);
      return run === undefined ? [] : [run];
    });
    const metadataRuns = runs.map(({ live_result: _liveResult, dispatched: _dispatched, ...metadata }) => metadata);
    const workerDigest = digest({ schema: AUTONOMOUS_GOAL_WORKER_SCHEMA, schedule_digest: schedule.schedule_digest, claim_digest: claim?.claim_digest ?? null, runs: metadataRuns, retention: AUTONOMOUS_GOAL_WORKER_RETENTION, goal_retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" });
    return new AutonomousGoalWorkerBatch(schedule, claim, runs, workerDigest);
  }

  private async executeClaimed(
    goalId: string,
    claimRow: AutonomousGoalClaim,
    request: AutonomousGoalExecutionRequest,
    claim: AutonomousGoalClaimResultSnapshot,
    schedule: AutonomousGoalScheduleSnapshot,
    batchId: string | undefined,
  ): Promise<LiveGoalWorkerRun> {
    const current = this.ledger.get(goalId);
    if (!current || current.status !== "running" || current.revision !== claimRow.running_revision) fail(`claimed goal ${goalId} changed before execution`);
    const unmetDependencies = request.schedule_row.dependencies.flatMap((dependencyId) => {
      const dependency = this.ledger.get(dependencyId);
      return dependency?.status === "completed" ? [] : [{ goal_id: dependencyId, status: dependency?.status ?? "missing" }];
    });
    if (unmetDependencies.length > 0) {
      const outcomeDigest = digest({ goal_id: goalId, attempt: current.attempt, result_status: "dependency_not_completed", dependencies: unmetDependencies });
      let blockers = unmetDependencies.map((dependency) => `dependency_not_completed:${digest({ goal_id: dependency.goal_id })}:${dependency.status}`);
      if (blockers.length > AUTONOMOUS_GOAL_MAX_BLOCKERS) {
        const omitted = blockers.length - AUTONOMOUS_GOAL_MAX_BLOCKERS + 1;
        blockers = [...blockers.slice(0, AUTONOMOUS_GOAL_MAX_BLOCKERS - 1), `dependency_not_completed:additional:${omitted}`];
      }
      const updated = this.ledger.transition(goalId, "paused", { expected_revision: current.revision, blockers, next_action_digest: goalTaskDigest("goal-dependency-retry"), outcome_digest: outcomeDigest });
      if (this.journal !== undefined && batchId !== undefined) this.journal.record({ batch_id: batchId, goal_id: goalId, phase: "settled", attempt: current.attempt, revision: updated.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, outcome_digest: outcomeDigest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
      return { goal_id: goalId, domain: current.domain, attempt: current.attempt, execution_status: "paused", goal_status: updated.status, outcome_digest: outcomeDigest, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, error_class: null, error_digest: null, live_result: null, dispatched: false };
    }
    if (this.journal !== undefined && batchId !== undefined) {
      this.journal.requireCapacity(2);
      const event = this.journal.record({ batch_id: batchId, goal_id: goalId, phase: "dispatch_started", attempt: current.attempt, revision: current.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
      if (this.persist_dispatch_intent !== undefined) await this.persistDispatchIntent(Object.freeze({ ...event }));
    }
    let liveResult: unknown;
    let updated: AutonomousGoalRecord;
    let outcomeDigest: string;
    try {
      liveResult = await this.executor(request);
      const resultStatus = status(liveResult);
      updated = this.settle(current, resultStatus, liveResult);
      outcomeDigest = digest({ goal_id: goalId, attempt: current.attempt, result_status: resultStatus });
    } catch (error) {
      const errorClass = error instanceof Error ? error.constructor.name : "UnknownError";
      let failedUpdate: AutonomousGoalRecord | null;
      let failureOutcomeDigest: string;
      if (this.journal !== undefined) {
        this.journal.recover(this.ledger, { goal_id: goalId });
        failedUpdate = this.ledger.get(goalId);
        if (failedUpdate === null || failedUpdate.status !== "blocked" || failedUpdate.outcome_digest === null || !failedUpdate.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) {
          const wrapped = new ArgumentError(`goal ${goalId} dispatch outcome is unknown and could not be fenced for reconciliation`);
          (wrapped as Error & { cause?: unknown }).cause = error;
          throw wrapped;
        }
        failureOutcomeDigest = failedUpdate.outcome_digest;
      } else {
        const latest = this.ledger.get(goalId);
        if (latest === null || latest.status !== "running" || latest.attempt !== current.attempt) {
          const wrapped = new ArgumentError(`goal ${goalId} dispatch outcome is unknown and its running state changed`);
          (wrapped as Error & { cause?: unknown }).cause = error;
          throw wrapped;
        }
        failureOutcomeDigest = digest({ goal_id: goalId, attempt: current.attempt, result_status: "worker_dispatch_outcome_unknown", error_class: errorClass });
        failedUpdate = this.ledger.transition(goalId, "blocked", { expected_revision: latest.revision, blockers: ["worker_dispatch_outcome_requires_reconciliation"], next_action_digest: goalTaskDigest("goal-reconciliation-review"), outcome_digest: failureOutcomeDigest });
      }
      return { goal_id: goalId, domain: current.domain, attempt: current.attempt, execution_status: "blocked", goal_status: failedUpdate.status, outcome_digest: failureOutcomeDigest, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, error_class: errorClass, error_digest: digest({ error_class: errorClass }), live_result: null, dispatched: true };
    }
    if (this.journal !== undefined && batchId !== undefined) this.journal.record({ batch_id: batchId, goal_id: goalId, phase: "settled", attempt: current.attempt, revision: updated.revision, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, outcome_digest: outcomeDigest, task_digest: request.task_digest, execution_binding_digest: request.execution_binding_digest });
    return { goal_id: goalId, domain: current.domain, attempt: current.attempt, execution_status: updated.status as AutonomousGoalWorkerRunStatus, goal_status: updated.status, outcome_digest: outcomeDigest, schedule_digest: schedule.schedule_digest, claim_digest: claim.claim_digest, error_class: null, error_digest: null, live_result: liveResult as any, dispatched: true };
  }

  private persistDispatchIntent(event: Readonly<AutonomousGoalWorkerEvent>): Promise<void> {
    const previous = this.persistDispatchIntentTail;
    const operation = previous.then(() => this.persist_dispatch_intent!(event));
    this.persistDispatchIntentTail = operation.then(() => undefined, () => undefined);
    return operation;
  }

  private settle(current: AutonomousGoalRecord, resultStatus: string, result: unknown): AutonomousGoalRecord {
    const outcomeDigest = digest({ goal_id: current.goal_id, attempt: current.attempt, result_status: resultStatus });
    const updates = criterionUpdates(result);
    const metadata = settlementMetadata(result);
    const statuses = new Map(current.criteria.map((criterion) => [criterion.criterion_id, criterion.status]));
    for (const update of updates) {
      if (typeof update.criterion_id === "string" && statuses.has(update.criterion_id) && update.status !== undefined) statuses.set(update.criterion_id, update.status as AutonomousGoalRecord["criteria"][number]["status"]);
    }
    const criteriaComplete = current.criteria.every((criterion) => !criterion.required || ["satisfied", "waived"].includes(statuses.get(criterion.criterion_id) ?? criterion.status));
    const target = goalStatusForResult(resultStatus, criteriaComplete);
    return this.ledger.transition(current.goal_id, target, { expected_revision: current.revision, criterion_updates: updates, blockers: target === "completed" ? [] : [`result:${resultStatus}`], next_action_digest: target === "completed" ? null : goalTaskDigest(`goal-next:${resultStatus}`), outcome_digest: outcomeDigest, ...metadata });
  }
}
