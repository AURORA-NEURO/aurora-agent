import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Durable, metadata-only state for the outer autonomous goal control loop. */
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA = "bioprism-autonomous-goal-control-checkpoint/0.1" as const;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION = "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES = 128;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS = 8_192;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_EVALUATIONS = 128;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SIGNALS = 4_096;
export const AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES = 2_000_000;

const BANDIT_SCHEMA = "bioprism-autonomous-goal-control-bandit/0.1";
const BANDIT_RETENTIONS = new Set(["value_only_goal_domain_bandit_state", "value_only_goal_contextual_bandit_state"]);
const STOP_REASONS = new Set(["all_terminal", "no_admissible_work", "cycle_budget_exhausted", "run_budget_exhausted"]);

export type AutonomousGoalControlLoopCheckpointStopReason = "all_terminal" | "no_admissible_work" | "cycle_budget_exhausted" | "run_budget_exhausted";

export interface AutonomousGoalControlLoopCheckpoint extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA;
  run_id: string;
  next_cycle: number;
  cycle_summaries: JsonObject[];
  previous_cycle: JsonObject | null;
  completed_cycles: number;
  total_selected: number;
  total_claimed: number;
  total_runs: number;
  status_counts: Record<string, number>;
  domain_counts: Record<string, number>;
  evaluation_count: number;
  evaluation_digests: string[];
  learning_state_digest: string | null;
  learned_signals: JsonObject[];
  learner_state: JsonObject | null;
  stop_reason: AutonomousGoalControlLoopCheckpointStopReason;
  generation: number;
  previous_snapshot_digest: string | null;
  retention: typeof AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION;
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousGoalControlLoopSnapshotTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface TransactionalAutonomousGoalControlLoopSnapshotTextStore extends AutonomousGoalControlLoopSnapshotTextStore {
  write_if_unchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface AutonomousGoalControlLoopSnapshotPersistence {
  read(): AutonomousGoalControlLoopCheckpoint | null | Promise<AutonomousGoalControlLoopCheckpoint | null>;
  write(snapshot: AutonomousGoalControlLoopCheckpoint): void | Promise<void>;
}

export interface TransactionalAutonomousGoalControlLoopSnapshotPersistence extends AutonomousGoalControlLoopSnapshotPersistence {
  write_if_unchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalControlLoopCheckpoint): boolean | Promise<boolean>;
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal control checkpoint ${message}`);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function jsonBytes(value: unknown): number {
  return new TextEncoder().encode(canonicalJson(value)).byteLength;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bounds`);
  return value;
}

