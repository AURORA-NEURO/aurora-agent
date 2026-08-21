import { ArgumentError, isObject } from "./errors.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only persistence for evaluator-supervised workflow cycles. */
export const AUTONOMOUS_WORKFLOW_CYCLE_STATE_SCHEMA = "bioprism-typescript-autonomous-workflow-cycle-state/0.1" as const;
export const AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-workflow-cycle-snapshot/0.1" as const;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_REPLANS = 3;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_ATTEMPTS = AUTONOMOUS_WORKFLOW_CYCLE_MAX_REPLANS + 1;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_STATES = 4_096;
export const AUTONOMOUS_WORKFLOW_CYCLE_MAX_SNAPSHOT_BYTES = 64_000_000;

export class AutonomousWorkflowCyclePersistenceError extends ArgumentError {
  override readonly name = "AutonomousWorkflowCyclePersistenceError";
}

export type AutonomousWorkflowCyclePersistencePhase =
  | "execution_pending"
  | "evaluation_pending"
  | "settlement_pending"
  | "replan_handoff"
  | "terminal";

export interface AutonomousWorkflowCycleAttemptState extends JsonObject {
  attempt: number;
  job_id: string;
  execution_status: string;
  workflow_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evidence_digest: string | null;
  settlement_digest: string | null;
  learning_episode_ids: string[];
  replan_instruction_digest: string | null;
}

/** The persisted state deliberately contains no task text, prompts, outputs, evidence, or instructions. */
export interface AutonomousWorkflowCycleState extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_CYCLE_STATE_SCHEMA;
  cycle_id: string;
  task_digest: string;
  domain: string | null;
  root_job_id: string;
  current_job_id: string;
  max_replans: number;
  attempt: number;
  phase: AutonomousWorkflowCyclePersistencePhase;
  execution_status: string | null;
  workflow_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evidence_digest: string | null;
  replan_instruction_digest: string | null;
  terminal_status: string | null;
  attempts: AutonomousWorkflowCycleAttemptState[];
  evaluations: JsonObject[];
  learning_episode_ids: string[];
  settlement_digests: string[];
  trajectory_ids: string[];
  context_digests: string[];
  generation: number;
  previous_state_digest: string | null;
  state_digest: string;
  retention: "metadata_only_hash_chained_no_private_payloads";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowCycleStateStore {
  load(cycleId: string): Promise<AutonomousWorkflowCycleState | null> | AutonomousWorkflowCycleState | null;
  save(state: AutonomousWorkflowCycleState): Promise<void> | void;
}

export interface AutonomousWorkflowCycleSnapshot {
  schema: typeof AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA;
  states: AutonomousWorkflowCycleState[];
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousWorkflowCycleSnapshotStore extends AutonomousWorkflowCycleStateStore {
  snapshot(): Promise<AutonomousWorkflowCycleSnapshot>;
  restore(snapshot: AutonomousWorkflowCycleSnapshot): Promise<void> | void;
}

export interface AutonomousWorkflowCycleSnapshotPersistence {
  read(): Promise<AutonomousWorkflowCycleSnapshot | null> | AutonomousWorkflowCycleSnapshot | null;
  write(snapshot: AutonomousWorkflowCycleSnapshot): Promise<void> | void;
}

export interface AutonomousWorkflowCycleRehydrationContext {
  cycle_id: string;
  task_digest: string;
  root_job_id: string;
  current_job_id: string;
  attempt: number;
  phase: AutonomousWorkflowCyclePersistencePhase;
  workflow_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evidence_digest: string | null;
  replan_instruction_digest: string | null;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: unknown): number {
  let encoded: string;
  try { encoded = JSON.stringify(value); } catch { throw new AutonomousWorkflowCyclePersistenceError("workflow cycle metadata must be JSON serializable"); }
  if (typeof encoded !== "string") throw new AutonomousWorkflowCyclePersistenceError("workflow cycle metadata must be JSON serializable");
  return new TextEncoder().encode(encoded).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || /[\u0000-\u001F\u007F]/.test(value)) throw new AutonomousWorkflowCyclePersistenceError(`${name} must be bounded text`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new AutonomousWorkflowCyclePersistenceError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousWorkflowCyclePersistenceError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new AutonomousWorkflowCyclePersistenceError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  return value as number;
}

function assertKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const keys = new Set(allowed);
  if (Object.keys(value).some((key) => !keys.has(key))) throw new AutonomousWorkflowCyclePersistenceError(`${name} contains unsupported fields`);
}

