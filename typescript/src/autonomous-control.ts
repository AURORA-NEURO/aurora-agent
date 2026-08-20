import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  rankAutonomousModels,
  type AutonomousModelSelector,
  type AutonomousSelectionDecision,
  type AutonomousSelectionRequest,
  type ProviderHealth,
  type ProviderInvocationMetadata,
  type ProviderInvocationObserver,
  type ProviderInvocationOutcome,
} from "./llm.js";
import { builtinAutonomousDomainEvaluatorProfiles } from "./autonomous-learning.js";
import type { AutonomousDomainName } from "./autonomous.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_MODEL_OBSERVATION_SCHEMA = "bioprism-typescript-autonomous-model-observation/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_SCHEMA = "bioprism-typescript-autonomous-model-health/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_EVENT_SCHEMA = "bioprism-typescript-autonomous-model-health-event/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-model-health-snapshot/0.1" as const;
export const AUTONOMOUS_REPLAY_CASE_SCHEMA = "bioprism-typescript-autonomous-replay-case/0.1" as const;
export const AUTONOMOUS_REPLAY_REPORT_SCHEMA = "bioprism-typescript-autonomous-replay-report/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS = 16_384;
export const AUTONOMOUS_MODEL_HEALTH_MAX_QUERY_LIMIT = 256;
export const AUTONOMOUS_REPLAY_MAX_CASES = 4_096;
export const AUTONOMOUS_REPLAY_MAX_SIGNALS = 128;

const PRIVATE_RETENTION = "metadata_only;provider_payloads_prompts_tool_arguments_credentials_not_retained" as const;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;
const DIGEST = /^[0-9a-f]{64}$/;
const OUTCOMES = ["success", "failure", "unknown"] as const;
const OBSERVATION_KINDS = ["invocation", "evaluation"] as const;

type ObservationOutcome = typeof OUTCOMES[number];
type ObservationKind = typeof OBSERVATION_KINDS[number];

export interface AutonomousModelObservation extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_OBSERVATION_SCHEMA;
  provider: string;
  model: string;
  domain: string;
  capability: string;
  risk_class: string;
  status: string;
  outcome: ObservationOutcome;
  observation_kind: ObservationKind;
  latency_ms: number;
  input_tokens: number | null;
  output_tokens: number | null;
  failure_class: string | null;
  quality_reward: number | null;
  quality_passed: boolean | null;
  outcome_digest: string | null;
  evidence_digest: string | null;
  evaluator_id: string | null;
  evaluator_version: string | null;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousModelObservationInput {
  provider: string;
  model: string;
  domain: string;
  capability: string;
  risk_class: string;
  status: string;
  outcome: ObservationOutcome;
  observation_kind?: ObservationKind;
  latency_ms: number;
  input_tokens?: number | null;
  output_tokens?: number | null;
  failure_class?: string | null;
  quality_reward?: number | null;
  quality_passed?: boolean | null;
  outcome_digest?: string | null;
  evidence_digest?: string | null;
  evaluator_id?: string | null;
  evaluator_version?: string | null;
}

export interface AutonomousModelHealth extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_HEALTH_SCHEMA;
  provider: string;
  model: string;
  attempts: number;
  successes: number;
  failures: number;
  unknown: number;
  success_rate: number;
  failure_rate: number;
  mean_latency_ms: number;
  quality_observations: number;
  quality_mean: number | null;
  quality_pass_rate: number | null;
  last_status: string | null;
  last_outcome: ObservationOutcome | null;
  last_sequence: number;
  circuit: "closed" | "open";
  retention: "aggregated_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousModelHealthQuery {
  provider?: string;
  model?: string;
  domain?: string;
  capability?: string;
  risk_class?: string;
  min_attempts?: number;
  failure_threshold?: number;
  limit?: number;
}

export interface AutonomousModelHealthEvent extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_HEALTH_EVENT_SCHEMA;
  sequence: number;
  observation: AutonomousModelObservation;
  previous_digest: string;
  created_at: number;
  event_digest: string;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousModelHealthReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_HEALTH_SCHEMA;
  sequence: number;
  event_digest: string;
  provider: string;
  model: string;
  observation_kind: ObservationKind;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousModelHealthSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  events: AutonomousModelHealthEvent[];
  snapshot_digest: string;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousModelHealthPersistence {
  read(): Promise<AutonomousModelHealthSnapshot | null> | AutonomousModelHealthSnapshot | null;
  write(snapshot: AutonomousModelHealthSnapshot): Promise<void> | void;
}

