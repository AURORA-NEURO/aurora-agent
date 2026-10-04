import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_MAX_GOALS,
  AUTONOMOUS_GOAL_RETENTION,
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalRecord,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, compareUnicodeScalars, digestJsonSync, isUnicodeScalarString } from "./tooling.js";
import type { JsonObject } from "./types.js";
import {
  autonomousGoalTimestampNowNs,
  normalizeAutonomousGoalTimestampNs,
  requireAutonomousGoalTimestampNsWire,
  type AutonomousGoalTimestampInput,
  type AutonomousGoalTimestampNs,
} from "./autonomous-goal-time.js";

/** A value-only, replayable admission plan for long-horizon autonomous goals. */
export const AUTONOMOUS_GOAL_SCHEDULE_SCHEMA = "bioprism-autonomous-goal-schedule/0.2" as const;
/** A value-only receipt for goals optimistically claimed by a scheduler worker. */
export const AUTONOMOUS_GOAL_CLAIM_SCHEMA = "bioprism-autonomous-goal-claim/0.2" as const;
export const AUTONOMOUS_GOAL_SCHEDULE_RETENTION = "metadata_only_goal_admission;task_text_and_payloads_not_retained" as const;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_GOALS = AUTONOMOUS_GOAL_MAX_GOALS;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SIGNALS = 4_096;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_DEPENDENCIES = 64;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED = 128;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SNAPSHOT_BYTES = 2_000_000;
// cross_domain is already a first-class member of the shared autonomous domain catalogue.
export const AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS = AUTONOMOUS_DOMAIN_NAMES;

export type AutonomousGoalScheduleDecision = "active" | "admit" | "defer" | "ineligible";
export type AutonomousGoalSchedulingDomain = AutonomousDomainName | "cross_domain";

export interface AutonomousGoalSchedulingSignal {
  goal_id: string;
  /** Caller/evaluator-owned urgency, normalized to [0, 1]. */
  priority?: number;
  urgency?: number;
  deadline_ns?: string | number | null;
  estimated_cost?: number;
  dependencies?: readonly string[];
}

export interface AutonomousGoalSchedulingOptions {
  now_ns?: AutonomousGoalTimestampInput;
  max_selected?: number;
  max_concurrent?: number;
  max_cost?: number;
  aging_window_ns?: AutonomousGoalTimestampInput;
  allow_failed_retry?: boolean;
  include_paused?: boolean;
  signals?: readonly AutonomousGoalSchedulingSignal[];
  required_domains?: readonly AutonomousGoalSchedulingDomain[];
  domain_quotas?: Readonly<Record<string, number>>;
}

export interface AutonomousGoalScheduleRow extends JsonObject {
  goal_id: string;
  domain: AutonomousGoalSchedulingDomain;
  status: AutonomousGoalStatus;
  revision: number;
  attempt: number;
  max_attempts: number;
  priority: number;
  urgency: number;
  deadline_ns: AutonomousGoalTimestampNs | null;
  estimated_cost: number;
  age_score: number;
  deadline_score: number;
  retry_pressure: number;
  score: number;
  efficiency: number;
  dependencies: string[];
  unmet_dependencies: string[];
  decision: AutonomousGoalScheduleDecision;
  reason: string;
  expected_revision: number;
}

export interface AutonomousGoalScheduleCoverage extends JsonObject {
  required_domains: AutonomousGoalSchedulingDomain[];
  selected_domains: AutonomousGoalSchedulingDomain[];
  missing_domains: AutonomousGoalSchedulingDomain[];
}

export interface AutonomousGoalSchedule extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_SCHEDULE_SCHEMA;
  now_ns: AutonomousGoalTimestampNs;
  max_selected: number;
  max_concurrent: number;
  max_cost: number;
  active_count: number;
  used_cost: number;
  selected_goal_ids: string[];
  rows: AutonomousGoalScheduleRow[];
  coverage: AutonomousGoalScheduleCoverage;
  schedule_digest: string;
  retention: typeof AUTONOMOUS_GOAL_SCHEDULE_RETENTION;
  secret_material: "never_returned";
}

