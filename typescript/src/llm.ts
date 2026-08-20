import { ArgumentError, AutonomousCostBudgetError, CredentialError, ProviderRuntimeError, ResponseTooLargeError, isObject } from "./errors.js";
import type { ProviderErrorCode, ProviderFailureClass } from "./errors.js";
import { AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS } from "./autonomous-execution.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import { digestJson } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Public schema for the cross-language, application-owned provider runtime. */
export const LLM_RUNTIME_SCHEMA = "bioprism-typescript-llm-runtime/0.1" as const;
export const PROVIDER_OBSERVATION_SCHEMA = "bioprism-typescript-llm-provider-observation/0.1" as const;
export const CREDENTIAL_ONBOARDING_SCHEMA = "bioprism-typescript-llm-credential-onboarding/0.1" as const;
export const PROVIDER_MODEL_DISCOVERY_SCHEMA = "bioprism-typescript-llm-provider-model-discovery/0.1" as const;

export const MAX_PROVIDER_MESSAGE_BYTES = 2_000_000;
export const MAX_PROVIDER_REQUEST_BYTES = 8_000_000;
export const MAX_PROVIDER_RESPONSE_BYTES = 20_000_000;
export const MAX_PROVIDER_TOOLS = 128;
export const MAX_PROVIDER_TOOL_ARGUMENT_BYTES = 1_000_000;
export const MAX_PROVIDER_STREAM_EVENTS = 100_000;
export const MAX_PROVIDER_STREAM_TEXT_BYTES = 20_000_000;
export const MAX_PROVIDER_TURNS = 32;
export const MAX_PROVIDER_CREDENTIAL_BYTES = 16_384;
export const MAX_PROVIDER_MODELS = 512;
export const MAX_CREDENTIAL_PROVISIONING_SOURCES = 128;
export const MAX_CREDENTIAL_PROVISIONING_PROVIDERS = 128;
export const MAX_CREDENTIAL_SOURCE_LABEL_BYTES = 256;
export const AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS = 1_000_000;
export const CREDENTIAL_PROVISIONING_SCHEMA = "bioprism-llm-credential-provisioning/0.1" as const;
const STRUCTURED_SCHEMA_TYPES = new Set(["object", "array", "string", "number", "integer", "boolean", "null"]);

export type ProviderProtocol = "openai_responses" | "openai_chat_completions" | "anthropic_messages";

export interface ProviderConfig {
  provider: string;
  baseUrl: string;
  protocol: ProviderProtocol;
  path?: string;
  modelsPath?: string;
  requiresCredential?: boolean;
  apiKeyHeader?: string;
  timeoutMs?: number;
  maxAttempts?: number;
  retryBackoffMs?: number;
  circuitBreakerFailureThreshold?: number;
  circuitBreakerResetMs?: number;
  maxResponseBytes?: number;
  structuredOutputMode?: "disabled" | "json_object" | "json_schema";
  allowInsecureHttp?: boolean;
}

interface NormalizedProviderConfig {
  readonly provider: string;
  readonly baseUrl: string;
  readonly protocol: ProviderProtocol;
  readonly path: string;
  readonly modelsPath: string;
  readonly requiresCredential: boolean;
  readonly apiKeyHeader: string;
  readonly timeoutMs: number;
  readonly maxAttempts: number;
  readonly retryBackoffMs: number;
  readonly circuitBreakerFailureThreshold: number;
  readonly circuitBreakerResetMs: number;
  readonly maxResponseBytes: number;
  readonly structuredOutputMode: "disabled" | "json_object" | "json_schema";
}

export interface CredentialStatus extends JsonObject {
  schema: typeof CREDENTIAL_ONBOARDING_SCHEMA;
  provider: string;
  registered: boolean;
  ready: boolean;
  active_handles: number;
  expires_at: number | null;
  next_action: "register_provider" | "collect_user_credential" | "ready";
  credential_posture: "caller_supplied_opaque_handle_not_returned";
  secret_material: "never_returned";
}

export interface ProviderCredentialInstructions extends JsonObject {
  provider: string;
  provider_registered: boolean;
  requires_credential: boolean | null;
  ready: boolean;
  next_action: "register_provider" | "collect_user_credential" | "ready";
  input_methods: string[];
  environment_variable: string | null;
  secret_material: "never_returned";
}

export interface CredentialSessionStatus extends JsonObject {
  session_id: string;
  active: boolean;
  created_at: number;
  expires_at: number | null;
  providers: string[];
  secret_persistence: "in_memory_only";
  secret_material: "never_returned";
}

export type CredentialSourceKind = "environment_variable" | "external_secret_resolver";

export interface CredentialSourceSpec extends JsonObject {
  provider: string;
  source_kind: CredentialSourceKind;
  source_id: string;
  source_label: string;
  environment_variable?: string;
  reference_digest?: string;
  ttl_ms: number | null;
  required: boolean;
  enabled: boolean;
  secret_material: "never_returned";
}

export interface CredentialProvisioningReceipt extends JsonObject {
  schema: typeof CREDENTIAL_PROVISIONING_SCHEMA;
  provider: string;
  status: "provisioned" | "already_present" | "not_required" | "missing_provider" | "missing_source" | "source_failed";
  credential_ready: boolean;
  source_kind: CredentialSourceKind | null;
  source_id: string | null;
  source_attempts: number;
  error_class: string | null;
  secret_persistence: "in_memory_only";
  secret_material: "never_returned";
}

export interface CredentialProvisioningResult extends JsonObject {
  schema: typeof CREDENTIAL_PROVISIONING_SCHEMA;
  session_id: string;
  ready: boolean;
  receipts: CredentialProvisioningReceipt[];
  required_failures: string[];
  credential_posture: "opaque_handles_only; sources_resolved_in_process";
  secret_material: "never_returned";
}

interface CredentialEntry {
  readonly provider: string;
  readonly secret: string;
  readonly expiresAt: number | null;
}

const credentialEntries = new WeakMap<CredentialHandle, CredentialEntry>();
let credentialSequence = 0;

function newOpaqueId(): string {
  const cryptoObject = (globalThis as { crypto?: { randomUUID?: () => string } }).crypto;
  const random = typeof cryptoObject?.randomUUID === "function"
    ? cryptoObject.randomUUID()
    : `${Math.random().toString(36).slice(2)}-${Math.random().toString(36).slice(2)}`;
  credentialSequence += 1;
  return `credential-${credentialSequence.toString(36)}-${random}`;
}

/** A non-serializable handle. The secret is held only in the store's private WeakMap. */
export class CredentialHandle {
  readonly provider: string;
  readonly id: string;
  readonly expiresAt: number | null;

  private constructor(provider: string, expiresAt: number | null) {
    this.provider = provider;
    this.id = newOpaqueId();
    this.expiresAt = expiresAt;
    Object.freeze(this);
  }

  static create(provider: string, entry: CredentialEntry): CredentialHandle {
    const handle = new CredentialHandle(provider, entry.expiresAt);
    credentialEntries.set(handle, entry);
    return handle;
  }

  toJSON(): JsonObject {
    return {
      provider: this.provider,
      credential_id: this.id,
      credential_posture: "opaque_handle_not_secret",
      secret_material: "never_returned",
    };
  }

  toString(): string {
    return "[CredentialHandle redacted]";
  }
}

/** BYOK storage with expiry/revocation and no serializable secret-bearing projection. */
export class CredentialStore {
  private readonly entries = new Map<CredentialHandle, CredentialEntry>();
  private readonly clock: () => number;

  constructor(options: { clock?: () => number } = {}) {
    this.clock = options.clock ?? (() => Date.now());
  }

  register(provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    const normalizedProvider = boundedIdentifier("provider", provider, 128);
    if (typeof value !== "string" || value.length === 0 || bytes(value) > MAX_PROVIDER_CREDENTIAL_BYTES) {
      throw new CredentialError("credential value must be a non-empty bounded string");
    }
    const expiresAt = options.ttlMs === undefined
      ? null
      : this.expiryFromTtl(options.ttlMs);
    const entry: CredentialEntry = { provider: normalizedProvider, secret: value, expiresAt };
    const handle = CredentialHandle.create(normalizedProvider, entry);
    this.entries.set(handle, entry);
    return handle;
  }

  registerEnvironment(
    provider: string,
    variable: string,
    environment?: Record<string, string | undefined>,
    options: { ttlMs?: number } = {},
  ): CredentialHandle {
    boundedIdentifier("environment variable", variable, 256);
    const source = environment ?? readProcessEnvironment();
    const value = source[variable];
    if (typeof value !== "string" || value.length === 0) {
      throw new CredentialError(`environment credential ${variable} is not available`);
    }
    return this.register(provider, value, options);
  }

  async registerResolver(
    provider: string,
    resolver: () => string | Promise<string>,
    options: { ttlMs?: number } = {},
  ): Promise<CredentialHandle> {
    if (typeof resolver !== "function") throw new CredentialError("credential resolver must be callable");
    const value = await resolver();
    return this.register(provider, value, options);
  }

  revoke(handle: CredentialHandle): void {
    this.assertHandle(handle);
    this.entries.delete(handle);
  }

  clear(): void {
    this.entries.clear();
  }

  status(provider: string, registered = true): CredentialStatus {
    const normalizedProvider = boundedIdentifier("provider", provider, 128);
    const active = [...this.entries.values()].filter((entry) => {
      if (entry.provider !== normalizedProvider) return false;
      return entry.expiresAt === null || entry.expiresAt > this.clock();
    });
    const expiresAt = active.length === 0
      ? null
      : active.reduce<number | null>((minimum, entry) => {
        if (entry.expiresAt === null) return minimum;
        return minimum === null ? entry.expiresAt : Math.min(minimum, entry.expiresAt);
      }, null);
    return {
      schema: CREDENTIAL_ONBOARDING_SCHEMA,
      provider: normalizedProvider,
      registered,
      ready: active.length > 0,
      active_handles: active.length,
      expires_at: expiresAt,
      next_action: !registered ? "register_provider" : active.length > 0 ? "ready" : "collect_user_credential",
      credential_posture: "caller_supplied_opaque_handle_not_returned",
      secret_material: "never_returned",
    };
  }

  statuses(providers: readonly string[], registeredProviders = new Set(providers)): CredentialStatus[] {
    return [...new Set(providers)].sort().map((provider) => this.status(provider, registeredProviders.has(provider)));
  }

  knownProviders(): string[] {
    return [...new Set([...this.entries.values()].map((entry) => entry.provider))].sort();
  }

  resolve(handle: CredentialHandle, provider: string): string {
    if (!(handle instanceof CredentialHandle)) throw new CredentialError("credential must be an opaque CredentialHandle");
    const entry = this.entries.get(handle);
    if (!entry || credentialEntries.get(handle) !== entry) throw new CredentialError("credential handle is revoked or unknown");
    if (entry.provider !== provider || handle.provider !== provider) throw new CredentialError("credential provider does not match invocation provider");
    if (entry.expiresAt !== null && entry.expiresAt <= this.clock()) {
      this.entries.delete(handle);
      throw new CredentialError("credential handle has expired");
    }
    return entry.secret;
  }

  private assertHandle(handle: CredentialHandle): void {
    if (!(handle instanceof CredentialHandle) || !this.entries.has(handle)) throw new CredentialError("credential handle is revoked or unknown");
  }

  private expiryFromTtl(ttlMs: number): number {
    if (!Number.isFinite(ttlMs) || !Number.isInteger(ttlMs) || ttlMs < 1 || ttlMs > 7 * 24 * 60 * 60 * 1000) {
      throw new CredentialError("credential ttlMs must be an integer between 1ms and 7 days");
    }
    return this.clock() + ttlMs;
  }
}

export interface ProviderMessage {
  role: "system" | "developer" | "user" | "assistant" | "tool";
  content: string;
  name?: string;
  toolCallId?: string;
  toolCalls?: readonly ProviderToolCall[];
  isError?: boolean;
}

export interface ProviderTool {
  name: string;
  description: string;
  parameters: JsonObject;
}

export interface ProviderToolCall {
  id: string;
  name: string;
  arguments: JsonObject;
}

export interface ProviderToolResult {
  callId: string;
  content: string | JsonValue;
  approved: boolean;
  isError?: boolean;
}

export interface ProviderRequest {
  model: string;
  messages: readonly ProviderMessage[];
  maxOutputTokens: number;
  temperature?: number;
  requireJson?: boolean;
  responseSchema?: JsonObject;
  idempotencyKey?: string;
  tools?: readonly ProviderTool[];
  toolChoice?: "auto" | "none" | "required" | string;
}

export interface ProviderUsage extends JsonObject {
  input_tokens?: number;
  output_tokens?: number;
  total_tokens?: number;
}

export interface ProviderResponse {
  provider: string;
  model: string;
  text: string;
  statusCode: number;
  requestId: string | null;
  usage: ProviderUsage;
  structured: JsonValue | null;
  toolCalls: ProviderToolCall[];
  stopReason: string | null;
}

/** Bounded provider model metadata; raw catalog rows and credential material are never returned. */
export interface ProviderModelRecord extends JsonObject {
  schema: typeof PROVIDER_MODEL_DISCOVERY_SCHEMA;
  provider: string;
  model: string;
  active: boolean | null;
  created_at: number | null;
  owned_by: string | null;
  context_window_tokens: number | null;
  max_output_tokens: number | null;
  capabilities: string[];
  metadata_only: true;
}

export interface ProviderModelDiscovery extends JsonObject {
  schema: typeof PROVIDER_MODEL_DISCOVERY_SCHEMA;
  provider: string;
  status_code: number;
  request_id: string | null;
  models_path: string;
  models: ProviderModelRecord[];
  model_count: number;
  retention: "metadata_only;credential_and_raw_provider_response_not_retained";
  secret_material: "never_returned";
}

export interface ProviderStreamEvent {
  provider: string;
  model: string;
  sequence: number;
  eventType: string;
  textDelta: string;
  toolCall?: ProviderToolCall;
  requestId: string | null;
  usage: ProviderUsage;
  done: boolean;
}

export interface ProviderToolLoopResult {
  status: "completed" | "authorization_required" | "turn_limit_reached";
  responses: ProviderResponse[];
  finalResponse: ProviderResponse | null;
  turns: number;
  toolCalls: number;
}

export interface ProviderInvocationMetadata {
  provider: string;
  model: string;
  kind: string;
  inputTokens: number;
  requestedOutputTokens: number;
  toolCount: number;
}

export interface ProviderInvocationOutcome {
  success: boolean;
  status: "completed" | "provider_refused";
  latencyMs: number;
  inputTokens: number;
  outputTokens: number;
  statusCode?: number;
  failureClass?: ProviderFailureClass;
  failureCode?: ProviderErrorCode;
  requestId?: string | null;
  retryable?: boolean;
}

export interface ProviderInvocationObserver {
  before?(metadata: ProviderInvocationMetadata): void | Promise<void>;
  after?(metadata: ProviderInvocationMetadata, outcome: ProviderInvocationOutcome): void | Promise<void>;
}

/** A synchronous reservation released only when a provider call fails before dispatch. */
export type AutonomousCostReservation = () => void;

/** Internal/provider-boundary hook used to compose one budget across nested autonomous calls. */
export type AutonomousCostReservationCallback = (costUnits: number) => AutonomousCostReservation | void;

/** Estimate the cost of one provider request from caller-owned candidate metadata. */
export type AutonomousProviderCostEstimator = (request: ProviderRequest) => number;

export interface ProviderInvocationOptions {
  credential?: CredentialHandle;
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  invocationKind?: string;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  executionTurn?: number;
  executionFailover?: boolean;
  selectionDigest?: string | null;
  estimatedCostUnits?: number;
  reserveCost?: AutonomousCostReservationCallback;
}

/**
 * Process-local aggregate cost accounting for composed autonomous work.
 *
 * Reservations are synchronous so parallel fan-out cannot interleave a check and an increment.
 * A reservation is retained once provider dispatch begins; callers only receive a release handle
 * for failures in the local admission path before the external request is sent.
 */
export class AutonomousCostBudget {
  readonly maxCostUnits: number;
  private consumedCostUnitsValue = 0;

  constructor(maxCostUnits: number) {
    if (!Number.isFinite(maxCostUnits) || maxCostUnits < 0 || maxCostUnits > AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS) {
      throw new ArgumentError(`maxTotalCostUnits must be finite and within [0, ${AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS}]`);
    }
    this.maxCostUnits = maxCostUnits;
  }

  get consumedCostUnits(): number {
    return this.consumedCostUnitsValue;
  }

  get remainingCostUnits(): number {
    return Math.max(0, this.maxCostUnits - this.consumedCostUnitsValue);
  }

  snapshot(): { max_cost_units: number; consumed_cost_units: number; remaining_cost_units: number } {
    return {
      max_cost_units: this.maxCostUnits,
      consumed_cost_units: this.consumedCostUnits,
      remaining_cost_units: this.remainingCostUnits,
    };
  }

  reserve(costUnits: number): AutonomousCostReservation {
    if (!Number.isFinite(costUnits) || costUnits < 0 || costUnits > AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS) {
      throw new ArgumentError(`provider estimated cost must be finite and within [0, ${AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS}]`);
    }
    if (this.consumedCostUnitsValue + costUnits > this.maxCostUnits) {
      throw new AutonomousCostBudgetError("autonomous aggregate cost budget exceeded before provider dispatch", {
        maxCostUnits: this.maxCostUnits,
        consumedCostUnits: this.consumedCostUnitsValue,
        requestedCostUnits: costUnits,
      });
    }
    this.consumedCostUnitsValue += costUnits;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.consumedCostUnitsValue = Math.max(0, this.consumedCostUnitsValue - costUnits);
    };
  }
}

