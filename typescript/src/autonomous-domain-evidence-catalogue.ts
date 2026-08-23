import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousDomainName,
} from "./autonomous.js";
import {
  AutonomousEvidencePlan,
  type AutonomousEvidenceRequirement,
} from "./autonomous-evidence.js";
import type {
  AutonomousEvidenceAcquirer,
  AutonomousEvidenceAcquisitionContext,
} from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceSourceReconciler,
  AutonomousEvidenceReconciliationPlan,
  AutonomousEvidenceReconciliationResult,
  type AutonomousEvidenceReconciliationExecuteOptions,
} from "./autonomous-evidence-reconciliation.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Stable metadata contract for a caller-managed source family serving one domain. */
export const AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA = "bioprism-typescript-autonomous-domain-evidence-profile/0.1" as const;
/** Metadata-only projection of the registered source catalogue. */
export const AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA = "bioprism-typescript-autonomous-domain-evidence-catalogue/0.1" as const;
/** Route identity used when a profile is bound to an acquirer. */
export const AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA = "bioprism-typescript-autonomous-domain-evidence-route/0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES = 128;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES = 512;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS = 64;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES = 64;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS = 32;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_METADATA_BYTES = 64_000;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_BYTES = 512_000;

const RETENTION = "metadata_only;source_values_queries_and_credentials_caller_owned" as const;
const RESERVED_METADATA_KEY = "__aurora_domain_evidence_source";
const SECRET_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

export const AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES = [
  "realtime", "bounded_cache", "historical", "caller_declared",
] as const;
export const AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES = [
  "none", "caller_managed_credential", "caller_signed_request", "delegated_session",
] as const;
export const AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES = [
  "none", "cursor", "page_number", "link_header", "caller_defined",
] as const;

export type AutonomousDomainEvidenceFreshnessMode = typeof AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES[number];
export type AutonomousDomainEvidenceAuthMode = typeof AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES[number];
export type AutonomousDomainEvidencePaginationMode = typeof AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES[number];

export interface AutonomousDomainEvidenceProfileJSON extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA;
  profile_id: string;
  version: string;
  domain: AutonomousDomainName;
  purpose: string;
  source_kinds: string[];
  capabilities: string[];
  operations: string[];
  required_metadata: string[];
  freshness: AutonomousDomainEvidenceFreshnessMode;
  auth_mode: AutonomousDomainEvidenceAuthMode;
  pagination: AutonomousDomainEvidencePaginationMode;
  normalizer_id: string;
  normalizer_version: string;
  default_quorum: number;
  default_max_concurrency: number;
  limitations: string[];
  profile_digest: string;
  execution: "profile_only;source_dispatch_not_started";
  retention: "profile_metadata_only;source_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousDomainEvidenceProfileInput {
  profileId: string;
  version: string;
  domain: AutonomousDomainName;
  purpose: string;
  sourceKinds: readonly string[];
  capabilities: readonly string[];
  operations: readonly string[];
  requiredMetadata?: readonly string[];
  freshness: AutonomousDomainEvidenceFreshnessMode;
  authMode: AutonomousDomainEvidenceAuthMode;
  pagination: AutonomousDomainEvidencePaginationMode;
  normalizerId: string;
  normalizerVersion: string;
  defaultQuorum?: number;
  defaultMaxConcurrency?: number;
  limitations: readonly string[];
}

export interface AutonomousDomainEvidenceRouteJSON extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA;
  source_id: string;
  profile_id: string;
  profile_version: string;
  profile_digest: string;
  domain: AutonomousDomainName;
  provider: string;
  source_kinds: string[];
  capabilities: string[];
  operations: string[];
  source_digest: string | null;
  request_id: string | null;
  contract_digest: string | null;
  adapter_id: string | null;
  adapter_manifest_digest: string | null;
  metadata_digest: string;
  route_digest: string;
  execution: "registered_route_only;source_dispatch_not_started";
  retention: "route_metadata_only;request_and_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousDomainEvidenceRoute {
  readonly json: AutonomousDomainEvidenceRouteJSON;
  readonly metadata: JsonObject;
  readonly acquirer: AutonomousEvidenceAcquirer;
}

export interface AutonomousDomainEvidenceRouteInput {
  sourceId: string;
  profileId: string;
  provider: string;
  sourceKinds?: readonly string[];
  capabilities?: readonly string[];
  operations?: readonly string[];
  sourceDigest?: string | null;
  requestId?: string | null;
  contractDigest?: string | null;
  adapterId?: string | null;
  adapterManifestDigest?: string | null;
  metadata?: JsonObject;
  acquirer: AutonomousEvidenceAcquirer;
}

export interface AutonomousDomainEvidenceCoverage extends JsonObject {
  domain: AutonomousDomainName;
  profile_ids: string[];
  route_count: number;
  source_ids: string[];
  capabilities: string[];
  state: "ready" | "partial" | "missing";
  retention: "metadata_only";
}

export interface AutonomousDomainEvidenceCatalogueJSON extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA;
  profiles: AutonomousDomainEvidenceProfileJSON[];
  routes: AutonomousDomainEvidenceRouteJSON[];
  coverage: AutonomousDomainEvidenceCoverage[];
  profile_count: number;
  route_count: number;
  covered_domain_count: number;
  registry_digest: string;
  execution: "catalogue_and_route_validation_only;source_dispatch_requires_review";
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousDomainEvidenceCataloguePrepareOptions {
  profileId?: string;
  sourceIds?: readonly string[];
  quorum?: number;
  maxConcurrency?: number;
  requireAll?: boolean;
  parentEvidenceDigests?: readonly string[];
}

