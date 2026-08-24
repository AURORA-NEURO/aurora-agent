/** Shared v1 research contracts for TypeScript adapters and workbench clients.
 *
 * This module intentionally validates transport and safety metadata only. Scientific conclusions
 * remain evidence-receipt values produced by the Rust kernel; a client cannot upgrade `unknown`
 * or bypass a protected omission by editing a JSON object.
 */
import { digestJsonSync } from "./tooling.js";

export const RESEARCH_CONTRACT_SCHEMA_VERSION = "aurora-research-contract/1.0" as const;
export const PRECLINICAL_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions" as const;
export const RESEARCH_FEATURE_ID = "AFA-bioir-P02-F01" as const;
export const RELEASE_REVIEW_FEATURE_ID = "AFA-evalengine-P13-F01" as const;
export const RESEARCH_INGESTION_FEATURE_ID = "AFA-adapter-P06-F01" as const;
export const EXPERIMENT_DESIGN_FEATURE_ID = "AFA-lab-P09-F01" as const;
export const PROTOCOL_SIMULATION_FEATURE_ID = "AFA-lab-P10-F01" as const;
export const REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F01" as const;
export const QUALITY_CONTROL_FEATURE_ID = "AFA-adapter-P07-F01" as const;
export const RESEARCH_CONTEXT_FEATURE_ID = "AFA-fiber-P03-F01" as const;
export const REPLAY_AUDIT_FEATURE_ID = "AFA-runtime-P23-F01" as const;
export const WORKFLOW_EXECUTION_FEATURE_ID = "AFA-runtime-P12-F10" as const;

export type PolicyDecision = "allow" | "deny" | "redact" | "local_only" | "approval_required" | "unresolved";
export type EvidenceState = "proven" | "supported" | "speculative" | "contradicted" | "unknown";

export interface PolicyReceipt {
  schema_version: string;
  receipt_id: string;
  decision: PolicyDecision;
  reasons: string[];
  evaluated_artifacts: string[];
  authority_reference?: string | null;
  boundary: string;
}

export interface EvidenceOmission {
  item: string;
  reason: string;
  could_change_decision: "no_known_impact" | "potentially_material" | "unknown";
}

export interface EvidenceReceipt {
  schema_version: string;
  receipt_id: string;
  intent: string;
  sources: readonly { source_id: string; source_type: string; locator: string; digest?: string | null; availability: string }[];
  derivation: string[];
  uncertainty: readonly { kind: string; statement: string }[];
  omissions: readonly EvidenceOmission[];
  competing_explanations: readonly unknown[];
  negative_evidence: readonly unknown[];
  conclusion_state: EvidenceState;
  boundary: string;
}

export interface ReleaseReview {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  card_digest: string;
  verdict: "pass" | "conditional" | "blocked" | "not_evaluated";
  reasons: string[];
  replications: readonly Record<string, unknown>[];
  checks: readonly Record<string, unknown>[];
  provenance_complete: boolean;
  boundary: string;
}

