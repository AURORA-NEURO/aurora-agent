import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_RETENTION,
  goalTaskDigest,
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalRecord,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import { canonicalJson, digestJsonSync, hmacSha256HexSync, isUnicodeScalarString } from "./tooling.js";
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
import type { JsonObject } from "./types.js";

/** Metadata-only restart fencing for the goal executor/provider boundary. */
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SCHEMA_V01 = "bioprism-autonomous-goal-worker-journal/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01 = "bioprism-autonomous-goal-worker-event/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01 = "bioprism-autonomous-goal-worker-snapshot/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SCHEMA = "bioprism-autonomous-goal-worker-journal/0.2" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA = "bioprism-autonomous-goal-worker-event/0.2" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-worker-snapshot/0.2" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION = "metadata_only_worker_boundary;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS = 16_384;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES = 2_000_000;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01 = "bioprism-autonomous-goal-worker-journal-auth/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA = "bioprism-autonomous-goal-worker-journal-auth/0.2" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES = AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES + 2_048;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MONOTONIC_ANCHOR_SCHEMA = "bioprism-autonomous-goal-worker-journal-monotonic-anchor/0.1" as const;
export const AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA_V01 = "bioprism-autonomous-goal-dispatch-resolution/0.1" as const;
export const AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA = "bioprism-autonomous-goal-dispatch-resolution/0.2" as const;
export const AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION = "metadata_only_dispatch_status;external_evidence_payloads_not_retained" as const;

export type AutonomousGoalWorkerJournalPhase = "prepared" | "claimed" | "dispatch_started" | "settled" | "failed" | "reconciled";
const ACTIVE_PHASES = new Set<AutonomousGoalWorkerJournalPhase>(["prepared", "claimed", "dispatch_started"]);
const ALL_PHASES = new Set<AutonomousGoalWorkerJournalPhase>(["prepared", "claimed", "dispatch_started", "settled", "failed", "reconciled"]);
export type AutonomousGoalDispatchResolutionStatus = "completed" | "not_applied" | "failed" | "pending" | "unknown";
type DispatchResolutionGoalStatus = "completed" | "failed" | "ready" | "blocked" | "paused";

export interface AutonomousGoalDispatchResolution extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA;
  goal_id: string;
  attempt: number;
  dispatch_event_digest: string;
  execution_binding_digest: string;
  status: AutonomousGoalDispatchResolutionStatus;
  evidence_digest: string;
  verifier_id: string;
  observed_ns: AutonomousGoalTimestampNs;
  retention: typeof AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION;
  secret_material: "never_returned";
}

/** Deployment-owned authenticator for an externally obtained dispatch status receipt. */
export interface AutonomousGoalDispatchResolutionVerifier {
  readonly verifier_id: string;
  /** Authenticate this exact metadata-only receipt with deployment-owned trust material. */
  verify(resolution: Readonly<AutonomousGoalDispatchResolution>): boolean;
}

export interface AutonomousGoalDispatchResolutionResult extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA;
  goal_id: string;
  attempt: number;
  dispatch_event_digest: string;
  resolution_digest: string;
  status: AutonomousGoalDispatchResolutionStatus;
  goal_status: AutonomousGoalStatus;
  goal_revision: number;
  idempotent: boolean;
  retention: typeof AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION;
  secret_material: "never_returned";
}

interface PreparedAutonomousGoalDispatchOutcome {
  resolution: AutonomousGoalDispatchResolution;
  dispatch: AutonomousGoalWorkerEvent;
  target_status: DispatchResolutionGoalStatus;
  resolution_digest: string;
  expected_revision: number;
  expected_outcome_digest: string | null;
  expected_head_digest: string;
  idempotent: boolean;
  already_staged?: boolean;
  goal_revision?: number;
}

export interface AutonomousGoalWorkerEvent extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA;
  sequence: number;
  batch_id: string;
  goal_id: string;
  phase: AutonomousGoalWorkerJournalPhase;
  attempt: number;
  revision: number;
  schedule_digest: string;
  claim_digest: string | null;
  outcome_digest: string | null;
  error_digest: string | null;
  /** Digest of the immutable goal task identity, when emitted by a worker. */
  task_digest?: string;
  /** Digest of transient executor parameters; raw bindings are never journaled. */
  execution_binding_digest?: string;
  /** Bounded goal outcome needed to replay a staged status receipt after a crash. */
  resolution_goal_status?: DispatchResolutionGoalStatus;
  created_ns: AutonomousGoalTimestampNs;
  previous_digest: string;
  event_digest: string;
  retention: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousGoalWorkerJournalSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  events: AutonomousGoalWorkerEvent[];
  snapshot_digest: string;
  retention: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION;
  secret_material: "never_returned";
  migration?: AutonomousGoalWorkerJournalMigration;
}

export interface AutonomousGoalWorkerJournalMigration extends JsonObject {
  source_schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01;
  source_timestamp_unit: AutonomousGoalLegacyTimestampUnit;
  source_snapshot_digest: string;
  source_head_digest: string;
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal worker journal ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !isUnicodeScalarString(value) || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is outside its bounded identifier contract`);
  return value.trim();
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bounds`);
  return value;
}

function normalizeJournalMigration(value: unknown): AutonomousGoalWorkerJournalMigration {
  if (!isObject(value) || Object.keys(value).sort().join(",") !== "source_head_digest,source_schema,source_snapshot_digest,source_timestamp_unit") fail("snapshot migration provenance is malformed");
  if (value.source_schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01) fail("snapshot migration source schema is unsupported");
  if (value.source_timestamp_unit !== "milliseconds" && value.source_timestamp_unit !== "nanoseconds") fail("snapshot migration timestamp unit is invalid");
  const sourceSnapshotDigest = digest("snapshot migration source digest", value.source_snapshot_digest)!;
  const sourceHeadDigest = value.source_head_digest === "" ? "" : digest("snapshot migration source head", value.source_head_digest)!;
  return {
    source_schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    source_timestamp_unit: value.source_timestamp_unit,
    source_snapshot_digest: sourceSnapshotDigest,
    source_head_digest: sourceHeadDigest,
  };
}

function restartOutcomeDigest(goalId: string, attempt: number, beforeDispatch: boolean): string {
  return digestJsonSync({
    goal_id: goalId,
    attempt,
    result_status: beforeDispatch ? "worker_restart_before_dispatch" : "worker_restart_after_dispatch",
  });
}

function needsExternalReconciliation(event: AutonomousGoalWorkerEvent): boolean {
  if (event.phase !== "reconciled") return false;
  // A post-dispatch restart receipt records that the goal is blocked, but does not
  // settle the external call. Keep it active across later process restarts until a
  // caller supplies a status receipt. A pre-dispatch restart is safe to retry.
  return event.outcome_digest !== restartOutcomeDigest(event.goal_id, event.attempt, true);
}

function futureEventReserve(event: AutonomousGoalWorkerEvent): number {
  if (event.phase === "prepared") return 1;
  if (event.phase === "claimed") return 1;
  if (event.phase === "dispatch_started") return 3;
  if (event.phase === "reconciled" && needsExternalReconciliation(event)) {
    // A pending/unknown receipt needs a later receipt and settlement. A staged
    // terminal receipt needs only the final settled marker.
    return event.resolution_goal_status === undefined || event.resolution_goal_status === "blocked" ? 2 : 1;
  }
  return 0;
}

