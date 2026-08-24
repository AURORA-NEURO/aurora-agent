import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_GOAL_RETENTION,
  goalTaskDigest,
  InMemoryAutonomousGoalLedger,
  type AutonomousGoalStatus,
} from "./autonomous-goals.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only restart fencing for the goal executor/provider boundary. */
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SCHEMA = "bioprism-autonomous-goal-worker-journal/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA = "bioprism-autonomous-goal-worker-event/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-worker-snapshot/0.1" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION = "metadata_only_worker_boundary;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS = 16_384;
export const AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES = 2_000_000;

export type AutonomousGoalWorkerJournalPhase = "prepared" | "claimed" | "dispatch_started" | "settled" | "failed" | "reconciled";
const ACTIVE_PHASES = new Set<AutonomousGoalWorkerJournalPhase>(["claimed", "dispatch_started"]);
const ALL_PHASES = new Set<AutonomousGoalWorkerJournalPhase>(["prepared", "claimed", "dispatch_started", "settled", "failed", "reconciled"]);

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
  created_ns: number;
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
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal worker journal ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is outside its bounded identifier contract`);
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

function clone<T>(value: T): T {
  return structuredClone(value);
}

function eventBody(event: Omit<AutonomousGoalWorkerEvent, "event_digest">): Omit<AutonomousGoalWorkerEvent, "event_digest"> {
  const { event_digest: _eventDigest, ...body } = event as AutonomousGoalWorkerEvent;
  return body;
}

function verifyEvent(raw: unknown): AutonomousGoalWorkerEvent {
  if (!isObject(raw)) fail("event must be an object");
  const allowed = new Set(["schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision", "schedule_digest", "claim_digest", "outcome_digest", "error_digest", "created_ns", "previous_digest", "event_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key)) || raw.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA || raw.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || raw.secret_material !== "never_returned") fail("event contains unsupported or unsafe metadata");
  if (typeof raw.phase !== "string" || !ALL_PHASES.has(raw.phase as AutonomousGoalWorkerJournalPhase)) fail("event phase is invalid");
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
    created_ns: integer("event.created_ns", raw.created_ns),
    previous_digest: typeof raw.previous_digest === "string" ? raw.previous_digest : "",
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

export class AutonomousGoalWorkerJournal {
  private readonly maxEvents: number;
  private readonly clock: () => number;
  private eventsValue: AutonomousGoalWorkerEvent[] = [];

  constructor(options: { maxEvents?: number; clock?: () => number } = {}) {
    this.maxEvents = integer("journal maxEvents", options.maxEvents ?? AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS, 1, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS);
    this.clock = options.clock ?? (() => Date.now());
  }

  get head_digest(): string {
    return this.eventsValue.at(-1)?.event_digest ?? "";
  }

  record(input: { batch_id: string; goal_id: string; phase: AutonomousGoalWorkerJournalPhase; attempt: number; revision: number; schedule_digest: string; claim_digest?: string | null; outcome_digest?: string | null; error_digest?: string | null; created_ns?: number }): AutonomousGoalWorkerEvent {
    if (this.eventsValue.length >= this.maxEvents) fail("event capacity is exhausted");
    if (!ALL_PHASES.has(input.phase)) fail("event phase is invalid");
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
      created_ns: integer("created_ns", input.created_ns ?? this.clock()),
      previous_digest: this.head_digest,
      retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
      secret_material: "never_returned" as const,
    } satisfies Omit<AutonomousGoalWorkerEvent, "event_digest">;
    const event = { ...body, event_digest: digestJsonSync(body) } satisfies AutonomousGoalWorkerEvent;
    this.eventsValue.push(event);
    return clone(event);
  }

  events(query: { batch_id?: string; goal_id?: string } = {}): AutonomousGoalWorkerEvent[] {
    const batch = query.batch_id === undefined ? undefined : identifier("batch_id", query.batch_id);
    const goal = query.goal_id === undefined ? undefined : identifier("goal_id", query.goal_id);
    return this.eventsValue.filter((event) => (batch === undefined || event.batch_id === batch) && (goal === undefined || event.goal_id === goal)).map(clone);
  }

  active(): AutonomousGoalWorkerEvent[] {
    const latest = new Map<string, AutonomousGoalWorkerEvent>();
    for (const event of this.eventsValue) latest.set(event.goal_id, event);
    return [...latest.values()].filter((event) => ACTIVE_PHASES.has(event.phase)).sort((left, right) => left.sequence - right.sequence).map(clone);
  }

  snapshot(): AutonomousGoalWorkerJournalSnapshot {
    const body = {
      schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
      sequence: this.eventsValue.length,
      head_digest: this.head_digest,
      events: this.eventsValue.map(clone),
      retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
      secret_material: "never_returned" as const,
    } satisfies Omit<AutonomousGoalWorkerJournalSnapshot, "snapshot_digest">;
    if (new TextEncoder().encode(canonicalJson(body)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    return { ...body, snapshot_digest: digestJsonSync(body) };
  }

  static validateSnapshot(raw: unknown): AutonomousGoalWorkerJournalSnapshot {
    if (!isObject(raw) || raw.schema !== AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA || !Array.isArray(raw.events)) fail("snapshot schema is invalid");
    const allowed = new Set(["schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"]);
    if (Object.keys(raw).some((key) => !allowed.has(key)) || raw.retention !== AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION || raw.secret_material !== "never_returned") fail("snapshot contains unsupported or unsafe metadata");
    if (raw.events.length > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS || integer("snapshot.sequence", raw.sequence, 0, AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_EVENTS) !== raw.events.length) fail("snapshot sequence or capacity is invalid");
    const head = raw.sequence === 0 ? "" : digest("snapshot.head_digest", raw.head_digest)!;
    if (raw.sequence === 0 && raw.head_digest !== "") fail("empty snapshot must have an empty head digest");
    const events = raw.events.map(verifyEvent);
    let previous = "";
    events.forEach((event, index) => {
      if (event.sequence !== index + 1 || event.previous_digest !== previous) fail(`snapshot event chain breaks at sequence ${index + 1}`);
      previous = event.event_digest;
    });
    if (previous !== head) fail("snapshot head digest does not match its event chain");
    const body = { schema: raw.schema, sequence: raw.sequence, head_digest: raw.head_digest, events, retention: raw.retention, secret_material: raw.secret_material };
    if (digest("snapshot.snapshot_digest", raw.snapshot_digest) !== digestJsonSync(body)) fail("snapshot digest does not match its content");
    if (new TextEncoder().encode(canonicalJson(raw)).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    return clone({ ...body, snapshot_digest: raw.snapshot_digest } as AutonomousGoalWorkerJournalSnapshot);
  }

  restore(raw: unknown): AutonomousGoalWorkerJournalSnapshot {
    const snapshot = AutonomousGoalWorkerJournal.validateSnapshot(raw);
    this.eventsValue = snapshot.events.map(clone);
    return clone(snapshot);
  }

  recover(ledger: InMemoryAutonomousGoalLedger, options: { now_ns?: number } = {}): JsonObject {
    if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("recover requires an InMemoryAutonomousGoalLedger");
    if (options.now_ns !== undefined) integer("recover.now_ns", options.now_ns);
    const recovered: JsonObject[] = [];
    for (const event of this.active()) {
      const current = ledger.get(event.goal_id);
      if (!current || current.status !== "running" || current.revision !== event.revision || current.attempt !== event.attempt) fail(`active event for goal ${event.goal_id} no longer matches the ledger`);
      const beforeDispatch = event.phase === "claimed";
      const resultStatus = beforeDispatch ? "worker_restart_before_dispatch" : "worker_restart_after_dispatch";
      const target: AutonomousGoalStatus = beforeDispatch ? "paused" : "blocked";
      const blocker = beforeDispatch ? "worker_restart_before_dispatch" : "worker_restart_after_dispatch_requires_reconciliation";
      const nextAction = beforeDispatch ? "goal-retry" : "goal-reconciliation-review";
      const outcomeDigest = digestJsonSync({ goal_id: event.goal_id, attempt: event.attempt, result_status: resultStatus });
      const updated = ledger.transition(event.goal_id, target, { expected_revision: current.revision, blockers: [blocker], next_action_digest: goalTaskDigest(nextAction), outcome_digest: outcomeDigest, now_ns: options.now_ns });
      this.record({ batch_id: event.batch_id, goal_id: event.goal_id, phase: "reconciled", attempt: event.attempt, revision: updated.revision, schedule_digest: event.schedule_digest, claim_digest: event.claim_digest, outcome_digest: outcomeDigest });
      recovered.push({ goal_id: event.goal_id, from_phase: event.phase, goal_status: updated.status, outcome_digest: outcomeDigest });
    }
    return { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SCHEMA, recovered, recovery_digest: digestJsonSync(recovered), retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION, secret_material: "never_returned" };
  }
}

export class JsonAutonomousGoalWorkerJournalPersistence {
  constructor(readonly store: AutonomousGoalWorkerJournalTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") fail("journal text store must implement read and write");
  }

  async read(): Promise<AutonomousGoalWorkerJournalSnapshot | null> {
    const raw = await this.store.read();
    if (raw === null) return null;
    if (typeof raw !== "string" || new TextEncoder().encode(raw).byteLength > AUTONOMOUS_GOAL_WORKER_JOURNAL_MAX_SNAPSHOT_BYTES) fail("journal JSON is invalid or non-canonical");
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
    if (typeof this.persistence.store.writeIfUnchanged === "function") {
      if (!await this.persistence.store.writeIfUnchanged(this.expectedSnapshotDigest, canonicalJson(snapshot))) fail("journal persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
}
