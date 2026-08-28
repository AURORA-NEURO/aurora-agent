import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only provider/model quota state used by the real LLM dispatch boundary. */
export const PROVIDER_QUOTA_SCHEMA = "bioprism-provider-quota/0.1" as const;
export const PROVIDER_QUOTA_SNAPSHOT_SCHEMA = "bioprism-provider-quota-snapshot/0.1" as const;
export const PROVIDER_QUOTA_RETENTION = "metadata_only;provider_model_counters_no_prompts_credentials_or_payloads" as const;
export const PROVIDER_QUOTA_SECRET_MATERIAL = "never_returned" as const;
export const MAX_PROVIDER_QUOTA_POLICIES = 256;
export const MAX_PROVIDER_QUOTA_BUCKETS = 2_048;
export const MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES = 2_000_000;
export const MAX_PROVIDER_QUOTA_WINDOW_MS = 7 * 24 * 60 * 60 * 1000;
export const MAX_PROVIDER_QUOTA_METRIC = 2_000_000_000;
export const MAX_PROVIDER_QUOTA_COST_UNITS = 1_000_000_000;
export const MAX_PROVIDER_QUOTA_TIMESTAMP = Number.MAX_SAFE_INTEGER;

export interface ProviderQuotaPolicyInput extends JsonObject {
  provider: string;
  model?: string | null;
  windowMs: number;
  maxRequests?: number | null;
  maxInputTokens?: number | null;
  maxOutputTokens?: number | null;
  maxTotalTokens?: number | null;
  maxCostUnits?: number | null;
  maxConcurrent?: number | null;
}

export interface ProviderQuotaPolicy extends JsonObject {
  policy_id: string;
  provider: string;
  model: string | null;
  window_ms: number;
  max_requests: number | null;
  max_input_tokens: number | null;
  max_output_tokens: number | null;
  max_total_tokens: number | null;
  max_cost_units: number | null;
  max_concurrent: number | null;
}

export interface ProviderQuotaReservationInput extends JsonObject {
  provider: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
  costUnits?: number;
}

export interface ProviderQuotaSettlementInput extends JsonObject {
  inputTokens?: number;
  outputTokens?: number;
  costUnits?: number;
}

export interface ProviderQuotaSettlement extends JsonObject {
  schema: typeof PROVIDER_QUOTA_SCHEMA;
  reservation_id: string;
  provider: string;
  model: string;
  dispatched: boolean;
  charged_requests: number;
  charged_input_tokens: number;
  charged_output_tokens: number;
  charged_cost_units: number;
  over_limit_dimensions: string[];
  retention: typeof PROVIDER_QUOTA_RETENTION;
  secret_material: typeof PROVIDER_QUOTA_SECRET_MATERIAL;
}

export interface ProviderQuotaStatus extends JsonObject {
  schema: typeof PROVIDER_QUOTA_SCHEMA;
  policy_id: string;
  provider: string;
  model: string | null;
  window_start: number;
  window_ends_at: number;
  requests_used: number;
  requests_reserved: number;
  input_tokens_used: number;
  input_tokens_reserved: number;
  output_tokens_used: number;
  output_tokens_reserved: number;
  total_tokens_used: number;
  total_tokens_reserved: number;
  cost_units_used: number;
  cost_units_reserved: number;
  concurrent: number;
  next_window_at: number;
  limits: {
    max_requests: number | null;
    max_input_tokens: number | null;
    max_output_tokens: number | null;
    max_total_tokens: number | null;
    max_cost_units: number | null;
    max_concurrent: number | null;
  };
  retention: typeof PROVIDER_QUOTA_RETENTION;
  secret_material: typeof PROVIDER_QUOTA_SECRET_MATERIAL;
}

export interface ProviderQuotaSnapshotBucket extends JsonObject {
  policy_id: string;
  window_start: number;
  requests: number;
  input_tokens: number;
  output_tokens: number;
  cost_units: number;
}

export interface ProviderQuotaSnapshot extends JsonObject {
  schema: typeof PROVIDER_QUOTA_SNAPSHOT_SCHEMA;
  snapshot_generation: number;
  previous_snapshot_digest: string | null;
  policies: ProviderQuotaPolicy[];
  buckets: ProviderQuotaSnapshotBucket[];
  snapshot_digest: string;
  retention: typeof PROVIDER_QUOTA_RETENTION;
  secret_material: typeof PROVIDER_QUOTA_SECRET_MATERIAL;
}

export interface ProviderQuotaSnapshotTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface ProviderQuotaTransactionalSnapshotTextStore extends ProviderQuotaSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface ProviderQuotaPersistence {
  read(): ProviderQuotaSnapshot | null | Promise<ProviderQuotaSnapshot | null>;
  write(snapshot: ProviderQuotaSnapshot): void | Promise<void>;
}

