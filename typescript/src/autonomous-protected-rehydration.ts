/**
 * Caller-owned protected-value rehydration with tenant and replay fencing.
 *
 * Durable records contain only opaque references, bounded labels, and digests. The caller
 * supplies both the resolver and (optionally) the authorization authority; returned values
 * are transient and are deliberately absent from every projection and snapshot.
 */

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import { canonicalJson, digestBytesSync, digestJsonSync } from "./tooling.js";

export const AUTONOMOUS_PROTECTED_REHYDRATION_SCHEMA = "bioprism-typescript-autonomous-protected-rehydration/0.1" as const;
export const AUTONOMOUS_PROTECTED_REHYDRATION_CONTEXT_SCHEMA = "bioprism-typescript-autonomous-protected-rehydration-context/0.1" as const;
export const AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA = "bioprism-typescript-autonomous-protected-rehydration-reference/0.1" as const;
export const AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-protected-rehydration-snapshot/0.1" as const;
export const AUTONOMOUS_PROTECTED_REHYDRATION_ADAPTER_SCHEMA = "bioprism-typescript-autonomous-protected-rehydration-adapter/0.1" as const;
export const MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES = 4_096;
export const MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES = 1_000_000;
export const MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS = 31 * 86_400;
export const AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES = ["canonical_json", "utf8_sha256"] as const;

const DOMAINS = [...AUTONOMOUS_DOMAIN_NAMES] as AutonomousDomainName[];
const ID = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$/;
const DIGEST = /^[0-9a-f]{64}$/;
const STATUSES = ["available", "consumed", "expired", "quarantined"] as const;
const RETENTION = "metadata_only_opaque_references_and_digests_no_protected_values" as const;
const SECRET_MATERIAL = "never_returned" as const;
const AUTHORITY = "caller_owned_resolver_and_authorizer_required" as const;
type RehydrationStatus = typeof STATUSES[number];

export class AutonomousProtectedRehydrationError extends ArgumentError {}

function fail(message: string): never {
  throw new AutonomousProtectedRehydrationError(`protected rehydration ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || !ID.test(value) || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is not a bounded identifier`);
  return value;
}

function digest(name: string, value: unknown, optional = false): string | null {
  if (optional && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !DIGEST.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function numberBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function integerBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}

function booleanValue(name: string, value: unknown): boolean {
  if (typeof value !== "boolean") fail(`${name} must be boolean`);
  return value;
}

function domains(name: string, value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length === 0 || value.some((item) => typeof item !== "string") || new Set(value).size !== value.length || value.some((item) => !DOMAINS.includes(item as AutonomousDomainName))) fail(`${name} contains an unsupported or duplicate domain`);
  return DOMAINS.filter((item) => (value as string[]).includes(item));
}

export function protectedValueDigest(value: unknown): string {
  try {
    return digestJsonSync(value);
  } catch (error) {
    throw new AutonomousProtectedRehydrationError("protected value must be canonical JSON", { cause: error });
  }
}

type AutonomousProtectedRehydrationDigestScheme = typeof AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES[number];

function digestScheme(value: unknown): AutonomousProtectedRehydrationDigestScheme {
  if (typeof value !== "string" || !AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES.includes(value as AutonomousProtectedRehydrationDigestScheme)) fail("digest scheme is unsupported");
  return value as AutonomousProtectedRehydrationDigestScheme;
}

function digestForScheme(value: unknown, scheme: AutonomousProtectedRehydrationDigestScheme): string {
  if (scheme === "canonical_json") return protectedValueDigest(value);
  if (typeof value !== "string") fail("utf8_sha256 protected values must be strings");
  return digestBytesSync(new TextEncoder().encode(value));
}

export class AutonomousProtectedRehydrationContext {
  readonly tenantId: string;
  readonly actorId: string;
  readonly sessionId: string;
  readonly authorizationDigest: string;
  readonly allowedDomains: AutonomousDomainName[];

