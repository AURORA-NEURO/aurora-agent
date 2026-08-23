import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousConnectorObservation,
  AutonomousConnectorRegistration,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  type AutonomousConnectorExecutor,
  type AutonomousConnectorReceiptStore,
} from "./autonomous-connectors.js";
import { AutonomousConnectorOperationRegistry } from "./autonomous-connector-worker.js";
import { AutonomousConnectorOperationFacade } from "./autonomous-connector-facade.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type {
  DomainEvidenceProviderConnectorManifest,
  JsonObject,
  JsonValue,
} from "./types.js";

/**
 * Credentialless deterministic connector for local development, air-gapped deployments,
 * replay tests, and evaluator fixtures. It projects metadata shape and digests only; it
 * never opens a socket, consumes a credential, or represents caller metadata as external
 * evidence.
 */
export const AUTONOMOUS_BUILTIN_CONNECTOR_SCHEMA = "bioprism-typescript-autonomous-builtin-connector/0.1" as const;
export const AUTONOMOUS_BUILTIN_CONNECTOR_ID = "builtin.offline-evidence" as const;
export const AUTONOMOUS_BUILTIN_CONNECTOR_VERSION = "1.0.0" as const;
export const AUTONOMOUS_BUILTIN_CONNECTOR_PROVIDER = "local-offline" as const;
export const MAX_AUTONOMOUS_BUILTIN_INPUT_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_BUILTIN_FIELDS = 128;
export const MAX_AUTONOMOUS_BUILTIN_FIELD_NAME_BYTES = 256;
export const MAX_AUTONOMOUS_BUILTIN_SEQUENCE_ITEMS = 1_024;
export const MAX_AUTONOMOUS_BUILTIN_SHAPE_DEPTH = 16;

const RECOMMENDED_FIELDS: Readonly<Record<string, readonly string[]>> = {
  "coding.repository_change_analysis": ["repository_digest", "changed_files", "test_results"],
  "browser.web_evidence_retrieval": ["source_digests", "retrieved_at", "citation_metadata"],
  "data.dataset_quality_profile": ["schema", "row_count", "column_count", "lineage"],
  "science.reproducible_evidence_acquisition": ["hypothesis", "evidence_digests", "analysis_digest"],
  "biomedical.clinical_data_review": ["provenance", "cohort_digest", "review_questions"],
  "neuroscience.signal_study_analysis": ["signal_digest", "sampling_rate", "study_design"],
  "operations.incident_runbook_observation": ["incident_digest", "telemetry_digest", "runbook_digest"],
  "enterprise.workflow_record_governance": ["workflow_digest", "record_type", "policy_digest"],
  "multi_agent.delegated_consensus_handoff": ["delegation_digest", "agent_digests", "conflicts"],
  "multimodal.asset_alignment": ["modalities", "asset_digests", "alignment_digest"],
  "cross_domain.evidence_fanout_synthesis": ["domain_digests", "evidence_digests", "route_digest"],
  "evaluation.benchmark_replay_analysis": ["benchmark_digest", "case_count", "replay_digest"],
};

const IDENTITY_FIELDS = new Set(["operation_id", "subject_digest"]);
const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey",
  "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function rejectsSecretField(name: string): boolean {
  const normalized = name.toLowerCase().replace(/[^a-z0-9]/g, "");
  return SECRET_FIELD_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj");
}

