import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousPromptAdaptiveSelection,
  AutonomousPromptLearningSettlement,
  AutonomousPromptLearningState,
  AutonomousPromptRegistry,
  selectAdaptiveAutonomousPrompts,
  settleAutonomousPromptSelection,
  type AutonomousPromptAdaptiveSelectionJSON,
  type AutonomousPromptLearningStateJSON,
  type AutonomousPromptSelectionRequest,
} from "./autonomous-prompt-registry.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Restart-safe, value-only persistence for the adaptive prompt learner. */
export const AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-prompt-learning-snapshot/0.1" as const;
export const AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION = "value_only_prompt_learning_state_snapshot" as const;
export const MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES = 1_000_000;

export interface AutonomousPromptLearningSnapshotJSON extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA;
  snapshot_generation: number;
  previous_snapshot_digest: string | null;
  state: AutonomousPromptLearningStateJSON;
  retention: typeof AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION;
  secret_material: "never_returned";
  snapshot_digest: string;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function generation(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) throw new ArgumentError(`${name} must be a positive safe integer`);
  return value as number;
}

function exactKeys(value: object, expected: ReadonlySet<string>, name: string): void {
  if (Object.keys(value).some((key) => !expected.has(key)) || Object.keys(value).length !== expected.size) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function encodedBytes(value: unknown): number {
  return new TextEncoder().encode(canonicalJson(value)).byteLength;
}

export class AutonomousPromptLearningSnapshot {
  readonly schema = AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA;
  readonly state: AutonomousPromptLearningState;
  readonly snapshotGeneration: number;
  readonly previousSnapshotDigest: string | null;
  readonly retention = AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_RETENTION;
  readonly secretMaterial = "never_returned" as const;
  readonly snapshotDigest: string;

  constructor(options: { state: AutonomousPromptLearningState; snapshotGeneration?: number; previousSnapshotDigest?: string | null; snapshotDigest?: string }) {
    if (!(options?.state instanceof AutonomousPromptLearningState)) throw new ArgumentError("prompt learning snapshot state is malformed");
    this.state = options.state;
    this.snapshotGeneration = generation(options.snapshotGeneration ?? 1, "prompt learning snapshot_generation");
    this.previousSnapshotDigest = options.previousSnapshotDigest ?? null;
    if (this.previousSnapshotDigest !== null) digest("prompt learning previous_snapshot_digest", this.previousSnapshotDigest);
    if ((this.snapshotGeneration === 1) !== (this.previousSnapshotDigest === null)) throw new ArgumentError("prompt learning snapshot generation chain is malformed");
    const expected = digestJsonSync(this.descriptor());
    if (options.snapshotDigest !== undefined && digest("prompt learning snapshot_digest", options.snapshotDigest) !== expected) throw new ArgumentError("prompt learning snapshot digest does not match its contents");
    this.snapshotDigest = options.snapshotDigest ?? expected;
    Object.freeze(this);
  }

  get registryDigest(): string {
    return this.state.registryDigest;
  }

  private descriptor(): JsonObject {
    return {
      schema: this.schema,
      snapshot_generation: this.snapshotGeneration,
      previous_snapshot_digest: this.previousSnapshotDigest,
      state: this.state.toJSON(),
      retention: this.retention,
      secret_material: this.secretMaterial,
    };
  }

  toJSON(): AutonomousPromptLearningSnapshotJSON {
    return { ...this.descriptor(), snapshot_digest: this.snapshotDigest } as AutonomousPromptLearningSnapshotJSON;
  }

  static fromJSON(value: JsonObject): AutonomousPromptLearningSnapshot {
    if (!isObject(value) || value.schema !== AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_SCHEMA) throw new ArgumentError("prompt learning snapshot schema is invalid");
    exactKeys(value, new Set(["schema", "snapshot_generation", "previous_snapshot_digest", "state", "retention", "secret_material", "snapshot_digest"]), "prompt learning snapshot");
    if (!isObject(value.state)) throw new ArgumentError("prompt learning snapshot state is malformed");
    const state = AutonomousPromptLearningState.fromJSON(value.state);
    return new AutonomousPromptLearningSnapshot({
      state,
      snapshotGeneration: value.snapshot_generation as number,
      previousSnapshotDigest: value.previous_snapshot_digest as string | null,
      snapshotDigest: value.snapshot_digest as string,
    });
  }
}

export function snapshotAutonomousPromptLearning(state: AutonomousPromptLearningState, options: { snapshotGeneration?: number; previousSnapshotDigest?: string | null } = {}): AutonomousPromptLearningSnapshot {
  return new AutonomousPromptLearningSnapshot({ state, ...options });
}

export async function validateAutonomousPromptLearningSnapshot(raw: unknown): Promise<AutonomousPromptLearningSnapshot> {
  if (!isObject(raw)) throw new ArgumentError("prompt learning snapshot is malformed");
  const snapshot = AutonomousPromptLearningSnapshot.fromJSON(raw as unknown as JsonObject);
  if (encodedBytes(snapshot.toJSON()) > MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES) throw new ArgumentError("prompt learning snapshot exceeds its byte bound");
  return snapshot;
}

/**
 * Extract exact adaptive selections from high-level result envelopes.
 *
 * Only reviewed metadata fields are traversed. Provider responses, task text, rendered
 * messages, credentials, and connector payloads are never inspected or serialized.
 */
export function extractAutonomousPromptLearningSelections(
  result: unknown,
  registry: AutonomousPromptRegistry,
): readonly AutonomousPromptAdaptiveSelection[] {
  if (!(registry instanceof AutonomousPromptRegistry)) throw new ArgumentError("prompt learning selection extraction requires an AutonomousPromptRegistry");
  const found: AutonomousPromptAdaptiveSelection[] = [];
  const seen = new Set<string>();
  const visited = new Set<object>();
  let nodes = 0;
  const add = (raw: unknown): void => {
    if (!isObject(raw)) throw new ArgumentError("run adaptive prompt selection receipt is malformed");
    const selection = AutonomousPromptAdaptiveSelection.fromJSON(raw as JsonObject);
    registry.verifySelection(selection.plan);
    if (!seen.has(selection.selectionDigest)) {
      seen.add(selection.selectionDigest);
      found.push(selection);
    }
    if (found.length > 128) throw new ArgumentError("run adaptive prompt selection receipts exceed their bound");
  };
  const visit = (value: unknown): void => {
    nodes += 1;
    if (nodes > 512) throw new ArgumentError("run adaptive prompt selection envelope is too deep");
    if (value instanceof AutonomousPromptAdaptiveSelection) {
      add(value.toJSON());
      return;
    }
    if (Array.isArray(value)) {
      if (visited.has(value)) return;
      visited.add(value);
      value.forEach(visit);
      return;
    }
    if (!isObject(value)) return;
    if (visited.has(value)) return;
    visited.add(value);
    const prompt = value.prompt;
    if (isObject(prompt)) {
      if (prompt.adaptive_selection !== undefined) add(prompt.adaptive_selection);
      const autonomousPrompt = prompt.autonomous_prompt;
      if (isObject(autonomousPrompt) && autonomousPrompt.adaptive_selection !== undefined) add(autonomousPrompt.adaptive_selection);
    }
    if (value.adaptive_selection !== undefined) add(value.adaptive_selection);
    for (const key of ["planning", "plan_refinement", "child_runs", "child_results", "synthesis", "synthesis_result", "cross_domain", "attempts", "final_result", "final_execution", "results", "decision", "result", "stage_results"] as const) {
      const child = value[key];
      if (child !== undefined) visit(child);
    }
  };
  visit(result);
  return Object.freeze(found);
}

export interface AutonomousPromptLearningSnapshotPersistence {
  read(): Promise<AutonomousPromptLearningSnapshotJSON | null> | AutonomousPromptLearningSnapshotJSON | null;
  write(snapshot: AutonomousPromptLearningSnapshotJSON): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousPromptLearningSnapshotJSON): Promise<boolean> | boolean;
}

