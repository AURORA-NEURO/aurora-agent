import { ArgumentError } from "./errors.js";
import { validateAutonomousDomainPolicy, type AutonomousDomainPolicy } from "./autonomous-domain-policy.js";
import { validateAutonomousDomainTaskLens, type AutonomousDomainTaskLens } from "./autonomous-task-lens.js";
import { AUTONOMOUS_TASK_INTENT_DOMAINS, validateAutonomousTaskIntent, type AutonomousTaskIntent } from "./autonomous-task-intent.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_TASK_DECISION_SCHEMA = "bioprism-autonomous-task-decision/0.1" as const;
export const AUTONOMOUS_TASK_DECISION_VERSION = "0.1" as const;
export const AUTONOMOUS_TASK_DECISION_POSTURES = ["admitted", "review_required", "blocked"] as const;
export const AUTONOMOUS_TASK_DECISION_PATHS = ["provider", "evidence_first", "workflow", "planning", "cross_domain"] as const;
export const AUTONOMOUS_TASK_DECISION_APPROVALS = ["provider_call", "evidence_dispatch", "plan_acceptance", "effect_approval", "evaluator_settlement"] as const;
export const AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES = ["optional", "required_before_provider"] as const;
export const MAX_AUTONOMOUS_TASK_DECISION_ITEMS = 12;
const MAX_DECISION_TEXT_BYTES = 512;

function text(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > MAX_DECISION_TEXT_BYTES) throw new ArgumentError(`${name} is outside the task-decision text bound`);
  return value;
}

