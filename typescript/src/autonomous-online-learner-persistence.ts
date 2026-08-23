import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousOnlineLearner,
} from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { BrainBanditPolicy, BrainBanditState, JsonObject } from "./types.js";

/** Digest-bound persistence contract for the local online bandit state. */
const LEGACY_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-online-learner-snapshot/0.1" as const;
export const AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-online-learner-snapshot/0.2" as const;
export const AUTONOMOUS_ONLINE_LEARNER_STATE_SCHEMA = "bioprism-brain-bandit-state/0.1" as const;
export const MAX_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_BYTES = 1_000_000;
export const MAX_AUTONOMOUS_ONLINE_LEARNER_ARMS = 4_096;
export const MAX_AUTONOMOUS_ONLINE_LEARNER_CONTEXTS = 64;
export const MAX_AUTONOMOUS_ONLINE_LEARNER_CREDITED_OUTCOMES = 4_096;

export interface AutonomousOnlineLearnerSnapshot extends JsonObject {
  /** 0.1 remains readable for migration; new snapshots use the chained 0.2 envelope. */
  schema: typeof AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA | typeof LEGACY_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA;
  /** Monotonic snapshot image generation, independent from bandit reward generation. */
  snapshot_generation?: number;
  /** Digest of the immediately preceding persisted snapshot; required by 0.2. */
  previous_snapshot_digest?: string | null;
  state: BrainBanditState;
  state_digest: string;
  snapshot_digest: string;
  retention: "bandit_arm_and_evaluator_digest_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousOnlineLearnerSnapshotPersistence {
  read(): Promise<AutonomousOnlineLearnerSnapshot | null> | AutonomousOnlineLearnerSnapshot | null;
  write(snapshot: AutonomousOnlineLearnerSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousOnlineLearnerSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousOnlineLearnerSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousOnlineLearnerTransactionalSnapshotTextStore extends AutonomousOnlineLearnerSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

const RETENTION = "bandit_arm_and_evaluator_digest_metadata_only" as const;
const SECRET_MATERIAL = "never_returned" as const;
const STATE_KEYS = new Set(["schema", "generation", "policy", "arms", "credited_outcomes", "contextual_states"]);
const POLICY_KEYS = new Set(["strategy", "exploration", "epsilon", "min_reward", "max_reward", "failure_penalty", "seed"]);
const ARM_KEYS = new Set(["arm_id", "pulls", "reward_sum", "failures", "disabled"]);
const OUTCOME_KEYS = new Set(["outcome_digest", "arm_id", "reward", "failed", "contract_digest", "context_digest"]);
const CONTEXT_STATE_KEYS = new Set(["context_digest", "context", "generation", "arms", "observed"]);
const CONTEXT_KEYS = new Set(["domain", "capability", "risk_class", "task_family"]);
const SECRET_KEY = /(?:api[_-]?key|authorization|bearer|credential|password|private[_-]?key|refresh[_-]?token|secret)/i;

function clone<T>(value: T): T {
  return structuredClone(value);
}

function digest(value: unknown, name: string): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedGeneration(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new ArgumentError(`${name} must be a non-negative safe integer`);
  return value as number;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded metadata contract`);
  return value;
}

function safeKeys(value: object, allowed: ReadonlySet<string>, name: string): void {
  for (const key of Object.keys(value)) {
    if (!allowed.has(key) || (SECRET_KEY.test(key) && key !== "secret_material")) throw new ArgumentError(`${name} contains unsupported or credential-shaped metadata`);
  }
}

function normalizePolicy(value: unknown): BrainBanditPolicy | undefined {
  if (value === undefined || value === null) return undefined;
  if (!isObject(value)) throw new ArgumentError("online learner policy is malformed");
  safeKeys(value, POLICY_KEYS, "online learner policy");
  return { ...value } as unknown as BrainBanditPolicy;
}

function normalizeState(raw: unknown): BrainBanditState {
  if (!isObject(raw)) throw new ArgumentError("online learner state is malformed");
  safeKeys(raw, STATE_KEYS, "online learner state");
  if (raw.schema !== AUTONOMOUS_ONLINE_LEARNER_STATE_SCHEMA) throw new ArgumentError("online learner state schema is invalid");
  if (!Array.isArray(raw.arms) || raw.arms.length > MAX_AUTONOMOUS_ONLINE_LEARNER_ARMS) throw new ArgumentError("online learner state arms exceed their bound");
  for (const arm of raw.arms) {
    if (!isObject(arm)) throw new ArgumentError("online learner arm is malformed");
    safeKeys(arm, ARM_KEYS, "online learner arm");
  }
  if (raw.credited_outcomes !== undefined) {
    if (!Array.isArray(raw.credited_outcomes) || raw.credited_outcomes.length > MAX_AUTONOMOUS_ONLINE_LEARNER_CREDITED_OUTCOMES) throw new ArgumentError("online learner credited outcomes exceed their bound");
    for (const outcome of raw.credited_outcomes) {
      if (!isObject(outcome)) throw new ArgumentError("online learner credited outcome is malformed");
      safeKeys(outcome, OUTCOME_KEYS, "online learner credited outcome");
    }
  }
  if (raw.contextual_states !== undefined) {
    if (!Array.isArray(raw.contextual_states) || raw.contextual_states.length > MAX_AUTONOMOUS_ONLINE_LEARNER_CONTEXTS) throw new ArgumentError("online learner contextual states exceed their bound");
    for (const state of raw.contextual_states) {
      if (!isObject(state)) throw new ArgumentError("online learner contextual state is malformed");
      safeKeys(state, CONTEXT_STATE_KEYS, "online learner contextual state");
      if (!isObject(state.context)) throw new ArgumentError("online learner contextual state context is malformed");
      safeKeys(state.context, CONTEXT_KEYS, "online learner context");
      if (!Array.isArray(state.arms) || state.arms.length > MAX_AUTONOMOUS_ONLINE_LEARNER_ARMS) throw new ArgumentError("online learner contextual arms exceed their bound");
      for (const arm of state.arms) {
        if (!isObject(arm)) throw new ArgumentError("online learner contextual arm is malformed");
        safeKeys(arm, ARM_KEYS, "online learner contextual arm");
      }
    }
  }
  const state = {
    schema: AUTONOMOUS_ONLINE_LEARNER_STATE_SCHEMA,
    generation: raw.generation,
    ...(normalizePolicy(raw.policy) ? { policy: normalizePolicy(raw.policy) } : {}),
    arms: raw.arms,
    ...(raw.credited_outcomes === undefined ? {} : { credited_outcomes: raw.credited_outcomes }),
    ...(raw.contextual_states === undefined ? {} : { contextual_states: raw.contextual_states }),
  } as unknown as BrainBanditState;
  const learner = new AutonomousOnlineLearner({ state });
  const normalized = learner.snapshot();
  if (normalized.schema !== AUTONOMOUS_ONLINE_LEARNER_STATE_SCHEMA) throw new ArgumentError("online learner state schema changed during validation");
  return clone(normalized);
}

/** Validate a restart image before it can alter a live learner. */
export async function validateAutonomousOnlineLearnerSnapshot(raw: unknown): Promise<AutonomousOnlineLearnerSnapshot> {
  if (!isObject(raw)) throw new ArgumentError("online learner snapshot schema is invalid");
  const legacy = raw.schema === LEGACY_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA;
  if (raw.schema !== AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA && !legacy) throw new ArgumentError("online learner snapshot schema is invalid");
  safeKeys(raw, legacy
    ? new Set(["schema", "state", "state_digest", "snapshot_digest", "retention", "secret_material"])
    : new Set(["schema", "snapshot_generation", "previous_snapshot_digest", "state", "state_digest", "snapshot_digest", "retention", "secret_material"]), "online learner snapshot");
  if (raw.retention !== RETENTION || raw.secret_material !== SECRET_MATERIAL) throw new ArgumentError("online learner snapshot retention markers are invalid");
  const state = normalizeState(raw.state);
  const stateDigest = digest(raw.state_digest, "online learner snapshot state_digest");
  if (await digestJson(state) !== stateDigest) throw new ArgumentError("online learner snapshot state digest does not match");
  const snapshotGeneration = legacy ? undefined : boundedGeneration(raw.snapshot_generation, "online learner snapshot_generation");
  if (!legacy) {
    if (snapshotGeneration! < 1) throw new ArgumentError("online learner snapshot_generation must start at one");
    if (raw.previous_snapshot_digest !== null) digest(raw.previous_snapshot_digest, "online learner previous_snapshot_digest");
    if ((snapshotGeneration === 1) !== (raw.previous_snapshot_digest === null)) throw new ArgumentError("online learner snapshot generation and previous_snapshot_digest are inconsistent");
  }
  const descriptor = legacy
    ? { schema: LEGACY_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA, state, state_digest: stateDigest, retention: RETENTION, secret_material: SECRET_MATERIAL } as const
    : { schema: AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA, snapshot_generation: snapshotGeneration!, previous_snapshot_digest: raw.previous_snapshot_digest as string | null, state, state_digest: stateDigest, retention: RETENTION, secret_material: SECRET_MATERIAL } as const;
  const snapshotDigest = digest(raw.snapshot_digest, "online learner snapshot snapshot_digest");
  if (await digestJson(descriptor) !== snapshotDigest) throw new ArgumentError("online learner snapshot digest does not match");
  const snapshot = { ...descriptor, snapshot_digest: snapshotDigest } satisfies AutonomousOnlineLearnerSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_BYTES) throw new ArgumentError("online learner snapshot exceeds its byte bound");
  return clone(snapshot);
}

/** Seal the learner's current bandit state without retaining task or provider values. */
export async function snapshotAutonomousOnlineLearner(learner: AutonomousOnlineLearner, options: { snapshotGeneration?: number; previousSnapshotDigest?: string | null } = {}): Promise<AutonomousOnlineLearnerSnapshot> {
  if (!(learner instanceof AutonomousOnlineLearner)) throw new ArgumentError("online learner snapshot requires an AutonomousOnlineLearner");
  const snapshotGeneration = boundedGeneration(options.snapshotGeneration ?? 1, "online learner snapshot_generation");
  if (snapshotGeneration < 1) throw new ArgumentError("online learner snapshot_generation must start at one");
  const previousSnapshotDigest = options.previousSnapshotDigest ?? null;
  if (previousSnapshotDigest !== null) digest(previousSnapshotDigest, "online learner previous_snapshot_digest");
  if ((snapshotGeneration === 1) !== (previousSnapshotDigest === null)) throw new ArgumentError("online learner snapshot generation and previous_snapshot_digest are inconsistent");
  const state = normalizeState(learner.snapshot());
  const stateDigest = await digestJson(state);
  const descriptor = { schema: AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_SCHEMA, snapshot_generation: snapshotGeneration, previous_snapshot_digest: previousSnapshotDigest, state, state_digest: stateDigest, retention: RETENTION, secret_material: SECRET_MATERIAL } as const;
  return validateAutonomousOnlineLearnerSnapshot({ ...descriptor, snapshot_digest: await digestJson(descriptor) });
}

/** CAS-aware connection between a live learner and caller-owned storage. */
export class AutonomousOnlineLearnerPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private snapshotGeneration = 0;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly learner: AutonomousOnlineLearner, readonly persistence: AutonomousOnlineLearnerSnapshotPersistence) {
    if (!(learner instanceof AutonomousOnlineLearner)) throw new ArgumentError("online learner persistence requires an AutonomousOnlineLearner");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("online learner persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousOnlineLearnerSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        this.snapshotGeneration = 0;
        return null;
      }
      const snapshot = await validateAutonomousOnlineLearnerSnapshot(raw);
      this.learner.restore(snapshot.state);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      // A legacy image has no chain metadata. Treat it as a migration root so the
      // first 0.2 write is valid and future writes are continuous.
      this.snapshotGeneration = snapshot.snapshot_generation ?? 0;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousOnlineLearnerSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await snapshotAutonomousOnlineLearner(this.learner, {
        snapshotGeneration: this.snapshotGeneration + 1,
        previousSnapshotDigest: this.snapshotGeneration === 0 ? null : this.expectedSnapshotDigest,
      });
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("online learner persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      this.snapshotGeneration = snapshot.snapshot_generation!;
      return clone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousOnlineLearnerSnapshotPersistence implements AutonomousOnlineLearnerSnapshotPersistence {
  constructor(readonly textStore: AutonomousOnlineLearnerSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("online learner text store is malformed");
  }

  async read(): Promise<AutonomousOnlineLearnerSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_ONLINE_LEARNER_SNAPSHOT_BYTES) throw new ArgumentError("online learner JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("online learner JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("online learner JSON is not canonical");
    return validateAutonomousOnlineLearnerSnapshot(parsed);
  }

  async write(snapshot: AutonomousOnlineLearnerSnapshot): Promise<void> {
    const validated = await validateAutonomousOnlineLearnerSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

export class TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence extends JsonAutonomousOnlineLearnerSnapshotPersistence {
  declare readonly textStore: AutonomousOnlineLearnerTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousOnlineLearnerTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("online learner text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousOnlineLearnerSnapshot): Promise<boolean> {
    const validated = await validateAutonomousOnlineLearnerSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

export class WebStorageAutonomousOnlineLearnerSnapshotTextStore implements AutonomousOnlineLearnerSnapshotTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("online learner Web Storage adapter is malformed");
    boundedText("online learner storage key", key, 256);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}