export interface AutonomousModelHealthStore {
  record(observation: AutonomousModelObservationInput): Promise<AutonomousModelHealthReceipt> | AutonomousModelHealthReceipt;
  recordInvocation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome"> & { outcome: Exclude<ObservationOutcome, "unknown"> }): Promise<AutonomousModelHealthReceipt> | AutonomousModelHealthReceipt;
  recordEvaluation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome" | "latency_ms"> & { quality_reward: number; quality_passed: boolean }): Promise<AutonomousModelHealthReceipt> | AutonomousModelHealthReceipt;
  health(query?: AutonomousModelHealthQuery): Promise<AutonomousModelHealth[]> | AutonomousModelHealth[];
  selectorHealth(query?: AutonomousModelHealthQuery): Promise<Record<string, ProviderHealth & { model: string; quality_mean?: number | null; quality_observations?: number }>> | Record<string, ProviderHealth & { model: string; quality_mean?: number | null; quality_observations?: number }>;
  snapshot(): Promise<AutonomousModelHealthSnapshot>;
  restore(snapshot: AutonomousModelHealthSnapshot): Promise<void>;
  verifyIntegrity(): Promise<{ verified: true; events: number; head_digest: string }>;
}

export interface AutonomousReplayCase extends JsonObject {
  schema: typeof AUTONOMOUS_REPLAY_CASE_SCHEMA;
  run_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  evaluator_id: string;
  evaluator_version: string;
  execution_status: "completed" | "failed" | "incomplete";
  signals: Record<string, number>;
  evidence_digest: string;
  expected_reward: number | null;
  expected_passed: boolean | null;
  expected_evaluation_digest: string | null;
  retention: "caller_rehydrated_numeric_evidence_only";
  secret_material: "never_returned";
}

export interface AutonomousReplayCaseInput {
  run_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  evaluator_id: string;
  evaluator_version: string;
  execution_status: AutonomousReplayCase["execution_status"];
  signals: Record<string, number>;
  evidence_digest: string;
  expected_reward?: number | null;
  expected_passed?: boolean | null;
  expected_evaluation_digest?: string | null;
}

export interface AutonomousReplayCaseResult extends JsonObject {
  run_id: string;
  domain: AutonomousDomainName;
  status: "passed" | "failed" | "incomplete" | "refused";
  reward: number;
  passed: boolean;
  missing_signals: string[];
  rejected_signals: string[];
  expected_reward: number | null;
  expected_passed: boolean | null;
  expected_evaluation_digest: string | null;
  evaluation_digest: string | null;
  mismatch_codes: string[];
}

export interface AutonomousReplayReport extends JsonObject {
  schema: typeof AUTONOMOUS_REPLAY_REPORT_SCHEMA;
  status: "completed" | "mismatch" | "refused";
  case_count: number;
  passed_count: number;
  failed_count: number;
  incomplete_count: number;
  mismatch_count: number;
  cases: AutonomousReplayCaseResult[];
  report_digest: string;
  retention: "metadata_only;provider_calls_not_replayed";
  secret_material: "never_returned";
}

export interface AutonomousHealthSelectorContext {
  domain: string;
  capability: string;
  riskClass: string;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function identifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || !IDENTIFIER.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function text(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || /[\u0000-\u001f]/.test(value)) throw new ArgumentError(`${name} must be bounded text`);
  return value;
}

