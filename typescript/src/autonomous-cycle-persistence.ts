import { ArgumentError, isObject } from "./errors.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Durable metadata for evaluator-guided autonomous decision-cycle replanning. */
export const AUTONOMOUS_CYCLE_REPLAN_STATE_SCHEMA = "bioprism-typescript-autonomous-cycle-replan-state/0.2" as const;
export const AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-cycle-replan-snapshot/0.2" as const;
export const AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS = 3;
export const AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS = AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS + 1;
export const AUTONOMOUS_CYCLE_REPLAN_MAX_STATES = 4_096;
export const AUTONOMOUS_CYCLE_REPLAN_MAX_EVALUATIONS = AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS;
export const AUTONOMOUS_CYCLE_REPLAN_MAX_SNAPSHOT_BYTES = 64_000_000;

export class AutonomousCyclePersistenceError extends ArgumentError {
  override readonly name: string = "AutonomousCyclePersistenceError";
}

export type AutonomousCycleReplanMode = "single_domain" | "cross_domain";
export type AutonomousCycleReplanPhase =
  | "execution_pending"
  | "evaluation_pending"
  | "settlement_pending"
  | "replan_handoff"
  | "terminal";

/** A run projection is deliberately limited to digests and bounded status labels. */
export interface AutonomousCycleReplanAttemptState extends JsonObject {
  attempt: number;
  status: string;
  run_status: string | null;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  selection_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  learning_episode_ids: string[];
  trajectory_id: string | null;
}

/**
 * Caller-owned restart state. It never contains a task, prompt, provider response, tool
 * arguments, evaluator instruction, credentials, or learning payloads.
 */
export interface AutonomousCycleReplanState extends JsonObject {
  schema: typeof AUTONOMOUS_CYCLE_REPLAN_STATE_SCHEMA;
  cycle_id: string;
  task_digest: string;
  mode: AutonomousCycleReplanMode;
  max_replans: number;
  attempt: number;
  phase: AutonomousCycleReplanPhase;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  replan_instruction_digest: string | null;
  terminal_status: string | null;
  attempts: AutonomousCycleReplanAttemptState[];
  evaluations: JsonObject[];
  learning_episode_ids: string[];
  /** Optional for legacy snapshots; new cross-domain replan states persist this independent ledger. */
  response_learning_episode_ids?: string[];
  settlement_digests: string[];
  trajectory_ids: string[];
  context_digests: string[];
  generation: number;
  previous_state_digest: string | null;
  state_digest: string;
  retention: "metadata_only_hash_chained_no_private_payloads";
  secret_material: "never_returned";
}

export interface AutonomousCycleReplanStateStore {
  load(cycleId: string): Promise<AutonomousCycleReplanState | null> | AutonomousCycleReplanState | null;
  save(state: AutonomousCycleReplanState): Promise<void> | void;
}

export interface AutonomousCycleReplanSnapshot {
  schema: typeof AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA;
  states: AutonomousCycleReplanState[];
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousCycleReplanSnapshotStore extends AutonomousCycleReplanStateStore {
  snapshot(): Promise<AutonomousCycleReplanSnapshot>;
  restore(snapshot: AutonomousCycleReplanSnapshot): Promise<void> | void;
}

export interface AutonomousCycleReplanSnapshotPersistence {
  read(): Promise<AutonomousCycleReplanSnapshot | null> | AutonomousCycleReplanSnapshot | null;
  write(snapshot: AutonomousCycleReplanSnapshot): Promise<void> | void;
}

export interface AutonomousCycleReplanRehydrationContext {
  cycle_id: string;
  task_digest: string;
  mode: AutonomousCycleReplanMode;
  attempt: number;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  replan_instruction_digest: string | null;
}

export type AutonomousCycleReplanRunRehydrator<T> = (
  context: AutonomousCycleReplanRehydrationContext,
) => Promise<T> | T;

export type AutonomousCycleReplanRouteRehydrator<T> = (
  context: AutonomousCycleReplanRehydrationContext,
) => Promise<T> | T;

export type AutonomousCycleReplanEvaluationRehydrator = (
  context: AutonomousCycleReplanRehydrationContext,
) => Promise<unknown> | unknown;

export type AutonomousCycleReplanInstructionRehydrator = (
  context: AutonomousCycleReplanRehydrationContext,
) => Promise<string> | string;

function clone<T>(value: T): T {
  return structuredClone(value);
}

function jsonBytes(value: unknown): number {
  let serialized: string;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new AutonomousCyclePersistenceError("autonomous cycle metadata must be JSON serializable");
  }
  if (typeof serialized !== "string") throw new AutonomousCyclePersistenceError("autonomous cycle metadata must be JSON serializable");
  return new TextEncoder().encode(serialized).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || /[\u0000-\u001F\u007F]/.test(value)) throw new AutonomousCyclePersistenceError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousCyclePersistenceError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousCyclePersistenceError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new AutonomousCyclePersistenceError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function boundedReward(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new AutonomousCyclePersistenceError(`${name} must be within [0, 1]`);
  return value;
}

