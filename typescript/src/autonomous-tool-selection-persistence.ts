import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
  normalizeAutonomousToolSelectionState,
  type AutonomousToolSelectionState,
} from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Restart image for the reviewed, value-only adaptive tool selector. */
export const AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-tool-selection-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES = 1_000_000;

export interface AutonomousToolSelectionSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA;
  snapshot_generation: number;
  previous_snapshot_digest: string | null;
  state: AutonomousToolSelectionState;
  state_digest: string;
  snapshot_digest: string;
  retention: "tool_selection_arm_and_evaluator_digest_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousToolSelectionPersistence {
  read(): Promise<AutonomousToolSelectionSnapshot | null> | AutonomousToolSelectionSnapshot | null;
  write(snapshot: AutonomousToolSelectionSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousToolSelectionSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousToolSelectionSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousToolSelectionTransactionalSnapshotTextStore extends AutonomousToolSelectionSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousToolSelectionStateBinding {
  get(): AutonomousToolSelectionState;
  set(state: AutonomousToolSelectionState): void;
}

const RETENTION = "tool_selection_arm_and_evaluator_digest_metadata_only" as const;
const SECRET_MATERIAL = "never_returned" as const;
const SNAPSHOT_KEYS = new Set([
  "schema",
  "snapshot_generation",
  "previous_snapshot_digest",
  "state",
  "state_digest",
  "snapshot_digest",
  "retention",
  "secret_material",
]);
const SECRET_KEY = /(?:api[_-]?key|authorization|bearer|credential|password|private[_-]?key|refresh[_-]?token|secret)/i;

function clone<T>(value: T): T {
  return structuredClone(value);
}

function digest(value: unknown, name: string): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function generation(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) throw new ArgumentError("tool selection snapshot_generation must be a positive safe integer");
  return value as number;
}

function safeSnapshotKeys(value: object): void {
  for (const key of Object.keys(value)) {
    if (!SNAPSHOT_KEYS.has(key) || (SECRET_KEY.test(key) && key !== "secret_material")) throw new ArgumentError("tool selection snapshot contains unsupported or credential-shaped metadata");
  }
}

function descriptor(snapshot: {
  schema: typeof AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA;
  snapshot_generation: number;
  previous_snapshot_digest: string | null;
  state: AutonomousToolSelectionState;
  state_digest: string;
  retention: "tool_selection_arm_and_evaluator_digest_metadata_only";
  secret_material: "never_returned";
}) {
  return {
    schema: snapshot.schema,
    snapshot_generation: snapshot.snapshot_generation,
    previous_snapshot_digest: snapshot.previous_snapshot_digest,
    state: snapshot.state,
    state_digest: snapshot.state_digest,
    retention: snapshot.retention,
    secret_material: snapshot.secret_material,
  } as const;
}

/** Validate a restart image before it can alter the live selector. */
export async function validateAutonomousToolSelectionSnapshot(raw: unknown): Promise<AutonomousToolSelectionSnapshot> {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA) throw new ArgumentError("tool selection snapshot schema is invalid");
  safeSnapshotKeys(raw);
  if (raw.retention !== RETENTION || raw.secret_material !== SECRET_MATERIAL) throw new ArgumentError("tool selection snapshot retention markers are invalid");
  const snapshotGeneration = generation(raw.snapshot_generation);
  const previousSnapshotDigest = raw.previous_snapshot_digest === null
    ? null
    : digest(raw.previous_snapshot_digest, "tool selection previous_snapshot_digest");
  if ((snapshotGeneration === 1) !== (previousSnapshotDigest === null)) throw new ArgumentError("tool selection snapshot chain is malformed");
  const state = normalizeAutonomousToolSelectionState(raw.state);
  const stateDigest = digest(raw.state_digest, "tool selection state_digest");
  if (await digestJson(state) !== stateDigest) throw new ArgumentError("tool selection state digest does not match");
  const body = descriptor({
    schema: AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA,
    snapshot_generation: snapshotGeneration,
    previous_snapshot_digest: previousSnapshotDigest,
    state,
    state_digest: stateDigest,
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  });
  const snapshotDigest = digest(raw.snapshot_digest, "tool selection snapshot_digest");
  if (await digestJson(body) !== snapshotDigest) throw new ArgumentError("tool selection snapshot digest does not match");
  const snapshot = { ...body, snapshot_digest: snapshotDigest } satisfies AutonomousToolSelectionSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES) throw new ArgumentError("tool selection snapshot exceeds its byte bound");
  return clone(snapshot);
}

