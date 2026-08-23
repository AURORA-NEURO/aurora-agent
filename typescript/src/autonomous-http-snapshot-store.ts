import { ArgumentError, ProtocolError, ResponseTooLargeError, TransportError, isObject } from "./errors.js";
import { canonicalJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Provider-neutral HTTP transport for the SDK's metadata-only JSON text stores.
 *
 * The store intentionally knows nothing about jobs, goals, learning, or evidence. Those higher
 * layers validate their own schemas before handing canonical JSON here. This adapter owns only
 * bounded transport, endpoint policy, timeout/cancellation, and compare-and-swap headers. A
 * caller-supplied header resolver may close over a short-lived auth session; the resolver output
 * is used for one request and is never returned, persisted, or included in an error.
 */
export const AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA = "bioprism-typescript-autonomous-http-snapshot-store/0.1" as const;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESOURCE_BYTES = 512;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES = 4_000_000;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES = 4_000_000;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_TIMEOUT_MS = 120_000;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_COUNT = 64;
export const MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_BYTES = 8_192;

const DEFAULT_TIMEOUT_MS = 30_000;
const SAFE_METHODS = ["GET", "PUT"] as const;
const OPERATIONS = ["read", "write", "write_if_unchanged"] as const;

export type AutonomousHttpSnapshotStoreOperation = typeof OPERATIONS[number];

export interface AutonomousHttpSnapshotStoreHeaderContext extends JsonObject {
  operation: AutonomousHttpSnapshotStoreOperation;
  resource: string;
  expected_snapshot_digest: string | null;
}

export type AutonomousHttpSnapshotStoreHeaderResolver = (
  context: AutonomousHttpSnapshotStoreHeaderContext,
) => Record<string, string> | Promise<Record<string, string>>;

export interface AutonomousHttpSnapshotStorePolicy {
  allowedHosts?: readonly string[];
  requireHttps?: boolean;
  allowLoopback?: boolean;
  timeoutMs?: number;
  maxRequestBytes?: number;
  maxResponseBytes?: number;
}

export interface AutonomousHttpSnapshotStoreOptions extends AutonomousHttpSnapshotStorePolicy {
  endpoint: string | URL;
  resource: string;
  fetch?: AutonomousHttpSnapshotStoreFetch;
  headerResolver?: AutonomousHttpSnapshotStoreHeaderResolver;
  signal?: AbortSignal;
}

export type AutonomousHttpSnapshotStoreFetch = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface AutonomousHttpSnapshotStoreDescription extends JsonObject {
  schema: typeof AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA;
  resource: string;
  host: string;
  scheme: "http" | "https";
  require_https: boolean;
  allow_loopback: boolean;
  timeout_ms: number;
  max_request_bytes: number;
  max_response_bytes: number;
  cas: "if_match_digest_or_if_none_match_star";
  credentials: "transient_header_resolver;never_returned";
  retention: "metadata_only;caller_schema_validation_required";
  secret_material: "never_returned";
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function isLoopback(host: string): boolean {
  const normalized = host.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (normalized === "localhost" || normalized === "::1") return true;
  const octets = normalized.split(".");
  return octets.length === 4 && octets[0] === "127" && octets.slice(1).every((part) => /^\d+$/.test(part) && Number(part) <= 255);
}

function normalizeHost(host: string): string {
  return boundedText("HTTP snapshot store allowed host", host, 512).toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
}

function hostAllowed(host: string, allowedHosts: readonly string[], allowLoopback: boolean): boolean {
  const normalized = host.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (allowLoopback && isLoopback(normalized)) return true;
  return allowedHosts.some((allowed) => allowed.startsWith("*.")
    ? normalized.endsWith(allowed.slice(1)) && normalized !== allowed.slice(2)
    : normalized === allowed);
}

function validateEndpoint(endpoint: string | URL, allowedHosts: readonly string[], requireHttps: boolean, allowLoopback: boolean): URL {
  let parsed: URL;
  try {
    parsed = endpoint instanceof URL ? new URL(endpoint.toString()) : new URL(boundedText("HTTP snapshot store endpoint", endpoint, 8_192));
  } catch {
    throw new ArgumentError("HTTP snapshot store endpoint is not a valid URL");
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") throw new ArgumentError("HTTP snapshot store endpoint must use HTTP or HTTPS");
  if (requireHttps && parsed.protocol !== "https:" && !(allowLoopback && parsed.protocol === "http:" && isLoopback(parsed.hostname))) throw new ArgumentError("HTTP snapshot store endpoint must use HTTPS unless loopback development is explicitly enabled");
  if (parsed.username || parsed.password || parsed.hash) throw new ArgumentError("HTTP snapshot store endpoint cannot contain credentials or a fragment");
  if (!hostAllowed(parsed.hostname, allowedHosts, allowLoopback)) throw new ArgumentError("HTTP snapshot store endpoint host is outside its allow-list");
  return parsed;
}

function normalizeHeaders(value: Record<string, string>): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).length > MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_COUNT) throw new ArgumentError("HTTP snapshot store headers are outside their bound");
  const normalized: Record<string, string> = {};
  for (const [rawName, rawValue] of Object.entries(value)) {
    const name = boundedText("HTTP snapshot store header name", rawName, 256);
    if (/\s|:|\r|\n/.test(name)) throw new ArgumentError("HTTP snapshot store header name is unsafe");
    const text = boundedText("HTTP snapshot store header value", rawValue, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_HEADER_BYTES);
    if (/\r|\n/.test(text)) throw new ArgumentError("HTTP snapshot store header value is unsafe");
    for (const existing of Object.keys(normalized)) if (existing.toLowerCase() === name.toLowerCase()) delete normalized[existing];
    normalized[name] = text;
  }
  return normalized;
}