function assertRequired(name: string, value: Record<string, unknown>, required: readonly string[]): void {
  if (required.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) throw new AutonomousWorkflowCyclePersistenceError(`${name} is missing required metadata fields`);
}

function inspectMetadata(value: unknown, path: string, depth = 0): void {
  if (depth > 16) throw new AutonomousWorkflowCyclePersistenceError(`${path} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 8_192) throw new AutonomousWorkflowCyclePersistenceError(`${path} contains too many rows`);
    for (let index = 0; index < value.length; index += 1) inspectMetadata(value[index], `${path}[${index}]`, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (/^(task|prompt|response|content|instruction|evidence|output|arguments?|credential|password|secret|token|payload|transcript)$/i.test(key)) {
      throw new AutonomousWorkflowCyclePersistenceError(`${path}.${key} is not allowed in metadata-only state`);
    }
    inspectMetadata(child, `${path}.${key}`, depth + 1);
  }
}

function digestDescriptor(value: AutonomousWorkflowCycleState): JsonObject {
  const { state_digest: _stateDigest, ...descriptor } = value;
  return descriptor;
}

function nullableDigestFields(value: Record<string, unknown>, label: string): void {
  for (const field of ["workflow_digest", "outcome_digest", "evaluation_digest", "evidence_digest", "settlement_digest", "replan_instruction_digest"] as const) {
    boundedDigest(`${label}.${field}`, value[field], true);
  }
}

function validateAttempt(value: unknown, index: number): AutonomousWorkflowCycleAttemptState {
  if (!isObject(value)) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle attempt ${index} must be an object`);
  const keys = ["attempt", "job_id", "execution_status", "workflow_digest", "outcome_digest", "evaluation_digest", "evidence_digest", "settlement_digest", "learning_episode_ids", "replan_instruction_digest"] as const;
  assertKeys(`workflow cycle attempt ${index}`, value, keys);
  assertRequired(`workflow cycle attempt ${index}`, value, keys);
  const attempt = boundedCount(`workflow cycle attempt ${index}.attempt`, value.attempt, AUTONOMOUS_WORKFLOW_CYCLE_MAX_ATTEMPTS, 1);
  const jobId = boundedIdentifier(`workflow cycle attempt ${index}.job_id`, value.job_id);
  const executionStatus = boundedIdentifier(`workflow cycle attempt ${index}.execution_status`, value.execution_status);
  nullableDigestFields(value, `workflow cycle attempt ${index}`);
  if (!Array.isArray(value.learning_episode_ids) || value.learning_episode_ids.length > 256) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle attempt ${index}.learning_episode_ids are malformed`);
  const learningEpisodeIds = value.learning_episode_ids.map((id, idIndex) => boundedIdentifier(`workflow cycle attempt ${index}.learning_episode_ids[${idIndex}]`, id));
  return {
    attempt,
    job_id: jobId,
    execution_status: executionStatus,
    workflow_digest: value.workflow_digest as string | null,
    outcome_digest: value.outcome_digest as string | null,
    evaluation_digest: value.evaluation_digest as string | null,
    evidence_digest: value.evidence_digest as string | null,
    settlement_digest: value.settlement_digest as string | null,
    learning_episode_ids: learningEpisodeIds,
    replan_instruction_digest: value.replan_instruction_digest as string | null,
  };
}

function validateEvaluation(value: unknown, index: number): JsonObject {
  if (!isObject(value)) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index} must be an object`);
  const keys = [
    "evaluation_digest", "evidence_digest", "evaluator_id", "evaluator_version", "status", "reward", "passed",
    "missing_signals", "rejected_signals", "replan_requested", "replan_instruction_digest", "feedback_digest", "failure_class",
    "retention", "secret_material",
  ] as const;
  assertKeys(`workflow cycle evaluation ${index}`, value, keys);
  assertRequired(`workflow cycle evaluation ${index}`, value, keys);
  boundedDigest(`workflow cycle evaluation ${index}.evaluation_digest`, value.evaluation_digest);
  boundedDigest(`workflow cycle evaluation ${index}.evidence_digest`, value.evidence_digest);
  boundedIdentifier(`workflow cycle evaluation ${index}.evaluator_id`, value.evaluator_id);
  boundedIdentifier(`workflow cycle evaluation ${index}.evaluator_version`, value.evaluator_version);
  boundedIdentifier(`workflow cycle evaluation ${index}.status`, value.status);
  if (typeof value.reward !== "number" || !Number.isFinite(value.reward) || value.reward < 0 || value.reward > 1) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index}.reward is outside [0, 1]`);
  if (typeof value.passed !== "boolean" || typeof value.replan_requested !== "boolean") throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index}.boolean fields are malformed`);
  for (const field of ["missing_signals", "rejected_signals"] as const) {
    if (!Array.isArray(value[field]) || value[field].length > 512) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index}.${field} is malformed`);
    value[field].forEach((item, itemIndex) => boundedText(`workflow cycle evaluation ${index}.${field}[${itemIndex}]`, item, 512));
  }
  boundedDigest(`workflow cycle evaluation ${index}.replan_instruction_digest`, value.replan_instruction_digest, true);
  boundedDigest(`workflow cycle evaluation ${index}.feedback_digest`, value.feedback_digest, true);
  if (value.failure_class !== null) boundedIdentifier(`workflow cycle evaluation ${index}.failure_class`, value.failure_class);
  if (value.retention !== "evaluator_values_and_digests_only" || value.secret_material !== "never_returned") throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index} retention markers are invalid`);
  if (value.replan_requested !== (value.replan_instruction_digest !== null)) throw new AutonomousWorkflowCyclePersistenceError(`workflow cycle evaluation ${index} replan fields are inconsistent`);
  return clone(value) as JsonObject;
}

