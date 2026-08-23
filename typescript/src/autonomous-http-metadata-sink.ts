import { ArgumentError, TransportError, isObject } from "./errors.js";
import {
  AutonomousConnectorObservation,
} from "./autonomous-connectors.js";
import {
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  createAutonomousHttpConnectorExecutor,
  type AutonomousHttpConnectorFetch,
  type AutonomousHttpConnectorHeaderResolver,
} from "./autonomous-http-connector.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { DomainEvidenceProviderConnectorManifest, JsonObject, JsonValue } from "./types.js";

/** HTTP exporter for already-redacted autonomous operational metadata. */
export const AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA = "bioprism-typescript-autonomous-http-metadata-sink/0.1" as const;
export const AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA = "bioprism-typescript-autonomous-http-metadata-event/0.1" as const;
export const AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-http-metadata-receipt/0.1" as const;
export const MAX_AUTONOMOUS_HTTP_METADATA_EVENT_BYTES = 24_000;
export const MAX_AUTONOMOUS_HTTP_METADATA_BATCH = 256;
export const MAX_AUTONOMOUS_HTTP_METADATA_RETRY_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_HTTP_METADATA_RETRY_DELAY_MS = 30_000;

const DEFAULT_RETRY_DELAY_MS = 250;
const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "body", "content", "credential", "credentials", "headers",
  "messages", "password", "privatekey", "prompt", "providerresponse", "response", "secret", "token",
  "toolarguments", "tooloutput", "value",
]);

export type AutonomousHttpMetadataSinkReceiptStatus = "exported" | "already_exported" | "refused" | "failed";

export interface AutonomousHttpMetadataSinkOptions {
  endpoint: string;
  acceptedSchemas?: readonly string[];
  sourceId?: string;
  policy?: AutonomousHttpConnectorPolicy;
  headerResolver?: AutonomousHttpConnectorHeaderResolver;
  fetch?: AutonomousHttpConnectorFetch;
  maxAttempts?: number;
  retryDelayMs?: number;
  sleep?: (milliseconds: number) => Promise<void>;
}

export interface AutonomousHttpMetadataSinkReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA;
  event_schema: string;
  event_digest: string;
  source_id: string;
  status: AutonomousHttpMetadataSinkReceiptStatus;
  attempts: number;
  status_code: number | null;
  failure_class: string | null;
  retryable: boolean;
  transport: "bounded_http_connector;idempotency_key_is_event_digest";
  retention: "metadata_only_event_identity_and_delivery_status";
  secret_material: "never_returned";
}

export interface AutonomousHttpMetadataSinkDescription extends JsonObject {
  schema: typeof AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA;
  source_id: string;
  endpoint_host: string;
  accepted_schemas: string[];
  max_attempts: number;
  retry_delay_ms: number;
  idempotency: "event_digest;collector_409_is_already_exported";
  transport: "bounded_http_connector;caller_header_resolver";
  retention: "metadata_only;event_payload_must_be_pre_redacted";
  secret_material: "never_returned";
}