async function recordExecutionProviderOutcome(
  execution: AutonomousExecutionController | undefined,
  metadata: ProviderInvocationMetadata,
  outcome: ProviderInvocationOutcome,
  options: { attempt?: number; turn?: number; selectionDigest?: string | null; estimatedCostUnits?: number } = {},
): Promise<void> {
  if (!execution) return;
  await execution.recordProviderOutcome({
    provider: metadata.provider,
    model: metadata.model,
    invocationKind: metadata.kind,
    attempt: options.attempt ?? 1,
    turn: options.turn ?? 1,
    status: outcome.status,
    outcome: outcome.success ? "success" : "failure",
    latencyMs: outcome.latencyMs,
    inputTokens: outcome.inputTokens,
    outputTokens: outcome.outputTokens,
    estimatedCostUnits: options.estimatedCostUnits ?? 0,
    actualCostUnits: options.estimatedCostUnits ?? 0,
    selectionDigest: options.selectionDigest ?? null,
    outcomeDigest: await digestJson({ provider: metadata.provider, model: metadata.model, kind: metadata.kind, outcome }),
    requestIdDigest: outcome.requestId ? await digestJson(outcome.requestId) : null,
    failureClass: outcome.failureClass ?? null,
    statusCode: outcome.statusCode ?? null,
    retryable: outcome.retryable ?? false,
  });
}

/** Candidate contract accepted from the value-only Rust/Python model-selection plane. */
export interface AutonomousModelCandidate extends JsonObject {
  provider: string;
  model: string;
  capabilities?: string[];
  context_window_tokens: number;
  max_output_tokens: number;
  quality: number;
  latency_ms: number;
  cost_per_million_tokens: number;
  reliability: number;
  requires_credential?: boolean;
  enabled?: boolean;
}

/** Explicit caller-supplied priors needed to turn provider metadata into selection candidates. */
export interface AutonomousModelCandidateDefaults extends JsonObject {
  context_window_tokens: number;
  max_output_tokens: number;
  quality: number;
  latency_ms: number;
  cost_per_million_tokens: number;
  reliability: number;
  capabilities?: string[];
}

export interface AutonomousSelectionRequest extends JsonObject {
  task: string;
  domain: string;
  capability: string;
  risk_class: string;
  task_family?: string | null;
  /** Digest of the bounded domain/capability/risk/workflow learning context. */
  context_digest?: string | null;
  required_capabilities: string[];
  estimated_input_tokens: number;
  requested_output_tokens: number;
  /** Hard caller-owned budget gate applied before utility ranking. */
  max_cost_per_million_tokens?: number | null;
  /** Hard caller-owned latency gate applied before utility ranking. */
  max_latency_ms?: number | null;
  /** Hard caller-owned quality floor applied before utility ranking. */
  min_quality?: number | null;
  /** Whether the provider response must be valid JSON at the transport boundary. */
  require_json?: boolean;
  candidates: AutonomousModelCandidate[];
  provider_health: Record<string, ProviderHealth>;
  model_health: Record<string, ProviderHealth>;
}

export interface AutonomousModelRanking extends JsonObject {
  provider: string;
  model: string;
  score: number;
  eligible: boolean;
  reasons: string[];
}

export interface AutonomousSelectionDecision extends JsonObject {
  selected_model: { provider: string; model: string } | null;
  strategy: "deterministic_health_utility" | "caller_selector";
  ranking: AutonomousModelRanking[];
  abstention_reason: string | null;
  exploration_draw?: number | null;
  exploration_taken?: boolean;
}

export interface AutonomousExecutionPlan {
  task: string;
  domain?: string;
  capability?: string;
  riskClass?: string;
  taskFamily?: string;
  learningContextDigest?: string;
  requiredCapabilities?: readonly string[];
  maxCostPerMillionTokens?: number;
  maxLatencyMs?: number;
  minQuality?: number;
  candidates: readonly AutonomousModelCandidate[];
  request: ProviderRequest;
}

export interface AutonomousExecutionResult {
  selection: AutonomousSelectionDecision;
  response: ProviderResponse;
}

export type AutonomousModelSelector = (request: AutonomousSelectionRequest) => AutonomousSelectionDecision | Promise<AutonomousSelectionDecision>;

export interface ProviderHealth extends JsonObject {
  provider: string;
  circuit: "closed" | "open";
  consecutive_failures: number;
  attempts: number;
  successes: number;
  failures: number;
  success_rate: number;
  mean_latency_ms: number | null;
  last_latency_ms: number | null;
  last_model: string | null;
  last_status_code: number | null;
  credential_posture: "caller_supplied_opaque_handle";
  credential_required: boolean;
  /** Provider transport capability used as a hard gate for explicit structured output. */
  structured_output_mode?: "disabled" | "json_object" | "json_schema";
  /** Optional persisted evaluator-quality projection supplied by a caller-owned health ledger. */
  quality_mean?: number | null;
  quality_observations?: number;
}

interface HealthState {
  attempts: number;
  successes: number;
  failures: number;
  totalLatencyMs: number;
  lastLatencyMs: number | null;
  lastModel: string | null;
  lastStatusCode: number | null;
}

interface CircuitState {
  consecutiveFailures: number;
  openedUntil: number | null;
}

type FetchImplementation = typeof fetch;

const DEFAULT_PATHS: Record<ProviderProtocol, string> = {
  openai_responses: "/v1/responses",
  openai_chat_completions: "/v1/chat/completions",
  anthropic_messages: "/v1/messages",
};

function readProcessEnvironment(): Record<string, string | undefined> {
  const processObject = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process;
  return processObject?.env ?? {};
}

function boundedIdentifier(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.length > maximum || /[\u0000-\u001f]/.test(value)) {
    throw new ProviderRuntimeError(`${name} is outside its bounded identifier contract`);
  }
  return value;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.length === 0 || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ProviderRuntimeError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedPath(name: string, value: string): string {
  const path = boundedText(name, value, 2_048);
  if (!path.startsWith("/") || path.includes("?") || path.includes("#") || /[\s\u0000-\u001f]/.test(path)) {
    throw new ProviderRuntimeError(`${name} is outside its bounded path contract`);
  }
  return path;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function safeJson(value: unknown, label: string, maximum: number): JsonObject {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new ProviderRuntimeError(`${label} is not JSON-serializable`);
  }
  if (encoded === undefined || bytes(encoded) > maximum) throw new ProviderRuntimeError(`${label} exceeds its bounded JSON size`);
  let parsed: unknown;
  try {
    parsed = JSON.parse(encoded);
  } catch {
    throw new ProviderRuntimeError(`${label} is not valid JSON`);
  }
  if (!isObject(parsed)) throw new ProviderRuntimeError(`${label} must be a JSON object`);
  return parsed as JsonObject;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return isObject(value) ? value : null;
}

function asString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function asNonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function jsonText(value: unknown): string {
  if (typeof value === "string") return value;
  const encoded = JSON.stringify(value);
  return encoded === undefined ? "null" : encoded;
}

function parseArguments(value: unknown): JsonObject {
  const decoded = typeof value === "string" ? (() => {
    try { return JSON.parse(value) as unknown; } catch { return null; }
  })() : value;
  if (!isObject(decoded)) throw new ProviderRuntimeError("provider returned malformed tool arguments");
  const normalized = safeJson(decoded, "provider tool arguments", MAX_PROVIDER_TOOL_ARGUMENT_BYTES);
  return normalized;
}

function validateJsonValue(value: unknown, depth = 0): void {
  if (depth > 16) throw new ProviderRuntimeError("provider request is too deeply nested");
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ProviderRuntimeError("provider request contains a non-finite number");
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > 1024) throw new ProviderRuntimeError("provider request contains too many array items");
    for (const child of value) validateJsonValue(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 1024) throw new ProviderRuntimeError("provider request contains too many object keys");
    for (const [key, child] of Object.entries(value)) {
      boundedIdentifier("provider request key", key, 256);
      validateJsonValue(child, depth + 1);
    }
    return;
  }
  throw new ProviderRuntimeError("provider request contains an unsupported value");
}

function validateStructuredResponse(value: unknown, schema: JsonObject, path = "$", depth = 0): void {
  if (depth > 16) throw new ProviderRuntimeError("provider structured response exceeds the schema nesting bound");
  const type = schema.type;
  const types = typeof type === "string" ? [type] : Array.isArray(type) && type.every((item) => typeof item === "string") ? type : undefined;
  if (type !== undefined && !types) throw new ProviderRuntimeError("responseSchema.type must be a string or string array");
  if (types && !types.some((candidate) => (
    (candidate === "object" && isObject(value)) ||
    (candidate === "array" && Array.isArray(value)) ||
    (candidate === "string" && typeof value === "string") ||
    (candidate === "number" && typeof value === "number" && Number.isFinite(value)) ||
    (candidate === "integer" && typeof value === "number" && Number.isSafeInteger(value)) ||
    (candidate === "boolean" && typeof value === "boolean") ||
    (candidate === "null" && value === null)
  ))) throw new ProviderRuntimeError(`structured response violates responseSchema at ${path}`);
  if (Array.isArray(schema.enum) && !schema.enum.some((candidate) => JSON.stringify(candidate) === JSON.stringify(value))) throw new ProviderRuntimeError(`structured response violates responseSchema.enum at ${path}`);
  if (schema.const !== undefined && JSON.stringify(schema.const) !== JSON.stringify(value)) throw new ProviderRuntimeError(`structured response violates responseSchema.const at ${path}`);
  if (typeof value === "string") {
    if (schema.minLength !== undefined && (typeof schema.minLength !== "number" || !Number.isSafeInteger(schema.minLength) || value.length < schema.minLength)) throw new ProviderRuntimeError(`structured response violates responseSchema.minLength at ${path}`);
    if (schema.maxLength !== undefined && (typeof schema.maxLength !== "number" || !Number.isSafeInteger(schema.maxLength) || value.length > schema.maxLength)) throw new ProviderRuntimeError(`structured response violates responseSchema.maxLength at ${path}`);
    if (schema.pattern !== undefined) {
      if (typeof schema.pattern !== "string") throw new ProviderRuntimeError("responseSchema.pattern must be a string");
      try { if (!new RegExp(schema.pattern).test(value)) throw new ProviderRuntimeError(`structured response violates responseSchema.pattern at ${path}`); } catch (error) { if (error instanceof ProviderRuntimeError) throw error; throw new ProviderRuntimeError("responseSchema.pattern is not a valid regular expression"); }
    }
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    if (schema.minimum !== undefined && (typeof schema.minimum !== "number" || value < schema.minimum)) throw new ProviderRuntimeError(`structured response violates responseSchema.minimum at ${path}`);
    if (schema.maximum !== undefined && (typeof schema.maximum !== "number" || value > schema.maximum)) throw new ProviderRuntimeError(`structured response violates responseSchema.maximum at ${path}`);
  }
  if (Array.isArray(value)) {
    if (schema.items !== undefined) {
      if (!isObject(schema.items)) throw new ProviderRuntimeError("responseSchema.items must be an object");
      value.forEach((child, index) => validateStructuredResponse(child, schema.items as JsonObject, `${path}[${index}]`, depth + 1));
    }
    if (schema.minItems !== undefined && (typeof schema.minItems !== "number" || !Number.isSafeInteger(schema.minItems) || value.length < schema.minItems)) throw new ProviderRuntimeError(`structured response violates responseSchema.minItems at ${path}`);
    if (schema.maxItems !== undefined && (typeof schema.maxItems !== "number" || !Number.isSafeInteger(schema.maxItems) || value.length > schema.maxItems)) throw new ProviderRuntimeError(`structured response violates responseSchema.maxItems at ${path}`);
  }
  if (isObject(value)) {
    if (schema.required !== undefined) {
      if (!Array.isArray(schema.required)) throw new ProviderRuntimeError("responseSchema.required must contain strings");
      const requiredKeys: string[] = [];
      for (const key of schema.required) {
        if (typeof key !== "string") throw new ProviderRuntimeError("responseSchema.required must contain strings");
        requiredKeys.push(key);
      }
      for (const key of requiredKeys) if (!(key in value)) throw new ProviderRuntimeError(`structured response is missing responseSchema.required field ${key}`);
    }
    const properties = schema.properties;
    if (properties !== undefined && !isObject(properties)) throw new ProviderRuntimeError("responseSchema.properties must be an object");
    if (isObject(properties)) {
      for (const [key, childSchema] of Object.entries(properties)) {
        if (key in value) {
          if (!isObject(childSchema)) throw new ProviderRuntimeError(`responseSchema property ${key} must be an object`);
          validateStructuredResponse(value[key], childSchema as JsonObject, `${path}.${key}`, depth + 1);
        }
      }
    }
    if (schema.additionalProperties === false) {
      const known = isObject(properties) ? new Set(Object.keys(properties)) : new Set<string>();
      for (const key of Object.keys(value)) if (!known.has(key)) throw new ProviderRuntimeError(`structured response contains additional property ${key} at ${path}`);
    } else if (schema.additionalProperties !== undefined && typeof schema.additionalProperties !== "boolean" && !isObject(schema.additionalProperties)) {
      throw new ProviderRuntimeError("responseSchema.additionalProperties must be false or an object");
    } else if (isObject(schema.additionalProperties)) {
      const known = isObject(properties) ? new Set(Object.keys(properties)) : new Set<string>();
      for (const [key, child] of Object.entries(value)) if (!known.has(key)) validateStructuredResponse(child, schema.additionalProperties as JsonObject, `${path}.${key}`, depth + 1);
    }
  }
}

function validateStructuredSchemaDefinition(schema: JsonObject, path = "$", depth = 0): void {
  if (depth > 16) throw new ProviderRuntimeError(`responseSchema exceeds its nesting bound at ${path}`);
  if (schema.type !== undefined) {
    const types = typeof schema.type === "string" ? [schema.type] : Array.isArray(schema.type) ? schema.type : null;
    if (!types || types.length === 0 || types.some((type) => typeof type !== "string" || !STRUCTURED_SCHEMA_TYPES.has(type))) throw new ProviderRuntimeError(`responseSchema.type is invalid at ${path}`);
  }
  if (schema.enum !== undefined && (!Array.isArray(schema.enum) || schema.enum.length > 1024)) throw new ProviderRuntimeError(`responseSchema.enum is invalid at ${path}`);
  for (const name of ["minLength", "maxLength", "minItems", "maxItems"] as const) {
    const value = schema[name];
    if (value !== undefined && (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0)) throw new ProviderRuntimeError(`responseSchema.${name} is invalid at ${path}`);
  }
  for (const name of ["minimum", "maximum"] as const) {
    const value = schema[name];
    if (value !== undefined && (typeof value !== "number" || !Number.isFinite(value))) throw new ProviderRuntimeError(`responseSchema.${name} is invalid at ${path}`);
  }
  if (schema.pattern !== undefined) {
    if (typeof schema.pattern !== "string") throw new ProviderRuntimeError(`responseSchema.pattern is invalid at ${path}`);
    try { new RegExp(schema.pattern); } catch { throw new ProviderRuntimeError(`responseSchema.pattern is invalid at ${path}`); }
  }
  if (schema.required !== undefined) {
    if (!Array.isArray(schema.required) || schema.required.length > 256 || schema.required.some((key) => typeof key !== "string" || bytes(key) > 256)) throw new ProviderRuntimeError(`responseSchema.required is invalid at ${path}`);
  }
  if (schema.items !== undefined) {
    if (!isObject(schema.items)) throw new ProviderRuntimeError(`responseSchema.items is invalid at ${path}`);
    validateStructuredSchemaDefinition(schema.items as JsonObject, `${path}[]`, depth + 1);
  }
  if (schema.properties !== undefined) {
    if (!isObject(schema.properties) || Object.keys(schema.properties).length > 256) throw new ProviderRuntimeError(`responseSchema.properties is invalid at ${path}`);
    for (const [key, child] of Object.entries(schema.properties)) {
      if (bytes(key) > 256 || key.includes("\u0000") || !isObject(child)) throw new ProviderRuntimeError(`responseSchema property is invalid at ${path}.${key}`);
      validateStructuredSchemaDefinition(child as JsonObject, `${path}.${key}`, depth + 1);
    }
  }
  if (schema.additionalProperties !== undefined && typeof schema.additionalProperties !== "boolean" && !isObject(schema.additionalProperties)) throw new ProviderRuntimeError(`responseSchema.additionalProperties is invalid at ${path}`);
  if (isObject(schema.additionalProperties)) validateStructuredSchemaDefinition(schema.additionalProperties as JsonObject, `${path}.*`, depth + 1);
}

function validateStructuredResponseOrThrow(value: JsonValue, schema: JsonObject | undefined): void {
  try {
    validateJsonValue(value);
    if (schema) validateStructuredResponse(value, schema);
  } catch (error) {
    if (error instanceof ProviderRuntimeError) throw new ProviderRuntimeError(error.message, { code: "invalid_response" });
    throw error;
  }
}

