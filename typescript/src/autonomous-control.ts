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
import type { AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, digestCanonicalJsonText, digestJson } from "./tooling.js";
import type {
  BrainHealthStatus,
  BrainModelHealthArgs,
  BrainModelHealthResult,
  BrainReplayEvaluateArgs,
  BrainReplayEvaluateResult,
  JsonObject,
  RestToolResponse,
} from "./types.js";

export const AUTONOMOUS_MODEL_OBSERVATION_SCHEMA = "bioprism-typescript-autonomous-model-observation/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_SCHEMA = "bioprism-typescript-autonomous-model-health/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_EVENT_SCHEMA = "bioprism-typescript-autonomous-model-health-event/0.1" as const;
const LEGACY_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-model-health-snapshot/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-model-health-snapshot/0.2" as const;
export const AUTONOMOUS_REPLAY_CASE_SCHEMA = "bioprism-typescript-autonomous-replay-case/0.1" as const;
export const AUTONOMOUS_REPLAY_REPORT_SCHEMA = "bioprism-typescript-autonomous-replay-report/0.1" as const;
export const BRAIN_DOMAIN_EVALUATOR_SCHEMA = "bioprism-brain-domain-evaluator/0.1" as const;
export const AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS = 16_384;
export const AUTONOMOUS_MODEL_HEALTH_MAX_QUERY_LIMIT = 256;
export const MAX_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_BYTES = 16_000_000;
export const AUTONOMOUS_REPLAY_MAX_CASES = 4_096;
export const AUTONOMOUS_REPLAY_MAX_SIGNALS = 128;
export const MAX_AUTONOMOUS_REPLAY_REPORT_BYTES = 8_000_000;

const PRIVATE_RETENTION = "metadata_only;provider_payloads_prompts_tool_arguments_credentials_not_retained" as const;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;
const DIGEST = /^[0-9a-f]{64}$/;
const OUTCOMES = ["success", "failure", "unknown"] as const;
const OBSERVATION_KINDS = ["invocation", "evaluation"] as const;

type ObservationOutcome = typeof OUTCOMES[number];
type ObservationKind = typeof OBSERVATION_KINDS[number];
export type AutonomousReplaySignal = boolean | number;

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
  /** 0.1 remains readable; current snapshots carry independent image lineage in 0.2. */
  schema: typeof AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA | typeof LEGACY_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA;
  snapshot_generation?: number;
  previous_snapshot_digest?: string | null;
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
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousModelHealthSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousModelHealthSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousModelHealthTransactionalSnapshotTextStore extends AutonomousModelHealthSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
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
  references: string[];
  limitations: string[];
  required_signals: string[] | null;
  signal_weights: Record<string, number> | null;
  pass_threshold: number | null;
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
  signals: Record<string, AutonomousReplaySignal>;
  evidence_digest?: string | null;
  references?: readonly string[];
  limitations?: readonly string[];
  required_signals?: readonly string[];
  signal_weights?: Record<string, number>;
  pass_threshold?: number;
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

/** Minimal structural client required by the cross-runtime control-plane adapter. */
export interface AutonomousBrainControlTransport {
  brainModelHealth(args?: BrainModelHealthArgs): Promise<RestToolResponse<BrainModelHealthResult>>;
  brainReplayEvaluate(args: BrainReplayEvaluateArgs): Promise<RestToolResponse<BrainReplayEvaluateResult>>;
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

function normalizeReplaySignals(value: unknown, maximum = AUTONOMOUS_REPLAY_MAX_SIGNALS): Record<string, number> {
  if (!isObject(value) || Object.keys(value).length === 0 || Object.keys(value).length > maximum) throw new ArgumentError(`replay signals must contain 1..${maximum} entries`);
  const normalized: Record<string, number> = {};
  for (const [name, raw] of Object.entries(value)) {
    identifier("replay signal", name, 128);
    normalized[name] = typeof raw === "boolean" ? (raw ? 1 : 0) : finite(`replay signal ${name}`, raw, 0, 1);
  }
  return normalized;
}

function canonicalReplayEvidenceJson(evidence: { schema: string; domain: string; capability: string; risk_class: string; signals: Record<string, number>; references: string[]; limitations: string[]; retention: string }): string {
  const entries = Object.keys(evidence).sort().map((key) => {
    const value = evidence[key as keyof typeof evidence];
    if (key !== "signals") return `${JSON.stringify(key)}:${canonicalJson(value)}`;
    const signals = Object.keys(evidence.signals).sort().map((signal) => {
      const encoded = canonicalJson(evidence.signals[signal]);
      // Python and serde_json retain the integral float marker (1.0/0.0) after
      // normalizing replay signals; preserve that spelling for cross-runtime IDs.
      const number = Number.isInteger(evidence.signals[signal]) && !encoded.includes(".") && !encoded.includes("e") && !encoded.includes("E") ? `${encoded}.0` : encoded;
      return `${JSON.stringify(signal)}:${number}`;
    });
    return `${JSON.stringify(key)}:{${signals.join(",")}}`;
  });
  return `{${entries.join(",")}}`;
}

interface AutonomousReplayEvidenceInput {
  domain: string;
  capability: string;
  risk_class: string;
  signals: Record<string, AutonomousReplaySignal>;
  references?: readonly string[];
  limitations?: readonly string[];
}

/** Compute the Rust/Python-compatible digest for value-only domain replay evidence. */
export async function autonomousReplayEvidenceDigest(input: AutonomousReplayEvidenceInput): Promise<string> {
  if (!isObject(input)) throw new ArgumentError("replay evidence must be an object");
  identifier("replay evidence domain", input.domain);
  identifier("replay evidence capability", input.capability);
  identifier("replay evidence risk_class", input.risk_class);
  const signals = normalizeReplaySignals(input.signals, 64);
  const references = input.references === undefined ? [] : [...input.references];
  if (references.length > 64 || references.some((reference) => typeof reference !== "string" || !DIGEST.test(reference))) throw new ArgumentError("replay evidence references must contain at most 64 lowercase SHA-256 digests");
  const limitations = input.limitations === undefined ? [] : [...input.limitations];
  if (limitations.length > 32 || limitations.some((limitation) => typeof limitation !== "string" || limitation.length > 2_048 || /[\u0000-\u001f]/.test(limitation))) throw new ArgumentError("replay evidence limitations must contain at most 32 bounded strings");
  const evidence = {
    schema: BRAIN_DOMAIN_EVALUATOR_SCHEMA,
    domain: input.domain,
    capability: input.capability,
    risk_class: input.risk_class,
    signals,
    references,
    limitations,
    retention: "value_only_digests_and_signal_scores",
  };
  return digestCanonicalJsonText(canonicalReplayEvidenceJson(evidence));
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
  private snapshotGeneration = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: AutonomousModelHealthSnapshot | null = null;
  private cachedEventSignature: string | null = null;
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
    this.cachedSnapshot = null;
    this.cachedEventSignature = null;
    return { schema: AUTONOMOUS_MODEL_HEALTH_SCHEMA, sequence: event.sequence, event_digest: event.event_digest, provider: observation.provider, model: observation.model, observation_kind: observation.observation_kind, retention: PRIVATE_RETENTION };
  }

