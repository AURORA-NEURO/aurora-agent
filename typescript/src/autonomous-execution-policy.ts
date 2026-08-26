import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Contextual action selection for the autonomous brain.
 *
 * This module is deliberately above provider invocation.  It decides which already-reviewed
 * execution candidate is appropriate for a bounded context, but it never resolves a credential,
 * calls a provider, dispatches a source, executes a tool, or authorizes an effect.  Only explicit
 * evaluator credit can update its value-only state.
 */
export const AUTONOMOUS_EXECUTION_POLICY_SCHEMA = "bioprism-autonomous-execution-policy/0.1" as const;
export const AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA = "bioprism-autonomous-execution-policy-state/0.1" as const;
export const AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA = "bioprism-autonomous-execution-policy-settlement/0.1" as const;
export const AUTONOMOUS_EXECUTION_POLICY_PATHS = ["provider", "evidence_first", "workflow", "planning", "cross_domain", "tool_loop"] as const;
export const AUTONOMOUS_EXECUTION_POLICY_POSTURES = ["selected", "review_required", "refused"] as const;
export const AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES = 256;
export const AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS = 512;
export const AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS = 4_096;
export const AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS = 32;
export const AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES = 8_000_000;

const DOMAINS = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"] as const;
const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;
const RETENTION = "value_only_policy_metadata;task_prompt_response_tool_and_credential_values_not_retained" as const;
const SECRET_MATERIAL = "never_returned" as const;

export type AutonomousExecutionPolicyPath = typeof AUTONOMOUS_EXECUTION_POLICY_PATHS[number];
export type AutonomousExecutionPolicyPosture = typeof AUTONOMOUS_EXECUTION_POLICY_POSTURES[number];
export type AutonomousExecutionPolicyDomain = typeof DOMAINS[number];

export interface AutonomousExecutionPolicyCandidateInput {
  arm_id: string;
  domain: AutonomousExecutionPolicyDomain;
  path: AutonomousExecutionPolicyPath;
  capabilities?: readonly string[];
  quality_prior?: number;
  reliability?: number;
  cost_units?: number;
  latency_ms?: number;
  risk?: number;
  available?: boolean;
  evidence_ready?: boolean;
  structured_output?: boolean;
  effects_supported?: boolean;
  approval_required?: boolean;
  provider?: string | null;
  model?: string | null;
}

export interface AutonomousExecutionPolicyCandidate extends JsonObject {
  arm_id: string;
  domain: AutonomousExecutionPolicyDomain;
  path: AutonomousExecutionPolicyPath;
  capabilities: string[];
  quality_prior: number;
  reliability: number;
  cost_units: number;
  latency_ms: number;
  risk: number;
  available: boolean;
  evidence_ready: boolean;
  structured_output: boolean;
  effects_supported: boolean;
  approval_required: boolean;
  provider: string | null;
  model: string | null;
  candidate_digest: string;
}

export interface AutonomousExecutionPolicyContextInput {
  context_digest?: string | null;
  requested_domains: readonly AutonomousExecutionPolicyDomain[];
  required_capabilities?: readonly string[];
  preferred_capabilities?: readonly string[];
  required_path?: AutonomousExecutionPolicyPath | null;
  evidence_required?: boolean;
  structured_output_required?: boolean;
  effects_requested?: boolean;
  effects_approved?: boolean;
  approval_granted?: boolean;
  max_cost_units?: number;
  max_latency_ms?: number;
  max_risk?: number;
  min_score?: number;
}

export interface AutonomousExecutionPolicyContext extends JsonObject {
  context_digest: string | null;
  requested_domains: AutonomousExecutionPolicyDomain[];
  required_capabilities: string[];
  preferred_capabilities: string[];
  required_path: AutonomousExecutionPolicyPath | null;
  evidence_required: boolean;
  structured_output_required: boolean;
  effects_requested: boolean;
  effects_approved: boolean;
  approval_granted: boolean;
  max_cost_units: number;
  max_latency_ms: number;
  max_risk: number;
  min_score: number;
}

export interface AutonomousExecutionPolicyArmState extends JsonObject {
  arm_id: string;
  pulls: number;
  failures: number;
  reward_sum: number;
  last_reward: number | null;
  last_outcome_digest: string | null;
  last_generation: number;
}

