import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainPolicy } from "./autonomous-domain-policy.js";
import type { AutonomousDomainTaskLens } from "./autonomous-task-lens.js";
import type { AutonomousTaskDecision } from "./autonomous-task-decision.js";
import type { AutonomousTaskIntent } from "./autonomous-task-intent.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Provider-free clarification planning for ambiguous or risky autonomous tasks.
 *
 * Intent and task-decision inference already notices ambiguity, missing output contracts,
 * evidence obligations, and effect risk.  This module turns those signals into a bounded
 * questionnaire that an application can show to a user.  Question text is generated only from
 * reviewed metadata; task text and answers never enter the returned durable projection.
 */
export const AUTONOMOUS_TASK_CLARIFICATION_SCHEMA = "bioprism-autonomous-task-clarification/0.1" as const;
export const AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA = "bioprism-autonomous-task-clarification-answer/0.1" as const;
export const AUTONOMOUS_TASK_CLARIFICATION_VERSION = "0.1" as const;
export const AUTONOMOUS_TASK_CLARIFICATION_STATUSES = ["not_required", "required", "blocked"] as const;
export const AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES = ["resolved", "still_required", "blocked"] as const;
export const AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS = ["action", "output", "scope", "evidence", "authority", "reviewer", "specialist", "success"] as const;
export const AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS = ["text", "choice", "approval_scope"] as const;
export const MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS = 8;
export const MAX_AUTONOMOUS_TASK_CLARIFICATION_OPTIONS = 12;
export const MAX_AUTONOMOUS_TASK_CLARIFICATION_TEXT_BYTES = 512;
export const MAX_AUTONOMOUS_TASK_CLARIFICATION_ANSWER_BYTES = 4_096;

export type AutonomousTaskClarificationStatus = typeof AUTONOMOUS_TASK_CLARIFICATION_STATUSES[number];
export type AutonomousTaskClarificationResolutionStatus = typeof AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES[number];
export type AutonomousTaskClarificationQuestionKind = typeof AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS[number];
export type AutonomousTaskClarificationAnswerKind = typeof AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS[number];

export class AutonomousTaskClarificationError extends ArgumentError {}

function text(name: string, value: unknown, maximum = MAX_AUTONOMOUS_TASK_CLARIFICATION_TEXT_BYTES): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\0") || new TextEncoder().encode(value).byteLength > maximum) throw new AutonomousTaskClarificationError(`${name} is outside its bound`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new AutonomousTaskClarificationError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function items(name: string, value: unknown, maximum = MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS, allowEmpty = true): string[] {
  if (!Array.isArray(value) || (!allowEmpty && value.length === 0) || value.length > maximum) throw new AutonomousTaskClarificationError(`${name} exceeds its bound`);
  const result = value.map((item) => text(`${name} item`, item));
  if (new Set(result).size !== result.length) throw new AutonomousTaskClarificationError(`${name} contains duplicate values`);
  return result;
}

function count(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new AutonomousTaskClarificationError(`${name} is outside its bound`);
  return value;
}

function unique(values: readonly string[]): string[] { return [...new Set(values)]; }

export interface AutonomousTaskClarificationQuestion extends JsonObject {
  question_id: string;
  kind: AutonomousTaskClarificationQuestionKind;
  dimension: string;
  priority: number;
  required: boolean;
  answer_kind: AutonomousTaskClarificationAnswerKind;
  prompt: string;
  reason_code: string;
  options: string[];
}

export interface AutonomousTaskClarificationPlan extends JsonObject {
  schema: typeof AUTONOMOUS_TASK_CLARIFICATION_SCHEMA;
  clarification_version: typeof AUTONOMOUS_TASK_CLARIFICATION_VERSION;
  domain: AutonomousTaskIntent["domain"];
  workflow_id: string;
  task_digest: string;
  intent_id: string;
  intent_digest: string;
  lens_digest: string;
  policy_digest: string;
  decision_digest: string;
  status: AutonomousTaskClarificationStatus;
  questions: AutonomousTaskClarificationQuestion[];
  review_dimensions: string[];
  missing_contracts: string[];
  omitted_contracts: string[];
  next_actions: string[];
  plan_digest: string;
  retention: "value_only_clarification_metadata;task_text_and_answers_not_retained";
  authorization: "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions";
  secret_material: "never_returned";
}

