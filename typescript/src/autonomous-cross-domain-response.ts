import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  evaluateAutonomousDomainResponse,
  validateAutonomousDomainResponse,
} from "./autonomous-domain-response.js";
import type {
  AutonomousDomainResponse,
  AutonomousDomainResponseContract,
  AutonomousDomainResponseEvaluation,
} from "./autonomous-domain-response.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

const RETENTION = "metadata_only;responses_prompts_credentials_and_provider_values_not_retained" as const;
const AUTHORITY = "structural_and_caller_alignment_metadata_only;not_external_truth" as const;

/** Digest-bound integrity and alignment gate for specialist responses before synthesis. */
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA = "bioprism-typescript-autonomous-cross-domain-response/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA = "bioprism-typescript-autonomous-cross-domain-response-row/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-response-alignment/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES = ["ready_to_synthesize", "needs_alignment_review", "partial", "blocked", "completed"] as const;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES = ["specialist", "synthesis"] as const;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES = ["support", "contradict", "neutral", "unresolved"] as const;
export const MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS = 128;
export const MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS = 32;
export const MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_REASONS = 32;
export const MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES = 512_000;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_REWARD = 0.8;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_ALIGNMENT_CONFIDENCE = 0.75;
export const AUTONOMOUS_CROSS_DOMAIN_RESPONSE_CONTRADICTION_CONFIDENCE = 0.75;

export type AutonomousCrossDomainResponseStatus = typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES[number];
export type AutonomousCrossDomainResponseRole = typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES[number];
export type AutonomousCrossDomainResponseAlignmentStance = typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES[number];

export interface AutonomousCrossDomainResponseEntry {
  domain: AutonomousDomainName;
  contract: AutonomousDomainResponseContract;
  /** Transient provider value; it is validated and never copied into the assessment projection. */
  response: unknown;
  role: AutonomousCrossDomainResponseRole;
}

export interface AutonomousCrossDomainResponseAlignmentInput {
  alignment_id: string;
  left_domain: AutonomousDomainName;
  right_domain: AutonomousDomainName;
  stance: AutonomousCrossDomainResponseAlignmentStance;
  confidence: number;
  topic_digest: string;
  rationale_digest: string | null;
  left_response_digest: string;
  right_response_digest: string;
}

export interface AutonomousCrossDomainResponseAlignment extends JsonObject {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA;
  alignment_id: string;
  left_domain: AutonomousDomainName;
  right_domain: AutonomousDomainName;
  stance: AutonomousCrossDomainResponseAlignmentStance;
  confidence: number;
  topic_digest: string;
  rationale_digest: string | null;
  left_response_digest: string;
  right_response_digest: string;
  alignment_digest: string;
}

export interface AutonomousCrossDomainResponseRow extends JsonObject {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA;
  domain: AutonomousDomainName;
  role: AutonomousCrossDomainResponseRole;
  workflow_id: string;
  contract_digest: string;
  response_digest: string;
  evaluation_digest: string;
  response_status: string;
  reward: number;
  passed: boolean;
  missing_signals: string[];
  signals: Record<string, number>;
  stage_status_counts: Record<string, number>;
  domain_detail_coverage: number;
  uncertainty_count: number;
  evidence_gap_count: number;
  next_action_count: number;
  answer_digest: string;
  row_digest: string;
}

