import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Redacted, restart-safe capability activation shared with the Python autonomous façade. */
export const AUTONOMOUS_ACTIVATION_SCHEMA = "bioprism-python-autonomous-capability-activation/0.1" as const;
export const AUTONOMOUS_ACTIVATION_STORE_SCHEMA = "bioprism-python-autonomous-capability-activation-store/0.1" as const;
export const AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA = "bioprism-typescript-autonomous-domain-tool-plan/0.1" as const;
export const AUTONOMOUS_ACTIVATION_STATUSES = [
  "created",
  "provider_pending",
  "catalogue_pending",
  "review_required",
  "partially_activated",
  "ready",
  "stale",
  "revoked",
] as const;
export type AutonomousActivationStatus = typeof AUTONOMOUS_ACTIVATION_STATUSES[number];

export const MAX_ACTIVATION_PROVIDERS = 64;
export const MAX_ACTIVATION_TOOLS = 512;
export const MAX_ACTIVATION_DOMAINS = 12;
export const MAX_ACTIVATION_STATE_BYTES = 512_000;
export const MAX_ACTIVATION_STORE_BYTES = 1_000_000;
export const MAX_ACTIVATION_ERROR_BYTES = 2_000;

export class AutonomousActivationError extends ArgumentError {
  override readonly name = "AutonomousActivationError";
}

export interface AutonomousActivationProviderStatus extends JsonObject {
  provider: string;
  provider_registered: boolean;
  requires_credential: boolean | null;
  credential_configured: boolean;
  credential_count: number;
  ready: boolean;
  next_action: string;
  secret_persistence: "in_memory_only";
}

export interface AutonomousActivationDomainStatus extends JsonObject {
  domain: string;
  required_tool_count: number;
  available_tool_count: number;
  proposed_tool_count: number;
  missing_tools: string[];
  missing_capabilities: string[];
  coverage_ratio: number;
  approved_coverage_ratio: number;
  status: "unavailable" | "partial" | "available";
}

export interface AutonomousCapabilityActivationState extends JsonObject {
  schema: typeof AUTONOMOUS_ACTIVATION_SCHEMA;
  activation_id: string;
  status: AutonomousActivationStatus;
  revision: number;
  created_at: number;
  updated_at: number;
  catalogue_digest: string | null;
  plan_digest: string | null;
  profile_digest: string | null;
  approved_tools: string[];
  pending_review_tools: string[];
  unclassified_tools: string[];
  provider_statuses: AutonomousActivationProviderStatus[];
  domain_statuses: AutonomousActivationDomainStatus[];
  registered_tool_count: number;
  last_error: string | null;
  state_digest: string;
  retention: "metadata_only_no_keys_handles_prompts_tasks_or_payloads";
  authorization: "status_only; does_not_grant_provider_or_tool_authority";
  secret_material: "never_returned";
}

export interface AutonomousCapabilityActivationSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_ACTIVATION_STORE_SCHEMA;
  state: AutonomousCapabilityActivationState;
  state_digest: string;
  snapshot_digest: string;
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
}