export interface AutonomousExecutionPolicySettlementRecord extends JsonObject {
  settlement_id: string;
  arm_id: string;
  outcome_digest: string;
  reward: number;
  passed: boolean;
  evaluator_id: string;
  evaluator_version: string;
}

export interface AutonomousExecutionPolicyState extends JsonObject {
  schema: typeof AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA;
  generation: number;
  previous_state_digest: string | null;
  arms: AutonomousExecutionPolicyArmState[];
  settlements: AutonomousExecutionPolicySettlementRecord[];
  retention: "value_only_policy_state;task_prompt_response_tool_and_credential_values_not_retained";
  secret_material: typeof SECRET_MATERIAL;
  state_digest: string;
}

export interface AutonomousExecutionPolicyRanking extends JsonObject {
  arm_id: string;
  domain: AutonomousExecutionPolicyDomain;
  path: AutonomousExecutionPolicyPath;
  candidate_digest: string;
  eligible: boolean;
  score: number | null;
  exploitation: number | null;
  exploration_bonus: number | null;
  confidence: number | null;
  mean_reward: number | null;
  preferred_capability_match: number;
  reasons: string[];
  review_reasons: string[];
}

export interface AutonomousExecutionPolicyDecision extends JsonObject {
  schema: typeof AUTONOMOUS_EXECUTION_POLICY_SCHEMA;
  context: AutonomousExecutionPolicyContext;
  policy_generation: number;
  total_pulls: number;
  posture: AutonomousExecutionPolicyPosture;
  selected_arm_id: string | null;
  selected_candidate: AutonomousExecutionPolicyCandidate | null;
  rankings: AutonomousExecutionPolicyRanking[];
  review_reasons: string[];
  refusal_reasons: string[];
  decision_digest: string;
  authorization: "guidance_only;provider_source_tool_effect_and_credential_authority_remain_separate";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousExecutionPolicySettlementInput {
  settlement_id: string;
  arm_id: string;
  decision_digest: string;
  outcome_digest: string;
  reward: number;
  passed: boolean;
  evaluator_id: string;
  evaluator_version: string;
}

export interface AutonomousExecutionPolicySettlement extends JsonObject {
  schema: typeof AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA;
  settlement_id: string;
  arm_id: string;
  outcome_digest: string;
  reward: number;
  passed: boolean;
  evaluator_id: string;
  evaluator_version: string;
  previous_state_digest: string;
  next_state_digest: string;
  generation: number;
  idempotent_replay: boolean;
  retention: "value_only_explicit_evaluator_credit;no_transport_reward";
  secret_material: typeof SECRET_MATERIAL;
}

function fail(message: string): never { throw new ArgumentError(`autonomous execution policy ${message}`); }
function clone<T>(value: T): T { return structuredClone(value); }
function bytes(value: unknown): number { return new TextEncoder().encode(JSON.stringify(value)).byteLength; }
function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || value.includes("\u0000") || /[\u0000-\u001F\u007F]/.test(value)) fail(`${name} is outside its bound`);
  return value.trim();
}
function identifier(name: string, value: unknown, maximum = 256): string {
  const text = boundedText(name, value, maximum);
  if (!IDENTIFIER.test(text)) fail(`${name} must be a bounded identifier`);
  return text;
}
function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !DIGEST.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}
function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bound`);
  return value;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bound`);
  return value;
}
function bool(name: string, value: unknown): boolean { if (typeof value !== "boolean") fail(`${name} must be boolean`); return value; }
function domains(name: string, value: unknown): AutonomousExecutionPolicyDomain[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > DOMAINS.length) fail(`${name} must contain 1..${DOMAINS.length} domains`);
  const result = value.map((item) => { if (typeof item !== "string" || !(DOMAINS as readonly string[]).includes(item)) fail(`${name} contains an unsupported domain`); return item as AutonomousExecutionPolicyDomain; });
  if (new Set(result).size !== result.length) fail(`${name} contains duplicate domains`);
  return result;
}
function items(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS) fail(`${name} exceeds its item bound`);
  const result = value.map((item) => identifier(`${name} item`, item, 128));
  if (new Set(result).size !== result.length) fail(`${name} contains duplicate items`);
  return result;
}
function round(value: number): number { return Number(value.toFixed(12)); }
function candidateDescriptor(value: AutonomousExecutionPolicyCandidate): JsonObject {
  const { candidate_digest: _ignored, ...descriptor } = value;
  return descriptor;
}
function context(value: AutonomousExecutionPolicyContextInput): AutonomousExecutionPolicyContext {
  if (!isObject(value)) fail("context must be an object");
  const contextDigest = digest("context_digest", value.context_digest ?? null, true);
  const required = items("required_capabilities", value.required_capabilities ?? []);
  const preferred = items("preferred_capabilities", value.preferred_capabilities ?? []);
  const requested = domains("requested_domains", value.requested_domains);
  const requiredPath = value.required_path === null || value.required_path === undefined ? null : value.required_path;
  if (requiredPath !== null && !(AUTONOMOUS_EXECUTION_POLICY_PATHS as readonly string[]).includes(requiredPath)) fail("required_path is unsupported");
  const maxCost = finite("max_cost_units", value.max_cost_units ?? 1_000_000, 0, 1_000_000);
  const maxLatency = finite("max_latency_ms", value.max_latency_ms ?? 86_400_000, 0, 86_400_000);
  const maxRisk = finite("max_risk", value.max_risk ?? 1, 0, 1);
  const minScore = finite("min_score", value.min_score ?? 0, -2, 2);
  return { context_digest: contextDigest, requested_domains: requested, required_capabilities: required, preferred_capabilities: preferred, required_path: requiredPath, evidence_required: bool("evidence_required", value.evidence_required ?? false), structured_output_required: bool("structured_output_required", value.structured_output_required ?? false), effects_requested: bool("effects_requested", value.effects_requested ?? false), effects_approved: bool("effects_approved", value.effects_approved ?? false), approval_granted: bool("approval_granted", value.approval_granted ?? false), max_cost_units: maxCost, max_latency_ms: maxLatency, max_risk: maxRisk, min_score: minScore };
}
function candidate(value: AutonomousExecutionPolicyCandidateInput): AutonomousExecutionPolicyCandidate {
  if (!isObject(value)) fail("candidate must be an object");
  const armId = identifier("candidate arm_id", value.arm_id);
  if (!(DOMAINS as readonly string[]).includes(value.domain)) fail("candidate domain is unsupported");
  if (!(AUTONOMOUS_EXECUTION_POLICY_PATHS as readonly string[]).includes(value.path)) fail("candidate path is unsupported");
  const capabilities = items("candidate capabilities", value.capabilities ?? []);
  const provider = value.provider === null || value.provider === undefined ? null : boundedText("candidate provider", value.provider);
  const model = value.model === null || value.model === undefined ? null : boundedText("candidate model", value.model);
  const result = { arm_id: armId, domain: value.domain, path: value.path, capabilities, quality_prior: finite("candidate quality_prior", value.quality_prior ?? 0.5, 0, 1), reliability: finite("candidate reliability", value.reliability ?? 0.5, 0, 1), cost_units: finite("candidate cost_units", value.cost_units ?? 1, 0, 1_000_000), latency_ms: finite("candidate latency_ms", value.latency_ms ?? 0, 0, 86_400_000), risk: finite("candidate risk", value.risk ?? 0.5, 0, 1), available: bool("candidate available", value.available ?? true), evidence_ready: bool("candidate evidence_ready", value.evidence_ready ?? false), structured_output: bool("candidate structured_output", value.structured_output ?? false), effects_supported: bool("candidate effects_supported", value.effects_supported ?? false), approval_required: bool("candidate approval_required", value.approval_required ?? false), provider, model, candidate_digest: "" } as AutonomousExecutionPolicyCandidate;
  result.candidate_digest = digestJsonSync(candidateDescriptor(result));
  return result;
}

