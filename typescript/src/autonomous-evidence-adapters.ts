import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import type {
  AutonomousEvidenceAcquisitionContext,
  AutonomousEvidenceAcquirer,
  AutonomousEvidenceObservationInput,
  AutonomousEvidenceProjector,
} from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Typed domain-scoped source acquisition registry for the value-only evidence runtime. */
export const AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-registry/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-manifest/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTERS = 256;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES = 64;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_BYTES = 256_000;

const RETENTION = "manifest_only;credentials_and_raw_source_values_never_persisted" as const;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return text;
}

function boundedList(name: string, value: readonly string[] | undefined, maximum: number): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} is outside its bound`);
  const normalized = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return [...normalized].sort();
}

function boundedDomains(value: readonly AutonomousDomainName[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_DOMAINS) throw new ArgumentError("evidence adapter domains are outside their bound");
  const domains = value.map((domain, index) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`evidence adapter domain ${index} is unsupported`);
    return domain;
  });
  if (new Set(domains).size !== domains.length) throw new ArgumentError("evidence adapter domains contain duplicates");
  return [...domains].sort();
}

export interface AutonomousEvidenceAdapterManifest extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA;
  adapter_id: string;
  version: string;
  domains: AutonomousDomainName[];
  capabilities: string[];
  source_kinds: string[];
  execution: "caller_owned_source_adapter;raw_value_transient";
  retention: typeof RETENTION;
  secret_material: "never_returned";
  manifest_digest: string;
}

export interface AutonomousEvidenceAdapterCoverage extends JsonObject {
  domain: AutonomousDomainName;
  adapter_ids: string[];
  capability_union: string[];
  state: "complete" | "missing";
}

export interface AutonomousEvidenceAdapterRegistryJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA;
  adapters: AutonomousEvidenceAdapterManifest[];
  coverage: AutonomousEvidenceAdapterCoverage[];
  registry_digest: string;
  coverage_digest: string;
  execution: "registry_projection_only;no_source_dispatch";
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterRegistrationInput {
  adapterId: string;
  version: string;
  domains: readonly AutonomousDomainName[];
  capabilities: readonly string[];
  sourceKinds: readonly string[];
  acquire: (context: AutonomousEvidenceAcquisitionContext) => JsonValue | Promise<JsonValue>;
  project?: AutonomousEvidenceProjector["project"];
}

interface AdapterEntry {
  readonly manifest: AutonomousEvidenceAdapterManifest;
  readonly acquire: AutonomousEvidenceAdapterRegistrationInput["acquire"];
  readonly project?: AutonomousEvidenceProjector["project"];
}

interface AutonomousEvidenceAdapterManifestDescriptor {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA;
  adapter_id: string;
  version: string;
  domains: AutonomousDomainName[];
  capabilities: string[];
  source_kinds: string[];
  execution: "caller_owned_source_adapter;raw_value_transient";
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

function manifestDescriptor(input: {
  adapterId: string;
  version: string;
  domains: AutonomousDomainName[];
  capabilities: string[];
  sourceKinds: string[];
}): AutonomousEvidenceAdapterManifestDescriptor {
  return {
    schema: AUTONOMOUS_EVIDENCE_ADAPTER_MANIFEST_SCHEMA,
    adapter_id: input.adapterId,
    version: input.version,
    domains: input.domains,
    capabilities: input.capabilities,
    source_kinds: input.sourceKinds,
    execution: "caller_owned_source_adapter;raw_value_transient",
    retention: RETENTION,
    secret_material: "never_returned",
  };
}

function manifestFor(input: AutonomousEvidenceAdapterRegistrationInput): AutonomousEvidenceAdapterManifest {
  const descriptor = manifestDescriptor({
    adapterId: identifier("evidence adapter adapterId", input.adapterId),
    version: identifier("evidence adapter version", input.version),
    domains: boundedDomains(input.domains),
    capabilities: boundedList("evidence adapter capabilities", input.capabilities, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_CAPABILITIES),
    sourceKinds: boundedList("evidence adapter sourceKinds", input.sourceKinds, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SOURCE_KINDS),
  });
  return {
    schema: descriptor.schema,
    adapter_id: descriptor.adapter_id,
    version: descriptor.version,
    domains: [...descriptor.domains],
    capabilities: [...descriptor.capabilities],
    source_kinds: [...descriptor.source_kinds],
    execution: descriptor.execution,
    retention: descriptor.retention,
    secret_material: descriptor.secret_material,
    manifest_digest: digestJsonSync(descriptor),
  };
}

/**
 * Registry that binds every source adapter to an explicit domain scope. The registry itself
 * stores only manifests and function references; source credentials and acquired values remain
 * inside the caller-owned adapter closure and the transient evidence runtime invocation.
 */
export class AutonomousEvidenceAdapterRegistry {
  private readonly entries = new Map<string, AdapterEntry>();

  register(input: AutonomousEvidenceAdapterRegistrationInput, options: { replace?: boolean } = {}): AutonomousEvidenceAdapterManifest {
    if (!input || typeof input.acquire !== "function") throw new ArgumentError("evidence adapter registration requires an acquire function");
    if (input.project !== undefined && typeof input.project !== "function") throw new ArgumentError("evidence adapter project function is malformed");
    const manifest = manifestFor(input);
    const existing = this.entries.get(manifest.adapter_id);
    if (existing && options.replace !== true) throw new ArgumentError(`evidence adapter ${manifest.adapter_id} is already registered`);
    if (!existing && this.entries.size >= MAX_AUTONOMOUS_EVIDENCE_ADAPTERS) throw new ArgumentError("evidence adapter registry is full");
    this.entries.set(manifest.adapter_id, { manifest, acquire: input.acquire, project: input.project });
    this.assertSize();
    return structuredClone(manifest);
  }

  unregister(adapterId: string): boolean {
    return this.entries.delete(identifier("evidence adapter adapterId", adapterId));
  }

  manifests(): AutonomousEvidenceAdapterManifest[] {
    return [...this.entries.values()].map((entry) => structuredClone(entry.manifest)).sort((left, right) => left.adapter_id.localeCompare(right.adapter_id));
  }

  coverage(): AutonomousEvidenceAdapterCoverage[] {
    return AUTONOMOUS_DOMAIN_NAMES.map((domain) => {
      const entries = [...this.entries.values()].filter((entry) => entry.manifest.domains.includes(domain));
      return {
        domain,
        adapter_ids: entries.map((entry) => entry.manifest.adapter_id).sort(),
        capability_union: [...new Set(entries.flatMap((entry) => entry.manifest.capabilities))].sort(),
        state: entries.length > 0 ? "complete" : "missing",
      } satisfies AutonomousEvidenceAdapterCoverage;
    });
  }

  resolve(domain: AutonomousDomainName, adapterId?: string): AutonomousEvidenceAdapterManifest {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("evidence adapter resolution domain is unsupported");
    const candidates = [...this.entries.values()].filter((entry) => entry.manifest.domains.includes(domain));
    if (adapterId !== undefined) {
      const selected = this.entries.get(identifier("evidence adapter selected adapterId", adapterId));
      if (!selected || !selected.manifest.domains.includes(domain)) throw new ArgumentError(`evidence adapter ${adapterId} is not registered for ${domain}`);
      return structuredClone(selected.manifest);
    }
    if (candidates.length === 0) throw new ArgumentError(`no evidence adapter is registered for ${domain}`);
    if (candidates.length > 1) throw new ArgumentError(`evidence adapter selection is ambiguous for ${domain}`);
    return structuredClone(candidates[0]!.manifest);
  }

  createAcquirer(options: { adapterIdForDomain?: Partial<Record<AutonomousDomainName, string>> } = {}): AutonomousEvidenceAcquirer {
    if (!options || (options.adapterIdForDomain !== undefined && typeof options.adapterIdForDomain !== "object")) throw new ArgumentError("evidence adapter acquirer options are malformed");
    return {
      acquire: async (context) => {
        const domain = context?.requirement?.domain;
        if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("evidence adapter acquire context has an unsupported domain");
        const requestedId = options.adapterIdForDomain?.[domain];
        const manifest = this.resolve(domain, requestedId);
        const entry = this.entries.get(manifest.adapter_id);
        if (!entry) throw new ArgumentError("evidence adapter disappeared during resolution");
        return entry.acquire(context);
      },
    };
  }

  createProjector(options: { adapterIdForDomain?: Partial<Record<AutonomousDomainName, string>> } = {}): AutonomousEvidenceProjector {
    return {
      project: async (value, context) => {
        const domain = context?.requirement?.domain;
        if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("evidence adapter project context has an unsupported domain");
        const manifest = this.resolve(domain, options.adapterIdForDomain?.[domain]);
        const entry = this.entries.get(manifest.adapter_id);
        if (!entry?.project) throw new ArgumentError(`evidence adapter ${manifest.adapter_id} does not provide a projector`);
        return entry.project(value, context);
      },
    };
  }

  toJSON(): AutonomousEvidenceAdapterRegistryJSON {
    const adapters = this.manifests();
    const coverage = this.coverage();
    const coverageDigest = digestJsonSync(coverage);
    const descriptor = {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_SCHEMA,
      adapters,
      coverage,
      coverage_digest: coverageDigest,
      execution: "registry_projection_only;no_source_dispatch" as const,
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    const encoded = canonicalJson(descriptor);
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_REGISTRY_BYTES) throw new ArgumentError("evidence adapter registry projection exceeds its bound");
    return { ...descriptor, registry_digest: digestJsonSync(descriptor) };
  }

  private assertSize(): void {
    this.toJSON();
  }
}

/** Register one caller-owned adapter for each built-in domain. */
export function registerAutonomousEvidenceAdaptersForAllDomains(
  registry: AutonomousEvidenceAdapterRegistry,
  factory: (domain: AutonomousDomainName) => Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains?: readonly AutonomousDomainName[] },
  options: { replace?: boolean } = {},
): AutonomousEvidenceAdapterManifest[] {
  if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("all-domain evidence adapter registration requires a typed registry");
  if (typeof factory !== "function") throw new ArgumentError("all-domain evidence adapter registration requires a factory");
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => {
    const registration = factory(domain);
    if (!registration || typeof registration !== "object") throw new ArgumentError(`evidence adapter factory returned no registration for ${domain}`);
    return registry.register({ ...registration, domains: registration.domains ?? [domain] }, options);
  });
}
