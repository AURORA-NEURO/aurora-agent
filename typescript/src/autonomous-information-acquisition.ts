/**
 * Provider-free information-acquisition planning for the autonomous brain.
 *
 * The evidence runtime executes reviewed requests; this module decides which bounded requests
 * are worth preparing next. It ranks caller-owned candidate metadata by expected information
 * gain, uncertainty reduction, reliability, freshness, cost, latency, risk, conflict risk, and
 * domain coverage. It never invokes a provider/source and never retains task text, prompts,
 * credentials, locators, evidence values, or tool arguments.
 */
import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_INFORMATION_ACQUISITION_SCHEMA = "bioprism-typescript-autonomous-information-acquisition/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_POLICY_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-policy/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-candidate/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_SELECTION_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-selection/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_OMISSION_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-omission/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_PLAN_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-plan/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_SCHEMA = "bioprism-typescript-autonomous-information-acquisition-observation/0.1" as const;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES = 512;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED = 64;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_DEPENDENCIES = 16;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_OBSERVATIONS = 512;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS = 86_400_000;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST = 1_000_000;
export const AUTONOMOUS_INFORMATION_ACQUISITION_MAX_PLAN_BYTES = 1_000_000;
const EPSILON = 1e-12;

export type AutonomousInformationAcquisitionStatus = "ready" | "partial" | "blocked" | "empty" | "review_required";
export type AutonomousInformationAcquisitionCandidateStatus = "available" | "partial" | "stale" | "unavailable" | "requires_approval" | "conflicted";
export type AutonomousInformationAcquisitionObservationStatus = "accepted" | "partial" | "rejected" | "stale" | "failed" | "reconciliation_required";

const SECRET_MARKERS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "privatekey", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "clientsecret", "gsk", "sk"]);
const WEIGHT_NAMES = ["information_gain", "uncertainty_reduction", "reliability", "freshness", "coverage", "priority", "cost", "latency", "risk", "conflict"] as const;
type WeightName = typeof WEIGHT_NAMES[number];
const DEFAULT_WEIGHTS: Record<WeightName, number> = { information_gain: 0.30, uncertainty_reduction: 0.25, reliability: 0.15, freshness: 0.10, coverage: 0.10, priority: 0.10, cost: 0.10, latency: 0.05, risk: 0.20, conflict: 0.15 };

function fail(message: string): never { throw new ArgumentError(`autonomous information acquisition ${message}`); }
function bytes(value: string): number { return new TextEncoder().encode(value).byteLength; }
function boundedText(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) fail(`${name} must be bounded non-empty text`);
  return value.trim();
}
function identifier(name: string, value: unknown, maximum = 256): string {
  const text = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(text)) fail(`${name} contains unsupported identifier characters`);
  return text;
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
function domains(name: string, value: readonly string[] | undefined, defaultAll = false): AutonomousDomainName[] {
  const input = value === undefined ? (defaultAll ? [...AUTONOMOUS_DOMAIN_NAMES] : []) : value;
  if (!Array.isArray(input) || input.length < 1 || input.length > AUTONOMOUS_DOMAIN_NAMES.length) fail(`${name} must contain between 1 and ${AUTONOMOUS_DOMAIN_NAMES.length} domains`);
  const normalized = input.map((item, index) => identifier(`${name}[${index}]`, item, 64) as AutonomousDomainName);
  if (new Set(normalized).size !== normalized.length || normalized.some((item) => !AUTONOMOUS_DOMAIN_NAMES.includes(item))) fail(`${name} contains duplicate or unsupported domains`);
  return normalized;
}
function identifiers(name: string, value: readonly string[] | undefined, maximum: number): string[] {
  const input = value ?? [];
  if (!Array.isArray(input) || input.length > maximum) fail(`${name} is outside its bounds`);
  const normalized = input.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(normalized).size !== normalized.length) fail(`${name} contains duplicate identifiers`);
  return normalized;
}
function read(value: Record<string, unknown>, snake: string, camel: string, fallback?: unknown): unknown { return value[snake] ?? value[camel] ?? fallback; }

export interface AutonomousInformationAcquisitionPolicyInput {
  maxCost?: number;
  maxItems?: number;
  maxLatencyMs?: number;
  minScore?: number;
  minReliability?: number;
  requireDomainCoverage?: boolean;
  allowPartial?: boolean;
  allowStale?: boolean;
  allowUnavailable?: boolean;
  exploration?: number;
  coverageBonus?: number;
  weights?: Partial<Record<WeightName, number>>;
}

