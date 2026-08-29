/**
 * Tenant-scoped, metadata-only authorization for autonomous execution.
 *
 * The SDK does not mint identities, verify bearer tokens, or turn a digest into proof of
 * authority. It does provide one reusable contract for a caller-issued grant to scope planning,
 * provider invocation, evidence, connectors, tools, learning, memory, trace, analytics, and
 * effects across all built-in domains. Raw task text, prompts, credentials, provider payloads,
 * tool arguments, and results are intentionally rejected from this boundary.
 */

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_AUTHORIZATION_SCHEMA = "bioprism-typescript-autonomous-authorization/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA = "bioprism-typescript-autonomous-authorization-grant/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA = "bioprism-typescript-autonomous-authorization-request/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA = "bioprism-typescript-autonomous-authorization-decision/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA = "bioprism-typescript-autonomous-authorization-event/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-authorization-snapshot/0.1" as const;
export const AUTONOMOUS_AUTHORIZATION_RETENTION = "metadata_only;tenant_actor_session_scope_and_digests;no_tasks_prompts_credentials_or_payloads" as const;
export const AUTONOMOUS_AUTHORIZATION_AUTHORITY = "caller_issued_grant_contract;identity_and_token_verification_remain_deployment_owned" as const;
export const AUTONOMOUS_AUTHORIZATION_EXECUTION = "scope_check_only;does_not_mint_identity_or_authorize_unlisted_external_effects" as const;
export const AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL = "never_returned" as const;
export const AUTONOMOUS_AUTHORIZATION_OPERATIONS = [
  "plan", "provider_invocation", "evidence_acquisition", "connector_dispatch", "tool_execution", "effect_dispatch",
  "evaluation", "learning", "memory_retrieval", "memory_write", "trace_write", "analytics_write",
] as const;
export const AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES = ["active", "revoked", "expired", "exhausted"] as const;
export const AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES = [
  "allowed", "already_allowed", "not_found", "revoked", "expired", "exhausted", "tenant_mismatch", "actor_mismatch",
  "session_mismatch", "authorization_mismatch", "domain_denied", "operation_denied", "capability_denied", "risk_denied",
] as const;
export const AUTONOMOUS_AUTHORIZATION_EVENT_TYPES = ["grant_issued", "grant_revoked", "request_allowed", "request_replayed"] as const;
export const MAX_AUTONOMOUS_AUTHORIZATION_GRANTS = 4_096;
export const MAX_AUTONOMOUS_AUTHORIZATION_EVENTS = 32_768;
export const MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT = 4_096;
export const MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS = 31 * 86_400_000;
export const MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES = 8_000_000;
export const MAX_AUTONOMOUS_AUTHORIZATION_IDENTIFIER_BYTES = 256;
export const MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS = 128;

export type AutonomousAuthorizationOperation = typeof AUTONOMOUS_AUTHORIZATION_OPERATIONS[number];
export type AutonomousAuthorizationGrantStatus = typeof AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES[number];
export type AutonomousAuthorizationDecisionStatus = typeof AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES[number];
export type AutonomousAuthorizationEventType = typeof AUTONOMOUS_AUTHORIZATION_EVENT_TYPES[number];

const DOMAINS = [...AUTONOMOUS_DOMAIN_NAMES] as AutonomousDomainName[];
const OPERATIONS = new Set<string>(AUTONOMOUS_AUTHORIZATION_OPERATIONS);
const ID = /^[A-Za-z0-9][A-Za-z0-9_.:+/-]{0,255}$/;
const DIGEST = /^[0-9a-f]{64}$/;
const FORBIDDEN_KEYS = new Set(["task", "prompt", "response", "credential", "credentials", "token", "secret", "password", "body", "headers", "messages", "arguments", "payload", "result"]);

export class AutonomousAuthorizationError extends ArgumentError {}

function fail(message: string): never { throw new AutonomousAuthorizationError(`autonomous authorization ${message}`); }

function bytes(value: string): number { return new TextEncoder().encode(value).byteLength; }

function text(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || value.length === 0 || value.includes("\u0000") || bytes(value) > maximum) fail(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const result = text(name, value, MAX_AUTONOMOUS_AUTHORIZATION_IDENTIFIER_BYTES);
  if (!ID.test(result)) fail(`${name} is not a safe identifier`);
  return result;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  const result = text(name, value, 64);
  if (!DIGEST.test(result)) fail(`${name} must be a lowercase SHA-256 digest`);
  return result;
}

