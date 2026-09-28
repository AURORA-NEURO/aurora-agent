import { ArgumentError, isObject } from "./errors.js";
import {
  autonomousGoalTimestampNowNs,
  compareAutonomousGoalTimestampNs,
  migrateLegacyAutonomousGoalTimestampNs,
  normalizeAutonomousGoalTimestampNs,
  requireAutonomousGoalTimestampNsWire,
  type AutonomousGoalLegacyTimestampUnit,
  type AutonomousGoalTimestampInput,
  type AutonomousGoalTimestampNs,
} from "./autonomous-goal-time.js";
import { canonicalJson, compareUnicodeScalars, digestBytesSync, digestJsonSync, hmacSha256HexSync, isUnicodeScalarString } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Goal records retain only bounded lifecycle and settlement identities; execution payloads remain transient. */

export const AUTONOMOUS_GOAL_SCHEMA = "bioprism-autonomous-goal/0.2" as const;
export const AUTONOMOUS_GOAL_EVENT_SCHEMA = "bioprism-autonomous-goal-event/0.2" as const;
export const AUTONOMOUS_GOAL_STEP_SCHEMA = "bioprism-autonomous-goal-step/0.1" as const;
export const AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-snapshot/0.2" as const;
export const AUTONOMOUS_GOAL_SCHEMA_V01 = "bioprism-autonomous-goal/0.1" as const;
export const AUTONOMOUS_GOAL_EVENT_SCHEMA_V01 = "bioprism-autonomous-goal-event/0.1" as const;
export const AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01 = "bioprism-autonomous-goal-snapshot/0.1" as const;
export const AUTONOMOUS_GOAL_AUTH_SCHEMA = "bioprism-autonomous-goal-auth/0.1" as const;
export const AUTONOMOUS_GOAL_MONOTONIC_ANCHOR_SCHEMA = "bioprism-autonomous-goal-monotonic-anchor/0.1" as const;
export const AUTONOMOUS_GOAL_RETENTION = "value_only_goal_state;task_prompt_response_tool_payloads_and_credentials_not_retained" as const;
export const AUTONOMOUS_GOAL_MAX_GOALS = 4_096;
export const AUTONOMOUS_GOAL_MAX_EVENTS = 16_384;
export const AUTONOMOUS_GOAL_MAX_CRITERIA = 64;
export const AUTONOMOUS_GOAL_MAX_BLOCKERS = 32;
export const AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES = 4_000_000;
export const AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES = AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES + 2_048;
export const AUTONOMOUS_GOAL_MAX_CLAIM_BATCH = 128;

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
  created_ns: AutonomousGoalTimestampNs;
  updated_ns: AutonomousGoalTimestampNs;
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
  created_ns: AutonomousGoalTimestampNs;
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
  migration?: AutonomousGoalMigrationProvenance;
}

export interface AutonomousGoalMigrationProvenance extends JsonObject {
  source_schema: typeof AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01;
  source_timestamp_unit: AutonomousGoalLegacyTimestampUnit;
  source_snapshot_digest: string | null;
  source_head_digest: string;
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

export interface AutonomousGoalMonotonicAnchor extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_MONOTONIC_ANCHOR_SCHEMA;
  sequence: number;
  snapshot_digest: string;
  head_digest: string;
  anchor_digest: string;
}

/** Deployment-owned anchor storage must survive rollback of the goal snapshot store. */
export interface TransactionalAutonomousGoalMonotonicAnchorStore {
  read(): unknown | Promise<unknown>;
  writeIfUnchanged(expectedAnchorDigest: string | null, value: Readonly<AutonomousGoalMonotonicAnchor>): boolean | Promise<boolean>;
}