export class AutonomousInformationAcquisitionPolicy {
  readonly maxCost: number;
  readonly maxItems: number;
  readonly maxLatencyMs: number;
  readonly minScore: number;
  readonly minReliability: number;
  readonly requireDomainCoverage: boolean;
  readonly allowPartial: boolean;
  readonly allowStale: boolean;
  readonly allowUnavailable: boolean;
  readonly exploration: number;
  readonly coverageBonus: number;
  readonly weights: Readonly<Record<WeightName, number>>;

  constructor(input: AutonomousInformationAcquisitionPolicyInput = {}) {
    this.maxCost = finite("policy maxCost", input.maxCost ?? 1, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST);
    if (this.maxCost <= 0) fail("policy maxCost must be positive");
    this.maxItems = integer("policy maxItems", input.maxItems ?? 8, 1, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED);
    this.maxLatencyMs = integer("policy maxLatencyMs", input.maxLatencyMs ?? 300_000, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS);
    this.minScore = finite("policy minScore", input.minScore ?? 0, -10, 10);
    this.minReliability = finite("policy minReliability", input.minReliability ?? 0, 0, 1);
    for (const name of ["requireDomainCoverage", "allowPartial", "allowStale", "allowUnavailable"] as const) if (typeof input[name] !== "undefined" && typeof input[name] !== "boolean") fail(`policy ${name} must be boolean`);
    this.requireDomainCoverage = input.requireDomainCoverage ?? false;
    this.allowPartial = input.allowPartial ?? false;
    this.allowStale = input.allowStale ?? false;
    this.allowUnavailable = input.allowUnavailable ?? false;
    this.exploration = finite("policy exploration", input.exploration ?? 0.15, 0, 2);
    this.coverageBonus = finite("policy coverageBonus", input.coverageBonus ?? 0.20, 0, 2);
    const supplied = input.weights ?? {};
    if (!isObject(supplied) || Object.keys(supplied).some((key) => !(WEIGHT_NAMES as readonly string[]).includes(key))) fail("policy weights contain unsupported dimensions");
    const weights = Object.fromEntries(WEIGHT_NAMES.map((name) => [name, finite(`policy weight ${name}`, supplied[name] ?? DEFAULT_WEIGHTS[name], 0, 4)])) as Record<WeightName, number>;
    if (Object.values(weights).every((value) => value === 0)) fail("policy weights must contain a positive value");
    this.weights = weights;
  }

  private payload(): JsonObject {
    return { schema: AUTONOMOUS_INFORMATION_ACQUISITION_POLICY_SCHEMA, max_cost: rounded(this.maxCost), max_items: this.maxItems, max_latency_ms: this.maxLatencyMs, min_score: rounded(this.minScore), min_reliability: rounded(this.minReliability), require_domain_coverage: this.requireDomainCoverage, allow_partial: this.allowPartial, allow_stale: this.allowStale, allow_unavailable: this.allowUnavailable, exploration: rounded(this.exploration), coverage_bonus: rounded(this.coverageBonus), weights: Object.fromEntries(WEIGHT_NAMES.map((name) => [name, rounded(this.weights[name])])) };
  }
  get policyDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), policy_digest: this.policyDigest, execution: "provider_free_candidate_prioritization;no_source_dispatch", retention: "metadata_only;candidate_values_and_source_payloads_caller_owned", secret_material: "never_returned" }; }
}

export interface AutonomousInformationAcquisitionCandidateInput {
  candidateId: string;
  domain: AutonomousDomainName;
  capability: string;
  sourceId: string;
  informationGain: number;
  uncertaintyReduction: number;
  reliability: number;
  freshness: number;
  coverage: number;
  cost: number;
  latencyMs: number;
  risk: number;
  conflictRisk: number;
  priority?: number;
  status?: AutonomousInformationAcquisitionCandidateStatus;
  dependsOn?: readonly string[];
  sourceDigest?: string | null;
  metadata?: Readonly<Record<string, unknown>>;
}

export class AutonomousInformationAcquisitionCandidate {
  readonly candidateId: string; readonly domain: AutonomousDomainName; readonly capability: string; readonly sourceId: string;
  readonly informationGain: number; readonly uncertaintyReduction: number; readonly reliability: number; readonly freshness: number; readonly coverage: number;
  readonly cost: number; readonly latencyMs: number; readonly risk: number; readonly conflictRisk: number; readonly priority: number;
  readonly status: AutonomousInformationAcquisitionCandidateStatus; readonly dependsOn: readonly string[]; readonly sourceDigest: string | null;
  readonly metadata: Readonly<Record<string, unknown>>;