export interface AutonomousHttpMetadataSinkBatchResult extends JsonObject {
  schema: typeof AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA;
  source_id: string;
  requested: number;
  exported: number;
  already_exported: number;
  refused: number;
  failed: number;
  receipts: AutonomousHttpMetadataSinkReceipt[];
  batch_digest: string;
  retention: "metadata_only_event_identity_and_delivery_status";
  secret_material: "never_returned";
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} is not a safe identifier`);
  return text;
}

function normalizedKey(value: string): string {
  return [...value.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
}

function secretFreeMetadata(name: string, value: unknown, maximum: number, depth = 0): JsonValue {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) {
    if (value.length > MAX_AUTONOMOUS_HTTP_METADATA_BATCH) throw new ArgumentError(`${name} array exceeds its bound`);
    const result = value.map((item) => secretFreeMetadata(name, item, maximum, depth + 1));
    if (bytes(canonicalJson(result)) > maximum) throw new ArgumentError(`${name} exceeds its byte bound`);
    return result;
  }
  if (!isObject(value)) throw new ArgumentError(`${name} must be JSON-safe`);
  const result: JsonObject = {};
  for (const [key, child] of Object.entries(value)) {
    const marker = normalizedKey(key);
    if (SECRET_FIELD_MARKERS.has(marker) || marker.startsWith("gsk") || marker.startsWith("skproj")) throw new ArgumentError(`${name} contains a transient or credential-shaped field`);
    if (child === undefined) throw new ArgumentError(`${name} contains an undefined field`);
    result[key] = secretFreeMetadata(`${name}.${key}`, child, maximum, depth + 1);
  }
  if (bytes(canonicalJson(result)) > maximum) throw new ArgumentError(`${name} exceeds its byte bound`);
  return result;
}

function responseMetadata(observation: AutonomousConnectorObservation): { statusCode: number | null; failure: string | null; retryable: boolean } {
  const value = isObject(observation.value) ? observation.value : null;
  const statusCode = typeof value?.status_code === "number" && Number.isSafeInteger(value.status_code) ? value.status_code : null;
  const failure = observation.failure_class ?? null;
  const retryable = failure === "rate_limited" || failure === "timeout" || failure === "transport_error" || failure === "http_5xx";
  return { statusCode, failure, retryable };
}

function manifest(): DomainEvidenceProviderConnectorManifest {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "autonomous.http.metadata-sink",
    version: "0.1.0",
    provider: "caller-http",
    connector_kind: "provider_api",
    domains: ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"],
    capabilities: ["metadata_event_export"],
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: [],
      does_not_claim: ["collector authorization is valid", "collector storage is durable", "metadata is task truth"],
    },
  };
}

function receipt(options: { eventSchema: string; eventDigest: string; sourceId: string; status: AutonomousHttpMetadataSinkReceiptStatus; attempts: number; statusCode: number | null; failure: string | null; retryable: boolean }): AutonomousHttpMetadataSinkReceipt {
  return {
    schema: AUTONOMOUS_HTTP_METADATA_SINK_RECEIPT_SCHEMA,
    event_schema: options.eventSchema,
    event_digest: options.eventDigest,
    source_id: options.sourceId,
    status: options.status,
    attempts: options.attempts,
    status_code: options.statusCode,
    failure_class: options.failure,
    retryable: options.retryable,
    transport: "bounded_http_connector;idempotency_key_is_event_digest",
    retention: "metadata_only_event_identity_and_delivery_status",
    secret_material: "never_returned",
  };
}

/**
 * Emits only pre-redacted trace/receipt metadata to a caller-owned HTTP collector.
 * A collector must treat `X-Aurora-Event-Digest` as the idempotency key and return 409 when that
 * exact event was already accepted. The sink never sends credentials, raw provider output, task
 * text, prompts, evidence values, tool arguments, or HTTP response bodies.
 */
export class AutonomousHttpMetadataEventSink {
  readonly endpoint: string;
  readonly acceptedSchemas: readonly string[];
  readonly sourceId: string;
  readonly policy: AutonomousHttpConnectorPolicy;
  readonly maxAttempts: number;
  readonly retryDelayMs: number;
  readonly sleep: (milliseconds: number) => Promise<void>;
  private readonly execute: ReturnType<typeof createAutonomousHttpConnectorExecutor>;
  private readonly collectorManifest: DomainEvidenceProviderConnectorManifest;

  constructor(options: AutonomousHttpMetadataSinkOptions) {
    if (!options || typeof options !== "object") throw new ArgumentError("HTTP metadata sink options are malformed");
    this.endpoint = boundedText("HTTP metadata sink endpoint", options.endpoint, 8_192);
    if (/\s/.test(this.endpoint)) throw new ArgumentError("HTTP metadata sink endpoint contains whitespace");
    const schemas = options.acceptedSchemas ?? [
      "bioprism-typescript-autonomous-run-trace-event/0.1",
      "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-event/0.1",
    ];
    if (!Array.isArray(schemas) || schemas.length < 1 || schemas.length > 32) throw new ArgumentError("HTTP metadata sink acceptedSchemas is outside its bound");
    this.acceptedSchemas = schemas.map((schema) => boundedText("HTTP metadata sink accepted schema", schema, 256));
    if (new Set(this.acceptedSchemas).size !== this.acceptedSchemas.length) throw new ArgumentError("HTTP metadata sink acceptedSchemas contains duplicates");
    this.sourceId = identifier("HTTP metadata sink sourceId", options.sourceId ?? "aurora-autonomous-runtime");
    this.policy = options.policy ?? new AutonomousHttpConnectorPolicy({ allowedMethods: ["POST"] });
    if (!(this.policy instanceof AutonomousHttpConnectorPolicy) || !this.policy.allowedMethods.includes("POST")) throw new ArgumentError("HTTP metadata sink policy must allow POST");
    this.maxAttempts = options.maxAttempts ?? 3;
    if (!Number.isSafeInteger(this.maxAttempts) || this.maxAttempts < 1 || this.maxAttempts > MAX_AUTONOMOUS_HTTP_METADATA_RETRY_ATTEMPTS) throw new ArgumentError("HTTP metadata sink maxAttempts is outside its bound");
    this.retryDelayMs = options.retryDelayMs ?? DEFAULT_RETRY_DELAY_MS;
    if (!Number.isSafeInteger(this.retryDelayMs) || this.retryDelayMs < 0 || this.retryDelayMs > MAX_AUTONOMOUS_HTTP_METADATA_RETRY_DELAY_MS) throw new ArgumentError("HTTP metadata sink retryDelayMs is outside its bound");
    this.sleep = options.sleep ?? ((milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)));
    if (typeof this.sleep !== "function") throw new ArgumentError("HTTP metadata sink sleep must be callable");
    this.collectorManifest = manifest();
    this.execute = createAutonomousHttpConnectorExecutor(
      (_manifest, request) => new AutonomousHttpConnectorRequest({ method: "POST", url: this.endpoint, body: request }),
      {
        policy: this.policy,
        fetch: options.fetch,
        headerResolver: options.headerResolver,
      },
    );
  }

  describe(): AutonomousHttpMetadataSinkDescription {
    let host = "unknown";
    try { host = new URL(this.endpoint).hostname; } catch { /* constructor leaves URL policy to the connector at dispatch */ }
    return {
      schema: AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA,
      source_id: this.sourceId,
      endpoint_host: host,
      accepted_schemas: [...this.acceptedSchemas],
      max_attempts: this.maxAttempts,
      retry_delay_ms: this.retryDelayMs,
      idempotency: "event_digest;collector_409_is_already_exported",
      transport: "bounded_http_connector;caller_header_resolver",
      retention: "metadata_only;event_payload_must_be_pre_redacted",
      secret_material: "never_returned",
    };
  }

  async emit(event: JsonObject): Promise<AutonomousHttpMetadataSinkReceipt> {
    const safeEvent = secretFreeMetadata("HTTP metadata sink event", event, MAX_AUTONOMOUS_HTTP_METADATA_EVENT_BYTES);
    if (!isObject(safeEvent) || typeof safeEvent.schema !== "string" || !this.acceptedSchemas.includes(safeEvent.schema)) throw new ArgumentError("HTTP metadata sink event schema is not accepted");
    const eventSchema = safeEvent.schema;
    const eventDigest = digestJsonSync(safeEvent);
    const request: JsonObject = {
      schema: AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA,
      source_id: this.sourceId,
      event: safeEvent,
      event_digest: eventDigest,
      idempotency_key: eventDigest,
      retention: "metadata_only_event_identity_and_delivery_status",
      secret_material: "never_returned",
    };
    if (bytes(canonicalJson(request)) > this.policy.maxRequestBytes) throw new ArgumentError("HTTP metadata sink request exceeds its bound");
    for (let attempt = 1; attempt <= this.maxAttempts; attempt += 1) {
      const observation = await this.execute(this.collectorManifest, request);
      if (!(observation instanceof AutonomousConnectorObservation)) throw new ArgumentError("HTTP metadata sink transport returned an invalid observation");
      const metadata = responseMetadata(observation);
      if (observation.status === "observed") return receipt({ eventSchema, eventDigest, sourceId: this.sourceId, status: "exported", attempts: attempt, statusCode: metadata.statusCode, failure: null, retryable: false });
      if (metadata.statusCode === 409) return receipt({ eventSchema, eventDigest, sourceId: this.sourceId, status: "already_exported", attempts: attempt, statusCode: metadata.statusCode, failure: "already_exists", retryable: false });
      const current = receipt({ eventSchema, eventDigest, sourceId: this.sourceId, status: observation.status === "refused" ? "refused" : "failed", attempts: attempt, statusCode: metadata.statusCode, failure: metadata.failure, retryable: metadata.retryable });
      if (!metadata.retryable || attempt >= this.maxAttempts) return current;
      await this.sleep(this.retryDelayMs * (2 ** (attempt - 1)));
    }
    throw new TransportError("HTTP metadata sink exhausted its bounded retry attempts");
  }

  async emitBatch(events: readonly JsonObject[]): Promise<AutonomousHttpMetadataSinkBatchResult> {
    if (!Array.isArray(events) || events.length < 1 || events.length > MAX_AUTONOMOUS_HTTP_METADATA_BATCH) throw new ArgumentError("HTTP metadata sink batch is outside its bound");
    const receipts: AutonomousHttpMetadataSinkReceipt[] = [];
    for (const event of events) receipts.push(await this.emit(event));
    const counts = {
      exported: receipts.filter((item) => item.status === "exported").length,
      already_exported: receipts.filter((item) => item.status === "already_exported").length,
      refused: receipts.filter((item) => item.status === "refused").length,
      failed: receipts.filter((item) => item.status === "failed").length,
    };
    const body = { schema: AUTONOMOUS_HTTP_METADATA_SINK_SCHEMA, source_id: this.sourceId, requested: receipts.length, ...counts, receipts, retention: "metadata_only_event_identity_and_delivery_status" as const, secret_material: "never_returned" as const };
    return { ...body, batch_digest: digestJsonSync(body) };
  }

  asSink(): (event: JsonObject) => Promise<void> {
    return async (event) => {
      const result = await this.emit(event);
      if (result.status !== "exported" && result.status !== "already_exported") throw new TransportError(`HTTP metadata sink refused event export: ${result.failure_class ?? result.status}`);
    };
  }
}