function identifier(value: unknown, name: string): string {
  if (typeof value !== "string" || !isUnicodeScalarString(value) || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
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
  return rows.sort((left, right) => compareUnicodeScalars(left.criterion_id, right.criterion_id));
}

function normalizeBlockers(value: unknown): string[] {
  return [...new Set(sequence(value, "goal blockers", AUTONOMOUS_GOAL_MAX_BLOCKERS).map((item) => identifier(item, "goal blocker")))].sort(compareUnicodeScalars);
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
  created_ns: AutonomousGoalTimestampNs;
  updated_ns: AutonomousGoalTimestampNs;
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
  created_ns: AutonomousGoalTimestampInput;
  updated_ns: AutonomousGoalTimestampInput;
}): AutonomousGoalRecord {
  if (!(fields.status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal status is unsupported");
  const attempt = finiteInteger(fields.attempt, "goal attempt");
  const maxAttempts = finiteInteger(fields.max_attempts, "goal max_attempts", 1);
  if (maxAttempts > 128 || attempt > maxAttempts) throw new ArgumentError("goal attempt budget is outside its bounds");
  const created = normalizeAutonomousGoalTimestampNs(fields.created_ns, "goal created_ns");
  const updated = normalizeAutonomousGoalTimestampNs(fields.updated_ns, "goal updated_ns");
  if (compareAutonomousGoalTimestampNs(updated, created) < 0) throw new ArgumentError("goal updated_ns cannot precede created_ns");
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
  if (typeof task !== "string" || !isUnicodeScalarString(task) || !task.trim() || task.includes("\u0000") || new TextEncoder().encode(task).byteLength > 32_000) throw new ArgumentError("goal task is outside its bounded contract");
  return digestBytesSync(new TextEncoder().encode(task));
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function eventBody(event: Omit<AutonomousGoalEvent, "event_digest">): Omit<AutonomousGoalEvent, "event_digest"> {
  return event;
}

function verifyRecord(value: unknown): AutonomousGoalRecord {
  if (!isObject(value) || value.schema !== AUTONOMOUS_GOAL_SCHEMA) {
    if (isObject(value) && value.schema === AUTONOMOUS_GOAL_SCHEMA_V01) throw new ArgumentError("goal record uses schema 0.1; migrate its snapshot with an explicit legacy timestamp unit");
    throw new ArgumentError("goal record has an invalid schema");
  }
  if (value.retention !== AUTONOMOUS_GOAL_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("goal record retention contract is invalid");
  requireAutonomousGoalTimestampNsWire(value.created_ns, "goal created_ns");
  requireAutonomousGoalTimestampNsWire(value.updated_ns, "goal updated_ns");
  const record = buildRecord(value as unknown as Parameters<typeof buildRecord>[0]);
  if (value.state_digest !== record.state_digest) {
    const settlementFields = ["outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"];
    const { state_digest: _stateDigest, retention: _retention, secret_material: _secretMaterial, ...legacy } = record;
    for (const field of settlementFields) delete legacy[field as keyof typeof legacy];
    if (settlementFields.some((field) => field in value) || digestJsonSync(legacy) !== value.state_digest) throw new ArgumentError("goal state_digest does not match its content");
  }
  return { ...record, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
}

function normalizeGoalMigration(value: unknown): AutonomousGoalMigrationProvenance | null {
  if (value === undefined) return null;
  if (!isObject(value)) throw new ArgumentError("goal snapshot migration provenance is malformed");
  const expected = ["source_schema", "source_timestamp_unit", "source_snapshot_digest", "source_head_digest"];
  if (Object.keys(value).length !== expected.length || expected.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) throw new ArgumentError("goal snapshot migration provenance is malformed");
  if (value.source_schema !== AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01) throw new ArgumentError("goal snapshot migration source schema is unsupported");
  if (value.source_timestamp_unit !== "milliseconds" && value.source_timestamp_unit !== "nanoseconds") throw new ArgumentError("goal snapshot migration timestamp unit is invalid");
  if (value.source_snapshot_digest !== null && (typeof value.source_snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.source_snapshot_digest))) throw new ArgumentError("goal snapshot migration source digest is invalid");
  if (value.source_head_digest !== "" && (typeof value.source_head_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.source_head_digest))) throw new ArgumentError("goal snapshot migration source head is invalid");
  return {
    source_schema: AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01,
    source_timestamp_unit: value.source_timestamp_unit,
    source_snapshot_digest: value.source_snapshot_digest,
    source_head_digest: value.source_head_digest,
  };
}

function migrateLegacyGoalRecord(value: unknown, sourceUnit: AutonomousGoalLegacyTimestampUnit): AutonomousGoalRecord {
  if (!isObject(value) || value.schema !== AUTONOMOUS_GOAL_SCHEMA_V01) throw new ArgumentError("legacy goal record schema is unsupported");
  if (value.retention !== AUTONOMOUS_GOAL_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("legacy goal record retention contract is invalid");
  const settlementFields = ["outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"];
  const presentSettlement = settlementFields.filter((field) => Object.prototype.hasOwnProperty.call(value, field));
  if (presentSettlement.length !== 0 && presentSettlement.length !== settlementFields.length) throw new ArgumentError("legacy goal record has a partial settlement contract");
  const fields = ["schema", "goal_id", "task_digest", "domain", "capability", "risk_class", "status", "attempt", "max_attempts", "revision", "criteria", "blockers", "next_action_digest", ...settlementFields, "created_ns", "updated_ns", "state_digest", "retention", "secret_material"];
  if (Object.keys(value).some((field) => !fields.includes(field))) throw new ArgumentError("legacy goal record contains unsupported fields");
  const required = ["schema", "goal_id", "task_digest", "domain", "status", "attempt", "max_attempts", "revision", "created_ns", "updated_ns", "state_digest", "retention", "secret_material"];
  if (required.some((field) => !Object.prototype.hasOwnProperty.call(value, field))) throw new ArgumentError("legacy goal record is incomplete");
  const created = migrateLegacyAutonomousGoalTimestampNs(value.created_ns, sourceUnit, "legacy goal.created_ns");
  const updated = migrateLegacyAutonomousGoalTimestampNs(value.updated_ns, sourceUnit, "legacy goal.updated_ns");
  const record = buildRecord({
    goal_id: value.goal_id as string,
    task_digest: value.task_digest as string,
    domain: value.domain as string,
    capability: value.capability as string | null | undefined,
    risk_class: value.risk_class as string | null | undefined,
    status: value.status as AutonomousGoalStatus,
    attempt: value.attempt as number,
    max_attempts: value.max_attempts as number,
    revision: value.revision as number,
    criteria: value.criteria ?? [],
    blockers: value.blockers ?? [],
    next_action_digest: value.next_action_digest as string | null | undefined,
    outcome_digest: value.outcome_digest as string | null | undefined,
    evaluator_digest: value.evaluator_digest as string | null | undefined,
    learning_state_digest: value.learning_state_digest as string | null | undefined,
    progress_digest: value.progress_digest as string | null | undefined,
    created_ns: created,
    updated_ns: updated,
  });
  const { state_digest: _stateDigest, retention: _retention, secret_material: _secretMaterial, ...legacyState } = record;
  legacyState.schema = AUTONOMOUS_GOAL_SCHEMA_V01 as unknown as typeof AUTONOMOUS_GOAL_SCHEMA;
  legacyState.created_ns = value.created_ns as unknown as AutonomousGoalTimestampNs;
  legacyState.updated_ns = value.updated_ns as unknown as AutonomousGoalTimestampNs;
  if (presentSettlement.length === 0) for (const field of settlementFields) delete legacyState[field as keyof typeof legacyState];
  const legacyDigest = digestJsonSync(legacyState);
  if (value.state_digest !== legacyDigest) throw new ArgumentError("legacy goal state_digest does not match its content");
  const normalizedLegacy = { ...legacyState, state_digest: legacyDigest, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
  if (canonicalJson(value) !== canonicalJson(normalizedLegacy)) throw new ArgumentError("legacy goal record is not in canonical normalized form");
  return { ...record, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" };
}

/** Verify a v0.1 snapshot and its complete hash chain before converting explicit-unit timestamps. */
export function migrateLegacyAutonomousGoalSnapshot(raw: unknown, sourceUnit: AutonomousGoalLegacyTimestampUnit): AutonomousGoalSnapshot {
  if (sourceUnit !== "milliseconds" && sourceUnit !== "nanoseconds") throw new ArgumentError("legacy goal snapshot migration requires milliseconds or nanoseconds as the source unit");
  if (!isObject(raw)) throw new ArgumentError("legacy goal snapshot is malformed");
  const oldKeys = ["schema", "sequence", "head_digest", "goals", "events", "snapshot_digest", "retention", "secret_material"];
  if (Object.keys(raw).length !== oldKeys.length || oldKeys.some((key) => !Object.prototype.hasOwnProperty.call(raw, key))) throw new ArgumentError("legacy goal snapshot is malformed");
  if (raw.schema !== AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01 || raw.retention !== AUTONOMOUS_GOAL_RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("legacy goal snapshot metadata is unsupported");
  if (!Array.isArray(raw.goals) || raw.goals.length > AUTONOMOUS_GOAL_MAX_GOALS || !Array.isArray(raw.events) || raw.events.length > AUTONOMOUS_GOAL_MAX_EVENTS || typeof raw.sequence !== "number" || !Number.isSafeInteger(raw.sequence) || raw.sequence < 0 || raw.sequence !== raw.events.length) throw new ArgumentError("legacy goal snapshot sequence or capacity is invalid");
  if (new TextEncoder().encode(canonicalJson(raw)).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("legacy goal snapshot exceeds its byte bound");
  const { snapshot_digest: sourceDigest, ...sourceBody } = raw;
  if (typeof sourceDigest !== "string" || !/^[0-9a-f]{64}$/.test(sourceDigest) || digestJsonSync(sourceBody) !== sourceDigest) throw new ArgumentError("legacy goal snapshot digest mismatch");

  const goals = raw.goals.map((goal) => migrateLegacyGoalRecord(goal, sourceUnit)).sort((left, right) => compareUnicodeScalars(left.goal_id, right.goal_id));
  if (new Set(goals.map((goal) => goal.goal_id)).size !== goals.length) throw new ArgumentError("legacy goal snapshot contains duplicate goals");
  let sourcePrevious = "";
  let previous = "";
  const latest = new Map<string, string>();
  const events: AutonomousGoalEvent[] = [];
  const eventKeys = ["schema", "sequence", "goal_id", "event_type", "payload", "previous_digest", "created_ns", "event_digest", "retention", "secret_material"];
  for (const [index, item] of raw.events.entries()) {
    if (!isObject(item) || Object.keys(item).length !== eventKeys.length || eventKeys.some((key) => !Object.prototype.hasOwnProperty.call(item, key))) throw new ArgumentError("legacy goal snapshot event is malformed");
    if (item.schema !== AUTONOMOUS_GOAL_EVENT_SCHEMA_V01 || item.retention !== AUTONOMOUS_GOAL_RETENTION || item.secret_material !== "never_returned" || item.sequence !== index + 1 || item.previous_digest !== sourcePrevious || (item.event_type !== "created" && item.event_type !== "transition")) throw new ArgumentError("legacy goal event metadata or chain is invalid");
    const { event_digest: sourceEventDigest, ...sourceEventBody } = item;
    if (typeof sourceEventDigest !== "string" || !/^[0-9a-f]{64}$/.test(sourceEventDigest) || digestJsonSync(sourceEventBody) !== sourceEventDigest) throw new ArgumentError("legacy goal event digest does not match its metadata");
    const payload = migrateLegacyGoalRecord(item.payload, sourceUnit);
    const goalId = identifier(item.goal_id, "legacy goal event goal_id");
    const eventType = item.event_type as "created" | "transition";
    if (payload.goal_id !== goalId) throw new ArgumentError("legacy goal event payload identity is inconsistent");
    if (eventType === "created" ? latest.has(goalId) : !latest.has(goalId)) throw new ArgumentError("legacy goal snapshot event lifecycle is invalid");
    latest.set(goalId, payload.state_digest);
    const created = migrateLegacyAutonomousGoalTimestampNs(item.created_ns, sourceUnit, "legacy goal event timestamp");
    const eventBody = { schema: AUTONOMOUS_GOAL_EVENT_SCHEMA, sequence: index + 1, goal_id: goalId, event_type: eventType, payload, previous_digest: previous, created_ns: created, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const };
    const event = { ...eventBody, event_digest: digestJsonSync(eventBody) };
    events.push(event);
    sourcePrevious = sourceEventDigest;
    previous = event.event_digest;
  }
  if (raw.head_digest !== sourcePrevious || (raw.sequence === 0 ? raw.head_digest !== "" : typeof raw.head_digest !== "string" || !/^[0-9a-f]{64}$/.test(raw.head_digest))) throw new ArgumentError("legacy goal snapshot head digest is invalid");
  const byGoal = new Map(goals.map((goal) => [goal.goal_id, goal]));
  if (latest.size !== byGoal.size) throw new ArgumentError("legacy goal snapshot current goals are not represented by events");
  for (const [goalId, stateDigest] of latest) if (byGoal.get(goalId)?.state_digest !== stateDigest) throw new ArgumentError(`legacy goal snapshot current state is not bound to its latest event for ${goalId}`);
  const migration: AutonomousGoalMigrationProvenance = { source_schema: AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA_V01, source_timestamp_unit: sourceUnit, source_snapshot_digest: sourceDigest, source_head_digest: sourcePrevious };
  const body = { schema: AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA, sequence: events.length, head_digest: previous, goals, events, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const, migration };
  const migrated = { ...body, snapshot_digest: digestJsonSync(body) };
  return validateAutonomousGoalSnapshot(migrated);
}

export class InMemoryAutonomousGoalLedger {
  private readonly goals = new Map<string, AutonomousGoalRecord>();
  private readonly events: AutonomousGoalEvent[] = [];
  private migration: AutonomousGoalMigrationProvenance | null = null;
  private readonly maxGoals: number;
  private readonly maxEvents: number;
  private readonly clock: () => AutonomousGoalTimestampInput;

  constructor(options: { maxGoals?: number; maxEvents?: number; clock?: () => AutonomousGoalTimestampInput } = {}) {
    this.maxGoals = finiteInteger(options.maxGoals ?? AUTONOMOUS_GOAL_MAX_GOALS, "goal maxGoals", 1);
    this.maxEvents = finiteInteger(options.maxEvents ?? AUTONOMOUS_GOAL_MAX_EVENTS, "goal maxEvents", 1);
    if (this.maxGoals > AUTONOMOUS_GOAL_MAX_GOALS || this.maxEvents > AUTONOMOUS_GOAL_MAX_EVENTS) throw new ArgumentError("goal ledger capacity is outside its bounds");
    this.clock = options.clock ?? autonomousGoalTimestampNowNs;
  }

  create(input: { goal_id: string; task_digest: string; domain: string; capability?: string | null; risk_class?: string | null; criteria?: readonly AutonomousGoalCriterion[]; max_attempts?: number; now_ns?: AutonomousGoalTimestampInput }): AutonomousGoalRecord {
    const now = input.now_ns ?? this.clock();
    const record = buildRecord({ ...input, max_attempts: input.max_attempts ?? 8, status: "ready", attempt: 0, revision: 0, criteria: input.criteria ?? [], blockers: [], created_ns: now, updated_ns: now });
    const prior = this.goals.get(record.goal_id);
    if (prior) {
      if (goalIdentity(prior) !== goalIdentity(record)) throw new ArgumentError("goal_id already exists with a different identity");
      return clone(prior);
    }
    if (this.goals.size >= this.maxGoals) throw new ArgumentError("goal ledger capacity is exhausted");
    if (this.events.length + 1 + this.reservedFutureEvents() > this.maxEvents) throw new ArgumentError("goal event capacity must preserve active execution and reconciliation transitions");
    this.append("created", record, record.updated_ns);
    this.goals.set(record.goal_id, record);
    return clone(record);
  }

  get(goalId: string): AutonomousGoalRecord | null {
    return clone(this.goals.get(identifier(goalId, "goal_id")) ?? null);
  }

  list(query: { domain?: string; statuses?: readonly AutonomousGoalStatus[]; limit?: number } = {}): AutonomousGoalRecord[] {
    const limit = finiteInteger(query.limit ?? 128, "goal list limit", 1);
    if (limit > AUTONOMOUS_GOAL_MAX_GOALS) throw new ArgumentError(`goal list limit must be at most ${AUTONOMOUS_GOAL_MAX_GOALS}`);
    const domain = query.domain === undefined ? undefined : identifier(query.domain, "goal domain");
    const statuses = new Set(query.statuses ?? []);
    for (const status of statuses) if (!(status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal list contains an unsupported status");
    return [...this.goals.values()].filter((goal) => (domain === undefined || goal.domain === domain) && (!statuses.size || statuses.has(goal.status))).sort((left, right) => -compareAutonomousGoalTimestampNs(left.updated_ns, right.updated_ns) || compareUnicodeScalars(left.goal_id, right.goal_id)).slice(0, limit).map(clone);
  }

  transition(goalId: string, status: AutonomousGoalStatus, options: { expected_revision?: number; criterion_updates?: readonly JsonObject[]; blockers?: readonly string[]; next_action_digest?: string | null; outcome_digest?: string | null; evaluator_digest?: string | null; learning_state_digest?: string | null; progress_digest?: string | null; now_ns?: AutonomousGoalTimestampInput } = {}): AutonomousGoalRecord {
    const id = identifier(goalId, "goal_id");
    const current = this.goals.get(id);
    if (!current) throw new ArgumentError(`goal ${id} was not found`);
    if (!(status in ALLOWED_TRANSITIONS)) throw new ArgumentError("goal status is unsupported");
    const recoveredDispatchSettlement = current.status === "blocked"
      && current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")
      && (status === "completed" || status === "failed" || status === "paused")
      && options.outcome_digest !== undefined
      && options.outcome_digest !== null;
    if (recoveredDispatchSettlement) digest(options.outcome_digest, "outcome_digest")!;
    if (status !== current.status && !ALLOWED_TRANSITIONS[current.status].includes(status) && !recoveredDispatchSettlement) throw new ArgumentError(`goal cannot transition from ${current.status} to ${status}`);
    if (options.expected_revision !== undefined) {
      const expectedRevision = finiteInteger(options.expected_revision, "expected_revision");
      if (expectedRevision !== current.revision) throw new ArgumentError(`goal revision conflict: expected ${expectedRevision}, observed ${current.revision}`);
    }
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
    this.append("transition", updated, updated.updated_ns);
    this.goals.set(id, updated);
    return clone(updated);
  }

  /** Atomically claim an optimistic batch; failed retries reopen and claim within the same batch. */
  claimMany(claims: readonly { goal_id: string; expected_revision: number; expected_status: "ready" | "paused" | "failed"; expected_domain: string; dependencies: readonly string[] }[], options: { now_ns?: AutonomousGoalTimestampInput; max_concurrent: number } ): AutonomousGoalRecord[] {
    if (!Array.isArray(claims) || claims.length < 1 || claims.length > AUTONOMOUS_GOAL_MAX_CLAIM_BATCH) throw new ArgumentError("goal claim batch is outside its bounds");
    const now = normalizeAutonomousGoalTimestampNs(options.now_ns ?? this.clock(), "goal claim batch now_ns");
    const maxConcurrent = finiteInteger(options.max_concurrent, "goal claim batch max_concurrent", 1);
    if (maxConcurrent > AUTONOMOUS_GOAL_MAX_CLAIM_BATCH) throw new ArgumentError("goal claim batch max_concurrent is outside its bounds");
    const seen = new Set<string>();
    const prepared: { id: string; current: AutonomousGoalRecord; transitions: AutonomousGoalRecord[] }[] = [];
    let requiredEvents = 0;
    for (const [index, claim] of claims.entries()) {
      if (!isObject(claim) || Object.keys(claim).length !== 5 || !["goal_id", "expected_revision", "expected_status", "expected_domain", "dependencies"].every((key) => Object.prototype.hasOwnProperty.call(claim, key))) throw new ArgumentError(`goal claim batch row ${index} is malformed`);
      const id = identifier(claim.goal_id, "goal claim batch goal_id");
      if (seen.has(id)) throw new ArgumentError("goal claim batch contains duplicate goal IDs");
      seen.add(id);
      const revision = finiteInteger(claim.expected_revision, `goal claim batch ${id}.expected_revision`, 0);
      if (!(claim.expected_status === "ready" || claim.expected_status === "paused" || claim.expected_status === "failed")) throw new ArgumentError(`goal claim batch ${id} has an ineligible expected status`);
      const expectedDomain = identifier(claim.expected_domain, `goal claim batch ${id}.expected_domain`);
      if (!Array.isArray(claim.dependencies) || claim.dependencies.length > 128) throw new ArgumentError(`goal claim batch ${id}.dependencies are malformed`);
      const dependencies = claim.dependencies.map((dependency) => identifier(dependency, `goal claim batch ${id}.dependency`));
      if (new Set(dependencies).size !== dependencies.length) throw new ArgumentError(`goal claim batch ${id}.dependencies contain duplicates`);
      const current = this.goals.get(id);
      if (!current || current.revision !== revision || current.status !== claim.expected_status) throw new ArgumentError(`goal ${id} claim is stale: state changed after scheduling`);
      if (current.domain !== expectedDomain) throw new ArgumentError(`goal ${id} claim is stale: domain changed after scheduling`);
      if (current.status === "paused" && current.blockers.includes("required_goal_criteria_review")) throw new ArgumentError(`goal ${id} requires goal-criteria review before resuming`);
      if (current.attempt >= current.max_attempts) throw new ArgumentError(`goal ${id} attempt budget is exhausted`);
      let base = current;
      const transitions: AutonomousGoalRecord[] = [];
      if (current.status === "failed") {
        base = buildRecord({ goal_id: current.goal_id, task_digest: current.task_digest, domain: current.domain, capability: current.capability, risk_class: current.risk_class, status: "ready", attempt: current.attempt, max_attempts: current.max_attempts, revision: current.revision + 1, criteria: current.criteria, blockers: [], next_action_digest: null, outcome_digest: current.outcome_digest, evaluator_digest: current.evaluator_digest, learning_state_digest: current.learning_state_digest, progress_digest: current.progress_digest, created_ns: current.created_ns, updated_ns: now });
        transitions.push(base);
      }
      const running = buildRecord({ goal_id: base.goal_id, task_digest: base.task_digest, domain: base.domain, capability: base.capability, risk_class: base.risk_class, status: "running", attempt: base.attempt + 1, max_attempts: base.max_attempts, revision: base.revision + 1, criteria: base.criteria, blockers: [], next_action_digest: null, outcome_digest: base.outcome_digest, evaluator_digest: base.evaluator_digest, learning_state_digest: base.learning_state_digest, progress_digest: base.progress_digest, created_ns: base.created_ns, updated_ns: now });
      transitions.push(running);
      requiredEvents += transitions.length;
      prepared.push({ id, current, transitions });
    }
    const selectedPositions = new Map(prepared.map((item, index) => [item.id, index]));
    for (const [index, claim] of claims.entries()) {
      for (const dependencyValue of claim.dependencies) {
        const dependency = identifier(dependencyValue, `goal claim batch ${claim.goal_id}.dependency`);
        const selectedPosition = selectedPositions.get(dependency);
        if (selectedPosition !== undefined) {
          if (selectedPosition >= index) throw new ArgumentError("goal claim batch does not order prerequisites before dependants");
          continue;
        }
        if (this.goals.get(dependency)?.status !== "completed") throw new ArgumentError(`goal dependency ${dependency} is not completed or claimed first`);
      }
    }
    const activeCount = [...this.goals.values()].filter((goal) => goal.status === "running").length;
    if (activeCount + prepared.length > maxConcurrent) throw new ArgumentError("goal claim batch exceeds the current concurrency budget");
    if (this.events.length + requiredEvents + this.reservedFutureEvents() + 2 * prepared.length > this.maxEvents) throw new ArgumentError("goal event capacity cannot accommodate claims and their recovery transitions");

    for (const item of prepared) {
      for (const next of item.transitions) {
        this.goals.set(item.id, next);
        this.append("transition", next, now);
      }
    }
    return prepared.map((item) => clone(item.transitions.at(-1)!));
  }

  updateCriteria(goalId: string, updates: readonly JsonObject[], options: { expected_revision?: number; now_ns?: AutonomousGoalTimestampInput } = {}): AutonomousGoalRecord {
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
      requireAutonomousGoalTimestampNsWire(event.created_ns, "goal event created_ns");
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
    const body = { schema: AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA, sequence: this.events.length, head_digest: this.events.at(-1)?.event_digest ?? "", goals: [...this.goals.values()].sort((left, right) => compareUnicodeScalars(left.goal_id, right.goal_id)).map(clone), events: this.events.map(clone), retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const, ...(this.migration ? { migration: this.migration } : {}) };
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) };
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal snapshot exceeds its byte bound");
    return snapshot;
  }

  restore(snapshot: AutonomousGoalSnapshot): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_GOAL_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.goals) || !Array.isArray(snapshot.events)) throw new ArgumentError("goal snapshot is malformed");
    const allowed = new Set(["schema", "sequence", "head_digest", "goals", "events", "snapshot_digest", "retention", "secret_material", "migration"]);
    if (Object.keys(snapshot).some((key) => !allowed.has(key)) || snapshot.retention !== AUTONOMOUS_GOAL_RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("goal snapshot contains unsupported or unsafe metadata");
    if (!Number.isSafeInteger(snapshot.sequence) || snapshot.sequence < 0 || snapshot.sequence !== snapshot.events.length || snapshot.events.length > this.maxEvents || snapshot.goals.length > this.maxGoals) throw new ArgumentError("goal snapshot sequence or capacity is invalid");
    if (typeof snapshot.head_digest !== "string" || (snapshot.sequence > 0 && !/^[0-9a-f]{64}$/.test(snapshot.head_digest)) || (snapshot.sequence === 0 && snapshot.head_digest !== "")) throw new ArgumentError("goal snapshot head digest is invalid");
    const { snapshot_digest: supplied, ...body } = snapshot;
    if (typeof supplied !== "string" || !/^[0-9a-f]{64}$/.test(supplied) || digestJsonSync(body) !== supplied) throw new ArgumentError("goal snapshot digest mismatch");
    const migration = normalizeGoalMigration(snapshot.migration);
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
    if (restored.events.length + restored.reservedFutureEvents() > this.maxEvents) throw new ArgumentError("goal snapshot capacity cannot preserve active execution and reconciliation transitions");
    if (restored.events.length !== snapshot.sequence || (restored.events.at(-1)?.event_digest ?? "") !== snapshot.head_digest) throw new ArgumentError("goal snapshot head is inconsistent");
    restored.verifyIntegrity();
    this.goals.clear();
    for (const [id, goal] of restored.goals) this.goals.set(id, goal);
    this.events.splice(0, this.events.length, ...restored.events);
    this.migration = migration;
  }

  private append(eventType: "created" | "transition", payload: AutonomousGoalRecord, created: AutonomousGoalTimestampNs): void {
    if (this.events.length >= this.maxEvents) throw new ArgumentError("goal event capacity is exhausted");
    const event = { schema: AUTONOMOUS_GOAL_EVENT_SCHEMA, sequence: this.events.length + 1, goal_id: payload.goal_id, event_type: eventType, payload: clone(payload), previous_digest: this.events.at(-1)?.event_digest ?? "", created_ns: created, retention: AUTONOMOUS_GOAL_RETENTION, secret_material: "never_returned" as const };
    this.events.push({ ...event, event_digest: digestJsonSync(event) });
  }

  private futureEventReserve(record: AutonomousGoalRecord): number {
    // Running work may need a restart hold and a later verified resolution;
    // an existing reconciliation hold needs only the resolution transition.
    if (record.status === "running") return 2;
    if (record.status === "blocked" && record.blockers.some((blocker) => blocker === "worker_restart_after_dispatch_requires_reconciliation" || blocker === "worker_dispatch_outcome_requires_reconciliation")) return 1;
    return 0;
  }

  private reservedFutureEvents(replacement?: AutonomousGoalRecord): number {
    let reserve = 0;
    for (const current of this.goals.values()) reserve += this.futureEventReserve(replacement?.goal_id === current.goal_id ? replacement : current);
    return reserve;
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
  const allowed = new Set(["schema", "sequence", "head_digest", "goals", "events", "snapshot_digest", "retention", "secret_material", "migration"]);
  if (Object.keys(raw).some((key) => !allowed.has(key)) || snapshot.retention !== AUTONOMOUS_GOAL_RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("goal snapshot contains unsupported or unsafe metadata");
  if (!Number.isSafeInteger(snapshot.sequence) || snapshot.sequence < 0 || snapshot.sequence !== snapshot.events.length || snapshot.goals.length > AUTONOMOUS_GOAL_MAX_GOALS || snapshot.events.length > AUTONOMOUS_GOAL_MAX_EVENTS) throw new ArgumentError("goal snapshot sequence or capacity is invalid");
  if (typeof snapshot.snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(snapshot.snapshot_digest)) throw new ArgumentError("goal snapshot digest is malformed");
  const { snapshot_digest: _snapshotDigest, ...body } = snapshot;
  const migration = normalizeGoalMigration(snapshot.migration);
  if (migration) body.migration = migration;
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
    if (!isUnicodeScalarString(encoded) || new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_MAX_SNAPSHOT_BYTES) throw new ArgumentError("goal JSON exceeds its byte bound");
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

interface AutonomousGoalSnapshotAuthentication extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_AUTH_SCHEMA;
  key_id: string;
  tag: string;
}

/** Authenticated goal snapshots for shared CAS stores; authentication does not provide anti-rollback. */
export class AuthenticatedTransactionalJsonAutonomousGoalPersistence extends TransactionalJsonAutonomousGoalPersistence {
  declare readonly textStore: AutonomousGoalTransactionalTextStore;
  readonly active_key_id: string;
  private readonly keys = new Map<string, Uint8Array>();

  constructor(options: {
    textStore: AutonomousGoalTransactionalTextStore;
    keys: ReadonlyMap<string, Uint8Array>;
    active_key_id: string;
  }) {
    super(options.textStore);
    if (!(options.keys instanceof Map) || options.keys.size < 1 || options.keys.size > 16) throw new ArgumentError("authenticated goal keyring must contain 1..16 keys");
    for (const [rawId, rawKey] of options.keys) {
      const keyId = identifier(rawId, "authentication key id");
      if (!(rawKey instanceof Uint8Array) || rawKey.byteLength < 32 || rawKey.byteLength > 4_096) throw new ArgumentError("authenticated goal keyring contains an invalid key");
      if (this.keys.has(keyId)) throw new ArgumentError("authenticated goal keyring contains a duplicate normalized id");
      this.keys.set(keyId, new Uint8Array(rawKey));
    }
    this.active_key_id = identifier(options.active_key_id, "active authentication key id");
    if (!this.keys.has(this.active_key_id)) throw new ArgumentError("active authentication key id is not present in the keyring");
    this.textStore = options.textStore;
  }

  protected envelope(snapshotValue: unknown): AutonomousGoalSnapshot & { authentication: AutonomousGoalSnapshotAuthentication } {
    const snapshot = validateAutonomousGoalSnapshot(snapshotValue);
    const body = { schema: AUTONOMOUS_GOAL_AUTH_SCHEMA, key_id: this.active_key_id, snapshot };
    const tag = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(body)), this.keys.get(this.active_key_id)!);
    return { ...snapshot, authentication: { schema: AUTONOMOUS_GOAL_AUTH_SCHEMA, key_id: this.active_key_id, tag } };
  }

  override async read(): Promise<AutonomousGoalSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || !isUnicodeScalarString(encoded) || new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES) throw new ArgumentError("authenticated goal JSON is outside its byte bound");
    let raw: unknown;
    try { raw = JSON.parse(encoded); } catch { throw new ArgumentError("authenticated goal JSON is invalid"); }
    if (!isObject(raw) || !isObject(raw.authentication)) throw new ArgumentError("authenticated goal envelope is malformed");
    const authentication = raw.authentication;
    if (Object.keys(authentication).sort().join(",") !== "key_id,schema,tag" || authentication.schema !== AUTONOMOUS_GOAL_AUTH_SCHEMA) throw new ArgumentError("authenticated goal envelope markers are invalid");
    const keyId = identifier(authentication.key_id, "authentication key id");
    const suppliedTag = digest(authentication.tag, "authentication tag")!;
    const key = this.keys.get(keyId);
    if (key === undefined) throw new ArgumentError("authenticated goal key id is not trusted by this keyring");
    const snapshotRaw = Object.fromEntries(Object.entries(raw).filter(([name]) => name !== "authentication"));
    const snapshot = validateAutonomousGoalSnapshot(snapshotRaw);
    const body = { schema: AUTONOMOUS_GOAL_AUTH_SCHEMA, key_id: keyId, snapshot };
    const expectedTag = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(body)), key);
    let mismatch = 0;
    for (let index = 0; index < expectedTag.length; index += 1) mismatch |= expectedTag.charCodeAt(index) ^ suppliedTag.charCodeAt(index);
    if (mismatch !== 0) throw new ArgumentError("authenticated goal tag does not match its snapshot");
    const normalizedEnvelope = { ...snapshot, authentication: { schema: AUTONOMOUS_GOAL_AUTH_SCHEMA, key_id: keyId, tag: suppliedTag } };
    if (canonicalJson(normalizedEnvelope) !== encoded) throw new ArgumentError("authenticated goal JSON is not canonical");
    return snapshot;
  }

  override async write(snapshot: AutonomousGoalSnapshot): Promise<void> {
    const current = await this.read();
    if (!await this.writeIfUnchanged(current?.snapshot_digest ?? null, snapshot)) throw new ArgumentError("authenticated goal persistence compare-and-swap conflict");
  }

  override async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest(expectedSnapshotDigest, "expected_snapshot_digest");
    const envelope = this.envelope(snapshot);
    if (new TextEncoder().encode(canonicalJson(envelope)).byteLength > AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES) throw new ArgumentError("authenticated goal snapshot exceeds its byte bound");
    if (expectedSnapshotDigest === envelope.snapshot_digest) {
      const current = await this.read();
      return current !== null && current.snapshot_digest === expectedSnapshotDigest;
    }
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(envelope));
  }
}

