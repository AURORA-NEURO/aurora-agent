import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousDomainName,
} from "./autonomous.js";
import {
  AutonomousDomainEvidenceSourceCatalogue,
  builtinAutonomousDomainEvidenceSourceProfiles,
  type AutonomousDomainEvidenceFreshnessMode,
  type AutonomousDomainEvidencePaginationMode,
} from "./autonomous-domain-evidence-catalogue.js";
import {
  AutonomousEvidenceAdapterRegistry,
} from "./autonomous-evidence-adapters.js";
import {
  AutonomousEvidenceProviderContractRegistry,
  type AutonomousEvidenceProviderAuthMode,
  type AutonomousEvidenceProviderProtocol,
} from "./autonomous-evidence-provider-contract.js";
import {
  registerAutonomousDomainHttpEvidenceSource,
  type AutonomousDomainHttpEvidenceSourceOptions,
  type AutonomousDomainHttpEvidenceSourceRegistration,
} from "./autonomous-domain-http-source.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only identity for the reviewed provider-neutral HTTP source presets. */
export const AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA = "bioprism-typescript-autonomous-domain-http-source-preset/0.1" as const;
/** Metadata-only result for one preset registration. */
export const AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA = "bioprism-typescript-autonomous-domain-http-source-preset-registration/0.1" as const;
/** Metadata-only result for registering a complete domain source matrix. */
export const AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA = "bioprism-typescript-autonomous-domain-http-source-matrix/0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESETS = 64;
export const MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_ENTRIES = 128;
export const MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES = 16_000;

const PRESET_EXECUTION = "preset_metadata_only;caller_transport_and_source_interpretation_required" as const;
const PRESET_RETENTION = "preset_metadata_only;credentials_requests_and_source_values_caller_owned" as const;

export interface AutonomousDomainHttpSourcePreset extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA;
  preset_id: string;
  version: string;
  profile_id: string;
  profile_digest: string;
  domain: AutonomousDomainName;
  provider_protocol: AutonomousEvidenceProviderProtocol;
  default_provider: string;
  default_adapter_id: string;
  default_contract_id: string;
  source_kinds: string[];
  capabilities: string[];
  operations: string[];
  required_metadata: string[];
  freshness: AutonomousDomainEvidenceFreshnessMode;
  auth_mode: AutonomousEvidenceProviderAuthMode;
  pagination: AutonomousDomainEvidencePaginationMode;
  normalizer_id: string;
  normalizer_version: string;
  limitations: string[];
  execution: typeof PRESET_EXECUTION;
  retention: typeof PRESET_RETENTION;
  secret_material: "never_returned";
  preset_digest: string;
}

export interface AutonomousDomainHttpSourcePresetRegistrationOptions extends Omit<
  AutonomousDomainHttpEvidenceSourceOptions,
  "catalogue" | "profileId" | "provider" | "adapterId" | "adapterVersion" | "capabilities" | "sourceKinds" | "operations" | "providerContract" | "metadata" | "replace"
> {
  catalogue: AutonomousDomainEvidenceSourceCatalogue;
  preset: AutonomousDomainHttpSourcePreset | string;
  sourceId: string;
  provider?: string;
  adapterId?: string;
  adapterVersion?: string;
  contractId?: string;
  contractVersion?: string;
  capabilities?: readonly string[];
  sourceKinds?: readonly string[];
  operations?: readonly string[];
  metadata?: JsonObject;
  replace?: boolean;
}

export interface AutonomousDomainHttpSourcePresetRegistration extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA;
  preset_id: string;
  preset_digest: string;
  registration: AutonomousDomainHttpEvidenceSourceRegistration;
  execution: "registered_only;HTTP_dispatch_requires_catalogue_approval";
  retention: "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousDomainHttpSourceMatrixEntry extends Omit<
  AutonomousDomainHttpSourcePresetRegistrationOptions,
  "catalogue" | "adapterRegistry" | "providerContractRegistry" | "replace"
> {}

export interface AutonomousDomainHttpSourceMatrixOptions {
  catalogue: AutonomousDomainEvidenceSourceCatalogue;
  entries: readonly AutonomousDomainHttpSourceMatrixEntry[];
  adapterRegistry?: AutonomousEvidenceAdapterRegistry;
  providerContractRegistry?: AutonomousEvidenceProviderContractRegistry;
  replace?: boolean;
  requireAllDomains?: boolean;
}