export interface ProviderQuotaTransactionalPersistence extends ProviderQuotaPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: ProviderQuotaSnapshot): boolean | Promise<boolean>;
}

interface Metrics {
  requests: number;
  inputTokens: number;
  outputTokens: number;
  costUnits: number;
}

interface QuotaBucket {
  readonly policyId: string;
  readonly windowStart: number;
  used: Metrics;
  reserved: Metrics;
}

interface ReservationEntry {
  readonly policy: ProviderQuotaPolicy;
  readonly bucket: QuotaBucket;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function identifier(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || bytes(value) > maximum || [...value].some((character) => character.charCodeAt(0) < 32)) {
    throw new ArgumentError(`${name} must be a bounded non-empty identifier`);
  }
  return value.trim();
}

function optionalIdentifier(name: string, value: unknown, maximum: number): string | null {
  if (value === null || value === undefined) return null;
  return identifier(name, value, maximum);
}

function integer(name: string, value: unknown, maximum: number, minimum = 0): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    throw new ArgumentError(`${name} must be an integer within [${minimum}, ${maximum}]`);
  }
  return value as number;
}

function numberValue(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) {
    throw new ArgumentError(`${name} must be finite within [0, ${maximum}]`);
  }
  return value;
}

function optionalInteger(name: string, value: unknown, maximum: number): number | null {
  if (value === null || value === undefined) return null;
  return integer(name, value, maximum);
}

function optionalNumber(name: string, value: unknown, maximum: number): number | null {
  if (value === null || value === undefined) return null;
  return numberValue(name, value, maximum);
}

function policyId(provider: string, model: string | null): string {
  return model === null ? provider : `${provider}/${model}`;
}

function normalizePolicy(value: ProviderQuotaPolicyInput | ProviderQuotaPolicy): ProviderQuotaPolicy {
  if (!isObject(value)) throw new ArgumentError("provider quota policy must be an object");
  const provider = identifier("provider quota provider", value.provider, 128);
  const model = optionalIdentifier("provider quota model", value.model, 512);
  const windowMs = integer("provider quota windowMs", "windowMs" in value ? value.windowMs : value.window_ms, MAX_PROVIDER_QUOTA_WINDOW_MS, 1);
  const maxRequests = optionalInteger("provider quota maxRequests", "maxRequests" in value ? value.maxRequests : value.max_requests, MAX_PROVIDER_QUOTA_METRIC);
  const maxInputTokens = optionalInteger("provider quota maxInputTokens", "maxInputTokens" in value ? value.maxInputTokens : value.max_input_tokens, MAX_PROVIDER_QUOTA_METRIC);
  const maxOutputTokens = optionalInteger("provider quota maxOutputTokens", "maxOutputTokens" in value ? value.maxOutputTokens : value.max_output_tokens, MAX_PROVIDER_QUOTA_METRIC);
  const maxTotalTokens = optionalInteger("provider quota maxTotalTokens", "maxTotalTokens" in value ? value.maxTotalTokens : value.max_total_tokens, MAX_PROVIDER_QUOTA_METRIC);
  const maxCostUnits = optionalNumber("provider quota maxCostUnits", "maxCostUnits" in value ? value.maxCostUnits : value.max_cost_units, MAX_PROVIDER_QUOTA_COST_UNITS);
  const maxConcurrent = optionalInteger("provider quota maxConcurrent", "maxConcurrent" in value ? value.maxConcurrent : value.max_concurrent, MAX_PROVIDER_QUOTA_METRIC);
  if ([maxRequests, maxInputTokens, maxOutputTokens, maxTotalTokens, maxCostUnits, maxConcurrent].every((item) => item === null)) throw new ArgumentError("provider quota policy must define at least one limit");
  return {
    policy_id: policyId(provider, model),
    provider,
    model,
    window_ms: windowMs,
    max_requests: maxRequests,
    max_input_tokens: maxInputTokens,
    max_output_tokens: maxOutputTokens,
    max_total_tokens: maxTotalTokens,
    max_cost_units: maxCostUnits,
    max_concurrent: maxConcurrent,
  };
}

function emptyMetrics(): Metrics {
  return { requests: 0, inputTokens: 0, outputTokens: 0, costUnits: 0 };
}

function addMetrics(left: Metrics, right: Metrics): Metrics {
  return {
    requests: left.requests + right.requests,
    inputTokens: left.inputTokens + right.inputTokens,
    outputTokens: left.outputTokens + right.outputTokens,
    costUnits: left.costUnits + right.costUnits,
  };
}

function subtractMetrics(left: Metrics, right: Metrics): Metrics {
  return {
    requests: Math.max(0, left.requests - right.requests),
    inputTokens: Math.max(0, left.inputTokens - right.inputTokens),
    outputTokens: Math.max(0, left.outputTokens - right.outputTokens),
    costUnits: Math.max(0, left.costUnits - right.costUnits),
  };
}

