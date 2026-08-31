import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  assessAutonomousClaimIntegrity,
  type AssessAutonomousClaimIntegrityOptions,
  type AutonomousClaimIntegrityAssessment,
  type AutonomousClaimIntegrityClaim,
  type AutonomousClaimIntegrityClaimInput,
  type AutonomousClaimIntegrityEvidence,
  type AutonomousClaimIntegrityEvidenceInput,
  type AutonomousClaimIntegrityPolicy,
  type AutonomousClaimIntegrityPolicyInput,
} from "./autonomous-claim-integrity.js";
import {
  validateAutonomousCrossDomainResponseAssessment,
  type AutonomousCrossDomainResponseAssessment,
} from "./autonomous-cross-domain-response.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * A final, provider-free reliance gate for one autonomous outcome.
 *
 * Claim integrity and cross-domain response alignment are useful independently, but an
 * application still needs one answer to a narrower question: did the claims get assessed
 * against the exact run and exact output the caller is about to rely on?  This module binds
 * those projections to a run identity without retaining task text, answer text, evidence
 * values, prompts, credentials, or provider payloads.  It never establishes external truth and
 * it never authorizes a provider, source, tool, effect, or evaluator settlement.
 */
export const AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA = "bioprism-typescript-autonomous-outcome-integrity/0.1" as const;
export const AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA = "bioprism-typescript-autonomous-outcome-integrity-run/0.1" as const;
export const AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA = "bioprism-typescript-autonomous-outcome-integrity-binding/0.1" as const;
export const AUTONOMOUS_OUTCOME_INTEGRITY_STATUSES = ["ready", "review_required", "blocked", "ineligible"] as const;
export const AUTONOMOUS_OUTCOME_INTEGRITY_MODES = ["single_domain", "cross_domain"] as const;
export const AUTONOMOUS_OUTCOME_INTEGRITY_ROLES = ["run_output", "specialist_response", "synthesis_response"] as const;
export const MAX_AUTONOMOUS_OUTCOME_INTEGRITY_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS = 512;
export const MAX_AUTONOMOUS_OUTCOME_INTEGRITY_REASONS = 32;
export const MAX_AUTONOMOUS_OUTCOME_INTEGRITY_ACTIONS = 32;
export const MAX_AUTONOMOUS_OUTCOME_INTEGRITY_BYTES = 512_000;

const RETENTION = "metadata_only;claims_evidence_responses_prompts_credentials_and_provider_values_not_retained" as const;
const AUTHORITY = "provider_free_reliance_metadata_only;not_external_truth_or_execution_authority" as const;

export type AutonomousOutcomeIntegrityStatus = typeof AUTONOMOUS_OUTCOME_INTEGRITY_STATUSES[number];
export type AutonomousOutcomeIntegrityMode = typeof AUTONOMOUS_OUTCOME_INTEGRITY_MODES[number];
export type AutonomousOutcomeIntegrityRole = typeof AUTONOMOUS_OUTCOME_INTEGRITY_ROLES[number];

export interface AutonomousOutcomeIntegrityRunInput {
  task_digest: string;
  route_digest: string | null;
  status: string;
  mode: AutonomousOutcomeIntegrityMode;
  domains: readonly AutonomousDomainName[];
  output_digest: string | null;
  response_digest: string | null;
  outcome_digest: string;
  response_assessment_digest?: string | null;
  response_assessment_status?: string | null;
}

export interface AutonomousOutcomeIntegrityRun extends JsonObject {
  schema: typeof AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA;
  task_digest: string;
  route_digest: string | null;
  status: string;
  mode: AutonomousOutcomeIntegrityMode;
  domains: AutonomousDomainName[];
  output_digest: string | null;
  response_digest: string | null;
  outcome_digest: string;
  response_assessment_digest: string | null;
  response_assessment_status: string | null;
  run_digest: string;
}

export interface AutonomousOutcomeIntegrityClaimBindingInput extends JsonObject {
  claim_id: string;
  domain: AutonomousDomainName;
  role: AutonomousOutcomeIntegrityRole;
  output_digest: string;
  response_digest: string | null;
}

export interface AutonomousOutcomeIntegrityClaimBinding extends JsonObject {
  schema: typeof AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA;
  claim_id: string;
  domain: AutonomousDomainName;
  role: AutonomousOutcomeIntegrityRole;
  output_digest: string;
  response_digest: string | null;
  binding_digest: string;
}

