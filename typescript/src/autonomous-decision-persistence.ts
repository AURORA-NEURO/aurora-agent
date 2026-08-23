import { ArgumentError } from "./errors.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only state for a single provider decision cycle. */
export const AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA = "bioprism-typescript-autonomous-decision-cycle-state/0.2" as const;
export const AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-decision-cycle-snapshot/0.2" as const;
export const AUTONOMOUS_DECISION_CYCLE_MAX_STATES = 8_192;
export const AUTONOMOUS_DECISION_CYCLE_MAX_SNAPSHOT_BYTES = 8_000_000;

export type AutonomousDecisionCycleMode = "single_domain" | "cross_domain";
export type AutonomousDecisionCyclePhase = "route_pending" | "planning_pending" | "execution_pending" | "evaluation_pending" | "settlement_pending" | "terminal";

export interface AutonomousDecisionCycleState extends JsonObject {
  schema: typeof AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA;
  cycle_id: string;
  task_digest: string;
  mode: AutonomousDecisionCycleMode;
  learning_enabled: boolean;
  evaluation_enabled: boolean;
  phase: AutonomousDecisionCyclePhase;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  selection_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  learning_episode_ids: string[];
  trajectory_id: string | null;
  settlement_digests: string[];
  terminal_status: string | null;
  generation: number;
  previous_state_digest: string | null;
  state_digest: string;
  retention: "metadata_only_hash_chained_no_private_payloads";
  secret_material: "never_returned";
}

export interface AutonomousDecisionCycleSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA;
  states: AutonomousDecisionCycleState[];
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousDecisionCycleStateStore {
  load(cycleId: string): Promise<AutonomousDecisionCycleState | null> | AutonomousDecisionCycleState | null;
  save(state: AutonomousDecisionCycleState): Promise<void> | void;
  snapshot(): Promise<AutonomousDecisionCycleSnapshot>;
  restore(snapshot: AutonomousDecisionCycleSnapshot): Promise<void> | void;
}