export function validateReleaseReview(review: ReleaseReview): void {
  if (review.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (review.feature_id !== RELEASE_REVIEW_FEATURE_ID || !review.capability_id.trim()) throw new Error("release review feature or capability is missing");
  if (review.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!/^[0-9a-f]{64}$/.test(review.card_digest)) throw new Error("release review card digest is not a canonical sha256");
  if (!review.reasons.length) throw new Error("release review requires reasons");
  if (review.verdict === "pass" && !review.provenance_complete) throw new Error("a passing release review requires complete provenance");
}

export function releaseReviewDigest(review: ReleaseReview): string {
  validateReleaseReview(review);
  return digestJsonSync(review);
}

export interface ResearchIngestionBundle {
  schema_version: string;
  feature_id: string;
  source_id: string;
  adapter: string;
  adapter_version: string;
  source_digest: string;
  ingestion_digest: string;
  artifact: Record<string, unknown>;
  conformance: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateResearchIngestionBundle(bundle: ResearchIngestionBundle): void {
  if (bundle.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (bundle.feature_id !== RESEARCH_INGESTION_FEATURE_ID || !bundle.source_id.trim()) throw new Error("research ingestion feature or source is missing");
  if (bundle.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  for (const digest of [bundle.source_digest, bundle.ingestion_digest, bundle.artifact.content_hash]) {
    if (typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)) throw new Error("research ingestion digest is not a canonical sha256");
  }
  if (!bundle.raw_data_local) throw new Error("raw research data must remain local");
  if (bundle.conformance.verified !== true) throw new Error("research ingestion is not conformance verified");
}

export function researchIngestionBundleDigest(bundle: ResearchIngestionBundle): string {
  validateResearchIngestionBundle(bundle);
  return digestJsonSync(bundle);
}

export interface ExperimentDesignPlan {
  payload: Record<string, unknown> & { allocations: readonly { arm_id: string; units: number }[]; total_units: number };
  artifact: Record<string, unknown>;
}

export function validateExperimentDesignPlan(plan: ExperimentDesignPlan): void {
  if (plan.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (plan.payload.feature_id !== EXPERIMENT_DESIGN_FEATURE_ID) throw new Error("experiment design feature mismatch");
  if (plan.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!plan.payload.allocations.length || plan.payload.allocations.reduce((sum, allocation) => sum + allocation.units, 0) !== plan.payload.total_units) throw new Error("experiment design allocation total is inconsistent");
  if (typeof plan.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(plan.artifact.content_hash)) throw new Error("experiment design artifact digest is invalid");
}

export function experimentDesignPlanDigest(plan: ExperimentDesignPlan): string {
  validateExperimentDesignPlan(plan);
  return digestJsonSync(plan);
}

export interface ProtocolSimulationReport {
  payload: Record<string, unknown> & { results: readonly { status: "passed" | "failed_closed" | "requires_approval" }[] };
  artifact: Record<string, unknown>;
}

export function validateProtocolSimulationReport(report: ProtocolSimulationReport): void {
  if (report.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.payload.feature_id !== PROTOCOL_SIMULATION_FEATURE_ID) throw new Error("protocol simulation feature mismatch");
  if (report.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!report.payload.results.length) throw new Error("protocol simulation results are incomplete");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("protocol simulation artifact digest is invalid");
}

export function protocolSimulationReportDigest(report: ProtocolSimulationReport): string {
  validateProtocolSimulationReport(report);
  return digestJsonSync(report);
}

export interface ReplicationReport {
  payload: Record<string, unknown> & {
    summary: {
      disposition: "replicated" | "partially_replicated" | "contradicted" | "null_result" | "insufficient_evidence";
      total_observations: number;
      reasons: readonly string[];
    };
  };
  artifact: Record<string, unknown>;
}

export function validateReplicationReport(report: ReplicationReport): void {
  if (report.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.payload.feature_id !== REPLICATION_FEATURE_ID) throw new Error("replication feature mismatch");
  if (report.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (report.payload.summary.total_observations <= 0 || report.payload.summary.reasons.length === 0) throw new Error("replication summary is incomplete");
  if (!["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence"].includes(report.payload.summary.disposition)) throw new Error("replication disposition is unknown");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("replication artifact digest is invalid");
}

export function replicationReportDigest(report: ReplicationReport): string {
  validateReplicationReport(report);
  return digestJsonSync(report);
}

export interface QualityControlReceipt {
  payload: Record<string, unknown> & {
    summary: {
      disposition: "pass" | "pass_with_warnings" | "blocked" | "unknown";
      reasons: readonly string[];
    };
    raw_data_local: boolean;
  };
  artifact: Record<string, unknown>;
}

export function validateQualityControlReceipt(receipt: QualityControlReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== QUALITY_CONTROL_FEATURE_ID) throw new Error("quality-control feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.payload.summary.reasons.length) throw new Error("quality-control summary is incomplete");
  if (!["pass", "pass_with_warnings", "blocked", "unknown"].includes(receipt.payload.summary.disposition)) throw new Error("quality-control disposition is unknown");
  if (!receipt.payload.raw_data_local) throw new Error("raw research data must remain local");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality-control artifact digest is invalid");
}

export function qualityControlReceiptDigest(receipt: QualityControlReceipt): string {
  validateQualityControlReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchContextReceipt {
  payload: Record<string, unknown> & {
    protected_closure_satisfied: boolean;
    supports_sufficiency_claim: boolean;
    unresolved_obligations: number;
    section_digest: string;
    certificate_digest: string;
  };
  artifact: Record<string, unknown>;
}

export function validateResearchContextReceipt(receipt: ResearchContextReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== RESEARCH_CONTEXT_FEATURE_ID) throw new Error("research-context feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.payload.protected_closure_satisfied) throw new Error("protected closure is not satisfied");
  if (!Number.isInteger(receipt.payload.unresolved_obligations) || receipt.payload.unresolved_obligations < 0) throw new Error("unresolved-obligation count is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.payload.section_digest) || !/^[0-9a-f]{64}$/.test(receipt.payload.certificate_digest)) throw new Error("research-context source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("research-context artifact digest is invalid");
}

export function researchContextReceiptDigest(receipt: ResearchContextReceipt): string {
  validateResearchContextReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ReplayAuditReceipt {
  payload: Record<string, unknown> & {
    status: "equivalent" | "diverged" | "invalid";
    baseline_digest: string;
    candidate_digest: string;
    reasons: readonly string[];
  };
  artifact: Record<string, unknown>;
}

export function validateReplayAuditReceipt(receipt: ReplayAuditReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== REPLAY_AUDIT_FEATURE_ID) throw new Error("replay-audit feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!["equivalent", "diverged", "invalid"].includes(receipt.payload.status)) throw new Error("replay-audit status is unknown");
  if (!receipt.payload.reasons.length) throw new Error("replay-audit reasons are required");
  if (!/^[0-9a-f]{64}$/.test(receipt.payload.baseline_digest) || !/^[0-9a-f]{64}$/.test(receipt.payload.candidate_digest)) throw new Error("replay-audit source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("replay-audit artifact digest is invalid");
}

export function replayAuditReceiptDigest(receipt: ReplayAuditReceipt): string {
  validateReplayAuditReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface WorkflowExecutionReceipt {
  schema_version: string;
  feature_id: string;
  workflow_id: string;
  mode: "dry_run" | "execute";
  status: "dry_run" | "succeeded";
  ordered_nodes: readonly string[];
  completed_nodes: readonly string[];
  run: Record<string, unknown>;
  run_digest: string;
  remaining_budget: Record<string, number>;
  artifact: Record<string, unknown>;
  reasons: readonly string[];
  boundary: string;
}

export function validateWorkflowExecutionReceipt(receipt: WorkflowExecutionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== WORKFLOW_EXECUTION_FEATURE_ID || !receipt.workflow_id.trim()) throw new Error("workflow-execution feature or workflow is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.ordered_nodes.length || receipt.completed_nodes.some((node) => !receipt.ordered_nodes.includes(node))) throw new Error("workflow execution order is incomplete");
  if (!receipt.reasons.length) throw new Error("workflow execution reasons are required");
  if (receipt.run.workflow_id !== receipt.workflow_id) throw new Error("workflow run identity does not match receipt");
  const expectedRunStatus = receipt.status === "dry_run" ? "planned" : "succeeded";
  if (receipt.run.status !== expectedRunStatus) throw new Error("workflow run status does not match receipt status");
  if (!/^[0-9a-f]{64}$/.test(receipt.run_digest)) throw new Error("workflow run digest is not a canonical sha256");
  if (Object.values(receipt.remaining_budget).some((amount) => !Number.isFinite(amount) || amount < 0)) throw new Error("workflow remaining budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("workflow execution artifact digest is invalid");
}

export function workflowExecutionReceiptDigest(receipt: WorkflowExecutionReceipt): string {
  validateWorkflowExecutionReceipt(receipt);
  return digestJsonSync(receipt);
}

export function validatePolicyReceipt(receipt: PolicyReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || receipt.reasons.length === 0) throw new Error("policy receipt needs an id and reason");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if ((receipt.decision === "approval_required" || receipt.decision === "unresolved") && receipt.authority_reference) throw new Error("authority is premature for unresolved policy");
  if (receipt.decision === "allow" && receipt.reasons.some((reason) => reason === "unresolved")) throw new Error("unresolved policy cannot allow");
}

export function validateEvidenceReceipt(receipt: EvidenceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || !receipt.intent.trim() || receipt.derivation.length === 0) throw new Error("evidence receipt is incomplete");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.sources.length === 0 && (receipt.conclusion_state !== "unknown" || receipt.omissions.length === 0 || receipt.uncertainty.length === 0)) throw new Error("empty evidence must be explicit unknown");
  if (receipt.conclusion_state === "proven" && receipt.omissions.some((omission) => omission.could_change_decision !== "no_known_impact")) throw new Error("protected omission blocks proven conclusion");
}

/** Hashes the same JSON payload that the Rust `TypedResearchArtifact` seals. */
export function researchArtifactDigest(payload: unknown): string {
  return digestJsonSync(payload);
}