/** Readonly schedule row returned by planning and replay validation. */
export type AutonomousGoalScheduleRowSnapshot = Pick<Readonly<AutonomousGoalScheduleRow>,
  | "goal_id" | "domain" | "status" | "revision" | "attempt" | "max_attempts" | "priority" | "urgency"
  | "deadline_ns" | "estimated_cost" | "age_score" | "deadline_score" | "retry_pressure" | "score"
  | "efficiency" | "decision" | "reason" | "expected_revision"
> & {
  readonly dependencies: readonly string[];
  readonly unmet_dependencies: readonly string[];
};

/** Readonly, digest-bound scheduler result shared with callbacks and control loops. */
export type AutonomousGoalScheduleSnapshot = Pick<Readonly<AutonomousGoalSchedule>,
  | "schema" | "now_ns" | "max_selected" | "max_concurrent" | "max_cost" | "active_count" | "used_cost"
  | "schedule_digest" | "retention" | "secret_material"
> & {
  readonly selected_goal_ids: readonly string[];
  readonly rows: readonly AutonomousGoalScheduleRowSnapshot[];
  readonly coverage: {
    readonly required_domains: readonly AutonomousGoalSchedulingDomain[];
    readonly selected_domains: readonly AutonomousGoalSchedulingDomain[];
    readonly missing_domains: readonly AutonomousGoalSchedulingDomain[];
  };
};

export interface AutonomousGoalClaim extends JsonObject {
  goal_id: string;
  previous_status: AutonomousGoalStatus;
  previous_revision: number;
  running_revision: number;
  schedule_digest: string;
}

export interface AutonomousGoalClaimResult extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_CLAIM_SCHEMA;
  schedule_digest: string;
  claims: AutonomousGoalClaim[];
  claim_digest: string;
  retention: typeof AUTONOMOUS_GOAL_SCHEDULE_RETENTION;
  secret_material: "never_returned";
}

/** Readonly claim row returned by optimistic scheduler admission. */
export type AutonomousGoalClaimSnapshot = Pick<Readonly<AutonomousGoalClaim>,
  "goal_id" | "previous_status" | "previous_revision" | "running_revision" | "schedule_digest"
>;

/** Readonly claim receipt whose digest cannot be invalidated by caller mutation. */
export type AutonomousGoalClaimResultSnapshot = Pick<Readonly<AutonomousGoalClaimResult>,
  "schema" | "schedule_digest" | "claim_digest" | "retention" | "secret_material"
> & {
  readonly claims: readonly AutonomousGoalClaimSnapshot[];
};

type NormalizedSignal = {
  priority: number;
  urgency: number;
  deadline_ns: AutonomousGoalTimestampNs | null;
  estimated_cost: number;
  dependencies: string[];
};

type Candidate = {
  goal: AutonomousGoalRecord;
  signal: NormalizedSignal;
  row: AutonomousGoalScheduleRow;
  eligible: boolean;
};

type ScoreFields = {
  priority: number;
  urgency: number;
  deadline_ns: AutonomousGoalTimestampNs | null;
  estimated_cost: number;
  age_score: number;
  deadline_score: number;
  retry_pressure: number;
  score: number;
  efficiency: number;
};

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal scheduler ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !isUnicodeScalarString(value) || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is outside its bounded identifier contract`);
  return value.trim();
}

function finiteNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function finiteInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bounds`);
  return value;
}

function digest(value: unknown): string {
  return digestJsonSync(value);
}

