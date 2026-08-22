import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterManifest,
} from "./autonomous-evidence-adapters.js";
import type {
  AutonomousEvidenceAcquirer,
  AutonomousEvidenceAcquisitionContext,
} from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Explicit provider/source semantics bound to a caller-owned evidence adapter. */
export const AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA = "bioprism-typescript-autonomous-evidence-provider-contract/0.1" as const;
export const AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-evidence-provider-contract-registry/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACTS = 256;
export const MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_BYTES = 512_000;

export const AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS = [
  "http_json",
  "graphql",
  "openai_responses",
  "openai_chat_completions",
  "anthropic_messages",
  "caller_defined",
] as const;
export const AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES = [
  "none",
  "caller_managed_credential",
  "caller_signed_request",
  "delegated_session",
] as const;
export const AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES = [
  "realtime",
  "bounded_cache",
  "historical",
  "caller_declared",
] as const;
export const AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES = [
  "none",
  "cursor",
  "page_number",
  "link_header",
  "caller_defined",
] as const;

export type AutonomousEvidenceProviderProtocol = typeof AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS[number];
export type AutonomousEvidenceProviderAuthMode = typeof AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES[number];
export type AutonomousEvidenceProviderFreshnessMode = typeof AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES[number];
export type AutonomousEvidenceProviderPaginationMode = typeof AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES[number];

const RETENTION = "manifest_and_contract_metadata_only;credentials_and_raw_source_values_caller_owned" as const;
const SECRET_KEYS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "privatekey", "refreshtoken"]);

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return text;
}

function metadataKey(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:+_-]+$/.test(text)) throw new ArgumentError(`${name} is outside its metadata-key contract`);
  const normalized = text.toLowerCase().replace(/[^a-z0-9]/g, "");
  if (SECRET_KEYS.has(normalized) || normalized.includes("token") || normalized.includes("secret") || normalized.includes("credential") || normalized.includes("authorization")) throw new ArgumentError(`${name} cannot be credential-shaped`);
  return text;
}

function boundedMetadataKeys(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS) throw new ArgumentError(`${name} must contain at most ${MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS} entries`);
  const result = value.map((item, index) => metadataKey(`${name}[${index}]`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return [...result].sort();
}

function boundedList(name: string, value: unknown, maximum: number, sort = true): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  const result = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return sort ? [...result].sort() : result;
}

function boundedDomains(value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("provider contract domains are outside their bound");
  const result = value.map((domain, index) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`provider contract domain ${index} is unsupported`);
    return domain as AutonomousDomainName;
  });
  if (new Set(result).size !== result.length) throw new ArgumentError("provider contract domains contain duplicates");
  return [...result].sort();
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function subset(name: string, values: readonly string[], allowed: readonly string[]): void {
  const permitted = new Set(allowed);
  const missing = values.filter((value) => !permitted.has(value));
  if (missing.length) throw new ArgumentError(`${name} exceeds the bound adapter contract: ${missing.join(", ")}`);
}

function contractDescriptor(input: {
  contractId: string;
  version: string;
  provider: string;
  protocol: AutonomousEvidenceProviderProtocol;
  operations: string[];
  domains: AutonomousDomainName[];
  capabilities: string[];
  sourceKinds: string[];
  authMode: AutonomousEvidenceProviderAuthMode;
  freshness: AutonomousEvidenceProviderFreshnessMode;
  pagination: AutonomousEvidenceProviderPaginationMode;
  requiredMetadata: string[];
  operationMetadataKey: string | null;
  adapterId: string;
  adapterManifestDigest: string;
  adapterRegistryDigest: string;
}): JsonObject {
  return {
    schema: AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA,
    contract_id: input.contractId,
    version: input.version,
    provider: input.provider,
    protocol: input.protocol,
    operations: [...input.operations],
    domains: [...input.domains],
    capabilities: [...input.capabilities],
    source_kinds: [...input.sourceKinds],
    auth_mode: input.authMode,
    freshness: input.freshness,
    pagination: input.pagination,
    required_metadata: [...input.requiredMetadata],
    operation_metadata_key: input.operationMetadataKey,
    adapter_id: input.adapterId,
    adapter_manifest_digest: input.adapterManifestDigest,
    adapter_registry_digest: input.adapterRegistryDigest,
    retention: RETENTION,
    secret_material: "never_returned",
  };
}

export interface AutonomousEvidenceProviderContractJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA;
  contract_id: string;
  version: string;
  provider: string;
  protocol: AutonomousEvidenceProviderProtocol;
  operations: string[];
  domains: AutonomousDomainName[];
  capabilities: string[];
  source_kinds: string[];
  auth_mode: AutonomousEvidenceProviderAuthMode;
  freshness: AutonomousEvidenceProviderFreshnessMode;
  pagination: AutonomousEvidenceProviderPaginationMode;
  required_metadata: string[];
  operation_metadata_key: string | null;
  adapter_id: string;
  adapter_manifest_digest: string;
  adapter_registry_digest: string;
  contract_digest: string;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceProviderContractInput {
  contractId: string;
  version: string;
  provider: string;
  protocol: AutonomousEvidenceProviderProtocol;
  operations: readonly string[];
  domains: readonly AutonomousDomainName[];
  capabilities: readonly string[];
  sourceKinds: readonly string[];
  authMode: AutonomousEvidenceProviderAuthMode;
  freshness: AutonomousEvidenceProviderFreshnessMode;
  pagination: AutonomousEvidenceProviderPaginationMode;
  requiredMetadata?: readonly string[];
  operationMetadataKey?: string | null;
  adapterId: string;
}

export class AutonomousEvidenceProviderContract {
  readonly contract_id: string;
  readonly version: string;
  readonly provider: string;
  readonly protocol: AutonomousEvidenceProviderProtocol;
  readonly operations: string[];
  readonly domains: AutonomousDomainName[];
  readonly capabilities: string[];
  readonly source_kinds: string[];
  readonly auth_mode: AutonomousEvidenceProviderAuthMode;
  readonly freshness: AutonomousEvidenceProviderFreshnessMode;
  readonly pagination: AutonomousEvidenceProviderPaginationMode;
  readonly required_metadata: string[];
  readonly operation_metadata_key: string | null;
  readonly adapter_id: string;
  readonly adapter_manifest_digest: string;
  readonly adapter_registry_digest: string;
  readonly contract_digest: string;

  constructor(input: AutonomousEvidenceProviderContractInput & { adapter: AutonomousEvidenceAdapterManifest; adapterRegistryDigest: string }) {
    if (!input || typeof input !== "object") throw new ArgumentError("provider evidence contract input is malformed");
    if (!AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS.includes(input.protocol)) throw new ArgumentError("provider evidence contract protocol is invalid");
    if (!AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES.includes(input.authMode)) throw new ArgumentError("provider evidence contract auth mode is invalid");
    if (!AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES.includes(input.freshness)) throw new ArgumentError("provider evidence contract freshness is invalid");
    if (!AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES.includes(input.pagination)) throw new ArgumentError("provider evidence contract pagination is invalid");
    if (!input.adapter || input.adapter.adapter_id !== input.adapterId) throw new ArgumentError("provider evidence contract adapter binding is malformed");
    const contractId = identifier("provider evidence contract contractId", input.contractId);
    const version = identifier("provider evidence contract version", input.version);
    const provider = identifier("provider evidence contract provider", input.provider);
    const operations = boundedList("provider evidence contract operations", input.operations, MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS);
    const domains = boundedDomains(input.domains);
    const capabilities = boundedList("provider evidence contract capabilities", input.capabilities, 64);
    const sourceKinds = boundedList("provider evidence contract sourceKinds", input.sourceKinds, 32);
    const requiredMetadata = input.requiredMetadata === undefined ? [] : boundedMetadataKeys("provider evidence contract requiredMetadata", input.requiredMetadata);
    const operationMetadataKey = input.operationMetadataKey === undefined || input.operationMetadataKey === null
      ? null
      : metadataKey("provider evidence contract operationMetadataKey", input.operationMetadataKey);
    if (operationMetadataKey !== null && !requiredMetadata.includes(operationMetadataKey)) throw new ArgumentError("provider evidence contract operationMetadataKey must be required metadata");
    subset("provider evidence contract domains", domains, input.adapter.domains);
    subset("provider evidence contract capabilities", capabilities, input.adapter.capabilities);
    subset("provider evidence contract sourceKinds", sourceKinds, input.adapter.source_kinds);
    const adapterRegistryDigest = digest("provider evidence contract adapterRegistryDigest", input.adapterRegistryDigest);
    const descriptor = contractDescriptor({
      contractId,
      version,
      provider,
      protocol: input.protocol,
      operations,
      domains,
      capabilities,
      sourceKinds,
      authMode: input.authMode,
      freshness: input.freshness,
      pagination: input.pagination,
      requiredMetadata,
      operationMetadataKey,
      adapterId: input.adapter.adapter_id,
      adapterManifestDigest: input.adapter.manifest_digest,
      adapterRegistryDigest,
    });
    this.contract_id = contractId;
    this.version = version;
    this.provider = provider;
    this.protocol = input.protocol;
    this.operations = operations;
    this.domains = domains;
    this.capabilities = capabilities;
    this.source_kinds = sourceKinds;
    this.auth_mode = input.authMode;
    this.freshness = input.freshness;
    this.pagination = input.pagination;
    this.required_metadata = requiredMetadata;
    this.operation_metadata_key = operationMetadataKey;
    this.adapter_id = input.adapter.adapter_id;
    this.adapter_manifest_digest = input.adapter.manifest_digest;
    this.adapter_registry_digest = adapterRegistryDigest;
    this.contract_digest = digestJsonSync(descriptor);
  }

