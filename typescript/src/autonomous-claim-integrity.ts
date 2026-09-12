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
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type { AutonomousEvidenceAcquisitionRequest } from "./autonomous-evidence-runtime.js";

export const AUTONOMOUS_CLAIM_INTEGRITY_SCHEMA = "bioprism-typescript-autonomous-claim-integrity/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-policy/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-claim/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-evidence/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-assessment/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-action/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-acquisition-bridge/0.1" as const;
export const AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BINDING_SCHEMA = "bioprism-typescript-autonomous-claim-integrity-acquisition-binding/0.1" as const;

export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIMS = 128;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE = 512;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS = 128;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIM_LINKS = 32;
export const AUTONOMOUS_CLAIM_INTEGRITY_MAX_MODALITIES = 16;
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
function timestamp(name: string, value: unknown): string {
  const candidate = text(name, value, 64);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})$/.test(candidate) || Number.isNaN(Date.parse(candidate))) fail(`${name} must be an RFC3339 timestamp`);
  return candidate;
}
function epoch(value: string): number { return Date.parse(value) / 1000; }
function safeMetadata(value: unknown, name = "metadata", depth = 0): void {
  if (depth > 8) fail(`${name} is too deeply nested`);
  if (Array.isArray(value)) { if (value.length > 128) fail(`${name} contains too many entries`); value.forEach((child, index) => safeMetadata(child, `${name}[${index}]`, depth + 1)); return; }
  if (isObject(value)) {
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
function metadataDigest(value: Readonly<Record<string, unknown>>): string { safeMetadata(value); return digestJsonSync(value); }
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