  constructor(options: { tenantId: string; actorId: string; sessionId: string; authorizationDigest: string; allowedDomains?: AutonomousDomainName[] }) {
    this.tenantId = identifier("context tenantId", options.tenantId);
    this.actorId = identifier("context actorId", options.actorId);
    this.sessionId = identifier("context sessionId", options.sessionId);
    this.authorizationDigest = digest("context authorizationDigest", options.authorizationDigest)!;
    this.allowedDomains = domains("context allowedDomains", options.allowedDomains ?? DOMAINS);
    if (canonicalJson(this.allowedDomains) !== canonicalJson(DOMAINS.filter((item) => this.allowedDomains.includes(item)))) fail("context domains are not in canonical built-in order");
  }

  immutableProjection(): Record<string, unknown> {
    return {
      schema: AUTONOMOUS_PROTECTED_REHYDRATION_CONTEXT_SCHEMA,
      tenant_id: this.tenantId,
      actor_id: this.actorId,
      session_id: this.sessionId,
      authorization_digest: this.authorizationDigest,
      allowed_domains: [...this.allowedDomains],
    };
  }

  get contextDigest(): string {
    return digestJsonSync(this.immutableProjection());
  }

  toJSON(): Record<string, unknown> {
    return { ...this.immutableProjection(), context_digest: this.contextDigest, retention: RETENTION, authority: AUTHORITY, secret_material: SECRET_MATERIAL };
  }
}

export interface AutonomousProtectedRehydrationReference {
  schema: typeof AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA;
  reference_id: string;
  domain: AutonomousDomainName;
  purpose: string;
  value_digest: string;
  value_kind: string;
  issued_at: number;
  expires_at: number;
  one_time: boolean;
  status: RehydrationStatus;
  attempts: number;
  context_digest: string;
  reference_digest: string;
  last_error_class: string | null;
  retention: typeof RETENTION;
  authority: typeof AUTHORITY;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousProtectedRehydrationResult {
  reference: AutonomousProtectedRehydrationReference;
  value: unknown;
  resolution_digest: string;
  toJSON(): Record<string, unknown>;
}

export class AutonomousProtectedRehydrationAdapter {
  readonly boundary: AutonomousProtectedRehydrationBoundary;

  constructor(boundary: AutonomousProtectedRehydrationBoundary) {
    if (!(boundary instanceof AutonomousProtectedRehydrationBoundary)) fail("receipt adapter requires an AutonomousProtectedRehydrationBoundary");
    this.boundary = boundary;
  }

  private metadata(receipt: unknown): Record<string, unknown> {
    if (!isObject(receipt)) fail("receipt must be a metadata object");
    const allowed = ["receipt_digest", "request_digest", "request_id", "dispatch_id", "work_id", "value_digest", "payload_digest", "domain", "source_id", "connector_id", "plan_digest", "workflow_digest", "stage_id", "attempt", "goal_id", "goal_digest", "task_digest", "schedule_digest", "claim_digest", "revision", "execution_binding_digest", "job_id", "index", "mode", "expected_result_digest", "spec_digest", "capability", "approval_released"];
    return Object.fromEntries(allowed.filter((key) => receipt[key] !== undefined && receipt[key] !== null).map((key) => [key, receipt[key]]));
  }

  private binding(receipt: unknown, purpose: string, digest_scheme: AutonomousProtectedRehydrationDigestScheme): { referenceId: string; valueDigest: string } {
    const metadata = this.metadata(receipt);
    const valueDigest = typeof metadata.value_digest === "string" ? metadata.value_digest : metadata.payload_digest;
    if (typeof valueDigest !== "string") fail("receipt has no protected value or payload digest");
    digest("receipt protected value digest", valueDigest);
    const normalizedPurpose = identifier("receipt purpose", purpose);
    const bindingDigest = digestJsonSync({ schema: AUTONOMOUS_PROTECTED_REHYDRATION_ADAPTER_SCHEMA, purpose: normalizedPurpose, digest_scheme, receipt: metadata });
    return { referenceId: `rehydrate-${bindingDigest.slice(0, 48)}`, valueDigest };
  }