function normalizeConfig(config: ProviderConfig): NormalizedProviderConfig {
  if (!isObject(config)) throw new ProviderRuntimeError("provider config must be an object");
  const provider = boundedIdentifier("provider", config.provider, 128);
  if (!["openai_responses", "openai_chat_completions", "anthropic_messages"].includes(config.protocol)) {
    throw new ProviderRuntimeError("provider protocol is unsupported");
  }
  let url: URL;
  try { url = new URL(config.baseUrl); } catch { throw new ProviderRuntimeError("provider baseUrl is invalid"); }
  const allowInsecure = config.allowInsecureHttp ?? false;
  if (url.protocol !== "https:" && !(allowInsecure && url.protocol === "http:")) {
    throw new ProviderRuntimeError("provider baseUrl must use HTTPS unless insecure HTTP is explicitly enabled");
  }
  if (url.username || url.password || url.search || url.hash) throw new ProviderRuntimeError("provider baseUrl cannot contain credentials, query, or fragment");
  if (!Number.isFinite(url.port ? Number(url.port) : 0)) throw new ProviderRuntimeError("provider baseUrl port is invalid");
  const path = boundedPath("provider path", config.path ?? DEFAULT_PATHS[config.protocol]);
  const modelsPath = boundedPath("provider modelsPath", config.modelsPath ?? "/models");
  const requiresCredential = config.requiresCredential ?? true;
  const timeoutMs = config.timeoutMs ?? 60_000;
  const maxAttempts = config.maxAttempts ?? 1;
  const retryBackoffMs = config.retryBackoffMs ?? 0;
  const failureThreshold = config.circuitBreakerFailureThreshold ?? 3;
  const resetMs = config.circuitBreakerResetMs ?? 30_000;
  const maxResponseBytes = config.maxResponseBytes ?? MAX_PROVIDER_RESPONSE_BYTES;
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 10 * 60_000) throw new ProviderRuntimeError("provider timeoutMs is outside its bounds");
  if (!Number.isInteger(maxAttempts) || maxAttempts < 1 || maxAttempts > 8) throw new ProviderRuntimeError("provider maxAttempts must be within [1, 8]");
  if (!Number.isInteger(retryBackoffMs) || retryBackoffMs < 0 || retryBackoffMs > 60_000) throw new ProviderRuntimeError("provider retryBackoffMs is outside its bounds");
  if (!Number.isInteger(failureThreshold) || failureThreshold < 1 || failureThreshold > 32) throw new ProviderRuntimeError("provider circuit failure threshold is outside its bounds");
  if (!Number.isInteger(resetMs) || resetMs < 1 || resetMs > 24 * 60 * 60_000) throw new ProviderRuntimeError("provider circuit reset is outside its bounds");
  if (!Number.isInteger(maxResponseBytes) || maxResponseBytes < 1 || maxResponseBytes > MAX_PROVIDER_RESPONSE_BYTES) throw new ProviderRuntimeError("provider maxResponseBytes is outside its bounds");
  const structuredOutputMode = config.structuredOutputMode ?? (config.protocol === "openai_responses" ? "json_schema" : config.protocol === "anthropic_messages" ? "disabled" : "json_object");
  const apiKeyHeader = config.apiKeyHeader ?? (config.protocol === "anthropic_messages" ? "x-api-key" : "Authorization");
  boundedIdentifier("provider apiKeyHeader", apiKeyHeader, 256);
  return {
    provider,
    baseUrl: url.toString().replace(/\/$/, ""),
    protocol: config.protocol,
    path,
    modelsPath,
    requiresCredential,
    apiKeyHeader,
    timeoutMs,
    maxAttempts,
    retryBackoffMs,
    circuitBreakerFailureThreshold: failureThreshold,
    circuitBreakerResetMs: resetMs,
    maxResponseBytes,
    structuredOutputMode,
  };
}

function requestMetadata(provider: string, request: ProviderRequest, kind: string): ProviderInvocationMetadata {
  boundedIdentifier("invocation kind", kind, 128);
  const inputTokens = Math.max(1, Math.ceil(request.messages.reduce((sum, message) => sum + bytes(message.content), 0) / 4));
  return {
    provider,
    model: request.model,
    kind,
    inputTokens,
    requestedOutputTokens: request.maxOutputTokens,
    toolCount: request.tools?.length ?? 0,
  };
}

function validateRequest(request: ProviderRequest): void {
  if (!isObject(request)) throw new ProviderRuntimeError("provider request must be an object");
  boundedIdentifier("model", request.model, 512);
  if (!Array.isArray(request.messages) || request.messages.length === 0 || request.messages.length > 1024) throw new ProviderRuntimeError("provider request messages are outside their bounds");
  for (const message of request.messages) {
    const role = isObject(message) && typeof message.role === "string" ? message.role : "";
    if (!isObject(message) || !["system", "developer", "user", "assistant", "tool"].includes(role)) throw new ProviderRuntimeError("provider request contains an invalid message");
    if (message.role === "assistant" && Array.isArray(message.toolCalls) && message.toolCalls.length && message.content === "") {
      if (bytes(message.content) > MAX_PROVIDER_MESSAGE_BYTES) throw new ProviderRuntimeError("provider message content is outside its bounded text contract");
    } else {
      boundedText("provider message content", message.content, MAX_PROVIDER_MESSAGE_BYTES);
    }
    if (message.name !== undefined) boundedIdentifier("provider message name", message.name, 256);
    if (message.toolCallId !== undefined) boundedIdentifier("provider tool call id", message.toolCallId, 256);
    if (message.toolCalls !== undefined) {
        if (!Array.isArray(message.toolCalls) || message.toolCalls.length > MAX_PROVIDER_TOOLS) throw new ProviderRuntimeError("provider message tool calls are outside their bounds");
      for (const call of message.toolCalls) {
        if (!isObject(call)) throw new ProviderRuntimeError("provider message contains a malformed tool call");
        boundedIdentifier("provider tool call name", call.name, 256);
        boundedIdentifier("provider tool call id", call.id, 256);
        validateJsonValue(call.arguments);
      }
    }
  }
  if (!Number.isInteger(request.maxOutputTokens) || request.maxOutputTokens < 1 || request.maxOutputTokens > 1_000_000) throw new ProviderRuntimeError("maxOutputTokens is outside its bounds");
  if (request.temperature !== undefined && (!Number.isFinite(request.temperature) || request.temperature < 0 || request.temperature > 2)) throw new ProviderRuntimeError("temperature must be within [0, 2]");
  if (request.requireJson !== undefined && typeof request.requireJson !== "boolean") throw new ProviderRuntimeError("provider requireJson must be boolean");
  if (request.responseSchema !== undefined) {
    if (!isObject(request.responseSchema)) throw new ProviderRuntimeError("provider responseSchema must be a JSON object");
    if (request.requireJson !== true) throw new ProviderRuntimeError("provider responseSchema requires requireJson: true");
    validateJsonValue(request.responseSchema);
    validateStructuredSchemaDefinition(request.responseSchema);
  }
  const tools = request.tools ?? [];
  if (!Array.isArray(tools) || tools.length > MAX_PROVIDER_TOOLS) throw new ProviderRuntimeError("provider tools are outside their bounds");
  const names = new Set<string>();
  for (const tool of tools) {
    if (!isObject(tool)) throw new ProviderRuntimeError("provider tools contain a malformed definition");
    const normalizedTool = tool as unknown as ProviderTool;
    boundedIdentifier("provider tool name", normalizedTool.name, 256);
    boundedText("provider tool description", normalizedTool.description, 8_000);
    if (names.has(normalizedTool.name)) throw new ProviderRuntimeError("provider tools contain duplicate names");
    names.add(normalizedTool.name);
    validateJsonValue(normalizedTool.parameters);
  }
  safeJson(request, "provider request", MAX_PROVIDER_REQUEST_BYTES);
}

function validateStructuredOutputSupport(config: NormalizedProviderConfig, request: ProviderRequest): void {
  if (request.requireJson !== true) return;
  if (config.structuredOutputMode === "disabled") throw new ProviderRuntimeError(`provider ${config.provider} does not support structured JSON output`, { code: "invalid_request" });
}

function wireMessages(protocol: ProviderProtocol, messages: readonly ProviderMessage[]): JsonValue[] {
  const output: JsonValue[] = [];
  for (const message of messages) {
    if (protocol === "openai_responses") {
      if (message.role === "tool") {
        output.push({ type: "function_call_output", call_id: message.toolCallId ?? "unknown", output: message.content });
      } else if (message.role === "assistant" && message.toolCalls?.length) {
        if (message.content) output.push({ role: "assistant", content: message.content });
        for (const call of message.toolCalls) output.push({ type: "function_call", call_id: call.id, name: call.name, arguments: JSON.stringify(call.arguments) });
      } else {
        output.push({ role: message.role, content: message.content });
      }
    } else if (protocol === "anthropic_messages") {
      if (message.role === "system" || message.role === "developer") continue;
      if (message.role === "tool") {
        output.push({ role: "user", content: [{ type: "tool_result", tool_use_id: message.toolCallId ?? "unknown", content: message.content, is_error: message.isError ?? false }] });
      } else if (message.role === "assistant" && message.toolCalls?.length) {
        const content: JsonValue[] = [];
        if (message.content) content.push({ type: "text", text: message.content });
        for (const call of message.toolCalls) content.push({ type: "tool_use", id: call.id, name: call.name, input: call.arguments });
        output.push({ role: "assistant", content });
      } else {
        output.push({ role: message.role === "assistant" ? "assistant" : "user", content: message.content });
      }
    } else {
      const row: Record<string, JsonValue> = { role: message.role, content: message.content };
      if (message.name) row.name = message.name;
      if (message.toolCallId) row.tool_call_id = message.toolCallId;
      if (message.toolCalls?.length) row.tool_calls = message.toolCalls.map((call) => ({ id: call.id, type: "function", function: { name: call.name, arguments: JSON.stringify(call.arguments) } }));
      output.push(row);
    }
  }
  return output;
}

function requestBody(config: NormalizedProviderConfig, request: ProviderRequest, stream = false): JsonObject {
  const messages = wireMessages(config.protocol, request.messages);
  const tools = request.tools ?? [];
  let body: JsonObject;
  if (config.protocol === "openai_responses") {
    body = { model: request.model, input: messages, max_output_tokens: request.maxOutputTokens };
    if (request.temperature !== undefined) body.temperature = request.temperature;
    if (tools.length) body.tools = tools.map((tool) => ({ type: "function", name: tool.name, description: tool.description, parameters: tool.parameters }));
    if (request.toolChoice) body.tool_choice = request.toolChoice;
    if (request.requireJson && config.structuredOutputMode !== "disabled") {
      body.text = request.responseSchema && config.structuredOutputMode === "json_schema"
        ? { format: { type: "json_schema", name: "response", schema: request.responseSchema, strict: true } }
        : { format: { type: "json_object" } };
    }
  } else if (config.protocol === "anthropic_messages") {
    const system = request.messages.filter((message) => message.role === "system" || message.role === "developer").map((message) => message.content).join("\n\n");
    body = { model: request.model, messages, max_tokens: request.maxOutputTokens };
    if (system) body.system = system;
    if (request.temperature !== undefined) body.temperature = request.temperature;
    if (tools.length) body.tools = tools.map((tool) => ({ name: tool.name, description: tool.description, input_schema: tool.parameters }));
    if (request.toolChoice) {
      body.tool_choice = request.toolChoice === "auto"
        ? { type: "auto" }
        : request.toolChoice === "required"
          ? { type: "any" }
          : request.toolChoice === "none"
            ? { type: "none" }
            : { type: "tool", name: request.toolChoice };
    }
  } else {
    body = { model: request.model, messages, max_tokens: request.maxOutputTokens };
    if (request.temperature !== undefined) body.temperature = request.temperature;
    if (tools.length) body.tools = tools.map((tool) => ({ type: "function", function: { name: tool.name, description: tool.description, parameters: tool.parameters } }));
    if (request.toolChoice) body.tool_choice = request.toolChoice;
    if (request.requireJson && config.structuredOutputMode !== "disabled") {
      body.response_format = request.responseSchema && config.structuredOutputMode === "json_schema"
        ? { type: "json_schema", json_schema: { name: "response", schema: request.responseSchema, strict: true } }
        : { type: "json_object" };
    }
  }
  if (stream) body.stream = true;
  safeJson(body, "provider wire request", MAX_PROVIDER_REQUEST_BYTES);
  return body;
}

function extractText(value: unknown): string {
  if (typeof value === "string") return value;
  if (!Array.isArray(value)) return "";
  return value.map((item) => {
    const row = asRecord(item);
    if (!row) return "";
    return typeof row.text === "string" ? row.text : typeof row.content === "string" ? row.content : "";
  }).join("");
}

function parseResponse(config: NormalizedProviderConfig, payload: JsonObject, statusCode: number, request: ProviderRequest, requestId: string | null): ProviderResponse {
  let model = asString(payload.model) ?? request.model;
  let text = "";
  let stopReason: string | null = null;
  const toolCalls: ProviderToolCall[] = [];
  const usage: ProviderUsage = {};
  if (config.protocol === "openai_responses") {
    text = asString(payload.output_text) ?? "";
    const output = Array.isArray(payload.output) ? payload.output : [];
    for (const item of output) {
      const row = asRecord(item);
      if (!row) continue;
      if (row.type === "message") text += extractText(row.content);
      if (row.type === "function_call") toolCalls.push({ id: asString(row.call_id) ?? asString(row.id) ?? `call-${toolCalls.length}`, name: boundedIdentifier("provider tool name", row.name, 256), arguments: parseArguments(row.arguments) });
    }
    const rawUsage = asRecord(payload.usage);
    if (rawUsage) {
      usage.input_tokens = asNonNegativeInteger(rawUsage.input_tokens) ?? undefined;
      usage.output_tokens = asNonNegativeInteger(rawUsage.output_tokens) ?? undefined;
      usage.total_tokens = asNonNegativeInteger(rawUsage.total_tokens) ?? undefined;
    }
  } else if (config.protocol === "anthropic_messages") {
    const content = Array.isArray(payload.content) ? payload.content : [];
    for (const item of content) {
      const row = asRecord(item);
      if (!row) continue;
      if (row.type === "text") text += asString(row.text) ?? "";
      if (row.type === "tool_use") toolCalls.push({ id: asString(row.id) ?? `call-${toolCalls.length}`, name: boundedIdentifier("provider tool name", row.name, 256), arguments: parseArguments(row.input) });
    }
    stopReason = asString(payload.stop_reason);
    const rawUsage = asRecord(payload.usage);
    if (rawUsage) {
      usage.input_tokens = asNonNegativeInteger(rawUsage.input_tokens) ?? undefined;
      usage.output_tokens = asNonNegativeInteger(rawUsage.output_tokens) ?? undefined;
      usage.total_tokens = (usage.input_tokens ?? 0) + (usage.output_tokens ?? 0);
    }
  } else {
    const choices = Array.isArray(payload.choices) ? payload.choices : [];
    const choice = asRecord(choices[0]);
    const message = choice ? asRecord(choice.message) : null;
    if (message) {
      text = extractText(message.content);
      const calls = Array.isArray(message.tool_calls) ? message.tool_calls : [];
      for (const item of calls) {
        const row = asRecord(item);
        const fn = row ? asRecord(row.function) : null;
        if (row && fn) toolCalls.push({ id: asString(row.id) ?? `call-${toolCalls.length}`, name: boundedIdentifier("provider tool name", fn.name, 256), arguments: parseArguments(fn.arguments) });
      }
    }
    stopReason = choice ? asString(choice.finish_reason) : null;
    const rawUsage = asRecord(payload.usage);
    if (rawUsage) {
      usage.input_tokens = asNonNegativeInteger(rawUsage.prompt_tokens) ?? undefined;
      usage.output_tokens = asNonNegativeInteger(rawUsage.completion_tokens) ?? undefined;
      usage.total_tokens = asNonNegativeInteger(rawUsage.total_tokens) ?? undefined;
    }
  }
  const allowedTools = new Set((request.tools ?? []).map((tool) => tool.name));
  if (toolCalls.some((call) => !allowedTools.has(call.name))) throw new ProviderRuntimeError("provider returned an unrequested tool call");
  let structured: JsonValue | null = null;
  if (!toolCalls.length && request.requireJson) {
    try { structured = JSON.parse(text) as JsonValue; } catch { throw new ProviderRuntimeError("provider returned invalid JSON for the requested structured response", { code: "invalid_response" }); }
    validateStructuredResponseOrThrow(structured, request.responseSchema);
  }
  return { provider: config.provider, model, text, statusCode, requestId, usage, structured, toolCalls, stopReason };
}

function projectModelCatalog(config: NormalizedProviderConfig, payload: JsonObject, statusCode: number, requestId: string | null): ProviderModelDiscovery {
  if (!Array.isArray(payload.data) || payload.data.length > MAX_PROVIDER_MODELS) {
    throw new ProviderRuntimeError("provider model catalog data is outside its bounded contract", { statusCode });
  }
  const seen = new Set<string>();
  const models: ProviderModelRecord[] = [];
  for (const item of payload.data) {
    if (!isObject(item) || typeof item.id !== "string") throw new ProviderRuntimeError("provider model catalog contains a malformed model row", { statusCode });
    const model = boundedIdentifier("provider model id", item.id, 512);
    if (seen.has(model)) throw new ProviderRuntimeError("provider model catalog contains duplicate model ids", { statusCode });
    seen.add(model);
    const active = typeof item.active === "boolean" ? item.active : null;
    const created = item.created ?? item.created_at;
    const createdAt = created === undefined ? null : boundedOptionalInteger("provider model created timestamp", created, 0, Number.MAX_SAFE_INTEGER, statusCode);
    const ownedBy = item.owned_by === undefined || item.owned_by === null
      ? null
      : boundedIdentifier("provider model owner", item.owned_by, 256);
    const contextWindow = firstBoundedInteger(
      "provider model context window",
      [item.context_window, item.context_length, item.context_window_tokens],
      1,
      100_000_000,
      statusCode,
    );
    const maxOutput = firstBoundedInteger(
      "provider model output capacity",
      [item.max_completion_tokens, item.max_output_tokens, item.max_tokens],
      1,
      10_000_000,
      statusCode,
    );
    const capabilities = modelCapabilities(item);
    models.push({
      schema: PROVIDER_MODEL_DISCOVERY_SCHEMA,
      provider: config.provider,
      model,
      active,
      created_at: createdAt,
      owned_by: ownedBy,
      context_window_tokens: contextWindow,
      max_output_tokens: maxOutput,
      capabilities,
      metadata_only: true,
    });
  }
  return {
    schema: PROVIDER_MODEL_DISCOVERY_SCHEMA,
    provider: config.provider,
    status_code: statusCode,
    request_id: requestId,
    models_path: config.modelsPath,
    models,
    model_count: models.length,
    retention: "metadata_only;credential_and_raw_provider_response_not_retained",
    secret_material: "never_returned",
  };
}

