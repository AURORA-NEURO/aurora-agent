import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestBytesSync, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Goal records retain only bounded lifecycle and settlement identities; execution payloads remain transient. */

export const AUTONOMOUS_GOAL_SCHEMA = "bioprism-autonomous-goal/0.1" as const;
export const AUTONOMOUS_GOAL_EVENT_SCHEMA = "bioprism-autonomous-goal-event/0.1" as const;
export const AUTONOMOUS_GOAL_STEP_SCHEMA = "bioprism-autonomous-goal-step/0.1" as const;
export const AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-snapshot/0.1" as const;
export const AUTONOMOUS_GOAL_RETENTION = "value_only_goal_state;task_prompt_response_tool_payloads_and_credentials_not_retained" as const;
export const AUTONOMOUS_GOAL_MAX_GOALS = 4_096;
export const AUTONOMOUS_GOAL_MAX_EVENTS = 16_384;
export const AUTONOMOUS_GOAL_MAX_CRITERIA = 64;
export const AUTONOMOUS_GOAL_MAX_BLOCKERS = 32;
export const AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES = 4_000_000;

export type AutonomousGoalStatus = "ready" | "running" | "paused" | "blocked" | "failed" | "completed" | "cancelled";
export type AutonomousGoalCriterionStatus = "pending" | "satisfied" | "failed" | "waived";

const ALLOWED_TRANSITIONS: Record<AutonomousGoalStatus, readonly AutonomousGoalStatus[]> = {
  ready: ["running", "blocked", "cancelled"],
  running: ["paused", "blocked", "failed", "completed", "cancelled"],
  paused: ["running", "blocked", "cancelled"],
  blocked: ["ready", "cancelled"],
  failed: ["ready", "cancelled"],
  completed: [],
  cancelled: [],
};

const GOAL_COMPLETED_RESULTS = new Set(["completed", "completed_without_replan", "children_completed"]);
const GOAL_PAUSED_RESULTS = new Set(["approval_required", "reconciliation_required", "turn_limit_reached", "paused", "stage_blocked", "children_partial", "child_incomplete"]);
const GOAL_BLOCKED_RESULTS = new Set(["route_review_required", "planning_review_required", "provider_disagreement"]);

/** Map a bounded runtime status to goal state without trusting provider text. */
export function goalStatusForResult(resultStatus: string, criteriaComplete: boolean): AutonomousGoalStatus {
  if (typeof resultStatus !== "string" || !resultStatus.trim()) return "failed";
  if (GOAL_COMPLETED_RESULTS.has(resultStatus)) return criteriaComplete ? "completed" : "paused";
  if (GOAL_PAUSED_RESULTS.has(resultStatus)) return "paused";
  if (GOAL_BLOCKED_RESULTS.has(resultStatus)) return "blocked";
  return "failed";
}

export interface AutonomousGoalCriterion extends JsonObject {
  criterion_id: string;
  criterion_digest: string;
  required: boolean;
  status: AutonomousGoalCriterionStatus;
  weight: number;
  evidence_digest: string | null;
}

export interface AutonomousGoalRecord extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_SCHEMA;
  goal_id: string;
  task_digest: string;
  domain: string;
  capability: string | null;
  risk_class: string | null;
  status: AutonomousGoalStatus;
  attempt: number;
  max_attempts: number;
  revision: number;
  criteria: AutonomousGoalCriterion[];
  blockers: string[];
  next_action_digest: string | null;
  outcome_digest: string | null;
  evaluator_digest: string | null;
  learning_state_digest: string | null;
  progress_digest: string | null;
  created_ns: number;
  updated_ns: number;
  state_digest: string;
  retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

/** Caller/evaluator-owned digest identities that may be attached to a goal settlement. */
export interface AutonomousGoalSettlementMetadata extends JsonObject {
  evaluator_digest?: string | null;
  learning_state_digest?: string | null;
  progress_digest?: string | null;
}

