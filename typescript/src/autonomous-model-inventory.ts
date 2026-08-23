import { ArgumentError, isObject } from "./errors.js";
import {
  CredentialSession,
  type CredentialHandle,
} from "./llm.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA,
  AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS,
  AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
  AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS,
  AutonomousAgent,
  builtinAutonomousDomainProfiles,
  validateAutonomousModelCatalogueSnapshot,
  type AutonomousDomainName,
  type AutonomousModelCatalogueRefreshResult,
  type AutonomousModelCatalogueSnapshot,
  type AutonomousModelRefreshSpec,
} from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { AutonomousModelCandidate } from "./llm.js";

/** Schema for a redacted provider inventory plus all-domain selection coverage. */
export const AUTONOMOUS_MODEL_INVENTORY_SCHEMA = "bioprism-typescript-autonomous-model-inventory/0.1" as const;
export const AUTONOMOUS_MODEL_INVENTORY_MAX_PROVIDERS = AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS;
export const AUTONOMOUS_MODEL_INVENTORY_MAX_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const AUTONOMOUS_MODEL_INVENTORY_MAX_SNAPSHOT_BYTES = 8_000_000;

export type AutonomousModelInventoryStatus = "completed" | "partial" | "failed";
export type AutonomousModelInventoryCoverageState = "complete" | "partial" | "missing";

/** Domain-level model readiness derived from explicit model capabilities and provider status. */
export interface AutonomousModelInventoryCoverage {
  schema: typeof AUTONOMOUS_MODEL_INVENTORY_SCHEMA;
  domain: AutonomousDomainName;
  required_model_capabilities: string[];
  compatible_model_ids: string[];
  eligible_model_ids: string[];
  compatible_model_count: number;
  eligible_model_count: number;
  coverage_state: AutonomousModelInventoryCoverageState;
  provider_readiness: Record<string, {
    registered: boolean;
    credential_ready: boolean;
    circuit: string;
  }>;
}

/** Digest-bound metadata-only inventory snapshot suitable for caller-owned persistence. */
export interface AutonomousModelInventorySnapshot {
  schema: typeof AUTONOMOUS_MODEL_INVENTORY_SCHEMA;
  refresh_id: string;
  status: AutonomousModelInventoryStatus;
  refresh: AutonomousModelCatalogueRefreshResult;
  models: AutonomousModelCandidate[];
  domains: AutonomousModelInventoryCoverage[];
  catalogue_digest: string;
  domain_coverage_digest: string;
  inventory_digest: string;
  readiness: "ready" | "partial" | "missing";
  execution: "provider_discovery_and_catalogue_reconciliation_only";
  selection_posture: "candidate_metadata_and_provider_readiness_only; evaluator_evidence_still_required";
  retention: "model_metadata_and_coverage_only;credentials_prompts_responses_and_raw_catalogues_not_retained";
  secret_material: "never_returned";
}

/** Caller-owned durable adapter for SQLite, IndexedDB, Postgres, or object storage. */
export interface AutonomousModelInventoryPersistence {
  read(): Promise<AutonomousModelInventorySnapshot | null> | AutonomousModelInventorySnapshot | null;
  write(snapshot: AutonomousModelInventorySnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedInventoryDigest: string | null, snapshot: AutonomousModelInventorySnapshot): Promise<boolean> | boolean;
}