/** Validate a cycle state before storage or rehydration. */
export async function validateAutonomousWorkflowCycleState(value: unknown): Promise<AutonomousWorkflowCycleState> {
  if (!isObject(value)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state must be an object");
  const keys = [
    "schema", "cycle_id", "task_digest", "domain", "root_job_id", "current_job_id", "max_replans", "attempt", "phase", "execution_status",
    "workflow_digest", "outcome_digest", "evaluation_digest", "evidence_digest", "replan_instruction_digest", "terminal_status", "attempts",
    "evaluations", "learning_episode_ids", "settlement_digests", "trajectory_ids", "context_digests", "generation", "previous_state_digest",
    "state_digest", "retention", "secret_material",
  ] as const;
  assertKeys("workflow cycle state", value, keys);
  assertRequired("workflow cycle state", value, keys);
  if (value.schema !== AUTONOMOUS_WORKFLOW_CYCLE_STATE_SCHEMA || value.retention !== "metadata_only_hash_chained_no_private_payloads" || value.secret_material !== "never_returned") throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state retention markers are invalid");
  const cycleId = boundedIdentifier("workflow cycle state cycle_id", value.cycle_id);
  const taskDigest = boundedDigest("workflow cycle state task_digest", value.task_digest)!;
  const domain = value.domain === null ? null : boundedIdentifier("workflow cycle state domain", value.domain);
  const rootJobId = boundedIdentifier("workflow cycle state root_job_id", value.root_job_id);
  const currentJobId = boundedIdentifier("workflow cycle state current_job_id", value.current_job_id);
  const maxReplans = boundedCount("workflow cycle state max_replans", value.max_replans, AUTONOMOUS_WORKFLOW_CYCLE_MAX_REPLANS);
  const attempt = boundedCount("workflow cycle state attempt", value.attempt, maxReplans + 1, 1);
  if (!["execution_pending", "evaluation_pending", "settlement_pending", "replan_handoff", "terminal"].includes(value.phase as string)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state phase is invalid");
  const phase = value.phase as AutonomousWorkflowCyclePersistencePhase;
  if (value.execution_status !== null) boundedIdentifier("workflow cycle state execution_status", value.execution_status);
  nullableDigestFields(value, "workflow cycle state");
  if (value.terminal_status !== null) boundedIdentifier("workflow cycle state terminal_status", value.terminal_status);
  if (!Array.isArray(value.attempts) || value.attempts.length > maxReplans + 1) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state attempts exceed capacity");
  const attempts = value.attempts.map(validateAttempt);
  const attemptNumbers = new Set<number>();
  for (const item of attempts) {
    if (attemptNumbers.has(item.attempt)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state attempts contain duplicates");
    attemptNumbers.add(item.attempt);
  }
  if (attempts.some((item) => item.attempt > attempt)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state contains a future attempt");
  if (attempts.some((item, index) => item.attempt !== index + 1)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state attempts are not contiguous");
  if (!Array.isArray(value.evaluations) || value.evaluations.length > maxReplans + 1) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state evaluations exceed capacity");
  const evaluations = value.evaluations.map(validateEvaluation);
  if (evaluations.length > attempts.length) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state has more evaluations than attempts");
  const ids = (name: string, candidate: unknown, maximum: number): string[] => {
    if (!Array.isArray(candidate) || candidate.length > maximum) throw new AutonomousWorkflowCyclePersistenceError(`${name} is malformed`);
    return candidate.map((item, index) => boundedIdentifier(`${name}[${index}]`, item));
  };
  const learningEpisodeIds = ids("workflow cycle state learning_episode_ids", value.learning_episode_ids, (maxReplans + 1) * 256);
  const settlementDigests = ids("workflow cycle state settlement_digests", value.settlement_digests, maxReplans + 1);
  settlementDigests.forEach((digest) => boundedDigest("workflow cycle state settlement_digest", digest));
  const trajectoryIds = ids("workflow cycle state trajectory_ids", value.trajectory_ids, maxReplans + 1);
  const contextDigests = ids("workflow cycle state context_digests", value.context_digests, maxReplans);
  contextDigests.forEach((digest) => boundedDigest("workflow cycle state context_digest", digest));
  const generation = boundedCount("workflow cycle state generation", value.generation, Number.MAX_SAFE_INTEGER, 1);
  const previousStateDigest = boundedDigest("workflow cycle state previous_state_digest", value.previous_state_digest, true);
  const stateDigest = boundedDigest("workflow cycle state state_digest", value.state_digest)!;
  if (phase === "replan_handoff" && (value.replan_instruction_digest === null || value.evaluation_digest === null)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle replan handoff requires evaluation and instruction digests");
  if (phase === "settlement_pending" && value.evaluation_digest === null) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle settlement pending requires an evaluation digest");
  if (phase === "terminal" && value.terminal_status === null) throw new AutonomousWorkflowCyclePersistenceError("terminal workflow cycle state requires a terminal status");
  inspectMetadata(value, "workflow cycle state");
  if (bytes(value) > 8_000_000) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state exceeds its byte capacity");
  if (await digestJson(digestDescriptor(value as unknown as AutonomousWorkflowCycleState)) !== stateDigest) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state digest does not match its metadata");
  return clone({ ...value, cycle_id: cycleId, task_digest: taskDigest, domain, root_job_id: rootJobId, current_job_id: currentJobId, max_replans: maxReplans, attempt, phase, attempts, evaluations, learning_episode_ids: learningEpisodeIds, settlement_digests: settlementDigests, trajectory_ids: trajectoryIds, context_digests: contextDigests, generation, previous_state_digest: previousStateDigest, state_digest: stateDigest }) as AutonomousWorkflowCycleState;
}

export async function sealAutonomousWorkflowCycleState(value: Omit<AutonomousWorkflowCycleState, "state_digest">): Promise<AutonomousWorkflowCycleState> {
  const descriptor = clone(value) as AutonomousWorkflowCycleState;
  return validateAutonomousWorkflowCycleState({ ...descriptor, state_digest: await digestJson(descriptor) });
}

/** Bounded in-memory implementation for tests and small workers. */
export class InMemoryAutonomousWorkflowCycleStateStore implements AutonomousWorkflowCycleSnapshotStore {
  private readonly states = new Map<string, AutonomousWorkflowCycleState>();

  async load(cycleId: string): Promise<AutonomousWorkflowCycleState | null> {
    return clone(this.states.get(boundedIdentifier("workflow cycle cycle_id", cycleId)) ?? null);
  }

  async save(raw: AutonomousWorkflowCycleState): Promise<void> {
    const state = await validateAutonomousWorkflowCycleState(raw);
    const prior = this.states.get(state.cycle_id);
    if (!prior && (state.generation !== 1 || state.previous_state_digest !== null)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle initial state must start at generation one");
    if (prior) {
      if (state.state_digest === prior.state_digest) return;
      if (state.generation !== prior.generation + 1 || state.previous_state_digest !== prior.state_digest) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle generation continuity check failed");
    }
    if (!prior && this.states.size >= AUTONOMOUS_WORKFLOW_CYCLE_MAX_STATES) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle state capacity exhausted");
    this.states.set(state.cycle_id, clone(state));
  }

  async snapshot(): Promise<AutonomousWorkflowCycleSnapshot> {
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA, states: [...this.states.values()].sort((left, right) => left.cycle_id.localeCompare(right.cycle_id)).map(clone), retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: await digestJson(descriptor) };
  }

  async restore(raw: AutonomousWorkflowCycleSnapshot): Promise<void> {
    const snapshot = await validateAutonomousWorkflowCycleSnapshot(raw);
    this.states.clear();
    for (const state of snapshot.states) this.states.set(state.cycle_id, clone(state));
  }
}

export async function validateAutonomousWorkflowCycleSnapshot(value: unknown): Promise<AutonomousWorkflowCycleSnapshot> {
  if (!isObject(value)) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot must be an object");
  assertKeys("workflow cycle snapshot", value, ["schema", "states", "retention", "secret_material", "snapshot_digest"]);
  if (value.schema !== AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA || value.retention !== "metadata_only_hash_bound" || value.secret_material !== "never_returned") throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot retention markers are invalid");
  if (!Array.isArray(value.states) || value.states.length > AUTONOMOUS_WORKFLOW_CYCLE_MAX_STATES) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot exceeds its state capacity");
  const states = await Promise.all(value.states.map((state) => validateAutonomousWorkflowCycleState(state)));
  if (new Set(states.map((state) => state.cycle_id)).size !== states.length) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot contains duplicate cycle IDs");
  const snapshotDigest = boundedDigest("workflow cycle snapshot snapshot_digest", value.snapshot_digest)!;
  const descriptor = { schema: value.schema, states, retention: value.retention, secret_material: value.secret_material };
  if (await digestJson(descriptor) !== snapshotDigest) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot digest does not match its metadata");
  if (bytes(value) > AUTONOMOUS_WORKFLOW_CYCLE_MAX_SNAPSHOT_BYTES) throw new AutonomousWorkflowCyclePersistenceError("workflow cycle snapshot exceeds its byte capacity");
  return clone({ ...descriptor, snapshot_digest: snapshotDigest }) as AutonomousWorkflowCycleSnapshot;
}

/** Coordinates a metadata-only cycle snapshot with caller-owned durable storage. */
export class AutonomousWorkflowCyclePersistenceCoordinator {
  constructor(readonly store: AutonomousWorkflowCycleSnapshotStore, readonly persistence: AutonomousWorkflowCycleSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new AutonomousWorkflowCyclePersistenceError("workflow cycle persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousWorkflowCyclePersistenceError("workflow cycle persistence requires readable and writable storage");
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA; bytes: number; snapshot_digest: string; retention: "metadata_only" }> {
    const snapshot = await validateAutonomousWorkflowCycleSnapshot(await this.store.snapshot());
    const snapshotBytes = bytes(snapshot);
    await this.persistence.write(snapshot);
    return { schema: AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA, bytes: snapshotBytes, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }

  async restore(): Promise<{ schema: typeof AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA; restored: boolean; cycles: number; attempts: number; snapshot_digest: string | null; retention: "metadata_only" }> {
    const raw = await this.persistence.read();
    if (raw === null) return { schema: AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA, restored: false, cycles: 0, attempts: 0, snapshot_digest: null, retention: "metadata_only" };
    const snapshot = await validateAutonomousWorkflowCycleSnapshot(raw);
    await this.store.restore(snapshot);
    return { schema: AUTONOMOUS_WORKFLOW_CYCLE_SNAPSHOT_SCHEMA, restored: true, cycles: snapshot.states.length, attempts: snapshot.states.reduce((total, state) => total + state.attempts.length, 0), snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
  }
}