  constructor(input: AutonomousInformationAcquisitionCandidateInput) {
    this.candidateId = identifier("candidate candidateId", input.candidateId);
    this.domain = identifier("candidate domain", input.domain, 64) as AutonomousDomainName;
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(this.domain)) fail("candidate domain is unsupported");
    this.capability = identifier("candidate capability", input.capability);
    this.sourceId = identifier("candidate sourceId", input.sourceId);
    for (const [name, value] of [["informationGain", input.informationGain], ["uncertaintyReduction", input.uncertaintyReduction], ["reliability", input.reliability], ["freshness", input.freshness], ["coverage", input.coverage], ["risk", input.risk], ["conflictRisk", input.conflictRisk], ["priority", input.priority ?? 0.5]] as const) finite(`candidate ${name}`, value, 0, 1);
    this.informationGain = finite("candidate informationGain", input.informationGain, 0, 1);
    this.uncertaintyReduction = finite("candidate uncertaintyReduction", input.uncertaintyReduction, 0, 1);
    this.reliability = finite("candidate reliability", input.reliability, 0, 1);
    this.freshness = finite("candidate freshness", input.freshness, 0, 1);
    this.coverage = finite("candidate coverage", input.coverage, 0, 1);
    this.cost = finite("candidate cost", input.cost, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST); if (this.cost <= 0) fail("candidate cost must be positive");
    this.latencyMs = integer("candidate latencyMs", input.latencyMs, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS);
    this.risk = finite("candidate risk", input.risk, 0, 1); this.conflictRisk = finite("candidate conflictRisk", input.conflictRisk, 0, 1); this.priority = finite("candidate priority", input.priority ?? 0.5, 0, 1);
    this.status = input.status ?? "available"; if (!["available", "partial", "stale", "unavailable", "requires_approval", "conflicted"].includes(this.status)) fail("candidate status is unsupported");
    this.dependsOn = identifiers("candidate dependsOn", input.dependsOn, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_DEPENDENCIES); if (this.dependsOn.includes(this.candidateId)) fail("candidate cannot depend on itself");
    this.sourceDigest = digest("candidate sourceDigest", input.sourceDigest ?? null, true);
    const metadata = input.metadata ?? {}; if (!isObject(metadata)) fail("candidate metadata must be an object"); safeMetadata(metadata, "candidate metadata"); this.metadata = { ...metadata };
  }

  private payload(): JsonObject {
    return { schema: AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_SCHEMA, candidate_id: this.candidateId, domain: this.domain, capability: this.capability, source_id: this.sourceId, information_gain: rounded(this.informationGain), uncertainty_reduction: rounded(this.uncertaintyReduction), reliability: rounded(this.reliability), freshness: rounded(this.freshness), coverage: rounded(this.coverage), cost: rounded(this.cost), latency_ms: this.latencyMs, risk: rounded(this.risk), conflict_risk: rounded(this.conflictRisk), priority: rounded(this.priority), status: this.status, depends_on: [...this.dependsOn], source_digest: this.sourceDigest, metadata_digest: metadataDigest(this.metadata) };
  }
  get candidateDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): JsonObject { return { ...this.payload(), candidate_digest: this.candidateDigest, retention: "metadata_only;candidate_values_and_source_payloads_caller_owned", secret_material: "never_returned" }; }
}

export interface AutonomousInformationAcquisitionObservationInput {
  candidateId: string; status: AutonomousInformationAcquisitionObservationStatus; observedInformationGain?: number | null; observedUncertaintyReduction?: number | null; actualCost?: number | null; actualLatencyMs?: number | null; valueDigest?: string | null; evaluatorDigest?: string | null;
}
export class AutonomousInformationAcquisitionObservation {
  readonly candidateId: string; readonly status: AutonomousInformationAcquisitionObservationStatus; readonly observedInformationGain: number | null; readonly observedUncertaintyReduction: number | null; readonly actualCost: number | null; readonly actualLatencyMs: number | null; readonly valueDigest: string | null; readonly evaluatorDigest: string | null;
  constructor(input: AutonomousInformationAcquisitionObservationInput) {
    this.candidateId = identifier("observation candidateId", input.candidateId); this.status = input.status;
    if (!["accepted", "partial", "rejected", "stale", "failed", "reconciliation_required"].includes(this.status)) fail("observation status is unsupported");
    this.observedInformationGain = input.observedInformationGain == null ? null : finite("observation observedInformationGain", input.observedInformationGain, 0, 1);
    this.observedUncertaintyReduction = input.observedUncertaintyReduction == null ? null : finite("observation observedUncertaintyReduction", input.observedUncertaintyReduction, 0, 1);
    this.actualCost = input.actualCost == null ? null : finite("observation actualCost", input.actualCost, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST);
    this.actualLatencyMs = input.actualLatencyMs == null ? null : integer("observation actualLatencyMs", input.actualLatencyMs, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS);
    this.valueDigest = digest("observation valueDigest", input.valueDigest ?? null, true); this.evaluatorDigest = digest("observation evaluatorDigest", input.evaluatorDigest ?? null, true);
  }
  toJSON(): JsonObject { const payload = { schema: AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_SCHEMA, candidate_id: this.candidateId, status: this.status, observed_information_gain: this.observedInformationGain === null ? null : rounded(this.observedInformationGain), observed_uncertainty_reduction: this.observedUncertaintyReduction === null ? null : rounded(this.observedUncertaintyReduction), actual_cost: this.actualCost === null ? null : rounded(this.actualCost), actual_latency_ms: this.actualLatencyMs, value_digest: this.valueDigest, evaluator_digest: this.evaluatorDigest }; return { ...payload, observation_digest: digestJsonSync(payload), retention: "value_only_observation_metadata", secret_material: "never_returned" }; }
}