function rounded(value: number): number {
  // Four decimal places keeps small aging values in the same JSON number spelling in
  // Python and JavaScript, while remaining more precise than the admission score weights.
  return Math.round(value * 10_000) / 10_000;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function domain(value: unknown): AutonomousGoalSchedulingDomain {
  if (typeof value !== "string" || !(AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS as readonly string[]).includes(value)) fail("goal domain is not a supported autonomous scheduling domain");
  return value as AutonomousGoalSchedulingDomain;
}

function normalizeSignal(value: AutonomousGoalSchedulingSignal, index: number): { goalId: string; signal: NormalizedSignal } {
  if (!isObject(value)) fail(`signal ${index} is malformed`);
  const goalId = identifier(`signal ${index}.goal_id`, value.goal_id);
  const dependencies = value.dependencies ?? [];
  if (!Array.isArray(dependencies) || dependencies.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_DEPENDENCIES) fail(`signal ${index}.dependencies is outside its bounds`);
  const normalizedDependencies = [...new Set(dependencies.map((item, dependencyIndex) => identifier(`signal ${index}.dependencies[${dependencyIndex}]`, item)))].sort(compareUnicodeScalars);
  return {
    goalId,
    signal: {
      priority: finiteNumber(`signal ${index}.priority`, value.priority ?? 0.5, 0, 1),
      urgency: finiteNumber(`signal ${index}.urgency`, value.urgency ?? 0, 0, 1),
      deadline_ns: value.deadline_ns === undefined || value.deadline_ns === null ? null : normalizeAutonomousGoalTimestampNs(value.deadline_ns, `signal ${index}.deadline_ns`),
      estimated_cost: finiteInteger(`signal ${index}.estimated_cost`, value.estimated_cost ?? 1, 1, 1_000_000),
      dependencies: normalizedDependencies,
    },
  };
}

function normalizeGoal(goal: AutonomousGoalRecord, index: number): AutonomousGoalRecord {
  if (!isObject(goal)) fail(`goal ${index} is malformed`);
  const normalized = clone(goal);
  identifier(`goal ${index}.goal_id`, normalized.goal_id);
  domain(normalized.domain);
  finiteInteger(`goal ${index}.revision`, normalized.revision, 0, Number.MAX_SAFE_INTEGER);
  finiteInteger(`goal ${index}.attempt`, normalized.attempt, 0, 128);
  finiteInteger(`goal ${index}.max_attempts`, normalized.max_attempts, 1, 128);
  normalized.updated_ns = normalizeAutonomousGoalTimestampNs(normalized.updated_ns, `goal ${index}.updated_ns`);
  return normalized;
}

function roundedRatio(numerator: bigint, denominator: bigint): number {
  if (numerator <= 0n) return 0;
  if (numerator >= denominator) return 1;
  return Number((numerator * 10_000n + denominator / 2n) / denominator) / 10_000;
}

function scoreFor(goal: AutonomousGoalRecord, signal: NormalizedSignal, now: AutonomousGoalTimestampNs, agingWindow: AutonomousGoalTimestampNs): ScoreFields {
  const nowExact = BigInt(now);
  const updatedExact = BigInt(goal.updated_ns);
  const agingExact = BigInt(agingWindow);
  const ageScore = roundedRatio(nowExact - updatedExact, agingExact);
  const deadlineExact = signal.deadline_ns === null ? null : BigInt(signal.deadline_ns);
  const deadlineScore = deadlineExact === null
    ? 0
    : deadlineExact <= nowExact
      ? 1
      : roundedRatio(agingExact, deadlineExact - nowExact + agingExact);
  const retryPressure = rounded(Math.min(1, goal.attempt / Math.max(1, goal.max_attempts)));
  const score = rounded(Math.max(0, Math.min(1, 0.45 * signal.priority + 0.25 * signal.urgency + 0.20 * deadlineScore + 0.10 * ageScore - 0.05 * retryPressure)));
  return {
    priority: signal.priority,
    urgency: signal.urgency,
    deadline_ns: signal.deadline_ns,
    estimated_cost: signal.estimated_cost,
    age_score: ageScore,
    deadline_score: deadlineScore,
    retry_pressure: retryPressure,
    score,
    efficiency: rounded(score / signal.estimated_cost),
  };
}

function statusReason(goal: AutonomousGoalRecord, allowFailedRetry: boolean, includePaused: boolean): { eligible: boolean; decision: AutonomousGoalScheduleDecision; reason: string } {
  if (goal.status === "running") return { eligible: false, decision: "active", reason: "already_running" };
  if (goal.status === "ready") return goal.attempt >= goal.max_attempts ? { eligible: false, decision: "ineligible", reason: "retry_budget_exhausted" } : { eligible: true, decision: "defer", reason: "eligible" };
  if (goal.status === "paused" && goal.blockers.includes("required_goal_criteria_review")) return { eligible: false, decision: "ineligible", reason: "required_goal_criteria_review" };
  if (goal.status === "paused") return goal.attempt >= goal.max_attempts ? { eligible: false, decision: "ineligible", reason: "retry_budget_exhausted" } : includePaused ? { eligible: true, decision: "defer", reason: "eligible" } : { eligible: false, decision: "ineligible", reason: "paused_excluded_by_policy" };
  if (goal.status === "failed") {
    if (!allowFailedRetry) return { eligible: false, decision: "ineligible", reason: "failed_retry_requires_explicit_policy" };
    if (goal.attempt >= goal.max_attempts) return { eligible: false, decision: "ineligible", reason: "retry_budget_exhausted" };
    return { eligible: true, decision: "defer", reason: "eligible_retry" };
  }
  if (goal.status === "blocked") return { eligible: false, decision: "ineligible", reason: "blocked_requires_explicit_reopen" };
  if (goal.status === "completed") return { eligible: false, decision: "ineligible", reason: "terminal_completed" };
  return { eligible: false, decision: "ineligible", reason: "terminal_cancelled" };
}

function validateOptions(options: AutonomousGoalSchedulingOptions): Required<Pick<AutonomousGoalSchedulingOptions, "max_selected" | "max_concurrent" | "max_cost" | "allow_failed_retry" | "include_paused">> & { now_ns: AutonomousGoalTimestampNs; aging_window_ns: AutonomousGoalTimestampNs } {
  const requiredDomains = options.required_domains ?? [];
  if (!Array.isArray(requiredDomains) || requiredDomains.length > AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS.length || new Set(requiredDomains).size !== requiredDomains.length) fail("required_domains is malformed");
  requiredDomains.forEach((item) => domain(item));
  const quotas = options.domain_quotas ?? {};
  if (!isObject(quotas)) fail("domain_quotas must be an object");
  for (const [key, value] of Object.entries(quotas)) {
    domain(key);
    finiteInteger(`domain_quotas.${key}`, value, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED);
  }
  if (options.allow_failed_retry !== undefined && typeof options.allow_failed_retry !== "boolean") fail("allow_failed_retry must be boolean");
  if (options.include_paused !== undefined && typeof options.include_paused !== "boolean") fail("include_paused must be boolean");
  const agingWindowNs = normalizeAutonomousGoalTimestampNs(options.aging_window_ns ?? 86_400_000_000_000, "aging_window_ns");
  if (agingWindowNs === "0") fail("aging_window_ns must be positive");
  return {
    now_ns: normalizeAutonomousGoalTimestampNs(options.now_ns ?? autonomousGoalTimestampNowNs(), "now_ns"),
    max_selected: finiteInteger("max_selected", options.max_selected ?? 1, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED),
    max_concurrent: finiteInteger("max_concurrent", options.max_concurrent ?? options.max_selected ?? 1, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED),
    max_cost: finiteInteger("max_cost", options.max_cost ?? 1_000_000, 1, 1_000_000_000),
    aging_window_ns: agingWindowNs,
    allow_failed_retry: options.allow_failed_retry ?? false,
    include_paused: options.include_paused ?? true,
  };
}

function canonicalRows(rows: readonly AutonomousGoalScheduleRow[]): AutonomousGoalScheduleRow[] {
  return [...rows].sort((left, right) => compareUnicodeScalars(left.goal_id, right.goal_id));
}

function scheduleBody(schedule: Omit<AutonomousGoalSchedule, "schedule_digest">): Omit<AutonomousGoalSchedule, "schedule_digest"> {
  return schedule;
}

function freezeSchedule(schedule: AutonomousGoalSchedule): AutonomousGoalScheduleSnapshot {
  for (const row of schedule.rows) {
    Object.freeze(row.dependencies);
    Object.freeze(row.unmet_dependencies);
    Object.freeze(row);
  }
  Object.freeze(schedule.rows);
  Object.freeze(schedule.selected_goal_ids);
  Object.freeze(schedule.coverage.required_domains);
  Object.freeze(schedule.coverage.selected_domains);
  Object.freeze(schedule.coverage.missing_domains);
  Object.freeze(schedule.coverage);
  return Object.freeze(schedule) as unknown as AutonomousGoalScheduleSnapshot;
}

const AUTONOMOUS_GOAL_SCHEDULE_ROW_FIELDS = new Set(["goal_id", "domain", "status", "revision", "attempt", "max_attempts", "priority", "urgency", "deadline_ns", "estimated_cost", "age_score", "deadline_score", "retry_pressure", "score", "efficiency", "dependencies", "unmet_dependencies", "decision", "reason", "expected_revision"]);
const AUTONOMOUS_GOAL_SCHEDULE_COVERAGE_FIELDS = new Set(["required_domains", "selected_domains", "missing_domains"]);

/** Validate a schedule before replay or claim; validation never contacts a provider. */
export function validateAutonomousGoalSchedule(value: unknown): AutonomousGoalScheduleSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_GOAL_SCHEDULE_SCHEMA) fail("schedule schema is invalid");
  const allowed = new Set(["schema", "now_ns", "max_selected", "max_concurrent", "max_cost", "active_count", "used_cost", "selected_goal_ids", "rows", "coverage", "schedule_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) fail("schedule contains unsupported fields");
  if (value.retention !== AUTONOMOUS_GOAL_SCHEDULE_RETENTION || value.secret_material !== "never_returned") fail("schedule retention posture is invalid");
  requireAutonomousGoalTimestampNsWire(value.now_ns, "schedule.now_ns");
  finiteInteger("schedule.max_selected", value.max_selected, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED);
  finiteInteger("schedule.max_concurrent", value.max_concurrent, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED);
  finiteInteger("schedule.max_cost", value.max_cost, 1, 1_000_000_000);
  finiteInteger("schedule.active_count", value.active_count, 0, AUTONOMOUS_GOAL_SCHEDULE_MAX_GOALS);
  finiteInteger("schedule.used_cost", value.used_cost, 0, 1_000_000_000);
  if (!Array.isArray(value.rows) || value.rows.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_GOALS) fail("schedule rows are outside their bounds");
  if (!Array.isArray(value.selected_goal_ids) || value.selected_goal_ids.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED) fail("schedule selected_goal_ids are outside their bounds");
  const rowIds = new Set<string>();
  const rows = value.rows.map((raw, index) => {
    if (!isObject(raw)) fail(`schedule row ${index} is malformed`);
    if (Object.keys(raw).some((key) => !AUTONOMOUS_GOAL_SCHEDULE_ROW_FIELDS.has(key))) fail(`schedule row ${index} contains unsupported fields`);
    const row = clone(raw as unknown as AutonomousGoalScheduleRow);
    const id = identifier(`schedule row ${index}.goal_id`, row.goal_id);
    if (rowIds.has(id)) fail("schedule contains duplicate goal rows");
    rowIds.add(id);
    domain(row.domain);
    if (row.deadline_ns !== null) row.deadline_ns = requireAutonomousGoalTimestampNsWire(row.deadline_ns, `schedule row ${id}.deadline_ns`);
    if (!("active" === row.decision || "admit" === row.decision || "defer" === row.decision || "ineligible" === row.decision)) fail(`schedule row ${id} decision is invalid`);
    finiteInteger(`schedule row ${id}.revision`, row.revision, 0, Number.MAX_SAFE_INTEGER);
    finiteInteger(`schedule row ${id}.expected_revision`, row.expected_revision, 0, Number.MAX_SAFE_INTEGER);
    if (!Array.isArray(row.dependencies) || !Array.isArray(row.unmet_dependencies)) fail(`schedule row ${id} dependencies are malformed`);
    row.dependencies.forEach((item) => identifier(`schedule row ${id}.dependency`, item));
    row.unmet_dependencies.forEach((item) => identifier(`schedule row ${id}.unmet_dependency`, item));
    return row;
  });
  const selected = value.selected_goal_ids.map((item, index) => identifier(`schedule selected_goal_ids[${index}]`, item));
  if (new Set(selected).size !== selected.length || selected.some((id) => !rowIds.has(id))) fail("schedule selected_goal_ids do not match rows");
  if (selected.some((id) => rows.find((row) => row.goal_id === id)?.decision !== "admit")) fail("schedule selected_goal_ids include a non-admitted row");
  if (!isObject(value.coverage) || !Array.isArray(value.coverage.required_domains) || !Array.isArray(value.coverage.selected_domains) || !Array.isArray(value.coverage.missing_domains)) fail("schedule coverage is malformed");
  if (Object.keys(value.coverage).some((key) => !AUTONOMOUS_GOAL_SCHEDULE_COVERAGE_FIELDS.has(key))) fail("schedule coverage contains unsupported fields");
  value.coverage.required_domains.forEach((item) => domain(item));
  value.coverage.selected_domains.forEach((item) => domain(item));
  value.coverage.missing_domains.forEach((item) => domain(item));
  if (typeof value.schedule_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.schedule_digest)) fail("schedule_digest is malformed");
  const normalized = { ...value, rows: canonicalRows(rows), selected_goal_ids: selected, coverage: clone(value.coverage) } as unknown as AutonomousGoalSchedule;
  const { schedule_digest: _digest, ...body } = normalized;
  if (digest(scheduleBody(body as Omit<AutonomousGoalSchedule, "schedule_digest">)) !== value.schedule_digest) fail("schedule_digest does not match schedule content");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > AUTONOMOUS_GOAL_SCHEDULE_MAX_SNAPSHOT_BYTES) fail("schedule exceeds its byte bound");
  return freezeSchedule(clone(normalized));
}