  resolveReceipt(receipt: unknown, options: { domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; now?: number; digestScheme?: string } = {}): unknown {
    const metadata = this.metadata(receipt);
    const domain = options.domain ?? (typeof metadata.domain === "string" ? metadata.domain as AutonomousDomainName : undefined);
    if (!domain || !this.boundary.context.allowedDomains.includes(domain)) fail("receipt domain is outside the active context scope");
    const purpose = options.purpose ?? "protected_receipt_value";
    const digest_scheme = digestScheme(options.digestScheme ?? "canonical_json");
    const binding = this.binding(metadata, purpose, digest_scheme);
    this.boundary.issue(binding.referenceId, { domain, purpose, valueDigest: binding.valueDigest, valueKind: options.valueKind ?? "opaque", oneTime: options.oneTime ?? false });
    return this.boundary.resolve(binding.referenceId, { now: options.now, valueDigestor: (value) => digestForScheme(value, digest_scheme) }).value;
  }

  resolver(options: { domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; digestScheme?: string } = {}): (receipt: unknown) => unknown {
    return (receipt) => this.resolveReceipt(receipt, options);
  }
}

export interface AutonomousProtectedRehydrationSnapshot {
  schema: typeof AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA;
  generation: number;
  previous_snapshot_digest: string | null;
  context_digest: string;
  policy: { max_references: number; max_attempts: number; max_ttl_seconds: number };
  references: AutonomousProtectedRehydrationReference[];
  coverage: AutonomousProtectedRehydrationCoverage[];
  retention: typeof RETENTION;
  authority: typeof AUTHORITY;
  secret_material: typeof SECRET_MATERIAL;
  snapshot_digest: string;
}

export interface AutonomousProtectedRehydrationCoverage {
  domain: AutonomousDomainName;
  reference_count: number;
  available_count: number;
  consumed_count: number;
  expired_count: number;
  quarantined_count: number;
}

export interface AutonomousProtectedRehydrationTextStore {
  read(): string | null;
  write(value: string): void;
}

export interface AutonomousProtectedRehydrationTransactionalTextStore extends AutonomousProtectedRehydrationTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean;
}

export type AutonomousProtectedRehydrationResolver = (reference: AutonomousProtectedRehydrationReference, context: AutonomousProtectedRehydrationContext) => unknown;
export type AutonomousProtectedRehydrationAuthorizer = (reference: AutonomousProtectedRehydrationReference, context: AutonomousProtectedRehydrationContext) => boolean;

function immutableProjection(reference: Pick<AutonomousProtectedRehydrationReference, "reference_id" | "domain" | "purpose" | "value_digest" | "value_kind" | "issued_at" | "expires_at" | "one_time" | "context_digest">): Record<string, unknown> {
  return {
    schema: AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA,
    reference_id: reference.reference_id,
    domain: reference.domain,
    purpose: reference.purpose,
    value_digest: reference.value_digest,
    value_kind: reference.value_kind,
    issued_at: reference.issued_at,
    expires_at: reference.expires_at,
    one_time: reference.one_time,
    context_digest: reference.context_digest,
  };
}

function coverageFor(references: readonly AutonomousProtectedRehydrationReference[]): AutonomousProtectedRehydrationCoverage[] {
  return DOMAINS.map((domain) => {
    const selected = references.filter((reference) => reference.domain === domain);
    return {
      domain,
      reference_count: selected.length,
      available_count: selected.filter((reference) => reference.status === "available").length,
      consumed_count: selected.filter((reference) => reference.status === "consumed").length,
      expired_count: selected.filter((reference) => reference.status === "expired").length,
      quarantined_count: selected.filter((reference) => reference.status === "quarantined").length,
    };
  });
}