function stateDescriptor(value: AutonomousExecutionPolicyState): JsonObject {
  const { state_digest: _ignored, ...descriptor } = value;
  return descriptor;
}
function state(value: AutonomousExecutionPolicyState): AutonomousExecutionPolicyState {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA || value.retention !== "value_only_policy_state;task_prompt_response_tool_and_credential_values_not_retained" || value.secret_material !== SECRET_MATERIAL) fail("state markers are invalid");
  const generation = integer("state generation", value.generation, 0, 2_147_483_647);
  const previousStateDigest = digest("state previous_state_digest", value.previous_state_digest ?? null, true);
  if ((generation === 0 && previousStateDigest !== null) || (generation > 0 && previousStateDigest === null)) fail("state predecessor digest fence is malformed");
  if (!Array.isArray(value.arms) || value.arms.length > AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS) fail("state arms exceed capacity");
  const arms = value.arms.map((raw) => {
    if (!isObject(raw)) fail("state arm is malformed");
    const armId = identifier("state arm_id", raw.arm_id);
    const pulls = integer("state arm pulls", raw.pulls, 0, 2_147_483_647);
    const failures = integer("state arm failures", raw.failures, 0, pulls);
    const rewardSum = finite("state arm reward_sum", raw.reward_sum, 0, pulls);
    const lastReward = raw.last_reward === null ? null : finite("state arm last_reward", raw.last_reward, 0, 1);
    const lastOutcome = digest("state arm last_outcome_digest", raw.last_outcome_digest ?? null, true);
    const lastGeneration = integer("state arm last_generation", raw.last_generation, 0, generation);
    return { arm_id: armId, pulls, failures, reward_sum: rewardSum, last_reward: lastReward, last_outcome_digest: lastOutcome, last_generation: lastGeneration } as AutonomousExecutionPolicyArmState;
  });
  if (new Set(arms.map((arm) => arm.arm_id)).size !== arms.length) fail("state contains duplicate arms");
  if (!Array.isArray(value.settlements) || value.settlements.length > AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS) fail("state settlements exceed capacity");
  const settlements = value.settlements.map((raw) => {
    if (!isObject(raw)) fail("state settlement is malformed");
    return { settlement_id: identifier("state settlement_id", raw.settlement_id), arm_id: identifier("state settlement arm_id", raw.arm_id), outcome_digest: digest("state settlement outcome_digest", raw.outcome_digest)! , reward: finite("state settlement reward", raw.reward, 0, 1), passed: bool("state settlement passed", raw.passed), evaluator_id: identifier("state settlement evaluator_id", raw.evaluator_id, 128), evaluator_version: identifier("state settlement evaluator_version", raw.evaluator_version, 128) } as AutonomousExecutionPolicySettlementRecord;
  });
  if (new Set(settlements.map((item) => item.settlement_id)).size !== settlements.length) fail("state contains duplicate settlements");
  const normalized = { schema: AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA, generation, previous_state_digest: previousStateDigest, arms: arms.sort((a, b) => a.arm_id.localeCompare(b.arm_id)), settlements, retention: value.retention, secret_material: value.secret_material, state_digest: value.state_digest } as AutonomousExecutionPolicyState;
  if (typeof normalized.state_digest !== "string" || !DIGEST.test(normalized.state_digest) || digestJsonSync(stateDescriptor(normalized)) !== normalized.state_digest) fail("state digest does not match metadata");
  if (bytes(normalized) > AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES) fail("state exceeds its byte bound");
  return clone(normalized);
}
function emptyState(): AutonomousExecutionPolicyState {
  const body = { schema: AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA, generation: 0, previous_state_digest: null, arms: [], settlements: [], retention: "value_only_policy_state;task_prompt_response_tool_and_credential_values_not_retained" as const, secret_material: SECRET_MATERIAL };
  return state({ ...body, state_digest: digestJsonSync(body) });
}
function decisionDescriptor(value: AutonomousExecutionPolicyDecision): JsonObject {
  const { decision_digest: _ignored, authorization: _authorization, retention: _retention, secret_material: _secret, ...descriptor } = value;
  return descriptor;
}
function validateDecision(value: AutonomousExecutionPolicyDecision): AutonomousExecutionPolicyDecision {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EXECUTION_POLICY_SCHEMA) fail("decision is malformed");
  const normalized = clone(value);
  const normalizedContext = context(normalized.context as unknown as AutonomousExecutionPolicyContextInput);
  if (canonicalJson(normalized.context) !== canonicalJson(normalizedContext)) fail("decision context is not normalized");
  integer("decision policy_generation", normalized.policy_generation, 0, 2_147_483_647);
  integer("decision total_pulls", normalized.total_pulls, 0, 2_147_483_647);
  if (!Array.isArray(normalized.rankings) || normalized.rankings.length > AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES) fail("decision rankings exceed capacity");
  const rankingIds = new Set<string>();
  for (const row of normalized.rankings) {
    if (!isObject(row) || !(DOMAINS as readonly string[]).includes(row.domain as string) || !(AUTONOMOUS_EXECUTION_POLICY_PATHS as readonly string[]).includes(row.path as string)) fail("decision ranking domain or path is invalid");
    const armId = identifier("decision ranking arm_id", row.arm_id);
    if (rankingIds.has(armId)) fail("decision rankings contain duplicate arm IDs");
    rankingIds.add(armId);
    digest("decision ranking candidate_digest", row.candidate_digest);
    bool("decision ranking eligible", row.eligible);
    if (row.score !== null) finite("decision ranking score", row.score, -2, 2);
    if (row.exploitation !== null) finite("decision ranking exploitation", row.exploitation, -2, 2);
    if (row.exploration_bonus !== null) finite("decision ranking exploration_bonus", row.exploration_bonus, 0, 2);
    if (row.confidence !== null) finite("decision ranking confidence", row.confidence, 0, 1);
    if (row.mean_reward !== null) finite("decision ranking mean_reward", row.mean_reward, 0, 1);
    finite("decision ranking preferred_capability_match", row.preferred_capability_match, 0, 1);
    items("decision ranking reasons", row.reasons);
    items("decision ranking review_reasons", row.review_reasons);
  }
  items("decision review_reasons", normalized.review_reasons);
  items("decision refusal_reasons", normalized.refusal_reasons);
  if (typeof normalized.decision_digest !== "string" || !DIGEST.test(normalized.decision_digest) || digestJsonSync(decisionDescriptor(normalized)) !== normalized.decision_digest) fail("decision digest does not match metadata");
  if (normalized.retention !== RETENTION || normalized.secret_material !== SECRET_MATERIAL) fail("decision retention markers are invalid");
  if (normalized.selected_arm_id !== null && !identifier("decision selected_arm_id", normalized.selected_arm_id)) fail("decision selected arm is invalid");
  if (!AUTONOMOUS_EXECUTION_POLICY_POSTURES.includes(normalized.posture)) fail("decision posture is invalid");
  if ((normalized.selected_arm_id === null) !== (normalized.selected_candidate === null)) fail("decision selected arm and candidate are inconsistent");
  if (normalized.posture === "refused" && normalized.selected_arm_id !== null) fail("refused decision cannot select an arm");
  if (normalized.posture !== "refused" && normalized.selected_arm_id === null) fail("non-refused decision must select an arm");
  if (normalized.selected_candidate !== null) {
    const selected = candidate(normalized.selected_candidate as unknown as AutonomousExecutionPolicyCandidateInput);
    if (selected.arm_id !== normalized.selected_arm_id || selected.candidate_digest !== normalized.selected_candidate.candidate_digest || !rankingIds.has(selected.arm_id)) fail("decision selected candidate is not bound to its ranking");
  }
  return normalized;
}