function modelCapabilities(row: Record<string, unknown>): string[] {
  const parameters = Array.isArray(row.supported_parameters)
    ? row.supported_parameters.filter((value): value is string => typeof value === "string").map((value) => value.toLowerCase())
    : [];
  const capabilities: string[] = [];
  if (parameters.some((value) => ["tools", "tool_choice", "functions", "function_call"].includes(value))) capabilities.push("tool_use");
  if (parameters.some((value) => ["response_format", "json_object", "json_schema", "structured_outputs"].includes(value))) capabilities.push("structured_output");
  return capabilities.sort();
}

function boundedOptionalInteger(name: string, value: unknown, minimum: number, maximum: number, statusCode: number): number | null {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    throw new ProviderRuntimeError(`${name} is outside its bounded contract`, { statusCode });
  }
  return value as number;
}

function firstBoundedInteger(name: string, values: unknown[], minimum: number, maximum: number, statusCode: number): number | null {
  for (const value of values) {
    if (value === undefined || value === null) continue;
    return boundedOptionalInteger(name, value, minimum, maximum, statusCode);
  }
  return null;
}

/** Convert discovered metadata into selectable candidates while keeping quality and cost caller-owned. */
export function providerModelsToCandidates(
  models: readonly ProviderModelRecord[],
  defaults: AutonomousModelCandidateDefaults,
): AutonomousModelCandidate[] {
  if (!Array.isArray(models) || models.length === 0 || models.length > MAX_PROVIDER_MODELS) throw new ProviderRuntimeError("provider model candidates are outside their bound");
  const contextWindow = boundedCandidateMetric("context_window_tokens", defaults.context_window_tokens, 1, 100_000_000);
  const maxOutput = boundedCandidateMetric("max_output_tokens", defaults.max_output_tokens, 1, 10_000_000);
  const quality = boundedCandidateMetric("quality", defaults.quality, 0, 1);
  const latency = boundedCandidateMetric("latency_ms", defaults.latency_ms, 0, 10 * 60_000);
  const cost = boundedCandidateMetric("cost_per_million_tokens", defaults.cost_per_million_tokens, 0, 1_000_000_000);
  const reliability = boundedCandidateMetric("reliability", defaults.reliability, 0, 1);
  const defaultCapabilities = normalizeCandidateCapabilities(defaults.capabilities ?? []);
  const seen = new Set<string>();
  return models.map((row) => {
    const raw = row as unknown as Record<string, unknown>;
    if (typeof raw.provider !== "string" || typeof raw.model !== "string") throw new ProviderRuntimeError("provider model candidate metadata is malformed");
    const provider = boundedIdentifier("provider model candidate provider", raw.provider, 128);
    const model = boundedIdentifier("provider model candidate model", raw.model, 512);
    const id = `${provider}/${model}`;
    if (seen.has(id)) throw new ProviderRuntimeError("provider model candidates contain duplicate arms");
    seen.add(id);
    const discoveredContext = typeof raw.context_window_tokens === "number" ? raw.context_window_tokens : contextWindow;
    const discoveredOutput = typeof raw.max_output_tokens === "number" ? raw.max_output_tokens : maxOutput;
    if (!Number.isSafeInteger(discoveredContext) || discoveredContext < 1 || discoveredContext > 100_000_000) throw new ProviderRuntimeError("discovered model context window is invalid");
    if (!Number.isSafeInteger(discoveredOutput) || discoveredOutput < 1 || discoveredOutput > 10_000_000) throw new ProviderRuntimeError("discovered model output capacity is invalid");
    const capabilities = normalizeCandidateCapabilities([...(Array.isArray(raw.capabilities) ? raw.capabilities : []), ...defaultCapabilities]);
    return {
      provider,
      model,
      capabilities,
      context_window_tokens: discoveredContext,
      max_output_tokens: discoveredOutput,
      quality,
      latency_ms: latency,
      cost_per_million_tokens: cost,
      reliability,
      enabled: raw.active !== false,
    };
  });
}

function boundedCandidateMetric(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ProviderRuntimeError(`${name} is outside its bounded candidate contract`);
  return value;
}

function normalizeCandidateCapabilities(values: readonly unknown[]): string[] {
  const result = new Set<string>();
  for (const value of values) result.add(boundedIdentifier("provider model capability", value, 128));
  return [...result].sort();
}

async function readBoundedBody(response: Response, maximum: number): Promise<string> {
  if (!response.body) {
    const text = await response.text();
    if (bytes(text) > maximum) throw new ResponseTooLargeError(maximum);
    return text;
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const chunks: string[] = [];
  let total = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      const chunk = decoder.decode(next.value, { stream: true });
      total += bytes(chunk);
      if (total > maximum) {
        await reader.cancel();
        throw new ResponseTooLargeError(maximum);
      }
      chunks.push(chunk);
    }
    const final = decoder.decode();
    total += bytes(final);
    if (total > maximum) throw new ResponseTooLargeError(maximum);
    chunks.push(final);
    return chunks.join("");
  } finally {
    reader.releaseLock();
  }
}

function retryableStatus(status: number): boolean {
  return status === 408 || status === 409 || status === 425 || status === 429 || status >= 500;
}

type ProviderFailure = ProviderRuntimeError | CredentialError | ResponseTooLargeError;

function errorFromUnknown(error: unknown): ProviderFailure {
  if (error instanceof CredentialError) return error;
  if (error instanceof ProviderRuntimeError) return error;
  if (error instanceof ResponseTooLargeError) return error;
  return new ProviderRuntimeError("provider transport failed; credential material was discarded", { retryable: true, code: "transport" });
}

function requestIdFromHeaders(headers: Headers): string | null {
  return headers.get("x-request-id") ?? headers.get("request-id");
}

function retryAfterMsFromHeaders(headers: Headers): number | undefined {
  const value = headers.get("retry-after")?.trim();
  if (!value) return undefined;
  if (/^\d+$/.test(value)) return Math.min(60_000, Number(value) * 1_000);
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return undefined;
  return Math.min(60_000, Math.max(0, timestamp - Date.now()));
}

function isAbortError(error: unknown): boolean {
  return isObject(error) && error.name === "AbortError";
}

function failureClass(error: ProviderFailure): ProviderFailureClass {
  if (error instanceof CredentialError) return "credential_error";
  if (error instanceof ResponseTooLargeError) return "response_too_large";
  if (error.code === "aborted") return "aborted";
  if (error.code === "timeout") return "timeout";
  if (error.code === "circuit_open") return "circuit_open";
  if (error.code === "http_4xx") return "http_4xx";
  if (error.code === "http_5xx") return "http_5xx";
  if (error.code === "protocol" || error.code === "invalid_response") return "protocol_error";
  return "provider_error";
}

function failureCode(error: ProviderFailure): ProviderErrorCode {
  if (error instanceof CredentialError) return "credential";
  if (error instanceof ResponseTooLargeError) return "response_too_large";
  return error.code;
}

function contextProviderFailure(error: ProviderFailure, provider: string, operation: string): ProviderFailure {
  return error instanceof ProviderRuntimeError ? error.withContext({ provider, operation }) : error;
}

function autonomousProviderFailoverLimit(options: { maxProviderFailovers?: number; execution?: AutonomousExecutionController }): number {
  const value = options.maxProviderFailovers ?? options.execution?.toJSON().policy.max_provider_failovers ?? 0;
  if (!Number.isSafeInteger(value) || value < 0 || value > AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS) {
    throw new ProviderRuntimeError(`autonomous maxProviderFailovers must be within [0, ${AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS}]`);
  }
  return value;
}

function abortFailure(callerSignal: AbortSignal | undefined, timedOut: boolean): ProviderRuntimeError {
  if (callerSignal?.aborted) return new ProviderRuntimeError("provider request was aborted by the caller", { code: "aborted" });
  if (timedOut) return new ProviderRuntimeError("provider request timed out", { code: "timeout", retryable: true });
  return new ProviderRuntimeError("provider request was aborted", { code: "aborted" });
}

function waitForRetry(delayMs: number, signal: AbortSignal | undefined): Promise<boolean> {
  if (delayMs <= 0) return Promise.resolve(!signal?.aborted);
  return new Promise((resolve) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const finish = (completed: boolean): void => {
      if (timer !== undefined) clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      resolve(completed);
    };
    const onAbort = (): void => finish(false);
    timer = setTimeout(() => finish(true), delayMs);
    if (signal?.aborted) finish(false);
    else signal?.addEventListener("abort", onAbort, { once: true });
  });
}

function endpointUrl(config: NormalizedProviderConfig): string {
  const url = new URL(config.baseUrl);
  const basePath = url.pathname.replace(/\/+$/, "");
  url.pathname = `${basePath}${config.path}` || "/";
  return url.toString();
}

function modelsEndpointUrl(config: NormalizedProviderConfig): string {
  const url = new URL(config.baseUrl);
  const basePath = url.pathname.replace(/\/+$/, "");
  url.pathname = `${basePath}${config.modelsPath}` || "/";
  return url.toString();
}

function nowMs(): number {
  return typeof performance !== "undefined" && typeof performance.now === "function" ? performance.now() : Date.now();
}

function streamEvent(
  provider: string,
  model: string,
  sequence: number,
  eventType: string,
  requestId: string | null,
  textDelta = "",
  usage: ProviderUsage = {},
  done = false,
  toolCall?: ProviderToolCall,
): ProviderStreamEvent {
  return { provider, model, sequence, eventType, textDelta, requestId, usage, done, ...(toolCall ? { toolCall } : {}) };
}

interface StreamState {
  model: string;
  requestId: string | null;
  sequence: number;
  textBytes: number;
  usage: ProviderUsage;
  calls: Map<number, { id: string; name: string; arguments: string }>;
  anthropicCalls: Map<number, { id: string; name: string; arguments: string }>;
}

function finalizeCalls(calls: Map<number, { id: string; name: string; arguments: string }>): ProviderToolCall[] {
  return [...calls.entries()].sort(([a], [b]) => a - b).map(([, call]) => ({ id: call.id, name: boundedIdentifier("provider tool name", call.name, 256), arguments: parseArguments(call.arguments) }));
}

function projectStreamPayload(
  protocol: ProviderProtocol,
  eventName: string,
  payload: JsonObject,
  state: StreamState,
  request: ProviderRequest,
): Array<{ type: string; text?: string; done?: boolean; usage?: ProviderUsage; calls?: ProviderToolCall[] }> {
  const result: Array<{ type: string; text?: string; done?: boolean; usage?: ProviderUsage; calls?: ProviderToolCall[] }> = [];
  if (protocol === "anthropic_messages") {
    if (eventName === "message_start") {
      const message = asRecord(payload.message);
      if (message) {
        state.model = asString(message.model) ?? state.model;
        const usage = asRecord(message.usage);
        if (usage) state.usage = { input_tokens: asNonNegativeInteger(usage.input_tokens) ?? undefined };
      }
    } else if (eventName === "content_block_start") {
      const index = typeof payload.index === "number" ? payload.index : state.anthropicCalls.size;
      const block = asRecord(payload.content_block);
      if (block?.type === "tool_use") state.anthropicCalls.set(index, { id: asString(block.id) ?? `call-${index}`, name: asString(block.name) ?? "", arguments: "" });
    } else if (eventName === "content_block_delta") {
      const index = typeof payload.index === "number" ? payload.index : 0;
      const delta = asRecord(payload.delta);
      if (delta?.type === "text_delta") result.push({ type: "text", text: asString(delta.text) ?? "" });
      if (delta?.type === "input_json_delta") {
        const call = state.anthropicCalls.get(index);
        if (call) call.arguments += asString(delta.partial_json) ?? "";
      }
    } else if (eventName === "message_delta") {
      const usage = asRecord(payload.usage);
      if (usage) state.usage = { ...state.usage, output_tokens: asNonNegativeInteger(usage.output_tokens) ?? undefined, total_tokens: (state.usage.input_tokens ?? 0) + (asNonNegativeInteger(usage.output_tokens) ?? 0) };
    } else if (eventName === "message_stop") {
      result.push({ type: "done", done: true, usage: state.usage, calls: finalizeCalls(state.anthropicCalls) });
    }
  } else if (protocol === "openai_responses") {
    if (eventName === "response.output_text.delta") result.push({ type: "text", text: asString(payload.delta) ?? "" });
    if (eventName === "response.function_call_arguments.delta") {
      const itemId = asString(payload.item_id) ?? `call-${state.calls.size}`;
      const existing = [...state.calls.values()].find((call) => call.id === itemId);
      if (existing) existing.arguments += asString(payload.delta) ?? "";
    }
    if (eventName === "response.output_item.added") {
      const item = asRecord(payload.item);
      if (item?.type === "function_call") {
        const index = state.calls.size;
        state.calls.set(index, { id: asString(item.call_id) ?? asString(item.id) ?? `call-${index}`, name: asString(item.name) ?? "", arguments: asString(item.arguments) ?? "" });
      }
    }
    if (eventName === "response.completed") {
      const response = asRecord(payload.response);
      if (response) {
        state.model = asString(response.model) ?? state.model;
        state.requestId = asString(response.id) ?? state.requestId;
        const usage = asRecord(response.usage);
        if (usage) state.usage = { input_tokens: asNonNegativeInteger(usage.input_tokens) ?? undefined, output_tokens: asNonNegativeInteger(usage.output_tokens) ?? undefined, total_tokens: asNonNegativeInteger(usage.total_tokens) ?? undefined };
      }
      result.push({ type: "done", done: true, usage: state.usage, calls: finalizeCalls(state.calls) });
    }
  } else {
    const choices = Array.isArray(payload.choices) ? payload.choices : [];
    const choice = asRecord(choices[0]);
    const delta = choice ? asRecord(choice.delta) : null;
    if (delta) {
      const content = extractText(delta.content);
      if (content) result.push({ type: "text", text: content });
      const calls = Array.isArray(delta.tool_calls) ? delta.tool_calls : [];
      for (const item of calls) {
        const row = asRecord(item);
        if (!row) continue;
        const index = typeof row.index === "number" ? row.index : state.calls.size;
        const fn = asRecord(row.function);
        const call = state.calls.get(index) ?? { id: asString(row.id) ?? `call-${index}`, name: "", arguments: "" };
        if (fn) { call.name += asString(fn.name) ?? ""; call.arguments += asString(fn.arguments) ?? ""; }
        state.calls.set(index, call);
      }
    }
    if (choice?.finish_reason) {
      const usage = asRecord(payload.usage);
      if (usage) state.usage = { input_tokens: asNonNegativeInteger(usage.prompt_tokens) ?? undefined, output_tokens: asNonNegativeInteger(usage.completion_tokens) ?? undefined, total_tokens: asNonNegativeInteger(usage.total_tokens) ?? undefined };
      result.push({ type: "done", done: true, usage: state.usage, calls: finalizeCalls(state.calls) });
    }
  }
  const allowed = new Set((request.tools ?? []).map((tool) => tool.name));
  for (const group of result) if (group.calls?.some((call) => !allowed.has(call.name))) throw new ProviderRuntimeError("provider returned an unrequested streamed tool call");
  return result;
}

function splitSseFrames(buffer: string): { frames: string[]; remainder: string } {
  const normalized = buffer.replaceAll("\r\n", "\n").replaceAll("\r", "\n");
  const frames: string[] = [];
  let start = 0;
  while (true) {
    const end = normalized.indexOf("\n\n", start);
    if (end < 0) return { frames, remainder: normalized.slice(start) };
    frames.push(normalized.slice(start, end));
    start = end + 2;
  }
}

function parseSseFrame(frame: string): { event: string; data: string } | null {
  let event = "message";
  const data: string[] = [];
  for (const line of frame.split("\n")) {
    if (line.startsWith(":") || line === "") continue;
    const separator = line.indexOf(":");
    const field = separator < 0 ? line : line.slice(0, separator);
    const valueStart = separator < 0 ? line.length : separator + 1;
    const value = line[valueStart] === " " ? line.slice(valueStart + 1) : line.slice(valueStart);
    if (field === "event") event = value;
    if (field === "data") data.push(value);
  }
  return data.length ? { event, data: data.join("\n") } : null;
}

/** A fetch-based, provider-neutral runtime for Node, browsers, and test harnesses. */
export class LLMRuntime {
  readonly credentials: CredentialStore;
  readonly onboarding: ProviderOnboarding;
  private readonly providers = new Map<string, NormalizedProviderConfig>();
  private readonly circuits = new Map<string, CircuitState>();
  private readonly providerHealthState = new Map<string, HealthState>();
  private readonly modelHealthState = new Map<string, HealthState>();
  private readonly fetchImplementation: FetchImplementation;
  private readonly clock: () => number;

  constructor(options: { credentials?: CredentialStore; fetch?: FetchImplementation; clock?: () => number } = {}) {
    this.credentials = options.credentials ?? new CredentialStore();
    const implementation = options.fetch ?? globalThis.fetch;
    if (typeof implementation !== "function") throw new ProviderRuntimeError("a fetch implementation is required");
    this.fetchImplementation = implementation;
    this.clock = options.clock ?? (() => Date.now());
    this.onboarding = new ProviderOnboarding(this);
  }