function validateReference(value: unknown): AutonomousProtectedRehydrationReference {
  const expected = ["attempts", "authority", "context_digest", "domain", "expires_at", "issued_at", "last_error_class", "one_time", "purpose", "reference_digest", "reference_id", "retention", "schema", "secret_material", "status", "value_digest", "value_kind"];
  if (!isObject(value) || Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA || value.retention !== RETENTION || value.authority !== AUTHORITY || value.secret_material !== SECRET_MATERIAL) fail("snapshot reference is malformed");
  const reference: AutonomousProtectedRehydrationReference = {
    schema: AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA,
    reference_id: identifier("snapshot reference_id", value.reference_id),
    domain: domains("snapshot reference domain", [value.domain])[0]!,
    purpose: identifier("snapshot purpose", value.purpose),
    value_digest: digest("snapshot value_digest", value.value_digest)!,
    value_kind: identifier("snapshot value_kind", value.value_kind),
    issued_at: numberBound("snapshot issued_at", value.issued_at, 0, 9_223_372_036_854_775),
    expires_at: numberBound("snapshot expires_at", value.expires_at, 0, 9_223_372_036_854_775),
    one_time: booleanValue("snapshot one_time", value.one_time),
    status: STATUSES.includes(value.status as RehydrationStatus) ? value.status as RehydrationStatus : fail("snapshot reference status is unsupported"),
    attempts: integerBound("snapshot attempts", value.attempts, 0, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS),
    context_digest: digest("snapshot context_digest", value.context_digest)!,
    reference_digest: digest("snapshot reference_digest", value.reference_digest)!,
    last_error_class: value.last_error_class === null ? null : identifier("snapshot last_error_class", value.last_error_class),
    retention: RETENTION,
    authority: AUTHORITY,
    secret_material: SECRET_MATERIAL,
  };
  if (reference.expires_at < reference.issued_at || reference.expires_at - reference.issued_at > MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS) fail("snapshot reference expiry is outside its bounded lifetime");
  if (reference.status === "consumed" && !reference.one_time) fail("non-one-time reference cannot be consumed");
  if (digestJsonSync(immutableProjection(reference)) !== reference.reference_digest) fail("snapshot reference digest does not match its immutable projection");
  return reference;
}

export function validateAutonomousProtectedRehydrationSnapshot(value: unknown): AutonomousProtectedRehydrationSnapshot {
  const expected = ["authority", "context_digest", "coverage", "generation", "policy", "previous_snapshot_digest", "references", "retention", "schema", "secret_material", "snapshot_digest"];
  if (!isObject(value) || Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA || value.retention !== RETENTION || value.authority !== AUTHORITY || value.secret_material !== SECRET_MATERIAL) fail("snapshot is malformed");
  const generation = integerBound("snapshot generation", value.generation, 1, 2_147_483_647);
  const previous = digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest ?? null, true);
  const contextDigest = digest("snapshot context_digest", value.context_digest)!;
  if (!isObject(value.policy) || Object.keys(value.policy).sort().join(",") !== "max_attempts,max_references,max_ttl_seconds") fail("snapshot policy is malformed");
  const policy = {
    max_references: integerBound("snapshot policy max_references", value.policy.max_references, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES),
    max_attempts: integerBound("snapshot policy max_attempts", value.policy.max_attempts, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS),
    max_ttl_seconds: numberBound("snapshot policy max_ttl_seconds", value.policy.max_ttl_seconds, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS),
  };
  if (!Array.isArray(value.references) || value.references.length > policy.max_references) fail("snapshot references are malformed");
  const references = value.references.map(validateReference);
  if (new Set(references.map((reference) => reference.reference_id)).size !== references.length || references.some((reference) => reference.context_digest !== contextDigest)) fail("snapshot references are not bound to its context");
  if (!Array.isArray(value.coverage) || canonicalJson(value.coverage) !== canonicalJson(coverageFor(references))) fail("snapshot coverage does not match references");
  const snapshotDigest = digest("snapshot snapshot_digest", value.snapshot_digest)!;
  const descriptor = { schema: value.schema, generation, previous_snapshot_digest: previous, context_digest: contextDigest, policy, references: [...references].sort((left, right) => left.reference_id.localeCompare(right.reference_id)), coverage: value.coverage, retention: RETENTION, authority: AUTHORITY, secret_material: SECRET_MATERIAL };
  if (digestJsonSync(descriptor) !== snapshotDigest) fail("snapshot digest does not match its canonical projection");
  return JSON.parse(canonicalJson({ ...descriptor, snapshot_digest: snapshotDigest })) as AutonomousProtectedRehydrationSnapshot;
}