export class AutonomousExecutionPolicy {
  private stateValue: AutonomousExecutionPolicyState;
  readonly exploration: number;

  constructor(options: { state?: AutonomousExecutionPolicyState | JsonObject; exploration?: number } = {}) {
    this.exploration = finite("exploration", options.exploration ?? 0.35, 0, 2);
    this.stateValue = options.state === undefined ? emptyState() : state(options.state as AutonomousExecutionPolicyState);
  }

  get generation(): number { return this.stateValue.generation; }
  snapshot(): AutonomousExecutionPolicyState { return clone(this.stateValue); }
  restore(value: AutonomousExecutionPolicyState | JsonObject): void {
    const next = state(value as AutonomousExecutionPolicyState);
    if (next.generation < this.stateValue.generation) fail("state restore would roll back a newer generation");
    if (next.generation === this.stateValue.generation && next.state_digest !== this.stateValue.state_digest) fail("state restore conflicts with the current generation");
    if (next.generation === this.stateValue.generation + 1 && next.previous_state_digest !== this.stateValue.state_digest) fail("state restore predecessor digest does not match the current generation");
    this.stateValue = next;
  }

  select(rawContext: AutonomousExecutionPolicyContextInput, rawCandidates: readonly AutonomousExecutionPolicyCandidateInput[]): AutonomousExecutionPolicyDecision {
    const selectedContext = context(rawContext);
    if (!Array.isArray(rawCandidates) || rawCandidates.length < 1 || rawCandidates.length > AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES) fail("candidates are outside their bound");
    const candidates = rawCandidates.map(candidate);
    if (new Set(candidates.map((item) => item.arm_id)).size !== candidates.length) fail("candidates contain duplicate arm_id values");
    const totalPulls = this.stateValue.arms.reduce((sum, arm) => sum + arm.pulls, 0);
    const byArm = new Map(this.stateValue.arms.map((arm) => [arm.arm_id, arm]));
    const rankingRows = candidates.map((item): AutonomousExecutionPolicyRanking & { candidate: AutonomousExecutionPolicyCandidate } => {
      const reasons: string[] = [];
      const reviewReasons: string[] = [];
      if (!selectedContext.requested_domains.includes(item.domain)) reasons.push("domain_not_requested");
      if (!item.available) reasons.push("candidate_unavailable");
      if (selectedContext.required_path !== null && item.path !== selectedContext.required_path) reasons.push("path_not_requested");
      if (selectedContext.required_capabilities.some((capability) => !item.capabilities.includes(capability))) reasons.push("required_capability_missing");
      if (selectedContext.evidence_required && !item.evidence_ready) reasons.push("evidence_not_ready");
      if (selectedContext.structured_output_required && !item.structured_output) reasons.push("structured_output_not_supported");
      if (selectedContext.effects_requested && !item.effects_supported) reasons.push("effects_not_supported");
      if (item.cost_units > selectedContext.max_cost_units) reasons.push("cost_budget_exceeded");
      if (item.latency_ms > selectedContext.max_latency_ms) reasons.push("latency_budget_exceeded");
      if (item.risk > selectedContext.max_risk) reasons.push("risk_budget_exceeded");
      if (item.approval_required && !selectedContext.approval_granted) reviewReasons.push("candidate_approval_required");
      if (selectedContext.effects_requested && !selectedContext.effects_approved) reviewReasons.push("effect_approval_required");
      const eligible = reasons.length === 0;
      if (!eligible) return { arm_id: item.arm_id, domain: item.domain, path: item.path, candidate_digest: item.candidate_digest, eligible, score: null, exploitation: null, exploration_bonus: null, confidence: null, mean_reward: null, preferred_capability_match: 0, reasons, review_reasons: reviewReasons, candidate: item };
      const arm = byArm.get(item.arm_id) ?? { arm_id: item.arm_id, pulls: 0, failures: 0, reward_sum: 0, last_reward: null, last_outcome_digest: null, last_generation: 0 };
      const priorWeight = 4;
      const meanReward = (arm.reward_sum + item.quality_prior * priorWeight) / (arm.pulls + priorWeight);
      const confidence = arm.pulls / (arm.pulls + priorWeight);
      const explorationBonus = this.exploration * Math.sqrt(Math.log(totalPulls + 2) / (arm.pulls + 1));
      const preferredMatch = selectedContext.preferred_capabilities.length === 0 ? 0.5 : selectedContext.preferred_capabilities.filter((capability) => item.capabilities.includes(capability)).length / selectedContext.preferred_capabilities.length;
      const costPenalty = selectedContext.max_cost_units === 0 ? 0 : item.cost_units / selectedContext.max_cost_units;
      const latencyPenalty = selectedContext.max_latency_ms === 0 ? 0 : item.latency_ms / selectedContext.max_latency_ms;
      const exploitation = 0.45 * meanReward + 0.2 * item.quality_prior + 0.2 * item.reliability + 0.15 * preferredMatch;
      const score = round(exploitation + explorationBonus - 0.12 * item.risk - 0.08 * costPenalty - 0.05 * latencyPenalty);
      return { arm_id: item.arm_id, domain: item.domain, path: item.path, candidate_digest: item.candidate_digest, eligible, score, exploitation: round(exploitation), exploration_bonus: round(explorationBonus), confidence: round(confidence), mean_reward: round(meanReward), preferred_capability_match: round(preferredMatch), reasons, review_reasons: reviewReasons, candidate: item };
    });
    const rankings = [...rankingRows].sort((left, right) => (right.score ?? -Infinity) - (left.score ?? -Infinity) || (right.exploitation ?? -Infinity) - (left.exploitation ?? -Infinity) || right.candidate.reliability - left.candidate.reliability || left.arm_id.localeCompare(right.arm_id));
    const winner = rankings.find((item) => item.eligible && (item.score ?? -Infinity) >= selectedContext.min_score);
    const refusalReasons = winner === undefined ? [...new Set(rankingRows.flatMap((item) => item.reasons.length > 0 ? item.reasons : ["all_candidates_below_score_floor"]))] : [];
    const reviewReasons = winner === undefined ? [] : [...new Set(winner.review_reasons)];
    const posture: AutonomousExecutionPolicyPosture = winner === undefined ? "refused" : reviewReasons.length > 0 ? "review_required" : "selected";
    const descriptor = { schema: AUTONOMOUS_EXECUTION_POLICY_SCHEMA, context: selectedContext, policy_generation: this.stateValue.generation, total_pulls: totalPulls, posture, selected_arm_id: winner?.candidate.arm_id ?? null, selected_candidate: winner?.candidate ?? null, rankings: rankings.map(({ candidate: _candidate, ...row }) => row), review_reasons: reviewReasons, refusal_reasons: refusalReasons };
    const decision = { ...descriptor, decision_digest: digestJsonSync(descriptor), authorization: "guidance_only;provider_source_tool_effect_and_credential_authority_remain_separate" as const, retention: RETENTION, secret_material: SECRET_MATERIAL } as AutonomousExecutionPolicyDecision;
    if (bytes(decision) > AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES) fail("decision exceeds its byte bound");
    return clone(decision);
  }