/** Seal only bounded tool-arm statistics and evaluator settlement identities. */
export async function snapshotAutonomousToolSelection(
  state: AutonomousToolSelectionState,
  options: { snapshotGeneration?: number; previousSnapshotDigest?: string | null } = {},
): Promise<AutonomousToolSelectionSnapshot> {
  const normalized = normalizeAutonomousToolSelectionState(state);
  const snapshotGeneration = generation(options.snapshotGeneration ?? 1);
  const previousSnapshotDigest = options.previousSnapshotDigest ?? null;
  if (previousSnapshotDigest !== null) digest(previousSnapshotDigest, "tool selection previous_snapshot_digest");
  if ((snapshotGeneration === 1) !== (previousSnapshotDigest === null)) throw new ArgumentError("tool selection snapshot chain is malformed");
  const stateDigest = await digestJson(normalized);
  const body = descriptor({
    schema: AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA,
    snapshot_generation: snapshotGeneration,
    previous_snapshot_digest: previousSnapshotDigest,
    state: normalized,
    state_digest: stateDigest,
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  });
  return validateAutonomousToolSelectionSnapshot({ ...body, snapshot_digest: await digestJson(body) });
}

/** Serialize restore/flush operations and fence concurrent writers with a snapshot digest. */
export class AutonomousToolSelectionPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private snapshotGeneration = 0;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly binding: AutonomousToolSelectionStateBinding, readonly persistence: AutonomousToolSelectionPersistence) {
    if (!binding || typeof binding.get !== "function" || typeof binding.set !== "function") throw new ArgumentError("tool selection state binding is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("tool selection persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousToolSelectionSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        this.snapshotGeneration = 0;
        return null;
      }
      const snapshot = await validateAutonomousToolSelectionSnapshot(raw);
      this.binding.set(snapshot.state);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      this.snapshotGeneration = snapshot.snapshot_generation;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousToolSelectionSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await snapshotAutonomousToolSelection(this.binding.get(), {
        snapshotGeneration: this.snapshotGeneration + 1,
        previousSnapshotDigest: this.snapshotGeneration === 0 ? null : this.expectedSnapshotDigest,
      });
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("tool selection persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      this.snapshotGeneration = snapshot.snapshot_generation;
      return clone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousToolSelectionPersistence implements AutonomousToolSelectionPersistence {
  constructor(readonly textStore: AutonomousToolSelectionSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("tool selection text store is malformed");
  }

  async read(): Promise<AutonomousToolSelectionSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES) throw new ArgumentError("tool selection JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("tool selection JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("tool selection JSON is not canonical");
    return validateAutonomousToolSelectionSnapshot(parsed);
  }

  async write(snapshot: AutonomousToolSelectionSnapshot): Promise<void> {
    const validated = await validateAutonomousToolSelectionSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

export class TransactionalJsonAutonomousToolSelectionPersistence extends JsonAutonomousToolSelectionPersistence {
  declare readonly textStore: AutonomousToolSelectionTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousToolSelectionTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("tool selection text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousToolSelectionSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest(expectedSnapshotDigest, "tool selection expected_snapshot_digest");
    const validated = await validateAutonomousToolSelectionSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

export class WebStorageAutonomousToolSelectionSnapshotTextStore implements AutonomousToolSelectionSnapshotTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("tool selection Web Storage adapter is malformed");
    if (typeof key !== "string" || key.length === 0 || key.length > 256 || key.includes("\u0000")) throw new ArgumentError("tool selection storage key is invalid");
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}
