import { ArgumentError, isObject } from "./errors.js";
import type {
  AutonomousDomainName,
  AutonomousDomainProfile,
} from "./autonomous.js";
import { digestJson, digestJsonSync } from "./tooling.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { JsonObject } from "./types.js";
import {
  autonomousDomainQualityPolicy,
  autonomousDomainQualityPrompt,
  evaluateAutonomousDomainResponseQuality,
} from "./autonomous-domain-quality.js";

/** Stable contract for opt-in structured answers emitted by every built-in domain workflow. */
export const AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA = "bioprism-typescript-autonomous-domain-response/0.1" as const;
export const AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA = "bioprism-typescript-autonomous-domain-response-contract/0.1" as const;
export const AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-domain-response-evaluation/0.1" as const;
export const AUTONOMOUS_DOMAIN_RESPONSE_STATUSES = ["complete", "partial", "blocked", "needs_review"] as const;
export const AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES = ["complete", "partial", "blocked", "not_attempted"] as const;
export const MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS = 64;
export const MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES = 8_192;
export const MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES = 64_000;
export const MAX_AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_BYTES = 1_000_000;
export const AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION = "2" as const;
export const AUTONOMOUS_DOMAIN_RESPONSE_PASS_THRESHOLD = 0.8;

export type AutonomousDomainResponseStatus = typeof AUTONOMOUS_DOMAIN_RESPONSE_STATUSES[number];
export type AutonomousDomainStageResponseStatus = typeof AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES[number];

/** The domain-specific fields that make an answer operationally useful instead of generic prose. */
export const AUTONOMOUS_DOMAIN_RESPONSE_FIELDS: Readonly<Record<AutonomousDomainName, readonly string[]>> = {
  coding: ["files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"],
  browser: ["sources", "citations", "freshness", "retrieval_gaps"],
  data: ["schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"],
  science: ["estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"],
  biomedical: ["scope_boundary", "provenance", "population_and_applicability", "neurosurgical_route", "molecular_assay_coverage", "uncertainty", "human_review_and_escalation"],
  neuroscience: ["measurement_contract", "preprocessing_and_exclusions", "neurosurgical_route", "molecular_assay_coverage", "confounds", "model_sensitivity", "validation_plan"],
  operations: ["observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"],
  enterprise: ["stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"],
  multi_agent: ["subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"],
  multimodal: ["available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"],
  cross_domain: ["domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"],
  evaluation: ["rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"],
};

export interface AutonomousDomainStageResponse extends JsonObject {
  stage_id: string;
  status: AutonomousDomainStageResponseStatus;
  evidence: string[];
  findings: string[];
  uncertainty: string[];
  open_questions: string[];
}

export interface AutonomousDomainResponse extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA;
  domain: AutonomousDomainName;
  workflow_id: string;
  status: AutonomousDomainResponseStatus;
  answer: string;
  observations: string[];
  inferences: string[];
  uncertainty: string[];
  evidence_gaps: string[];
  next_actions: string[];
  stages: AutonomousDomainStageResponse[];
  domain_details: JsonObject;
  retention: "transient_provider_response_only;validated_against_reviewed_domain_contract";
  secret_material: "never_returned";
}

export interface AutonomousDomainResponseContract extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA;
  version: "1";
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_ids: string[];
  domain_fields: string[];
  response_schema: JsonObject;
  prompt_contract: string;
  contract_digest: string;
  retention: "contract_metadata_only;provider_response_remains_transient";
  secret_material: "never_returned";
}

/**
 * Deterministic value-only feedback for response composition. It measures contract adherence,
 * reporting coverage, and uncertainty disclosure; it is deliberately not a task-quality or
 * external-world truth evaluator.
 */