function safeInput(name: string, value: unknown, maximum: number, depth = 0): JsonValue {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) {
    const result = value.map((item) => safeInput(name, item, maximum, depth + 1));
    if (bytes(canonicalJson(result)) > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  if (isObject(value)) {
    const result: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      if (rejectsSecretField(key)) throw new ArgumentError(`${name} contains credential-shaped fields`);
      result[key] = safeInput(name, child, maximum, depth + 1);
    }
    if (bytes(canonicalJson(result)) > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  throw new ArgumentError(`${name} must be JSON-safe`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedFieldName(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > MAX_AUTONOMOUS_BUILTIN_FIELD_NAME_BYTES) throw new ArgumentError("built-in connector field names are outside their bound");
  return value;
}

function shape(value: JsonValue, depth = 0): JsonObject {
  if (depth > MAX_AUTONOMOUS_BUILTIN_SHAPE_DEPTH) return { type: "depth_limited" };
  if (value === null) return { type: "null" };
  if (typeof value === "boolean") return { type: "boolean" };
  if (typeof value === "string") return { type: "string", bytes: bytes(value) };
  if (typeof value === "number") return { type: "number" };
  if (Array.isArray(value)) {
    return {
      type: "array",
      item_count: value.length,
      item_types: [...new Set(value.slice(0, MAX_AUTONOMOUS_BUILTIN_SEQUENCE_ITEMS).map((item) => String(shape(item, depth + 1).type)))].sort(),
    };
  }
  return { type: "object", field_count: Object.keys(value).length, field_names_digest: digestJsonSync(Object.keys(value).sort()) };
}

function contentPresent(value: JsonValue): boolean {
  if (value === null || value === "") return false;
  if (Array.isArray(value)) return value.some((item) => contentPresent(item));
  if (isObject(value)) return Object.values(value).some((item) => contentPresent(item as JsonValue));
  return true;
}

function fieldProjection(request: JsonObject): JsonObject[] {
  const fields: JsonObject[] = [];
  for (const name of Object.keys(request).sort()) {
    const bounded = boundedFieldName(name);
    if (IDENTITY_FIELDS.has(bounded)) continue;
    const value = request[bounded];
    if (value === undefined) throw new ArgumentError("built-in connector request contains an undefined field");
    fields.push({ name: bounded, digest: digestJsonSync(value), shape: shape(value), present: contentPresent(value) });
  }
  if (fields.length > MAX_AUTONOMOUS_BUILTIN_FIELDS) throw new ArgumentError("built-in connector request contains too many fields");
  return fields;
}

function domainValue(value: string | undefined): AutonomousDomainName | null {
  if (value === undefined) return null;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value as AutonomousDomainName)) throw new ArgumentError("built-in connector domain is unsupported");
  return value as AutonomousDomainName;
}

export class AutonomousBuiltinConnectorAdapter {
  readonly operationRegistry: AutonomousConnectorOperationRegistry;
  readonly connector_id: string;
  readonly version: string;
  readonly domain: AutonomousDomainName | null;

  constructor(options: { operationRegistry?: AutonomousConnectorOperationRegistry; connectorId?: string; version?: string; domain?: AutonomousDomainName } = {}) {
    this.operationRegistry = options.operationRegistry ?? new AutonomousConnectorOperationRegistry();
    if (!(this.operationRegistry instanceof AutonomousConnectorOperationRegistry)) throw new ArgumentError("built-in connector operationRegistry is invalid");
    this.connector_id = identifier("built-in connector connectorId", options.connectorId ?? AUTONOMOUS_BUILTIN_CONNECTOR_ID);
    this.version = identifier("built-in connector version", options.version ?? AUTONOMOUS_BUILTIN_CONNECTOR_VERSION);
    this.domain = domainValue(options.domain);
    const contracts = this.domain === null ? this.operationRegistry.operations() : this.operationRegistry.forDomain(this.domain);
    if (!contracts.length) throw new ArgumentError("built-in connector domain has no operation contract");
    const missing = contracts.map((contract) => contract.operation_id).filter((id) => RECOMMENDED_FIELDS[id] === undefined);
    if (missing.length) throw new ArgumentError(`built-in connector has no field profile for: ${missing.join(", ")}`);
  }

  get domains(): AutonomousDomainName[] {
    return this.domain === null ? [...AUTONOMOUS_DOMAIN_NAMES] : [this.domain];
  }

  get capabilities(): string[] {
    const contracts = this.domain === null ? this.operationRegistry.operations() : this.operationRegistry.forDomain(this.domain);
    const primary = [...new Set(contracts.map((operation) => operation.capabilities[0]).filter((value): value is string => value !== undefined))];
    const secondary = [...new Set(contracts.flatMap((operation) => operation.capabilities))].filter((value) => !primary.includes(value)).sort();
    return [...primary, ...secondary].slice(0, 128);
  }

  manifest(): DomainEvidenceProviderConnectorManifest {
    return {
      schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
      connector_id: this.connector_id,
      version: this.version,
      provider: AUTONOMOUS_BUILTIN_CONNECTOR_PROVIDER,
      connector_kind: "provider_api",
      domains: this.domains,
      capabilities: this.capabilities,
      transport: "caller_managed",
      auth_posture: {
        status: "none",
        secret_refs: [],
        does_not_claim: [
          "no external provider was contacted",
          "caller-supplied metadata is not independently verified",
          "no credential material is accepted or retained",
        ],
      },
    };
  }

  execute(manifest: DomainEvidenceProviderConnectorManifest, request: JsonObject): AutonomousConnectorObservation {
    if (!isObject(manifest) || manifest.connector_id !== this.connector_id || manifest.version !== this.version) throw new ArgumentError("built-in connector manifest identity does not match the adapter");
    const safeRequest = safeInput("built-in connector request", request, MAX_AUTONOMOUS_BUILTIN_INPUT_BYTES);
    if (!isObject(safeRequest)) throw new ArgumentError("built-in connector request must be an object");
    const operationId = identifier("built-in connector operation_id", safeRequest.operation_id);
    const contract = this.operationRegistry.resolve(operationId);
    if (this.domain !== null && contract.domain !== this.domain) throw new ArgumentError("built-in connector operation exceeds its domain scope");
    const subjectDigest = digest("built-in connector subject_digest", safeRequest.subject_digest);
    const fields = fieldProjection(safeRequest);
    const available = fields.map((field) => String(field.name));
    const recommended = RECOMMENDED_FIELDS[contract.operation_id] ?? [];
    const missing = recommended.filter((field) => !available.includes(field));
    const hasEvidence = fields.some((field) => field.present === true);
    const status = hasEvidence && missing.length === 0 ? "observed" : "partial";
    const fieldDigests = Object.fromEntries(fields.map((field) => [String(field.name), String(field.digest)]));
    const fieldShapes = Object.fromEntries(fields.map((field) => [String(field.name), field.shape as JsonValue]));
    const value: JsonObject = {
      schema: AUTONOMOUS_BUILTIN_CONNECTOR_SCHEMA,
      operation_id: contract.operation_id,
      domain: contract.domain,
      subject_digest: subjectDigest,
      operation_digest: contract.operation_digest,
      operation_capabilities: [...contract.capabilities],
      evaluator_signals: [...contract.evaluator_signals],
      recommended_fields: [...recommended],
      available_fields: available,
      missing_fields: missing,
      field_digests: fieldDigests,
      field_shapes: fieldShapes,
      field_count: fields.length,
      input_digest: digestJsonSync(safeRequest),
      status,
      failure_class: status === "observed" ? null : "incomplete_local_fixture",
      evidence_posture: "caller_supplied_metadata;offline_fixture;not_external_validation",
      effect_posture: "read_only;no_network;no_provider_invocation",
      retention: "transient_metadata_projection;receipt_retains_digest_only",
      secret_material: "never_accepted_or_returned",
    };
    return new AutonomousConnectorObservation(value, status, status === "observed" ? null : "incomplete_local_fixture");
  }

  asExecutor(): AutonomousConnectorExecutor {
    return (manifest, request) => this.execute(manifest, request);
  }
}

export function builtinAutonomousConnectorRegistration(options: {
  operationRegistry?: AutonomousConnectorOperationRegistry;
  connectorId?: string;
  version?: string;
  approvalRequired?: boolean;
  domain?: AutonomousDomainName;
} = {}): AutonomousConnectorRegistration {
  if (options.approvalRequired !== undefined && typeof options.approvalRequired !== "boolean") throw new ArgumentError("built-in connector approvalRequired must be boolean");
  const adapter = new AutonomousBuiltinConnectorAdapter(options);
  return new AutonomousConnectorRegistration(adapter.manifest(), adapter.asExecutor(), options.approvalRequired ?? true);
}

export function registerBuiltinAutonomousConnectors(
  registry: AutonomousConnectorRegistry,
  options: { operationRegistry?: AutonomousConnectorOperationRegistry; connectorId?: string; version?: string; approvalRequired?: boolean; replace?: boolean } = {},
): AutonomousConnectorRegistration {
  if (!(registry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("built-in connector registration requires an AutonomousConnectorRegistry");
  const registration = builtinAutonomousConnectorRegistration(options);
  registry.register(registration, { replace: options.replace === true });
  return registration;
}

export function builtinAutonomousDomainConnectorRegistrations(options: {
  operationRegistry?: AutonomousConnectorOperationRegistry;
  connectorId?: string;
  version?: string;
  approvalRequired?: boolean;
} = {}): AutonomousConnectorRegistration[] {
  const operationRegistry = options.operationRegistry ?? new AutonomousConnectorOperationRegistry();
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => builtinAutonomousConnectorRegistration({
    operationRegistry,
    connectorId: `${options.connectorId ?? AUTONOMOUS_BUILTIN_CONNECTOR_ID}.${domain}`,
    version: options.version,
    approvalRequired: options.approvalRequired,
    domain,
  }));
}

export function registerBuiltinAutonomousDomainConnectors(
  registry: AutonomousConnectorRegistry,
  options: { operationRegistry?: AutonomousConnectorOperationRegistry; connectorId?: string; version?: string; approvalRequired?: boolean; replace?: boolean } = {},
): AutonomousConnectorRegistration[] {
  if (!(registry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("built-in domain connector registration requires an AutonomousConnectorRegistry");
  const registrations = builtinAutonomousDomainConnectorRegistrations(options);
  const existing = new Set(registry.registrations().map((registration) => registration.connector_id));
  if (options.replace !== true && registrations.some((registration) => existing.has(registration.connector_id))) throw new ArgumentError("a built-in domain connector is already registered");
  if (existing.size + registrations.filter((registration) => !existing.has(registration.connector_id)).length > 256) throw new ArgumentError("built-in domain connector registration exceeds registry capacity");
  for (const registration of registrations) registry.register(registration, { replace: options.replace === true });
  return registrations;
}

/**
 * Build an immediately usable local connector runtime for an embedding application.
 * The returned runtime still requires the normal reviewed selection and approval gates;
 * this helper only removes boilerplate from keyless development and evaluation setups.
 */
export function createBuiltinAutonomousConnectorRuntime(options: {
  domainScoped?: boolean;
  operationRegistry?: AutonomousConnectorOperationRegistry;
  connectorId?: string;
  version?: string;
  approvalRequired?: boolean;
  receiptStore?: AutonomousConnectorReceiptStore;
  receiptSink?: (receipt: unknown) => unknown | Promise<unknown>;
} = {}): {
  operationRegistry: AutonomousConnectorOperationRegistry;
  registry: AutonomousConnectorRegistry;
  runtime: AutonomousConnectorRuntime;
  operationFacade: AutonomousConnectorOperationFacade;
  registrations: AutonomousConnectorRegistration[];
} {
  const operationRegistry = options.operationRegistry ?? new AutonomousConnectorOperationRegistry();
  const registry = new AutonomousConnectorRegistry();
  const registrations = options.domainScoped === false
    ? [builtinAutonomousConnectorRegistration({ operationRegistry, connectorId: options.connectorId, version: options.version, approvalRequired: options.approvalRequired })]
    : builtinAutonomousDomainConnectorRegistrations({ operationRegistry, connectorId: options.connectorId, version: options.version, approvalRequired: options.approvalRequired });
  for (const registration of registrations) registry.register(registration);
  const runtime = new AutonomousConnectorRuntime(registry, {
    receiptStore: options.receiptStore,
    receiptSink: options.receiptSink,
  });
  const operationFacade = new AutonomousConnectorOperationFacade({ registry, runtime, operationRegistry });
  return { operationRegistry, registry, runtime, operationFacade, registrations };
}
