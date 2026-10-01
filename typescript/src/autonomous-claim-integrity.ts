/**
 * Provider-free claim-integrity fusion for the autonomous brain.
 *
 * Acquisition, grounding, contradiction, temporal, and reproducibility contracts remain
 * independently useful. This module is the brain-level join: it decides what a claim may rely on
 * and proposes the next bounded action. It consumes metadata and digests only; it never fetches a
 * source, calls an LLM, reads a clock, or retains claim/evidence values, prompts, locators, or
 * credentials.
 */
import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  AutonomousInformationAcquisitionCandidate,
  type AutonomousInformationAcquisitionCandidateInput,
  AutonomousInformationAcquisitionPlan,
  type AutonomousInformationAcquisitionPolicy,
  type AutonomousInformationAcquisitionPolicyInput,
  planAutonomousInformationAcquisition,
} from "./autonomous-information-acquisition.js";
import { canonicalJson, digestCanonicalJsonTextSync, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import {
  AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
  type AutonomousEvidenceAssessmentJSON,
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceReceiptJSON,
} from "./autonomous-evidence-runtime.js";
import { AutonomousEvidenceExecutionResult } from "./autonomous-evidence-execution.js";
import type { AutonomousEvidenceExecutionResumableRun } from "./autonomous-evidence-execution-resumable.js";

export const AUTONOMOUS_CLAIM_INTEGRITY_SCHEMA = "bioprism-typescript-autonomous-claim-integrity/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-policy/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-claim/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-evidence/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-assessment/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-action/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-acquisition-bridge/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BINDING_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-acquisition-binding/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_AUTHORITY_REVIEW_SCHEMA = "bioprism-autonomous-claim-integrity-evidence-authority-review/0.2" as const;

export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIMS = 128;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE = 512;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS = 128;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIM_LINKS = 32;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_MODALITIES = 16;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES = 16_384;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_AGE_SECONDS = 31_536_000;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACQUISITION_REQUESTS = 64;

export const AUTONOMOUS_CLAIM_INTEGRITY_STATUSES = ["supported", "partially_supported", "missing", "stale", "conflicted", "contradicted", "insufficient_independence", "insufficient_modalities", "unreproducible", "blocked"] as const;
export type AutonomousClaimIntegrityStatus = typeof AUTONOMOUS_CLAIM_INTEGRITY_STATUSES[number];
export const AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES = ["accepted", "partial", "rejected", "stale", "failed", "reconciliation_required"] as const;
export type AutonomousClaimIntegrityEvidenceStatus = typeof AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES[number];
export const AUTONOMOUS_CLAIM_INTEGRITY_STANCES = ["support", "contradict", "neutral"] as const;
export type AutonomousClaimIntegrityStance = typeof AUTONOMOUS_CLAIM_INTEGRITY_STANCES[number];
export const AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY = ["reproduced", "observed", "declared", "unverified", "failed"] as const;
export type AutonomousClaimIntegrityReproducibility = typeof AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY[number];
export const AUTONOMOUS_CLAIM_INTEGRITY_ACTION_TYPES = ["acquire_evidence", "acquire_fresh_evidence", "acquire_independent_source", "acquire_cross_modal_evidence", "resolve_contradiction", "reproduce_evidence"] as const;
export type AutonomousClaimIntegrityActionType = typeof AUTONOMOUS_CLAIM_INTEGRITY_ACTION_TYPES[number];
export type AutonomousClaimIntegrityTemporalState = "valid" | "stale" | "future" | "not_yet_valid" | "expired";

const SECRET_MARKERS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "privatekey", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "clientsecret", "gsk", "sk"]);