export interface AutonomousInformationAcquisitionSelection extends JsonObject { candidate_id: string; domain: AutonomousDomainName; capability: string; source_id: string; candidate_digest: string; rank: number; score: number; utility_per_cost: number; projected_information_gain: number; projected_uncertainty_reduction: number; projected_cost: number; projected_latency_ms: number; selection_reason: string; }
export interface AutonomousInformationAcquisitionOmission extends JsonObject { candidate_id: string; domain: AutonomousDomainName; candidate_digest: string; reason: string; score: number | null; }

function selectionJson(candidate: AutonomousInformationAcquisitionCandidate, rank: number, score: number, utilityPerCost: number, reason: string): AutonomousInformationAcquisitionSelection {
  return { schema: AUTONOMOUS_INFORMATION_ACQUISITION_SELECTION_SCHEMA, candidate_id: candidate.candidateId, domain: candidate.domain, capability: candidate.capability, source_id: candidate.sourceId, candidate_digest: candidate.candidateDigest, rank, score: rounded(score), utility_per_cost: rounded(utilityPerCost), projected_information_gain: rounded(candidate.informationGain), projected_uncertainty_reduction: rounded(candidate.uncertaintyReduction), projected_cost: rounded(candidate.cost), projected_latency_ms: candidate.latencyMs, selection_reason: reason, retention: "metadata_only;source_dispatch_requires_review", secret_material: "never_returned" };
}
function omissionJson(candidate: AutonomousInformationAcquisitionCandidate, reason: string, score: number | null = null): AutonomousInformationAcquisitionOmission {
  return { schema: AUTONOMOUS_INFORMATION_ACQUISITION_OMISSION_SCHEMA, candidate_id: candidate.candidateId, domain: candidate.domain, candidate_digest: candidate.candidateDigest, reason, score: score === null ? null : rounded(score), retention: "metadata_only;omitted_candidate_payload_not_retained", secret_material: "never_returned" };
}

export interface AutonomousInformationAcquisitionPlanInput { taskDigest: string; routeDigest?: string | null; requestedDomains?: readonly AutonomousDomainName[]; selected: readonly AutonomousInformationAcquisitionSelection[]; omissions: readonly AutonomousInformationAcquisitionOmission[]; policy: AutonomousInformationAcquisitionPolicy; candidateCount: number; consumedCost: number; consumedLatencyMs: number; status: AutonomousInformationAcquisitionStatus; missingDomains: readonly AutonomousDomainName[]; priorPlanDigest?: string | null; observationsDigest?: string | null; generation?: number; planDigest?: string | null; }