export class AutonomousProtectedRehydrationBoundary {
  readonly context: AutonomousProtectedRehydrationContext;
  readonly resolver: AutonomousProtectedRehydrationResolver;
  readonly authorizer?: AutonomousProtectedRehydrationAuthorizer;
  readonly maxReferences: number;
  readonly maxAttempts: number;
  readonly maxTtlSeconds: number;
  readonly clock: () => number;
  private readonly references = new Map<string, AutonomousProtectedRehydrationReference>();
  private generation = 0;
  private previousSnapshotDigest: string | null = null;

  constructor(context: AutonomousProtectedRehydrationContext, resolver: AutonomousProtectedRehydrationResolver, options: { authorizer?: AutonomousProtectedRehydrationAuthorizer; maxReferences?: number; maxAttempts?: number; maxTtlSeconds?: number; clock?: () => number } = {}) {
    if (!(context instanceof AutonomousProtectedRehydrationContext)) fail("context is malformed");
    if (typeof resolver !== "function") fail("resolver is required");
    if (options.authorizer !== undefined && typeof options.authorizer !== "function") fail("authorizer is malformed");
    if (options.clock !== undefined && typeof options.clock !== "function") fail("clock is malformed");
    this.context = context;
    this.resolver = resolver;
    this.authorizer = options.authorizer;
    this.maxReferences = integerBound("maxReferences", options.maxReferences ?? MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES);
    this.maxAttempts = integerBound("maxAttempts", options.maxAttempts ?? 3, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS);
    this.maxTtlSeconds = numberBound("maxTtlSeconds", options.maxTtlSeconds ?? 3_600, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS);
    this.clock = options.clock ?? (() => Date.now() / 1000);
  }

  get policy(): { max_references: number; max_attempts: number; max_ttl_seconds: number } {
    return { max_references: this.maxReferences, max_attempts: this.maxAttempts, max_ttl_seconds: this.maxTtlSeconds };
  }

  private replace(reference: AutonomousProtectedRehydrationReference, changes: Partial<AutonomousProtectedRehydrationReference>): AutonomousProtectedRehydrationReference {
    return { ...reference, ...changes };
  }

  issue(referenceId: string, options: { domain: AutonomousDomainName; purpose: string; valueDigest: string; valueKind?: string; issuedAt?: number; expiresAt?: number; oneTime?: boolean }): AutonomousProtectedRehydrationReference {
    const normalizedId = identifier("referenceId", referenceId);
    if (!this.context.allowedDomains.includes(options.domain)) fail("reference domain is outside the context scope");
    const issuedAt = numberBound("issuedAt", options.issuedAt ?? this.clock(), 0, 9_223_372_036_854_775);
    const expiresAt = numberBound("expiresAt", options.expiresAt ?? issuedAt + this.maxTtlSeconds, 0, 9_223_372_036_854_775);
    const valueDigest = digest("valueDigest", options.valueDigest)!;
    if (expiresAt < issuedAt || expiresAt - issuedAt > this.maxTtlSeconds) fail("expiry exceeds the configured lifetime");
    const base = { schema: AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA, reference_id: normalizedId, domain: options.domain, purpose: identifier("purpose", options.purpose), value_digest: valueDigest, value_kind: identifier("valueKind", options.valueKind ?? "opaque"), issued_at: issuedAt, expires_at: expiresAt, one_time: options.oneTime ?? true, context_digest: this.context.contextDigest };
    const reference: AutonomousProtectedRehydrationReference = { ...base, status: "available", attempts: 0, reference_digest: digestJsonSync(base), last_error_class: null, retention: RETENTION, authority: AUTHORITY, secret_material: SECRET_MATERIAL };
    const existing = this.references.get(normalizedId);
    if (existing) {
      if (existing.reference_digest !== reference.reference_digest) fail("reference identifier already exists with a different immutable payload");
      return { ...existing };
    }
    if (this.references.size >= this.maxReferences) fail("reference registry is full");
    this.references.set(normalizedId, reference);
    return { ...reference };
  }

  issueForValue(referenceId: string, value: unknown, options: Omit<Parameters<AutonomousProtectedRehydrationBoundary["issue"]>[1], "valueDigest">): AutonomousProtectedRehydrationReference {
    return this.issue(referenceId, { ...options, valueDigest: protectedValueDigest(value) });
  }

