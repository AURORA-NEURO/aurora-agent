/**
 * Policy-gated HTTP transport for caller-managed autonomous connectors.
 *
 * The autonomous connector registry intentionally does not discover providers or retain keys.
 * This adapter supplies a bounded transport seam: an embedding provides an endpoint resolver and,
 * when needed, a transient header resolver that closes over its own credential session. The adapter
 * enforces an explicit host/scheme/method policy, bounded request and response bytes, no redirects,
 * and a finite timeout. It returns only JSON evidence or a digest/status projection; raw headers,
 * credentials, and response bytes never enter receipts.
 */

import { ArgumentError, isObject } from "./errors.js";
import { AutonomousConnectorObservation } from "./autonomous-connectors.js";
import { canonicalJson, digestBytesSync } from "./tooling.js";
import type { AutonomousConnectorExecutor } from "./autonomous-connectors.js";
import type { DomainEvidenceProviderConnectorManifest, JsonObject, JsonValue } from "./types.js";

export const AUTONOMOUS_HTTP_CONNECTOR_ADAPTER_SCHEMA = "bioprism-typescript-autonomous-http-connector-adapter/0.1" as const;
export const MAX_AUTONOMOUS_HTTP_REQUEST_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_HTTP_HEADERS = 64;
export const MAX_AUTONOMOUS_HTTP_HEADER_BYTES = 8_192;
export const MAX_AUTONOMOUS_HTTP_URL_BYTES = 8_192;
export const MAX_AUTONOMOUS_HTTP_TIMEOUT_MS = 120_000;
export const AUTONOMOUS_HTTP_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE"] as const;
export const AUTONOMOUS_HTTP_FAILURE_CLASSES = [
  "auth_refused",
  "not_found",
  "rate_limited",
  "timeout",
  "transport_error",
  "http_4xx",
  "http_5xx",
  "invalid_json",
  "response_too_large",
] as const;

const SECRET_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);
const MAX_JSON_DEPTH = 32;