function assertDigest(name: string, value: string | null): void {
  if (value !== null && !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
}

async function boundedResponseText(response: Response, maximum: number): Promise<string> {
  const declared = response.headers.get("content-length");
  if (declared !== null && /^\d+$/.test(declared) && Number(declared) > maximum) throw new ResponseTooLargeError(maximum);
  if (response.body && typeof response.body.getReader === "function") {
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let total = 0;
    try {
      while (true) {
        const next = await reader.read();
        if (next.done) break;
        const chunk = next.value instanceof Uint8Array ? next.value : new Uint8Array(next.value);
        total += chunk.byteLength;
        if (total > maximum) {
          await reader.cancel();
          throw new ResponseTooLargeError(maximum);
        }
        chunks.push(chunk);
      }
    } catch (error) {
      try { await reader.cancel(); } catch { /* preserve the bounded transport failure */ }
      throw error;
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return new TextDecoder().decode(bytes);
  }
  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > maximum) throw new ResponseTooLargeError(maximum);
  return text;
}

function validateSnapshotText(value: string, maximum: number): string {
  if (typeof value !== "string" || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError("HTTP snapshot store snapshot exceeds its request bound");
  let parsed: unknown;
  try { parsed = JSON.parse(value); } catch { throw new ArgumentError("HTTP snapshot store snapshot must be valid JSON"); }
  if (!isObject(parsed)) throw new ArgumentError("HTTP snapshot store snapshot must be a JSON object");
  if (canonicalJson(parsed) !== value) throw new ArgumentError("HTTP snapshot store snapshot must use canonical JSON");
  return value;
}

function responseError(operation: AutonomousHttpSnapshotStoreOperation, status: number): TransportError {
  const retryable = status === 408 || status === 425 || status === 429 || status >= 500;
  return new TransportError(`HTTP snapshot store ${operation} returned status ${status}${retryable ? " (retryable)" : ""}`);
}

/**
 * A portable remote text store for any strict metadata snapshot persistence adapter.
 *
 * The server contract is deliberately small: GET returns 200 with canonical JSON or 404 when
 * absent; PUT returns any 2xx on success; conditional PUT returns 409/412 for a CAS miss. The
 * expected digest is sent as an HTTP validator, never as a persisted payload field.
 */
export class AutonomousHttpSnapshotTextStore {
  readonly endpoint: URL;
  readonly resource: string;
  readonly requireHttps: boolean;
  readonly allowLoopback: boolean;
  readonly timeoutMs: number;
  readonly maxRequestBytes: number;
  readonly maxResponseBytes: number;
  readonly headerResolver?: AutonomousHttpSnapshotStoreHeaderResolver;
  readonly fetch: AutonomousHttpSnapshotStoreFetch;
  readonly signal?: AbortSignal;
  private readonly allowedHosts: readonly string[];

  constructor(options: AutonomousHttpSnapshotStoreOptions) {
    if (!options || typeof options !== "object") throw new ArgumentError("HTTP snapshot store options are malformed");
    this.requireHttps = options.requireHttps ?? true;
    this.allowLoopback = options.allowLoopback ?? false;
    if (typeof this.requireHttps !== "boolean" || typeof this.allowLoopback !== "boolean") throw new ArgumentError("HTTP snapshot store HTTPS policy must be boolean");
    this.allowedHosts = (options.allowedHosts ?? []).map(normalizeHost);
    if (this.allowedHosts.length > 128) throw new ArgumentError("HTTP snapshot store allowed hosts exceed their bound");
    this.endpoint = validateEndpoint(options.endpoint, this.allowedHosts, this.requireHttps, this.allowLoopback);
    this.resource = boundedText("HTTP snapshot store resource", options.resource, MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESOURCE_BYTES);
    if (!/^[A-Za-z0-9_.:/+-]+$/.test(this.resource)) throw new ArgumentError("HTTP snapshot store resource contains unsafe characters");
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    if (!Number.isSafeInteger(this.timeoutMs) || this.timeoutMs < 100 || this.timeoutMs > MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_TIMEOUT_MS) throw new ArgumentError("HTTP snapshot store timeoutMs is outside its bound");
    this.maxRequestBytes = options.maxRequestBytes ?? MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES;
    this.maxResponseBytes = options.maxResponseBytes ?? MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES;
    if (!Number.isSafeInteger(this.maxRequestBytes) || this.maxRequestBytes < 1 || this.maxRequestBytes > MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_REQUEST_BYTES) throw new ArgumentError("HTTP snapshot store maxRequestBytes is outside its bound");
    if (!Number.isSafeInteger(this.maxResponseBytes) || this.maxResponseBytes < 1 || this.maxResponseBytes > MAX_AUTONOMOUS_HTTP_SNAPSHOT_STORE_RESPONSE_BYTES) throw new ArgumentError("HTTP snapshot store maxResponseBytes is outside its bound");
    if (options.fetch !== undefined && typeof options.fetch !== "function") throw new ArgumentError("HTTP snapshot store fetch must be callable");
    if (options.headerResolver !== undefined && typeof options.headerResolver !== "function") throw new ArgumentError("HTTP snapshot store headerResolver must be callable");
    const fetcher = options.fetch ?? globalThis.fetch;
    if (typeof fetcher !== "function") throw new ArgumentError("HTTP snapshot store requires a fetch implementation");
    this.fetch = options.fetch ?? fetcher.bind(globalThis);
    this.headerResolver = options.headerResolver;
    this.signal = options.signal;
  }

  describe(): AutonomousHttpSnapshotStoreDescription {
    return {
      schema: AUTONOMOUS_HTTP_SNAPSHOT_STORE_SCHEMA,
      resource: this.resource,
      host: this.endpoint.hostname,
      scheme: this.endpoint.protocol === "https:" ? "https" : "http",
      require_https: this.requireHttps,
      allow_loopback: this.allowLoopback,
      timeout_ms: this.timeoutMs,
      max_request_bytes: this.maxRequestBytes,
      max_response_bytes: this.maxResponseBytes,
      cas: "if_match_digest_or_if_none_match_star",
      credentials: "transient_header_resolver;never_returned",
      retention: "metadata_only;caller_schema_validation_required",
      secret_material: "never_returned",
    };
  }

  async read(): Promise<string | null> {
    const result = await this.request("read", "GET", null, null);
    if (result.status === 404) return null;
    if (result.status < 200 || result.status >= 300) throw responseError("read", result.status);
    return validateSnapshotText(result.body, this.maxResponseBytes);
  }

  async write(value: string): Promise<void> {
    validateSnapshotText(value, this.maxRequestBytes);
    const result = await this.request("write", "PUT", value, null);
    if (result.status < 200 || result.status >= 300) throw responseError("write", result.status);
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> {
    assertDigest("HTTP snapshot store expected snapshot digest", expectedSnapshotDigest);
    validateSnapshotText(value, this.maxRequestBytes);
    const result = await this.request("write_if_unchanged", "PUT", value, expectedSnapshotDigest);
    if (result.status === 409 || result.status === 412) {
      return false;
    }
    if (result.status < 200 || result.status >= 300) throw responseError("write_if_unchanged", result.status);
    return true;
  }

  private async request(operation: AutonomousHttpSnapshotStoreOperation, method: "GET" | "PUT", body: string | null, expectedSnapshotDigest: string | null): Promise<{ status: number; body: string }> {
    if (!SAFE_METHODS.includes(method)) throw new ArgumentError("HTTP snapshot store method is unsupported");
    const context: AutonomousHttpSnapshotStoreHeaderContext = { operation, resource: this.resource, expected_snapshot_digest: expectedSnapshotDigest };
    let resolvedHeaders: Record<string, string> = {};
    if (this.headerResolver) {
      const value = await this.headerResolver(context);
      resolvedHeaders = normalizeHeaders(value);
    }
    const headers = new Headers({ accept: "application/json", ...resolvedHeaders });
    headers.set("x-aurora-snapshot-resource", this.resource);
    if (body !== null) {
      headers.set("content-type", "application/json");
      if (new TextEncoder().encode(body).byteLength > this.maxRequestBytes) throw new ArgumentError("HTTP snapshot store request exceeds its byte bound");
    }
    if (operation === "write_if_unchanged") {
      if (expectedSnapshotDigest === null) headers.set("if-none-match", "*");
      else headers.set("if-match", `\"${expectedSnapshotDigest}\"`);
    }
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      controller.abort();
    }, this.timeoutMs);
    const abort = () => controller.abort();
    if (this.signal?.aborted) controller.abort();
    else this.signal?.addEventListener("abort", abort, { once: true });
    try {
      let response: Response;
      try {
        response = await this.fetch(this.endpoint, { method, headers, body, redirect: "error", signal: controller.signal });
      } catch (error) {
        if (timedOut) throw new TransportError(`HTTP snapshot store ${operation} timed out`);
        if (this.signal?.aborted) throw new TransportError(`HTTP snapshot store ${operation} was aborted`);
        throw new TransportError(`HTTP snapshot store ${operation} transport failed`, error);
      }
      if (!response || typeof response.status !== "number" || !response.headers) throw new ProtocolError("HTTP snapshot store returned a malformed response");
      let responseBody: string;
      try {
        responseBody = await boundedResponseText(response, this.maxResponseBytes);
      } catch (error) {
        if (error instanceof ResponseTooLargeError) throw error;
        throw new TransportError(`HTTP snapshot store ${operation} response could not be read`, error);
      }
      return { status: response.status, body: responseBody };
    } finally {
      clearTimeout(timer);
      this.signal?.removeEventListener("abort", abort);
    }
  }
}

export type AutonomousHttpSnapshotTextStoreDescription = AutonomousHttpSnapshotStoreDescription;