/** Build a deterministic, dependency-closed admission plan from value-only goal records. */
export function scheduleAutonomousGoals(goals: readonly AutonomousGoalRecord[], options: AutonomousGoalSchedulingOptions = {}): AutonomousGoalScheduleSnapshot {
  if (!Array.isArray(goals) || goals.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_GOALS) fail("goals are outside their bounds");
  const limits = validateOptions(options);
  const goalMap = new Map<string, AutonomousGoalRecord>();
  goals.forEach((goal, index) => {
    const normalized = normalizeGoal(goal, index);
    if (goalMap.has(normalized.goal_id)) fail(`duplicate goal_id ${normalized.goal_id}`);
    goalMap.set(normalized.goal_id, normalized);
  });
  const signals = new Map<string, NormalizedSignal>();
  const suppliedSignals = options.signals ?? [];
  if (!Array.isArray(suppliedSignals) || suppliedSignals.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_SIGNALS) fail("signals are outside their bounds");
  suppliedSignals.forEach((raw, index) => {
    const normalized = normalizeSignal(raw, index);
    if (!goalMap.has(normalized.goalId)) fail(`signal references unknown goal ${normalized.goalId}`);
    if (signals.has(normalized.goalId)) fail(`duplicate signal for goal ${normalized.goalId}`);
    signals.set(normalized.goalId, normalized.signal);
  });
  const activeCount = goals.filter((goal) => goal.status === "running").length;
  const candidates = new Map<string, Candidate>();
  for (const goal of goalMap.values()) {
    const signal = signals.get(goal.goal_id) ?? { priority: 0.5, urgency: 0, deadline_ns: null, estimated_cost: 1, dependencies: [] } satisfies NormalizedSignal;
    const lifecycle = statusReason(goal, limits.allow_failed_retry, limits.include_paused);
    const scores = scoreFor(goal, signal, limits.now_ns, limits.aging_window_ns);
    candidates.set(goal.goal_id, {
      goal,
      signal,
      eligible: lifecycle.eligible,
      row: {
        goal_id: goal.goal_id,
        domain: domain(goal.domain),
        status: goal.status,
        revision: goal.revision,
        attempt: goal.attempt,
        max_attempts: goal.max_attempts,
        ...scores,
        dependencies: [...signal.dependencies],
        unmet_dependencies: [],
        decision: lifecycle.decision,
        reason: lifecycle.reason,
        expected_revision: goal.revision,
      } as AutonomousGoalScheduleRow,
    });
  }
  const cycleNodes = new Set<string>();
  const visited = new Set<string>();
  for (const root of candidates.keys()) {
    if (visited.has(root)) continue;
    const path = [root];
    const pathIndex = new Map([[root, 0]]);
    const stack: Array<{ id: string; dependencyIndex: number }> = [{ id: root, dependencyIndex: 0 }];
    visited.add(root);
    while (stack.length) {
      const frame = stack[stack.length - 1]!;
      const dependencies = candidates.get(frame.id)?.signal.dependencies ?? [];
      if (frame.dependencyIndex >= dependencies.length) {
        stack.pop();
        path.pop();
        pathIndex.delete(frame.id);
        continue;
      }
      const dependency = dependencies[frame.dependencyIndex++]!;
      if (!candidates.has(dependency)) continue;
      const cycleStart = pathIndex.get(dependency);
      if (cycleStart !== undefined) {
        for (let index = cycleStart; index < path.length; index += 1) cycleNodes.add(path[index]!);
        continue;
      }
      if (visited.has(dependency)) continue;
      visited.add(dependency);
      pathIndex.set(dependency, path.length);
      path.push(dependency);
      stack.push({ id: dependency, dependencyIndex: 0 });
    }
  }
  for (const id of cycleNodes) {
    const candidate = candidates.get(id)!;
    candidate.eligible = false;
    candidate.row.decision = "ineligible";
    candidate.row.reason = "dependency_cycle";
    candidate.row.unmet_dependencies = [...candidate.signal.dependencies];
  }
  const sortedEligible = [...candidates.values()].filter((candidate) => candidate.eligible).sort((left, right) => right.row.efficiency - left.row.efficiency || right.row.score - left.row.score || compareUnicodeScalars(left.goal.goal_id, right.goal.goal_id));
  const ordered: string[] = [];
  const orderedSet = new Set<string>();
  for (const candidate of sortedEligible) {
    const root = candidate.goal.goal_id;
    if (orderedSet.has(root)) continue;
    const stack: Array<{ id: string; dependencyIndex: number }> = [{ id: root, dependencyIndex: 0 }];
    const active = new Set([root]);
    while (stack.length) {
      const frame = stack[stack.length - 1]!;
      const dependencies = candidates.get(frame.id)?.signal.dependencies ?? [];
      if (frame.dependencyIndex >= dependencies.length) {
        stack.pop();
        active.delete(frame.id);
        if (!orderedSet.has(frame.id)) {
          orderedSet.add(frame.id);
          ordered.push(frame.id);
        }
        continue;
      }
      const dependency = dependencies[frame.dependencyIndex++]!;
      if (!candidates.get(dependency)?.eligible || orderedSet.has(dependency) || active.has(dependency)) continue;
      active.add(dependency);
      stack.push({ id: dependency, dependencyIndex: 0 });
    }
  }
  const selected = new Set<string>();
  const selectedGoalIds: string[] = [];
  const selectedDomainCounts = new Map<string, number>();
  let usedCost = 0;
  const quotas = options.domain_quotas ?? {};
  for (const id of ordered) {
    const candidate = candidates.get(id)!;
    const unmet = candidate.signal.dependencies.filter((dependency) => !goalMap.has(dependency) || (goalMap.get(dependency)!.status !== "completed" && !selected.has(dependency)));
    candidate.row.unmet_dependencies = [...unmet];
    if (unmet.length) {
      candidate.row.decision = "defer";
      candidate.row.reason = "dependency_not_ready";
      continue;
    }
    if (activeCount + selectedGoalIds.length >= limits.max_concurrent) {
      candidate.row.decision = "defer";
      candidate.row.reason = "concurrency_budget_exhausted";
      continue;
    }
    if (selectedGoalIds.length >= limits.max_selected) {
      candidate.row.decision = "defer";
      candidate.row.reason = "selection_budget_exhausted";
      continue;
    }
    const quota = quotas[candidate.goal.domain];
    if (quota !== undefined && (selectedDomainCounts.get(candidate.goal.domain) ?? 0) >= quota) {
      candidate.row.decision = "defer";
      candidate.row.reason = "domain_quota_exhausted";
      continue;
    }
    if (usedCost + candidate.signal.estimated_cost > limits.max_cost) {
      candidate.row.decision = "defer";
      candidate.row.reason = "cost_budget_exhausted";
      continue;
    }
    selected.add(id);
    selectedGoalIds.push(id);
    selectedDomainCounts.set(candidate.goal.domain, (selectedDomainCounts.get(candidate.goal.domain) ?? 0) + 1);
    usedCost += candidate.signal.estimated_cost;
    candidate.row.decision = "admit";
    candidate.row.reason = "admitted_dependency_closed_candidate";
  }
  const requiredDomains = [...(options.required_domains ?? [])].sort((left, right) => AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS.indexOf(left) - AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS.indexOf(right));
  const selectedDomains = AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS.filter((item) => selectedDomainCounts.has(item));
  const missingDomains = requiredDomains.filter((item) => !selectedDomainCounts.has(item));
  const body = {
    schema: AUTONOMOUS_GOAL_SCHEDULE_SCHEMA,
    now_ns: limits.now_ns,
    max_selected: limits.max_selected,
    max_concurrent: limits.max_concurrent,
    max_cost: limits.max_cost,
    active_count: activeCount,
    used_cost: usedCost,
    selected_goal_ids: selectedGoalIds,
    rows: canonicalRows([...candidates.values()].map((candidate) => candidate.row)),
    coverage: { required_domains: requiredDomains, selected_domains: selectedDomains, missing_domains: missingDomains },
    retention: AUTONOMOUS_GOAL_SCHEDULE_RETENTION,
    secret_material: "never_returned" as const,
  } satisfies Omit<AutonomousGoalSchedule, "schedule_digest">;
  return freezeSchedule({ ...body, schedule_digest: digest(scheduleBody(body)) } as AutonomousGoalSchedule);
}