export interface AutonomousPromptLearningSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousPromptLearningTransactionalSnapshotTextStore extends AutonomousPromptLearningSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export class JsonAutonomousPromptLearningSnapshotPersistence implements AutonomousPromptLearningSnapshotPersistence {
  constructor(readonly textStore: AutonomousPromptLearningSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("prompt learning text store is malformed");
  }

  async read(): Promise<AutonomousPromptLearningSnapshotJSON | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_PROMPT_LEARNING_SNAPSHOT_BYTES) throw new ArgumentError("prompt learning JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("prompt learning JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("prompt learning JSON is not canonical");
    return (await validateAutonomousPromptLearningSnapshot(parsed)).toJSON();
  }

  async write(snapshot: AutonomousPromptLearningSnapshotJSON): Promise<void> {
    const validated = await validateAutonomousPromptLearningSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated.toJSON()));
  }
}

export class TransactionalJsonAutonomousPromptLearningSnapshotPersistence extends JsonAutonomousPromptLearningSnapshotPersistence {
  declare readonly textStore: AutonomousPromptLearningTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousPromptLearningTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("prompt learning text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousPromptLearningSnapshotJSON): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("prompt learning expected_snapshot_digest", expectedSnapshotDigest);
    const validated = await validateAutonomousPromptLearningSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated.toJSON()));
  }
}

