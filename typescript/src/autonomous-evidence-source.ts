import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
  type AutonomousEvidenceAcquirer,
  type AutonomousEvidenceAcquisitionContext,
} from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAcquisitionError,
  type AutonomousEvidenceRetryClassifier,
} from "./autonomous-evidence-retry.js";
import {
  AutonomousEvidenceProviderContract,
  AutonomousEvidenceProviderContractRegistry,
  type AutonomousEvidenceProviderFreshnessMode,
} from "./autonomous-evidence-provider-contract.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Strict source-truth metadata that can be retained without retaining source values. */
export const AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA = "bioprism-typescript-autonomous-evidence-source/0.1" as const;
export const AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA = "bioprism-typescript-autonomous-evidence-source-ledger-entry/0.1" as const;
export const AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA = "bioprism-typescript-autonomous-evidence-source-ledger/0.1" as const;
export const AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA = "bioprism-typescript-autonomous-evidence-source-policy/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES = 512;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS = 4_096;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES = 64_000_000;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES = 1_000_000;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_AGE_MS = 31_536_000_000;
export const MAX_AUTONOMOUS_EVIDENCE_SOURCE_FUTURE_SKEW_MS = 86_400_000;
export const DEFAULT_AUTONOMOUS_REALTIME_SOURCE_AGE_MS = 300_000;

const RETENTION = "metadata_only;raw_source_values_and_locators_caller_owned" as const;
const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

export type AutonomousEvidenceSourceAuthority = "caller_declared" | "provider_observed" | "human_verified" | "derived";
export type AutonomousEvidenceSourceStatus = "observed" | "partial" | "unavailable" | "refused" | "stale";
export type AutonomousEvidenceSourceDecision = "accepted" | "partial" | "stale" | "unverified" | "refused";

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(text)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return text;
}

function digest(name: string, value: unknown, required = true): string | null {
  if (value === undefined || value === null) {
    if (required) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    return null;
  }
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value;
}

function list(name: string, value: readonly string[] | undefined): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS) throw new ArgumentError(`${name} must contain at most ${MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS} entries`);
  const normalized = value.map((item, index) => boundedText(`${name}[${index}]`, item, 512));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return [...normalized].sort();
}

function safeFieldMarker(value: string): string {
  return [...value.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
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
      const marker = safeFieldMarker(key);
      if (SECRET_FIELD_MARKERS.has(marker) || marker.includes("token") || marker.includes("secret") || marker.includes("credential")) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertSafeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function jsonBytes(value: JsonValue, name: string): number {
  assertSafeMetadata(value, name);
  const encoded = canonicalJson(value);
  const size = bytes(encoded);
  if (size > MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES) throw new ArgumentError(`${name} exceeds ${MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES} bytes`);
  return size;
}

function sourceRequestDigest(context: AutonomousEvidenceAcquisitionContext): string {
  return digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    plan_digest: context.plan_digest,
    requirement_id: context.request.requirement_id,
    source_id: context.request.source_id,
    source_digest: context.request.source_digest ?? null,
    request_id: context.request.request_id ?? null,
    metadata: context.request.metadata ?? {},
  });
}

function freshness(value: unknown): AutonomousEvidenceProviderFreshnessMode {
  if (!["realtime", "bounded_cache", "historical", "caller_declared"].includes(String(value))) throw new ArgumentError("source freshness mode is invalid");
  return value as AutonomousEvidenceProviderFreshnessMode;
}

function authority(value: unknown): AutonomousEvidenceSourceAuthority {
  if (!["caller_declared", "provider_observed", "human_verified", "derived"].includes(String(value))) throw new ArgumentError("source authority is invalid");
  return value as AutonomousEvidenceSourceAuthority;
}

function sourceStatus(value: unknown): AutonomousEvidenceSourceStatus {
  if (!["observed", "partial", "unavailable", "refused", "stale"].includes(String(value))) throw new ArgumentError("source status is invalid");
  return value as AutonomousEvidenceSourceStatus;
}

export interface AutonomousEvidenceSourceDescriptorInput {
  sourceId?: string;
  sourceDigest?: string | null;
  authority: AutonomousEvidenceSourceAuthority;
  status: AutonomousEvidenceSourceStatus;
  observedAtMs: number;
  expiresAtMs?: number | null;
  citationDigest?: string | null;
  limitations?: readonly string[];
}

export interface AutonomousEvidenceSourceDescriptorContext extends JsonObject {
  context: AutonomousEvidenceAcquisitionContext;
  value_digest: string;
  value_bytes: number;
  contract_digest: string;
  provider: string;
  protocol: string;
  source_kind: string;
  now_ms: number;
}

export interface AutonomousEvidenceSourceReceiptJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA;
  request_digest: string;
  plan_digest: string;
  requirement_id: string;
  domain: AutonomousDomainName;
  source_id: string;
  source_digest: string | null;
  value_digest: string;
  value_bytes: number;
  provider: string;
  protocol: string;
  adapter_id: string;
  contract_digest: string;
  policy_digest: string;
  source_kind: string;
  freshness: AutonomousEvidenceProviderFreshnessMode;
  authority: AutonomousEvidenceSourceAuthority;
  status: AutonomousEvidenceSourceStatus;
  observed_at_ms: number;
  expires_at_ms: number | null;
  citation_digest: string | null;
  decision: AutonomousEvidenceSourceDecision;
  decision_reasons: string[];
  limitations: string[];
  receipt_digest: string;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceSourceLedgerEntryJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA;
  sequence: number;
  previous_entry_digest: string | null;
  receipt: AutonomousEvidenceSourceReceiptJSON;
  entry_digest: string;
  retention: "metadata_only;raw_source_values_excluded";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceSourceLedgerJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA;
  entries: AutonomousEvidenceSourceLedgerEntryJSON[];
  head_digest: string | null;
  ledger_digest: string;
  execution: "metadata_only_source_observation_ledger";
  retention: "metadata_only;raw_source_values_excluded";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceSourceLedgerPersistence {
  append(entry: AutonomousEvidenceSourceLedgerEntryJSON): AutonomousEvidenceSourceLedgerEntryJSON | Promise<AutonomousEvidenceSourceLedgerEntryJSON>;
  records(): readonly AutonomousEvidenceSourceLedgerEntryJSON[] | Promise<readonly AutonomousEvidenceSourceLedgerEntryJSON[]>;
}

export interface AutonomousEvidenceSourceLedgerTextStore {
  read(): string | null | Promise<string | null>;
  write(value: string): void | Promise<void>;
}

export interface AutonomousEvidenceSourceLedgerTransactionalTextStore extends AutonomousEvidenceSourceLedgerTextStore {
  writeIfUnchanged(expectedLedgerDigest: string | null, value: string): boolean | Promise<boolean>;
}

export interface AutonomousEvidenceSourcePolicyJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA;
  max_age_ms: number | null;
  max_future_skew_ms: number;
  allow_partial: boolean;
  allow_unverified: boolean;
  require_source_digest: boolean;
  execution: "freshness_and_authority_gate;no_source_dispatch";
  retention: "metadata_only_policy";
  secret_material: "never_returned";
  policy_digest: string;
}

export interface AutonomousEvidenceSourcePolicyDecision extends JsonObject {
  decision: AutonomousEvidenceSourceDecision;
  usable: boolean;
  reasons: string[];
}

export interface AutonomousEvidenceSourcePolicyOptions {
  maxAgeMs?: number | null;
  maxFutureSkewMs?: number;
  allowPartial?: boolean;
  allowUnverified?: boolean;
  requireSourceDigest?: boolean;
  now?: () => number;
}

function receiptDescriptor(receipt: Omit<AutonomousEvidenceSourceReceiptJSON, "receipt_digest">): JsonObject {
  return { ...receipt };
}

function makeReceipt(input: Omit<AutonomousEvidenceSourceReceiptJSON, "receipt_digest">): AutonomousEvidenceSourceReceiptJSON {
  return { ...input, receipt_digest: digestJsonSync(receiptDescriptor(input)) } as AutonomousEvidenceSourceReceiptJSON;
}

function entryDescriptor(entry: Omit<AutonomousEvidenceSourceLedgerEntryJSON, "entry_digest">): JsonObject {
  return { ...entry };
}

function validateReceipt(value: unknown): AutonomousEvidenceSourceReceiptJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA) throw new ArgumentError("source ledger receipt is malformed");
  const receipt = value as unknown as AutonomousEvidenceSourceReceiptJSON;
  digest("source ledger request_digest", receipt.request_digest);
  digest("source ledger plan_digest", receipt.plan_digest);
  digest("source ledger value_digest", receipt.value_digest);
  digest("source ledger contract_digest", receipt.contract_digest);
  digest("source ledger policy_digest", receipt.policy_digest);
  digest("source ledger receipt_digest", receipt.receipt_digest);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(receipt.domain)) throw new ArgumentError("source ledger receipt domain is unsupported");
  identifier("source ledger receipt requirement_id", receipt.requirement_id);
  identifier("source ledger receipt source_id", receipt.source_id, MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES);
  digest("source ledger receipt source_digest", receipt.source_digest, false);
  digest("source ledger receipt citation_digest", receipt.citation_digest, false);
  identifier("source ledger receipt provider", receipt.provider);
  identifier("source ledger receipt protocol", receipt.protocol);
  identifier("source ledger receipt adapter_id", receipt.adapter_id);
  identifier("source ledger receipt source_kind", receipt.source_kind);
  freshness(receipt.freshness);
  authority(receipt.authority);
  sourceStatus(receipt.status);
  integer("source ledger receipt value_bytes", receipt.value_bytes, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES);
  integer("source ledger receipt observed_at_ms", receipt.observed_at_ms, 0, Number.MAX_SAFE_INTEGER);
  if (receipt.expires_at_ms !== null) integer("source ledger receipt expires_at_ms", receipt.expires_at_ms, 0, Number.MAX_SAFE_INTEGER);
  if (!Array.isArray(receipt.decision_reasons) || !Array.isArray(receipt.limitations)) throw new ArgumentError("source ledger receipt arrays are malformed");
  list("source ledger receipt decision_reasons", receipt.decision_reasons);
  list("source ledger receipt limitations", receipt.limitations);
  if (receipt.retention !== RETENTION || receipt.secret_material !== "never_returned") throw new ArgumentError("source ledger receipt retention is invalid");
  const { receipt_digest: _receiptDigest, ...descriptor } = receipt;
  if (digestJsonSync(descriptor) !== receipt.receipt_digest) throw new ArgumentError("source ledger receipt digest is invalid");
  return structuredClone(receipt);
}

