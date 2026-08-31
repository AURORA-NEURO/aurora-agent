import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous.js";
import type { AutonomousDomainName } from "./autonomous.js";
import { digestJsonSync } from "./tooling.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { JsonObject } from "./types.js";

/** Strict, digest-bound composition contract for one executable workflow stage. */
export const AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-workflow-stage-response-evaluation/0.1" as const;
export const AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION = "1" as const;
export const AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES = ["completed", "proposed", "blocked", "not_attempted"] as const;
export const AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_PASS_THRESHOLD = 0.8;
export const MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS = 32;
export const MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES = 4_096;
export const MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_NOTES_BYTES = 16_000;

export type AutonomousWorkflowStageResponseStatus = typeof AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES[number];

export interface AutonomousWorkflowStageResponse extends JsonObject {
  stage_id: string;
  status: AutonomousWorkflowStageResponseStatus;
  evidence: string[];
  uncertainty: string[];
  notes: string;
  next_actions: string[];
}

export interface AutonomousWorkflowStageResponseEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: typeof AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  response_digest: string;
  signals: Record<string, number>;
  missing_signals: string[];
  reward: number;
  passed: boolean;
  failed: boolean;
  failure_class: string | null;
  feedback_digest: string;
  evidence_digest: string;
  replan_requested: boolean;
  replan_instruction: string | null;
  reward_input: AutonomousEvaluatorRewardInput;
  evaluator_authority: "workflow_stage_response_contract_only;not_external_truth";
  retention: "value_only;stage_response_and_credentials_not_retained";
  secret_material: "never_returned";
  evaluation_digest: string;
}