  get(referenceId: string): AutonomousProtectedRehydrationReference | null {
    const reference = this.references.get(identifier("referenceId", referenceId));
    return reference ? { ...reference } : null;
  }

  listReferences(limit = 128): AutonomousProtectedRehydrationReference[] {
    const boundedLimit = integerBound("list limit", limit, 1, this.maxReferences);
    return [...this.references.values()].sort((left, right) => Number(left.status !== "available") - Number(right.status !== "available") || left.expires_at - right.expires_at || left.reference_id.localeCompare(right.reference_id)).slice(0, boundedLimit).map((reference) => ({ ...reference }));
  }

  private failure(reference: AutonomousProtectedRehydrationReference, errorClass: string): void {
    const attempts = Math.min(this.maxAttempts, reference.attempts + 1);
    this.references.set(reference.reference_id, this.replace(reference, { attempts, status: attempts >= this.maxAttempts ? "quarantined" : reference.status, last_error_class: identifier("errorClass", errorClass) }));
  }

  resolve(referenceId: string, options: { now?: number; valueDigestor?: (value: unknown) => string } = {}): AutonomousProtectedRehydrationResult {
    const now = numberBound("resolve now", options.now ?? this.clock(), 0, 9_223_372_036_854_775);
    const normalizedId = identifier("referenceId", referenceId);
    const reference = this.references.get(normalizedId);
    if (!reference) fail("reference does not exist");
    if (reference.context_digest !== this.context.contextDigest) fail("reference context does not match the active tenant and authorization");
    if (reference.status === "consumed") fail("one-time reference has already been consumed");
    if (reference.status === "quarantined") fail("reference is quarantined");
    if (now >= reference.expires_at) {
      this.references.set(normalizedId, this.replace(reference, { status: "expired", last_error_class: "reference_expired" }));
      fail("reference has expired");
    }
    if (this.authorizer) {
      try {
        if (this.authorizer(reference, this.context) !== true) {
          this.failure(reference, "authorization_denied");
          fail("caller authorization was denied");
        }
      } catch (error) {
        if (error instanceof AutonomousProtectedRehydrationError) throw error;
        this.failure(reference, "authorization_check_failure");
        throw new AutonomousProtectedRehydrationError("protected rehydration authorization check failed", { cause: error });
      }
    }
    let value: unknown;
    try {
      value = this.resolver(reference, this.context);
      if ((options.valueDigestor ?? protectedValueDigest)(value) !== reference.value_digest) {
        this.failure(reference, "value_digest_mismatch");
        fail("resolver returned a value with a different digest");
      }
    } catch (error) {
      if (error instanceof AutonomousProtectedRehydrationError && error.message.includes("different digest")) throw error;
      this.failure(reference, "resolver_failure");
      throw new AutonomousProtectedRehydrationError("protected value resolver failed", { cause: error });
    }
    const updated = this.replace(reference, { status: reference.one_time ? "consumed" : "available", attempts: reference.attempts + 1, last_error_class: null });
    this.references.set(normalizedId, updated);
    const resolutionDigest = digestJsonSync({ schema: AUTONOMOUS_PROTECTED_REHYDRATION_SCHEMA, reference_digest: reference.reference_digest, context_digest: this.context.contextDigest, attempt: updated.attempts });
    return {
      reference: { ...updated }, value, resolution_digest: resolutionDigest,
      toJSON: () => ({ reference: { ...updated }, resolution_digest: resolutionDigest, value_present: true, value_retained: false, retention: "transient_caller_value_only", authority: AUTHORITY, secret_material: SECRET_MATERIAL }),
    };
  }

  quarantine(referenceId: string, errorClass = "caller_quarantined"): AutonomousProtectedRehydrationReference {
    const normalizedId = identifier("referenceId", referenceId);
    const reference = this.references.get(normalizedId);
    if (!reference) fail("reference does not exist");
    const updated = this.replace(reference, { status: "quarantined", last_error_class: identifier("errorClass", errorClass) });
    this.references.set(normalizedId, updated);
    return { ...updated };
  }