function assertKnownKeys(name: string, value: Record<string, unknown>, keys: readonly string[]): void {
  const allowed = new Set(keys);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousCyclePersistenceError(`${name} contains unsupported or non-metadata fields`);
}

function inspectMetadata(value: unknown, path: string, depth = 0): void {
  if (depth > 16) throw new AutonomousCyclePersistenceError(`${path} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 8_192) throw new AutonomousCyclePersistenceError(`${path} contains too many rows`);
    for (let index = 0; index < value.length; index += 1) inspectMetadata(value[index], `${path}[${index}]`, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (/^(arguments?|output|response|prompt|content|instruction|credential|password|secret(?!_material)|token|payload|result)$/i.test(key)) throw new AutonomousCyclePersistenceError(`${path}.${key} is not allowed in metadata-only state`);
    inspectMetadata(child, `${path}.${key}`, depth + 1);
  }
}

function assertNoCredentialShape(value: unknown, path: string): void {
  let serialized: string;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new AutonomousCyclePersistenceError(`${path} must be JSON serializable`);
  }
  if (typeof serialized === "string" && /(?:api[_-]?key|authorization|bearer|password|private[_-]?key|access[_-]?token|refresh[_-]?token|credential|\bgsk_|\bsk-proj-|\bsk-[A-Za-z0-9]{16,})/i.test(serialized)) throw new AutonomousCyclePersistenceError(`${path} contains credential-shaped material`);
}

function stateDescriptor(state: AutonomousCycleReplanState): JsonObject {
  const { state_digest: _stateDigest, ...descriptor } = state;
  return descriptor;
}

function attemptDescriptor(value: AutonomousCycleReplanAttemptState): JsonObject {
  return {
    attempt: value.attempt,
    status: value.status,
    run_status: value.run_status,
    route_digest: value.route_digest,
    plan_refinement_digest: value.plan_refinement_digest,
    selection_digest: value.selection_digest,
    outcome_digest: value.outcome_digest,
    evaluation_digest: value.evaluation_digest,
    learning_episode_ids: [...value.learning_episode_ids],
    trajectory_id: value.trajectory_id,
  };
}

function validateAttempt(value: unknown, index: number): AutonomousCycleReplanAttemptState {
  if (!isObject(value)) throw new AutonomousCyclePersistenceError(`autonomous cycle attempt ${index} must be an object`);
  const attempt = value as unknown as AutonomousCycleReplanAttemptState;
  assertKnownKeys(`autonomous cycle attempt ${index}`, attempt, ["attempt", "status", "run_status", "route_digest", "plan_refinement_digest", "selection_digest", "outcome_digest", "evaluation_digest", "learning_episode_ids", "trajectory_id"]);
  boundedCount(`autonomous cycle attempt ${index}.attempt`, attempt.attempt, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS, 1);
  boundedIdentifier(`autonomous cycle attempt ${index}.status`, attempt.status);
  if (attempt.run_status !== null) boundedIdentifier(`autonomous cycle attempt ${index}.run_status`, attempt.run_status);
  boundedDigest(`autonomous cycle attempt ${index}.route_digest`, attempt.route_digest, true);
  boundedDigest(`autonomous cycle attempt ${index}.plan_refinement_digest`, attempt.plan_refinement_digest, true);
  boundedDigest(`autonomous cycle attempt ${index}.selection_digest`, attempt.selection_digest, true);
  boundedDigest(`autonomous cycle attempt ${index}.outcome_digest`, attempt.outcome_digest, true);
  boundedDigest(`autonomous cycle attempt ${index}.evaluation_digest`, attempt.evaluation_digest, true);
  if (!Array.isArray(attempt.learning_episode_ids) || attempt.learning_episode_ids.length > 256) throw new AutonomousCyclePersistenceError(`autonomous cycle attempt ${index}.learning_episode_ids are malformed`);
  for (const episodeId of attempt.learning_episode_ids) boundedIdentifier(`autonomous cycle attempt ${index}.learning_episode_id`, episodeId);
  if (attempt.trajectory_id !== null) boundedIdentifier(`autonomous cycle attempt ${index}.trajectory_id`, attempt.trajectory_id);
  return clone(attempt);
}

/** Validate state before it crosses a process or storage boundary. */
export async function validateAutonomousCycleReplanState(value: unknown): Promise<AutonomousCycleReplanState> {
  if (!isObject(value)) throw new AutonomousCyclePersistenceError("autonomous cycle state must be an object");
  const state = value as unknown as AutonomousCycleReplanState;
  assertKnownKeys("autonomous cycle state", state, ["schema", "cycle_id", "task_digest", "mode", "max_replans", "attempt", "phase", "route_digest", "plan_refinement_digest", "outcome_digest", "evaluation_digest", "replan_instruction_digest", "terminal_status", "attempts", "evaluations", "learning_episode_ids", "response_learning_episode_ids", "settlement_digests", "trajectory_ids", "context_digests", "generation", "previous_state_digest", "state_digest", "retention", "secret_material"]);
  if (state.schema !== AUTONOMOUS_CYCLE_REPLAN_STATE_SCHEMA || state.retention !== "metadata_only_hash_chained_no_private_payloads" || state.secret_material !== "never_returned") throw new AutonomousCyclePersistenceError("autonomous cycle state retention markers are invalid");
  boundedIdentifier("autonomous cycle state cycle_id", state.cycle_id);
  boundedDigest("autonomous cycle state task_digest", state.task_digest);
  if (state.mode !== "single_domain" && state.mode !== "cross_domain") throw new AutonomousCyclePersistenceError("autonomous cycle state mode is invalid");
  boundedCount("autonomous cycle state max_replans", state.max_replans, AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS);
  boundedCount("autonomous cycle state attempt", state.attempt, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS, 1);
  if (!(state.phase === "execution_pending" || state.phase === "evaluation_pending" || state.phase === "settlement_pending" || state.phase === "replan_handoff" || state.phase === "terminal")) throw new AutonomousCyclePersistenceError("autonomous cycle state phase is invalid");
  boundedDigest("autonomous cycle state route_digest", state.route_digest, true);
  boundedDigest("autonomous cycle state plan_refinement_digest", state.plan_refinement_digest, true);
  boundedDigest("autonomous cycle state outcome_digest", state.outcome_digest, true);
  boundedDigest("autonomous cycle state evaluation_digest", state.evaluation_digest, true);
  boundedDigest("autonomous cycle state replan_instruction_digest", state.replan_instruction_digest, true);
  if (state.terminal_status !== null) boundedIdentifier("autonomous cycle state terminal_status", state.terminal_status);
  if (!Array.isArray(state.attempts) || state.attempts.length > AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS) throw new AutonomousCyclePersistenceError("autonomous cycle state attempts exceed capacity");
  if (!Array.isArray(state.evaluations) || state.evaluations.length > AUTONOMOUS_CYCLE_REPLAN_MAX_EVALUATIONS) throw new AutonomousCyclePersistenceError("autonomous cycle state evaluations exceed capacity");
  const attempts = state.attempts.map(validateAttempt);
  const seenAttempts = new Set<number>();
  for (const attempt of attempts) {
    if (seenAttempts.has(attempt.attempt) || attempt.attempt > state.attempt) throw new AutonomousCyclePersistenceError("autonomous cycle state contains duplicate or future attempts");
    seenAttempts.add(attempt.attempt);
  }
  if (attempts.some((attempt, index) => attempt.attempt !== index + 1)) throw new AutonomousCyclePersistenceError("autonomous cycle attempts are not contiguous");
  for (let index = 0; index < state.evaluations.length; index += 1) {
    const evaluation = state.evaluations[index];
    if (!isObject(evaluation)) throw new AutonomousCyclePersistenceError(`autonomous cycle evaluation ${index} must be an object`);
    const evaluationKeys = state.mode === "cross_domain"
      ? ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest", "reward_episode_count", "replan_requested", "replan_instruction_digest"]
      : ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest", "replan_requested", "replan_instruction_digest"];
    assertKnownKeys(`autonomous cycle evaluation ${index}`, evaluation, evaluationKeys);
    boundedIdentifier(`autonomous cycle evaluation ${index}.evaluator_id`, evaluation.evaluator_id);
    boundedIdentifier(`autonomous cycle evaluation ${index}.evaluator_version`, evaluation.evaluator_version);
    boundedReward(`autonomous cycle evaluation ${index}.reward`, evaluation.reward);
    if (typeof evaluation.passed !== "boolean" || typeof evaluation.failed !== "boolean" || typeof evaluation.replan_requested !== "boolean") throw new AutonomousCyclePersistenceError(`autonomous cycle evaluation ${index} boolean fields are malformed`);
    if (evaluation.feedback_digest === undefined || evaluation.failure_class === undefined || evaluation.evidence_digest === undefined || evaluation.replan_instruction_digest === undefined) throw new AutonomousCyclePersistenceError(`autonomous cycle evaluation ${index} nullable fields are missing`);
    boundedDigest(`autonomous cycle evaluation ${index}.feedback_digest`, evaluation.feedback_digest, true);
    if (evaluation.failure_class !== null) boundedIdentifier(`autonomous cycle evaluation ${index}.failure_class`, evaluation.failure_class);
    boundedDigest(`autonomous cycle evaluation ${index}.evidence_digest`, evaluation.evidence_digest, true);
    boundedDigest(`autonomous cycle evaluation ${index}.replan_instruction_digest`, evaluation.replan_instruction_digest, true);
    if (evaluation.replan_requested !== (evaluation.replan_instruction_digest !== null)) throw new AutonomousCyclePersistenceError(`autonomous cycle evaluation ${index} instruction state is inconsistent`);
    if (state.mode === "cross_domain") boundedCount(`autonomous cycle evaluation ${index}.reward_episode_count`, evaluation.reward_episode_count, 32);
    inspectMetadata(evaluation, `autonomous cycle evaluation ${index}`);
    if (jsonBytes(evaluation) > 4_000_000) throw new AutonomousCyclePersistenceError(`autonomous cycle evaluation ${index} exceeds its metadata budget`);
  }
  const validateIds = (name: string, value: unknown, maximum: number): string[] => {
    if (!Array.isArray(value) || value.length > maximum) throw new AutonomousCyclePersistenceError(`${name} is malformed`);
    for (const item of value) boundedIdentifier(name, item);
    return [...value] as string[];
  };
  const learningEpisodeIds = validateIds("autonomous cycle learning_episode_ids", state.learning_episode_ids, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS * 256);
  const responseLearningEpisodeIds = state.response_learning_episode_ids === undefined ? undefined : validateIds("autonomous cycle response_learning_episode_ids", state.response_learning_episode_ids, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS * 256);
  const settlementDigests = validateIds("autonomous cycle settlement_digests", state.settlement_digests, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS);
  for (const digest of settlementDigests) boundedDigest("autonomous cycle settlement_digest", digest);
  const trajectoryIds = validateIds("autonomous cycle trajectory_ids", state.trajectory_ids, AUTONOMOUS_CYCLE_REPLAN_MAX_ATTEMPTS);
  const contextDigests = [...(Array.isArray(state.context_digests) ? state.context_digests : [])];
  if (contextDigests.length > AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS) throw new AutonomousCyclePersistenceError("autonomous cycle context_digests exceed capacity");
  for (const digest of contextDigests) boundedDigest("autonomous cycle context_digest", digest);
  boundedCount("autonomous cycle generation", state.generation, Number.MAX_SAFE_INTEGER, 1);
  boundedDigest("autonomous cycle previous_state_digest", state.previous_state_digest, true);
  boundedDigest("autonomous cycle state_digest", state.state_digest);
  if (state.phase === "replan_handoff" && (state.replan_instruction_digest === null || state.evaluation_digest === null)) throw new AutonomousCyclePersistenceError("autonomous cycle replan handoff requires evaluation and instruction digests");
  if (state.phase === "settlement_pending" && state.evaluation_digest === null) throw new AutonomousCyclePersistenceError("autonomous cycle settlement pending requires an evaluation digest");
  if (state.phase === "terminal" && state.terminal_status === null) throw new AutonomousCyclePersistenceError("terminal autonomous cycle state requires a terminal status");
  if (state.evaluations.length > state.attempts.length) throw new AutonomousCyclePersistenceError("autonomous cycle has more evaluations than attempts");
  inspectMetadata(state, "autonomous cycle state");
  assertNoCredentialShape(state, "autonomous cycle state");
  if (jsonBytes(state) > 8_000_000) throw new AutonomousCyclePersistenceError("autonomous cycle state exceeds its metadata budget");
  if (await digestJson(stateDescriptor(state)) !== state.state_digest) throw new AutonomousCyclePersistenceError("autonomous cycle state digest does not match its metadata");
  return clone({ ...state, attempts, learning_episode_ids: learningEpisodeIds, ...(responseLearningEpisodeIds === undefined ? {} : { response_learning_episode_ids: responseLearningEpisodeIds }), settlement_digests: settlementDigests, trajectory_ids: trajectoryIds, context_digests: contextDigests });
}

/** Seal a state descriptor with its content digest before saving it. */
export async function sealAutonomousCycleReplanState(value: Omit<AutonomousCycleReplanState, "state_digest">): Promise<AutonomousCycleReplanState> {
  const descriptor = clone(value) as AutonomousCycleReplanState;
  const state = { ...descriptor, state_digest: await digestJson(descriptor) };
  return validateAutonomousCycleReplanState(state);
}

/** In-memory reference store implementing optimistic generation and hash-chain checks. */
export class InMemoryAutonomousCycleReplanStateStore implements AutonomousCycleReplanSnapshotStore {
  private readonly states = new Map<string, AutonomousCycleReplanState>();

  async load(cycleId: string): Promise<AutonomousCycleReplanState | null> {
    return clone(this.states.get(boundedIdentifier("cycle_id", cycleId)) ?? null);
  }

  async save(raw: AutonomousCycleReplanState): Promise<void> {
    const state = await validateAutonomousCycleReplanState(raw);
    const prior = this.states.get(state.cycle_id);
    if (!prior && (state.generation !== 1 || state.previous_state_digest !== null)) throw new AutonomousCyclePersistenceError("autonomous cycle initial state must start at generation one");
    if (prior) {
      if (state.state_digest === prior.state_digest) return;
      if (state.generation !== prior.generation + 1 || state.previous_state_digest !== prior.state_digest) throw new AutonomousCyclePersistenceError("autonomous cycle state generation continuity check failed");
    }
    if (!prior && this.states.size >= AUTONOMOUS_CYCLE_REPLAN_MAX_STATES) throw new AutonomousCyclePersistenceError("autonomous cycle state store capacity exhausted");
    this.states.set(state.cycle_id, clone(state));
  }

  async snapshot(): Promise<AutonomousCycleReplanSnapshot> {
    const states = [...this.states.values()].sort((left, right) => left.cycle_id.localeCompare(right.cycle_id)).map(clone);
    const descriptor = { schema: AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA, states, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: await digestJson(descriptor) };
  }

  async restore(raw: AutonomousCycleReplanSnapshot): Promise<void> {
    const snapshot = await validateAutonomousCycleReplanSnapshot(raw);
    this.states.clear();
    for (const state of snapshot.states) this.states.set(state.cycle_id, clone(state));
  }
}

export async function validateAutonomousCycleReplanSnapshot(value: unknown): Promise<AutonomousCycleReplanSnapshot> {
  if (!isObject(value)) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot must be an object");
  const snapshot = value as unknown as AutonomousCycleReplanSnapshot;
  assertKnownKeys("autonomous cycle snapshot", snapshot as unknown as Record<string, unknown>, ["schema", "states", "retention", "secret_material", "snapshot_digest"]);
  if (snapshot.schema !== AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA || snapshot.retention !== "metadata_only_hash_bound" || snapshot.secret_material !== "never_returned") throw new AutonomousCyclePersistenceError("autonomous cycle snapshot retention markers are invalid");
  if (!Array.isArray(snapshot.states) || snapshot.states.length > AUTONOMOUS_CYCLE_REPLAN_MAX_STATES) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot exceeds its state capacity");
  const states: AutonomousCycleReplanState[] = [];
  const ids = new Set<string>();
  for (const raw of snapshot.states) {
    const state = await validateAutonomousCycleReplanState(raw);
    if (ids.has(state.cycle_id)) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot contains duplicate cycle IDs");
    ids.add(state.cycle_id);
    states.push(state);
  }
  boundedDigest("autonomous cycle snapshot snapshot_digest", snapshot.snapshot_digest);
  const descriptor = { schema: snapshot.schema, states, retention: snapshot.retention, secret_material: snapshot.secret_material };
  if (await digestJson(descriptor) !== snapshot.snapshot_digest) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot digest does not match its metadata");
  if (jsonBytes(snapshot) > AUTONOMOUS_CYCLE_REPLAN_MAX_SNAPSHOT_BYTES) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot exceeds its byte capacity");
  return clone({ ...snapshot, states });
}

/** Coordinates a metadata-only cycle snapshot with a caller-owned durable adapter. */
export class AutonomousCycleReplanPersistenceCoordinator {
  constructor(readonly store: AutonomousCycleReplanSnapshotStore, readonly persistence: AutonomousCycleReplanSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new AutonomousCyclePersistenceError("autonomous cycle persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousCyclePersistenceError("autonomous cycle persistence requires readable and writable storage");
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA; bytes: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await this.store.snapshot();
    const validated = await validateAutonomousCycleReplanSnapshot(snapshot);
    const bytes = jsonBytes(validated);
    if (bytes > AUTONOMOUS_CYCLE_REPLAN_MAX_SNAPSHOT_BYTES) throw new AutonomousCyclePersistenceError("autonomous cycle snapshot exceeds its byte capacity");
    await this.persistence.write(validated);
    return { schema: AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA, bytes, snapshot_digest: validated.snapshot_digest, retention: "metadata_only" };
  }

  async restore(): Promise<{ schema: typeof AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA; restored: boolean; cycles: number; attempts: number; snapshot_digest: string | null; retention: "metadata_only" }> {
    const raw = await this.persistence.read();
    if (raw === null) return { schema: AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA, restored: false, cycles: 0, attempts: 0, snapshot_digest: null, retention: "metadata_only" };
    const snapshot = await validateAutonomousCycleReplanSnapshot(raw);
    await this.store.restore(snapshot);
    return { schema: AUTONOMOUS_CYCLE_REPLAN_SNAPSHOT_SCHEMA, restored: true, cycles: snapshot.states.length, attempts: snapshot.states.reduce((total, state) => total + state.attempts.length, 0), snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }
}