  recordInvocation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome"> & { outcome: Exclude<ObservationOutcome, "unknown"> }): Promise<AutonomousModelHealthReceipt> {
    return this.record({ ...input, observation_kind: "invocation" });
  }

  async recordEvaluation(input: Omit<AutonomousModelObservationInput, "observation_kind" | "outcome" | "latency_ms"> & { quality_reward: number; quality_passed: boolean }): Promise<AutonomousModelHealthReceipt> {
    const normalized = normalizeObservation({ ...input, observation_kind: "evaluation", outcome: "unknown", latency_ms: 0 });
    if (normalized.outcome_digest !== null) {
      const prior = this.events.find((event) => event.observation.observation_kind === "evaluation"
        && event.observation.outcome_digest === normalized.outcome_digest
        && event.observation.provider === normalized.provider
        && event.observation.model === normalized.model
        && event.observation.domain === normalized.domain
        && event.observation.capability === normalized.capability
        && event.observation.risk_class === normalized.risk_class);
      if (prior) {
        const same = prior.observation.provider === normalized.provider
          && prior.observation.model === normalized.model
          && prior.observation.domain === normalized.domain
          && prior.observation.capability === normalized.capability
          && prior.observation.risk_class === normalized.risk_class
          && prior.observation.quality_reward === normalized.quality_reward
          && prior.observation.quality_passed === normalized.quality_passed
          && prior.observation.evidence_digest === normalized.evidence_digest
          && prior.observation.evaluator_id === normalized.evaluator_id
          && prior.observation.evaluator_version === normalized.evaluator_version;
        if (!same) throw new ArgumentError("model health evaluation outcome_digest conflicts with an existing evaluation");
        return { schema: AUTONOMOUS_MODEL_HEALTH_SCHEMA, sequence: prior.sequence, event_digest: prior.event_digest, provider: prior.observation.provider, model: prior.observation.model, observation_kind: "evaluation", retention: PRIVATE_RETENTION };
      }
    }
    return this.record(normalized);
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
    const signature = this.events.map((event) => event.event_digest).join(":");
    if (this.cachedSnapshot !== null && this.cachedEventSignature === signature) return clone(this.cachedSnapshot);
    const body = { schema: AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA, snapshot_generation: this.snapshotGeneration + 1, previous_snapshot_digest: this.snapshotGeneration === 0 ? null : this.previousSnapshotDigest, sequence: this.events.length, head_digest: this.events.at(-1)?.event_digest ?? "", events: this.events.map(clone), retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const snapshot = await validateAutonomousModelHealthSnapshot({ ...body, snapshot_digest: await digestJson(body) }, { maxEvents: this.maxEvents });
    this.snapshotGeneration = snapshot.snapshot_generation!;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    this.cachedSnapshot = clone(snapshot);
    this.cachedEventSignature = signature;
    return clone(snapshot);
  }

  async restore(snapshot: AutonomousModelHealthSnapshot): Promise<void> {
    const validated = await validateAutonomousModelHealthSnapshot(snapshot, { maxEvents: this.maxEvents });
    this.events.splice(0, this.events.length, ...validated.events.map(clone));
    this.snapshotGeneration = validated.snapshot_generation ?? 0;
    this.previousSnapshotDigest = this.snapshotGeneration === 0 ? null : validated.snapshot_digest;
    this.cachedSnapshot = validated.schema === AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA ? clone(validated) : null;
    this.cachedEventSignature = this.cachedSnapshot === null ? null : this.events.map((event) => event.event_digest).join(":");
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

/** Validate a metadata-only model-health restart image before it reaches a live ledger. */
export async function validateAutonomousModelHealthSnapshot(
  raw: unknown,
  options: { maxEvents?: number; maxBytes?: number } = {},
): Promise<AutonomousModelHealthSnapshot> {
  const maxEvents = options.maxEvents ?? AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS;
  const maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_BYTES;
  if (!Number.isSafeInteger(maxEvents) || maxEvents < 1 || maxEvents > AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS) throw new ArgumentError("model health snapshot maxEvents is outside its bound");
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_BYTES) throw new ArgumentError("model health snapshot maxBytes is outside its bound");
  if (!isObject(raw) || !Array.isArray(raw.events)) throw new ArgumentError("model health snapshot is malformed");
  const legacy = raw.schema === LEGACY_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA;
  if (raw.schema !== AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_SCHEMA && !legacy) throw new ArgumentError("model health snapshot schema is unsupported");
  const snapshotValue = raw as unknown as Record<string, unknown>;
  const rawEvents = snapshotValue.events as unknown[];
  const allowedSnapshotKeys = new Set(legacy
    ? ["schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"]
    : ["schema", "snapshot_generation", "previous_snapshot_digest", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"]);
  for (const key of Object.keys(raw)) if (!allowedSnapshotKeys.has(key)) throw new ArgumentError("model health snapshot contains unsupported metadata");
  if (raw.retention !== PRIVATE_RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("model health snapshot retention is invalid");
  if (!legacy) {
    if (!Number.isSafeInteger(snapshotValue.snapshot_generation) || (snapshotValue.snapshot_generation as number) < 1) throw new ArgumentError("model health snapshot generation is outside its bounds");
    if (snapshotValue.previous_snapshot_digest !== null && !DIGEST.test(String(snapshotValue.previous_snapshot_digest))) throw new ArgumentError("model health previous_snapshot_digest is invalid");
    if (((snapshotValue.snapshot_generation as number) === 1) !== (snapshotValue.previous_snapshot_digest === null)) throw new ArgumentError("model health snapshot generation and previous_snapshot_digest are inconsistent");
  }
  if (!Number.isSafeInteger(snapshotValue.sequence) || (snapshotValue.sequence as number) < 0 || snapshotValue.sequence !== rawEvents.length || rawEvents.length > maxEvents) throw new ArgumentError("model health snapshot sequence is outside its bound");
  if (raw.head_digest !== "") digest("model health snapshot head_digest", raw.head_digest);
  digest("model health snapshot snapshot_digest", raw.snapshot_digest);

  const allowedEventKeys = new Set(["schema", "sequence", "observation", "previous_digest", "created_at", "event_digest", "retention", "secret_material"]);
  const events: AutonomousModelHealthEvent[] = [];
  let previous = "";
  for (let index = 0; index < rawEvents.length; index += 1) {
    const candidate = rawEvents[index];
    if (!isObject(candidate)) throw new ArgumentError(`model health event ${index + 1} is malformed`);
    for (const key of Object.keys(candidate)) if (!allowedEventKeys.has(key)) throw new ArgumentError(`model health event ${index + 1} contains unsupported metadata`);
    if (candidate.schema !== AUTONOMOUS_MODEL_HEALTH_EVENT_SCHEMA || candidate.sequence !== index + 1 || candidate.previous_digest !== previous || candidate.retention !== PRIVATE_RETENTION || candidate.secret_material !== "never_returned") throw new ArgumentError(`model health event chain is invalid at sequence ${index + 1}`);
    finite(`model health event ${index + 1} created_at`, candidate.created_at, 0, Number.MAX_SAFE_INTEGER);
    if (candidate.event_digest === "") throw new ArgumentError(`model health event ${index + 1} digest is empty`);
    digest(`model health event ${index + 1} digest`, candidate.event_digest);
    normalizeObservation(candidate.observation as unknown as AutonomousModelObservationInput);
    const { event_digest: suppliedEventDigest, ...eventBody } = candidate;
    if (await digestJson(eventBody) !== suppliedEventDigest) throw new ArgumentError(`model health snapshot digest mismatch: event ${index + 1} digest does not match its metadata`);
    const event = clone(candidate) as unknown as AutonomousModelHealthEvent;
    events.push(event);
    previous = suppliedEventDigest as string;
  }
  if (raw.head_digest !== previous) throw new ArgumentError("model health snapshot head is inconsistent");
  const { snapshot_digest: suppliedSnapshotDigest, ...snapshotBody } = raw;
  if (await digestJson(snapshotBody) !== suppliedSnapshotDigest) throw new ArgumentError("model health snapshot digest mismatch");
  const snapshot = clone(raw) as unknown as AutonomousModelHealthSnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > maxBytes) throw new ArgumentError("model health snapshot exceeds its byte bound");
  return snapshot;
}

/** Canonical JSON persistence for health state over a caller-owned text store. */
export class JsonAutonomousModelHealthSnapshotPersistence implements AutonomousModelHealthPersistence {
  protected readonly textStore: AutonomousModelHealthSnapshotTextStore;
  readonly maxEvents: number;
  readonly maxBytes: number;

  constructor(textStore: AutonomousModelHealthSnapshotTextStore, options: { maxEvents?: number; maxBytes?: number } = {}) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("model health text store is malformed");
    this.textStore = textStore;
    this.maxEvents = options.maxEvents ?? AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS;
    this.maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_BYTES;
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > AUTONOMOUS_MODEL_HEALTH_MAX_EVENTS) throw new ArgumentError("model health JSON maxEvents is outside its bound");
    if (!Number.isSafeInteger(this.maxBytes) || this.maxBytes < 1 || this.maxBytes > MAX_AUTONOMOUS_MODEL_HEALTH_SNAPSHOT_BYTES) throw new ArgumentError("model health JSON maxBytes is outside its bound");
  }

  async read(): Promise<AutonomousModelHealthSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > this.maxBytes) throw new ArgumentError("model health JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("model health JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("model health JSON is not canonical");
    return validateAutonomousModelHealthSnapshot(parsed, { maxEvents: this.maxEvents, maxBytes: this.maxBytes });
  }

  async write(snapshot: AutonomousModelHealthSnapshot): Promise<void> {
    await this.textStore.write(await this.encode(snapshot));
  }

  protected async encode(snapshot: AutonomousModelHealthSnapshot): Promise<string> {
    const validated = await validateAutonomousModelHealthSnapshot(snapshot, { maxEvents: this.maxEvents, maxBytes: this.maxBytes });
    const encoded = canonicalJson(validated);
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) throw new ArgumentError("model health JSON exceeds its byte bound");
    return encoded;
  }
}