function estimatedMetrics(input: ProviderQuotaReservationInput): Metrics {
  const provider = identifier("provider quota request provider", input.provider, 128);
  const model = identifier("provider quota request model", input.model, 512);
  void provider;
  void model;
  const inputTokens = integer("provider quota request inputTokens", input.inputTokens, MAX_PROVIDER_QUOTA_METRIC);
  const outputTokens = integer("provider quota request outputTokens", input.outputTokens, MAX_PROVIDER_QUOTA_METRIC);
  const costUnits = numberValue("provider quota request costUnits", input.costUnits ?? 0, MAX_PROVIDER_QUOTA_COST_UNITS);
  if (inputTokens + outputTokens > MAX_PROVIDER_QUOTA_METRIC) throw new ArgumentError("provider quota request total tokens exceed the bound");
  return { requests: 1, inputTokens, outputTokens, costUnits };
}

function actualMetrics(estimate: Metrics, input: ProviderQuotaSettlementInput): Metrics {
  const inputTokens = input.inputTokens === undefined ? estimate.inputTokens : integer("provider quota settlement inputTokens", input.inputTokens, MAX_PROVIDER_QUOTA_METRIC);
  const outputTokens = input.outputTokens === undefined ? estimate.outputTokens : integer("provider quota settlement outputTokens", input.outputTokens, MAX_PROVIDER_QUOTA_METRIC);
  const costUnits = input.costUnits === undefined ? estimate.costUnits : numberValue("provider quota settlement costUnits", input.costUnits, MAX_PROVIDER_QUOTA_COST_UNITS);
  if (inputTokens + outputTokens > MAX_PROVIDER_QUOTA_METRIC) throw new ArgumentError("provider quota settlement total tokens exceed the bound");
  return { requests: 1, inputTokens, outputTokens, costUnits };
}

function overLimit(policy: ProviderQuotaPolicy, metrics: Metrics, concurrent: number): string[] {
  const reasons: string[] = [];
  if (policy.max_requests !== null && metrics.requests > policy.max_requests) reasons.push("requests");
  if (policy.max_input_tokens !== null && metrics.inputTokens > policy.max_input_tokens) reasons.push("input_tokens");
  if (policy.max_output_tokens !== null && metrics.outputTokens > policy.max_output_tokens) reasons.push("output_tokens");
  if (policy.max_total_tokens !== null && metrics.inputTokens + metrics.outputTokens > policy.max_total_tokens) reasons.push("total_tokens");
  if (policy.max_cost_units !== null && metrics.costUnits > policy.max_cost_units) reasons.push("cost_units");
  if (policy.max_concurrent !== null && concurrent > policy.max_concurrent) reasons.push("concurrent");
  return reasons;
}

function quotaKeys(value: unknown): readonly string[] {
  if (!isObject(value)) throw new ProviderRuntimeError("provider quota snapshot must be an object", { code: "protocol" });
  const keys = Object.keys(value);
  return keys;
}

function assertQuotaKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const accepted = new Set(allowed);
  if (Object.keys(value).some((key) => !accepted.has(key))) throw new ProviderRuntimeError(`${name} contains unsupported metadata`, { code: "protocol" });
}

function snapshotMetric(name: string, value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > MAX_PROVIDER_QUOTA_METRIC) throw new ProviderRuntimeError(`${name} is outside its bound`, { code: "protocol" });
  return value as number;
}

function snapshotTimestamp(name: string, value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > MAX_PROVIDER_QUOTA_TIMESTAMP) throw new ProviderRuntimeError(`${name} is outside its bound`, { code: "protocol" });
  return value as number;
}

function snapshotCost(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > MAX_PROVIDER_QUOTA_COST_UNITS) throw new ProviderRuntimeError(`${name} is outside its bound`, { code: "protocol" });
  return value;
}

function normalizeSnapshotPolicy(value: unknown): ProviderQuotaPolicy {
  if (!isObject(value)) throw new ProviderRuntimeError("provider quota snapshot policy is malformed", { code: "protocol" });
  assertQuotaKeys("provider quota snapshot policy", value, ["policy_id", "provider", "model", "window_ms", "max_requests", "max_input_tokens", "max_output_tokens", "max_total_tokens", "max_cost_units", "max_concurrent"]);
  const policy = normalizePolicy(value as unknown as ProviderQuotaPolicy);
  if (value.policy_id !== policy.policy_id) throw new ProviderRuntimeError("provider quota policy id is not canonical", { code: "protocol" });
  return policy;
}