export interface AutonomousDomainResponseEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: typeof AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  contract_digest: string;
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
  evaluator_authority: "structural_response_contract_only;not_external_truth";
  retention: "value_only;response_and_credentials_not_retained";
  secret_material: "never_returned";
  evaluation_digest: string;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedList(name: string, value: unknown, maximum = MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must contain at most ${maximum} entries`);
  return value.map((item) => boundedText(`${name} entry`, item, MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES));
}

function stageIds(profile: AutonomousDomainProfile): string[] {
  const stages = profile.workflow.stages;
  if (!Array.isArray(stages) || stages.length === 0 || stages.length > MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS) {
    throw new ArgumentError(`domain response workflow ${profile.workflow.workflow_id} has an invalid stage count`);
  }
  const ids = stages.map((stage) => boundedIdentifier("domain response stage id", stage.id));
  if (new Set(ids).size !== ids.length) throw new ArgumentError("domain response workflow stages contain duplicate ids");
  return ids;
}

function stringArraySchema(): JsonObject {
  return {
    type: "array",
    items: { type: "string", maxLength: MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES },
    maxItems: MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS,
  };
}

function stageSchema(ids: readonly string[]): JsonObject {
  return {
    type: "object",
    additionalProperties: false,
    properties: {
      stage_id: { type: "string", enum: [...ids] },
      status: { type: "string", enum: [...AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES] },
      evidence: stringArraySchema(),
      findings: stringArraySchema(),
      uncertainty: stringArraySchema(),
      open_questions: stringArraySchema(),
    },
    required: ["stage_id", "status", "evidence", "findings", "uncertainty", "open_questions"],
  };
}

function responseSchema(profile: AutonomousDomainProfile, ids: readonly string[], fields: readonly string[]): JsonObject {
  const detailProperties = Object.fromEntries(fields.map((field) => [field, stringArraySchema()]));
  return {
    type: "object",
    additionalProperties: false,
    properties: {
      schema: { type: "string", const: AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA },
      domain: { type: "string", const: profile.domain },
      workflow_id: { type: "string", const: profile.workflow.workflow_id },
      status: { type: "string", enum: [...AUTONOMOUS_DOMAIN_RESPONSE_STATUSES] },
      answer: { type: "string", minLength: 1, maxLength: MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES },
      observations: stringArraySchema(),
      inferences: stringArraySchema(),
      uncertainty: stringArraySchema(),
      evidence_gaps: stringArraySchema(),
      next_actions: stringArraySchema(),
      stages: { type: "array", minItems: ids.length, maxItems: ids.length, items: stageSchema(ids) },
      domain_details: {
        type: "object",
        additionalProperties: false,
        properties: detailProperties,
        required: [...fields],
      },
      retention: { type: "string", const: "transient_provider_response_only;validated_against_reviewed_domain_contract" },
      secret_material: { type: "string", const: "never_returned" },
    },
    required: ["schema", "domain", "workflow_id", "status", "answer", "observations", "inferences", "uncertainty", "evidence_gaps", "next_actions", "stages", "domain_details", "retention", "secret_material"],
  };
}

function promptContract(profile: AutonomousDomainProfile, ids: readonly string[], fields: readonly string[]): string {
  const quality = autonomousDomainQualityPolicy(profile.domain);
  return [
    `Return only one JSON object matching the ${AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA} contract for domain ${profile.domain}.`,
    `Use workflow ${profile.workflow.workflow_id} and include exactly one stage row for every stage, in reviewed order: ${ids.join(", ")}.`,
    `Each stage must report status, evidence, findings, uncertainty, and open_questions. Populate every domain_details field: ${fields.join(", ")}.`,
    "Separate observations from inferences, mark missing evidence and uncertainty explicitly, and put proposed work in next_actions.",
    "Never claim that a provider response, tool dispatch, simulation, or plan proves an external-world effect.",
    autonomousDomainQualityPrompt(quality),
  ].join(" ");
}

/** Build a digest-bound structured-answer contract from one reviewed domain workflow. */
export async function buildAutonomousDomainResponseContract(
  profile: AutonomousDomainProfile,
): Promise<AutonomousDomainResponseContract> {
  if (!profile || typeof profile !== "object" || !profile.workflow || !profile.domain) throw new ArgumentError("domain response contract requires a reviewed domain profile");
  const ids = stageIds(profile);
  const fields = AUTONOMOUS_DOMAIN_RESPONSE_FIELDS[profile.domain];
  if (!fields || fields.length === 0) throw new ArgumentError(`domain response contract has no field set for ${profile.domain}`);
  const response = responseSchema(profile, ids, fields);
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA,
    version: "1" as const,
    domain: profile.domain,
    workflow_id: profile.workflow.workflow_id,
    workflow_digest: profile.workflow.workflow_digest,
    stage_ids: [...ids],
    domain_fields: [...fields],
    response_schema: response,
    prompt_contract: promptContract(profile, ids, fields),
    retention: "contract_metadata_only;provider_response_remains_transient" as const,
    secret_material: "never_returned" as const,
  };
  const contract = { ...descriptor, contract_digest: await digestJson(descriptor) };
  if (bytes(JSON.stringify(contract)) > MAX_AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_BYTES) throw new ArgumentError("domain response contract exceeds its byte bound");
  return structuredClone(contract);
}

function secretKey(key: string): boolean {
  const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
  return ["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey"].includes(normalized)
    || normalized.startsWith("gsk")
    || normalized.startsWith("skproj");
}

function assertSafeResponseValue(value: unknown, depth = 0): void {
  if (depth > 16) throw new ArgumentError("domain response is too deeply nested");
  if (typeof value === "string") {
    if (/\b(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})/i.test(value)) throw new ArgumentError("domain response contains credential-shaped material");
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) assertSafeResponseValue(item, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      if (secretKey(key)) throw new ArgumentError("domain response contains credential-shaped fields");
      assertSafeResponseValue(child, depth + 1);
    }
  }
}

function exactKeys(name: string, value: Record<string, unknown>, allowed: readonly string[]): void {
  const allowedSet = new Set(allowed);
  if (Object.keys(value).some((key) => !allowedSet.has(key))) throw new ArgumentError(`${name} contains unsupported fields`);
}

/** Validate the semantic stage/domain invariants left intentionally beyond JSON Schema. */
export function validateAutonomousDomainResponse(
  value: unknown,
  contract: AutonomousDomainResponseContract,
): AutonomousDomainResponse {
  if (!contract || typeof contract !== "object") throw new ArgumentError("domain response validation requires a contract");
  if (!isObject(value)) throw new ArgumentError("domain response must be a JSON object");
  const ids = stageIdsFromContract(contract);
  const fields = fieldsFromContract(contract);
  exactKeys("domain response", value, ["schema", "domain", "workflow_id", "status", "answer", "observations", "inferences", "uncertainty", "evidence_gaps", "next_actions", "stages", "domain_details", "retention", "secret_material"]);
  if (value.schema !== AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA || value.domain !== contract.domain || value.workflow_id !== contract.workflow_id || value.retention !== "transient_provider_response_only;validated_against_reviewed_domain_contract" || value.secret_material !== "never_returned") throw new ArgumentError("domain response identity or retention markers are invalid");
  if (!AUTONOMOUS_DOMAIN_RESPONSE_STATUSES.includes(value.status as AutonomousDomainResponseStatus)) throw new ArgumentError("domain response status is invalid");
  const answer = boundedText("domain response answer", value.answer, MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES);
  const observations = boundedList("domain response observations", value.observations);
  const inferences = boundedList("domain response inferences", value.inferences);
  const uncertainty = boundedList("domain response uncertainty", value.uncertainty);
  const evidenceGaps = boundedList("domain response evidence_gaps", value.evidence_gaps);
  const nextActions = boundedList("domain response next_actions", value.next_actions);
  if (!Array.isArray(value.stages) || value.stages.length !== ids.length) throw new ArgumentError("domain response must contain exactly one row per reviewed stage");
  const stages: AutonomousDomainStageResponse[] = value.stages.map((rawStage, index) => {
    if (!isObject(rawStage)) throw new ArgumentError("domain response stage row is malformed");
    exactKeys("domain response stage", rawStage, ["stage_id", "status", "evidence", "findings", "uncertainty", "open_questions"]);
    if (rawStage.stage_id !== ids[index]) throw new ArgumentError("domain response stages must follow reviewed workflow order");
    if (!AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES.includes(rawStage.status as AutonomousDomainStageResponseStatus)) throw new ArgumentError("domain response stage status is invalid");
    return {
      stage_id: ids[index]!,
      status: rawStage.status as AutonomousDomainStageResponseStatus,
      evidence: boundedList("domain response stage evidence", rawStage.evidence),
      findings: boundedList("domain response stage findings", rawStage.findings),
      uncertainty: boundedList("domain response stage uncertainty", rawStage.uncertainty),
      open_questions: boundedList("domain response stage open_questions", rawStage.open_questions),
    };
  });
  if (!isObject(value.domain_details)) throw new ArgumentError("domain response domain_details must be an object");
  exactKeys("domain response domain_details", value.domain_details, fields);
  const domainDetails: JsonObject = {};
  for (const field of fields) domainDetails[field] = boundedList(`domain response domain_details.${field}`, value.domain_details[field]);
  const normalized: AutonomousDomainResponse = {
    schema: AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: value.status as AutonomousDomainResponseStatus,
    answer,
    observations,
    inferences,
    uncertainty,
    evidence_gaps: evidenceGaps,
    next_actions: nextActions,
    stages,
    domain_details: domainDetails,
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
  assertSafeResponseValue(normalized);
  return structuredClone(normalized);
}

function stageIdsFromContract(contract: AutonomousDomainResponseContract): string[] {
  if (!Array.isArray(contract.stage_ids) || contract.stage_ids.length === 0 || contract.stage_ids.some((id) => typeof id !== "string") || new Set(contract.stage_ids).size !== contract.stage_ids.length) throw new ArgumentError("domain response contract stage_ids are malformed");
  return contract.stage_ids.map((id) => boundedIdentifier("domain response contract stage id", id));
}

function fieldsFromContract(contract: AutonomousDomainResponseContract): string[] {
  if (!Array.isArray(contract.domain_fields) || contract.domain_fields.length === 0 || contract.domain_fields.some((field) => typeof field !== "string") || new Set(contract.domain_fields).size !== contract.domain_fields.length) throw new ArgumentError("domain response contract domain_fields are malformed");
  return contract.domain_fields.map((field) => {
    const normalized = boundedText("domain response contract field", field, 256);
    if (!/^[A-Za-z0-9_.:_-]+$/.test(normalized)) throw new ArgumentError("domain response contract field is malformed");
    return normalized;
  });
}

/** Validate a provider response only when the caller explicitly selected the domain contract. */
export function validateAutonomousProviderDomainResponse(
  response: { structured: unknown } | null,
  contract: AutonomousDomainResponseContract | null | undefined,
): AutonomousDomainResponse | null {
  if (!contract) return null;
  if (!response || response.structured === null || response.structured === undefined) throw new ArgumentError("structured domain response is missing");
  return validateAutonomousDomainResponse(response.structured, contract);
}

function fraction(total: number, satisfied: number): number {
  if (total <= 0) return 0;
  return Number(Math.max(0, Math.min(1, satisfied / total)).toFixed(12));
}

function hasEntries(values: readonly string[]): number {
  return values.length > 0 ? 1 : 0;
}

/**
 * Convert one validated domain response into a deterministic structural reward. The response body
 * is reduced to a digest immediately; only signal scores and digests are returned or suitable for
 * persistence. This signal is intentionally safe to use for format/composition adaptation only.
 */
export function evaluateAutonomousDomainResponse(
  value: unknown,
  contract: AutonomousDomainResponseContract,
): AutonomousDomainResponseEvaluation {
  const response = validateAutonomousDomainResponse(value, contract);
  const quality = evaluateAutonomousDomainResponseQuality(response, contract);
  const responseDigest = digestJsonSync(response);
  const stageReporting = response.stages.map((stage) => Number(
    [stage.evidence, stage.findings, stage.uncertainty, stage.open_questions].some((items) => items.length > 0),
  ));
  const detailReporting = contract.domain_fields.map((field) => hasEntries(response.domain_details[field] as string[]));
  const signals: Record<string, number> = {
    answer_present: hasEntries([response.answer]),
    stage_rows_complete: 1,
    stage_reporting_coverage: fraction(stageReporting.length, stageReporting.reduce((sum, score) => sum + score, 0)),
    domain_detail_coverage: fraction(detailReporting.length, detailReporting.reduce((sum, score) => sum + score, 0)),
    observations_present: hasEntries(response.observations),
    inferences_present: hasEntries(response.inferences),
    uncertainty_disclosed: hasEntries(response.uncertainty),
    evidence_gaps_disclosed: hasEntries(response.evidence_gaps),
    next_actions_present: hasEntries(response.next_actions),
    ...quality.signals,
  };
  const weights: Record<string, number> = {
    answer_present: 1,
    stage_rows_complete: 2,
    stage_reporting_coverage: 2,
    domain_detail_coverage: 2,
    observations_present: 1,
    inferences_present: 1,
    uncertainty_disclosed: 1.5,
    evidence_gaps_disclosed: 1,
    next_actions_present: 1,
    ...quality.weights,
  };
  const totalWeight = Object.values(weights).reduce((sum, weight) => sum + weight, 0);
  const reward = Number((Object.entries(weights).reduce((sum, [signal, weight]) => sum + (signals[signal] ?? 0) * weight, 0) / totalWeight).toFixed(12));
  const missingSignals = Object.entries(signals).filter(([, score]) => score < 1).map(([signal]) => signal);
  const passed = reward >= AUTONOMOUS_DOMAIN_RESPONSE_PASS_THRESHOLD && quality.passed;
  const evaluatorId = `autonomous-${contract.domain}-response-integrity`;
  const feedbackDigest = digestJsonSync({ contract_digest: contract.contract_digest, response_digest: responseDigest, signals });
  // Keep the stable outer failure class for callers while the quality-prefixed signals and
  // recommendations explain the domain-specific reason for the replan.
  const failureClass = passed ? null : "response_integrity_gate";
  const replanInstruction = passed
    ? null
    : `Improve bounded ${contract.domain} response composition: ${[...missingSignals, ...quality.recommendations].join("; ") || "the response integrity score"}.`;
  const rewardInput: AutonomousEvaluatorRewardInput = {
    evaluator_id: evaluatorId,
    evaluator_version: AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION,
    reward,
    passed,
    failed: !passed,
    feedback_digest: feedbackDigest,
    failure_class: failureClass,
    evidence_digest: responseDigest,
  };
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA,
    evaluator_id: evaluatorId,
    evaluator_version: AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION,
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    workflow_digest: contract.workflow_digest,
    contract_digest: contract.contract_digest,
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
    evaluator_authority: "structural_response_contract_only;not_external_truth" as const,
    retention: "value_only;response_and_credentials_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, evaluation_digest: digestJsonSync(descriptor) };
}

/** Re-run the deterministic structural evaluator and refuse replay drift. */
export function replayAutonomousDomainResponseEvaluation(
  value: unknown,
  contract: AutonomousDomainResponseContract,
  expected: AutonomousDomainResponseEvaluation,
): AutonomousDomainResponseEvaluation {
  if (!expected || typeof expected !== "object" || typeof expected.evaluation_digest !== "string") throw new ArgumentError("domain response replay requires an evaluation digest");
  const replayed = evaluateAutonomousDomainResponse(value, contract);
  if (replayed.evaluation_digest !== expected.evaluation_digest) throw new ArgumentError("domain response evaluator replay drifted from the recorded evaluation");
  return replayed;
}
