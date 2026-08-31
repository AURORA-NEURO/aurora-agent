import { ArgumentError, isObject } from "./errors.js";
import type {
  AutonomousBrainDomainPlanSummary,
  AutonomousBrainPlanJSON,
  AutonomousBrainCrossDomainPlanSummary,
} from "./autonomous-brain-facade.js";
import type { AutonomousDomainName, AutonomousRouteProposal } from "./autonomous.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_ACTION_PLAN_SCHEMA = "bioprism-typescript-autonomous-action-plan/0.1" as const;
export const AUTONOMOUS_ACTION_PLAN_VERSION = "0.1" as const;
export const AUTONOMOUS_ACTION_PLAN_STATUSES = ["ready", "review_required", "blocked", "route_review_required"] as const;
export const AUTONOMOUS_ACTION_PLAN_ROLES = ["single", "child", "synthesis"] as const;
export const AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS = [
  "review_route",
  "recompute_route",
  "review_task_decision",
  "resolve_policy_block",
  "acquire_evidence",
  "review_plan",
  "review_effect",
  "review_connector",
  "approve_provider_call",
  "settle_evaluator",
  "stop_before_dispatch",
] as const;
const ACTION_PLAN_APPROVALS = ["provider_call", "evidence_dispatch", "plan_acceptance", "effect_approval", "evaluator_settlement"] as const;
const ACTION_PLAN_RETENTION = "metadata_only;task_prompt_provider_connector_and_credential_values_not_retained" as const;
const ACTION_PLAN_AUTHORITY = "guidance_only;route_and_plan_metadata_do_not_authorize_dispatch" as const;
const CANDIDATE_RETENTION = "metadata_only;task_prompt_provider_and_connector_values_not_retained" as const;
const CANDIDATE_AUTHORITY = "guidance_only;does_not_authorize_provider_source_tool_or_effect_actions" as const;
const MAX_ACTION_PLAN_CANDIDATES = 16;
const MAX_ACTION_PLAN_ITEMS = 128;

export type AutonomousActionPlanStatus = typeof AUTONOMOUS_ACTION_PLAN_STATUSES[number];
export type AutonomousActionPlanRole = typeof AUTONOMOUS_ACTION_PLAN_ROLES[number];
export type AutonomousActionPlanNextAction = typeof AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS[number];
export type AutonomousActionPlanApproval = typeof ACTION_PLAN_APPROVALS[number];
type ActionPlanApproval = AutonomousActionPlanApproval;

export const AUTONOMOUS_ACTION_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-action-execution/0.1" as const;
export const AUTONOMOUS_ACTION_EXECUTION_VERSION = "0.1" as const;
export const AUTONOMOUS_ACTION_EXECUTION_STATUSES = ["admitted", "review_required", "blocked", "route_review_required"] as const;
export const AUTONOMOUS_ACTION_EXECUTION_RESULT_STATUSES = ["review_required", "blocked", "route_review_required", "completed"] as const;
export const AUTONOMOUS_ACTION_EXECUTION_PATHS = ["provider", "evidence_first", "workflow", "planning", "cross_domain", "route_review"] as const;
const ACTION_EXECUTION_AUTHORITY = "admission_only;does_not_authorize_provider_source_tool_effect_or_credential_actions" as const;
const ACTION_EXECUTION_RETENTION = "metadata_only;task_prompt_provider_connector_and_credential_values_not_retained" as const;
const ACTION_EXECUTION_SECRET = "never_returned" as const;
const ACTION_EXECUTION_TO_NEXT_ACTION: Record<ActionPlanApproval, AutonomousActionPlanNextAction> = {
  provider_call: "approve_provider_call",
  evidence_dispatch: "acquire_evidence",
  plan_acceptance: "review_plan",
  effect_approval: "review_effect",
  evaluator_settlement: "settle_evaluator",
};