function normalizeSnapshotBucket(value: unknown): ProviderQuotaSnapshotBucket {
  if (!isObject(value)) throw new ProviderRuntimeError("provider quota snapshot bucket is malformed", { code: "protocol" });
  assertQuotaKeys("provider quota snapshot bucket", value, ["policy_id", "window_start", "requests", "input_tokens", "output_tokens", "cost_units"]);
  return {
    policy_id: identifier("provider quota bucket policy_id", value.policy_id, 640),
    window_start: snapshotTimestamp("provider quota bucket window_start", value.window_start),
    requests: snapshotMetric("provider quota bucket requests", value.requests),
    input_tokens: snapshotMetric("provider quota bucket input_tokens", value.input_tokens),
    output_tokens: snapshotMetric("provider quota bucket output_tokens", value.output_tokens),
    cost_units: snapshotCost("provider quota bucket cost_units", value.cost_units),
  };
}

/** A bounded, synchronous reservation held across one actual provider attempt. */
export class ProviderQuotaReservation {
  readonly reservation_id: string;
  private active = true;
  private dispatched = false;
  private finalSettlement: ProviderQuotaSettlement | null = null;
  private readonly entries: readonly ReservationEntry[];
  private readonly estimate: Metrics;
  private readonly provider: string;
  private readonly model: string;
  private readonly controller: ProviderQuotaController;

  constructor(controller: ProviderQuotaController, provider: string, model: string, estimate: Metrics, entries: readonly ReservationEntry[], reservationId: string) {
    this.controller = controller;
    this.provider = provider;
    this.model = model;
    this.estimate = estimate;
    this.entries = entries;
    this.reservation_id = reservationId;
  }

  /** Mark the point after approval and immediately before the provider transport is entered. */
  markDispatched(): void {
    if (!this.active) throw new ProviderRuntimeError("provider quota reservation is no longer active", { code: "quota_exceeded" });
    this.dispatched = true;
  }

  /** Release an admission that never reached provider transport. Idempotent by design. */
  release(): void {
    if (!this.active) return;
    this.active = false;
    this.controller.releaseReservation(this);
  }

  /** Settle one dispatched attempt using authoritative usage when the provider supplied it. */
  settle(input: ProviderQuotaSettlementInput = {}): ProviderQuotaSettlement {
    if (this.finalSettlement !== null) return { ...this.finalSettlement };
    if (!this.active) throw new ProviderRuntimeError("provider quota reservation was released", { code: "protocol" });
    if (!this.dispatched) throw new ProviderRuntimeError("provider quota reservation must be marked dispatched before settlement", { code: "protocol" });
    this.active = false;
    const actual = actualMetrics(this.estimate, input);
    this.finalSettlement = this.controller.settleReservation(this, actual);
    return { ...this.finalSettlement };
  }

  get isDispatched(): boolean {
    return this.dispatched;
  }

  get reservationProvider(): string {
    return this.provider;
  }

  get reservationModel(): string {
    return this.model;
  }

  get reservedEstimate(): Metrics {
    return { ...this.estimate };
  }

  get reservationEntries(): readonly ReservationEntry[] {
    return this.entries;
  }
}

/**
 * Process-local provider/model quota admission.
 *
 * A provider attempt reserves worst-case request metadata before dispatch. A pre-dispatch
 * refusal releases that reservation; once transport begins, the request count remains charged
 * and returned usage replaces the estimate. This prevents failover loops and parallel domain
 * fan-out from bypassing a shared provider ceiling. The controller never sees prompts, keys,
 * headers, responses, tool arguments, or effect values.
 */
export class ProviderQuotaController {
  private readonly policiesById = new Map<string, ProviderQuotaPolicy>();
  private readonly bucketsByPolicy = new Map<string, QuotaBucket>();
  private readonly activeByPolicy = new Map<string, number>();
  private readonly clock: () => number;
  private sequence = 0;
  private snapshotGeneration = 0;
  private previousSnapshotDigest: string | null = null;

  constructor(options: { clock?: () => number } = {}) {
    if (options.clock !== undefined && typeof options.clock !== "function") throw new ArgumentError("provider quota clock must be callable");
    this.clock = options.clock ?? (() => Date.now());
  }

  setPolicy(input: ProviderQuotaPolicyInput | ProviderQuotaPolicy): ProviderQuotaPolicy {
    const policy = normalizePolicy(input);
    if (this.policiesById.size >= MAX_PROVIDER_QUOTA_POLICIES && !this.policiesById.has(policy.policy_id)) throw new ArgumentError("provider quota policy capacity is exhausted");
    if ((this.activeByPolicy.get(policy.policy_id) ?? 0) > 0) throw new ArgumentError("cannot replace a provider quota policy with active reservations");
    this.policiesById.set(policy.policy_id, policy);
    this.bucketsByPolicy.delete(policy.policy_id);
    return { ...policy };
  }

  removePolicy(provider: string, model: string | null = null): boolean {
    const normalizedProvider = identifier("provider quota provider", provider, 128);
    const normalizedModel = optionalIdentifier("provider quota model", model, 512);
    const id = policyId(normalizedProvider, normalizedModel);
    if ((this.activeByPolicy.get(id) ?? 0) > 0) throw new ArgumentError("cannot remove a provider quota policy with active reservations");
    this.bucketsByPolicy.delete(id);
    return this.policiesById.delete(id);
  }

