import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_RETENTION,
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalRecord,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** A value-only, replayable admission plan for long-horizon autonomous goals. */
export const AUTONOMOUS_GOAL_SCHEDULE_SCHEMA = "bioprism-autonomous-goal-schedule/0.1" as const;
/** A value-only receipt for goals optimistically claimed by a scheduler worker. */
export const AUTONOMOUS_GOAL_CLAIM_SCHEMA = "bioprism-autonomous-goal-claim/0.1" as const;
export const AUTONOMOUS_GOAL_SCHEDULE_RETENTION = "metadata_only_goal_admission;task_text_and_payloads_not_retained" as const;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_GOALS = 4_096;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SIGNALS = 4_096;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_DEPENDENCIES = 64;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED = 128;
export const AUTONOMOUS_GOAL_SCHEDULE_MAX_SNAPSHOT_BYTES = 2_000_000;

export type AutonomousGoalScheduleDecision = "active" | "admit" | "defer" | "ineligible";

export interface AutonomousGoalSchedulingSignal {
  goal_id: string;
  /** Caller/evaluator-owned urgency, normalized to [0, 1]. */
  priority?: number;
  urgency?: number;
  deadline_ns?: number | null;
  estimated_cost?: number;
  dependencies?: readonly string[];
}

export interface AutonomousGoalSchedulingOptions {
  now_ns?: number;
  max_selected?: number;
  max_concurrent?: number;
  max_cost?: number;
  aging_window_ns?: number;
  allow_failed_retry?: boolean;
  include_paused?: boolean;
  signals?: readonly AutonomousGoalSchedulingSignal[];
  required_domains?: readonly AutonomousDomainName[];
  domain_quotas?: Readonly<Record<string, number>>;
}

export interface AutonomousGoalScheduleRow extends JsonObject {
  goal_id: string;
  domain: AutonomousDomainName;
  status: AutonomousGoalStatus;
  revision: number;
  attempt: number;
  max_attempts: number;
  priority: number;
  urgency: number;
  deadline_ns: number | null;
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
  required_domains: AutonomousDomainName[];
  selected_domains: AutonomousDomainName[];
  missing_domains: AutonomousDomainName[];
}

export interface AutonomousGoalSchedule extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_SCHEDULE_SCHEMA;
  now_ns: number;
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

