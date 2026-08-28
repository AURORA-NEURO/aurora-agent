import { ArgumentError, AutonomousCostBudgetError, CredentialError, ProviderRuntimeError, ResponseTooLargeError, isObject } from "./errors.js";
import type { ProviderErrorCode, ProviderFailureClass } from "./errors.js";
import { AUTONOMOUS_EXECUTION_MAX_PROVIDER_FAILOVERS } from "./autonomous-execution.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import { AutonomousEffectReconciliationRequiredError } from "./autonomous-effects.js";
import type { AutonomousEffectBoundary } from "./autonomous-effects.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";
import { ProviderQuotaController, type ProviderQuotaReservation } from "./provider-quota.js";
import {
  advanceAutonomousModelContinuationState,
  compileAutonomousModelContinuationPlan,
  completeAutonomousModelContinuationState,
  continuationSelectionDecision,
  createAutonomousModelContinuationState,
} from "./autonomous-continuation.js";
import type {
  AutonomousContinuationFailureScope,
  AutonomousModelContinuationPlan,
  AutonomousModelContinuationState,
} from "./autonomous-continuation.js";
import {
  compactAutonomousProviderRequest,
  type AutonomousContextBudgetOptions,
  type AutonomousContextBudgetPlan,
} from "./autonomous-context-budget.js";

/** Public schema for the cross-language, application-owned provider runtime. */
export const LLM_RUNTIME_SCHEMA = "bioprism-typescript-llm-runtime/0.1" as const;
export const PROVIDER_OBSERVATION_SCHEMA = "bioprism-typescript-llm-provider-observation/0.1" as const;
export const CREDENTIAL_ONBOARDING_SCHEMA = "bioprism-typescript-llm-credential-onboarding/0.1" as const;
export const PROVIDER_MODEL_DISCOVERY_SCHEMA = "bioprism-typescript-llm-provider-model-discovery/0.1" as const;
const LEGACY_LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-llm-runtime-health-snapshot/0.1" as const;
export const LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA = "bioprism-typescript-llm-runtime-health-snapshot/0.2" as const;
export const IN_MEMORY_PROVIDER_SCHEMA = "bioprism-typescript-llm-in-memory-provider/0.1" as const;

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
export const MAX_PROVIDER_CONTENT_PARTS = 64;
export const MAX_PROVIDER_CONTENT_PART_BYTES = 2_000_000;
export const MAX_CREDENTIAL_PROVISIONING_SOURCES = 128;
export const MAX_CREDENTIAL_PROVISIONING_PROVIDERS = 128;
export const MAX_CREDENTIAL_SOURCE_LABEL_BYTES = 256;
export const AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS = 1_000_000;
export const CREDENTIAL_PROVISIONING_SCHEMA = "bioprism-llm-credential-provisioning/0.1" as const;
export const MAX_LLM_RUNTIME_HEALTH_PROVIDERS = 128;
export const MAX_LLM_RUNTIME_HEALTH_MODELS = 2_048;
export const MAX_LLM_RUNTIME_HEALTH_SNAPSHOT_BYTES = 1_000_000;
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
  /** Explicit caller-owned local transport. When present, no network or credential is used. */
  transport?: InMemoryProviderTransport;
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
  readonly transport?: InMemoryProviderTransport;
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
  content: string | readonly ProviderContentPart[];
  name?: string;
  toolCallId?: string;
  toolCalls?: readonly ProviderToolCall[];
  isError?: boolean;
}

/**
 * Provider-neutral multimodal input. The runtime translates this small contract into each
 * provider's native image shape and refuses unsupported parts rather than silently dropping
 * evidence. URLs and base64 payloads are transient request material; they are never included in
 * health, learning, planning, or public response projections.
 */
export type ProviderContentPart = ProviderTextContentPart | ProviderImageUrlContentPart | ProviderImageBase64ContentPart;

export interface ProviderTextContentPart {
  type: "text";
  text: string;
}

export interface ProviderImageUrlContentPart {
  type: "image_url";
  url: string;
  detail?: "auto" | "low" | "high";
}

export interface ProviderImageBase64ContentPart {
  type: "image_base64";
  mediaType: "image/png" | "image/jpeg" | "image/webp" | "image/gif";
  data: string;
  detail?: "auto" | "low" | "high";
}

/** Build a provider-neutral text part without exposing provider wire details to callers. */
export function providerTextPart(text: string): ProviderTextContentPart {
  boundedText("provider text content part", text, MAX_PROVIDER_CONTENT_PART_BYTES);
  return { type: "text", text };
}

/** Build a provider-neutral HTTPS image reference for a transient model request. */
export function providerImageUrlPart(url: string, detail: ProviderImageUrlContentPart["detail"] = "auto"): ProviderImageUrlContentPart {
  const normalized = boundedText("provider image URL content part", url, 8_192);
  if (!/^https:\/\/[^\s\u0000-\u001f]+$/i.test(normalized)) throw new ProviderRuntimeError("provider image URL content part must be an HTTPS URL");
  return { type: "image_url", url: normalized, detail };
}

/** Build a bounded inline image part; callers own the transient bytes and lifecycle. */
export function providerImageBase64Part(
  data: string,
  mediaType: ProviderImageBase64ContentPart["mediaType"],
  detail: ProviderImageBase64ContentPart["detail"] = "auto",
): ProviderImageBase64ContentPart {
  const normalized = boundedText("provider image base64 content part", data, MAX_PROVIDER_CONTENT_PART_BYTES);
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(normalized) || normalized.length % 4 !== 0) {
    throw new ProviderRuntimeError("provider image base64 content part is malformed");
  }
  if (!("image/png" === mediaType || "image/jpeg" === mediaType || "image/webp" === mediaType || "image/gif" === mediaType)) {
    throw new ProviderRuntimeError("provider image media type is unsupported");
  }
  return { type: "image_base64", mediaType, data: normalized, detail };
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
  /** Present only on explicit local responses; identifies the bounded local transport posture. */
  schema?: typeof IN_MEMORY_PROVIDER_SCHEMA;
  transport?: "caller_owned";
}

/** Values accepted from an explicit local provider handler before normalization. */
export type InMemoryProviderResponse = ProviderResponse | JsonObject | string;

/** Provider-neutral local invocation callback used by deterministic and offline runtimes. */
export type InMemoryProviderHandler = (request: ProviderRequest) => InMemoryProviderResponse | Promise<InMemoryProviderResponse>;

/** Provider-neutral stream callback; events are validated before reaching callers. */
export type InMemoryProviderStreamHandler = (
  request: ProviderRequest,
) => AsyncIterable<ProviderStreamEvent> | Iterable<ProviderStreamEvent> | Promise<AsyncIterable<ProviderStreamEvent> | Iterable<ProviderStreamEvent>>;

/** Optional model inventory callback for a local provider. The payload follows `{ data: [...] }`. */
export type InMemoryProviderDiscoveryHandler = () => JsonObject | Promise<JsonObject>;

/** Explicit local transport implementation. It is never inferred for an HTTP provider. */
export interface InMemoryProviderTransport {
  invoke: InMemoryProviderHandler;
  stream?: InMemoryProviderStreamHandler;
  discoverModels?: InMemoryProviderDiscoveryHandler;
}

/** Configuration options for {@link LLMRuntime.registerInMemoryProvider}. */
export type InMemoryProviderOptions = Omit<ProviderConfig, "provider" | "baseUrl" | "protocol" | "requiresCredential" | "transport"> & {
  protocol?: ProviderProtocol;
  stream?: InMemoryProviderStreamHandler;
  discoverModels?: InMemoryProviderDiscoveryHandler;
};

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
  status: "completed" | "authorization_required" | "reconciliation_required" | "turn_limit_reached";
  responses: ProviderResponse[];
  finalResponse: ProviderResponse | null;
  turns: number;
  toolCalls: number;
}

function providerToolResultStatus(result: ProviderToolResult): "completed" | "authorization_required" | "reconciliation_required" {
  if (result.approved) return "completed";
  return isObject(result.content) && result.content.status === "reconciliation_required" ? "reconciliation_required" : "authorization_required";
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

/**
 * Stable, value-only evidence for one provider invocation made by the autonomous runtime.
 *
 * This intentionally mirrors the Python SDK receipt contract while keeping the schema
 * language-qualified. It records transport facts useful for replay, cost accounting, health
 * learning, and evaluator settlement; prompts, provider payloads, credentials, and response
 * text never cross this boundary.
 */
export const AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA = "bioprism-typescript-autonomous-provider-invocation/0.1" as const;
export const AUTONOMOUS_PROVIDER_FAILOVER_SCHEMA = "bioprism-typescript-autonomous-provider-failover/0.1" as const;

export interface AutonomousProviderInvocationReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA;
  execution_id: string | null;
  provider: string;
  model: string;
  kind: string;
  /** Zero-based autonomous selection attempt; a retry/failover increments this value. */
  attempt: number;
  /** Zero-based provider turn; tool loops can produce several turns per selection attempt. */
  turn: number;
  status: ProviderInvocationOutcome["status"];
  outcome: "success" | "failure";
  input_tokens: number;
  output_tokens: number;
  estimated_cost_units: number;
  actual_cost_units: number;
  latency_ms: number;
  selection_digest: string;
  outcome_digest: string;
  request_id_digest: string | null;
  failure_class: ProviderFailureClass | null;
  status_code: number | null;
  retention: "metadata_only_no_provider_payloads_or_credentials";
  secret_material: "never_returned";
}

export interface AutonomousProviderFailoverAttempt extends JsonObject {
  attempt: number;
  provider: string;
  model: string;
  status: ProviderInvocationOutcome["status"];
  outcome: "success" | "failure";
  reason: ProviderFailureClass | null;
  status_code: number | null;
  selection_digest: string;
  outcome_digest: string;
}

export interface AutonomousProviderFailoverProjection extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_FAILOVER_SCHEMA;
  strategy: "deterministic_model_selector_with_provider_health_gating";
  attempts: AutonomousProviderFailoverAttempt[];
  fallback_count: number;
  failover_digest: string;
  /** Digest of the immutable fallback ladder used by this invocation. */
  continuation_plan_digest?: string;
  /** Bounded ladder metadata; never includes task, prompt, credentials, or response values. */
  continuation_plan?: AutonomousModelContinuationPlan;
  retention: "metadata_only";
  secret_material: "never_returned";
}

interface AutonomousProviderInvocationSample {
  executionId: string | null;
  metadata: ProviderInvocationMetadata;
  outcome: ProviderInvocationOutcome;
  attempt: number;
  turn: number;
  selectionDigest: string;
  estimatedCostUnits: number;
  costPerMillionTokens: number;
}

function boundedReceiptInteger(value: number): number {
  return Number.isSafeInteger(value) && value >= 0 ? value : 0;
}

function boundedReceiptMetric(value: number): number {
  return Number.isFinite(value) && value >= 0 ? value : 0;
}

async function autonomousProviderInvocationProjection(
  samples: readonly AutonomousProviderInvocationSample[],
  continuationPlan?: AutonomousModelContinuationPlan,
): Promise<{ providerInvocations: AutonomousProviderInvocationReceipt[]; providerFailover: AutonomousProviderFailoverProjection | null }> {
  const providerInvocations: AutonomousProviderInvocationReceipt[] = [];
  for (const sample of samples) {
    const inputTokens = boundedReceiptInteger(sample.outcome.inputTokens);
    const outputTokens = boundedReceiptInteger(sample.outcome.outputTokens);
    const latencyMs = boundedReceiptMetric(sample.outcome.latencyMs);
    const estimatedCostUnits = boundedReceiptMetric(sample.estimatedCostUnits);
    const costPerMillionTokens = boundedReceiptMetric(sample.costPerMillionTokens);
    const actualCostUnits = boundedReceiptMetric(((inputTokens + outputTokens) / 1_000_000) * costPerMillionTokens);
    const outcomeDigest = await digestJson({
      provider: sample.metadata.provider,
      model: sample.metadata.model,
      kind: sample.metadata.kind,
      status: sample.outcome.status,
      success: sample.outcome.success,
      latency_ms: latencyMs,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      status_code: sample.outcome.statusCode ?? null,
      failure_class: sample.outcome.failureClass ?? null,
      failure_code: sample.outcome.failureCode ?? null,
      retryable: sample.outcome.retryable ?? false,
      request_id_present: typeof sample.outcome.requestId === "string" && sample.outcome.requestId.length > 0,
    });
    providerInvocations.push({
      schema: AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA,
      execution_id: sample.executionId,
      provider: sample.metadata.provider,
      model: sample.metadata.model,
      kind: sample.metadata.kind,
      attempt: boundedReceiptInteger(sample.attempt),
      turn: boundedReceiptInteger(sample.turn),
      status: sample.outcome.status,
      outcome: sample.outcome.success ? "success" : "failure",
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      estimated_cost_units: estimatedCostUnits,
      actual_cost_units: actualCostUnits,
      latency_ms: latencyMs,
      selection_digest: sample.selectionDigest,
      outcome_digest: outcomeDigest,
      request_id_digest: typeof sample.outcome.requestId === "string" && sample.outcome.requestId.length > 0 ? await digestJson(sample.outcome.requestId) : null,
      failure_class: sample.outcome.failureClass ?? null,
      status_code: sample.outcome.statusCode ?? null,
      retention: "metadata_only_no_provider_payloads_or_credentials",
      secret_material: "never_returned",
    });
  }
  const fallbackCount = providerInvocations.length === 0 ? 0 : Math.max(...providerInvocations.map((receipt) => receipt.attempt));
  if (fallbackCount === 0) return { providerInvocations, providerFailover: null };
  const attempts = providerInvocations.map((receipt): AutonomousProviderFailoverAttempt => ({
    attempt: receipt.attempt,
    provider: receipt.provider,
    model: receipt.model,
    status: receipt.status,
    outcome: receipt.outcome,
    reason: receipt.failure_class,
    status_code: receipt.status_code,
    selection_digest: receipt.selection_digest,
    outcome_digest: receipt.outcome_digest,
  }));
  return {
    providerInvocations,
    providerFailover: {
      schema: AUTONOMOUS_PROVIDER_FAILOVER_SCHEMA,
      strategy: "deterministic_model_selector_with_provider_health_gating",
      attempts,
      fallback_count: fallbackCount,
      failover_digest: await digestJson({
        strategy: "deterministic_model_selector_with_provider_health_gating",
        attempts,
        fallback_count: fallbackCount,
        continuation_plan_digest: continuationPlan?.plan_digest ?? null,
      }),
      ...(continuationPlan ? { continuation_plan_digest: continuationPlan.plan_digest, continuation_plan: continuationPlan } : {}),
      retention: "metadata_only",
      secret_material: "never_returned",
    },
  };
}

/** A synchronous reservation released only when a provider call fails before dispatch. */
export type AutonomousCostReservation = () => void;

/** Internal/provider-boundary hook used to compose one budget across nested autonomous calls. */
export type AutonomousCostReservationCallback = (costUnits: number) => AutonomousCostReservation | void;

export interface AutonomousCostBudgetSnapshot extends JsonObject {
  max_cost_units: number;
  consumed_cost_units: number;
  remaining_cost_units: number;
}

/** Estimate the cost of one provider request from caller-owned candidate metadata. */
export type AutonomousProviderCostEstimator = (request: ProviderRequest) => number;