  registerProvider(config: ProviderConfig): void {
    const normalized = normalizeConfig(config);
    this.providers.set(normalized.provider, normalized);
    this.circuits.set(normalized.provider, this.circuits.get(normalized.provider) ?? { consecutiveFailures: 0, openedUntil: null });
    this.providerHealthState.set(normalized.provider, this.providerHealthState.get(normalized.provider) ?? emptyHealth());
  }

  providerMetadata(): JsonObject[] {
    return [...this.providers.values()].sort((a, b) => a.provider.localeCompare(b.provider)).map((config) => ({
      provider: config.provider,
      protocol: config.protocol,
      base_url: config.baseUrl,
      path: config.path,
      models_path: config.modelsPath,
      requires_credential: config.requiresCredential,
      structured_output_mode: config.structuredOutputMode,
      credential_posture: "caller_supplied_opaque_handle_not_returned",
      secret_material: "never_returned",
    }));
  }

  /**
   * Discover currently available models through a provider's bounded catalog endpoint.
   *
   * The credential is resolved only while constructing the request. The response is reduced to
   * stable model metadata, and neither the raw catalog nor authorization material enters runtime
   * health, selection, telemetry, or persistence surfaces.
   */
  async discoverModels(
    provider: string,
    options: { credential?: CredentialHandle; signal?: AbortSignal } = {},
  ): Promise<ProviderModelDiscovery> {
    const config = this.requireProvider(provider);
    let response: Response;
    try {
      response = await this.fetchModelCatalog(config, options.credential, options.signal);
    } catch (unknownError) {
      const error = contextProviderFailure(errorFromUnknown(unknownError), provider, "model_discovery");
      throw error;
    }
    try {
      const body = await readBoundedBody(response, config.maxResponseBytes);
      if (response.status >= 400) throw providerHttpError(response.status, response.headers);
      let payload: unknown;
      try { payload = JSON.parse(body); } catch { throw new ProviderRuntimeError("provider model catalog returned non-JSON data", { statusCode: response.status, code: "invalid_response" }); }
      if (!isObject(payload)) throw new ProviderRuntimeError("provider model catalog must be a JSON object", { statusCode: response.status, code: "invalid_response" });
      return projectModelCatalog(config, payload as JsonObject, response.status, requestIdFromHeaders(response.headers));
    } catch (unknownError) {
      throw contextProviderFailure(errorFromUnknown(unknownError), provider, "model_discovery");
    }
  }

  providerStatus(provider: string): ProviderHealth {
    const config = this.requireProvider(provider);
    const health = this.providerHealthState.get(provider) ?? emptyHealth();
    const circuit = this.circuits.get(provider) ?? { consecutiveFailures: 0, openedUntil: null };
    const open = circuit.openedUntil !== null && circuit.openedUntil > this.clock();
    return healthProjection(provider, health, open ? "open" : "closed", circuit.consecutiveFailures, config.requiresCredential);
  }

  modelHealthSnapshot(): Record<string, ProviderHealth> {
    const result: Record<string, ProviderHealth> = {};
    for (const [arm, health] of [...this.modelHealthState.entries()].sort(([a], [b]) => a.localeCompare(b))) {
      const separator = arm.indexOf("/");
      const provider = separator < 0 ? arm : arm.slice(0, separator);
      const model = separator < 0 ? arm : arm.slice(separator + 1);
      const circuit = this.circuits.get(provider) ?? { consecutiveFailures: 0, openedUntil: null };
      const open = circuit.openedUntil !== null && circuit.openedUntil > this.clock();
      result[arm] = { ...healthProjection(provider, health, open ? "open" : "closed", circuit.consecutiveFailures, true), model };
    }
    return result;
  }

  async invoke(
    provider: string,
    request: ProviderRequest,
    options: ProviderInvocationOptions = {},
  ): Promise<ProviderResponse> {
    const config = this.requireProvider(provider);
    validateRequest(request);
    validateStructuredOutputSupport(config, request);
    const metadata = requestMetadata(provider, request, options.invocationKind ?? "provider_call");
    const releaseCost = options.reserveCost?.(options.estimatedCostUnits ?? 0);
    try {
      await options.execution?.admitProviderCall({ provider, model: request.model, invocationKind: metadata.kind, attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits, costUnits: options.estimatedCostUnits, failover: options.executionFailover });
      await options.observer?.before?.(metadata);
    } catch (error) {
      releaseCost?.();
      throw error;
    }
    const started = nowMs();
    let outcomeRecorded = false;
    const recordOutcome = async (outcome: ProviderInvocationOutcome): Promise<void> => {
      if (outcomeRecorded) return;
      outcomeRecorded = true;
      await options.observer?.after?.(metadata, outcome);
      await recordExecutionProviderOutcome(options.execution, metadata, outcome, { attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits });
    };
    try {
      const response = await this.request(config, request, options.credential, options.signal, false);
      const latencyMs = Math.max(0, nowMs() - started);
      this.record(provider, request.model, true, latencyMs, response.statusCode, response);
      await recordOutcome({ success: true, status: "completed", latencyMs, inputTokens: response.usage.input_tokens ?? metadata.inputTokens, outputTokens: response.usage.output_tokens ?? 0, statusCode: response.statusCode });
      return response;
    } catch (unknownError) {
      const error = contextProviderFailure(errorFromUnknown(unknownError), provider, "invoke");
      const latencyMs = Math.max(0, nowMs() - started);
      this.record(provider, request.model, false, latencyMs, error instanceof ProviderRuntimeError ? error.statusCode ?? null : null);
      await recordOutcome({
        success: false,
        status: "provider_refused",
        latencyMs,
        inputTokens: metadata.inputTokens,
        outputTokens: 0,
        ...(error instanceof ProviderRuntimeError && error.statusCode !== undefined ? { statusCode: error.statusCode } : {}),
        failureClass: failureClass(error),
        failureCode: failureCode(error),
        requestId: error instanceof ProviderRuntimeError ? error.requestId ?? null : null,
        retryable: error instanceof ProviderRuntimeError ? error.retryable : false,
      });
      throw error;
    }
  }

  async *invokeStream(
    provider: string,
    request: ProviderRequest,
    options: ProviderInvocationOptions = {},
  ): AsyncIterable<ProviderStreamEvent> {
    const config = this.requireProvider(provider);
    validateRequest(request);
    validateStructuredOutputSupport(config, request);
    const metadata = requestMetadata(provider, request, options.invocationKind ?? "provider_stream");
    const releaseCost = options.reserveCost?.(options.estimatedCostUnits ?? 0);
    try {
      await options.execution?.admitProviderCall({ provider, model: request.model, invocationKind: metadata.kind, attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits, costUnits: options.estimatedCostUnits, failover: options.executionFailover });
      await options.observer?.before?.(metadata);
    } catch (error) {
      releaseCost?.();
      throw error;
    }
    const started = nowMs();
    let outcome: ProviderInvocationOutcome | null = null;
    try {
      const response = await this.fetchWithRetries(config, request, options.credential, options.signal, true);
      if (response.status >= 400) throw providerHttpError(response.status, response.headers);
      const contentType = response.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase() ?? "";
      if (contentType && contentType !== "text/event-stream") throw new ProviderRuntimeError("provider stream did not return text/event-stream", { statusCode: response.status });
      if (!response.body) throw new ProviderRuntimeError("provider stream did not return a readable body");
      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      const state: StreamState = { model: request.model, requestId: requestIdFromHeaders(response.headers), sequence: 0, textBytes: 0, usage: {}, calls: new Map(), anthropicCalls: new Map() };
      try {
        while (true) {
          const next = await reader.read();
          const text = decoder.decode(next.value, { stream: !next.done });
          buffer += text;
          const split = splitSseFrames(buffer);
          buffer = split.remainder;
          for (const frame of split.frames) {
            const parsed = parseSseFrame(frame);
            if (!parsed) continue;
            if (parsed.data === "[DONE]") {
              const calls = finalizeCalls(config.protocol === "anthropic_messages" ? state.anthropicCalls : state.calls);
              state.sequence += 1;
              yield streamEvent(provider, state.model, state.sequence, "stream.done", state.requestId, "", state.usage, true, undefined);
              for (const call of calls) { state.sequence += 1; yield streamEvent(provider, state.model, state.sequence, "tool_call", state.requestId, "", state.usage, false, call); }
              continue;
            }
            let payload: JsonObject;
            try { const decodedPayload: unknown = JSON.parse(parsed.data); if (!isObject(decodedPayload)) throw new Error(); payload = decodedPayload as JsonObject; } catch { throw new ProviderRuntimeError("provider stream contained invalid JSON"); }
            for (const projected of projectStreamPayload(config.protocol, parsed.event, payload, state, request)) {
              if (projected.text) {
                state.textBytes += bytes(projected.text);
                if (state.textBytes > MAX_PROVIDER_STREAM_TEXT_BYTES) throw new ProviderRuntimeError("provider stream text exceeded its bound");
              }
              state.sequence += 1;
              if (state.sequence > MAX_PROVIDER_STREAM_EVENTS) throw new ProviderRuntimeError("provider stream exceeded its event bound");
              if (projected.calls?.length) {
                yield streamEvent(provider, state.model, state.sequence, "tool_call", state.requestId, "", projected.usage ?? state.usage, false, projected.calls[0]);
                for (const call of projected.calls.slice(1)) { state.sequence += 1; yield streamEvent(provider, state.model, state.sequence, "tool_call", state.requestId, "", projected.usage ?? state.usage, false, call); }
              } else {
                yield streamEvent(provider, state.model, state.sequence, projected.type, state.requestId, projected.text ?? "", projected.usage ?? state.usage, projected.done ?? false);
              }
            }
          }
          if (next.done) {
            const final = decoder.decode();
            if (final) buffer += final;
            const parsed = parseSseFrame(buffer);
            if (parsed && parsed.data !== "[DONE]") {
              let payload: JsonObject;
              try { const decodedPayload: unknown = JSON.parse(parsed.data); if (!isObject(decodedPayload)) throw new Error(); payload = decodedPayload as JsonObject; } catch { throw new ProviderRuntimeError("provider stream contained invalid JSON"); }
              for (const projected of projectStreamPayload(config.protocol, parsed.event, payload, state, request)) {
                state.sequence += 1;
                yield streamEvent(provider, state.model, state.sequence, projected.type, state.requestId, projected.text ?? "", projected.usage ?? state.usage, projected.done ?? false, projected.calls?.[0]);
              }
            }
            break;
          }
        }
      } finally { reader.releaseLock(); }
      this.record(provider, request.model, true, Math.max(0, nowMs() - started), 200);
      outcome = { success: true, status: "completed", latencyMs: Math.max(0, nowMs() - started), inputTokens: metadata.inputTokens, outputTokens: 0, statusCode: 200 };
    } catch (unknownError) {
      const error = contextProviderFailure(errorFromUnknown(unknownError), provider, "stream");
      const latencyMs = Math.max(0, nowMs() - started);
      this.record(provider, request.model, false, latencyMs, error instanceof ProviderRuntimeError ? error.statusCode ?? null : null);
      outcome = {
        success: false,
        status: "provider_refused",
        latencyMs,
        inputTokens: metadata.inputTokens,
        outputTokens: 0,
        ...(error instanceof ProviderRuntimeError && error.statusCode !== undefined ? { statusCode: error.statusCode } : {}),
        failureClass: failureClass(error),
        failureCode: failureCode(error),
        requestId: error instanceof ProviderRuntimeError ? error.requestId ?? null : null,
        retryable: error instanceof ProviderRuntimeError ? error.retryable : false,
      };
      throw error;
    } finally {
      if (outcome) {
        await options.observer?.after?.(metadata, outcome);
        await recordExecutionProviderOutcome(options.execution, metadata, outcome, { attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits });
      }
    }
  }

  async collectStream(provider: string, request: ProviderRequest, options: ProviderInvocationOptions = {}): Promise<ProviderResponse> {
    const text: string[] = [];
    const calls: ProviderToolCall[] = [];
    let usage: ProviderUsage = {};
    let model = request.model;
    let requestId: string | null = null;
    let done = false;
    for await (const event of this.invokeStream(provider, request, options)) {
      text.push(event.textDelta);
      if (event.toolCall) calls.push(event.toolCall);
      usage = { ...usage, ...event.usage };
      model = event.model || model;
      requestId = event.requestId ?? requestId;
      done = done || event.done;
    }
    if (!done && text.join("").length === 0 && calls.length === 0) throw new ProviderRuntimeError("provider stream contained no assistant output");
    const outputText = text.join("");
    let structured: JsonValue | null = null;
    if (!calls.length && request.requireJson) {
      try { structured = JSON.parse(outputText) as JsonValue; } catch { throw new ProviderRuntimeError("provider stream returned invalid JSON", { code: "invalid_response" }); }
      validateStructuredResponseOrThrow(structured, request.responseSchema);
    }
    return { provider, model, text: outputText, statusCode: 200, requestId, usage, structured, toolCalls: calls, stopReason: null };
  }

  async invokeToolLoop(
    provider: string,
    request: ProviderRequest,
    options: ProviderInvocationOptions & {
      authorizeAndExecute: (calls: ProviderToolCall[]) => ProviderToolResult[] | Promise<ProviderToolResult[]>;
      maxTurns?: number;
      maxToolCalls?: number;
      stream?: boolean;
      initialResponse?: ProviderResponse;
      costEstimator?: AutonomousProviderCostEstimator;
      toolReadOnly?: (call: ProviderToolCall) => boolean | Promise<boolean>;
    },
  ): Promise<ProviderToolLoopResult> {
    if (typeof options.authorizeAndExecute !== "function") throw new ProviderRuntimeError("authorizeAndExecute must be callable");
    const maxTurns = options.maxTurns ?? 4;
    const maxToolCalls = options.maxToolCalls ?? MAX_PROVIDER_TOOLS;
    if (!Number.isInteger(maxTurns) || maxTurns < 1 || maxTurns > MAX_PROVIDER_TURNS) throw new ProviderRuntimeError("maxTurns is outside its bounds");
    if (!Number.isInteger(maxToolCalls) || maxToolCalls < 1 || maxToolCalls > MAX_PROVIDER_TOOLS * 8) throw new ProviderRuntimeError("maxToolCalls is outside its bounds");
    let current = request;
    let response = options.initialResponse ?? null;
    const responses: ProviderResponse[] = [];
    let toolCalls = 0;
    for (let turn = 0; turn < maxTurns; turn += 1) {
      const providerOptions = {
        ...options,
        executionTurn: turn + 1,
        ...(options.costEstimator ? { estimatedCostUnits: options.costEstimator(current) } : {}),
      };
      response ??= options.stream ? await this.collectStream(provider, current, providerOptions) : await this.invoke(provider, current, providerOptions);
      responses.push(response);
      if (response.toolCalls.length === 0) return { status: "completed", responses, finalResponse: response, turns: responses.length, toolCalls };
      toolCalls += response.toolCalls.length;
      if (toolCalls > maxToolCalls || turn + 1 >= maxTurns) return { status: "turn_limit_reached", responses, finalResponse: response, turns: responses.length, toolCalls };
      for (const call of response.toolCalls) {
        const readOnly = options.toolReadOnly ? await options.toolReadOnly(call) : true;
        await options.execution?.admitToolCall({ tool: call.name, callId: call.id, readOnly, approvalRequired: !readOnly });
      }
      let returned: ProviderToolResult[];
      try {
        returned = await options.authorizeAndExecute(response.toolCalls);
      } catch (unknownError) {
        const reason = unknownError instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(unknownError.name) ? unknownError.name : "tool_executor_error";
        for (const call of response.toolCalls) await options.execution?.recordToolOutcome({ tool: call.name, callId: call.id, status: "failed", reason });
        throw unknownError;
      }
      if (!Array.isArray(returned) || returned.length !== response.toolCalls.length || returned.some((result) => !isObject(result) || typeof result.callId !== "string")) throw new ProviderRuntimeError("authorization callback returned malformed tool results");
      const requestedCallIds = new Set(response.toolCalls.map((call) => call.id));
      if (returned.some((result) => !requestedCallIds.has(result.callId)) || new Set(returned.map((result) => result.callId)).size !== returned.length) throw new ProviderRuntimeError("authorization callback returned unbound or duplicate tool call ids");
      for (const result of returned) {
        const call = response.toolCalls.find((candidate) => candidate.id === result.callId)!;
        await options.execution?.recordToolOutcome({ tool: call.name, callId: result.callId, status: result.approved ? "completed" : "authorization_required", outcomeDigest: await digestJson({ call_id: result.callId, approved: result.approved, is_error: result.isError ?? false, content: result.content }) });
      }
      if (returned.some((result) => !result.approved)) return { status: "authorization_required", responses, finalResponse: response, turns: responses.length, toolCalls };
      const assistant: ProviderMessage = { role: "assistant", content: response.text, toolCalls: response.toolCalls };
      const resultMessages: ProviderMessage[] = returned.map((result) => ({ role: "tool", content: jsonText(result.content), toolCallId: result.callId }));
      current = { ...current, messages: [...current.messages, assistant, ...resultMessages] };
      response = null;
    }
    return { status: "turn_limit_reached", responses, finalResponse: responses.at(-1) ?? null, turns: responses.length, toolCalls };
  }

  private requireProvider(provider: string): NormalizedProviderConfig {
    const config = this.providers.get(provider);
    if (!config) throw new ProviderRuntimeError(`provider ${provider} is not configured`);
    return config;
  }