export interface AutonomousCapabilityActivationPersistence {
  read(): Promise<AutonomousCapabilityActivationSnapshot | null> | AutonomousCapabilityActivationSnapshot | null;
  write(snapshot: AutonomousCapabilityActivationSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousCapabilityActivationSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousCapabilityActivationSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousCapabilityActivationTransactionalSnapshotTextStore extends AutonomousCapabilityActivationSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousCapabilityActivationSnapshotStore {
  load(): Promise<AutonomousCapabilityActivationState | null> | AutonomousCapabilityActivationState | null;
  save(state: AutonomousCapabilityActivationState): Promise<void> | void;
  saveIfUnchanged?(expectedStateDigest: string | null, state: AutonomousCapabilityActivationState): Promise<boolean> | boolean;
  snapshot(): Promise<AutonomousCapabilityActivationSnapshot> | AutonomousCapabilityActivationSnapshot;
  restore(snapshot: AutonomousCapabilityActivationSnapshot): Promise<void> | void;
}

const ALLOWED_TRANSITIONS: Readonly<Record<AutonomousActivationStatus, readonly AutonomousActivationStatus[]>> = {
  created: ["created", "provider_pending", "catalogue_pending", "review_required", "revoked"],
  provider_pending: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  catalogue_pending: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  review_required: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  partially_activated: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  ready: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  stale: ["provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked"],
  revoked: ["revoked"],
};

const SECRET_KEYS = new Set([
  "apikey", "authorization", "bearer", "credential", "password", "secret", "accesstoken",
  "refreshtoken", "token", "privatekey", "prompt", "response", "rawpayload", "arguments", "output", "task", "messages",
]);
const ACTIVATION_DOMAINS = new Set([
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation",
]);
const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;

function clone<T>(value: T): T {
  return structuredClone(value);
}

function jsonBytes(value: unknown): number {
  let serialized: string;
  try { serialized = JSON.stringify(value); } catch { throw new AutonomousActivationError("activation metadata must be JSON serializable"); }
  if (typeof serialized !== "string") throw new AutonomousActivationError("activation metadata must be JSON serializable");
  return new TextEncoder().encode(serialized).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new AutonomousActivationError(`${name} must be bounded text`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum = 512): string {
  const text = boundedText(name, value, maximum);
  if (!IDENTIFIER.test(text)) throw new AutonomousActivationError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !DIGEST.test(value)) throw new AutonomousActivationError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) throw new AutonomousActivationError(`${name} is outside its bound`);
  return value as number;
}

function boundedRatio(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new AutonomousActivationError(`${name} must be within [0, 1]`);
  return value;
}

function normalizedKey(key: string): string {
  return key.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function assertSafe(value: unknown, depth = 0): void {
  if (depth > 24) throw new AutonomousActivationError("activation metadata is too deeply nested");
  if (Array.isArray(value)) {
    if (value.length > 8_192) throw new AutonomousActivationError("activation metadata contains too many rows");
    for (const child of value) assertSafe(child, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (SECRET_KEYS.has(normalizedKey(key))) throw new AutonomousActivationError("activation metadata contains transient or secret-shaped fields");
    assertSafe(child, depth + 1);
  }
}

function uniqueSorted(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new AutonomousActivationError(`${name} must be a bounded array`);
  const result = value.map((item) => boundedIdentifier(`${name} entry`, item));
  if (new Set(result).size !== result.length) throw new AutonomousActivationError(`${name} contains duplicate entries`);
  return result.sort();
}

function knownKeys(name: string, value: JsonObject, allowed: readonly string[]): void {
  const set = new Set(allowed);
  if (Object.keys(value).some((key) => !set.has(key))) throw new AutonomousActivationError(`${name} contains unsupported fields`);
}

function providerProjection(value: JsonObject): AutonomousActivationProviderStatus {
  knownKeys("provider activation status", value, ["provider", "provider_registered", "requires_credential", "credential_configured", "credential_count", "ready", "next_action", "credential_ready", "credential", "health", "circuit", "secret_material", "secret_persistence"]);
  const provider = boundedIdentifier("activation provider", value.provider);
  const requires = value.requires_credential;
  if (requires !== null && requires !== undefined && typeof requires !== "boolean") throw new AutonomousActivationError("provider requires_credential must be boolean or null");
  const nested = isObject(value.credential) ? value.credential : null;
  if (value.credential !== undefined && nested === null) throw new AutonomousActivationError("provider credential posture must be a metadata object");
  if (nested) knownKeys("provider credential posture", nested, ["configured", "ready", "active_handles", "credential_count", "expires_at", "next_action"]);
  const configured = typeof value.credential_configured === "boolean"
    ? value.credential_configured
    : nested ? nested.configured === true || nested.ready === true : value.credential_ready === true;
  const countValue = value.credential_count ?? nested?.credential_count ?? nested?.active_handles ?? 0;
  const count = boundedCount("provider credential_count", countValue, MAX_ACTIVATION_PROVIDERS);
  const ready = typeof value.ready === "boolean" ? value.ready : value.credential_ready === true;
  const nextAction = boundedIdentifier("provider next_action", value.next_action ?? (ready ? "ready" : "collect_user_credential"));
  return { provider, provider_registered: value.provider_registered === true, requires_credential: requires === undefined ? null : requires as boolean | null, credential_configured: configured, credential_count: count, ready, next_action: nextAction, secret_persistence: "in_memory_only" };
}

function coverageProjection(domain: string, raw: JsonObject, proposedToolCount: number): AutonomousActivationDomainStatus {
  const required = boundedCount(`coverage ${domain} required_tool_count`, raw.required_tool_count ?? 0, MAX_ACTIVATION_TOOLS);
  const available = boundedCount(`coverage ${domain} available_tool_count`, raw.available_tool_count ?? 0, MAX_ACTIVATION_TOOLS);
  const proposed = boundedCount(`coverage ${domain} proposed_tool_count`, proposedToolCount, MAX_ACTIVATION_TOOLS);
  const missingTools = uniqueSorted(`coverage ${domain} missing_tools`, raw.missing_tools ?? [], MAX_ACTIVATION_TOOLS);
  const missingCapabilities = uniqueSorted(`coverage ${domain} missing_capabilities`, raw.missing_capabilities ?? [], MAX_ACTIVATION_TOOLS);
  const coverageRatio = boundedRatio(`coverage ${domain} coverage_ratio`, raw.coverage_ratio ?? 0);
  const approvedRatio = boundedRatio(`coverage ${domain} approved_coverage_ratio`, raw.approved_coverage_ratio ?? 0);
  const status = proposed === 0 ? "unavailable" : proposed < required ? "partial" : "available";
  return { domain: boundedIdentifier("activation domain", domain), required_tool_count: required, available_tool_count: available, proposed_tool_count: proposed, missing_tools: missingTools, missing_capabilities: missingCapabilities, coverage_ratio: coverageRatio, approved_coverage_ratio: approvedRatio, status };
}

function bindingRow(name: string, raw: unknown): JsonObject {
  if (!isObject(raw)) throw new AutonomousActivationError("binding plan contains a malformed binding row");
  const row = raw as Record<string, unknown>;
  return {
    name: row.name ?? name,
    domains: row.domains ?? [],
    capability: row.capability ?? null,
    risk_class: row.risk_class ?? null,
    read_only: row.read_only ?? null,
    approval_required: row.approval_required ?? null,
    live_schema_digest: row.live_schema_digest ?? null,
    catalogue_digest: row.catalogue_digest ?? null,
  } as unknown as JsonObject;
}

/** Digest only policy-bearing catalogue fields; presentation and the digest itself are excluded. */
export function autonomousBindingPlanDigest(plan: JsonObject): string {
  if (!isObject(plan)) throw new AutonomousActivationError("binding plan must be an object");
  const proposed = Array.isArray(plan.proposed_bindings) ? plan.proposed_bindings : [];
  const review = Array.isArray(plan.review_bindings) ? plan.review_bindings : [];
  const sortedRows = (rows: unknown[]) => rows.map((row) => {
    const name = isObject(row) ? String(row.name ?? "") : "";
    return bindingRow(name, row);
  }).sort((left, right) => String(left.name).localeCompare(String(right.name)));
  const descriptor = {
    schema: plan.schema,
    catalogue_digest: plan.catalogue_digest,
    profile_digest: plan.profile_digest,
    domains: plan.domains ?? [],
    available_curated_tools: plan.available_curated_tools ?? [],
    missing_curated_tools: plan.missing_curated_tools ?? [],
    review_required_tools: plan.review_required_tools ?? [],
    unclassified_tools: plan.unclassified_tools ?? [],
    coverage: plan.coverage ?? [],
    proposed_bindings: sortedRows(proposed),
    review_bindings: sortedRows(review),
  };
  assertSafe(descriptor);
  if (jsonBytes(descriptor) > MAX_ACTIVATION_STATE_BYTES) throw new AutonomousActivationError("binding plan exceeds its metadata bound");
  return digestJsonSync(descriptor);
}

function statePayload(state: Omit<AutonomousCapabilityActivationState, "state_digest" | "retention" | "authorization" | "secret_material">): JsonObject {
  const typed = state as unknown as {
    schema: typeof AUTONOMOUS_ACTIVATION_SCHEMA;
    activation_id: string;
    status: AutonomousActivationStatus;
    revision: number;
    created_at: number;
    updated_at: number;
    catalogue_digest: string | null;
    plan_digest: string | null;
    profile_digest: string | null;
    approved_tools: string[];
    pending_review_tools: string[];
    unclassified_tools: string[];
    provider_statuses: AutonomousActivationProviderStatus[];
    domain_statuses: AutonomousActivationDomainStatus[];
    registered_tool_count: number;
    last_error: string | null;
  };
  return {
    schema: AUTONOMOUS_ACTIVATION_SCHEMA,
    activation_id: typed.activation_id,
    status: typed.status,
    revision: typed.revision,
    created_at: typed.created_at,
    updated_at: typed.updated_at,
    catalogue_digest: typed.catalogue_digest,
    plan_digest: typed.plan_digest,
    profile_digest: typed.profile_digest,
    approved_tools: [...typed.approved_tools],
    pending_review_tools: [...typed.pending_review_tools],
    unclassified_tools: [...typed.unclassified_tools],
    provider_statuses: typed.provider_statuses.map((row) => ({ ...row })),
    domain_statuses: typed.domain_statuses.map((row) => ({ ...row })),
    registered_tool_count: typed.registered_tool_count,
    last_error: typed.last_error,
  };
}

function sealState(raw: Omit<AutonomousCapabilityActivationState, "state_digest" | "retention" | "authorization" | "secret_material">): AutonomousCapabilityActivationState {
  const payload = statePayload(raw);
  const state = {
    ...payload,
    state_digest: digestJsonSync(payload),
    retention: "metadata_only_no_keys_handles_prompts_tasks_or_payloads",
    authorization: "status_only; does_not_grant_provider_or_tool_authority",
    secret_material: "never_returned",
  } as unknown as AutonomousCapabilityActivationState;
  return validateAutonomousCapabilityActivationState(state);
}

/** Validate a state before it crosses a process or persistence boundary. */
export function validateAutonomousCapabilityActivationState(value: unknown): AutonomousCapabilityActivationState {
  if (!isObject(value)) throw new AutonomousActivationError("activation state must be an object");
  const state = value as unknown as AutonomousCapabilityActivationState & {
    approved_tools: unknown;
    pending_review_tools: unknown;
    unclassified_tools: unknown;
    provider_statuses: unknown;
    domain_statuses: unknown;
    registered_tool_count: unknown;
    last_error: unknown;
  };
  knownKeys("activation state", state, ["schema", "activation_id", "status", "revision", "created_at", "updated_at", "catalogue_digest", "plan_digest", "profile_digest", "approved_tools", "pending_review_tools", "unclassified_tools", "provider_statuses", "domain_statuses", "registered_tool_count", "last_error", "state_digest", "retention", "authorization", "secret_material"]);
  if (state.schema !== AUTONOMOUS_ACTIVATION_SCHEMA || state.retention !== "metadata_only_no_keys_handles_prompts_tasks_or_payloads" || state.authorization !== "status_only; does_not_grant_provider_or_tool_authority" || state.secret_material !== "never_returned") throw new AutonomousActivationError("activation retention markers are invalid");
  boundedIdentifier("activation_id", state.activation_id, 256);
  if (!AUTONOMOUS_ACTIVATION_STATUSES.includes(state.status)) throw new AutonomousActivationError("activation status is unsupported");
  boundedCount("activation revision", state.revision, 1_000_000);
  if (typeof state.created_at !== "number" || !Number.isFinite(state.created_at) || state.created_at < 0 || typeof state.updated_at !== "number" || !Number.isFinite(state.updated_at) || state.updated_at < state.created_at) throw new AutonomousActivationError("activation timestamps are invalid");
  boundedDigest("activation catalogue_digest", state.catalogue_digest, true);
  boundedDigest("activation plan_digest", state.plan_digest, true);
  boundedDigest("activation profile_digest", state.profile_digest, true);
  const approved = uniqueSorted("activation approved_tools", state.approved_tools, MAX_ACTIVATION_TOOLS);
  const pending = uniqueSorted("activation pending_review_tools", state.pending_review_tools, MAX_ACTIVATION_TOOLS);
  const unclassified = uniqueSorted("activation unclassified_tools", state.unclassified_tools, MAX_ACTIVATION_TOOLS);
  if (!Array.isArray(state.provider_statuses) || state.provider_statuses.length > MAX_ACTIVATION_PROVIDERS) throw new AutonomousActivationError("activation provider statuses exceed their bound");
  const providers = state.provider_statuses.map((row) => providerProjection(row));
  if (new Set(providers.map((row) => row.provider)).size !== providers.length) throw new AutonomousActivationError("activation provider statuses contain duplicates");
  if (!Array.isArray(state.domain_statuses) || state.domain_statuses.length > MAX_ACTIVATION_DOMAINS) throw new AutonomousActivationError("activation domain statuses exceed their bound");
  const domains = state.domain_statuses.map((row) => {
    const domain = String(row.domain);
    if (!ACTIVATION_DOMAINS.has(domain)) throw new AutonomousActivationError(`activation domain is unsupported: ${domain}`);
    return coverageProjection(domain, row, Number(row.proposed_tool_count ?? 0));
  });
  if (new Set(domains.map((row) => row.domain)).size !== domains.length) throw new AutonomousActivationError("activation domain statuses contain duplicates");
  const registered = boundedCount("activation registered_tool_count", state.registered_tool_count, MAX_ACTIVATION_TOOLS);
  const lastError = state.last_error === null ? null : boundedText("activation last_error", state.last_error, MAX_ACTIVATION_ERROR_BYTES);
  const descriptor = statePayload({ ...state, approved_tools: approved, pending_review_tools: pending, unclassified_tools: unclassified, provider_statuses: providers, domain_statuses: domains, registered_tool_count: registered, last_error: lastError });
  assertSafe(descriptor);
  if (jsonBytes(state) > MAX_ACTIVATION_STATE_BYTES) throw new AutonomousActivationError("activation state exceeds its byte bound");
  if (state.state_digest !== digestJsonSync(descriptor)) throw new AutonomousActivationError("activation state digest does not match its contents");
  return clone({ ...state, approved_tools: approved, pending_review_tools: pending, unclassified_tools: unclassified, provider_statuses: providers, domain_statuses: domains, registered_tool_count: registered, last_error: lastError });
}

function makeInitialState(activationId: string, now: number): AutonomousCapabilityActivationState {
  return sealState({ activation_id: activationId, status: "created", revision: 0, created_at: now, updated_at: now, catalogue_digest: null, plan_digest: null, profile_digest: null, approved_tools: [], pending_review_tools: [], unclassified_tools: [], provider_statuses: [], domain_statuses: [], registered_tool_count: 0, last_error: null });
}

/** Thread-safe-by-event-loop state machine for provider onboarding and domain-tool activation. */
export class AutonomousCapabilityActivation {
  private stateValue: AutonomousCapabilityActivationState;
  private readonly clock: () => number;

  constructor(options: { activationId?: string; state?: AutonomousCapabilityActivationState | JsonObject; clock?: () => number } = {}) {
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new AutonomousActivationError("activation clock must be callable");
    if (options.state !== undefined) this.stateValue = validateAutonomousCapabilityActivationState(options.state);
    else this.stateValue = makeInitialState(boundedIdentifier("activation_id", options.activationId ?? `activation-${Math.random().toString(36).slice(2)}`, 256), this.now());
  }

  get state(): AutonomousCapabilityActivationState { return clone(this.stateValue); }
  toJSON(): AutonomousCapabilityActivationState { return this.state; }

  /** Replace local state from a validated durable snapshot without accepting a key or payload. */
  restore(raw: AutonomousCapabilityActivationState): AutonomousCapabilityActivationState {
    const next = validateAutonomousCapabilityActivationState(raw);
    if (this.stateValue.status === "revoked" && next.status !== "revoked") throw new AutonomousActivationError("a revoked activation cannot be restored to an active state");
    if (this.stateValue.revision > 0 && next.activation_id !== this.stateValue.activation_id) throw new AutonomousActivationError("activation identity cannot change after initialization");
    if (next.revision < this.stateValue.revision) throw new AutonomousActivationError("activation revision cannot move backwards");
    this.stateValue = clone(next);
    return this.state;
  }

  recordProviderStatuses(statuses: readonly JsonObject[]): AutonomousCapabilityActivationState {
    if (!Array.isArray(statuses) || statuses.length > MAX_ACTIVATION_PROVIDERS) throw new AutonomousActivationError("provider statuses exceed their bound");
    const projected = statuses.map(providerProjection).sort((left, right) => left.provider.localeCompare(right.provider));
    if (new Set(projected.map((row) => row.provider)).size !== projected.length) throw new AutonomousActivationError("provider statuses contain duplicates");
    this.commit({ provider_statuses: projected, last_error: null });
    return this.state;
  }

  recordBindingPlan(plan: JsonObject): AutonomousCapabilityActivationState {
    if (plan.schema !== AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA) throw new AutonomousActivationError("activation requires a valid domain tool binding plan");
    const catalogueDigest = boundedDigest("binding plan catalogue_digest", plan.catalogue_digest);
    const profileDigest = boundedDigest("binding plan profile_digest", plan.profile_digest);
    const suppliedPlanDigest = boundedDigest("binding plan plan_digest", plan.plan_digest);
    const policyPlanDigest = autonomousBindingPlanDigest(plan);
    const descriptor = { ...plan };
    delete descriptor.plan_digest;
    const descriptorPlanDigest = digestJsonSync(descriptor);
    if (suppliedPlanDigest !== policyPlanDigest && suppliedPlanDigest !== descriptorPlanDigest) throw new AutonomousActivationError("binding plan digest does not match its contents");
    const computedPlanDigest = suppliedPlanDigest;
    const domains = uniqueSorted("binding plan domains", plan.domains ?? [], MAX_ACTIVATION_DOMAINS);
    if (domains.some((domain) => !ACTIVATION_DOMAINS.has(domain))) throw new AutonomousActivationError("binding plan contains an unsupported domain");
    const rawCoverage = Array.isArray(plan.coverage) ? plan.coverage : [];
    const proposed = Array.isArray(plan.proposed_bindings) ? plan.proposed_bindings : [];
    const review = uniqueSorted("binding plan review_required_tools", plan.review_required_tools ?? [], MAX_ACTIVATION_TOOLS);
    const unclassified = uniqueSorted("binding plan unclassified_tools", plan.unclassified_tools ?? [], MAX_ACTIVATION_TOOLS);
    const coverage = domains.map((domain) => {
      const row = rawCoverage.find((candidate) => isObject(candidate) && candidate.domain === domain);
      if (!isObject(row)) throw new AutonomousActivationError(`binding plan coverage is missing for ${domain}`);
      const proposedCount = proposed.filter((binding) => isObject(binding) && Array.isArray(binding.domains) && binding.domains.includes(domain)).length;
      return coverageProjection(domain, row, proposedCount);
    });
    const changed = (this.stateValue.catalogue_digest !== null && this.stateValue.catalogue_digest !== catalogueDigest)
      || (this.stateValue.plan_digest !== null && this.stateValue.plan_digest !== computedPlanDigest)
      || (this.stateValue.profile_digest !== null && this.stateValue.profile_digest !== profileDigest);
    const invalidated = changed && this.stateValue.approved_tools.length > 0;
    this.commit({ status: invalidated ? "stale" : undefined, catalogue_digest: catalogueDigest, plan_digest: computedPlanDigest, profile_digest: profileDigest, approved_tools: changed ? [] : this.stateValue.approved_tools, pending_review_tools: uniqueSorted("activation pending_review_tools", [...review, ...unclassified], MAX_ACTIVATION_TOOLS), unclassified_tools: unclassified, domain_statuses: coverage, registered_tool_count: changed ? 0 : this.stateValue.registered_tool_count, last_error: null });
    if (!invalidated) this.commit({ status: this.derivedStatus(false) });
    return this.state;
  }

  recordRegisteredTools(count: number): AutonomousCapabilityActivationState {
    this.commit({ registered_tool_count: boundedCount("registered tool count", count, MAX_ACTIVATION_TOOLS), status: this.derivedStatus() });
    return this.state;
  }

  approveBindings(plan: JsonObject, approvedTools: readonly string[], registeredToolCount = this.stateValue.registered_tool_count): AutonomousCapabilityActivationState {
    if (!Array.isArray(approvedTools) || approvedTools.length === 0) throw new AutonomousActivationError("approved_tools must be a non-empty array");
    const suppliedPlanDigest = boundedDigest("approval plan plan_digest", plan.plan_digest, true);
    if (suppliedPlanDigest !== this.stateValue.plan_digest && autonomousBindingPlanDigest(plan) !== this.stateValue.plan_digest) throw new AutonomousActivationError("approved binding plan does not match the recorded plan");
    const approved = uniqueSorted("approved_tools", approvedTools, MAX_ACTIVATION_TOOLS);
    const proposed = Array.isArray(plan.proposed_bindings) ? plan.proposed_bindings : [];
    const proposedNames = new Set(proposed.filter((row): row is JsonObject => isObject(row)).map((row) => String(row.name)));
    if (approved.some((name) => !proposedNames.has(name))) throw new AutonomousActivationError("approved tools must be present in proposed_bindings");
    this.commit({ approved_tools: approved, registered_tool_count: boundedCount("registered tool count", registeredToolCount, MAX_ACTIVATION_TOOLS), last_error: null });
    this.commit({ status: this.derivedStatus(false) });
    return this.state;
  }

  revoke(reason = "activation_revoked"): AutonomousCapabilityActivationState {
    if (this.stateValue.status === "revoked") return this.state;
    this.commit({ status: "revoked", approved_tools: [], last_error: boundedText("activation revocation reason", reason, MAX_ACTIVATION_ERROR_BYTES) });
    return this.state;
  }

  private now(): number {
    const value = this.clock();
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0) throw new AutonomousActivationError("activation clock must return a finite non-negative number");
    return value;
  }

  private derivedStatus(preserveStale = true): AutonomousActivationStatus {
    const state = this.stateValue;
    if (state.status === "revoked" || (preserveStale && state.status === "stale")) return state.status;
    if (!state.provider_statuses.length || !state.provider_statuses.some((row) => row.ready)) return "provider_pending";
    if (state.plan_digest === null) return "catalogue_pending";
    if (!state.approved_tools.length) return "review_required";
    if (state.pending_review_tools.length) return "partially_activated";
    return "ready";
  }

  private commit(changes: Partial<Omit<AutonomousCapabilityActivationState, "schema" | "state_digest" | "retention" | "authorization" | "secret_material">> & { status?: AutonomousActivationStatus }): void {
    if (this.stateValue.status === "revoked" && changes.status !== "revoked") throw new AutonomousActivationError("activation is revoked");
    const filtered = Object.fromEntries(Object.entries(changes).filter(([, value]) => value !== undefined));
    if (!Object.entries(filtered).some(([key, value]) => (this.stateValue as unknown as Record<string, unknown>)[key] !== value)) return;
    const nextStatus = (filtered.status as AutonomousActivationStatus | undefined) ?? this.stateValue.status;
    if (!ALLOWED_TRANSITIONS[this.stateValue.status].includes(nextStatus)) throw new AutonomousActivationError(`activation transition ${this.stateValue.status} -> ${nextStatus} is not allowed`);
    const next = { ...this.stateValue, ...filtered, revision: this.stateValue.revision + 1, updated_at: Math.max(this.now(), this.stateValue.updated_at) } as AutonomousCapabilityActivationState;
    this.stateValue = sealState(next);
  }
}

/** In-memory reference implementation; callers can pair it with any durable JSON adapter. */
export class AutonomousCapabilityActivationStore implements AutonomousCapabilityActivationSnapshotStore {
  private value: AutonomousCapabilityActivationState | null = null;

  async load(): Promise<AutonomousCapabilityActivationState | null> { return clone(this.value); }

  async save(raw: AutonomousCapabilityActivationState): Promise<void> {
    const state = validateAutonomousCapabilityActivationState(raw);
    if (this.value && state.state_digest !== this.value.state_digest && state.revision !== this.value.revision + 1) throw new AutonomousActivationError("activation revision continuity check failed");
    this.value = clone(state);
  }

  async saveIfUnchanged(expectedStateDigest: string | null, raw: AutonomousCapabilityActivationState): Promise<boolean> {
    if (expectedStateDigest !== null && !DIGEST.test(expectedStateDigest)) throw new AutonomousActivationError("activation expected state digest is invalid");
    if ((this.value?.state_digest ?? null) !== expectedStateDigest) return false;
    await this.save(raw);
    return true;
  }

  async snapshot(): Promise<AutonomousCapabilityActivationSnapshot> {
    const state = this.value ?? makeInitialState("activation-empty", 0);
    const descriptor = { schema: AUTONOMOUS_ACTIVATION_STORE_SCHEMA, state, state_digest: state.state_digest, retention: "metadata_only_hash_bound" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
  }

  async restore(raw: AutonomousCapabilityActivationSnapshot): Promise<void> {
    const snapshot = validateAutonomousCapabilityActivationSnapshot(raw);
    this.value = clone(snapshot.state);
  }
}

export function validateAutonomousCapabilityActivationSnapshot(value: unknown): AutonomousCapabilityActivationSnapshot {
  if (!isObject(value)) throw new AutonomousActivationError("activation snapshot must be an object");
  const snapshot = value as unknown as AutonomousCapabilityActivationSnapshot;
  knownKeys("activation snapshot", snapshot, ["schema", "state", "state_digest", "snapshot_digest", "retention", "secret_material"]);
  if (snapshot.schema !== AUTONOMOUS_ACTIVATION_STORE_SCHEMA || snapshot.retention !== "metadata_only_hash_bound" || snapshot.secret_material !== "never_returned") throw new AutonomousActivationError("activation snapshot retention markers are invalid");
  const state = validateAutonomousCapabilityActivationState(snapshot.state);
  if (snapshot.state_digest !== state.state_digest || boundedDigest("activation snapshot snapshot_digest", snapshot.snapshot_digest) === null) throw new AutonomousActivationError("activation snapshot state digest is invalid");
  const descriptor = { schema: snapshot.schema, state, state_digest: snapshot.state_digest, retention: snapshot.retention, secret_material: snapshot.secret_material };
  if (snapshot.snapshot_digest !== digestJsonSync(descriptor)) throw new AutonomousActivationError("activation snapshot digest does not match its contents");
  if (jsonBytes(snapshot) > MAX_ACTIVATION_STORE_BYTES) throw new AutonomousActivationError("activation snapshot exceeds its byte bound");
  return clone({ ...snapshot, state });
}

/** Flushes/restores activation metadata through caller-owned durable storage. */
export class AutonomousCapabilityActivationPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousCapabilityActivationSnapshotStore, readonly persistence: AutonomousCapabilityActivationPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new AutonomousActivationError("activation persistence requires a snapshot-capable store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new AutonomousActivationError("activation persistence requires readable and writable storage");
  }

  async flush(): Promise<{ schema: typeof AUTONOMOUS_ACTIVATION_STORE_SCHEMA; bytes: number; state_digest: string; snapshot_digest: string; retention: "metadata_only" }> {
    return this.enqueue(async () => {
      const snapshot = validateAutonomousCapabilityActivationSnapshot(await this.store.snapshot());
      const bytes = jsonBytes(snapshot);
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new AutonomousActivationError("activation persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return { schema: AUTONOMOUS_ACTIVATION_STORE_SCHEMA, bytes, state_digest: snapshot.state_digest, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
    });
  }

  async restore(): Promise<{ schema: typeof AUTONOMOUS_ACTIVATION_STORE_SCHEMA; restored: boolean; state_digest: string | null; snapshot_digest: string | null; retention: "metadata_only" }> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        this.expectedSnapshotDigest = null;
        return { schema: AUTONOMOUS_ACTIVATION_STORE_SCHEMA, restored: false, state_digest: null, snapshot_digest: null, retention: "metadata_only" };
      }
      const snapshot = validateAutonomousCapabilityActivationSnapshot(raw);
      await this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return { schema: AUTONOMOUS_ACTIVATION_STORE_SCHEMA, restored: true, state_digest: snapshot.state_digest, snapshot_digest: snapshot.snapshot_digest, retention: "metadata_only" };
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousCapabilityActivationSnapshotPersistence implements AutonomousCapabilityActivationPersistence {
  constructor(readonly textStore: AutonomousCapabilityActivationSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new AutonomousActivationError("activation text store is malformed");
  }

  async read(): Promise<AutonomousCapabilityActivationSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > MAX_ACTIVATION_STORE_BYTES) throw new AutonomousActivationError("activation JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new AutonomousActivationError("activation JSON is invalid"); }
    if (canonicalJson(parsed) !== encoded) throw new AutonomousActivationError("activation JSON is not canonical");
    return validateAutonomousCapabilityActivationSnapshot(parsed);
  }

  async write(raw: AutonomousCapabilityActivationSnapshot): Promise<void> {
    const snapshot = validateAutonomousCapabilityActivationSnapshot(raw);
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousCapabilityActivationSnapshotPersistence extends JsonAutonomousCapabilityActivationSnapshotPersistence {
  declare readonly textStore: AutonomousCapabilityActivationTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousCapabilityActivationTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new AutonomousActivationError("activation text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, raw: AutonomousCapabilityActivationSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !DIGEST.test(expectedSnapshotDigest)) throw new AutonomousActivationError("activation expected snapshot digest is invalid");
    const snapshot = validateAutonomousCapabilityActivationSnapshot(raw);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}