function normalizeDispatchResolution(value: unknown): AutonomousGoalDispatchResolution {
  if (!isObject(value)) fail("dispatch resolution must be an object");
  const expected = ["schema", "goal_id", "attempt", "dispatch_event_digest", "execution_binding_digest", "status", "evidence_digest", "verifier_id", "observed_ns", "retention", "secret_material"];
  const allowed = new Set(expected);
  if (Object.keys(value).some((key) => !allowed.has(key)) || expected.some((key) => !(key in value))) fail("dispatch resolution contains unsupported or missing fields");
  if (value.schema === AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA_V01) fail("legacy dispatch resolution requires external re-verification and re-issuance under schema 0.2");
  if (value.schema !== AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA || value.retention !== AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION || value.secret_material !== "never_returned") fail("dispatch resolution markers are invalid");
  if (!["completed", "not_applied", "failed", "pending", "unknown"].includes(String(value.status))) fail("dispatch resolution status is invalid");
  return {
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
    goal_id: identifier("dispatch resolution goal_id", value.goal_id),
    attempt: integer("dispatch resolution attempt", value.attempt, 1, 128),
    dispatch_event_digest: digest("dispatch resolution dispatch_event_digest", value.dispatch_event_digest)!,
    execution_binding_digest: digest("dispatch resolution execution_binding_digest", value.execution_binding_digest)!,
    status: value.status as AutonomousGoalDispatchResolutionStatus,
    evidence_digest: digest("dispatch resolution evidence_digest", value.evidence_digest)!,
    verifier_id: identifier("dispatch resolution verifier_id", value.verifier_id),
    observed_ns: requireAutonomousGoalTimestampNsWire(value.observed_ns, "dispatch resolution observed_ns"),
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  };
}

function authenticateDispatchResolution(
  raw: unknown,
  verifier: AutonomousGoalDispatchResolutionVerifier | undefined,
): AutonomousGoalDispatchResolution {
  const resolution = normalizeDispatchResolution(raw);
  if (!verifier || typeof verifier.verify !== "function") fail("external status reconciliation requires a deployment-owned verifier");
  const verifierId = identifier("configured verifier_id", verifier.verifier_id);
  if (resolution.verifier_id !== verifierId) fail("dispatch resolution verifier_id does not match the configured status authority");
  let verified: boolean;
  try {
    verified = verifier.verify(Object.freeze({ ...resolution }));
  } catch {
    fail("deployment verifier failed");
  }
  if (verified !== true) fail("deployment verifier rejected the dispatch resolution");
  return resolution;
}

