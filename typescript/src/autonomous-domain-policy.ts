import { ArgumentError } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Versioned, provider-free execution policy shared by every built-in domain. */
export const AUTONOMOUS_DOMAIN_POLICY_SCHEMA = "bioprism-autonomous-domain-policy/0.1" as const;
export const AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA = "bioprism-autonomous-domain-policy-admission/0.1" as const;
export const AUTONOMOUS_DOMAIN_POLICY_VERSION = "0.1" as const;

const POLICY_DOMAINS: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];

export type AutonomousDomainPolicyResponseMode = "freeform_allowed" | "structured_required";
export type AutonomousDomainPolicyEvidenceMode = "optional" | "required_before_provider";
export type AutonomousDomainPolicyEffectMode = "read_only" | "approval_gated" | "forbidden";
export type AutonomousDomainPolicyLearningMode = "health_only" | "evaluator_credit" | "evaluator_credit_and_trajectory";

export interface AutonomousDomainPolicy extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_POLICY_SCHEMA;
  domain: AutonomousDomainName;
  policy_id: string;
  policy_version: typeof AUTONOMOUS_DOMAIN_POLICY_VERSION;
  max_input_tokens: number;
  max_output_tokens: number;
  max_provider_attempts: number;
  max_tool_turns: number;
  max_total_cost_units: number;
  min_route_confidence: number;
  min_selection_confidence: number;
  min_selection_margin: number;
  response_mode: AutonomousDomainPolicyResponseMode;
  evidence_mode: AutonomousDomainPolicyEvidenceMode;
  effect_mode: AutonomousDomainPolicyEffectMode;
  learning_mode: AutonomousDomainPolicyLearningMode;
  evaluator_required: boolean;
  plan_acceptance_required: boolean;
  policy_digest: string;
  retention: "value_only_policy_metadata";
  secret_material: "never_returned";
}

export interface AutonomousDomainPolicyOverrides {
  max_input_tokens?: number;
  max_output_tokens?: number;
  max_provider_attempts?: number;
  max_tool_turns?: number;
  max_total_cost_units?: number;
  min_route_confidence?: number;
  min_selection_confidence?: number;
  min_selection_margin?: number;
  response_mode?: AutonomousDomainPolicyResponseMode;
  evidence_mode?: AutonomousDomainPolicyEvidenceMode;
  effect_mode?: AutonomousDomainPolicyEffectMode;
  learning_mode?: AutonomousDomainPolicyLearningMode;
  evaluator_required?: boolean;
  plan_acceptance_required?: boolean;
}

export interface AutonomousDomainPolicyAdmissionInput {
  route_confidence?: number;
  route_abstained?: boolean;
  selection_confidence?: number;
  selection_margin?: number;
  estimated_input_tokens?: number;
  requested_output_tokens?: number;
  estimated_cost_units?: number;
  structured_response?: boolean;
  evidence_ready?: boolean;
  evaluator_configured?: boolean;
  plan_accepted?: boolean;
  effects_requested?: boolean;
  effects_approved?: boolean;
}

export type AutonomousDomainPolicyAdmissionDecision = "admitted" | "review_required" | "blocked";

export interface AutonomousDomainPolicyAdmission extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA;
  domain: AutonomousDomainName;
  policy_digest: string;
  decision: AutonomousDomainPolicyAdmissionDecision;
  reasons: string[];
  checked: {
    route: boolean;
    selection: boolean;
    budget: boolean;
    response: boolean;
    evidence: boolean;
    evaluator: boolean;
    plan: boolean;
    effects: boolean;
  };
  effective_limits: Pick<AutonomousDomainPolicy, "max_input_tokens" | "max_output_tokens" | "max_provider_attempts" | "max_tool_turns" | "max_total_cost_units">;
  retention: "value_only_admission_metadata";
  secret_material: "never_returned";
  admission_digest: string;
}

type PolicyDescriptor = Omit<AutonomousDomainPolicy, "policy_digest">;