export interface AutonomousOutcomeIntegrityAssessmentJSON extends JsonObject {
  schema: typeof AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA;
  run: AutonomousOutcomeIntegrityRun;
  claim_integrity_assessment_digest: string;
  claim_integrity_status: AutonomousClaimIntegrityAssessment["status"];
  claim_count: number;
  evidence_count: number;
  claim_status_counts: Record<string, number>;
  claim_action_ids: string[];
  claim_binding_digests: string[];
  response_assessment_digest: string | null;
  response_assessment_status: string | null;
  require_completed_run: boolean;
  require_response_assessment: boolean;
  require_synthesis: boolean;
  status: AutonomousOutcomeIntegrityStatus;
  gate_reasons: string[];
  next_actions: string[];
  retention: typeof RETENTION;
  evaluator_authority: typeof AUTHORITY;
  secret_material: "never_returned";
  assessment_digest: string;
}

/** The sealed JSON projection is intentionally the public in-memory assessment as well. */
export type AutonomousOutcomeIntegrityAssessment = AutonomousOutcomeIntegrityAssessmentJSON;

export interface AssessAutonomousOutcomeIntegrityOptions {
  run: AutonomousOutcomeIntegrityRunInput | AutonomousOutcomeIntegrityRun;
  claims: readonly (AutonomousClaimIntegrityClaim | AutonomousClaimIntegrityClaimInput | Record<string, unknown>)[];
  evidence: readonly (AutonomousClaimIntegrityEvidence | AutonomousClaimIntegrityEvidenceInput | Record<string, unknown>)[];
  claimBindings: readonly (AutonomousOutcomeIntegrityClaimBindingInput | Record<string, unknown>)[];
  referenceTime: string;
  policy?: AutonomousClaimIntegrityPolicy | AutonomousClaimIntegrityPolicyInput;
  responseAssessment?: AutonomousCrossDomainResponseAssessment | null;
  requireCompletedRun?: boolean;
  requireResponseAssessment?: boolean;
  requireSynthesis?: boolean;
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

function digest(name: string, value: unknown, nullable = false): string | null {
  if (value === null && nullable) return null;
  const text = boundedText(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(text)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return text;
}

function exactKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const allowedSet = new Set(allowed);
  if (Object.keys(value).length !== allowed.length || Object.keys(value).some((key) => !allowedSet.has(key))) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function safeMetadata(value: unknown, name = "outcome integrity metadata", depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((item, index) => safeMetadata(item, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (normalized === "secretmaterial" && child === "never_returned") continue;
      if (["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret"].includes(normalized) || ["token", "secret", "credential"].some((marker) => normalized.includes(marker))) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      safeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function domains(name: string, value: unknown): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_DOMAINS) throw new ArgumentError(`${name} is outside its domain bound`);
  const result = value.map((item) => boundedText(`${name} entry`, item, 64) as AutonomousDomainName);
  if (result.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain))) throw new ArgumentError(`${name} contains an unsupported domain`);
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate domains`);
  return [...result].sort((left, right) => AUTONOMOUS_DOMAIN_NAMES.indexOf(left) - AUTONOMOUS_DOMAIN_NAMES.indexOf(right));
}

function outputDigest(response: unknown): string | null {
  if (!isObject(response)) return null;
  const text = typeof response.text === "string" ? response.text : null;
  const structured = "structured" in response ? response.structured : null;
  if (text === null && structured === null) return null;
  return digestJsonSync({ text, structured });
}

function responseDigest(response: unknown, responseEvaluation: unknown): string | null {
  if (isObject(responseEvaluation) && typeof responseEvaluation.response_digest === "string" && /^[0-9a-f]{64}$/.test(responseEvaluation.response_digest)) return responseEvaluation.response_digest;
  return outputDigest(response);
}

function resultOutcomeDescriptor(value: Record<string, unknown>, run: Omit<AutonomousOutcomeIntegrityRun, "run_digest">): JsonObject {
  const selection = isObject(value.selection) ? value.selection : null;
  const responseEvaluation = isObject(value.response_evaluation) ? value.response_evaluation : null;
  const providerInvocations = Array.isArray(value.provider_invocations)
    ? value.provider_invocations.map((item) => isObject(item) ? {
      provider: typeof item.provider === "string" ? item.provider : null,
      model: typeof item.model === "string" ? item.model : null,
      attempt: typeof item.attempt === "number" ? item.attempt : null,
      status: typeof item.status === "string" ? item.status : null,
      receipt_digest: typeof item.receipt_digest === "string" ? item.receipt_digest : null,
    } : null)
    : [];
  return {
    schema: AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA,
    run,
    selection_digest: selection && typeof selection.selection_digest === "string" ? selection.selection_digest : selection && typeof selection.selectionDigest === "string" ? selection.selectionDigest : null,
    evaluation_digest: responseEvaluation && typeof responseEvaluation.evaluation_digest === "string" ? responseEvaluation.evaluation_digest : null,
    provider_invocation_receipts: providerInvocations,
  };
}

/** Project a TypeScript autonomous run into a metadata-only, digest-bound outcome identity. */
export function projectAutonomousOutcomeIntegrityRun(value: unknown): AutonomousOutcomeIntegrityRun {
  if (!isObject(value)) throw new ArgumentError("outcome integrity run must be an object");
  const route = isObject(value.route) ? value.route : null;
  const taskDigest = route && typeof route.task_digest === "string"
    ? route.task_digest
    : isObject(value.blueprint) && typeof value.blueprint.task_digest === "string" ? value.blueprint.task_digest : null;
  if (taskDigest === null) throw new ArgumentError("outcome integrity run is missing route task_digest");
  const routeDigest = route && (typeof route.route_digest === "string" || route.route_digest === null) ? route.route_digest : null;
  const status = boundedText("outcome integrity run status", value.status, 64);
  const cross = Array.isArray(value.child_runs) || value.synthesis !== undefined || value.execution_receipt !== undefined;
  const synthesis = cross && isObject(value.synthesis) ? value.synthesis : null;
  const source = synthesis ?? value;
  const responseEvaluation = isObject(source) && isObject(source.response_evaluation) ? source.response_evaluation : null;
  const output = outputDigest(isObject(source) ? source.response : null);
  const structuredResponse = responseDigest(isObject(source) ? source.response : null, responseEvaluation);
  const childDomains = cross && Array.isArray(value.child_runs)
    ? value.child_runs.flatMap((item) => isObject(item) && typeof item.domain === "string" ? [item.domain as AutonomousDomainName] : [])
    : [];
  const domain = cross ? [...new Set([...childDomains, "cross_domain" as AutonomousDomainName])] : [isObject(value.blueprint) && isObject(value.blueprint.domain_profile) && typeof value.blueprint.domain_profile.domain === "string" ? value.blueprint.domain_profile.domain as AutonomousDomainName : "coding"];
  const normalizedDomains = domains("outcome integrity run domains", domain);
  const mode: AutonomousOutcomeIntegrityMode = cross ? "cross_domain" : "single_domain";
  const responseAssessment = isObject(value.response_assessment) ? value.response_assessment : null;
  const responseAssessmentDigest = responseAssessment && typeof responseAssessment.assessment_digest === "string" ? responseAssessment.assessment_digest : null;
  const responseAssessmentStatus = responseAssessment && typeof responseAssessment.status === "string" ? responseAssessment.status : null;
  const base: Omit<AutonomousOutcomeIntegrityRun, "run_digest" | "outcome_digest"> = {
    schema: AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA,
    task_digest: digest("outcome integrity run task_digest", taskDigest)!,
    route_digest: digest("outcome integrity run route_digest", routeDigest, true),
    status,
    mode,
    domains: normalizedDomains,
    output_digest: digest("outcome integrity run output_digest", output, true),
    response_digest: digest("outcome integrity run response_digest", structuredResponse, true),
    response_assessment_digest: digest("outcome integrity response assessment digest", responseAssessmentDigest, true),
    response_assessment_status: responseAssessmentStatus,
  };
  const outcomeDigest = digestJsonSync(resultOutcomeDescriptor(value, { ...base, outcome_digest: "", run_digest: "" }));
  const result: Omit<AutonomousOutcomeIntegrityRun, "run_digest"> = { ...base, outcome_digest: outcomeDigest };
  return Object.assign({}, result, { run_digest: digestJsonSync(result) }) as AutonomousOutcomeIntegrityRun;
}

function normalizeRun(value: AutonomousOutcomeIntegrityRunInput | AutonomousOutcomeIntegrityRun): AutonomousOutcomeIntegrityRun {
  const taskDigest = digest("outcome integrity task_digest", value.task_digest)!;
  const routeDigest = digest("outcome integrity route_digest", value.route_digest, true);
  const status = boundedText("outcome integrity status", value.status, 64);
  const mode = boundedText("outcome integrity mode", value.mode, 32) as AutonomousOutcomeIntegrityMode;
  if (!AUTONOMOUS_OUTCOME_INTEGRITY_MODES.includes(mode)) throw new ArgumentError("outcome integrity mode is unsupported");
  const normalizedDomains = domains("outcome integrity domains", value.domains);
  const output = digest("outcome integrity output_digest", value.output_digest, true);
  const response = digest("outcome integrity response_digest", value.response_digest, true);
  const outcome = digest("outcome integrity outcome_digest", value.outcome_digest)!;
  const assessment = digest("outcome integrity response_assessment_digest", value.response_assessment_digest ?? null, true);
  const assessmentStatus = value.response_assessment_status === undefined || value.response_assessment_status === null ? null : boundedText("outcome integrity response_assessment_status", value.response_assessment_status, 64);
  const descriptor = { schema: AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA, task_digest: taskDigest, route_digest: routeDigest, status, mode, domains: normalizedDomains, output_digest: output, response_digest: response, outcome_digest: outcome, response_assessment_digest: assessment, response_assessment_status: assessmentStatus } satisfies Omit<AutonomousOutcomeIntegrityRun, "run_digest">;
  const runDigest = digestJsonSync(descriptor);
  if ("run_digest" in value && value.run_digest !== runDigest) throw new ArgumentError("outcome integrity run digest does not match its fields");
  return { ...descriptor, run_digest: runDigest };
}

function normalizeBinding(value: AutonomousOutcomeIntegrityClaimBindingInput | Record<string, unknown>, run: AutonomousOutcomeIntegrityRun): AutonomousOutcomeIntegrityClaimBinding {
  if (!isObject(value)) throw new ArgumentError("outcome integrity claim bindings must be objects");
  const input = "schema" in value || "binding_digest" in value
    ? (() => {
      if (value.schema !== AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA) throw new ArgumentError("outcome integrity binding schema is invalid");
      const { schema: _schema, binding_digest: _bindingDigest, ...descriptor } = value;
      return descriptor;
    })()
    : value;
  exactKeys("outcome integrity claim binding", input, ["claim_id", "domain", "role", "output_digest", "response_digest"]);
  const claimId = boundedIdentifier("outcome integrity binding claim_id", input.claim_id);
  const domain = boundedText("outcome integrity binding domain", input.domain, 64) as AutonomousDomainName;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("outcome integrity binding domain is unsupported");
  const role = boundedText("outcome integrity binding role", input.role, 32) as AutonomousOutcomeIntegrityRole;
  if (!AUTONOMOUS_OUTCOME_INTEGRITY_ROLES.includes(role)) throw new ArgumentError("outcome integrity binding role is unsupported");
  const output = digest("outcome integrity binding output_digest", input.output_digest)!;
  const response = digest("outcome integrity binding response_digest", input.response_digest, true);
  if (output !== run.output_digest) throw new ArgumentError("outcome integrity binding output_digest does not match the run output");
  if (response !== run.response_digest) throw new ArgumentError("outcome integrity binding response_digest does not match the run response");
  const descriptor = { schema: AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA, claim_id: claimId, domain, role, output_digest: output, response_digest: response } satisfies Omit<AutonomousOutcomeIntegrityClaimBinding, "binding_digest">;
  return { ...descriptor, binding_digest: digestJsonSync(descriptor) };
}

/** Normalize and digest every claim binding before the outcome assessment is built. */
export function bindAutonomousOutcomeIntegrityClaims(
  run: AutonomousOutcomeIntegrityRunInput | AutonomousOutcomeIntegrityRun,
  bindings: readonly (AutonomousOutcomeIntegrityClaimBindingInput | Record<string, unknown>)[],
): AutonomousOutcomeIntegrityClaimBinding[] {
  const normalizedRun = normalizeRun(run);
  if (!Array.isArray(bindings) || bindings.length < 1 || bindings.length > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS) throw new ArgumentError("outcome integrity claim bindings are outside their bound");
  const normalized = bindings.map((value) => normalizeBinding(value, normalizedRun));
  if (new Set(normalized.map((value) => value.claim_id)).size !== normalized.length) throw new ArgumentError("outcome integrity claim bindings contain duplicate claim ids");
  return normalized;
}

function claimStatusCounts(assessment: AutonomousClaimIntegrityAssessment): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const claim of assessment.claims) counts[claim.status] = (counts[claim.status] ?? 0) + 1;
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

function actionIds(assessment: AutonomousClaimIntegrityAssessment): string[] {
  return assessment.actions.map((action) => action.actionId).sort();
}

function boundedReasonList(values: readonly string[]): string[] {
  return [...new Set(values.map((value) => boundedText("outcome integrity gate reason", value, 1_024)))].slice(0, MAX_AUTONOMOUS_OUTCOME_INTEGRITY_REASONS);
}

function nextActions(status: AutonomousOutcomeIntegrityStatus, reasons: readonly string[], claimAssessment: AutonomousClaimIntegrityAssessment): string[] {
  const actions: string[] = [];
  if (reasons.includes("run_not_completed")) actions.push("inspect_incomplete_run");
  if (reasons.includes("run_output_missing")) actions.push("obtain_reviewed_run_output");
  if (reasons.includes("claim_bindings_incomplete") || reasons.includes("claim_binding_drift")) actions.push("rebind_claims_to_exact_run_output");
  if (reasons.includes("claim_integrity_blocked") || claimAssessment.actions.length > 0) actions.push("execute_reviewed_claim_integrity_actions");
  if (reasons.includes("response_assessment_missing") || reasons.includes("response_alignment_incomplete")) actions.push("complete_cross_domain_response_review");
  if (reasons.includes("synthesis_not_completed")) actions.push("complete_cross_domain_synthesis_review");
  if (status === "review_required" && actions.length === 0) actions.push("obtain_caller_reliance_review");
  if (status === "blocked" && actions.length === 0) actions.push("repair_blocked_outcome_contract");
  if (status === "ineligible" && actions.length === 0) actions.push("wait_for_a_usable_autonomous_outcome");
  return [...new Set(actions)].slice(0, MAX_AUTONOMOUS_OUTCOME_INTEGRITY_ACTIONS);
}

/** Fuse explicit claim/evidence review with an exact run/output identity. */
export function assessAutonomousOutcomeIntegrity(options: AssessAutonomousOutcomeIntegrityOptions): AutonomousOutcomeIntegrityAssessment {
  const run = normalizeRun(options.run);
  const requireCompletedRun = options.requireCompletedRun ?? true;
  const requireResponseAssessment = options.requireResponseAssessment ?? false;
  const requireSynthesis = options.requireSynthesis ?? false;
  if (typeof requireCompletedRun !== "boolean" || typeof requireResponseAssessment !== "boolean" || typeof requireSynthesis !== "boolean") throw new ArgumentError("outcome integrity gate controls must be booleans");
  const claimAssessment = assessAutonomousClaimIntegrity({
    contextDigest: run.task_digest,
    claims: options.claims,
    evidence: options.evidence,
    referenceTime: options.referenceTime,
    policy: options.policy,
  } satisfies AssessAutonomousClaimIntegrityOptions);
  if (!Array.isArray(options.claimBindings) || options.claimBindings.length > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS) throw new ArgumentError("outcome integrity claim bindings are outside their bound");
  const bindings = options.claimBindings.map((value) => normalizeBinding(value, run));
  const claimIds = claimAssessment.claims.map((claim) => claim.claim_id);
  const bindingIds = bindings.map((binding) => binding.claim_id);
  const reasons: string[] = [];
  if (new Set(bindingIds).size !== bindingIds.length || bindingIds.some((id) => !claimIds.includes(id)) || bindingIds.length !== claimIds.length) reasons.push("claim_bindings_incomplete");
  if (requireCompletedRun && run.status !== "completed") reasons.push("run_not_completed");
  if (run.output_digest === null) reasons.push("run_output_missing");
  if (claimAssessment.status === "blocked") reasons.push("claim_integrity_blocked");
  else if (claimAssessment.status !== "ready") reasons.push("claim_integrity_requires_review");
  const responseAssessment = options.responseAssessment === undefined || options.responseAssessment === null
    ? null
    : validateAutonomousCrossDomainResponseAssessment(options.responseAssessment);
  if (responseAssessment !== null) {
    if (responseAssessment.context_digest !== run.task_digest) throw new ArgumentError("outcome integrity response assessment is bound to a different task");
    if (run.response_assessment_digest !== null && responseAssessment.assessment_digest !== run.response_assessment_digest) reasons.push("response_assessment_digest_drift");
    if (responseAssessment.status !== "completed" && requireSynthesis) reasons.push("response_alignment_incomplete");
    if (responseAssessment.status !== "completed" && responseAssessment.status !== "ready_to_synthesize") reasons.push("response_alignment_incomplete");
    if (requireSynthesis && !responseAssessment.synthesis_domain_present) reasons.push("synthesis_not_completed");
  } else if (requireResponseAssessment) {
    reasons.push("response_assessment_missing");
  }
  if (requireSynthesis && responseAssessment === null) reasons.push("synthesis_not_completed");
  if (requireSynthesis && run.mode !== "cross_domain") reasons.push("synthesis_not_completed");
  const uniqueReasons = boundedReasonList(reasons);
  const status: AutonomousOutcomeIntegrityStatus = uniqueReasons.includes("run_output_missing") ? "ineligible" : uniqueReasons.some((reason) => reason.endsWith("blocked") || reason === "claim_bindings_incomplete" || reason === "run_not_completed" || reason === "synthesis_not_completed") ? "blocked" : uniqueReasons.length > 0 ? "review_required" : "ready";
  const descriptor = {
    schema: AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA,
    run,
    claim_integrity_assessment_digest: claimAssessment.assessmentDigest,
    claim_integrity_status: claimAssessment.status,
    claim_count: claimAssessment.claims.length,
    evidence_count: claimAssessment.evidence.length,
    claim_status_counts: claimStatusCounts(claimAssessment),
    claim_action_ids: actionIds(claimAssessment),
    claim_binding_digests: bindings.map((binding) => binding.binding_digest),
    response_assessment_digest: responseAssessment?.assessment_digest ?? run.response_assessment_digest,
    response_assessment_status: responseAssessment?.status ?? run.response_assessment_status,
    require_completed_run: requireCompletedRun,
    require_response_assessment: requireResponseAssessment,
    require_synthesis: requireSynthesis,
    status,
    gate_reasons: uniqueReasons,
    next_actions: nextActions(status, uniqueReasons, claimAssessment),
    retention: RETENTION,
    evaluator_authority: AUTHORITY,
    secret_material: "never_returned" as const,
  } satisfies Omit<AutonomousOutcomeIntegrityAssessmentJSON, "assessment_digest">;
  const serialized = JSON.stringify(descriptor);
  if (bytes(serialized) > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_BYTES) throw new ArgumentError("outcome integrity assessment exceeds its bound");
  return { ...descriptor, assessment_digest: digestJsonSync(descriptor) };
}

/** Verify the in-memory assessment and its nested run identity without re-running providers. */
export function validateAutonomousOutcomeIntegrity(value: AutonomousOutcomeIntegrityAssessmentJSON): AutonomousOutcomeIntegrityAssessmentJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA) throw new ArgumentError("outcome integrity assessment schema is invalid");
  const { assessment_digest: assessmentDigest, ...descriptor } = value;
  if (typeof assessmentDigest !== "string" || digestJsonSync(descriptor) !== assessmentDigest) throw new ArgumentError("outcome integrity assessment digest does not match its metadata");
  safeMetadata(descriptor);
  const run = normalizeRun(value.run);
  if (run.run_digest !== value.run.run_digest) throw new ArgumentError("outcome integrity assessment run digest is invalid");
  if (value.status === "ready" && value.gate_reasons.length !== 0) throw new ArgumentError("ready outcome integrity assessment cannot contain gate reasons");
  if (value.status === "ready" && value.next_actions.length !== 0) throw new ArgumentError("ready outcome integrity assessment cannot contain next actions");
  return value;
}

export function validateAutonomousOutcomeIntegritySnapshot(value: unknown): AutonomousOutcomeIntegrityAssessmentJSON {
  if (!isObject(value)) throw new ArgumentError("outcome integrity snapshot must be an object");
  return validateAutonomousOutcomeIntegrity(value as unknown as AutonomousOutcomeIntegrityAssessmentJSON);
}