function validateEntry(value: unknown): AutonomousEvidenceSourceLedgerEntryJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA) throw new ArgumentError("source ledger entry is malformed");
  const entry = value as unknown as AutonomousEvidenceSourceLedgerEntryJSON;
  integer("source ledger entry sequence", entry.sequence, 1, MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS);
  digest("source ledger entry entry_digest", entry.entry_digest);
  digest("source ledger entry previous_entry_digest", entry.previous_entry_digest, false);
  const receipt = validateReceipt(entry.receipt);
  if (entry.retention !== "metadata_only;raw_source_values_excluded" || entry.secret_material !== "never_returned") throw new ArgumentError("source ledger entry retention is invalid");
  const { entry_digest: _entryDigest, ...descriptor } = entry;
  if (digestJsonSync(descriptor) !== entry.entry_digest) throw new ArgumentError("source ledger entry digest is invalid");
  return { ...structuredClone(entry), receipt };
}

function validateChain(entries: readonly AutonomousEvidenceSourceLedgerEntryJSON[]): void {
  let previous: string | null = null;
  entries.forEach((entry, index) => {
    if (entry.sequence !== index + 1) throw new ArgumentError("source ledger sequence is not contiguous");
    if (entry.previous_entry_digest !== previous) throw new ArgumentError("source ledger hash chain is invalid");
    previous = entry.entry_digest;
  });
}

function ledgerBody(entries: readonly AutonomousEvidenceSourceLedgerEntryJSON[]): Omit<AutonomousEvidenceSourceLedgerJSON, "ledger_digest"> {
  const normalized = entries.map((entry) => structuredClone(entry));
  validateChain(normalized);
  const body = {
    schema: AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA,
    entries: normalized,
    head_digest: normalized.at(-1)?.entry_digest ?? null,
    execution: "metadata_only_source_observation_ledger" as const,
    retention: "metadata_only;raw_source_values_excluded" as const,
    secret_material: "never_returned" as const,
  };
  if (bytes(canonicalJson(body)) > MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) throw new ArgumentError("source ledger exceeds its metadata byte bound");
  return body;
}

function ledgerSnapshot(entries: readonly AutonomousEvidenceSourceLedgerEntryJSON[]): AutonomousEvidenceSourceLedgerJSON {
  const body = ledgerBody(entries);
  return { ...body, ledger_digest: digestJsonSync(body) } as AutonomousEvidenceSourceLedgerJSON;
}

function validateLedgerSnapshot(value: unknown): AutonomousEvidenceSourceLedgerJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA || !Array.isArray(value.entries)) throw new ArgumentError("source ledger snapshot is malformed");
  const snapshot = value as unknown as AutonomousEvidenceSourceLedgerJSON;
  if (snapshot.execution !== "metadata_only_source_observation_ledger" || snapshot.retention !== "metadata_only;raw_source_values_excluded" || snapshot.secret_material !== "never_returned") throw new ArgumentError("source ledger snapshot retention is invalid");
  digest("source ledger snapshot ledger_digest", snapshot.ledger_digest);
  if (snapshot.entries.length > MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS) throw new ArgumentError("source ledger snapshot exceeds its record bound");
  const entries = snapshot.entries.map(validateEntry);
  const body = ledgerBody(entries);
  if (body.head_digest !== snapshot.head_digest || digestJsonSync(body) !== snapshot.ledger_digest) throw new ArgumentError("source ledger snapshot digest or head is invalid");
  return structuredClone({ ...body, ledger_digest: snapshot.ledger_digest }) as AutonomousEvidenceSourceLedgerJSON;
}

/** Explicit freshness, authority, and source-integrity policy. It never contacts a source. */
export class AutonomousEvidenceSourcePolicy {
  readonly max_age_ms: number | null;
  readonly max_future_skew_ms: number;
  readonly allow_partial: boolean;
  readonly allow_unverified: boolean;
  readonly require_source_digest: boolean;
  readonly policy_digest: string;
  private readonly clock: () => number;