export interface ProviderInvocationOptions {
  credential?: CredentialHandle;
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  /** Optional metadata-only crash-safe boundary for the actual provider dispatch. */
  effectBoundary?: AutonomousEffectBoundary;
  invocationKind?: string;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  executionTurn?: number;
  executionFailover?: boolean;
  selectionDigest?: string | null;
  estimatedCostUnits?: number;
  reserveCost?: AutonomousCostReservationCallback;
  /** Optional process-local provider/model quota; the runtime quota is used by default. */
  providerQuota?: ProviderQuotaController;
  /** Optional explicit history compaction applied before each tool-loop turn. */
  contextBudget?: AutonomousContextBudgetOptions;
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

  /** Rehydrate a caller-owned budget without permitting its consumed accounting to reset. */
  static fromSnapshot(snapshot: AutonomousCostBudgetSnapshot): AutonomousCostBudget {
    if (!snapshot || typeof snapshot !== "object" || !Number.isFinite(snapshot.max_cost_units) || !Number.isFinite(snapshot.consumed_cost_units) || !Number.isFinite(snapshot.remaining_cost_units) || snapshot.max_cost_units < 0 || snapshot.consumed_cost_units < 0 || snapshot.consumed_cost_units > snapshot.max_cost_units || snapshot.remaining_cost_units !== Math.max(0, snapshot.max_cost_units - snapshot.consumed_cost_units)) {
      throw new ArgumentError("cost budget snapshot is malformed");
    }
    const budget = new AutonomousCostBudget(snapshot.max_cost_units);
    budget.consumedCostUnitsValue = snapshot.consumed_cost_units;
    return budget;
  }

  get consumedCostUnits(): number {
    return this.consumedCostUnitsValue;
  }

  get remainingCostUnits(): number {
    return Math.max(0, this.maxCostUnits - this.consumedCostUnitsValue);
  }

  snapshot(): AutonomousCostBudgetSnapshot {
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

/**
 * Explicit multi-objective policy for model selection.
 *
 * The values are non-negative utility weights, not probabilities.  They intentionally mirror
 * the Rust brain kernel's `SelectionWeights` contract so a selection preview, an autonomous
 * provider call, and an offline replay can make the same decision from the same metadata.  A
 * policy with all weights set to zero is refused because it would make the decision entirely
 * dependent on tie-breaking.
 */
export interface AutonomousSelectionWeights extends JsonObject {
  quality: number;
  reliability: number;
  cost: number;
  latency: number;
  exploration: number;
}

export const AUTONOMOUS_SELECTION_WEIGHTS_SCHEMA = "bioprism-autonomous-selection-weights/0.1" as const;

/** Defaults shared with the executable Rust selection kernel. */
export const DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS: Readonly<AutonomousSelectionWeights> = Object.freeze({
  quality: 0.55,
  reliability: 0.25,
  cost: 0.10,
  latency: 0.10,
  exploration: 0.15,
});

const AUTONOMOUS_SELECTION_WEIGHT_NAMES = ["quality", "reliability", "cost", "latency", "exploration"] as const;

/** Validate and fill a partial policy without mutating caller-owned state. */
export function normalizeAutonomousSelectionWeights(value: unknown = undefined): AutonomousSelectionWeights {
  if (value === undefined || value === null) return { ...DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS };
  if (!isObject(value)) throw new ProviderRuntimeError("autonomous selection weights must be an object");
  const unsupported = Object.keys(value).filter((key) => !(AUTONOMOUS_SELECTION_WEIGHT_NAMES as readonly string[]).includes(key));
  if (unsupported.length) throw new ProviderRuntimeError(`autonomous selection weights contain unsupported fields: ${unsupported.sort().join(", ")}`);
  const normalized = { ...DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS } as AutonomousSelectionWeights;
  for (const name of AUTONOMOUS_SELECTION_WEIGHT_NAMES) {
    const supplied = value[name];
    if (supplied === undefined) continue;
    if (typeof supplied !== "number" || !Number.isFinite(supplied) || supplied < 0 || supplied > 100) {
      throw new ProviderRuntimeError(`autonomous selection weight ${name} is outside [0, 100]`);
    }
    normalized[name] = Number(supplied.toFixed(12));
  }
  if (AUTONOMOUS_SELECTION_WEIGHT_NAMES.every((name) => normalized[name] === 0)) {
    throw new ProviderRuntimeError("autonomous selection weights must contain at least one positive value");
  }
  return normalized;
}

/** One caller-owned value-only global observation for an online selection arm. */
export interface AutonomousModelObservation extends JsonObject {
  arm_id: string;
  pulls: number;
  reward_sum: number;
  failures: number;
  disabled?: boolean;
}

/** Validate and canonicalize observations before they influence ranking. */
export function normalizeAutonomousModelObservations(value: unknown = undefined): AutonomousModelObservation[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value) || value.length > MAX_PROVIDER_MODELS) throw new ProviderRuntimeError("autonomous model observations are outside their bounds");
  const seen = new Set<string>();
  return value.map((raw, index) => {
    if (!isObject(raw)) throw new ProviderRuntimeError(`autonomous model observation ${index} must be an object`);
    const armId = boundedText(`autonomous model observation ${index} arm_id`, raw.arm_id, 768);
    if (seen.has(armId)) throw new ProviderRuntimeError(`autonomous model observations contain duplicate arm ${armId}`);
    seen.add(armId);
    const pulls = raw.pulls;
    const rewardSum = raw.reward_sum;
    const failures = raw.failures;
    if (!Number.isSafeInteger(pulls) || (pulls as number) < 0 || (pulls as number) > 1_000_000_000) throw new ProviderRuntimeError(`autonomous model observation ${armId} pulls are outside their bounds`);
    if (typeof rewardSum !== "number" || !Number.isFinite(rewardSum) || rewardSum < -1e12 || rewardSum > 1e12) throw new ProviderRuntimeError(`autonomous model observation ${armId} reward_sum is outside its bounds`);
    if (!Number.isSafeInteger(failures) || (failures as number) < 0 || (failures as number) > (pulls as number)) throw new ProviderRuntimeError(`autonomous model observation ${armId} failures are outside their bounds`);
    if (raw.disabled !== undefined && typeof raw.disabled !== "boolean") throw new ProviderRuntimeError(`autonomous model observation ${armId} disabled must be boolean`);
    return { arm_id: armId, pulls: pulls as number, reward_sum: Number(rewardSum.toFixed(12)), failures: failures as number, ...(raw.disabled === undefined ? {} : { disabled: raw.disabled as boolean }) };
  });
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
  /** Optional normalized rank-separation floor; ambiguous selections abstain. */
  min_selection_confidence?: number | null;
  /** Whether the provider response must be valid JSON at the transport boundary. */
  require_json?: boolean;
  /** Explicit multi-objective utility policy; defaults to the Rust kernel's policy. */
  weights?: AutonomousSelectionWeights;
  /** Optional global online-learning observations used by the deterministic ranker. */
  observations?: AutonomousModelObservation[];
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
  /** Utility before exploration; retained for decision audits and replay diagnostics. */
  base_score?: number;
  /** Exploration contribution for this model arm. */
  exploration_bonus?: number;
  /** Number of caller-supplied observations used for the arm. */
  observed_pulls?: number;
}

export interface AutonomousSelectionDecision extends JsonObject {
  selected_model: { provider: string; model: string } | null;
  strategy: "deterministic_health_utility" | "caller_selector";
  ranking: AutonomousModelRanking[];
  abstention_reason: string | null;
  /** Normalized separation of the top eligible candidates; never answer correctness. */
  selection_confidence?: number;
  /** Caller-supplied confidence floor retained for auditability. */
  min_selection_confidence?: number | null;
  exploration_draw?: number | null;
  exploration_taken?: boolean;
}

/** Metadata-only lifecycle emitted before and after each autonomous model-selection attempt. */
export interface AutonomousModelSelectionTraceEvent extends JsonObject {
  phase: "model_selection_started" | "model_selection_finished";
  status: "running" | "selected" | "abstained" | "failed";
  attempt: number | null;
  failover: boolean;
  candidate_count: number;
  eligible_candidate_count: number;
  strategy: AutonomousSelectionDecision["strategy"] | null;
  selected_provider: string | null;
  selected_model: string | null;
  selection_digest: string | null;
  detail_digest: string | null;
  failure_code: string | null;
}

export type AutonomousModelSelectionTraceEventCallback = (event: AutonomousModelSelectionTraceEvent) => unknown | Promise<unknown>;

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
  minSelectionConfidence?: number;
  selectionWeights?: Partial<AutonomousSelectionWeights>;
  selectionObservations?: readonly AutonomousModelObservation[];
  /** Optional explicit lossy history budget; omitted requests retain legacy behavior. */
  contextBudget?: AutonomousContextBudgetOptions;
  candidates: readonly AutonomousModelCandidate[];
  request: ProviderRequest;
}

export interface AutonomousExecutionResult {
  selection: AutonomousSelectionDecision;
  response: ProviderResponse;
  /** Exact bounded fallback ladder compiled from the first selection. */
  continuation_plan: AutonomousModelContinuationPlan;
  /** Metadata-only receipt for every provider turn performed by this autonomous invocation. */
  provider_invocations: AutonomousProviderInvocationReceipt[];
  /** Present only when bounded provider/model failover was actually used. */
  provider_failover: AutonomousProviderFailoverProjection | null;
  /** Metadata-only record of any deterministic prompt-history compaction. */
  context_budget?: AutonomousContextBudgetPlan | null;
}

/** Public schema for the live autonomous provider-neutral stream envelope. */
export const AUTONOMOUS_STREAM_COMPLETION_SCHEMA = "bioprism-typescript-autonomous-stream-completion/0.1" as const;

/**
 * Metadata-only completion state for an autonomous stream.
 *
 * Stream deltas are deliberately absent. Consumers can render the transient events, while this
 * value-only receipt can be persisted, replayed, and sent to an evaluator without retaining task
 * text, provider payloads, credentials, or model output.
 */
export interface AutonomousStreamCompletion extends JsonObject {
  schema: typeof AUTONOMOUS_STREAM_COMPLETION_SCHEMA;
  status: "completed" | "failed" | "abandoned";
  event_count: number;
  text_delta_bytes: number;
  done_seen: boolean;
  provider_invocations: AutonomousProviderInvocationReceipt[];
  provider_failover: AutonomousProviderFailoverProjection | null;
  error_code: ProviderErrorCode | null;
  error_class: string | null;
  retention: "metadata_only_no_stream_payloads_or_credentials";
  secret_material: "never_returned";
}

/** Handle returned after autonomous selection, before the transient stream is consumed. */
export interface AutonomousStreamHandle {
  selection: AutonomousSelectionDecision;
  continuation_plan: AutonomousModelContinuationPlan;
  context_budget: AutonomousContextBudgetPlan | null;
  events: AsyncIterable<ProviderStreamEvent>;
  /** Resolves after normal exhaustion, terminal failure, or consumer cancellation. */
  completion: Promise<AutonomousStreamCompletion>;
}

export interface AutonomousStreamInvocationOptions {
  credential?: CredentialHandle;
  credentialFor?: (provider: string) => CredentialHandle | undefined;
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  feedback?: (decision: AutonomousSelectionDecision, outcome: ProviderInvocationOutcome) => void | Promise<void>;
  selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  maxProviderFailovers?: number;
  reserveCost?: AutonomousCostReservationCallback;
  effectBoundary?: AutonomousEffectBoundary;
}

export type AutonomousModelSelector = (request: AutonomousSelectionRequest) => AutonomousSelectionDecision | Promise<AutonomousSelectionDecision>;

export interface ProviderHealth extends JsonObject {
  provider: string;
  circuit: "closed" | "open";
  /** Optional registration projection used by cross-runtime selection adapters. */
  registered?: boolean;
  /** Optional aggregate eligibility projection; false can only narrow a live decision. */
  eligible?: boolean;
  consecutive_failures: number;
  attempts: number;
  successes: number;
  failures: number;
  success_rate: number;
  mean_latency_ms: number | null;
  last_latency_ms: number | null;
  last_model: string | null;
  last_status_code: number | null;
  credential_posture: "caller_supplied_opaque_handle" | "caller_supplied_in_memory_handle";
  credential_required: boolean;
  /** Provider transport capability used as a hard gate for explicit structured output. */
  structured_output_mode?: "disabled" | "json_object" | "json_schema";
  /** Optional persisted evaluator-quality projection supplied by a caller-owned health ledger. */
  quality_mean?: number | null;
  quality_observations?: number;
}

/** Redacted transport counters and circuit state for one configured provider. */
export interface LLMRuntimeProviderHealthSnapshot extends JsonObject {
  provider: string;
  attempts: number;
  successes: number;
  failures: number;
  total_latency_ms: number;
  last_latency_ms: number | null;
  last_model: string | null;
  last_status_code: number | null;
  consecutive_failures: number;
  circuit_opened_until: number | null;
}

/** Redacted transport counters for one provider/model arm. */
export interface LLMRuntimeModelHealthSnapshot extends JsonObject {
  provider: string;
  model: string;
  attempts: number;
  successes: number;
  failures: number;
  total_latency_ms: number;
  last_latency_ms: number | null;
  last_model: string | null;
  last_status_code: number | null;
}

/** Restart-safe provider transport health; credentials and task payloads are never retained. */
export interface LLMRuntimeHealthSnapshot extends JsonObject {
  /** 0.1 remains readable; current snapshots carry an independent image lineage in 0.2. */
  schema: typeof LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA | typeof LEGACY_LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA;
  snapshot_generation?: number;
  previous_snapshot_digest?: string | null;
  providers: LLMRuntimeProviderHealthSnapshot[];
  models: LLMRuntimeModelHealthSnapshot[];
  snapshot_digest: string;
  retention: "transport_health_metadata_only_hash_bound";
  secret_material: "never_returned";
}

export interface LLMRuntimeHealthPersistence {
  read(): Promise<LLMRuntimeHealthSnapshot | null> | LLMRuntimeHealthSnapshot | null;
  write(snapshot: LLMRuntimeHealthSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: LLMRuntimeHealthSnapshot): Promise<boolean> | boolean;
}

export interface LLMRuntimeHealthSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface LLMRuntimeTransactionalHealthSnapshotTextStore extends LLMRuntimeHealthSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
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

function safeJsonValue(value: unknown, label: string, maximum: number): JsonValue {
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
  validateJsonValue(parsed);
  return parsed as JsonValue;
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
  if (config.transport !== undefined && (!config.transport || typeof config.transport.invoke !== "function")) {
    throw new ProviderRuntimeError("provider local transport must expose a callable invoke handler");
  }
  if (config.transport !== undefined && config.requiresCredential === true) {
    throw new ProviderRuntimeError("an in-memory provider transport cannot require a credential");
  }
  const requiresCredential = config.transport !== undefined ? false : config.requiresCredential ?? true;
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
    ...(config.transport ? { transport: config.transport } : {}),
  };
}

function requestMetadata(provider: string, request: ProviderRequest, kind: string): ProviderInvocationMetadata {
  boundedIdentifier("invocation kind", kind, 128);
  const inputTokens = Math.max(1, Math.ceil(request.messages.reduce((sum, message) => sum + providerContentBytes(message.content, message.role), 0) / 4));
  return {
    provider,
    model: request.model,
    kind,
    inputTokens,
    requestedOutputTokens: request.maxOutputTokens,
    toolCount: request.tools?.length ?? 0,
  };
}

