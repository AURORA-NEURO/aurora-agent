import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import type {
  AutonomousEvidenceAcquirer,
  AutonomousEvidenceAcquisitionContext,
  AutonomousEvidenceEvaluator,
  AutonomousEvidenceEvaluationInput,
  AutonomousEvidenceEvaluatorAssessmentInput,
} from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterManifest,
} from "./autonomous-evidence-adapters.js";
import {
  AutonomousEvidenceAdapterSelectionPlan,
  AutonomousEvidenceAdapterSelector,
  type AutonomousEvidenceAdapterSelectionRow,
} from "./autonomous-evidence-adapter-selection.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Restart-safe, value-free adapter health ledger schemas. */
export const AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-health/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-health-observation/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-health-event/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-health-receipt/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-health-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS = 16_384;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT = 512;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_ADAPTERS = 256;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES = 512_000;

const RETENTION = "metadata_only;raw_evidence_credentials_prompts_and_evaluator_payloads_never_persisted" as const;
const OUTCOMES = ["success", "failure", "unknown"] as const;
const OBSERVATION_KINDS = ["acquisition", "evaluation"] as const;
const IDENTIFIER = /^[A-Za-z0-9_.:+\-/ ]+$/;
const DIGEST = /^[0-9a-f]{64}$/;

export type AutonomousEvidenceAdapterHealthOutcome = typeof OUTCOMES[number];
export type AutonomousEvidenceAdapterHealthObservationKind = typeof OBSERVATION_KINDS[number];

export interface AutonomousEvidenceAdapterHealthObservationInput extends JsonObject {
  adapter_id: string;
  manifest_digest: string;
  domain: AutonomousDomainName;
  observation_kind?: AutonomousEvidenceAdapterHealthObservationKind;
  outcome: AutonomousEvidenceAdapterHealthOutcome;
  status: string;
  latency_ms: number;
  cost_units?: number | null;
  failure_class?: string | null;
  evaluator_reward?: number | null;
  evaluator_passed?: boolean | null;
  evaluator_id?: string | null;
  evaluator_version?: string | null;
  evidence_digest?: string | null;
}

export interface AutonomousEvidenceAdapterHealthObservation extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA;
  adapter_id: string;
  manifest_digest: string;
  domain: AutonomousDomainName;
  observation_kind: AutonomousEvidenceAdapterHealthObservationKind;
  outcome: AutonomousEvidenceAdapterHealthOutcome;
  status: string;
  latency_ms: number;
  cost_units: number | null;
  failure_class: string | null;
  evaluator_reward: number | null;
  evaluator_passed: boolean | null;
  evaluator_id: string | null;
  evaluator_version: string | null;
  evidence_digest: string | null;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterHealth extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA;
  adapter_id: string;
  manifest_digest: string;
  domain: AutonomousDomainName;
  attempts: number;
  successes: number;
  failures: number;
  unknown: number;
  success_rate: number;
  failure_rate: number;
  mean_latency_ms: number;
  mean_cost_units: number | null;
  quality_observations: number;
  evaluator_reward_mean: number | null;
  evaluator_pass_rate: number | null;
  consecutive_failures: number;
  last_status: string | null;
  last_outcome: AutonomousEvidenceAdapterHealthOutcome | null;
  last_sequence: number;
  circuit: "closed" | "open";
  retention: "aggregated_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterHealthEvent extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA;
  sequence: number;
  observation: AutonomousEvidenceAdapterHealthObservation;
  previous_digest: string;
  created_at: number;
  event_digest: string;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterHealthReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA;
  sequence: number;
  event_digest: string;
  adapter_id: string;
  manifest_digest: string;
  domain: AutonomousDomainName;
  observation_kind: AutonomousEvidenceAdapterHealthObservationKind;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterHealthSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  events: AutonomousEvidenceAdapterHealthEvent[];
  snapshot_digest: string;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterHealthQuery {
  adapter_id?: string;
  manifest_digest?: string;
  domain?: AutonomousDomainName;
  min_attempts?: number;
  failure_threshold?: number;
  limit?: number;
}

export interface AutonomousEvidenceAdapterHealthSelectionOptions {
  manifest_digests?: Readonly<Record<string, string>>;
  domain?: AutonomousDomainName;
  min_attempts?: number;
  failure_threshold?: number;
}