function normalizeGoalMonotonicAnchor(value: unknown): AutonomousGoalMonotonicAnchor | null {
  if (value === null) return null;
  if (!isObject(value) || Object.keys(value).sort().join(",") !== "anchor_digest,head_digest,schema,sequence,snapshot_digest" || value.schema !== AUTONOMOUS_GOAL_MONOTONIC_ANCHOR_SCHEMA) throw new ArgumentError("goal monotonic anchor record is malformed");
  const body = {
    schema: AUTONOMOUS_GOAL_MONOTONIC_ANCHOR_SCHEMA,
    sequence: finiteInteger(value.sequence, "goal anchor sequence"),
    snapshot_digest: digest(value.snapshot_digest, "goal anchor snapshot_digest")!,
    head_digest: value.head_digest === "" ? "" : digest(value.head_digest, "goal anchor head_digest")!,
  };
  if (body.sequence > AUTONOMOUS_GOAL_MAX_EVENTS) throw new ArgumentError("goal monotonic anchor sequence exceeds its journal bound");
  const anchorDigest = digest(value.anchor_digest, "goal anchor digest")!;
  if (digestJsonSync(body) !== anchorDigest) throw new ArgumentError("goal monotonic anchor digest does not match its content");
  return Object.freeze({ ...body, anchor_digest: anchorDigest });
}