  private async request(config: NormalizedProviderConfig, request: ProviderRequest, credential: CredentialHandle | undefined, signal: AbortSignal | undefined, stream: boolean): Promise<ProviderResponse> {
    const response = await this.fetchWithRetries(config, request, credential, signal, stream);
    const body = await readBoundedBody(response, config.maxResponseBytes);
    if (response.status >= 400) throw providerHttpError(response.status, response.headers);
    let payload: unknown;
    try { payload = JSON.parse(body); } catch { throw new ProviderRuntimeError("provider returned a non-JSON response", { statusCode: response.status }); }
    if (!isObject(payload)) throw new ProviderRuntimeError("provider response must be a JSON object", { statusCode: response.status });
    const parsed = parseResponse(config, payload as JsonObject, response.status, request, requestIdFromHeaders(response.headers) ?? asString(payload.id));
    return parsed;
  }

  private async fetchWithRetries(config: NormalizedProviderConfig, request: ProviderRequest, credential: CredentialHandle | undefined, signal: AbortSignal | undefined, stream: boolean): Promise<Response> {
    const circuit = this.circuits.get(config.provider) ?? { consecutiveFailures: 0, openedUntil: null };
    if (signal?.aborted) throw new ProviderRuntimeError("provider request was aborted before dispatch", { code: "aborted" });
    if (circuit.openedUntil !== null && circuit.openedUntil > this.clock()) throw new ProviderRuntimeError("provider circuit is open; invocation is temporarily refused", { circuitOpen: true, code: "circuit_open" });
    if (circuit.openedUntil !== null) { circuit.openedUntil = null; circuit.consecutiveFailures = 0; }
    let lastError: ProviderFailure | null = null;
    for (let attempt = 0; attempt < config.maxAttempts; attempt += 1) {
      try {
        const response = await this.fetchOnce(config, request, credential, signal, stream);
        if (response.status >= 400) throw providerHttpError(response.status, response.headers);
        circuit.consecutiveFailures = 0;
        circuit.openedUntil = null;
        return response;
      } catch (unknownError) {
        const normalizedError = errorFromUnknown(unknownError);
        const error = normalizedError instanceof ProviderRuntimeError
          ? normalizedError.withContext({ provider: config.provider, operation: "provider_request", attempt: attempt + 1 })
          : normalizedError;
        lastError = error;
        if (!(error instanceof ProviderRuntimeError) || !error.retryable || attempt + 1 >= config.maxAttempts) break;
        const delayMs = error.retryAfterMs ?? Math.min(60_000, config.retryBackoffMs * 2 ** attempt);
        if (!(await waitForRetry(delayMs, signal))) {
          throw abortFailure(signal, false).withContext({ provider: config.provider, operation: "provider_request", attempt: attempt + 1 });
        }
      }
    }
    if (lastError instanceof ProviderRuntimeError && lastError.retryable) {
      circuit.consecutiveFailures += 1;
      if (lastError.retryable && circuit.consecutiveFailures >= config.circuitBreakerFailureThreshold) circuit.openedUntil = this.clock() + config.circuitBreakerResetMs;
    }
    throw lastError ?? new ProviderRuntimeError("provider invocation failed", { code: "transport", retryable: true, provider: config.provider, operation: "provider_request" });
  }

  private async fetchOnce(config: NormalizedProviderConfig, request: ProviderRequest, credential: CredentialHandle | undefined, callerSignal: AbortSignal | undefined, stream: boolean): Promise<Response> {
    if (config.requiresCredential && credential === undefined) throw new CredentialError(`provider ${config.provider} requires a user credential handle`);
    if (!config.requiresCredential && credential !== undefined) throw new CredentialError(`provider ${config.provider} does not accept a credential handle`);
    if (callerSignal?.aborted) throw abortFailure(callerSignal, false);
    const body = requestBody(config, request, stream);
    const encoded = JSON.stringify(body);
    if (encoded === undefined || bytes(encoded) > MAX_PROVIDER_REQUEST_BYTES) throw new ProviderRuntimeError("provider request exceeds its bounded size");
    const secret = credential === undefined ? null : this.credentials.resolve(credential, config.provider);
    const headers: Record<string, string> = { Accept: stream ? "text/event-stream" : "application/json", "Content-Type": "application/json" };
    if (secret !== null) {
      headers[config.apiKeyHeader] = config.protocol === "anthropic_messages" ? secret : `Bearer ${secret}`;
      if (config.protocol === "anthropic_messages") headers["anthropic-version"] = "2023-06-01";
    }
    if (request.idempotencyKey !== undefined) headers["Idempotency-Key"] = boundedIdentifier("idempotency key", request.idempotencyKey, 512);
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; controller.abort(); }, config.timeoutMs);
    const abort = (): void => controller.abort();
    callerSignal?.addEventListener("abort", abort, { once: true });
    try {
      return await this.fetchImplementation(endpointUrl(config), { method: "POST", headers, body: encoded, signal: controller.signal });
    } catch (unknownError) {
      if (unknownError instanceof ProviderRuntimeError) throw unknownError;
      if (isAbortError(unknownError)) throw abortFailure(callerSignal, timedOut);
      throw new ProviderRuntimeError("provider transport failed; credential material was discarded", { retryable: true, code: "transport" });
    } finally {
      clearTimeout(timer);
      callerSignal?.removeEventListener("abort", abort);
    }
  }

  private async fetchModelCatalog(config: NormalizedProviderConfig, credential: CredentialHandle | undefined, callerSignal: AbortSignal | undefined): Promise<Response> {
    if (config.requiresCredential && credential === undefined) throw new CredentialError(`provider ${config.provider} requires a user credential handle`);
    if (!config.requiresCredential && credential !== undefined) throw new CredentialError(`provider ${config.provider} does not accept a credential handle`);
    if (callerSignal?.aborted) throw abortFailure(callerSignal, false);
    const secret = credential === undefined ? null : this.credentials.resolve(credential, config.provider);
    const headers: Record<string, string> = { Accept: "application/json" };
    if (secret !== null) headers[config.apiKeyHeader] = config.protocol === "anthropic_messages" ? secret : `Bearer ${secret}`;
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; controller.abort(); }, config.timeoutMs);
    const abort = (): void => controller.abort();
    callerSignal?.addEventListener("abort", abort, { once: true });
    try {
      return await this.fetchImplementation(modelsEndpointUrl(config), { method: "GET", headers, signal: controller.signal });
    } catch (unknownError) {
      if (unknownError instanceof CredentialError || unknownError instanceof ProviderRuntimeError) throw unknownError;
      if (isAbortError(unknownError)) throw abortFailure(callerSignal, timedOut);
      throw new ProviderRuntimeError("provider model discovery transport failed; credential material was discarded", { retryable: true, code: "transport" });
    } finally {
      clearTimeout(timer);
      callerSignal?.removeEventListener("abort", abort);
    }
  }

  private record(provider: string, model: string, success: boolean, latencyMs: number, statusCode: number | null, response?: ProviderResponse): void {
    const record = (state: HealthState): void => {
      state.attempts += 1;
      if (success) state.successes += 1; else state.failures += 1;
      state.totalLatencyMs += Math.max(0, latencyMs);
      state.lastLatencyMs = Math.max(0, latencyMs);
      state.lastModel = model;
      state.lastStatusCode = statusCode;
      if (response?.usage.input_tokens !== undefined) void response.usage.input_tokens;
    };
    const providerState = this.providerHealthState.get(provider) ?? emptyHealth();
    const modelState = this.modelHealthState.get(`${provider}/${model}`) ?? emptyHealth();
    record(providerState); record(modelState);
    this.providerHealthState.set(provider, providerState);
    this.modelHealthState.set(`${provider}/${model}`, modelState);
    const circuit = this.circuits.get(provider);
    if (circuit && success) { circuit.consecutiveFailures = 0; circuit.openedUntil = null; }
  }
}

function estimatedProviderCostUnits(candidate: AutonomousModelCandidate | undefined, request: ProviderRequest): number {
  if (!candidate) return 0;
  const estimatedInputTokens = Math.max(1, Math.ceil(request.messages.reduce((sum, message) => sum + bytes(message.content), 0) / 4));
  return ((estimatedInputTokens + request.maxOutputTokens) / 1_000_000) * candidate.cost_per_million_tokens;
}

/**
 * Application-side composition for the autonomous brain boundary.
 *
 * The selector receives candidates plus value-only health and credential readiness. It never
 * receives provider keys, authorization headers, tool arguments, or provider response text. A
 * caller can supply `ApiClient.brainModelSelectContextual` as the selector, or use the bounded
 * local health-weighted fallback for offline workers and tests.
 */
export class AutonomousRuntime {
  readonly llm: LLMRuntime;
  private readonly selector?: AutonomousModelSelector;

  constructor(llm: LLMRuntime, options: { selector?: AutonomousModelSelector } = {}) {
    if (!(llm instanceof LLMRuntime)) throw new ProviderRuntimeError("AutonomousRuntime requires an LLMRuntime");
    if (options.selector !== undefined && typeof options.selector !== "function") throw new ProviderRuntimeError("autonomous model selector must be callable");
    this.llm = llm;
    this.selector = options.selector;
  }

  async select(plan: AutonomousExecutionPlan, options: { excludedProviders?: readonly string[] } = {}): Promise<AutonomousSelectionDecision> {
    const request = this.selectionRequest(plan, options.excludedProviders);
    const ranking = this.rank(request);
    if (!ranking.some((row) => row.eligible)) {
      return { selected_model: null, strategy: this.selector ? "caller_selector" : "deterministic_health_utility", ranking, abstention_reason: ranking.flatMap((row) => row.reasons).join("; ") || "no eligible model candidate" };
    }
    if (this.selector) {
      const selected = await this.selector(request);
      if (!isObject(selected)) throw new ProviderRuntimeError("autonomous model selector returned a malformed decision");
      const projectedRanking = selectorRankingProjection(selected.ranking, ranking);
      const exploration = selectorExplorationProjection(selected);
      const selectedModel = selected.selected_model;
      if (selectedModel === null) return { selected_model: null, strategy: "caller_selector", ranking: projectedRanking, abstention_reason: typeof selected.abstention_reason === "string" ? selected.abstention_reason : "caller selector abstained", ...exploration };
      if (!isObject(selectedModel) || typeof selectedModel.provider !== "string" || typeof selectedModel.model !== "string") throw new ProviderRuntimeError("autonomous selector returned an invalid selected_model");
      const chosen = ranking.find((row) => row.provider === selectedModel.provider && row.model === selectedModel.model);
      if (!chosen || !chosen.eligible) throw new ProviderRuntimeError("autonomous selector chose an ineligible model");
      return { selected_model: { provider: chosen.provider, model: chosen.model }, strategy: "caller_selector", ranking: projectedRanking, abstention_reason: null, ...exploration };
    }
    const chosen = ranking.find((row) => row.eligible);
    return { selected_model: chosen ? { provider: chosen.provider, model: chosen.model } : null, strategy: "deterministic_health_utility", ranking, abstention_reason: chosen ? null : "no eligible model candidate" };
  }

  async invoke(
    plan: AutonomousExecutionPlan,
    options: {
      credential?: CredentialHandle;
      credentialFor?: (provider: string) => CredentialHandle | undefined;
      signal?: AbortSignal;
      observer?: ProviderInvocationObserver;
      feedback?: (decision: AutonomousSelectionDecision, outcome: ProviderInvocationOutcome) => void | Promise<void>;
      execution?: AutonomousExecutionController;
      executionAttempt?: number;
      maxProviderFailovers?: number;
      reserveCost?: AutonomousCostReservationCallback;
    } = {},
  ): Promise<AutonomousExecutionResult> {
    const maxProviderFailovers = autonomousProviderFailoverLimit(options);
    const excludedProviders = new Set<string>();
    let failovers = 0;
    while (true) {
      const selection = await this.select(plan, { excludedProviders: [...excludedProviders] });
      if (!selection.selected_model) throw new ProviderRuntimeError(`autonomous selection abstained: ${selection.abstention_reason ?? "no model"}`);
      const provider = selection.selected_model.provider;
      const credential = options.credential ?? options.credentialFor?.(provider);
      const observer: ProviderInvocationObserver = {
        before: options.observer?.before,
        after: async (metadata, outcome) => {
          await options.observer?.after?.(metadata, outcome);
          await options.feedback?.(selection, outcome);
        },
      };
      const selectedCandidate = plan.candidates.find((candidate) => candidate.provider === provider && candidate.model === selection.selected_model!.model);
      const estimatedCostUnits = estimatedProviderCostUnits(selectedCandidate, plan.request);
      const selectionDigest = await digestJson(selection);
      try {
        const response = await this.llm.invoke(provider, { ...plan.request, model: selection.selected_model.model }, { credential, signal: options.signal, observer, invocationKind: "autonomous_selected_model", execution: options.execution, executionAttempt: options.executionAttempt, executionTurn: 1, executionFailover: failovers > 0, selectionDigest, estimatedCostUnits, reserveCost: options.reserveCost });
        return { selection, response };
      } catch (error) {
        if (!(error instanceof ProviderRuntimeError) || !error.retryable || failovers >= maxProviderFailovers) throw error;
        excludedProviders.add(provider);
        const anotherProviderRemains = plan.candidates.some((candidate) => !excludedProviders.has(candidate.provider));
        if (!anotherProviderRemains) throw error;
        failovers += 1;
      }
    }
  }

  async invokeToolLoop(
    plan: AutonomousExecutionPlan,
    options: {
      authorizeAndExecute: (calls: ProviderToolCall[]) => ProviderToolResult[] | Promise<ProviderToolResult[]>;
      credential?: CredentialHandle;
      credentialFor?: (provider: string) => CredentialHandle | undefined;
      maxTurns?: number;
      maxToolCalls?: number;
      stream?: boolean;
      signal?: AbortSignal;
      observer?: ProviderInvocationObserver;
      feedback?: (decision: AutonomousSelectionDecision, outcome: ProviderInvocationOutcome) => void | Promise<void>;
      execution?: AutonomousExecutionController;
      executionAttempt?: number;
      maxProviderFailovers?: number;
      reserveCost?: AutonomousCostReservationCallback;
      toolReadOnly?: (call: ProviderToolCall) => boolean | Promise<boolean>;
    },
  ): Promise<{ selection: AutonomousSelectionDecision; loop: ProviderToolLoopResult }> {
    const maxProviderFailovers = autonomousProviderFailoverLimit(options);
    const excludedProviders = new Set<string>();
    let failovers = 0;
    let toolActivity = false;
    while (true) {
      const selection = await this.select(plan, { excludedProviders: [...excludedProviders] });
      if (!selection.selected_model) throw new ProviderRuntimeError(`autonomous selection abstained: ${selection.abstention_reason ?? "no model"}`);
      const provider = selection.selected_model.provider;
      const credential = options.credential ?? options.credentialFor?.(provider);
      const observer: ProviderInvocationObserver = {
        before: options.observer?.before,
        after: async (metadata, outcome) => {
          await options.observer?.after?.(metadata, outcome);
          await options.feedback?.(selection, outcome);
        },
      };
      const selectedCandidate = plan.candidates.find((candidate) => candidate.provider === provider && candidate.model === selection.selected_model!.model);
      const estimatedCostUnits = estimatedProviderCostUnits(selectedCandidate, plan.request);
      const selectionDigest = await digestJson(selection);
      const authorizeAndExecute = async (calls: ProviderToolCall[]): Promise<ProviderToolResult[]> => {
        if (calls.length > 0) toolActivity = true;
        return options.authorizeAndExecute(calls);
      };
      try {
        const loop = await this.llm.invokeToolLoop(provider, { ...plan.request, model: selection.selected_model.model }, {
          credential,
          authorizeAndExecute,
          maxTurns: options.maxTurns,
          maxToolCalls: options.maxToolCalls,
          stream: options.stream,
          signal: options.signal,
          observer,
          execution: options.execution,
          executionAttempt: options.executionAttempt,
          executionFailover: failovers > 0,
          selectionDigest,
          estimatedCostUnits,
          reserveCost: options.reserveCost,
          costEstimator: (request) => estimatedProviderCostUnits(selectedCandidate, request),
          toolReadOnly: options.toolReadOnly,
        });
        return { selection, loop };
      } catch (error) {
        // Replaying a loop after any provider-issued tool call could duplicate an effect. A
        // failover is therefore permitted only before the first tool request is observed.
        if (toolActivity || !(error instanceof ProviderRuntimeError) || !error.retryable || failovers >= maxProviderFailovers) throw error;
        excludedProviders.add(provider);
        const anotherProviderRemains = plan.candidates.some((candidate) => !excludedProviders.has(candidate.provider));
        if (!anotherProviderRemains) throw error;
        failovers += 1;
      }
    }
  }