export class AutonomousInformationAcquisitionPlan {
  readonly taskDigest: string; readonly routeDigest: string | null; readonly requestedDomains: readonly AutonomousDomainName[]; readonly selected: readonly AutonomousInformationAcquisitionSelection[]; readonly omissions: readonly AutonomousInformationAcquisitionOmission[]; readonly policy: AutonomousInformationAcquisitionPolicy; readonly candidateCount: number; readonly consumedCost: number; readonly consumedLatencyMs: number; readonly status: AutonomousInformationAcquisitionStatus; readonly missingDomains: readonly AutonomousDomainName[]; readonly priorPlanDigest: string | null; readonly observationsDigest: string | null; readonly generation: number; readonly planDigest: string;
  constructor(input: AutonomousInformationAcquisitionPlanInput) {
    this.taskDigest = digest("plan taskDigest", input.taskDigest)!; this.routeDigest = digest("plan routeDigest", input.routeDigest ?? null, true); this.requestedDomains = domains("plan requestedDomains", input.requestedDomains);
    this.selected = [...input.selected]; this.omissions = [...input.omissions]; if (this.selected.length > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED || this.selected.some((item) => !isObject(item))) fail("plan selections are malformed");
    if (new Set(this.selected.map((item) => item.candidate_id)).size !== this.selected.length || this.selected.some((item, index) => item.rank !== index + 1)) fail("plan selection ids or ranks are invalid");
    if (this.omissions.length > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES || this.omissions.some((item) => !isObject(item))) fail("plan omissions are malformed");
    if (new Set(this.omissions.map((item) => item.candidate_id)).size !== this.omissions.length || this.selected.some((item) => this.omissions.some((omission) => omission.candidate_id === item.candidate_id))) fail("plan selected and omitted ids overlap");
    this.policy = input.policy instanceof AutonomousInformationAcquisitionPolicy ? input.policy : new AutonomousInformationAcquisitionPolicy(input.policy as unknown as AutonomousInformationAcquisitionPolicyInput);
    this.candidateCount = integer("plan candidateCount", input.candidateCount, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES); if (this.candidateCount !== this.selected.length + this.omissions.length) fail("plan candidateCount does not reconcile");
    this.consumedCost = finite("plan consumedCost", input.consumedCost, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST); if (this.consumedCost > this.policy.maxCost + 1e-8) fail("plan consumedCost exceeds policy");
    this.consumedLatencyMs = integer("plan consumedLatencyMs", input.consumedLatencyMs, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS * AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED);
    this.status = input.status; if (!["ready", "partial", "blocked", "empty", "review_required"].includes(this.status)) fail("plan status is unsupported");
    this.missingDomains = input.missingDomains.length === 0 ? [] : domains("plan missingDomains", input.missingDomains); if (this.missingDomains.some((domain) => !this.requestedDomains.includes(domain))) fail("plan missingDomains is outside requestedDomains");
    this.priorPlanDigest = digest("plan priorPlanDigest", input.priorPlanDigest ?? null, true); this.observationsDigest = digest("plan observationsDigest", input.observationsDigest ?? null, true); this.generation = integer("plan generation", input.generation ?? 1, 1, 2_147_483_647);
    const expected = digestJsonSync(this.payload()); if (input.planDigest !== undefined && input.planDigest !== null && input.planDigest !== expected) fail("plan digest does not match its fields"); this.planDigest = expected;
  }
  private payload(): JsonObject { return { schema: AUTONOMOUS_INFORMATION_ACQUISITION_PLAN_SCHEMA, task_digest: this.taskDigest, route_digest: this.routeDigest, requested_domains: [...this.requestedDomains], selected: [...this.selected], omissions: [...this.omissions], policy_digest: this.policy.policyDigest, candidate_count: this.candidateCount, consumed_cost: rounded(this.consumedCost), consumed_latency_ms: this.consumedLatencyMs, status: this.status, missing_domains: [...this.missingDomains], prior_plan_digest: this.priorPlanDigest, observations_digest: this.observationsDigest, generation: this.generation }; }
  get selectedDomains(): readonly AutonomousDomainName[] { const selected = new Set(this.selected.map((item) => item.domain)); return this.requestedDomains.filter((domain) => selected.has(domain)); }
  get coverageRatio(): number { return this.requestedDomains.length === 0 ? 0 : this.selectedDomains.length / this.requestedDomains.length; }
  toJSON(): JsonObject { return { ...this.payload(), plan_digest: this.planDigest, policy: this.policy.toJSON(), selected_domains: [...this.selectedDomains], coverage_ratio: rounded(this.coverageRatio), remaining_cost: rounded(Math.max(0, this.policy.maxCost - this.consumedCost)), remaining_items: this.policy.maxItems - this.selected.length, execution: "planning_only;source_dispatch_requires_reviewed_evidence_boundary", retention: "metadata_only;task_text_prompts_source_values_credentials_and_locators_caller_owned", secret_material: "never_returned" }; }
  withReplan(priorPlanDigest: string, observations: readonly AutonomousInformationAcquisitionObservation[], generation = this.generation + 1): AutonomousInformationAcquisitionPlan {
    return new AutonomousInformationAcquisitionPlan({ taskDigest: this.taskDigest, routeDigest: this.routeDigest, requestedDomains: this.requestedDomains, selected: this.selected, omissions: this.omissions, policy: this.policy, candidateCount: this.candidateCount, consumedCost: this.consumedCost, consumedLatencyMs: this.consumedLatencyMs, status: this.status, missingDomains: this.missingDomains, priorPlanDigest, observationsDigest: digestJsonSync(observations.map((item) => item.toJSON())), generation });
  }
}