function goalAnchorForSnapshot(snapshot: AutonomousGoalSnapshot): AutonomousGoalMonotonicAnchor {
  const body = {
    schema: AUTONOMOUS_GOAL_MONOTONIC_ANCHOR_SCHEMA,
    sequence: snapshot.sequence,
    snapshot_digest: snapshot.snapshot_digest,
    head_digest: snapshot.head_digest,
  };
  return Object.freeze({ ...body, anchor_digest: digestJsonSync(body) });
}

/** HMAC goal persistence with a caller-owned non-rollback anchor and monotonic journal sequence. */
export class MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalPersistence extends AuthenticatedTransactionalJsonAutonomousGoalPersistence {
  constructor(options: {
    textStore: AutonomousGoalTransactionalTextStore;
    anchor: TransactionalAutonomousGoalMonotonicAnchorStore;
    keys: ReadonlyMap<string, Uint8Array>;
    active_key_id: string;
  }) {
    super(options);
    if (!options.anchor || typeof options.anchor.read !== "function" || typeof options.anchor.writeIfUnchanged !== "function") throw new ArgumentError("goal monotonic anchor must implement read and writeIfUnchanged");
    this.anchor = options.anchor;
  }

  private readonly anchor: TransactionalAutonomousGoalMonotonicAnchorStore;