function integer(name: string, value: unknown, minimum = 0, maximum = 2_147_483_647): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bound`);
  return value;
}

function timestamp(name: string, value: unknown): number { return integer(name, value, 0, 253_402_300_799_999); }

function scope(name: string, value: unknown, allowed?: Set<string>, required = false): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS || (required && value.length === 0) || value.some((item) => typeof item !== "string")) fail(`${name} is empty or exceeds its bound`);
  const result = value.map((item) => identifier(`${name} entry`, item));
  if (new Set(result).size !== result.length || (allowed !== undefined && result.some((item) => !allowed.has(item)))) fail(`${name} contains an unsupported or duplicate value`);
  return [...result].sort();
}

function normalizeAuthorizationDomains(name: string, value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS || value.some((item) => typeof item !== "string")) fail(`${name} is empty or exceeds its bound`);
  const normalized = value.map((item) => identifier(`${name} entry`, item));
  if (new Set(normalized).size !== normalized.length || normalized.some((item) => !DOMAINS.includes(item as AutonomousDomainName))) fail(`${name} contains an unsupported or duplicate domain`);
  const canonical = DOMAINS.filter((item) => normalized.includes(item));
  if (canonical.join("\u0000") !== normalized.join("\u0000")) fail(`${name} must use canonical built-in domain order`);
  return canonical;
}

function clone<T>(value: T): T { return structuredClone(value); }

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 12) fail("metadata nesting exceeds its bound");
  if (value === null || typeof value === "string" || typeof value === "boolean" || typeof value === "number") {
    if (typeof value === "number" && !Number.isFinite(value)) fail("metadata contains a non-finite number");
    return;
  }
  if (value instanceof Uint8Array) fail("metadata cannot contain binary material");
  if (Array.isArray(value)) {
    if (value.length > 512) fail("metadata sequence exceeds its bound");
    value.forEach((item) => safeMetadata(item, depth + 1));
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 512) fail("metadata mapping exceeds its bound");
    for (const [key, child] of Object.entries(value)) {
      if (FORBIDDEN_KEYS.has(key.toLowerCase().replace(/[_-]/g, ""))) fail("metadata contains transient or secret-shaped material");
      safeMetadata(child, depth + 1);
    }
    return;
  }
  fail("metadata contains an unsupported value");
}

export function autonomousAuthorizationContextDigest(options: { tenantId: string; actorId: string; sessionId: string; authorizationDigest: string }): string {
  return digestJsonSync({
    schema: AUTONOMOUS_AUTHORIZATION_SCHEMA,
    tenant_id: identifier("tenantId", options.tenantId),
    actor_id: identifier("actorId", options.actorId),
    session_id: identifier("sessionId", options.sessionId),
    authorization_digest: digest("authorizationDigest", options.authorizationDigest),
  } as JsonObject);
}

function grantCore(grant: AutonomousAuthorizationGrant): JsonObject {
  return {
    schema: AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA, grant_id: grant.grant_id, tenant_id: grant.tenant_id, actor_id: grant.actor_id,
    session_id: grant.session_id, authorization_digest: grant.authorization_digest, allowed_domains: [...grant.allowed_domains],
    allowed_operations: [...grant.allowed_operations], allowed_capabilities: [...grant.allowed_capabilities],
    allowed_risk_classes: [...grant.allowed_risk_classes], issued_at: grant.issued_at, expires_at: grant.expires_at, max_uses: grant.max_uses,
  };
}

export interface AutonomousAuthorizationGrantJSON extends JsonObject {
  schema: typeof AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA; grant_id: string; tenant_id: string; actor_id: string; session_id: string;
  authorization_digest: string; allowed_domains: AutonomousDomainName[]; allowed_operations: AutonomousAuthorizationOperation[];
  allowed_capabilities: string[]; allowed_risk_classes: string[]; issued_at: number; expires_at: number; max_uses: number | null;
  used_count: number; used_request_digests: string[]; status: AutonomousAuthorizationGrantStatus; revoked_at: number | null;
  revocation_reason_digest: string | null; grant_digest: string; retention: typeof AUTONOMOUS_AUTHORIZATION_RETENTION;
  authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY; execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION; secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL;
}

export class AutonomousAuthorizationGrant {
  readonly grant_id: string; readonly tenant_id: string; readonly actor_id: string; readonly session_id: string; readonly authorization_digest: string;
  readonly allowed_domains: AutonomousDomainName[]; readonly allowed_operations: AutonomousAuthorizationOperation[]; readonly allowed_capabilities: string[];
  readonly allowed_risk_classes: string[]; readonly issued_at: number; readonly expires_at: number; readonly max_uses: number | null;
  readonly used_count: number; readonly used_request_digests: string[]; readonly status: AutonomousAuthorizationGrantStatus; readonly revoked_at: number | null;
  readonly revocation_reason_digest: string | null; readonly grant_digest: string;

  constructor(input: { grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; allowed_domains: AutonomousDomainName[]; allowed_operations: AutonomousAuthorizationOperation[]; allowed_capabilities: string[]; allowed_risk_classes: string[]; issued_at: number; expires_at: number; max_uses: number | null; used_count: number; used_request_digests: string[]; status: AutonomousAuthorizationGrantStatus; revoked_at: number | null; revocation_reason_digest: string | null; grant_digest: string }) {
    this.grant_id = identifier("grant_id", input.grant_id); this.tenant_id = identifier("grant tenant_id", input.tenant_id); this.actor_id = identifier("grant actor_id", input.actor_id); this.session_id = identifier("grant session_id", input.session_id); this.authorization_digest = digest("grant authorization_digest", input.authorization_digest)!;
    this.allowed_domains = normalizeAuthorizationDomains("grant allowed_domains", input.allowed_domains); this.allowed_operations = scope("grant allowed_operations", input.allowed_operations, new Set(OPERATIONS), true) as AutonomousAuthorizationOperation[]; this.allowed_capabilities = scope("grant allowed_capabilities", input.allowed_capabilities); this.allowed_risk_classes = scope("grant allowed_risk_classes", input.allowed_risk_classes);
    this.issued_at = timestamp("grant issued_at", input.issued_at); this.expires_at = timestamp("grant expires_at", input.expires_at); if (this.expires_at < this.issued_at || this.expires_at - this.issued_at > MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS) fail("grant lifetime exceeds its bound");
    if (input.max_uses !== null) integer("grant max_uses", input.max_uses, 1, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT); this.max_uses = input.max_uses;
    this.used_count = integer("grant used_count", input.used_count, 0, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT); this.used_request_digests = input.used_request_digests.map((item) => digest("grant used request digest", item)!); if (this.used_count !== this.used_request_digests.length || new Set(this.used_request_digests).size !== this.used_request_digests.length || (this.max_uses !== null && this.used_count > this.max_uses)) fail("grant use accounting is inconsistent");
    if (!AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES.includes(input.status)) fail("grant status is invalid"); this.status = input.status; this.revoked_at = input.revoked_at === null ? null : timestamp("grant revoked_at", input.revoked_at); this.revocation_reason_digest = digest("grant revocation_reason_digest", input.revocation_reason_digest, true); if (this.status === "revoked" && this.revoked_at === null) fail("revoked grant requires revoked_at"); if (this.status !== "revoked" && this.revoked_at !== null) fail("non-revoked grant cannot retain revoked_at");
    this.grant_digest = digest("grant grant_digest", input.grant_digest)!; if (this.grant_digest !== digestJsonSync(grantCore(this))) fail("grant_digest does not match grant scope");
  }

  static issue(input: { grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; allowed_domains: AutonomousDomainName[]; allowed_operations: AutonomousAuthorizationOperation[]; allowed_capabilities?: string[]; allowed_risk_classes?: string[]; issued_at: number; expires_at: number; max_uses?: number | null }): AutonomousAuthorizationGrant {
    const normalized = {
      schema: AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA, grant_id: identifier("grant_id", input.grant_id), tenant_id: identifier("grant tenant_id", input.tenant_id), actor_id: identifier("grant actor_id", input.actor_id), session_id: identifier("grant session_id", input.session_id), authorization_digest: digest("grant authorization_digest", input.authorization_digest)!, allowed_domains: normalizeAuthorizationDomains("grant allowed_domains", input.allowed_domains), allowed_operations: scope("grant allowed_operations", input.allowed_operations, new Set(OPERATIONS), true) as AutonomousAuthorizationOperation[], allowed_capabilities: scope("grant allowed_capabilities", input.allowed_capabilities ?? []), allowed_risk_classes: scope("grant allowed_risk_classes", input.allowed_risk_classes ?? []), issued_at: timestamp("grant issued_at", input.issued_at), expires_at: timestamp("grant expires_at", input.expires_at), max_uses: input.max_uses === undefined ? 1 : input.max_uses,
    };
    if (normalized.expires_at < normalized.issued_at || normalized.expires_at - normalized.issued_at > MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS) fail("grant lifetime exceeds its bound");
    return new AutonomousAuthorizationGrant({ ...normalized, used_count: 0, used_request_digests: [], status: "active", revoked_at: null, revocation_reason_digest: null, grant_digest: digestJsonSync(normalized) });
  }

  static fromJSON(value: unknown): AutonomousAuthorizationGrant {
    if (!isObject(value)) fail("grant must be an object"); const expected = ["actor_id", "allowed_capabilities", "allowed_domains", "allowed_operations", "allowed_risk_classes", "authority", "authorization_digest", "execution", "expires_at", "grant_digest", "grant_id", "issued_at", "max_uses", "retention", "revocation_reason_digest", "revoked_at", "schema", "secret_material", "session_id", "status", "tenant_id", "used_count", "used_request_digests"];
    if (Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA || value.retention !== AUTONOMOUS_AUTHORIZATION_RETENTION || value.authority !== AUTONOMOUS_AUTHORIZATION_AUTHORITY || value.execution !== AUTONOMOUS_AUTHORIZATION_EXECUTION || value.secret_material !== AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL) fail("grant contains unsupported fields or invalid markers");
    return new AutonomousAuthorizationGrant(value as unknown as ConstructorParameters<typeof AutonomousAuthorizationGrant>[0]);
  }

  toJSON(): AutonomousAuthorizationGrantJSON { return { ...grantCore(this), used_count: this.used_count, used_request_digests: [...this.used_request_digests], status: this.status, revoked_at: this.revoked_at, revocation_reason_digest: this.revocation_reason_digest, grant_digest: this.grant_digest, retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL } as AutonomousAuthorizationGrantJSON; }
}

function requestCore(request: AutonomousAuthorizationRequest): JsonObject { return { schema: AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA, request_id: request.request_id, grant_id: request.grant_id, tenant_id: request.tenant_id, actor_id: request.actor_id, session_id: request.session_id, authorization_digest: request.authorization_digest, domains: [...request.domains], operation: request.operation, capability: request.capability, risk_class: request.risk_class, resource_digest: request.resource_digest, issued_at: request.issued_at }; }

export interface AutonomousAuthorizationRequestJSON extends JsonObject { schema: typeof AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA; request_id: string; grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; domains: AutonomousDomainName[]; operation: AutonomousAuthorizationOperation; capability: string | null; risk_class: string | null; resource_digest: string | null; issued_at: number; request_digest: string; retention: typeof AUTONOMOUS_AUTHORIZATION_RETENTION; authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY; execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION; secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL; }

export class AutonomousAuthorizationRequest {
  readonly request_id: string; readonly grant_id: string; readonly tenant_id: string; readonly actor_id: string; readonly session_id: string; readonly authorization_digest: string; readonly domains: AutonomousDomainName[]; readonly operation: AutonomousAuthorizationOperation; readonly capability: string | null; readonly risk_class: string | null; readonly resource_digest: string | null; readonly issued_at: number; readonly request_digest: string;
  constructor(input: { request_id: string; grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; domains: AutonomousDomainName[]; operation: AutonomousAuthorizationOperation; capability: string | null; risk_class: string | null; resource_digest: string | null; issued_at: number; request_digest: string }) { this.request_id = identifier("request_id", input.request_id); this.grant_id = identifier("request grant_id", input.grant_id); this.tenant_id = identifier("request tenant_id", input.tenant_id); this.actor_id = identifier("request actor_id", input.actor_id); this.session_id = identifier("request session_id", input.session_id); this.authorization_digest = digest("request authorization_digest", input.authorization_digest)!; this.domains = normalizeAuthorizationDomains("request domains", input.domains); this.operation = identifier("request operation", input.operation) as AutonomousAuthorizationOperation; if (!OPERATIONS.has(this.operation)) fail("request operation is unsupported"); this.capability = input.capability === null ? null : identifier("request capability", input.capability); this.risk_class = input.risk_class === null ? null : identifier("request risk_class", input.risk_class); this.resource_digest = digest("request resource_digest", input.resource_digest, true); this.issued_at = timestamp("request issued_at", input.issued_at); this.request_digest = digest("request request_digest", input.request_digest)!; if (this.request_digest !== digestJsonSync(requestCore(this))) fail("request_digest does not match request metadata"); }
  static create(input: { request_id: string; grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; domains: AutonomousDomainName[]; operation: AutonomousAuthorizationOperation; capability?: string | null; risk_class?: string | null; resource_digest?: string | null; issued_at: number }): AutonomousAuthorizationRequest { const core = { schema: AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA, request_id: identifier("request_id", input.request_id), grant_id: identifier("request grant_id", input.grant_id), tenant_id: identifier("request tenant_id", input.tenant_id), actor_id: identifier("request actor_id", input.actor_id), session_id: identifier("request session_id", input.session_id), authorization_digest: digest("request authorization_digest", input.authorization_digest)!, domains: normalizeAuthorizationDomains("request domains", input.domains), operation: identifier("request operation", input.operation) as AutonomousAuthorizationOperation, capability: input.capability === undefined || input.capability === null ? null : identifier("request capability", input.capability), risk_class: input.risk_class === undefined || input.risk_class === null ? null : identifier("request risk_class", input.risk_class), resource_digest: digest("request resource_digest", input.resource_digest === undefined ? null : input.resource_digest, true), issued_at: timestamp("request issued_at", input.issued_at) }; if (!OPERATIONS.has(core.operation)) fail("request operation is unsupported"); return new AutonomousAuthorizationRequest({ ...core, request_digest: digestJsonSync(core) }); }
  static fromJSON(value: unknown): AutonomousAuthorizationRequest { if (!isObject(value)) fail("request must be an object"); const expected = ["actor_id", "authority", "authorization_digest", "capability", "domains", "execution", "grant_id", "issued_at", "request_digest", "request_id", "resource_digest", "retention", "risk_class", "schema", "secret_material", "session_id", "tenant_id"]; if (Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA || value.retention !== AUTONOMOUS_AUTHORIZATION_RETENTION || value.authority !== AUTONOMOUS_AUTHORIZATION_AUTHORITY || value.execution !== AUTONOMOUS_AUTHORIZATION_EXECUTION || value.secret_material !== AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL) fail("request contains unsupported fields or invalid markers"); return new AutonomousAuthorizationRequest(value as unknown as ConstructorParameters<typeof AutonomousAuthorizationRequest>[0]); }
  get contextDigest(): string { return autonomousAuthorizationContextDigest({ tenantId: this.tenant_id, actorId: this.actor_id, sessionId: this.session_id, authorizationDigest: this.authorization_digest }); }
  toJSON(): AutonomousAuthorizationRequestJSON { return { ...requestCore(this), request_digest: this.request_digest, retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL } as AutonomousAuthorizationRequestJSON; }
}

function decisionCore(decision: AutonomousAuthorizationDecision): JsonObject { return { schema: AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA, status: decision.status, grant_id: decision.grant_id, request_digest: decision.request_digest, grant_digest: decision.grant_digest, context_digest: decision.context_digest, checked_at: decision.checked_at, reason: decision.reason, remaining_uses: decision.remaining_uses }; }
export interface AutonomousAuthorizationDecisionJSON extends JsonObject { schema: typeof AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA; status: AutonomousAuthorizationDecisionStatus; grant_id: string; request_digest: string; grant_digest: string | null; context_digest: string; checked_at: number; reason: string; remaining_uses: number | null; decision_digest: string; retention: typeof AUTONOMOUS_AUTHORIZATION_RETENTION; authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY; execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION; secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL; }
export class AutonomousAuthorizationDecision { readonly status: AutonomousAuthorizationDecisionStatus; readonly grant_id: string; readonly request_digest: string; readonly grant_digest: string | null; readonly context_digest: string; readonly checked_at: number; readonly reason: string; readonly remaining_uses: number | null; readonly decision_digest: string; constructor(input: { status: AutonomousAuthorizationDecisionStatus; grant_id: string; request_digest: string; grant_digest: string | null; context_digest: string; checked_at: number; reason: string; remaining_uses: number | null; decision_digest: string }) { if (!AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES.includes(input.status)) fail("decision status is invalid"); this.status = input.status; this.grant_id = identifier("decision grant_id", input.grant_id); this.request_digest = digest("decision request_digest", input.request_digest)!; this.grant_digest = digest("decision grant_digest", input.grant_digest, true); this.context_digest = digest("decision context_digest", input.context_digest)!; this.checked_at = timestamp("decision checked_at", input.checked_at); this.reason = identifier("decision reason", input.reason); this.remaining_uses = input.remaining_uses === null ? null : integer("decision remaining_uses", input.remaining_uses, 0, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT); this.decision_digest = digest("decision decision_digest", input.decision_digest)!; if (this.decision_digest !== digestJsonSync(decisionCore(this))) fail("decision_digest does not match decision metadata"); } static create(input: { status: AutonomousAuthorizationDecisionStatus; request: AutonomousAuthorizationRequest; grant: AutonomousAuthorizationGrant | null; checkedAt: number; reason: string; remainingUses: number | null }): AutonomousAuthorizationDecision { const core = { status: input.status, grant_id: input.request.grant_id, request_digest: input.request.request_digest, grant_digest: input.grant?.grant_digest ?? null, context_digest: input.request.contextDigest, checked_at: timestamp("decision checkedAt", input.checkedAt), reason: identifier("decision reason", input.reason), remaining_uses: input.remainingUses }; return new AutonomousAuthorizationDecision({ ...core, decision_digest: digestJsonSync({ schema: AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA, ...core }) }); } toJSON(): AutonomousAuthorizationDecisionJSON { return { ...decisionCore(this), decision_digest: this.decision_digest, retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL } as AutonomousAuthorizationDecisionJSON; } }

function eventBody(event: AutonomousAuthorizationEvent): JsonObject { return { schema: AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA, sequence: event.sequence, event_type: event.event_type, grant_id: event.grant_id, request_digest: event.request_digest, occurred_at: event.occurred_at, reason: event.reason, previous_event_digest: event.previous_event_digest }; }
export interface AutonomousAuthorizationEventJSON extends JsonObject { schema: typeof AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA; sequence: number; event_type: AutonomousAuthorizationEventType; grant_id: string; request_digest: string | null; occurred_at: number; reason: string; previous_event_digest: string | null; event_digest: string; retention: typeof AUTONOMOUS_AUTHORIZATION_RETENTION; authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY; execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION; secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL; }
export class AutonomousAuthorizationEvent { readonly sequence: number; readonly event_type: AutonomousAuthorizationEventType; readonly grant_id: string; readonly request_digest: string | null; readonly occurred_at: number; readonly reason: string; readonly previous_event_digest: string | null; readonly event_digest: string; constructor(input: { sequence: number; event_type: AutonomousAuthorizationEventType; grant_id: string; request_digest: string | null; occurred_at: number; reason: string; previous_event_digest: string | null; event_digest: string }) { this.sequence = integer("event sequence", input.sequence, 1, MAX_AUTONOMOUS_AUTHORIZATION_EVENTS); if (!AUTONOMOUS_AUTHORIZATION_EVENT_TYPES.includes(input.event_type)) fail("event type is invalid"); this.event_type = input.event_type; this.grant_id = identifier("event grant_id", input.grant_id); this.request_digest = digest("event request_digest", input.request_digest, true); this.occurred_at = timestamp("event occurred_at", input.occurred_at); this.reason = identifier("event reason", input.reason); this.previous_event_digest = digest("event previous_event_digest", input.previous_event_digest, true); this.event_digest = digest("event event_digest", input.event_digest)!; if (this.event_digest !== digestJsonSync(eventBody(this))) fail("event_digest does not match event metadata"); } static create(input: { sequence: number; eventType: AutonomousAuthorizationEventType; grantId: string; requestDigest: string | null; occurredAt: number; reason: string; previousEventDigest: string | null }): AutonomousAuthorizationEvent { const body = { schema: AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA, sequence: input.sequence, event_type: input.eventType, grant_id: input.grantId, request_digest: input.requestDigest, occurred_at: input.occurredAt, reason: input.reason, previous_event_digest: input.previousEventDigest }; return new AutonomousAuthorizationEvent({ sequence: input.sequence, event_type: input.eventType, grant_id: input.grantId, request_digest: input.requestDigest, occurred_at: input.occurredAt, reason: input.reason, previous_event_digest: input.previousEventDigest, event_digest: digestJsonSync(body) }); } static fromJSON(value: unknown): AutonomousAuthorizationEvent { if (!isObject(value)) fail("event must be an object"); const expected = ["authority", "event_digest", "event_type", "execution", "grant_id", "occurred_at", "previous_event_digest", "reason", "request_digest", "retention", "schema", "secret_material", "sequence"]; if (Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA || value.retention !== AUTONOMOUS_AUTHORIZATION_RETENTION || value.authority !== AUTONOMOUS_AUTHORIZATION_AUTHORITY || value.execution !== AUTONOMOUS_AUTHORIZATION_EXECUTION || value.secret_material !== AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL) fail("event contains unsupported fields or invalid markers"); return new AutonomousAuthorizationEvent({ sequence: value.sequence as number, event_type: value.event_type as AutonomousAuthorizationEventType, grant_id: value.grant_id as string, request_digest: value.request_digest as string | null, occurred_at: value.occurred_at as number, reason: value.reason as string, previous_event_digest: value.previous_event_digest as string | null, event_digest: value.event_digest as string }); } toJSON(): AutonomousAuthorizationEventJSON { return { ...eventBody(this), event_digest: this.event_digest, retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL } as AutonomousAuthorizationEventJSON; } }

export interface AutonomousAuthorizationSnapshotJSON extends JsonObject { schema: typeof AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA; generation: number; previous_snapshot_digest: string | null; grants: AutonomousAuthorizationGrantJSON[]; events: AutonomousAuthorizationEventJSON[]; retention: typeof AUTONOMOUS_AUTHORIZATION_RETENTION; authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY; execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION; secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL; snapshot_digest: string; }
export function validateAutonomousAuthorizationSnapshot(value: unknown): AutonomousAuthorizationSnapshotJSON { if (!isObject(value)) fail("snapshot must be an object"); const expected = ["authority", "events", "execution", "generation", "grants", "previous_snapshot_digest", "retention", "schema", "secret_material", "snapshot_digest"]; if (Object.keys(value).sort().join(",") !== expected.join(",") || value.schema !== AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA || value.retention !== AUTONOMOUS_AUTHORIZATION_RETENTION || value.authority !== AUTONOMOUS_AUTHORIZATION_AUTHORITY || value.execution !== AUTONOMOUS_AUTHORIZATION_EXECUTION || value.secret_material !== AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL) fail("snapshot contains unsupported fields or invalid markers"); safeMetadata(value); const generation = integer("snapshot generation", value.generation); const previous = digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest, true); if (!Array.isArray(value.grants) || value.grants.length > MAX_AUTONOMOUS_AUTHORIZATION_GRANTS || !Array.isArray(value.events) || value.events.length > MAX_AUTONOMOUS_AUTHORIZATION_EVENTS) fail("snapshot records exceed their bound"); const grants = value.grants.map((item) => AutonomousAuthorizationGrant.fromJSON(item)); if (new Set(grants.map((item) => item.grant_id)).size !== grants.length) fail("snapshot contains duplicate grant ids"); const events = value.events.map((item) => AutonomousAuthorizationEvent.fromJSON(item)); const grantIds = new Set(grants.map((item) => item.grant_id)); const issued = new Set<string>(); events.forEach((event, index) => { if (event.sequence !== index + 1 || event.previous_event_digest !== (index === 0 ? null : events[index - 1]!.event_digest) || !grantIds.has(event.grant_id)) fail("snapshot event chain is invalid"); const grant = grants.find((item) => item.grant_id === event.grant_id)!; if (event.event_type === "grant_issued") { if (event.request_digest !== null || issued.has(event.grant_id)) fail("snapshot grant issuance history is inconsistent"); issued.add(event.grant_id); } else if (!issued.has(event.grant_id)) fail("snapshot event precedes grant issuance"); else if (event.event_type === "grant_revoked") { if (event.request_digest !== null || grant.status !== "revoked") fail("snapshot grant revocation history is inconsistent"); } else if ((event.event_type === "request_allowed" || event.event_type === "request_replayed") && (event.request_digest === null || !grant.used_request_digests.includes(event.request_digest))) fail("snapshot request history is inconsistent with grant use accounting"); }); if (issued.size !== grantIds.size) fail("snapshot is missing grant issuance history"); const body = { schema: AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA, generation, previous_snapshot_digest: previous, grants: grants.sort((a, b) => a.grant_id.localeCompare(b.grant_id)).map((item) => item.toJSON()), events: events.map((item) => item.toJSON()), retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL }; const supplied = digest("snapshot snapshot_digest", value.snapshot_digest)!; if (supplied !== digestJsonSync(body)) fail("snapshot_digest does not match snapshot contents"); const result = { ...body, snapshot_digest: supplied } as AutonomousAuthorizationSnapshotJSON; if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound"); return clone(result); }
export function sealAutonomousAuthorizationSnapshot(input: { generation: number; grants: readonly AutonomousAuthorizationGrant[]; events: readonly AutonomousAuthorizationEvent[]; previousSnapshotDigest?: string | null }): AutonomousAuthorizationSnapshotJSON { const body = { schema: AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA, generation: integer("snapshot generation", input.generation), previous_snapshot_digest: digest("snapshot previousSnapshotDigest", input.previousSnapshotDigest ?? null, true), grants: [...input.grants].sort((a, b) => a.grant_id.localeCompare(b.grant_id)).map((item) => item.toJSON()), events: [...input.events].map((item) => item.toJSON()), retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL }; return validateAutonomousAuthorizationSnapshot({ ...body, snapshot_digest: digestJsonSync(body) }); }

export class AutonomousAuthorizationLedger {
  private readonly grantMap = new Map<string, AutonomousAuthorizationGrant>(); private eventLog: AutonomousAuthorizationEvent[] = []; private generation = 0; private previousSnapshotDigest: string | null = null;
  constructor(readonly maxGrants = MAX_AUTONOMOUS_AUTHORIZATION_GRANTS, readonly maxEvents = MAX_AUTONOMOUS_AUTHORIZATION_EVENTS) { integer("maxGrants", maxGrants, 1, MAX_AUTONOMOUS_AUTHORIZATION_GRANTS); integer("maxEvents", maxEvents, 1, MAX_AUTONOMOUS_AUTHORIZATION_EVENTS); }
  private append(eventType: AutonomousAuthorizationEventType, grantId: string, requestDigest: string | null, occurredAt: number, reason: string): void { if (this.eventLog.length >= this.maxEvents) fail("event capacity is exhausted"); this.eventLog.push(AutonomousAuthorizationEvent.create({ sequence: this.eventLog.length + 1, eventType, grantId, requestDigest, occurredAt, reason: identifier("event reason", reason), previousEventDigest: this.eventLog.at(-1)?.event_digest ?? null })); }
  issue(input: { grant_id: string; tenant_id: string; actor_id: string; session_id: string; authorization_digest: string; allowed_domains: AutonomousDomainName[]; allowed_operations: AutonomousAuthorizationOperation[]; allowed_capabilities?: string[]; allowed_risk_classes?: string[]; issued_at: number; expires_at: number; max_uses?: number | null }): AutonomousAuthorizationGrant { const id = identifier("grant_id", input.grant_id); if (this.grantMap.has(id)) fail("grant_id already exists"); if (this.grantMap.size >= this.maxGrants) fail("grant capacity is exhausted"); const grant = AutonomousAuthorizationGrant.issue({ ...input, grant_id: id }); this.grantMap.set(id, grant); this.append("grant_issued", id, null, grant.issued_at, "issued"); return grant; }
  revoke(grantId: string, revokedAt: number, reason = "revoked"): AutonomousAuthorizationGrant { const id = identifier("grant_id", grantId); const current = this.grantMap.get(id); if (!current) fail("cannot revoke an unknown grant"); if (current.status === "revoked") return current; const updated = new AutonomousAuthorizationGrant({ ...current.toJSON(), status: "revoked", revoked_at: timestamp("revokedAt", revokedAt), revocation_reason_digest: digestJsonSync(identifier("revocation reason", reason)) }); this.grantMap.set(id, updated); this.append("grant_revoked", id, null, revokedAt, reason); return updated; }
  private currentStatus(grant: AutonomousAuthorizationGrant, now: number): AutonomousAuthorizationGrantStatus { if (grant.status === "revoked") return "revoked"; if (now >= grant.expires_at) return "expired"; if (grant.max_uses !== null && grant.used_count >= grant.max_uses) return "exhausted"; return "active"; }
  authorize(request: AutonomousAuthorizationRequest | AutonomousAuthorizationRequestJSON, now: number): AutonomousAuthorizationDecision { const normalized = request instanceof AutonomousAuthorizationRequest ? request : AutonomousAuthorizationRequest.fromJSON(request); const checkedAt = timestamp("authorization checkedAt", now); const found = this.grantMap.get(normalized.grant_id); if (!found) return AutonomousAuthorizationDecision.create({ status: "not_found", request: normalized, grant: null, checkedAt, reason: "grant_not_found", remainingUses: null }); let grant = found; const status = this.currentStatus(grant, checkedAt); if (status !== grant.status && (status === "expired" || status === "exhausted")) { grant = new AutonomousAuthorizationGrant({ ...grant.toJSON(), status }); this.grantMap.set(grant.grant_id, grant); } const remaining = grant.max_uses === null ? null : Math.max(0, grant.max_uses - grant.used_count); if (status !== "active") return AutonomousAuthorizationDecision.create({ status, request: normalized, grant, checkedAt, reason: `grant_${status}`, remainingUses: remaining }); if (grant.used_request_digests.includes(normalized.request_digest)) { this.append("request_replayed", grant.grant_id, normalized.request_digest, checkedAt, "request_replay"); return AutonomousAuthorizationDecision.create({ status: "already_allowed", request: normalized, grant, checkedAt, reason: "request_replay", remainingUses: remaining }); } const checks: Array<[boolean, AutonomousAuthorizationDecisionStatus]> = [[normalized.tenant_id === grant.tenant_id, "tenant_mismatch"], [normalized.actor_id === grant.actor_id, "actor_mismatch"], [normalized.session_id === grant.session_id, "session_mismatch"], [normalized.authorization_digest === grant.authorization_digest, "authorization_mismatch"], [normalized.domains.every((domain) => grant.allowed_domains.includes(domain)), "domain_denied"], [grant.allowed_operations.includes(normalized.operation), "operation_denied"], [grant.allowed_capabilities.length === 0 || (normalized.capability !== null && grant.allowed_capabilities.includes(normalized.capability)), "capability_denied"], [grant.allowed_risk_classes.length === 0 || (normalized.risk_class !== null && grant.allowed_risk_classes.includes(normalized.risk_class)), "risk_denied"]]; const failure = checks.find(([passed]) => !passed)?.[1]; if (failure) return AutonomousAuthorizationDecision.create({ status: failure, request: normalized, grant, checkedAt, reason: failure, remainingUses: remaining }); if (grant.used_request_digests.length >= MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT) fail("grant request replay window is exhausted"); const usedCount = grant.used_count + 1; const nextStatus: AutonomousAuthorizationGrantStatus = grant.max_uses !== null && usedCount >= grant.max_uses ? "exhausted" : "active"; grant = new AutonomousAuthorizationGrant({ ...grant.toJSON(), used_count: usedCount, used_request_digests: [...grant.used_request_digests, normalized.request_digest], status: nextStatus }); this.grantMap.set(grant.grant_id, grant); this.append("request_allowed", grant.grant_id, normalized.request_digest, checkedAt, "allowed"); return AutonomousAuthorizationDecision.create({ status: "allowed", request: normalized, grant, checkedAt, reason: "allowed", remainingUses: grant.max_uses === null ? null : Math.max(0, grant.max_uses - grant.used_count) }); }
  get(grantId: string): AutonomousAuthorizationGrant | null { return this.grantMap.get(identifier("grant_id", grantId)) ?? null; }
  grants(): AutonomousAuthorizationGrant[] { return [...this.grantMap.values()].sort((a, b) => a.grant_id.localeCompare(b.grant_id)); }
  events(): AutonomousAuthorizationEvent[] { return [...this.eventLog]; }
  snapshot(): AutonomousAuthorizationSnapshotJSON { return sealAutonomousAuthorizationSnapshot({ generation: this.generation, grants: this.grants(), events: this.events(), previousSnapshotDigest: this.previousSnapshotDigest }); }
  restore(snapshot: unknown): AutonomousAuthorizationSnapshotJSON { const normalized = validateAutonomousAuthorizationSnapshot(snapshot); if (normalized.grants.length > this.maxGrants || normalized.events.length > this.maxEvents) fail("snapshot exceeds ledger capacity"); this.grantMap.clear(); normalized.grants.forEach((item) => { const grant = AutonomousAuthorizationGrant.fromJSON(item); this.grantMap.set(grant.grant_id, grant); }); this.eventLog = normalized.events.map((item) => AutonomousAuthorizationEvent.fromJSON(item)); this.generation = normalized.generation; this.previousSnapshotDigest = normalized.snapshot_digest; return clone(normalized); }
  verifyIntegrity(): JsonObject { const snapshot = this.snapshot(); const grants = this.grants(); return { verified: true, grant_count: grants.length, event_count: this.eventLog.length, active_grant_count: grants.filter((item) => item.status === "active").length, revoked_grant_count: grants.filter((item) => item.status === "revoked").length, expired_grant_count: grants.filter((item) => item.status === "expired").length, exhausted_grant_count: grants.filter((item) => item.status === "exhausted").length, domain_coverage: Object.fromEntries(DOMAINS.map((domain) => [domain, grants.filter((grant) => grant.allowed_domains.includes(domain)).length])), snapshot_digest: snapshot.snapshot_digest, retention: AUTONOMOUS_AUTHORIZATION_RETENTION, authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL }; }
  restoreCheckpoint(snapshot: AutonomousAuthorizationSnapshotJSON): void { this.restore(snapshot); }
  setCheckpointMetadata(generation: number, previousSnapshotDigest: string | null): void { this.generation = integer("checkpoint generation", generation); this.previousSnapshotDigest = digest("checkpoint previousSnapshotDigest", previousSnapshotDigest, true); }
}

export interface AutonomousAuthorizedOperation<T = unknown> {
  decision: AutonomousAuthorizationDecisionJSON;
  result_present: true;
  result_retained: false;
  retention: "transient_caller_result_only";
  authority: typeof AUTONOMOUS_AUTHORIZATION_AUTHORITY;
  execution: typeof AUTONOMOUS_AUTHORIZATION_EXECUTION;
  secret_material: typeof AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL;
  /** The transient caller-owned result is available only on the runtime value, never in toJSON. */
  result?: T;
}

export class AutonomousAuthorizationGate {
  constructor(readonly ledger: AutonomousAuthorizationLedger) { if (!(ledger instanceof AutonomousAuthorizationLedger)) fail("gate requires an AutonomousAuthorizationLedger"); }
  require(request: AutonomousAuthorizationRequest | AutonomousAuthorizationRequestJSON, now: number): AutonomousAuthorizationDecision { const decision = this.ledger.authorize(request, now); if (decision.status !== "allowed" && decision.status !== "already_allowed") fail(`operation authorization was refused: ${decision.status}`); return decision; }
  async execute<T>(request: AutonomousAuthorizationRequest | AutonomousAuthorizationRequestJSON, now: number, operation: () => T | Promise<T>): Promise<AutonomousAuthorizedOperation<T>> { if (typeof operation !== "function") fail("authorized operation must be callable"); const decision = this.require(request, now); const result = await operation(); return { decision: decision.toJSON(), result_present: true, result_retained: false, retention: "transient_caller_result_only", authority: AUTONOMOUS_AUTHORIZATION_AUTHORITY, execution: AUTONOMOUS_AUTHORIZATION_EXECUTION, secret_material: AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL, result }; }
  toJSON<T>(operation: AutonomousAuthorizedOperation<T>): Omit<AutonomousAuthorizedOperation<T>, "result"> { const { result: _result, ...safe } = operation; return safe; }
}

/** Bind one caller-issued grant to live provider attempts without retaining payloads or secrets. */
export class AutonomousAuthorizationContext {
  private requestCounter = 0;

  constructor(
    readonly gate: AutonomousAuthorizationGate,
    readonly grantId: string,
    readonly tenantId: string,
    readonly actorId: string,
    readonly sessionId: string,
    readonly authorizationDigest: string,
    readonly domains: AutonomousDomainName[],
    readonly capability: string | null = null,
    readonly riskClass: string | null = "provider_invocation",
    readonly requestPrefix = "provider",
    readonly clock: () => number = () => Date.now(),
  ) {
    if (!(gate instanceof AutonomousAuthorizationGate)) fail("context requires an AutonomousAuthorizationGate");
    identifier("context grantId", grantId); identifier("context tenantId", tenantId); identifier("context actorId", actorId); identifier("context sessionId", sessionId);
    digest("context authorizationDigest", authorizationDigest); normalizeAuthorizationDomains("context domains", domains);
    if (capability !== null) identifier("context capability", capability);
    if (riskClass !== null) identifier("context riskClass", riskClass);
    identifier("context requestPrefix", text("context requestPrefix", requestPrefix, 128));
    if (typeof clock !== "function") fail("context clock must be callable");
  }

  /** Return a child context narrowed to one already-authorized domain. */
  forDomain(domain: AutonomousDomainName): AutonomousAuthorizationContext {
    const normalized = identifier("context domain", domain) as AutonomousDomainName;
    if (!this.domains.includes(normalized)) fail("context domain is outside its authorized scope");
    return new AutonomousAuthorizationContext(this.gate, this.grantId, this.tenantId, this.actorId, this.sessionId, this.authorizationDigest, [normalized], this.capability, this.riskClass, this.requestPrefix, this.clock);
  }

  /** Require permission for one provider attempt; request metadata excludes task and provider payloads. */
  authorizeProvider(input: { provider: string; model: string; invocationKind: string; domain?: string; attempt?: number; turn?: number }): AutonomousAuthorizationDecision {
    const selectedDomain = input.domain ?? (this.domains.length === 1 ? this.domains[0] : undefined);
    if (!selectedDomain) fail("provider authorization requires an exact domain when context spans domains");
    const domain = identifier("provider authorization domain", selectedDomain) as AutonomousDomainName;
    if (!this.domains.includes(domain)) fail("provider authorization domain is outside its context scope");
    const provider = identifier("provider authorization provider", input.provider);
    const model = identifier("provider authorization model", input.model);
    const invocationKind = identifier("provider authorization invocationKind", input.invocationKind);
    const attempt = integer("provider authorization attempt", input.attempt ?? 0, 0, 8);
    const turn = integer("provider authorization turn", input.turn ?? 0, 0, 32);
    const issuedAt = timestamp("provider authorization issuedAt", this.clock());
    const resourceDigest = digestJsonSync({ schema: "bioprism-autonomous-provider-authorization-resource/0.1", domain, provider, model, invocation_kind: invocationKind, attempt, turn });
    this.requestCounter += 1;
    const request = AutonomousAuthorizationRequest.create({
      request_id: identifier("provider authorization requestId", `${this.requestPrefix}-${this.requestCounter}`),
      grant_id: this.grantId,
      tenant_id: this.tenantId,
      actor_id: this.actorId,
      session_id: this.sessionId,
      authorization_digest: this.authorizationDigest,
      domains: [domain],
      operation: "provider_invocation",
      capability: this.capability,
      risk_class: this.riskClass,
      resource_digest: resourceDigest,
      issued_at: issuedAt,
    });
    return this.gate.require(request, issuedAt);
  }
}

export interface AutonomousAuthorizationSnapshotTextStore { read(): Promise<string | null> | string | null; write(value: string): Promise<void> | void; }
export interface AutonomousAuthorizationTransactionalSnapshotTextStore extends AutonomousAuthorizationSnapshotTextStore { writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean; }
export interface AutonomousAuthorizationSnapshotPersistence { read(): Promise<unknown | null> | unknown | null; write(snapshot: unknown): Promise<void> | void; writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: unknown): Promise<boolean> | boolean; }

export class JsonAutonomousAuthorizationSnapshotPersistence implements AutonomousAuthorizationSnapshotPersistence {
  constructor(readonly store: AutonomousAuthorizationSnapshotTextStore, readonly maxBytes = MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES) { if (!store || typeof store.read !== "function" || typeof store.write !== "function") fail("JSON persistence requires a text store"); integer("persistence maxBytes", maxBytes, 1, MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES); }
  read(): AutonomousAuthorizationSnapshotJSON | null { const encoded = this.store.read(); if (encoded instanceof Promise) throw new ArgumentError("autonomous authorization text store must be synchronous"); if (encoded === null) return null; if (typeof encoded !== "string" || bytes(encoded) > this.maxBytes) fail("stored JSON exceeds its byte bound"); let raw: unknown; try { raw = JSON.parse(encoded); } catch (error) { throw new AutonomousAuthorizationError("autonomous authorization stored JSON is invalid", { cause: error }); } const normalized = validateAutonomousAuthorizationSnapshot(raw); if (canonicalJson(normalized) !== encoded) fail("stored JSON is not canonical"); return normalized; }
  write(snapshot: unknown): void { const normalized = validateAutonomousAuthorizationSnapshot(snapshot); const encoded = canonicalJson(normalized); if (bytes(encoded) > this.maxBytes) fail("snapshot exceeds configured byte capacity"); const result = this.store.write(encoded); if (result instanceof Promise) throw new ArgumentError("autonomous authorization text store must be synchronous"); }
}

export class TransactionalJsonAutonomousAuthorizationSnapshotPersistence extends JsonAutonomousAuthorizationSnapshotPersistence {
  constructor(override readonly store: AutonomousAuthorizationTransactionalSnapshotTextStore, maxBytes = MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES) { super(store, maxBytes); if (typeof store.writeIfUnchanged !== "function") fail("transactional JSON persistence requires writeIfUnchanged"); }
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: unknown): boolean { digest("expectedSnapshotDigest", expectedSnapshotDigest, true); const normalized = validateAutonomousAuthorizationSnapshot(snapshot); const result = this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(normalized)); if (result instanceof Promise) throw new ArgumentError("autonomous authorization text store must be synchronous"); return Boolean(result); }
}

export class AutonomousAuthorizationPersistenceCoordinator {
  expectedSnapshotDigest: string | null = null; expectedGeneration = 0;
  constructor(readonly ledger: AutonomousAuthorizationLedger, readonly persistence: AutonomousAuthorizationSnapshotPersistence) { if (!(ledger instanceof AutonomousAuthorizationLedger) || !persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") fail("coordinator dependencies are malformed"); }
  restore(): AutonomousAuthorizationSnapshotJSON | null { const snapshot = this.persistence.read(); if (snapshot instanceof Promise) throw new ArgumentError("autonomous authorization persistence must be synchronous"); if (snapshot === null) { this.expectedSnapshotDigest = null; this.expectedGeneration = 0; return null; } const normalized = validateAutonomousAuthorizationSnapshot(snapshot); this.ledger.restore(normalized); this.expectedSnapshotDigest = normalized.snapshot_digest; this.expectedGeneration = normalized.generation; return normalized; }
  flush(): AutonomousAuthorizationSnapshotJSON { const snapshot = sealAutonomousAuthorizationSnapshot({ generation: this.expectedGeneration + 1, grants: this.ledger.grants(), events: this.ledger.events(), previousSnapshotDigest: this.expectedSnapshotDigest }); const writer = this.persistence.writeIfUnchanged; if (writer) { const result = writer.call(this.persistence, this.expectedSnapshotDigest, snapshot); if (result instanceof Promise) throw new ArgumentError("autonomous authorization persistence must be synchronous"); if (!result) fail("persistence compare-and-set conflict"); } else { this.persistence.write(snapshot); } this.expectedSnapshotDigest = snapshot.snapshot_digest; this.expectedGeneration = snapshot.generation; this.ledger.setCheckpointMetadata(this.expectedGeneration, this.expectedSnapshotDigest); return snapshot; }
}