/** Optimistically claim every admitted goal, rechecking revisions before mutating the ledger. */
export function claimAutonomousGoals(ledger: InMemoryAutonomousGoalLedger, schedule: AutonomousGoalSchedule | AutonomousGoalScheduleSnapshot, options: { now_ns?: AutonomousGoalTimestampInput } = {}): AutonomousGoalClaimResultSnapshot {
  if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("claim requires an InMemoryAutonomousGoalLedger");
  const validated = validateAutonomousGoalSchedule(schedule);
  const admitted = validated.rows.filter((row) => row.decision === "admit").sort((left, right) => validated.selected_goal_ids.indexOf(left.goal_id) - validated.selected_goal_ids.indexOf(right.goal_id));
  if (admitted.length > validated.max_selected) fail("schedule exceeds its selected-goal budget");
  if (admitted.reduce((total, row) => total + row.estimated_cost, 0) > validated.max_cost) fail("schedule exceeds its estimated-cost budget");
  const runningRecords = admitted.length
    ? ledger.claimMany(admitted.map((row) => ({ goal_id: row.goal_id, expected_revision: row.expected_revision, expected_status: row.status as "ready" | "paused" | "failed", expected_domain: row.domain, dependencies: row.dependencies })), { now_ns: options.now_ns, max_concurrent: validated.max_concurrent })
    : [];
  const claims: AutonomousGoalClaim[] = admitted.map((row, index) => ({ goal_id: row.goal_id, previous_status: row.status, previous_revision: row.expected_revision, running_revision: runningRecords[index]!.revision, schedule_digest: validated.schedule_digest }));
  const body = { schema: AUTONOMOUS_GOAL_CLAIM_SCHEMA, schedule_digest: validated.schedule_digest, claims, retention: AUTONOMOUS_GOAL_SCHEDULE_RETENTION, secret_material: "never_returned" as const };
  const result = { ...body, claim_digest: digest(body) };
  result.claims.forEach((claim) => Object.freeze(claim));
  Object.freeze(result.claims);
  return Object.freeze(result) as unknown as AutonomousGoalClaimResultSnapshot;
}

export class AutonomousGoalScheduler {
  plan(goals: readonly AutonomousGoalRecord[], options: AutonomousGoalSchedulingOptions = {}): AutonomousGoalScheduleSnapshot {
    return scheduleAutonomousGoals(goals, options);
  }

  claim(ledger: InMemoryAutonomousGoalLedger, schedule: AutonomousGoalSchedule | AutonomousGoalScheduleSnapshot, options: { now_ns?: AutonomousGoalTimestampInput } = {}): AutonomousGoalClaimResultSnapshot {
    return claimAutonomousGoals(ledger, schedule, options);
  }
}