function items(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_TASK_DECISION_ITEMS) throw new ArgumentError(`${name} exceeds the task-decision item bound`);
  const result = value.map((item) => text(`${name} item`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate items`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function unique(values: readonly string[]): string[] { return [...new Set(values)]; }

export interface AutonomousTaskDecision extends JsonObject {
  schema: typeof AUTONOMOUS_TASK_DECISION_SCHEMA;
  decision_version: typeof AUTONOMOUS_TASK_DECISION_VERSION;
  domain: AutonomousTaskIntent["domain"];
  workflow_id: string;
  task_digest: string;
  intent_id: string;
  intent_digest: string;
  lens_digest: string;
  policy_digest: string;
  decision_id: string;
  posture: typeof AUTONOMOUS_TASK_DECISION_POSTURES[number];
  recommended_path: typeof AUTONOMOUS_TASK_DECISION_PATHS[number];
  requested_effect: AutonomousTaskIntent["requested_effect"];
  evidence_posture: AutonomousDomainPolicy["evidence_mode"];
  required_model_capabilities: string[];
  preferred_model_capabilities: string[];
  approval_requirements: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  next_actions: string[];
  decision_digest: string;
  authorization: "guidance_only;provider_source_tool_and_effect_authority_remain_separate";
  retention: "value_only_decision_metadata;task_text_not_retained";
  secret_material: "never_returned";
}

type DecisionDescriptor = Omit<AutonomousTaskDecision, "decision_digest" | "authorization" | "retention" | "secret_material">;

function descriptorFor(decision: AutonomousTaskDecision | DecisionDescriptor): DecisionDescriptor {
  const value = decision as AutonomousTaskDecision;
  return {
    schema: AUTONOMOUS_TASK_DECISION_SCHEMA,
    decision_version: AUTONOMOUS_TASK_DECISION_VERSION,
    domain: value.domain,
    workflow_id: value.workflow_id,
    task_digest: value.task_digest,
    intent_id: value.intent_id,
    intent_digest: value.intent_digest,
    lens_digest: value.lens_digest,
    policy_digest: value.policy_digest,
    decision_id: value.decision_id,
    posture: value.posture,
    recommended_path: value.recommended_path,
    requested_effect: value.requested_effect,
    evidence_posture: value.evidence_posture,
    required_model_capabilities: [...value.required_model_capabilities],
    preferred_model_capabilities: [...value.preferred_model_capabilities],
    approval_requirements: [...value.approval_requirements],
    review_reasons: [...value.review_reasons],
    blocking_reasons: [...value.blocking_reasons],
    next_actions: [...value.next_actions],
  };
}

export function autonomousTaskDecisionDigest(decision: AutonomousTaskDecision | DecisionDescriptor): string {
  return digestJsonSync(descriptorFor(decision));
}

export function autonomousTaskDecisionPromptContract(decision: AutonomousTaskDecision, compact = false): JsonObject {
  if (!decision || decision.schema !== AUTONOMOUS_TASK_DECISION_SCHEMA) throw new ArgumentError("task decision prompt contract requires a valid decision");
  const result: JsonObject = {
    schema: AUTONOMOUS_TASK_DECISION_SCHEMA,
    decision_id: decision.decision_id,
    decision_digest: decision.decision_digest,
    intent_digest: decision.intent_digest,
    posture: decision.posture,
    recommended_path: decision.recommended_path,
    requested_effect: decision.requested_effect,
    evidence_posture: decision.evidence_posture,
    approval_requirements: [...decision.approval_requirements],
    review_reasons: [...decision.review_reasons],
    blocking_reasons: [...decision.blocking_reasons],
    authority: "guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
  };
  if (!compact) Object.assign(result, { required_model_capabilities: [...decision.required_model_capabilities], preferred_model_capabilities: [...decision.preferred_model_capabilities], next_actions: [...decision.next_actions] });
  result.secret_material = "never_returned";
  return result;
}

export function inferAutonomousTaskDecision(args: {
  intent: AutonomousTaskIntent;
  lens: AutonomousDomainTaskLens;
  policy: AutonomousDomainPolicy;
  requiredModelCapabilities: readonly string[];
}): AutonomousTaskDecision {
  if (!args.intent || args.intent.schema !== "bioprism-autonomous-task-intent/0.1" || !args.lens || args.lens.schema !== "bioprism-autonomous-domain-task-lens/0.1" || !args.policy || args.policy.schema !== "bioprism-autonomous-domain-policy/0.1") throw new ArgumentError("task decision requires a valid intent, lens, and policy");
  if (args.intent.domain !== args.lens.domain || args.intent.domain !== args.policy.domain) throw new ArgumentError("task decision intent, lens, and policy domains must agree");
  const required = items("task decision requiredModelCapabilities", args.requiredModelCapabilities);
  if (!required.length) throw new ArgumentError("task decision requires at least one model capability");
  const action = args.intent.action_mode;
  const path = args.intent.domain === "cross_domain" || action === "coordinate" || action === "synthesize"
    ? "cross_domain"
    : action === "create" || action === "modify"
      ? "workflow"
      : action === "plan"
        ? "planning"
        : args.policy.evidence_mode === "required_before_provider" || ["observe", "investigate", "analyze", "compare", "evaluate"].includes(action)
          ? "evidence_first"
          : "provider";
  const preferred = unique([
    ...args.lens.model_capability_hints,
    ...(path === "workflow" ? ["structured_output"] : []),
    ...(path === "cross_domain" ? ["coordination", "structured_output"] : []),
    ...(["analyze", "compare", "evaluate", "synthesize"].includes(action) ? ["reasoning"] : []),
  ]);
  const approvals: string[] = ["provider_call"];
  const review: string[] = [];
  const blocked: string[] = [];
  if (args.policy.evidence_mode === "required_before_provider") { approvals.push("evidence_dispatch"); review.push("evidence_required_before_provider"); }
  if (["workflow", "planning", "cross_domain"].includes(path) && args.policy.plan_acceptance_required) { approvals.push("plan_acceptance"); review.push("plan_acceptance_required"); }
  if (args.intent.requested_effect === "external_effect") {
    if (args.policy.effect_mode === "forbidden") blocked.push("requested_effect_forbidden_by_domain_policy");
    else { approvals.push("effect_approval"); review.push("external_effect_requires_explicit_approval"); }
  } else if (args.intent.requested_effect === "local_change" && args.policy.effect_mode === "approval_gated") { approvals.push("effect_approval"); review.push("local_change_requires_explicit_approval"); }
  if (args.policy.evaluator_required) approvals.push("evaluator_settlement");
  for (const flag of args.intent.ambiguity_flags) review.push(`intent:${flag}`);
  if (args.intent.domain === "cross_domain" || action === "coordinate" || action === "synthesize") review.push("specialist_boundaries_require_review");
  if (args.intent.risk_signals.length) review.push("domain_risk_signals_require_review");
  const approvalRequirements = unique(approvals);
  const reviewReasons = unique(review);
  const blockingReasons = unique(blocked);
  const posture = blockingReasons.length ? "blocked" : reviewReasons.length ? "review_required" : "admitted";
  const nextActions = blockingReasons.length
    ? ["stop_before_provider_dispatch", "resolve_domain_policy_conflict"]
    : reviewReasons.length
      ? ["review_task_intent_and_decision", "satisfy_required_approval_gates", ...(approvalRequirements.includes("evidence_dispatch") ? ["acquire_and_review_required_evidence"] : [])]
      : ["select_model", "request_provider_call_approval"];
  if (approvalRequirements.includes("evaluator_settlement")) nextActions.push("settle_explicit_evaluator_feedback_after_run");
  const descriptor: DecisionDescriptor = {
    schema: AUTONOMOUS_TASK_DECISION_SCHEMA,
    decision_version: AUTONOMOUS_TASK_DECISION_VERSION,
    domain: args.intent.domain,
    workflow_id: args.intent.workflow_id,
    task_digest: args.intent.task_digest,
    intent_id: args.intent.intent_id,
    intent_digest: args.intent.intent_digest,
    lens_digest: args.lens.lens_digest,
    policy_digest: args.policy.policy_digest,
    decision_id: `${args.intent.intent_id}:${posture}:${path}`,
    posture,
    recommended_path: path,
    requested_effect: args.intent.requested_effect,
    evidence_posture: args.policy.evidence_mode,
    required_model_capabilities: required,
    preferred_model_capabilities: preferred,
    approval_requirements: approvalRequirements,
    review_reasons: reviewReasons,
    blocking_reasons: blockingReasons,
    next_actions: unique(nextActions),
  };
  return Object.freeze({ ...descriptor, decision_digest: autonomousTaskDecisionDigest(descriptor), authorization: "guidance_only;provider_source_tool_and_effect_authority_remain_separate", retention: "value_only_decision_metadata;task_text_not_retained", secret_material: "never_returned" }) as AutonomousTaskDecision;
}

/**
 * Validate a persisted decision and optionally replay it against live task artifacts.
 *
 * The serialized decision is guidance metadata, never an authorization token. Structural
 * validation checks its canonical digest, bounded fields, and markers. When the original intent,
 * lens, and policy are supplied, deterministic inference is rerun and every descriptor field
 * must match before a caller crosses a provider, source, tool, evaluator, or effect boundary.
 */
export function validateAutonomousTaskDecision(
  value: AutonomousTaskDecision | unknown,
  options: {
    intent?: AutonomousTaskIntent;
    lens?: AutonomousDomainTaskLens;
    policy?: AutonomousDomainPolicy | JsonObject;
    requiredModelCapabilities?: readonly string[];
  } = {},
): AutonomousTaskDecision {
  let decision: AutonomousTaskDecision;
  if (value && typeof value === "object" && !Array.isArray(value) && (value as Partial<AutonomousTaskDecision>).schema === AUTONOMOUS_TASK_DECISION_SCHEMA) {
    const candidate = value as Record<string, unknown>;
    const allowed = new Set([
      "schema", "decision_version", "domain", "workflow_id", "task_digest", "intent_id", "intent_digest",
      "lens_digest", "policy_digest", "decision_id", "posture", "recommended_path", "requested_effect",
      "evidence_posture", "required_model_capabilities", "preferred_model_capabilities", "approval_requirements",
      "review_reasons", "blocking_reasons", "next_actions", "decision_digest", "authorization", "retention",
      "secret_material",
    ]);
    if (Object.keys(candidate).some((key) => !allowed.has(key))) throw new ArgumentError("task decision contains unsupported fields");
    if (
      candidate.authorization !== "guidance_only;provider_source_tool_and_effect_authority_remain_separate"
      || candidate.retention !== "value_only_decision_metadata;task_text_not_retained"
      || candidate.secret_material !== "never_returned"
    ) throw new ArgumentError("task decision markers are invalid");
    const asItems = (name: string): string[] => {
      const raw = candidate[name];
      if (!Array.isArray(raw)) throw new ArgumentError(`task decision ${name} must be a sequence`);
      return items(`task decision ${name}`, raw);
    };
    const domain = text("task decision domain", candidate.domain);
    if (!AUTONOMOUS_TASK_INTENT_DOMAINS.includes(domain as typeof AUTONOMOUS_TASK_INTENT_DOMAINS[number])) throw new ArgumentError("task decision domain is unsupported");
    const workflowId = text("task decision workflow_id", candidate.workflow_id);
    const decisionId = text("task decision decision_id", candidate.decision_id);
    const taskDigest = digest("task decision task_digest", candidate.task_digest);
    const intentDigest = digest("task decision intent_digest", candidate.intent_digest);
    const lensDigest = digest("task decision lens_digest", candidate.lens_digest);
    const policyDigest = digest("task decision policy_digest", candidate.policy_digest);
    const decisionDigest = digest("task decision decision_digest", candidate.decision_digest);
    if (candidate.decision_version !== AUTONOMOUS_TASK_DECISION_VERSION) throw new ArgumentError("unsupported task-decision version");
    if (!AUTONOMOUS_TASK_DECISION_POSTURES.includes(candidate.posture as typeof AUTONOMOUS_TASK_DECISION_POSTURES[number])) throw new ArgumentError("task decision posture is unsupported");
    if (!AUTONOMOUS_TASK_DECISION_PATHS.includes(candidate.recommended_path as typeof AUTONOMOUS_TASK_DECISION_PATHS[number])) throw new ArgumentError("task decision recommended_path is unsupported");
    if (!["none", "local_change", "external_effect"].includes(candidate.requested_effect as string)) throw new ArgumentError("task decision requested_effect is unsupported");
    const evidencePosture = text("task decision evidence_posture", candidate.evidence_posture);
    if (!AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES.includes(evidencePosture as typeof AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES[number])) throw new ArgumentError("task decision evidence_posture is unsupported");
    const descriptor: DecisionDescriptor = {
      schema: AUTONOMOUS_TASK_DECISION_SCHEMA,
      decision_version: AUTONOMOUS_TASK_DECISION_VERSION,
      domain: domain as AutonomousTaskIntent["domain"],
      workflow_id: workflowId,
      task_digest: taskDigest,
      intent_id: text("task decision intent_id", candidate.intent_id),
      intent_digest: intentDigest,
      lens_digest: lensDigest,
      policy_digest: policyDigest,
      decision_id: decisionId,
      posture: candidate.posture as AutonomousTaskDecision["posture"],
      recommended_path: candidate.recommended_path as AutonomousTaskDecision["recommended_path"],
      requested_effect: candidate.requested_effect as AutonomousTaskDecision["requested_effect"],
      evidence_posture: evidencePosture as AutonomousTaskDecision["evidence_posture"],
      required_model_capabilities: asItems("required_model_capabilities"),
      preferred_model_capabilities: asItems("preferred_model_capabilities"),
      approval_requirements: asItems("approval_requirements"),
      review_reasons: asItems("review_reasons"),
      blocking_reasons: asItems("blocking_reasons"),
      next_actions: asItems("next_actions"),
    };
    const approvals = descriptor.approval_requirements as string[];
    if (approvals.some((approval) => !AUTONOMOUS_TASK_DECISION_APPROVALS.includes(approval as typeof AUTONOMOUS_TASK_DECISION_APPROVALS[number]))) throw new ArgumentError("task decision approval_requirements contains an unsupported gate");
    if (autonomousTaskDecisionDigest(descriptor) !== decisionDigest) throw new ArgumentError("task decision digest does not match its metadata");
    decision = Object.freeze({
      ...descriptor,
      decision_digest: decisionDigest,
      authorization: "guidance_only;provider_source_tool_and_effect_authority_remain_separate",
      retention: "value_only_decision_metadata;task_text_not_retained",
      secret_material: "never_returned",
    }) as AutonomousTaskDecision;
  } else {
    throw new ArgumentError("task decision must be an object");
  }

  const supplied = [options.intent, options.lens, options.policy];
  if (supplied.some((entry) => entry !== undefined)) {
    if (!options.intent || !options.lens || !options.policy) throw new ArgumentError("task decision replay requires intent, lens, and policy together");
    const reviewedLens = validateAutonomousDomainTaskLens(options.lens);
    const reviewedIntent = validateAutonomousTaskIntent(options.intent, { lens: reviewedLens });
    const reviewedPolicy = validateAutonomousDomainPolicy(options.policy, reviewedIntent.domain);
    const replay = inferAutonomousTaskDecision({
      intent: reviewedIntent,
      lens: reviewedLens,
      policy: reviewedPolicy,
      requiredModelCapabilities: options.requiredModelCapabilities ?? decision.required_model_capabilities,
    });
    if (JSON.stringify(descriptorFor(replay)) !== JSON.stringify(descriptorFor(decision))) throw new ArgumentError("task decision does not match the supplied intent, lens, and policy");
  } else if (options.requiredModelCapabilities !== undefined) {
    throw new ArgumentError("task decision replay capabilities require intent, lens, and policy");
  }
  return decision;
}