  toJSON(): AutonomousEvidenceProviderContractJSON {
    return {
      ...contractDescriptor({
        contractId: this.contract_id,
        version: this.version,
        provider: this.provider,
        protocol: this.protocol,
        operations: this.operations,
        domains: this.domains,
        capabilities: this.capabilities,
        sourceKinds: this.source_kinds,
        authMode: this.auth_mode,
        freshness: this.freshness,
        pagination: this.pagination,
        requiredMetadata: this.required_metadata,
        operationMetadataKey: this.operation_metadata_key,
        adapterId: this.adapter_id,
        adapterManifestDigest: this.adapter_manifest_digest,
        adapterRegistryDigest: this.adapter_registry_digest,
      }),
      contract_digest: this.contract_digest,
    } as AutonomousEvidenceProviderContractJSON;
  }
}

export interface AutonomousEvidenceProviderContractCoverage extends JsonObject {
  domain: AutonomousDomainName;
  contract_ids: string[];
  providers: string[];
  protocols: AutonomousEvidenceProviderProtocol[];
  state: "complete" | "missing";
}

export interface AutonomousEvidenceProviderContractRegistryJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA;
  adapter_registry_digest: string;
  contracts: AutonomousEvidenceProviderContractJSON[];
  coverage: AutonomousEvidenceProviderContractCoverage[];
  registry_digest: string;
  execution: "registry_projection_only;contract_validation_no_source_dispatch";
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

function metadataValue(context: AutonomousEvidenceAcquisitionContext, key: string): unknown {
  return isObject(context.request.metadata) ? context.request.metadata[key] : undefined;
}

/**
 * Registry that turns provider-specific source assumptions into executable, digest-bound
 * validation. It stores no credentials and never performs network work; the adapter remains
 * caller-owned, while this layer makes its declared protocol and request contract enforceable.
 */
export class AutonomousEvidenceProviderContractRegistry {
  private readonly entries = new Map<string, AutonomousEvidenceProviderContract>();