export interface AutonomousDecisionCycleSnapshotPersistence {
  read(): Promise<AutonomousDecisionCycleSnapshot | null> | AutonomousDecisionCycleSnapshot | null;
  write(snapshot: AutonomousDecisionCycleSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousDecisionCycleSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousDecisionCycleSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousDecisionCycleTransactionalSnapshotTextStore extends AutonomousDecisionCycleSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousDecisionCycleRehydrationContext extends JsonObject {
  cycle_id: string;
  task_digest: string;
  mode: AutonomousDecisionCycleMode;
  learning_enabled: boolean;
  evaluation_enabled: boolean;
  phase: AutonomousDecisionCyclePhase;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  selection_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  learning_episode_ids: string[];
  trajectory_id: string | null;
  settlement_digests: string[];
  terminal_status: string | null;
  generation: number;
  state_digest: string;
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function jsonBytes(value: unknown): number {
  let encoded: string | undefined;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new ArgumentError("autonomous decision-cycle metadata must be JSON serializable");
  }
  if (typeof encoded !== "string") throw new ArgumentError("autonomous decision-cycle metadata must be JSON serializable");
  return new TextEncoder().encode(encoded).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || /[\u0000-\u001F\u007F]/.test(value)) throw new ArgumentError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function exactKeys(name: string, value: JsonObject, allowed: readonly string[]): void {
  const permitted = new Set(allowed);
  if (Object.keys(value).some((key) => !permitted.has(key)) || allowed.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function assertNoPrivateShape(value: unknown, name: string): void {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new ArgumentError(`${name} must be JSON serializable`);
  }
  const markerFree = encoded.replace(/metadata_only_hash_chained_no_private_payloads|metadata_only_hash_bound|never_returned/g, "");
  if (/(?:api[_-]?key|authorization|bearer|password|private[_-]?key|access[_-]?token|refresh[_-]?token|credential|prompt|response|arguments?|output|payload|transcript|secret)(?!_material)/i.test(markerFree)) throw new ArgumentError(`${name} contains private or payload-shaped material`);
}

const STATE_KEYS = [
  "schema", "cycle_id", "task_digest", "mode", "learning_enabled", "evaluation_enabled", "phase", "route_digest", "plan_refinement_digest", "selection_digest",
  "outcome_digest", "evaluation_digest", "learning_episode_ids", "trajectory_id", "settlement_digests", "terminal_status", "generation", "previous_state_digest",
  "state_digest", "retention", "secret_material",
] as const;

function stateDescriptor(value: AutonomousDecisionCycleState): JsonObject {
  const { state_digest: _stateDigest, ...descriptor } = value;
  return descriptor;
}

function snapshotDescriptor(value: AutonomousDecisionCycleSnapshot): JsonObject {
  const { snapshot_digest: _snapshotDigest, ...descriptor } = value;
  return descriptor;
}

function validateDigestList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its capacity`);
  const rows = value.map((item, index) => boundedDigest(`${name}[${index}]`, item)!);
  if (new Set(rows).size !== rows.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return rows;
}

function validateIdentifierList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its capacity`);
  const rows = value.map((item, index) => boundedIdentifier(`${name}[${index}]`, item));
  if (new Set(rows).size !== rows.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return rows;
}

/** Validate one state before it crosses a process or storage boundary. */
export async function validateAutonomousDecisionCycleState(value: unknown): Promise<AutonomousDecisionCycleState> {
  if (!isObject(value)) throw new ArgumentError("autonomous decision-cycle state must be an object");
  exactKeys("autonomous decision-cycle state", value, STATE_KEYS);
  if (value.schema !== AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA || value.retention !== "metadata_only_hash_chained_no_private_payloads" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous decision-cycle state markers are invalid");
  const cycleId = boundedIdentifier("autonomous decision-cycle state cycle_id", value.cycle_id);
  const taskDigest = boundedDigest("autonomous decision-cycle state task_digest", value.task_digest)!;
  if (value.mode !== "single_domain" && value.mode !== "cross_domain") throw new ArgumentError("autonomous decision-cycle state mode is invalid");
  if (typeof value.learning_enabled !== "boolean" || typeof value.evaluation_enabled !== "boolean") throw new ArgumentError("autonomous decision-cycle learning flags are invalid");
  if (!["route_pending", "planning_pending", "execution_pending", "evaluation_pending", "settlement_pending", "terminal"].includes(value.phase as string)) throw new ArgumentError("autonomous decision-cycle state phase is invalid");
  const phase = value.phase as AutonomousDecisionCyclePhase;
  const routeDigest = boundedDigest("autonomous decision-cycle state route_digest", value.route_digest, true);
  const planRefinementDigest = boundedDigest("autonomous decision-cycle state plan_refinement_digest", value.plan_refinement_digest, true);
  const selectionDigest = boundedDigest("autonomous decision-cycle state selection_digest", value.selection_digest, true);
  const outcomeDigest = boundedDigest("autonomous decision-cycle state outcome_digest", value.outcome_digest, true);
  const evaluationDigest = boundedDigest("autonomous decision-cycle state evaluation_digest", value.evaluation_digest, true);
  const learningEpisodeIds = validateIdentifierList("autonomous decision-cycle state learning_episode_ids", value.learning_episode_ids, 256);
  const trajectoryId = value.trajectory_id === null ? null : boundedIdentifier("autonomous decision-cycle state trajectory_id", value.trajectory_id);
  const settlementDigests = validateDigestList("autonomous decision-cycle state settlement_digests", value.settlement_digests, 256);
  const terminalStatus = value.terminal_status === null ? null : boundedIdentifier("autonomous decision-cycle state terminal_status", value.terminal_status);
  const generation = boundedCount("autonomous decision-cycle state generation", value.generation, Number.MAX_SAFE_INTEGER, 1);
  const previousStateDigest = boundedDigest("autonomous decision-cycle state previous_state_digest", value.previous_state_digest, true);
  const stateDigest = boundedDigest("autonomous decision-cycle state state_digest", value.state_digest)!;
  if ((generation === 1 && previousStateDigest !== null) || (generation > 1 && previousStateDigest === null)) throw new ArgumentError("autonomous decision-cycle state hash chain is malformed");
  if (phase === "route_pending" && routeDigest !== null && (planRefinementDigest !== null || selectionDigest !== null || outcomeDigest !== null || evaluationDigest !== null || learningEpisodeIds.length > 0 || settlementDigests.length > 0 || terminalStatus !== null)) throw new ArgumentError("route-pending decision route receipt cannot contain planning, execution, or terminal metadata");
  if (phase !== "route_pending" && routeDigest === null) throw new ArgumentError("decision state phase requires a route digest");
  if (phase === "planning_pending" && (selectionDigest !== null || outcomeDigest !== null || evaluationDigest !== null || learningEpisodeIds.length > 0 || settlementDigests.length > 0 || terminalStatus !== null)) throw new ArgumentError("planning-pending decision state cannot contain execution or terminal metadata");
  if (["evaluation_pending", "settlement_pending", "terminal"].includes(phase) && outcomeDigest === null) throw new ArgumentError("decision state phase requires an outcome digest");
  if (phase === "settlement_pending" && (!value.evaluation_enabled || evaluationDigest === null)) throw new ArgumentError("settlement-pending decision state requires an evaluation digest");
  if (!value.evaluation_enabled && evaluationDigest !== null) throw new ArgumentError("decision state cannot retain an evaluation digest when evaluation is disabled");
  if (phase === "terminal" && terminalStatus === null) throw new ArgumentError("terminal decision state requires a terminal status");
  if (phase !== "terminal" && terminalStatus !== null) throw new ArgumentError("non-terminal decision state cannot contain a terminal status");
  if (value.mode === "single_domain" && trajectoryId !== null) throw new ArgumentError("single-domain decision state cannot contain a trajectory ID");
  if (value.mode === "cross_domain" && value.learning_enabled && trajectoryId === null) throw new ArgumentError("cross-domain learning state requires a trajectory ID");
  if (value.evaluation_enabled && terminalStatus === "completed" && learningEpisodeIds.length > 0 && settlementDigests.length === 0) throw new ArgumentError("completed evaluated decision state requires a settlement digest");
  assertNoPrivateShape(value, "autonomous decision-cycle state");
  if (jsonBytes(value) > 1_000_000) throw new ArgumentError("autonomous decision-cycle state exceeds its metadata budget");
  if (await digestJson(stateDescriptor(value as unknown as AutonomousDecisionCycleState)) !== stateDigest) throw new ArgumentError("autonomous decision-cycle state digest does not match metadata");
  return clone({
    ...value,
    cycle_id: cycleId,
    task_digest: taskDigest,
    mode: value.mode,
    phase,
    route_digest: routeDigest,
    plan_refinement_digest: planRefinementDigest,
    selection_digest: selectionDigest,
    outcome_digest: outcomeDigest,
    evaluation_digest: evaluationDigest,
    learning_episode_ids: learningEpisodeIds,
    trajectory_id: trajectoryId,
    settlement_digests: settlementDigests,
    terminal_status: terminalStatus,
    generation,
    previous_state_digest: previousStateDigest,
    state_digest: stateDigest,
  }) as AutonomousDecisionCycleState;
}

/** Seal a state descriptor with a canonical content digest. */
export async function sealAutonomousDecisionCycleState(value: Omit<AutonomousDecisionCycleState, "state_digest">): Promise<AutonomousDecisionCycleState> {
  const descriptor = clone(value) as AutonomousDecisionCycleState;
  return validateAutonomousDecisionCycleState({ ...descriptor, state_digest: await digestJson(descriptor) });
}

export async function validateAutonomousDecisionCycleSnapshot(value: unknown): Promise<AutonomousDecisionCycleSnapshot> {
  if (!isObject(value)) throw new ArgumentError("autonomous decision-cycle snapshot must be an object");
  exactKeys("autonomous decision-cycle snapshot", value, ["schema", "states", "retention", "secret_material", "snapshot_digest"]);
  if (value.schema !== AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA || value.retention !== "metadata_only_hash_bound" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous decision-cycle snapshot markers are invalid");
  if (!Array.isArray(value.states) || value.states.length > AUTONOMOUS_DECISION_CYCLE_MAX_STATES) throw new ArgumentError("autonomous decision-cycle snapshot exceeds its state capacity");
  const states: AutonomousDecisionCycleState[] = [];
  const ids = new Set<string>();
  for (const candidate of value.states) {
    const state = await validateAutonomousDecisionCycleState(candidate);
    if (ids.has(state.cycle_id)) throw new ArgumentError("autonomous decision-cycle snapshot contains duplicate cycle IDs");
    ids.add(state.cycle_id);
    states.push(state);
  }
  const snapshotDigest = boundedDigest("autonomous decision-cycle snapshot snapshot_digest", value.snapshot_digest)!;
  const descriptor = { schema: AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA, states, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
  if (await digestJson(descriptor) !== snapshotDigest) throw new ArgumentError("autonomous decision-cycle snapshot digest does not match metadata");
  const normalized = { ...descriptor, snapshot_digest: snapshotDigest } as AutonomousDecisionCycleSnapshot;
  if (jsonBytes(normalized) > AUTONOMOUS_DECISION_CYCLE_MAX_SNAPSHOT_BYTES) throw new ArgumentError("autonomous decision-cycle snapshot exceeds its byte capacity");
  return clone(normalized);
}

/** Bounded process-local reference store; applications should provide a transactional durable implementation. */
export class InMemoryAutonomousDecisionCycleStateStore implements AutonomousDecisionCycleStateStore {
  private readonly states = new Map<string, AutonomousDecisionCycleState>();

  async load(cycleId: string): Promise<AutonomousDecisionCycleState | null> {
    return clone(this.states.get(boundedIdentifier("autonomous decision-cycle cycle_id", cycleId)) ?? null);
  }

  async save(raw: AutonomousDecisionCycleState): Promise<void> {
    const state = await validateAutonomousDecisionCycleState(raw);
    const prior = this.states.get(state.cycle_id);
    if (prior && state.state_digest === prior.state_digest) return;
    if (!prior && (state.generation !== 1 || state.previous_state_digest !== null)) throw new ArgumentError("autonomous decision-cycle initial state must start at generation one");
    if (prior && (state.generation !== prior.generation + 1 || state.previous_state_digest !== prior.state_digest)) throw new ArgumentError("autonomous decision-cycle state generation chain is not contiguous");
    if (!prior && this.states.size >= AUTONOMOUS_DECISION_CYCLE_MAX_STATES) throw new ArgumentError("autonomous decision-cycle state store is full");
    this.states.set(state.cycle_id, clone(state));
  }

  async snapshot(): Promise<AutonomousDecisionCycleSnapshot> {
    const states = [...this.states.values()].sort((left, right) => left.cycle_id.localeCompare(right.cycle_id)).map(clone);
    const descriptor = { schema: AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA, states, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return validateAutonomousDecisionCycleSnapshot({ ...descriptor, snapshot_digest: await digestJson(descriptor) });
  }

  async restore(snapshot: AutonomousDecisionCycleSnapshot): Promise<void> {
    const validated = await validateAutonomousDecisionCycleSnapshot(snapshot);
    this.states.clear();
    for (const state of validated.states) this.states.set(state.cycle_id, clone(state));
  }
}

/** Coordinates hash-bound decision-cycle state with caller-owned durable storage. */
export class AutonomousDecisionCyclePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousDecisionCycleStateStore, readonly persistence: AutonomousDecisionCycleSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("decision-cycle persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("decision-cycle persistence adapter is malformed");
  }

  async flush(): Promise<AutonomousDecisionCycleSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await this.store.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("decision-cycle persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  async restore(): Promise<AutonomousDecisionCycleSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = await validateAutonomousDecisionCycleSnapshot(raw);
      await this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousDecisionCycleSnapshotPersistence implements AutonomousDecisionCycleSnapshotPersistence {
  constructor(readonly textStore: AutonomousDecisionCycleSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("decision-cycle text store is malformed");
  }

  async read(): Promise<AutonomousDecisionCycleSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_DECISION_CYCLE_MAX_SNAPSHOT_BYTES) throw new ArgumentError("decision-cycle JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("decision-cycle JSON is invalid"); }
    return validateAutonomousDecisionCycleSnapshot(parsed);
  }

  async write(raw: AutonomousDecisionCycleSnapshot): Promise<void> {
    const snapshot = await validateAutonomousDecisionCycleSnapshot(raw);
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousDecisionCycleSnapshotPersistence extends JsonAutonomousDecisionCycleSnapshotPersistence {
  declare readonly textStore: AutonomousDecisionCycleTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousDecisionCycleTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("decision-cycle text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, raw: AutonomousDecisionCycleSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ArgumentError("decision-cycle expected snapshot digest is invalid");
    const snapshot = await validateAutonomousDecisionCycleSnapshot(raw);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}