  constructor(options: AutonomousEvidenceSourcePolicyOptions = {}) {
    if (!options || typeof options !== "object") throw new ArgumentError("source policy options are malformed");
    this.max_age_ms = options.maxAgeMs === undefined || options.maxAgeMs === null ? null : integer("source policy maxAgeMs", options.maxAgeMs, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_AGE_MS);
    this.max_future_skew_ms = integer("source policy maxFutureSkewMs", options.maxFutureSkewMs ?? 60_000, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_FUTURE_SKEW_MS);
    this.allow_partial = options.allowPartial ?? false;
    this.allow_unverified = options.allowUnverified ?? false;
    this.require_source_digest = options.requireSourceDigest ?? true;
    if (typeof this.allow_partial !== "boolean" || typeof this.allow_unverified !== "boolean" || typeof this.require_source_digest !== "boolean") throw new ArgumentError("source policy flags are malformed");
    this.clock = options.now ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new ArgumentError("source policy clock is malformed");
    this.policy_digest = digestJsonSync(this.policyDescriptor());
  }

  now(): number {
    return integer("source policy clock value", this.clock(), 0, Number.MAX_SAFE_INTEGER);
  }

  toJSON(): AutonomousEvidenceSourcePolicyJSON {
    return { ...this.policyDescriptor(), policy_digest: this.policy_digest } as AutonomousEvidenceSourcePolicyJSON;
  }

  evaluate(contract: AutonomousEvidenceProviderContract, input: AutonomousEvidenceSourceDescriptorInput, nowMs = this.now()): AutonomousEvidenceSourcePolicyDecision {
    if (!(contract instanceof AutonomousEvidenceProviderContract)) throw new ArgumentError("source policy requires a typed provider contract");
    const descriptor = normalizeSourceDescriptor(input);
    const reasons: string[] = [];
    let decision: AutonomousEvidenceSourceDecision = "accepted";
    const priority: Record<AutonomousEvidenceSourceDecision, number> = { accepted: 0, partial: 1, unverified: 2, stale: 3, refused: 4 };
    const applyDecision = (candidate: AutonomousEvidenceSourceDecision): void => {
      if (priority[candidate] > priority[decision]) decision = candidate;
    };
    const timestamp = integer("source policy evaluation nowMs", nowMs, 0, Number.MAX_SAFE_INTEGER);
    if (descriptor.observed_at_ms > timestamp + this.max_future_skew_ms) return { decision: "refused", usable: false, reasons: ["observed_at_is_in_the_future"] };
    if (descriptor.expires_at_ms !== null && descriptor.expires_at_ms < descriptor.observed_at_ms) return { decision: "refused", usable: false, reasons: ["expiry_precedes_observation"] };
    if (descriptor.status === "unavailable" || descriptor.status === "refused") return { decision: "refused", usable: false, reasons: [`source_status_${descriptor.status}`] };
    if (descriptor.status === "stale") applyDecision("stale");
    if (descriptor.status === "partial") {
      reasons.push("source_status_partial");
      if (this.allow_partial) applyDecision("partial");
      else applyDecision("refused");
    }
    if (this.require_source_digest && descriptor.source_digest === null) {
      reasons.push("source_digest_missing");
      applyDecision("unverified");
    }
    if (descriptor.authority === "caller_declared") {
      reasons.push("authority_caller_declared");
      applyDecision("unverified");
    }
    const ageLimit = this.max_age_ms ?? (contract.freshness === "realtime" ? DEFAULT_AUTONOMOUS_REALTIME_SOURCE_AGE_MS : null);
    if (ageLimit !== null && timestamp >= descriptor.observed_at_ms && timestamp - descriptor.observed_at_ms > ageLimit) {
      reasons.push("source_observation_exceeds_max_age");
      applyDecision("stale");
    }
    if (contract.freshness === "bounded_cache" && (descriptor.expires_at_ms === null || timestamp > descriptor.expires_at_ms)) {
      reasons.push(descriptor.expires_at_ms === null ? "bounded_cache_expiry_missing" : "bounded_cache_expired");
      applyDecision("stale");
    }
    if (contract.freshness === "caller_declared" && descriptor.authority !== "caller_declared") {
      reasons.push("caller_declared_contract_requires_explicit_authority");
      applyDecision("unverified");
    }
    const finalDecision = decision as AutonomousEvidenceSourceDecision;
    if (finalDecision === "unverified" && this.allow_unverified) return { decision: finalDecision, usable: true, reasons: [...new Set(reasons)].sort() };
    const usable = finalDecision === "accepted" || (finalDecision === "partial" && this.allow_partial);
    return { decision: finalDecision, usable, reasons: [...new Set(reasons)].sort() };
  }