export class WebStorageAutonomousPromptLearningSnapshotTextStore implements AutonomousPromptLearningSnapshotTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("prompt learning Web Storage adapter is malformed");
    if (!key || key.length > 256 || key.includes("\u0000")) throw new ArgumentError("prompt learning storage key is malformed");
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

export interface AutonomousPromptLearningSettlementOptions {
  armId: string;
  evaluatorId: string;
  evaluatorVersion: string;
  reward: number;
  passed: boolean;
  outcomeDigest?: string;
  settlementKey?: string;
}

export class AutonomousPromptLearningPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private snapshotGeneration = 0;
  private operationTail: Promise<void> = Promise.resolve();
  private currentState: AutonomousPromptLearningState;

  constructor(readonly registry: AutonomousPromptRegistry, options: { state?: AutonomousPromptLearningState | AutonomousPromptLearningStateJSON; persistence?: AutonomousPromptLearningSnapshotPersistence } = {}) {
    if (!(registry instanceof AutonomousPromptRegistry)) throw new ArgumentError("prompt learning persistence requires an AutonomousPromptRegistry");
    this.currentState = options.state === undefined ? new AutonomousPromptLearningState(registry.registryDigest) : options.state instanceof AutonomousPromptLearningState ? options.state : AutonomousPromptLearningState.fromJSON(options.state);
    if (this.currentState.registryDigest !== registry.registryDigest) throw new ArgumentError("prompt learning state is stale for the current registry");
    if (options.persistence !== undefined && (typeof options.persistence.read !== "function" || typeof options.persistence.write !== "function")) throw new ArgumentError("prompt learning persistence adapter is malformed");
    this.persistence = options.persistence;
  }

  readonly persistence?: AutonomousPromptLearningSnapshotPersistence;

  get state(): AutonomousPromptLearningState { return this.currentState; }

  select(requests: readonly AutonomousPromptSelectionRequest[], exploration = 0.35): AutonomousPromptAdaptiveSelection {
    return selectAdaptiveAutonomousPrompts(this.registry, requests, { state: this.currentState, exploration });
  }

  async restore(): Promise<AutonomousPromptLearningSnapshot | null> {
    return this.enqueue(async () => {
      if (!this.persistence) throw new ArgumentError("prompt learning restore requires persistence");
      const raw = await this.persistence.read();
      if (raw === null) { this.expectedSnapshotDigest = null; this.snapshotGeneration = 0; return null; }
      const snapshot = await validateAutonomousPromptLearningSnapshot(raw);
      if (snapshot.registryDigest !== this.registry.registryDigest) throw new ArgumentError("prompt learning snapshot is stale for the current registry");
      this.currentState = snapshot.state;
      this.expectedSnapshotDigest = snapshot.snapshotDigest;
      this.snapshotGeneration = snapshot.snapshotGeneration;
      return snapshot;
    });
  }

  async flush(): Promise<AutonomousPromptLearningSnapshot> {
    return this.enqueue(async () => this.flushUnlocked());
  }

  async settle(selection: AutonomousPromptAdaptiveSelection | AutonomousPromptAdaptiveSelectionJSON, options: AutonomousPromptLearningSettlementOptions): Promise<AutonomousPromptLearningSettlement> {
    return this.enqueue(async () => {
      const settlement = settleAutonomousPromptSelection(this.registry, this.currentState, selection, options);
      if (settlement.status === "replayed") return settlement;
      const prior = this.currentState;
      this.currentState = settlement.nextState;
      try { if (this.persistence) await this.flushUnlocked(); } catch (error) { this.currentState = prior; throw error; }
      return settlement;
    });
  }

  private async flushUnlocked(): Promise<AutonomousPromptLearningSnapshot> {
    if (!this.persistence) throw new ArgumentError("prompt learning flush requires persistence");
    const snapshot = snapshotAutonomousPromptLearning(this.currentState, { snapshotGeneration: this.snapshotGeneration + 1, previousSnapshotDigest: this.snapshotGeneration === 0 ? null : this.expectedSnapshotDigest });
    if (typeof this.persistence.writeIfUnchanged === "function") {
      if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot.toJSON())) throw new ArgumentError("prompt learning persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot.toJSON());
    this.expectedSnapshotDigest = snapshot.snapshotDigest;
    this.snapshotGeneration = snapshot.snapshotGeneration;
    return snapshot;
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(operation);
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