  policies(): ProviderQuotaPolicy[] {
    return [...this.policiesById.values()].sort((left, right) => left.policy_id.localeCompare(right.policy_id)).map((policy) => ({ ...policy }));
  }

  reserve(input: ProviderQuotaReservationInput, now = this.readClock()): ProviderQuotaReservation {
    const provider = identifier("provider quota request provider", input.provider, 128);
    const model = identifier("provider quota request model", input.model, 512);
    if (!Number.isFinite(now) || now < 0) throw new ArgumentError("provider quota reservation time is invalid");
    const estimate = estimatedMetrics(input);
    const policies = [this.policiesById.get(policyId(provider, null)), this.policiesById.get(policyId(provider, model))]
      .filter((policy): policy is ProviderQuotaPolicy => policy !== undefined)
      .sort((left, right) => left.policy_id.localeCompare(right.policy_id));
    const entries: ReservationEntry[] = [];
    for (const policy of policies) {
      const bucket = this.currentBucket(policy, now);
      const projected = addMetrics(bucket.used, addMetrics(bucket.reserved, estimate));
      const concurrent = (this.activeByPolicy.get(policy.policy_id) ?? 0) + 1;
      const reasons = overLimit(policy, projected, concurrent);
      if (reasons.length > 0) {
        const retryAfterMs = reasons.every((reason) => reason === "concurrent") ? null : Math.max(0, bucket.windowStart + policy.window_ms - now);
        throw new ProviderQuotaExceededError({ provider, model, policy, dimensions: reasons, retryAfterMs, windowStart: bucket.windowStart, observed: projected, concurrent });
      }
      entries.push({ policy, bucket });
    }
    for (const entry of entries) {
      entry.bucket.reserved = addMetrics(entry.bucket.reserved, estimate);
      this.activeByPolicy.set(entry.policy.policy_id, (this.activeByPolicy.get(entry.policy.policy_id) ?? 0) + 1);
    }
    this.sequence += 1;
    const reservationId = `quota-reservation-${this.sequence.toString(36)}`;
    return new ProviderQuotaReservation(this, provider, model, estimate, entries, reservationId);
  }

  status(provider?: string, model?: string | null, now = this.readClock()): ProviderQuotaStatus[] {
    if (!Number.isFinite(now) || now < 0) throw new ArgumentError("provider quota status time is invalid");
    const normalizedProvider = provider === undefined ? null : identifier("provider quota status provider", provider, 128);
    const normalizedModel = model === undefined ? undefined : optionalIdentifier("provider quota status model", model, 512);
    return this.policies().filter((policy) => normalizedProvider === null || policy.provider === normalizedProvider).filter((policy) => normalizedModel === undefined || policy.model === normalizedModel).map((policy) => {
      const bucket = this.currentBucket(policy, now);
      const active = this.activeByPolicy.get(policy.policy_id) ?? 0;
      return {
        schema: PROVIDER_QUOTA_SCHEMA,
        policy_id: policy.policy_id,
        provider: policy.provider,
        model: policy.model,
        window_start: bucket.windowStart,
        window_ends_at: bucket.windowStart + policy.window_ms,
        requests_used: bucket.used.requests,
        requests_reserved: bucket.reserved.requests,
        input_tokens_used: bucket.used.inputTokens,
        input_tokens_reserved: bucket.reserved.inputTokens,
        output_tokens_used: bucket.used.outputTokens,
        output_tokens_reserved: bucket.reserved.outputTokens,
        total_tokens_used: bucket.used.inputTokens + bucket.used.outputTokens,
        total_tokens_reserved: bucket.reserved.inputTokens + bucket.reserved.outputTokens,
        cost_units_used: bucket.used.costUnits,
        cost_units_reserved: bucket.reserved.costUnits,
        concurrent: active,
        next_window_at: bucket.windowStart + policy.window_ms,
        limits: {
          max_requests: policy.max_requests,
          max_input_tokens: policy.max_input_tokens,
          max_output_tokens: policy.max_output_tokens,
          max_total_tokens: policy.max_total_tokens,
          max_cost_units: policy.max_cost_units,
          max_concurrent: policy.max_concurrent,
        },
        retention: PROVIDER_QUOTA_RETENTION,
        secret_material: PROVIDER_QUOTA_SECRET_MATERIAL,
      };
    });
  }