  private async readAnchor(): Promise<AutonomousGoalMonotonicAnchor | null> {
    return normalizeGoalMonotonicAnchor(await this.anchor.read());
  }

  override async read(): Promise<AutonomousGoalSnapshot | null> {
    const snapshot = await super.read();
    const anchor = await this.readAnchor();
    if (snapshot === null && anchor === null) return null;
    if (snapshot === null || anchor === null) throw new ArgumentError("goal monotonic anchor and authenticated snapshot are not both present");
    if (canonicalJson(goalAnchorForSnapshot(snapshot)) !== canonicalJson(anchor)) throw new ArgumentError("goal monotonic anchor does not match the authenticated snapshot; rollback or incomplete commit detected");
    return snapshot;
  }

  override async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshotValue: AutonomousGoalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest(expectedSnapshotDigest, "expected_snapshot_digest");
    const current = await this.read();
    const currentDigest = current?.snapshot_digest ?? null;
    if (currentDigest !== expectedSnapshotDigest) return false;
    const snapshot = validateAutonomousGoalSnapshot(snapshotValue);
    if (current !== null && snapshot.snapshot_digest === currentDigest) return true;
    if (current !== null && snapshot.sequence <= current.sequence) throw new ArgumentError("goal monotonic snapshot sequence must advance");
    const encoded = canonicalJson(this.envelope(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES) throw new ArgumentError("authenticated goal snapshot exceeds its byte bound");
    const currentAnchor = current === null ? null : await this.readAnchor();
    const nextAnchor = goalAnchorForSnapshot(snapshot);
    const anchorAdvanced = await this.anchor.writeIfUnchanged(currentAnchor?.anchor_digest ?? null, nextAnchor);
    if (anchorAdvanced !== true && anchorAdvanced !== false) throw new ArgumentError("goal monotonic anchor compare-and-set must return a boolean");
    if (anchorAdvanced === false) return false;
    const snapshotWritten = await this.textStore.writeIfUnchanged(expectedSnapshotDigest, encoded);
    if (snapshotWritten !== true && snapshotWritten !== false) throw new ArgumentError("goal snapshot compare-and-set must return a boolean");
    if (snapshotWritten === false) throw new ArgumentError("goal monotonic anchor advanced before snapshot compare-and-swap; matching anchor-bound snapshot roll-forward is required");
    return true;
  }