const POLICY_SEEDS: Record<AutonomousDomainName, Omit<PolicyDescriptor, "schema" | "domain" | "policy_id" | "policy_version" | "retention" | "secret_material">> = {
  coding: { max_input_tokens: 16_000, max_output_tokens: 6_000, max_provider_attempts: 3, max_tool_turns: 12, max_total_cost_units: 16, min_route_confidence: 0.55, min_selection_confidence: 0.58, min_selection_margin: 0.06, response_mode: "structured_required", evidence_mode: "optional", effect_mode: "approval_gated", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  browser: { max_input_tokens: 12_000, max_output_tokens: 4_000, max_provider_attempts: 3, max_tool_turns: 8, max_total_cost_units: 12, min_route_confidence: 0.62, min_selection_confidence: 0.62, min_selection_margin: 0.08, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "read_only", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  data: { max_input_tokens: 16_000, max_output_tokens: 6_000, max_provider_attempts: 3, max_tool_turns: 10, max_total_cost_units: 16, min_route_confidence: 0.58, min_selection_confidence: 0.60, min_selection_margin: 0.07, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
  science: { max_input_tokens: 16_000, max_output_tokens: 7_000, max_provider_attempts: 3, max_tool_turns: 10, max_total_cost_units: 18, min_route_confidence: 0.62, min_selection_confidence: 0.64, min_selection_margin: 0.09, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
  biomedical: { max_input_tokens: 14_000, max_output_tokens: 5_000, max_provider_attempts: 2, max_tool_turns: 8, max_total_cost_units: 12, min_route_confidence: 0.72, min_selection_confidence: 0.70, min_selection_margin: 0.12, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "forbidden", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  neuroscience: { max_input_tokens: 14_000, max_output_tokens: 5_000, max_provider_attempts: 2, max_tool_turns: 8, max_total_cost_units: 12, min_route_confidence: 0.68, min_selection_confidence: 0.68, min_selection_margin: 0.11, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "read_only", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
  operations: { max_input_tokens: 14_000, max_output_tokens: 5_000, max_provider_attempts: 2, max_tool_turns: 8, max_total_cost_units: 12, min_route_confidence: 0.68, min_selection_confidence: 0.70, min_selection_margin: 0.12, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  enterprise: { max_input_tokens: 14_000, max_output_tokens: 5_000, max_provider_attempts: 3, max_tool_turns: 8, max_total_cost_units: 14, min_route_confidence: 0.62, min_selection_confidence: 0.64, min_selection_margin: 0.10, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  multi_agent: { max_input_tokens: 18_000, max_output_tokens: 6_000, max_provider_attempts: 3, max_tool_turns: 12, max_total_cost_units: 20, min_route_confidence: 0.64, min_selection_confidence: 0.66, min_selection_margin: 0.10, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
  multimodal: { max_input_tokens: 20_000, max_output_tokens: 7_000, max_provider_attempts: 3, max_tool_turns: 10, max_total_cost_units: 20, min_route_confidence: 0.64, min_selection_confidence: 0.66, min_selection_margin: 0.10, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit", evaluator_required: true, plan_acceptance_required: true },
  cross_domain: { max_input_tokens: 20_000, max_output_tokens: 8_000, max_provider_attempts: 3, max_tool_turns: 14, max_total_cost_units: 24, min_route_confidence: 0.68, min_selection_confidence: 0.68, min_selection_margin: 0.12, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "approval_gated", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
  evaluation: { max_input_tokens: 16_000, max_output_tokens: 6_000, max_provider_attempts: 3, max_tool_turns: 10, max_total_cost_units: 18, min_route_confidence: 0.70, min_selection_confidence: 0.72, min_selection_margin: 0.12, response_mode: "structured_required", evidence_mode: "required_before_provider", effect_mode: "read_only", learning_mode: "evaluator_credit_and_trajectory", evaluator_required: true, plan_acceptance_required: true },
};

function finiteNumber(name: string, value: unknown, minimum: number, maximum: number, integer = false): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum || (integer && !Number.isSafeInteger(value))) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function makePolicy(domain: AutonomousDomainName, overrides: AutonomousDomainPolicyOverrides = {}): AutonomousDomainPolicy {
  if (!POLICY_DOMAINS.includes(domain)) throw new ArgumentError(`autonomous domain policy domain is unsupported: ${domain}`);
  const seed = POLICY_SEEDS[domain];
  const descriptor: PolicyDescriptor = {
    schema: AUTONOMOUS_DOMAIN_POLICY_SCHEMA,
    domain,
    policy_id: `builtin-${domain}-execution-policy`,
    policy_version: AUTONOMOUS_DOMAIN_POLICY_VERSION,
    ...seed,
    ...overrides,
    retention: "value_only_policy_metadata",
    secret_material: "never_returned",
  };
  for (const [name, value] of Object.entries(descriptor)) if (name.startsWith("max_")) finiteNumber(`domain policy ${name}`, value, 1, 1_000_000, true);
  for (const name of ["min_route_confidence", "min_selection_confidence", "min_selection_margin"]) finiteNumber(`domain policy ${name}`, descriptor[name as keyof PolicyDescriptor], 0, 1);
  if (descriptor.response_mode !== "freeform_allowed" && descriptor.response_mode !== "structured_required") throw new ArgumentError("domain policy response_mode is unsupported");
  if (descriptor.evidence_mode !== "optional" && descriptor.evidence_mode !== "required_before_provider") throw new ArgumentError("domain policy evidence_mode is unsupported");
  if (descriptor.effect_mode !== "read_only" && descriptor.effect_mode !== "approval_gated" && descriptor.effect_mode !== "forbidden") throw new ArgumentError("domain policy effect_mode is unsupported");
  if (descriptor.learning_mode !== "health_only" && descriptor.learning_mode !== "evaluator_credit" && descriptor.learning_mode !== "evaluator_credit_and_trajectory") throw new ArgumentError("domain policy learning_mode is unsupported");
  for (const name of ["evaluator_required", "plan_acceptance_required"] as const) if (typeof descriptor[name] !== "boolean") throw new ArgumentError(`domain policy ${name} must be boolean`);
  const policyDigest = digestJsonSync(descriptor);
  return Object.freeze({ ...descriptor, policy_digest: policyDigest }) as AutonomousDomainPolicy;
}

const BUILTIN_POLICIES = new Map(POLICY_DOMAINS.map((domain) => [domain, makePolicy(domain)]));

/** Return all twelve immutable built-in policies in canonical domain order. */
export function builtinAutonomousDomainPolicies(): readonly AutonomousDomainPolicy[] {
  return POLICY_DOMAINS.map((domain) => BUILTIN_POLICIES.get(domain) as AutonomousDomainPolicy);
}

/** Resolve a policy without contacting a provider, tool, source, or evaluator. */
export function autonomousDomainPolicy(domain: AutonomousDomainName, overrides: AutonomousDomainPolicyOverrides = {}): AutonomousDomainPolicy {
  if (!Object.keys(overrides).length) return BUILTIN_POLICIES.get(domain) as AutonomousDomainPolicy ?? makePolicy(domain);
  return makePolicy(domain, overrides);
}

function optionalFinite(name: string, value: unknown, minimum: number, maximum: number): number | undefined {
  return value === undefined ? undefined : finiteNumber(name, value, minimum, maximum);
}

/**
 * Evaluate all gates that can be decided before a provider call. This is an admission projection,
 * not authorization: an `admitted` result still requires the ordinary caller/provider boundary.
 */
export function evaluateAutonomousDomainPolicy(policy: AutonomousDomainPolicy, input: AutonomousDomainPolicyAdmissionInput = {}): AutonomousDomainPolicyAdmission {
  if (!policy || policy.schema !== AUTONOMOUS_DOMAIN_POLICY_SCHEMA) throw new ArgumentError("domain policy admission requires a valid policy");
  const routeConfidence = optionalFinite("domain policy route_confidence", input.route_confidence, 0, 1);
  const selectionConfidence = optionalFinite("domain policy selection_confidence", input.selection_confidence, 0, 1);
  const selectionMargin = optionalFinite("domain policy selection_margin", input.selection_margin, 0, 1);
  const estimatedInput = input.estimated_input_tokens === undefined ? undefined : finiteNumber("domain policy estimated_input_tokens", input.estimated_input_tokens, 0, 1_000_000, true);
  const requestedOutput = input.requested_output_tokens === undefined ? undefined : finiteNumber("domain policy requested_output_tokens", input.requested_output_tokens, 0, 1_000_000, true);
  const estimatedCost = input.estimated_cost_units === undefined ? undefined : finiteNumber("domain policy estimated_cost_units", input.estimated_cost_units, 0, 1_000_000, true);
  for (const [name, value] of [["route_abstained", input.route_abstained], ["structured_response", input.structured_response], ["evidence_ready", input.evidence_ready], ["evaluator_configured", input.evaluator_configured], ["plan_accepted", input.plan_accepted], ["effects_requested", input.effects_requested], ["effects_approved", input.effects_approved]] as const) if (value !== undefined && typeof value !== "boolean") throw new ArgumentError(`domain policy ${name} must be boolean when supplied`);
  const blocked: string[] = [];
  const review: string[] = [];
  if (input.route_abstained === true) blocked.push("route_abstained");
  if (routeConfidence !== undefined && routeConfidence < policy.min_route_confidence) review.push("route_confidence_below_policy_floor");
  if (selectionConfidence !== undefined && selectionConfidence < policy.min_selection_confidence) review.push("selection_confidence_below_policy_floor");
  if (selectionMargin !== undefined && selectionMargin < policy.min_selection_margin) review.push("selection_margin_below_policy_floor");
  if (estimatedInput !== undefined && estimatedInput > policy.max_input_tokens) blocked.push("input_budget_exceeded");
  if (requestedOutput !== undefined && requestedOutput > policy.max_output_tokens) blocked.push("output_budget_exceeded");
  if (estimatedCost !== undefined && estimatedCost > policy.max_total_cost_units) blocked.push("cost_budget_exceeded");
  if (policy.response_mode === "structured_required" && input.structured_response !== true) review.push("structured_response_required");
  if (policy.evidence_mode === "required_before_provider" && input.evidence_ready !== true) review.push("evidence_required_before_provider");
  if (policy.evaluator_required && input.evaluator_configured !== true) review.push("evaluator_required");
  if (policy.plan_acceptance_required && input.plan_accepted !== true) review.push("plan_acceptance_required");
  if (input.effects_requested === true && policy.effect_mode === "forbidden") blocked.push("effects_forbidden_by_policy");
  if (input.effects_requested === true && policy.effect_mode === "approval_gated" && input.effects_approved !== true) review.push("effect_approval_required");
  const reasons = [...blocked, ...review];
  const decision: AutonomousDomainPolicyAdmissionDecision = blocked.length ? "blocked" : review.length ? "review_required" : "admitted";
  const descriptor = { schema: AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA, domain: policy.domain, policy_digest: policy.policy_digest, decision, reasons, checked: { route: routeConfidence !== undefined || input.route_abstained !== undefined, selection: selectionConfidence !== undefined || selectionMargin !== undefined, budget: estimatedInput !== undefined || requestedOutput !== undefined || estimatedCost !== undefined, response: policy.response_mode === "structured_required", evidence: policy.evidence_mode === "required_before_provider", evaluator: policy.evaluator_required, plan: policy.plan_acceptance_required, effects: input.effects_requested === true }, effective_limits: { max_input_tokens: policy.max_input_tokens, max_output_tokens: policy.max_output_tokens, max_provider_attempts: policy.max_provider_attempts, max_tool_turns: policy.max_tool_turns, max_total_cost_units: policy.max_total_cost_units }, retention: "value_only_admission_metadata" as const, secret_material: "never_returned" as const };
  return Object.freeze({ ...descriptor, admission_digest: digestJsonSync(descriptor) });
}
