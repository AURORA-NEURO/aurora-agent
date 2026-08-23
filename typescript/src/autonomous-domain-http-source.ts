import { ArgumentError } from "./errors.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterManifest,
} from "./autonomous-evidence-adapters.js";
import {
  createAutonomousHttpEvidenceAdapterRegistration,
  type AutonomousHttpEvidenceAdapterOptions,
} from "./autonomous-evidence-http-adapter.js";
import {
  AutonomousDomainEvidenceSourceCatalogue,
  type AutonomousDomainEvidenceRouteJSON,
} from "./autonomous-domain-evidence-catalogue.js";
import type { JsonObject } from "./types.js";

/** Bridge schema for a domain-scoped HTTP source bound into the evidence catalogue. */
export const AUTONOMOUS_DOMAIN_HTTP_SOURCE_SCHEMA = "bioprism-typescript-autonomous-domain-http-source/0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_METADATA_BYTES = 64_000;

export interface AutonomousDomainHttpEvidenceSourceOptions extends Omit<AutonomousHttpEvidenceAdapterOptions, "adapterId" | "version" | "domain" | "provider" | "capabilities" | "sourceKinds"> {
  catalogue: AutonomousDomainEvidenceSourceCatalogue;
  profileId: string;
  sourceId: string;
  provider: string;
  adapterId: string;
  adapterVersion: string;
  capabilities?: readonly string[];
  sourceKinds?: readonly string[];
  operations?: readonly string[];
  sourceDigest?: string | null;
  requestId?: string | null;
  contractDigest?: string | null;
  adapterRegistry?: AutonomousEvidenceAdapterRegistry;
  metadata?: JsonObject;
  replace?: boolean;
}

export interface AutonomousDomainHttpEvidenceSourceRegistration extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_HTTP_SOURCE_SCHEMA;
  profile_id: string;
  source_id: string;
  provider: string;
  adapter_id: string;
  adapter_version: string;
  route: AutonomousDomainEvidenceRouteJSON;
  adapter_manifest: AutonomousEvidenceAdapterManifest | null;
  adapter_registry_digest: string | null;
  execution: "registered_only;HTTP_dispatch_requires_catalogue_approval";
  transport: "bounded_http_connector;caller_endpoint_and_header_resolvers";
  retention: "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned";
  secret_material: "never_returned";
}

/**
 * Register one policy-gated HTTP adapter as a catalogue route. The endpoint, request shape,
 * response interpretation, and credential session remain caller-owned; this function only
 * composes their typed boundaries and never dispatches during registration.
 */
export function registerAutonomousDomainHttpEvidenceSource(
  options: AutonomousDomainHttpEvidenceSourceOptions,
): AutonomousDomainHttpEvidenceSourceRegistration {
  if (!options || typeof options !== "object") throw new ArgumentError("domain HTTP evidence source options are malformed");
  if (!(options.catalogue instanceof AutonomousDomainEvidenceSourceCatalogue)) throw new ArgumentError("domain HTTP evidence source requires a typed catalogue");
  if (options.adapterRegistry !== undefined && !(options.adapterRegistry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("domain HTTP evidence source adapterRegistry is malformed");
  const profile = options.catalogue.profile(options.profileId);
  const sourceKinds = options.sourceKinds ?? [profile.source_kinds[0]!];
  const capabilities = options.capabilities ?? profile.capabilities;
  const registration = createAutonomousHttpEvidenceAdapterRegistration({
    adapterId: options.adapterId,
    version: options.adapterVersion,
    domain: profile.domain,
    provider: options.provider,
    capabilities,
    sourceKinds,
    manifest: options.manifest,
    policy: options.policy,
    fetch: options.fetch,
    endpointResolver: options.endpointResolver,
    requestForContext: options.requestForContext,
    headerResolver: options.headerResolver,
    project: options.project,
  });
  const adapterManifest = options.adapterRegistry?.register(registration, { replace: options.replace === true }) ?? null;
  const route = options.catalogue.registerRoute({
    sourceId: options.sourceId,
    profileId: profile.profile_id,
    provider: options.provider,
    sourceKinds,
    capabilities,
    operations: options.operations ?? profile.operations,
    sourceDigest: options.sourceDigest,
    requestId: options.requestId,
    contractDigest: options.contractDigest,
    adapterId: options.adapterId,
    adapterManifestDigest: adapterManifest?.manifest_digest ?? null,
    metadata: options.metadata,
    acquirer: { acquire: registration.acquire },
  }, { replace: options.replace === true });
  const adapterRegistryDigest = options.adapterRegistry?.toJSON().registry_digest ?? null;
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_HTTP_SOURCE_SCHEMA,
    profile_id: profile.profile_id,
    source_id: route.source_id,
    provider: route.provider,
    adapter_id: options.adapterId,
    adapter_version: options.adapterVersion,
    route,
    adapter_manifest: adapterManifest,
    adapter_registry_digest: adapterRegistryDigest,
    execution: "registered_only;HTTP_dispatch_requires_catalogue_approval" as const,
    transport: "bounded_http_connector;caller_endpoint_and_header_resolvers" as const,
    retention: "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned" as const,
    secret_material: "never_returned" as const,
  };
  const encoded = JSON.stringify(descriptor);
  if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_METADATA_BYTES) throw new ArgumentError("domain HTTP evidence source registration exceeds its metadata bound");
  return descriptor;
}