export interface AutonomousEvidenceAdapterHealthPersistence {
  read(): Promise<AutonomousEvidenceAdapterHealthSnapshot | null> | AutonomousEvidenceAdapterHealthSnapshot | null;
  write(snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceAdapterHealthSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceAdapterHealthTransactionalSnapshotTextStore extends AutonomousEvidenceAdapterHealthSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceAdapterHealthStore {
  record(input: AutonomousEvidenceAdapterHealthObservationInput): Promise<AutonomousEvidenceAdapterHealthReceipt>;
  recordAcquisition(input: Omit<AutonomousEvidenceAdapterHealthObservationInput, "observation_kind" | "outcome" | "evaluator_reward" | "evaluator_passed"> & { outcome: Exclude<AutonomousEvidenceAdapterHealthOutcome, "unknown"> }): Promise<AutonomousEvidenceAdapterHealthReceipt>;
  recordEvaluation(input: Omit<AutonomousEvidenceAdapterHealthObservationInput, "observation_kind" | "outcome" | "latency_ms" | "evaluator_reward" | "evaluator_passed"> & { evaluator_reward: number; evaluator_passed: boolean }): Promise<AutonomousEvidenceAdapterHealthReceipt>;
  health(query?: AutonomousEvidenceAdapterHealthQuery): AutonomousEvidenceAdapterHealth[];
  selectionSignals(options?: AutonomousEvidenceAdapterHealthSelectionOptions): Record<string, JsonObject>;
  snapshot(): Promise<AutonomousEvidenceAdapterHealthSnapshot>;
  restore(snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<void>;
  verifyIntegrity(): Promise<{ verified: true; events: number; head_digest: string }>;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function identifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || !IDENTIFIER.test(value)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return value.trim();
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be between ${minimum} and ${maximum}`);
  return value;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 16) throw new ArgumentError("adapter health metadata is too deeply nested");
  if (Array.isArray(value)) {
    if (value.length > 256) throw new ArgumentError("adapter health metadata contains too many entries");
    value.forEach((child) => safeMetadata(child, depth + 1));
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
    if (["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "privatekey", "prompt", "messages", "response", "raw", "rawvalue", "payload", "arguments", "output", "task", "content", "body", "headers", "input"].includes(normalized)) throw new ArgumentError("adapter health metadata contains transient or secret-shaped fields");
    safeMetadata(child, depth + 1);
  }
}

function normalizeObservation(input: AutonomousEvidenceAdapterHealthObservationInput): AutonomousEvidenceAdapterHealthObservation {
  if (!isObject(input)) throw new ArgumentError("adapter health observation must be an object");
  safeMetadata(input);
  const observationKind = input.observation_kind ?? "acquisition";
  if (!OBSERVATION_KINDS.includes(observationKind)) throw new ArgumentError("adapter health observation kind is unsupported");
  if (!OUTCOMES.includes(input.outcome)) throw new ArgumentError("adapter health observation outcome is unsupported");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(input.domain)) throw new ArgumentError("adapter health observation domain is unsupported");
  const reward = input.evaluator_reward === undefined || input.evaluator_reward === null ? null : finite("adapter health evaluator_reward", input.evaluator_reward, -1, 1);
  const passed = input.evaluator_passed === undefined || input.evaluator_passed === null ? null : input.evaluator_passed;
  if (passed !== null && typeof passed !== "boolean") throw new ArgumentError("adapter health evaluator_passed must be boolean or null");
  if (observationKind === "evaluation" && reward === null) throw new ArgumentError("adapter health evaluation requires an explicit evaluator reward");
  if (observationKind === "evaluation" && input.outcome !== "unknown") throw new ArgumentError("adapter health evaluation outcome must be unknown");
  return {
    schema: AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_OBSERVATION_SCHEMA,
    adapter_id: identifier("adapter health adapter_id", input.adapter_id),
    manifest_digest: digest("adapter health manifest_digest", input.manifest_digest),
    domain: input.domain,
    observation_kind: observationKind,
    outcome: input.outcome,
    status: identifier("adapter health status", input.status),
    latency_ms: finite("adapter health latency_ms", input.latency_ms, 0, 86_400_000),
    cost_units: input.cost_units === undefined || input.cost_units === null ? null : finite("adapter health cost_units", input.cost_units, 0, 1_000_000),
    failure_class: input.failure_class === undefined || input.failure_class === null ? null : identifier("adapter health failure_class", input.failure_class),
    evaluator_reward: reward,
    evaluator_passed: passed,
    evaluator_id: input.evaluator_id === undefined || input.evaluator_id === null ? null : identifier("adapter health evaluator_id", input.evaluator_id),
    evaluator_version: input.evaluator_version === undefined || input.evaluator_version === null ? null : identifier("adapter health evaluator_version", input.evaluator_version),
    evidence_digest: input.evidence_digest === undefined || input.evidence_digest === null ? null : digest("adapter health evidence_digest", input.evidence_digest),
    retention: RETENTION,
    secret_material: "never_returned",
  };
}

interface Aggregate {
  adapter_id: string;
  manifest_digest: string;
  domain: AutonomousDomainName;
  attempts: number;
  successes: number;
  failures: number;
  unknown: number;
  total_latency: number;
  total_cost: number;
  cost_observations: number;
  quality_observations: number;
  reward_total: number;
  quality_passed: number;
  consecutive_failures: number;
  last_status: string | null;
  last_outcome: AutonomousEvidenceAdapterHealthOutcome | null;
  last_sequence: number;
}

function emptyAggregate(adapter_id: string, manifest_digest: string, domain: AutonomousDomainName): Aggregate {
  return { adapter_id, manifest_digest, domain, attempts: 0, successes: 0, failures: 0, unknown: 0, total_latency: 0, total_cost: 0, cost_observations: 0, quality_observations: 0, reward_total: 0, quality_passed: 0, consecutive_failures: 0, last_status: null, last_outcome: null, last_sequence: 0 };
}

function aggregateKey(adapterId: string, manifestDigest: string, domain: AutonomousDomainName): string {
  return `${adapterId}\u0000${manifestDigest}\u0000${domain}`;
}

function projectAggregate(entry: Aggregate, minAttempts: number, failureThreshold: number): AutonomousEvidenceAdapterHealth {
  const attempts = entry.attempts;
  const failures = entry.failures;
  return {
    schema: AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SCHEMA,
    adapter_id: entry.adapter_id,
    manifest_digest: entry.manifest_digest,
    domain: entry.domain,
    attempts,
    successes: entry.successes,
    failures,
    unknown: entry.unknown,
    success_rate: attempts === 0 ? 0 : entry.successes / attempts,
    failure_rate: attempts === 0 ? 0 : failures / attempts,
    mean_latency_ms: attempts === 0 ? 0 : entry.total_latency / attempts,
    mean_cost_units: entry.cost_observations === 0 ? null : entry.total_cost / entry.cost_observations,
    quality_observations: entry.quality_observations,
    evaluator_reward_mean: entry.quality_observations === 0 ? null : entry.reward_total / entry.quality_observations,
    evaluator_pass_rate: entry.quality_observations === 0 ? null : entry.quality_passed / entry.quality_observations,
    consecutive_failures: entry.consecutive_failures,
    last_status: entry.last_status,
    last_outcome: entry.last_outcome,
    last_sequence: entry.last_sequence,
    circuit: attempts >= minAttempts && failures / attempts >= failureThreshold ? "open" : "closed",
    retention: "aggregated_metadata_only",
    secret_material: "never_returned",
  };
}

/** In-memory reference ledger; production applications can provide the persistence seam below. */
export class InMemoryAutonomousEvidenceAdapterHealthStore implements AutonomousEvidenceAdapterHealthStore {
  private readonly events: AutonomousEvidenceAdapterHealthEvent[] = [];
  readonly maxEvents: number;
  private readonly clock: () => number;

  constructor(options: { maxEvents?: number; clock?: () => number } = {}) {
    this.maxEvents = options.maxEvents ?? MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS;
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS) throw new ArgumentError("adapter health maxEvents is outside its bounds");
    this.clock = options.clock ?? (() => Date.now() / 1_000);
  }

  async record(input: AutonomousEvidenceAdapterHealthObservationInput): Promise<AutonomousEvidenceAdapterHealthReceipt> {
    const observation = normalizeObservation(input);
    if (this.events.length >= this.maxEvents) throw new ArgumentError("adapter health event capacity is exhausted");
    const base = {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA,
      sequence: this.events.length + 1,
      observation,
      previous_digest: this.events.at(-1)?.event_digest ?? "",
      created_at: finite("adapter health clock", this.clock(), 0, Number.MAX_SAFE_INTEGER),
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    const event: AutonomousEvidenceAdapterHealthEvent = { ...base, event_digest: await digestJson(base) };
    this.events.push(event);
    return {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_RECEIPT_SCHEMA,
      sequence: event.sequence,
      event_digest: event.event_digest,
      adapter_id: observation.adapter_id,
      manifest_digest: observation.manifest_digest,
      domain: observation.domain,
      observation_kind: observation.observation_kind,
      retention: RETENTION,
      secret_material: "never_returned",
    };
  }

  recordAcquisition(input: Omit<AutonomousEvidenceAdapterHealthObservationInput, "observation_kind" | "outcome" | "evaluator_reward" | "evaluator_passed"> & { outcome: Exclude<AutonomousEvidenceAdapterHealthOutcome, "unknown"> }): Promise<AutonomousEvidenceAdapterHealthReceipt> {
    const observation = { ...input, observation_kind: "acquisition" as const } as AutonomousEvidenceAdapterHealthObservationInput;
    return this.record(observation);
  }

  recordEvaluation(input: Omit<AutonomousEvidenceAdapterHealthObservationInput, "observation_kind" | "outcome" | "latency_ms" | "evaluator_reward" | "evaluator_passed"> & { evaluator_reward: number; evaluator_passed: boolean }): Promise<AutonomousEvidenceAdapterHealthReceipt> {
    const observation = { ...input, observation_kind: "evaluation" as const, outcome: "unknown" as const, latency_ms: 0 } as AutonomousEvidenceAdapterHealthObservationInput;
    return this.record(observation);
  }

  health(query: AutonomousEvidenceAdapterHealthQuery = {}): AutonomousEvidenceAdapterHealth[] {
    safeMetadata(query);
    const minAttempts = query.min_attempts ?? 3;
    const failureThreshold = query.failure_threshold ?? 0.75;
    const limit = query.limit ?? MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT;
    if (!Number.isSafeInteger(minAttempts) || minAttempts < 1) throw new ArgumentError("adapter health min_attempts must be positive");
    finite("adapter health failure_threshold", failureThreshold, 0, 1);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT) throw new ArgumentError("adapter health limit is outside its bounds");
    if (query.adapter_id !== undefined) identifier("adapter health query adapter_id", query.adapter_id);
    if (query.manifest_digest !== undefined) digest("adapter health query manifest_digest", query.manifest_digest);
    if (query.domain !== undefined && !AUTONOMOUS_DOMAIN_NAMES.includes(query.domain)) throw new ArgumentError("adapter health query domain is unsupported");
    const aggregates = new Map<string, Aggregate>();
    for (const event of this.events) {
      const observation = event.observation;
      if (query.adapter_id !== undefined && observation.adapter_id !== query.adapter_id) continue;
      if (query.manifest_digest !== undefined && observation.manifest_digest !== query.manifest_digest) continue;
      if (query.domain !== undefined && observation.domain !== query.domain) continue;
      const key = aggregateKey(observation.adapter_id, observation.manifest_digest, observation.domain);
      const entry = aggregates.get(key) ?? emptyAggregate(observation.adapter_id, observation.manifest_digest, observation.domain);
      if (observation.observation_kind === "acquisition") {
        entry.attempts += 1;
        if (observation.outcome === "success") {
          entry.successes += 1;
          entry.consecutive_failures = 0;
        } else if (observation.outcome === "failure") {
          entry.failures += 1;
          entry.consecutive_failures += 1;
        } else {
          entry.unknown += 1;
        }
        entry.total_latency += observation.latency_ms;
        entry.last_status = observation.status;
        entry.last_outcome = observation.outcome;
      }
      if (observation.cost_units !== null) {
        entry.total_cost += observation.cost_units;
        entry.cost_observations += 1;
      }
      if (observation.evaluator_reward !== null) {
        entry.quality_observations += 1;
        entry.reward_total += observation.evaluator_reward;
        if (observation.evaluator_passed === true) entry.quality_passed += 1;
      }
      entry.last_sequence = event.sequence;
      aggregates.set(key, entry);
    }
    return [...aggregates.values()]
      .map((entry) => projectAggregate(entry, minAttempts, failureThreshold))
      .sort((left, right) => right.attempts - left.attempts || left.adapter_id.localeCompare(right.adapter_id) || left.domain.localeCompare(right.domain))
      .slice(0, limit)
      .map(clone);
  }

  selectionSignals(options: AutonomousEvidenceAdapterHealthSelectionOptions = {}): Record<string, JsonObject> {
    safeMetadata(options);
    const minAttempts = options.min_attempts ?? 3;
    const failureThreshold = options.failure_threshold ?? 0.75;
    if (!Number.isSafeInteger(minAttempts) || minAttempts < 1) throw new ArgumentError("adapter health selection min_attempts must be positive");
    finite("adapter health selection failure_threshold", failureThreshold, 0, 1);
    const manifestDigests = options.manifest_digests ?? {};
    const normalizedManifestDigests: Record<string, string> = {};
    for (const [adapterId, manifestDigest] of Object.entries(manifestDigests)) {
      normalizedManifestDigests[identifier("adapter health selection adapter_id", adapterId)] = digest("adapter health selection manifest_digest", manifestDigest);
    }
    const aggregates = new Map<string, Aggregate>();
    for (const row of this.health({ domain: options.domain, min_attempts: minAttempts, failure_threshold: failureThreshold, limit: MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_QUERY_LIMIT })) {
      const currentDigest = normalizedManifestDigests[row.adapter_id];
      if (currentDigest !== undefined && currentDigest !== row.manifest_digest) continue;
      const key = `${row.adapter_id}\u0000${row.manifest_digest}`;
      const entry = aggregates.get(key) ?? emptyAggregate(row.adapter_id, row.manifest_digest, row.domain);
      entry.attempts += row.attempts;
      entry.successes += row.successes;
      entry.failures += row.failures;
      entry.unknown += row.unknown;
      entry.total_latency += row.mean_latency_ms * row.attempts;
      if (row.mean_cost_units !== null) {
        entry.total_cost += row.mean_cost_units * Math.max(1, row.attempts);
        entry.cost_observations += Math.max(1, row.attempts);
      }
      entry.quality_observations += row.quality_observations;
      if (row.evaluator_reward_mean !== null) entry.reward_total += row.evaluator_reward_mean * row.quality_observations;
      if (row.evaluator_pass_rate !== null) entry.quality_passed += row.evaluator_pass_rate * row.quality_observations;
      entry.consecutive_failures = Math.max(entry.consecutive_failures, row.consecutive_failures);
      entry.last_sequence = Math.max(entry.last_sequence, row.last_sequence);
      entry.last_status = row.last_status;
      entry.last_outcome = row.last_outcome;
      aggregates.set(key, entry);
    }
    const signals: Record<string, JsonObject> = {};
    for (const entry of aggregates.values()) {
      const attempts = entry.attempts;
      const failures = entry.failures;
      signals[entry.adapter_id] = {
        eligible: attempts > 0 && !(attempts >= minAttempts && failures / attempts >= failureThreshold),
        health: attempts === 0 ? 0 : entry.successes / attempts,
        success_rate: attempts === 0 ? 0 : entry.successes / attempts,
        evaluator_reward: entry.quality_observations === 0 ? 0 : entry.reward_total / entry.quality_observations,
        latency_ms: attempts === 0 ? null : entry.total_latency / attempts,
        cost_units: entry.cost_observations === 0 ? null : entry.total_cost / entry.cost_observations,
      };
    }
    return signals;
  }

  async snapshot(): Promise<AutonomousEvidenceAdapterHealthSnapshot> {
    const body = {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA,
      sequence: this.events.length,
      head_digest: this.events.at(-1)?.event_digest ?? "",
      events: this.events.map(clone),
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    return { ...body, snapshot_digest: await digestJson(body) };
  }

  async restore(snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<void> {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.events)) throw new ArgumentError("adapter health snapshot is malformed");
    if (snapshot.retention !== RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("adapter health snapshot retention is invalid");
    const { snapshot_digest: supplied, ...body } = snapshot;
    if (await digestJson(body) !== supplied) throw new ArgumentError("adapter health snapshot digest mismatch");
    if (snapshot.events.length > this.maxEvents) throw new ArgumentError("adapter health snapshot exceeds store capacity");
    const events = snapshot.events.map(clone);
    const verified = await this.verifyEvents(events);
    if (verified.events !== snapshot.sequence || verified.head_digest !== snapshot.head_digest) throw new ArgumentError("adapter health snapshot head is inconsistent");
    this.events.splice(0, this.events.length, ...events);
  }

  async verifyIntegrity(): Promise<{ verified: true; events: number; head_digest: string }> {
    return this.verifyEvents(this.events);
  }

  private async verifyEvents(events: readonly AutonomousEvidenceAdapterHealthEvent[]): Promise<{ verified: true; events: number; head_digest: string }> {
    let previous = "";
    for (let index = 0; index < events.length; index += 1) {
      const event = events[index]!;
      if (!isObject(event) || event.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA || event.sequence !== index + 1 || event.previous_digest !== previous) throw new ArgumentError(`adapter health hash chain breaks at sequence ${event.sequence}`);
      const { event_digest: _eventDigest, ...body } = event;
      if (await digestJson(body) !== event.event_digest) throw new ArgumentError(`adapter health event digest mismatch at sequence ${event.sequence}`);
      normalizeObservation(event.observation);
      finite("adapter health event created_at", event.created_at, 0, Number.MAX_SAFE_INTEGER);
      previous = event.event_digest;
    }
    return { verified: true, events: events.length, head_digest: previous };
  }
}

function snapshotBytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

/** Validate a health snapshot before it reaches a caller-owned persistence adapter. */
export function validateAutonomousEvidenceAdapterHealthSnapshot(raw: unknown, maxEvents = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS): AutonomousEvidenceAdapterHealthSnapshot {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_SCHEMA || !Array.isArray(raw.events)) throw new ArgumentError("adapter health snapshot is malformed");
  const snapshot = raw as unknown as AutonomousEvidenceAdapterHealthSnapshot;
  if (snapshot.retention !== RETENTION || snapshot.secret_material !== "never_returned") throw new ArgumentError("adapter health snapshot retention is invalid");
  if (!Number.isSafeInteger(snapshot.sequence) || snapshot.sequence < 0 || snapshot.events.length !== snapshot.sequence) throw new ArgumentError("adapter health snapshot sequence is inconsistent");
  if (!Number.isSafeInteger(maxEvents) || maxEvents < 1 || maxEvents > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS || snapshot.events.length > maxEvents) throw new ArgumentError("adapter health snapshot exceeds its bound");
  digest("adapter health snapshot head_digest", snapshot.head_digest);
  digest("adapter health snapshot snapshot_digest", snapshot.snapshot_digest);
  const { snapshot_digest: supplied, ...descriptor } = snapshot;
  const encoded = canonicalJson(descriptor);
  if (snapshotBytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES || digestJsonSync(descriptor) !== supplied) throw new ArgumentError("adapter health snapshot digest or byte bound is invalid");
  let previous = "";
  for (let index = 0; index < snapshot.events.length; index += 1) {
    const event = snapshot.events[index];
    if (!isObject(event) || event.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENT_SCHEMA || event.sequence !== index + 1 || event.previous_digest !== previous || event.retention !== RETENTION || event.secret_material !== "never_returned") throw new ArgumentError(`adapter health event chain is invalid at sequence ${index + 1}`);
    const { event_digest: suppliedEventDigest, ...eventBody } = event;
    digest(`adapter health event ${index + 1} digest`, suppliedEventDigest);
    if (digestJsonSync(eventBody) !== suppliedEventDigest) throw new ArgumentError(`adapter health event digest is invalid at sequence ${index + 1}`);
    normalizeObservation(event.observation as unknown as AutonomousEvidenceAdapterHealthObservationInput);
    finite(`adapter health event ${index + 1} created_at`, event.created_at, 0, Number.MAX_SAFE_INTEGER);
    previous = suppliedEventDigest;
  }
  if (previous !== snapshot.head_digest) throw new ArgumentError("adapter health snapshot head digest is inconsistent");
  return clone(snapshot);
}

/** Portable JSON persistence for Node, browser, SQLite bridges, and embedded text stores. */
export class JsonAutonomousEvidenceAdapterHealthPersistence implements AutonomousEvidenceAdapterHealthPersistence {
  protected readonly store: AutonomousEvidenceAdapterHealthSnapshotTextStore;
  readonly maxEvents: number;
  readonly maxBytes: number;

  constructor(
    store: AutonomousEvidenceAdapterHealthSnapshotTextStore,
    maxEvents = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS,
    maxBytes = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES,
  ) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("adapter health JSON persistence requires a text store");
    if (!Number.isSafeInteger(maxEvents) || maxEvents < 1 || maxEvents > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS) throw new ArgumentError("adapter health JSON persistence maxEvents is outside its bound");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES) throw new ArgumentError("adapter health JSON persistence maxBytes is outside its bound");
    this.store = store;
    this.maxEvents = maxEvents;
    this.maxBytes = maxBytes;
  }

  async read(): Promise<AutonomousEvidenceAdapterHealthSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || snapshotBytes(encoded) > this.maxBytes) throw new ArgumentError("adapter health JSON persistence text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("adapter health JSON persistence text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("adapter health JSON persistence text is not canonical for its digest-bound snapshot");
    const snapshot = validateAutonomousEvidenceAdapterHealthSnapshot(parsed, this.maxEvents);
    if (snapshotBytes(canonicalJson(snapshot)) > this.maxBytes) throw new ArgumentError("adapter health JSON persistence snapshot exceeds its bound");
    return snapshot;
  }

  async write(snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<void> {
    await this.store.write(this.encode(snapshot));
  }

  protected encode(snapshot: AutonomousEvidenceAdapterHealthSnapshot): string {
    const validated = validateAutonomousEvidenceAdapterHealthSnapshot(snapshot, this.maxEvents);
    const encoded = canonicalJson(validated);
    if (snapshotBytes(encoded) > this.maxBytes) throw new ArgumentError("adapter health JSON persistence snapshot exceeds its bound");
    return encoded;
  }
}

/** JSON persistence variant with an atomic digest fence for multi-host health writers. */
export class TransactionalJsonAutonomousEvidenceAdapterHealthPersistence extends JsonAutonomousEvidenceAdapterHealthPersistence {
  private readonly transactionalStore: AutonomousEvidenceAdapterHealthTransactionalSnapshotTextStore;

  constructor(
    store: AutonomousEvidenceAdapterHealthTransactionalSnapshotTextStore,
    maxEvents = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_EVENTS,
    maxBytes = MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES,
  ) {
    super(store, maxEvents, maxBytes);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional adapter health JSON persistence requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousEvidenceAdapterHealthSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("adapter health expected snapshot digest", expectedSnapshotDigest);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedSnapshotDigest, this.encode(snapshot));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional adapter health persistence returned a non-boolean commit result");
    return committed;
  }
}

/** Browser-compatible single-writer text store for localStorage/sessionStorage-like objects. */
export class WebStorageAutonomousEvidenceAdapterHealthSnapshotTextStore implements AutonomousEvidenceAdapterHealthSnapshotTextStore {
  readonly storage: Pick<Storage, "getItem" | "setItem">;
  readonly key: string;

  constructor(storage: Pick<Storage, "getItem" | "setItem">, key = "aurora.autonomous.evidence.adapter.health") {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("adapter health web storage requires getItem and setItem");
    if (typeof key !== "string" || !key.trim() || key.length > 256 || key.includes("\u0000")) throw new ArgumentError("adapter health web storage key is outside its bound");
    this.storage = storage;
    this.key = key;
  }

  read(): string | null {
    return this.storage.getItem(this.key);
  }

  write(value: string): void {
    if (typeof value !== "string" || snapshotBytes(value) > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_HEALTH_SNAPSHOT_BYTES) throw new ArgumentError("adapter health web storage value exceeds its bound");
    this.storage.setItem(this.key, value);
  }
}

export class AutonomousEvidenceAdapterHealthPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private pending: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousEvidenceAdapterHealthStore, readonly persistence: AutonomousEvidenceAdapterHealthPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("adapter health store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("adapter health persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousEvidenceAdapterHealthSnapshot | null> {
    return this.serialized(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot) {
        await this.store.restore(snapshot);
        this.expectedSnapshotDigest = snapshot.snapshot_digest;
      } else {
        this.expectedSnapshotDigest = null;
      }
      return snapshot;
    });
  }

  async flush(): Promise<AutonomousEvidenceAdapterHealthSnapshot> {
    return this.serialized(async () => {
      const snapshot = await this.store.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        const committed = await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot);
        if (!committed) throw new ArgumentError("adapter health persistence rejected a stale writer");
      } else {
        await this.persistence.write(snapshot);
      }
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  private async serialized<T>(work: () => Promise<T>): Promise<T> {
    let release!: () => void;
    const next = new Promise<void>((resolve) => { release = resolve; });
    const previous = this.pending;
    this.pending = next;
    await previous;
    try {
      return await work();
    } finally {
      release();
    }
  }
}

export interface AutonomousEvidenceAdapterHealthSelectionBridgeOptions {
  capability?: string | null;
  minScore?: number;
  minMargin?: number;
  min_attempts?: number;
  failure_threshold?: number;
}

export interface AutonomousEvidenceAdapterHealthAcquirerOptions {
  clock?: () => number;
  cost_units_by_adapter?: Readonly<Record<string, number>>;
}

/** Connects the value-only runtime to health observation and future adaptive selection. */
export class AutonomousEvidenceAdapterHealthController {
  readonly selector: AutonomousEvidenceAdapterSelector;

  constructor(readonly store: AutonomousEvidenceAdapterHealthStore, readonly registry: AutonomousEvidenceAdapterRegistry) {
    if (!store || typeof store.record !== "function" || typeof store.health !== "function" || typeof store.selectionSignals !== "function") throw new ArgumentError("adapter health controller requires a typed health store");
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("adapter health controller requires a typed adapter registry");
    this.selector = new AutonomousEvidenceAdapterSelector(registry);
  }

  /** Select each requested domain against its own persisted history, then combine one digest-bound plan. */
  async selectAdaptiveForDomains(domains: readonly AutonomousDomainName[], options: AutonomousEvidenceAdapterHealthSelectionBridgeOptions = {}): Promise<AutonomousEvidenceAdapterSelectionPlan> {
    const manifests = this.registry.manifests();
    const manifestDigests = Object.fromEntries(manifests.map((manifest) => [manifest.adapter_id, manifest.manifest_digest]));
    const rows: AutonomousEvidenceAdapterSelectionRow[] = [];
    const signalEvidence: JsonObject[] = [];
    for (const domain of domains) {
      const signals = this.store.selectionSignals({ domain, manifest_digests: manifestDigests, min_attempts: options.min_attempts, failure_threshold: options.failure_threshold });
      const partial = this.selector.selectAdaptiveForDomains([domain], signals, { capability: options.capability, minScore: options.minScore, minMargin: options.minMargin });
      rows.push(partial.rows[0]!);
      signalEvidence.push({ domain, signals });
    }
    return new AutonomousEvidenceAdapterSelectionPlan({
      domains,
      capability: options.capability ?? null,
      registry_digest: this.registry.toJSON().registry_digest,
      rows,
      strategy: "weighted_evidence",
      signal_digest: digestJsonSync(signalEvidence),
    });
  }

  createObservedAcquirerFromSelection(
    plan: AutonomousEvidenceAdapterSelectionPlan | unknown,
    options: AutonomousEvidenceAdapterHealthAcquirerOptions = {},
  ): AutonomousEvidenceAcquirer {
    const resolved = this.resolveSelection(plan);
    return this.createObservedAcquirer(this.selector.createAcquirerFromSelection(resolved.plan), resolved.route, options);
  }

  createObservedEvaluatorFromSelection(
    plan: AutonomousEvidenceAdapterSelectionPlan | unknown,
    evaluator: AutonomousEvidenceEvaluator,
  ): AutonomousEvidenceEvaluator {
    const resolved = this.resolveSelection(plan);
    return this.createObservedEvaluator(evaluator, resolved.route);
  }

  createObservedAcquirer(
    acquirer: AutonomousEvidenceAcquirer,
    adapterIdForDomain: Readonly<Record<AutonomousDomainName, string>>,
    options: AutonomousEvidenceAdapterHealthAcquirerOptions = {},
  ): AutonomousEvidenceAcquirer {
    if (!acquirer || typeof acquirer.acquire !== "function") throw new ArgumentError("observed adapter acquirer is malformed");
    const clock = options.clock ?? (() => Date.now());
    const costs = options.cost_units_by_adapter ?? {};
    for (const [adapterId, cost] of Object.entries(costs)) {
      identifier("adapter health cost adapter_id", adapterId);
      finite("adapter health cost_units_by_adapter", cost, 0, 1_000_000);
    }
    return {
      acquire: async (context: AutonomousEvidenceAcquisitionContext): Promise<JsonValue> => {
        const manifest = this.manifestForContext(context, adapterIdForDomain);
        const started = finite("adapter health acquisition clock", clock(), 0, Number.MAX_SAFE_INTEGER);
        try {
          const value = await acquirer.acquire(context);
          await this.store.recordAcquisition({
            adapter_id: manifest.adapter_id,
            manifest_digest: manifest.manifest_digest,
            domain: manifest.domains.includes(context.requirement.domain) ? context.requirement.domain : manifest.domains[0]!,
            outcome: "success",
            status: "success",
            latency_ms: Math.max(0, finite("adapter health acquisition clock", clock(), 0, Number.MAX_SAFE_INTEGER) - started),
            cost_units: costs[manifest.adapter_id] ?? null,
            evidence_digest: context.request.source_digest ?? null,
          });
          return value;
        } catch (error) {
          await this.store.recordAcquisition({
            adapter_id: manifest.adapter_id,
            manifest_digest: manifest.manifest_digest,
            domain: manifest.domains.includes(context.requirement.domain) ? context.requirement.domain : manifest.domains[0]!,
            outcome: "failure",
            status: "failure",
            latency_ms: Math.max(0, finite("adapter health acquisition clock", clock(), 0, Number.MAX_SAFE_INTEGER) - started),
            cost_units: costs[manifest.adapter_id] ?? null,
            failure_class: error instanceof Error ? error.constructor.name : "acquisition_failed",
            evidence_digest: context.request.source_digest ?? null,
          });
          throw error;
        }
      },
    };
  }

  createObservedEvaluator(
    evaluator: AutonomousEvidenceEvaluator,
    adapterIdForDomain: Readonly<Record<AutonomousDomainName, string>>,
  ): AutonomousEvidenceEvaluator {
    if (!evaluator || typeof evaluator.evaluate !== "function") throw new ArgumentError("observed adapter evaluator is malformed");
    const evaluatorId = identifier("adapter health evaluator_id", evaluator.evaluator_id);
    const evaluatorVersion = identifier("adapter health evaluator_version", evaluator.evaluator_version);
    return {
      evaluator_id: evaluator.evaluator_id,
      evaluator_version: evaluator.evaluator_version,
      evaluate: async (input: AutonomousEvidenceEvaluationInput): Promise<AutonomousEvidenceEvaluatorAssessmentInput> => {
        const manifest = this.manifestForRequirement(input.requirement.domain, adapterIdForDomain);
        try {
          const decision = await evaluator.evaluate(input);
          const score = finite("adapter health evaluator score", decision.score, 0, 1);
          await this.store.recordEvaluation({
            adapter_id: manifest.adapter_id,
            manifest_digest: manifest.manifest_digest,
            domain: input.requirement.domain,
            status: `verdict_${decision.verdict}`,
            evaluator_reward: score * 2 - 1,
            evaluator_passed: decision.verdict === "accepted",
            evaluator_id: evaluatorId,
            evaluator_version: evaluatorVersion,
            evidence_digest: decision.evidence_digest ?? null,
          });
          return decision;
        } catch (error) {
          await this.store.recordEvaluation({
            adapter_id: manifest.adapter_id,
            manifest_digest: manifest.manifest_digest,
            domain: input.requirement.domain,
            status: "evaluation_failed",
            evaluator_reward: -1,
            evaluator_passed: false,
            evaluator_id: evaluatorId,
            evaluator_version: evaluatorVersion,
            failure_class: error instanceof Error ? error.constructor.name : "evaluation_failed",
          });
          throw error;
        }
      },
    };
  }

  private manifestForRequirement(domain: AutonomousDomainName, adapterIdForDomain: Readonly<Record<AutonomousDomainName, string>>): AutonomousEvidenceAdapterManifest {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("adapter health route domain is unsupported");
    const adapterId = adapterIdForDomain[domain];
    if (typeof adapterId !== "string") throw new ArgumentError(`adapter health route is missing for ${domain}`);
    return this.registry.resolve(domain, adapterId);
  }

  private manifestForContext(context: AutonomousEvidenceAcquisitionContext, adapterIdForDomain: Readonly<Record<AutonomousDomainName, string>>): AutonomousEvidenceAdapterManifest {
    if (!context || !context.requirement || !context.request) throw new ArgumentError("adapter health acquisition context is malformed");
    return this.manifestForRequirement(context.requirement.domain, adapterIdForDomain);
  }

  private resolveSelection(plan: AutonomousEvidenceAdapterSelectionPlan | unknown): { plan: AutonomousEvidenceAdapterSelectionPlan; route: Record<AutonomousDomainName, string> } {
    const typedPlan = plan instanceof AutonomousEvidenceAdapterSelectionPlan ? plan : AutonomousEvidenceAdapterSelectionPlan.fromJSON(plan);
    typedPlan.verify(this.registry);
    const route = {} as Record<AutonomousDomainName, string>;
    for (const row of typedPlan.rows) {
      if (row.status !== "selected" || row.adapter_id === null) throw new ArgumentError(`adapter health selection is incomplete for ${row.domain}`);
      route[row.domain] = row.adapter_id;
    }
    return { plan: typedPlan, route };
  }
}