function normalizedField(value: string): string {
  return [...value.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
}

function containsSecretField(value: string): boolean {
  const normalized = normalizedField(value);
  return SECRET_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj");
}

function safeJson(value: unknown, name: string, maximum: number, depth = 0): JsonValue {
  if (depth > MAX_JSON_DEPTH) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
    return value;
  }
  if (Array.isArray(value)) {
    const result = value.map((item) => safeJson(item, name, maximum, depth + 1));
    if (new TextEncoder().encode(canonicalJson(result)).byteLength > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  if (isObject(value)) {
    const result: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\0") || containsSecretField(key)) throw new ArgumentError(`${name} contains credential-shaped fields`);
      if (child === undefined) throw new ArgumentError(`${name} contains an undefined field`);
      result[key] = safeJson(child, name, maximum, depth + 1);
    }
    if (new TextEncoder().encode(canonicalJson(result)).byteLength > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  throw new ArgumentError(`${name} must be JSON-safe`);
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\0") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedHeader(name: string, value: unknown): string {
  const text = boundedText(name, value, MAX_AUTONOMOUS_HTTP_HEADER_BYTES);
  if (text.includes("\r") || text.includes("\n")) throw new ArgumentError(`${name} contains an unsafe header value`);
  return text;
}

function normalizeHeaders(name: string, value: unknown): Record<string, string> {
  if (!isObject(value) || Object.keys(value).length > MAX_AUTONOMOUS_HTTP_HEADERS) {
    throw new ArgumentError(`${name} are outside their bound`);
  }
  const result: Record<string, string> = {};
  for (const [rawName, rawValue] of Object.entries(value)) {
    const headerName = boundedText(`${name} name`, rawName, 256);
    if (/\s|:|\r|\n/.test(headerName)) throw new ArgumentError(`${name} name is unsafe`);
    for (const existing of Object.keys(result)) if (existing.toLowerCase() === headerName.toLowerCase()) delete result[existing];
    result[headerName] = boundedHeader(`${name} value`, rawValue);
  }
  return result;
}

function isLoopback(host: string): boolean {
  const normalized = host.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (normalized === "localhost" || normalized === "::1") return true;
  const octets = normalized.split(".");
  return octets.length === 4 && octets[0] === "127" && octets.slice(1).every((part) => /^\d+$/.test(part) && Number(part) <= 255);
}

function hostAllowed(host: string, allowedHosts: readonly string[], allowLoopback: boolean): boolean {
  const normalized = host.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (allowLoopback && isLoopback(normalized)) return true;
  return allowedHosts.some((allowed) => allowed.startsWith("*.")
    ? normalized.endsWith(allowed.slice(1)) && normalized !== allowed.slice(2)
    : normalized === allowed);
}

export class AutonomousHttpConnectorPolicy {
  readonly allowedHosts: readonly string[];
  readonly requireHttps: boolean;
  readonly allowLoopback: boolean;
  readonly timeoutMs: number;
  readonly maxRequestBytes: number;
  readonly maxResponseBytes: number;
  readonly allowedMethods: readonly string[];

  constructor(input: {
    allowedHosts?: readonly string[];
    requireHttps?: boolean;
    allowLoopback?: boolean;
    timeoutMs?: number;
    maxRequestBytes?: number;
    maxResponseBytes?: number;
    allowedMethods?: readonly string[];
  } = {}) {
    const hosts = input.allowedHosts ?? [];
    if (!Array.isArray(hosts) || hosts.length > 128) throw new ArgumentError("HTTP connector allowedHosts is outside its bound");
    this.allowedHosts = hosts.map((host) => {
      const normalized = boundedText("HTTP connector allowed host", host, 512).toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
      if (normalized.includes("://") || normalized.includes("/") || normalized.includes("@") || (normalized.startsWith("*.") && normalized.length <= 2)) {
        throw new ArgumentError("HTTP connector allowed host must not contain a scheme or path");
      }
      return normalized;
    });
    this.requireHttps = input.requireHttps ?? true;
    this.allowLoopback = input.allowLoopback ?? false;
    if (typeof this.requireHttps !== "boolean" || typeof this.allowLoopback !== "boolean") throw new ArgumentError("HTTP connector scheme policy must be boolean");
    this.timeoutMs = input.timeoutMs ?? 30_000;
    if (!Number.isFinite(this.timeoutMs) || this.timeoutMs < 100 || this.timeoutMs > MAX_AUTONOMOUS_HTTP_TIMEOUT_MS) throw new ArgumentError("HTTP connector timeoutMs is outside its bound");
    this.maxRequestBytes = input.maxRequestBytes ?? MAX_AUTONOMOUS_HTTP_REQUEST_BYTES;
    this.maxResponseBytes = input.maxResponseBytes ?? MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES;
    for (const [name, value, maximum] of [["maxRequestBytes", this.maxRequestBytes, MAX_AUTONOMOUS_HTTP_REQUEST_BYTES], ["maxResponseBytes", this.maxResponseBytes, MAX_AUTONOMOUS_HTTP_RESPONSE_BYTES]] as const) {
      if (!Number.isInteger(value) || value < 1 || value > maximum) throw new ArgumentError(`HTTP connector ${name} is outside its bound`);
    }
    const methods = input.allowedMethods ?? AUTONOMOUS_HTTP_METHODS;
    if (!Array.isArray(methods) || methods.length === 0) throw new ArgumentError("HTTP connector allowedMethods must be non-empty");
    this.allowedMethods = methods.map((method) => boundedText("HTTP connector method", method, 16).toUpperCase());
    if (this.allowedMethods.some((method) => !(AUTONOMOUS_HTTP_METHODS as readonly string[]).includes(method)) || new Set(this.allowedMethods).size !== this.allowedMethods.length) {
      throw new ArgumentError("HTTP connector allowedMethods contains an unsupported or duplicate method");
    }
  }
}

export class AutonomousHttpConnectorRequest {
  readonly method: string;
  readonly url: string;
  readonly body: JsonValue | null;
  readonly headers: Readonly<Record<string, string>>;

  constructor(input: { method: string; url: string; body?: JsonValue | null; headers?: Readonly<Record<string, string>> }) {
    this.method = boundedText("HTTP connector method", input.method, 16).toUpperCase();
    if (!(AUTONOMOUS_HTTP_METHODS as readonly string[]).includes(this.method)) throw new ArgumentError("HTTP connector method is unsupported");
    this.url = boundedText("HTTP connector URL", input.url, MAX_AUTONOMOUS_HTTP_URL_BYTES);
    if (/\s/.test(this.url)) throw new ArgumentError("HTTP connector URL contains whitespace");
    this.headers = normalizeHeaders("HTTP connector headers", input.headers ?? {});
    this.body = input.body === undefined || input.body === null ? null : safeJson(input.body, "HTTP connector request body", MAX_AUTONOMOUS_HTTP_REQUEST_BYTES);
    if ((this.method === "GET" || this.method === "DELETE") && this.body !== null) throw new ArgumentError("HTTP connector GET/DELETE requests cannot contain a body");
  }
}

export type AutonomousHttpConnectorEndpointResolver = (
  manifest: DomainEvidenceProviderConnectorManifest,
  request: JsonObject,
) => AutonomousHttpConnectorRequest | Promise<AutonomousHttpConnectorRequest>;
export type AutonomousHttpConnectorHeaderResolver = (
  manifest: DomainEvidenceProviderConnectorManifest,
  request: JsonObject,
) => Readonly<Record<string, string>> | Promise<Readonly<Record<string, string>>>;
export type AutonomousHttpConnectorFetch = (input: string, init: RequestInit) => Promise<Response>;

function failureForStatus(status: number): { status: "refused" | "error"; failure: string } {
  if (status === 401 || status === 403) return { status: "refused", failure: "auth_refused" };
  if (status === 404) return { status: "refused", failure: "not_found" };
  if (status === 429) return { status: "error", failure: "rate_limited" };
  if (status === 408 || status === 425) return { status: "error", failure: "timeout" };
  if (status >= 400 && status < 500) return { status: "refused", failure: "http_4xx" };
  return { status: "error", failure: "http_5xx" };
}

function concatBytes(chunks: readonly Uint8Array[], total: number): Uint8Array {
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

class ResponseTooLarge extends Error {
  readonly bytes: Uint8Array;

  constructor(bytes: Uint8Array) {
    super("HTTP connector response exceeds its byte bound");
    this.bytes = bytes;
  }
}

async function readResponseBounded(response: Response, maximum: number): Promise<Uint8Array> {
  if (response.body) {
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let total = 0;
    try {
      while (true) {
        const next = await reader.read();
        if (next.done) break;
        const chunk = next.value;
        const remaining = maximum + 1 - total;
        if (chunk.byteLength > remaining) {
          chunks.push(chunk.slice(0, remaining));
          total += remaining;
          await reader.cancel();
          throw new ResponseTooLarge(concatBytes(chunks, total));
        }
        chunks.push(chunk);
        total += chunk.byteLength;
      }
      return concatBytes(chunks, total);
    } finally {
      reader.releaseLock();
    }
  }
  const raw = new Uint8Array(await response.arrayBuffer());
  if (raw.byteLength > maximum) throw new ResponseTooLarge(raw.slice(0, maximum + 1));
  return raw;
}

export function createAutonomousHttpConnectorExecutor(
  endpointResolver: AutonomousHttpConnectorEndpointResolver,
  options: {
    policy?: AutonomousHttpConnectorPolicy;
    headerResolver?: AutonomousHttpConnectorHeaderResolver;
    fetch?: AutonomousHttpConnectorFetch;
  } = {},
): AutonomousConnectorExecutor {
  if (typeof endpointResolver !== "function") throw new ArgumentError("HTTP connector endpointResolver must be callable");
  if (options.headerResolver !== undefined && typeof options.headerResolver !== "function") throw new ArgumentError("HTTP connector headerResolver must be callable");
  const policy = options.policy ?? new AutonomousHttpConnectorPolicy();
  if (!(policy instanceof AutonomousHttpConnectorPolicy)) throw new ArgumentError("HTTP connector policy is malformed");
  const fetchImpl = options.fetch ?? (typeof globalThis.fetch === "function" ? globalThis.fetch.bind(globalThis) as AutonomousHttpConnectorFetch : undefined);
  if (!fetchImpl) throw new ArgumentError("HTTP connector requires a fetch implementation");

  return async (manifest, request) => {
    const endpoint = await endpointResolver(manifest, request);
    if (!(endpoint instanceof AutonomousHttpConnectorRequest)) throw new ArgumentError("HTTP connector endpoint resolver returned an invalid request");
    let parsed: URL;
    try {
      parsed = new URL(endpoint.url);
    } catch (error) {
      throw new ArgumentError(`HTTP connector URL failed transport admission: ${error instanceof Error ? error.constructor.name : "URL"}`);
    }
    const host = parsed.hostname;
    if (!(parsed.protocol === "http:" || parsed.protocol === "https:") || parsed.username || parsed.password || parsed.hash) throw new ArgumentError("HTTP connector URL failed transport admission");
    if (policy.requireHttps && parsed.protocol !== "https:") throw new ArgumentError("HTTP connector requires HTTPS");
    if (!hostAllowed(host, policy.allowedHosts, policy.allowLoopback)) throw new ArgumentError("HTTP connector host is outside its allowlist");
    parsed.searchParams.forEach((_value, key) => {
      if (containsSecretField(key)) throw new ArgumentError("HTTP connector URL query contains credential-shaped fields");
    });
    if (!policy.allowedMethods.includes(endpoint.method)) throw new ArgumentError("HTTP connector method is outside its policy");

    const resolvedHeaders = options.headerResolver ? await options.headerResolver(manifest, request) : {};
    const headers = normalizeHeaders("HTTP connector resolved headers", { ...endpoint.headers, ...resolvedHeaders });
    let body: string | undefined;
    if (endpoint.body !== null) {
      body = canonicalJson(endpoint.body);
      if (new TextEncoder().encode(body).byteLength > policy.maxRequestBytes) throw new ArgumentError("HTTP connector request exceeds its byte bound");
      if (!Object.keys(headers).some((name) => name.toLowerCase() === "content-type")) {
        if (Object.keys(headers).length >= MAX_AUTONOMOUS_HTTP_HEADERS) throw new ArgumentError("HTTP connector resolved headers are outside their bound");
        headers["Content-Type"] = "application/json";
      }
    }
    const controller = typeof AbortController === "function" ? new AbortController() : undefined;
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; controller?.abort(); }, policy.timeoutMs);
    try {
      const response = await fetchImpl(parsed.toString(), { method: endpoint.method, headers, body, redirect: "error", signal: controller?.signal });
      if (!response.ok) {
        const result = failureForStatus(response.status);
        return new AutonomousConnectorObservation({ status_code: response.status }, result.status, result.failure);
      }
      let raw: Uint8Array;
      try {
        raw = await readResponseBounded(response, policy.maxResponseBytes);
      } catch (error) {
        if (!(error instanceof ResponseTooLarge)) throw error;
        return new AutonomousConnectorObservation({ status_code: response.status, body_digest: digestBytesSync(error.bytes) }, "error", "response_too_large");
      }
      if (raw.byteLength === 0) return new AutonomousConnectorObservation(null, "observed");
      let value: unknown;
      try {
        value = JSON.parse(new TextDecoder().decode(raw)) as unknown;
      } catch {
        const contentType = boundedHeader("HTTP connector content type", response.headers.get("content-type") ?? "application/octet-stream");
        return new AutonomousConnectorObservation({ status_code: response.status, content_type: contentType, body_digest: digestBytesSync(raw) }, "partial", "invalid_json");
      }
      return new AutonomousConnectorObservation(value, "observed");
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      if (timedOut || (typeof DOMException !== "undefined" && error instanceof DOMException && error.name === "AbortError")) return new AutonomousConnectorObservation(null, "error", "timeout");
      return new AutonomousConnectorObservation(null, "error", "transport_error");
    } finally {
      clearTimeout(timer);
    }
  };
}

export type AutonomousHttpConnectorFailureClass = typeof AUTONOMOUS_HTTP_FAILURE_CLASSES[number];