export function validateAutonomousGoalDispatchResolution(value: unknown): AutonomousGoalDispatchResolution {
  return clone(normalizeDispatchResolution(value));
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function eventBody(event: Omit<AutonomousGoalWorkerEvent, "event_digest">): Omit<AutonomousGoalWorkerEvent, "event_digest"> {
  const { event_digest: _eventDigest, ...body } = event as AutonomousGoalWorkerEvent;
  return body;
}

function verifyEvent(raw: unknown): AutonomousGoalWorkerEvent {
  if (!isObject(raw)) fail("event must be an object");
  const allowed = new Set(["schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision", "schedule_digest", "claim_digest", "outcome_digest", "error_digest", "task_digest", "execution_binding_digest", "resolution_goal_status", "created_ns", "previous_digest", "event_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key)) || raw.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA || raw.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || raw.secret_material !== "never_returned") fail("event contains unsupported or unsafe metadata");
  const required = ["schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision", "schedule_digest", "claim_digest", "outcome_digest", "error_digest", "created_ns", "previous_digest", "event_digest", "retention", "secret_material"];
  if (required.some((key) => !Object.prototype.hasOwnProperty.call(raw, key))) fail("event is missing required fields");
  if (typeof raw.phase !== "string" || !ALL_PHASES.has(raw.phase as AutonomousGoalWorkerJournalPhase)) fail("event phase is invalid");
  if (typeof raw.previous_digest !== "string") fail("event.previous_digest must be a string");
  const taskDigest = raw.task_digest === undefined ? undefined : digest("event.task_digest", raw.task_digest)!;
  const executionBindingDigest = raw.execution_binding_digest === undefined ? undefined : digest("event.execution_binding_digest", raw.execution_binding_digest)!;
  const resolutionGoalStatus = raw.resolution_goal_status === undefined ? undefined : raw.resolution_goal_status;
  if (resolutionGoalStatus !== undefined && (!(["completed", "failed", "ready", "blocked", "paused"] as unknown[]).includes(resolutionGoalStatus) || (raw.phase !== "reconciled" && raw.phase !== "settled") || raw.outcome_digest === null || raw.outcome_digest === undefined)) fail("event resolution goal status is inconsistent with its phase");
  const event = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
    sequence: integer("event.sequence", raw.sequence, 1, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS),
    batch_id: identifier("event.batch_id", raw.batch_id),
    goal_id: identifier("event.goal_id", raw.goal_id),
    phase: raw.phase as AutonomousGoalWorkerJournalPhase,
    attempt: integer("event.attempt", raw.attempt, 0, 128),
    revision: integer("event.revision", raw.revision),
    schedule_digest: digest("event.schedule_digest", raw.schedule_digest)!,
    claim_digest: digest("event.claim_digest", raw.claim_digest ?? null, true),
    outcome_digest: digest("event.outcome_digest", raw.outcome_digest ?? null, true),
    error_digest: digest("event.error_digest", raw.error_digest ?? null, true),
    ...(taskDigest === undefined ? {} : { task_digest: taskDigest }),
    ...(executionBindingDigest === undefined ? {} : { execution_binding_digest: executionBindingDigest }),
    ...(resolutionGoalStatus === undefined ? {} : { resolution_goal_status: resolutionGoalStatus as DispatchResolutionGoalStatus }),
    created_ns: requireAutonomousGoalTimestampNsWire(raw.created_ns, "event.created_ns"),
    previous_digest: raw.previous_digest,
    retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
    secret_material: "never_returned" as const,
    event_digest: digest("event.event_digest", raw.event_digest)!,
  } satisfies AutonomousGoalWorkerEvent;
  if (event.sequence === 1 && event.previous_digest !== "") fail("first event must have an empty previous digest");
  if (event.sequence > 1 && !/^[0-9a-f]{64}$/.test(event.previous_digest)) fail("event.previous_digest is malformed");
  if (digestJsonSync(eventBody(event)) !== event.event_digest) fail(`event ${event.sequence} digest does not match its content`);
  return clone(event);
}

export interface AutonomousGoalWorkerJournalTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface TransactionalAutonomousGoalWorkerJournalTextStore extends AutonomousGoalWorkerJournalTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface AutonomousGoalWorkerJournalMonotonicAnchor extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_MONOTONIC_ANCHOR_SCHEMA;
  sequence: number;
  snapshot_digest: string;
  head_digest: string;
  anchor_digest: string;
}

/** Deployment-owned anchor storage must survive rollback of the journal snapshot store. */
export interface TransactionalAutonomousGoalWorkerJournalMonotonicAnchorStore {
  read(): unknown | Promise<unknown>;
  writeIfUnchanged(expectedAnchorDigest: string | null, value: Readonly<AutonomousGoalWorkerJournalMonotonicAnchor>): boolean | Promise<boolean>;
}

export class AutonomousGoalWorkerJournal {
  private readonly maxEvents: number;
  private readonly clock: () => AutonomousGoalTimestampInput;
  private eventsValue: AutonomousGoalWorkerEvent[] = [];
  private latestByGoal = new Map<string, AutonomousGoalWorkerEvent>();
  private futureEventReserveCount = 0;
  private workerRunActive = false;
  private migration: AutonomousGoalWorkerJournalMigration | undefined;

  constructor(options: { maxEvents?: number; clock?: () => AutonomousGoalTimestampInput } = {}) {
    this.maxEvents = integer("journal maxEvents", options.maxEvents ?? AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS, 1, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS);
    this.clock = options.clock ?? (() => autonomousGoalTimestampNowNs());
  }

  get head_digest(): string {
    return this.eventsValue.at(-1)?.event_digest ?? "";
  }

  /** Check new work while preserving capacity for every active journal boundary. */
  requireCapacity(additionalEvents: number): void {
    const additional = integer("additional event capacity", additionalEvents, 0, this.maxEvents);
    if (this.eventsValue.length + this.futureEventReserveCount + additional > this.maxEvents) fail("journal event capacity cannot preserve active recovery and requested transitions");
  }

  /** Admit one local worker against this journal's shared event budget. */
  beginWorkerRun(): () => void {
    if (this.workerRunActive) fail("another worker run is already active for this journal");
    this.workerRunActive = true;
    let released = false;
    return () => {
      if (!released) {
        released = true;
        this.workerRunActive = false;
      }
    };
  }

  record(input: { batch_id: string; goal_id: string; phase: AutonomousGoalWorkerJournalPhase; attempt: number; revision: number; schedule_digest: string; claim_digest?: string | null; outcome_digest?: string | null; error_digest?: string | null; task_digest?: string | null; execution_binding_digest?: string | null; resolution_goal_status?: DispatchResolutionGoalStatus; created_ns?: AutonomousGoalTimestampInput }): AutonomousGoalWorkerEvent {
    if (this.eventsValue.length >= this.maxEvents) fail("event capacity is exhausted");
    if (!ALL_PHASES.has(input.phase)) fail("event phase is invalid");
    const taskDigest = input.task_digest === undefined || input.task_digest === null ? undefined : digest("task_digest", input.task_digest)!;
    const executionBindingDigest = input.execution_binding_digest === undefined || input.execution_binding_digest === null ? undefined : digest("execution_binding_digest", input.execution_binding_digest)!;
    if (input.resolution_goal_status !== undefined && (!(["completed", "failed", "ready", "blocked", "paused"] as unknown[]).includes(input.resolution_goal_status) || (input.phase !== "reconciled" && input.phase !== "settled") || input.outcome_digest === undefined || input.outcome_digest === null)) fail("resolution goal status is inconsistent with its phase");
    const body = {
      schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
      sequence: this.eventsValue.length + 1,
      batch_id: identifier("batch_id", input.batch_id),
      goal_id: identifier("goal_id", input.goal_id),
      phase: input.phase,
      attempt: integer("attempt", input.attempt, 0, 128),
      revision: integer("revision", input.revision),
      schedule_digest: digest("schedule_digest", input.schedule_digest)!,
      claim_digest: digest("claim_digest", input.claim_digest ?? null, true),
      outcome_digest: digest("outcome_digest", input.outcome_digest ?? null, true),
      error_digest: digest("error_digest", input.error_digest ?? null, true),
      ...(taskDigest === undefined ? {} : { task_digest: taskDigest }),
      ...(executionBindingDigest === undefined ? {} : { execution_binding_digest: executionBindingDigest }),
      ...(input.resolution_goal_status === undefined ? {} : { resolution_goal_status: input.resolution_goal_status }),
      created_ns: normalizeAutonomousGoalTimestampNs(input.created_ns ?? this.clock(), "created_ns"),
      previous_digest: this.head_digest,
      retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
      secret_material: "never_returned" as const,
    } satisfies Omit<AutonomousGoalWorkerEvent, "event_digest">;
    const event = { ...body, event_digest: digestJsonSync(body) } satisfies AutonomousGoalWorkerEvent;
    const prior = this.latestByGoal.get(event.goal_id);
    if (input.phase === "prepared" && prior !== undefined && (ACTIVE_PHASES.has(prior.phase) || needsExternalReconciliation(prior))) fail(`goal ${event.goal_id} already has an active journal boundary`);
    const nextReserve = this.futureEventReserveCount - (prior === undefined ? 0 : futureEventReserve(prior)) + futureEventReserve(event);
    if (this.eventsValue.length + 1 + nextReserve > this.maxEvents) fail("event capacity cannot preserve active recovery transitions");
    this.eventsValue.push(event);
    this.latestByGoal.set(event.goal_id, event);
    this.futureEventReserveCount = nextReserve;
    return clone(event);
  }

  events(query: { batch_id?: string; goal_id?: string } = {}): AutonomousGoalWorkerEvent[] {
    const batch = query.batch_id === undefined ? undefined : identifier("batch_id", query.batch_id);
    const goal = query.goal_id === undefined ? undefined : identifier("goal_id", query.goal_id);
    return this.eventsValue.filter((event) => (batch === undefined || event.batch_id === batch) && (goal === undefined || event.goal_id === goal)).map(clone);
  }

  active(): AutonomousGoalWorkerEvent[] {
    return [...this.latestByGoal.values()].filter((event) => ACTIVE_PHASES.has(event.phase) || needsExternalReconciliation(event)).sort((left, right) => left.sequence - right.sequence).map(clone);
  }

  /** Return the current unreconciled boundary event for one goal, if any. */
  activeFor(goalId: string): AutonomousGoalWorkerEvent | undefined {
    const normalizedGoalId = identifier("goal_id", goalId);
    const event = this.active().find((candidate) => candidate.goal_id === normalizedGoalId);
    return event === undefined ? undefined : clone(event);
  }

  replaySettledExternalOutcomes(ledger: InMemoryAutonomousGoalLedger): number {
    const latest = new Map<string, AutonomousGoalWorkerEvent>();
    for (const event of this.eventsValue) latest.set(event.goal_id, event);
    let replayed = 0;
    for (const event of latest.values()) {
      if (event.phase !== "settled" || event.resolution_goal_status === undefined) continue;
      const current = ledger.get(event.goal_id);
      if (current === null) fail(`settled dispatch resolution for goal ${event.goal_id} has no ledger record`);
      if (current.attempt > event.attempt || current.revision > event.revision) continue;
      if (current.attempt === event.attempt && current.revision === event.revision && current.status === event.resolution_goal_status && current.outcome_digest === event.outcome_digest) continue;
      const history = this.events({ goal_id: event.goal_id });
      const eventIndex = history.findIndex((candidate) => candidate.event_digest === event.event_digest);
      let previousIndex = eventIndex - 1;
      const staged = previousIndex > 0 ? history[previousIndex]! : undefined;
      if (staged?.phase === "reconciled" && staged.outcome_digest === event.outcome_digest && staged.resolution_goal_status === event.resolution_goal_status) previousIndex -= 1;
      const previousDigest = previousIndex >= 0 ? history[previousIndex]!.outcome_digest : null;
      if (current.attempt !== event.attempt || current.revision !== event.revision - 1 || current.status !== "blocked" || current.outcome_digest !== previousDigest || !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) fail(`settled dispatch resolution for goal ${event.goal_id} conflicts with the restored ledger`);
      const targetStatus = event.resolution_goal_status;
      const nextAction = targetStatus === "failed" || targetStatus === "paused" ? goalTaskDigest("goal-reconciliation-review") : targetStatus === "ready" ? goalTaskDigest("goal-retry") : null;
      ledger.transition(event.goal_id, targetStatus, {
        expected_revision: current.revision, blockers: targetStatus === "paused" ? ["required_goal_criteria_review"] : [], next_action_digest: nextAction,
        outcome_digest: event.outcome_digest, now_ns: event.created_ns,
      });
      replayed += 1;
    }
    return replayed;
  }

  /** Refuse a new worker pass until a prior in-flight boundary has been reconciled. */
  assertNoActive(goalId: string): void {
    const event = this.activeFor(goalId);
    if (event !== undefined) fail(`goal ${goalId} has an unreconciled ${event.phase} event; recover or reconcile the worker journal before retrying`);
  }

  /** Validate a status receipt and produce an in-memory, revision-fenced commit plan. */
  prepareExternalOutcome(
    ledger: InMemoryAutonomousGoalLedger,
    raw: unknown,
    verifier: AutonomousGoalDispatchResolutionVerifier,
  ): PreparedAutonomousGoalDispatchOutcome {
    if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("external outcome reconciliation requires an InMemoryAutonomousGoalLedger");
    const resolution = authenticateDispatchResolution(raw, verifier);
    const resolutionDigest = digestJsonSync(resolution);
    const history = this.events({ goal_id: resolution.goal_id });
    const dispatchIndex = history.findIndex((event) => event.event_digest === resolution.dispatch_event_digest);
    if (dispatchIndex < 0) fail("dispatch resolution references an unknown journal event");
    const dispatch = history[dispatchIndex]!;
    if (dispatch.phase !== "dispatch_started" || dispatch.attempt !== resolution.attempt) fail("dispatch resolution does not match a dispatched attempt");
    if (dispatch.execution_binding_digest === undefined || dispatch.execution_binding_digest !== resolution.execution_binding_digest) fail("dispatch resolution does not match the execution binding");
    const recoveryDigest = restartOutcomeDigest(dispatch.goal_id, dispatch.attempt, false);
    const afterDispatch = history.slice(dispatchIndex + 1);
    if (!afterDispatch.some((event) => event.phase === "reconciled" && event.attempt === dispatch.attempt && event.batch_id === dispatch.batch_id && event.outcome_digest === recoveryDigest)) fail("dispatch resolution requires completed journal recovery first");
    if (afterDispatch.some((event) => (event.phase === "prepared" || event.phase === "claimed" || event.phase === "dispatch_started") && event.attempt !== dispatch.attempt)) fail("dispatch resolution is stale after a later attempt");

    const current = ledger.get(resolution.goal_id);
    if (current === null || current.attempt !== resolution.attempt) fail("dispatch resolution does not match the current goal attempt");
    const targetStatus: DispatchResolutionGoalStatus = resolution.status === "completed"
      ? (current.criteria.every((criterion) => !criterion.required || criterion.status === "satisfied" || criterion.status === "waived") ? "completed" : "paused")
      : resolution.status === "failed"
        ? "failed"
        : resolution.status === "not_applied"
          ? (current.attempt < current.max_attempts ? "ready" : "failed")
          : "blocked";
    const latest = history.at(-1);
    if (latest !== undefined && (latest.phase === "reconciled" || latest.phase === "settled") && latest.attempt === resolution.attempt && latest.execution_binding_digest === resolution.execution_binding_digest && latest.outcome_digest === resolutionDigest && latest.resolution_goal_status === targetStatus) {
      if (current.status === targetStatus && current.revision === latest.revision && current.outcome_digest === resolutionDigest) return { resolution, dispatch, target_status: targetStatus, resolution_digest: resolutionDigest, expected_revision: current.revision, expected_outcome_digest: current.outcome_digest, expected_head_digest: this.head_digest, idempotent: true, goal_revision: current.revision };
      const previous = history.at(-2);
      if (latest.phase === "reconciled" && current.status === "blocked" && current.revision + 1 === latest.revision && current.outcome_digest === previous?.outcome_digest) return { resolution, dispatch, target_status: targetStatus, resolution_digest: resolutionDigest, expected_revision: current.revision, expected_outcome_digest: current.outcome_digest, expected_head_digest: this.head_digest, idempotent: false, already_staged: true };
      fail("idempotent dispatch resolution conflicts with the current goal state");
    }
    if (current.status !== "blocked" || !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) fail("goal is not awaiting post-dispatch outcome reconciliation");
    if (latest === undefined || latest.revision !== current.revision || latest.attempt !== current.attempt) fail("goal revision has drifted from its recovered dispatch journal");
    if (compareAutonomousGoalTimestampNs(resolution.observed_ns, current.updated_ns) <= 0) fail("dispatch status evidence is stale relative to the recovered goal state");
    return { resolution, dispatch, target_status: targetStatus, resolution_digest: resolutionDigest, expected_revision: current.revision, expected_outcome_digest: current.outcome_digest, expected_head_digest: this.head_digest, idempotent: false };
  }

  stageExternalOutcome(prepared: PreparedAutonomousGoalDispatchOutcome): AutonomousGoalWorkerEvent {
    if (prepared.idempotent) fail("idempotent external outcome does not need staging");
    if (prepared.already_staged) return this.events({ goal_id: prepared.resolution.goal_id }).at(-1)!;
    if (this.head_digest !== prepared.expected_head_digest) fail("worker journal changed while dispatch resolution was being prepared");
    return this.record({
      batch_id: prepared.dispatch.batch_id, goal_id: prepared.dispatch.goal_id, phase: "reconciled",
      attempt: prepared.dispatch.attempt, revision: prepared.expected_revision + 1,
      schedule_digest: prepared.dispatch.schedule_digest, claim_digest: prepared.dispatch.claim_digest,
      outcome_digest: prepared.resolution_digest, task_digest: prepared.dispatch.task_digest,
      execution_binding_digest: prepared.dispatch.execution_binding_digest,
      resolution_goal_status: prepared.target_status, created_ns: prepared.resolution.observed_ns,
    });
  }

  commitExternalOutcome(ledger: InMemoryAutonomousGoalLedger, prepared: PreparedAutonomousGoalDispatchOutcome): AutonomousGoalDispatchResolutionResult {
    const { resolution, dispatch, target_status: targetStatus, resolution_digest: resolutionDigest } = prepared;
    if (prepared.idempotent) return this.dispatchResolutionResult(resolution, targetStatus, prepared.goal_revision!, true);
    const latest = this.events({ goal_id: resolution.goal_id }).at(-1)!;
    if (latest.phase !== "reconciled" || latest.resolution_goal_status !== targetStatus || latest.outcome_digest !== resolutionDigest) fail("dispatch resolution was not durably staged before commit");
    const current = ledger.get(resolution.goal_id);
    if (current === null || current.attempt !== resolution.attempt) fail("dispatch resolution does not match the current goal attempt");
    let updated = current;
    if (current.revision === prepared.expected_revision) {
      if (current.status !== "blocked" || current.outcome_digest !== prepared.expected_outcome_digest || !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation") || compareAutonomousGoalTimestampNs(resolution.observed_ns, current.updated_ns) <= 0) fail("goal changed after dispatch resolution was staged");
      const nextAction = targetStatus === "blocked" || targetStatus === "failed" || targetStatus === "paused"
        ? goalTaskDigest("goal-reconciliation-review")
        : targetStatus === "ready" ? goalTaskDigest("goal-retry") : null;
      updated = ledger.transition(resolution.goal_id, targetStatus, {
        expected_revision: current.revision, blockers: targetStatus === "blocked" ? current.blockers : targetStatus === "paused" ? ["required_goal_criteria_review"] : [],
        next_action_digest: nextAction, outcome_digest: resolutionDigest, now_ns: resolution.observed_ns,
      });
    } else if (current.revision !== latest.revision || current.status !== targetStatus || current.outcome_digest !== resolutionDigest) {
      fail("goal revision has drifted from its staged dispatch resolution");
    }
    if (targetStatus !== "blocked" && latest.phase === "reconciled") this.record({
      batch_id: dispatch.batch_id, goal_id: dispatch.goal_id, phase: "settled", attempt: dispatch.attempt,
      revision: updated.revision, schedule_digest: dispatch.schedule_digest, claim_digest: dispatch.claim_digest,
      outcome_digest: resolutionDigest, task_digest: dispatch.task_digest,
      execution_binding_digest: dispatch.execution_binding_digest, resolution_goal_status: targetStatus,
      created_ns: resolution.observed_ns,
    });
    return this.dispatchResolutionResult(resolution, updated.status, updated.revision, false);
  }

  reconcileExternalOutcome(
    ledger: InMemoryAutonomousGoalLedger,
    raw: unknown,
    verifier: AutonomousGoalDispatchResolutionVerifier,
  ): AutonomousGoalDispatchResolutionResult {
    const prepared = this.prepareExternalOutcome(ledger, raw, verifier);
    if (prepared.idempotent) return this.dispatchResolutionResult(prepared.resolution, prepared.target_status, prepared.goal_revision!, true);
    this.stageExternalOutcome(prepared);
    return this.commitExternalOutcome(ledger, prepared);
  }

  private dispatchResolutionResult(resolution: AutonomousGoalDispatchResolution, goalStatus: AutonomousGoalStatus, goalRevision: number, idempotent: boolean): AutonomousGoalDispatchResolutionResult {
    return {
      schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA, goal_id: resolution.goal_id,
      attempt: resolution.attempt, dispatch_event_digest: resolution.dispatch_event_digest,
      resolution_digest: digestJsonSync(resolution), status: resolution.status, goal_status: goalStatus,
      goal_revision: goalRevision, idempotent, retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
      secret_material: "never_returned",
    };
  }

  snapshot(): AutonomousGoalWorkerJournalSnapshot {
    const body = {
      schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
      sequence: this.eventsValue.length,
      head_digest: this.head_digest,
      events: this.eventsValue.map(clone),
      retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
      secret_material: "never_returned" as const,
      ...(this.migration === undefined ? {} : { migration: clone(this.migration) }),
    } satisfies Omit<AutonomousGoalWorkerJournalSnapshot, "snapshot_digest">;
    if (new TextEncoder().encode(canonicalJson(body)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    return { ...body, snapshot_digest: digestJsonSync(body) };
  }

  static validateSnapshot(raw: unknown): AutonomousGoalWorkerJournalSnapshot {
    if (!isObject(raw) || raw.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA || !Array.isArray(raw.events)) fail("snapshot schema is invalid");
    const allowed = new Set(["schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material", "migration"]);
    if (Object.keys(raw).some((key) => !allowed.has(key)) || raw.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || raw.secret_material !== "never_returned") fail("snapshot contains unsupported or unsafe metadata");
    if (raw.events.length > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS || integer("snapshot.sequence", raw.sequence, 0, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS) !== raw.events.length) fail("snapshot sequence or capacity is invalid");
    const head = raw.sequence === 0 ? "" : digest("snapshot.head_digest", raw.head_digest)!;
    if (raw.sequence === 0 && raw.head_digest !== "") fail("empty snapshot must have an empty head digest");
    const events = raw.events.map(verifyEvent);
    let previous = "";
    const latestByGoal = new Map<string, AutonomousGoalWorkerEvent>();
    events.forEach((event, index) => {
      if (event.sequence !== index + 1 || event.previous_digest !== previous) fail(`snapshot event chain breaks at sequence ${index + 1}`);
      const prior = latestByGoal.get(event.goal_id);
      if (event.phase === "prepared" && prior !== undefined && (ACTIVE_PHASES.has(prior.phase) || needsExternalReconciliation(prior))) fail(`snapshot preparation for goal ${event.goal_id} overwrites an active journal boundary`);
      latestByGoal.set(event.goal_id, event);
      previous = event.event_digest;
    });
    if (previous !== head) fail("snapshot head digest does not match its event chain");
    const migration = raw.migration === undefined ? undefined : normalizeJournalMigration(raw.migration);
    const body = { schema: raw.schema, sequence: raw.sequence, head_digest: raw.head_digest, events, retention: raw.retention, secret_material: raw.secret_material, ...(migration === undefined ? {} : { migration }) };
    if (digest("snapshot.snapshot_digest", raw.snapshot_digest) !== digestJsonSync(body)) fail("snapshot digest does not match its content");
    if (new TextEncoder().encode(canonicalJson(raw)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    return clone({ ...body, snapshot_digest: raw.snapshot_digest } as AutonomousGoalWorkerJournalSnapshot);
  }

  restore(raw: unknown): AutonomousGoalWorkerJournalSnapshot {
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(raw);
    const events = snapshot.events.map(clone);
    const latest = new Map<string, AutonomousGoalWorkerEvent>();
    for (const event of events) latest.set(event.goal_id, event);
    const reserve = [...latest.values()].reduce((sum, event) => sum + futureEventReserve(event), 0);
    if (events.length + reserve > this.maxEvents) fail("snapshot capacity cannot preserve active recovery transitions");
    this.eventsValue = events;
    this.latestByGoal = latest;
    this.futureEventReserveCount = reserve;
    this.migration = snapshot.migration;
    return clone(snapshot);
  }

  recover(ledger: InMemoryAutonomousGoalLedger, options: { now_ns?: AutonomousGoalTimestampInput; goal_id?: string } = {}): JsonObject {
    if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("recover requires an InMemoryAutonomousGoalLedger");
    const nowNs = options.now_ns === undefined ? undefined : normalizeAutonomousGoalTimestampNs(options.now_ns, "recover.now_ns");
    const goalId = options.goal_id === undefined ? undefined : identifier("recover.goal_id", options.goal_id);
    const recovered: JsonObject[] = [];
    for (const event of this.active()) {
      if (goalId !== undefined && event.goal_id !== goalId) continue;
      const current = ledger.get(event.goal_id);
      if (event.phase === "prepared") {
        if (current === null) fail(`prepared event for goal ${event.goal_id} has no ledger record`);
        let recoveryAttempt: number;
        let updated: AutonomousGoalRecord;
        if (current.attempt === event.attempt && current.revision === event.revision && (current.status === "ready" || current.status === "paused" || current.status === "failed")) {
          recoveryAttempt = event.attempt;
          updated = current;
        } else if (current.status === "running" && current.attempt === event.attempt + 1 && [1, 2].includes(current.revision - event.revision)) {
          recoveryAttempt = current.attempt;
          const outcomeDigest = restartOutcomeDigest(event.goal_id, recoveryAttempt, true);
          updated = ledger.transition(event.goal_id, "paused", {
            expected_revision: current.revision,
            blockers: ["worker_restart_before_dispatch"],
            next_action_digest: goalTaskDigest("goal-retry"),
            outcome_digest: outcomeDigest,
            now_ns: nowNs,
          });
        } else if (current.status === "paused" && current.attempt === event.attempt + 1 && [2, 3].includes(current.revision - event.revision) && current.outcome_digest === restartOutcomeDigest(event.goal_id, current.attempt, true) && current.blockers.includes("worker_restart_before_dispatch")) {
          recoveryAttempt = current.attempt;
          updated = current;
        } else {
          fail(`prepared event for goal ${event.goal_id} no longer matches the ledger`);
        }
        const outcomeDigest = restartOutcomeDigest(event.goal_id, recoveryAttempt, true);
        this.record({ batch_id: event.batch_id, goal_id: event.goal_id, phase: "reconciled", attempt: recoveryAttempt, revision: updated.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest, outcome_digest: outcomeDigest, task_digest: event.task_digest, execution_binding_digest: event.execution_binding_digest });
        recovered.push({ goal_id: event.goal_id, from_phase: "prepared", goal_status: updated.status, outcome_digest: outcomeDigest });
        continue;
      }
      if (event.resolution_goal_status !== undefined) {
        if (current === null || current.attempt !== event.attempt) fail(`staged dispatch resolution for goal ${event.goal_id} no longer matches the ledger`);
        const targetStatus = event.resolution_goal_status;
        let updated = current;
        if (current.revision === event.revision - 1) {
          const history = this.events({ goal_id: event.goal_id });
          const eventIndex = history.findIndex((candidate) => candidate.event_digest === event.event_digest);
          const priorDigest = eventIndex > 0 ? history[eventIndex - 1]!.outcome_digest : null;
          if (current.status !== "blocked" || current.outcome_digest !== priorDigest || !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) fail(`staged dispatch resolution for goal ${event.goal_id} cannot be applied to the current ledger state`);
          const nextAction = targetStatus === "blocked" || targetStatus === "failed" || targetStatus === "paused"
            ? goalTaskDigest("goal-reconciliation-review")
            : targetStatus === "ready" ? goalTaskDigest("goal-retry") : null;
          updated = ledger.transition(event.goal_id, targetStatus, {
            expected_revision: current.revision, blockers: targetStatus === "blocked" ? current.blockers : targetStatus === "paused" ? ["required_goal_criteria_review"] : [],
            next_action_digest: nextAction, outcome_digest: event.outcome_digest, now_ns: event.created_ns,
          });
        } else if (current.revision !== event.revision || current.status !== targetStatus || current.outcome_digest !== event.outcome_digest) {
          fail(`staged dispatch resolution for goal ${event.goal_id} conflicts with the current ledger state`);
        }
        if (targetStatus !== "blocked" && event.phase === "reconciled") this.record({
          batch_id: event.batch_id, goal_id: event.goal_id, phase: "settled", attempt: event.attempt,
          revision: updated.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest,
          outcome_digest: event.outcome_digest, task_digest: event.task_digest,
          execution_binding_digest: event.execution_binding_digest, resolution_goal_status: targetStatus,
          created_ns: event.created_ns,
        });
        recovered.push({ goal_id: event.goal_id, from_phase: "dispatch_started", goal_status: updated.status, outcome_digest: event.outcome_digest });
        continue;
      }
      if (needsExternalReconciliation(event)) {
        if (current === null || current.status !== "blocked" || current.attempt !== event.attempt || current.revision !== event.revision || !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) fail(`pending external resolution for goal ${event.goal_id} no longer matches the ledger`);
        recovered.push({ goal_id: event.goal_id, from_phase: "dispatch_started", goal_status: current.status, outcome_digest: event.outcome_digest });
        continue;
      }
      if (event.phase === "dispatch_started" && current !== null && current.attempt === event.attempt && current.revision === event.revision + 1 && (current.status === "completed" || current.status === "failed" || current.status === "paused" || current.status === "blocked") && current.outcome_digest !== null && !current.blockers.includes("worker_restart_after_dispatch_requires_reconciliation")) {
        // The ledger transition is atomic and precedes the journal's terminal event. Seal
        // that committed result instead of asking a status authority to verify it again.
        this.record({ batch_id: event.batch_id, goal_id: event.goal_id, phase: "settled", attempt: event.attempt, revision: current.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest, outcome_digest: current.outcome_digest, task_digest: event.task_digest, execution_binding_digest: event.execution_binding_digest });
        recovered.push({ goal_id: event.goal_id, from_phase: "dispatch_started", goal_status: current.status, outcome_digest: current.outcome_digest });
        continue;
      }
      const beforeDispatch = event.phase === "claimed";
      const target: AutonomousGoalStatus = beforeDispatch ? "paused" : "blocked";
      const blocker = beforeDispatch ? "worker_restart_before_dispatch" : "worker_restart_after_dispatch_requires_reconciliation";
      const nextAction = beforeDispatch ? "goal-retry" : "goal-reconciliation-review";
      const outcomeDigest = restartOutcomeDigest(event.goal_id, event.attempt, beforeDispatch);
      if (current !== null && current.attempt === event.attempt && current.revision === event.revision + 1 && current.status === target && current.outcome_digest === outcomeDigest && current.blockers.includes(blocker)) {
        this.record({ batch_id: event.batch_id, goal_id: event.goal_id, phase: "reconciled", attempt: event.attempt, revision: current.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest, outcome_digest: outcomeDigest, task_digest: event.task_digest, execution_binding_digest: event.execution_binding_digest });
        recovered.push({ goal_id: event.goal_id, from_phase: beforeDispatch ? "claimed" : "dispatch_started", goal_status: current.status, outcome_digest: outcomeDigest });
        continue;
      }
      if (!current || current.status !== "running" || current.revision !== event.revision || current.attempt !== event.attempt) fail(`active event for goal ${event.goal_id} no longer matches the ledger`);
      const updated = ledger.transition(event.goal_id, target, { expected_revision: current.revision, blockers: [blocker], next_action_digest: goalTaskDigest(nextAction), outcome_digest: outcomeDigest, now_ns: nowNs });
      this.record({ batch_id: event.batch_id, goal_id: event.goal_id, phase: "reconciled", attempt: event.attempt, revision: updated.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest, outcome_digest: outcomeDigest, task_digest: event.task_digest, execution_binding_digest: event.execution_binding_digest });
      recovered.push({ goal_id: event.goal_id, from_phase: beforeDispatch ? "claimed" : "dispatch_started", goal_status: updated.status, outcome_digest: outcomeDigest });
    }
    return { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SCHEMA, recovered, recovery_digest: digestJsonSync(recovered), retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION, secret_material: "never_returned" };
  }
}

/** Verify a v0.1 worker journal before converting its numeric timestamps to exact v0.2 nanoseconds. */
export function migrateLegacyAutonomousGoalWorkerJournalSnapshot(
  raw: unknown,
  sourceUnit: AutonomousGoalLegacyTimestampUnit,
): AutonomousGoalWorkerJournalSnapshot {
  if (sourceUnit !== "milliseconds" && sourceUnit !== "nanoseconds") fail("legacy snapshot migration requires milliseconds or nanoseconds as the source unit");
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01 || !Array.isArray(raw.events)) fail("legacy snapshot schema is unsupported");
  const snapshotKeys = ["schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"];
  if (Object.keys(raw).some((key) => !snapshotKeys.includes(key)) || snapshotKeys.some((key) => !(key in raw)) || raw.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || raw.secret_material !== "never_returned") fail("legacy snapshot contains unsupported or unsafe metadata");
  if (raw.events.length > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS || integer("legacy snapshot.sequence", raw.sequence, 0, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS) !== raw.events.length) fail("legacy snapshot sequence or capacity is invalid");
  if (new TextEncoder().encode(canonicalJson(raw)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("legacy snapshot exceeds its byte bound");
  const legacyBody = Object.fromEntries(Object.entries(raw).filter(([key]) => key !== "snapshot_digest"));
  const sourceSnapshotDigest = digest("legacy snapshot.snapshot_digest", raw.snapshot_digest)!;
  if (digestJsonSync(legacyBody) !== sourceSnapshotDigest) fail("legacy snapshot digest does not match its content");
  const sourceHead = raw.sequence === 0 ? "" : digest("legacy snapshot.head_digest", raw.head_digest)!;
  if (raw.sequence === 0 && raw.head_digest !== "") fail("empty legacy snapshot must have an empty head digest");

  const requiredEventKeys = ["schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision", "schedule_digest", "claim_digest", "outcome_digest", "error_digest", "created_ns", "previous_digest", "event_digest", "retention", "secret_material"];
  const optionalEventKeys = ["task_digest", "execution_binding_digest", "resolution_goal_status"];
  const migratedEvents: AutonomousGoalWorkerEvent[] = [];
  let sourcePrevious = "";
  let migratedPrevious = "";
  for (let index = 0; index < raw.events.length; index += 1) {
    const legacyEvent = raw.events[index];
    if (!isObject(legacyEvent) || Object.keys(legacyEvent).some((key) => !requiredEventKeys.includes(key) && !optionalEventKeys.includes(key)) || requiredEventKeys.some((key) => !(key in legacyEvent))) fail(`legacy event ${index + 1} is malformed`);
    if (legacyEvent.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01 || legacyEvent.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || legacyEvent.secret_material !== "never_returned") fail(`legacy event ${index + 1} markers are invalid`);
    if (integer(`legacy event ${index + 1}.sequence`, legacyEvent.sequence, 1, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS) !== index + 1 || legacyEvent.previous_digest !== sourcePrevious) fail(`legacy event chain breaks at sequence ${index + 1}`);
    const legacyEventBody = Object.fromEntries(Object.entries(legacyEvent).filter(([key]) => key !== "event_digest"));
    const sourceEventDigest = digest(`legacy event ${index + 1}.event_digest`, legacyEvent.event_digest)!;
    if (digestJsonSync(legacyEventBody) !== sourceEventDigest) fail(`legacy event ${index + 1} digest does not match its content`);
    const migratedBody = {
      ...legacyEventBody,
      schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
      created_ns: migrateLegacyAutonomousGoalTimestampNs(legacyEvent.created_ns, sourceUnit, `legacy event ${index + 1}.created_ns`),
      previous_digest: migratedPrevious,
    };
    const event = verifyEvent({ ...migratedBody, event_digest: digestJsonSync(migratedBody) });
    migratedEvents.push(event);
    sourcePrevious = sourceEventDigest;
    migratedPrevious = event.event_digest;
  }
  if (sourcePrevious !== sourceHead) fail("legacy snapshot head digest does not match its event chain");
  const migration: AutonomousGoalWorkerJournalMigration = {
    source_schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    source_timestamp_unit: sourceUnit,
    source_snapshot_digest: sourceSnapshotDigest,
    source_head_digest: sourceHead,
  };
  const migratedBody = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
    sequence: migratedEvents.length,
    head_digest: migratedPrevious,
    events: migratedEvents,
    retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
    secret_material: "never_returned" as const,
    migration,
  };
  return AutonomousGoalWorkerJournal.validateSnapshot({ ...migratedBody, snapshot_digest: digestJsonSync(migratedBody) });
}

/** Authenticate a legacy shared-store envelope, migrate its verified snapshot, and reseal it with the current key. */
export function migrateLegacyAuthenticatedAutonomousGoalWorkerJournalEnvelope(
  raw: unknown,
  options: {
    source_timestamp_unit: AutonomousGoalLegacyTimestampUnit;
    keys: ReadonlyMap<string, Uint8Array>;
    active_key_id: string;
  },
): AutonomousGoalWorkerJournalSnapshot & { authentication: { schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA; key_id: string; tag: string } } {
  if (!isObject(raw) || !isObject(raw.authentication) || !(options.keys instanceof Map)) fail("legacy authenticated journal envelope is malformed");
  if (new TextEncoder().encode(canonicalJson(raw)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("legacy authenticated journal envelope exceeds its byte bound");
  const authentication = raw.authentication;
  if (Object.keys(authentication).sort().join(",") !== "key_id,schema,tag" || authentication.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01) fail("legacy authenticated journal envelope markers are invalid");
  const sourceKeyId = identifier("legacy authentication key id", authentication.key_id);
  const sourceTag = digest("legacy authentication tag", authentication.tag)!;
  const sourceKey = options.keys.get(sourceKeyId);
  if (!(sourceKey instanceof Uint8Array) || sourceKey.byteLength < 32 || sourceKey.byteLength > 4_096) fail("legacy authenticated journal key id is not trusted by this keyring");
  const sourceSnapshot = Object.fromEntries(Object.entries(raw).filter(([key]) => key !== "authentication"));
  const sourceBody = { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01, key_id: sourceKeyId, snapshot: sourceSnapshot };
  const expectedSourceTag = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(sourceBody)), sourceKey);
  let mismatch = 0;
  for (let index = 0; index < expectedSourceTag.length; index += 1) mismatch |= expectedSourceTag.charCodeAt(index) ^ sourceTag.charCodeAt(index);
  if (mismatch !== 0) fail("legacy authenticated journal tag does not match its snapshot");

  const activeKeyId = identifier("active authentication key id", options.active_key_id);
  const activeKey = options.keys.get(activeKeyId);
  if (!(activeKey instanceof Uint8Array) || activeKey.byteLength < 32 || activeKey.byteLength > 4_096) fail("active authentication key id is not present in the keyring");
  const snapshot = migrateLegacyAutonomousGoalWorkerJournalSnapshot(sourceSnapshot, options.source_timestamp_unit);
  const body = { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: activeKeyId, snapshot };
  const tag = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(body)), activeKey);
  const envelope = { ...snapshot, authentication: { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: activeKeyId, tag } };
  if (new TextEncoder().encode(canonicalJson(envelope)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("migrated authenticated journal envelope exceeds its byte bound");
  return envelope;
}

export class JsonAutonomousGoalWorkerJournalPersistence {
  constructor(readonly store: AutonomousGoalWorkerJournalTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") fail("journal text store must implement read and write");
  }

  async read(): Promise<AutonomousGoalWorkerJournalSnapshot | null> {
    const raw = await this.store.read();
    if (raw === null) return null;
    if (typeof raw !== "string" || !isUnicodeScalarString(raw) || new TextEncoder().encode(raw).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("journal JSON is invalid or non-canonical");
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch (error) {
      const wrapped = new ArgumentError("autonomous goal worker journal JSON is invalid");
      (wrapped as Error & { cause?: unknown }).cause = error;
      throw wrapped;
    }
    if (canonicalJson(parsed) !== raw) fail("journal JSON is invalid or non-canonical");
    return AutonomousGoalWorkerJournal.validateSnapshot(parsed);
  }

  async write(snapshot: AutonomousGoalWorkerJournalSnapshot): Promise<void> {
    const normalized = AutonomousGoalWorkerJournal.validateSnapshot(snapshot);
    await this.store.write(canonicalJson(normalized));
  }
}

interface GoalWorkerJournalAuthentication {
  schema: typeof AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA;
  key_id: string;
  tag: string;
}

/** HMAC-authenticated journal persistence for a shared transactional store.
 * The keyring is deployment-owned; readers can trust prior keys during rotation while writes use
 * the active key. HMAC provides authenticity and integrity, not encryption, authorization, or
 * anti-rollback protection.
 */
export class AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence extends JsonAutonomousGoalWorkerJournalPersistence {
  override readonly store: TransactionalAutonomousGoalWorkerJournalTextStore;
  readonly active_key_id: string;
  private readonly keys = new Map<string, Uint8Array>();

  constructor(options: {
    store: TransactionalAutonomousGoalWorkerJournalTextStore;
    keys: ReadonlyMap<string, Uint8Array>;
    active_key_id: string;
  }) {
    super(options.store);
    if (typeof options.store.writeIfUnchanged !== "function") fail("authenticated shared journal persistence requires writeIfUnchanged");
    if (!(options.keys instanceof Map) || options.keys.size < 1 || options.keys.size > 16) fail("authenticated journal keyring must contain 1..16 keys");
    for (const [rawId, rawKey] of options.keys) {
      const keyId = identifier("authentication key id", rawId);
      if (!(rawKey instanceof Uint8Array) || rawKey.byteLength < 32 || rawKey.byteLength > 4_096) fail("authenticated journal keyring contains an invalid key");
      if (this.keys.has(keyId)) fail("authenticated journal keyring contains a duplicate normalized id");
      this.keys.set(keyId, new Uint8Array(rawKey));
    }
    this.active_key_id = identifier("active authentication key id", options.active_key_id);
    if (!this.keys.has(this.active_key_id)) fail("active authentication key id is not present in the keyring");
    this.store = options.store;
  }

  protected envelope(snapshotValue: unknown): AutonomousGoalWorkerJournalSnapshot & { authentication: GoalWorkerJournalAuthentication } {
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(snapshotValue);
    const authenticatedBody = { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: this.active_key_id, snapshot };
    const tag = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(authenticatedBody)), this.keys.get(this.active_key_id)!);
    return {
      ...snapshot,
      authentication: { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: this.active_key_id, tag },
    };
  }

  override async read(): Promise<AutonomousGoalWorkerJournalSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || !isUnicodeScalarString(encoded) || new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("authenticated journal JSON is outside its byte bound");
    let raw: unknown;
    try { raw = JSON.parse(encoded); } catch { fail("authenticated journal JSON is invalid"); }
    if (!isObject(raw) || !isObject(raw.authentication)) fail("authenticated journal envelope is malformed");
    const authentication = raw.authentication;
    if (Object.keys(authentication).sort().join(",") !== "key_id,schema,tag" || authentication.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA) fail("authenticated journal envelope markers are invalid");
    const keyId = identifier("authentication key id", authentication.key_id);
    const tag = digest("authentication tag", authentication.tag)!;
    const key = this.keys.get(keyId);
    if (key === undefined) fail("authenticated journal key id is not trusted by this keyring");
    const snapshotRaw = Object.fromEntries(Object.entries(raw).filter(([name]) => name !== "authentication"));
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(snapshotRaw);
    const authenticatedBody = { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: keyId, snapshot };
    const expected = hmacSha256HexSync(new TextEncoder().encode(canonicalJson(authenticatedBody)), key);
    let mismatch = 0;
    for (let index = 0; index < expected.length; index += 1) mismatch |= expected.charCodeAt(index) ^ tag.charCodeAt(index);
    if (mismatch !== 0) fail("authenticated journal tag does not match its snapshot");
    const normalizedEnvelope = {
      ...snapshot,
      authentication: { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA, key_id: keyId, tag },
    };
    if (canonicalJson(normalizedEnvelope) !== encoded) fail("authenticated journal JSON is not canonical");
    return snapshot;
  }

  override async write(snapshot: AutonomousGoalWorkerJournalSnapshot): Promise<void> {
    const current = await this.read();
    const expected = current?.snapshot_digest ?? null;
    if (!await this.writeIfUnchanged(expected, snapshot)) fail("authenticated journal persistence compare-and-swap conflict");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalWorkerJournalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("expected_snapshot_digest", expectedSnapshotDigest);
    const envelope = this.envelope(snapshot);
    if (expectedSnapshotDigest === envelope.snapshot_digest) {
      const current = await this.read();
      return current !== null && current.snapshot_digest === expectedSnapshotDigest;
    }
    const encoded = canonicalJson(envelope);
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("authenticated journal snapshot exceeds its byte bound");
    return this.store.writeIfUnchanged(expectedSnapshotDigest, encoded);
  }
}

function normalizeMonotonicAnchor(value: unknown): AutonomousGoalWorkerJournalMonotonicAnchor | null {
  if (value === null) return null;
  if (!isObject(value) || Object.keys(value).sort().join(",") !== "anchor_digest,head_digest,schema,sequence,snapshot_digest" || value.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_MONOTONIC_ANCHOR_SCHEMA) fail("monotonic anchor record is malformed");
  const body = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_MONOTONIC_ANCHOR_SCHEMA,
    sequence: integer("anchor.sequence", value.sequence, 0, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS),
    snapshot_digest: digest("anchor.snapshot_digest", value.snapshot_digest)!,
    head_digest: value.head_digest === "" ? "" : digest("anchor.head_digest", value.head_digest)!,
  };
  const anchorDigest = digest("anchor.anchor_digest", value.anchor_digest)!;
  if (digestJsonSync(body) !== anchorDigest) fail("monotonic anchor digest does not match its content");
  return Object.freeze({ ...body, anchor_digest: anchorDigest });
}

function anchorForSnapshot(snapshot: AutonomousGoalWorkerJournalSnapshot): AutonomousGoalWorkerJournalMonotonicAnchor {
  const body = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_MONOTONIC_ANCHOR_SCHEMA,
    sequence: snapshot.sequence,
    snapshot_digest: snapshot.snapshot_digest,
    head_digest: snapshot.head_digest,
  };
  return Object.freeze({ ...body, anchor_digest: digestJsonSync(body) });
}

/**
 * HMAC persistence with a caller-owned non-rollback anchor. The anchor advances before the text
 * snapshot CAS; a crash between those writes fails closed and requires explicit signed roll-forward.
 */
export class MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence extends AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence {
  constructor(options: {
    store: TransactionalAutonomousGoalWorkerJournalTextStore;
    anchor: TransactionalAutonomousGoalWorkerJournalMonotonicAnchorStore;
    keys: ReadonlyMap<string, Uint8Array>;
    active_key_id: string;
  }) {
    super(options);
    if (!options.anchor || typeof options.anchor.read !== "function" || typeof options.anchor.writeIfUnchanged !== "function") fail("monotonic anchor must implement read and writeIfUnchanged");
    this.anchor = options.anchor;
  }

  private readonly anchor: TransactionalAutonomousGoalWorkerJournalMonotonicAnchorStore;

  private async readAnchor(): Promise<AutonomousGoalWorkerJournalMonotonicAnchor | null> {
    return normalizeMonotonicAnchor(await this.anchor.read());
  }

  override async read(): Promise<AutonomousGoalWorkerJournalSnapshot | null> {
    const snapshot = await super.read();
    const anchor = await this.readAnchor();
    if (snapshot === null && anchor === null) return null;
    if (snapshot === null || anchor === null) fail("monotonic anchor and authenticated snapshot are not both present");
    if (canonicalJson(anchorForSnapshot(snapshot)) !== canonicalJson(anchor)) fail("monotonic anchor does not match the authenticated snapshot; rollback or incomplete commit detected");
    return snapshot;
  }

  override async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshotValue: AutonomousGoalWorkerJournalSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("expected_snapshot_digest", expectedSnapshotDigest);
    const current = await this.read();
    const currentDigest = current?.snapshot_digest ?? null;
    if (currentDigest !== expectedSnapshotDigest) return false;
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(snapshotValue);
    if (current !== null && snapshot.snapshot_digest === currentDigest) return true;
    if (current !== null && snapshot.sequence <= current.sequence) fail("monotonic journal snapshot sequence must advance");
    const encoded = canonicalJson(this.envelope(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("authenticated journal snapshot exceeds its byte bound");
    const currentAnchor = current === null ? null : await this.readAnchor();
    const nextAnchor = anchorForSnapshot(snapshot);
    const anchorAdvanced = await this.anchor.writeIfUnchanged(currentAnchor?.anchor_digest ?? null, nextAnchor);
    if (anchorAdvanced !== true && anchorAdvanced !== false) fail("monotonic anchor compare-and-set must return a boolean");
    if (anchorAdvanced === false) return false;
    const snapshotWritten = await this.store.writeIfUnchanged(expectedSnapshotDigest, encoded);
    if (snapshotWritten !== true && snapshotWritten !== false) fail("journal compare-and-set must return a boolean");
    if (snapshotWritten === false) fail("monotonic anchor advanced before journal compare-and-swap; matching anchor-bound snapshot roll-forward is required");
    return true;
  }

  async rollForward(snapshotValue: AutonomousGoalWorkerJournalSnapshot): Promise<AutonomousGoalWorkerJournalSnapshot> {
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(snapshotValue);
    const anchor = await this.readAnchor();
    if (anchor === null || canonicalJson(anchorForSnapshot(snapshot)) !== canonicalJson(anchor)) fail("roll-forward snapshot does not match the trusted monotonic anchor");
    const current = await super.read();
    if (current !== null && current.snapshot_digest === snapshot.snapshot_digest) {
      const restored = await this.read();
      if (restored === null) fail("roll-forward did not restore the anchor-bound journal snapshot");
      return restored;
    }
    if (current !== null && current.sequence >= snapshot.sequence) fail("roll-forward snapshot does not advance the stored journal");
    const encoded = canonicalJson(this.envelope(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_AUTHENTICATED_BYTES) fail("authenticated journal snapshot exceeds its byte bound");
    const expected = current?.snapshot_digest ?? null;
    const written = await this.store.writeIfUnchanged(expected, encoded);
    if (written !== true && written !== false) fail("journal compare-and-set must return a boolean");
    if (written === false) fail("journal roll-forward compare-and-swap conflict");
    const restored = await this.read();
    if (restored === null || restored.snapshot_digest !== snapshot.snapshot_digest) fail("roll-forward did not restore the anchor-bound journal snapshot");
    return restored;
  }
}

export class AutonomousGoalWorkerJournalPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;

  constructor(readonly journal: AutonomousGoalWorkerJournal, readonly persistence: JsonAutonomousGoalWorkerJournalPersistence) {
    if (!(journal instanceof AutonomousGoalWorkerJournal) || !(persistence instanceof JsonAutonomousGoalWorkerJournalPersistence)) fail("journal persistence coordinator arguments are invalid");
  }

  async restore(): Promise<AutonomousGoalWorkerJournalSnapshot | null> {
    const snapshot = await this.persistence.read();
    this.expectedSnapshotDigest = snapshot?.snapshot_digest ?? null;
    if (snapshot) this.journal.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousGoalWorkerJournalSnapshot> {
    const snapshot = this.journal.snapshot();
    const adapterWriter = this.persistence as JsonAutonomousGoalWorkerJournalPersistence & Partial<Pick<AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence, "writeIfUnchanged">>;
    if (typeof adapterWriter.writeIfUnchanged === "function") {
      if (!await adapterWriter.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) fail("journal persistence compare-and-swap conflict");
    } else if (typeof this.persistence.store.writeIfUnchanged === "function") {
      if (!await this.persistence.store.writeIfUnchanged(this.expectedSnapshotDigest, canonicalJson(snapshot))) fail("journal persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }

  async reconcileExternalOutcome(
    ledger: InMemoryAutonomousGoalLedger,
    raw: unknown,
    verifier: AutonomousGoalDispatchResolutionVerifier,
  ): Promise<AutonomousGoalDispatchResolutionResult> {
    const prepared = this.journal.prepareExternalOutcome(ledger, raw, verifier);
    if (prepared.idempotent) return this.journal.commitExternalOutcome(ledger, prepared);
    this.journal.stageExternalOutcome(prepared);
    await this.flush();
    const result = this.journal.commitExternalOutcome(ledger, prepared);
    if (result.goal_status !== "blocked") await this.flush();
    return result;
  }
}