function normalizePolicy(value: AutonomousInformationAcquisitionPolicy | AutonomousInformationAcquisitionPolicyInput | undefined): AutonomousInformationAcquisitionPolicy { return value instanceof AutonomousInformationAcquisitionPolicy ? value : new AutonomousInformationAcquisitionPolicy(value); }
function normalizeCandidate(value: AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>): AutonomousInformationAcquisitionCandidate {
  if (value instanceof AutonomousInformationAcquisitionCandidate) return value;
  if ("candidateId" in value && "informationGain" in value) return new AutonomousInformationAcquisitionCandidate(value as AutonomousInformationAcquisitionCandidateInput);
  return new AutonomousInformationAcquisitionCandidate({ candidateId: read(value, "candidate_id", "candidateId") as string, domain: read(value, "domain", "domain") as AutonomousDomainName, capability: read(value, "capability", "capability") as string, sourceId: read(value, "source_id", "sourceId") as string, informationGain: read(value, "information_gain", "informationGain") as number, uncertaintyReduction: read(value, "uncertainty_reduction", "uncertaintyReduction") as number, reliability: read(value, "reliability", "reliability") as number, freshness: read(value, "freshness", "freshness") as number, coverage: read(value, "coverage", "coverage") as number, cost: read(value, "cost", "cost") as number, latencyMs: read(value, "latency_ms", "latencyMs") as number, risk: read(value, "risk", "risk") as number, conflictRisk: read(value, "conflict_risk", "conflictRisk") as number, priority: read(value, "priority", "priority", 0.5) as number, status: read(value, "status", "status", "available") as AutonomousInformationAcquisitionCandidateStatus, dependsOn: (read(value, "depends_on", "dependsOn", []) as string[]) ?? [], sourceDigest: read(value, "source_digest", "sourceDigest", null) as string | null, metadata: (read(value, "metadata", "metadata", {}) as Record<string, unknown>) ?? {} });
}
function normalizeCandidates(values: readonly (AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>)[]): AutonomousInformationAcquisitionCandidate[] { if (!Array.isArray(values) || values.length < 1 || values.length > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES) fail("candidates are outside their bounds"); const normalized = values.map(normalizeCandidate); if (new Set(normalized.map((candidate) => candidate.candidateId)).size !== normalized.length) fail("candidates contain duplicate ids"); return normalized; }
function normalizeObservation(value: AutonomousInformationAcquisitionObservation | Record<string, unknown>): AutonomousInformationAcquisitionObservation {
  if (value instanceof AutonomousInformationAcquisitionObservation) return value;
  return new AutonomousInformationAcquisitionObservation({ candidateId: read(value, "candidate_id", "candidateId") as string, status: read(value, "status", "status") as AutonomousInformationAcquisitionObservationStatus, observedInformationGain: read(value, "observed_information_gain", "observedInformationGain", null) as number | null, observedUncertaintyReduction: read(value, "observed_uncertainty_reduction", "observedUncertaintyReduction", null) as number | null, actualCost: read(value, "actual_cost", "actualCost", null) as number | null, actualLatencyMs: read(value, "actual_latency_ms", "actualLatencyMs", null) as number | null, valueDigest: read(value, "value_digest", "valueDigest", null) as string | null, evaluatorDigest: read(value, "evaluator_digest", "evaluatorDigest", null) as string | null });
}
function normalizeObservations(values: readonly (AutonomousInformationAcquisitionObservation | Record<string, unknown>)[]): AutonomousInformationAcquisitionObservation[] { if (!Array.isArray(values) || values.length > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_OBSERVATIONS) fail("observations are outside their bounds"); const normalized = values.map(normalizeObservation); if (new Set(normalized.map((item) => item.candidateId)).size !== normalized.length) fail("observations contain duplicate candidate ids"); return normalized; }
function score(candidate: AutonomousInformationAcquisitionCandidate, policy: AutonomousInformationAcquisitionPolicy, domainMissing: boolean, observationCount: number): [number, number] {
  const w = policy.weights; const latencyRatio = policy.maxLatencyMs === 0 ? (candidate.latencyMs > 0 ? 1 : 0) : Math.min(1, candidate.latencyMs / policy.maxLatencyMs); const costRatio = Math.min(1, candidate.cost / policy.maxCost);
  const value = w.information_gain * candidate.informationGain + w.uncertainty_reduction * candidate.uncertaintyReduction + w.reliability * candidate.reliability + w.freshness * candidate.freshness + w.coverage * candidate.coverage + w.priority * candidate.priority;
  const penalties = w.cost * costRatio + w.latency * latencyRatio + w.risk * candidate.risk + w.conflict * candidate.conflictRisk;
  const exploration = (policy.exploration / Math.sqrt(1 + observationCount)) * (observationCount === 0 ? 1 - 0.5 * candidate.reliability : 1);
  const valueScore = rounded(value + (domainMissing ? policy.coverageBonus : 0) + exploration - penalties);
  return [valueScore, rounded(valueScore / Math.max(candidate.cost, EPSILON))];
}