export interface AutonomousDomainHttpSourceMatrixRegistration extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA;
  preset_count: number;
  registrations: AutonomousDomainHttpSourcePresetRegistration[];
  coverage: ReturnType<AutonomousDomainEvidenceSourceCatalogue["coverage"]>;
  adapter_registry_digest: string | null;
  provider_contract_registry_digest: string | null;
  execution: "registered_only;HTTP_dispatch_requires_catalogue_approval";
  retention: "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned";
  secret_material: "never_returned";
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function presetFromProfile(profile: ReturnType<typeof builtinAutonomousDomainEvidenceSourceProfiles>[number]): AutonomousDomainHttpSourcePreset {
  const domain = profile.domain;
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA,
    preset_id: `builtin.http.${domain}`,
    version: profile.version,
    profile_id: profile.profile_id,
    profile_digest: profile.profile_digest,
    domain,
    provider_protocol: "http_json" as const,
    default_provider: `caller-http-${domain}`,
    default_adapter_id: `builtin.http.${domain}.adapter`,
    default_contract_id: `builtin.http.${domain}.contract`,
    source_kinds: [...profile.source_kinds],
    capabilities: [...profile.capabilities],
    operations: [...profile.operations],
    required_metadata: [...profile.required_metadata],
    freshness: profile.freshness,
    auth_mode: profile.auth_mode,
    pagination: profile.pagination,
    normalizer_id: profile.normalizer_id,
    normalizer_version: profile.normalizer_version,
    limitations: [...profile.limitations],
    execution: PRESET_EXECUTION,
    retention: PRESET_RETENTION,
    secret_material: "never_returned" as const,
  };
  const result = { ...descriptor, preset_digest: digestJsonSync(descriptor) };
  if (new TextEncoder().encode(JSON.stringify(result)).byteLength > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES) throw new ArgumentError("domain HTTP source preset exceeds its metadata bound");
  return result;
}

function assertPreset(value: unknown): AutonomousDomainHttpSourcePreset {
  if (!isObject(value) || value.schema !== AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA) throw new ArgumentError("domain HTTP source preset is malformed");
  const preset = value as AutonomousDomainHttpSourcePreset;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(preset.domain)) throw new ArgumentError("domain HTTP source preset domain is unsupported");
  if (typeof preset.preset_digest !== "string" || !/^[0-9a-f]{64}$/.test(preset.preset_digest)) throw new ArgumentError("domain HTTP source preset digest is malformed");
  const { preset_digest: _presetDigest, ...descriptor } = preset;
  if (digestJsonSync(descriptor) !== preset.preset_digest) throw new ArgumentError("domain HTTP source preset digest does not match its metadata");
  if (new TextEncoder().encode(JSON.stringify(preset)).byteLength > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES) throw new ArgumentError("domain HTTP source preset exceeds its metadata bound");
  if (preset.provider_protocol !== "http_json") throw new ArgumentError("domain HTTP source preset must use the bounded HTTP JSON protocol");
  if (preset.execution !== PRESET_EXECUTION || preset.retention !== PRESET_RETENTION || preset.secret_material !== "never_returned") throw new ArgumentError("domain HTTP source preset retention posture is invalid");
  return clone(preset);
}

function resolvePreset(catalogue: AutonomousDomainEvidenceSourceCatalogue, value: AutonomousDomainHttpSourcePreset | string): AutonomousDomainHttpSourcePreset {
  if (!(catalogue instanceof AutonomousDomainEvidenceSourceCatalogue)) throw new ArgumentError("domain HTTP source preset requires a typed catalogue");
  const preset = typeof value === "string"
    ? builtinAutonomousDomainHttpSourcePresets().find((candidate) => candidate.preset_id === value)
    : assertPreset(value);
  if (!preset) throw new ArgumentError(`unknown domain HTTP source preset: ${String(value)}`);
  const profile = catalogue.profile(preset.profile_id);
  if (profile.domain !== preset.domain || profile.profile_digest !== preset.profile_digest) throw new ArgumentError("domain HTTP source preset is stale or bound to a different catalogue profile");
  return preset;
}

function selectedList(name: string, supplied: readonly string[] | undefined, allowed: readonly string[]): readonly string[] {
  if (supplied === undefined) return [...allowed];
  if (!Array.isArray(supplied) || supplied.length < 1 || supplied.some((value) => typeof value !== "string" || !value.trim())) throw new ArgumentError(`${name} must be a non-empty string list`);
  if (new Set(supplied).size !== supplied.length || supplied.some((value) => !allowed.includes(value))) throw new ArgumentError(`${name} must be a unique subset of the preset contract`);
  return [...supplied];
}