  private policyDescriptor(): Omit<AutonomousEvidenceSourcePolicyJSON, "policy_digest"> {
    return {
      schema: AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA,
      max_age_ms: this.max_age_ms,
      max_future_skew_ms: this.max_future_skew_ms,
      allow_partial: this.allow_partial,
      allow_unverified: this.allow_unverified,
      require_source_digest: this.require_source_digest,
      execution: "freshness_and_authority_gate;no_source_dispatch",
      retention: "metadata_only_policy",
      secret_material: "never_returned",
    };
  }
}

function normalizeSourceDescriptor(input: AutonomousEvidenceSourceDescriptorInput): {
  source_id: string;
  source_digest: string | null;
  authority: AutonomousEvidenceSourceAuthority;
  status: AutonomousEvidenceSourceStatus;
  observed_at_ms: number;
  expires_at_ms: number | null;
  citation_digest: string | null;
  limitations: string[];
} {
  if (!input || typeof input !== "object") throw new ArgumentError("source descriptor is malformed");
  const sourceId = identifier("source descriptor sourceId", input.sourceId, MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES);
  const observedAtMs = integer("source descriptor observedAtMs", input.observedAtMs, 0, Number.MAX_SAFE_INTEGER);
  const expiresAtMs = input.expiresAtMs === undefined || input.expiresAtMs === null ? null : integer("source descriptor expiresAtMs", input.expiresAtMs, 0, Number.MAX_SAFE_INTEGER);
  if (expiresAtMs !== null && expiresAtMs < observedAtMs) throw new ArgumentError("source descriptor expiry precedes observation");
  return {
    source_id: sourceId,
    source_digest: digest("source descriptor sourceDigest", input.sourceDigest, false),
    authority: authority(input.authority),
    status: sourceStatus(input.status),
    observed_at_ms: observedAtMs,
    expires_at_ms: expiresAtMs,
    citation_digest: digest("source descriptor citationDigest", input.citationDigest, false),
    limitations: list("source descriptor limitations", input.limitations ?? []),
  };
}

/** In-memory or caller-persisted hash-chained metadata ledger for source observations. */
export class AutonomousEvidenceSourceLedger {
  private readonly entriesByRequest = new Map<string, AutonomousEvidenceSourceLedgerEntryJSON>();

  constructor(private readonly persistence?: AutonomousEvidenceSourceLedgerPersistence) {}

  async append(receiptInput: Omit<AutonomousEvidenceSourceReceiptJSON, "receipt_digest"> | AutonomousEvidenceSourceReceiptJSON): Promise<AutonomousEvidenceSourceLedgerEntryJSON> {
    const { receipt_digest: _receiptDigest, ...descriptor } = receiptInput as AutonomousEvidenceSourceReceiptJSON;
    const receipt = makeReceipt(descriptor as Omit<AutonomousEvidenceSourceReceiptJSON, "receipt_digest">);
    const existing = this.entriesByRequest.get(receipt.request_digest);
    if (existing) {
      if (existing.receipt.receipt_digest !== receipt.receipt_digest) throw new ArgumentError("source ledger request already has a conflicting receipt");
      return structuredClone(existing);
    }
    if (this.entriesByRequest.size >= MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS) throw new ArgumentError("source ledger is full");
    const previous = this.records().at(-1);
    const base = {
      schema: AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA,
      sequence: (previous?.sequence ?? 0) + 1,
      previous_entry_digest: previous?.entry_digest ?? null,
      receipt,
      retention: "metadata_only;raw_source_values_excluded" as const,
      secret_material: "never_returned" as const,
    };
    const entry = { ...base, entry_digest: digestJsonSync(entryDescriptor(base)) };
    const persisted = this.persistence ? await this.persistence.append(structuredClone(entry)) : entry;
    const validated = validateEntry(persisted);
    if (validated.sequence !== entry.sequence || validated.previous_entry_digest !== entry.previous_entry_digest || validated.entry_digest !== entry.entry_digest) throw new ArgumentError("source ledger persistence changed the appended entry");
    this.entriesByRequest.set(receipt.request_digest, validated);
    return structuredClone(validated);
  }

  records(): AutonomousEvidenceSourceLedgerEntryJSON[] {
    return [...this.entriesByRequest.values()].sort((left, right) => left.sequence - right.sequence).map((entry) => structuredClone(entry));
  }

  get(requestDigest: string): AutonomousEvidenceSourceLedgerEntryJSON | null {
    const key = digest("source ledger request digest", requestDigest);
    if (key === null) throw new ArgumentError("source ledger request digest is required");
    const entry = this.entriesByRequest.get(key);
    return entry ? structuredClone(entry) : null;
  }