type NormalizedSignal = {
  priority: number;
  urgency: number;
  deadline_ns: number | null;
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
  deadline_ns: number | null;
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
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is outside its bounded identifier contract`);
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

function domain(value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(value)) fail("goal domain is not a built-in autonomous domain");
  return value as AutonomousDomainName;
}

function normalizeSignal(value: AutonomousGoalSchedulingSignal, index: number): { goalId: string; signal: NormalizedSignal } {
  if (!isObject(value)) fail(`signal ${index} is malformed`);
  const goalId = identifier(`signal ${index}.goal_id`, value.goal_id);
  const dependencies = value.dependencies ?? [];
  if (!Array.isArray(dependencies) || dependencies.length > AUTONOMOUS_GOAL_SCHEDULE_MAX_DEPENDENCIES) fail(`signal ${index}.dependencies is outside its bounds`);
  const normalizedDependencies = [...new Set(dependencies.map((item, dependencyIndex) => identifier(`signal ${index}.dependencies[${dependencyIndex}]`, item)))].sort();
  return {
    goalId,
    signal: {
      priority: finiteNumber(`signal ${index}.priority`, value.priority ?? 0.5, 0, 1),
      urgency: finiteNumber(`signal ${index}.urgency`, value.urgency ?? 0, 0, 1),
      deadline_ns: value.deadline_ns === undefined || value.deadline_ns === null ? null : finiteInteger(`signal ${index}.deadline_ns`, value.deadline_ns, 0, Number.MAX_SAFE_INTEGER),
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
  finiteInteger(`goal ${index}.updated_ns`, normalized.updated_ns, 0, Number.MAX_SAFE_INTEGER);
  return normalized;
}

function scoreFor(goal: AutonomousGoalRecord, signal: NormalizedSignal, now: number, agingWindow: number): ScoreFields {
  const ageScore = rounded(Math.min(1, Math.max(0, now - goal.updated_ns) / agingWindow));
  const deadlineScore = signal.deadline_ns === null
    ? 0
    : signal.deadline_ns <= now
      ? 1
      : rounded(Math.min(1, agingWindow / (signal.deadline_ns - now + agingWindow)));
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
  if (goal.status === "ready") return { eligible: true, decision: "defer", reason: "eligible" };
  if (goal.status === "paused") return includePaused ? { eligible: true, decision: "defer", reason: "eligible" } : { eligible: false, decision: "ineligible", reason: "paused_excluded_by_policy" };
  if (goal.status === "failed") {
    if (!allowFailedRetry) return { eligible: false, decision: "ineligible", reason: "failed_retry_requires_explicit_policy" };
    if (goal.attempt >= goal.max_attempts) return { eligible: false, decision: "ineligible", reason: "retry_budget_exhausted" };
    return { eligible: true, decision: "defer", reason: "eligible_retry" };
  }
  if (goal.status === "blocked") return { eligible: false, decision: "ineligible", reason: "blocked_requires_explicit_reopen" };
  if (goal.status === "completed") return { eligible: false, decision: "ineligible", reason: "terminal_completed" };
  return { eligible: false, decision: "ineligible", reason: "terminal_cancelled" };
}

function validateOptions(options: AutonomousGoalSchedulingOptions): Required<Pick<AutonomousGoalSchedulingOptions, "now_ns" | "max_selected" | "max_concurrent" | "max_cost" | "aging_window_ns" | "allow_failed_retry" | "include_paused">> {
  const requiredDomains = options.required_domains ?? [];
  if (!Array.isArray(requiredDomains) || requiredDomains.length > AUTONOMOUS_DOMAIN_NAMES.length || new Set(requiredDomains).size !== requiredDomains.length) fail("required_domains is malformed");
  requiredDomains.forEach((item) => domain(item));
  const quotas = options.domain_quotas ?? {};
  if (!isObject(quotas)) fail("domain_quotas must be an object");
  for (const [key, value] of Object.entries(quotas)) {
    domain(key);
    finiteInteger(`domain_quotas.${key}`, value, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED);
  }
  if (options.allow_failed_retry !== undefined && typeof options.allow_failed_retry !== "boolean") fail("allow_failed_retry must be boolean");
  if (options.include_paused !== undefined && typeof options.include_paused !== "boolean") fail("include_paused must be boolean");
  return {
    now_ns: finiteInteger("now_ns", options.now_ns ?? Date.now(), 0, Number.MAX_SAFE_INTEGER),
    max_selected: finiteInteger("max_selected", options.max_selected ?? 1, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED),
    max_concurrent: finiteInteger("max_concurrent", options.max_concurrent ?? options.max_selected ?? 1, 1, AUTONOMOUS_GOAL_SCHEDULE_MAX_SELECTED),
    max_cost: finiteInteger("max_cost", options.max_cost ?? 1_000_000, 1, 1_000_000_000),
    aging_window_ns: finiteInteger("aging_window_ns", options.aging_window_ns ?? 86_400_000, 1, Number.MAX_SAFE_INTEGER),
    allow_failed_retry: options.allow_failed_retry ?? false,
    include_paused: options.include_paused ?? true,
  };
}

function canonicalRows(rows: readonly AutonomousGoalScheduleRow[]): AutonomousGoalScheduleRow[] {
  return [...rows].sort((left, right) => left.goal_id.localeCompare(right.goal_id));
}

function scheduleBody(schedule: Omit<AutonomousGoalSchedule, "schedule_digest">): Omit<AutonomousGoalSchedule, "schedule_digest"> {
  return schedule;
}

/** Validate a schedule before replay or claim; validation never contacts a provider. */
export function validateAutonomousGoalSchedule(value: unknown): AutonomousGoalSchedule {
  if (!isObject(value) || value.schema !== AUTONOMOUS_GOAL_SCHEDULE_SCHEMA) fail("schedule schema is invalid");
  const allowed = new Set(["schema", "now_ns", "max_selected", "max_concurrent", "max_cost", "active_count", "used_cost", "selected_goal_ids", "rows", "coverage", "schedule_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) fail("schedule contains unsupported fields");
  if (value.retention !== AUTONOMOUS_GOAL_SCHEDULE_RETENTION || value.secret_material !== "never_returned") fail("schedule retention posture is invalid");
  finiteInteger("schedule.now_ns", value.now_ns, 0, Number.MAX_SAFE_INTEGER);
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
    const row = raw as unknown as AutonomousGoalScheduleRow;
    const id = identifier(`schedule row ${index}.goal_id`, row.goal_id);
    if (rowIds.has(id)) fail("schedule contains duplicate goal rows");
    rowIds.add(id);
    domain(row.domain);
    if (!("active" === row.decision || "admit" === row.decision || "defer" === row.decision || "ineligible" === row.decision)) fail(`schedule row ${id} decision is invalid`);
    finiteInteger(`schedule row ${id}.revision`, row.revision, 0, Number.MAX_SAFE_INTEGER);
    finiteInteger(`schedule row ${id}.expected_revision`, row.expected_revision, 0, Number.MAX_SAFE_INTEGER);
    if (!Array.isArray(row.dependencies) || !Array.isArray(row.unmet_dependencies)) fail(`schedule row ${id} dependencies are malformed`);
    row.dependencies.forEach((item) => identifier(`schedule row ${id}.dependency`, item));
    row.unmet_dependencies.forEach((item) => identifier(`schedule row ${id}.unmet_dependency`, item));
    return clone(row);
  });
  const selected = value.selected_goal_ids.map((item, index) => identifier(`schedule selected_goal_ids[${index}]`, item));
  if (new Set(selected).size !== selected.length || selected.some((id) => !rowIds.has(id))) fail("schedule selected_goal_ids do not match rows");
  if (selected.some((id) => rows.find((row) => row.goal_id === id)?.decision !== "admit")) fail("schedule selected_goal_ids include a non-admitted row");
  if (!isObject(value.coverage) || !Array.isArray(value.coverage.required_domains) || !Array.isArray(value.coverage.selected_domains) || !Array.isArray(value.coverage.missing_domains)) fail("schedule coverage is malformed");
  value.coverage.required_domains.forEach((item) => domain(item));
  value.coverage.selected_domains.forEach((item) => domain(item));
  value.coverage.missing_domains.forEach((item) => domain(item));
  if (typeof value.schedule_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.schedule_digest)) fail("schedule_digest is malformed");
  const normalized = { ...value, rows: canonicalRows(rows), selected_goal_ids: selected, coverage: clone(value.coverage) } as unknown as AutonomousGoalSchedule;
  const { schedule_digest: _digest, ...body } = normalized;
  if (digest(scheduleBody(body as Omit<AutonomousGoalSchedule, "schedule_digest">)) !== value.schedule_digest) fail("schedule_digest does not match schedule content");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > AUTONOMOUS_GOAL_SCHEDULE_MAX_SNAPSHOT_BYTES) fail("schedule exceeds its byte bound");
  return clone(normalized);
}

/** Build a deterministic, dependency-closed admission plan from value-only goal records. */
export function scheduleAutonomousGoals(goals: readonly AutonomousGoalRecord[], options: AutonomousGoalSchedulingOptions = {}): AutonomousGoalSchedule {
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
  const visiting: string[] = [];
  const visited = new Set<string>();
  const visitCycle = (id: string): void => {
    if (visiting.includes(id)) {
      for (let index = visiting.indexOf(id); index < visiting.length; index += 1) cycleNodes.add(visiting[index]!);
      return;
    }
    if (visited.has(id)) return;
    visited.add(id);
    visiting.push(id);
    for (const dependency of candidates.get(id)?.signal.dependencies ?? []) if (candidates.has(dependency)) visitCycle(dependency);
    visiting.pop();
  };
  for (const id of candidates.keys()) visitCycle(id);
  for (const id of cycleNodes) {
    const candidate = candidates.get(id)!;
    candidate.eligible = false;
    candidate.row.decision = "ineligible";
    candidate.row.reason = "dependency_cycle";
    candidate.row.unmet_dependencies = [...candidate.signal.dependencies];
  }
  const sortedEligible = [...candidates.values()].filter((candidate) => candidate.eligible).sort((left, right) => right.row.efficiency - left.row.efficiency || right.row.score - left.row.score || left.goal.goal_id.localeCompare(right.goal.goal_id));
  const ordered: string[] = [];
  const orderedSet = new Set<string>();
  const visitOrder = (id: string): void => {
    if (orderedSet.has(id)) return;
    const candidate = candidates.get(id);
    if (!candidate?.eligible) return;
    for (const dependency of candidate.signal.dependencies) if (candidates.get(dependency)?.eligible) visitOrder(dependency);
    orderedSet.add(id);
    ordered.push(id);
  };
  sortedEligible.forEach((candidate) => visitOrder(candidate.goal.goal_id));
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
  const requiredDomains = [...(options.required_domains ?? [])].sort((left, right) => AUTONOMOUS_DOMAIN_NAMES.indexOf(left) - AUTONOMOUS_DOMAIN_NAMES.indexOf(right));
  const selectedDomains = AUTONOMOUS_DOMAIN_NAMES.filter((item) => selectedDomainCounts.has(item));
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
  return { ...body, schedule_digest: digest(scheduleBody(body)) };
}

/** Optimistically claim every admitted goal, rechecking revisions before mutating the ledger. */
export function claimAutonomousGoals(ledger: InMemoryAutonomousGoalLedger, schedule: AutonomousGoalSchedule, options: { now_ns?: number } = {}): AutonomousGoalClaimResult {
  if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("claim requires an InMemoryAutonomousGoalLedger");
  const validated = validateAutonomousGoalSchedule(schedule);
  const admitted = validated.rows.filter((row) => row.decision === "admit").sort((left, right) => validated.selected_goal_ids.indexOf(left.goal_id) - validated.selected_goal_ids.indexOf(right.goal_id));
  for (const row of admitted) {
    const current = ledger.get(row.goal_id);
    if (current === null || current.revision !== row.expected_revision || current.status !== row.status || !["ready", "paused", "failed"].includes(current.status)) fail(`schedule is stale for goal ${row.goal_id}`);
  }
  const claims: AutonomousGoalClaim[] = [];
  for (const row of admitted) {
    let current = ledger.get(row.goal_id)!;
    const previousStatus = current.status;
    const previousRevision = current.revision;
    if (current.status === "failed") current = ledger.transition(current.goal_id, "ready", { expected_revision: current.revision, now_ns: options.now_ns });
    const running = ledger.transition(current.goal_id, "running", { expected_revision: current.revision, now_ns: options.now_ns });
    claims.push({ goal_id: row.goal_id, previous_status: previousStatus, previous_revision: previousRevision, running_revision: running.revision, schedule_digest: validated.schedule_digest });
  }
  const body = { schema: AUTONOMOUS_GOAL_CLAIM_SCHEMA, schedule_digest: validated.schedule_digest, claims, retention: AUTONOMOUS_GOAL_SCHEDULE_RETENTION, secret_material: "never_returned" as const };
  return { ...body, claim_digest: digest(body) };
}

export class AutonomousGoalScheduler {
  plan(goals: readonly AutonomousGoalRecord[], options: AutonomousGoalSchedulingOptions = {}): AutonomousGoalSchedule {
    return scheduleAutonomousGoals(goals, options);
  }

  claim(ledger: InMemoryAutonomousGoalLedger, schedule: AutonomousGoalSchedule, options: { now_ns?: number } = {}): AutonomousGoalClaimResult {
    return claimAutonomousGoals(ledger, schedule, options);
  }
}