export interface PlanAutonomousInformationAcquisitionOptions { taskDigest: string; routeDigest?: string | null; candidates: readonly (AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>)[]; requestedDomains?: readonly AutonomousDomainName[]; policy?: AutonomousInformationAcquisitionPolicy | AutonomousInformationAcquisitionPolicyInput; satisfiedCandidateIds?: readonly string[]; }
export function planAutonomousInformationAcquisition(options: PlanAutonomousInformationAcquisitionOptions): AutonomousInformationAcquisitionPlan {
  const taskDigest = digest("taskDigest", options.taskDigest)!; const routeDigest = digest("routeDigest", options.routeDigest ?? null, true); const candidates = normalizeCandidates(options.candidates); const policy = normalizePolicy(options.policy); const requested = domains("requestedDomains", options.requestedDomains, options.requestedDomains === undefined); const satisfied = new Set(identifiers("satisfiedCandidateIds", options.satisfiedCandidateIds, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES)); const byId = new Map(candidates.map((candidate) => [candidate.candidateId, candidate]));
  const selectedIds = new Set<string>(); const selectedDomains = new Set<AutonomousDomainName>(); const selected: AutonomousInformationAcquisitionSelection[] = []; const omissions = new Map<string, AutonomousInformationAcquisitionOmission>(); const observationCounts = new Map<string, number>(); let consumedCost = 0; let consumedLatencyMs = 0; const remaining = new Set(candidates.map((candidate) => candidate.candidateId));
  const eligible = (candidate: AutonomousInformationAcquisitionCandidate): string | null => { if (!requested.includes(candidate.domain)) return "domain_not_requested"; if (candidate.reliability < policy.minReliability) return "below_reliability_floor"; if (candidate.latencyMs > policy.maxLatencyMs) return "latency_budget_exceeded"; if (candidate.status === "partial" && !policy.allowPartial) return "partial_not_allowed"; if (candidate.status === "stale" && !policy.allowStale) return "stale_not_allowed"; if (candidate.status === "unavailable" && !policy.allowUnavailable) return "unavailable"; if (candidate.status === "requires_approval" || candidate.status === "conflicted") return "approval_or_conflict_review_required"; if (candidate.dependsOn.some((dependency) => !satisfied.has(dependency) && !selectedIds.has(dependency))) return "dependency_unavailable"; return null; };
  while (remaining.size > 0 && selected.length < policy.maxItems) {
    const rows: { ratio: number; score: number; candidate: AutonomousInformationAcquisitionCandidate }[] = [];
    for (const id of [...remaining].sort()) { const candidate = byId.get(id)!; const reason = eligible(candidate); if (reason !== null) { omissions.set(id, omissionJson(candidate, reason)); continue; } const [candidateScore, ratio] = score(candidate, policy, !selectedDomains.has(candidate.domain), observationCounts.get(id) ?? 0); rows.push({ ratio, score: candidateScore, candidate }); }
    if (rows.length === 0) break;
    rows.sort((left, right) => right.ratio - left.ratio || right.score - left.score || left.candidate.domain.localeCompare(right.candidate.domain) || left.candidate.candidateId.localeCompare(right.candidate.candidateId));
    const row = rows[0]!;
    if (row.score < policy.minScore) { for (const item of rows) omissions.set(item.candidate.candidateId, omissionJson(item.candidate, "below_score_floor", item.score)); break; }
    if (consumedCost + row.candidate.cost > policy.maxCost + 1e-8) { omissions.set(row.candidate.candidateId, omissionJson(row.candidate, "budget_exceeded", row.score)); remaining.delete(row.candidate.candidateId); continue; }
    omissions.delete(row.candidate.candidateId); selected.push(selectionJson(row.candidate, selected.length + 1, row.score, row.ratio, selectedDomains.has(row.candidate.domain) ? "utility_per_cost" : "domain_coverage_priority")); selectedIds.add(row.candidate.candidateId); selectedDomains.add(row.candidate.domain); consumedCost += row.candidate.cost; consumedLatencyMs += row.candidate.latencyMs; remaining.delete(row.candidate.candidateId);
  }
  for (const candidate of candidates) if (!selectedIds.has(candidate.candidateId) && !omissions.has(candidate.candidateId)) omissions.set(candidate.candidateId, omissionJson(candidate, selected.length >= policy.maxItems ? "max_items_reached" : "dependency_unavailable"));
  const missing = requested.filter((domain) => !selectedDomains.has(domain)); const omitted = candidates.filter((candidate) => omissions.has(candidate.candidateId)).map((candidate) => omissions.get(candidate.candidateId)!);
  const status: AutonomousInformationAcquisitionStatus = selected.length === 0 ? (omitted.length > 0 ? "blocked" : "empty") : policy.requireDomainCoverage && missing.length > 0 ? "partial" : omitted.some((item) => ["budget_exceeded", "below_score_floor", "max_items_reached"].includes(item.reason)) ? "partial" : "ready";
  const plan = new AutonomousInformationAcquisitionPlan({ taskDigest, routeDigest, requestedDomains: requested, selected, omissions: omitted, policy, candidateCount: candidates.length, consumedCost, consumedLatencyMs, status, missingDomains: missing });
  if (bytes(JSON.stringify(plan.toJSON())) > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_PLAN_BYTES) fail("plan exceeds its byte bound");
  return plan;
}