  async restore(): Promise<{ restored: number; head_digest: string | null }> {
    if (!this.persistence) return { restored: 0, head_digest: this.records().at(-1)?.entry_digest ?? null };
    const raw = await this.persistence.records();
    if (!Array.isArray(raw) || raw.length > MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS) throw new ArgumentError("source ledger persistence returned too many entries");
    const entries = raw.map(validateEntry).sort((left, right) => left.sequence - right.sequence);
    validateChain(entries);
    this.entriesByRequest.clear();
    for (const entry of entries) {
      if (this.entriesByRequest.has(entry.receipt.request_digest)) throw new ArgumentError("source ledger persistence contains duplicate request digests");
      this.entriesByRequest.set(entry.receipt.request_digest, entry);
    }
    return { restored: entries.length, head_digest: entries.at(-1)?.entry_digest ?? null };
  }

  toJSON(): AutonomousEvidenceSourceLedgerJSON {
    return ledgerSnapshot(this.records());
  }
}

export class InMemoryAutonomousEvidenceSourceLedgerPersistence implements AutonomousEvidenceSourceLedgerPersistence {
  private entries: AutonomousEvidenceSourceLedgerEntryJSON[] = [];

  append(entry: AutonomousEvidenceSourceLedgerEntryJSON): AutonomousEvidenceSourceLedgerEntryJSON {
    const validated = validateEntry(entry);
    const existing = this.entries.find((candidate) => candidate.sequence === validated.sequence);
    if (existing && existing.entry_digest !== validated.entry_digest) throw new ArgumentError("in-memory source ledger persistence has a conflicting sequence");
    if (!existing) this.entries.push(structuredClone(validated));
    return structuredClone(validated);
  }

  records(): readonly AutonomousEvidenceSourceLedgerEntryJSON[] {
    return this.entries.map((entry) => structuredClone(entry));
  }
}

/** Portable JSON persistence for files, databases, object stores, and browser bridges. */
export class JsonAutonomousEvidenceSourceLedgerPersistence implements AutonomousEvidenceSourceLedgerPersistence {
  protected readonly textStore: AutonomousEvidenceSourceLedgerTextStore;
  readonly maxBytes: number;

  constructor(textStore: AutonomousEvidenceSourceLedgerTextStore, maxBytes = MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("source ledger JSON persistence requires a text store");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) throw new ArgumentError("source ledger JSON persistence maxBytes is outside its bound");
    this.textStore = textStore;
    this.maxBytes = maxBytes;
  }

  protected async readSnapshot(): Promise<AutonomousEvidenceSourceLedgerJSON | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > this.maxBytes) throw new ArgumentError("source ledger JSON persistence text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("source ledger JSON persistence text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("source ledger JSON persistence text is not canonical");
    const snapshot = validateLedgerSnapshot(parsed);
    if (bytes(canonicalJson(snapshot)) > this.maxBytes) throw new ArgumentError("source ledger JSON persistence snapshot exceeds its bound");
    return snapshot;
  }

  protected encode(snapshot: AutonomousEvidenceSourceLedgerJSON): string {
    const validated = validateLedgerSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > this.maxBytes) throw new ArgumentError("source ledger JSON persistence snapshot exceeds its bound");
    return encoded;
  }

  protected async persist(expectedLedgerDigest: string | null, snapshot: AutonomousEvidenceSourceLedgerJSON): Promise<void> {
    void expectedLedgerDigest;
    await this.textStore.write(this.encode(snapshot));
  }

  async records(): Promise<readonly AutonomousEvidenceSourceLedgerEntryJSON[]> {
    const snapshot = await this.readSnapshot();
    return snapshot ? snapshot.entries.map((entry) => structuredClone(entry)) : [];
  }

  async append(entry: AutonomousEvidenceSourceLedgerEntryJSON): Promise<AutonomousEvidenceSourceLedgerEntryJSON> {
    const validated = validateEntry(entry);
    const current = await this.readSnapshot();
    const entries = current?.entries ?? [];
    const existing = entries.find((candidate) => candidate.sequence === validated.sequence);
    if (existing) {
      if (existing.entry_digest !== validated.entry_digest) throw new ArgumentError("source ledger persistence has a conflicting sequence");
      return structuredClone(existing);
    }
    if (validated.sequence !== entries.length + 1 || validated.previous_entry_digest !== (current?.head_digest ?? null)) throw new ArgumentError("source ledger persistence append is stale or out of order");
    const next = ledgerSnapshot([...entries, validated]);
    await this.persist(current?.ledger_digest ?? null, next);
    return structuredClone(validated);
  }
}

/** JSON source-ledger persistence with an atomic compare-and-swap writer fence. */
export class TransactionalJsonAutonomousEvidenceSourceLedgerPersistence extends JsonAutonomousEvidenceSourceLedgerPersistence {
  private readonly transactionalStore: AutonomousEvidenceSourceLedgerTransactionalTextStore;