  snapshot(): AutonomousProtectedRehydrationSnapshot {
    this.generation += 1;
    const descriptor = { schema: AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA, generation: this.generation, previous_snapshot_digest: this.previousSnapshotDigest, context_digest: this.context.contextDigest, policy: this.policy, references: [...this.references.values()].sort((left, right) => left.reference_id.localeCompare(right.reference_id)), coverage: coverageFor([...this.references.values()]), retention: RETENTION, authority: AUTHORITY, secret_material: SECRET_MATERIAL };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousProtectedRehydrationSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    return JSON.parse(canonicalJson(snapshot)) as AutonomousProtectedRehydrationSnapshot;
  }

  restore(snapshot: AutonomousProtectedRehydrationSnapshot): AutonomousProtectedRehydrationSnapshot {
    const validated = validateAutonomousProtectedRehydrationSnapshot(snapshot);
    if (validated.context_digest !== this.context.contextDigest) fail("restored snapshot belongs to a different tenant, actor, session, or authorization");
    if (canonicalJson(validated.policy) !== canonicalJson(this.policy)) fail("restored policy conflicts with the configured boundary");
    this.references.clear();
    for (const reference of validated.references) this.references.set(reference.reference_id, reference);
    this.generation = validated.generation;
    this.previousSnapshotDigest = validated.snapshot_digest;
    return JSON.parse(canonicalJson(validated)) as AutonomousProtectedRehydrationSnapshot;
  }
}

export class JsonAutonomousProtectedRehydrationPersistence {
  readonly textStore: AutonomousProtectedRehydrationTextStore;
  readonly maxBytes: number;
  constructor(textStore: AutonomousProtectedRehydrationTextStore, maxBytes = MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") fail("JSON text store is malformed");
    this.textStore = textStore;
    this.maxBytes = integerBound("JSON maxBytes", maxBytes, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES);
  }
  read(): AutonomousProtectedRehydrationSnapshot | null {
    const encoded = this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch (error) { throw new AutonomousProtectedRehydrationError("protected rehydration JSON is invalid", { cause: error }); }
    if (canonicalJson(parsed) !== encoded) fail("JSON snapshot is not canonical");
    return validateAutonomousProtectedRehydrationSnapshot(parsed);
  }
  write(snapshot: AutonomousProtectedRehydrationSnapshot): void {
    const encoded = canonicalJson(validateAutonomousProtectedRehydrationSnapshot(snapshot));
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    this.textStore.write(encoded);
  }
}

export class TransactionalJsonAutonomousProtectedRehydrationPersistence extends JsonAutonomousProtectedRehydrationPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousProtectedRehydrationSnapshot): boolean {
    digest("expectedSnapshotDigest", expectedSnapshotDigest, true);
    if (typeof (this.textStore as Partial<AutonomousProtectedRehydrationTransactionalTextStore>).writeIfUnchanged !== "function") fail("transactional JSON text store lacks compare-and-swap");
    const encoded = canonicalJson(validateAutonomousProtectedRehydrationSnapshot(snapshot));
    return Boolean((this.textStore as AutonomousProtectedRehydrationTransactionalTextStore).writeIfUnchanged(expectedSnapshotDigest, encoded));
  }
}

export class AutonomousProtectedRehydrationPersistenceCoordinator {
  readonly boundary: AutonomousProtectedRehydrationBoundary;
  readonly persistence: JsonAutonomousProtectedRehydrationPersistence;
  private expectedSnapshotDigest: string | null = null;
  constructor(boundary: AutonomousProtectedRehydrationBoundary, persistence: JsonAutonomousProtectedRehydrationPersistence) {
    if (!(boundary instanceof AutonomousProtectedRehydrationBoundary) || !persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") fail("persistence coordinator inputs are malformed");
    this.boundary = boundary;
    this.persistence = persistence;
  }
  restore(): AutonomousProtectedRehydrationSnapshot | null {
    const snapshot = this.persistence.read();
    if (snapshot === null) return null;
    this.boundary.restore(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
  flush(): AutonomousProtectedRehydrationSnapshot {
    const snapshot = this.boundary.snapshot();
    if (this.persistence instanceof TransactionalJsonAutonomousProtectedRehydrationPersistence) {
      if (!this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) fail("persistence compare-and-swap conflict");
    } else this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
}