  constructor(readonly adapterRegistry: AutonomousEvidenceAdapterRegistry) {
    if (!(adapterRegistry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("provider contract registry requires a typed adapter registry");
  }

  register(input: AutonomousEvidenceProviderContractInput, options: { replace?: boolean } = {}): AutonomousEvidenceProviderContractJSON {
    if (!input || typeof input !== "object") throw new ArgumentError("provider evidence contract registration is malformed");
    const adapter = this.adapterRegistry.manifests().find((candidate) => candidate.adapter_id === input.adapterId);
    if (!adapter) throw new ArgumentError(`provider evidence contract references unknown adapter: ${input.adapterId}`);
    const contract = new AutonomousEvidenceProviderContract({
      ...input,
      adapter,
      adapterRegistryDigest: this.adapterRegistry.toJSON().registry_digest,
    });
    const existing = this.entries.get(contract.contract_id);
    if (existing && options.replace !== true) throw new ArgumentError(`provider evidence contract ${contract.contract_id} is already registered`);
    if (!existing && this.entries.size >= MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACTS) throw new ArgumentError("provider evidence contract registry is full");
    const conflicting = [...this.entries.values()].find((candidate) => candidate.contract_id !== contract.contract_id && candidate.adapter_id === contract.adapter_id && candidate.domains.some((domain) => contract.domains.includes(domain)));
    if (conflicting) throw new ArgumentError(`provider evidence contract overlaps adapter/domain binding ${conflicting.contract_id}`);
    this.entries.set(contract.contract_id, contract);
    this.assertSize();
    return structuredClone(contract.toJSON());
  }

  unregister(contractId: string): boolean {
    return this.entries.delete(identifier("provider evidence contract contractId", contractId));
  }

  contracts(): AutonomousEvidenceProviderContractJSON[] {
    return [...this.entries.values()].sort((left, right) => left.contract_id.localeCompare(right.contract_id)).map((contract) => structuredClone(contract.toJSON()));
  }

  resolve(contractId: string): AutonomousEvidenceProviderContract {
    const contract = this.entries.get(identifier("provider evidence contract contractId", contractId));
    if (!contract) throw new ArgumentError(`provider evidence contract ${contractId} is not registered`);
    return contract;
  }

  coverage(): AutonomousEvidenceProviderContractCoverage[] {
    return AUTONOMOUS_DOMAIN_NAMES.map((domain) => {
      const contracts = [...this.entries.values()].filter((contract) => contract.domains.includes(domain));
      return {
        domain,
        contract_ids: contracts.map((contract) => contract.contract_id).sort(),
        providers: [...new Set(contracts.map((contract) => contract.provider))].sort(),
        protocols: [...new Set(contracts.map((contract) => contract.protocol))].sort(),
        state: contracts.length > 0 ? "complete" : "missing",
      } satisfies AutonomousEvidenceProviderContractCoverage;
    });
  }

  /** Verify adapter and contract identities before a reviewed execution is allowed to proceed. */
  verify(): this {
    const current = this.adapterRegistry.toJSON().registry_digest;
    for (const contract of this.entries.values()) {
      if (contract.adapter_registry_digest !== current) throw new ArgumentError("provider evidence contract adapter registry is stale or tampered");
      const adapter = this.adapterRegistry.manifests().find((candidate) => candidate.adapter_id === contract.adapter_id);
      if (!adapter || adapter.manifest_digest !== contract.adapter_manifest_digest) throw new ArgumentError(`provider evidence contract adapter binding changed: ${contract.contract_id}`);
      if (contract.domains.some((domain) => !adapter.domains.includes(domain)) || contract.capabilities.some((capability) => !adapter.capabilities.includes(capability)) || contract.source_kinds.some((sourceKind) => !adapter.source_kinds.includes(sourceKind))) throw new ArgumentError(`provider evidence contract exceeds its live adapter binding: ${contract.contract_id}`);
    }
    return this;
  }

  /** Return the single explicit provider contract bound to an adapter/domain route. */
  contractForAdapter(adapterId: string, domain: AutonomousDomainName): AutonomousEvidenceProviderContract {
    const normalizedAdapterId = identifier("provider evidence contract adapterId", adapterId);
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("provider evidence contract domain is unsupported");
    this.verify();
    const matches = [...this.entries.values()].filter((contract) => contract.adapter_id === normalizedAdapterId && contract.domains.includes(domain));
    if (matches.length !== 1) throw new ArgumentError(matches.length === 0 ? `no provider evidence contract is bound to ${normalizedAdapterId}/${domain}` : `provider evidence contract binding is ambiguous for ${normalizedAdapterId}/${domain}`);
    return matches[0]!;
  }

  /** Wrap one selected adapter so every attempt enforces provider protocol/request semantics. */
  createAcquirerForAdapter(adapterId: string, domain: AutonomousDomainName): AutonomousEvidenceAcquirer {
    const normalizedAdapterId = identifier("provider evidence contract adapterId", adapterId);
    const contract = this.contractForAdapter(normalizedAdapterId, domain);
    const base = this.adapterRegistry.createAcquirer({ adapterIdForDomain: { [domain]: normalizedAdapterId } as Partial<Record<AutonomousDomainName, string>> });
    return {
      acquire: async (context) => {
        this.verify();
        if (context.requirement.domain !== domain) throw new ArgumentError("provider evidence contract acquirer received a different domain");
        const liveContract = this.contractForAdapter(normalizedAdapterId, context.requirement.domain);
        if (liveContract.contract_digest !== contract.contract_digest) throw new ArgumentError("provider evidence contract changed after acquirer creation");
        if (!liveContract.domains.includes(context.requirement.domain)) throw new ArgumentError("provider evidence contract does not cover the requested domain");
        subset("provider evidence contract required capabilities", context.requirement.required_capabilities, liveContract.capabilities);
        for (const key of liveContract.required_metadata) {
          if (metadataValue(context, key) === undefined) throw new ArgumentError(`provider evidence request is missing required metadata: ${key}`);
        }
        if (liveContract.operation_metadata_key !== null) {
          const operation = metadataValue(context, liveContract.operation_metadata_key);
          if (typeof operation !== "string" || !liveContract.operations.includes(operation)) throw new ArgumentError(`provider evidence request operation is not declared by ${liveContract.contract_id}`);
        }
        return base.acquire(context);
      },
    };
  }

  toJSON(): AutonomousEvidenceProviderContractRegistryJSON {
    this.verify();
    const contracts = this.contracts();
    const coverage = this.coverage();
    const descriptor = {
      schema: AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA,
      adapter_registry_digest: this.adapterRegistry.toJSON().registry_digest,
      contracts,
      coverage,
      execution: "registry_projection_only;contract_validation_no_source_dispatch" as const,
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    if (new TextEncoder().encode(canonicalJson(descriptor)).byteLength > MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_BYTES) throw new ArgumentError("provider evidence contract registry exceeds its bound");
    return { ...descriptor, registry_digest: digestJsonSync(descriptor) };
  }

  private assertSize(): void {
    this.toJSON();
  }
}
