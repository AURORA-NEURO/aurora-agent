import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import type {
  AutonomousDomainToolExecutionReceipt,
  AutonomousToolSelectionOutcome,
  AutonomousToolSelectionState,
} from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Stable schema for the TypeScript receipt-to-evaluator learning boundary. */
export const AUTONOMOUS_TOOL_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-tool-evaluation/0.1" as const;
export const AUTONOMOUS_TOOL_LEARNING_SCHEMA = "bioprism-typescript-autonomous-tool-learning/0.1" as const;
export const MAX_AUTONOMOUS_TOOL_EVALUATION_EVIDENCE_BYTES = 256_000;
export const MAX_AUTONOMOUS_TOOL_EVALUATION_RECEIPTS = 128;

const TOOL_STATUSES = new Set<AutonomousDomainToolExecutionReceipt["status"]>([
  "approval_required",
  "executed",
  "reconciliation_required",
  "execution_failed",
]);
const SAFE_IDENTIFIER = /^[A-Za-z0-9_.-]+$/;
const SHA256 = /^[0-9a-f]{64}$/;
const FORBIDDEN_FIELDS = new Set([
  "apikey", "authorization", "bearer", "credential", "password", "secret",
  "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response",
  "rawpayload", "arguments", "output", "task", "messages",
]);

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || value.length === 0 || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!SAFE_IDENTIFIER.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, nullable = false): string | null {
  if ((value === null || value === undefined) && nullable) return null;
  if (typeof value !== "string" || !SHA256.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function assertSafeMetadata(value: unknown, depth = 0): void {
  if (depth > 32) throw new ArgumentError("tool evaluator evidence is too deeply nested");
  if (Array.isArray(value)) {
    for (const child of value) assertSafeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (FORBIDDEN_FIELDS.has(normalized)) throw new ArgumentError("tool evaluator evidence contains transient or secret-shaped fields");
      assertSafeMetadata(child, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError("tool evaluator evidence contains a non-finite number");
}

function safeObject(name: string, value: unknown): JsonObject {
  if (!isObject(value)) throw new ArgumentError(`${name} must be an object`);
  assertSafeMetadata(value);
  let encoded: string;
  try {
    encoded = canonicalJson(value);
  } catch (error) {
    throw new ArgumentError(`${name} must be canonical JSON`, { cause: error });
  }
  if (bytes(encoded) > MAX_AUTONOMOUS_TOOL_EVALUATION_EVIDENCE_BYTES) throw new ArgumentError(`${name} exceeds its bounded size`);
  return JSON.parse(encoded) as JsonObject;
}

function safeStringList(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > 512) throw new ArgumentError(`${name} must be a bounded string list`);
  return value.map((item, index) => boundedText(`${name}[${index}]`, item, 256));
}

function optionalIdentifier(name: string, value: unknown): string | null {
  return value === null || value === undefined ? null : boundedIdentifier(name, value);
}

function receiptIdentity(receipt: AutonomousDomainToolExecutionReceipt): string {
  return `${receipt.execution_id ?? "unjournaled"}:${boundedText("tool receipt call_id", receipt.call_id, 256)}`;
}

function receiptMetadata(receipt: AutonomousDomainToolExecutionReceipt): {
  execution_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  call_id: string;
  tool: string;
  status: AutonomousDomainToolExecutionReceipt["status"];
  workflow_id: string | null;
  workflow_digest: string | null;
  stage_id: string | null;
  stage_contract_digest: string | null;
  required_evidence_outputs: string[];
  schema_digest: string | null;
  arguments_digest: string | null;
  output_digest: string | null;
  duration_ms: number;
} {
  if (!isObject(receipt) || receipt.schema !== "bioprism-typescript-autonomous-domain-tool-registry/0.1" || receipt.receipt_kind !== "tool_execution_receipt") throw new ArgumentError("tool evaluation requires a tool execution receipt");
  if (!TOOL_STATUSES.has(receipt.status)) throw new ArgumentError("tool evaluation receipt status is invalid");
  const callId = boundedText("tool receipt call_id", receipt.call_id, 256);
  const executionId = optionalIdentifier("tool receipt execution_id", receipt.execution_id) ?? "unjournaled";
  const domain = receipt.domain === null || receipt.domain === undefined ? "cross_domain" : receipt.domain;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("tool evaluation receipt domain is unsupported");
  const duration = receipt.duration_ms;
  if (typeof duration !== "number" || !Number.isFinite(duration) || duration < 0 || duration > 3_600_000) throw new ArgumentError("tool evaluation receipt duration_ms is invalid");
  return {
    execution_id: executionId,
    domain: domain as AutonomousDomainName,
    capability: optionalIdentifier("tool receipt capability", receipt.capability) ?? "tool_execution",
    risk_class: boundedIdentifier("tool receipt risk_class", receipt.effect ?? "read_only"),
    call_id: callId,
    tool: boundedIdentifier("tool receipt tool", receipt.tool),
    status: receipt.status,
    workflow_id: optionalIdentifier("tool receipt workflow_id", receipt.workflow_id),
    workflow_digest: boundedDigest("tool receipt workflow_digest", receipt.workflow_digest, true),
    stage_id: optionalIdentifier("tool receipt stage_id", receipt.stage_id),
    stage_contract_digest: boundedDigest("tool receipt stage_contract_digest", receipt.stage_contract_digest, true),
    required_evidence_outputs: safeStringList("tool receipt required_evidence_outputs", receipt.required_evidence_outputs),
    schema_digest: boundedDigest("tool receipt schema_digest", receipt.schema_digest, true),
    arguments_digest: boundedDigest("tool receipt arguments_digest", receipt.arguments_digest, true),
    output_digest: boundedDigest("tool receipt result_digest", receipt.result_digest, true),
    duration_ms: duration,
  };
}

/** Safe evaluator input projected from a receipt; raw calls, arguments, results, and prompts are absent. */
export interface AutonomousToolOutcomeEvaluationInput extends JsonObject {
  schema: typeof AUTONOMOUS_TOOL_EVALUATION_SCHEMA;
  execution_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  call_id: string;
  tool: string;
  status: AutonomousDomainToolExecutionReceipt["status"];
  workflow_id: string | null;
  workflow_digest: string | null;
  stage_id: string | null;
  stage_contract_digest: string | null;
  required_evidence_outputs: string[];
  schema_digest: string | null;
  arguments_digest: string | null;
  output_digest: string | null;
  duration_ms: number;
  evidence_digest: string;
  evidence: JsonObject;
  retention: "digests_and_safe_evidence_only_no_arguments_or_outputs";
}

/** Build the evaluator-safe projection for one live receipt. */
export async function autonomousToolOutcomeEvaluationInput(
  receipt: AutonomousDomainToolExecutionReceipt,
  evidence: JsonObject = {},
): Promise<AutonomousToolOutcomeEvaluationInput> {
  const metadata = receiptMetadata(receipt);
  const safeEvidence = safeObject("tool evaluator evidence", evidence);
  const evidenceDigest = await digestJson(safeEvidence);
  return {
    schema: AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
    ...metadata,
    evidence_digest: evidenceDigest,
    evidence: safeEvidence,
    retention: "digests_and_safe_evidence_only_no_arguments_or_outputs",
  };
}

export interface AutonomousToolEvaluatorAssessment extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
}

export interface AutonomousToolEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_TOOL_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string;
  decision_digest: string;
  retention: "value_only";
}

export interface AutonomousToolOutcomeEvaluatorOptions {
  evaluator_id: string;
  evaluator_version: string;
  evaluate: (input: AutonomousToolOutcomeEvaluationInput) => AutonomousToolEvaluatorAssessment | Promise<AutonomousToolEvaluatorAssessment>;
}

interface NormalizedAutonomousToolEvaluatorAssessment {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string;
}

export interface AutonomousToolSelectionUpdater {
  (state: AutonomousToolSelectionState | null | undefined, outcome: AutonomousToolSelectionOutcome): AutonomousToolSelectionState;
}

export interface AutonomousToolLearningEvaluation extends JsonObject {
  execution_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  call_id: string;
  tool: string;
  status: AutonomousDomainToolExecutionReceipt["status"];
  evidence_digest: string;
  decision_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  failure_class: string | null;
  tool_selection_outcome_digest: string;
  idempotent_replay: boolean;
}

export interface AutonomousToolLearningReport extends JsonObject {
  schema: typeof AUTONOMOUS_TOOL_LEARNING_SCHEMA;
  status: "completed" | "no_receipts";
  receipts: number;
  evaluations: AutonomousToolLearningEvaluation[];
  by_domain: Record<string, number>;
  by_status: Record<string, number>;
  next_tool_selection_state: AutonomousToolSelectionState | null;
  learning_digest: string;
  retention: "metadata_and_digests_only";
  secret_material: "never_returned";
}

function normalizedAssessment(
  raw: AutonomousToolEvaluatorAssessment,
  evaluatorId: string,
  evaluatorVersion: string,
  evidenceDigest: string,
): NormalizedAutonomousToolEvaluatorAssessment {
  if (!isObject(raw)) throw new ArgumentError("tool evaluator callback must return an object");
  const allowed = new Set(["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("tool evaluator decision contains unsupported fields");
  if (raw.evaluator_id !== undefined && raw.evaluator_id !== evaluatorId) throw new ArgumentError("tool evaluator decision identity does not match the evaluator");
  if (raw.evaluator_version !== undefined && raw.evaluator_version !== evaluatorVersion) throw new ArgumentError("tool evaluator decision identity does not match the evaluator");
  if (typeof raw.reward !== "number" || !Number.isFinite(raw.reward) || raw.reward < -1 || raw.reward > 1) throw new ArgumentError("tool evaluator reward must be finite and within [-1, 1]");
  if (typeof raw.passed !== "boolean") throw new ArgumentError("tool evaluator passed must be boolean");
  const failed = raw.failed === undefined ? !raw.passed : raw.failed;
  if (typeof failed !== "boolean") throw new ArgumentError("tool evaluator failed must be boolean");
  if (raw.passed && failed) throw new ArgumentError("tool evaluator cannot be both passed and failed");
  const feedbackDigest = raw.feedback_digest === undefined ? null : boundedDigest("tool evaluator feedback_digest", raw.feedback_digest, true);
  const failureClass = raw.failure_class === undefined || raw.failure_class === null ? null : boundedIdentifier("tool evaluator failure_class", raw.failure_class);
  const returnedEvidenceDigest = raw.evidence_digest === undefined ? null : boundedDigest("tool evaluator evidence_digest", raw.evidence_digest, true);
  if (returnedEvidenceDigest !== null && returnedEvidenceDigest !== evidenceDigest) throw new ArgumentError("tool evaluator evidence_digest does not match the input");
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward: raw.reward,
    passed: raw.passed,
    failed,
    feedback_digest: feedbackDigest,
    failure_class: failureClass,
    evidence_digest: evidenceDigest,
  };
}

/**
 * Independent evaluator adapter for ordinary provider tool-loop receipts. It only accepts
 * bounded metadata projections and leaves reward authority with the caller-owned evaluator.
 */
export class AutonomousToolOutcomeEvaluator {
  readonly evaluatorId: string;
  readonly evaluatorVersion: string;
  private readonly callback: AutonomousToolOutcomeEvaluatorOptions["evaluate"];

  constructor(options: AutonomousToolOutcomeEvaluatorOptions) {
    if (!options || typeof options !== "object" || typeof options.evaluate !== "function") throw new ArgumentError("tool evaluator requires an evaluate callback");
    this.evaluatorId = boundedIdentifier("tool evaluator_id", options.evaluator_id);
    this.evaluatorVersion = boundedIdentifier("tool evaluator_version", options.evaluator_version);
    this.callback = options.evaluate;
  }

  async assess(input: AutonomousToolOutcomeEvaluationInput): Promise<AutonomousToolEvaluation> {
    if (!isObject(input) || input.schema !== AUTONOMOUS_TOOL_EVALUATION_SCHEMA) throw new ArgumentError("tool evaluator input schema is invalid");
    const safeInput = await autonomousToolOutcomeEvaluationInput({
      receipt_kind: "tool_execution_receipt",
      schema: "bioprism-typescript-autonomous-domain-tool-registry/0.1",
      call_id: input.call_id,
      execution_id: input.execution_id,
      domain: input.domain,
      workflow_id: input.workflow_id,
      workflow_digest: input.workflow_digest,
      stage_id: input.stage_id,
      stage_contract_digest: input.stage_contract_digest,
      required_evidence_outputs: input.required_evidence_outputs,
      evidence_status: "tool_execution_only",
      does_not_claim: [],
      tool: input.tool,
      capability: input.capability,
      status: input.status,
      effect: input.risk_class,
      schema_digest: input.schema_digest ?? undefined,
      arguments_digest: input.arguments_digest ?? undefined,
      result_digest: input.output_digest ?? undefined,
      duration_ms: input.duration_ms,
      secret_material: "never_returned",
    }, input.evidence);
    if (safeInput.evidence_digest !== input.evidence_digest) throw new ArgumentError("tool evaluator input evidence_digest is invalid");
    let raw: AutonomousToolEvaluatorAssessment;
    try {
      raw = await this.callback(structuredClone(safeInput));
    } catch (error) {
      throw new ArgumentError("tool evaluator callback failed", { cause: error });
    }
    const decision = normalizedAssessment(raw, this.evaluatorId, this.evaluatorVersion, input.evidence_digest);
    const base = {
      schema: AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
      evaluator_id: decision.evaluator_id,
      evaluator_version: decision.evaluator_version,
      reward: decision.reward,
      passed: decision.passed,
      failed: decision.failed,
      feedback_digest: decision.feedback_digest ?? null,
      failure_class: decision.failure_class ?? null,
      evidence_digest: input.evidence_digest,
      retention: "value_only" as const,
    };
    return { ...base, decision_digest: await digestJson(base) };
  }

  async evaluateReceipts(
    receipts: readonly AutonomousDomainToolExecutionReceipt[],
    options: {
      evidence?: Readonly<Record<string, JsonObject>>;
      toolSelectionState?: AutonomousToolSelectionState | null;
      toolSelectionUpdater?: AutonomousToolSelectionUpdater;
    } = {},
  ): Promise<AutonomousToolLearningReport> {
    if (!Array.isArray(receipts) || receipts.length > MAX_AUTONOMOUS_TOOL_EVALUATION_RECEIPTS) throw new ArgumentError(`tool receipt batches must contain at most ${MAX_AUTONOMOUS_TOOL_EVALUATION_RECEIPTS} entries`);
    const metadata = receipts.map((receipt) => ({ receipt, value: receiptMetadata(receipt) }));
    const identities = metadata.map(({ receipt }) => receiptIdentity(receipt));
    if (new Set(identities).size !== identities.length) throw new ArgumentError("tool receipt batches cannot contain duplicate execution_id/call_id identities");
    if (options.evidence !== undefined && (!isObject(options.evidence) || Object.values(options.evidence).some((packet) => !isObject(packet)))) throw new ArgumentError("tool receipt evidence must map identities to objects");
    const evidence = options.evidence ?? {};
    const uniqueCallIds = new Set(metadata.filter(({ receipt }) => metadata.filter((candidate) => candidate.receipt.call_id === receipt.call_id).length === 1).map(({ receipt }) => receipt.call_id));
    const validEvidenceKeys = new Set([...identities, ...uniqueCallIds]);
    if (Object.keys(evidence).some((key) => !validEvidenceKeys.has(key))) throw new ArgumentError("tool receipt evidence contains an unknown receipt identity");
    const hasSelectionState = options.toolSelectionState !== undefined || options.toolSelectionUpdater !== undefined;
    if (hasSelectionState && !options.toolSelectionUpdater) throw new ArgumentError("tool receipt tool-selection state requires a toolSelectionUpdater");
    let selectionState = options.toolSelectionState;
    const evaluations: AutonomousToolLearningEvaluation[] = [];
    const byDomain: Record<string, number> = {};
    const byStatus: Record<string, number> = {};
    for (const { receipt, value } of metadata) {
      const receiptEvidence = evidence[receiptIdentity(receipt)] ?? (uniqueCallIds.has(receipt.call_id) ? evidence[receipt.call_id] : undefined) ?? {};
      const input = await autonomousToolOutcomeEvaluationInput(receipt, receiptEvidence);
      const decision = await this.assess(input);
      const { evidence: _evidence, ...inputMetadata } = input;
      const toolSelectionOutcomeDigest = await digestJson({
        schema: "bioprism-autonomous-tool-selection-outcome/0.1",
        receipt_identity: receiptIdentity(receipt),
        input_digest: await digestJson(inputMetadata),
        decision_digest: decision.decision_digest,
      });
      const priorCredit = isObject(selectionState) && Array.isArray(selectionState.credited_outcomes)
        ? selectionState.credited_outcomes.some((credit) => isObject(credit) && credit.outcome_digest === toolSelectionOutcomeDigest)
        : false;
      if (options.toolSelectionUpdater) {
        selectionState = options.toolSelectionUpdater(selectionState, {
          domain: value.domain,
          capability: value.capability,
          tool: value.tool,
          reward: decision.reward,
          failed: decision.failed,
          latencyMs: value.duration_ms,
          outcomeDigest: toolSelectionOutcomeDigest,
        });
      }
      evaluations.push({
        execution_id: value.execution_id,
        domain: value.domain,
        capability: value.capability,
        risk_class: value.risk_class,
        call_id: value.call_id,
        tool: value.tool,
        status: value.status,
        evidence_digest: input.evidence_digest,
        decision_digest: decision.decision_digest,
        evaluator_id: decision.evaluator_id,
        evaluator_version: decision.evaluator_version,
        reward: decision.reward,
        passed: decision.passed,
        failed: decision.failed,
        failure_class: decision.failure_class,
        tool_selection_outcome_digest: toolSelectionOutcomeDigest,
        idempotent_replay: priorCredit,
      });
      byDomain[value.domain] = (byDomain[value.domain] ?? 0) + 1;
      byStatus[value.status] = (byStatus[value.status] ?? 0) + 1;
    }
    // Replay observability is intentionally excluded from the learning identity. Re-running
    // the same evaluator decision should produce the same digest even though the report marks
    // the second pass as idempotent_replay.
    const digestEvaluations = evaluations.map(({ idempotent_replay: _replay, ...evaluation }) => evaluation);
    const learningDigest = await digestJson(digestEvaluations);
    return {
      schema: AUTONOMOUS_TOOL_LEARNING_SCHEMA,
      status: evaluations.length ? "completed" : "no_receipts",
      receipts: evaluations.length,
      evaluations,
      by_domain: Object.fromEntries(Object.entries(byDomain).sort(([left], [right]) => left.localeCompare(right))),
      by_status: Object.fromEntries(Object.entries(byStatus).sort(([left], [right]) => left.localeCompare(right))),
      next_tool_selection_state: selectionState === undefined ? null : selectionState,
      learning_digest: learningDigest,
      retention: "metadata_and_digests_only",
      secret_material: "never_returned",
    };
  }
}