export interface AutonomousTaskClarificationResolution extends JsonObject {
  schema: typeof AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA;
  plan_digest: string;
  task_digest: string;
  status: AutonomousTaskClarificationResolutionStatus;
  answered_count: number;
  required_answer_count: number;
  unanswered_question_ids: string[];
  answer_digests: Array<{ question_id: string; answer_digest: string }>;
  resolution_digest: string;
  retention: "answer_digests_only;answer_values_not_retained";
  authorization: "review_receipt_only;requires_recompiled_intent_and_decision";
  secret_material: "never_returned";
}

function validateQuestion(value: unknown): AutonomousTaskClarificationQuestion {
  if (!isObject(value)) throw new AutonomousTaskClarificationError("clarification question must be an object");
  const allowed = new Set(["question_id", "kind", "dimension", "priority", "required", "answer_kind", "prompt", "reason_code", "options"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousTaskClarificationError("clarification question contains unsupported fields");
  const questionId = text("clarification question_id", value.question_id);
  const kind = text("clarification question kind", value.kind) as AutonomousTaskClarificationQuestionKind;
  if (!AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS.includes(kind)) throw new AutonomousTaskClarificationError("clarification question kind is unsupported");
  const dimension = text("clarification question dimension", value.dimension);
  if (typeof value.priority !== "number" || !Number.isSafeInteger(value.priority) || value.priority < 1 || value.priority > 4) throw new AutonomousTaskClarificationError("clarification question priority is outside its bound");
  if (typeof value.required !== "boolean") throw new AutonomousTaskClarificationError("clarification question required must be boolean");
  const answerKind = text("clarification question answer_kind", value.answer_kind) as AutonomousTaskClarificationAnswerKind;
  if (!AUTONOMOUS_TASK_CLARIFICATION_ANSWER_KINDS.includes(answerKind)) throw new AutonomousTaskClarificationError("clarification question answer_kind is unsupported");
  const prompt = text("clarification question prompt", value.prompt);
  const reasonCode = text("clarification question reason_code", value.reason_code);
  const options = items("clarification question options", value.options, MAX_AUTONOMOUS_TASK_CLARIFICATION_OPTIONS);
  if (answerKind === "choice" && options.length === 0) throw new AutonomousTaskClarificationError("choice clarification questions require options");
  if (answerKind !== "choice" && options.length > 0) throw new AutonomousTaskClarificationError("non-choice clarification questions cannot have options");
  return { question_id: questionId, kind, dimension, priority: value.priority, required: value.required, answer_kind: answerKind, prompt, reason_code: reasonCode, options };
}

type AutonomousTaskClarificationPlanInput = {
  schema?: typeof AUTONOMOUS_TASK_CLARIFICATION_SCHEMA;
  clarification_version: typeof AUTONOMOUS_TASK_CLARIFICATION_VERSION;
  domain: AutonomousTaskIntent["domain"];
  workflow_id: string;
  task_digest: string;
  intent_id: string;
  intent_digest: string;
  lens_digest: string;
  policy_digest: string;
  decision_digest: string;
  status: AutonomousTaskClarificationStatus;
  questions: AutonomousTaskClarificationQuestion[];
  review_dimensions: string[];
  missing_contracts: string[];
  omitted_contracts: string[];
  next_actions: string[];
};

function planDescriptor(value: AutonomousTaskClarificationPlanInput): JsonObject {
  return {
    schema: AUTONOMOUS_TASK_CLARIFICATION_SCHEMA,
    clarification_version: value.clarification_version,
    domain: value.domain,
    workflow_id: value.workflow_id,
    task_digest: value.task_digest,
    intent_id: value.intent_id,
    intent_digest: value.intent_digest,
    lens_digest: value.lens_digest,
    policy_digest: value.policy_digest,
    decision_digest: value.decision_digest,
    status: value.status,
    questions: value.questions.map((question) => ({ ...question, options: [...question.options] })),
    review_dimensions: [...value.review_dimensions],
    missing_contracts: [...value.missing_contracts],
    omitted_contracts: [...value.omitted_contracts],
    next_actions: [...value.next_actions],
  };
}

function buildPlan(value: AutonomousTaskClarificationPlanInput): AutonomousTaskClarificationPlan {
  const descriptor = planDescriptor(value);
  return {
    ...descriptor,
    plan_digest: digestJsonSync(descriptor),
    retention: "value_only_clarification_metadata;task_text_and_answers_not_retained",
    authorization: "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
    secret_material: "never_returned",
  } as AutonomousTaskClarificationPlan;
}

function validatePlanShape(value: unknown): AutonomousTaskClarificationPlan {
  if (!isObject(value)) throw new AutonomousTaskClarificationError("clarification plan must be an object");
  const allowed = new Set(["schema", "clarification_version", "domain", "workflow_id", "task_digest", "intent_id", "intent_digest", "lens_digest", "policy_digest", "decision_digest", "status", "questions", "review_dimensions", "missing_contracts", "omitted_contracts", "next_actions", "plan_digest", "retention", "authorization", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousTaskClarificationError("clarification plan contains unsupported fields");
  if (value.schema !== AUTONOMOUS_TASK_CLARIFICATION_SCHEMA || value.clarification_version !== AUTONOMOUS_TASK_CLARIFICATION_VERSION || value.retention !== "value_only_clarification_metadata;task_text_and_answers_not_retained" || value.authorization !== "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions" || value.secret_material !== "never_returned") throw new AutonomousTaskClarificationError("clarification plan markers are invalid");
  const domain = text("clarification domain", value.domain) as AutonomousTaskIntent["domain"];
  const workflowId = text("clarification workflow_id", value.workflow_id);
  const taskDigest = digest("clarification task_digest", value.task_digest);
  const intentId = text("clarification intent_id", value.intent_id);
  const intentDigest = digest("clarification intent_digest", value.intent_digest);
  const lensDigest = digest("clarification lens_digest", value.lens_digest);
  const policyDigest = digest("clarification policy_digest", value.policy_digest);
  const decisionDigest = digest("clarification decision_digest", value.decision_digest);
  if (!AUTONOMOUS_TASK_CLARIFICATION_STATUSES.includes(value.status as AutonomousTaskClarificationStatus)) throw new AutonomousTaskClarificationError("clarification status is unsupported");
  if (!Array.isArray(value.questions) || value.questions.length > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS) throw new AutonomousTaskClarificationError("clarification questions exceed their bound");
  const questions = value.questions.map(validateQuestion);
  if (new Set(questions.map((question) => question.question_id)).size !== questions.length) throw new AutonomousTaskClarificationError("clarification question IDs must be unique");
  const reviewDimensions = items("clarification review_dimensions", value.review_dimensions);
  const missingContracts = items("clarification missing_contracts", value.missing_contracts);
  const omittedContracts = items("clarification omitted_contracts", value.omitted_contracts);
  const nextActions = items("clarification next_actions", value.next_actions);
  if (value.status === "not_required" && questions.length > 0) throw new AutonomousTaskClarificationError("not_required clarification cannot contain questions");
  if (value.status === "required" && questions.length === 0 && omittedContracts.length === 0) throw new AutonomousTaskClarificationError("required clarification must contain questions or omitted contracts");
  if (value.status === "blocked" && questions.length > 0) throw new AutonomousTaskClarificationError("blocked clarification cannot offer bypass questions");
  const candidate: AutonomousTaskClarificationPlanInput = { schema: AUTONOMOUS_TASK_CLARIFICATION_SCHEMA, clarification_version: AUTONOMOUS_TASK_CLARIFICATION_VERSION, domain, workflow_id: workflowId, task_digest: taskDigest, intent_id: intentId, intent_digest: intentDigest, lens_digest: lensDigest, policy_digest: policyDigest, decision_digest: decisionDigest, status: value.status as AutonomousTaskClarificationStatus, questions, review_dimensions: reviewDimensions, missing_contracts: missingContracts, omitted_contracts: omittedContracts, next_actions: nextActions };
  if (value.plan_digest !== digestJsonSync(planDescriptor(candidate))) throw new AutonomousTaskClarificationError("clarification plan digest does not match its metadata");
  return buildPlan(candidate);
}

function validateArtifacts(intent: AutonomousTaskIntent, lens: AutonomousDomainTaskLens, policy: AutonomousDomainPolicy, decision: AutonomousTaskDecision): void {
  if (!intent || intent.schema !== "bioprism-autonomous-task-intent/0.1" || !lens || lens.schema !== "bioprism-autonomous-domain-task-lens/0.1" || !policy || policy.schema !== "bioprism-autonomous-domain-policy/0.1" || !decision || decision.schema !== "bioprism-autonomous-task-decision/0.1") throw new AutonomousTaskClarificationError("clarification requires valid intent, lens, policy, and decision");
  if (intent.domain !== lens.domain || intent.domain !== policy.domain || intent.domain !== decision.domain) throw new AutonomousTaskClarificationError("clarification artifacts must use the same domain");
  if (intent.workflow_id !== decision.workflow_id || intent.task_digest !== decision.task_digest || intent.intent_id !== decision.intent_id || intent.intent_digest !== decision.intent_digest || lens.lens_digest !== decision.lens_digest || policy.policy_digest !== decision.policy_digest) throw new AutonomousTaskClarificationError("clarification artifacts are not bound to the same decision");
}

function question(intent: AutonomousTaskIntent, args: { kind: AutonomousTaskClarificationQuestionKind; dimension: string; priority: number; answerKind: AutonomousTaskClarificationAnswerKind; reasonCode: string; prompt: string; options?: readonly string[] }): AutonomousTaskClarificationQuestion {
  return { question_id: `${intent.intent_id}:clarify:${args.kind}`, kind: args.kind, dimension: args.dimension, priority: args.priority, required: true, answer_kind: args.answerKind, prompt: args.prompt, reason_code: args.reasonCode, options: unique(args.options ?? []) };
}

/** Build one deterministic questionnaire from existing provider-free decision artifacts. */
export function planAutonomousTaskClarification(args: { intent: AutonomousTaskIntent; lens: AutonomousDomainTaskLens; policy: AutonomousDomainPolicy; decision: AutonomousTaskDecision; maxQuestions?: number }): AutonomousTaskClarificationPlan {
  validateArtifacts(args.intent, args.lens, args.policy, args.decision);
  const maxQuestions = args.maxQuestions ?? MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS;
  if (!Number.isSafeInteger(maxQuestions) || maxQuestions < 1 || maxQuestions > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS) throw new AutonomousTaskClarificationError("maxQuestions is outside its bound");
  const { intent, lens, policy, decision } = args;
  if (decision.posture === "blocked") {
    const missingContracts = unique([...decision.blocking_reasons, "policy_blocker"]);
    const nextActions = unique([...decision.next_actions, "resolve_blocking_policy_before_provider"]);
    return buildPlan({ schema: AUTONOMOUS_TASK_CLARIFICATION_SCHEMA, clarification_version: AUTONOMOUS_TASK_CLARIFICATION_VERSION, domain: intent.domain, workflow_id: intent.workflow_id, task_digest: intent.task_digest, intent_id: intent.intent_id, intent_digest: intent.intent_digest, lens_digest: lens.lens_digest, policy_digest: policy.policy_digest, decision_digest: decision.decision_digest, status: "blocked", questions: [], review_dimensions: [...lens.planning_dimensions], missing_contracts: missingContracts, omitted_contracts: [], next_actions: nextActions });
  }
  const flags = new Set(intent.ambiguity_flags);
  const approvals = new Set(decision.approval_requirements);
  const candidates: AutonomousTaskClarificationQuestion[] = [];
  const firstDimension = lens.planning_dimensions[0]!;
  const lastDimension = lens.planning_dimensions[lens.planning_dimensions.length - 1]!;
  const actionOptions = unique([intent.action_mode, ...intent.alternative_action_modes, "other"]);
  if (flags.has("missing_action_signal") || flags.has("competing_action_modes")) candidates.push(question(intent, { kind: "action", dimension: firstDimension, priority: 1, answerKind: "choice", reasonCode: "ambiguous_action_mode", prompt: `Choose the primary action for the reviewed ${intent.domain} workflow.`, options: actionOptions }));
  if (flags.has("no_explicit_output_contract")) candidates.push(question(intent, { kind: "output", dimension: "output_contract", priority: 1, answerKind: "text", reasonCode: "missing_output_contract", prompt: `What concrete output should the ${intent.domain} workflow produce? Name the artifact, decision, or handoff.` }));
  if (flags.has("uncertainty_language")) candidates.push(question(intent, { kind: "success", dimension: lastDimension, priority: 2, answerKind: "text", reasonCode: "uncertainty_tolerance_missing", prompt: `What observable success criterion and acceptable uncertainty should end the ${intent.domain} workflow?` }));
  if (intent.requested_effect === "external_effect" || approvals.has("effect_approval")) candidates.push(question(intent, { kind: "authority", dimension: "authority", priority: 1, answerKind: "approval_scope", reasonCode: "effect_scope_and_authority_missing", prompt: "What exact effect is in scope, who approves it, and what rollback or postcondition is required?" }));
  if (policy.evidence_mode === "required_before_provider" || approvals.has("evidence_dispatch")) candidates.push(question(intent, { kind: "evidence", dimension: lens.evidence_priorities[0]!, priority: 1, answerKind: "text", reasonCode: "evidence_boundary_missing", prompt: `Which caller-owned evidence or source boundary must be satisfied before ${intent.domain} provider work?` }));
  if (intent.domain === "cross_domain" || decision.review_reasons.includes("specialist_boundaries_require_review") || intent.action_mode === "coordinate" || intent.action_mode === "synthesize") candidates.push(question(intent, { kind: "specialist", dimension: "specialist_contracts", priority: 1, answerKind: "text", reasonCode: "specialist_scope_missing", prompt: "Which specialist domains or handoff boundaries must participate in this cross-domain result?" }));
  const substantiveRisks = intent.risk_signals.filter((signal) => signal !== "domain_policy_review" && signal !== "output_contract_missing");
  if (substantiveRisks.length > 0 || decision.review_reasons.includes("risk_signals_require_review")) candidates.push(question(intent, { kind: "reviewer", dimension: lastDimension, priority: 2, answerKind: "text", reasonCode: "accountable_reviewer_missing", prompt: `Which qualified reviewer or accountable owner must review the ${intent.domain} result before reliance?` }));
  if (decision.posture === "review_required" && candidates.length === 0) candidates.push(question(intent, { kind: "scope", dimension: firstDimension, priority: 2, answerKind: "text", reasonCode: "review_scope_missing", prompt: `What scope should the reviewed ${intent.domain} workflow cover, and what is explicitly out of scope?` }));
  const order = new Map(AUTONOMOUS_TASK_CLARIFICATION_QUESTION_KINDS.map((kind, index) => [kind, index]));
  candidates.sort((left, right) => left.priority - right.priority || (order.get(left.kind)! - order.get(right.kind)!));
  const selected = candidates.slice(0, maxQuestions);
  const omittedContracts = candidates.slice(maxQuestions).map((item) => item.reason_code);
  const missingContracts = unique([...candidates.map((item) => item.reason_code), ...omittedContracts, ...(omittedContracts.length ? ["clarification_question_limit_reached"] : [])]);
  const status: AutonomousTaskClarificationStatus = selected.length > 0 || omittedContracts.length > 0 ? "required" : "not_required";
  const nextActions = status === "required" ? unique(["answer_clarification_questions", "recompile_intent_and_decision_before_execution", ...decision.next_actions]) : ["continue_to_reviewed_execution_boundary"];
  return buildPlan({ schema: AUTONOMOUS_TASK_CLARIFICATION_SCHEMA, clarification_version: AUTONOMOUS_TASK_CLARIFICATION_VERSION, domain: intent.domain, workflow_id: intent.workflow_id, task_digest: intent.task_digest, intent_id: intent.intent_id, intent_digest: intent.intent_digest, lens_digest: lens.lens_digest, policy_digest: policy.policy_digest, decision_digest: decision.decision_digest, status, questions: selected, review_dimensions: [...lens.planning_dimensions], missing_contracts: missingContracts, omitted_contracts: omittedContracts, next_actions: nextActions });
}

/** Validate a persisted or caller-transferred clarification plan and its digest. */
export function validateAutonomousTaskClarificationPlan(value: unknown): AutonomousTaskClarificationPlan {
  return validatePlanShape(value);
}

function resolutionDescriptor(value: Pick<AutonomousTaskClarificationResolution, "plan_digest" | "task_digest" | "status" | "answered_count" | "required_answer_count" | "unanswered_question_ids" | "answer_digests">): JsonObject {
  return { schema: AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA, plan_digest: value.plan_digest, task_digest: value.task_digest, status: value.status, answered_count: value.answered_count, required_answer_count: value.required_answer_count, unanswered_question_ids: [...value.unanswered_question_ids], answer_digests: value.answer_digests.map((item) => ({ question_id: item.question_id, answer_digest: item.answer_digest })) };
}

/** Resolve transient answers into a plan-bound metadata-only review receipt. */
export function resolveAutonomousTaskClarification(plan: AutonomousTaskClarificationPlan | unknown, options: { taskDigest: string; answers: Readonly<Record<string, string>> }): AutonomousTaskClarificationResolution {
  if (!options || !isObject(options)) throw new AutonomousTaskClarificationError("clarification resolution options must be an object");
  const resolvedPlan = validateAutonomousTaskClarificationPlan(plan);
  const taskDigest = digest("clarification resolution taskDigest", options.taskDigest);
  if (taskDigest !== resolvedPlan.task_digest) throw new AutonomousTaskClarificationError("clarification answers do not match the task digest");
  if (!options || !isObject(options.answers)) throw new AutonomousTaskClarificationError("clarification answers must be an object");
  if (Object.keys(options.answers).length > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS) throw new AutonomousTaskClarificationError("clarification answers exceed their bound");
  const questions = new Map(resolvedPlan.questions.map((item) => [item.question_id, item]));
  for (const key of Object.keys(options.answers)) if (!questions.has(key)) throw new AutonomousTaskClarificationError("clarification answers contain an unknown question ID");
  if (resolvedPlan.status === "blocked") {
    if (Object.keys(options.answers).length > 0) throw new AutonomousTaskClarificationError("blocked clarification cannot accept answers");
    const descriptor = { plan_digest: resolvedPlan.plan_digest, task_digest: taskDigest, status: "blocked" as const, answered_count: 0, required_answer_count: 0, unanswered_question_ids: [], answer_digests: [] as Array<{ question_id: string; answer_digest: string }> };
    return { ...resolutionDescriptor(descriptor), resolution_digest: digestJsonSync(resolutionDescriptor(descriptor)), retention: "answer_digests_only;answer_values_not_retained", authorization: "review_receipt_only;requires_recompiled_intent_and_decision", secret_material: "never_returned" } as AutonomousTaskClarificationResolution;
  }
  const answerDigests: Array<{ question_id: string; answer_digest: string }> = [];
  for (const item of resolvedPlan.questions) {
    const answer = options.answers[item.question_id];
    if (answer === undefined) continue;
    text("clarification answer", answer, MAX_AUTONOMOUS_TASK_CLARIFICATION_ANSWER_BYTES);
    if (item.answer_kind === "choice" && !item.options.includes(answer)) throw new AutonomousTaskClarificationError(`clarification answer for ${item.question_id} is not one of the offered options`);
    answerDigests.push({ question_id: item.question_id, answer_digest: digestJsonSync({ schema: AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA, plan_digest: resolvedPlan.plan_digest, question_id: item.question_id, answer }) });
  }
  let unanswered = resolvedPlan.questions.filter((item) => item.required && options.answers[item.question_id] === undefined).map((item) => item.question_id);
  const requiredAnswerCount = resolvedPlan.questions.filter((item) => item.required).length;
  const status: AutonomousTaskClarificationResolutionStatus = unanswered.length === 0 && resolvedPlan.omitted_contracts.length === 0 ? "resolved" : "still_required";
  if (resolvedPlan.omitted_contracts.length > 0) unanswered = unique([...unanswered, "clarification_question_limit_reached"]);
  const descriptor = { plan_digest: resolvedPlan.plan_digest, task_digest: taskDigest, status, answered_count: answerDigests.length, required_answer_count: requiredAnswerCount, unanswered_question_ids: unanswered, answer_digests: answerDigests };
  return { ...resolutionDescriptor(descriptor), resolution_digest: digestJsonSync(resolutionDescriptor(descriptor)), retention: "answer_digests_only;answer_values_not_retained", authorization: "review_receipt_only;requires_recompiled_intent_and_decision", secret_material: "never_returned" } as AutonomousTaskClarificationResolution;
}

/** Rehydrate a receipt and optionally bind it to the exact plan that produced it. */
export function validateAutonomousTaskClarificationResolution(value: unknown, plan?: AutonomousTaskClarificationPlan | unknown): AutonomousTaskClarificationResolution {
  if (!isObject(value)) throw new AutonomousTaskClarificationError("clarification resolution must be an object");
  const allowed = new Set(["schema", "plan_digest", "task_digest", "status", "answered_count", "required_answer_count", "unanswered_question_ids", "answer_digests", "resolution_digest", "retention", "authorization", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new AutonomousTaskClarificationError("clarification resolution contains unsupported fields");
  if (value.schema !== AUTONOMOUS_TASK_CLARIFICATION_ANSWER_SCHEMA || value.retention !== "answer_digests_only;answer_values_not_retained" || value.authorization !== "review_receipt_only;requires_recompiled_intent_and_decision" || value.secret_material !== "never_returned") throw new AutonomousTaskClarificationError("clarification resolution markers are invalid");
  const planDigest = digest("clarification resolution plan_digest", value.plan_digest);
  const taskDigest = digest("clarification resolution task_digest", value.task_digest);
  const status = value.status as AutonomousTaskClarificationResolutionStatus;
  if (!AUTONOMOUS_TASK_CLARIFICATION_RESOLUTION_STATUSES.includes(status)) throw new AutonomousTaskClarificationError("clarification resolution status is unsupported");
  const answeredCount = count("clarification resolution answered_count", value.answered_count, MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS);
  const requiredAnswerCount = count("clarification resolution required_answer_count", value.required_answer_count, MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS);
  const unanswered = items("clarification resolution unanswered_question_ids", value.unanswered_question_ids);
  if (!Array.isArray(value.answer_digests) || value.answer_digests.length > MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS) throw new AutonomousTaskClarificationError("clarification answer digests exceed their bound");
  const answerDigests = value.answer_digests.map((raw) => {
    if (!isObject(raw)) throw new AutonomousTaskClarificationError("clarification answer digest row must be an object");
    return { question_id: text("clarification answer question_id", raw.question_id), answer_digest: digest("clarification answer digest", raw.answer_digest) };
  });
  if (new Set(answerDigests.map((item) => item.question_id)).size !== answerDigests.length) throw new AutonomousTaskClarificationError("clarification answer question IDs must be unique");
  if (answerDigests.length !== answeredCount) throw new AutonomousTaskClarificationError("clarification answered_count does not match answer digests");
  if (answeredCount > requiredAnswerCount) throw new AutonomousTaskClarificationError("clarification answered_count exceeds required_answer_count");
  if (status === "blocked" && (answeredCount > 0 || unanswered.length > 0)) throw new AutonomousTaskClarificationError("blocked clarification resolution cannot contain answer state");
  if (status === "resolved" && unanswered.length > 0) throw new AutonomousTaskClarificationError("resolved clarification resolution cannot have unanswered questions");
  const descriptor = resolutionDescriptor({ plan_digest: planDigest, task_digest: taskDigest, status, answered_count: answeredCount, required_answer_count: requiredAnswerCount, unanswered_question_ids: unanswered, answer_digests: answerDigests });
  if (value.resolution_digest !== digestJsonSync(descriptor)) throw new AutonomousTaskClarificationError("clarification resolution digest does not match its metadata");
  const resolution = { ...descriptor, resolution_digest: value.resolution_digest, retention: "answer_digests_only;answer_values_not_retained", authorization: "review_receipt_only;requires_recompiled_intent_and_decision", secret_material: "never_returned" } as AutonomousTaskClarificationResolution;

  if (plan !== undefined) {
    const resolvedPlan = validateAutonomousTaskClarificationPlan(plan);
    if (resolution.plan_digest !== resolvedPlan.plan_digest) throw new AutonomousTaskClarificationError("clarification resolution does not match the supplied plan");
    const questionIds = new Set(resolvedPlan.questions.map((question) => question.question_id));
    const marker = "clarification_question_limit_reached";
    const answerIds = new Set(answerDigests.map((item) => item.question_id));
    const unansweredIds = new Set(unanswered);
    if ([...answerIds].some((id) => !questionIds.has(id))) throw new AutonomousTaskClarificationError("clarification resolution contains an answer for an unknown question");
    if ([...unansweredIds].some((id) => id !== marker && !questionIds.has(id))) throw new AutonomousTaskClarificationError("clarification resolution contains an unknown unanswered question");
    if ([...answerIds].some((id) => unansweredIds.has(id))) throw new AutonomousTaskClarificationError("clarification resolution marks one question answered and unanswered");
    const requiredCount = resolvedPlan.questions.filter((question) => question.required).length;
    if (requiredAnswerCount !== requiredCount) throw new AutonomousTaskClarificationError("clarification resolution required count does not match the plan");
    const actualUnanswered = [...unansweredIds].filter((id) => questionIds.has(id)).length;
    if (answeredCount + actualUnanswered !== requiredCount) throw new AutonomousTaskClarificationError("clarification resolution does not account for every required question");
    if (resolvedPlan.status === "blocked" && status !== "blocked") throw new AutonomousTaskClarificationError("blocked clarification plan requires a blocked resolution");
    if (resolvedPlan.status !== "blocked" && status === "blocked") throw new AutonomousTaskClarificationError("non-blocked clarification plan cannot have a blocked resolution");
    if (status === "resolved" && resolvedPlan.omitted_contracts.length > 0) throw new AutonomousTaskClarificationError("clarification with omitted contracts cannot be resolved");
    if (status === "still_required" && actualUnanswered === 0 && resolvedPlan.omitted_contracts.length === 0) throw new AutonomousTaskClarificationError("complete clarification resolution must be marked resolved");
  }
  return resolution;
}