  async rollForward(snapshotValue: AutonomousGoalSnapshot): Promise<AutonomousGoalSnapshot> {
    const snapshot = validateAutonomousGoalSnapshot(snapshotValue);
    const anchor = await this.readAnchor();
    if (anchor === null || canonicalJson(goalAnchorForSnapshot(snapshot)) !== canonicalJson(anchor)) throw new ArgumentError("goal roll-forward snapshot does not match the trusted monotonic anchor");
    const current = await super.read();
    if (current !== null && current.snapshot_digest === snapshot.snapshot_digest) {
      const restored = await this.read();
      if (restored === null) throw new ArgumentError("goal roll-forward did not restore the anchor-bound snapshot");
      return restored;
    }
    if (current !== null && current.sequence >= snapshot.sequence) throw new ArgumentError("goal roll-forward snapshot does not advance the stored ledger");
    const encoded = canonicalJson(this.envelope(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES) throw new ArgumentError("authenticated goal snapshot exceeds its byte bound");
    const expected = current?.snapshot_digest ?? null;
    const written = await this.textStore.writeIfUnchanged(expected, encoded);
    if (written !== true && written !== false) throw new ArgumentError("goal snapshot compare-and-set must return a boolean");
    if (written === false) throw new ArgumentError("goal roll-forward compare-and-swap conflict");
    const restored = await this.read();
    if (restored === null || restored.snapshot_digest !== snapshot.snapshot_digest) throw new ArgumentError("goal roll-forward did not restore the anchor-bound snapshot");
    return restored;
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