/** Canonical JSON health persistence with atomic stale-writer fencing. */
export class TransactionalJsonAutonomousModelHealthSnapshotPersistence extends JsonAutonomousModelHealthSnapshotPersistence {
  declare protected readonly textStore: AutonomousModelHealthTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousModelHealthTransactionalSnapshotTextStore, options: { maxEvents?: number; maxBytes?: number } = {}) {
    super(textStore, options);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("model health text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousModelHealthSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("model health expected snapshot digest", expectedSnapshotDigest);
    const encoded = await this.encode(snapshot);
    const committed = await this.textStore.writeIfUnchanged(expectedSnapshotDigest, encoded);
    if (typeof committed !== "boolean") throw new ArgumentError("model health compare-and-swap returned a non-boolean result");
    return committed;
  }
}

/** Browser-compatible local text storage for model-health snapshots. */
export class WebStorageAutonomousModelHealthSnapshotTextStore implements AutonomousModelHealthSnapshotTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("model health Web Storage adapter is malformed");
    identifier("model health storage key", key, 256);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

export class AutonomousModelHealthPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousModelHealthStore, readonly persistence: AutonomousModelHealthPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("model health store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("model health persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousModelHealthSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = await validateAutonomousModelHealthSnapshot(raw);
      await this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousModelHealthSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await this.store.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("model health persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
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
      // Evaluator quality is distinct from transport success, but it is still a caller-owned
      // routing prior. Blend it into the candidate's quality with the same capped evidence
      // confidence used for reliability/latency so the canonical ranker remains policy-driven.
      const candidates = request.candidates.map((candidate) => {
        const health = persistent[`${candidate.provider}/${candidate.model}`];
        const observations = health?.quality_observations ?? 0;
        const quality = health?.quality_mean;
        if (typeof quality !== "number" || !Number.isFinite(quality) || observations <= 0) return candidate;
        const confidence = Math.min(observations / 12, 0.75);
        return { ...candidate, quality: (1 - confidence) * candidate.quality + confidence * quality };
      });
      const merged: AutonomousSelectionRequest = { ...request, candidates, model_health: { ...request.model_health, ...persistent } };
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

  async recordEvaluation(input: { provider: string; model: string; domain: string; capability: string; riskClass: string; evaluatorId: string; evaluatorVersion: string; reward: number; passed: boolean; evidenceDigest?: string | null; outcomeDigest?: string | null }): Promise<AutonomousModelHealthReceipt> {
    identifier("health evaluation evaluatorId", input.evaluatorId);
    identifier("health evaluation evaluatorVersion", input.evaluatorVersion);
    return this.store.recordEvaluation({ provider: input.provider, model: input.model, domain: input.domain, capability: input.capability, risk_class: input.riskClass, status: "evaluated", quality_reward: input.reward, quality_passed: input.passed, evidence_digest: input.evidenceDigest ?? null, outcome_digest: input.outcomeDigest ?? null, evaluator_id: input.evaluatorId, evaluator_version: input.evaluatorVersion });
  }
}

function toBrainHealthStatus(observation: AutonomousModelObservation): BrainHealthStatus {
  if (["success", "failure", "timeout", "rate_limited", "circuit_open", "unknown"].includes(observation.status)) return observation.status as BrainHealthStatus;
  return observation.outcome === "success" ? "success" : observation.outcome === "failure" ? "failure" : "unknown";
}

function totalObservationTokens(observation: AutonomousModelObservation): number | undefined {
  const tokens = [observation.input_tokens, observation.output_tokens].filter((value): value is number => value !== null);
  if (tokens.length === 0) return undefined;
  const total = tokens.reduce((sum, value) => sum + value, 0);
  if (!Number.isSafeInteger(total) || total > 1_000_000_000) throw new ArgumentError("model observation token total exceeds the control-plane bound");
  return total;
}

/** Translate local metadata-only learning signals to the Rust/Python brain control plane. */
export class AutonomousBrainControlPlaneBridge {
  constructor(readonly client: AutonomousBrainControlTransport) {
    if (!client || typeof client.brainModelHealth !== "function" || typeof client.brainReplayEvaluate !== "function") throw new ArgumentError("brain control bridge requires model-health and replay client methods");
  }

  async recordObservation(input: AutonomousModelObservationInput): Promise<RestToolResponse<BrainModelHealthResult>> {
    const observation = normalizeObservation(input);
    const args: BrainModelHealthArgs = {
      operation: "record",
      provider: observation.provider,
      model: observation.model,
      status: toBrainHealthStatus(observation),
      latency_ms: Math.round(observation.latency_ms),
    };
    const tokens = totalObservationTokens(observation);
    if (tokens !== undefined) args.tokens = tokens;
    if (observation.quality_reward !== null) args.quality = observation.quality_reward;
    return this.client.brainModelHealth(args);
  }

  async recordEvaluation(input: { provider: string; model: string; domain: string; capability: string; risk_class: string; evaluator_id: string; evaluator_version: string; reward: number; passed: boolean; evidence_digest?: string | null }): Promise<RestToolResponse<BrainModelHealthResult>> {
    const observation = normalizeObservation({
      provider: input.provider,
      model: input.model,
      domain: input.domain,
      capability: input.capability,
      risk_class: input.risk_class,
      status: "evaluated",
      outcome: "unknown",
      observation_kind: "evaluation",
      latency_ms: 0,
      quality_reward: input.reward,
      quality_passed: input.passed,
      evidence_digest: input.evidence_digest ?? null,
      evaluator_id: input.evaluator_id,
      evaluator_version: input.evaluator_version,
    });
    return this.recordObservation(observation);
  }

  snapshot(provider?: string, model?: string): Promise<RestToolResponse<BrainModelHealthResult>> {
    const args: BrainModelHealthArgs = { operation: "snapshot" };
    if (provider !== undefined) args.provider = provider;
    if (model !== undefined) args.model = model;
    return this.client.brainModelHealth(args);
  }

  /**
   * Select with restart-persisted remote model health while retaining local provider gates.
   * Remote rows can influence reliability/quality/circuit scoring, but cannot make an
   * unregistered provider or an unready local credential eligible.
   */
  selector(options: { circuitFailureThreshold?: number } = {}): AutonomousModelSelector {
    const circuitFailureThreshold = options.circuitFailureThreshold ?? 3;
    if (!Number.isSafeInteger(circuitFailureThreshold) || circuitFailureThreshold < 1 || circuitFailureThreshold > 128) throw new ArgumentError("remote health circuitFailureThreshold is outside its bounds");
    return async (request: AutonomousSelectionRequest): Promise<AutonomousSelectionDecision> => {
      const response = await this.snapshot();
      if (!response.ok || !isObject(response.mcp) || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError("remote model health snapshot returned a refusal");
      const projected = response.mcp.result?.structuredContent as BrainModelHealthResult | undefined;
      if (!projected || !Array.isArray(projected.models)) throw new ProviderRuntimeError("remote model health snapshot returned no bounded model rows");
      const remoteHealth: Record<string, ProviderHealth & { model: string; quality_mean?: number | null; quality_observations?: number }> = {};
      for (const row of projected.models) {
        if (!isObject(row) || typeof row.provider !== "string" || typeof row.model !== "string") throw new ProviderRuntimeError("remote model health snapshot contains a malformed model row");
        const rowKey = `${row.provider}/${row.model}`;
        if (remoteHealth[rowKey] !== undefined) throw new ProviderRuntimeError("remote model health snapshot contains duplicate model rows");
        const attempts = nonnegativeInteger("remote health attempts", row.attempts) ?? 0;
        const successes = nonnegativeInteger("remote health successes", row.successes) ?? 0;
        const failures = nonnegativeInteger("remote health failures", row.failures) ?? 0;
        const consecutiveFailures = nonnegativeInteger("remote health consecutive_failures", row.consecutive_failures) ?? 0;
        const averageLatency = finite("remote health average_latency_ms", row.average_latency_ms, 0, 86_400_000);
        if (successes > attempts || failures > attempts) throw new ProviderRuntimeError("remote model health snapshot contains inconsistent counters");
        const qualityMean = row.average_quality === null || row.average_quality === undefined ? null : finite("remote health average_quality", row.average_quality, 0, 1);
        const qualityObservations = row.quality_observations === undefined ? (qualityMean === null ? 0 : 1) : nonnegativeInteger("remote health quality_observations", row.quality_observations) ?? 0;
        remoteHealth[rowKey] = {
          provider: row.provider,
          model: row.model,
          circuit: row.last_status === "circuit_open" || consecutiveFailures >= circuitFailureThreshold ? "open" : "closed",
          consecutive_failures: consecutiveFailures,
          attempts,
          successes,
          failures,
          success_rate: attempts === 0 ? 0 : successes / attempts,
          mean_latency_ms: attempts === 0 ? null : averageLatency,
          last_latency_ms: attempts === 0 ? null : averageLatency,
          last_model: row.model,
          last_status_code: null,
          credential_posture: "caller_supplied_opaque_handle",
          credential_required: false,
          credential_ready: true,
          quality_mean: qualityMean,
          quality_observations: qualityObservations,
        };
      }
      const merged: AutonomousSelectionRequest = { ...request, model_health: { ...remoteHealth, ...request.model_health } };
      const ranking = rankAutonomousModels(merged);
      const chosen = ranking.find((row) => row.eligible);
      return { selected_model: chosen ? { provider: chosen.provider, model: chosen.model } : null, strategy: "caller_selector", ranking, abstention_reason: chosen ? null : "no eligible model candidate after remote persisted health" };
    };
  }

  observer(context: AutonomousHealthSelectorContext): ProviderInvocationObserver {
    identifier("remote health context domain", context.domain);
    identifier("remote health context capability", context.capability);
    identifier("remote health context riskClass", context.riskClass);
    return {
      after: async (metadata: ProviderInvocationMetadata, outcome: ProviderInvocationOutcome): Promise<void> => {
        await this.recordObservation({
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

  async replay(input: AutonomousReplayCaseInput): Promise<RestToolResponse<BrainReplayEvaluateResult>> {
    const replayCase = await normalizeReplayCase(input);
    return this.client.brainReplayEvaluate(autonomousReplayCaseToBrainArguments(replayCase));
  }
}

/** Convert a normalized local replay case to the existing Rust/Python wire contract. */
export function autonomousReplayCaseToBrainArguments(replayCase: AutonomousReplayCase): BrainReplayEvaluateArgs {
  if (!DIGEST.test(replayCase.evidence_digest)) throw new ArgumentError("replay case evidence_digest must be a lowercase SHA-256 digest");
  const result: BrainReplayEvaluateArgs = {
    case_id: replayCase.run_id,
    domain: replayCase.domain,
    capability: replayCase.capability,
    risk_class: replayCase.risk_class,
    evidence_digest: replayCase.evidence_digest,
    signals: replayCase.signals,
    references: [...replayCase.references],
    limitations: [...replayCase.limitations],
  };
  if (replayCase.required_signals !== null) result.required_signals = [...replayCase.required_signals];
  if (replayCase.signal_weights !== null) result.signal_weights = { ...replayCase.signal_weights };
  if (replayCase.pass_threshold !== null) result.pass_threshold = replayCase.pass_threshold;
  return result;
}

async function normalizeReplayCase(input: AutonomousReplayCaseInput): Promise<AutonomousReplayCase> {
  if (!isObject(input)) throw new ArgumentError("replay case must be an object");
  safeMetadata(input);
  identifier("replay run_id", input.run_id);
  if (typeof input.domain !== "string" || !input.domain) throw new ArgumentError("replay domain must be non-empty");
  identifier("replay capability", input.capability);
  identifier("replay risk_class", input.risk_class);
  identifier("replay evaluator_id", input.evaluator_id);
  identifier("replay evaluator_version", input.evaluator_version);
  if (!["completed", "failed", "incomplete"].includes(input.execution_status)) throw new ArgumentError("replay execution_status is unsupported");
  const signals = normalizeReplaySignals(input.signals);
  const references = input.references === undefined ? [] : [...input.references];
  if (references.length > 64 || references.some((reference) => typeof reference !== "string" || !DIGEST.test(reference))) throw new ArgumentError("replay references must contain at most 64 lowercase SHA-256 digests");
  const limitations = input.limitations === undefined ? [] : [...input.limitations];
  if (limitations.length > 32 || limitations.some((limitation) => typeof limitation !== "string" || limitation.length > 2_048 || /[\u0000-\u001f]/.test(limitation))) throw new ArgumentError("replay limitations must contain at most 32 bounded strings");
  let requiredSignals: string[] | null = null;
  if (input.required_signals !== undefined) {
    if (input.required_signals.length === 0 || input.required_signals.length > 64) throw new ArgumentError("replay required_signals must contain 1..64 entries");
    requiredSignals = input.required_signals.map((signal) => identifier("replay required signal", signal, 128));
    if (new Set(requiredSignals).size !== requiredSignals.length) throw new ArgumentError("replay required_signals must be unique");
  }
  let signalWeights: Record<string, number> | null = null;
  if (input.signal_weights !== undefined) {
    if (!isObject(input.signal_weights) || Object.keys(input.signal_weights).length === 0 || Object.keys(input.signal_weights).length > 64) throw new ArgumentError("replay signal_weights must contain 1..64 entries");
    signalWeights = {};
    for (const [name, weight] of Object.entries(input.signal_weights)) {
      identifier("replay signal weight", name, 128);
      signalWeights[name] = finite(`replay signal weight ${name}`, weight, Number.MIN_VALUE, Number.MAX_SAFE_INTEGER);
    }
  }
  const passThreshold = input.pass_threshold === undefined ? null : finite("replay pass_threshold", input.pass_threshold, 0, 1);
  const computedEvidenceDigest = await autonomousReplayEvidenceDigest({ domain: input.domain, capability: input.capability, risk_class: input.risk_class, signals, references, limitations });
  if (input.evidence_digest !== undefined && input.evidence_digest !== null && digest("replay evidence_digest", input.evidence_digest) !== computedEvidenceDigest) throw new ArgumentError("replay evidence_digest does not match normalized evidence");
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
    references,
    limitations,
    required_signals: requiredSignals,
    signal_weights: signalWeights,
    pass_threshold: passThreshold,
    evidence_digest: computedEvidenceDigest,
    expected_reward: input.expected_reward === undefined || input.expected_reward === null ? null : finite("replay expected_reward", input.expected_reward, 0, 1),
    expected_passed: input.expected_passed === undefined ? null : input.expected_passed,
    expected_evaluation_digest: input.expected_evaluation_digest === undefined || input.expected_evaluation_digest === null ? null : digest("replay expected_evaluation_digest", input.expected_evaluation_digest, true),
    retention: "caller_rehydrated_numeric_evidence_only",
    secret_material: "never_returned",
  };
}

/** Validate a replay report before it is persisted, displayed, or used as promotion evidence. */
export async function validateAutonomousReplayReport(raw: unknown): Promise<AutonomousReplayReport> {
  if (!isObject(raw)) throw new ArgumentError("replay report must be an object");
  safeMetadata(raw);
  const allowed = new Set(["schema", "status", "case_count", "passed_count", "failed_count", "incomplete_count", "mismatch_count", "cases", "report_digest", "retention", "secret_material"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("replay report contains unsupported fields");
  if (raw.schema !== AUTONOMOUS_REPLAY_REPORT_SCHEMA || raw.retention !== "metadata_only;provider_calls_not_replayed" || raw.secret_material !== "never_returned") throw new ArgumentError("replay report retention markers are invalid");
  if (raw.status !== "completed" && raw.status !== "mismatch" && raw.status !== "refused") throw new ArgumentError("replay report status is invalid");
  const cases = raw.cases;
  if (!Array.isArray(cases) || cases.length < 1 || cases.length > AUTONOMOUS_REPLAY_MAX_CASES) throw new ArgumentError("replay report cases are outside their bounds");
  const runIds = new Set<string>();
  const normalizedCases: AutonomousReplayCaseResult[] = [];
  for (const [index, candidate] of cases.entries()) {
    if (!isObject(candidate)) throw new ArgumentError(`replay report case ${index + 1} is malformed`);
    const caseKeys = new Set(["run_id", "domain", "status", "reward", "passed", "missing_signals", "rejected_signals", "expected_reward", "expected_passed", "expected_evaluation_digest", "evaluation_digest", "mismatch_codes"]);
    if (Object.keys(candidate).some((key) => !caseKeys.has(key))) throw new ArgumentError(`replay report case ${index + 1} contains unsupported fields`);
    const runId = identifier(`replay report case ${index + 1} run_id`, candidate.run_id);
    if (runIds.has(runId)) throw new ArgumentError("replay report run_id values must be unique");
    runIds.add(runId);
    if (typeof candidate.domain !== "string" || !candidate.domain.trim() || candidate.domain.length > 128) throw new ArgumentError(`replay report case ${index + 1} domain is invalid`);
    if (candidate.status !== "passed" && candidate.status !== "failed" && candidate.status !== "incomplete" && candidate.status !== "refused") throw new ArgumentError(`replay report case ${index + 1} status is invalid`);
    const reward = finite(`replay report case ${index + 1} reward`, candidate.reward, 0, 1);
    if (typeof candidate.passed !== "boolean") throw new ArgumentError(`replay report case ${index + 1} passed must be boolean`);
    const list = (name: string, value: unknown, maximum: number): string[] => {
      if (!Array.isArray(value) || value.length > maximum || value.some((entry) => typeof entry !== "string" || !entry.trim() || entry.length > 128)) throw new ArgumentError(`replay report case ${index + 1} ${name} is invalid`);
      const values = value as string[];
      if (new Set(values).size !== values.length) throw new ArgumentError(`replay report case ${index + 1} ${name} must be unique`);
      return [...values];
    };
    const missingSignals = list("missing_signals", candidate.missing_signals, AUTONOMOUS_REPLAY_MAX_SIGNALS);
    const rejectedSignals = list("rejected_signals", candidate.rejected_signals, AUTONOMOUS_REPLAY_MAX_SIGNALS);
    const mismatchCodes = list("mismatch_codes", candidate.mismatch_codes, 16);
    const optionalReward = candidate.expected_reward === null ? null : finite(`replay report case ${index + 1} expected_reward`, candidate.expected_reward, 0, 1);
    const optionalPassed = candidate.expected_passed === null ? null : candidate.expected_passed;
    if (optionalPassed !== null && typeof optionalPassed !== "boolean") throw new ArgumentError(`replay report case ${index + 1} expected_passed must be boolean or null`);
    const optionalDigest = (name: string, value: unknown): string | null => value === null ? null : digest(`replay report case ${index + 1} ${name}`, value);
    normalizedCases.push({ run_id: runId, domain: candidate.domain as AutonomousDomainName, status: candidate.status, reward, passed: candidate.passed, missing_signals: missingSignals, rejected_signals: rejectedSignals, expected_reward: optionalReward, expected_passed: optionalPassed, expected_evaluation_digest: optionalDigest("expected_evaluation_digest", candidate.expected_evaluation_digest), evaluation_digest: optionalDigest("evaluation_digest", candidate.evaluation_digest), mismatch_codes: mismatchCodes });
  }
  const number = (name: string, value: unknown): number => {
    if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > AUTONOMOUS_REPLAY_MAX_CASES) throw new ArgumentError(`replay report ${name} is outside its bounds`);
    return value as number;
  };
  const counts = { case_count: number("case_count", raw.case_count), passed_count: number("passed_count", raw.passed_count), failed_count: number("failed_count", raw.failed_count), incomplete_count: number("incomplete_count", raw.incomplete_count), mismatch_count: number("mismatch_count", raw.mismatch_count) };
  if (counts.case_count !== normalizedCases.length || counts.passed_count !== normalizedCases.filter((candidate) => candidate.status === "passed").length || counts.failed_count !== normalizedCases.filter((candidate) => candidate.status === "failed").length || counts.incomplete_count !== normalizedCases.filter((candidate) => candidate.status === "incomplete").length || counts.mismatch_count !== normalizedCases.filter((candidate) => candidate.mismatch_codes.length > 0).length) throw new ArgumentError("replay report counts do not match its cases");
  if ((raw.status === "completed" && counts.mismatch_count !== 0) || (raw.status === "mismatch" && counts.mismatch_count === 0) || (raw.status === "refused" && counts.mismatch_count === 0)) throw new ArgumentError("replay report status does not match its mismatch count");
  const status = raw.status as AutonomousReplayReport["status"];
  const body = { schema: AUTONOMOUS_REPLAY_REPORT_SCHEMA, status, ...counts, cases: normalizedCases, retention: "metadata_only;provider_calls_not_replayed" as const, secret_material: "never_returned" as const };
  if (typeof raw.report_digest !== "string" || raw.report_digest !== await digestJson(body)) throw new ArgumentError("replay report digest is invalid");
  if (new TextEncoder().encode(canonicalJson({ ...body, report_digest: raw.report_digest })).byteLength > MAX_AUTONOMOUS_REPLAY_REPORT_BYTES) throw new ArgumentError("replay report exceeds its byte capacity");
  return { ...body, report_digest: raw.report_digest };
}

/** Re-evaluate caller-rehydrated numeric evidence without replaying a provider or tool call. */
export class AutonomousOfflineReplayEngine {
  async replay(inputs: readonly AutonomousReplayCaseInput[]): Promise<AutonomousReplayReport> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > AUTONOMOUS_REPLAY_MAX_CASES) throw new ArgumentError("replay cases must contain 1..4096 entries");
    const cases = await Promise.all(inputs.map(normalizeReplayCase));
    if (new Set(cases.map((replayCase) => replayCase.run_id)).size !== cases.length) throw new ArgumentError("replay run_id values must be unique");
    const { builtinAutonomousDomainEvaluatorProfiles } = await import("./autonomous-learning.js");
    const profiles = await builtinAutonomousDomainEvaluatorProfiles();
    const profilesByDomain = new Map(profiles.map((profile) => [profile.domain, profile]));
    const results: AutonomousReplayCaseResult[] = [];
    for (const replayCase of cases) {
      const profile = profilesByDomain.get(replayCase.domain);
      if (!profile) {
        results.push({ run_id: replayCase.run_id, domain: replayCase.domain, status: "refused", reward: 0, passed: false, missing_signals: [], rejected_signals: [], expected_reward: replayCase.expected_reward, expected_passed: replayCase.expected_passed, expected_evaluation_digest: replayCase.expected_evaluation_digest, evaluation_digest: null, mismatch_codes: ["unsupported_domain"] });
        continue;
      }
      const required = [...new Set(replayCase.required_signals ?? profile.required_signals)].sort();
      const missing = required.filter((signal) => replayCase.signals[signal] === undefined);
      const rejected = Object.keys(replayCase.signals).filter((signal) => !required.includes(signal)).sort();
      const weights = replayCase.signal_weights ?? profile.signal_weights;
      const threshold = replayCase.pass_threshold ?? profile.pass_threshold;
      const weighted = Object.entries(weights).map(([signal, weight]) => ({ score: replayCase.signals[signal] ?? 0, weight, observed: replayCase.signals[signal] !== undefined }));
      const observedWeighted = weighted.filter((row) => row.observed);
      const weightedTotal = observedWeighted.reduce((sum, row) => sum + row.score * row.weight, 0);
      const weightTotal = observedWeighted.reduce((sum, row) => sum + row.weight, 0);
      const reward = weightTotal === 0 ? 0 : Number((weightedTotal / weightTotal).toFixed(12));
      const passed = replayCase.execution_status === "completed" && missing.length === 0 && required.every((signal) => (replayCase.signals[signal] ?? 0) >= threshold);
      const status: AutonomousReplayCaseResult["status"] = passed ? "passed" : replayCase.execution_status === "completed" && missing.length === 0 ? "failed" : "incomplete";
      const descriptor = { schema: AUTONOMOUS_REPLAY_CASE_SCHEMA, run_id: replayCase.run_id, domain: replayCase.domain, evaluator_id: profile.evaluator_id, evaluator_version: profile.evaluator_version, execution_status: replayCase.execution_status, required_signals: required, signal_weights: weights, pass_threshold: threshold, reward, passed, missing_signals: missing, rejected_signals: rejected, evidence_digest: replayCase.evidence_digest };
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
    return await validateAutonomousReplayReport({ ...reportBody, report_digest: await digestJson(reportBody) });
  }
}