async function providerEffectProjection(response: ProviderResponse): Promise<JsonObject> {
  if (!response || typeof response !== "object") throw new ProviderRuntimeError("provider effect returned a malformed response");
  const usage = response.usage ?? {};
  return {
    provider: response.provider,
    model: response.model,
    status_code: response.statusCode,
    input_tokens: usage.input_tokens ?? 0,
    output_tokens: usage.output_tokens ?? 0,
    tool_call_count: response.toolCalls.length,
    structured_output_present: response.structured !== null,
    request_id_digest: response.requestId ? await digestJson(response.requestId) : null,
  };
}

function providerEffectFailureIsDefinite(error: unknown): boolean {
  if (!(error instanceof ProviderRuntimeError)) return false;
  if (error.circuitOpen) return true;
  const status = error.statusCode;
  return typeof status === "number" && status >= 400 && status < 500 && ![408, 409, 425, 429].includes(status);
}

function generatedProviderIdempotencyKey(prefix: string): string {
  const cryptoObject = globalThis.crypto as { randomUUID?: () => string } | undefined;
  const uuid = typeof cryptoObject?.randomUUID === "function" ? cryptoObject.randomUUID() : `${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`;
  return `${prefix}-${uuid}`;
}

function normalizedContentPart(value: unknown): ProviderContentPart {
  if (!isObject(value)) throw new ProviderRuntimeError("provider content part must be an object");
  const type = value.type;
  const detail = value.detail;
  if (detail !== undefined && detail !== "auto" && detail !== "low" && detail !== "high") throw new ProviderRuntimeError("provider image detail is invalid");
  if (type === "text") {
    const text = boundedText("provider text content part", value.text, MAX_PROVIDER_CONTENT_PART_BYTES);
    if (Object.keys(value).some((key) => !["type", "text"].includes(key))) throw new ProviderRuntimeError("provider text content part contains unsupported fields");
    return { type, text };
  }
  if (type === "image_url") {
    const url = boundedText("provider image URL content part", value.url, 8_192);
    if (!/^https:\/\/[^\s\u0000-\u001f]+$/i.test(url)) throw new ProviderRuntimeError("provider image URL content part must be an HTTPS URL");
    if (Object.keys(value).some((key) => !["type", "url", "detail"].includes(key))) throw new ProviderRuntimeError("provider image URL content part contains unsupported fields");
    return { type, url, ...(detail === undefined ? {} : { detail }) };
  }
  if (type === "image_base64") {
    const data = boundedText("provider image base64 content part", value.data, MAX_PROVIDER_CONTENT_PART_BYTES);
    if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(data) || data.length % 4 !== 0) throw new ProviderRuntimeError("provider image base64 content part is malformed");
    const mediaType = value.mediaType;
    if (mediaType !== "image/png" && mediaType !== "image/jpeg" && mediaType !== "image/webp" && mediaType !== "image/gif") throw new ProviderRuntimeError("provider image media type is unsupported");
    if (Object.keys(value).some((key) => !["type", "mediaType", "data", "detail"].includes(key))) throw new ProviderRuntimeError("provider image base64 content part contains unsupported fields");
    return { type, mediaType, data, ...(detail === undefined ? {} : { detail }) };
  }
  throw new ProviderRuntimeError("provider content part type is unsupported");
}

function normalizedProviderContent(value: unknown, role: ProviderMessage["role"]): string | ProviderContentPart[] {
  if (typeof value === "string") {
    if (bytes(value) > MAX_PROVIDER_MESSAGE_BYTES) throw new ProviderRuntimeError("provider message content is outside its bounded text contract");
    return value;
  }
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_PROVIDER_CONTENT_PARTS) throw new ProviderRuntimeError("provider message content parts are outside their bounds");
  const parts = value.map(normalizedContentPart);
  if (role === "tool") throw new ProviderRuntimeError("provider tool message content must remain text");
  if ((role === "system" || role === "developer") && parts.some((part) => part.type !== "text")) throw new ProviderRuntimeError("provider system and developer messages support text content only");
  if (bytes(jsonText(parts)) > MAX_PROVIDER_MESSAGE_BYTES) throw new ProviderRuntimeError("provider message content parts exceed their bounded size");
  return parts;
}

/** Normalize transient user evidence before it is attached to an autonomous provider request. */
export function normalizeProviderContentParts(value: unknown): ProviderContentPart[] {
  const normalized = normalizedProviderContent(value, "user");
  if (typeof normalized === "string") throw new ProviderRuntimeError("provider content parts must be an array");
  return normalized;
}

function providerContentBytes(value: ProviderMessage["content"], role: ProviderMessage["role"]): number {
  const normalized = normalizedProviderContent(value, role);
  return bytes(typeof normalized === "string" ? normalized : jsonText(normalized));
}

function imageDataUrl(part: ProviderImageBase64ContentPart): string {
  return `data:${part.mediaType};base64,${part.data}`;
}

function wireContent(protocol: ProviderProtocol, value: ProviderMessage["content"], role: ProviderMessage["role"]): JsonValue {
  const normalized = normalizedProviderContent(value, role);
  if (typeof normalized === "string") return normalized;
  if (protocol === "openai_responses") {
    return normalized.map((part) => part.type === "text"
      ? { type: "input_text", text: part.text }
      : { type: "input_image", image_url: part.type === "image_url" ? part.url : imageDataUrl(part), ...(part.detail === undefined ? {} : { detail: part.detail }) });
  }
  if (protocol === "anthropic_messages") {
    return normalized.map((part) => part.type === "text"
      ? { type: "text", text: part.text }
      : part.type === "image_url"
        ? { type: "image", source: { type: "url", url: part.url } }
        : { type: "image", source: { type: "base64", media_type: part.mediaType, data: part.data } });
  }
  return normalized.map((part) => part.type === "text"
    ? { type: "text", text: part.text }
    : { type: "image_url", image_url: { url: part.type === "image_url" ? part.url : imageDataUrl(part), ...(part.detail === undefined ? {} : { detail: part.detail }) } });
}

function textContentForSystem(value: ProviderMessage["content"], role: ProviderMessage["role"]): string {
  const normalized = normalizedProviderContent(value, role);
  if (typeof normalized === "string") return normalized;
  return normalized.filter((part): part is ProviderTextContentPart => part.type === "text").map((part) => part.text).join("\n");
}