function fail(message: string): never { throw new ArgumentError(`autonomous claim integrity ${message}`); }
function bytes(value: string): number { return new TextEncoder().encode(value).byteLength; }
function text(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) fail(`${name} must be bounded non-empty text`);
  return value.trim();
}
function identifier(name: string, value: unknown, maximum = 256): string {
  const candidate = text(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(candidate)) fail(`${name} contains unsupported identifier characters`);
  return candidate;
}
function digest(name: string, value: unknown, allowNull = false): string | null {
  if ((value === null || value === undefined) && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}
function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its bounds`);
  return value;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}
function rounded(value: number): number { return Math.round(value * 100_000_000) / 100_000_000; }
function reviewUnits(value: number): number { return Math.floor(value * 100_000_000 + 0.5); }
function timestamp(name: string, value: unknown): string {
  const candidate = text(name, value, 64);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})$/.test(candidate) || Number.isNaN(Date.parse(candidate))) fail(`${name} must be an RFC3339 timestamp`);
  return candidate;
}
function epoch(value: string): number { return Date.parse(value) / 1000; }
function isPlainJsonObject(value: unknown): value is Record<string, unknown> {
  if (!isObject(value)) return false;
  try {
    const prototype = Object.getPrototypeOf(value);
    return prototype === Object.prototype || prototype === null;
  } catch {
    return false;
  }
}
function safeMetadata(value: unknown, name = "metadata", depth = 0): void {
  if (depth > 8) fail(`${name} is too deeply nested`);
  if (Array.isArray(value)) { if (value.length > 128) fail(`${name} contains too many entries`); value.forEach((child, index) => safeMetadata(child, `${name}[${index}]`, depth + 1)); return; }
  if (isPlainJsonObject(value)) {
    if (Object.keys(value).length > 64) fail(`${name} contains too many fields`);
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\u0000")) fail(`${name} contains an invalid key`);
      const marker = [...key.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
      if (SECRET_MARKERS.has(marker) || marker.includes("secret") || marker.includes("credential") || marker.includes("token")) fail(`${name}.${key} is credential-shaped metadata`);
      safeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (value === null || typeof value === "string" || typeof value === "boolean" || typeof value === "number" && Number.isFinite(value)) return;
  fail(`${name} contains unsupported metadata`);
}
function freezeJson<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map((item) => freezeJson(item))) as T;
  if (isObject(value)) return Object.freeze(Object.fromEntries(Object.entries(value).map(([key, item]) => [key, freezeJson(item)]))) as T;
  return value;
}
function metadataDigest(value: Readonly<Record<string, unknown>>): string {
  return digestCanonicalJsonTextSync(canonicalMetadata(value));
}
function reviewMetadataDigest(value: Readonly<Record<string, unknown>>): string {
  canonicalMetadata(value);
  return digestJsonSync(reviewMetadataNode(value));
}
function canonicalMetadata(value: Readonly<Record<string, unknown>>): string {
  safeMetadata(value);
  metadataFootprint(value);
  const serialized = canonicalJson(value);
  if (bytes(serialized) > AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES) fail("metadata exceeds its canonical byte bound");
  return serialized;
}
function metadataFootprint(value: unknown): void {
  let total = 0;
  const add = (size: number): void => {
    total += size;
    if (total > AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES) fail("metadata exceeds its bounded footprint");
  };
  const visit = (item: unknown): void => {
    if (item === null) { add(4); return; }
    if (typeof item === "boolean") { add(item ? 4 : 5); return; }
    if (typeof item === "string") { add(metadataStringBytes(item) + 2); return; }
    if (typeof item === "number") {
      if (Number.isInteger(item) && !Number.isSafeInteger(item)) fail("metadata contains an unsafe integer");
      add(1);
      return;
    }
    if (Array.isArray(item)) {
      add(2);
      item.forEach((child, index) => { if (index > 0) add(1); visit(child); });
      return;
    }
    if (isPlainJsonObject(item)) {
      add(2);
      Object.keys(item).sort().forEach((key, index) => {
        if (index > 0) add(1);
        add(metadataStringBytes(key) + 3);
        visit(item[key]);
      });
      return;
    }
    fail("metadata contains unsupported JSON");
  };
  visit(value);
}
function metadataStringBytes(value: string): number {
  if (value.length > AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES) fail("metadata string exceeds its bound");
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) fail("metadata contains an unpaired surrogate");
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) fail("metadata contains an unpaired surrogate");
  }
  return bytes(value);
}
function reviewMetadataNode(value: unknown): unknown {
  if (value === null) return ["null"];
  if (typeof value === "boolean") return ["boolean", value];
  if (typeof value === "string") return ["string", utf16Hex(value)];
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Number.isInteger(value) && !Number.isSafeInteger(value)) fail("authority review metadata contains an unsafe number");
    const normalized = value === 0 ? 0 : value;
    const buffer = new ArrayBuffer(8);
    new DataView(buffer).setFloat64(0, normalized, false);
    const numberHex = [...new Uint8Array(buffer)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    return ["number", numberHex];
  }
  if (Array.isArray(value)) return ["array", value.map(reviewMetadataNode)];
  if (isPlainJsonObject(value)) {
    const keys = Object.keys(value).sort();
    return ["object", keys.map((key) => [utf16Hex(key), reviewMetadataNode(value[key])])];
  }
  fail("authority review metadata contains unsupported JSON");
}
function utf16Hex(value: string): string {
  let encoded = "";
  for (let index = 0; index < value.length; index += 1) encoded += value.charCodeAt(index).toString(16).padStart(4, "0");
  return encoded;
}
function identifiers(name: string, value: readonly unknown[], maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) fail(`${name} is outside its bounds`);
  const normalized = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(normalized).size !== normalized.length) fail(`${name} contains duplicate identifiers`);
  return normalized;
}
function domains(name: string, value: readonly string[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > AUTONOMOUS_DOMAIN_NAMES.length) fail(`${name} is outside its bounds`);
  const normalized = value.map((item, index) => identifier(`${name}[${index}]`, item, 64) as AutonomousDomainName);
  if (new Set(normalized).size !== normalized.length || normalized.some((item) => !AUTONOMOUS_DOMAIN_NAMES.includes(item))) fail(`${name} contains duplicate or unsupported domains`);
  return normalized;
}
function read(value: Record<string, unknown>, snake: string, camel: string, fallback?: unknown): unknown { return value[snake] ?? value[camel] ?? fallback; }

export interface AutonomousClaimIntegrityPolicyInput {
  maxAgeSeconds?: number;
  minReliability?: number;
  minSupport?: number;
  requireIndependentSources?: boolean;
  minIndependentSources?: number;
  requireCrossModalAgreement?: boolean;
  contradictionVeto?: boolean;
  requireReproducibility?: boolean;
  allowPartial?: boolean;
  maxActions?: number;
}

export class AutonomousClaimIntegrityPolicy {
  readonly maxAgeSeconds: number;
  readonly minReliability: number;
  readonly minSupport: number;
  readonly requireIndependentSources: boolean;
  readonly minIndependentSources: number;
  readonly requireCrossModalAgreement: boolean;
  readonly contradictionVeto: boolean;
  readonly requireReproducibility: boolean;
  readonly allowPartial: boolean;
  readonly maxActions: number;
  constructor(input: AutonomousClaimIntegrityPolicyInput = {}) {
    this.maxAgeSeconds = integer("policy maxAgeSeconds", input.maxAgeSeconds ?? 86_400, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_AGE_SECONDS);
    this.minReliability = finite("policy minReliability", input.minReliability ?? 0.5, 0, 1);
    this.minSupport = finite("policy minSupport", input.minSupport ?? 0.5, 0, 1);
    for (const name of ["requireIndependentSources", "requireCrossModalAgreement", "contradictionVeto", "requireReproducibility", "allowPartial"] as const) if (input[name] !== undefined && typeof input[name] !== "boolean") fail(`policy ${name} must be boolean`);
    this.requireIndependentSources = input.requireIndependentSources ?? false;
    this.minIndependentSources = integer("policy minIndependentSources", input.minIndependentSources ?? 1, 1, 16);
    this.requireCrossModalAgreement = input.requireCrossModalAgreement ?? false;
    this.contradictionVeto = input.contradictionVeto ?? true;
    this.requireReproducibility = input.requireReproducibility ?? false;
    this.allowPartial = input.allowPartial ?? false;
    this.maxActions = integer("policy maxActions", input.maxActions ?? 32, 1, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS);
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA, max_age_seconds: this.maxAgeSeconds, min_reliability: rounded(this.minReliability), min_support: rounded(this.minSupport), require_independent_sources: this.requireIndependentSources, min_independent_sources: this.minIndependentSources, require_cross_modal_agreement: this.requireCrossModalAgreement, contradiction_veto: this.contradictionVeto, require_reproducibility: this.requireReproducibility, allow_partial: this.allowPartial, max_actions: this.maxActions }; }
  get policyDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), policy_digest: this.policyDigest, execution: "provider_free_metadata_fusion;no_source_or_provider_dispatch", retention: "metadata_only;raw_claim_and_evidence_values_caller_owned", secret_material: "never_returned" }; }
}

export interface AutonomousClaimIntegrityClaimInput {
  claimId: string;
  domain: AutonomousDomainName;
  claimDigest: string;
  requiredSupport?: number;
  requiredIndependentSources?: number;
  requiredReproducibility?: boolean;
  requiredModalities?: readonly string[];
  priority?: number;
  metadata?: Readonly<Record<string, unknown>>;
}

export class AutonomousClaimIntegrityClaim {
  readonly claimId: string; readonly domain: AutonomousDomainName; readonly claimDigest: string; readonly requiredSupport: number;
  readonly requiredIndependentSources: number; readonly requiredReproducibility: boolean; readonly requiredModalities: readonly string[];
  readonly priority: number; readonly metadata: Readonly<Record<string, unknown>>;
  constructor(input: AutonomousClaimIntegrityClaimInput) {
    this.claimId = identifier("claim claimId", input.claimId); this.domain = identifier("claim domain", input.domain, 64) as AutonomousDomainName; if (!AUTONOMOUS_DOMAIN_NAMES.includes(this.domain)) fail("claim domain is unsupported");
    this.claimDigest = digest("claim claimDigest", input.claimDigest)!; this.requiredSupport = finite("claim requiredSupport", input.requiredSupport ?? 0.5, 0, 1); this.requiredIndependentSources = integer("claim requiredIndependentSources", input.requiredIndependentSources ?? 1, 1, 16);
    if (input.requiredReproducibility !== undefined && typeof input.requiredReproducibility !== "boolean") fail("claim requiredReproducibility must be boolean"); this.requiredReproducibility = input.requiredReproducibility ?? false;
    this.requiredModalities = identifiers("claim requiredModalities", input.requiredModalities ?? [], AUTONOMOUS_CLAIM_INTEGRITY_MAX_MODALITIES); this.priority = finite("claim priority", input.priority ?? 0.5, 0, 1);
    const metadata = input.metadata ?? {}; if (!isObject(metadata)) fail("claim metadata must be an object"); safeMetadata(metadata, "claim metadata"); this.metadata = { ...metadata };
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA, claim_id: this.claimId, domain: this.domain, claim_digest: this.claimDigest, required_support: rounded(this.requiredSupport), required_independent_sources: this.requiredIndependentSources, required_reproducibility: this.requiredReproducibility, required_modalities: [...this.requiredModalities], priority: rounded(this.priority), metadata_digest: metadataDigest(this.metadata) }; }
  get claimContractDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), claim_contract_digest: this.claimContractDigest, secret_material: "never_returned" }; }
}

export interface AutonomousClaimIntegrityEvidenceInput {
  evidenceId: string;
  domain: AutonomousDomainName;
  claimIds: readonly string[];
  sourceId: string;
  evidenceDigest: string;
  sourceDigest?: string | null;
  observedAt: string;
  validFrom?: string | null;
  validUntil?: string | null;
  reliability: number;
  support: number;
  status: AutonomousClaimIntegrityEvidenceStatus;
  stance: AutonomousClaimIntegrityStance;
  modality?: string;
  reproducibility?: AutonomousClaimIntegrityReproducibility;
  metadata?: Readonly<Record<string, unknown>>;
}

export class AutonomousClaimIntegrityEvidence {
  readonly evidenceId: string; readonly domain: AutonomousDomainName; readonly claimIds: readonly string[]; readonly sourceId: string;
  readonly evidenceDigest: string; readonly sourceDigest: string | null; readonly observedAt: string; readonly validFrom: string | null; readonly validUntil: string | null;
  readonly reliability: number; readonly support: number; readonly status: AutonomousClaimIntegrityEvidenceStatus; readonly stance: AutonomousClaimIntegrityStance;
  readonly modality: string; readonly reproducibility: AutonomousClaimIntegrityReproducibility; readonly metadata: Readonly<Record<string, unknown>>;
  constructor(input: AutonomousClaimIntegrityEvidenceInput) {
    this.evidenceId = identifier("evidence evidenceId", input.evidenceId); this.domain = identifier("evidence domain", input.domain, 64) as AutonomousDomainName; if (!AUTONOMOUS_DOMAIN_NAMES.includes(this.domain)) fail("evidence domain is unsupported");
    this.claimIds = identifiers("evidence claimIds", input.claimIds, 32); if (this.claimIds.length === 0) fail("evidence claimIds must not be empty"); this.sourceId = identifier("evidence sourceId", input.sourceId);
    this.evidenceDigest = digest("evidence evidenceDigest", input.evidenceDigest)!; this.sourceDigest = digest("evidence sourceDigest", input.sourceDigest ?? null, true); this.observedAt = timestamp("evidence observedAt", input.observedAt);
    this.validFrom = input.validFrom === undefined || input.validFrom === null ? null : timestamp("evidence validFrom", input.validFrom); this.validUntil = input.validUntil === undefined || input.validUntil === null ? null : timestamp("evidence validUntil", input.validUntil);
    if (this.validFrom !== null && this.validUntil !== null && epoch(this.validFrom) >= epoch(this.validUntil)) fail("evidence validFrom must precede validUntil");
    this.reliability = finite("evidence reliability", input.reliability, 0, 1); this.support = finite("evidence support", input.support, 0, 1);
    this.status = input.status; if (!(AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES as readonly string[]).includes(this.status)) fail("evidence status is unsupported");
    this.stance = input.stance; if (!(AUTONOMOUS_CLAIM_INTEGRITY_STANCES as readonly string[]).includes(this.stance)) fail("evidence stance is unsupported"); this.modality = identifier("evidence modality", input.modality ?? "unspecified");
    this.reproducibility = input.reproducibility ?? "unverified"; if (!(AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY as readonly string[]).includes(this.reproducibility)) fail("evidence reproducibility is unsupported");
    const metadata = input.metadata ?? {}; if (!isObject(metadata)) fail("evidence metadata must be an object"); safeMetadata(metadata, "evidence metadata"); this.metadata = { ...metadata };
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA, evidence_id: this.evidenceId, domain: this.domain, claim_ids: [...this.claimIds], source_id: this.sourceId, source_digest: this.sourceDigest, evidence_digest: this.evidenceDigest, observed_at: this.observedAt, valid_from: this.validFrom, valid_until: this.validUntil, reliability: rounded(this.reliability), support: rounded(this.support), status: this.status, stance: this.stance, modality: this.modality, reproducibility: this.reproducibility, metadata_digest: metadataDigest(this.metadata) }; }
  get evidenceContractDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), evidence_contract_digest: this.evidenceContractDigest, secret_material: "never_returned" }; }
}

export interface AutonomousClaimIntegrityEvidenceRow extends JsonObject { evidence_id: string; domain: AutonomousDomainName; claim_ids: string[]; status: AutonomousClaimIntegrityEvidenceStatus; stance: AutonomousClaimIntegrityStance; usable: boolean; temporal_state: AutonomousClaimIntegrityTemporalState; source_key: string; reliability: number; support: number; reproducibility: AutonomousClaimIntegrityReproducibility; issues: string[]; }
export interface AutonomousClaimIntegrityClaimAssessmentJSON extends JsonObject { claim_id: string; domain: AutonomousDomainName; status: AutonomousClaimIntegrityStatus; support_score: number; confidence: number; supporting_evidence_ids: string[]; contradicting_evidence_ids: string[]; usable_evidence_ids: string[]; independent_source_count: number; modalities: string[]; missing_modalities: string[]; reproducibility: string; temporal_state: string; issues: string[]; next_action_type: AutonomousClaimIntegrityActionType | null; priority: number; }

export class AutonomousClaimIntegrityAction {
  readonly actionType: AutonomousClaimIntegrityActionType; readonly domain: AutonomousDomainName; readonly claimIds: readonly string[]; readonly blockingEvidenceIds: readonly string[]; readonly reasonCodes: readonly string[]; readonly priority: number; readonly expectedValue: number;
  constructor(input: { actionType: AutonomousClaimIntegrityActionType; domain: AutonomousDomainName; claimIds: readonly string[]; blockingEvidenceIds: readonly string[]; reasonCodes: readonly string[]; priority: number; expectedValue: number }) {
    if (!(AUTONOMOUS_CLAIM_INTEGRITY_ACTION_TYPES as readonly string[]).includes(input.actionType)) fail("action type is unsupported"); this.actionType = input.actionType; this.domain = domains("action domain", [input.domain])[0]!;
    this.claimIds = identifiers("action claimIds", input.claimIds, 32); this.blockingEvidenceIds = identifiers("action blockingEvidenceIds", input.blockingEvidenceIds, 512); this.reasonCodes = identifiers("action reasonCodes", input.reasonCodes, 32); this.priority = finite("action priority", input.priority, 0, 1); this.expectedValue = finite("action expectedValue", input.expectedValue, 0, 1);
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA, action_type: this.actionType, domain: this.domain, claim_ids: [...this.claimIds], blocking_evidence_ids: [...this.blockingEvidenceIds], reason_codes: [...this.reasonCodes], priority: rounded(this.priority), expected_value: rounded(this.expectedValue) }; }
  get actionId(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), action_id: this.actionId, dispatch: "planning_only;caller_approval_required", secret_material: "never_returned" }; }
}

export interface AutonomousClaimIntegrityAssessmentJSON extends JsonObject { schema: string; context_digest: string; reference_time: string; policy_digest: string; claims: AutonomousClaimIntegrityClaimAssessmentJSON[]; evidence: AutonomousClaimIntegrityEvidenceRow[]; actions: JsonObject[]; omitted_actions: number; status: "ready" | "partial" | "blocked"; summary: JsonObject; prior_assessment_digest: string | null; generation: number; assessment_digest?: string; }

export class AutonomousClaimIntegrityAssessment {
  readonly contextDigest: string; readonly referenceTime: string; readonly policy: AutonomousClaimIntegrityPolicy; readonly claims: readonly AutonomousClaimIntegrityClaimAssessmentJSON[]; readonly evidence: readonly AutonomousClaimIntegrityEvidenceRow[]; readonly actions: readonly AutonomousClaimIntegrityAction[]; readonly omittedActions: number; readonly status: "ready" | "partial" | "blocked"; readonly summary: JsonObject; readonly priorAssessmentDigest: string | null; readonly generation: number;
  constructor(input: { contextDigest: string; referenceTime: string; policy: AutonomousClaimIntegrityPolicy; claims: readonly AutonomousClaimIntegrityClaimAssessmentJSON[]; evidence: readonly AutonomousClaimIntegrityEvidenceRow[]; actions: readonly AutonomousClaimIntegrityAction[]; omittedActions: number; status: "ready" | "partial" | "blocked"; summary: JsonObject; priorAssessmentDigest?: string | null; generation?: number }) {
    this.contextDigest = digest("assessment contextDigest", input.contextDigest)!; this.referenceTime = timestamp("assessment referenceTime", input.referenceTime); this.policy = input.policy; if (!(input.policy instanceof AutonomousClaimIntegrityPolicy)) fail("assessment policy is malformed");
    this.claims = [...input.claims]; this.evidence = [...input.evidence]; this.actions = [...input.actions]; this.omittedActions = integer("assessment omittedActions", input.omittedActions, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS); this.status = input.status; if (!(input.status === "ready" || input.status === "partial" || input.status === "blocked")) fail("assessment status is unsupported");
    if (!isObject(input.summary)) fail("assessment summary must be an object"); this.summary = { ...input.summary }; this.priorAssessmentDigest = digest("assessment priorAssessmentDigest", input.priorAssessmentDigest ?? null, true); this.generation = integer("assessment generation", input.generation ?? 1, 1, 2_147_483_647);
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA, context_digest: this.contextDigest, reference_time: this.referenceTime, policy_digest: this.policy.policyDigest, claims: [...this.claims], evidence: [...this.evidence], actions: this.actions.map((action) => action.toJSON()), omitted_actions: this.omittedActions, status: this.status, summary: this.summary, prior_assessment_digest: this.priorAssessmentDigest, generation: this.generation }; }
  get digestDescriptor(): JsonObject { return this.payload(); }
  get assessmentDigest(): string { return digestJsonSync(this.payload()); }
  get ready(): boolean { return this.status === "ready"; }
  toJSON(): AutonomousClaimIntegrityAssessmentJSON { return { ...this.payload(), assessment_digest: this.assessmentDigest, policy: this.policy.toJSON(), execution: "provider_free_claim_integrity_fusion;no_source_or_provider_dispatch", retention: "metadata_only;claim_text_evidence_values_prompts_locators_credentials_caller_owned", authorization: "actions_are_proposals;acquisition_resolution_and_provider_calls_require_separate_approval", secret_material: "never_returned" } as unknown as AutonomousClaimIntegrityAssessmentJSON; }
}

export interface AutonomousClaimIntegrityAcquisitionBridgeJSON extends JsonObject {
  schema: string;
  assessment_digest: string;
  action_ids: string[];
  targeted_candidate_ids: string[];
  candidate_action_matches: JsonObject[];
  acquisition_plan_digest: string | null;
  unmatched_action_count: number;
  status: "planned" | "no_action_required" | "blocked";
  generation: number;
  bridge_digest?: string;
}

export class AutonomousClaimIntegrityAcquisitionBridge {
  readonly assessmentDigest: string;
  readonly actionIds: readonly string[];
  readonly targetedCandidateIds: readonly string[];
  readonly candidateActionMatches: readonly JsonObject[];
  readonly acquisitionPlan: AutonomousInformationAcquisitionPlan | null;
  readonly unmatchedActionCount: number;
  readonly status: "planned" | "no_action_required" | "blocked";
  readonly generation: number;
  constructor(input: { assessmentDigest: string; actionIds: readonly string[]; targetedCandidateIds: readonly string[]; candidateActionMatches: readonly JsonObject[]; acquisitionPlan: AutonomousInformationAcquisitionPlan | null; unmatchedActionCount: number; status: "planned" | "no_action_required" | "blocked"; generation?: number }) {
    this.assessmentDigest = digest("bridge assessmentDigest", input.assessmentDigest)!; this.actionIds = identifiers("bridge actionIds", input.actionIds, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS); this.targetedCandidateIds = identifiers("bridge targetedCandidateIds", input.targetedCandidateIds, 512); this.candidateActionMatches = input.candidateActionMatches.map((item, index) => { if (!isObject(item)) fail(`bridge match ${index} must be an object`); safeMetadata(item, `bridge match ${index}`); return { ...item }; });
    this.acquisitionPlan = input.acquisitionPlan; if (this.acquisitionPlan !== null && !(this.acquisitionPlan instanceof AutonomousInformationAcquisitionPlan)) fail("bridge acquisitionPlan is malformed"); this.unmatchedActionCount = integer("bridge unmatchedActionCount", input.unmatchedActionCount, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS); this.status = input.status; if (this.status !== "planned" && this.status !== "no_action_required" && this.status !== "blocked") fail("bridge status is unsupported"); this.generation = integer("bridge generation", input.generation ?? 1, 1, 2_147_483_647);
    if (this.status === "planned" && this.acquisitionPlan === null) fail("planned bridge requires an acquisition plan"); if (this.status === "no_action_required" && this.unmatchedActionCount !== 0) fail("no-action bridge cannot have unmatched actions");
  }
  get digestDescriptor(): JsonObject { return { schema: AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA, assessment_digest: this.assessmentDigest, action_ids: [...this.actionIds], targeted_candidate_ids: [...this.targetedCandidateIds], candidate_action_matches: [...this.candidateActionMatches], acquisition_plan_digest: this.acquisitionPlan?.planDigest ?? null, unmatched_action_count: this.unmatchedActionCount, status: this.status, generation: this.generation }; }
  get bridgeDigest(): string { return digestJsonSync(this.digestDescriptor); }
  toJSON(): AutonomousClaimIntegrityAcquisitionBridgeJSON { return { ...this.digestDescriptor, bridge_digest: this.bridgeDigest, actions_are: "proposals_only;source_dispatch_requires_reviewed_evidence_approval", acquisition_plan: this.acquisitionPlan?.toJSON() ?? null, retention: "metadata_only;raw_claim_text_evidence_values_and_source_payloads_caller_owned", secret_material: "never_returned" } as unknown as AutonomousClaimIntegrityAcquisitionBridgeJSON; }
}

export interface AutonomousClaimIntegrityAcquisitionRequestInput extends JsonObject {
  candidate_id: string;
  requirement_id: string;
  source_id: string;
  source_digest?: string | null;
  request_id?: string | null;
  metadata?: JsonObject;
}

export interface AutonomousClaimIntegrityAcquisitionBindingJSON extends JsonObject {
  schema: string;
  assessment_digest: string;
  bridge_digest: string;
  acquisition_plan_digest: string;
  candidate_ids: string[];
  domains: string[];
  request_digests: string[];
  request_count: number;
  status: "ready";
  binding_digest?: string;
}

/** Exact transient request batch emitted from one reviewed integrity acquisition bridge. */
export class AutonomousClaimIntegrityAcquisitionBinding {
  readonly assessmentDigest: string;
  readonly bridgeDigest: string;
  readonly acquisitionPlanDigest: string;
  readonly candidateIds: readonly string[];
  /** Domain is aligned to candidateIds and may repeat for multiple candidates in one domain. */
  readonly domains: readonly AutonomousDomainName[];
  readonly requestDigests: readonly string[];
  readonly status = "ready" as const;
  private readonly transientRequests: readonly AutonomousEvidenceAcquisitionRequest[];

  constructor(input: { assessmentDigest: string; bridgeDigest: string; acquisitionPlanDigest: string; candidateIds: readonly string[]; domains: readonly AutonomousDomainName[]; requestDigests: readonly string[]; requests: readonly AutonomousEvidenceAcquisitionRequest[] }) {
    this.assessmentDigest = digest("acquisition binding assessmentDigest", input.assessmentDigest)!;
    this.bridgeDigest = digest("acquisition binding bridgeDigest", input.bridgeDigest)!;
    this.acquisitionPlanDigest = digest("acquisition binding acquisitionPlanDigest", input.acquisitionPlanDigest)!;
    this.candidateIds = identifiers("acquisition binding candidateIds", input.candidateIds, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACQUISITION_REQUESTS);
    if (!Array.isArray(input.domains) || input.domains.length !== this.candidateIds.length) fail("acquisition binding domains must align with candidates");
    this.domains = input.domains.map((domain, index) => {
      const normalized = identifier(`acquisition binding domain ${index}`, domain, 64) as AutonomousDomainName;
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(normalized)) fail(`acquisition binding domain ${index} is unsupported`);
      return normalized;
    });
    this.requestDigests = input.requestDigests.map((value, index) => digest(`acquisition binding requestDigest ${index}`, value)!);
    if (this.requestDigests.length !== this.candidateIds.length || new Set(this.requestDigests).size !== this.requestDigests.length) fail("acquisition binding request digests must align with unique requests");
    if (!Array.isArray(input.requests) || input.requests.length !== this.candidateIds.length || this.candidateIds.length < 1) fail("acquisition binding requests are malformed");
    this.transientRequests = input.requests.map((request) => ({ ...request, metadata: request.metadata === undefined ? {} : { ...request.metadata } }));
  }

  get requests(): readonly AutonomousEvidenceAcquisitionRequest[] {
    return this.transientRequests.map((request) => ({ ...request, metadata: request.metadata === undefined ? {} : { ...request.metadata } }));
  }

  private payload(): JsonObject {
    return { schema: AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BINDING_SCHEMA, assessment_digest: this.assessmentDigest, bridge_digest: this.bridgeDigest, acquisition_plan_digest: this.acquisitionPlanDigest, candidate_ids: [...this.candidateIds], domains: [...this.domains], request_digests: [...this.requestDigests], request_count: this.requestDigests.length, status: this.status };
  }

  get digestDescriptor(): JsonObject { return this.payload(); }
  get bindingDigest(): string { return digestJsonSync(this.payload()); }

  toJSON(): AutonomousClaimIntegrityAcquisitionBindingJSON {
    return { ...this.payload(), binding_digest: this.bindingDigest, execution: "bound_reviewed_evidence_request_batch;source_dispatch_requires_separate_approval", retention: "metadata_only;request_values_locators_and_source_payloads_caller_owned", secret_material: "never_returned" } as unknown as AutonomousClaimIntegrityAcquisitionBindingJSON;
  }
}

export function bindAutonomousClaimIntegrityAcquisitionRequests(
  bridge: AutonomousClaimIntegrityAcquisitionBridge,
  requests: readonly (AutonomousClaimIntegrityAcquisitionRequestInput | Record<string, unknown>)[],
): AutonomousClaimIntegrityAcquisitionBinding {
  validateAutonomousClaimIntegrityAcquisitionBridge(bridge);
  if (bridge.status !== "planned" || bridge.acquisitionPlan === null) fail("request binding requires a planned bridge");
  const selections = [...bridge.acquisitionPlan.selected];
  if (selections.length < 1 || selections.length > AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACQUISITION_REQUESTS) fail("acquisition plan selections are outside the binding limit");
  if (!Array.isArray(requests) || requests.length !== selections.length) fail("requests must contain exactly one request per selected candidate");
  const selectedById = new Map(selections.map((selection) => [selection.candidate_id, selection]));
  const supplied = new Map<string, AutonomousEvidenceAcquisitionRequest>();
  const reserved = new Set(["claim_integrity_assessment_digest", "claim_integrity_bridge_digest", "claim_integrity_acquisition_plan_digest", "claim_integrity_candidate_id", "claim_integrity_candidate_digest"]);
  for (const [index, raw] of requests.entries()) {
    if (!isObject(raw)) fail(`request ${index} must be an object`);
    const allowed = new Set(["candidate_id", "candidateId", "requirement_id", "source_id", "source_digest", "request_id", "metadata"]);
    if (Object.keys(raw).some((key) => !allowed.has(key))) fail(`request ${index} contains unsupported fields`);
    const candidateId = identifier(`request ${index} candidate_id`, read(raw, "candidate_id", "candidateId"));
    const selection = selectedById.get(candidateId);
    if (!selection) fail(`request ${index} targets a candidate outside the selected plan`);
    if (supplied.has(candidateId)) fail(`candidate ${candidateId} is duplicated`);
    const sourceId = identifier(`request ${index} source_id`, read(raw, "source_id", "sourceId"));
    if (sourceId !== selection.source_id) fail(`candidate ${candidateId} source does not match the selected source`);
    const requirementId = identifier(`request ${index} requirement_id`, read(raw, "requirement_id", "requirementId"));
    const sourceDigest = digest(`request ${index} source_digest`, read(raw, "source_digest", "sourceDigest", null), true);
    const requestIdValue = read(raw, "request_id", "requestId", null);
    const requestId = requestIdValue === null || requestIdValue === undefined ? null : identifier(`request ${index} request_id`, requestIdValue);
    const metadataValue = read(raw, "metadata", "metadata", {});
    if (!isObject(metadataValue)) fail(`request ${index} metadata must be an object`);
    if ([...reserved].some((key) => Object.prototype.hasOwnProperty.call(metadataValue, key))) fail(`request ${index} attempts to override binding metadata`);
    const metadata = { ...metadataValue, claim_integrity_assessment_digest: bridge.assessmentDigest, claim_integrity_bridge_digest: bridge.bridgeDigest, claim_integrity_acquisition_plan_digest: bridge.acquisitionPlan.planDigest, claim_integrity_candidate_id: candidateId, claim_integrity_candidate_digest: selection.candidate_digest };
    safeMetadata(metadata, `request ${index} metadata`);
    const bound = { requirement_id: requirementId, source_id: sourceId, source_digest: sourceDigest, request_id: requestId, metadata } as AutonomousEvidenceAcquisitionRequest;
    try { digestJsonSync(bound); } catch (error) { fail(`request ${index} is not canonical JSON`); }
    supplied.set(candidateId, bound);
  }
  if (supplied.size !== selections.length) fail("requests are missing selected candidates");
  const ordered = selections.map((selection) => supplied.get(selection.candidate_id)!);
  return new AutonomousClaimIntegrityAcquisitionBinding({ assessmentDigest: bridge.assessmentDigest, bridgeDigest: bridge.bridgeDigest, acquisitionPlanDigest: bridge.acquisitionPlan.planDigest, candidateIds: selections.map((selection) => selection.candidate_id), domains: selections.map((selection) => selection.domain as AutonomousDomainName), requestDigests: ordered.map((request) => digestJsonSync(request)), requests: ordered });
}

export function validateAutonomousClaimIntegrityAcquisitionBinding(value: AutonomousClaimIntegrityAcquisitionBinding): AutonomousClaimIntegrityAcquisitionBinding {
  if (!(value instanceof AutonomousClaimIntegrityAcquisitionBinding)) fail("binding validation requires a typed binding");
  if (digestJsonSync(value.digestDescriptor) !== value.bindingDigest) fail("binding digest does not match its fields");
  const requestDigests = value.requests.map((request) => digestJsonSync(request));
  if (JSON.stringify(requestDigests) !== JSON.stringify(value.requestDigests)) fail("binding request digest does not match its request");
  return value;
}

export interface AutonomousClaimIntegrityAcquiredEvidenceLink {
  receiptDigest: string;
  assessmentDigest: string;
  evidence: AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>;
}

type ReadonlyJsonValue = string | number | boolean | null | ReadonlyJsonObject | readonly ReadonlyJsonValue[];
interface ReadonlyJsonObject { readonly [key: string]: ReadonlyJsonValue; }

export interface AutonomousClaimIntegrityEvidenceAuthorityReview extends ReadonlyJsonObject {
  readonly schema: typeof AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_AUTHORITY_REVIEW_SCHEMA;
  readonly authority_id: string;
  readonly authority_version: string;
  readonly context_digest: string;
  readonly assessment_digest: string;
  readonly bridge_digest: string;
  readonly binding_digest: string;
  readonly candidate_id: string;
  readonly request_digest: string;
  readonly receipt: ReadonlyJsonObject;
  readonly source_quality_assessment: ReadonlyJsonObject;
  readonly claim_contracts: readonly ReadonlyJsonObject[];
  readonly evidence: ReadonlyJsonObject;
  readonly review_digest: string;
}

/** Deployment-owned, synchronous verifier for an existing claim-quality receipt. Verification must be read-only. */
export interface AutonomousClaimIntegrityEvidenceAuthority {
  readonly authority_id: string;
  readonly authority_version: string;
  verify(review: AutonomousClaimIntegrityEvidenceAuthorityReview): string;
}

export interface SettleAutonomousClaimIntegrityAcquisitionOptions {
  previous: AutonomousClaimIntegrityAssessment;
  bridge: AutonomousClaimIntegrityAcquisitionBridge;
  binding: AutonomousClaimIntegrityAcquisitionBinding;
  execution: AutonomousEvidenceExecutionResult | AutonomousEvidenceExecutionResumableRun;
  claims: readonly (AutonomousClaimIntegrityClaim | AutonomousClaimIntegrityClaimInput | Record<string, unknown>)[];
  existingEvidence: readonly (AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>)[];
  acquiredEvidence: readonly AutonomousClaimIntegrityAcquiredEvidenceLink[];
  referenceTime: string;
  evidenceAuthority: AutonomousClaimIntegrityEvidenceAuthority;
  policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput;
}

/**
 * Reassess claims with caller-owned claim judgments bound to the exact reviewed acquisition and
 * accepted source-quality assessment. Stance, support, reliability, modality, and reproducibility
 * remain explicit caller judgments; receipt success and evaluator score never become factual truth.
 */
export function settleAutonomousClaimIntegrityAcquisition(options: SettleAutonomousClaimIntegrityAcquisitionOptions): AutonomousClaimIntegrityAssessment {
  validateAutonomousClaimIntegrity(options.previous);
  validateAutonomousClaimIntegrityAcquisitionBridge(options.bridge);
  validateAutonomousClaimIntegrityAcquisitionBinding(options.binding);
  if (!options.evidenceAuthority || typeof options.evidenceAuthority.verify !== "function") fail("settlement requires a deployment-owned independent evidence authority");
  const evidenceAuthorityId = identifier("evidence authority id", options.evidenceAuthority.authority_id);
  const evidenceAuthorityVersion = identifier("evidence authority version", options.evidenceAuthority.authority_version);
  const { previous, bridge, binding } = options;
  if (bridge.assessmentDigest !== previous.assessmentDigest || bridge.generation !== previous.generation) fail("acquisition bridge is stale for the previous assessment");
  if (binding.assessmentDigest !== previous.assessmentDigest || binding.bridgeDigest !== bridge.bridgeDigest) fail("acquisition binding is stale for the selected bridge");
  if (bridge.status !== "planned" || bridge.acquisitionPlan === null) fail("settlement requires a planned acquisition bridge");
  if (binding.acquisitionPlanDigest !== bridge.acquisitionPlan.planDigest) fail("binding plan does not match its bridge");

  const result = options.execution instanceof AutonomousEvidenceExecutionResult
    ? options.execution
    : options.execution && "result" in options.execution ? options.execution.result : null;
  if (!(result instanceof AutonomousEvidenceExecutionResult)) fail("settlement requires a completed reviewed execution result");
  const executionPlanJSON = result.plan.toJSON();
  const { plan_digest: executionPlanDigest, ...executionPlanPayload } = executionPlanJSON;
  if (digestJsonSync(executionPlanPayload) !== executionPlanDigest) fail("execution plan digest is invalid");
  const readinessJSON = result.readiness.toJSON();
  const { report_digest: readinessDigest, ...readinessPayload } = readinessJSON;
  if (digestJsonSync(readinessPayload) !== readinessDigest) fail("execution readiness digest is invalid");
  if (JSON.stringify(result.plan.domains) !== JSON.stringify(bridge.acquisitionPlan.selectedDomains)) fail("execution domains do not match the reviewed acquisition plan");
  const runtimeJSON = result.runtime.toJSON();
  const expectedRuntimeDigest = digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    status: runtimeJSON.status,
    plan_digest: runtimeJSON.plan.plan_digest,
    receipt_digests: runtimeJSON.receipts.map((receipt) => receipt.receipt_digest),
    assessment_digests: runtimeJSON.assessments.map((assessment) => assessment.assessment_digest),
    completed_requirement_ids: [...runtimeJSON.completed_requirement_ids].sort(),
    pending_evaluation_requirement_ids: [...runtimeJSON.pending_evaluation_requirement_ids].sort(),
    missing_requirement_ids: [...runtimeJSON.missing_requirement_ids].sort(),
    next_stage_ids: [...runtimeJSON.next_stage_ids].sort(),
    omitted_request_digests: [...runtimeJSON.omitted_request_digests].sort(),
    retention: "metadata_only;raw_values_caller_owned",
    secret_material: "never_returned",
  });
  if (expectedRuntimeDigest !== runtimeJSON.result_digest) fail("execution runtime result digest is invalid");

  const selections = [...bridge.acquisitionPlan.selected];
  if (JSON.stringify(selections.map((item) => item.candidate_id)) !== JSON.stringify(binding.candidateIds)) fail("binding candidate order differs from the reviewed plan");
  if (JSON.stringify(selections.map((item) => item.domain)) !== JSON.stringify(binding.domains)) fail("binding domains differ from the reviewed plan");
  const actionById = new Map(previous.actions.map((action) => [action.actionId, action]));
  const normalizedClaims = options.claims.map(normalizeClaim);
  const previousClaims = new Map(previous.claims.map((item) => [item.claim_id, item.domain]));
  if (normalizedClaims.length !== previousClaims.size || normalizedClaims.some((claim) => previousClaims.get(claim.claimId) !== claim.domain)) fail("settlement claim identities do not match the previous assessment");
  const claimContracts = new Map(normalizedClaims.map((claim) => [claim.claimId, {
    claim_id: claim.claimId,
    domain: claim.domain,
    claim_digest: claim.claimDigest,
    required_support_units_1e8: reviewUnits(claim.requiredSupport),
    required_independent_sources: claim.requiredIndependentSources,
    required_reproducibility: claim.requiredReproducibility,
    required_modalities: [...claim.requiredModalities],
    priority_units_1e8: reviewUnits(claim.priority),
    metadata_digest: reviewMetadataDigest(claim.metadata),
  }]));
  const authorizedClaimsByCandidate = new Map<string, Set<string>>();
  for (const match of bridge.candidateActionMatches) {
    const candidateId = match.candidate_id;
    const actionIds = match.action_ids;
    if (typeof candidateId !== "string" || !Array.isArray(actionIds)) fail("bridge action match is malformed");
    const claimIds = new Set<string>();
    for (const actionId of actionIds) {
      if (typeof actionId !== "string") fail("bridge action match contains a malformed action id");
      const action = actionById.get(actionId);
      if (!action) fail("bridge action match references a foreign claim action");
      action.claimIds.forEach((claimId) => claimIds.add(claimId));
    }
    authorizedClaimsByCandidate.set(candidateId, claimIds);
  }

  const requests = binding.requests;
  const expectedRequests = new Map<string, { candidateId: string; domain: string; request: AutonomousEvidenceAcquisitionRequest }>();
  for (const [index, selection] of selections.entries()) {
    const request = requests[index];
    if (!request) fail("binding request batch is incomplete");
    const metadata = request.metadata ?? {};
    if (metadata.claim_integrity_assessment_digest !== previous.assessmentDigest
      || metadata.claim_integrity_bridge_digest !== bridge.bridgeDigest
      || metadata.claim_integrity_acquisition_plan_digest !== bridge.acquisitionPlan.planDigest
      || metadata.claim_integrity_candidate_id !== selection.candidate_id
      || metadata.claim_integrity_candidate_digest !== selection.candidate_digest) fail("bound request metadata was changed");
    const requestDigest = digestJsonSync({
      schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
      plan_digest: result.plan.evidence_plan_digest,
      requirement_id: request.requirement_id,
      source_id: request.source_id,
      source_digest: request.source_digest ?? null,
      request_id: request.request_id ?? null,
      metadata,
    });
    if (expectedRequests.has(requestDigest)) fail("requests map to a duplicate runtime request");
    expectedRequests.set(requestDigest, { candidateId: selection.candidate_id, domain: selection.domain, request });
  }

  const receiptsByDigest = new Map<string, AutonomousEvidenceReceiptJSON>();
  const receiptToCandidate = new Map<string, string>();
  for (const receipt of runtimeJSON.receipts) {
    const { receipt_digest: receiptDigest, ...receiptPayload } = receipt;
    if (digestJsonSync(receiptPayload) !== receiptDigest) fail("execution contains an invalid receipt digest");
    const expected = expectedRequests.get(receipt.request_digest);
    if (!expected) fail("execution contains a receipt outside its request binding");
    if (receiptsByDigest.has(receipt.receipt_digest) || [...receiptsByDigest.values()].some((item) => item.request_digest === receipt.request_digest)) fail("execution contains duplicate request receipts");
    if (receipt.plan_digest !== result.plan.evidence_plan_digest
      || receipt.requirement_id !== expected.request.requirement_id
      || receipt.domain !== expected.domain
      || receipt.source_id !== expected.request.source_id
      || receipt.source_digest !== (expected.request.source_digest ?? null)) fail("receipt does not match its bound source request");
    receiptsByDigest.set(receipt.receipt_digest, receipt);
    receiptToCandidate.set(receipt.receipt_digest, expected.candidateId);
  }
  const settledRequestDigests = new Set([...receiptsByDigest.values()].map((receipt) => receipt.request_digest));
  const omittedRequestDigests = new Set(runtimeJSON.omitted_request_digests);
  if ([...settledRequestDigests].some((requestDigest) => omittedRequestDigests.has(requestDigest))
    || new Set([...settledRequestDigests, ...omittedRequestDigests]).size !== expectedRequests.size
    || [...expectedRequests.keys()].some((requestDigest) => !settledRequestDigests.has(requestDigest) && !omittedRequestDigests.has(requestDigest))) fail("execution does not account for the exact bound request batch");

  const assessmentsByReceipt = new Map<string, AutonomousEvidenceAssessmentJSON>();
  for (const assessment of runtimeJSON.assessments) {
    const { assessment_digest: assessmentDigest, ...assessmentPayload } = assessment;
    if (digestJsonSync(assessmentPayload) !== assessmentDigest) fail("execution contains an invalid evaluator assessment digest");
    if (!receiptsByDigest.has(assessment.receipt_digest)) fail("assessment references a receipt outside the execution");
    const receipt = receiptsByDigest.get(assessment.receipt_digest)!;
    if (receipt.requirement_id !== assessment.requirement_id) fail("assessment requirement does not match its receipt");
    if (assessmentsByReceipt.has(assessment.receipt_digest)) fail("execution contains duplicate evaluator assessments");
    assessmentsByReceipt.set(assessment.receipt_digest, assessment);
  }

  if (!Array.isArray(options.acquiredEvidence) || options.acquiredEvidence.length > AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE) fail("acquired evidence links are outside their bound");
  const pendingEvidenceReviews: Array<{
    evidence: AutonomousClaimIntegrityEvidence;
    receipt: AutonomousEvidenceReceiptJSON;
    assessment: AutonomousEvidenceAssessmentJSON;
    candidateId: string;
    reviewDigest: string;
    review: AutonomousClaimIntegrityEvidenceAuthorityReview;
  }> = [];
  const evidenceIds = new Set<string>();
  const reservedMetadata = new Set([
    "claim_integrity_acquisition_receipt_digest",
    "claim_integrity_source_quality_assessment_digest",
    "claim_integrity_source_quality_evaluator_id",
    "claim_integrity_source_quality_evaluator_version",
    "claim_integrity_evidence_authority_id",
    "claim_integrity_evidence_authority_version",
    "claim_integrity_evidence_authority_review_digest",
    "claim_integrity_evidence_authority_receipt_digest",
    "claim_integrity_acquisition_candidate_id",
    "claim_integrity_acquisition_binding_digest",
    "claim_integrity_acquisition_bridge_digest",
  ]);
  for (const [index, link] of options.acquiredEvidence.entries()) {
    if (!isObject(link) || Object.keys(link).length !== 3 || !["receiptDigest", "assessmentDigest", "evidence"].every((key) => Object.prototype.hasOwnProperty.call(link, key))) fail(`acquired evidence link ${index} is malformed`);
    const receiptDigest = digest(`acquired evidence link ${index} receiptDigest`, link.receiptDigest)!;
    const assessmentDigest = digest(`acquired evidence link ${index} assessmentDigest`, link.assessmentDigest)!;
    const receipt = receiptsByDigest.get(receiptDigest);
    const assessment = receipt ? assessmentsByReceipt.get(receipt.receipt_digest) : undefined;
    if (!receipt || !assessment || receipt.assessment_digest !== assessment.assessment_digest) fail("acquired evidence link is not attached to an assessed receipt");
    if (assessment.assessment_digest !== assessmentDigest || assessment.verdict !== "accepted" || assessment.evidence_digest === null) fail("acquired evidence requires the exact accepted source-quality assessment");
    if (receipt.status !== "observed" || receipt.evidence_status !== "declared_for_evaluator" || receipt.evaluator_status !== "accepted") fail("acquired evidence requires a complete accepted acquisition receipt");
    const rawEvidence = link.evidence;
    if (!(rawEvidence instanceof AutonomousClaimIntegrityEvidence) && !isObject(rawEvidence)) fail(`acquired evidence link ${index} has malformed evidence`);
    if (isObject(rawEvidence)) {
      const allowedEvidenceFields = new Set(["evidence_id", "evidenceId", "domain", "claim_ids", "claimIds", "source_id", "sourceId", "evidence_digest", "evidenceDigest", "source_digest", "sourceDigest", "observed_at", "observedAt", "valid_from", "validFrom", "valid_until", "validUntil", "reliability", "support", "status", "stance", "modality", "reproducibility", "metadata"]);
      if (Object.keys(rawEvidence).some((key) => !allowedEvidenceFields.has(key))) fail(`acquired evidence link ${index} contains unsupported evidence fields`);
    }
    const evidenceItem = normalizeEvidence(rawEvidence as AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>);
    const candidateId = receiptToCandidate.get(receipt.receipt_digest)!;
    if (evidenceItem.domain !== receipt.domain || evidenceItem.sourceId !== receipt.source_id || evidenceItem.sourceDigest !== receipt.source_digest || evidenceItem.evidenceDigest !== assessment.evidence_digest) fail("acquired evidence does not match its assessed source receipt");
    const allowedClaims = authorizedClaimsByCandidate.get(candidateId);
    if (!allowedClaims?.size || evidenceItem.claimIds.some((claimId) => !allowedClaims.has(claimId))) fail("acquired evidence targets claims outside the candidate's reviewed actions");
    if ([...reservedMetadata].some((key) => Object.prototype.hasOwnProperty.call(evidenceItem.metadata, key))) fail("acquired evidence attempts to override settlement provenance");
    if (evidenceIds.has(evidenceItem.evidenceId)) fail("acquired evidence contains duplicate identifiers");
    const sourceQualityReviewBody = {
      schema: AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_AUTHORITY_REVIEW_SCHEMA,
      authority_id: evidenceAuthorityId,
      authority_version: evidenceAuthorityVersion,
      context_digest: previous.contextDigest,
      assessment_digest: previous.assessmentDigest,
      bridge_digest: bridge.bridgeDigest,
      binding_digest: binding.bindingDigest,
      candidate_id: candidateId,
      request_digest: receipt.request_digest,
      receipt: {
        receipt_digest: receipt.receipt_digest,
        request_digest: receipt.request_digest,
        requirement_id: receipt.requirement_id,
        domain: receipt.domain,
        source_id: receipt.source_id,
        source_digest: receipt.source_digest,
        status: receipt.status,
        evidence_status: receipt.evidence_status,
        evaluator_status: receipt.evaluator_status,
      },
      source_quality_assessment: {
        assessment_digest: assessment.assessment_digest,
        receipt_digest: assessment.receipt_digest,
        requirement_id: assessment.requirement_id,
        evaluator_id: assessment.evaluator_id,
        evaluator_version: assessment.evaluator_version,
        verdict: assessment.verdict,
        score_units_1e8: reviewUnits(assessment.score),
        evidence_digest: assessment.evidence_digest,
      },
      claim_contracts: evidenceItem.claimIds.map((claimId) => {
        const claimContract = claimContracts.get(claimId);
        if (!claimContract) fail("acquired evidence references a claim without a reviewed contract");
        return claimContract;
      }),
      evidence: {
        evidence_id: evidenceItem.evidenceId,
        domain: evidenceItem.domain,
        claim_ids: [...evidenceItem.claimIds],
        source_id: evidenceItem.sourceId,
        source_digest: evidenceItem.sourceDigest,
        evidence_digest: evidenceItem.evidenceDigest,
        observed_at: evidenceItem.observedAt,
        valid_from: evidenceItem.validFrom,
        valid_until: evidenceItem.validUntil,
        reliability_units_1e8: reviewUnits(evidenceItem.reliability),
        support_units_1e8: reviewUnits(evidenceItem.support),
        status: evidenceItem.status,
        stance: evidenceItem.stance,
        modality: evidenceItem.modality,
        reproducibility: evidenceItem.reproducibility,
        metadata_digest: reviewMetadataDigest(evidenceItem.metadata),
      },
    } satisfies JsonObject;
    const sourceQualityReviewDigest = digestJsonSync(sourceQualityReviewBody);
    const authorityReview = freezeJson({ ...sourceQualityReviewBody, review_digest: sourceQualityReviewDigest }) as AutonomousClaimIntegrityEvidenceAuthorityReview;
    evidenceIds.add(evidenceItem.evidenceId);
    pendingEvidenceReviews.push({ evidence: evidenceItem, receipt, assessment, candidateId, reviewDigest: sourceQualityReviewDigest, review: authorityReview });
  }

  const priorEvidence = options.existingEvidence.map(normalizeEvidence);
  const unverifiedEvidence = pendingEvidenceReviews.map((item) => item.evidence);
  const allEvidenceIds = [...priorEvidence, ...unverifiedEvidence].map((item) => item.evidenceId);
  if (new Set(allEvidenceIds).size !== allEvidenceIds.length) fail("settlement evidence identifiers must be unique");
  reassessAutonomousClaimIntegrity({ previous, claims: normalizedClaims, evidence: [...priorEvidence, ...unverifiedEvidence], referenceTime: options.referenceTime, policy: options.policy });
  const linkedEvidence = pendingEvidenceReviews.map(({ evidence: evidenceItem, receipt, assessment, candidateId, reviewDigest, review }) => {
    let rawAuthorityReceiptDigest: unknown;
    try {
      rawAuthorityReceiptDigest = options.evidenceAuthority.verify(review);
    } catch {
      fail("deployment evidence authority failed to verify acquired claim evidence");
    }
    const authorityReceiptDigest = digest("evidence authority receipt digest", rawAuthorityReceiptDigest)!;
    return new AutonomousClaimIntegrityEvidence({
      evidenceId: evidenceItem.evidenceId,
      domain: evidenceItem.domain,
      claimIds: evidenceItem.claimIds,
      sourceId: evidenceItem.sourceId,
      sourceDigest: evidenceItem.sourceDigest,
      evidenceDigest: evidenceItem.evidenceDigest,
      observedAt: evidenceItem.observedAt,
      validFrom: evidenceItem.validFrom,
      validUntil: evidenceItem.validUntil,
      reliability: evidenceItem.reliability,
      support: evidenceItem.support,
      status: evidenceItem.status,
      stance: evidenceItem.stance,
      modality: evidenceItem.modality,
      reproducibility: evidenceItem.reproducibility,
      metadata: {
        ...evidenceItem.metadata,
        claim_integrity_acquisition_receipt_digest: receipt.receipt_digest,
        claim_integrity_source_quality_assessment_digest: assessment.assessment_digest,
        claim_integrity_source_quality_evaluator_id: assessment.evaluator_id,
        claim_integrity_source_quality_evaluator_version: assessment.evaluator_version,
        claim_integrity_evidence_authority_id: evidenceAuthorityId,
        claim_integrity_evidence_authority_version: evidenceAuthorityVersion,
        claim_integrity_evidence_authority_review_digest: reviewDigest,
        claim_integrity_evidence_authority_receipt_digest: authorityReceiptDigest,
        claim_integrity_acquisition_candidate_id: candidateId,
        claim_integrity_acquisition_binding_digest: binding.bindingDigest,
        claim_integrity_acquisition_bridge_digest: bridge.bridgeDigest,
      },
    });
  });
  const allEvidence = [...priorEvidence, ...linkedEvidence];
  return reassessAutonomousClaimIntegrity({ previous, claims: normalizedClaims, evidence: allEvidence, referenceTime: options.referenceTime, policy: options.policy });
}

function normalizePolicy(value: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput | undefined): AutonomousClaimIntegrityPolicy { return value instanceof AutonomousClaimIntegrityPolicy ? value : new AutonomousClaimIntegrityPolicy(value); }
function normalizeClaim(value: AutonomousClaimIntegrityClaim | AutonomousClaimIntegrityClaimInput | Record<string, unknown>): AutonomousClaimIntegrityClaim {
  if (value instanceof AutonomousClaimIntegrityClaim) return value;
  if ("claimId" in value) return new AutonomousClaimIntegrityClaim(value as AutonomousClaimIntegrityClaimInput);
  const record = value as Record<string, unknown>;
  return new AutonomousClaimIntegrityClaim({ claimId: String(read(record, "claim_id", "claimId")), domain: read(record, "domain", "domain") as AutonomousDomainName, claimDigest: String(read(record, "claim_digest", "claimDigest")), requiredSupport: read(record, "required_support", "requiredSupport", 0.5) as number, requiredIndependentSources: read(record, "required_independent_sources", "requiredIndependentSources", 1) as number, requiredReproducibility: read(record, "required_reproducibility", "requiredReproducibility", false) as boolean, requiredModalities: (read(record, "required_modalities", "requiredModalities", []) as string[]) ?? [], priority: read(record, "priority", "priority", 0.5) as number, metadata: (read(record, "metadata", "metadata", {}) as Record<string, unknown>) ?? {} });
}
function normalizeEvidence(value: AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>): AutonomousClaimIntegrityEvidence {
  if (value instanceof AutonomousClaimIntegrityEvidence) return value;
  if ("evidenceId" in value) return new AutonomousClaimIntegrityEvidence(value as AutonomousClaimIntegrityEvidenceInput);
  const record = value as Record<string, unknown>;
  return new AutonomousClaimIntegrityEvidence({ evidenceId: String(read(record, "evidence_id", "evidenceId")), domain: read(record, "domain", "domain") as AutonomousDomainName, claimIds: (read(record, "claim_ids", "claimIds", []) as string[]) ?? [], sourceId: String(read(record, "source_id", "sourceId")), evidenceDigest: String(read(record, "evidence_digest", "evidenceDigest")), sourceDigest: read(record, "source_digest", "sourceDigest", null) as string | null, observedAt: String(read(record, "observed_at", "observedAt")), validFrom: read(record, "valid_from", "validFrom", null) as string | null, validUntil: read(record, "valid_until", "validUntil", null) as string | null, reliability: read(record, "reliability", "reliability") as number, support: read(record, "support", "support") as number, status: read(record, "status", "status") as AutonomousClaimIntegrityEvidenceStatus, stance: read(record, "stance", "stance") as AutonomousClaimIntegrityStance, modality: String(read(record, "modality", "modality", "unspecified")), reproducibility: read(record, "reproducibility", "reproducibility", "unverified") as AutonomousClaimIntegrityReproducibility, metadata: (read(record, "metadata", "metadata", {}) as Record<string, unknown>) ?? {} });
}
function normalizeAcquisitionCandidate(value: AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>): AutonomousInformationAcquisitionCandidate {
  if (value instanceof AutonomousInformationAcquisitionCandidate) return value;
  if ("candidateId" in value) return new AutonomousInformationAcquisitionCandidate(value as AutonomousInformationAcquisitionCandidateInput);
  const record = value as Record<string, unknown>;
  return new AutonomousInformationAcquisitionCandidate({ candidateId: String(read(record, "candidate_id", "candidateId")), domain: read(record, "domain", "domain") as AutonomousDomainName, capability: String(read(record, "capability", "capability")), sourceId: String(read(record, "source_id", "sourceId")), informationGain: read(record, "information_gain", "informationGain") as number, uncertaintyReduction: read(record, "uncertainty_reduction", "uncertaintyReduction") as number, reliability: read(record, "reliability", "reliability") as number, freshness: read(record, "freshness", "freshness") as number, coverage: read(record, "coverage", "coverage") as number, cost: read(record, "cost", "cost") as number, latencyMs: read(record, "latency_ms", "latencyMs") as number, risk: read(record, "risk", "risk") as number, conflictRisk: read(record, "conflict_risk", "conflictRisk") as number, priority: read(record, "priority", "priority", 0.5) as number, status: read(record, "status", "status", "available") as "available" | "partial" | "stale" | "unavailable" | "requires_approval" | "conflicted", dependsOn: (read(record, "depends_on", "dependsOn", []) as string[]) ?? [], sourceDigest: read(record, "source_digest", "sourceDigest", null) as string | null, metadata: (read(record, "metadata", "metadata", {}) as Record<string, unknown>) ?? {} });
}
function temporalState(item: AutonomousClaimIntegrityEvidence, referenceSeconds: number, maxAgeSeconds: number): AutonomousClaimIntegrityTemporalState {
  const observed = epoch(item.observedAt); if (observed > referenceSeconds) return "future"; if (item.validFrom !== null && referenceSeconds < epoch(item.validFrom)) return "not_yet_valid"; if (item.validUntil !== null && referenceSeconds >= epoch(item.validUntil)) return "expired"; if (referenceSeconds - observed > maxAgeSeconds || item.status === "stale") return "stale"; return "valid";
}
function evidenceRow(item: AutonomousClaimIntegrityEvidence, referenceSeconds: number, policy: AutonomousClaimIntegrityPolicy, claimIds: ReadonlySet<string>): AutonomousClaimIntegrityEvidenceRow {
  const temporal = temporalState(item, referenceSeconds, policy.maxAgeSeconds); const issues: string[] = []; if (temporal !== "valid") issues.push(temporal); if (item.status !== "accepted" && item.status !== "partial") issues.push(item.status); if (item.status === "partial" && !policy.allowPartial) issues.push("partial_not_allowed"); if (item.reliability < policy.minReliability) issues.push("below_reliability_floor"); if (item.support < policy.minSupport) issues.push("below_support_floor"); if (item.claimIds.some((id) => !claimIds.has(id))) issues.push("orphan_claim_reference");
  const usable = temporal === "valid" && item.reliability >= policy.minReliability && item.support >= policy.minSupport && (item.status === "accepted" || policy.allowPartial && item.status === "partial");
  return { schema: AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA, evidence_id: item.evidenceId, domain: item.domain, claim_ids: [...item.claimIds], status: item.status, stance: item.stance, usable, temporal_state: temporal, source_key: item.sourceDigest ?? item.sourceId, reliability: rounded(item.reliability), support: rounded(item.support), reproducibility: item.reproducibility, issues: [...new Set(issues)].sort() };
}
function capabilityMatch(candidate: AutonomousInformationAcquisitionCandidate, action: AutonomousClaimIntegrityAction): [boolean, "domain" | "capability" | "claim_and_domain" | "claim_and_capability"] {
  const capability = candidate.capability.toLowerCase().replaceAll("-", "_").replaceAll(" ", "_"); const actionToken = action.actionType.replace("acquire_", ""); const direct = capability.includes(actionToken) || actionToken === "evidence" && capability.includes("evidence"); const rawClaimIds = candidate.metadata.claim_ids; const claimMatch = Array.isArray(rawClaimIds) && rawClaimIds.some((item) => action.claimIds.includes(String(item)));
  if (claimMatch && direct) return [true, "claim_and_capability"]; if (claimMatch) return [true, "claim_and_domain"]; if (direct) return [true, "capability"]; return [true, "domain"];
}
function actionType(status: AutonomousClaimIntegrityStatus): AutonomousClaimIntegrityActionType | null {
  if (status === "supported") return null; if (status === "conflicted" || status === "contradicted") return "resolve_contradiction"; if (status === "stale") return "acquire_fresh_evidence"; if (status === "insufficient_independence") return "acquire_independent_source"; if (status === "insufficient_modalities") return "acquire_cross_modal_evidence"; if (status === "unreproducible") return "reproduce_evidence"; return "acquire_evidence";
}

export interface PlanAutonomousClaimIntegrityAcquisitionOptions { candidates: readonly (AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>)[]; policy?: AutonomousInformationAcquisitionPolicy | AutonomousInformationAcquisitionPolicyInput; requestedDomains?: readonly AutonomousDomainName[]; }
export function planAutonomousClaimIntegrityAcquisition(assessment: AutonomousClaimIntegrityAssessment, options: PlanAutonomousClaimIntegrityAcquisitionOptions): AutonomousClaimIntegrityAcquisitionBridge {
  validateAutonomousClaimIntegrity(assessment); if (!Array.isArray(options.candidates) || options.candidates.length > 512) fail("acquisition candidates are outside their bounds"); const candidates = options.candidates.map(normalizeAcquisitionCandidate); if (new Set(candidates.map((item) => item.candidateId)).size !== candidates.length) fail("acquisition candidates contain duplicate ids"); const actions = [...assessment.actions];
  if (actions.length === 0) return new AutonomousClaimIntegrityAcquisitionBridge({ assessmentDigest: assessment.assessmentDigest, actionIds: [], targetedCandidateIds: [], candidateActionMatches: [], acquisitionPlan: null, unmatchedActionCount: 0, status: "no_action_required", generation: assessment.generation });
  if (candidates.length === 0) return new AutonomousClaimIntegrityAcquisitionBridge({ assessmentDigest: assessment.assessmentDigest, actionIds: actions.map((action) => action.actionId), targetedCandidateIds: [], candidateActionMatches: [], acquisitionPlan: null, unmatchedActionCount: actions.length, status: "blocked", generation: assessment.generation });
  const actionDomains = new Set(actions.map((action) => action.domain)); const requestedDomains = options.requestedDomains === undefined ? AUTONOMOUS_DOMAIN_NAMES.filter((domain) => actionDomains.has(domain)) : domains("acquisition requestedDomains", options.requestedDomains); const matches: JsonObject[] = []; const targetedCandidateIds: string[] = []; const matchedActionIds = new Set<string>(); const adjusted: AutonomousInformationAcquisitionCandidate[] = []; const rank: Record<string, number> = { domain: 1, capability: 2, claim_and_domain: 3, claim_and_capability: 4 };
  for (const candidate of candidates) {
    const candidateMatches = actions
      .filter((action) => action.domain === candidate.domain)
      .map((action) => [action, capabilityMatch(candidate, action)[1]] as const);
    if (candidateMatches.length === 0) { adjusted.push(candidate); continue; }
    targetedCandidateIds.push(candidate.candidateId);
    candidateMatches.forEach(([action]) => matchedActionIds.add(action.actionId));
    const strongest = [...candidateMatches].sort((left, right) => (rank[right[1]] ?? 0) - (rank[left[1]] ?? 0) || right[0]!.priority - left[0]!.priority || left[0]!.actionId.localeCompare(right[0]!.actionId))[0]!;
    const strongestAction = strongest[0]!;
    const boost = Math.min(0.4, 0.1 + 0.05 * (rank[strongest[1]] ?? 0) + 0.1 * strongestAction.priority);
    adjusted.push(new AutonomousInformationAcquisitionCandidate({ candidateId: candidate.candidateId, domain: candidate.domain, capability: candidate.capability, sourceId: candidate.sourceId, informationGain: Math.min(1, candidate.informationGain + boost), uncertaintyReduction: Math.min(1, candidate.uncertaintyReduction + boost), reliability: candidate.reliability, freshness: candidate.freshness, coverage: Math.min(1, candidate.coverage + boost * 0.5), cost: candidate.cost, latencyMs: candidate.latencyMs, risk: candidate.risk, conflictRisk: candidate.conflictRisk, priority: Math.min(1, candidate.priority + boost), status: candidate.status, dependsOn: candidate.dependsOn, sourceDigest: candidate.sourceDigest, metadata: candidate.metadata }));
    matches.push({ candidate_id: candidate.candidateId, action_ids: candidateMatches.map(([action]) => action.actionId).sort(), action_types: [...new Set(candidateMatches.map(([action]) => action.actionType))].sort(), match_strength: strongest[1], priority_boost: rounded(boost) });
  }
  const acquisitionPlan = planAutonomousInformationAcquisition({ taskDigest: assessment.contextDigest, candidates: adjusted, requestedDomains, policy: options.policy }); const unmatchedActionCount = actions.filter((action) => !matchedActionIds.has(action.actionId)).length; return new AutonomousClaimIntegrityAcquisitionBridge({ assessmentDigest: assessment.assessmentDigest, actionIds: actions.map((action) => action.actionId), targetedCandidateIds, candidateActionMatches: matches, acquisitionPlan, unmatchedActionCount, status: acquisitionPlan.selected.length > 0 ? "planned" : "blocked", generation: assessment.generation });
}

export function validateAutonomousClaimIntegrityAcquisitionBridge(value: AutonomousClaimIntegrityAcquisitionBridge): AutonomousClaimIntegrityAcquisitionBridge { if (!(value instanceof AutonomousClaimIntegrityAcquisitionBridge)) fail("bridge validation requires a typed bridge"); if (digestJsonSync(value.digestDescriptor) !== value.bridgeDigest) fail("bridge digest does not match its fields"); return value; }

export interface AssessAutonomousClaimIntegrityOptions { contextDigest: string; claims: readonly (AutonomousClaimIntegrityClaim | AutonomousClaimIntegrityClaimInput | Record<string, unknown>)[]; evidence: readonly (AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>)[]; referenceTime: string; policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput; priorAssessmentDigest?: string | null; generation?: number; }

export function assessAutonomousClaimIntegrity(options: AssessAutonomousClaimIntegrityOptions): AutonomousClaimIntegrityAssessment {
  const contextDigest = digest("contextDigest", options.contextDigest)!; const referenceTime = timestamp("referenceTime", options.referenceTime); const policy = normalizePolicy(options.policy); if (!Array.isArray(options.claims) || options.claims.length < 1 || options.claims.length > AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIMS) fail("claims are outside their bounds"); if (!Array.isArray(options.evidence) || options.evidence.length > AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE) fail("evidence is outside its bounds");
  const claims = options.claims.map(normalizeClaim); const evidence = options.evidence.map(normalizeEvidence); const claimIds = new Set(claims.map((claim) => claim.claimId)); const evidenceIds = evidence.map((item) => item.evidenceId); const evidenceDigests = evidence.map((item) => item.evidenceDigest); if (claimIds.size !== claims.length) fail("claims contain duplicate ids"); if (new Set(evidenceIds).size !== evidenceIds.length) fail("evidence contains duplicate ids"); if (new Set(evidenceDigests).size !== evidenceDigests.length) fail("evidence contains duplicate evidence digests");
  const referenceSeconds = epoch(referenceTime); const rows = evidence.map((item) => evidenceRow(item, referenceSeconds, policy, claimIds)); const rowById = new Map(rows.map((row) => [row.evidence_id, row])); const assessments: AutonomousClaimIntegrityClaimAssessmentJSON[] = [];
  for (const claim of claims) {
    const linked = evidence.filter((item) => item.claimIds.includes(claim.claimId)); const usable = linked.filter((item) => rowById.get(item.evidenceId)!.usable && item.domain === claim.domain); const domainMismatch = linked.filter((item) => item.domain !== claim.domain); const supporting = usable.filter((item) => item.stance === "support"); const contradicting = usable.filter((item) => item.stance === "contradict"); const usableIds = usable.map((item) => item.evidenceId); const supportingIds = supporting.map((item) => item.evidenceId); const contradictingIds = contradicting.map((item) => item.evidenceId); const sources = new Set(supporting.map((item) => item.sourceDigest ?? item.sourceId)); const modalities = [...new Set(supporting.map((item) => item.modality))].sort();
    const requiredModalities = new Set(claim.requiredModalities); if (policy.requireCrossModalAgreement && requiredModalities.size === 0) requiredModalities.add("__at_least_two_modalities__"); const missingModalities = requiredModalities.has("__at_least_two_modalities__") ? [] : [...requiredModalities].filter((item) => !modalities.includes(item)).sort(); const modalShortfall = requiredModalities.has("__at_least_two_modalities__") ? modalities.length < 2 : missingModalities.length > 0;
    const supportScore = Math.min(1, supporting.reduce((total, item) => total + item.support * item.reliability * (item.status === "partial" ? 0.5 : 1), 0)); const requiredSources = Math.max(claim.requiredIndependentSources, policy.requireIndependentSources ? policy.minIndependentSources : 1); const temporalStates = new Set(linked.map((item) => temporalState(item, referenceSeconds, policy.maxAgeSeconds))); const temporal = usable.length > 0 ? "valid" : temporalStates.size > 0 && [...temporalStates].every((state) => state === "stale") ? "stale" : temporalStates.size > 0 && [...temporalStates].some((state) => state === "future" || state === "not_yet_valid" || state === "expired") ? "invalid" : "unknown";
    const reproduced = supporting.some((item) => item.reproducibility === "reproduced"); const reproduction = reproduced ? "reproduced" : supporting.length > 0 ? "unreproduced" : "unknown"; const issues: string[] = []; if (linked.length === 0) issues.push("no_evidence"); if (domainMismatch.length > 0) issues.push("domain_mismatch"); if (linked.length > 0 && usable.length === 0) { if (temporal === "stale") issues.push("stale"); else if (temporal === "invalid") issues.push("temporal_firewall"); if (linked.every((item) => item.status === "rejected" || item.status === "failed" || item.status === "reconciliation_required")) issues.push("evidence_not_accepted"); } if (supportScore < claim.requiredSupport) issues.push("insufficient_support"); if (contradicting.length > 0) issues.push("contradiction"); if (sources.size < requiredSources) issues.push("insufficient_independence"); if (modalShortfall) issues.push("missing_modality"); const requiresReproduction = claim.requiredReproducibility || policy.requireReproducibility; if (requiresReproduction && supporting.length > 0 && !reproduced) issues.push("unreproduced");
    let status: AutonomousClaimIntegrityStatus; if (linked.length === 0) status = "missing"; else if (contradicting.length > 0 && policy.contradictionVeto) status = supporting.length > 0 ? "conflicted" : "contradicted"; else if (usable.length === 0 && temporal === "stale") status = "stale"; else if (supporting.length === 0) status = "blocked"; else if (requiresReproduction && !reproduced) status = "unreproducible"; else if (sources.size < requiredSources) status = "insufficient_independence"; else if (modalShortfall) status = "insufficient_modalities"; else if (supportScore < claim.requiredSupport) status = "partially_supported"; else status = "supported";
    const quality = supporting.length > 0 ? Math.min(1, supportScore / Math.max(claim.requiredSupport, 1e-12)) : 0; const independence = supporting.length > 0 ? Math.min(1, sources.size / Math.max(requiredSources, 1)) : 0; const consistency = contradicting.length > 0 && policy.contradictionVeto ? 0 : 1; const modalityFactor = modalShortfall ? 0 : 1; const confidence = rounded(quality * independence * consistency * modalityFactor);
    const nextActionType = actionType(status); assessments.push({ claim_id: claim.claimId, domain: claim.domain, status, support_score: rounded(supportScore), confidence, supporting_evidence_ids: supportingIds, contradicting_evidence_ids: contradictingIds, usable_evidence_ids: usableIds, independent_source_count: sources.size, modalities, missing_modalities: missingModalities, reproducibility: reproduction, temporal_state: temporal, issues: [...new Set(issues)].sort(), next_action_type: nextActionType, priority: rounded(claim.priority) });
  }
  const claimById = new Map(claims.map((claim) => [claim.claimId, claim])); const candidates = assessments.filter((item) => item.next_action_type !== null).map((item) => { const claim = claimById.get(item.claim_id)!; return new AutonomousClaimIntegrityAction({ actionType: item.next_action_type!, domain: item.domain, claimIds: [item.claim_id], blockingEvidenceIds: [...new Set([...item.contradicting_evidence_ids, ...item.supporting_evidence_ids])].sort(), reasonCodes: item.issues, priority: rounded(Math.min(1, claim.priority + (1 - item.confidence) * 0.5)), expectedValue: rounded(Math.min(1, Math.max(0, claim.requiredSupport - item.support_score) + 0.1 * item.issues.length)) }); }); candidates.sort((left, right) => right.priority - left.priority || right.expectedValue - left.expectedValue || left.domain.localeCompare(right.domain) || left.claimIds[0]!.localeCompare(right.claimIds[0]!) || left.actionType.localeCompare(right.actionType)); const actions = candidates.slice(0, policy.maxActions); const statusCounts = Object.fromEntries(AUTONOMOUS_CLAIM_INTEGRITY_STATUSES.map((status) => [status, assessments.filter((item) => item.status === status).length]));
  const summary: JsonObject = { claim_count: assessments.length, evidence_count: { total: rows.length, usable: rows.filter((row) => row.usable).length, stale: rows.filter((row) => row.temporal_state === "stale").length, future: rows.filter((row) => row.temporal_state === "future").length, expired: rows.filter((row) => row.temporal_state === "expired").length, rejected_or_failed: rows.filter((row) => row.status === "rejected" || row.status === "failed" || row.status === "reconciliation_required").length }, status_counts: statusCounts, supported_claim_count: statusCounts.supported, action_count: actions.length, omitted_action_count: candidates.length - actions.length, domains: [...new Set(claims.map((claim) => claim.domain))].sort(), temporal_firewall: "explicit_reference_time;future_and_expired_observations_excluded", source_independence: "source_digest_or_source_id_unique_supporting_sources", contradiction_policy: policy.contradictionVeto ? "veto" : "reported_without_veto" };
  const supported = statusCounts.supported as number; const status: "ready" | "partial" | "blocked" = supported === assessments.length ? "ready" : supported === 0 ? "blocked" : "partial"; return new AutonomousClaimIntegrityAssessment({ contextDigest, referenceTime, policy, claims: assessments, evidence: rows, actions, omittedActions: candidates.length - actions.length, status, summary, priorAssessmentDigest: options.priorAssessmentDigest ?? null, generation: options.generation ?? 1 });
}

export interface ReassessAutonomousClaimIntegrityOptions { previous: AutonomousClaimIntegrityAssessment; claims: AssessAutonomousClaimIntegrityOptions["claims"]; evidence: AssessAutonomousClaimIntegrityOptions["evidence"]; referenceTime: string; policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput; }
export function reassessAutonomousClaimIntegrity(options: ReassessAutonomousClaimIntegrityOptions): AutonomousClaimIntegrityAssessment {
  validateAutonomousClaimIntegrity(options.previous); return assessAutonomousClaimIntegrity({ contextDigest: options.previous.contextDigest, claims: options.claims, evidence: options.evidence, referenceTime: options.referenceTime, policy: options.policy ?? options.previous.policy, priorAssessmentDigest: options.previous.assessmentDigest, generation: options.previous.generation + 1 });
}
export function validateAutonomousClaimIntegrity(value: AutonomousClaimIntegrityAssessment): AutonomousClaimIntegrityAssessment { if (!(value instanceof AutonomousClaimIntegrityAssessment)) fail("validation requires a typed assessment"); if (digestJsonSync(value.digestDescriptor) !== value.assessmentDigest) fail("assessment digest does not match its fields"); return value; }
export function validateAutonomousClaimIntegritySnapshot(value: Record<string, unknown>): Record<string, unknown> {
  if (!isObject(value)) fail("snapshot must be an object"); const provided = digest("snapshot assessmentDigest", value.assessment_digest, false)!; const fields = ["schema", "context_digest", "reference_time", "policy_digest", "claims", "evidence", "actions", "omitted_actions", "status", "summary", "prior_assessment_digest", "generation"] as const; if (fields.some((field) => !(field in value))) fail("snapshot is missing digest-bound fields"); const descriptor = Object.fromEntries(fields.map((field) => [field, value[field]])); if (digestJsonSync(descriptor) !== provided) fail("snapshot digest does not match its fields"); return { ...value };
}