export interface AutonomousGoalEvent extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_EVENT_SCHEMA;
  sequence: number;
  goal_id: string;
  event_type: "created" | "transition";
  payload: AutonomousGoalRecord;
  previous_digest: string;
  event_digest: string;
  created_ns: number;
  retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousGoalSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  goals: AutonomousGoalRecord[];
  events: AutonomousGoalEvent[];
  snapshot_digest: string;
  retention: typeof AUTONOMOUS_GOAL_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousGoalPersistence {
  read(): AutonomousGoalSnapshot | null | Promise<AutonomousGoalSnapshot | null>;
  write(snapshot: AutonomousGoalSnapshot): void | Promise<void>;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalSnapshot): boolean | Promise<boolean>;
}

export interface AutonomousGoalTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface AutonomousGoalTransactionalTextStore extends AutonomousGoalTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

function identifier(value: unknown, name: string): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value.trim();
}

function digest(value: unknown, name: string, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function sequence(value: unknown, name: string, maximum: number): unknown[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bounded sequence contract`);
  return value;
}

function finiteInteger(value: unknown, name: string, minimum = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum) throw new ArgumentError(`${name} must be a non-negative safe integer`);
  return value;
}

function criterion(value: unknown): AutonomousGoalCriterion {
  if (!isObject(value)) throw new ArgumentError("goal criterion must be an object");
  const weight = value.weight ?? 1;
  if (typeof weight !== "number" || !Number.isFinite(weight) || weight <= 0 || weight > 1_000) throw new ArgumentError("goal criterion weight is outside its bounds");
  const status = value.status ?? "pending";
  if (!["pending", "satisfied", "failed", "waived"].includes(String(status))) throw new ArgumentError("goal criterion status is unsupported");
  const required = value.required ?? true;
  if (typeof required !== "boolean") throw new ArgumentError("goal criterion required must be boolean");
  return {
    criterion_id: identifier(value.criterion_id, "goal criterion_id"),
    criterion_digest: digest(value.criterion_digest, "goal criterion_digest")!,
    required,
    status: status as AutonomousGoalCriterionStatus,
    weight: weight,
    evidence_digest: digest(value.evidence_digest ?? null, "goal criterion evidence_digest", true),
  };
}

function normalizeCriteria(value: unknown): AutonomousGoalCriterion[] {
  const rows = sequence(value, "goal criteria", AUTONOMOUS_GOAL_MAX_CRITERIA).map(criterion);
  const ids = new Set<string>();
  for (const row of rows) {
    if (typeof row.required !== "boolean") throw new ArgumentError("goal criterion required must be boolean");
    if (ids.has(row.criterion_id)) throw new ArgumentError("goal criteria contain duplicate criterion_id values");
    ids.add(row.criterion_id);
  }
  return rows.sort((left, right) => left.criterion_id.localeCompare(right.criterion_id));
}

function normalizeBlockers(value: unknown): string[] {
  return [...new Set(sequence(value, "goal blockers", AUTONOMOUS_GOAL_MAX_BLOCKERS).map((item) => identifier(item, "goal blocker")))].sort();
}

function goalIdentity(record: AutonomousGoalRecord): string {
  return canonicalJson({
    goal_id: record.goal_id,
    task_digest: record.task_digest,
    domain: record.domain,
    capability: record.capability,
    risk_class: record.risk_class,
    max_attempts: record.max_attempts,
    criteria: record.criteria.map((item) => ({ criterion_id: item.criterion_id, criterion_digest: item.criterion_digest, required: item.required, weight: Number.isInteger(item.weight) ? item.weight : item.weight })),
  });
}

type AutonomousGoalCore = {
  schema: typeof AUTONOMOUS_GOAL_SCHEMA;
  goal_id: string;
  task_digest: string;
  domain: string;
  capability: string | null;
  risk_class: string | null;
  status: AutonomousGoalStatus;
  attempt: number;
  max_attempts: number;
  revision: number;
  criteria: AutonomousGoalCriterion[];
  blockers: string[];
  next_action_digest: string | null;
  outcome_digest: string | null;
  evaluator_digest: string | null;
  learning_state_digest: string | null;
  progress_digest: string | null;
  created_ns: number;
  updated_ns: number;
};

function core(record: AutonomousGoalCore): AutonomousGoalCore {
  return record;
}

function buildRecord(fields: {
  goal_id: string;
  task_digest: string;
  domain: string;
  capability?: string | null;
  risk_class?: string | null;
  status: AutonomousGoalStatus;
  attempt: number;
  max_attempts: number;
  revision: number;
  criteria: unknown;
  blockers: unknown;
  next_action_digest?: string | null;
  outcome_digest?: string | null;
  evaluator_digest?: string | null;
  learning_state_digest?: string | null;
  progress_digest?: string | null;
  created_ns: number;
  updated_ns: number;
}): AutonomousGoalRecord {
  if (!(fields.status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal status is unsupported");
  const attempt = finiteInteger(fields.attempt, "goal attempt");
  const maxAttempts = finiteInteger(fields.max_attempts, "goal max_attempts", 1);
  if (maxAttempts > 128 || attempt > maxAttempts) throw new ArgumentError("goal attempt budget is outside its bounds");
  const created = finiteInteger(fields.created_ns, "goal created_ns");
  const updated = finiteInteger(fields.updated_ns, "goal updated_ns");
  if (updated < created) throw new ArgumentError("goal updated_ns cannot precede created_ns");
  const normalized = core({
    schema: AUTONOMOUS_GOAL_SCHEMA,
    goal_id: identifier(fields.goal_id, "goal_id"),
    task_digest: digest(fields.task_digest, "goal task_digest")!,
    domain: identifier(fields.domain, "goal domain"),
    capability: fields.capability === null || fields.capability === undefined ? null : identifier(fields.capability, "goal capability"),
    risk_class: fields.risk_class === null || fields.risk_class === undefined ? null : identifier(fields.risk_class, "goal risk_class"),
    status: fields.status,
    attempt,
    max_attempts: maxAttempts,
    revision: finiteInteger(fields.revision, "goal revision"),
    criteria: normalizeCriteria(fields.criteria),
    blockers: normalizeBlockers(fields.blockers),
    next_action_digest: digest(fields.next_action_digest ?? null, "goal next_action_digest", true),
    outcome_digest: digest(fields.outcome_digest ?? null, "goal outcome_digest", true),
    evaluator_digest: digest(fields.evaluator_digest ?? null, "goal evaluator_digest", true),
    learning_state_digest: digest(fields.learning_state_digest ?? null, "goal learning_state_digest", true),
    progress_digest: digest(fields.progress_digest ?? null, "goal progress_digest", true),
    created_ns: created,
    updated_ns: updated,
  });
  return { ...normalized, state_digest: digestJsonSync(normalized), retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
}

export function goalTaskDigest(task: string): string {
  if (typeof task !== "string" || !task.trim() || task.includes("\u0000") || new TextEncoder().encode(task).byteLength > 32_000) throw new ArgumentError("goal task is outside its bounded contract");
  return digestBytesSync(new TextEncoder().encode(task));
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function eventBody(event: Omit<AutonomousGoalEvent, "event_digest">): Omit<AutonomousGoalEvent, "event_digest"> {
  return event;
}

function verifyRecord(value: unknown): AutonomousGoalRecord {
  if (!isObject(value) || value.schema !== AUTONOMOUS_GOAL_SCHEMA) throw new ArgumentError("goal record has an invalid schema");
  if (value.retention !== AUTONOMOUS_GOAL_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("goal record retention contract is invalid");
  const record = buildRecord(value as unknown as Parameters<typeof buildRecord>[0]);
  if (value.state_digest !== record.state_digest) {
    const settlementFields = ["outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"];
    const { state_digest: _stateDigest, retention: _retention, secret_material: _secretMaterial, ...legacy } = record;
    for (const field of settlementFields) delete legacy[field as keyof typeof legacy];
    if (settlementFields.some((field) => field in value) || digestJsonSync(legacy) !== value.state_digest) throw new ArgumentError("goal state_digest does not match its content");
  }
  return { ...record, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
}

export class InMemoryAutonomousGoalLedger {
  private readonly goals = new Map<string, AutonomousGoalRecord>();
  private readonly events: AutonomousGoalEvent[] = [];
  private readonly maxGoals: number;
  private readonly maxEvents: number;
  private readonly clock: () => number;

  constructor(options: { maxGoals?: number; maxEvents?: number; clock?: () => number } = {}) {
    this.maxGoals = finiteInteger(options.maxGoals ?? AUTONOMOUS_GOAL_MAX_GOALS, "goal maxGoals", 1);
    this.maxEvents = finiteInteger(options.maxEvents ?? AUTONOMOUS_GOAL_MAX_EVENTS, "goal maxEvents", 1);
    if (this.maxGoals > AUTONOMOUS_GOAL_MAX_GOALS || this.maxEvents > AUTONOMOUS_GOAL_MAX_EVENTS) throw new ArgumentError("goal ledger capacity is outside its bounds");
    this.clock = options.clock ?? (() => Date.now());
  }

  create(input: { goal_id: string; task_digest: string; domain: string; capability?: string | null; risk_class?: string | null; criteria?: readonly AutonomousGoalCriterion[]; max_attempts?: number; now_ns?: number }): AutonomousGoalRecord {
    const now = input.now_ns ?? this.clock();
    const record = buildRecord({ ...input, max_attempts: input.max_attempts ?? 8, status: "ready", attempt: 0, revision: 0, criteria: input.criteria ?? [], blockers: [], created_ns: now, updated_ns: now });
    const prior = this.goals.get(record.goal_id);
    if (prior) {
      if (goalIdentity(prior) !== goalIdentity(record)) throw new ArgumentError("goal_id already exists with a different identity");
      return clone(prior);
    }
    if (this.goals.size >= this.maxGoals) throw new ArgumentError("goal ledger capacity is exhausted");
    this.goals.set(record.goal_id, record);
    this.append("created", record, record.updated_ns);
    return clone(record);
  }

  get(goalId: string): AutonomousGoalRecord | null {
    return clone(this.goals.get(identifier(goalId, "goal_id")) ?? null);
  }

  list(query: { domain?: string; statuses?: readonly AutonomousGoalStatus[]; limit?: number } = {}): AutonomousGoalRecord[] {
    const limit = finiteInteger(query.limit ?? 128, "goal list limit", 1);
    if (limit > 512) throw new ArgumentError("goal list limit must be at most 512");
    const domain = query.domain === undefined ? undefined : identifier(query.domain, "goal domain");
    const statuses = new Set(query.statuses ?? []);
    for (const status of statuses) if (!(status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal list contains an unsupported status");
    return [...this.goals.values()].filter((goal) => (domain === undefined || goal.domain === domain) && (!statuses.size || statuses.has(goal.status))).sort((left, right) => right.updated_ns - left.updated_ns || left.goal_id.localeCompare(right.goal_id)).slice(0, limit).map(clone);
  }

  transition(goalId: string, status: AutonomousGoalStatus, options: { expected_revision?: number; criterion_updates?: readonly JsonObject[]; blockers?: readonly string[]; next_action_digest?: string | null; outcome_digest?: string | null; evaluator_digest?: string | null; learning_state_digest?: string | null; progress_digest?: string | null; now_ns?: number } = {}): AutonomousGoalRecord {
    const id = identifier(goalId, "goal_id");
    const current = this.goals.get(id);
    if (!current) throw new ArgumentError(`goal ${id} was not found`);
    if (!(status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal status is unsupported");
    if (status !== current.status && !ALLOWED_TRANSITIONS[current.status].includes(status)) throw new ArgumentError(`goal cannot transition from ${current.status} to ${status}`);
    if (options.expected_revision !== undefined && options.expected_revision !== current.revision) throw new ArgumentError(`goal revision conflict: expected ${options.expected_revision}, observed ${current.revision}`);
    if (status === "ready" && current.status === "failed" && current.attempt >= current.max_attempts) throw new ArgumentError("goal attempt budget is exhausted");
    const updates = options.criterion_updates ?? [];
    if (updates.length > AUTONOMOUS_GOAL_MAX_CRITERIA) throw new ArgumentError("criterion updates exceed their bound");
    const criteria = current.criteria.map((item) => ({ ...item }));
    for (const update of updates) {
      if (!isObject(update)) throw new ArgumentError("criterion update must be an object");
      const prior = criteria.find((item) => item.criterion_id === update.criterion_id);
      if (!prior) throw new ArgumentError(`criterion update references unknown criterion ${String(update.criterion_id)}`);
      const nextStatus = update.status ?? prior.status;
      if (!["pending", "satisfied", "failed", "waived"].includes(String(nextStatus))) throw new ArgumentError("criterion update status is unsupported");
      if ((prior.status === "satisfied" || prior.status === "waived") && nextStatus !== prior.status) throw new ArgumentError("satisfied or waived criteria cannot regress");
      prior.status = nextStatus as AutonomousGoalCriterionStatus;
      prior.evidence_digest = digest(update.evidence_digest ?? prior.evidence_digest, "criterion update evidence_digest", true);
    }
    if (status === "completed" && criteria.some((item) => item.required && !["satisfied", "waived"].includes(item.status))) throw new ArgumentError("goal cannot complete while a required criterion is unresolved");
    let attempt = current.attempt;
    if (status === "running" && current.status !== "running") {
      if (attempt >= current.max_attempts) throw new ArgumentError("goal attempt budget is exhausted");
      attempt += 1;
    }
    const updated = buildRecord({ goal_id: current.goal_id, task_digest: current.task_digest, domain: current.domain, capability: current.capability, risk_class: current.risk_class, status, attempt, max_attempts: current.max_attempts, revision: current.revision + 1, criteria, blockers: options.blockers ?? current.blockers, next_action_digest: options.next_action_digest === undefined ? current.next_action_digest : options.next_action_digest, outcome_digest: options.outcome_digest === undefined ? current.outcome_digest : options.outcome_digest, evaluator_digest: options.evaluator_digest === undefined ? current.evaluator_digest : options.evaluator_digest, learning_state_digest: options.learning_state_digest === undefined ? current.learning_state_digest : options.learning_state_digest, progress_digest: options.progress_digest === undefined ? current.progress_digest : options.progress_digest, created_ns: current.created_ns, updated_ns: options.now_ns ?? this.clock() });
    this.goals.set(id, updated);
    this.append("transition", updated, updated.updated_ns);
    return clone(updated);
  }

  updateCriteria(goalId: string, updates: readonly JsonObject[], options: { expected_revision?: number; now_ns?: number } = {}): AutonomousGoalRecord {
    const current = this.goals.get(identifier(goalId, "goal_id"));
    if (!current) throw new ArgumentError(`goal ${goalId} was not found`);
    return this.transition(goalId, current.status, { ...options, criterion_updates: updates, blockers: current.blockers, next_action_digest: current.next_action_digest });
  }

  stats(): JsonObject {
    const statuses: Record<string, number> = {};
    for (const goal of this.goals.values()) statuses[goal.status] = (statuses[goal.status] ?? 0) + 1;
    return { schema: AUTONOMOUS_GOAL_SCHEMA, total: this.goals.size, statuses, events: this.events.length, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  }

  verifyIntegrity(): JsonObject {
    let previous = "";
    const latestByGoal = new Map<string, string>();
    for (let index = 0; index < this.events.length; index += 1) {
      const event = this.events[index]!;
      if (event.schema !== AUTONOMOUS_GOAL_EVENT_SCHEMA || !["created", "transition"].includes(event.event_type) || event.retention !== AUTONOMOUS_GOAL_RETENTION || event.secret_material !== "never_returned" || event.goal_id !== event.payload.goal_id) throw new ArgumentError(`goal event metadata is malformed at sequence ${event.sequence}`);
      verifyRecord(event.payload);
      if ((event.event_type === "created" && latestByGoal.has(event.goal_id)) || (event.event_type === "transition" && !latestByGoal.has(event.goal_id))) throw new ArgumentError(`goal event lifecycle is malformed for ${event.goal_id}`);
      const { event_digest: _eventDigest, ...body } = event;
      if (event.sequence !== index + 1 || event.previous_digest !== previous || digestJsonSync(eventBody(body)) !== event.event_digest) throw new ArgumentError(`goal event hash chain breaks at sequence ${event.sequence}`);
      if (!this.goals.has(event.goal_id)) throw new ArgumentError(`goal event references missing goal ${event.goal_id}`);
      latestByGoal.set(event.goal_id, event.payload.state_digest);
      previous = event.event_digest;
    }
    for (const goal of this.goals.values()) {
      verifyRecord(goal);
      if (latestByGoal.get(goal.goal_id) !== goal.state_digest) throw new ArgumentError(`goal current state is not bound to its latest event for ${goal.goal_id}`);
    }
    return { schema: AUTONOMOUS_GOAL_EVENT_SCHEMA, ok: true, goals: this.goals.size, events: this.events.length, head_digest: previous, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  }

  snapshot(): AutonomousGoalSnapshot {
    const body = { schema: AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA, sequence: this.events.length, head_digest: this.events.at(-1)?.event_digest ?? "", goals: [...this.goals.values()].sort((left, right) => left.goal_id.localeCompare(right.goal_id)).map(clone), events: this.events.map(clone), retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const };
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) };
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal snapshot exceeds its byte bound");
    return snapshot;
  }

  restore(snapshot: AutonomousGoalSnapshot): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.goals) || !Array.isArray(snapshot.events)) throw new ArgumentError("goal snapshot is malformed");
    const allowed = new Set(["schema", "sequence", "head_digest", "goals", "events", "snapshot_digest", "retention", "secret_material"]);
    if (Object.keys(snapshot).some((key) => !allowed.has(key)) || snapshot.retention !== AUTONOMOUS_GOAL_RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("goal snapshot contains unsupported or unsafe metadata");
    if (!Number.isSafeInteger(snapshot.sequence) || snapshot.sequence < 0 || snapshot.sequence !== snapshot.events.length || snapshot.events.length > this.maxEvents || snapshot.goals.length > this.maxGoals) throw new ArgumentError("goal snapshot sequence or capacity is invalid");
    if (typeof snapshot.head_digest !== "string" || (snapshot.sequence > 0 && !/^[0-9a-f]{64}$/.test(snapshot.head_digest)) || (snapshot.sequence === 0 && snapshot.head_digest !== "")) throw new ArgumentError("goal snapshot head digest is invalid");
    const { snapshot_digest: supplied, ...body } = snapshot;
    if (typeof supplied !== "string" || !/^[0-9a-f]{64}$/.test(supplied) || digestJsonSync(body) !== supplied) throw new ArgumentError("goal snapshot digest mismatch");
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal snapshot exceeds its byte bound");
    const restored = new InMemoryAutonomousGoalLedger({ maxGoals: this.maxGoals, maxEvents: this.maxEvents, clock: this.clock });
    for (const value of snapshot.goals) {
      const goal = verifyRecord(value);
      if (restored.goals.has(goal.goal_id)) throw new ArgumentError("goal snapshot contains duplicate goals");
      restored.goals.set(goal.goal_id, goal);
    }
    for (const value of snapshot.events) {
      if (!isObject(value)) throw new ArgumentError("goal snapshot event is malformed");
      restored.events.push(clone(value as unknown as AutonomousGoalEvent));
    }
    if (restored.events.length !== snapshot.sequence || (restored.events.at(-1)?.event_digest ?? "") !== snapshot.head_digest) throw new ArgumentError("goal snapshot head is inconsistent");
    restored.verifyIntegrity();
    this.goals.clear();
    for (const [id, goal] of restored.goals) this.goals.set(id, goal);
    this.events.splice(0, this.events.length, ...restored.events);
  }

  private append(eventType: "created" | "transition", payload: AutonomousGoalRecord, created: number): void {
    if (this.events.length >= this.maxEvents) throw new ArgumentError("goal event capacity is exhausted");
    const event = { schema: AUTONOMOUS_GOAL_EVENT_SCHEMA, sequence: this.events.length + 1, goal_id: payload.goal_id, event_type: eventType, payload: clone(payload), previous_digest: this.events.at(-1)?.event_digest ?? "", created_ns: created, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const };
    this.events.push({ ...event, event_digest: digestJsonSync(event) });
  }
}

export class AutonomousGoalPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly ledger: InMemoryAutonomousGoalLedger, readonly persistence: AutonomousGoalPersistence) {
    if (!ledger || typeof ledger.snapshot !== "function" || typeof ledger.restore !== "function") throw new ArgumentError("goal ledger is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("goal persistence is malformed");
  }

  async restore(): Promise<AutonomousGoalSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = validateAutonomousGoalSnapshot(raw);
      this.ledger.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousGoalSnapshot> {
    return this.enqueue(async () => {
      const snapshot = validateAutonomousGoalSnapshot(this.ledger.snapshot());
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("goal persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

/** Validate a goal restart image without mutating a caller's live ledger. */
export function validateAutonomousGoalSnapshot(raw: unknown): AutonomousGoalSnapshot {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA || !Array.isArray(raw.goals) || !Array.isArray(raw.events)) throw new ArgumentError("goal snapshot is malformed");
  const snapshot = raw as unknown as AutonomousGoalSnapshot;
  const allowed = new Set(["schema", "sequence", "head_digest", "goals", "events", "snapshot_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key)) || snapshot.retention !== AUTONOMOUS_GOAL_RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("goal snapshot contains unsupported or unsafe metadata");
  if (!Number.isSafeInteger(snapshot.sequence) || snapshot.sequence < 0 || snapshot.sequence !== snapshot.events.length || snapshot.goals.length > AUTONOMOUS_GOAL_MAX_GOALS || snapshot.events.length > AUTONOMOUS_GOAL_MAX_EVENTS) throw new ArgumentError("goal snapshot sequence or capacity is invalid");
  if (typeof snapshot.snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(snapshot.snapshot_digest)) throw new ArgumentError("goal snapshot digest is malformed");
  const { snapshot_digest: _snapshotDigest, ...body } = snapshot;
  if (digestJsonSync(body) !== snapshot.snapshot_digest) throw new ArgumentError("goal snapshot digest mismatch");
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal snapshot exceeds its byte bound");
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: AUTONOMOUS_GOAL_MAX_GOALS, maxEvents: AUTONOMOUS_GOAL_MAX_EVENTS });
  ledger.restore(snapshot);
  return clone(ledger.snapshot());
}

/** Strict JSON persistence for goal ledgers. */
export class JsonAutonomousGoalPersistence implements AutonomousGoalPersistence {
  constructor(readonly textStore: AutonomousGoalTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("goal text store is malformed");
  }

  async read(): Promise<AutonomousGoalSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("goal JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("goal JSON is not canonical");
    return validateAutonomousGoalSnapshot(parsed);
  }

  async write(snapshot: AutonomousGoalSnapshot): Promise<void> {
    const validated = validateAutonomousGoalSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

/** JSON persistence with compare-and-swap support for multi-writer goal handoffs. */
export class TransactionalJsonAutonomousGoalPersistence extends JsonAutonomousGoalPersistence {
  declare readonly textStore: AutonomousGoalTransactionalTextStore;

  constructor(textStore: AutonomousGoalTransactionalTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("goal text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalSnapshot): Promise<boolean> {
    const validated = validateAutonomousGoalSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

/** Browser-compatible goal persistence; callers choose the storage lifetime and encryption. */
export class WebStorageAutonomousGoalTextStore implements AutonomousGoalTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("goal Web Storage adapter is malformed");
    identifier(key, "goal storage key");
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}