export interface AutonomousCrossDomainResponseAssessment extends JsonObject {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA;
  context_digest: string | null;
  requested_domains: AutonomousDomainName[];
  specialist_domains: AutonomousDomainName[];
  present_domains: AutonomousDomainName[];
  missing_domains: AutonomousDomainName[];
  unexpected_domains: AutonomousDomainName[];
  rows: AutonomousCrossDomainResponseRow[];
  alignments: AutonomousCrossDomainResponseAlignment[];
  alignment_pairs_expected: number;
  alignment_pairs_observed: number;
  missing_alignment_pairs: string[];
  contradictory_alignment_ids: string[];
  unresolved_alignment_ids: string[];
  low_confidence_alignment_ids: string[];
  synthesis_domain_present: boolean;
  synthesis_response_digest: string | null;
  synthesis_evaluation_digest: string | null;
  require_synthesis: boolean;
  require_complete_alignment: boolean;
  minimum_reward: number;
  minimum_alignment_confidence: number;
  contradiction_confidence_threshold: number;
  status: AutonomousCrossDomainResponseStatus;
  ready_to_synthesize: boolean;
  gate_reasons: string[];
  next_actions: string[];
  retention: typeof RETENTION;
  evaluator_authority: typeof AUTHORITY;
  secret_material: "never_returned";
  assessment_digest: string;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} is not a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown): string {
  const text = boundedText(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(text)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return text;
}

function optionalDigest(name: string, value: unknown): string | null {
  return value === null ? null : boundedDigest(name, value);
}

function fraction(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be a finite fraction between zero and one`);
  return Number(value.toFixed(12));
}

function boundedStrings(name: string, value: unknown, maximum = MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bounded sequence contract`);
  const result = value.map((item) => boundedText(`${name} entry`, item, 1_024));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function exactKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const allowedSet = new Set(allowed);
  if (Object.keys(value).length !== allowed.length || Object.keys(value).some((key) => !allowedSet.has(key))) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function assertSafeMetadata(value: unknown, name = "cross-domain response assessment", depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((item, index) => assertSafeMetadata(item, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (normalized === "secretmaterial" && child === "never_returned") {
        assertSafeMetadata(child, `${name}.${key}`, depth + 1);
        continue;
      }
      if (["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret"].includes(normalized) || ["token", "secret", "credential"].some((marker) => normalized.includes(marker))) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertSafeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function canonicalDomains(name: string, value: unknown, minimum = 2): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES) throw new ArgumentError(`${name} is outside its domain bound`);
  const result = value.map((item) => boundedText(`${name} entry`, item, 64) as AutonomousDomainName);
  if (result.some((domain) => !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain))) throw new ArgumentError(`${name} contains an unsupported domain`);
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate domains`);
  return [...result].sort((left, right) => AUTONOMOUS_DOMAIN_NAMES.indexOf(left) - AUTONOMOUS_DOMAIN_NAMES.indexOf(right));
}

function canonicalDomainOrder(domain: AutonomousDomainName): number {
  const index = AUTONOMOUS_DOMAIN_NAMES.indexOf(domain);
  if (index < 0) throw new ArgumentError(`unsupported domain ${domain}`);
  return index;
}

function pair(left: AutonomousDomainName, right: AutonomousDomainName): string {
  return `${left}::${right}`;
}

function rowDescriptor(row: AutonomousCrossDomainResponseRow): JsonObject {
  const { row_digest: _rowDigest, ...descriptor } = row;
  return descriptor;
}

function alignmentDescriptor(alignment: AutonomousCrossDomainResponseAlignment): JsonObject {
  const { alignment_digest: _alignmentDigest, ...descriptor } = alignment;
  return descriptor;
}

function assessmentDescriptor(value: AutonomousCrossDomainResponseAssessment): JsonObject {
  const { assessment_digest: _assessmentDigest, ...descriptor } = value;
  return descriptor;
}

function normalizeEntry(value: unknown): { domain: AutonomousDomainName; role: AutonomousCrossDomainResponseRole; contract: AutonomousDomainResponseContract; response: AutonomousDomainResponse; evaluation: AutonomousDomainResponseEvaluation } {
  if (!isObject(value)) throw new ArgumentError("cross-domain response entries must be objects");
  exactKeys("cross-domain response entry", value, ["domain", "contract", "response", "role"]);
  const domain = boundedText("cross-domain response entry domain", value.domain, 64) as AutonomousDomainName;
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError("cross-domain response entry domain is unsupported");
  const role = boundedText("cross-domain response entry role", value.role, 32) as AutonomousCrossDomainResponseRole;
  if (!AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES.includes(role)) throw new ArgumentError("cross-domain response entry role is unsupported");
  if (!isObject(value.contract) || typeof value.contract.domain !== "string" || value.contract.domain !== domain) throw new ArgumentError("cross-domain response entry contract does not match its domain");
  const contract = value.contract as unknown as AutonomousDomainResponseContract;
  const response = validateAutonomousDomainResponse(value.response, contract);
  const evaluation = evaluateAutonomousDomainResponse(response, contract);
  if (domain === "cross_domain" && role !== "synthesis") throw new ArgumentError("cross_domain response entries must use the synthesis role");
  if (domain !== "cross_domain" && role !== "specialist") throw new ArgumentError("non-cross-domain response entries must use the specialist role");
  return { domain, role, contract, response, evaluation };
}

function responseRow(item: ReturnType<typeof normalizeEntry>): AutonomousCrossDomainResponseRow {
  const stageStatusCounts: Record<string, number> = { complete: 0, partial: 0, blocked: 0, not_attempted: 0 };
  for (const stage of item.response.stages) stageStatusCounts[stage.status] = (stageStatusCounts[stage.status] ?? 0) + 1;
  const signals = Object.fromEntries(Object.entries(item.evaluation.signals).map(([key, value]) => [key, Number(value.toFixed(12))]));
  const descriptor = {
    schema: AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA,
    domain: item.domain,
    role: item.role,
    workflow_id: item.contract.workflow_id,
    contract_digest: item.contract.contract_digest,
    response_digest: item.evaluation.response_digest,
    evaluation_digest: item.evaluation.evaluation_digest,
    response_status: item.response.status,
    reward: item.evaluation.reward,
    passed: item.evaluation.passed,
    missing_signals: [...item.evaluation.missing_signals],
    signals,
    stage_status_counts: stageStatusCounts,
    domain_detail_coverage: signals.domain_detail_coverage ?? 0,
    uncertainty_count: item.response.uncertainty.length,
    evidence_gap_count: item.response.evidence_gaps.length,
    next_action_count: item.response.next_actions.length,
    answer_digest: digestJsonSync({ answer: item.response.answer }),
  } satisfies Omit<AutonomousCrossDomainResponseRow, "row_digest">;
  return { ...descriptor, row_digest: digestJsonSync(descriptor) };
}

function normalizeAlignment(value: unknown, rows: ReadonlyMap<AutonomousDomainName, AutonomousCrossDomainResponseRow>): AutonomousCrossDomainResponseAlignment {
  if (!isObject(value)) throw new ArgumentError("cross-domain alignments must be objects");
  const alignmentKeys = ["alignment_id", "left_domain", "right_domain", "stance", "confidence", "topic_digest", "rationale_digest", "left_response_digest", "right_response_digest"] as const;
  const input = "schema" in value || "alignment_digest" in value ? (() => {
    if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA) throw new ArgumentError("cross-domain alignment schema is invalid");
    const { schema: _schema, alignment_digest: _alignmentDigest, ...withoutDigests } = value;
    return withoutDigests;
  })() : value;
  exactKeys("cross-domain alignment", input, alignmentKeys);
  const alignmentId = boundedIdentifier("cross-domain alignment id", input.alignment_id);
  let left = boundedText("cross-domain alignment left domain", input.left_domain, 64) as AutonomousDomainName;
  let right = boundedText("cross-domain alignment right domain", input.right_domain, 64) as AutonomousDomainName;
  if (left === right || !rows.has(left) || !rows.has(right)) throw new ArgumentError("cross-domain alignment domains must be distinct response rows");
  let leftDigest = boundedDigest("cross-domain alignment left response digest", input.left_response_digest);
  let rightDigest = boundedDigest("cross-domain alignment right response digest", input.right_response_digest);
  if (canonicalDomainOrder(left) > canonicalDomainOrder(right)) {
    [left, right] = [right, left];
    [leftDigest, rightDigest] = [rightDigest, leftDigest];
  }
  if (leftDigest !== rows.get(left)?.response_digest || rightDigest !== rows.get(right)?.response_digest) throw new ArgumentError("cross-domain alignment response digests do not match the reviewed rows");
  const stance = boundedText("cross-domain alignment stance", input.stance, 32) as AutonomousCrossDomainResponseAlignmentStance;
  if (!AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES.includes(stance)) throw new ArgumentError("cross-domain alignment stance is unsupported");
  const descriptor = {
    schema: AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA,
    alignment_id: alignmentId,
    left_domain: left,
    right_domain: right,
    stance,
    confidence: fraction("cross-domain alignment confidence", input.confidence),
    topic_digest: boundedDigest("cross-domain alignment topic digest", input.topic_digest),
    rationale_digest: optionalDigest("cross-domain alignment rationale digest", input.rationale_digest),
    left_response_digest: leftDigest,
    right_response_digest: rightDigest,
  } satisfies Omit<AutonomousCrossDomainResponseAlignment, "alignment_digest">;
  return { ...descriptor, alignment_digest: digestJsonSync(descriptor) };
}

function gateActions(input: { missing: readonly string[]; blocked: boolean; weak: boolean; missingPairs: readonly string[]; contradictions: readonly string[]; unresolved: readonly string[]; lowConfidence: readonly string[]; synthesisMissing: boolean; completed: boolean }): string[] {
  const actions: string[] = [];
  if (input.missing.length) actions.push("acquire_missing_domain_responses");
  if (input.blocked) actions.push("review_blocked_domain_response");
  if (input.weak) actions.push("repair_domain_response_integrity");
  if (input.missingPairs.length) actions.push("perform_pairwise_cross_domain_alignment");
  if (input.contradictions.length) actions.push("resolve_cross_domain_contradiction");
  if (input.unresolved.length) actions.push("review_unresolved_cross_domain_alignment");
  if (input.lowConfidence.length) actions.push("review_low_confidence_cross_domain_alignment");
  if (input.synthesisMissing) actions.push("run_cross_domain_synthesis");
  if (input.completed) return [];
  if (!actions.length) actions.push("review_cross_domain_synthesis_gate");
  return actions.slice(0, MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS);
}

/** Assess a transient set of structured specialist responses before synthesis. */
export function assessAutonomousCrossDomainResponseSet(
  responses: readonly AutonomousCrossDomainResponseEntry[],
  options: {
    requestedDomains?: readonly AutonomousDomainName[];
    contextDigest?: string | null;
    alignments?: readonly AutonomousCrossDomainResponseAlignmentInput[];
    requireSynthesis?: boolean;
    requireCompleteAlignment?: boolean;
    minimumReward?: number;
    minimumAlignmentConfidence?: number;
    contradictionConfidenceThreshold?: number;
  } = {},
): AutonomousCrossDomainResponseAssessment {
  if (!Array.isArray(responses) || responses.length < 1 || responses.length > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES) throw new ArgumentError("cross-domain response entries are outside their bound");
  const requireSynthesis = options.requireSynthesis ?? false;
  const requireCompleteAlignment = options.requireCompleteAlignment ?? true;
  if (typeof requireSynthesis !== "boolean" || typeof requireCompleteAlignment !== "boolean") throw new ArgumentError("cross-domain response gate controls must be booleans");
  const minimumReward = fraction("cross-domain minimum reward", options.minimumReward ?? AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_REWARD);
  const minimumAlignmentConfidence = fraction("cross-domain minimum alignment confidence", options.minimumAlignmentConfidence ?? AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_ALIGNMENT_CONFIDENCE);
  const contradictionConfidenceThreshold = fraction("cross-domain contradiction confidence threshold", options.contradictionConfidenceThreshold ?? AUTONOMOUS_CROSS_DOMAIN_RESPONSE_CONTRADICTION_CONFIDENCE);
  const contextDigest = options.contextDigest === undefined || options.contextDigest === null ? null : boundedDigest("cross-domain response context digest", options.contextDigest);
  const items = responses.map(normalizeEntry);
  const domains = new Set<AutonomousDomainName>();
  const rows: AutonomousCrossDomainResponseRow[] = [];
  for (const item of items) {
    if (domains.has(item.domain)) throw new ArgumentError(`cross-domain response domain ${item.domain} is duplicated`);
    domains.add(item.domain);
    rows.push(responseRow(item));
  }
  rows.sort((left, right) => canonicalDomainOrder(left.domain) - canonicalDomainOrder(right.domain));
  const rowMap = new Map(rows.map((row) => [row.domain, row]));
  const requested = options.requestedDomains === undefined
    ? rows.filter((row) => row.domain !== "cross_domain").map((row) => row.domain)
    : canonicalDomains("cross-domain requested domains", options.requestedDomains);
  const specialists = requested.filter((domain) => domain !== "cross_domain");
  if (specialists.length < 2) throw new ArgumentError("cross-domain response assessment requires at least two specialist domains");
  const present = rows.map((row) => row.domain);
  const missing = requested.filter((domain) => !rowMap.has(domain));
  const unexpected = present.filter((domain) => !requested.includes(domain) && domain !== "cross_domain");
  if (unexpected.length) throw new ArgumentError("cross-domain response entries include domains outside the requested review set");
  const rawAlignments = options.alignments ?? [];
  if (!Array.isArray(rawAlignments) || rawAlignments.length > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS) throw new ArgumentError("cross-domain alignments are outside their bound");
  const alignments = rawAlignments.map((value) => normalizeAlignment(value, rowMap));
  const alignmentIds = new Set<string>();
  for (const alignment of alignments) {
    if (alignmentIds.has(alignment.alignment_id)) throw new ArgumentError("cross-domain alignment ids must be unique");
    alignmentIds.add(alignment.alignment_id);
  }
  alignments.sort((left, right) => left.alignment_id.localeCompare(right.alignment_id));
  const presentSpecialists = specialists.filter((domain) => rowMap.has(domain));
  const expectedPairs = presentSpecialists.length * (presentSpecialists.length - 1) / 2;
  const observedPairSet = new Set(alignments.filter((alignment) => presentSpecialists.some((domain) => domain === alignment.left_domain) && presentSpecialists.some((domain) => domain === alignment.right_domain)).map((alignment) => pair(alignment.left_domain, alignment.right_domain)));
  const allPairs = new Set<string>();
  presentSpecialists.forEach((left, index) => presentSpecialists.slice(index + 1).forEach((right) => allPairs.add(pair(left, right))));
  const missingPairs = requireCompleteAlignment ? [...allPairs].filter((candidate) => !observedPairSet.has(candidate)).sort() : [];
  const contradictions = alignments.filter((alignment) => alignment.stance === "contradict" && alignment.confidence >= contradictionConfidenceThreshold).map((alignment) => alignment.alignment_id).sort();
  const unresolved = alignments.filter((alignment) => alignment.stance === "unresolved" && alignment.confidence >= minimumAlignmentConfidence).map((alignment) => alignment.alignment_id).sort();
  const lowConfidence = alignments.filter((alignment) => alignment.confidence < minimumAlignmentConfidence).map((alignment) => alignment.alignment_id).sort();
  const blocked = rows.some((row) => row.response_status === "blocked" || (row.stage_status_counts.blocked ?? 0) > 0);
  const weakRows = rows.some((row) => row.role === "specialist" && (row.response_status !== "complete" || !row.passed || row.reward < minimumReward));
  const synthesisRow = rowMap.get("cross_domain");
  const synthesisMissing = requireSynthesis && !synthesisRow;
  const synthesisWeak = requireSynthesis && !!synthesisRow && (synthesisRow.response_status !== "complete" || !synthesisRow.passed || synthesisRow.reward < minimumReward);
  const reasons: string[] = [];
  if (missing.length) reasons.push("missing_domain_coverage");
  if (unexpected.length) reasons.push("unexpected_domain_coverage");
  if (blocked) reasons.push("blocked_domain_response");
  if (weakRows) reasons.push("domain_response_integrity_below_threshold");
  if (synthesisMissing) reasons.push("synthesis_response_missing");
  if (synthesisWeak) reasons.push("synthesis_response_integrity_below_threshold");
  if (missingPairs.length) reasons.push("pairwise_alignment_incomplete");
  if (contradictions.length) reasons.push("high_confidence_contradiction");
  if (unresolved.length) reasons.push("unresolved_alignment");
  if (lowConfidence.length) reasons.push("low_confidence_alignment");
  const alignmentReasons = ["pairwise_alignment_incomplete", "high_confidence_contradiction", "unresolved_alignment", "low_confidence_alignment"];
  const alignmentOnly = reasons.length > 0 && reasons.every((reason) => alignmentReasons.includes(reason));
  const materialFailure = reasons.some((reason) => !alignmentReasons.includes(reason));
  const completed = reasons.length === 0 && !!synthesisRow;
  const status: AutonomousCrossDomainResponseStatus = blocked ? "blocked" : materialFailure ? "partial" : alignmentOnly ? "needs_alignment_review" : completed ? "completed" : "ready_to_synthesize";
  const readyToSynthesize = status === "ready_to_synthesize";
  const descriptor = {
    schema: AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA,
    context_digest: contextDigest,
    requested_domains: requested,
    specialist_domains: specialists,
    present_domains: present,
    missing_domains: missing,
    unexpected_domains: unexpected,
    rows,
    alignments,
    alignment_pairs_expected: expectedPairs,
    alignment_pairs_observed: observedPairSet.size,
    missing_alignment_pairs: missingPairs,
    contradictory_alignment_ids: contradictions,
    unresolved_alignment_ids: unresolved,
    low_confidence_alignment_ids: lowConfidence,
    synthesis_domain_present: !!synthesisRow,
    synthesis_response_digest: synthesisRow?.response_digest ?? null,
    synthesis_evaluation_digest: synthesisRow?.evaluation_digest ?? null,
    require_synthesis: requireSynthesis,
    require_complete_alignment: requireCompleteAlignment,
    minimum_reward: minimumReward,
    minimum_alignment_confidence: minimumAlignmentConfidence,
    contradiction_confidence_threshold: contradictionConfidenceThreshold,
    status,
    ready_to_synthesize: readyToSynthesize,
    gate_reasons: reasons,
    next_actions: gateActions({ missing, blocked, weak: weakRows || synthesisWeak, missingPairs, contradictions, unresolved, lowConfidence, synthesisMissing, completed }),
    retention: RETENTION,
    evaluator_authority: AUTHORITY,
    secret_material: "never_returned" as const,
  } satisfies Omit<AutonomousCrossDomainResponseAssessment, "assessment_digest">;
  const result = { ...descriptor, assessment_digest: digestJsonSync(descriptor) };
  assertSafeMetadata(result);
  if (bytes(JSON.stringify(result)) > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES) throw new ArgumentError("cross-domain response assessment exceeds its byte bound");
  return result;
}

/** Validate a persisted digest-only gate projection without needing provider values. */
export function validateAutonomousCrossDomainResponseAssessment(value: unknown): AutonomousCrossDomainResponseAssessment {
  if (!isObject(value)) throw new ArgumentError("cross-domain response assessment must be an object");
  assertSafeMetadata(value);
  const allowed = ["schema", "context_digest", "requested_domains", "specialist_domains", "present_domains", "missing_domains", "unexpected_domains", "rows", "alignments", "alignment_pairs_expected", "alignment_pairs_observed", "missing_alignment_pairs", "contradictory_alignment_ids", "unresolved_alignment_ids", "low_confidence_alignment_ids", "synthesis_domain_present", "synthesis_response_digest", "synthesis_evaluation_digest", "require_synthesis", "require_complete_alignment", "minimum_reward", "minimum_alignment_confidence", "contradiction_confidence_threshold", "status", "ready_to_synthesize", "gate_reasons", "next_actions", "retention", "evaluator_authority", "secret_material", "assessment_digest"] as const;
  exactKeys("cross-domain response assessment", value, allowed);
  if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA) throw new ArgumentError("cross-domain response assessment schema is invalid");
  if (value.retention !== RETENTION || value.evaluator_authority !== AUTHORITY || value.secret_material !== "never_returned") throw new ArgumentError("cross-domain response assessment retention contract is invalid");
  optionalDigest("cross-domain response context digest", value.context_digest);
  const requested = canonicalDomains("cross-domain response requested domains", value.requested_domains);
  const specialists = requested.filter((domain) => domain !== "cross_domain");
  if (specialists.length < 2 || JSON.stringify(value.specialist_domains) !== JSON.stringify(specialists)) throw new ArgumentError("cross-domain response specialist domain projection is inconsistent");
  const present = canonicalDomains("cross-domain response present domains", value.present_domains, 1);
  if (JSON.stringify(value.missing_domains) !== JSON.stringify(requested.filter((domain) => !present.includes(domain)))) throw new ArgumentError("cross-domain response missing domain projection is inconsistent");
  if (JSON.stringify(value.unexpected_domains) !== JSON.stringify(present.filter((domain) => !requested.includes(domain) && domain !== "cross_domain"))) throw new ArgumentError("cross-domain response unexpected domain projection is inconsistent");
  if (!Array.isArray(value.rows) || value.rows.length < 1 || value.rows.length > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES) throw new ArgumentError("cross-domain response rows are outside their bound");
  const rowDomains = new Set<AutonomousDomainName>();
  for (const raw of value.rows) {
    if (!isObject(raw)) throw new ArgumentError("cross-domain response row is malformed");
    const rowKeys = ["schema", "domain", "role", "workflow_id", "contract_digest", "response_digest", "evaluation_digest", "response_status", "reward", "passed", "missing_signals", "signals", "stage_status_counts", "domain_detail_coverage", "uncertainty_count", "evidence_gap_count", "next_action_count", "answer_digest", "row_digest"] as const;
    exactKeys("cross-domain response row", raw, rowKeys);
    if (raw.schema !== AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA) throw new ArgumentError("cross-domain response row schema is invalid");
    const domain = boundedText("cross-domain response row domain", raw.domain, 64) as AutonomousDomainName;
    if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain) || rowDomains.has(domain)) throw new ArgumentError("cross-domain response row domain is invalid or duplicated");
    rowDomains.add(domain);
    const role = boundedText("cross-domain response row role", raw.role, 32);
    if (!AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES.includes(role as AutonomousCrossDomainResponseRole)) throw new ArgumentError("cross-domain response row role is invalid");
    boundedIdentifier("cross-domain response row workflow id", raw.workflow_id);
    for (const name of ["contract_digest", "response_digest", "evaluation_digest", "answer_digest"] as const) boundedDigest(`cross-domain response row ${name}`, raw[name]);
    fraction("cross-domain response row reward", raw.reward);
    if (typeof raw.passed !== "boolean") throw new ArgumentError("cross-domain response row passed flag is invalid");
    boundedStrings("cross-domain response row missing signals", raw.missing_signals);
    if (!isObject(raw.signals) || Object.keys(raw.signals).length === 0) throw new ArgumentError("cross-domain response row signals are malformed");
    for (const [name, score] of Object.entries(raw.signals)) fraction(`cross-domain response signal ${name}`, score);
    if (!isObject(raw.stage_status_counts)) throw new ArgumentError("cross-domain response row stage status counts are malformed");
    for (const [name, rawCount] of Object.entries(raw.stage_status_counts)) {
      const count = rawCount as number;
      if (!Number.isSafeInteger(count) || count < 0 || count > 64) throw new ArgumentError(`cross-domain response row stage count ${name} is invalid`);
    }
    fraction("cross-domain response row detail coverage", raw.domain_detail_coverage);
    for (const name of ["uncertainty_count", "evidence_gap_count", "next_action_count"] as const) {
      const count = raw[name] as number;
      if (!Number.isSafeInteger(count) || count < 0) throw new ArgumentError(`cross-domain response row ${name} is invalid`);
    }
    const { row_digest: rowDigest, ...rowWithoutDigest } = raw as unknown as AutonomousCrossDomainResponseRow;
    if (digestJsonSync(rowWithoutDigest) !== boundedDigest("cross-domain response row digest", rowDigest)) throw new ArgumentError("cross-domain response row digest does not match its projection");
  }
  const rowMap = new Map((value.rows as unknown as AutonomousCrossDomainResponseRow[]).map((row) => [row.domain, row]));
  if (JSON.stringify([...rowDomains].sort((left, right) => canonicalDomainOrder(left) - canonicalDomainOrder(right))) !== JSON.stringify(present)) throw new ArgumentError("cross-domain response rows do not match the present domains");
  if (!Array.isArray(value.alignments) || value.alignments.length > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS) throw new ArgumentError("cross-domain response alignments are outside their bound");
  const alignments = (value.alignments as unknown[]).map((raw) => normalizeAlignment(raw, rowMap));
  if (JSON.stringify(alignments) !== JSON.stringify(value.alignments)) throw new ArgumentError("cross-domain response alignments are not normalized");
  const expectedPairs = value.alignment_pairs_expected as number;
  const observedPairs = value.alignment_pairs_observed as number;
  if (!Number.isSafeInteger(expectedPairs) || expectedPairs < 0 || !Number.isSafeInteger(observedPairs) || observedPairs < 0 || observedPairs > expectedPairs) throw new ArgumentError("cross-domain response alignment pair counts are invalid");
  if (typeof value.require_synthesis !== "boolean" || typeof value.require_complete_alignment !== "boolean" || typeof value.synthesis_domain_present !== "boolean") throw new ArgumentError("cross-domain response assessment controls are invalid");
  optionalDigest("cross-domain synthesis response digest", value.synthesis_response_digest);
  optionalDigest("cross-domain synthesis evaluation digest", value.synthesis_evaluation_digest);
  fraction("cross-domain assessment minimum reward", value.minimum_reward);
  fraction("cross-domain assessment minimum alignment confidence", value.minimum_alignment_confidence);
  fraction("cross-domain assessment contradiction threshold", value.contradiction_confidence_threshold);
  if (!AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES.includes(value.status as AutonomousCrossDomainResponseStatus) || typeof value.ready_to_synthesize !== "boolean") throw new ArgumentError("cross-domain response assessment status is invalid");
  boundedStrings("cross-domain response gate reasons", value.gate_reasons, MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_REASONS);
  boundedStrings("cross-domain response next actions", value.next_actions);
  const assessmentDigest = boundedDigest("cross-domain response assessment digest", value.assessment_digest);
  const { assessment_digest: _assessmentDigest, ...descriptor } = value as unknown as AutonomousCrossDomainResponseAssessment;
  if (digestJsonSync(descriptor) !== assessmentDigest) throw new ArgumentError("cross-domain response assessment digest does not match its projection");
  return structuredClone(value) as unknown as AutonomousCrossDomainResponseAssessment;
}

/** Recompute the gate and reject drift from a persisted projection. */
export function replayAutonomousCrossDomainResponseAssessment(
  responses: readonly AutonomousCrossDomainResponseEntry[],
  expected: AutonomousCrossDomainResponseAssessment,
  options: Parameters<typeof assessAutonomousCrossDomainResponseSet>[1] = {},
): AutonomousCrossDomainResponseAssessment {
  const validated = validateAutonomousCrossDomainResponseAssessment(expected);
  const replayed = assessAutonomousCrossDomainResponseSet(responses, options);
  if (replayed.assessment_digest !== validated.assessment_digest) throw new ArgumentError("cross-domain response assessment replay drifted from the recorded projection");
  return replayed;
}