export interface AutonomousDomainEvidenceCatalogueExecuteOptions extends Omit<AutonomousEvidenceReconciliationExecuteOptions, "normalizerId" | "normalizerVersion"> {
  profileId?: string;
}

export interface AutonomousDomainEvidenceCatalogueReconciliation {
  profile: AutonomousDomainEvidenceProfileJSON;
  plan: AutonomousEvidenceReconciliationPlan;
  routes: AutonomousDomainEvidenceRouteJSON[];
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const result = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(result)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return result;
}

function digest(name: string, value: unknown, required = true): string | null {
  if (value === undefined || value === null) {
    if (required) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    return null;
  }
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedList(name: string, value: readonly string[], maximum: number, minimum = 1): string[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum) throw new ArgumentError(`${name} must contain between ${minimum} and ${maximum} entries`);
  const result = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicates`);
  return [...result].sort();
}

function boundedTextList(name: string, value: readonly string[], maximum: number): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  const result = value.map((item, index) => boundedText(`${name}[${index}]`, item, 2_048));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicates`);
  return [...result].sort();
}

function boundedDomains(value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length !== 1 || !AUTONOMOUS_DOMAIN_NAMES.includes(value[0] as AutonomousDomainName)) throw new ArgumentError("domain evidence profile must bind exactly one supported domain");
  return [value[0] as AutonomousDomainName];
}

function secretKey(key: string): boolean {
  const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
  return SECRET_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj") || normalized.includes("token") || normalized.includes("secret") || normalized.includes("credential") || normalized.includes("authorization");
}