function text(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) fail(`${name} is outside its text bounds`);
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const result = text(name, value, maximum);
  if (!/^[A-Za-z0-9_.:/-]+$/.test(result)) fail(`${name} contains unsupported identifier characters`);
  return result;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function numberValue(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function exactKeys(name: string, value: Record<string, unknown>, expected: readonly string[]): void {
  const allowed = new Set(expected);
  if (Object.keys(value).some((key) => !allowed.has(key)) || expected.some((key) => !(key in value))) fail(`${name} has unsupported or missing fields`);
}

function counts(name: string, value: unknown, maximum: number): Record<string, number> {
  if (!isObject(value) || Object.keys(value).length > 128) fail(`${name} is malformed`);
  const result: Record<string, number> = {};
  for (const [key, raw] of Object.entries(value)) result[identifier(`${name} key`, key, 128)] = integer(`${name} value`, raw, 0, maximum);
  return Object.fromEntries(Object.entries(result).sort(([left], [right]) => left.localeCompare(right)));
}

function signal(value: unknown, index: number): JsonObject {
  if (!isObject(value)) fail(`signal ${index} is malformed`);
  exactKeys(`signal ${index}`, value, ["goal_id", "priority", "urgency", "deadline_ns", "estimated_cost", "dependencies"]);
  if (!Array.isArray(value.dependencies) || value.dependencies.length > 64) fail(`signal ${index} dependencies are outside their bounds`);
  const dependencies = [...new Set(value.dependencies.map((item, dependencyIndex) => identifier(`signal ${index} dependency ${dependencyIndex}`, item)))].sort();
  const deadline = value.deadline_ns === null ? null : integer(`signal ${index} deadline_ns`, value.deadline_ns, 0, Number.MAX_SAFE_INTEGER);
  return {
    goal_id: identifier(`signal ${index} goal_id`, value.goal_id),
    priority: numberValue(`signal ${index} priority`, value.priority, 0, 1),
    urgency: numberValue(`signal ${index} urgency`, value.urgency, 0, 1),
    deadline_ns: deadline,
    estimated_cost: integer(`signal ${index} estimated_cost`, value.estimated_cost, 1, 1_000_000),
    dependencies,
  };
}

function cycleSummary(value: unknown, name: string): JsonObject {
  if (!isObject(value)) fail(`${name} is malformed`);
  const optional = new Set(["evaluated", "evaluation_digest", "learning_state_digest", "signals_digest"]);
  const required = ["cycle", "schedule_digest", "claim_digest", "worker_digest", "selected", "claimed", "runs", "counts", "selected_domains", "missing_domains", "retention", "secret_material"];
  if (Object.keys(value).some((key) => !required.includes(key) && !optional.has(key)) || required.some((key) => !(key in value))) fail(`${name} has unsupported or missing fields`);
  if (value.retention !== "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained" || value.secret_material !== "never_returned") fail(`${name} retention markers are invalid`);
  const selected = integer(`${name}.selected`, value.selected, 0, 128);
  const claimed = integer(`${name}.claimed`, value.claimed, 0, 128);
  const runs = integer(`${name}.runs`, value.runs, 0, 128);
  if (claimed > selected || runs > claimed) fail(`${name} counts are inconsistent`);
  if (!Array.isArray(value.selected_domains) || value.selected_domains.length > 128 || !Array.isArray(value.missing_domains) || value.missing_domains.length > 128) fail(`${name} domain lists are malformed`);
  const result: JsonObject = {
    cycle: integer(`${name}.cycle`, value.cycle, 1, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES),
    schedule_digest: digest(`${name}.schedule_digest`, value.schedule_digest)!,
    claim_digest: digest(`${name}.claim_digest`, value.claim_digest, true),
    worker_digest: digest(`${name}.worker_digest`, value.worker_digest)!,
    selected,
    claimed,
    runs,
    counts: counts(`${name}.counts`, value.counts, 128),
    selected_domains: value.selected_domains.map((item, index) => identifier(`${name}.selected_domain ${index}`, item, 128)),
    missing_domains: value.missing_domains.map((item, index) => identifier(`${name}.missing_domain ${index}`, item, 128)),
    retention: value.retention as string,
    secret_material: value.secret_material as string,
  };
  if ("evaluated" in value) {
    result.evaluated = integer(`${name}.evaluated`, value.evaluated, 0, 128);
    result.evaluation_digest = digest(`${name}.evaluation_digest`, value.evaluation_digest)!;
  }
  for (const field of ["learning_state_digest", "signals_digest"] as const) if (field in value) result[field] = digest(`${name}.${field}`, value[field])!;
  return result;
}

function learnerState(value: unknown): JsonObject {
  if (!isObject(value)) fail("learner_state must be an object or null");
  exactKeys("learner_state", value, ["schema", "generation", "arms", "exploration", "retention", "secret_material", "state_digest"]);
  if (value.schema !== BANDIT_SCHEMA || !BANDIT_RETENTIONS.has(String(value.retention)) || value.secret_material !== "never_returned") fail("learner_state markers are invalid");
  if (!Array.isArray(value.arms) || value.arms.length > 128) fail("learner_state arms are outside their bounds");
  const seen = new Set<string>();
  const arms = value.arms.map((raw, index) => {
    if (!isObject(raw)) fail(`learner_state arm ${index} is malformed`);
    const requiredArmFields = new Set(["domain", "pulls", "failures", "reward_sum"]);
    const optionalContextFields = new Set(["capability", "risk_class", "arm_id"]);
    if (Object.keys(raw).some((key) => !requiredArmFields.has(key) && !optionalContextFields.has(key)) || [...requiredArmFields].some((key) => !(key in raw))) fail(`learner_state arm ${index} has unsupported or missing fields`);
    const domain = identifier(`learner_state arm ${index}.domain`, raw.domain, 128);
    const capability = raw.capability === undefined || raw.capability === null ? null : text(`learner_state arm ${index}.capability`, raw.capability, 128);
    const riskClass = raw.risk_class === undefined || raw.risk_class === null ? null : text(`learner_state arm ${index}.risk_class`, raw.risk_class, 128);
    const expectedArmId = capability === null && riskClass === null
      ? domain
      : digestJsonSync({ schema: `${BANDIT_SCHEMA}/context-arm`, domain, capability, risk_class: riskClass });
    const armId = raw.arm_id === undefined || raw.arm_id === null ? null : digest(`learner_state arm ${index}.arm_id`, raw.arm_id);
    if (armId !== expectedArmId && !(armId === null && expectedArmId === domain)) fail(`learner_state arm ${index}.arm_id does not match its context`);
    const armKey = expectedArmId;
    if (seen.has(armKey)) fail("learner_state contains duplicate contextual arms");
    seen.add(armKey);
    const pulls = integer(`learner_state arm ${index}.pulls`, raw.pulls, 0, 2_147_483_647);
    const failures = integer(`learner_state arm ${index}.failures`, raw.failures, 0, 2_147_483_647);
    if (failures > pulls) fail(`learner_state arm ${index} failures exceed pulls`);
    const normalizedArm: JsonObject = { domain, pulls, failures, reward_sum: numberValue(`learner_state arm ${index}.reward_sum`, raw.reward_sum, -pulls, pulls) };
    if (capability !== null || riskClass !== null) {
      normalizedArm.capability = capability;
      normalizedArm.risk_class = riskClass;
      normalizedArm.arm_id = armKey;
    }
    return normalizedArm;
  }).sort((left, right) => String(left.arm_id ?? left.domain).localeCompare(String(right.arm_id ?? right.domain)));
  const body: JsonObject = {
    schema: value.schema,
    generation: integer("learner_state.generation", value.generation, 0, 2_147_483_647),
    arms,
    exploration: numberValue("learner_state.exploration", value.exploration, 0, 2),
    retention: value.retention as string,
    secret_material: value.secret_material as string,
  };
  if (digest("learner_state.state_digest", value.state_digest)! !== digestJsonSync(body)) fail("learner_state digest mismatch");
  return { ...body, state_digest: value.state_digest as string };
}

function normalize(value: JsonObject, requireDigest: boolean): JsonObject {
  const fields = ["schema", "run_id", "next_cycle", "cycle_summaries", "previous_cycle", "completed_cycles", "total_selected", "total_claimed", "total_runs", "status_counts", "domain_counts", "evaluation_count", "evaluation_digests", "learning_state_digest", "learned_signals", "learner_state", "stop_reason", "generation", "previous_snapshot_digest", "retention", "secret_material"];
  exactKeys("snapshot", value, requireDigest ? [...fields, "snapshot_digest"] : fields);
  if (value.schema !== AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA || value.retention !== AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_RETENTION || value.secret_material !== "never_returned") fail("snapshot markers are invalid");
  const completed = integer("completed_cycles", value.completed_cycles, 0, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES);
  const next = integer("next_cycle", value.next_cycle, 1, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES + 1);
  if (next !== completed + 1) fail("next_cycle is not bound to completed_cycles");
  if (!Array.isArray(value.cycle_summaries) || value.cycle_summaries.length !== completed || value.cycle_summaries.length > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES) fail("cycle_summaries exceed capacity");
  const summaries = value.cycle_summaries.map((item, index) => cycleSummary(item, `cycle_summaries[${index}]`));
  if (summaries.some((item, index) => item.cycle !== index + 1)) fail("cycle_summaries are not contiguous");
  const previous = value.previous_cycle === null ? null : cycleSummary(value.previous_cycle, "previous_cycle");
  if (JSON.stringify(previous) !== JSON.stringify(summaries.at(-1) ?? null)) fail("previous_cycle is not bound to cycle_summaries");
  const totalSelected = integer("total_selected", value.total_selected, 0, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS);
  const totalClaimed = integer("total_claimed", value.total_claimed, 0, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS);
  const totalRuns = integer("total_runs", value.total_runs, 0, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS);
  if (totalClaimed > totalSelected || totalRuns > totalClaimed) fail("aggregate counts are inconsistent");
  if (!Array.isArray(value.evaluation_digests) || value.evaluation_digests.length > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES) fail("evaluation_digests are outside their bounds");
  const evaluationDigests = value.evaluation_digests.map((item) => digest("evaluation_digest", item)!);
  if (!Array.isArray(value.learned_signals) || value.learned_signals.length > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SIGNALS) fail("learned_signals are outside their bounds");
  const body: JsonObject = {
    schema: value.schema,
    run_id: identifier("run_id", value.run_id),
    next_cycle: next,
    cycle_summaries: summaries,
    previous_cycle: previous,
    completed_cycles: completed,
    total_selected: totalSelected,
    total_claimed: totalClaimed,
    total_runs: totalRuns,
    status_counts: counts("status_counts", value.status_counts, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS),
    domain_counts: counts("domain_counts", value.domain_counts, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_RUNS),
    evaluation_count: integer("evaluation_count", value.evaluation_count, 0, AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_CYCLES * AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_EVALUATIONS),
    evaluation_digests: evaluationDigests,
    learning_state_digest: digest("learning_state_digest", value.learning_state_digest, true),
    learned_signals: value.learned_signals.map((item, index) => signal(item, index)),
    learner_state: value.learner_state === null ? null : learnerState(value.learner_state),
    stop_reason: value.stop_reason as string,
    generation: integer("generation", value.generation, 1, 2_147_483_647),
    previous_snapshot_digest: digest("previous_snapshot_digest", value.previous_snapshot_digest, true),
    retention: value.retention as string,
    secret_material: value.secret_material as string,
  };
  if (typeof value.stop_reason !== "string" || !STOP_REASONS.has(value.stop_reason)) fail("stop_reason is invalid");
  if (requireDigest) {
    const supplied = digest("snapshot_digest", value.snapshot_digest)!;
    if (supplied !== digestJsonSync(body)) fail("snapshot digest mismatch");
    body.snapshot_digest = supplied;
  }
  return body;
}

export function sealAutonomousGoalControlLoopSnapshot(descriptor: JsonObject): AutonomousGoalControlLoopCheckpoint {
  const body = normalize(descriptor, false);
  const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousGoalControlLoopCheckpoint;
  if (jsonBytes(snapshot) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return clone(snapshot);
}

export function validateAutonomousGoalControlLoopSnapshot(value: JsonObject): AutonomousGoalControlLoopCheckpoint {
  const normalized = normalize(value, true) as AutonomousGoalControlLoopCheckpoint;
  if (jsonBytes(normalized) > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return clone(normalized);
}

export class JsonAutonomousGoalControlLoopSnapshotPersistence implements AutonomousGoalControlLoopSnapshotPersistence {
  readonly store: AutonomousGoalControlLoopSnapshotTextStore;
  readonly max_bytes: number;

  constructor(store: AutonomousGoalControlLoopSnapshotTextStore, max_bytes = AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) {
    if (typeof store?.read !== "function" || typeof store?.write !== "function") fail("JSON persistence requires a text store");
    if (!Number.isSafeInteger(max_bytes) || max_bytes < 1 || max_bytes > AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) fail("JSON persistence max_bytes is outside its bound");
    this.store = store;
    this.max_bytes = max_bytes;
  }

  async read(): Promise<AutonomousGoalControlLoopCheckpoint | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > this.max_bytes) fail("stored JSON exceeds its byte bound");
    let raw: unknown;
    try { raw = JSON.parse(encoded); } catch { fail("stored JSON is invalid"); }
    if (!isObject(raw)) fail("stored JSON must be an object");
    const normalized = validateAutonomousGoalControlLoopSnapshot(raw as JsonObject);
    if (canonicalJson(normalized) !== encoded) fail("stored JSON is not canonical");
    return normalized;
  }

  async write(snapshot: AutonomousGoalControlLoopCheckpoint): Promise<void> {
    const normalized = validateAutonomousGoalControlLoopSnapshot(snapshot);
    const encoded = canonicalJson(normalized);
    if (new TextEncoder().encode(encoded).byteLength > this.max_bytes) fail("snapshot exceeds the configured byte bound");
    await this.store.write(encoded);
  }
}

export class TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence extends JsonAutonomousGoalControlLoopSnapshotPersistence implements TransactionalAutonomousGoalControlLoopSnapshotPersistence {
  override readonly store: TransactionalAutonomousGoalControlLoopSnapshotTextStore;

  constructor(store: TransactionalAutonomousGoalControlLoopSnapshotTextStore, max_bytes = AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_MAX_SNAPSHOT_BYTES) {
    super(store, max_bytes);
    if (typeof store.write_if_unchanged !== "function") fail("transactional JSON persistence requires write_if_unchanged");
    this.store = store;
  }

  async write_if_unchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousGoalControlLoopCheckpoint): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("expected_snapshot_digest", expectedSnapshotDigest);
    const normalized = validateAutonomousGoalControlLoopSnapshot(snapshot);
    return this.store.write_if_unchanged(expectedSnapshotDigest, canonicalJson(normalized));
  }
}