function validateRequest(request: ProviderRequest): void {
  if (!isObject(request)) throw new ProviderRuntimeError("provider request must be an object");
  boundedIdentifier("model", request.model, 512);
  if (!Array.isArray(request.messages) || request.messages.length === 0 || request.messages.length > 1024) throw new ProviderRuntimeError("provider request messages are outside their bounds");
  for (const message of request.messages) {
    const role = isObject(message) && typeof message.role === "string" ? message.role : "";
    if (!isObject(message) || !["system", "developer", "user", "assistant", "tool"].includes(role)) throw new ProviderRuntimeError("provider request contains an invalid message");
    normalizedProviderContent(message.content, role as ProviderMessage["role"]);
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
        output.push({ type: "function_call_output", call_id: message.toolCallId ?? "unknown", output: textContentForSystem(message.content, message.role) });
      } else if (message.role === "assistant" && message.toolCalls?.length) {
        const assistantContent = wireContent(protocol, message.content, message.role);
        if (Array.isArray(assistantContent)) output.push({ role: "assistant", content: assistantContent });
        else if (assistantContent) output.push({ role: "assistant", content: assistantContent });
        for (const call of message.toolCalls) output.push({ type: "function_call", call_id: call.id, name: call.name, arguments: JSON.stringify(call.arguments) });
      } else {
        output.push({ role: message.role, content: wireContent(protocol, message.content, message.role) });
      }
    } else if (protocol === "anthropic_messages") {
      if (message.role === "system" || message.role === "developer") continue;
      if (message.role === "tool") {
        output.push({ role: "user", content: [{ type: "tool_result", tool_use_id: message.toolCallId ?? "unknown", content: textContentForSystem(message.content, message.role), is_error: message.isError ?? false }] });
      } else if (message.role === "assistant" && message.toolCalls?.length) {
        const content: JsonValue[] = [];
        const assistantContent = wireContent(protocol, message.content, message.role);
        if (Array.isArray(assistantContent)) content.push(...assistantContent);
        else if (assistantContent) content.push({ type: "text", text: assistantContent });
        for (const call of message.toolCalls) content.push({ type: "tool_use", id: call.id, name: call.name, input: call.arguments });
        output.push({ role: "assistant", content });
      } else {
        output.push({ role: message.role === "assistant" ? "assistant" : "user", content: wireContent(protocol, message.content, message.role) });
      }
    } else {
      const row: Record<string, JsonValue> = { role: message.role, content: wireContent(protocol, message.content, message.role) };
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
    const system = request.messages.filter((message) => message.role === "system" || message.role === "developer").map((message) => textContentForSystem(message.content, message.role)).join("\n\n");
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

function normalizeLocalUsage(value: unknown): ProviderUsage {
  if (value === undefined) return {};
  const raw = safeJson(value, "in-memory provider usage", 256_000);
  const usage: ProviderUsage = {};
  const inputTokens = asNonNegativeInteger(raw.input_tokens ?? raw.prompt_tokens);
  const outputTokens = asNonNegativeInteger(raw.output_tokens ?? raw.completion_tokens);
  const totalTokens = asNonNegativeInteger(raw.total_tokens);
  if (inputTokens !== null) usage.input_tokens = inputTokens;
  if (outputTokens !== null) usage.output_tokens = outputTokens;
  if (totalTokens !== null) usage.total_tokens = totalTokens;
  return usage;
}

function normalizeLocalToolCall(value: unknown, request: ProviderRequest): ProviderToolCall {
  if (!isObject(value)) throw new ProviderRuntimeError("in-memory provider tool_calls contain an invalid value", { code: "invalid_response" });
  const id = boundedIdentifier("in-memory provider tool call id", value.id ?? value.call_id, 256);
  const name = boundedIdentifier("in-memory provider tool name", value.name, 256);
  if (!(request.tools ?? []).some((tool) => tool.name === name)) throw new ProviderRuntimeError("in-memory provider returned an unrequested tool call", { code: "invalid_response" });
  return { id, name, arguments: parseArguments(value.arguments ?? {}) };
}

/** Project a caller-owned local result into the same safe response boundary as HTTP. */
function normalizeLocalResponse(config: NormalizedProviderConfig, value: InMemoryProviderResponse, request: ProviderRequest): ProviderResponse {
  let source: Record<string, unknown>;
  if (typeof value === "string") {
    source = { text: value };
  } else if (isObject(value)) {
    source = { ...value };
  } else {
    throw new ProviderRuntimeError("in-memory provider handler returned an unsupported response", { code: "invalid_response" });
  }
  const envelopeKeys = [
    "provider", "model", "text", "output_text", "statusCode", "status_code", "requestId", "request_id",
    "usage", "structured", "toolCalls", "tool_calls", "stopReason", "stop_reason",
  ];
  if (!envelopeKeys.some((key) => Object.prototype.hasOwnProperty.call(source, key))) {
    const structured = safeJson(source, "in-memory provider response", config.maxResponseBytes);
    source = { text: JSON.stringify(structured), structured };
  }
  const provider = source.provider === undefined ? config.provider : boundedIdentifier("in-memory provider response provider", source.provider, 128);
  const model = source.model === undefined ? request.model : boundedIdentifier("in-memory provider response model", source.model, 512);
  if (provider !== config.provider || model !== request.model) throw new ProviderRuntimeError("in-memory provider response identity does not match request", { code: "invalid_response" });
  const statusCodeValue = source.statusCode ?? source.status_code ?? 200;
  if (typeof statusCodeValue !== "number" || !Number.isSafeInteger(statusCodeValue) || statusCodeValue < 200 || statusCodeValue >= 300) throw new ProviderRuntimeError("in-memory provider response status is not successful", { code: "invalid_response" });
  const statusCode = statusCodeValue;
  const text = source.text ?? source.output_text ?? "";
  if (typeof text !== "string" || bytes(text) > MAX_PROVIDER_MESSAGE_BYTES) throw new ProviderRuntimeError("in-memory provider response text is outside its bound", { code: "invalid_response" });
  const requestIdValue = source.requestId ?? source.request_id ?? null;
  const requestId = requestIdValue === null ? null : boundedIdentifier("in-memory provider request id", requestIdValue, 512);
  const rawCalls = source.toolCalls ?? source.tool_calls ?? [];
  if (!Array.isArray(rawCalls) || rawCalls.length > MAX_PROVIDER_TOOLS) throw new ProviderRuntimeError("in-memory provider tool_calls are outside their bounds", { code: "invalid_response" });
  const toolCalls = rawCalls.map((call) => normalizeLocalToolCall(call, request));
  const usage = normalizeLocalUsage(source.usage);
  let structured: JsonValue | null = null;
  if (!toolCalls.length && Object.prototype.hasOwnProperty.call(source, "structured") && source.structured !== undefined) {
    structured = safeJsonValue(source.structured, "in-memory provider structured response", config.maxResponseBytes);
  }
  if (!toolCalls.length && request.requireJson) {
    const candidate = structured ?? (() => {
      try { return JSON.parse(text) as JsonValue; } catch { throw new ProviderRuntimeError("in-memory provider returned invalid JSON for the requested structured response", { code: "invalid_response" }); }
    })();
    validateStructuredResponseOrThrow(candidate, request.responseSchema);
    structured = candidate;
  }
  const stopReasonValue = source.stopReason ?? source.stop_reason ?? null;
  const stopReason = stopReasonValue === null ? null : boundedIdentifier("in-memory provider stop reason", stopReasonValue, 256);
  return { provider: config.provider, model: request.model, text, statusCode, requestId, usage, structured, toolCalls, stopReason, schema: IN_MEMORY_PROVIDER_SCHEMA, transport: "caller_owned" };
}

function normalizeLocalStreamEvent(config: NormalizedProviderConfig, request: ProviderRequest, value: unknown): ProviderStreamEvent {
  if (!isObject(value)) throw new ProviderRuntimeError("in-memory provider stream returned a malformed event", { code: "invalid_response" });
  const provider = boundedIdentifier("in-memory stream provider", value.provider, 128);
  const model = boundedIdentifier("in-memory stream model", value.model, 512);
  if (provider !== config.provider || model !== request.model) throw new ProviderRuntimeError("in-memory provider stream event identity does not match request", { code: "invalid_response" });
  const sequenceValue = value.sequence;
  if (typeof sequenceValue !== "number" || !Number.isSafeInteger(sequenceValue) || sequenceValue < 0 || sequenceValue >= MAX_PROVIDER_STREAM_EVENTS) throw new ProviderRuntimeError("in-memory provider stream sequence is outside its bound", { code: "invalid_response" });
  const sequence = sequenceValue;
  const eventType = boundedIdentifier("in-memory provider stream event type", value.eventType ?? value.event_type, 128);
  const textDelta = value.textDelta ?? value.text_delta ?? "";
  if (typeof textDelta !== "string" || bytes(textDelta) > MAX_PROVIDER_STREAM_TEXT_BYTES) throw new ProviderRuntimeError("in-memory provider stream text is outside its bound", { code: "invalid_response" });
  const requestIdValue = value.requestId ?? value.request_id ?? null;
  const requestId = requestIdValue === null ? null : boundedIdentifier("in-memory stream request id", requestIdValue, 512);
  if (typeof value.done !== "boolean") throw new ProviderRuntimeError("in-memory provider stream done flag is invalid", { code: "invalid_response" });
  const toolCallValue = value.toolCall ?? value.tool_call;
  const toolCall = toolCallValue === undefined || toolCallValue === null ? undefined : normalizeLocalToolCall(toolCallValue, request);
  return { provider, model, sequence, eventType, textDelta, requestId, usage: normalizeLocalUsage(value.usage), done: value.done, ...(toolCall ? { toolCall } : {}) };
}

function inMemoryFailure(error: unknown, operation: string): ProviderRuntimeError {
  const options = error instanceof ProviderRuntimeError
    ? {
        retryable: error.retryable,
        statusCode: error.statusCode,
        circuitOpen: error.circuitOpen,
        code: error.code,
        retryAfterMs: error.retryAfterMs,
      }
    : { code: "provider_error" as const };
  return new ProviderRuntimeError(`in-memory provider ${operation} failed`, options);
}

function isProviderResponseValue(value: Response | ProviderResponse): value is ProviderResponse {
  return isObject(value)
    && typeof value.provider === "string"
    && typeof value.model === "string"
    && typeof value.text === "string"
    && typeof value.statusCode === "number"
    && Array.isArray(value.toolCalls)
    && isObject(value.usage);
}

async function invokeLocalTransport(config: NormalizedProviderConfig, request: ProviderRequest): Promise<ProviderResponse> {
  const transport = config.transport;
  if (!transport) throw new ProviderRuntimeError("provider local transport is not configured");
  let value: InMemoryProviderResponse;
  try {
    value = await transport.invoke(request);
  } catch (error) {
    throw inMemoryFailure(error, "handler");
  }
  return normalizeLocalResponse(config, value, request);
}

async function* streamLocalTransport(config: NormalizedProviderConfig, request: ProviderRequest): AsyncIterable<ProviderStreamEvent> {
  const transport = config.transport;
  if (!transport) throw new ProviderRuntimeError("provider local transport is not configured");
  if (!transport.stream) {
    const response = await invokeLocalTransport(config, request);
    let sequence = 1;
    if (response.text) {
      yield streamEvent(config.provider, request.model, sequence, "in_memory.text", response.requestId, response.text, response.usage, false);
      sequence += 1;
    }
    for (const call of response.toolCalls) {
      yield streamEvent(config.provider, request.model, sequence, "in_memory.tool_call", response.requestId, "", response.usage, false, call);
      sequence += 1;
    }
    yield streamEvent(config.provider, request.model, sequence, "in_memory.done", response.requestId, "", response.usage, true);
    return;
  }
  let source: AsyncIterable<ProviderStreamEvent> | Iterable<ProviderStreamEvent>;
  try {
    source = await transport.stream(request);
  } catch (error) {
    throw inMemoryFailure(error, "stream handler");
  }
  if (!source || (typeof (source as AsyncIterable<ProviderStreamEvent>)[Symbol.asyncIterator] !== "function" && typeof (source as Iterable<ProviderStreamEvent>)[Symbol.iterator] !== "function")) {
    throw new ProviderRuntimeError("in-memory provider stream handler must return an iterable", { code: "invalid_response" });
  }
  try {
    let count = 0;
    let textBytes = 0;
    for await (const raw of source) {
      count += 1;
      if (count > MAX_PROVIDER_STREAM_EVENTS) throw new ProviderRuntimeError("in-memory provider stream exceeded its event bound", { code: "invalid_response" });
      const event = normalizeLocalStreamEvent(config, request, raw);
      textBytes += bytes(event.textDelta);
      if (textBytes > MAX_PROVIDER_STREAM_TEXT_BYTES) throw new ProviderRuntimeError("in-memory provider stream text exceeded its bound", { code: "invalid_response" });
      yield event;
    }
  } catch (error) {
    throw inMemoryFailure(error, "stream handler");
  }
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
  const capabilities = Array.isArray(row.capabilities)
    ? row.capabilities.filter((value): value is string => typeof value === "string").map((value) => boundedIdentifier("provider model capability", value, 128))
    : [];
  if (parameters.some((value) => ["tools", "tool_choice", "functions", "function_call"].includes(value))) capabilities.push("tool_use");
  if (parameters.some((value) => ["response_format", "json_object", "json_schema", "structured_outputs"].includes(value))) capabilities.push("structured_output");
  return [...new Set(capabilities)].sort();
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
  readonly providerQuota?: ProviderQuotaController;
  private readonly providers = new Map<string, NormalizedProviderConfig>();
  private readonly circuits = new Map<string, CircuitState>();
  private readonly providerHealthState = new Map<string, HealthState>();
  private readonly modelHealthState = new Map<string, HealthState>();
  private healthSnapshotGeneration = 0;
  private previousHealthSnapshotDigest: string | null = null;
  private cachedHealthSnapshot: LLMRuntimeHealthSnapshot | null = null;
  private cachedHealthSignature: string | null = null;
  private readonly fetchImplementation: FetchImplementation;
  private readonly clock: () => number;
  private effectBoundaryValue?: AutonomousEffectBoundary;

  constructor(options: { credentials?: CredentialStore; fetch?: FetchImplementation; clock?: () => number; effectBoundary?: AutonomousEffectBoundary; providerQuota?: ProviderQuotaController } = {}) {
    this.credentials = options.credentials ?? new CredentialStore();
    const implementation = options.fetch ?? globalThis.fetch;
    if (typeof implementation !== "function") throw new ProviderRuntimeError("a fetch implementation is required");
    this.fetchImplementation = implementation;
    this.clock = options.clock ?? (() => Date.now());
    if (options.providerQuota !== undefined && !(options.providerQuota instanceof ProviderQuotaController)) throw new ProviderRuntimeError("providerQuota must be a ProviderQuotaController");
    this.providerQuota = options.providerQuota;
    this.effectBoundaryValue = options.effectBoundary;
    if (this.effectBoundaryValue !== undefined && (typeof this.effectBoundaryValue.execute !== "function" || typeof this.effectBoundaryValue.executeStream !== "function")) throw new ProviderRuntimeError("effectBoundary must expose execute and executeStream methods");
    this.onboarding = new ProviderOnboarding(this);
  }

  get effectBoundary(): AutonomousEffectBoundary | undefined {
    return this.effectBoundaryValue;
  }

  bindEffectBoundary(effectBoundary: AutonomousEffectBoundary | undefined): void {
    if (effectBoundary !== undefined && (!effectBoundary || typeof effectBoundary.execute !== "function" || typeof effectBoundary.executeStream !== "function")) throw new ProviderRuntimeError("effectBoundary must expose execute and executeStream methods");
    if (this.effectBoundaryValue !== undefined && this.effectBoundaryValue !== effectBoundary) throw new ProviderRuntimeError("a different effectBoundary is already bound to this runtime");
    this.effectBoundaryValue = effectBoundary;
  }

  registerProvider(config: ProviderConfig): void {
    const normalized = normalizeConfig(config);
    this.providers.set(normalized.provider, normalized);
    this.circuits.set(normalized.provider, this.circuits.get(normalized.provider) ?? { consecutiveFailures: 0, openedUntil: null });
    this.providerHealthState.set(normalized.provider, this.providerHealthState.get(normalized.provider) ?? emptyHealth());
  }

  /**
   * Register an explicit credentialless local provider without opening a network socket.
   *
   * The handler is still behind the normal request validation, circuit, retry, observation,
   * health, tool-loop, and autonomous model-selection boundaries. It is never inferred for an
   * HTTP provider and cannot be configured to accept a credential handle.
   */
  registerInMemoryProvider(provider: string, handler: InMemoryProviderHandler, options: InMemoryProviderOptions = {}): void {
    if (typeof handler !== "function") throw new ProviderRuntimeError("in-memory provider handler must be callable");
    if (options.stream !== undefined && typeof options.stream !== "function") throw new ProviderRuntimeError("in-memory provider stream handler must be callable");
    if (options.discoverModels !== undefined && typeof options.discoverModels !== "function") throw new ProviderRuntimeError("in-memory provider model discovery handler must be callable");
    const { protocol, stream, discoverModels, ...config } = options;
    this.registerProvider({
      ...config,
      provider,
      baseUrl: "https://in-memory.invalid",
      protocol: protocol ?? "openai_responses",
      requiresCredential: false,
      transport: {
        invoke: handler,
        ...(stream ? { stream } : {}),
        ...(discoverModels ? { discoverModels } : {}),
      },
    });
  }

  providerMetadata(): JsonObject[] {
    return [...this.providers.values()].sort((a, b) => a.provider.localeCompare(b.provider)).map((config) => ({
      provider: config.provider,
      protocol: config.protocol,
      transport: config.transport ? "in_memory" : "http",
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
    if (config.transport) {
      if (options.credential !== undefined) throw new CredentialError(`provider ${provider} does not accept a credential handle`);
      if (!config.transport.discoverModels) throw new ProviderRuntimeError(`provider ${provider} does not expose local model discovery`, { code: "invalid_request" });
      try {
        const payload = await config.transport.discoverModels();
        return projectModelCatalog(config, safeJson(payload, "in-memory provider model catalog", config.maxResponseBytes), 200, null);
      } catch (error) {
        throw contextProviderFailure(errorFromUnknown(error), provider, "model_discovery");
      }
    }
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
    return healthProjection(provider, health, open ? "open" : "closed", circuit.consecutiveFailures, config.requiresCredential, config.transport !== undefined);
  }

  modelHealthSnapshot(): Record<string, ProviderHealth> {
    const result: Record<string, ProviderHealth> = {};
    for (const [arm, health] of [...this.modelHealthState.entries()].sort(([a], [b]) => a.localeCompare(b))) {
      const { provider, model } = splitProviderModelArm(arm, this.providers.keys());
      const circuit = this.circuits.get(provider) ?? { consecutiveFailures: 0, openedUntil: null };
      const open = circuit.openedUntil !== null && circuit.openedUntil > this.clock();
      const config = this.providers.get(provider);
      result[arm] = { ...healthProjection(provider, health, open ? "open" : "closed", circuit.consecutiveFailures, config?.requiresCredential ?? true, config?.transport !== undefined), model };
    }
    return result;
  }

  /** Seal provider transport health without retaining prompts, responses, headers, or credentials. */
  async snapshotHealth(): Promise<LLMRuntimeHealthSnapshot> {
    const providers = [...this.providers.keys()].sort().map((provider) => {
      const state = this.providerHealthState.get(provider) ?? emptyHealth();
      const circuit = this.circuits.get(provider) ?? { consecutiveFailures: 0, openedUntil: null };
      return {
        provider,
        attempts: state.attempts,
        successes: state.successes,
        failures: state.failures,
        total_latency_ms: state.totalLatencyMs,
        last_latency_ms: state.lastLatencyMs,
        last_model: state.lastModel,
        last_status_code: state.lastStatusCode,
        consecutive_failures: circuit.consecutiveFailures,
        circuit_opened_until: circuit.openedUntil,
      };
    });
    const models = [...this.modelHealthState.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([arm, state]) => {
      const { provider, model } = splitProviderModelArm(arm, this.providers.keys());
      return {
        provider,
        model,
        attempts: state.attempts,
        successes: state.successes,
        failures: state.failures,
        total_latency_ms: state.totalLatencyMs,
        last_latency_ms: state.lastLatencyMs,
        last_model: state.lastModel,
        last_status_code: state.lastStatusCode,
      };
    });
    const signature = canonicalJson({ providers, models });
    if (this.cachedHealthSnapshot !== null && this.cachedHealthSignature === signature) return structuredClone(this.cachedHealthSnapshot);
    const body = {
      schema: LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA,
      snapshot_generation: this.healthSnapshotGeneration + 1,
      previous_snapshot_digest: this.healthSnapshotGeneration === 0 ? null : this.previousHealthSnapshotDigest,
      providers,
      models,
      retention: "transport_health_metadata_only_hash_bound" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = await validateLLMRuntimeHealthSnapshot({ ...body, snapshot_digest: await digestJson(body) });
    this.healthSnapshotGeneration = snapshot.snapshot_generation!;
    this.previousHealthSnapshotDigest = snapshot.snapshot_digest;
    this.cachedHealthSnapshot = structuredClone(snapshot);
    this.cachedHealthSignature = signature;
    return structuredClone(snapshot);
  }

  /** Restore validated provider transport health atomically; providers must already be registered. */
  async restoreHealth(raw: unknown): Promise<void> {
    const snapshot = await validateLLMRuntimeHealthSnapshot(raw);
    for (const row of snapshot.providers) if (!this.providers.has(row.provider)) throw new ProviderRuntimeError(`cannot restore health for unregistered provider ${row.provider}`);
    for (const row of snapshot.models) if (!this.providers.has(row.provider)) throw new ProviderRuntimeError(`cannot restore model health for unregistered provider ${row.provider}`);
    const providers = new Map<string, HealthState>();
    const circuits = new Map<string, CircuitState>();
    for (const provider of this.providers.keys()) {
      providers.set(provider, emptyHealth());
      circuits.set(provider, { consecutiveFailures: 0, openedUntil: null });
    }
    for (const row of snapshot.providers) {
      providers.set(row.provider, {
        attempts: row.attempts,
        successes: row.successes,
        failures: row.failures,
        totalLatencyMs: row.total_latency_ms,
        lastLatencyMs: row.last_latency_ms,
        lastModel: row.last_model,
        lastStatusCode: row.last_status_code,
      });
      circuits.set(row.provider, { consecutiveFailures: row.consecutive_failures, openedUntil: row.circuit_opened_until });
    }
    const models = new Map<string, HealthState>();
    for (const row of snapshot.models) models.set(`${row.provider}/${row.model}`, {
      attempts: row.attempts,
      successes: row.successes,
      failures: row.failures,
      totalLatencyMs: row.total_latency_ms,
      lastLatencyMs: row.last_latency_ms,
      lastModel: row.last_model,
      lastStatusCode: row.last_status_code,
    });
    this.providerHealthState.clear();
    for (const [provider, state] of providers) this.providerHealthState.set(provider, state);
    this.circuits.clear();
    for (const [provider, circuit] of circuits) this.circuits.set(provider, circuit);
    this.modelHealthState.clear();
    for (const [arm, state] of models) this.modelHealthState.set(arm, state);
    this.healthSnapshotGeneration = snapshot.snapshot_generation ?? 0;
    this.previousHealthSnapshotDigest = this.healthSnapshotGeneration === 0 ? null : snapshot.snapshot_digest;
    this.cachedHealthSnapshot = snapshot.schema === LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA ? structuredClone(snapshot) : null;
    this.cachedHealthSignature = this.cachedHealthSnapshot === null ? null : canonicalJson({ providers: this.cachedHealthSnapshot.providers, models: this.cachedHealthSnapshot.models });
  }

  async saveHealth(persistence: LLMRuntimeHealthPersistence): Promise<LLMRuntimeHealthSnapshot> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("LLM runtime health persistence adapter is malformed");
    const snapshot = await this.snapshotHealth();
    await persistence.write(snapshot);
    return snapshot;
  }

  async restorePersistedHealth(persistence: LLMRuntimeHealthPersistence): Promise<LLMRuntimeHealthSnapshot | null> {
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("LLM runtime health persistence adapter is malformed");
    const raw = await persistence.read();
    if (raw === null) return null;
    const snapshot = await validateLLMRuntimeHealthSnapshot(raw);
    await this.restoreHealth(snapshot);
    return snapshot;
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
    const quota = options.providerQuota ?? this.providerQuota;
    const quotaReservation = quota?.reserve({ provider, model: request.model, inputTokens: metadata.inputTokens, outputTokens: request.maxOutputTokens, costUnits: options.estimatedCostUnits ?? 0 });
    const releaseCost = options.reserveCost?.(options.estimatedCostUnits ?? 0);
    try {
      await options.execution?.admitProviderCall({ provider, model: request.model, invocationKind: metadata.kind, attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits, costUnits: options.estimatedCostUnits, failover: options.executionFailover });
      await options.observer?.before?.(metadata);
    } catch (error) {
      quotaReservation?.release();
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
      const selectedBoundary = options.effectBoundary ?? this.effectBoundaryValue;
      let response: ProviderResponse;
      if (!selectedBoundary) {
        quotaReservation?.markDispatched();
        response = await this.request(config, request, options.credential, options.signal, false);
      } else {
        const requestDigest = await digestJson({
          provider,
          model: request.model,
          kind: metadata.kind,
          messages: request.messages,
          max_output_tokens: request.maxOutputTokens,
          temperature: request.temperature ?? null,
          require_json: request.requireJson ?? false,
          response_schema: request.responseSchema ?? null,
          tools: request.tools ?? [],
          tool_choice: request.toolChoice ?? null,
        });
        const providerKey = request.idempotencyKey ?? generatedProviderIdempotencyKey("aurora-provider");
        const callId = `provider-call-${(await digestJson(providerKey)).slice(0, 48)}`;
        const executionId = options.execution?.state.execution_id ?? null;
        response = await selectedBoundary.execute(
          {
            execution_id: executionId,
            tool: `provider.${provider}.invoke`,
            call_id: callId,
            risk_class: "provider_invocation",
            arguments: {
              provider,
              model: request.model,
              kind: metadata.kind,
              request_digest: requestDigest,
              requested_output_tokens: request.maxOutputTokens,
              tool_count: request.tools?.length ?? 0,
              idempotency_key_present: request.idempotencyKey !== undefined,
            },
          },
          async (context) => {
            quotaReservation?.markDispatched();
            return this.request(config, request.idempotencyKey ? request : { ...request, idempotencyKey: context.idempotency_key }, options.credential, options.signal, false);
          },
          { execution: options.execution, resultProjector: providerEffectProjection, cacheResult: false, definiteFailure: providerEffectFailureIsDefinite },
        );
      }
      const latencyMs = Math.max(0, nowMs() - started);
      this.record(provider, request.model, true, latencyMs, response.statusCode, response);
      await recordOutcome({ success: true, status: "completed", latencyMs, inputTokens: response.usage.input_tokens ?? metadata.inputTokens, outputTokens: response.usage.output_tokens ?? 0, statusCode: response.statusCode });
      quotaReservation?.settle({ inputTokens: response.usage.input_tokens ?? metadata.inputTokens, outputTokens: response.usage.output_tokens ?? 0, costUnits: options.estimatedCostUnits ?? 0 });
      return response;
    } catch (unknownError) {
      if (unknownError instanceof AutonomousEffectReconciliationRequiredError) {
        const latencyMs = Math.max(0, nowMs() - started);
        this.record(provider, request.model, false, latencyMs, null);
        await recordOutcome({ success: false, status: "provider_refused", latencyMs, inputTokens: metadata.inputTokens, outputTokens: 0, failureClass: "provider_error", failureCode: "provider_error", retryable: false });
        throw unknownError;
      }
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
      if (quotaReservation) {
        if (quotaReservation.isDispatched) quotaReservation.settle();
        else quotaReservation.release();
      }
      throw error;
    }
  }

  async *invokeStream(
    provider: string,
    request: ProviderRequest,
    options: ProviderInvocationOptions = {},
  ): AsyncIterable<ProviderStreamEvent> {
    const selectedBoundary = options.effectBoundary ?? this.effectBoundaryValue;
    if (!selectedBoundary) {
      for await (const event of this.invokeStreamUnbounded(provider, request, options)) yield event;
      return;
    }
    if (typeof selectedBoundary.executeStream !== "function") throw new ProviderRuntimeError("effectBoundary must expose executeStream for live provider streams");
    const requestDigest = await digestJson({
      provider,
      model: request.model,
      kind: options.invocationKind ?? "provider_stream",
      messages: request.messages,
      max_output_tokens: request.maxOutputTokens,
      temperature: request.temperature ?? null,
      require_json: request.requireJson ?? false,
      response_schema: request.responseSchema ?? null,
      tools: request.tools ?? [],
      tool_choice: request.toolChoice ?? null,
    });
    const generatedKey = request.idempotencyKey ?? generatedProviderIdempotencyKey("aurora-provider-stream");
    const callId = `provider-stream-${(await digestJson(generatedKey)).slice(0, 48)}`;
    const executionId = options.execution?.state.execution_id ?? null;
    const summary: JsonObject = {
      provider,
      model: request.model,
      event_count: 0,
      text_delta_bytes: 0,
      tool_call_count: 0,
      done_seen: false,
    };
    const observe = async (event: ProviderStreamEvent, eventCount: number): Promise<void> => {
      summary.event_count = eventCount;
      summary.text_delta_bytes = (summary.text_delta_bytes as number) + bytes(event.textDelta);
      if (event.toolCall) summary.tool_call_count = (summary.tool_call_count as number) + 1;
      summary.done_seen = Boolean(summary.done_seen || event.done);
      for (const key of ["input_tokens", "output_tokens", "total_tokens"] as const) {
        const value = event.usage[key];
        if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) summary[key] = value;
      }
      if (event.requestId) summary.request_id_digest = await digestJson(event.requestId);
    };
    const project = (base: JsonObject): JsonObject => ({ ...summary, event_count: base.event_count, completed: true });
    const stream = selectedBoundary.executeStream(
      {
        execution_id: executionId,
        tool: `provider.${provider}.stream`,
        call_id: callId,
        risk_class: "provider_invocation",
        arguments: {
          provider,
          model: request.model,
          kind: options.invocationKind ?? "provider_stream",
          request_digest: requestDigest,
          requested_output_tokens: request.maxOutputTokens,
          tool_count: request.tools?.length ?? 0,
          idempotency_key_present: request.idempotencyKey !== undefined,
        },
      },
      async (context) => this.invokeStreamUnbounded(provider, request.idempotencyKey ? request : { ...request, idempotencyKey: context.idempotency_key }, options),
      { execution: options.execution, summaryProjector: project, observe, definiteFailure: providerEffectFailureIsDefinite },
    );
    for await (const event of stream) yield event;
  }

  private async *invokeStreamUnbounded(
    provider: string,
    request: ProviderRequest,
    options: ProviderInvocationOptions = {},
  ): AsyncIterable<ProviderStreamEvent> {
    const config = this.requireProvider(provider);
    validateRequest(request);
    validateStructuredOutputSupport(config, request);
    const metadata = requestMetadata(provider, request, options.invocationKind ?? "provider_stream");
    const quota = options.providerQuota ?? this.providerQuota;
    const quotaReservation = quota?.reserve({ provider, model: request.model, inputTokens: metadata.inputTokens, outputTokens: request.maxOutputTokens, costUnits: options.estimatedCostUnits ?? 0 });
    const releaseCost = options.reserveCost?.(options.estimatedCostUnits ?? 0);
    try {
      await options.execution?.admitProviderCall({ provider, model: request.model, invocationKind: metadata.kind, attempt: options.executionAttempt, turn: options.executionTurn, selectionDigest: options.selectionDigest, estimatedCostUnits: options.estimatedCostUnits, costUnits: options.estimatedCostUnits, failover: options.executionFailover });
      await options.observer?.before?.(metadata);
    } catch (error) {
      quotaReservation?.release();
      releaseCost?.();
      throw error;
    }
    const started = nowMs();
    let outcome: ProviderInvocationOutcome | null = null;
    try {
      if (config.transport) {
        const circuit = this.circuits.get(provider) ?? { consecutiveFailures: 0, openedUntil: null };
        if (options.signal?.aborted) throw new ProviderRuntimeError("provider request was aborted before dispatch", { code: "aborted" });
        if (circuit.openedUntil !== null && circuit.openedUntil > this.clock()) throw new ProviderRuntimeError("provider circuit is open; invocation is temporarily refused", { circuitOpen: true, code: "circuit_open" });
        if (circuit.openedUntil !== null) { circuit.openedUntil = null; circuit.consecutiveFailures = 0; }
        try {
          quotaReservation?.markDispatched();
          for await (const event of streamLocalTransport(config, request)) yield event;
          circuit.consecutiveFailures = 0;
          circuit.openedUntil = null;
          this.record(provider, request.model, true, Math.max(0, nowMs() - started), 200);
          outcome = { success: true, status: "completed", latencyMs: Math.max(0, nowMs() - started), inputTokens: metadata.inputTokens, outputTokens: 0, statusCode: 200 };
          return;
        } catch (error) {
          const normalized = errorFromUnknown(error);
          if (normalized instanceof ProviderRuntimeError && normalized.retryable) {
            circuit.consecutiveFailures += 1;
            if (circuit.consecutiveFailures >= config.circuitBreakerFailureThreshold) circuit.openedUntil = this.clock() + config.circuitBreakerResetMs;
          }
          throw normalized;
        }
      }
      quotaReservation?.markDispatched();
      const response = await this.fetchWithRetries(config, request, options.credential, options.signal, true);
      if (isProviderResponseValue(response)) throw new ProviderRuntimeError("provider stream returned a non-stream response");
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
        if (quotaReservation?.isDispatched) quotaReservation.settle({ inputTokens: metadata.inputTokens, outputTokens: request.maxOutputTokens, costUnits: options.estimatedCostUnits ?? 0 });
        else quotaReservation?.release();
      }
    }
  }

  async collectStream(provider: string, request: ProviderRequest, options: ProviderInvocationOptions = {}): Promise<ProviderResponse> {
    const collect = async (dispatchedRequest: ProviderRequest): Promise<ProviderResponse> => {
      const text: string[] = [];
      const calls: ProviderToolCall[] = [];
      let usage: ProviderUsage = {};
      let model = dispatchedRequest.model;
      let requestId: string | null = null;
      let done = false;
      // collectStream owns the response-level effect boundary below; do not nest a second
      // stream boundary around the same provider dispatch.
      for await (const event of this.invokeStreamUnbounded(provider, dispatchedRequest, options)) {
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
      if (!calls.length && dispatchedRequest.requireJson) {
        try { structured = JSON.parse(outputText) as JsonValue; } catch { throw new ProviderRuntimeError("provider stream returned invalid JSON", { code: "invalid_response" }); }
        validateStructuredResponseOrThrow(structured, dispatchedRequest.responseSchema);
      }
      return { provider, model, text: outputText, statusCode: 200, requestId, usage, structured, toolCalls: calls, stopReason: null };
    };
    const selectedBoundary = options.effectBoundary ?? this.effectBoundaryValue;
    if (!selectedBoundary) return collect(request);
    const requestDigest = await digestJson({
      provider,
      model: request.model,
      kind: options.invocationKind ?? "provider_stream",
      messages: request.messages,
      max_output_tokens: request.maxOutputTokens,
      temperature: request.temperature ?? null,
      require_json: request.requireJson ?? false,
      response_schema: request.responseSchema ?? null,
      tools: request.tools ?? [],
      tool_choice: request.toolChoice ?? null,
    });
    const providerKey = request.idempotencyKey ?? generatedProviderIdempotencyKey("aurora-provider-stream");
    const callId = `provider-stream-${(await digestJson(providerKey)).slice(0, 48)}`;
    const executionId = options.execution?.state.execution_id ?? null;
    return selectedBoundary.execute(
      {
        execution_id: executionId,
        tool: `provider.${provider}.stream`,
        call_id: callId,
        risk_class: "provider_invocation",
        arguments: {
          provider,
          model: request.model,
          kind: options.invocationKind ?? "provider_stream",
          request_digest: requestDigest,
          requested_output_tokens: request.maxOutputTokens,
          tool_count: request.tools?.length ?? 0,
          idempotency_key_present: request.idempotencyKey !== undefined,
        },
      },
      async (context) => collect(request.idempotencyKey ? request : { ...request, idempotencyKey: context.idempotency_key }),
      { execution: options.execution, resultProjector: providerEffectProjection, cacheResult: false, definiteFailure: providerEffectFailureIsDefinite },
    );
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
      if (options.contextBudget !== undefined) current = (await compactAutonomousProviderRequest(current, options.contextBudget)).request;
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
        await options.execution?.recordToolOutcome({ tool: call.name, callId: result.callId, status: providerToolResultStatus(result), outcomeDigest: await digestJson({ call_id: result.callId, approved: result.approved, is_error: result.isError ?? false, content: result.content }) });
      }
      if (returned.some((result) => !result.approved)) return { status: returned.some((result) => providerToolResultStatus(result) === "reconciliation_required") ? "reconciliation_required" : "authorization_required", responses, finalResponse: response, turns: responses.length, toolCalls };
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
    if (isProviderResponseValue(response)) return response;
    const body = await readBoundedBody(response, config.maxResponseBytes);
    if (response.status >= 400) throw providerHttpError(response.status, response.headers);
    let payload: unknown;
    try { payload = JSON.parse(body); } catch { throw new ProviderRuntimeError("provider returned a non-JSON response", { statusCode: response.status }); }
    if (!isObject(payload)) throw new ProviderRuntimeError("provider response must be a JSON object", { statusCode: response.status });
    const parsed = parseResponse(config, payload as JsonObject, response.status, request, requestIdFromHeaders(response.headers) ?? asString(payload.id));
    return parsed;
  }

  private async fetchWithRetries(config: NormalizedProviderConfig, request: ProviderRequest, credential: CredentialHandle | undefined, signal: AbortSignal | undefined, stream: boolean): Promise<Response | ProviderResponse> {
    const circuit = this.circuits.get(config.provider) ?? { consecutiveFailures: 0, openedUntil: null };
    if (signal?.aborted) throw new ProviderRuntimeError("provider request was aborted before dispatch", { code: "aborted" });
    if (circuit.openedUntil !== null && circuit.openedUntil > this.clock()) throw new ProviderRuntimeError("provider circuit is open; invocation is temporarily refused", { circuitOpen: true, code: "circuit_open" });
    if (circuit.openedUntil !== null) { circuit.openedUntil = null; circuit.consecutiveFailures = 0; }
    let lastError: ProviderFailure | null = null;
    for (let attempt = 0; attempt < config.maxAttempts; attempt += 1) {
      try {
        const response = await this.fetchOnce(config, request, credential, signal, stream);
        if (isProviderResponseValue(response)) {
          circuit.consecutiveFailures = 0;
          circuit.openedUntil = null;
          return response;
        }
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

  private async fetchOnce(config: NormalizedProviderConfig, request: ProviderRequest, credential: CredentialHandle | undefined, callerSignal: AbortSignal | undefined, stream: boolean): Promise<Response | ProviderResponse> {
    if (config.requiresCredential && credential === undefined) throw new CredentialError(`provider ${config.provider} requires a user credential handle`);
    if (!config.requiresCredential && credential !== undefined) throw new CredentialError(`provider ${config.provider} does not accept a credential handle`);
    if (callerSignal?.aborted) throw abortFailure(callerSignal, false);
    if (config.transport) {
      if (stream) throw new ProviderRuntimeError("in-memory streaming is handled by the stream transport boundary");
      return invokeLocalTransport(config, request);
    }
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

function runtimeHealthKeys(name: string, value: JsonObject, allowed: readonly string[]): void {
  const accepted = new Set(allowed);
  if (Object.keys(value).some((key) => !accepted.has(key))) throw new ProviderRuntimeError(`${name} contains unsupported or secret-shaped metadata`);
}

function boundedHealthCount(name: string, value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > Number.MAX_SAFE_INTEGER) throw new ProviderRuntimeError(`${name} is outside its bounded health contract`);
  return value as number;
}

function boundedHealthMetric(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER) throw new ProviderRuntimeError(`${name} is outside its bounded health contract`);
  return value;
}

function boundedHealthTimestamp(name: string, value: unknown): number | null {
  if (value === null) return null;
  return boundedHealthMetric(name, value);
}

function boundedHealthModel(name: string, value: unknown): string | null {
  if (value === null) return null;
  return boundedIdentifier(name, value, 512);
}

interface NormalizedRuntimeHealthCounts {
  attempts: number;
  successes: number;
  failures: number;
  total_latency_ms: number;
  last_latency_ms: number | null;
  last_model: string | null;
  last_status_code: number | null;
}

function normalizeRuntimeHealthCounts(name: string, value: JsonObject): NormalizedRuntimeHealthCounts {
  const attempts = boundedHealthCount(`${name} attempts`, value.attempts);
  const successes = boundedHealthCount(`${name} successes`, value.successes);
  const failures = boundedHealthCount(`${name} failures`, value.failures);
  if (successes + failures !== attempts) throw new ProviderRuntimeError(`${name} attempts do not equal successes plus failures`);
  return {
    attempts,
    successes,
    failures,
    total_latency_ms: boundedHealthMetric(`${name} total_latency_ms`, value.total_latency_ms),
    last_latency_ms: value.last_latency_ms === null ? null : boundedHealthMetric(`${name} last_latency_ms`, value.last_latency_ms),
    last_model: boundedHealthModel(`${name} last_model`, value.last_model),
    last_status_code: value.last_status_code === null ? null : boundedHealthCount(`${name} last_status_code`, value.last_status_code),
  };
}

function normalizeRuntimeProviderHealthSnapshot(value: unknown): LLMRuntimeProviderHealthSnapshot {
  if (!isObject(value)) throw new ProviderRuntimeError("LLM runtime provider health row must be an object");
  const row = value as unknown as JsonObject;
  runtimeHealthKeys("LLM runtime provider health row", row, ["provider", "attempts", "successes", "failures", "total_latency_ms", "last_latency_ms", "last_model", "last_status_code", "consecutive_failures", "circuit_opened_until"]);
  const counts = normalizeRuntimeHealthCounts("LLM runtime provider health row", row);
  const provider = boundedIdentifier("LLM runtime provider health provider", row.provider, 128);
  return {
    provider,
    ...counts,
    consecutive_failures: boundedHealthCount("LLM runtime provider health consecutive_failures", row.consecutive_failures),
    circuit_opened_until: boundedHealthTimestamp("LLM runtime provider health circuit_opened_until", row.circuit_opened_until),
  };
}

function normalizeRuntimeModelHealthSnapshot(value: unknown): LLMRuntimeModelHealthSnapshot {
  if (!isObject(value)) throw new ProviderRuntimeError("LLM runtime model health row must be an object");
  const row = value as unknown as JsonObject;
  runtimeHealthKeys("LLM runtime model health row", row, ["provider", "model", "attempts", "successes", "failures", "total_latency_ms", "last_latency_ms", "last_model", "last_status_code"]);
  const counts = normalizeRuntimeHealthCounts("LLM runtime model health row", row);
  return { provider: boundedIdentifier("LLM runtime model health provider", row.provider, 128), model: boundedIdentifier("LLM runtime model health model", row.model, 512), ...counts };
}

export async function validateLLMRuntimeHealthSnapshot(value: unknown): Promise<LLMRuntimeHealthSnapshot> {
  if (!isObject(value)) throw new ProviderRuntimeError("LLM runtime health snapshot must be an object");
  const snapshotValue = value as unknown as JsonObject;
  const legacy = snapshotValue.schema === LEGACY_LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA;
  if (snapshotValue.schema !== LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA && !legacy) throw new ProviderRuntimeError("LLM runtime health snapshot schema is unsupported");
  runtimeHealthKeys("LLM runtime health snapshot", snapshotValue, legacy
    ? ["schema", "providers", "models", "snapshot_digest", "retention", "secret_material"]
    : ["schema", "snapshot_generation", "previous_snapshot_digest", "providers", "models", "snapshot_digest", "retention", "secret_material"]);
  if (snapshotValue.retention !== "transport_health_metadata_only_hash_bound" || snapshotValue.secret_material !== "never_returned") throw new ProviderRuntimeError("LLM runtime health snapshot markers are invalid");
  if (!legacy) {
    if (!Number.isSafeInteger(snapshotValue.snapshot_generation) || (snapshotValue.snapshot_generation as number) < 1) throw new ProviderRuntimeError("LLM runtime health snapshot generation is outside its bounds");
    if (snapshotValue.previous_snapshot_digest !== null && (typeof snapshotValue.previous_snapshot_digest !== "string" || !/^[0-9a-f]{64}$/.test(snapshotValue.previous_snapshot_digest))) throw new ProviderRuntimeError("LLM runtime health previous_snapshot_digest is malformed");
    if (((snapshotValue.snapshot_generation as number) === 1) !== (snapshotValue.previous_snapshot_digest === null)) throw new ProviderRuntimeError("LLM runtime health snapshot generation and previous_snapshot_digest are inconsistent");
  }
  if (!Array.isArray(snapshotValue.providers) || snapshotValue.providers.length > MAX_LLM_RUNTIME_HEALTH_PROVIDERS) throw new ProviderRuntimeError("LLM runtime health snapshot provider capacity is exceeded");
  if (!Array.isArray(snapshotValue.models) || snapshotValue.models.length > MAX_LLM_RUNTIME_HEALTH_MODELS) throw new ProviderRuntimeError("LLM runtime health snapshot model capacity is exceeded");
  const providers = snapshotValue.providers.map(normalizeRuntimeProviderHealthSnapshot);
  const models = snapshotValue.models.map(normalizeRuntimeModelHealthSnapshot);
  const providerIds = new Set<string>();
  for (const row of providers) {
    if (providerIds.has(row.provider)) throw new ProviderRuntimeError(`LLM runtime health snapshot contains duplicate provider ${row.provider}`);
    providerIds.add(row.provider);
  }
  const modelIds = new Set<string>();
  for (const row of models) {
    const id = `${row.provider}/${row.model}`;
    if (modelIds.has(id)) throw new ProviderRuntimeError(`LLM runtime health snapshot contains duplicate model ${id}`);
    modelIds.add(id);
  }
  const snapshotDigest = typeof snapshotValue.snapshot_digest === "string" && /^[0-9a-f]{64}$/.test(snapshotValue.snapshot_digest) ? snapshotValue.snapshot_digest : null;
  if (!snapshotDigest) throw new ProviderRuntimeError("LLM runtime health snapshot digest is malformed");
  const descriptor = legacy
    ? { schema: LEGACY_LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA, providers, models, retention: "transport_health_metadata_only_hash_bound" as const, secret_material: "never_returned" as const }
    : { schema: LLM_RUNTIME_HEALTH_SNAPSHOT_SCHEMA, snapshot_generation: snapshotValue.snapshot_generation as number, previous_snapshot_digest: snapshotValue.previous_snapshot_digest as string | null, providers, models, retention: "transport_health_metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
  if (await digestJson(descriptor) !== snapshotDigest) throw new ProviderRuntimeError("LLM runtime health snapshot digest mismatch");
  const snapshot = { ...descriptor, snapshot_digest: snapshotDigest };
  if (bytes(canonicalJson(snapshot)) > MAX_LLM_RUNTIME_HEALTH_SNAPSHOT_BYTES) throw new ProviderRuntimeError("LLM runtime health snapshot exceeds its byte capacity");
  return structuredClone(snapshot);
}

/** Canonical JSON persistence for transport-health snapshots over a caller-owned text store. */
export class JsonLLMRuntimeHealthSnapshotPersistence implements LLMRuntimeHealthPersistence {
  protected readonly textStore: LLMRuntimeHealthSnapshotTextStore;
  readonly maxBytes: number;

  constructor(textStore: LLMRuntimeHealthSnapshotTextStore, maxBytes = MAX_LLM_RUNTIME_HEALTH_SNAPSHOT_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("LLM runtime health text store is malformed");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_LLM_RUNTIME_HEALTH_SNAPSHOT_BYTES) throw new ArgumentError("LLM runtime health JSON maxBytes is outside its bound");
    this.textStore = textStore;
    this.maxBytes = maxBytes;
  }

  async read(): Promise<LLMRuntimeHealthSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > this.maxBytes) throw new ProviderRuntimeError("LLM runtime health JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ProviderRuntimeError("LLM runtime health JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new ProviderRuntimeError("LLM runtime health JSON is not canonical");
    const snapshot = await validateLLMRuntimeHealthSnapshot(parsed);
    if (bytes(canonicalJson(snapshot)) > this.maxBytes) throw new ProviderRuntimeError("LLM runtime health JSON exceeds its byte bound");
    return snapshot;
  }

  async write(snapshot: LLMRuntimeHealthSnapshot): Promise<void> {
    await this.textStore.write(await this.encode(snapshot));
  }

  protected async encode(snapshot: LLMRuntimeHealthSnapshot): Promise<string> {
    const validated = await validateLLMRuntimeHealthSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > this.maxBytes) throw new ProviderRuntimeError("LLM runtime health JSON exceeds its byte bound");
    return encoded;
  }
}

/** Canonical JSON transport-health persistence with atomic stale-writer fencing. */
export class TransactionalJsonLLMRuntimeHealthSnapshotPersistence extends JsonLLMRuntimeHealthSnapshotPersistence {
  declare protected readonly textStore: LLMRuntimeTransactionalHealthSnapshotTextStore;

  constructor(textStore: LLMRuntimeTransactionalHealthSnapshotTextStore, maxBytes = MAX_LLM_RUNTIME_HEALTH_SNAPSHOT_BYTES) {
    super(textStore, maxBytes);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("LLM runtime health text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: LLMRuntimeHealthSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ProviderRuntimeError("LLM runtime health expected snapshot digest is malformed");
    const committed = await this.textStore.writeIfUnchanged(expectedSnapshotDigest, await this.encode(snapshot));
    if (typeof committed !== "boolean") throw new ProviderRuntimeError("LLM runtime health compare-and-swap returned a non-boolean result");
    return committed;
  }
}

/** Browser-compatible local text storage for transport-health snapshots. */
export class WebStorageLLMRuntimeHealthSnapshotTextStore implements LLMRuntimeHealthSnapshotTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("LLM runtime health Web Storage adapter is malformed");
    boundedIdentifier("LLM runtime health storage key", key, 256);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/** Connect LLM transport-health snapshots to a caller-owned durable adapter. */
export class LLMRuntimeHealthPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly runtime: LLMRuntime, readonly persistence: LLMRuntimeHealthPersistence) {
    if (!runtime || typeof runtime.snapshotHealth !== "function" || typeof runtime.restoreHealth !== "function") throw new ArgumentError("LLM runtime health persistence requires an LLMRuntime");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("LLM runtime health persistence adapter is malformed");
  }

  async restore(): Promise<LLMRuntimeHealthSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = await validateLLMRuntimeHealthSnapshot(raw);
      await this.runtime.restoreHealth(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  async flush(): Promise<LLMRuntimeHealthSnapshot> {
    return this.enqueue(async () => {
      const snapshot = await this.runtime.snapshotHealth();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ProviderRuntimeError("LLM runtime health persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

function estimatedProviderCostUnits(candidate: AutonomousModelCandidate | undefined, request: ProviderRequest): number {
  if (!candidate) return 0;
  const estimatedInputTokens = Math.max(1, Math.ceil(request.messages.reduce((sum, message) => sum + providerContentBytes(message.content, message.role), 0) / 4));
  return ((estimatedInputTokens + request.maxOutputTokens) / 1_000_000) * candidate.cost_per_million_tokens;
}

/**
 * A timeout can be isolated to one model's latency/capacity profile. Keep sibling models from
 * being discarded in that case; transport, circuit, and provider HTTP failures remain
 * provider-scoped so failover does not multiply an outage across every model arm.
 */
function modelFailoverAllowed(error: ProviderRuntimeError): boolean {
  return error.code === "timeout" && error.circuitOpen !== true;
}

function selectionIsCredentialUnavailable(ranking: readonly AutonomousModelRanking[]): boolean {
  const credentialGateReasons = new Set(["credential not ready", "provider health ineligible"]);
  return ranking.length > 0 && ranking.every((row) =>
    row.reasons.includes("credential not ready")
    && row.reasons.every((reason) => credentialGateReasons.has(reason))
  );
}

async function emitContinuationSelectionTrace(
  callback: AutonomousModelSelectionTraceEventCallback | undefined,
  selection: AutonomousSelectionDecision,
  plan: AutonomousModelContinuationPlan,
  attempt: number,
): Promise<void> {
  if (!callback) return;
  const selected = selection.selected_model;
  const common = {
    attempt,
    failover: attempt > 1,
    candidate_count: selection.ranking.length,
    eligible_candidate_count: selection.ranking.filter((row) => row.eligible).length,
    strategy: selection.strategy,
    selected_provider: selected?.provider ?? null,
    selected_model: selected?.model ?? null,
    selection_digest: await digestJson(selection),
    detail_digest: await digestJson({ continuation_plan_digest: plan.plan_digest, step_order: plan.steps.find((step) => step.provider === selected?.provider && step.model === selected?.model)?.order ?? null }),
    failure_code: null,
  } satisfies Omit<AutonomousModelSelectionTraceEvent, "phase" | "status">;
  await callback({ ...common, phase: "model_selection_started", status: "running", selected_provider: null, selected_model: null, selection_digest: null });
  await callback({ ...common, phase: "model_selection_finished", status: "selected" });
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

  async select(
    plan: AutonomousExecutionPlan,
    options: {
      excludedProviders?: readonly string[];
      excludedModels?: readonly string[];
      selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
      attempt?: number;
    } = {},
  ): Promise<AutonomousSelectionDecision> {
    if (options.selectionEventCallback !== undefined && typeof options.selectionEventCallback !== "function") throw new ProviderRuntimeError("autonomous model selection trace callback must be callable");
    // Selection must rank against the request that will actually be dispatched. This prevents a
    // large stale transcript from making a model appear eligible before compaction and then
    // overflowing the same model at the provider boundary.
    if (plan.contextBudget !== undefined) {
      const prepared = await compactAutonomousProviderRequest(plan.request, plan.contextBudget);
      plan = { ...plan, request: prepared.request };
    }
    const selectionEventCallback = options.selectionEventCallback;
    const traceEnabled = selectionEventCallback !== undefined;
    const request = this.selectionRequest(plan, options.excludedProviders, options.excludedModels);
    const ranking = this.rank(request);
    const selectionConfidence = autonomousSelectionConfidence(ranking);
    const minimumConfidence = request.min_selection_confidence ?? null;
    const attempt = options.attempt ?? null;
    const failover = attempt !== null && attempt > 1;
    const selectionEvent = async (
      phase: AutonomousModelSelectionTraceEvent["phase"],
      status: AutonomousModelSelectionTraceEvent["status"],
      decision: AutonomousSelectionDecision | null,
      detailDigest: string | null = null,
      failureCode: string | null = null,
    ): Promise<void> => {
      if (!traceEnabled) return;
      const selected = decision?.selected_model ?? null;
      const event: AutonomousModelSelectionTraceEvent = {
        phase,
        status,
        attempt,
        failover,
        candidate_count: request.candidates.length,
        eligible_candidate_count: ranking.filter((row) => row.eligible).length,
        strategy: decision?.strategy ?? (this.selector ? "caller_selector" : "deterministic_health_utility"),
        selected_provider: selected?.provider ?? null,
        selected_model: selected?.model ?? null,
        selection_digest: decision === null ? null : await digestJson(decision),
        detail_digest: detailDigest,
        failure_code: failureCode,
      };
      await selectionEventCallback(event);
    };
    const startDetailDigest = traceEnabled ? await digestJson({
      task: request.task,
      domain: request.domain,
      capability: request.capability,
      risk_class: request.risk_class,
      context_digest: request.context_digest ?? null,
      candidate_count: request.candidates.length,
      excluded_providers: [...(options.excludedProviders ?? [])].sort(),
      excluded_models: [...(options.excludedModels ?? [])].sort(),
    }) : null;
    await selectionEvent("model_selection_started", "running", null, startDetailDigest);
    const finishSelection = async (decision: AutonomousSelectionDecision): Promise<AutonomousSelectionDecision> => {
      const status = decision.selected_model === null ? "abstained" as const : "selected" as const;
      const failureCode = decision.selected_model === null ? "selection_abstained" : null;
      const detailDigest = traceEnabled ? await digestJson({
        candidate_count: request.candidates.length,
        eligible_candidate_count: ranking.filter((row) => row.eligible).length,
        strategy: decision.strategy,
        abstention_reason: decision.abstention_reason,
        selection_confidence: decision.selection_confidence ?? null,
        min_selection_confidence: decision.min_selection_confidence ?? null,
        exploration_taken: decision.exploration_taken ?? false,
      }) : null;
      await selectionEvent(
        "model_selection_finished",
        status,
        decision,
        detailDigest,
        failureCode,
      );
      return decision;
    };
    try {
      if (!ranking.some((row) => row.eligible)) {
        return finishSelection({ selected_model: null, strategy: this.selector ? "caller_selector" : "deterministic_health_utility", ranking, abstention_reason: ranking.flatMap((row) => row.reasons).join("; ") || "no eligible model candidate", selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence });
      }
      if (minimumConfidence !== null && selectionConfidence < minimumConfidence) {
        return finishSelection({ selected_model: null, strategy: this.selector ? "caller_selector" : "deterministic_health_utility", ranking, abstention_reason: `selection confidence ${selectionConfidence.toFixed(6)} is below caller floor ${minimumConfidence.toFixed(6)}`, selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence });
      }
      if (this.selector) {
        const selected = await this.selector(request);
        if (!isObject(selected)) throw new ProviderRuntimeError("autonomous model selector returned a malformed decision");
        const projectedRanking = selectorRankingProjection(selected.ranking, ranking);
        const exploration = selectorExplorationProjection(selected);
        const selectedModel = selected.selected_model;
        if (selectedModel === null) return finishSelection({ selected_model: null, strategy: "caller_selector", ranking: projectedRanking, abstention_reason: typeof selected.abstention_reason === "string" ? selected.abstention_reason : "caller selector abstained", selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence, ...exploration });
        if (!isObject(selectedModel) || typeof selectedModel.provider !== "string" || typeof selectedModel.model !== "string") throw new ProviderRuntimeError("autonomous selector returned an invalid selected_model");
        const chosen = ranking.find((row) => row.provider === selectedModel.provider && row.model === selectedModel.model);
        if (!chosen || !chosen.eligible) throw new ProviderRuntimeError("autonomous selector chose an ineligible model");
        return finishSelection({ selected_model: { provider: chosen.provider, model: chosen.model }, strategy: "caller_selector", ranking: projectedRanking, abstention_reason: null, selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence, ...exploration });
      }
      const chosen = ranking.find((row) => row.eligible);
      return finishSelection({ selected_model: chosen ? { provider: chosen.provider, model: chosen.model } : null, strategy: "deterministic_health_utility", ranking, abstention_reason: chosen ? null : "no eligible model candidate", selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence });
    } catch (error) {
      if (traceEnabled) {
        await selectionEvent(
          "model_selection_finished",
          "failed",
          null,
          await digestJson({ error_class: error instanceof Error ? error.constructor.name : "UnknownError" }),
          error instanceof ProviderRuntimeError ? error.code : "selection_error",
        ).catch(() => undefined);
      }
      throw error;
    }
  }

  async invoke(
    plan: AutonomousExecutionPlan,
    options: {
      credential?: CredentialHandle;
      credentialFor?: (provider: string) => CredentialHandle | undefined;
      signal?: AbortSignal;
      observer?: ProviderInvocationObserver;
      feedback?: (decision: AutonomousSelectionDecision, outcome: ProviderInvocationOutcome) => void | Promise<void>;
      selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
      execution?: AutonomousExecutionController;
      executionAttempt?: number;
      maxProviderFailovers?: number;
      reserveCost?: AutonomousCostReservationCallback;
    } = {},
  ): Promise<AutonomousExecutionResult> {
    const maxProviderFailovers = autonomousProviderFailoverLimit(options);
    let contextBudget: AutonomousContextBudgetPlan | null = null;
    if (plan.contextBudget !== undefined) {
      const prepared = await compactAutonomousProviderRequest(plan.request, plan.contextBudget);
      plan = { ...plan, request: prepared.request };
      contextBudget = prepared.plan;
    }
    const initialSelection = await this.select(plan, { selectionEventCallback: options.selectionEventCallback, attempt: 1 });
    if (!initialSelection.selected_model) {
      if (selectionIsCredentialUnavailable(initialSelection.ranking)) throw new CredentialError("autonomous selection requires a user credential handle");
      throw new ProviderRuntimeError(`autonomous selection abstained: ${initialSelection.abstention_reason ?? "no model"}`);
    }
    const continuationPlan = await compileAutonomousModelContinuationPlan(plan, initialSelection, { maxFailovers: maxProviderFailovers });
    let continuationState: AutonomousModelContinuationState = await createAutonomousModelContinuationState(continuationPlan);
    const invocationSamples: AutonomousProviderInvocationSample[] = [];
    const executionId = options.execution?.state.execution_id ?? null;
    while (true) {
      const step = continuationPlan.steps[continuationState.next_step_index ?? -1];
      if (!step) throw new ProviderRuntimeError("autonomous continuation has no next model");
      const failovers = continuationState.failovers_used;
      const selection = failovers === 0 ? initialSelection : continuationSelectionDecision(initialSelection, step);
      if (failovers > 0) await emitContinuationSelectionTrace(options.selectionEventCallback, selection, continuationPlan, failovers + 1);
      const provider = step.provider;
      const credential = options.credential ?? options.credentialFor?.(provider);
      const observer: ProviderInvocationObserver = {
        before: options.observer?.before,
        after: async (metadata, outcome) => {
          try {
            await options.observer?.after?.(metadata, outcome);
            await options.feedback?.(selection, outcome);
          } finally {
            invocationSamples.push({
              executionId,
              metadata: { ...metadata },
              outcome: { ...outcome },
              attempt: failovers,
              turn: 0,
              selectionDigest,
              estimatedCostUnits,
              costPerMillionTokens: selectedCandidate?.cost_per_million_tokens ?? 0,
            });
          }
        },
      };
      const selectedCandidate = plan.candidates.find((candidate) => candidate.provider === provider && candidate.model === step.model);
      const estimatedCostUnits = estimatedProviderCostUnits(selectedCandidate, plan.request);
      const selectionDigest = await digestJson(selection);
      try {
        const response = await this.llm.invoke(provider, { ...plan.request, model: step.model }, { credential, signal: options.signal, observer, invocationKind: "autonomous_selected_model", execution: options.execution, executionAttempt: options.executionAttempt, executionTurn: 1, executionFailover: failovers > 0, selectionDigest, estimatedCostUnits, reserveCost: options.reserveCost });
        continuationState = await completeAutonomousModelContinuationState(continuationPlan, continuationState, { provider, model: step.model, statusCode: response.statusCode });
        const projection = await autonomousProviderInvocationProjection(invocationSamples, continuationPlan);
        return { selection, response, continuation_plan: continuationPlan, provider_invocations: projection.providerInvocations, provider_failover: projection.providerFailover, context_budget: contextBudget };
      } catch (error) {
        if (!(error instanceof ProviderRuntimeError) || !error.retryable || failovers >= maxProviderFailovers) throw error;
        const failureScope: AutonomousContinuationFailureScope = modelFailoverAllowed(error) ? "model" : "provider";
        continuationState = await advanceAutonomousModelContinuationState(continuationPlan, continuationState, { provider, model: step.model, failureScope, failureCode: error.code, statusCode: error.statusCode });
        if (continuationState.status !== "ready") throw error;
      }
    }
  }

  /**
   * Select a model and open a live provider-neutral stream.
   *
   * Selection, context compaction, continuation compilation, and provider admission all happen
   * before the handle is returned. Once an event has been observed, the stream is never replayed
   * onto another model: a partial assistant turn may contain a caller-visible tool intent. A
   * retry is therefore limited to a provider/model failure that occurs before the first event.
   */
  async invokeStream(
    plan: AutonomousExecutionPlan,
    options: AutonomousStreamInvocationOptions = {},
  ): Promise<AutonomousStreamHandle> {
    const maxProviderFailovers = autonomousProviderFailoverLimit(options);
    let contextBudget: AutonomousContextBudgetPlan | null = null;
    if (plan.contextBudget !== undefined) {
      const prepared = await compactAutonomousProviderRequest(plan.request, plan.contextBudget);
      plan = { ...plan, request: prepared.request };
      contextBudget = prepared.plan;
    }
    const initialSelection = await this.select(plan, {
      selectionEventCallback: options.selectionEventCallback,
      attempt: 1,
    });
    if (!initialSelection.selected_model) {
      if (selectionIsCredentialUnavailable(initialSelection.ranking)) throw new CredentialError("autonomous selection requires a user credential handle");
      throw new ProviderRuntimeError(`autonomous selection abstained: ${initialSelection.abstention_reason ?? "no model"}`);
    }
    const continuationPlan = await compileAutonomousModelContinuationPlan(plan, initialSelection, { maxFailovers: maxProviderFailovers });
    let completionResolver: ((completion: AutonomousStreamCompletion) => void) | undefined;
    const completion = new Promise<AutonomousStreamCompletion>((resolve) => { completionResolver = resolve; });
    const invocationSamples: AutonomousProviderInvocationSample[] = [];
    const executionId = options.execution?.state.execution_id ?? null;
    let eventCount = 0;
    let textDeltaBytes = 0;
    let doneSeen = false;
    let sawEvent = false;
    let finalized = false;
    let consumed = false;
    const runtime = this.llm;

    const finish = (status: AutonomousStreamCompletion["status"], error: unknown = null): void => {
      if (finalized) return;
      finalized = true;
      const errorCode = error instanceof ProviderRuntimeError ? error.code : null;
      const errorClass = error instanceof Error ? error.constructor.name : error === null ? null : "UnknownError";
      void autonomousProviderInvocationProjection(invocationSamples, continuationPlan).then((projection) => {
        completionResolver?.({
          schema: AUTONOMOUS_STREAM_COMPLETION_SCHEMA,
          status,
          event_count: eventCount,
          text_delta_bytes: textDeltaBytes,
          done_seen: doneSeen,
          provider_invocations: projection.providerInvocations,
          provider_failover: projection.providerFailover,
          error_code: errorCode,
          error_class: errorClass,
          retention: "metadata_only_no_stream_payloads_or_credentials",
          secret_material: "never_returned",
        });
      }).catch(() => {
        // Completion is a non-authoritative metadata receipt. Never turn a provider result into
        // an unhandled promise rejection because a local digest/evidence projection failed.
        completionResolver?.({
          schema: AUTONOMOUS_STREAM_COMPLETION_SCHEMA,
          status,
          event_count: eventCount,
          text_delta_bytes: textDeltaBytes,
          done_seen: doneSeen,
          provider_invocations: [],
          provider_failover: null,
          error_code: errorCode,
          error_class: errorClass,
          retention: "metadata_only_no_stream_payloads_or_credentials",
          secret_material: "never_returned",
        });
      });
    };

    const events: AsyncIterable<ProviderStreamEvent> = {
      [Symbol.asyncIterator]: async function* (): AsyncGenerator<ProviderStreamEvent> {
        if (consumed) throw new ArgumentError("autonomous stream handles are single-consumer");
        consumed = true;
        let continuationState: AutonomousModelContinuationState;
        try {
          continuationState = await createAutonomousModelContinuationState(continuationPlan);
          while (true) {
            const step = continuationPlan.steps[continuationState.next_step_index ?? -1];
            if (!step) throw new ProviderRuntimeError("autonomous stream continuation has no next model");
            const failovers = continuationState.failovers_used;
            const selection = failovers === 0 ? initialSelection : continuationSelectionDecision(initialSelection, step);
            if (failovers > 0) await emitContinuationSelectionTrace(options.selectionEventCallback, selection, continuationPlan, failovers + 1);
            const provider = step.provider;
            const credential = options.credential ?? options.credentialFor?.(provider);
            const selectedCandidate = plan.candidates.find((candidate) => candidate.provider === provider && candidate.model === step.model);
            const estimatedCostUnits = estimatedProviderCostUnits(selectedCandidate, plan.request);
            const selectionDigest = await digestJson(selection);
            const observer: ProviderInvocationObserver = {
              before: options.observer?.before,
              after: async (metadata, outcome) => {
                try {
                  await options.observer?.after?.(metadata, outcome);
                  await options.feedback?.(selection, outcome);
                } finally {
                  invocationSamples.push({
                    executionId,
                    metadata: { ...metadata },
                    outcome: { ...outcome },
                    attempt: failovers,
                    turn: 0,
                    selectionDigest,
                    estimatedCostUnits,
                    costPerMillionTokens: selectedCandidate?.cost_per_million_tokens ?? 0,
                  });
                }
              },
            };
            const attemptEventCount = eventCount;
            try {
              let localDone = false;
              for await (const event of runtime.invokeStream(provider, { ...plan.request, model: step.model }, {
                credential,
                signal: options.signal,
                observer,
                effectBoundary: options.effectBoundary,
                invocationKind: "autonomous_selected_model_stream",
                execution: options.execution,
                executionAttempt: options.executionAttempt,
                executionTurn: 1,
                executionFailover: failovers > 0,
                selectionDigest,
                estimatedCostUnits,
                reserveCost: options.reserveCost,
              })) {
                sawEvent = true;
                eventCount += 1;
                textDeltaBytes += bytes(event.textDelta);
                doneSeen = doneSeen || event.done;
                localDone = localDone || event.done;
                yield event;
              }
              if (!localDone) {
                throw new ProviderRuntimeError("autonomous provider stream ended without a done event", {
                  retryable: attemptEventCount === eventCount,
                  code: "invalid_response",
                });
              }
              continuationState = await completeAutonomousModelContinuationState(continuationPlan, continuationState, {
                provider,
                model: step.model,
                statusCode: null,
              });
              finish("completed");
              return;
            } catch (error) {
              // A stream can only fail over before its first event. This protects callers from
              // receiving a concatenation of two model answers or replayed tool intent.
              if (sawEvent || !(error instanceof ProviderRuntimeError) || !error.retryable || failovers >= maxProviderFailovers) {
                finish("failed", error);
                throw error;
              }
              const failureScope: AutonomousContinuationFailureScope = modelFailoverAllowed(error) ? "model" : "provider";
              continuationState = await advanceAutonomousModelContinuationState(continuationPlan, continuationState, {
                provider,
                model: step.model,
                failureScope,
                failureCode: error.code,
                statusCode: error.statusCode,
              });
              if (continuationState.status !== "ready") {
                finish("failed", error);
                throw error;
              }
            }
          }
        } catch (error) {
          if (!finalized) finish("failed", error);
          throw error;
        } finally {
          if (!finalized) finish("abandoned");
        }
      },
    };

    return { selection: initialSelection, continuation_plan: continuationPlan, context_budget: contextBudget, events, completion };
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
      selectionEventCallback?: AutonomousModelSelectionTraceEventCallback;
      execution?: AutonomousExecutionController;
      executionAttempt?: number;
      maxProviderFailovers?: number;
      reserveCost?: AutonomousCostReservationCallback;
      toolReadOnly?: (call: ProviderToolCall) => boolean | Promise<boolean>;
    },
  ): Promise<{ selection: AutonomousSelectionDecision; loop: ProviderToolLoopResult; continuation_plan: AutonomousModelContinuationPlan; provider_invocations: AutonomousProviderInvocationReceipt[]; provider_failover: AutonomousProviderFailoverProjection | null; context_budget?: AutonomousContextBudgetPlan | null }> {
    const maxProviderFailovers = autonomousProviderFailoverLimit(options);
    let contextBudget: AutonomousContextBudgetPlan | null = null;
    if (plan.contextBudget !== undefined) {
      const prepared = await compactAutonomousProviderRequest(plan.request, plan.contextBudget);
      plan = { ...plan, request: prepared.request };
      contextBudget = prepared.plan;
    }
    const initialSelection = await this.select(plan, { selectionEventCallback: options.selectionEventCallback, attempt: 1 });
    if (!initialSelection.selected_model) {
      if (selectionIsCredentialUnavailable(initialSelection.ranking)) throw new CredentialError("autonomous selection requires a user credential handle");
      throw new ProviderRuntimeError(`autonomous selection abstained: ${initialSelection.abstention_reason ?? "no model"}`);
    }
    const continuationPlan = await compileAutonomousModelContinuationPlan(plan, initialSelection, { maxFailovers: maxProviderFailovers });
    let continuationState: AutonomousModelContinuationState = await createAutonomousModelContinuationState(continuationPlan);
    const invocationSamples: AutonomousProviderInvocationSample[] = [];
    const executionId = options.execution?.state.execution_id ?? null;
    let toolActivity = false;
    while (true) {
      const step = continuationPlan.steps[continuationState.next_step_index ?? -1];
      if (!step) throw new ProviderRuntimeError("autonomous continuation has no next model");
      const failovers = continuationState.failovers_used;
      const selection = failovers === 0 ? initialSelection : continuationSelectionDecision(initialSelection, step);
      if (failovers > 0) await emitContinuationSelectionTrace(options.selectionEventCallback, selection, continuationPlan, failovers + 1);
      const provider = step.provider;
      const credential = options.credential ?? options.credentialFor?.(provider);
      const observer: ProviderInvocationObserver = {
        before: options.observer?.before,
        after: async (metadata, outcome) => {
          try {
            await options.observer?.after?.(metadata, outcome);
            await options.feedback?.(selection, outcome);
          } finally {
            invocationSamples.push({
              executionId,
              metadata: { ...metadata },
              outcome: { ...outcome },
              attempt: failovers,
              turn: invocationTurn,
              selectionDigest,
              estimatedCostUnits,
              costPerMillionTokens: selectedCandidate?.cost_per_million_tokens ?? 0,
            });
            invocationTurn += 1;
          }
        },
      };
      const selectedCandidate = plan.candidates.find((candidate) => candidate.provider === provider && candidate.model === step.model);
      const estimatedCostUnits = estimatedProviderCostUnits(selectedCandidate, plan.request);
      const selectionDigest = await digestJson(selection);
      let invocationTurn = 0;
      const authorizeAndExecute = async (calls: ProviderToolCall[]): Promise<ProviderToolResult[]> => {
        if (calls.length > 0) toolActivity = true;
        return options.authorizeAndExecute(calls);
      };
      try {
        const loop = await this.llm.invokeToolLoop(provider, { ...plan.request, model: step.model }, {
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
          contextBudget: plan.contextBudget,
        });
        continuationState = await completeAutonomousModelContinuationState(continuationPlan, continuationState, { provider, model: step.model, statusCode: loop.finalResponse?.statusCode ?? null });
        const projection = await autonomousProviderInvocationProjection(invocationSamples, continuationPlan);
        return { selection, loop, continuation_plan: continuationPlan, provider_invocations: projection.providerInvocations, provider_failover: projection.providerFailover, context_budget: contextBudget };
      } catch (error) {
        // Replaying a loop after any provider-issued tool call could duplicate an effect. A
        // failover is therefore permitted only before the first tool request is observed.
        if (toolActivity || !(error instanceof ProviderRuntimeError) || !error.retryable || failovers >= maxProviderFailovers) throw error;
        const failureScope: AutonomousContinuationFailureScope = modelFailoverAllowed(error) ? "model" : "provider";
        continuationState = await advanceAutonomousModelContinuationState(continuationPlan, continuationState, { provider, model: step.model, failureScope, failureCode: error.code, statusCode: error.statusCode });
        if (continuationState.status !== "ready") throw error;
      }
    }
  }

  private selectionRequest(plan: AutonomousExecutionPlan, excludedProviders: readonly string[] = [], excludedModels: readonly string[] = []): AutonomousSelectionRequest {
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
      min_selection_confidence: plan.minSelectionConfidence,
    });
    const weights = normalizeAutonomousSelectionWeights(plan.selectionWeights);
    const observations = normalizeAutonomousModelObservations(plan.selectionObservations);
    const excluded = new Set(excludedProviders.map((provider) => boundedIdentifier("excluded provider", provider, 128)));
    const excludedModelIds = new Set(excludedModels.map((modelId) => boundedText("excluded model", modelId, 768)));
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
    }).filter((candidate) => !excluded.has(candidate.provider) && !excludedModelIds.has(`${candidate.provider}/${candidate.model}`));
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
      estimated_input_tokens: Math.max(1, Math.ceil(plan.request.messages.reduce((sum, message) => sum + providerContentBytes(message.content, message.role), 0) / 4)),
      requested_output_tokens: plan.request.maxOutputTokens,
      max_cost_per_million_tokens: plan.maxCostPerMillionTokens ?? null,
      max_latency_ms: plan.maxLatencyMs ?? null,
      min_quality: plan.minQuality ?? null,
      min_selection_confidence: plan.minSelectionConfidence ?? null,
      require_json: plan.request.requireJson === true,
      weights,
      observations,
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

function validateSelectionConstraints(request: Pick<AutonomousSelectionRequest, "max_cost_per_million_tokens" | "max_latency_ms" | "min_quality" | "min_selection_confidence">): void {
  const constraints: Array<[string, unknown, number]> = [
    ["max_cost_per_million_tokens", request.max_cost_per_million_tokens, 1_000_000_000],
    ["max_latency_ms", request.max_latency_ms, 10 * 60_000],
    ["min_quality", request.min_quality, 1],
    ["min_selection_confidence", request.min_selection_confidence, 1],
  ];
  for (const [name, value, maximum] of constraints) {
    if (value === undefined || value === null) continue;
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) {
      throw new ProviderRuntimeError(`autonomous selection ${name} is outside its bounds`);
    }
  }
}

/** Return normalized separation of the top eligible ranks, never answer correctness. */
export function autonomousSelectionConfidence(ranking: readonly AutonomousModelRanking[]): number {
  const eligible = ranking.filter((row) => row.eligible).sort((left, right) => right.score - left.score || left.provider.localeCompare(right.provider) || left.model.localeCompare(right.model));
  if (eligible.length === 0) return 0;
  if (eligible.length === 1) return 1;
  const top = eligible[0] as AutonomousModelRanking;
  const runnerUp = eligible[1] as AutonomousModelRanking;
  return Math.max(0, Math.min(1, (top.score - runnerUp.score) / (1 + Math.abs(top.score) + Math.abs(runnerUp.score))));
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
  const weights = normalizeAutonomousSelectionWeights(request.weights);
  const observations = normalizeAutonomousModelObservations(request.observations);
  const observationByArm = new Map(observations.map((observation) => [observation.arm_id, observation]));
  const maxCost = Math.max(1, ...request.candidates.map((candidate) => candidate.cost_per_million_tokens));
  const effectiveMetrics = new Map(request.candidates.map((candidate) => {
    const armId = `${candidate.provider}/${candidate.model}`;
    const health = request.model_health[armId];
    const attempts = health?.attempts ?? 0;
    const evidence = attempts > 0 && typeof health?.success_rate === "number"
      ? { successRate: health.success_rate, latency: health.last_latency_ms ?? health.mean_latency_ms }
      : null;
    if (!evidence) return [armId, { reliability: candidate.reliability, latency: candidate.latency_ms }] as const;
    const confidence = Math.min(attempts / 12, 0.75);
    return [armId, {
      reliability: (1 - confidence) * candidate.reliability + confidence * evidence.successRate,
      latency: evidence.latency === null || evidence.latency === undefined
        ? candidate.latency_ms
        : (1 - confidence) * candidate.latency_ms + confidence * evidence.latency,
    }] as const;
  }));
  const maxLatency = Math.max(1, ...[...effectiveMetrics.values()].map((metrics) => metrics.latency));
  const totalPulls = observations.reduce((sum, observation) => sum + observation.pulls, 0);
  const logTotal = Math.log(totalPulls + 1);
  return request.candidates.map((candidate) => {
    const reasons: string[] = [];
    const provider = request.provider_health[candidate.provider];
    const armId = `${candidate.provider}/${candidate.model}`;
    const model = request.model_health[armId];
    const metrics = effectiveMetrics.get(armId) ?? { reliability: candidate.reliability, latency: candidate.latency_ms };
    const observation = observationByArm.get(armId);
    if (candidate.enabled === false) reasons.push("candidate disabled");
    if (!provider) reasons.push("provider not registered");
    if (provider?.registered === false) reasons.push("provider not registered");
    if (provider?.circuit === "open") reasons.push("provider circuit open");
    if (model?.circuit === "open") reasons.push("model circuit open");
    if (provider?.credential_required !== false && provider?.credential_ready !== true) reasons.push("credential not ready");
    if (provider?.eligible === false) reasons.push("provider health ineligible");
    if (observation?.disabled === true) reasons.push("disabled by learning policy");
    if (candidate.max_output_tokens < request.requested_output_tokens) reasons.push("model output capacity is below the request");
    if (candidate.context_window_tokens < request.estimated_input_tokens + request.requested_output_tokens) reasons.push("model context capacity is below the request");
    if (request.required_capabilities.some((required) => !(candidate.capabilities ?? []).includes(required))) reasons.push("model lacks a required capability");
    if (request.require_json === true && !(candidate.capabilities ?? []).includes("structured_output")) reasons.push("model lacks structured output capability");
    if (request.require_json === true && provider && provider.structured_output_mode === undefined) reasons.push("provider structured output capability is unknown");
    if (request.require_json === true && provider?.structured_output_mode === "disabled") reasons.push("provider structured output is disabled");
    if (request.max_cost_per_million_tokens !== undefined && request.max_cost_per_million_tokens !== null && candidate.cost_per_million_tokens > request.max_cost_per_million_tokens) reasons.push("model cost exceeds the caller budget");
    if (request.max_latency_ms !== undefined && request.max_latency_ms !== null && metrics.latency > request.max_latency_ms) reasons.push("model latency exceeds the caller bound");
    if (request.min_quality !== undefined && request.min_quality !== null && candidate.quality < request.min_quality) reasons.push("model quality is below the caller floor");
    const pulls = observation?.pulls ?? 0;
    const meanReward = pulls > 0 ? observation!.reward_sum / pulls : 0;
    const explorationBonus = pulls === 0
      ? weights.exploration
      : weights.exploration * Math.sqrt(logTotal / pulls);
    const baseScore = weights.quality * candidate.quality
      + weights.reliability * metrics.reliability
      + weights.exploration * meanReward
      - weights.cost * (candidate.cost_per_million_tokens / maxCost)
      - weights.latency * (metrics.latency / maxLatency);
    const score = baseScore + explorationBonus;
    return {
      provider: candidate.provider,
      model: candidate.model,
      score: Number(score.toFixed(12)),
      eligible: reasons.length === 0,
      reasons,
      base_score: Number(baseScore.toFixed(12)),
      exploration_bonus: Number(explorationBonus.toFixed(12)),
      observed_pulls: pulls,
    };
  }).sort((left, right) => Number(right.eligible) - Number(left.eligible) || right.score - left.score || left.provider.localeCompare(right.provider) || left.model.localeCompare(right.model));
}

function emptyHealth(): HealthState {
  return { attempts: 0, successes: 0, failures: 0, totalLatencyMs: 0, lastLatencyMs: null, lastModel: null, lastStatusCode: null };
}

function splitProviderModelArm(arm: string, providerNames: Iterable<string>): { provider: string; model: string } {
  const provider = [...providerNames].sort((left, right) => right.length - left.length || left.localeCompare(right)).find((candidate) => arm.startsWith(`${candidate}/`));
  if (provider) return { provider, model: arm.slice(provider.length + 1) };
  const separator = arm.indexOf("/");
  return { provider: separator < 0 ? arm : arm.slice(0, separator), model: separator < 0 ? arm : arm.slice(separator + 1) };
}

function healthProjection(provider: string, state: HealthState, circuit: "closed" | "open", consecutiveFailures: number, requiresCredential: boolean, inMemory = false): ProviderHealth {
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
    credential_posture: inMemory ? "caller_supplied_in_memory_handle" : "caller_supplied_opaque_handle",
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

export type ProviderFactoryOptions = Omit<ProviderConfig, "provider" | "protocol" | "baseUrl" | "transport"> & { baseUrl?: string };

export function openaiProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return { ...rest, provider: "openai", protocol: "openai_responses", baseUrl: baseUrl ?? "https://api.openai.com", modelsPath: rest.modelsPath ?? "/v1/models" };
}

export function anthropicProvider(options: ProviderFactoryOptions = {}): ProviderConfig {
  const { baseUrl, ...rest } = options;
  return { ...rest, provider: "anthropic", protocol: "anthropic_messages", baseUrl: baseUrl ?? "https://api.anthropic.com", modelsPath: rest.modelsPath ?? "/v1/models", structuredOutputMode: rest.structuredOutputMode ?? "disabled" };
}

export function openaiCompatibleProvider(provider: string, baseUrl: string, options: Omit<ProviderConfig, "provider" | "protocol" | "baseUrl" | "transport"> = {}): ProviderConfig {
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