function registrationOptions(
  options: AutonomousDomainHttpSourcePresetRegistrationOptions,
  preset: AutonomousDomainHttpSourcePreset,
): AutonomousDomainHttpEvidenceSourceOptions {
  const sourceKinds = selectedList("domain HTTP source sourceKinds", options.sourceKinds, preset.source_kinds);
  const capabilities = selectedList("domain HTTP source capabilities", options.capabilities, preset.capabilities);
  const operations = selectedList("domain HTTP source operations", options.operations, preset.operations);
  const metadata = { ...(options.metadata ?? {}) } as JsonObject;
  if (metadata.operation === undefined) metadata.operation = operations[0]!;
  if (typeof metadata.operation !== "string" || !operations.includes(metadata.operation)) throw new ArgumentError("domain HTTP source metadata.operation must be one of the selected preset operations");
  const providerContract = options.providerContractRegistry === undefined
    ? undefined
    : {
      contractId: options.contractId ?? preset.default_contract_id,
      version: options.contractVersion ?? preset.version,
      protocol: preset.provider_protocol,
      operations,
      authMode: preset.auth_mode,
      freshness: preset.freshness,
      pagination: preset.pagination,
      requiredMetadata: [...preset.required_metadata],
      operationMetadataKey: preset.required_metadata.includes("operation") ? "operation" : null,
    };
  return {
    ...options,
    catalogue: options.catalogue,
    profileId: preset.profile_id,
    sourceId: options.sourceId,
    provider: options.provider ?? preset.default_provider,
    adapterId: options.adapterId ?? preset.default_adapter_id,
    adapterVersion: options.adapterVersion ?? preset.version,
    capabilities,
    sourceKinds,
    operations,
    metadata,
    providerContract,
  };
}

/** Return one caller-safe HTTP source preset for each autonomous domain. */
export function builtinAutonomousDomainHttpSourcePresets(): AutonomousDomainHttpSourcePreset[] {
  const profiles = builtinAutonomousDomainEvidenceSourceProfiles();
  if (profiles.length > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESETS) throw new ArgumentError("built-in domain HTTP source preset capacity exceeded");
  return profiles.map((profile) => presetFromProfile(profile));
}

/** Register one caller-implemented HTTP source using a reviewed domain preset. */
export function registerAutonomousDomainHttpSourcePreset(
  options: AutonomousDomainHttpSourcePresetRegistrationOptions,
): AutonomousDomainHttpSourcePresetRegistration {
  if (!options || typeof options !== "object") throw new ArgumentError("domain HTTP source preset registration is malformed");
  const preset = resolvePreset(options.catalogue, options.preset);
  const registration = registerAutonomousDomainHttpEvidenceSource(registrationOptions(options, preset));
  return {
    schema: AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA,
    preset_id: preset.preset_id,
    preset_digest: preset.preset_digest,
    registration,
    execution: registration.execution,
    retention: registration.retention,
    secret_material: registration.secret_material,
  };
}

/**
 * Register a complete source matrix without invoking any source. Each entry still needs a
 * caller endpoint resolver, request builder, optional header resolver, and response projector.
 * The default requires one route for every built-in domain so readiness cannot look complete
 * because only a narrow domain subset was configured.
 */
export function registerAutonomousDomainHttpSourceMatrix(
  options: AutonomousDomainHttpSourceMatrixOptions,
): AutonomousDomainHttpSourceMatrixRegistration {
  if (!options || typeof options !== "object" || !(options.catalogue instanceof AutonomousDomainEvidenceSourceCatalogue)) throw new ArgumentError("domain HTTP source matrix requires a typed catalogue");
  if (!Array.isArray(options.entries) || options.entries.length < 1 || options.entries.length > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_ENTRIES) throw new ArgumentError("domain HTTP source matrix entries are outside their bound");
  if (options.adapterRegistry !== undefined && !(options.adapterRegistry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("domain HTTP source matrix adapterRegistry is malformed");
  if (options.providerContractRegistry !== undefined && !(options.providerContractRegistry instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("domain HTTP source matrix providerContractRegistry is malformed");
  if (options.providerContractRegistry !== undefined && options.adapterRegistry !== undefined && options.providerContractRegistry.adapterRegistry !== options.adapterRegistry) throw new ArgumentError("domain HTTP source matrix registries do not match");
  const resolved = options.entries.map((entry) => resolvePreset(options.catalogue, entry.preset));
  const domains = new Set(resolved.map((preset) => preset.domain));
  if (options.requireAllDomains !== false && AUTONOMOUS_DOMAIN_NAMES.some((domain) => !domains.has(domain))) throw new ArgumentError("domain HTTP source matrix must cover every autonomous domain");
  const sourceIds = options.entries.map((entry) => entry.sourceId);
  if (new Set(sourceIds).size !== sourceIds.length) throw new ArgumentError("domain HTTP source matrix contains duplicate source IDs");
  const registrations = options.entries.map((entry, index) => registerAutonomousDomainHttpSourcePreset({
    ...entry,
    catalogue: options.catalogue,
    adapterRegistry: options.adapterRegistry,
    providerContractRegistry: options.providerContractRegistry,
    replace: options.replace,
    preset: resolved[index]!,
  }));
  return {
    schema: AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA,
    preset_count: registrations.length,
    registrations,
    coverage: options.catalogue.coverage(),
    adapter_registry_digest: options.adapterRegistry?.toJSON().registry_digest ?? null,
    provider_contract_registry_digest: options.providerContractRegistry?.toJSON().registry_digest ?? null,
    execution: "registered_only;HTTP_dispatch_requires_catalogue_approval",
    retention: "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned",
    secret_material: "never_returned",
  };
}
