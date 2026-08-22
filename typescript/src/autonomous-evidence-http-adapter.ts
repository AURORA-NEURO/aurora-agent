import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousConnectorObservation,
} from "./autonomous-connectors.js";
import {
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  createAutonomousHttpConnectorExecutor,
  type AutonomousHttpConnectorEndpointResolver,
  type AutonomousHttpConnectorFetch,
  type AutonomousHttpConnectorHeaderResolver,
} from "./autonomous-http-connector.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type { AutonomousEvidenceAcquisitionContext, AutonomousEvidenceProjector } from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterRegistrationInput,
} from "./autonomous-evidence-adapters.js";
import { canonicalJson } from "./tooling.js";
import type { DomainEvidenceProviderConnectorManifest, JsonObject, JsonValue } from "./types.js";

/** Bridge schema for an HTTP-backed domain evidence adapter registration. */
export const AUTONOMOUS_HTTP_EVIDENCE_ADAPTER_SCHEMA = "bioprism-typescript-autonomous-http-evidence-adapter/0.1" as const;
export const MAX_AUTONOMOUS_HTTP_EVIDENCE_REQUEST_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_HTTP_EVIDENCE_REQUEST_DEPTH = 32;

const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

function fieldMarker(value: string): string {
  return [...value.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
}

function containsSecretField(value: string): boolean {
  const normalized = fieldMarker(value);
  return SECRET_FIELD_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj");
}

function safeRequest(value: unknown, depth = 0): JsonValue {
  if (depth > MAX_AUTONOMOUS_HTTP_EVIDENCE_REQUEST_DEPTH) throw new ArgumentError("HTTP evidence adapter request is too deeply nested");
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) {
    const result = value.map((item) => safeRequest(item, depth + 1));
    if (new TextEncoder().encode(canonicalJson(result)).byteLength > MAX_AUTONOMOUS_HTTP_EVIDENCE_REQUEST_BYTES) throw new ArgumentError("HTTP evidence adapter request exceeds its byte bound");
    return result;
  }
  if (isObject(value)) {
    const result: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\u0000") || containsSecretField(key)) throw new ArgumentError("HTTP evidence adapter request contains credential-shaped fields");
      if (child === undefined) throw new ArgumentError("HTTP evidence adapter request contains an undefined field");
      result[key] = safeRequest(child, depth + 1);
    }
    if (new TextEncoder().encode(canonicalJson(result)).byteLength > MAX_AUTONOMOUS_HTTP_EVIDENCE_REQUEST_BYTES) throw new ArgumentError("HTTP evidence adapter request exceeds its byte bound");
    return result;
  }
  throw new ArgumentError("HTTP evidence adapter request must be JSON-safe");
}

function defaultManifest(options: {
  adapterId: string;
  version: string;
  domain: AutonomousDomainName;
  provider: string;
  capabilities: readonly string[];
}): DomainEvidenceProviderConnectorManifest {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: options.adapterId,
    version: options.version,
    provider: options.provider,
    connector_kind: "provider_api",
    domains: [options.domain],
    capabilities: [...options.capabilities],
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: [],
      does_not_claim: [
        "the caller-owned endpoint is truthful or current",
        "the adapter stores or validates credentials",
        "HTTP success is evaluator success",
      ],
    },
  };
}

export interface AutonomousHttpEvidenceAdapterOptions {
  adapterId: string;
  version: string;
  domain: AutonomousDomainName;
  provider?: string;
  capabilities: readonly string[];
  sourceKinds?: readonly string[];
  manifest?: DomainEvidenceProviderConnectorManifest;
  policy?: AutonomousHttpConnectorPolicy;
  fetch?: AutonomousHttpConnectorFetch;
  endpointResolver: AutonomousHttpConnectorEndpointResolver;
  requestForContext: (context: AutonomousEvidenceAcquisitionContext) => JsonObject | Promise<JsonObject>;
  headerResolver?: AutonomousHttpConnectorHeaderResolver;
  project?: AutonomousEvidenceProjector["project"];
}

/** Build one registry registration backed by the existing bounded HTTP connector. */
export function createAutonomousHttpEvidenceAdapterRegistration(options: AutonomousHttpEvidenceAdapterOptions): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: [AutonomousDomainName] } {
  if (!options || typeof options !== "object") throw new ArgumentError("HTTP evidence adapter options are malformed");
  if (typeof options.endpointResolver !== "function") throw new ArgumentError("HTTP evidence adapter endpointResolver is required");
  if (typeof options.requestForContext !== "function") throw new ArgumentError("HTTP evidence adapter requestForContext is required");
  if (options.headerResolver !== undefined && typeof options.headerResolver !== "function") throw new ArgumentError("HTTP evidence adapter headerResolver is malformed");
  if (options.project !== undefined && typeof options.project !== "function") throw new ArgumentError("HTTP evidence adapter project is malformed");
  if (!(options.policy instanceof AutonomousHttpConnectorPolicy)) throw new ArgumentError("HTTP evidence adapter policy must be an AutonomousHttpConnectorPolicy");
  const manifest = options.manifest ?? defaultManifest({ adapterId: options.adapterId, version: options.version, domain: options.domain, provider: options.provider ?? "caller-http", capabilities: options.capabilities });
  if (!manifest.domains.includes(options.domain) || manifest.connector_id !== options.adapterId || manifest.version !== options.version) throw new ArgumentError("HTTP evidence adapter manifest does not match its registration identity or domain");
  const executor = createAutonomousHttpConnectorExecutor(options.endpointResolver, { policy: options.policy, headerResolver: options.headerResolver, fetch: options.fetch });
  return {
    adapterId: options.adapterId,
    version: options.version,
    domains: [options.domain],
    capabilities: options.capabilities,
    sourceKinds: options.sourceKinds ?? ["http_json"],
    project: options.project,
    acquire: async (context) => {
      const rawRequest = await options.requestForContext(context);
      const requestValue = safeRequest(rawRequest);
      if (!isObject(requestValue)) throw new ArgumentError("HTTP evidence adapter request must be an object");
      const observation = await executor(manifest, requestValue) as AutonomousConnectorObservation;
      if (!(observation instanceof AutonomousConnectorObservation)) throw new ArgumentError("HTTP evidence adapter transport returned an invalid observation");
      if (observation.status === "error" || observation.status === "refused") throw new ArgumentError(`HTTP evidence adapter source refused: ${observation.failure_class ?? observation.status}`);
      return observation.value;
    },
  };
}

/** Register a concrete HTTP evidence adapter for one domain. */
export function registerAutonomousHttpEvidenceAdapter(
  registry: AutonomousEvidenceAdapterRegistry,
  options: AutonomousHttpEvidenceAdapterOptions,
  registrationOptions: { replace?: boolean } = {},
): JsonObject {
  if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("HTTP evidence adapter registration requires a typed registry");
  const registration = createAutonomousHttpEvidenceAdapterRegistration(options);
  return registry.register(registration, registrationOptions);
}