  private selectionRequest(plan: AutonomousExecutionPlan, excludedProviders: readonly string[] = []): AutonomousSelectionRequest {
    if (!isObject(plan) || typeof plan.task !== "string" || plan.task.trim().length === 0 || bytes(plan.task) > 16_000) throw new ProviderRuntimeError("autonomous task is outside its bounds");
    validateRequest(plan.request);
    if (!Array.isArray(plan.candidates) || plan.candidates.length === 0 || plan.candidates.length > MAX_PROVIDER_TOOLS) throw new ProviderRuntimeError("autonomous model candidates are outside their bounds");
    if (plan.requiredCapabilities !== undefined && (!Array.isArray(plan.requiredCapabilities) || plan.requiredCapabilities.length > 64 || plan.requiredCapabilities.some((capability) => typeof capability !== "string" || capability.trim().length === 0 || capability.length > 256))) throw new ProviderRuntimeError("autonomous required capabilities are outside their bounds");
    if (plan.taskFamily !== undefined && (typeof plan.taskFamily !== "string" || !plan.taskFamily.trim() || bytes(plan.taskFamily) > 256)) throw new ProviderRuntimeError("autonomous task family is outside its bounds");
    if (plan.learningContextDigest !== undefined && (typeof plan.learningContextDigest !== "string" || !/^[0-9a-f]{64}$/.test(plan.learningContextDigest))) throw new ProviderRuntimeError("autonomous learning context digest is malformed");
    validateSelectionConstraints({
      max_cost_per_million_tokens: plan.maxCostPerMillionTokens,
      max_latency_ms: plan.maxLatencyMs,
      min_quality: plan.minQuality,
    });
    const excluded = new Set(excludedProviders.map((provider) => boundedIdentifier("excluded provider", provider, 128)));
    const candidates = plan.candidates.map((candidate) => {
      if (!isObject(candidate)) throw new ProviderRuntimeError("autonomous model candidate must be an object");
      const normalized = candidate as unknown as AutonomousModelCandidate;
      boundedIdentifier("candidate provider", normalized.provider, 128);
      boundedIdentifier("candidate model", normalized.model, 512);
      for (const field of ["context_window_tokens", "max_output_tokens", "quality", "latency_ms", "cost_per_million_tokens", "reliability"] as const) {
        if (typeof normalized[field] !== "number" || !Number.isFinite(normalized[field]) || normalized[field] < 0) throw new ProviderRuntimeError(`candidate ${field} is outside its bounds`);
      }
      if (normalized.quality > 1 || normalized.reliability > 1 || normalized.context_window_tokens < 1 || normalized.max_output_tokens < 1) throw new ProviderRuntimeError("candidate quality, reliability, or capacity is outside its bounds");
      return normalized;
    }).filter((candidate) => !excluded.has(candidate.provider));
    const providers = new Set(candidates.map((candidate) => candidate.provider));
    const providerHealth: Record<string, ProviderHealth> = {};
    for (const provider of providers) {
      const metadata = this.llm.providerMetadata().find((row) => row.provider === provider);
      if (!metadata) {
        providerHealth[provider] = { provider, circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: true, credential_ready: false, eligible: false };
        continue;
      }
      const status = this.llm.providerStatus(provider);
      const credential = this.llm.onboarding.status(provider);
      const structuredOutputMode = metadata.structured_output_mode;
      providerHealth[provider] = {
        ...status,
        ...(structuredOutputMode === "disabled" || structuredOutputMode === "json_object" || structuredOutputMode === "json_schema" ? { structured_output_mode: structuredOutputMode } : {}),
        credential_ready: credential.ready === true,
        eligible: status.circuit !== "open" && (status.credential_required === false || credential.ready === true),
      };
    }
    return {
      task: plan.task,
      domain: plan.domain ?? "general",
      capability: plan.capability ?? "general_reasoning",
      risk_class: plan.riskClass ?? "review_required",
      ...(plan.taskFamily !== undefined ? { task_family: plan.taskFamily } : {}),
      ...(plan.learningContextDigest !== undefined ? { context_digest: plan.learningContextDigest } : {}),
      required_capabilities: [...(plan.requiredCapabilities ?? [])],
      estimated_input_tokens: Math.max(1, Math.ceil(plan.request.messages.reduce((sum, message) => sum + bytes(message.content), 0) / 4)),
      requested_output_tokens: plan.request.maxOutputTokens,
      max_cost_per_million_tokens: plan.maxCostPerMillionTokens ?? null,
      max_latency_ms: plan.maxLatencyMs ?? null,
      min_quality: plan.minQuality ?? null,
      require_json: plan.request.requireJson === true,
      candidates,
      provider_health: providerHealth,
      model_health: this.llm.modelHealthSnapshot(),
    };
  }

  private rank(request: AutonomousSelectionRequest): AutonomousModelRanking[] {
    return rankAutonomousModels(request);
  }
}

function selectorRankingProjection(value: unknown, fallback: AutonomousModelRanking[]): AutonomousModelRanking[] {
  if (value === undefined || value === null) return fallback;
  if (!Array.isArray(value) || value.length > 128) throw new ProviderRuntimeError("autonomous model selector returned a malformed ranking");
  if (value.length === 0) return fallback;
  return value.map((row) => {
    if (!isObject(row) || typeof row.provider !== "string" || !row.provider.trim() || typeof row.model !== "string" || !row.model.trim() || typeof row.score !== "number" || !Number.isFinite(row.score) || typeof row.eligible !== "boolean" || !Array.isArray(row.reasons) || row.reasons.length > 64 || row.reasons.some((reason) => typeof reason !== "string" || !reason.trim())) {
      throw new ProviderRuntimeError("autonomous model selector returned a malformed ranking row");
    }
    return { provider: row.provider, model: row.model, score: row.score, eligible: row.eligible, reasons: [...row.reasons] };
  });
}

function selectorExplorationProjection(value: JsonObject): Pick<AutonomousSelectionDecision, "exploration_draw" | "exploration_taken"> {
  const draw = value.exploration_draw;
  const taken = value.exploration_taken;
  if (draw !== undefined && draw !== null && (typeof draw !== "number" || !Number.isFinite(draw) || draw < 0 || draw > 1)) throw new ProviderRuntimeError("autonomous model selector returned an invalid exploration_draw");
  if (taken !== undefined && typeof taken !== "boolean") throw new ProviderRuntimeError("autonomous model selector returned an invalid exploration_taken flag");
  return {
    ...(draw === undefined ? {} : { exploration_draw: draw as number | null }),
    ...(taken === undefined ? {} : { exploration_taken: taken as boolean }),
  };
}

function validateSelectionConstraints(request: Pick<AutonomousSelectionRequest, "max_cost_per_million_tokens" | "max_latency_ms" | "min_quality">): void {
  const constraints: Array<[string, unknown, number]> = [
    ["max_cost_per_million_tokens", request.max_cost_per_million_tokens, 1_000_000_000],
    ["max_latency_ms", request.max_latency_ms, 10 * 60_000],
    ["min_quality", request.min_quality, 1],
  ];
  for (const [name, value, maximum] of constraints) {
    if (value === undefined || value === null) continue;
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) {
      throw new ProviderRuntimeError(`autonomous selection ${name} is outside its bounds`);
    }
  }
}

function validateSelectionOutputRequirements(request: Pick<AutonomousSelectionRequest, "require_json">): void {
  if (request.require_json !== undefined && typeof request.require_json !== "boolean") throw new ProviderRuntimeError("autonomous selection require_json must be boolean");
}

/**
 * Pure, deterministic model ranking shared by the local runtime and persisted-health adapters.
 * It consumes only candidate metadata, credential readiness, aggregate health values, and
 * caller-owned selection constraints.
 */
export function rankAutonomousModels(request: AutonomousSelectionRequest): AutonomousModelRanking[] {
  validateSelectionConstraints(request);
  validateSelectionOutputRequirements(request);
  return request.candidates.map((candidate) => {
    const reasons: string[] = [];
    const provider = request.provider_health[candidate.provider];
    const model = request.model_health[`${candidate.provider}/${candidate.model}`];
    if (candidate.enabled === false) reasons.push("candidate disabled");
    if (!provider) reasons.push("provider not registered");
    if (provider?.circuit === "open") reasons.push("provider circuit open");
    if (model?.circuit === "open") reasons.push("model circuit open");
    if (provider?.credential_required !== false && provider?.credential_ready !== true) reasons.push("credential not ready");
    if (candidate.max_output_tokens < request.requested_output_tokens) reasons.push("model output capacity is below the request");
    if (candidate.context_window_tokens < request.estimated_input_tokens + request.requested_output_tokens) reasons.push("model context capacity is below the request");
    if (request.required_capabilities.some((required) => !(candidate.capabilities ?? []).includes(required))) reasons.push("model lacks a required capability");
    if (request.require_json === true && !(candidate.capabilities ?? []).includes("structured_output")) reasons.push("model lacks structured output capability");
    if (request.require_json === true && provider && provider.structured_output_mode === undefined) reasons.push("provider structured output capability is unknown");
    if (request.require_json === true && provider?.structured_output_mode === "disabled") reasons.push("provider structured output is disabled");
    if (request.max_cost_per_million_tokens !== undefined && request.max_cost_per_million_tokens !== null && candidate.cost_per_million_tokens > request.max_cost_per_million_tokens) reasons.push("model cost exceeds the caller budget");
    if (request.max_latency_ms !== undefined && request.max_latency_ms !== null && candidate.latency_ms > request.max_latency_ms) reasons.push("model latency exceeds the caller bound");
    if (request.min_quality !== undefined && request.min_quality !== null && candidate.quality < request.min_quality) reasons.push("model quality is below the caller floor");
    const healthRate = typeof model?.success_rate === "number" && model.attempts && model.attempts > 0 ? model.success_rate : 0.5;
    const qualityObservations = model?.quality_observations ?? 0;
    const qualityRate = typeof model?.quality_mean === "number" && qualityObservations > 0 ? model.quality_mean : null;
    const latencyUtility = 1 - Math.min(1, candidate.latency_ms / 60_000);
    const costUtility = 1 - Math.min(1, candidate.cost_per_million_tokens / 10_000);
    const adaptiveHealth = qualityRate === null ? healthRate * 0.15 : healthRate * 0.1 + qualityRate * 0.05;
    const score = candidate.quality * 0.4 + candidate.reliability * 0.3 + adaptiveHealth + latencyUtility * 0.1 + costUtility * 0.05;
    return { provider: candidate.provider, model: candidate.model, score: Number(score.toFixed(12)), eligible: reasons.length === 0, reasons };
  }).sort((left, right) => Number(right.eligible) - Number(left.eligible) || right.score - left.score || left.provider.localeCompare(right.provider) || left.model.localeCompare(right.model));
}

function emptyHealth(): HealthState {
  return { attempts: 0, successes: 0, failures: 0, totalLatencyMs: 0, lastLatencyMs: null, lastModel: null, lastStatusCode: null };
}

function healthProjection(provider: string, state: HealthState, circuit: "closed" | "open", consecutiveFailures: number, requiresCredential: boolean): ProviderHealth {
  return {
    provider,
    circuit,
    consecutive_failures: consecutiveFailures,
    attempts: state.attempts,
    successes: state.successes,
    failures: state.failures,
    success_rate: state.attempts === 0 ? 0 : state.successes / state.attempts,
    mean_latency_ms: state.attempts === 0 ? null : state.totalLatencyMs / state.attempts,
    last_latency_ms: state.lastLatencyMs,
    last_model: state.lastModel,
    last_status_code: state.lastStatusCode,
    credential_posture: "caller_supplied_opaque_handle",
    credential_required: requiresCredential,
  };
}

function providerHttpError(status: number, headers?: Headers): ProviderRuntimeError {
  return new ProviderRuntimeError(`provider returned HTTP status ${status}`, {
    retryable: retryableStatus(status),
    statusCode: status,
    code: status >= 500 ? "http_5xx" : "http_4xx",
    requestId: headers ? requestIdFromHeaders(headers) ?? undefined : undefined,
    retryAfterMs: headers ? retryAfterMsFromHeaders(headers) : undefined,
  });
}

export type ProviderFactoryOptions = Omit<ProviderConfig, "provider" | "protocol" | "baseUrl"> & { baseUrl?: string };

export function openaiProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return { ...rest, provider: "openai", protocol: "openai_responses", baseUrl: baseUrl ?? "https://api.openai.com", modelsPath: rest.modelsPath ?? "/v1/models" };
}

export function anthropicProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return { ...rest, provider: "anthropic", protocol: "anthropic_messages", baseUrl: baseUrl ?? "https://api.anthropic.com", modelsPath: rest.modelsPath ?? "/v1/models", structuredOutputMode: rest.structuredOutputMode ?? "disabled" };
}

export function openaiCompatibleProvider(provider: string, baseUrl: string, options: Omit<ProviderConfig, "provider" | "protocol" | "baseUrl"> = {}): ProviderConfig {
  return { provider, baseUrl, protocol: "openai_chat_completions", ...options };
}

/** Official OpenAI-compatible provider presets used by the autonomous BYOK setup flow. */
export function deepseekProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return openaiCompatibleProvider("deepseek", baseUrl ?? "https://api.deepseek.com", {
    ...rest,
    path: rest.path ?? "/chat/completions",
    modelsPath: rest.modelsPath ?? "/models",
    structuredOutputMode: rest.structuredOutputMode ?? "json_object",
  });
}

export function groqProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return openaiCompatibleProvider("groq", baseUrl ?? "https://api.groq.com/openai/v1", {
    ...rest,
    path: rest.path ?? "/chat/completions",
    modelsPath: rest.modelsPath ?? "/models",
    structuredOutputMode: rest.structuredOutputMode ?? "json_object",
  });
}

export function mistralProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return openaiCompatibleProvider("mistral", baseUrl ?? "https://api.mistral.ai", {
    ...rest,
    path: rest.path ?? "/v1/chat/completions",
    modelsPath: rest.modelsPath ?? "/v1/models",
    structuredOutputMode: rest.structuredOutputMode ?? "json_object",
  });
}

export function openrouterProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return openaiCompatibleProvider("openrouter", baseUrl ?? "https://openrouter.ai/api/v1", {
    ...rest,
    path: rest.path ?? "/chat/completions",
    modelsPath: rest.modelsPath ?? "/models",
    structuredOutputMode: rest.structuredOutputMode ?? "json_object",
  });
}

export function xaiProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return openaiCompatibleProvider("xai", baseUrl ?? "https://api.x.ai", {
    ...rest,
    path: rest.path ?? "/v1/chat/completions",
    modelsPath: rest.modelsPath ?? "/v1/models",
    structuredOutputMode: rest.structuredOutputMode ?? "json_object",
  });
}

const DEFAULT_CREDENTIAL_ENVIRONMENT_VARIABLES: Record<string, string> = {
  openai: "OPENAI_API_KEY",
  anthropic: "ANTHROPIC_API_KEY",
  deepseek: "DEEPSEEK_API_KEY",
  groq: "GROQ_API_KEY",
  mistral: "MISTRAL_API_KEY",
  openrouter: "OPENROUTER_API_KEY",
  xai: "XAI_API_KEY",
};

type CredentialResolver = (reference: string) => string | Promise<string>;

function boundedCredentialLabel(name: string, value: string): string {
  if (typeof value !== "string" || value.trim().length === 0 || bytes(value) > MAX_CREDENTIAL_SOURCE_LABEL_BYTES || /[\u0000-\u001f]/.test(value)) {
    throw new CredentialError(`${name} is outside its bounded metadata contract`);
  }
  return value;
}

function boundedCredentialTtl(ttlMs: number | null | undefined): number | null {
  if (ttlMs === undefined || ttlMs === null) return null;
  if (!Number.isInteger(ttlMs) || ttlMs < 1 || ttlMs > 7 * 24 * 60 * 60 * 1000) throw new CredentialError("credential ttlMs must be an integer between 1ms and 7 days");
  return ttlMs;
}

async function digestCredentialReference(reference: string): Promise<string> {
  const cryptoObject = (globalThis as { crypto?: { subtle?: SubtleCrypto } }).crypto;
  if (cryptoObject?.subtle) {
    const digest = await cryptoObject.subtle.digest("SHA-256", new TextEncoder().encode(reference));
    return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("");
  }
  // Older non-secure test/browser runtimes may not expose Web Crypto. This fallback is only a
  // deterministic redacted identity for source replacement; it is never used for authentication.
  let first = 2166136261;
  let second = 16777619;
  for (const character of reference) {
    first = Math.imul(first ^ character.charCodeAt(0), 16777619) >>> 0;
    second = Math.imul(second ^ character.charCodeAt(0), 2166136261) >>> 0;
  }
  return `${first.toString(16).padStart(8, "0")}${second.toString(16).padStart(8, "0")}`.repeat(4);
}

/** Explicit BYOK lifecycle for UI, environment, or secret-manager integrations. */
export class ProviderOnboarding {
  readonly runtime: LLMRuntime;
  private readonly environmentVariables: Record<string, string>;

  constructor(runtime: LLMRuntime, options: { environmentVariables?: Record<string, string> } = {}) {
    if (!runtime || typeof runtime.providerMetadata !== "function") throw new CredentialError("ProviderOnboarding requires an LLMRuntime");
    this.runtime = runtime;
    this.environmentVariables = { ...DEFAULT_CREDENTIAL_ENVIRONMENT_VARIABLES };
    for (const [provider, variable] of Object.entries(options.environmentVariables ?? {})) {
      boundedIdentifier("provider", provider, 128);
      this.environmentVariables[provider] = boundedCredentialLabel("environment variable", variable);
    }
  }

  registerProvider(config: ProviderConfig): void {
    this.runtime.registerProvider(config);
  }

  registerValue(provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    this.requireProvider(provider);
    return this.runtime.credentials.register(provider, value, options);
  }

  collectUserCredential(provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    // The caller must obtain `value` through its own authenticated/encrypted UI boundary. This
    // method is deliberately the only named API that presents as user key collection.
    return this.registerValue(provider, value, options);
  }

  async configureFromPrompt(provider: string, options: { prompt?: string; ttlMs?: number; reader?: (prompt: string) => string | Promise<string> } = {}): Promise<CredentialHandle> {
    this.requireProvider(provider);
    if (typeof options.reader !== "function") throw new CredentialError("a no-echo prompt reader is required in TypeScript runtimes");
    const value = await options.reader(options.prompt ?? `${provider} API key: `);
    return this.collectUserCredential(provider, value, { ttlMs: options.ttlMs });
  }