function text(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside the autonomous action-plan text bound`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function items(name: string, value: unknown, maximum = MAX_ACTION_PLAN_ITEMS): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} exceeds its autonomous action-plan item bound`);
  const result = value.map((entry) => text(`${name} item`, entry));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate items`);
  return result;
}

function unique<T extends string>(values: readonly T[]): T[] { return [...new Set(values)]; }

function domain(name: string, value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !value.trim()) throw new ArgumentError(`${name} is invalid`);
  return value as AutonomousDomainName;
}

type CandidateDescriptor = {
  schema: typeof AUTONOMOUS_ACTION_PLAN_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_PLAN_VERSION;
  candidate_id: string;
  role: AutonomousActionPlanRole;
  domain: AutonomousDomainName;
  task_digest: string;
  route_digest: string;
  workflow_id: string;
  workflow_digest: string;
  domain_pack_digest: string;
  domain_policy_digest: string;
  evidence_plan_digest: string;
  capability: string;
  risk_class: string;
  task_intent_digest: string;
  task_decision_digest: string;
  task_decision_posture: AutonomousActionCandidate["task_decision_posture"];
  recommended_path: AutonomousActionCandidate["recommended_path"];
  requested_effect: string;
  evidence_posture: string;
  required_model_capabilities: string[];
  preferred_model_capabilities: string[];
  approval_requirements: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_actions: AutonomousActionPlanNextAction[];
};

function candidateDescriptor(candidate: AutonomousActionCandidate): CandidateDescriptor {
  return {
    schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
    version: AUTONOMOUS_ACTION_PLAN_VERSION,
    candidate_id: candidate.candidate_id,
    role: candidate.role,
    domain: candidate.domain,
    task_digest: candidate.task_digest,
    route_digest: candidate.route_digest,
    workflow_id: candidate.workflow_id,
    workflow_digest: candidate.workflow_digest,
    domain_pack_digest: candidate.domain_pack_digest,
    domain_policy_digest: candidate.domain_policy_digest,
    evidence_plan_digest: candidate.evidence_plan_digest,
    capability: candidate.capability,
    risk_class: candidate.risk_class,
    task_intent_digest: candidate.task_intent_digest,
    task_decision_digest: candidate.task_decision_digest,
    task_decision_posture: candidate.task_decision_posture,
    recommended_path: candidate.recommended_path,
    requested_effect: candidate.requested_effect,
    evidence_posture: candidate.evidence_posture,
    required_model_capabilities: [...candidate.required_model_capabilities],
    preferred_model_capabilities: [...candidate.preferred_model_capabilities],
    approval_requirements: [...candidate.approval_requirements],
    review_reasons: [...candidate.review_reasons],
    blocking_reasons: [...candidate.blocking_reasons],
    next_actions: [...candidate.next_actions],
  };
}

function validateCandidate(candidate: AutonomousActionCandidate, routeDigest: string): AutonomousActionCandidate {
  if (!isObject(candidate) || candidate.schema !== AUTONOMOUS_ACTION_PLAN_SCHEMA || candidate.version !== AUTONOMOUS_ACTION_PLAN_VERSION) throw new ArgumentError("autonomous action candidate metadata is malformed");
  if (!AUTONOMOUS_ACTION_PLAN_ROLES.includes(candidate.role)) throw new ArgumentError("autonomous action candidate role is invalid");
  digest("action candidate route_digest", candidate.route_digest);
  digest("action candidate task_digest", candidate.task_digest);
  digest("action candidate workflow_digest", candidate.workflow_digest);
  digest("action candidate domain_pack_digest", candidate.domain_pack_digest);
  digest("action candidate domain_policy_digest", candidate.domain_policy_digest);
  digest("action candidate evidence_plan_digest", candidate.evidence_plan_digest);
  digest("action candidate task_intent_digest", candidate.task_intent_digest);
  digest("action candidate task_decision_digest", candidate.task_decision_digest);
  if (candidate.route_digest !== routeDigest) throw new ArgumentError("autonomous action candidate route digest does not match the plan");
  if (candidate.authority !== CANDIDATE_AUTHORITY || candidate.retention !== CANDIDATE_RETENTION || candidate.secret_material !== "never_returned") throw new ArgumentError("autonomous action candidate retention posture is invalid");
  items("action candidate required_model_capabilities", candidate.required_model_capabilities, 64);
  items("action candidate preferred_model_capabilities", candidate.preferred_model_capabilities, 64);
  const approvals = items("action candidate approval_requirements", candidate.approval_requirements, 16);
  if (approvals.some((approval) => !(ACTION_PLAN_APPROVALS as readonly string[]).includes(approval))) throw new ArgumentError("autonomous action candidate approval requirement is invalid");
  items("action candidate review_reasons", candidate.review_reasons);
  items("action candidate blocking_reasons", candidate.blocking_reasons);
  const nextActions = items("action candidate next_actions", candidate.next_actions, 32);
  if (!nextActions.length || nextActions.some((action) => !(AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS as readonly string[]).includes(action))) throw new ArgumentError("autonomous action candidate next action is invalid");
  if (candidate.candidate_digest !== digestJsonSync(candidateDescriptor(candidate))) throw new ArgumentError("autonomous action candidate digest is invalid");
  return { ...candidate, required_model_capabilities: [...candidate.required_model_capabilities], preferred_model_capabilities: [...candidate.preferred_model_capabilities], approval_requirements: [...candidate.approval_requirements], review_reasons: [...candidate.review_reasons], blocking_reasons: [...candidate.blocking_reasons], next_actions: nextActions as AutonomousActionPlanNextAction[] };
}

export interface AutonomousActionCandidate extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_PLAN_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_PLAN_VERSION;
  candidate_id: string;
  role: AutonomousActionPlanRole;
  domain: AutonomousDomainName;
  task_digest: string;
  route_digest: string;
  workflow_id: string;
  workflow_digest: string;
  domain_pack_digest: string;
  domain_policy_digest: string;
  evidence_plan_digest: string;
  capability: string;
  risk_class: string;
  task_intent_digest: string;
  task_decision_digest: string;
  task_decision_posture: "admitted" | "review_required" | "blocked";
  recommended_path: "provider" | "evidence_first" | "workflow" | "planning" | "cross_domain";
  requested_effect: string;
  evidence_posture: string;
  required_model_capabilities: string[];
  preferred_model_capabilities: string[];
  approval_requirements: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_actions: AutonomousActionPlanNextAction[];
  candidate_digest: string;
  authority: typeof CANDIDATE_AUTHORITY;
  retention: typeof CANDIDATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousActionPlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_PLAN_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_PLAN_VERSION;
  status: AutonomousActionPlanStatus;
  route_digest: string;
  task_digest: string;
  selected_domains: AutonomousDomainName[];
  cross_domain: boolean;
  route_confidence: number;
  route_reason: string;
  route_source: string;
  semantic_route_status: string | null;
  recommended_path: AutonomousActionCandidate["recommended_path"] | "route_review";
  candidates: AutonomousActionCandidate[];
  required_approvals: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_action: AutonomousActionPlanNextAction;
  next_actions: AutonomousActionPlanNextAction[];
  plan_digest: string;
  authorization: typeof ACTION_PLAN_AUTHORITY;
  retention: typeof ACTION_PLAN_RETENTION;
  secret_material: "never_returned";
}

function candidateFromSummary(
  summary: AutonomousBrainDomainPlanSummary,
  routeDigest: string,
  candidateId: string,
  role: AutonomousActionPlanRole,
): AutonomousActionCandidate {
  const candidate: CandidateDescriptor & Pick<AutonomousActionCandidate, "authority" | "retention" | "secret_material"> = {
    schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
    version: AUTONOMOUS_ACTION_PLAN_VERSION,
    candidate_id: text("action candidate id", candidateId),
    role,
    domain: domain("action candidate domain", summary.domain),
    task_digest: digest("action candidate task_digest", summary.task_digest),
    route_digest: digest("action candidate route_digest", routeDigest),
    workflow_id: text("action candidate workflow_id", summary.workflow_id),
    workflow_digest: digest("action candidate workflow_digest", summary.workflow_digest),
    domain_pack_digest: digest("action candidate domain_pack_digest", summary.domain_pack_digest),
    domain_policy_digest: digest("action candidate domain_policy_digest", summary.domain_policy_digest),
    evidence_plan_digest: digest("action candidate evidence_plan_digest", summary.evidence_plan_digest),
    capability: text("action candidate capability", summary.capability),
    risk_class: text("action candidate risk_class", summary.risk_class),
    task_intent_digest: digest("action candidate task_intent_digest", summary.task_intent_digest),
    task_decision_digest: digest("action candidate task_decision_digest", summary.task_decision_digest),
    task_decision_posture: summary.task_decision_posture,
    recommended_path: summary.task_decision_recommended_path,
    requested_effect: text("action candidate requested_effect", summary.task_decision_requested_effect),
    evidence_posture: text("action candidate evidence_posture", summary.task_decision_evidence_posture),
    required_model_capabilities: items("action candidate required_model_capabilities", summary.required_capabilities, 64),
    preferred_model_capabilities: items("action candidate preferred_model_capabilities", summary.task_decision_preferred_model_capabilities, 64),
    approval_requirements: items("action candidate approval_requirements", summary.task_decision_approval_requirements, 16),
    review_reasons: items("action candidate review_reasons", summary.task_decision_review_reasons),
    blocking_reasons: items("action candidate blocking_reasons", summary.task_decision_blocking_reasons),
    next_actions: [],
    authority: CANDIDATE_AUTHORITY,
    retention: CANDIDATE_RETENTION,
    secret_material: "never_returned",
  };
  const nextActions: AutonomousActionPlanNextAction[] = [];
  if (candidate.approval_requirements.includes("evidence_dispatch")) nextActions.push("acquire_evidence");
  if (candidate.approval_requirements.includes("plan_acceptance")) nextActions.push("review_plan");
  if (candidate.approval_requirements.includes("effect_approval")) nextActions.push("review_effect");
  if (candidate.approval_requirements.includes("provider_call")) nextActions.push("approve_provider_call");
  if (candidate.approval_requirements.includes("evaluator_settlement")) nextActions.push("settle_evaluator");
  candidate.next_actions = unique(nextActions);
  const descriptor = candidateDescriptor(candidate as AutonomousActionCandidate);
  return Object.freeze({ ...candidate, candidate_digest: digestJsonSync(descriptor) }) as AutonomousActionCandidate;
}

type ActionPlanDescriptor = {
  schema: typeof AUTONOMOUS_ACTION_PLAN_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_PLAN_VERSION;
  status: AutonomousActionPlanStatus;
  route_digest: string;
  task_digest: string;
  selected_domains: AutonomousDomainName[];
  cross_domain: boolean;
  route_confidence: number;
  route_reason: string;
  route_source: string;
  semantic_route_status: string | null;
  recommended_path: AutonomousActionPlanJSON["recommended_path"];
  candidates: AutonomousActionCandidate[];
  required_approvals: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_action: AutonomousActionPlanNextAction;
  next_actions: AutonomousActionPlanNextAction[];
};

function planDescriptor(plan: AutonomousActionPlan): ActionPlanDescriptor {
  return {
    schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
    version: AUTONOMOUS_ACTION_PLAN_VERSION,
    status: plan.status,
    route_digest: plan.route_digest,
    task_digest: plan.task_digest,
    selected_domains: [...plan.selected_domains],
    cross_domain: plan.cross_domain,
    route_confidence: plan.route_confidence,
    route_reason: plan.route_reason,
    route_source: plan.route_source,
    semantic_route_status: plan.semantic_route_status,
    recommended_path: plan.recommended_path,
    candidates: plan.candidates.map((candidate) => ({ ...candidate })),
    required_approvals: [...plan.required_approvals],
    review_reasons: [...plan.review_reasons],
    blocking_reasons: [...plan.blocking_reasons],
    next_action: plan.next_action,
    next_actions: [...plan.next_actions],
  };
}

function sourceCandidates(source: AutonomousBrainPlanJSON): AutonomousActionCandidate[] {
  if (source.domain_plan !== null) return [candidateFromSummary(source.domain_plan, source.route.route_digest, "single", "single")];
  if (source.cross_domain_plan === null) return [];
  const cross: AutonomousBrainCrossDomainPlanSummary = source.cross_domain_plan;
  return [
    ...cross.children.map((child, index) => candidateFromSummary(child, cross.route_digest, cross.child_ids[index] ?? `child-${index + 1}`, "child")),
    candidateFromSummary(cross.synthesis, cross.route_digest, "synthesis", "synthesis"),
  ];
}

function aggregateActions(status: AutonomousActionPlanStatus, approvals: readonly string[], review: readonly string[]): { nextAction: AutonomousActionPlanNextAction; nextActions: AutonomousActionPlanNextAction[] } {
  if (status === "route_review_required") return { nextAction: "review_route", nextActions: ["review_route", "recompute_route"] };
  if (status === "blocked") return { nextAction: "resolve_policy_block", nextActions: ["resolve_policy_block", "stop_before_dispatch"] };
  const next: AutonomousActionPlanNextAction[] = [];
  if (review.length) next.push("review_task_decision");
  if (approvals.includes("evidence_dispatch")) next.push("acquire_evidence");
  if (approvals.includes("plan_acceptance")) next.push("review_plan");
  if (approvals.includes("effect_approval")) next.push("review_effect");
  if (approvals.includes("provider_call")) next.push("approve_provider_call");
  if (approvals.includes("evaluator_settlement")) next.push("settle_evaluator");
  if (!next.length) next.push("review_task_decision");
  return { nextAction: next[0]!, nextActions: unique(next) };
}

function buildFromJSON(source: AutonomousBrainPlanJSON): AutonomousActionPlanJSON {
  if (!isObject(source) || !isObject(source.route) || typeof source.route.route_digest !== "string") throw new ArgumentError("autonomous action plan source is malformed");
  const route = source.route as AutonomousRouteProposal;
  const semanticStatus = source.semantic_route?.status ?? null;
  const routeReview = source.status === "route_review_required" || route.abstained || (semanticStatus !== null && semanticStatus !== "completed");
  if (routeReview) {
    const reviewReasons = [`route:${route.reason}`, ...(semanticStatus !== null && semanticStatus !== "completed" ? [`semantic_route:${semanticStatus}`] : [])];
    const base: ActionPlanDescriptor = {
      schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
      version: AUTONOMOUS_ACTION_PLAN_VERSION,
      status: "route_review_required" as const,
      route_digest: digest("action plan route_digest", route.route_digest),
      task_digest: digest("action plan task_digest", route.task_digest),
      selected_domains: [...route.selected_domains],
      cross_domain: route.cross_domain,
      route_confidence: route.confidence,
      route_reason: text("action plan route_reason", route.reason),
      route_source: text("action plan route_source", route.source),
      semantic_route_status: semanticStatus,
      recommended_path: "route_review" as const,
      candidates: [],
      required_approvals: [],
      review_reasons: reviewReasons,
      blocking_reasons: [],
      next_action: "review_route" as const,
      next_actions: ["review_route", "recompute_route"] as AutonomousActionPlanNextAction[],
    };
    return { ...base, plan_digest: digestJsonSync(base), authorization: ACTION_PLAN_AUTHORITY, retention: ACTION_PLAN_RETENTION, secret_material: "never_returned" };
  }
  const candidates = sourceCandidates(source);
  if (!candidates.length) throw new ArgumentError("routed autonomous action plan source has no domain plan");
  const approvals = unique(candidates.flatMap((candidate) => candidate.approval_requirements));
  const review = unique(candidates.flatMap((candidate) => candidate.review_reasons.map((reason) => `${candidate.candidate_id}:${reason}`)));
  const blocking = unique(candidates.flatMap((candidate) => candidate.blocking_reasons.map((reason) => `${candidate.candidate_id}:${reason}`)));
  const connectorReview = source.status === "connector_review_required" || source.connector_plan?.status === "connector_missing";
  if (connectorReview) review.push("connector:review_required");
  const status: AutonomousActionPlanStatus = blocking.length ? "blocked" : review.length ? "review_required" : "ready";
  const actions = aggregateActions(status, approvals, review);
  if (connectorReview && status !== "blocked") {
    actions.nextActions.splice(1, 0, "review_connector");
    if (actions.nextAction === "approve_provider_call") actions.nextAction = "review_connector";
  }
  const base: ActionPlanDescriptor = {
    schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
    version: AUTONOMOUS_ACTION_PLAN_VERSION,
    status,
    route_digest: digest("action plan route_digest", route.route_digest),
    task_digest: digest("action plan task_digest", route.task_digest),
    selected_domains: [...route.selected_domains],
    cross_domain: route.cross_domain,
    route_confidence: route.confidence,
    route_reason: text("action plan route_reason", route.reason),
    route_source: text("action plan route_source", route.source),
    semantic_route_status: semanticStatus,
    recommended_path: route.cross_domain ? "cross_domain" as const : candidates[0]!.recommended_path,
    candidates,
    required_approvals: approvals,
    review_reasons: review,
    blocking_reasons: blocking,
    next_action: actions.nextAction,
    next_actions: unique(actions.nextActions),
  };
  return { ...base, plan_digest: digestJsonSync(base), authorization: ACTION_PLAN_AUTHORITY, retention: ACTION_PLAN_RETENTION, secret_material: "never_returned" };
}

export class AutonomousActionPlan {
  readonly status: AutonomousActionPlanStatus;
  readonly route_digest: string;
  readonly task_digest: string;
  readonly selected_domains: AutonomousDomainName[];
  readonly cross_domain: boolean;
  readonly route_confidence: number;
  readonly route_reason: string;
  readonly route_source: string;
  readonly semantic_route_status: string | null;
  readonly recommended_path: AutonomousActionPlanJSON["recommended_path"];
  readonly candidates: AutonomousActionCandidate[];
  readonly required_approvals: string[];
  readonly review_reasons: string[];
  readonly blocking_reasons: string[];
  readonly next_action: AutonomousActionPlanNextAction;
  readonly next_actions: AutonomousActionPlanNextAction[];
  readonly plan_digest: string;

  constructor(input: AutonomousActionPlanJSON | AutonomousBrainPlanJSON) {
    const value = "plan_digest" in input && input.schema === AUTONOMOUS_ACTION_PLAN_SCHEMA ? input as AutonomousActionPlanJSON : buildFromJSON(input as AutonomousBrainPlanJSON);
    if (value.schema !== AUTONOMOUS_ACTION_PLAN_SCHEMA || value.version !== AUTONOMOUS_ACTION_PLAN_VERSION || value.authorization !== ACTION_PLAN_AUTHORITY || value.retention !== ACTION_PLAN_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous action plan metadata is malformed");
    if (!AUTONOMOUS_ACTION_PLAN_STATUSES.includes(value.status)) throw new ArgumentError("autonomous action plan status is invalid");
    this.status = value.status;
    this.route_digest = digest("action plan route_digest", value.route_digest);
    this.task_digest = digest("action plan task_digest", value.task_digest);
    this.selected_domains = value.selected_domains.map((entry) => domain("action plan selected domain", entry));
    this.cross_domain = value.cross_domain;
    if (this.cross_domain !== (this.selected_domains.length > 1)) throw new ArgumentError("action plan cross_domain does not match selected domains");
    this.route_confidence = value.route_confidence;
    this.route_reason = text("action plan route_reason", value.route_reason);
    this.route_source = text("action plan route_source", value.route_source);
    this.semantic_route_status = value.semantic_route_status === null ? null : text("action plan semantic route status", value.semantic_route_status);
    this.recommended_path = value.recommended_path;
    this.candidates = value.candidates.map((candidate) => validateCandidate(candidate, this.route_digest));
    if (this.status === "route_review_required" && this.candidates.length) throw new ArgumentError("route-review action plan contains candidates");
    if (this.status !== "route_review_required" && !this.candidates.length) throw new ArgumentError("routed action plan has no candidates");
    this.required_approvals = items("action plan required_approvals", value.required_approvals, 16);
    if (this.required_approvals.some((approval) => !(ACTION_PLAN_APPROVALS as readonly string[]).includes(approval))) throw new ArgumentError("action plan approval requirement is invalid");
    this.review_reasons = items("action plan review_reasons", value.review_reasons);
    this.blocking_reasons = items("action plan blocking_reasons", value.blocking_reasons);
    if (!AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS.includes(value.next_action)) throw new ArgumentError("action plan next_action is invalid");
    this.next_action = value.next_action;
    this.next_actions = items("action plan next_actions", value.next_actions, 32) as AutonomousActionPlanNextAction[];
    if (!this.next_actions.includes(this.next_action)) throw new ArgumentError("action plan next_action is not present in next_actions");
    this.plan_digest = digest("action plan plan_digest", value.plan_digest);
    if (this.plan_digest !== digestJsonSync(this.descriptor())) throw new ArgumentError("autonomous action plan digest is invalid");
    if (this.status === "blocked" && !this.blocking_reasons.length) throw new ArgumentError("blocked action plan lacks blocking reasons");
    if (this.status === "review_required" && !this.review_reasons.length) throw new ArgumentError("review-required action plan lacks review reasons");
    Object.freeze(this.selected_domains);
    Object.freeze(this.candidates);
    Object.freeze(this.required_approvals);
    Object.freeze(this.review_reasons);
    Object.freeze(this.blocking_reasons);
    Object.freeze(this.next_actions);
  }

  private descriptor(): ActionPlanDescriptor {
    return {
      schema: AUTONOMOUS_ACTION_PLAN_SCHEMA,
      version: AUTONOMOUS_ACTION_PLAN_VERSION,
      status: this.status,
      route_digest: this.route_digest,
      task_digest: this.task_digest,
      selected_domains: [...this.selected_domains],
      cross_domain: this.cross_domain,
      route_confidence: this.route_confidence,
      route_reason: this.route_reason,
      route_source: this.route_source,
      semantic_route_status: this.semantic_route_status,
      recommended_path: this.recommended_path,
      candidates: this.candidates.map((candidate) => ({ ...candidate })),
      required_approvals: [...this.required_approvals],
      review_reasons: [...this.review_reasons],
      blocking_reasons: [...this.blocking_reasons],
      next_action: this.next_action,
      next_actions: [...this.next_actions],
    };
  }

  toJSON(): AutonomousActionPlanJSON {
    return { ...this.descriptor(), plan_digest: this.plan_digest, authorization: ACTION_PLAN_AUTHORITY, retention: ACTION_PLAN_RETENTION, secret_material: "never_returned" };
  }

  static fromJSON(value: unknown): AutonomousActionPlan {
    if (!isObject(value)) throw new ArgumentError("autonomous action plan must be an object");
    return new AutonomousActionPlan(value as AutonomousActionPlanJSON);
  }
}

export function buildAutonomousActionPlan(source: AutonomousBrainPlanJSON): AutonomousActionPlan {
  return new AutonomousActionPlan(source);
}

export type AutonomousActionExecutionStatus = typeof AUTONOMOUS_ACTION_EXECUTION_STATUSES[number];
export type AutonomousActionExecutionPath = typeof AUTONOMOUS_ACTION_EXECUTION_PATHS[number];

type ActionAdmissionDescriptor = {
  schema: typeof AUTONOMOUS_ACTION_EXECUTION_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_EXECUTION_VERSION;
  status: AutonomousActionExecutionStatus;
  plan_digest: string;
  task_digest: string;
  selected_domains: AutonomousDomainName[];
  execution_path: AutonomousActionExecutionPath;
  reviewed: boolean;
  required_approvals: string[];
  approved_approvals: string[];
  missing_approvals: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_action: AutonomousActionPlanNextAction;
  next_actions: AutonomousActionPlanNextAction[];
};

function admissionDescriptor(admission: AutonomousActionAdmission): ActionAdmissionDescriptor {
  return {
    schema: AUTONOMOUS_ACTION_EXECUTION_SCHEMA,
    version: AUTONOMOUS_ACTION_EXECUTION_VERSION,
    status: admission.status,
    plan_digest: admission.plan_digest,
    task_digest: admission.task_digest,
    selected_domains: [...admission.selected_domains],
    execution_path: admission.execution_path,
    reviewed: admission.reviewed,
    required_approvals: [...admission.required_approvals],
    approved_approvals: [...admission.approved_approvals],
    missing_approvals: [...admission.missing_approvals],
    review_reasons: [...admission.review_reasons],
    blocking_reasons: [...admission.blocking_reasons],
    next_action: admission.next_action,
    next_actions: [...admission.next_actions],
  };
}

export interface AutonomousActionAdmissionJSON extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_EXECUTION_SCHEMA;
  version: typeof AUTONOMOUS_ACTION_EXECUTION_VERSION;
  status: AutonomousActionExecutionStatus;
  plan_digest: string;
  task_digest: string;
  selected_domains: AutonomousDomainName[];
  execution_path: AutonomousActionExecutionPath;
  reviewed: boolean;
  required_approvals: string[];
  approved_approvals: string[];
  missing_approvals: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_action: AutonomousActionPlanNextAction;
  next_actions: AutonomousActionPlanNextAction[];
  admission_digest: string;
  authority: typeof ACTION_EXECUTION_AUTHORITY;
  retention: typeof ACTION_EXECUTION_RETENTION;
  execution: "admission_only;caller_must_bind_provider_and_effect_authority_separately";
  secret_material: typeof ACTION_EXECUTION_SECRET;
}

export class AutonomousActionAdmission {
  readonly status: AutonomousActionExecutionStatus;
  readonly plan_digest: string;
  readonly task_digest: string;
  readonly selected_domains: AutonomousDomainName[];
  readonly execution_path: AutonomousActionExecutionPath;
  readonly reviewed: boolean;
  readonly required_approvals: string[];
  readonly approved_approvals: string[];
  readonly missing_approvals: string[];
  readonly review_reasons: string[];
  readonly blocking_reasons: string[];
  readonly next_action: AutonomousActionPlanNextAction;
  readonly next_actions: AutonomousActionPlanNextAction[];
  readonly admission_digest: string;

  constructor(input: AutonomousActionAdmissionJSON | ActionAdmissionDescriptor) {
    if (input.schema !== AUTONOMOUS_ACTION_EXECUTION_SCHEMA || input.version !== AUTONOMOUS_ACTION_EXECUTION_VERSION) throw new ArgumentError("autonomous action admission metadata is malformed");
    if (!AUTONOMOUS_ACTION_EXECUTION_STATUSES.includes(input.status)) throw new ArgumentError("autonomous action admission status is invalid");
    this.status = input.status;
    this.plan_digest = digest("action admission plan_digest", input.plan_digest);
    this.task_digest = digest("action admission task_digest", input.task_digest);
    if (!Array.isArray(input.selected_domains) || input.selected_domains.length > 12) throw new ArgumentError("action admission selected domains exceed their bound");
    this.selected_domains = input.selected_domains.map((entry) => domain("action admission selected domain", entry));
    if (!AUTONOMOUS_ACTION_EXECUTION_PATHS.includes(input.execution_path)) throw new ArgumentError("action admission execution path is invalid");
    this.execution_path = input.execution_path;
    if (typeof input.reviewed !== "boolean") throw new ArgumentError("action admission reviewed must be boolean");
    this.reviewed = input.reviewed;
    this.required_approvals = items("action admission required approvals", input.required_approvals, 32);
    this.approved_approvals = items("action admission approved approvals", input.approved_approvals, 32);
    this.missing_approvals = items("action admission missing approvals", input.missing_approvals, 32);
    for (const approval of [...this.required_approvals, ...this.approved_approvals, ...this.missing_approvals]) if (!(ACTION_PLAN_APPROVALS as readonly string[]).includes(approval)) throw new ArgumentError("action admission approval is invalid");
    if (this.approved_approvals.some((approval) => !this.required_approvals.includes(approval))) throw new ArgumentError("action admission approved approval is not required");
    if (this.missing_approvals.some((approval) => !this.required_approvals.includes(approval))) throw new ArgumentError("action admission missing approval is not required");
    this.review_reasons = items("action admission review reasons", input.review_reasons);
    this.blocking_reasons = items("action admission blocking reasons", input.blocking_reasons);
    if (!AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS.includes(input.next_action)) throw new ArgumentError("action admission next action is invalid");
    this.next_action = input.next_action;
    this.next_actions = items("action admission next actions", input.next_actions, 32) as AutonomousActionPlanNextAction[];
    if (!this.next_actions.includes(this.next_action)) throw new ArgumentError("action admission next action is not present");
    if (this.status === "blocked" && !this.blocking_reasons.length) throw new ArgumentError("blocked action admission lacks blocking reasons");
    if (this.status === "review_required" && !this.review_reasons.length && !this.missing_approvals.length) throw new ArgumentError("review-required action admission lacks a pending gate");
    const descriptor = admissionDescriptor(this);
    this.admission_digest = "admission_digest" in input ? digest("action admission admission_digest", input.admission_digest) : digestJsonSync(descriptor);
    if (this.admission_digest !== digestJsonSync(descriptor)) throw new ArgumentError("autonomous action admission digest is invalid");
    Object.freeze(this.selected_domains);
    Object.freeze(this.required_approvals);
    Object.freeze(this.approved_approvals);
    Object.freeze(this.missing_approvals);
    Object.freeze(this.review_reasons);
    Object.freeze(this.blocking_reasons);
    Object.freeze(this.next_actions);
  }

  toJSON(): AutonomousActionAdmissionJSON {
    return {
      ...admissionDescriptor(this),
      admission_digest: this.admission_digest,
      authority: ACTION_EXECUTION_AUTHORITY,
      retention: ACTION_EXECUTION_RETENTION,
      execution: "admission_only;caller_must_bind_provider_and_effect_authority_separately",
      secret_material: ACTION_EXECUTION_SECRET,
    };
  }

  static fromJSON(value: unknown): AutonomousActionAdmission {
    if (!isObject(value)) throw new ArgumentError("autonomous action admission must be an object");
    const input = value as AutonomousActionAdmissionJSON;
    if (input.authority !== ACTION_EXECUTION_AUTHORITY || input.retention !== ACTION_EXECUTION_RETENTION || input.execution !== "admission_only;caller_must_bind_provider_and_effect_authority_separately" || input.secret_material !== ACTION_EXECUTION_SECRET) throw new ArgumentError("autonomous action admission authority posture is invalid");
    return new AutonomousActionAdmission(input);
  }
}

function admissionNextActions(plan: AutonomousActionPlan, missing: readonly ActionPlanApproval[], reviewPending: boolean): { nextAction: AutonomousActionPlanNextAction; nextActions: AutonomousActionPlanNextAction[] } {
  if (plan.status === "route_review_required") return { nextAction: "review_route", nextActions: ["review_route", "recompute_route"] };
  if (plan.status === "blocked") return { nextAction: "resolve_policy_block", nextActions: ["resolve_policy_block", "stop_before_dispatch"] };
  const next: AutonomousActionPlanNextAction[] = [];
  if (reviewPending) next.push("review_task_decision");
  for (const approval of missing) next.push(ACTION_EXECUTION_TO_NEXT_ACTION[approval]);
  if (!next.length) next.push("review_task_decision");
  return { nextAction: next[0]!, nextActions: unique(next) };
}

export function admitAutonomousActionPlan(
  source: AutonomousActionPlan | AutonomousActionPlanJSON,
  options: { approvals?: Partial<Record<ActionPlanApproval, boolean>>; reviewed?: boolean } = {},
): AutonomousActionAdmission {
  const plan = source instanceof AutonomousActionPlan ? source : AutonomousActionPlan.fromJSON(source);
  const reviewed = options.reviewed ?? false;
  if (typeof reviewed !== "boolean") throw new ArgumentError("action-plan reviewed must be boolean");
  const rawApprovals = options.approvals ?? {};
  if (!isObject(rawApprovals)) throw new ArgumentError("action-plan approvals must be an object");
  const approvalValues = rawApprovals as Record<string, unknown>;
  for (const [name, value] of Object.entries(approvalValues)) {
    if (!(ACTION_PLAN_APPROVALS as readonly string[]).includes(name)) throw new ArgumentError(`action-plan approval ${name} is unsupported`);
    if (typeof value !== "boolean") throw new ArgumentError(`action-plan approval ${name} must be boolean`);
  }
  const required = [...plan.required_approvals];
  const approved = required.filter((approval) => approvalValues[approval] === true) as ActionPlanApproval[];
  const missing = required.filter((approval) => approvalValues[approval] !== true) as ActionPlanApproval[];
  const reviewReasons = plan.status === "review_required" && reviewed ? [] : [...plan.review_reasons];
  if (plan.status === "review_required" && !reviewed) reviewReasons.push("caller_review_required_for_plan_decision");
  reviewReasons.push(...missing.map((approval) => `approval:${approval}:required`));
  const blockingReasons = [...plan.blocking_reasons];
  let status: AutonomousActionExecutionStatus;
  let executionPath: AutonomousActionExecutionPath;
  if (plan.status === "route_review_required") {
    status = "route_review_required";
    executionPath = "route_review";
  } else if (plan.status === "blocked") {
    status = "blocked";
    executionPath = plan.cross_domain ? "cross_domain" : plan.recommended_path;
  } else if (reviewReasons.length || missing.length) {
    status = "review_required";
    executionPath = plan.cross_domain ? "cross_domain" : plan.recommended_path;
  } else {
    status = "admitted";
    executionPath = plan.cross_domain ? "cross_domain" : plan.recommended_path;
  }
  const next = admissionNextActions(plan, missing, Boolean(reviewReasons.length && plan.status === "review_required" && !reviewed));
  return new AutonomousActionAdmission({
    schema: AUTONOMOUS_ACTION_EXECUTION_SCHEMA,
    version: AUTONOMOUS_ACTION_EXECUTION_VERSION,
    status,
    plan_digest: plan.plan_digest,
    task_digest: plan.task_digest,
    selected_domains: [...plan.selected_domains],
    execution_path: executionPath,
    reviewed,
    required_approvals: required,
    approved_approvals: approved,
    missing_approvals: missing,
    review_reasons: unique(reviewReasons),
    blocking_reasons: blockingReasons,
    next_action: next.nextAction,
    next_actions: next.nextActions,
  });
}