  settle(decision: AutonomousExecutionPolicyDecision, input: AutonomousExecutionPolicySettlementInput): AutonomousExecutionPolicySettlement {
    const checked = validateDecision(decision);
    if (checked.selected_arm_id === null || checked.posture === "refused") fail("cannot settle a refused decision");
    const settlementId = identifier("settlement_id", input.settlement_id);
    const armId = identifier("settlement arm_id", input.arm_id);
    if (armId !== checked.selected_arm_id) fail("settlement arm_id does not match the selected arm");
    const outcomeDigest = digest("settlement outcome_digest", input.outcome_digest)!;
    const decisionDigest = digest("settlement decision_digest", input.decision_digest)!;
    if (decisionDigest !== checked.decision_digest) fail("settlement decision_digest does not match the decision");
    const reward = finite("settlement reward", input.reward, 0, 1);
    const passed = bool("settlement passed", input.passed);
    const evaluatorId = identifier("settlement evaluator_id", input.evaluator_id, 128);
    const evaluatorVersion = identifier("settlement evaluator_version", input.evaluator_version, 128);
    const existing = this.stateValue.settlements.find((item) => item.settlement_id === settlementId);
    if (existing !== undefined) {
      if (existing.arm_id !== armId || existing.outcome_digest !== outcomeDigest || existing.reward !== reward || existing.passed !== passed || existing.evaluator_id !== evaluatorId || existing.evaluator_version !== evaluatorVersion) fail("settlement_id was reused for different evaluator credit");
      return { schema: AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA, settlement_id: settlementId, arm_id: armId, outcome_digest: outcomeDigest, reward, passed, evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, previous_state_digest: this.stateValue.state_digest, next_state_digest: this.stateValue.state_digest, generation: this.stateValue.generation, idempotent_replay: true, retention: "value_only_explicit_evaluator_credit;no_transport_reward", secret_material: SECRET_MATERIAL };
    }
    if (this.stateValue.generation >= 2_147_483_647) fail("state generation is exhausted");
    if (this.stateValue.settlements.length >= AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS) fail("settlement capacity is exhausted");
    const previous = this.stateValue.state_digest;
    const arms = this.stateValue.arms.map((arm) => ({ ...arm }));
    const arm = arms.find((item) => item.arm_id === armId) ?? { arm_id: armId, pulls: 0, failures: 0, reward_sum: 0, last_reward: null, last_outcome_digest: null, last_generation: 0 };
    if (!arms.includes(arm)) arms.push(arm);
    arm.pulls += 1;
    arm.failures += passed ? 0 : 1;
    arm.reward_sum = round(arm.reward_sum + reward);
    arm.last_reward = reward;
    arm.last_outcome_digest = outcomeDigest;
    arm.last_generation = this.stateValue.generation + 1;
    const settlements = [...this.stateValue.settlements, { settlement_id: settlementId, arm_id: armId, outcome_digest: outcomeDigest, reward, passed, evaluator_id: evaluatorId, evaluator_version: evaluatorVersion }];
    const nextBody = { schema: AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA, generation: this.stateValue.generation + 1, previous_state_digest: previous, arms, settlements, retention: this.stateValue.retention, secret_material: this.stateValue.secret_material };
    this.stateValue = state({ ...nextBody, state_digest: digestJsonSync(nextBody) });
    return { schema: AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA, settlement_id: settlementId, arm_id: armId, outcome_digest: outcomeDigest, reward, passed, evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, previous_state_digest: previous, next_state_digest: this.stateValue.state_digest, generation: this.stateValue.generation, idempotent_replay: false, retention: "value_only_explicit_evaluator_credit;no_transport_reward", secret_material: SECRET_MATERIAL };
  }
}