  configureFromEnvironment(provider: string, options: { variable?: string; ttlMs?: number; environment?: Record<string, string | undefined> } = {}): CredentialHandle {
    this.requireProvider(provider);
    const variable = boundedCredentialLabel("environment variable", options.variable ?? this.environmentVariables[provider] ?? "");
    return this.runtime.credentials.registerEnvironment(provider, variable, options.environment, { ttlMs: options.ttlMs });
  }

  async configureFromResolver(provider: string, reference: string, resolver: CredentialResolver, options: { ttlMs?: number } = {}): Promise<CredentialHandle> {
    this.requireProvider(provider);
    boundedCredentialLabel("credential resolver reference", reference);
    if (typeof resolver !== "function") throw new CredentialError("external credential resolver must be callable");
    return this.runtime.credentials.registerResolver(provider, () => resolver(reference), options);
  }

  revoke(handle: CredentialHandle): void {
    this.runtime.credentials.revoke(handle);
  }

  status(provider: string): JsonObject {
    const normalizedProvider = boundedIdentifier("provider", provider, 128);
    const metadata = this.runtime.providerMetadata().find((row) => row.provider === normalizedProvider);
    const registered = metadata !== undefined;
    const requiresCredential = metadata?.requires_credential;
    const credential = this.runtime.credentials.status(normalizedProvider, registered);
    const ready = registered && (requiresCredential === false || credential.ready);
    return {
      provider: normalizedProvider,
      provider_registered: registered,
      credential: {
        provider: normalizedProvider,
        configured: credential.ready,
        credential_count: credential.active_handles,
        credentials: [{ active_handles: credential.active_handles, expires_at: credential.expires_at }],
        secret_persistence: "in_memory_only",
        secret_material: "never_returned",
      },
      requires_credential: requiresCredential ?? null,
      ready,
      next_action: ready ? "ready" : registered ? "collect_user_credential" : "register_provider",
      secret_material: "never_returned",
    };
  }

  instructions(provider: string): ProviderCredentialInstructions {
    const current = this.status(provider);
    const registered = current.provider_registered === true;
    const requiresCredential = typeof current.requires_credential === "boolean" ? current.requires_credential : null;
    const ready = current.ready === true || (registered && requiresCredential === false);
    return {
      provider: current.provider as string,
      provider_registered: registered,
      requires_credential: requiresCredential,
      ready,
      next_action: ready ? "ready" : registered ? "collect_user_credential" : "register_provider",
      input_methods: ["protected_ui", "no_echo_prompt", "environment_variable", "external_secret_resolver"],
      environment_variable: this.environmentVariables[current.provider as string] ?? null,
      secret_material: "never_returned",
    };
  }

  statuses(): JsonObject[] {
    const providers = new Set<string>([
      ...this.runtime.providerMetadata().map((row) => row.provider).filter((provider): provider is string => typeof provider === "string"),
      ...this.runtime.credentials.knownProviders(),
    ]);
    return [...providers].sort().map((provider) => this.status(provider));
  }

  startSession(options: { ttlMs?: number; sessionId?: string; clock?: () => number } = {}): CredentialSession {
    return new CredentialSession(this, options);
  }

  requireProvider(provider: string): void {
    const normalized = boundedIdentifier("provider", provider, 128);
    if (!this.runtime.providerMetadata().some((row) => row.provider === normalized)) throw new CredentialError(`provider ${normalized} is not registered with the runtime`);
  }
}

/** Short-lived handle group. Closing or expiry revokes every handle created through the session. */
export class CredentialSession {
  readonly onboarding: ProviderOnboarding;
  readonly sessionId: string;
  readonly createdAt: number;
  readonly expiresAt: number | null;
  private readonly clock: () => number;
  private readonly handlesByProvider = new Map<string, CredentialHandle>();
  private closed = false;

  constructor(onboarding: ProviderOnboarding, options: { ttlMs?: number; sessionId?: string; clock?: () => number } = {}) {
    if (!(onboarding instanceof ProviderOnboarding)) throw new CredentialError("CredentialSession requires ProviderOnboarding");
    this.onboarding = onboarding;
    this.clock = options.clock ?? (() => Date.now());
    this.createdAt = this.clock();
    const ttlMs = boundedCredentialTtl(options.ttlMs);
    this.expiresAt = ttlMs === null ? null : this.createdAt + ttlMs;
    this.sessionId = boundedCredentialLabel("session id", options.sessionId ?? `session-${newOpaqueId()}`);
  }

  registerValue(provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    return this.attach(this.onboarding.registerValue(provider, value, options));
  }

  collectUserCredential(provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    return this.attach(this.onboarding.collectUserCredential(provider, value, options));
  }

  async configureFromPrompt(provider: string, options: { prompt?: string; ttlMs?: number; reader?: (prompt: string) => string | Promise<string> } = {}): Promise<CredentialHandle> {
    return this.attach(await this.onboarding.configureFromPrompt(provider, options));
  }

  configureFromEnvironment(provider: string, options: { variable?: string; ttlMs?: number; environment?: Record<string, string | undefined> } = {}): CredentialHandle {
    return this.attach(this.onboarding.configureFromEnvironment(provider, options));
  }

  async configureFromResolver(provider: string, reference: string, resolver: CredentialResolver, options: { ttlMs?: number } = {}): Promise<CredentialHandle> {
    return this.attach(await this.onboarding.configureFromResolver(provider, reference, resolver, options));
  }

  handle(provider: string): CredentialHandle {
    this.assertActive();
    const normalized = boundedIdentifier("provider", provider, 128);
    const handle = this.handlesByProvider.get(normalized);
    if (!handle) throw new CredentialError(`provider ${normalized} is not configured in this session`);
    this.onboarding.runtime.credentials.resolve(handle, normalized);
    return handle;
  }

  handles(): Record<string, CredentialHandle> {
    this.assertActive();
    const result: Record<string, CredentialHandle> = {};
    for (const [provider, handle] of this.handlesByProvider) {
      this.onboarding.runtime.credentials.resolve(handle, provider);
      result[provider] = handle;
    }
    return result;
  }

  status(): CredentialSessionStatus {
    return {
      session_id: this.sessionId,
      active: this.isActive(),
      created_at: this.createdAt,
      expires_at: this.expiresAt,
      providers: [...this.handlesByProvider.keys()].sort(),
      secret_persistence: "in_memory_only",
      secret_material: "never_returned",
    };
  }

  providerStatuses(): JsonObject[] {
    this.assertActive();
    return [...this.handlesByProvider.keys()].sort().map((provider) => this.onboarding.status(provider));
  }

  instructions(provider: string): ProviderCredentialInstructions {
    this.assertActive();
    return this.onboarding.instructions(provider);
  }

  revoke(provider: string): void {
    this.assertActive();
    const normalized = boundedIdentifier("provider", provider, 128);
    const handle = this.handlesByProvider.get(normalized);
    if (handle) {
      this.handlesByProvider.delete(normalized);
      this.onboarding.revoke(handle);
    }
  }

  close(): void {
    if (this.closed) return;
    const handles = [...this.handlesByProvider.values()];
    this.handlesByProvider.clear();
    this.closed = true;
    for (const handle of handles) {
      try { this.onboarding.revoke(handle); } catch (error) { if (!(error instanceof CredentialError)) throw error; }
    }
  }

  private isActive(): boolean {
    return !this.closed && (this.expiresAt === null || this.expiresAt > this.clock());
  }

  private assertActive(): void {
    if (this.isActive()) return;
    this.close();
    throw new CredentialError("credential session is closed or expired");
  }

  private attach(handle: CredentialHandle): CredentialHandle {
    try { this.assertActive(); } catch (error) { this.onboarding.revoke(handle); throw error; }
    const previous = this.handlesByProvider.get(handle.provider);
    this.handlesByProvider.set(handle.provider, handle);
    if (previous && previous !== handle) this.onboarding.revoke(previous);
    return handle;
  }
}

interface CredentialSourceInternal {
  readonly spec: CredentialSourceSpec;
  readonly environmentVariable?: string;
  readonly reference?: string;
  readonly resolver?: CredentialResolver;
}

/** Process-local source registry for non-interactive deployments and secret rotation. */
export class CredentialProvisioner {
  readonly onboarding: ProviderOnboarding;
  readonly maxSources: number;
  private readonly sources = new Map<string, CredentialSourceInternal[]>();

  constructor(onboarding: ProviderOnboarding, options: { maxSources?: number } = {}) {
    if (!(onboarding instanceof ProviderOnboarding)) throw new CredentialError("CredentialProvisioner requires ProviderOnboarding");
    const maxSources = options.maxSources ?? MAX_CREDENTIAL_PROVISIONING_SOURCES;
    if (!Number.isInteger(maxSources) || maxSources < 1 || maxSources > MAX_CREDENTIAL_PROVISIONING_SOURCES) throw new CredentialError("maxSources is outside its bounds");
    this.onboarding = onboarding;
    this.maxSources = maxSources;
  }

  registerEnvironment(provider: string, options: { variable?: string; ttlMs?: number; required?: boolean; sourceLabel?: string; replaceExisting?: boolean } = {}): CredentialSourceSpec {
    this.onboarding.requireProvider(provider);
    const variable = boundedCredentialLabel("environment variable", options.variable ?? DEFAULT_CREDENTIAL_ENVIRONMENT_VARIABLES[provider] ?? "");
    const spec: CredentialSourceSpec = {
      provider,
      source_kind: "environment_variable",
      source_id: `environment:${variable}`,
      source_label: boundedCredentialLabel("credential source label", options.sourceLabel ?? `environment:${variable}`),
      environment_variable: variable,
      ttl_ms: boundedCredentialTtl(options.ttlMs),
      required: options.required ?? true,
      enabled: true,
      secret_material: "never_returned",
    };
    return this.registerInternal({ spec, environmentVariable: variable }, `environment:${variable}`, options.replaceExisting ?? false);
  }

  async registerResolver(provider: string, reference: string, resolver: CredentialResolver, options: { ttlMs?: number; required?: boolean; sourceLabel?: string; replaceExisting?: boolean } = {}): Promise<CredentialSourceSpec> {
    this.onboarding.requireProvider(provider);
    boundedCredentialLabel("credential resolver reference", reference);
    if (typeof resolver !== "function") throw new CredentialError("external credential resolver must be callable");
    const digest = await digestCredentialReference(reference);
    const spec: CredentialSourceSpec = {
      provider,
      source_kind: "external_secret_resolver",
      source_id: `resolver:${digest.slice(0, 16)}`,
      source_label: boundedCredentialLabel("credential source label", options.sourceLabel ?? "external secret resolver"),
      reference_digest: digest,
      ttl_ms: boundedCredentialTtl(options.ttlMs),
      required: options.required ?? true,
      enabled: true,
      secret_material: "never_returned",
    };
    return this.registerInternal({ spec, reference, resolver }, `resolver:${digest}`, options.replaceExisting ?? false);
  }

  unregister(provider: string, sourceId: string): boolean {
    boundedIdentifier("provider", provider, 128);
    boundedCredentialLabel("source id", sourceId);
    const current = this.sources.get(provider) ?? [];
    const retained = current.filter((source) => source.spec.source_id !== sourceId);
    if (retained.length === current.length) return false;
    if (retained.length) this.sources.set(provider, retained); else this.sources.delete(provider);
    return true;
  }

  sourceSpecs(provider?: string): CredentialSourceSpec[] {
    if (provider !== undefined) boundedIdentifier("provider", provider, 128);
    return [...this.sources.entries()]
      .filter(([name]) => provider === undefined || name === provider)
      .flatMap(([, rows]) => rows.map((source) => source.spec))
      .sort((a, b) => a.provider.localeCompare(b.provider) || a.source_id.localeCompare(b.source_id));
  }

  plan(providers?: readonly string[]): JsonObject {
    const selected = this.selectedProviders(providers);
    const rows = selected.map((provider) => {
      const status = this.onboarding.status(provider);
      const metadata = this.onboarding.runtime.providerMetadata().find((row) => row.provider === provider);
      const sourceRows = this.sources.get(provider) ?? [];
      const required = sourceRows.length > 0 && sourceRows.some((source) => source.spec.required);
      const nextAction = !metadata ? "register_provider" : metadata.requires_credential === false || status.ready === true ? "ready" : sourceRows.length === 0 ? "register_credential_source" : "provision_session";
      return {
        provider,
        provider_registered: metadata !== undefined,
        requires_credential: metadata?.requires_credential ?? null,
        credential_ready: status.ready === true,
        required,
        source_count: sourceRows.length,
        sources: sourceRows.map((source) => source.spec),
        next_action: nextAction,
      };
    });
    return {
      schema: CREDENTIAL_PROVISIONING_SCHEMA,
      providers: rows,
      provider_count: rows.length,
      execution: "process_local_resolution_into_short_lived_session",
      restart_posture: "re-register_sources_and_resolve_fresh_handles",
      retention: "metadata_only_no_keys_references_or_callbacks",
      secret_material: "never_returned",
    };
  }

  async provision(session: CredentialSession, options: { providers?: readonly string[]; environment?: Record<string, string | undefined> } = {}): Promise<CredentialProvisioningResult> {
    if (!(session instanceof CredentialSession) || session.onboarding !== this.onboarding) throw new CredentialError("credential session belongs to a different onboarding runtime");
    const selected = this.selectedProviders(options.providers);
    const receipts: CredentialProvisioningReceipt[] = [];
    const failures = new Set<string>();
    for (const provider of selected) {
      const metadata = this.onboarding.runtime.providerMetadata().find((row) => row.provider === provider);
      if (!metadata) {
        receipts.push(this.receipt(provider, "missing_provider", false));
        failures.add(provider);
        continue;
      }
      if (metadata.requires_credential === false) {
        receipts.push(this.receipt(provider, "not_required", true));
        continue;
      }
      try {
        session.handle(provider);
        receipts.push(this.receipt(provider, "already_present", true));
        continue;
      } catch (error) {
        if (!(error instanceof CredentialError)) throw error;
      }
      const sourceRows = (this.sources.get(provider) ?? []).filter((source) => source.spec.enabled);
      if (!sourceRows.length) {
        receipts.push(this.receipt(provider, "missing_source", false));
        failures.add(provider);
        continue;
      }
      let lastError = "CredentialError";
      let provisioned: CredentialProvisioningReceipt | null = null;
      for (let index = 0; index < sourceRows.length; index += 1) {
        const source = sourceRows[index];
        if (!source) break;
        try {
          if (source.spec.source_kind === "environment_variable") {
            session.configureFromEnvironment(provider, { variable: source.environmentVariable, ttlMs: source.spec.ttl_ms ?? undefined, environment: options.environment });
          } else if (source.reference && source.resolver) {
            await session.configureFromResolver(provider, source.reference, source.resolver, { ttlMs: source.spec.ttl_ms ?? undefined });
          } else {
            throw new CredentialError("credential resolver source is not operational");
          }
          provisioned = this.receipt(provider, "provisioned", true, source.spec, index + 1);
          break;
        } catch (error) {
          if (!(error instanceof CredentialError)) throw error;
          lastError = error.constructor.name;
        }
      }
      if (provisioned) receipts.push(provisioned);
      else {
        const required = sourceRows.some((source) => source.spec.required);
        receipts.push(this.receipt(provider, "source_failed", false, undefined, sourceRows.length, lastError));
        if (required) failures.add(provider);
      }
    }
    return {
      schema: CREDENTIAL_PROVISIONING_SCHEMA,
      session_id: session.sessionId,
      ready: failures.size === 0,
      receipts,
      required_failures: [...failures].sort(),
      credential_posture: "opaque_handles_only; sources_resolved_in_process",
      secret_material: "never_returned",
    };
  }

  private selectedProviders(providers?: readonly string[]): string[] {
    const selected = providers === undefined
      ? new Set<string>([
        ...this.onboarding.runtime.providerMetadata().map((row) => row.provider).filter((provider): provider is string => typeof provider === "string"),
        ...this.sources.keys(),
      ])
      : new Set(providers.map((provider) => boundedIdentifier("provider", provider, 128)));
    if (selected.size > MAX_CREDENTIAL_PROVISIONING_PROVIDERS) throw new CredentialError("credential provisioning provider count exceeds its bound");
    return [...selected].sort();
  }

  private registerInternal(source: CredentialSourceInternal, identity: string, replaceExisting: boolean): CredentialSourceSpec {
    const current = this.sources.get(source.spec.provider) ?? [];
    const index = current.findIndex((row) => row.spec.source_kind === source.spec.source_kind && (row.environmentVariable ?? row.spec.reference_digest) === identity.replace(/^environment:/, "").replace(/^resolver:/, ""));
    if (index >= 0) {
      if (!replaceExisting) throw new CredentialError("credential source is already registered");
      current[index] = source;
      this.sources.set(source.spec.provider, current);
      return source.spec;
    }
    const count = [...this.sources.values()].reduce((sum, rows) => sum + rows.length, 0);
    if (count >= this.maxSources) throw new CredentialError("credential provisioning source capacity is exhausted");
    this.sources.set(source.spec.provider, [...current, source]);
    return source.spec;
  }

  private receipt(provider: string, status: CredentialProvisioningReceipt["status"], ready: boolean, source?: CredentialSourceSpec, attempts = 0, errorClass: string | null = null): CredentialProvisioningReceipt {
    return { schema: CREDENTIAL_PROVISIONING_SCHEMA, provider, status, credential_ready: ready, source_kind: source?.source_kind ?? null, source_id: source?.source_id ?? null, source_attempts: attempts, error_class: errorClass, secret_persistence: "in_memory_only", secret_material: "never_returned" };
  }
}