export class AutonomousGoalControlLoopPersistenceCoordinator {
  private expectedSnapshotDigestValue: string | null = null;
  private expectedGeneration = 0;
  readonly persistence: AutonomousGoalControlLoopSnapshotPersistence;

  constructor(persistence: AutonomousGoalControlLoopSnapshotPersistence) {
    if (typeof persistence?.read !== "function" || typeof persistence?.write !== "function") fail("persistence adapter is malformed");
    this.persistence = persistence;
  }

  get expected_snapshot_digest(): string | null { return this.expectedSnapshotDigestValue; }

  async restore(): Promise<AutonomousGoalControlLoopCheckpoint | null> {
    const raw = await this.persistence.read();
    if (raw === null) {
      this.expectedSnapshotDigestValue = null;
      this.expectedGeneration = 0;
      return null;
    }
    const snapshot = validateAutonomousGoalControlLoopSnapshot(raw);
    this.expectedSnapshotDigestValue = snapshot.snapshot_digest;
    this.expectedGeneration = snapshot.generation;
    return snapshot;
  }

  async flush(snapshot: AutonomousGoalControlLoopCheckpoint): Promise<AutonomousGoalControlLoopCheckpoint> {
    const normalized = validateAutonomousGoalControlLoopSnapshot(snapshot);
    if (normalized.generation !== this.expectedGeneration + 1) fail("checkpoint generation is not contiguous");
    if (normalized.previous_snapshot_digest !== this.expectedSnapshotDigestValue) fail("checkpoint previous digest does not match the restored head");
    const transactional = this.persistence as Partial<TransactionalAutonomousGoalControlLoopSnapshotPersistence>;
    if (typeof transactional.write_if_unchanged === "function") {
      if (!await transactional.write_if_unchanged(this.expectedSnapshotDigestValue, normalized)) fail("persistence compare-and-swap conflict");
    } else {
      await this.persistence.write(normalized);
    }
    this.expectedSnapshotDigestValue = normalized.snapshot_digest;
    this.expectedGeneration = normalized.generation;
    return normalized;
  }
}