export interface ReplanAutonomousInformationAcquisitionOptions { previousPlan: AutonomousInformationAcquisitionPlan; candidates: readonly (AutonomousInformationAcquisitionCandidate | AutonomousInformationAcquisitionCandidateInput | Record<string, unknown>)[]; observations: readonly (AutonomousInformationAcquisitionObservation | Record<string, unknown>)[]; policy?: AutonomousInformationAcquisitionPolicy | AutonomousInformationAcquisitionPolicyInput; satisfiedCandidateIds?: readonly string[]; }
export function replanAutonomousInformationAcquisition(options: ReplanAutonomousInformationAcquisitionOptions): AutonomousInformationAcquisitionPlan {
  if (!(options.previousPlan instanceof AutonomousInformationAcquisitionPlan)) fail("replan requires a typed previous plan");
  const candidates = normalizeCandidates(options.candidates); const observations = normalizeObservations(options.observations); const byId = new Map(candidates.map((candidate) => [candidate.candidateId, candidate]));
  for (const selection of options.previousPlan.selected) { const current = byId.get(selection.candidate_id); if (current !== undefined && current.candidateDigest !== selection.candidate_digest) fail(`candidate ${selection.candidate_id} changed since the previous plan`); }
  const observed = new Map(observations.map((item) => [item.candidateId, item]));
  const adjusted = candidates.map((candidate) => { const item = observed.get(candidate.candidateId); if (item === undefined) return candidate; if (["rejected", "failed", "reconciliation_required"].includes(item.status)) return new AutonomousInformationAcquisitionCandidate({ ...candidate, status: "unavailable", reliability: Math.max(0, candidate.reliability * 0.5) }); return new AutonomousInformationAcquisitionCandidate({ ...candidate, informationGain: item.observedInformationGain ?? candidate.informationGain, uncertaintyReduction: item.observedUncertaintyReduction ?? candidate.uncertaintyReduction, reliability: Math.min(1, Math.max(0, candidate.reliability + (item.status === "accepted" ? 0.10 : -0.05))), status: item.status === "accepted" ? "available" : "partial" }); });
  return planAutonomousInformationAcquisition({ taskDigest: options.previousPlan.taskDigest, routeDigest: options.previousPlan.routeDigest, candidates: adjusted, requestedDomains: options.previousPlan.requestedDomains, policy: options.policy ?? options.previousPlan.policy, satisfiedCandidateIds: options.satisfiedCandidateIds }).withReplan(options.previousPlan.planDigest, observations, options.previousPlan.generation + 1);
}

export function validateAutonomousInformationAcquisitionPlan(value: AutonomousInformationAcquisitionPlan): AutonomousInformationAcquisitionPlan {
  if (!(value instanceof AutonomousInformationAcquisitionPlan)) fail("plan validation requires a typed plan");
  return new AutonomousInformationAcquisitionPlan({ taskDigest: value.taskDigest, routeDigest: value.routeDigest, requestedDomains: value.requestedDomains, selected: value.selected, omissions: value.omissions, policy: value.policy, candidateCount: value.candidateCount, consumedCost: value.consumedCost, consumedLatencyMs: value.consumedLatencyMs, status: value.status, missingDomains: value.missingDomains, priorPlanDigest: value.priorPlanDigest, observationsDigest: value.observationsDigest, generation: value.generation, planDigest: value.planDigest });
}
