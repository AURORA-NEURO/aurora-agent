import { ArgumentError, isObject } from "./errors.js";
import { assertSafeTransientValue, bytes } from "./autonomous-validation.js";
import type {
  AutonomousEvidenceBackedRunStatus,
  AutonomousEvidenceExecutionMode,
  AutonomousPromptChunk,
  AutonomousRunStatus,
} from "./autonomous-agent-contracts.js";
import type { AutonomousEvidenceExecutionResult } from "./autonomous-evidence-execution.js";

export const AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-run/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_CHUNKS = 32;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_CONTEXT_BYTES = 48_000;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_RESULT_BYTES = 512_000;
export function defaultEvidenceBackedPromptContext(execution: AutonomousEvidenceExecutionResult): AutonomousPromptChunk[] {
  const runtime = execution.runtime.toJSON();
  const receipts = runtime.receipts.map((receipt) => ({
    requirement_id: receipt.requirement_id,
    domain: receipt.domain,
    workflow_id: receipt.workflow_id,
    workflow_digest: receipt.workflow_digest,
    stage_id: receipt.stage_id,
    source_id: receipt.source_id,
    source_digest: receipt.source_digest,
    status: receipt.status,
    value_digest: receipt.value_digest,
    observations: receipt.observations.map((observation) => ({
      label: observation.label,
      kind: observation.kind,
      status: observation.status,
      confidence: observation.confidence,
      limitations: observation.limitations,
    })),
    evaluator_status: receipt.evaluator_status,
    assessment_digest: receipt.assessment_digest,
    limitations: receipt.limitations,
  }));
  const content = JSON.stringify({
    schema: "bioprism-typescript-autonomous-evidence-backed-context/0.1",
    execution_plan_digest: execution.plan.plan_digest,
    evidence_result_digest: execution.result_digest,
    status: runtime.status,
    completed_requirement_ids: runtime.completed_requirement_ids,
    pending_evaluation_requirement_ids: runtime.pending_evaluation_requirement_ids,
    missing_requirement_ids: runtime.missing_requirement_ids,
    next_stage_ids: runtime.next_stage_ids,
    receipts,
    assessments: runtime.assessments.map((assessment) => ({
      requirement_id: assessment.requirement_id,
      evaluator_id: assessment.evaluator_id,
      evaluator_version: assessment.evaluator_version,
      verdict: assessment.verdict,
      score: assessment.score,
      feedback_digest: assessment.feedback_digest,
      evidence_digest: assessment.evidence_digest,
      failure_class: assessment.failure_class,
      assessment_digest: assessment.assessment_digest,
    })),
    retention: "metadata_only;raw_evidence_values_caller_owned",
    secret_material: "never_returned",
  });
  if (bytes(content) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CONTEXT_BYTES) throw new ArgumentError("default evidence-backed prompt context exceeds its bound");
  return [{ id: "reviewed-evidence-execution", content, required: true, priority: 960 }];
}

export function normalizeEvidenceBackedPromptContext(value: readonly AutonomousPromptChunk[], maximum = MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_CHUNKS): AutonomousPromptChunk[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError("evidence-backed prompt context is outside its bounds");
  const result = value.map((chunk, index) => {
    if (!isObject(chunk) || typeof chunk.id !== "string" || !chunk.id.trim() || typeof chunk.content !== "string" || bytes(chunk.content) > 64_000) throw new ArgumentError(`evidence-backed prompt context chunk ${index} is malformed`);
    if (chunk.required !== undefined && typeof chunk.required !== "boolean") throw new ArgumentError(`evidence-backed prompt context chunk ${index}.required is malformed`);
    if (chunk.priority !== undefined && (typeof chunk.priority !== "number" || !Number.isFinite(chunk.priority))) throw new ArgumentError(`evidence-backed prompt context chunk ${index}.priority is malformed`);
    assertSafeTransientValue(chunk);
    return structuredClone(chunk) as unknown as AutonomousPromptChunk;
  });
  if (new Set(result.map((chunk) => chunk.id)).size !== result.length) throw new ArgumentError("evidence-backed prompt context contains duplicate chunk IDs");
  const encoded = JSON.stringify(result);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CONTEXT_BYTES) throw new ArgumentError("evidence-backed prompt context exceeds its bound");
  return result;
}

export function evidenceBackedStatus(status: ReturnType<AutonomousEvidenceExecutionResult["toJSON"]>["status"]): Exclude<AutonomousEvidenceBackedRunStatus, AutonomousRunStatus> {
  if (status === "failed") return "evidence_failed";
  if (status === "reconciliation_required") return "evidence_incomplete";
  return "evidence_incomplete";
}

export function normalizeAutonomousEvidenceExecutionMode(value: unknown): AutonomousEvidenceExecutionMode {
  if (value === undefined) return "domain";
  if (value !== "domain" && value !== "cross_domain" && value !== "auto") throw new ArgumentError("evidence-backed runMode must be domain, cross_domain, or auto");
  return value;
}