function assertSafeMetadata(value: unknown, name: string, depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((child, index) => assertSafeMetadata(child, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\u0000") || secretKey(key)) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      if (key === RESERVED_METADATA_KEY) throw new ArgumentError(`${name}.${key} is reserved for the catalogue route binding`);
      assertSafeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function safeMetadata(value: JsonObject | undefined, name: string): JsonObject {
  const result = value ?? {};
  if (!isObject(result)) throw new ArgumentError(`${name} must be a JSON object`);
  assertSafeMetadata(result, name);
  if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_METADATA_BYTES) throw new ArgumentError(`${name} exceeds its byte bound`);
  return structuredClone(result) as JsonObject;
}

function boundedContractDigest(name: string, value: unknown): string | null {
  return digest(name, value, false);
}

function enumValue<T extends readonly string[]>(name: string, value: unknown, values: T): T[number] {
  if (!values.includes(value as T[number])) throw new ArgumentError(`${name} is invalid`);
  return value as T[number];
}

function profileDescriptor(input: {
  profileId: string;
  version: string;
  domain: AutonomousDomainName;
  purpose: string;
  sourceKinds: string[];
  capabilities: string[];
  operations: string[];
  requiredMetadata: string[];
  freshness: AutonomousDomainEvidenceFreshnessMode;
  authMode: AutonomousDomainEvidenceAuthMode;
  pagination: AutonomousDomainEvidencePaginationMode;
  normalizerId: string;
  normalizerVersion: string;
  defaultQuorum: number;
  defaultMaxConcurrency: number;
  limitations: string[];
}): Omit<AutonomousDomainEvidenceProfileJSON, "profile_digest"> {
  return {
    schema: AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA,
    profile_id: input.profileId,
    version: input.version,
    domain: input.domain,
    purpose: input.purpose,
    source_kinds: [...input.sourceKinds],
    capabilities: [...input.capabilities],
    operations: [...input.operations],
    required_metadata: [...input.requiredMetadata],
    freshness: input.freshness,
    auth_mode: input.authMode,
    pagination: input.pagination,
    normalizer_id: input.normalizerId,
    normalizer_version: input.normalizerVersion,
    default_quorum: input.defaultQuorum,
    default_max_concurrency: input.defaultMaxConcurrency,
    limitations: [...input.limitations],
    execution: "profile_only;source_dispatch_not_started",
    retention: "profile_metadata_only;source_values_caller_owned",
    secret_material: "never_returned",
  };
}

/** One versioned source family; it declares semantics but never performs source work. */
export class AutonomousDomainEvidenceSourceProfile {
  readonly profile_id: string;
  readonly version: string;
  readonly domain: AutonomousDomainName;
  readonly purpose: string;
  readonly source_kinds: string[];
  readonly capabilities: string[];
  readonly operations: string[];
  readonly required_metadata: string[];
  readonly freshness: AutonomousDomainEvidenceFreshnessMode;
  readonly auth_mode: AutonomousDomainEvidenceAuthMode;
  readonly pagination: AutonomousDomainEvidencePaginationMode;
  readonly normalizer_id: string;
  readonly normalizer_version: string;
  readonly default_quorum: number;
  readonly default_max_concurrency: number;
  readonly limitations: string[];
  readonly profile_digest: string;

  constructor(input: AutonomousDomainEvidenceProfileInput) {
    if (!input || typeof input !== "object") throw new ArgumentError("domain evidence profile is malformed");
    const profileId = identifier("domain evidence profileId", input.profileId);
    const version = identifier("domain evidence profile version", input.version);
    const domain = boundedDomains([input.domain])[0]!;
    const purpose = boundedText("domain evidence profile purpose", input.purpose, 2_048);
    const sourceKinds = boundedList("domain evidence profile sourceKinds", input.sourceKinds, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS);
    const capabilities = boundedList("domain evidence profile capabilities", input.capabilities, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES);
    const operations = boundedList("domain evidence profile operations", input.operations, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS);
    const requiredMetadata = input.requiredMetadata === undefined ? [] : boundedList("domain evidence profile requiredMetadata", input.requiredMetadata, 64, 0);
    for (const key of requiredMetadata) {
      if (secretKey(key) || key === RESERVED_METADATA_KEY) throw new ArgumentError("domain evidence profile required metadata is credential-shaped or reserved");
    }
    const freshness = enumValue("domain evidence profile freshness", input.freshness, AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES);
    const authMode = enumValue("domain evidence profile authMode", input.authMode, AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES);
    const pagination = enumValue("domain evidence profile pagination", input.pagination, AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES);
    const normalizerId = identifier("domain evidence profile normalizerId", input.normalizerId);
    const normalizerVersion = identifier("domain evidence profile normalizerVersion", input.normalizerVersion);
    const defaultQuorum = input.defaultQuorum ?? 1;
    const defaultMaxConcurrency = input.defaultMaxConcurrency ?? 4;
    if (!Number.isSafeInteger(defaultQuorum) || defaultQuorum < 1 || defaultQuorum > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES) throw new ArgumentError("domain evidence profile defaultQuorum is outside its bound");
    if (!Number.isSafeInteger(defaultMaxConcurrency) || defaultMaxConcurrency < 1 || defaultMaxConcurrency > 8) throw new ArgumentError("domain evidence profile defaultMaxConcurrency is outside its bound");
    const limitations = boundedTextList("domain evidence profile limitations", input.limitations, 32);
    const descriptor = profileDescriptor({ profileId, version, domain, purpose, sourceKinds, capabilities, operations, requiredMetadata, freshness, authMode, pagination, normalizerId, normalizerVersion, defaultQuorum, defaultMaxConcurrency, limitations });
    this.profile_id = profileId;
    this.version = version;
    this.domain = domain;
    this.purpose = purpose;
    this.source_kinds = sourceKinds;
    this.capabilities = capabilities;
    this.operations = operations;
    this.required_metadata = requiredMetadata;
    this.freshness = freshness;
    this.auth_mode = authMode;
    this.pagination = pagination;
    this.normalizer_id = normalizerId;
    this.normalizer_version = normalizerVersion;
    this.default_quorum = defaultQuorum;
    this.default_max_concurrency = defaultMaxConcurrency;
    this.limitations = limitations;
    this.profile_digest = digestJsonSync(descriptor);
  }

  toJSON(): AutonomousDomainEvidenceProfileJSON {
    return {
      ...profileDescriptor({
        profileId: this.profile_id,
        version: this.version,
        domain: this.domain,
        purpose: this.purpose,
        sourceKinds: this.source_kinds,
        capabilities: this.capabilities,
        operations: this.operations,
        requiredMetadata: this.required_metadata,
        freshness: this.freshness,
        authMode: this.auth_mode,
        pagination: this.pagination,
        normalizerId: this.normalizer_id,
        normalizerVersion: this.normalizer_version,
        defaultQuorum: this.default_quorum,
        defaultMaxConcurrency: this.default_max_concurrency,
        limitations: this.limitations,
      }),
      profile_digest: this.profile_digest,
    } as AutonomousDomainEvidenceProfileJSON;
  }
}

function subset(name: string, values: readonly string[], allowed: readonly string[]): void {
  const permitted = new Set(allowed);
  const missing = values.filter((value) => !permitted.has(value));
  if (missing.length) throw new ArgumentError(`${name} exceeds its profile contract: ${missing.join(", ")}`);
}

function sourceIdList(name: string, value: readonly string[] | undefined): string[] {
  return boundedList(name, value ?? [], MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES, 0);
}

function routeDescriptor(input: {
  sourceId: string;
  profile: AutonomousDomainEvidenceSourceProfile;
  provider: string;
  sourceKinds: string[];
  capabilities: string[];
  operations: string[];
  sourceDigest: string | null;
  requestId: string | null;
  contractDigest: string | null;
  adapterId: string | null;
  adapterManifestDigest: string | null;
  metadataDigest: string;
}): Omit<AutonomousDomainEvidenceRouteJSON, "route_digest"> {
  return {
    schema: AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA,
    source_id: input.sourceId,
    profile_id: input.profile.profile_id,
    profile_version: input.profile.version,
    profile_digest: input.profile.profile_digest,
    domain: input.profile.domain,
    provider: input.provider,
    source_kinds: [...input.sourceKinds],
    capabilities: [...input.capabilities],
    operations: [...input.operations],
    source_digest: input.sourceDigest,
    request_id: input.requestId,
    contract_digest: input.contractDigest,
    adapter_id: input.adapterId,
    adapter_manifest_digest: input.adapterManifestDigest,
    metadata_digest: input.metadataDigest,
    execution: "registered_route_only;source_dispatch_not_started",
    retention: "route_metadata_only;request_and_values_caller_owned",
    secret_material: "never_returned",
  };
}

function routeDigest(descriptor: Omit<AutonomousDomainEvidenceRouteJSON, "route_digest">): string {
  return digestJsonSync(descriptor);
}

function assertRouteJSON(route: AutonomousDomainEvidenceRouteJSON): void {
  if (!isObject(route) || route.schema !== AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA) throw new ArgumentError("domain evidence route projection is malformed");
  identifier("domain evidence route source_id", route.source_id);
  identifier("domain evidence route profile_id", route.profile_id);
  identifier("domain evidence route profile_version", route.profile_version);
  digest("domain evidence route profile_digest", route.profile_digest);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(route.domain)) throw new ArgumentError("domain evidence route domain is unsupported");
  identifier("domain evidence route provider", route.provider);
  boundedList("domain evidence route source_kinds", route.source_kinds, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS);
  boundedList("domain evidence route capabilities", route.capabilities, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES);
  boundedList("domain evidence route operations", route.operations, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS);
  digest("domain evidence route source_digest", route.source_digest, false);
  if (route.request_id !== null) identifier("domain evidence route request_id", route.request_id);
  digest("domain evidence route contract_digest", route.contract_digest, false);
  if (route.adapter_id !== null) identifier("domain evidence route adapter_id", route.adapter_id);
  digest("domain evidence route adapter_manifest_digest", route.adapter_manifest_digest, false);
  digest("domain evidence route metadata_digest", route.metadata_digest);
  digest("domain evidence route route_digest", route.route_digest);
  if (route.execution !== "registered_route_only;source_dispatch_not_started" || route.retention !== "route_metadata_only;request_and_values_caller_owned" || route.secret_material !== "never_returned") throw new ArgumentError("domain evidence route retention posture is invalid");
  const { route_digest: _routeDigest, ...descriptor } = route;
  if (digestJsonSync(descriptor) !== route.route_digest) throw new ArgumentError("domain evidence route digest does not match its metadata");
}

function internalRouteMetadata(route: AutonomousDomainEvidenceRouteJSON): JsonObject {
  return {
    [RESERVED_METADATA_KEY]: {
      profile_id: route.profile_id,
      profile_version: route.profile_version,
      profile_digest: route.profile_digest,
      domain: route.domain,
      provider: route.provider,
      source_kinds: [...route.source_kinds],
      capabilities: [...route.capabilities],
      operations: [...route.operations],
      contract_digest: route.contract_digest,
      adapter_id: route.adapter_id,
      adapter_manifest_digest: route.adapter_manifest_digest,
    },
  };
}

interface ProfileDefinition extends Omit<AutonomousDomainEvidenceProfileInput, "domain"> {
  domain: AutonomousDomainName;
}

const BUILTIN_PROFILE_DEFINITIONS: readonly ProfileDefinition[] = [
  {
    profileId: "builtin.coding.evidence",
    version: "1",
    domain: "coding",
    purpose: "Repository, change, test, and delivery evidence for engineering tasks.",
    sourceKinds: ["repository", "issue_tracker", "ci", "artifact_registry"],
    capabilities: ["review", "debugging", "implementation", "testing"],
    operations: ["repository_snapshot", "change_set", "test_run", "delivery_receipt"],
    requiredMetadata: ["operation"],
    freshness: "realtime",
    authMode: "caller_managed_credential",
    pagination: "cursor",
    normalizerId: "builtin.coding.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["source access and repository truth remain caller-owned", "test evidence is not inferred from a provider response"],
  },
  {
    profileId: "builtin.browser.evidence",
    version: "1",
    domain: "browser",
    purpose: "Fresh web retrieval, citation identity, and independent source comparison.",
    sourceKinds: ["web_search", "web_page", "archive", "feed"],
    capabilities: ["web_research", "navigation", "source_comparison"],
    operations: ["search", "retrieve", "compare", "freshness_check"],
    requiredMetadata: ["operation"],
    freshness: "realtime",
    authMode: "caller_managed_credential",
    pagination: "cursor",
    normalizerId: "builtin.browser.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["retrieval does not establish truth", "robots, access, freshness, and citation authority remain caller-owned"],
  },
  {
    profileId: "builtin.data.evidence",
    version: "1",
    domain: "data",
    purpose: "Dataset schema, lineage, quality, and transformation evidence.",
    sourceKinds: ["dataset", "schema_registry", "lineage_store", "quality_report"],
    capabilities: ["schema_validation", "lineage", "quality_control", "data_analysis"],
    operations: ["schema", "lineage", "quality", "profile", "transformation_check"],
    requiredMetadata: ["operation"],
    freshness: "bounded_cache",
    authMode: "caller_managed_credential",
    pagination: "page_number",
    normalizerId: "builtin.data.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["schema and lineage declarations are not independently verified by this catalogue", "raw rows remain outside the SDK projection"],
  },
  {
    profileId: "builtin.science.evidence",
    version: "1",
    domain: "science",
    purpose: "Literature, measurements, hypotheses, experimental design, and reproducibility evidence.",
    sourceKinds: ["literature", "registry", "measurement", "experiment_log"],
    capabilities: ["literature", "hypothesis", "experiment", "statistics", "reproducibility"],
    operations: ["literature_search", "evidence_map", "measurement", "design", "reproduction"],
    requiredMetadata: ["operation"],
    freshness: "historical",
    authMode: "caller_managed_credential",
    pagination: "cursor",
    normalizerId: "builtin.science.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["citation retrieval is not causal validation", "the evaluator must distinguish hypothesis, correlation, and causal claim"],
  },
  {
    profileId: "builtin.biomedical.evidence",
    version: "1",
    domain: "biomedical",
    purpose: "Biomedical provenance, population applicability, safety boundaries, and human-review evidence.",
    sourceKinds: ["literature", "guideline", "clinical_dataset", "safety_review"],
    capabilities: ["biomedical_review", "provenance", "safety_boundary", "human_review"],
    operations: ["evidence", "population", "provenance", "safety", "escalation"],
    requiredMetadata: ["operation"],
    freshness: "bounded_cache",
    authMode: "caller_managed_credential",
    pagination: "cursor",
    normalizerId: "builtin.biomedical.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["no diagnosis, prescription, triage, or clinical authorization", "individual decisions require qualified human and institutional review"],
  },
  {
    profileId: "builtin.neuroscience.evidence",
    version: "1",
    domain: "neuroscience",
    purpose: "Neural measurement, preprocessing, signal interpretation, model sensitivity, and reproducibility evidence.",
    sourceKinds: ["neuro_dataset", "signal_store", "literature", "study_registry"],
    capabilities: ["neuroscience_analysis", "signal_interpretation", "study_design", "reproducibility"],
    operations: ["measurement", "preprocess", "model", "interpretation", "reproduction"],
    requiredMetadata: ["operation"],
    freshness: "historical",
    authMode: "caller_managed_credential",
    pagination: "page_number",
    normalizerId: "builtin.neuroscience.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["signal transport and preprocessing are not supplied by the catalogue", "biological interpretation remains bounded by measurement and confound evidence"],
  },
  {
    profileId: "builtin.operations.evidence",
    version: "1",
    domain: "operations",
    purpose: "Telemetry, incidents, runbooks, blast radius, rollback, and approval evidence.",
    sourceKinds: ["telemetry", "incident_system", "runbook", "change_system"],
    capabilities: ["observability", "incident_response", "risk_review", "rollback", "approval", "runbook"],
    operations: ["observe", "impact", "rollback", "approval", "runbook"],
    requiredMetadata: ["operation"],
    freshness: "realtime",
    authMode: "delegated_session",
    pagination: "cursor",
    normalizerId: "builtin.operations.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["observations do not authorize an effect", "rollback and external state require a separate effect and reconciliation boundary"],
  },
  {
    profileId: "builtin.enterprise.evidence",
    version: "1",
    domain: "enterprise",
    purpose: "Business workflow, policy, compliance, ownership, and audit evidence.",
    sourceKinds: ["workflow", "policy_registry", "audit_log", "risk_register"],
    capabilities: ["workflow", "governance", "compliance", "analytics", "coordination"],
    operations: ["request", "policy", "options", "decision", "audit"],
    requiredMetadata: ["operation"],
    freshness: "bounded_cache",
    authMode: "delegated_session",
    pagination: "page_number",
    normalizerId: "builtin.enterprise.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["policy text is not authorization", "ownership and approval authority remain external organizational controls"],
  },
  {
    profileId: "builtin.multi-agent.evidence",
    version: "1",
    domain: "multi_agent",
    purpose: "Bounded delegation, specialist handoff, conflict reconciliation, and synthesis evidence.",
    sourceKinds: ["agent_report", "mission_log", "trace", "handoff"],
    capabilities: ["delegation", "coordination", "consensus", "conflict_resolution", "handoff"],
    operations: ["decompose", "delegate", "reconcile", "synthesize", "handoff"],
    requiredMetadata: ["operation"],
    freshness: "realtime",
    authMode: "delegated_session",
    pagination: "cursor",
    normalizerId: "builtin.multi-agent.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["agent agreement is not independent truth", "one accountable authority must own any external effect"],
  },
  {
    profileId: "builtin.multimodal.evidence",
    version: "1",
    domain: "multimodal",
    purpose: "Asset identity, modality transport, cross-modal alignment, and missing-modality evidence.",
    sourceKinds: ["image", "audio", "video", "document", "asset_registry"],
    capabilities: ["image", "audio", "video", "document", "cross_modal_alignment"],
    operations: ["asset", "modality", "transport", "alignment", "comparison"],
    requiredMetadata: ["operation"],
    freshness: "caller_declared",
    authMode: "caller_managed_credential",
    pagination: "none",
    normalizerId: "builtin.multimodal.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 1,
    defaultMaxConcurrency: 4,
    limitations: ["the catalogue does not inspect raw media", "absence of a modality must remain explicit rather than inferred away"],
  },
  {
    profileId: "builtin.cross-domain.evidence",
    version: "1",
    domain: "cross_domain",
    purpose: "Evidence alignment across domain specialists, synthesis inputs, and workflow composition.",
    sourceKinds: ["domain_evidence", "synthesis_input", "lineage", "workflow"],
    capabilities: ["routing", "synthesis", "evidence_alignment", "workflow_composition"],
    operations: ["route", "synthesis", "alignment", "composition"],
    requiredMetadata: ["operation"],
    freshness: "caller_declared",
    authMode: "delegated_session",
    pagination: "cursor",
    normalizerId: "builtin.cross-domain.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["cross-domain synthesis cannot erase specialist evaluator boundaries", "route composition does not grant domain authority"],
  },
  {
    profileId: "builtin.evaluation.evidence",
    version: "1",
    domain: "evaluation",
    purpose: "Benchmark, rubric, oracle, replay, failure, and reproducibility evidence.",
    sourceKinds: ["benchmark", "oracle", "replay", "evaluation_log"],
    capabilities: ["benchmarking", "rubric", "replay", "failure_analysis", "reproducibility"],
    operations: ["benchmark", "rubric", "replay", "failure", "reproduction"],
    requiredMetadata: ["operation"],
    freshness: "historical",
    authMode: "caller_managed_credential",
    pagination: "page_number",
    normalizerId: "builtin.evaluation.claim-projection",
    normalizerVersion: "1",
    defaultQuorum: 2,
    defaultMaxConcurrency: 4,
    limitations: ["the system under evaluation cannot author its own pass signal", "oracle independence and holdout integrity remain evaluator-owned"],
  },
];

function builtinProfiles(): AutonomousDomainEvidenceSourceProfile[] {
  return BUILTIN_PROFILE_DEFINITIONS.map((definition) => new AutonomousDomainEvidenceSourceProfile(definition));
}

/** Return the reviewed source-family profile for each autonomous domain. */
export function builtinAutonomousDomainEvidenceSourceProfiles(): AutonomousDomainEvidenceSourceProfile[] {
  return builtinProfiles().map((profile) => new AutonomousDomainEvidenceSourceProfile({
    profileId: profile.profile_id,
    version: profile.version,
    domain: profile.domain,
    purpose: profile.purpose,
    sourceKinds: profile.source_kinds,
    capabilities: profile.capabilities,
    operations: profile.operations,
    requiredMetadata: profile.required_metadata,
    freshness: profile.freshness,
    authMode: profile.auth_mode,
    pagination: profile.pagination,
    normalizerId: profile.normalizer_id,
    normalizerVersion: profile.normalizer_version,
    defaultQuorum: profile.default_quorum,
    defaultMaxConcurrency: profile.default_max_concurrency,
    limitations: profile.limitations,
  }));
}

function profileClone(profile: AutonomousDomainEvidenceSourceProfile): AutonomousDomainEvidenceSourceProfile {
  return new AutonomousDomainEvidenceSourceProfile({
    profileId: profile.profile_id,
    version: profile.version,
    domain: profile.domain,
    purpose: profile.purpose,
    sourceKinds: profile.source_kinds,
    capabilities: profile.capabilities,
    operations: profile.operations,
    requiredMetadata: profile.required_metadata,
    freshness: profile.freshness,
    authMode: profile.auth_mode,
    pagination: profile.pagination,
    normalizerId: profile.normalizer_id,
    normalizerVersion: profile.normalizer_version,
    defaultQuorum: profile.default_quorum,
    defaultMaxConcurrency: profile.default_max_concurrency,
    limitations: profile.limitations,
  });
}

function routeClone(route: AutonomousDomainEvidenceRoute): AutonomousDomainEvidenceRoute {
  return { json: structuredClone(route.json), metadata: structuredClone(route.metadata), acquirer: route.acquirer };
}

/**
 * Cross-domain source catalogue. It supplies the missing composition between a domain evidence
 * requirement and caller-owned source adapters. Registration is metadata-only; `prepare()` is
 * request-free; `execute()` delegates to the reviewed reconciler and still requires approval.
 */
export class AutonomousDomainEvidenceSourceCatalogue {
  private readonly profileEntries = new Map<string, AutonomousDomainEvidenceSourceProfile>();
  private readonly routeEntries = new Map<string, AutonomousDomainEvidenceRoute>();

  constructor(profiles: readonly AutonomousDomainEvidenceSourceProfile[] = builtinProfiles(), options: { requireAllDomains?: boolean } = {}) {
    if (!Array.isArray(profiles) || profiles.length < 1 || profiles.length > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES) throw new ArgumentError("domain evidence catalogue profiles are outside their bound");
    for (const profile of profiles) this.registerProfile(profile);
    if (options.requireAllDomains === true && AUTONOMOUS_DOMAIN_NAMES.some((domain) => ![...this.profileEntries.values()].some((profile) => profile.domain === domain))) throw new ArgumentError("domain evidence catalogue must cover every autonomous domain");
  }

  registerProfile(profile: AutonomousDomainEvidenceSourceProfile, options: { replace?: boolean } = {}): AutonomousDomainEvidenceProfileJSON {
    if (!(profile instanceof AutonomousDomainEvidenceSourceProfile)) throw new ArgumentError("domain evidence catalogue profile must be typed");
    const existing = this.profileEntries.get(profile.profile_id);
    if (existing && options.replace !== true) throw new ArgumentError(`domain evidence profile ${profile.profile_id} is already registered`);
    if (!existing && this.profileEntries.size >= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES) throw new ArgumentError("domain evidence catalogue profile capacity exceeded");
    if (existing && existing.domain !== profile.domain) throw new ArgumentError("replacing a domain evidence profile cannot change its domain");
    const dependent = [...this.routeEntries.values()].filter((route) => route.json.profile_id === profile.profile_id);
    if (dependent.length && existing?.profile_digest !== profile.profile_digest) throw new ArgumentError("cannot replace a domain evidence profile while routes bind its previous digest");
    this.profileEntries.set(profile.profile_id, profileClone(profile));
    this.assertSize();
    return structuredClone(profile.toJSON());
  }

  unregisterProfile(profileId: string): boolean {
    const id = identifier("domain evidence profileId", profileId);
    if ([...this.routeEntries.values()].some((route) => route.json.profile_id === id)) throw new ArgumentError("cannot unregister a domain evidence profile with registered routes");
    return this.profileEntries.delete(id);
  }

  profiles(): AutonomousDomainEvidenceProfileJSON[] {
    return [...this.profileEntries.values()].sort((left, right) => left.profile_id.localeCompare(right.profile_id)).map((profile) => structuredClone(profile.toJSON()));
  }

  profile(profileId: string): AutonomousDomainEvidenceSourceProfile {
    const profile = this.profileEntries.get(identifier("domain evidence profileId", profileId));
    if (!profile) throw new ArgumentError(`domain evidence profile is not registered: ${profileId}`);
    return profileClone(profile);
  }

  registerRoute(input: AutonomousDomainEvidenceRouteInput, options: { replace?: boolean } = {}): AutonomousDomainEvidenceRouteJSON {
    if (!input || typeof input !== "object") throw new ArgumentError("domain evidence route registration is malformed");
    if (typeof input.acquirer?.acquire !== "function") throw new ArgumentError("domain evidence route acquirer is required");
    const sourceId = identifier("domain evidence sourceId", input.sourceId);
    const profile = this.profile(input.profileId);
    const provider = identifier("domain evidence route provider", input.provider);
    const sourceKinds = boundedList("domain evidence route sourceKinds", input.sourceKinds ?? profile.source_kinds, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS);
    const capabilities = boundedList("domain evidence route capabilities", input.capabilities ?? profile.capabilities, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES);
    const operations = boundedList("domain evidence route operations", input.operations ?? profile.operations, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS);
    subset("domain evidence route sourceKinds", sourceKinds, profile.source_kinds);
    subset("domain evidence route capabilities", capabilities, profile.capabilities);
    subset("domain evidence route operations", operations, profile.operations);
    const metadata = safeMetadata(input.metadata, "domain evidence route metadata");
    for (const required of profile.required_metadata) if (metadata[required] === undefined) throw new ArgumentError(`domain evidence route metadata is missing required field: ${required}`);
    const sourceDigest = digest("domain evidence route sourceDigest", input.sourceDigest, false);
    const requestId = input.requestId === undefined || input.requestId === null ? null : identifier("domain evidence route requestId", input.requestId);
    const contractDigest = boundedContractDigest("domain evidence route contractDigest", input.contractDigest);
    const adapterId = input.adapterId === undefined || input.adapterId === null ? null : identifier("domain evidence route adapterId", input.adapterId);
    const adapterManifestDigest = input.adapterManifestDigest === undefined || input.adapterManifestDigest === null ? null : digest("domain evidence route adapterManifestDigest", input.adapterManifestDigest);
    if (adapterManifestDigest !== null && adapterId === null) throw new ArgumentError("domain evidence route adapterManifestDigest requires adapterId");
    const descriptor = routeDescriptor({ sourceId, profile, provider, sourceKinds, capabilities, operations, sourceDigest, requestId, contractDigest, adapterId, adapterManifestDigest, metadataDigest: digestJsonSync(metadata) });
    const route = { ...descriptor, route_digest: routeDigest(descriptor) } as AutonomousDomainEvidenceRouteJSON;
    const existing = this.routeEntries.get(sourceId);
    if (existing && options.replace !== true) throw new ArgumentError(`domain evidence source ${sourceId} is already registered`);
    if (!existing && this.routeEntries.size >= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES) throw new ArgumentError("domain evidence catalogue route capacity exceeded");
    this.routeEntries.set(sourceId, { json: route, metadata, acquirer: input.acquirer });
    this.assertSize();
    return structuredClone(route);
  }

  unregisterRoute(sourceId: string): boolean {
    return this.routeEntries.delete(identifier("domain evidence sourceId", sourceId));
  }

  routes(options: { domain?: AutonomousDomainName; profileId?: string } = {}): AutonomousDomainEvidenceRouteJSON[] {
    if (options.domain !== undefined && !AUTONOMOUS_DOMAIN_NAMES.includes(options.domain)) throw new ArgumentError("domain evidence route domain is unsupported");
    const profileId = options.profileId === undefined ? undefined : identifier("domain evidence route profileId", options.profileId);
    return [...this.routeEntries.values()]
      .filter((route) => (options.domain === undefined || route.json.domain === options.domain) && (profileId === undefined || route.json.profile_id === profileId))
      .sort((left, right) => left.json.source_id.localeCompare(right.json.source_id))
      .map((route) => structuredClone(route.json));
  }

  route(sourceId: string): AutonomousDomainEvidenceRouteJSON {
    const route = this.routeEntries.get(identifier("domain evidence sourceId", sourceId));
    if (!route) throw new ArgumentError(`domain evidence source is not registered: ${sourceId}`);
    return structuredClone(route.json);
  }

  coverage(): AutonomousDomainEvidenceCoverage[] {
    return AUTONOMOUS_DOMAIN_NAMES.map((domain) => {
      const routes = [...this.routeEntries.values()].filter((route) => route.json.domain === domain);
      const profiles = [...new Set(routes.map((route) => route.json.profile_id))].sort();
      const sourceIds = routes.map((route) => route.json.source_id).sort();
      const capabilities = [...new Set(routes.flatMap((route) => route.json.capabilities))].sort();
      return {
        domain,
        profile_ids: profiles,
        route_count: routes.length,
        source_ids: sourceIds,
        capabilities,
        state: routes.length === 0 ? "missing" : routes.length === 1 ? "partial" : "ready",
        retention: "metadata_only",
      } satisfies AutonomousDomainEvidenceCoverage;
    });
  }

  /** Select source routes and bind them to one requirement without calling an acquirer. */
  prepare(evidencePlan: AutonomousEvidencePlan, requirementId: string, options: AutonomousDomainEvidenceCataloguePrepareOptions = {}): AutonomousDomainEvidenceCatalogueReconciliation {
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("domain evidence preparation requires a typed evidence plan");
    const requirement = evidencePlan.requirements.find((candidate) => candidate.requirement_id === requirementId);
    if (!requirement) throw new ArgumentError(`domain evidence requirement is not in the plan: ${requirementId}`);
    const eligible = [...this.routeEntries.values()].filter((route) => route.json.domain === requirement.domain && (options.profileId === undefined || route.json.profile_id === options.profileId) && requirement.required_capabilities.every((capability) => route.json.capabilities.includes(capability)));
    let matching = eligible;
    if (options.sourceIds !== undefined) {
      const requested = new Set(sourceIdList("domain evidence sourceIds", options.sourceIds));
      if (requested.size === 0) throw new ArgumentError("domain evidence sourceIds cannot be empty");
      if ([...requested].some((sourceId) => !eligible.some((route) => route.json.source_id === sourceId))) throw new ArgumentError("domain evidence sourceIds contain an ineligible or unknown route");
      matching = eligible.filter((route) => requested.has(route.json.source_id));
    }
    if (!matching.length) throw new ArgumentError(`no registered source route satisfies evidence requirement ${requirementId}`);
    const profileIds = [...new Set(matching.map((route) => route.json.profile_id))];
    if (profileIds.length !== 1) throw new ArgumentError("domain evidence preparation requires one explicit profile when eligible routes span profiles");
    const profile = this.profile(profileIds[0]!);
    if (options.profileId !== undefined && options.profileId !== profile.profile_id) throw new ArgumentError("domain evidence preparation profile does not match eligible routes");
    const routes = matching.map((route) => this.reconciliationRoute(route));
    const reconciler = new AutonomousEvidenceSourceReconciler(evidencePlan);
    const plan = reconciler.prepare(requirementId, routes, {
      quorum: options.quorum ?? profile.default_quorum,
      maxConcurrency: Math.min(options.maxConcurrency ?? profile.default_max_concurrency, matching.length),
      requireAll: options.requireAll,
      normalizerId: profile.normalizer_id,
      normalizerVersion: profile.normalizer_version,
      parentEvidenceDigests: options.parentEvidenceDigests,
    });
    return { profile: structuredClone(profile.toJSON()), plan, routes: matching.map((route) => structuredClone(route.json)).sort((left, right) => left.source_id.localeCompare(right.source_id)) };
  }

  /** Execute a prepared catalogue plan through the existing bounded reconciler. */
  async execute(evidencePlan: AutonomousEvidencePlan, prepared: AutonomousDomainEvidenceCatalogueReconciliation, options: AutonomousDomainEvidenceCatalogueExecuteOptions = {}): Promise<AutonomousEvidenceReconciliationResult> {
    if (!(evidencePlan instanceof AutonomousEvidencePlan) || !prepared || !(prepared.plan instanceof AutonomousEvidenceReconciliationPlan)) throw new ArgumentError("domain evidence execution requires a typed prepared reconciliation");
    const profile = this.profile(prepared.profile.profile_id);
    if (options.profileId !== undefined && options.profileId !== profile.profile_id) throw new ArgumentError("domain evidence execution profile does not match the prepared reconciliation");
    if (prepared.profile.profile_digest !== profile.profile_digest || prepared.profile.normalizer_id !== profile.normalizer_id || prepared.profile.normalizer_version !== profile.normalizer_version) throw new ArgumentError("domain evidence profile changed after preparation");
    const routeIds = prepared.routes.map((route) => route.source_id);
    const routeEntries = routeIds.map((sourceId) => {
      const route = this.routeEntries.get(sourceId);
      if (!route) throw new ArgumentError(`domain evidence source route disappeared after preparation: ${sourceId}`);
      if (route.json.route_digest !== prepared.routes.find((candidate) => candidate.source_id === sourceId)?.route_digest) throw new ArgumentError(`domain evidence source route changed after preparation: ${sourceId}`);
      return this.reconciliationRoute(route);
    });
    const reconciler = new AutonomousEvidenceSourceReconciler(evidencePlan);
    return reconciler.execute(prepared.plan, routeEntries, {
      approveSourceDispatch: options.approveSourceDispatch,
      normalizer: options.normalizer,
      normalizerId: profile.normalizer_id,
      normalizerVersion: profile.normalizer_version,
    });
  }

  /** Convenience method that keeps preparation and approved execution visibly separate. */
  async reconcile(evidencePlan: AutonomousEvidencePlan, requirementId: string, options: AutonomousDomainEvidenceCataloguePrepareOptions & AutonomousDomainEvidenceCatalogueExecuteOptions = {}): Promise<AutonomousEvidenceReconciliationResult> {
    const prepared = this.prepare(evidencePlan, requirementId, options);
    return this.execute(evidencePlan, prepared, options);
  }

  toJSON(): AutonomousDomainEvidenceCatalogueJSON {
    const profiles = this.profiles();
    const routes = this.routes();
    const coverage = this.coverage();
    const descriptor = {
      schema: AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA,
      profiles,
      routes,
      coverage,
      profile_count: profiles.length,
      route_count: routes.length,
      covered_domain_count: coverage.filter((row) => row.state !== "missing").length,
      execution: "catalogue_and_route_validation_only;source_dispatch_requires_review" as const,
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    if (bytes(canonicalJson(descriptor)) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_BYTES) throw new ArgumentError("domain evidence catalogue exceeds its byte bound");
    return { ...descriptor, registry_digest: digestJsonSync(descriptor) };
  }

  private reconciliationRoute(route: AutonomousDomainEvidenceRoute): {
    source_id: string;
    source_digest: string | null;
    request_id: string | null;
    metadata: JsonObject;
    acquirer: AutonomousEvidenceAcquirer;
  } {
    return {
      source_id: route.json.source_id,
      source_digest: route.json.source_digest,
      request_id: route.json.request_id,
      metadata: { ...structuredClone(route.metadata), ...internalRouteMetadata(route.json) },
      acquirer: route.acquirer,
    };
  }

  private assertSize(): void {
    this.toJSON();
  }
}

/** Build a catalogue with one reviewed source profile for every autonomous domain. */
export function createBuiltinAutonomousDomainEvidenceSourceCatalogue(): AutonomousDomainEvidenceSourceCatalogue {
  return new AutonomousDomainEvidenceSourceCatalogue(builtinProfiles(), { requireAllDomains: true });
}

/** Type-only helper for source adapters that want to inspect the reviewed request identity. */
export function domainEvidenceRequestIdentity(context: AutonomousEvidenceAcquisitionContext): JsonObject {
  return {
    plan_digest: context.plan_digest,
    requirement_id: context.requirement.requirement_id,
    source_id: context.request.source_id,
    source_digest: context.request.source_digest ?? null,
    request_id: context.request.request_id ?? null,
    metadata_digest: digestJsonSync(context.request.metadata ?? {}),
    attempt: context.attempt,
    parent_evidence_digests: [...context.parent_evidence_digests],
    execution: context.execution,
    retention: "identity_only;raw_request_and_value_caller_owned",
    secret_material: "never_returned",
  };
}