function digest(name: string, value: unknown, nullable = false): string | null {
  if (value === null && nullable) return null;
  if (typeof value !== "string" || !DIGEST.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be finite and within [${minimum}, ${maximum}]`);
  return value;
}

function nonnegativeInteger(name: string, value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new ArgumentError(`${name} must be a non-negative integer or null`);
  return value as number;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 16) throw new ArgumentError("autonomous control metadata is too deeply nested");
  if (Array.isArray(value)) {
    if (value.length > 256) throw new ArgumentError("autonomous control metadata contains too many items");
    for (const child of value) safeMetadata(child, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
    if (["apikey", "authorization", "bearer", "credential", "password", "secret", "token", "privatekey", "prompt", "messages", "response", "rawpayload", "arguments", "output", "task", "content", "body", "headers", "input"].includes(normalized)) throw new ArgumentError("autonomous control metadata contains transient or secret-shaped fields");
    safeMetadata(child, depth + 1);
  }
}

function normalizeObservation(input: AutonomousModelObservationInput): AutonomousModelObservation {
  if (!isObject(input)) throw new ArgumentError("model observation must be an object");
  safeMetadata(input);
  const observationKind = input.observation_kind ?? "invocation";
  if (!OBSERVATION_KINDS.includes(observationKind)) throw new ArgumentError("model observation kind is unsupported");
  const outcome = input.outcome;
  if (!OUTCOMES.includes(outcome)) throw new ArgumentError("model observation outcome is unsupported");
  const qualityReward = input.quality_reward === undefined || input.quality_reward === null ? null : finite("model observation quality_reward", input.quality_reward, 0, 1);
  const qualityPassed = input.quality_passed === undefined ? null : input.quality_passed;
  if (qualityPassed !== null && typeof qualityPassed !== "boolean") throw new ArgumentError("model observation quality_passed must be boolean or null");
  if (observationKind === "evaluation" && (qualityReward === null || qualityPassed === null)) throw new ArgumentError("evaluation observations require explicit quality reward and pass state");
  return {
    schema: AUTONOMOUS_MODEL_OBSERVATION_SCHEMA,
    provider: identifier("model observation provider", input.provider, 128),
    model: identifier("model observation model", input.model, 512),
    domain: identifier("model observation domain", input.domain),
    capability: identifier("model observation capability", input.capability),
    risk_class: identifier("model observation risk_class", input.risk_class),
    status: text("model observation status", input.status, 128),
    outcome,
    observation_kind: observationKind,
    latency_ms: finite("model observation latency_ms", input.latency_ms, 0, 86_400_000),
    input_tokens: nonnegativeInteger("model observation input_tokens", input.input_tokens),
    output_tokens: nonnegativeInteger("model observation output_tokens", input.output_tokens),
    failure_class: input.failure_class === undefined || input.failure_class === null ? null : identifier("model observation failure_class", input.failure_class, 128),
    quality_reward: qualityReward,
    quality_passed: qualityPassed,
    outcome_digest: input.outcome_digest === undefined || input.outcome_digest === null ? null : digest("model observation outcome_digest", input.outcome_digest, true),
    evidence_digest: input.evidence_digest === undefined || input.evidence_digest === null ? null : digest("model observation evidence_digest", input.evidence_digest, true),
    evaluator_id: input.evaluator_id === undefined || input.evaluator_id === null ? null : identifier("model observation evaluator_id", input.evaluator_id),
    evaluator_version: input.evaluator_version === undefined || input.evaluator_version === null ? null : identifier("model observation evaluator_version", input.evaluator_version),
    retention: PRIVATE_RETENTION,
    secret_material: "never_returned",
  };
}

function observationKey(observation: AutonomousModelObservation): string {
  return `${observation.provider}/${observation.model}`;
}

interface Aggregate {
  provider: string;
  model: string;
  attempts: number;
  successes: number;
  failures: number;
  unknown: number;
  totalLatency: number;
  qualityObservations: number;
  qualityTotal: number;
  qualityPassed: number;
  lastStatus: string | null;
  lastOutcome: ObservationOutcome | null;
  lastSequence: number;
}

function emptyAggregate(provider: string, model: string): Aggregate {
  return { provider, model, attempts: 0, successes: 0, failures: 0, unknown: 0, totalLatency: 0, qualityObservations: 0, qualityTotal: 0, qualityPassed: 0, lastStatus: null, lastOutcome: null, lastSequence: 0 };
}

function healthProjection(entry: Aggregate, minAttempts: number, failureThreshold: number): AutonomousModelHealth {
  const attempts = entry.attempts;
  const failures = entry.failures;
  return {
    schema: AUTONOMOUS_MODEL_HEALTH_SCHEMA,
    provider: entry.provider,
    model: entry.model,
    attempts,
    successes: entry.successes,
    failures,
    unknown: entry.unknown,
    success_rate: attempts === 0 ? 0 : entry.successes / attempts,
    failure_rate: attempts === 0 ? 0 : failures / attempts,
    mean_latency_ms: attempts === 0 ? 0 : entry.totalLatency / attempts,
    quality_observations: entry.qualityObservations,
    quality_mean: entry.qualityObservations === 0 ? null : entry.qualityTotal / entry.qualityObservations,
    quality_pass_rate: entry.qualityObservations === 0 ? null : entry.qualityPassed / entry.qualityObservations,
    last_status: entry.lastStatus,
    last_outcome: entry.lastOutcome,
    last_sequence: entry.lastSequence,
    circuit: attempts >= minAttempts && failures / attempts >= failureThreshold ? "open" : "closed",
    retention: "aggregated_metadata_only",
    secret_material: "never_returned",
  };
}

export class InMemoryAutonomousModelHealthStore implements AutonomousModelHealthStore {
  private readonly events: AutonomousModelHealthEvent[] = [];
  readonly maxEvents: number;
  private readonly clock: () => number;

  constructor(options: { maxEvents?: number; clock?: () => number } = {}) {
    this.maxEvents = options.maxEvents ?? AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS;
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS) throw new ArgumentError("model health maxEvents is outside its bounds");
    this.clock = options.clock ?? (() => Date.now() / 1_000);
  }

  async record(input: AutonomousModelObservationInput): Promise<AutonomousModelHealthReceipt> {
    const observation = normalizeObservation(input);
    if (this.events.length >= this.maxEvents) throw new ArgumentError("model health event capacity is exhausted");
    const base = { schema: AUTONOMOUS_MODEL_HEALTH_EVENT_SCHEMA, sequence: this.events.length + 1, observation, previous_digest: this.events.at(-1)?.event_digest ?? "", created_at: finite("model health clock", this.clock(), 0, Number.MAX_SAFE_INTEGER), retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const event: AutonomousModelHealthEvent = { ...base, event_digest: await digestJson(base) };
    this.events.push(event);
    return { schema: AUTONOMOUS_MODEL_HEALTH_SCHEMA, sequence: event.sequence, event_digest: event.event_digest, provider: observation.provider, model: observation.model, observation_kind: observation.observation_kind, retention: PRIVATE_RETENTION };
  }

  recordInvocation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome"> & { outcome: Exclude<ObservationOutcome, "unknown"> }): Promise<AutonomousModelHealthReceipt> {
    return this.record({ ...input, observation_kind: "invocation" });
  }

  recordEvaluation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome" | "latency_ms"> & { quality_reward: number; quality_passed: boolean }): Promise<AutonomousModelHealthReceipt> {
    return this.record({ ...input, observation_kind: "evaluation", outcome: "unknown", latency_ms: 0 });
  }

  health(query: AutonomousModelHealthQuery = {}): AutonomousModelHealth[] {
    safeMetadata(query);
    const minAttempts = query.min_attempts ?? 1;
    const failureThreshold = query.failure_threshold ?? 0.75;
    const limit = query.limit ?? AUTONOMOUS_MODEL_HEALTH_MAX_QUERY_LIMIT;
    if (!Number.isSafeInteger(minAttempts) || minAttempts < 1) throw new ArgumentError("model health min_attempts must be positive");
    finite("model health failure_threshold", failureThreshold, 0, 1);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_MODEL_HEALTH_MAX_QUERY_LIMIT) throw new ArgumentError("model health limit is outside its bounds");
    for (const [name, value] of [["provider", query.provider], ["model", query.model], ["domain", query.domain], ["capability", query.capability], ["risk_class", query.risk_class]] as const) if (value !== undefined) identifier(`model health ${name}`, value, 512);
    const aggregates = new Map<string, Aggregate>();
    for (const event of this.events) {
      const observation = event.observation;
      if (query.provider !== undefined && observation.provider !== query.provider) continue;
      if (query.model !== undefined && observation.model !== query.model) continue;
      if (query.domain !== undefined && observation.domain !== query.domain) continue;
      if (query.capability !== undefined && observation.capability !== query.capability) continue;
      if (query.risk_class !== undefined && observation.risk_class !== query.risk_class) continue;
      const key = observationKey(observation);
      const entry = aggregates.get(key) ?? emptyAggregate(observation.provider, observation.model);
      if (observation.observation_kind === "invocation") {
        entry.attempts += 1;
        if (observation.outcome === "success") entry.successes += 1;
        else if (observation.outcome === "failure") entry.failures += 1;
        else entry.unknown += 1;
        entry.totalLatency += observation.latency_ms;
        entry.lastStatus = observation.status;
        entry.lastOutcome = observation.outcome;
      }
      if (observation.quality_reward !== null) {
        entry.qualityObservations += 1;
        entry.qualityTotal += observation.quality_reward;
        if (observation.quality_passed === true) entry.qualityPassed += 1;
      }
      entry.lastSequence = event.sequence;
      aggregates.set(key, entry);
    }
    return [...aggregates.values()].map((entry) => healthProjection(entry, minAttempts, failureThreshold)).sort((a, b) => b.attempts - a.attempts || a.provider.localeCompare(b.provider) || a.model.localeCompare(b.model)).slice(0, limit).map(clone);
  }

  selectorHealth(query: AutonomousModelHealthQuery = {}): Record<string, ProviderHealth & { model: string; quality_mean?: number | null; quality_observations?: number }> {
    const result: Record<string, ProviderHealth & { model: string; quality_mean?: number | null; quality_observations?: number }> = {};
    for (const row of this.health(query)) {
      result[`${row.provider}/${row.model}`] = {
        provider: row.provider,
        model: row.model,
        circuit: row.circuit,
        consecutive_failures: row.failures,
        attempts: row.attempts,
        successes: row.successes,
        failures: row.failures,
        success_rate: row.success_rate,
        mean_latency_ms: row.mean_latency_ms,
        last_latency_ms: row.mean_latency_ms,
        last_model: row.model,
        last_status_code: null,
        credential_posture: "caller_supplied_opaque_handle",
        credential_required: true,
        quality_mean: row.quality_mean,
        quality_observations: row.quality_observations,
      };
    }
    return result;
  }

  async snapshot(): Promise<AutonomousModelHealthSnapshot> {
    const body = { schema: AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA, sequence: this.events.length, head_digest: this.events.at(-1)?.event_digest ?? "", events: this.events.map(clone), retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    return { ...body, snapshot_digest: await digestJson(body) };
  }

  async restore(snapshot: AutonomousModelHealthSnapshot): Promise<void> {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.events)) throw new ArgumentError("model health snapshot is malformed");
    const { snapshot_digest: supplied, ...body } = snapshot;
    if (await digestJson(body) !== supplied) throw new ArgumentError("model health snapshot digest mismatch");
    if (snapshot.events.length > this.maxEvents) throw new ArgumentError("model health snapshot exceeds store capacity");
    const events = snapshot.events.map(clone);
    const verified = await this.verifyEvents(events);
    if (verified.events !== snapshot.sequence || verified.head_digest !== snapshot.head_digest) throw new ArgumentError("model health snapshot head is inconsistent");
    this.events.splice(0, this.events.length, ...events);
  }

  async verifyIntegrity(): Promise<{ verified: true; events: number; head_digest: string }> {
    return this.verifyEvents(this.events);
  }

  private async verifyEvents(events: readonly AutonomousModelHealthEvent[]): Promise<{ verified: true; events: number; head_digest: string }> {
    let previous = "";
    for (let index = 0; index < events.length; index += 1) {
      const event = events[index]!;
      if (event.sequence !== index + 1 || event.previous_digest !== previous) throw new ArgumentError(`model health hash chain breaks at sequence ${event.sequence}`);
      const { event_digest: _eventDigest, ...body } = event;
      if (await digestJson(body) !== event.event_digest) throw new ArgumentError(`model health event digest mismatch at sequence ${event.sequence}`);
      normalizeObservation(event.observation);
      previous = event.event_digest;
    }
    return { verified: true, events: events.length, head_digest: previous };
  }
}

export class AutonomousModelHealthPersistenceCoordinator {
  constructor(readonly store: AutonomousModelHealthStore, readonly persistence: AutonomousModelHealthPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("model health store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("model health persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousModelHealthSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousModelHealthSnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

/** Selector/observer bridge that makes a caller-owned health ledger affect future model choice. */
export class AutonomousModelHealthController {
  constructor(readonly store: AutonomousModelHealthStore) {
    if (!store || typeof store.health !== "function" || typeof store.selectorHealth !== "function" || typeof store.record !== "function") throw new ArgumentError("model health controller requires a health store");
  }

  selector(): AutonomousModelSelector {
    return async (request: AutonomousSelectionRequest): Promise<AutonomousSelectionDecision> => {
      const persistent = await this.store.selectorHealth();
      const merged: AutonomousSelectionRequest = { ...request, model_health: { ...request.model_health, ...persistent } };
      const ranking = rankAutonomousModels(merged);
      const chosen = ranking.find((row) => row.eligible);
      return { selected_model: chosen ? { provider: chosen.provider, model: chosen.model } : null, strategy: "caller_selector", ranking, abstention_reason: chosen ? null : "no eligible model candidate after persisted health" };
    };
  }

  observer(context: AutonomousHealthSelectorContext): ProviderInvocationObserver {
    identifier("health context domain", context.domain);
    identifier("health context capability", context.capability);
    identifier("health context riskClass", context.riskClass);
    return {
      after: async (metadata: ProviderInvocationMetadata, outcome: ProviderInvocationOutcome): Promise<void> => {
        await this.store.recordInvocation({
          provider: metadata.provider,
          model: metadata.model,
          domain: context.domain,
          capability: context.capability,
          risk_class: context.riskClass,
          status: outcome.status,
          outcome: outcome.success ? "success" : "failure",
          latency_ms: outcome.latencyMs,
          input_tokens: outcome.inputTokens,
          output_tokens: outcome.outputTokens,
          failure_class: outcome.failureClass ?? null,
        });
      },
    };
  }

  async recordEvaluation(input: { provider: string; model: string; domain: string; capability: string; riskClass: string; evaluatorId: string; evaluatorVersion: string; reward: number; passed: boolean; evidenceDigest?: string | null }): Promise<AutonomousModelHealthReceipt> {
    identifier("health evaluation evaluatorId", input.evaluatorId);
    identifier("health evaluation evaluatorVersion", input.evaluatorVersion);
    return this.store.recordEvaluation({ provider: input.provider, model: input.model, domain: input.domain, capability: input.capability, risk_class: input.riskClass, status: "evaluated", quality_reward: input.reward, quality_passed: input.passed, evidence_digest: input.evidenceDigest ?? null, evaluator_id: input.evaluatorId, evaluator_version: input.evaluatorVersion });
  }
}

function normalizeReplayCase(input: AutonomousReplayCaseInput): AutonomousReplayCase {
  if (!isObject(input)) throw new ArgumentError("replay case must be an object");
  safeMetadata(input);
  identifier("replay run_id", input.run_id);
  if (typeof input.domain !== "string" || !input.domain) throw new ArgumentError("replay domain must be non-empty");
  identifier("replay capability", input.capability);
  identifier("replay risk_class", input.risk_class);
  identifier("replay evaluator_id", input.evaluator_id);
  identifier("replay evaluator_version", input.evaluator_version);
  if (!["completed", "failed", "incomplete"].includes(input.execution_status)) throw new ArgumentError("replay execution_status is unsupported");
  if (!isObject(input.signals) || Object.keys(input.signals).length > AUTONOMOUS_REPLAY_MAX_SIGNALS) throw new ArgumentError("replay signals are outside their bounds");
  const signals: Record<string, number> = {};
  for (const [name, value] of Object.entries(input.signals)) {
    identifier("replay signal", name);
    signals[name] = finite(`replay signal ${name}`, value, 0, 1);
  }
  if (input.expected_passed !== undefined && input.expected_passed !== null && typeof input.expected_passed !== "boolean") throw new ArgumentError("replay expected_passed must be boolean or null");
  return {
    schema: AUTONOMOUS_REPLAY_CASE_SCHEMA,
    run_id: input.run_id,
    domain: input.domain,
    capability: input.capability,
    risk_class: input.risk_class,
    evaluator_id: input.evaluator_id,
    evaluator_version: input.evaluator_version,
    execution_status: input.execution_status,
    signals,
    evidence_digest: digest("replay evidence_digest", input.evidence_digest)!,
    expected_reward: input.expected_reward === undefined || input.expected_reward === null ? null : finite("replay expected_reward", input.expected_reward, 0, 1),
    expected_passed: input.expected_passed === undefined ? null : input.expected_passed,
    expected_evaluation_digest: input.expected_evaluation_digest === undefined || input.expected_evaluation_digest === null ? null : digest("replay expected_evaluation_digest", input.expected_evaluation_digest, true),
    retention: "caller_rehydrated_numeric_evidence_only",
    secret_material: "never_returned",
  };
}

/** Re-evaluate caller-rehydrated numeric evidence without replaying a provider or tool call. */
export class AutonomousOfflineReplayEngine {
  async replay(inputs: readonly AutonomousReplayCaseInput[]): Promise<AutonomousReplayReport> {
    if (!Array.isArray(inputs) || inputs.length > AUTONOMOUS_REPLAY_MAX_CASES) throw new ArgumentError("replay cases are outside their bounds");
    const cases = inputs.map(normalizeReplayCase);
    const profiles = await builtinAutonomousDomainEvaluatorProfiles();
    const profilesByDomain = new Map(profiles.map((profile) => [profile.domain, profile]));
    const results: AutonomousReplayCaseResult[] = [];
    for (const replayCase of cases) {
      const profile = profilesByDomain.get(replayCase.domain);
      if (!profile) {
        results.push({ run_id: replayCase.run_id, domain: replayCase.domain, status: "refused", reward: 0, passed: false, missing_signals: [], rejected_signals: [], expected_reward: replayCase.expected_reward, expected_passed: replayCase.expected_passed, expected_evaluation_digest: replayCase.expected_evaluation_digest, evaluation_digest: null, mismatch_codes: ["unsupported_domain"] });
        continue;
      }
      const required = [...new Set(profile.required_signals)].sort();
      const missing = required.filter((signal) => replayCase.signals[signal] === undefined);
      const rejected = Object.keys(replayCase.signals).filter((signal) => !required.includes(signal)).sort();
      const weighted = required.map((signal) => ({ score: replayCase.signals[signal] ?? 0, weight: profile.signal_weights[signal] ?? 1 }));
      const weightTotal = weighted.reduce((sum, row) => sum + row.weight, 0);
      const reward = weightTotal === 0 ? 0 : Number((weighted.reduce((sum, row) => sum + row.score * row.weight, 0) / weightTotal).toFixed(12));
      const passed = replayCase.execution_status === "completed" && missing.length === 0 && rejected.length === 0 && required.every((signal) => (replayCase.signals[signal] ?? 0) >= profile.pass_threshold);
      const status: AutonomousReplayCaseResult["status"] = passed ? "passed" : replayCase.execution_status === "completed" && missing.length === 0 ? "failed" : "incomplete";
      const descriptor = { schema: AUTONOMOUS_REPLAY_CASE_SCHEMA, run_id: replayCase.run_id, domain: replayCase.domain, evaluator_id: profile.evaluator_id, evaluator_version: profile.evaluator_version, execution_status: replayCase.execution_status, reward, passed, missing_signals: missing, rejected_signals: rejected, evidence_digest: replayCase.evidence_digest };
      const evaluationDigest = await digestJson(descriptor);
      const mismatchCodes: string[] = [];
      if (replayCase.evaluator_id !== profile.evaluator_id) mismatchCodes.push("evaluator_id_mismatch");
      if (replayCase.evaluator_version !== profile.evaluator_version) mismatchCodes.push("evaluator_version_mismatch");
      if (replayCase.expected_reward !== null && replayCase.expected_reward !== reward) mismatchCodes.push("reward_mismatch");
      if (replayCase.expected_passed !== null && replayCase.expected_passed !== passed) mismatchCodes.push("pass_mismatch");
      if (replayCase.expected_evaluation_digest !== null && replayCase.expected_evaluation_digest !== evaluationDigest) mismatchCodes.push("evaluation_digest_mismatch");
      results.push({ run_id: replayCase.run_id, domain: replayCase.domain, status, reward, passed, missing_signals: missing, rejected_signals: rejected, expected_reward: replayCase.expected_reward, expected_passed: replayCase.expected_passed, expected_evaluation_digest: replayCase.expected_evaluation_digest, evaluation_digest: evaluationDigest, mismatch_codes: mismatchCodes });
    }
    const mismatchCount = results.filter((result) => result.mismatch_codes.length > 0).length;
    const reportBody = { schema: AUTONOMOUS_REPLAY_REPORT_SCHEMA, status: mismatchCount ? "mismatch" as const : "completed" as const, case_count: results.length, passed_count: results.filter((result) => result.status === "passed").length, failed_count: results.filter((result) => result.status === "failed").length, incomplete_count: results.filter((result) => result.status === "incomplete").length, mismatch_count: mismatchCount, cases: results, retention: "metadata_only;provider_calls_not_replayed" as const, secret_material: "never_returned" as const };
    return { ...reportBody, report_digest: await digestJson(reportBody) };
  }
}