export function validateAutonomousExecutionPolicyState(value: unknown): AutonomousExecutionPolicyState { return state(value as AutonomousExecutionPolicyState); }
export function validateAutonomousExecutionPolicyDecision(value: unknown): AutonomousExecutionPolicyDecision { return validateDecision(value as AutonomousExecutionPolicyDecision); }
export function selectAutonomousExecutionPolicy(contextValue: AutonomousExecutionPolicyContextInput, candidates: readonly AutonomousExecutionPolicyCandidateInput[], options: { state?: AutonomousExecutionPolicyState | JsonObject; exploration?: number } = {}): AutonomousExecutionPolicyDecision {
  return new AutonomousExecutionPolicy(options).select(contextValue, candidates);
}

// The lower-level execution journal already owns the shorter AutonomousExecutionPolicy name.
// Keep this joint selector namespaced so both contracts can safely be imported together.
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_SCHEMA;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_STATE_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_STATE_SCHEMA;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_SETTLEMENT_SCHEMA = AUTONOMOUS_EXECUTION_POLICY_SETTLEMENT_SCHEMA;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_PATHS = AUTONOMOUS_EXECUTION_POLICY_PATHS;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_POSTURES = AUTONOMOUS_EXECUTION_POLICY_POSTURES;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS = DOMAINS;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_CANDIDATES = AUTONOMOUS_EXECUTION_POLICY_MAX_CANDIDATES;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ARMS = AUTONOMOUS_EXECUTION_POLICY_MAX_ARMS;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_SETTLEMENTS = AUTONOMOUS_EXECUTION_POLICY_MAX_SETTLEMENTS;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_ITEMS = AUTONOMOUS_EXECUTION_POLICY_MAX_ITEMS;
export const AUTONOMOUS_JOINT_EXECUTION_POLICY_MAX_BYTES = AUTONOMOUS_EXECUTION_POLICY_MAX_BYTES;
export { AutonomousExecutionPolicy as AutonomousJointExecutionPolicy };
export function selectAutonomousJointExecutionPolicy(contextValue: AutonomousExecutionPolicyContextInput, candidates: readonly AutonomousExecutionPolicyCandidateInput[], options: { state?: AutonomousExecutionPolicyState | JsonObject; exploration?: number } = {}): AutonomousExecutionPolicyDecision {
  return selectAutonomousExecutionPolicy(contextValue, candidates, options);
}
export function validateAutonomousJointExecutionPolicyState(value: unknown): AutonomousExecutionPolicyState { return validateAutonomousExecutionPolicyState(value); }
export function validateAutonomousJointExecutionPolicyDecision(value: unknown): AutonomousExecutionPolicyDecision { return validateAutonomousExecutionPolicyDecision(value); }
export type AutonomousJointExecutionPolicyPath = AutonomousExecutionPolicyPath;
export type AutonomousJointExecutionPolicyPosture = AutonomousExecutionPolicyPosture;
export type AutonomousJointExecutionPolicyDomain = AutonomousExecutionPolicyDomain;
export type AutonomousJointExecutionPolicyCandidateInput = AutonomousExecutionPolicyCandidateInput;
export type AutonomousJointExecutionPolicyCandidate = AutonomousExecutionPolicyCandidate;
export type AutonomousJointExecutionPolicyContextInput = AutonomousExecutionPolicyContextInput;
export type AutonomousJointExecutionPolicyContext = AutonomousExecutionPolicyContext;
export type AutonomousJointExecutionPolicyArmState = AutonomousExecutionPolicyArmState;
export type AutonomousJointExecutionPolicySettlementRecord = AutonomousExecutionPolicySettlementRecord;
export type AutonomousJointExecutionPolicyState = AutonomousExecutionPolicyState;
export type AutonomousJointExecutionPolicyRanking = AutonomousExecutionPolicyRanking;
export type AutonomousJointExecutionPolicyDecision = AutonomousExecutionPolicyDecision;
export type AutonomousJointExecutionPolicySettlementInput = AutonomousExecutionPolicySettlementInput;
export type AutonomousJointExecutionPolicySettlement = AutonomousExecutionPolicySettlement;