const SIGNAL_WEIGHTS: Readonly<Record<string, number>> = {
  schema_valid: 2,
  stage_identity: 1.5,
  status_declared: 1,
  evidence_present: 2,
  uncertainty_reported: 1.5,
  notes_present: 1,
  next_actions_present: 1,
  response_digest_bound: 1,
};
const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;
const SECRET_KEYS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey"]);
const CREDENTIAL_SHAPE = /\b(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})/i;
const STAGE_FIELDS = ["stage_id", "status", "evidence", "uncertainty", "notes", "next_actions"] as const;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number, allowEmpty = false): string {
  if (typeof value !== "string" || (!allowEmpty && value.trim().length === 0) || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!IDENTIFIER.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function assertSafeValue(value: unknown, depth = 0): void {
  if (depth > 16) throw new ArgumentError("workflow stage response is too deeply nested");
  if (typeof value === "string") {
    if (CREDENTIAL_SHAPE.test(value)) throw new ArgumentError("workflow stage response contains credential-shaped material");
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) assertSafeValue(item, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (SECRET_KEYS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj")) throw new ArgumentError("workflow stage response contains credential-shaped fields");
      assertSafeValue(child, depth + 1);
    }
  }
}

function exactKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const allowedSet = new Set(allowed);
  if (Object.keys(value).length !== allowed.length || Object.keys(value).some((key) => !allowedSet.has(key))) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function boundedList(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS) throw new ArgumentError(`${name} must contain at most ${MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS} entries`);
  return value.map((item, index) => boundedText(`${name}[${index}]`, item, MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES));
}

function normalizeStageResponse(value: unknown, stageId: string): AutonomousWorkflowStageResponse {
  if (!isObject(value)) throw new ArgumentError("workflow stage response must be an object");
  assertSafeValue(value);
  exactKeys("workflow stage response", value, STAGE_FIELDS);
  const expectedStageId = boundedIdentifier("workflow stage response expected stage_id", stageId);
  const actualStageId = boundedIdentifier("workflow stage response stage_id", value.stage_id);
  if (actualStageId !== expectedStageId) throw new ArgumentError("workflow stage response stage_id does not match the scheduled stage");
  if (!AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES.includes(value.status as AutonomousWorkflowStageResponseStatus)) throw new ArgumentError("workflow stage response status is invalid");
  return {
    stage_id: actualStageId,
    status: value.status as AutonomousWorkflowStageResponseStatus,
    evidence: boundedList("workflow stage response evidence", value.evidence),
    uncertainty: boundedList("workflow stage response uncertainty", value.uncertainty),
    notes: boundedText("workflow stage response notes", value.notes, MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_NOTES_BYTES, true),
    next_actions: boundedList("workflow stage response next_actions", value.next_actions),
  };
}

function normalizedEvaluation(value: unknown): AutonomousWorkflowStageResponseEvaluation {
  if (!isObject(value)) throw new ArgumentError("workflow stage response evaluation must be an object");
  assertSafeValue(value);
  const allowed = ["schema", "evaluator_id", "evaluator_version", "domain", "workflow_id", "workflow_digest", "stage_id", "response_digest", "signals", "missing_signals", "reward", "passed", "failed", "failure_class", "feedback_digest", "evidence_digest", "replan_requested", "replan_instruction", "reward_input", "evaluator_authority", "retention", "secret_material", "evaluation_digest"] as const;
  exactKeys("workflow stage response evaluation", value, allowed);
  if (value.schema !== AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA || value.evaluator_version !== AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION) throw new ArgumentError("workflow stage response evaluation schema or version is invalid");
  const evaluatorId = boundedIdentifier("workflow stage evaluation evaluator_id", value.evaluator_id);
  const domain = boundedIdentifier("workflow stage evaluation domain", value.domain);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("workflow stage evaluation domain is not supported");
  const workflowId = boundedIdentifier("workflow stage evaluation workflow_id", value.workflow_id);
  const stageId = boundedIdentifier("workflow stage evaluation stage_id", value.stage_id);
  const workflowDigest = boundedDigest("workflow stage evaluation workflow_digest", value.workflow_digest);
  const responseDigest = boundedDigest("workflow stage evaluation response_digest", value.response_digest);
  const feedbackDigest = boundedDigest("workflow stage evaluation feedback_digest", value.feedback_digest);
  const evidenceDigest = boundedDigest("workflow stage evaluation evidence_digest", value.evidence_digest);
  if (evidenceDigest !== responseDigest) throw new ArgumentError("workflow stage evaluation evidence_digest must match response_digest");
  if (value.evaluator_authority !== "workflow_stage_response_contract_only;not_external_truth" || value.retention !== "value_only;stage_response_and_credentials_not_retained" || value.secret_material !== "never_returned") throw new ArgumentError("workflow stage evaluation authority or retention markers are invalid");
  if (!isObject(value.signals) || Object.keys(value.signals).length !== Object.keys(SIGNAL_WEIGHTS).length || Object.keys(SIGNAL_WEIGHTS).some((signal) => !Object.prototype.hasOwnProperty.call(value.signals, signal))) throw new ArgumentError("workflow stage evaluation signals are incomplete");
  const signals: Record<string, number> = {};
  for (const signal of Object.keys(SIGNAL_WEIGHTS)) {
    const score = value.signals[signal];
    if (typeof score !== "number" || !Number.isFinite(score) || score < 0 || score > 1) throw new ArgumentError("workflow stage evaluation signals must be finite values within [0, 1]");
    signals[signal] = score;
  }
  if (!Array.isArray(value.missing_signals)) throw new ArgumentError("workflow stage evaluation missing_signals must be an array");
  const missing = value.missing_signals.map((signal, index) => boundedIdentifier(`workflow stage evaluation missing_signals[${index}]`, signal));
  if (new Set(missing).size !== missing.length || missing.some((signal) => !Object.prototype.hasOwnProperty.call(signals, signal) || signals[signal]! >= 1)) throw new ArgumentError("workflow stage evaluation missing_signals do not match signals");
  if (typeof value.reward !== "number" || !Number.isFinite(value.reward) || value.reward < 0 || value.reward > 1 || typeof value.passed !== "boolean" || typeof value.failed !== "boolean" || value.failed === value.passed || typeof value.replan_requested !== "boolean" || value.replan_requested !== value.failed) throw new ArgumentError("workflow stage evaluation reward or status flags are inconsistent");
  const failureClass = value.failure_class === null ? null : boundedIdentifier("workflow stage evaluation failure_class", value.failure_class);
  const replanInstruction = value.replan_instruction === null ? null : boundedText("workflow stage evaluation replan_instruction", value.replan_instruction, MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES);
  if ((!value.passed && (failureClass === null || replanInstruction === null)) || (value.passed && (failureClass !== null || replanInstruction !== null))) throw new ArgumentError("workflow stage evaluation failure guidance is inconsistent");
  if (!isObject(value.reward_input)) throw new ArgumentError("workflow stage evaluation reward_input is malformed");
  const rewardInput = value.reward_input as unknown as Record<string, unknown>;
  exactKeys("workflow stage evaluation reward_input", rewardInput, ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest"]);
  if (rewardInput.evaluator_id !== evaluatorId || rewardInput.evaluator_version !== AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION || rewardInput.reward !== value.reward || rewardInput.passed !== value.passed || rewardInput.failed !== value.failed || rewardInput.feedback_digest !== feedbackDigest || rewardInput.failure_class !== failureClass || rewardInput.evidence_digest !== evidenceDigest) throw new ArgumentError("workflow stage evaluation reward_input does not match its projection");
  const evaluationDigest = boundedDigest("workflow stage evaluation evaluation_digest", value.evaluation_digest);
  const { evaluation_digest: _ignored, ...descriptor } = value;
  if (digestJsonSync(descriptor) !== evaluationDigest) throw new ArgumentError("workflow stage evaluation digest does not match its projection");
  return {
    schema: value.schema as typeof AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA,
    evaluator_id: evaluatorId,
    evaluator_version: AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION,
    domain: domain as AutonomousDomainName,
    workflow_id: workflowId,
    workflow_digest: workflowDigest,
    stage_id: stageId,
    response_digest: responseDigest,
    signals,
    missing_signals: missing,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed,
    failure_class: failureClass,
    feedback_digest: feedbackDigest,
    evidence_digest: evidenceDigest,
    replan_requested: value.replan_requested,
    replan_instruction: replanInstruction,
    reward_input: rewardInput as unknown as AutonomousEvaluatorRewardInput,
    evaluator_authority: value.evaluator_authority,
    retention: value.retention,
    secret_material: value.secret_material,
    evaluation_digest: evaluationDigest,
  };
}

/** Score only stage-report composition; task correctness remains the normal evaluator's job. */
export function evaluateAutonomousWorkflowStageResponse(
  value: unknown,
  options: { domain: AutonomousDomainName; workflowId: string; workflowDigest: string; stageId: string },
): AutonomousWorkflowStageResponseEvaluation {
  const domain = boundedIdentifier("workflow stage evaluation domain", options.domain);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("workflow stage evaluation domain is not supported");
  const workflowId = boundedIdentifier("workflow stage evaluation workflow_id", options.workflowId);
  const workflowDigest = boundedDigest("workflow stage evaluation workflow_digest", options.workflowDigest);
  const normalized = normalizeStageResponse(value, options.stageId);
  const responseDigest = digestJsonSync(normalized);
  // A completed stage may legitimately have no unresolved uncertainty or follow-up action.
  // The required notes field is the explicit bounded declaration that the stage has no such
  // disclosure. Non-completed stages must still report both fields themselves.
  const completedWithoutDisclosure = normalized.status === "completed" && normalized.notes.trim().length > 0;
  const signals: Record<string, number> = {
    schema_valid: 1,
    stage_identity: 1,
    status_declared: 1,
    evidence_present: normalized.evidence.length > 0 ? 1 : 0,
    uncertainty_reported: normalized.uncertainty.length > 0 || completedWithoutDisclosure ? 1 : 0,
    notes_present: normalized.notes.length > 0 ? 1 : 0,
    next_actions_present: normalized.next_actions.length > 0 || completedWithoutDisclosure ? 1 : 0,
    response_digest_bound: 1,
  };
  const totalWeight = Object.values(SIGNAL_WEIGHTS).reduce((sum, weight) => sum + weight, 0);
  const reward = Number((Object.entries(SIGNAL_WEIGHTS).reduce((sum, [signal, weight]) => sum + signals[signal]! * weight, 0) / totalWeight).toFixed(12));
  const missingSignals = Object.entries(signals).filter(([, score]) => score < 1).map(([signal]) => signal);
  // The aggregate reward is useful for learning, but it must never hide a missing integrity
  // signal at a continuation boundary. A stage is therefore passable only when every signal
  // is satisfied and the score clears the documented floor.
  const passed = reward >= AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_PASS_THRESHOLD && missingSignals.length === 0;
  const evaluatorId = `autonomous-${domain}-workflow-stage-integrity`;
  const feedbackDigest = digestJsonSync({ workflow_digest: workflowDigest, stage_id: normalized.stage_id, response_digest: responseDigest, signals });
  const failureClass = passed ? null : "workflow_stage_response_integrity_gate";
  const replanInstruction = passed ? null : `Improve bounded ${domain} workflow stage composition: ${missingSignals.join(", ") || "the stage integrity score"}.`;
  const rewardInput: AutonomousEvaluatorRewardInput = {
    evaluator_id: evaluatorId,
    evaluator_version: AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION,
    reward,
    passed,
    failed: !passed,
    feedback_digest: feedbackDigest,
    failure_class: failureClass,
    evidence_digest: responseDigest,
  };
  const descriptor = {
    schema: AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA,
    evaluator_id: evaluatorId,
    evaluator_version: AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION,
    domain: domain as AutonomousDomainName,
    workflow_id: workflowId,
    workflow_digest: workflowDigest,
    stage_id: normalized.stage_id,
    response_digest: responseDigest,
    signals,
    missing_signals: missingSignals,
    reward,
    passed,
    failed: !passed,
    failure_class: failureClass,
    feedback_digest: feedbackDigest,
    evidence_digest: responseDigest,
    replan_requested: !passed,
    replan_instruction: replanInstruction,
    reward_input: rewardInput,
    evaluator_authority: "workflow_stage_response_contract_only;not_external_truth" as const,
    retention: "value_only;stage_response_and_credentials_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, evaluation_digest: digestJsonSync(descriptor) };
}

/** Validate a persisted value-only evaluation projection. */
export function validateAutonomousWorkflowStageResponseEvaluation(value: unknown): AutonomousWorkflowStageResponseEvaluation {
  return normalizedEvaluation(value);
}

/** Recompute stage composition from caller-owned transient output and reject drift. */
export function replayAutonomousWorkflowStageResponseEvaluation(
  value: unknown,
  expected: AutonomousWorkflowStageResponseEvaluation,
): AutonomousWorkflowStageResponseEvaluation {
  const validated = normalizedEvaluation(expected);
  const replayed = evaluateAutonomousWorkflowStageResponse(value, {
    domain: validated.domain,
    workflowId: validated.workflow_id,
    workflowDigest: validated.workflow_digest,
    stageId: validated.stage_id,
  });
  if (replayed.evaluation_digest !== validated.evaluation_digest) throw new ArgumentError("workflow stage evaluator replay drifted from the recorded evaluation");
  return replayed;
}