  constructor(transactionalStore: AutonomousEvidenceSourceLedgerTransactionalTextStore, maxBytes = MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) {
    super(transactionalStore, maxBytes);
    if (typeof transactionalStore.writeIfUnchanged !== "function") throw new ArgumentError("transactional source ledger persistence requires writeIfUnchanged");
    this.transactionalStore = transactionalStore;
  }

  protected override async persist(expectedLedgerDigest: string | null, snapshot: AutonomousEvidenceSourceLedgerJSON): Promise<void> {
    const committed = await this.transactionalStore.writeIfUnchanged(expectedLedgerDigest, this.encode(snapshot));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional source ledger persistence returned a non-boolean commit result");
    if (!committed) throw new ArgumentError("source ledger persistence rejected a stale writer");
  }
}

/** Browser Web Storage adapter for source-ledger JSON snapshots. */
export class AutonomousEvidenceSourceLedgerWebStorage implements AutonomousEvidenceSourceLedgerTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string, readonly maxBytes = MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("source ledger web storage adapter is malformed");
    if (!key.trim() || key.length > 256) throw new ArgumentError("source ledger web storage key is invalid");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) throw new ArgumentError("source ledger web storage maxBytes is outside its bound");
  }

  read(): string | null {
    const value = this.storage.getItem(this.key);
    if (value !== null && bytes(value) > this.maxBytes) throw new ArgumentError("source ledger web storage value exceeds its bound");
    return value;
  }

  write(value: string): void {
    if (typeof value !== "string" || bytes(value) > this.maxBytes) throw new ArgumentError("source ledger web storage value exceeds its bound");
    this.storage.setItem(this.key, value);
  }
}

export interface AutonomousEvidenceSourceAcquirerOptions {
  providerContracts: AutonomousEvidenceProviderContractRegistry;
  adapterId: string;
  domain: AutonomousDomainName;
  sourceKind?: string;
  policy?: AutonomousEvidenceSourcePolicy;
  ledger?: AutonomousEvidenceSourceLedger;
  describeSource: (input: AutonomousEvidenceSourceDescriptorContext) => AutonomousEvidenceSourceDescriptorInput | Promise<AutonomousEvidenceSourceDescriptorInput>;
}

export interface AutonomousEvidenceSourceGuardOptions {
  contract: AutonomousEvidenceProviderContract;
  adapterId: string;
  domain: AutonomousDomainName;
  sourceKind: string;
  policy: AutonomousEvidenceSourcePolicy;
  ledger?: AutonomousEvidenceSourceLedger;
  describeSource: (input: AutonomousEvidenceSourceDescriptorContext) => AutonomousEvidenceSourceDescriptorInput | Promise<AutonomousEvidenceSourceDescriptorInput>;
}

function failureForDecision(decision: AutonomousEvidenceSourceDecision): { failureClass: string; retryable: boolean } {
  if (decision === "stale") return { failureClass: "source_stale", retryable: false };
  if (decision === "unverified") return { failureClass: "source_unverified", retryable: false };
  if (decision === "partial") return { failureClass: "source_partial", retryable: false };
  return { failureClass: "source_refused", retryable: false };
}

/**
 * Wrap a reviewed provider contract with source identity, provenance, freshness, and ledger
 * enforcement. The returned acquirer exposes only the raw value for the existing projector;
 * the corresponding source receipt is retained in the caller-owned metadata ledger.
 */