export interface AutonomousModelInventorySnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousModelInventoryTransactionalSnapshotTextStore extends AutonomousModelInventorySnapshotTextStore {
  writeIfUnchanged(expectedInventoryDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousModelInventoryRefreshOptions {
  /** Optional active protected onboarding session; raw credential values never enter this API. */
  credentialSession?: CredentialSession;
  credentialFor?: (provider: string) => CredentialHandle | undefined;
  replaceExisting?: boolean;
  maxParallel?: number;
  stopOnError?: boolean;
  /** Override required model capabilities for selected domains; omitted domains use built-in profiles. */
  domainRequirements?: Partial<Record<AutonomousDomainName, readonly string[]>>;
  estimatedInputTokens?: number;
  requestedOutputTokens?: number;
  refreshId?: string;
  persistence?: AutonomousModelInventoryPersistence;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown): string {
  const digest = boundedText(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return digest;
}

function boundedPositiveInteger(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function boundedNonNegativeInteger(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function boundedCapabilities(name: string, values: readonly string[]): string[] {
  if (!Array.isArray(values) || values.length > 128) throw new ArgumentError(`${name} is outside its bounds`);
  const normalized = values.map((value) => boundedIdentifier(`${name} capability`, value));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicate capabilities`);
  return [...normalized].sort();
}

function modelId(candidate: AutonomousModelCandidate): string {
  return `${candidate.provider}/${candidate.model}`;
}

function candidateSupports(candidate: AutonomousModelCandidate, required: readonly string[], inputTokens: number, outputTokens: number): boolean {
  if (candidate.enabled === false) return false;
  if (candidate.context_window_tokens < inputTokens + outputTokens || candidate.max_output_tokens < outputTokens) return false;
  const capabilities = new Set(candidate.capabilities ?? []);
  return required.every((capability) => capabilities.has(capability));
}

/** Validate an inventory snapshot before it can be restored into a live model catalogue. */
export async function validateAutonomousModelInventorySnapshot(value: unknown): Promise<AutonomousModelInventorySnapshot> {
  if (!isObject(value)) throw new ArgumentError("autonomous model inventory snapshot must be an object");
  const allowedKeys = new Set([
    "schema", "refresh_id", "status", "refresh", "models", "domains", "catalogue_digest",
    "domain_coverage_digest", "inventory_digest", "readiness", "execution", "selection_posture",
    "retention", "secret_material",
  ]);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError("autonomous model inventory snapshot contains unsupported metadata");
  if (value.schema !== AUTONOMOUS_MODEL_INVENTORY_SCHEMA || value.execution !== "provider_discovery_and_catalogue_reconciliation_only" || value.selection_posture !== "candidate_metadata_and_provider_readiness_only; evaluator_evidence_still_required" || value.retention !== "model_metadata_and_coverage_only;credentials_prompts_responses_and_raw_catalogues_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous model inventory snapshot markers are invalid");
  const refreshId = boundedIdentifier("autonomous model inventory refresh_id", value.refresh_id);
  if (value.status !== "completed" && value.status !== "partial" && value.status !== "failed") throw new ArgumentError("autonomous model inventory status is invalid");
  if (!isObject(value.refresh) || value.refresh.schema !== AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA) throw new ArgumentError("autonomous model inventory refresh projection is malformed");
  if (value.refresh.status !== value.status) throw new ArgumentError("autonomous model inventory status does not match its refresh projection");
  if (!Array.isArray(value.models) || value.models.length > AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) throw new ArgumentError("autonomous model inventory model catalogue exceeds its bound");
  const catalogueDigest = boundedDigest("autonomous model inventory catalogue_digest", value.catalogue_digest);
  const models = value.models as AutonomousModelCandidate[];
  const catalogueDescriptor = {
    schema: AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
    models,
    catalogue_digest: catalogueDigest,
    retention: "model_metadata_only_hash_bound" as const,
    secret_material: "never_returned" as const,
  };
  const catalogueSnapshot: AutonomousModelCatalogueSnapshot = {
    ...catalogueDescriptor,
    snapshot_digest: await digestJson(catalogueDescriptor),
  };
  await validateAutonomousModelCatalogueSnapshot(catalogueSnapshot);
  if (!Array.isArray(value.domains) || value.domains.length !== AUTONOMOUS_MODEL_INVENTORY_MAX_DOMAINS) throw new ArgumentError("autonomous model inventory domain coverage is incomplete");
  const domains = value.domains as AutonomousModelInventoryCoverage[];
  const catalogueIds = new Set(models.map(modelId));
  const domainIds = new Set<string>();
  for (const rawDomain of domains) {
    if (!isObject(rawDomain)) throw new ArgumentError("autonomous model inventory coverage row is malformed");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(rawDomain.domain as AutonomousDomainName)) throw new ArgumentError("autonomous model inventory coverage contains an unknown domain");
    const domain = rawDomain.domain as AutonomousDomainName;
    if (domainIds.has(domain)) throw new ArgumentError("autonomous model inventory coverage contains duplicate domains");
    domainIds.add(domain);
    if (rawDomain.schema !== AUTONOMOUS_MODEL_INVENTORY_SCHEMA) throw new ArgumentError("autonomous model inventory coverage schema is invalid");
    if (!Array.isArray(rawDomain.required_model_capabilities) || !rawDomain.required_model_capabilities.every((item): item is string => typeof item === "string")) throw new ArgumentError("autonomous model inventory required capabilities are malformed");
    boundedCapabilities(`autonomous model inventory ${domain} requirements`, rawDomain.required_model_capabilities);
    if (!Array.isArray(rawDomain.compatible_model_ids) || !Array.isArray(rawDomain.eligible_model_ids)) throw new ArgumentError("autonomous model inventory model coverage is malformed");
    const compatible = rawDomain.compatible_model_ids.map((item) => boundedText("autonomous model inventory compatible model id", item, 768));
    const eligible = rawDomain.eligible_model_ids.map((item) => boundedText("autonomous model inventory eligible model id", item, 768));
    if (new Set(compatible).size !== compatible.length || new Set(eligible).size !== eligible.length) throw new ArgumentError("autonomous model inventory model coverage contains duplicate arms");
    if (eligible.some((item) => !compatible.includes(item))) throw new ArgumentError("autonomous model inventory eligible models must be compatible");
    if (compatible.some((item) => !catalogueIds.has(item)) || eligible.some((item) => !catalogueIds.has(item))) throw new ArgumentError("autonomous model inventory coverage references an unknown model");
    if (boundedNonNegativeInteger("autonomous model inventory compatible_model_count", rawDomain.compatible_model_count, AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) !== compatible.length || boundedNonNegativeInteger("autonomous model inventory eligible_model_count", rawDomain.eligible_model_count, AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) !== eligible.length) throw new ArgumentError("autonomous model inventory coverage counts do not match model ids");
    if (rawDomain.coverage_state !== "complete" && rawDomain.coverage_state !== "partial" && rawDomain.coverage_state !== "missing") throw new ArgumentError("autonomous model inventory coverage state is invalid");
    const expectedState = eligible.length > 0 ? "complete" : compatible.length > 0 ? "partial" : "missing";
    if (rawDomain.coverage_state !== expectedState) throw new ArgumentError("autonomous model inventory coverage state does not match its model ids");
    if (!isObject(rawDomain.provider_readiness)) throw new ArgumentError("autonomous model inventory provider readiness is malformed");
    for (const [provider, readiness] of Object.entries(rawDomain.provider_readiness)) {
      boundedIdentifier("autonomous model inventory provider", provider);
      if (!isObject(readiness) || typeof readiness.registered !== "boolean" || typeof readiness.credential_ready !== "boolean") throw new ArgumentError("autonomous model inventory provider readiness row is malformed");
      boundedText("autonomous model inventory provider circuit", readiness.circuit, 64);
    }
  }
  if (domainIds.size !== AUTONOMOUS_MODEL_INVENTORY_MAX_DOMAINS) throw new ArgumentError("autonomous model inventory coverage is missing a built-in domain");
  const domainCoverageDigest = boundedDigest("autonomous model inventory domain_coverage_digest", value.domain_coverage_digest);
  if (await digestJson(domains) !== domainCoverageDigest) throw new ArgumentError("autonomous model inventory domain coverage digest mismatch");
  if (value.readiness !== "ready" && value.readiness !== "partial" && value.readiness !== "missing") throw new ArgumentError("autonomous model inventory readiness is invalid");
  const expectedReadiness = domains.every((row) => row.coverage_state === "complete")
    ? "ready"
    : domains.some((row) => row.coverage_state !== "missing") ? "partial" : "missing";
  if (value.readiness !== expectedReadiness) throw new ArgumentError("autonomous model inventory readiness does not match domain coverage");
  const descriptor = {
    schema: AUTONOMOUS_MODEL_INVENTORY_SCHEMA,
    refresh_id: refreshId,
    status: value.status as AutonomousModelInventoryStatus,
    refresh: value.refresh as unknown as AutonomousModelCatalogueRefreshResult,
    models,
    domains,
    catalogue_digest: catalogueDigest,
    domain_coverage_digest: domainCoverageDigest,
    readiness: value.readiness as "ready" | "partial" | "missing",
    execution: "provider_discovery_and_catalogue_reconciliation_only" as const,
    selection_posture: "candidate_metadata_and_provider_readiness_only; evaluator_evidence_still_required" as const,
    retention: "model_metadata_and_coverage_only;credentials_prompts_responses_and_raw_catalogues_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  const inventoryDigest = boundedDigest("autonomous model inventory inventory_digest", value.inventory_digest);
  if (await digestJson(descriptor) !== inventoryDigest) throw new ArgumentError("autonomous model inventory digest mismatch");
  const snapshot = { ...descriptor, inventory_digest: inventoryDigest } satisfies AutonomousModelInventorySnapshot;
  if (new TextEncoder().encode(JSON.stringify(snapshot)).byteLength > AUTONOMOUS_MODEL_INVENTORY_MAX_SNAPSHOT_BYTES) throw new ArgumentError("autonomous model inventory snapshot exceeds its byte capacity");
  return structuredClone(snapshot);
}

/**
 * Synchronizes discovered provider models with the autonomous selector and reports exact
 * all-domain coverage. Discovery metadata is useful for routing, but quality/cost priors still
 * come from the caller and compatible models are not treated as successful task outcomes.
 */
export class AutonomousModelInventoryCoordinator {
  private expectedInventoryDigest: string | null = null;
  private expectedPersistence: AutonomousModelInventoryPersistence | undefined;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly agent: AutonomousAgent, readonly persistence?: AutonomousModelInventoryPersistence) {
    if (!(agent instanceof AutonomousAgent)) throw new ArgumentError("model inventory requires an AutonomousAgent");
    if (persistence !== undefined && (typeof persistence.read !== "function" || typeof persistence.write !== "function")) throw new ArgumentError("model inventory persistence adapter is malformed");
  }

  async refresh(specs: readonly AutonomousModelRefreshSpec[], options: AutonomousModelInventoryRefreshOptions = {}): Promise<AutonomousModelInventorySnapshot> {
    return this.enqueue(() => this.refreshInternal(specs, options));
  }

  private async refreshInternal(specs: readonly AutonomousModelRefreshSpec[], options: AutonomousModelInventoryRefreshOptions): Promise<AutonomousModelInventorySnapshot> {
    if (!Array.isArray(specs) || specs.length < 1 || specs.length > AUTONOMOUS_MODEL_INVENTORY_MAX_PROVIDERS) throw new ArgumentError(`model inventory refresh must contain 1..=${AUTONOMOUS_MODEL_INVENTORY_MAX_PROVIDERS} providers`);
    if (options.credentialSession !== undefined && !(options.credentialSession instanceof CredentialSession)) throw new ArgumentError("model inventory credentialSession is malformed");
    const estimatedInputTokens = boundedPositiveInteger("model inventory estimatedInputTokens", options.estimatedInputTokens ?? 4_096, 10_000_000);
    const requestedOutputTokens = boundedPositiveInteger("model inventory requestedOutputTokens", options.requestedOutputTokens ?? 1_024, 10_000_000);
    const refreshId = boundedIdentifier("model inventory refreshId", options.refreshId ?? `inventory-${Date.now().toString(36)}`);
    const refresh = await this.agent.refreshModelCatalogue(specs, {
      credentialFor: options.credentialFor ?? (options.credentialSession
        ? (provider) => {
          const metadata = this.agent.llm.providerMetadata().find((row) => row.provider === provider);
          return metadata?.requires_credential === false ? undefined : options.credentialSession!.handle(provider);
        }
        : undefined),
      replaceExisting: options.replaceExisting,
      maxParallel: options.maxParallel,
      stopOnError: options.stopOnError,
    });
    const profiles = await builtinAutonomousDomainProfiles();
    const models = this.agent.models();
    const metadataByProvider = new Map(this.agent.llm.providerMetadata().map((row) => [String(row.provider), row]));
    const providerState = new Map<string, { registered: boolean; credentialReady: boolean; circuit: string }>();
    const providerNames = [...new Set([...metadataByProvider.keys(), ...models.map((candidate) => candidate.provider)])].sort();
    for (const provider of providerNames) {
      const metadata = metadataByProvider.get(provider);
      const registered = metadata !== undefined;
      const requiresCredential = typeof metadata?.requires_credential === "boolean" ? metadata.requires_credential : true;
      const credential = this.agent.llm.credentials.status(provider, registered);
      const health = registered ? this.agent.llm.providerStatus(provider) : null;
      providerState.set(provider, { registered, credentialReady: !requiresCredential || credential.ready === true, circuit: health?.circuit ?? "unconfigured" });
    }
    const domains: AutonomousModelInventoryCoverage[] = profiles.map((profile) => {
      const override = options.domainRequirements?.[profile.domain];
      const required = boundedCapabilities(`model inventory ${profile.domain} requirements`, override ?? profile.required_model_capabilities);
      const compatible = models.filter((candidate) => candidateSupports(candidate, required, estimatedInputTokens, requestedOutputTokens));
      const eligible = compatible.filter((candidate) => {
        const state = providerState.get(candidate.provider);
        return state?.registered === true && state.credentialReady && state.circuit !== "open";
      });
      const providers = Object.fromEntries([...new Set(compatible.map((candidate) => candidate.provider))].sort().map((provider) => {
        const state = providerState.get(provider) ?? { registered: false, credentialReady: false, circuit: "unconfigured" };
        return [provider, { registered: state.registered, credential_ready: state.credentialReady, circuit: state.circuit }];
      }));
      return {
        schema: AUTONOMOUS_MODEL_INVENTORY_SCHEMA,
        domain: profile.domain,
        required_model_capabilities: required,
        compatible_model_ids: compatible.map(modelId).sort(),
        eligible_model_ids: eligible.map(modelId).sort(),
        compatible_model_count: compatible.length,
        eligible_model_count: eligible.length,
        coverage_state: eligible.length > 0 ? "complete" : compatible.length > 0 ? "partial" : "missing",
        provider_readiness: providers,
      };
    });
    const catalogueDigest = await digestJson(models);
    const domainCoverageDigest = await digestJson(domains);
    const readiness = domains.every((row) => row.coverage_state === "complete")
      ? "ready"
      : domains.some((row) => row.coverage_state !== "missing") ? "partial" : "missing";
    const descriptor = {
      schema: AUTONOMOUS_MODEL_INVENTORY_SCHEMA,
      refresh_id: refreshId,
      status: refresh.status,
      refresh,
      models,
      domains,
      catalogue_digest: catalogueDigest,
      domain_coverage_digest: domainCoverageDigest,
      readiness,
      execution: "provider_discovery_and_catalogue_reconciliation_only" as const,
      selection_posture: "candidate_metadata_and_provider_readiness_only; evaluator_evidence_still_required" as const,
      retention: "model_metadata_and_coverage_only;credentials_prompts_responses_and_raw_catalogues_not_retained" as const,
      secret_material: "never_returned" as const,
    };
    const snapshot = await validateAutonomousModelInventorySnapshot({ ...descriptor, inventory_digest: await digestJson(descriptor) });
    const persistence = options.persistence ?? this.persistence;
    if (persistence) {
      this.selectPersistence(persistence);
      if (typeof persistence.writeIfUnchanged === "function") {
        if (!await persistence.writeIfUnchanged(this.expectedInventoryDigest, snapshot)) throw new ArgumentError("model inventory persistence compare-and-swap conflict");
      } else await persistence.write(snapshot);
      this.expectedInventoryDigest = snapshot.inventory_digest;
    }
    return snapshot;
  }

  async restore(persistence: AutonomousModelInventoryPersistence = this.persistence as AutonomousModelInventoryPersistence): Promise<AutonomousModelInventorySnapshot | null> {
    return this.enqueue(() => this.restoreInternal(persistence));
  }

  private async restoreInternal(persistence: AutonomousModelInventoryPersistence): Promise<AutonomousModelInventorySnapshot | null> {
    if (!persistence || typeof persistence.read !== "function") throw new ArgumentError("model inventory restore requires persistence");
    this.selectPersistence(persistence);
    const raw = await persistence.read();
    if (raw === null) {
      this.expectedInventoryDigest = null;
      return null;
    }
    const snapshot = await validateAutonomousModelInventorySnapshot(raw);
    const catalogueSnapshot: AutonomousModelCatalogueSnapshot = {
      schema: AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
      models: snapshot.models,
      catalogue_digest: snapshot.catalogue_digest,
      snapshot_digest: await digestJson({ schema: AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA, models: snapshot.models, catalogue_digest: snapshot.catalogue_digest, retention: "model_metadata_only_hash_bound", secret_material: "never_returned" }),
      retention: "model_metadata_only_hash_bound",
      secret_material: "never_returned",
    };
    await this.agent.restoreModels(catalogueSnapshot);
    this.expectedInventoryDigest = snapshot.inventory_digest;
    return snapshot;
  }

  private selectPersistence(persistence: AutonomousModelInventoryPersistence): void {
    if (this.expectedPersistence !== persistence) {
      this.expectedPersistence = persistence;
      this.expectedInventoryDigest = null;
    }
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousModelInventorySnapshotPersistence implements AutonomousModelInventoryPersistence {
  constructor(readonly textStore: AutonomousModelInventorySnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("model inventory text store is malformed");
  }

  async read(): Promise<AutonomousModelInventorySnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_MODEL_INVENTORY_MAX_SNAPSHOT_BYTES) throw new ArgumentError("model inventory JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("model inventory JSON is invalid"); }
    return validateAutonomousModelInventorySnapshot(parsed);
  }

  async write(raw: AutonomousModelInventorySnapshot): Promise<void> {
    const snapshot = await validateAutonomousModelInventorySnapshot(raw);
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousModelInventorySnapshotPersistence extends JsonAutonomousModelInventorySnapshotPersistence {
  declare readonly textStore: AutonomousModelInventoryTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousModelInventoryTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("model inventory text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedInventoryDigest: string | null, raw: AutonomousModelInventorySnapshot): Promise<boolean> {
    if (expectedInventoryDigest !== null && !/^[0-9a-f]{64}$/.test(expectedInventoryDigest)) throw new ArgumentError("model inventory expected inventory digest is invalid");
    const snapshot = await validateAutonomousModelInventorySnapshot(raw);
    return this.textStore.writeIfUnchanged(expectedInventoryDigest, canonicalJson(snapshot));
  }
}