  async snapshot(now = this.readClock()): Promise<ProviderQuotaSnapshot> {
    if (!Number.isFinite(now) || now < 0) throw new ArgumentError("provider quota snapshot time is invalid");
    const buckets = [...this.policiesById.values()].sort((left, right) => left.policy_id.localeCompare(right.policy_id)).flatMap((policy) => {
      const bucket = this.bucketsByPolicy.get(policy.policy_id);
      if (!bucket || (bucket.used.requests === 0 && bucket.used.inputTokens === 0 && bucket.used.outputTokens === 0 && bucket.used.costUnits === 0)) return [];
      const current = this.currentBucket(policy, now);
      if (current !== bucket) return [];
      return [{ policy_id: policy.policy_id, window_start: bucket.windowStart, requests: bucket.used.requests, input_tokens: bucket.used.inputTokens, output_tokens: bucket.used.outputTokens, cost_units: bucket.used.costUnits }];
    });
    const body = {
      schema: PROVIDER_QUOTA_SNAPSHOT_SCHEMA,
      snapshot_generation: this.snapshotGeneration + 1,
      previous_snapshot_digest: this.snapshotGeneration === 0 ? null : this.previousSnapshotDigest,
      policies: this.policies(),
      buckets,
      retention: PROVIDER_QUOTA_RETENTION,
      secret_material: PROVIDER_QUOTA_SECRET_MATERIAL,
    } as const;
    const snapshot = { ...body, snapshot_digest: await digestJson(body) };
    await validateProviderQuotaSnapshot(snapshot);
    this.snapshotGeneration = snapshot.snapshot_generation;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    return structuredClone(snapshot);
  }

  async restore(raw: unknown): Promise<void> {
    if ([...this.activeByPolicy.values()].some((count) => count > 0)) throw new ProviderRuntimeError("cannot restore provider quota with active reservations", { code: "protocol" });
    const snapshot = await validateProviderQuotaSnapshot(raw);
    const policies = new Map(snapshot.policies.map((policy) => [policy.policy_id, policy]));
    const buckets = new Map<string, QuotaBucket>();
    for (const row of snapshot.buckets) buckets.set(row.policy_id, { policyId: row.policy_id, windowStart: row.window_start, used: { requests: row.requests, inputTokens: row.input_tokens, outputTokens: row.output_tokens, costUnits: row.cost_units }, reserved: emptyMetrics() });
    this.policiesById.clear();
    for (const [id, policy] of policies) this.policiesById.set(id, policy);
    this.bucketsByPolicy.clear();
    for (const [id, bucket] of buckets) this.bucketsByPolicy.set(id, bucket);
    this.activeByPolicy.clear();
    this.snapshotGeneration = snapshot.snapshot_generation;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
  }

  async save(persistence: ProviderQuotaPersistence): Promise<ProviderQuotaSnapshot> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("provider quota persistence adapter is malformed");
    const snapshot = await this.snapshot();
    await persistence.write(snapshot);
    return snapshot;
  }

  async restorePersisted(persistence: ProviderQuotaPersistence): Promise<ProviderQuotaSnapshot | null> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("provider quota persistence adapter is malformed");
    const raw = await persistence.read();
    if (raw === null) return null;
    await this.restore(raw);
      return await validateProviderQuotaSnapshot(raw);
  }

  private readClock(): number {
    const current = this.clock();
    if (!Number.isFinite(current) || current < 0) throw new ArgumentError("provider quota clock returned an invalid time");
    return current;
  }

  private currentBucket(policy: ProviderQuotaPolicy, now: number): QuotaBucket {
    const windowStart = Math.floor(now / policy.window_ms) * policy.window_ms;
    const existing = this.bucketsByPolicy.get(policy.policy_id);
    if (existing && existing.windowStart === windowStart) return existing;
    const next: QuotaBucket = { policyId: policy.policy_id, windowStart, used: emptyMetrics(), reserved: emptyMetrics() };
    if (existing && (this.activeByPolicy.get(policy.policy_id) ?? 0) > 0) {
      // A reservation that crosses a fixed-window boundary retains its old bucket through the
      // reservation object. New admissions use the fresh bucket, while concurrency remains global.
      this.bucketsByPolicy.set(policy.policy_id, next);
      return next;
    }
    this.bucketsByPolicy.set(policy.policy_id, next);
    return next;
  }

  releaseReservation(reservation: ProviderQuotaReservation): void {
    for (const entry of reservation.reservationEntries) {
      entry.bucket.reserved = subtractMetrics(entry.bucket.reserved, reservation.reservedEstimate);
      this.decrementActive(entry.policy.policy_id);
    }
  }

  settleReservation(reservation: ProviderQuotaReservation, actual: Metrics): ProviderQuotaSettlement {
    const overLimitDimensions = new Set<string>();
    for (const entry of reservation.reservationEntries) {
      entry.bucket.reserved = subtractMetrics(entry.bucket.reserved, reservation.reservedEstimate);
      entry.bucket.used = addMetrics(entry.bucket.used, actual);
      const concurrent = Math.max(0, (this.activeByPolicy.get(entry.policy.policy_id) ?? 1) - 1);
      for (const reason of overLimit(entry.policy, entry.bucket.used, concurrent)) overLimitDimensions.add(`${entry.policy.policy_id}:${reason}`);
      this.decrementActive(entry.policy.policy_id);
    }
    return {
      schema: PROVIDER_QUOTA_SCHEMA,
      reservation_id: reservation.reservation_id,
      provider: reservation.reservationProvider,
      model: reservation.reservationModel,
      dispatched: reservation.isDispatched,
      charged_requests: actual.requests,
      charged_input_tokens: actual.inputTokens,
      charged_output_tokens: actual.outputTokens,
      charged_cost_units: actual.costUnits,
      over_limit_dimensions: [...overLimitDimensions].sort(),
      retention: PROVIDER_QUOTA_RETENTION,
      secret_material: PROVIDER_QUOTA_SECRET_MATERIAL,
    };
  }

  private decrementActive(policy: string): void {
    const current = this.activeByPolicy.get(policy) ?? 0;
    if (current <= 1) this.activeByPolicy.delete(policy); else this.activeByPolicy.set(policy, current - 1);
  }
}