export function createAutonomousEvidenceSourceAcquirer(options: AutonomousEvidenceSourceAcquirerOptions): AutonomousEvidenceAcquirer {
  if (!options || typeof options !== "object") throw new ArgumentError("source acquirer options are malformed");
  if (!(options.providerContracts instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("source acquirer requires a typed provider contract registry");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(options.domain)) throw new ArgumentError("source acquirer domain is unsupported");
  if (typeof options.describeSource !== "function") throw new ArgumentError("source acquirer requires an explicit source descriptor callback");
  if (options.policy !== undefined && !(options.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("source acquirer policy is malformed");
  if (options.ledger !== undefined && !(options.ledger instanceof AutonomousEvidenceSourceLedger)) throw new ArgumentError("source acquirer ledger is malformed");
  const normalizedAdapterId = identifier("source acquirer adapterId", options.adapterId);
  const contract = options.providerContracts.contractForAdapter(normalizedAdapterId, options.domain);
  const sourceKind = options.sourceKind === undefined
    ? contract.source_kinds.length === 1 ? contract.source_kinds[0]! : (() => { throw new ArgumentError("source acquirer requires sourceKind when a contract declares multiple source kinds"); })()
    : identifier("source acquirer sourceKind", options.sourceKind);
  if (!contract.source_kinds.includes(sourceKind)) throw new ArgumentError(`source kind ${sourceKind} is not declared by ${contract.contract_id}`);
  const policy = options.policy ?? new AutonomousEvidenceSourcePolicy();
  const base = options.providerContracts.createAcquirerForAdapter(normalizedAdapterId, options.domain);
  return createAutonomousEvidenceSourceGuard(base, {
    contract,
    adapterId: normalizedAdapterId,
    domain: options.domain,
    sourceKind,
    policy,
    ...(options.ledger === undefined ? {} : { ledger: options.ledger }),
    describeSource: options.describeSource,
  });
}

/** Wrap an already selected or failover-scoped acquirer with the same source admission gate. */
export function createAutonomousEvidenceSourceGuard(base: AutonomousEvidenceAcquirer, options: AutonomousEvidenceSourceGuardOptions): AutonomousEvidenceAcquirer {
  if (!base || typeof base.acquire !== "function") throw new ArgumentError("source guard requires a typed acquirer");
  if (!(options.contract instanceof AutonomousEvidenceProviderContract)) throw new ArgumentError("source guard requires a typed provider contract");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(options.domain) || !options.contract.domains.includes(options.domain)) throw new ArgumentError("source guard contract does not cover its domain");
  if (!(options.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("source guard policy is malformed");
  if (options.ledger !== undefined && !(options.ledger instanceof AutonomousEvidenceSourceLedger)) throw new ArgumentError("source guard ledger is malformed");
  if (typeof options.describeSource !== "function") throw new ArgumentError("source guard requires an explicit source descriptor callback");
  const normalizedAdapterId = identifier("source guard adapterId", options.adapterId);
  const sourceKind = identifier("source guard sourceKind", options.sourceKind);
  if (!options.contract.source_kinds.includes(sourceKind)) throw new ArgumentError(`source kind ${sourceKind} is not declared by ${options.contract.contract_id}`);
  return {
    acquire: async (context) => {
      if (context.requirement.domain !== options.domain) throw new ArgumentError("source acquirer received a different domain");
      const liveContract = options.contract;
      const value = await base.acquire(context);
      const valueDigest = digestJsonSync(value);
      const valueBytes = jsonBytes(value, "source acquirer value");
      const nowMs = options.policy.now();
      const described = await options.describeSource({
        context,
        value_digest: valueDigest,
        value_bytes: valueBytes,
        contract_digest: liveContract.contract_digest,
        provider: liveContract.provider,
        protocol: liveContract.protocol,
        source_kind: sourceKind,
        now_ms: nowMs,
      });
      const descriptor = normalizeSourceDescriptor({ ...described, sourceId: described.sourceId ?? context.request.source_id });
      if (descriptor.source_id !== context.request.source_id) throw new ArgumentError("source descriptor source_id does not match the acquisition request");
      if (context.request.source_digest !== undefined && context.request.source_digest !== null && descriptor.source_digest !== context.request.source_digest) throw new ArgumentError("source descriptor source_digest does not match the acquisition request");
      const decision = options.policy.evaluate(liveContract, {
        sourceId: descriptor.source_id,
        sourceDigest: descriptor.source_digest,
        authority: descriptor.authority,
        status: descriptor.status,
        observedAtMs: descriptor.observed_at_ms,
        expiresAtMs: descriptor.expires_at_ms,
        citationDigest: descriptor.citation_digest,
        limitations: descriptor.limitations,
      }, nowMs);
      const requestDigest = sourceRequestDigest(context);
      const receipt = makeReceipt({
        schema: AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA,
        request_digest: requestDigest,
        plan_digest: context.plan_digest,
        requirement_id: context.requirement.requirement_id,
        domain: options.domain,
        source_id: descriptor.source_id,
        source_digest: descriptor.source_digest,
        value_digest: valueDigest,
        value_bytes: valueBytes,
        provider: liveContract.provider,
        protocol: liveContract.protocol,
        adapter_id: normalizedAdapterId,
        contract_digest: liveContract.contract_digest,
        policy_digest: options.policy.policy_digest,
        source_kind: sourceKind,
        freshness: liveContract.freshness,
        authority: descriptor.authority,
        status: descriptor.status,
        observed_at_ms: descriptor.observed_at_ms,
        expires_at_ms: descriptor.expires_at_ms,
        citation_digest: descriptor.citation_digest,
        decision: decision.decision,
        decision_reasons: decision.reasons,
        limitations: descriptor.limitations,
        retention: RETENTION,
        secret_material: "never_returned",
      });
      if (options.ledger) await options.ledger.append(receipt);
      if (!decision.usable) {
        const failure = failureForDecision(decision.decision);
        throw new AutonomousEvidenceAcquisitionError(failure.failureClass, failure.retryable, `source admission ${decision.decision}`);
      }
      return value;
    },
  };
}

/** Stable classifier for source-gate failures at the existing retry boundary. */
export const classifyAutonomousEvidenceSourceError: AutonomousEvidenceRetryClassifier = (error) => {
  if (error instanceof AutonomousEvidenceAcquisitionError && error.failure_class.startsWith("source_")) return { failure_class: error.failure_class, retryable: error.retryable };
  return { failure_class: "source_boundary_error", retryable: false };
};