/** Stable retry-safe error returned before provider transport when a quota would be exceeded. */
export class ProviderQuotaExceededError extends ProviderRuntimeError {
  override readonly name = "ProviderQuotaExceededError";
  readonly policy_id: string;
  readonly dimensions: string[];
  readonly retry_after_ms: number | null;
  readonly observed: ProviderQuotaSnapshotBucket;
  readonly concurrent: number;

  constructor(input: { provider: string; model: string; policy: ProviderQuotaPolicy; dimensions: string[]; retryAfterMs: number | null; windowStart: number; observed: Metrics; concurrent: number }) {
    super(`provider quota exceeded for ${input.provider}/${input.model}: ${input.dimensions.join(",")}`, { retryable: true, statusCode: 429, code: "quota_exceeded", provider: input.provider, operation: "quota_admission", retryAfterMs: input.retryAfterMs ?? undefined });
    this.policy_id = input.policy.policy_id;
    this.dimensions = [...input.dimensions].sort();
    this.retry_after_ms = input.retryAfterMs;
    this.observed = { policy_id: input.policy.policy_id, window_start: input.windowStart, requests: input.observed.requests, input_tokens: input.observed.inputTokens, output_tokens: input.observed.outputTokens, cost_units: input.observed.costUnits };
    this.concurrent = input.concurrent;
  }
}

export async function validateProviderQuotaSnapshot(value: unknown): Promise<ProviderQuotaSnapshot> {
  if (!isObject(value)) throw new ProviderRuntimeError("provider quota snapshot must be an object", { code: "protocol" });
  assertQuotaKeys("provider quota snapshot", value, ["schema", "snapshot_generation", "previous_snapshot_digest", "policies", "buckets", "snapshot_digest", "retention", "secret_material"]);
  if (value.schema !== PROVIDER_QUOTA_SNAPSHOT_SCHEMA || value.retention !== PROVIDER_QUOTA_RETENTION || value.secret_material !== PROVIDER_QUOTA_SECRET_MATERIAL) throw new ProviderRuntimeError("provider quota snapshot markers are invalid", { code: "protocol" });
  if (!Number.isSafeInteger(value.snapshot_generation) || (value.snapshot_generation as number) < 1) throw new ProviderRuntimeError("provider quota snapshot generation is invalid", { code: "protocol" });
  if (value.previous_snapshot_digest !== null && (typeof value.previous_snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.previous_snapshot_digest))) throw new ProviderRuntimeError("provider quota previous snapshot digest is invalid", { code: "protocol" });
  if (((value.snapshot_generation as number) === 1) !== (value.previous_snapshot_digest === null)) throw new ProviderRuntimeError("provider quota snapshot chain is inconsistent", { code: "protocol" });
  if (!Array.isArray(value.policies) || value.policies.length > MAX_PROVIDER_QUOTA_POLICIES) throw new ProviderRuntimeError("provider quota policy capacity is exceeded", { code: "protocol" });
  if (!Array.isArray(value.buckets) || value.buckets.length > MAX_PROVIDER_QUOTA_BUCKETS) throw new ProviderRuntimeError("provider quota bucket capacity is exceeded", { code: "protocol" });
  const policies = value.policies.map(normalizeSnapshotPolicy);
  const policyIds = new Set<string>();
  for (const policy of policies) {
    if (policyIds.has(policy.policy_id)) throw new ProviderRuntimeError("provider quota snapshot contains duplicate policy", { code: "protocol" });
    policyIds.add(policy.policy_id);
  }
  const buckets = value.buckets.map(normalizeSnapshotBucket);
  const bucketIds = new Set<string>();
  for (const bucket of buckets) {
    if (!policyIds.has(bucket.policy_id)) throw new ProviderRuntimeError("provider quota bucket references an unknown policy", { code: "protocol" });
    if (bucketIds.has(bucket.policy_id)) throw new ProviderRuntimeError("provider quota snapshot contains duplicate bucket", { code: "protocol" });
    bucketIds.add(bucket.policy_id);
    const policy = policies.find((candidate) => candidate.policy_id === bucket.policy_id)!;
    if (bucket.window_start % policy.window_ms !== 0) throw new ProviderRuntimeError("provider quota bucket window is not canonical", { code: "protocol" });
    if (policy.max_requests !== null && bucket.requests > policy.max_requests) throw new ProviderRuntimeError("provider quota bucket exceeds request limit", { code: "protocol" });
    if (policy.max_input_tokens !== null && bucket.input_tokens > policy.max_input_tokens) throw new ProviderRuntimeError("provider quota bucket exceeds input limit", { code: "protocol" });
    if (policy.max_output_tokens !== null && bucket.output_tokens > policy.max_output_tokens) throw new ProviderRuntimeError("provider quota bucket exceeds output limit", { code: "protocol" });
    if (policy.max_total_tokens !== null && bucket.input_tokens + bucket.output_tokens > policy.max_total_tokens) throw new ProviderRuntimeError("provider quota bucket exceeds total token limit", { code: "protocol" });
    if (policy.max_cost_units !== null && bucket.cost_units > policy.max_cost_units) throw new ProviderRuntimeError("provider quota bucket exceeds cost limit", { code: "protocol" });
  }
  const snapshotDigest = value.snapshot_digest;
  if (typeof snapshotDigest !== "string" || !/^[0-9a-f]{64}$/.test(snapshotDigest)) throw new ProviderRuntimeError("provider quota snapshot digest is invalid", { code: "protocol" });
  const descriptor = { schema: PROVIDER_QUOTA_SNAPSHOT_SCHEMA, snapshot_generation: value.snapshot_generation as number, previous_snapshot_digest: value.previous_snapshot_digest as string | null, policies, buckets, retention: PROVIDER_QUOTA_RETENTION, secret_material: PROVIDER_QUOTA_SECRET_MATERIAL } as const;
  if (await digestJson(descriptor) !== snapshotDigest) throw new ProviderRuntimeError("provider quota snapshot digest mismatch", { code: "protocol" });
  const snapshot = { ...descriptor, snapshot_digest: snapshotDigest };
  if (bytes(canonicalJson(snapshot)) > MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES) throw new ProviderRuntimeError("provider quota snapshot exceeds its byte bound", { code: "protocol" });
  return structuredClone(snapshot);
}

/** Canonical JSON persistence for quota state over a caller-owned text store. */
export class JsonProviderQuotaPersistence implements ProviderQuotaPersistence {
  protected readonly textStore: ProviderQuotaSnapshotTextStore;
  readonly maxBytes: number;

  constructor(textStore: ProviderQuotaSnapshotTextStore, maxBytes = MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("provider quota text store is malformed");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES) throw new ArgumentError("provider quota persistence maxBytes is outside its bound");
    this.textStore = textStore;
    this.maxBytes = maxBytes;
  }

  async read(): Promise<ProviderQuotaSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > this.maxBytes) throw new ProviderRuntimeError("provider quota JSON exceeds its byte bound", { code: "protocol" });
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ProviderRuntimeError("provider quota JSON is invalid", { code: "protocol" }); }
    if (canonicalJson(parsed) !== encoded) throw new ProviderRuntimeError("provider quota JSON is not canonical", { code: "protocol" });
    return validateProviderQuotaSnapshot(parsed);
  }

  async write(snapshot: ProviderQuotaSnapshot): Promise<void> {
    const validated = await validateProviderQuotaSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > this.maxBytes) throw new ProviderRuntimeError("provider quota JSON exceeds its byte bound", { code: "protocol" });
    await this.textStore.write(encoded);
  }
}

/** Canonical JSON persistence with optimistic compare-and-swap fencing. */
export class TransactionalJsonProviderQuotaPersistence extends JsonProviderQuotaPersistence implements ProviderQuotaTransactionalPersistence {
  declare protected readonly textStore: ProviderQuotaTransactionalSnapshotTextStore;

  constructor(textStore: ProviderQuotaTransactionalSnapshotTextStore, maxBytes = MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES) {
    super(textStore, maxBytes);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("provider quota text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: ProviderQuotaSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ProviderRuntimeError("provider quota expected snapshot digest is invalid", { code: "protocol" });
    const validated = await validateProviderQuotaSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > this.maxBytes) throw new ProviderRuntimeError("provider quota JSON exceeds its byte bound", { code: "protocol" });
    const result = await this.textStore.writeIfUnchanged(expectedSnapshotDigest, encoded);
    if (typeof result !== "boolean") throw new ProviderRuntimeError("provider quota compare-and-swap returned a non-boolean result", { code: "protocol" });
    return result;
  }
}
