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
export const EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-evalengine-P23-F01" as const;
export const RESEARCH_RELEASE_FEATURE_ID = "AFA-services-P16-F02" as const;
export const INSTRUMENT_PREFLIGHT_FEATURE_ID = "AFA-lab-P11-F01" as const;
export const MULTIMODAL_HARMONIZATION_FEATURE_ID = "AFA-adapter-P06-F02" as const;
export const ANALYSIS_QUALIFICATION_FEATURE_ID = "AFA-evalengine-P13-F01" as const;
export const PROTOCOL_MATRIX_FEATURE_ID = "AFA-lab-P10-F02" as const;
export const MULTIMODAL_REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F02" as const;
export const QUALITY_DRIFT_FEATURE_ID = "AFA-adapter-P07-F02" as const;
export const DESIGN_FRONTIER_FEATURE_ID = "AFA-lab-P09-F02" as const;
export const AUTONOMY_BATCH_FEATURE_ID = "AFA-policy-P19-F02" as const;
export const WORKFLOW_BATCH_FEATURE_ID = "AFA-runtime-P12-F11" as const;
export const RESEARCH_RELEASE_BATCH_FEATURE_ID = "AFA-services-P16-F03" as const;
export const FEDERATED_EVALUATION_FEATURE_ID = "AFA-evalengine-P23-F02" as const;
export const RESOURCE_WORKBENCH_FEATURE_ID = "AFA-fiber-P05-F20" as const;
export const RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID = "AFA-mcp-P05-F08" as const;
export const RESOURCE_DISCOVERY_CONTRACT_VERSION = "aurora-mcp-resource-discovery/2.0" as const;
export const GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID = "AFA-governance-P16-F08" as const;
export const GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION = "signed-research-object/2.0" as const;
export const RELEASE_HARNESS_FEATURE_ID = "AFA-obligation-P16-F27" as const;
export const RELEASE_HARNESS_CONTRACT_VERSION = "release-assurance-harness/1.0" as const;
export const PROTOCOL_ASSURANCE_FEATURE_ID = "AFA-policy-P10-F27" as const;
export const PROTOCOL_ASSURANCE_CONTRACT_VERSION = "protocol-assurance-harness/1.0" as const;
export const FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID = "AFA-routing-P06-F28" as const;
export const FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION = "federated-multimodal-assurance/1.0" as const;
export const FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID = "AFA-store-P04-F24" as const;
export const FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION = "federated-knowledge-gateway/1.0" as const;
export const FEDERATED_LENS_ASSURANCE_FEATURE_ID = "AFA-lens-P04-F28" as const;
export const FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION = "federated-lens-assurance/1.0" as const;
export const SEMANTIC_PARITY_FEATURE_ID = "AFA-lab-P28-F12" as const;
export const SEMANTIC_PARITY_CONTRACT_VERSION = "lab-semantic-parity/1.0" as const;
export const FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-fiber-P02-F28" as const;
export const FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "federated-retrieval-assurance/1.0" as const;
export const FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID = "AFA-atlashub-P02-F12" as const;
export const FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION = "federated-continual-retrieval-copilot/1.0" as const;
export const CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-devplat-P03-F28" as const;
export const CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "federated-context-compilation-assurance/1.0" as const;
export const KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID = "AFA-ops-P04-F28" as const;
export const KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION = "federated-knowledge-representation-assurance/1.0" as const;
export const RESOURCE_CONTROL_PLANE_FEATURE_ID = "AFA-weave-P05-F32" as const;
export const RESOURCE_CONTROL_PLANE_CONTRACT_VERSION = "federated-resource-control-plane/1.0" as const;
export const WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID = "AFA-weavelang-P16-F27" as const;
export const WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION = "weavelang-release-assurance/1.0" as const;
export const MECHANISM_CONTROL_PLANE_FEATURE_ID = "AFA-adapter-P08-F31" as const;
export const MECHANISM_CONTROL_PLANE_CONTRACT_VERSION = "federated-mechanism-control-plane/1.0" as const;
export const MECHANISM_GATEWAY_FEATURE_ID = "AFA-fiber-P08-F24" as const;
export const MECHANISM_GATEWAY_CONTRACT_VERSION = "federated-mechanism-interoperability-gateway/1.0" as const;
export const EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-adapter-P01-F09" as const;
export const EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "evidence-surveillance-copilot/1.0" as const;
export const RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-adapter-P02-F06" as const;
export const RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "multimodal-retrieval-synthesis/1.0" as const;
export const ADAPTER_CONTEXT_COMPILATION_FEATURE_ID = "AFA-adapter-P03-F27" as const;
export const ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION = "prospective-context-compilation-assurance/1.0" as const;
export const KNOWLEDGE_WORKFLOW_FEATURE_ID = "AFA-adapter-P04-F14" as const;
export const KNOWLEDGE_WORKFLOW_CONTRACT_VERSION = "multimodal-knowledge-workflow-fabric/1.0" as const;
export const RESOURCE_WORKBENCH_FEATURE_ID = "AFA-adapter-P05-F18" as const;
export const RESOURCE_WORKBENCH_CONTRACT_VERSION = "multimodal-resource-workbench/1.0" as const;
export const INGESTION_GATEWAY_FEATURE_ID = "AFA-adapter-P06-F23" as const;
export const INGESTION_GATEWAY_CONTRACT_VERSION = "1.0" as const;
export const QUALITY_ENVELOPE_FEATURE_ID = "AFA-adapter-P07-F06" as const;
export const QUALITY_ENVELOPE_CONTRACT_VERSION = "multi-study-quality-envelope/1.0" as const;
export const EXPERIMENT_DESIGN_CONTROL_FEATURE_ID = "AFA-adapter-P09-F30" as const;
export const EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION = "federated-experiment-design-control-plane/1.0" as const;
export const PROTOCOL_SIMULATION_FEATURE_ID = "AFA-adapter-P10-F03" as const;
export const PROTOCOL_SIMULATION_CONTRACT_VERSION = "prospective-protocol-simulation/1.0" as const;
export const INSTRUMENT_MESH_FEATURE_ID = "AFA-adapter-P11-F04" as const;
export const INSTRUMENT_MESH_CONTRACT_VERSION = "federated-laboratory-integration/1.0" as const;
export const EXECUTION_CONTROL_FEATURE_ID = "AFA-adapter-P12-F31" as const;
export const EXECUTION_CONTROL_CONTRACT_VERSION = "computational-execution-control-plane/1.0" as const;
export const ANALYSIS_PORTFOLIO_FEATURE_ID = "AFA-adapter-P13-F01" as const;
export const ANALYSIS_PORTFOLIO_CONTRACT_VERSION = "local-analysis-model-portfolio/1.0" as const;
export const INTERPRETATION_ASSURANCE_FEATURE_ID = "AFA-adapter-P14-F27" as const;
export const INTERPRETATION_ASSURANCE_CONTRACT_VERSION = "interpretation-assurance/1.0" as const;
export const REPLICATION_ASSURANCE_FEATURE_ID = "AFA-adapter-P15-F28" as const;
export const REPLICATION_ASSURANCE_CONTRACT_VERSION = "federated-replication-assurance/1.0" as const;
export const RELEASE_ASSURANCE_FEATURE_ID = "AFA-adapter-P16-F26" as const;
export const RELEASE_ASSURANCE_CONTRACT_VERSION = "multimodal-research-release-assurance/1.0" as const;

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

export interface EvaluationCardReceipt {
  schema_version: string;
  feature_id: string;
  card: Record<string, unknown> & {
    schema_version: string;
    capability_id: string;
    benchmark_world: string;
    baselines: readonly string[];
    metrics: readonly Record<string, unknown>[];
    uncertainty: readonly Record<string, unknown>[];
    release_verdict: "pass" | "conditional" | "blocked" | "not_evaluated";
  };
  card_digest: string;
  observations_digest: string;
  baseline_counts: Record<string, number>;
  omissions: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateEvaluationCardReceipt(receipt: EvaluationCardReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== EVALUATION_OBSERVABILITY_FEATURE_ID) throw new Error("evaluation-observability feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.card.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || !receipt.card.capability_id.trim() || !receipt.card.benchmark_world.trim()) throw new Error("evaluation card identity is incomplete");
  if (!receipt.card.baselines.length || !receipt.card.metrics.length || !receipt.card.uncertainty.length) throw new Error("evaluation card evidence fields are incomplete");
  if (!receipt.reasons.length || !Object.keys(receipt.baseline_counts).length) throw new Error("evaluation receipt needs baseline counts and reasons");
  if (receipt.card.release_verdict === "pass" && receipt.omissions.length) throw new Error("a passing evaluation card cannot hide baseline omissions");
  if (Object.values(receipt.baseline_counts).some((count) => !Number.isInteger(count) || count < 0)) throw new Error("evaluation baseline count is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.card_digest) || !/^[0-9a-f]{64}$/.test(receipt.observations_digest)) throw new Error("evaluation receipt source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("evaluation receipt artifact digest is invalid");
}

export function evaluationCardReceiptDigest(receipt: EvaluationCardReceipt): string {
  validateEvaluationCardReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchReleaseReceipt {
  schema_version: string;
  feature_id: string;
  release_id: string;
  research_object: {
    release_id: string;
    artifact_ids: readonly string[];
    evidence_receipt_ids: readonly string[];
    boundary: string;
    federation: {
      envelope: {
        raw_data_local: boolean;
        signature?: string | null;
        localization_statement: string;
        export: Record<string, unknown> & { content_hash: string; provenance: readonly Record<string, unknown>[] };
      };
    };
  };
  release_digest: string;
  omissions: readonly string[];
  reasons: readonly string[];
  boundary: string;
}

export function validateResearchReleaseReceipt(receipt: ResearchReleaseReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== RESEARCH_RELEASE_FEATURE_ID || !receipt.release_id.trim()) throw new Error("research-release feature or identity is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.research_object.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.research_object.release_id !== receipt.release_id) throw new Error("research object release identity does not match receipt");
  if (!receipt.research_object.artifact_ids.length || new Set(receipt.research_object.artifact_ids).size !== receipt.research_object.artifact_ids.length) throw new Error("research object artifact ids are incomplete or duplicated");
  if (!receipt.research_object.evidence_receipt_ids.length || new Set(receipt.research_object.evidence_receipt_ids).size !== receipt.research_object.evidence_receipt_ids.length) throw new Error("research object evidence ids are incomplete or duplicated");
  const envelope = receipt.research_object.federation.envelope;
  if (!envelope.raw_data_local || !envelope.signature || !envelope.localization_statement.trim()) throw new Error("research release signature and localization are required");
  if (!envelope.export.provenance.length) throw new Error("research release provenance is incomplete");
  if (!receipt.reasons.length) throw new Error("research release reasons are required");
  if (!/^[0-9a-f]{64}$/.test(receipt.release_digest) || !/^[0-9a-f]{64}$/.test(envelope.export.content_hash)) throw new Error("research release digest is not a canonical sha256");
}

export function researchReleaseReceiptDigest(receipt: ResearchReleaseReceipt): string {
  validateResearchReleaseReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface InstrumentPreflightReceipt {
  schema_version: string;
  feature_id: string;
  run_id: string;
  study_id: string;
  decision: "ready" | "blocked" | "requires_approval" | "emergency_stop";
  ordered_actions: readonly string[];
  action_digests: Record<string, string>;
  remaining_budget: Record<string, number>;
  omissions: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateInstrumentPreflightReceipt(receipt: InstrumentPreflightReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== INSTRUMENT_PREFLIGHT_FEATURE_ID || !receipt.run_id.trim() || !receipt.study_id.trim()) throw new Error("instrument-preflight identity is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.ordered_actions.length || !Object.keys(receipt.action_digests).length || !receipt.reasons.length) throw new Error("instrument preflight evidence is incomplete");
  if (new Set(receipt.ordered_actions).size !== receipt.ordered_actions.length || receipt.ordered_actions.some((action) => !(action in receipt.action_digests))) throw new Error("instrument action ordering or digest coverage is invalid");
  if (Object.values(receipt.action_digests).some((digest) => !/^[0-9a-f]{64}$/.test(digest))) throw new Error("instrument action digest is invalid");
  if (Object.values(receipt.remaining_budget).some((amount) => !Number.isFinite(amount) || amount < 0)) throw new Error("instrument remaining budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("instrument preflight artifact digest is invalid");
}

export function instrumentPreflightReceiptDigest(receipt: InstrumentPreflightReceipt): string {
  validateInstrumentPreflightReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface HarmonizedResearchObject {
  schema_version: string;
  feature_id: string;
  study_id: string;
  reference_schema: string;
  decision: "comparable" | "partial" | "blocked";
  modality_order: readonly string[];
  alignment: Record<string, readonly string[]>;
  omitted_modalities: readonly string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateHarmonizedResearchObject(object: HarmonizedResearchObject): void {
  if (object.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (object.feature_id !== MULTIMODAL_HARMONIZATION_FEATURE_ID || !object.study_id.trim() || !object.reference_schema.trim()) throw new Error("multimodal research object identity is incomplete");
  if (object.boundary !== PRECLINICAL_BOUNDARY || !object.raw_data_local) throw new Error("multimodal raw data must remain local");
  if (!object.modality_order.length || !Object.keys(object.alignment).length || !object.reasons.length) throw new Error("multimodal alignment and reasons are incomplete");
  if (object.modality_order.some((modality) => !(modality in object.alignment))) throw new Error("multimodal alignment omits a modality projection");
  if (typeof object.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(object.artifact.content_hash)) throw new Error("multimodal artifact digest is invalid");
}

export function harmonizedResearchObjectDigest(object: HarmonizedResearchObject): string {
  validateHarmonizedResearchObject(object);
  return digestJsonSync(object);
}

export interface QualifiedAnalysisResult {
  schema_version: string;
  feature_id: string;
  question_id: string;
  estimand: string;
  verdict: "qualified" | "conditional" | "blocked";
  selected_candidate: string | null;
  candidate_order: readonly string[];
  uncertainty: readonly string[];
  omissions: readonly string[];
  negative_evidence: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualifiedAnalysisResult(result: QualifiedAnalysisResult): void {
  if (result.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (result.feature_id !== ANALYSIS_QUALIFICATION_FEATURE_ID || !result.question_id.trim() || !result.estimand.trim()) throw new Error("qualified analysis identity is incomplete");
  if (result.boundary !== PRECLINICAL_BOUNDARY || !result.raw_data_local) throw new Error("qualified analysis must retain raw data locally");
  if (!result.candidate_order.length || !result.reasons.length || !result.uncertainty.length) throw new Error("qualified analysis evidence is incomplete");
  if (result.verdict === "qualified" && result.selected_candidate === null) throw new Error("qualified analysis needs a selected candidate");
  if (typeof result.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(result.artifact.content_hash)) throw new Error("qualified analysis artifact digest is invalid");
}

export function qualifiedAnalysisResultDigest(result: QualifiedAnalysisResult): string {
  validateQualifiedAnalysisResult(result);
  return digestJsonSync(result);
}

export interface ProtocolMatrixReceipt {
  schema_version: string;
  feature_id: string;
  protocol_id: string;
  total_cells: number;
  passed_cells: number;
  failed_closed_cells: number;
  approval_cells: number;
  cells: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateProtocolMatrixReceipt(receipt: ProtocolMatrixReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== PROTOCOL_MATRIX_FEATURE_ID || !receipt.protocol_id.trim()) throw new Error("protocol matrix identity is incomplete");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!Number.isInteger(receipt.total_cells) || receipt.total_cells <= 0 || receipt.total_cells !== receipt.cells.length) throw new Error("protocol matrix cell count is invalid");
  if ([receipt.passed_cells, receipt.failed_closed_cells, receipt.approval_cells].some((value) => !Number.isInteger(value) || value < 0)) throw new Error("protocol matrix status count is invalid");
  if (receipt.passed_cells + receipt.failed_closed_cells + receipt.approval_cells !== receipt.total_cells) throw new Error("protocol matrix status counts do not partition cells");
  if (!receipt.cells.length || receipt.cells.some((cell) => typeof cell.cell_id !== "string" || !cell.cell_id.trim() || !Array.isArray(cell.reasons) || (cell.reasons as unknown[]).length === 0)) throw new Error("protocol matrix cells need ids and reasons");
  const statuses = new Set(["passed", "failed_closed", "requires_approval"]);
  if (receipt.cells.some((cell) => typeof cell.status !== "string" || !statuses.has(cell.status))) throw new Error("protocol matrix cell status is unknown");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol matrix artifact digest is invalid");
}

export function protocolMatrixReceiptDigest(receipt: ProtocolMatrixReceipt): string {
  validateProtocolMatrixReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface MultimodalReplicationReport {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  claim: string;
  request_digest: string;
  required_modalities: readonly string[];
  summary: Record<string, unknown>;
  studies: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateMultimodalReplicationReport(report: MultimodalReplicationReport): void {
  if (report.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.feature_id !== MULTIMODAL_REPLICATION_FEATURE_ID || !report.capability_id.trim() || !report.claim.trim()) throw new Error("multimodal replication identity is incomplete");
  if (report.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!report.required_modalities.length || !report.studies.length) throw new Error("multimodal replication evidence set is incomplete");
  const disposition = report.summary.disposition;
  if (typeof disposition !== "string" || !new Set(["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence"]).has(disposition)) throw new Error("multimodal replication disposition is unknown");
  if (report.summary.total_observations !== report.studies.length || !Array.isArray(report.summary.reasons) || report.summary.reasons.length === 0) throw new Error("multimodal replication summary is inconsistent");
  if (report.studies.some((study) => typeof study.study_id !== "string" || !study.study_id.trim() || !Array.isArray(study.reasons))) throw new Error("multimodal study comparability record is incomplete");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("multimodal replication artifact digest is invalid");
}

export function multimodalReplicationReportDigest(report: MultimodalReplicationReport): string {
  validateMultimodalReplicationReport(report);
  return digestJsonSync(report);
}

export interface QualityDriftReceipt {
  schema_version: string;
  feature_id: string;
  dataset_id: string;
  modality: string;
  request_digest: string;
  summary: Record<string, unknown>;
  metrics: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualityDriftReceipt(receipt: QualityDriftReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_DRIFT_FEATURE_ID) throw new Error("quality drift schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.dataset_id.trim() || !receipt.modality.trim()) throw new Error("quality drift identity or locality is invalid");
  if (typeof receipt.summary.disposition !== "string" || !new Set(["stable", "drifted", "unknown", "blocked"]).has(receipt.summary.disposition)) throw new Error("quality drift disposition is unknown");
  if (!receipt.metrics.length || !Array.isArray(receipt.summary.reasons) || receipt.summary.reasons.length === 0) throw new Error("quality drift metrics and reasons are incomplete");
  if (receipt.metrics.length !== Number(receipt.summary.stable ?? 0) + Number(receipt.summary.drifted ?? 0) + Number(receipt.summary.unknown ?? 0)) throw new Error("quality drift metric counts are inconsistent");
  if (!/^[0-9a-f]{64}$/.test(receipt.request_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality drift digest is invalid");
}

export function qualityDriftReceiptDigest(receipt: QualityDriftReceipt): string {
  validateQualityDriftReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface DesignFrontierReceipt {
  schema_version: string;
  feature_id: string;
  study_id: string;
  feasible_scenarios: number;
  blocked_scenarios: number;
  scenarios: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateDesignFrontierReceipt(receipt: DesignFrontierReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== DESIGN_FRONTIER_FEATURE_ID) throw new Error("design frontier schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.study_id.trim() || !receipt.scenarios.length) throw new Error("design frontier identity or boundary is invalid");
  if (receipt.feasible_scenarios < 0 || receipt.blocked_scenarios < 0 || receipt.feasible_scenarios + receipt.blocked_scenarios !== receipt.scenarios.length) throw new Error("design frontier scenario counts are inconsistent");
  if (receipt.scenarios.some((scenario) => typeof scenario.scenario_id !== "string" || !scenario.scenario_id.trim() || !new Set(["feasible", "blocked"]).has(String(scenario.disposition)) || !Array.isArray(scenario.reasons) || scenario.reasons.length === 0)) throw new Error("design frontier scenario record is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("design frontier artifact digest is invalid");
}

export function designFrontierReceiptDigest(receipt: DesignFrontierReceipt): string {
  validateDesignFrontierReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface BatchAdmissionReceipt {
  schema_version: string;
  feature_id: string;
  actor: string;
  total_actions: number;
  allowed_actions: number;
  approval_actions: number;
  denied_actions: number;
  actions: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateBatchAdmissionReceipt(receipt: BatchAdmissionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== AUTONOMY_BATCH_FEATURE_ID) throw new Error("autonomy batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.actor.trim() || receipt.total_actions <= 0 || receipt.total_actions !== receipt.actions.length) throw new Error("autonomy batch identity or boundary is invalid");
  if ([receipt.allowed_actions, receipt.approval_actions, receipt.denied_actions].some((value) => !Number.isInteger(value) || value < 0) || receipt.allowed_actions + receipt.approval_actions + receipt.denied_actions !== receipt.total_actions) throw new Error("autonomy batch counts are inconsistent");
  if (receipt.actions.some((action) => typeof action.action_id !== "string" || !action.action_id.trim() || !new Set(["allowed", "approval_required", "denied"]).has(String(action.decision)) || !Array.isArray(action.reasons) || action.reasons.length === 0)) throw new Error("autonomy batch action record is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("autonomy batch artifact digest is invalid");
}

export function batchAdmissionReceiptDigest(receipt: BatchAdmissionReceipt): string {
  validateBatchAdmissionReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface WorkflowBatchReceipt {
  schema_version: string;
  feature_id: string;
  total_workflows: number;
  succeeded_workflows: number;
  dry_run_workflows: number;
  blocked_workflows: number;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateWorkflowBatchReceipt(receipt: WorkflowBatchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== WORKFLOW_BATCH_FEATURE_ID) throw new Error("workflow batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.total_workflows <= 0 || receipt.total_workflows !== receipt.entries.length) throw new Error("workflow batch identity or boundary is invalid");
  if ([receipt.succeeded_workflows, receipt.dry_run_workflows, receipt.blocked_workflows].some((value) => !Number.isInteger(value) || value < 0) || receipt.succeeded_workflows + receipt.dry_run_workflows + receipt.blocked_workflows !== receipt.total_workflows) throw new Error("workflow batch counts are inconsistent");
  if (receipt.entries.some((entry) => typeof entry.workflow_id !== "string" || !entry.workflow_id.trim() || !new Set(["succeeded", "dry_run", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0)) throw new Error("workflow batch entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("workflow batch artifact digest is invalid");
}

export function workflowBatchReceiptDigest(receipt: WorkflowBatchReceipt): string {
  validateWorkflowBatchReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchReleaseBatchReceipt {
  schema_version: string;
  feature_id: string;
  total_releases: number;
  published_releases: number;
  blocked_releases: number;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateResearchReleaseBatchReceipt(receipt: ResearchReleaseBatchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESEARCH_RELEASE_BATCH_FEATURE_ID) throw new Error("research-release batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.total_releases <= 0 || receipt.total_releases !== receipt.entries.length) throw new Error("research-release batch identity or boundary is invalid");
  if (![receipt.published_releases, receipt.blocked_releases].every((value) => Number.isInteger(value) && value >= 0) || receipt.published_releases + receipt.blocked_releases !== receipt.total_releases) throw new Error("research-release batch counts are inconsistent");
  if (receipt.entries.some((entry) => typeof entry.release_id !== "string" || !entry.release_id.trim() || !new Set(["published", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0 || (entry.disposition === "published" && typeof entry.release_digest !== "string"))) throw new Error("research-release batch entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("research-release batch artifact digest is invalid");
}

export function researchReleaseBatchReceiptDigest(receipt: ResearchReleaseBatchReceipt): string {
  validateResearchReleaseBatchReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedEvaluationReceipt {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  benchmark_world: string;
  minimum_sites: number;
  total_sites: number;
  agreeing_sites: number;
  contradictory_sites: number;
  blocked_sites: number;
  disposition: string;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedEvaluationReceipt(receipt: FederatedEvaluationReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_EVALUATION_FEATURE_ID) throw new Error("federated evaluation schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.capability_id.trim() || !receipt.benchmark_world.trim() || !Number.isInteger(receipt.minimum_sites) || receipt.minimum_sites <= 0 || !Number.isInteger(receipt.total_sites) || receipt.total_sites <= 0 || receipt.total_sites !== receipt.entries.length) throw new Error("federated evaluation identity or boundary is invalid");
  if ([receipt.agreeing_sites, receipt.contradictory_sites, receipt.blocked_sites].some((value) => !Number.isInteger(value) || value < 0) || receipt.agreeing_sites + receipt.contradictory_sites + receipt.blocked_sites !== receipt.total_sites) throw new Error("federated evaluation counts are inconsistent");
  if (!new Set(["consensus", "partial", "contradicted", "blocked"]).has(receipt.disposition)) throw new Error("federated evaluation disposition is unknown");
  if (receipt.entries.some((entry) => typeof entry.site_id !== "string" || !entry.site_id.trim() || !new Set(["accepted", "contradictory", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0 || (entry.disposition === "accepted" && typeof entry.card_digest !== "string"))) throw new Error("federated evaluation site entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated evaluation artifact digest is invalid");
}

export function federatedEvaluationReceiptDigest(receipt: FederatedEvaluationReceipt): string {
  validateFederatedEvaluationReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface QualifiedResourceSet {
  schema_version: string;
  feature_id: string;
  need_id: string;
  requester: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  considered_candidates: number;
  qualified_count: number;
  resources: readonly Record<string, unknown>[];
  omissions: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateQualifiedResourceSet(receipt: QualifiedResourceSet): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID) throw new Error("resource workbench schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.need_id.trim() || !receipt.requester.trim()) throw new Error("resource workbench identity or boundary is invalid");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("resource discovery disposition is unknown");
  if (!Number.isInteger(receipt.considered_candidates) || receipt.considered_candidates <= 0 || !Number.isInteger(receipt.qualified_count) || receipt.qualified_count < 0 || receipt.qualified_count !== receipt.resources.length || receipt.reasons.length === 0) throw new Error("resource discovery counts or reasons are incomplete");
  if (receipt.resources.some((resource) => typeof resource.resource_id !== "string" || !resource.resource_id.trim() || typeof resource.origin !== "string" || !resource.origin.trim() || !Number.isInteger(resource.rank) || Number(resource.rank) <= 0 || !Array.isArray(resource.reasons) || resource.reasons.length === 0)) throw new Error("qualified resource entry is incomplete");
  if (receipt.omissions.some((omission) => typeof omission.resource_id !== "string" || !omission.resource_id.trim() || typeof omission.reason !== "string" || !omission.reason.trim())) throw new Error("resource omission entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource workbench artifact digest is invalid");
}

export function qualifiedResourceSetDigest(receipt: QualifiedResourceSet): string {
  validateQualifiedResourceSet(receipt);
  return digestJsonSync(receipt);
}

export interface ResourceDiscoveryContractReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  requested_by: string;
  compatibility_profile: string;
  result: Record<string, unknown>;
  migration_notes: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateResourceDiscoveryContractReceipt(receipt: ResourceDiscoveryContractReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID || receipt.contract_version !== RESOURCE_DISCOVERY_CONTRACT_VERSION) throw new Error("resource discovery contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.requested_by.trim() || !receipt.compatibility_profile.trim() || new TextEncoder().encode(receipt.compatibility_profile).length > 256 || receipt.migration_notes.length === 0) throw new Error("resource discovery contract identity, compatibility, migration, or boundary is invalid");
  if (receipt.result.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID || receipt.result.boundary !== PRECLINICAL_BOUNDARY) throw new Error("resource discovery contract result is not the qualified-resource contract");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource discovery contract artifact digest is invalid");
}

export function resourceDiscoveryContractReceiptDigest(receipt: ResourceDiscoveryContractReceipt): string {
  validateResourceDiscoveryContractReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface SignedResearchObjectReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  run_id: string;
  release_id: string;
  origin: string;
  purpose: string;
  artifact_ids: string[];
  evidence_receipt_ids: string[];
  release_digest: string;
  signer_public_key_hex: string;
  signer_signature_hex: string;
  migration_notes: string[];
  omissions: string[];
  raw_data_local: boolean;
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateSignedResearchObjectReceipt(receipt: SignedResearchObjectReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID || receipt.contract_version !== GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION) throw new Error("governance research-release schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || [receipt.run_id, receipt.release_id, receipt.origin, receipt.purpose].some((value) => !value.trim())) throw new Error("signed research object identity or locality is invalid");
  if (receipt.artifact_ids.length === 0 || new Set(receipt.artifact_ids).size !== receipt.artifact_ids.length || receipt.evidence_receipt_ids.length === 0 || new Set(receipt.evidence_receipt_ids).size !== receipt.evidence_receipt_ids.length || receipt.migration_notes.length === 0) throw new Error("signed research object provenance or migration is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.release_digest) || !/^[0-9a-f]{64}$/.test(receipt.signer_public_key_hex) || !/^[0-9a-f]{128}$/.test(receipt.signer_signature_hex)) throw new Error("signed research object signature material is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("signed research object artifact digest is invalid");
}

export function signedResearchObjectReceiptDigest(receipt: SignedResearchObjectReceipt): string {
  validateSignedResearchObjectReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ReleaseHarnessReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  object_digest: string;
  disposition: "passed" | "blocked" | "unknown";
  checks: readonly Record<string, unknown>[];
  omissions: string[];
  reasons: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateReleaseHarnessReceipt(receipt: ReleaseHarnessReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RELEASE_HARNESS_FEATURE_ID || receipt.contract_version !== RELEASE_HARNESS_CONTRACT_VERSION) throw new Error("release harness schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0 || receipt.reasons.length === 0) throw new Error("release harness identity, disposition, checks, or boundary is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.object_digest)) throw new Error("release harness object digest is invalid");
  if (receipt.checks.some((check) => typeof check.check_id !== "string" || !check.check_id.trim() || !new Set(["passed", "blocked", "unknown"]).has(String(check.disposition)) || typeof check.reason !== "string" || !check.reason.trim())) throw new Error("release harness check is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("release harness artifact digest is invalid");
}

export function releaseHarnessReceiptDigest(receipt: ReleaseHarnessReceipt): string {
  validateReleaseHarnessReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ProtocolAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  protocol_id: string;
  disposition: "passed" | "blocked" | "unknown";
  total_cells: number;
  passed_cells: number;
  blocked_cells: number;
  unknown_cells: number;
  checks: string[];
  omissions: string[];
  simulation_digest: string;
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateProtocolAssuranceReceipt(receipt: ProtocolAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== PROTOCOL_ASSURANCE_FEATURE_ID || receipt.contract_version !== PROTOCOL_ASSURANCE_CONTRACT_VERSION) throw new Error("protocol assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.protocol_id.trim()) throw new Error("protocol assurance identity or boundary is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("protocol assurance disposition or checks is incomplete");
  if (!Number.isInteger(receipt.total_cells) || receipt.total_cells <= 0 || [receipt.passed_cells, receipt.blocked_cells, receipt.unknown_cells].some((value) => !Number.isInteger(value) || value < 0) || receipt.total_cells !== receipt.passed_cells + receipt.blocked_cells + receipt.unknown_cells) throw new Error("protocol assurance cell counts do not partition");
  if (!/^[0-9a-f]{64}$/.test(receipt.simulation_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol assurance digest is not a canonical sha256");
}

export function protocolAssuranceReceiptDigest(receipt: ProtocolAssuranceReceipt): string {
  validateProtocolAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedMultimodalAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  benchmark_id: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  harmonized_digest: string;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedMultimodalAssuranceReceipt(receipt: FederatedMultimodalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION) throw new Error("federated multimodal assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.benchmark_id.trim()) throw new Error("federated multimodal assurance identity or locality is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || new Set(receipt.institution_ids).size !== receipt.institution_ids.length) throw new Error("federated multimodal institution set is incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated multimodal disposition or checks is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.harmonized_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated multimodal digest is not a canonical sha256");
}

export function federatedMultimodalAssuranceReceiptDigest(receipt: FederatedMultimodalAssuranceReceipt): string {
  validateFederatedMultimodalAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedKnowledgeGatewayReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  interoperability_profile: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  manifest_digest: string;
  permitted_tags: string[];
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedKnowledgeGatewayReceipt(receipt: FederatedKnowledgeGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID || receipt.contract_version !== FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION) throw new Error("federated knowledge gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.interoperability_profile.trim()) throw new Error("federated knowledge gateway identity or locality is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || new Set(receipt.institution_ids).size !== receipt.institution_ids.length) throw new Error("federated knowledge institution set is incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated knowledge disposition or checks is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.manifest_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated knowledge digest is not a canonical sha256");
}

export function federatedKnowledgeGatewayReceiptDigest(receipt: FederatedKnowledgeGatewayReceipt): string {
  validateFederatedKnowledgeGatewayReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedLensAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  institution_ids: string[];
  required_lens_ids: string[];
  report_digests: string[];
  absent_lens_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedLensAssuranceReceipt(receipt: FederatedLensAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_LENS_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION) throw new Error("federated lens assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim()) throw new Error("federated lens assurance identity or boundary is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || JSON.stringify([...receipt.institution_ids].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("federated lens institution ordering is invalid");
  if (receipt.required_lens_ids.length === 0 || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated lens required set, disposition, or checks is incomplete");
  if (receipt.report_digests.some((digest) => !/^[0-9a-f]{64}$/.test(digest)) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated lens digest is invalid");
}

export function federatedLensAssuranceReceiptDigest(receipt: FederatedLensAssuranceReceipt): string {
  validateFederatedLensAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface LabSemanticParityReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  protocol_id: string;
  benchmark_id: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  semantic_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateLabSemanticParityReceipt(receipt: LabSemanticParityReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== SEMANTIC_PARITY_FEATURE_ID || receipt.contract_version !== SEMANTIC_PARITY_CONTRACT_VERSION) throw new Error("lab semantic parity schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.protocol_id.trim() || !receipt.benchmark_id.trim()) throw new Error("lab semantic parity identity or boundary is invalid");
  if (receipt.institution_ids.length < 2 || JSON.stringify([...new Set(receipt.institution_ids)].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("lab semantic parity institution ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("lab semantic parity disposition or checks is incomplete");
  if (receipt.semantic_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.semantic_digest)) throw new Error("lab semantic parity semantic digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("lab semantic parity artifact digest is invalid");
}

export function labSemanticParityReceiptDigest(receipt: LabSemanticParityReceipt): string {
  validateLabSemanticParityReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedRetrievalAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  returned_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedRetrievalAssuranceReceipt(receipt: FederatedRetrievalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("federated retrieval assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("federated retrieval identity, boundary, or checks are incomplete");
  if (JSON.stringify([...new Set(receipt.returned_source_ids)].sort()) !== JSON.stringify(receipt.returned_source_ids)) throw new Error("federated retrieval source ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("federated retrieval disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("federated retrieval evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated retrieval artifact digest is invalid");
}

export function federatedRetrievalAssuranceReceiptDigest(receipt: FederatedRetrievalAssuranceReceipt): string {
  validateFederatedRetrievalAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface RetrievalSourceUpdate {
  source_id: string;
  version: string;
  digest: string;
  evidence_state: string;
  stale: boolean;
}

export interface FederatedContinualRetrievalReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  selected_source_ids: string[];
  stale_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  prior_synthesis_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedContinualRetrievalReceipt(receipt: FederatedContinualRetrievalReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID || receipt.contract_version !== FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION) throw new Error("federated continual retrieval schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("federated continual retrieval identity, boundary, or checks are incomplete");
  if (!receipt.selected_source_ids.length || JSON.stringify([...new Set(receipt.selected_source_ids)].sort()) !== JSON.stringify(receipt.selected_source_ids)) throw new Error("federated continual retrieval source ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("federated continual retrieval disposition is unknown");
  if (receipt.prior_synthesis_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.prior_synthesis_digest)) throw new Error("federated continual retrieval prior digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated continual retrieval artifact digest is invalid");
}

export function federatedContinualRetrievalReceiptDigest(receipt: FederatedContinualRetrievalReceipt): string {
  validateFederatedContinualRetrievalReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ContextCompilationAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  resolved_context_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateContextCompilationAssuranceReceipt(receipt: ContextCompilationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("context compilation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("context compilation assurance identity, boundary, or checks are incomplete");
  if (!receipt.resolved_context_ids.length || JSON.stringify([...new Set(receipt.resolved_context_ids)].sort()) !== JSON.stringify(receipt.resolved_context_ids)) throw new Error("context compilation resolved identities are invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("context compilation disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("context compilation evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("context compilation artifact digest is invalid");
}

export function contextCompilationAssuranceReceiptDigest(receipt: ContextCompilationAssuranceReceipt): string {
  validateContextCompilationAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface KnowledgeRepresentationAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  resolved_fact_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateKnowledgeRepresentationAssuranceReceipt(receipt: KnowledgeRepresentationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION) throw new Error("knowledge representation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("knowledge representation assurance identity, boundary, or checks are incomplete");
  if (!receipt.resolved_fact_ids.length || JSON.stringify([...new Set(receipt.resolved_fact_ids)].sort()) !== JSON.stringify(receipt.resolved_fact_ids)) throw new Error("knowledge representation fact identities are invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("knowledge representation disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("knowledge representation evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("knowledge representation artifact digest is invalid");
}

export function knowledgeRepresentationAssuranceReceiptDigest(receipt: KnowledgeRepresentationAssuranceReceipt): string {
  validateKnowledgeRepresentationAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResourceControlPlaneReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; institution_ids: string[]; qualified_resource_ids: string[]; disposition: "passed" | "blocked" | "unknown"; qualification_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateResourceControlPlaneReceipt(receipt: ResourceControlPlaneReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_CONTROL_PLANE_FEATURE_ID || receipt.contract_version !== RESOURCE_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("resource control-plane schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || receipt.institution_ids.length < 2 || JSON.stringify([...new Set(receipt.institution_ids)].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("resource control-plane identity is invalid"); if (!receipt.qualified_resource_ids.length || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || !receipt.checks.length) throw new Error("resource control-plane qualification is incomplete"); if (receipt.qualification_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.qualification_digest)) throw new Error("resource control-plane digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource control-plane artifact digest is invalid"); }
export function resourceControlPlaneReceiptDigest(receipt: ResourceControlPlaneReceipt): string { validateResourceControlPlaneReceipt(receipt); return digestJsonSync(receipt); }

export interface WeaveLangReleaseAssuranceReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; run_id: string; release_id: string; disposition: "passed" | "blocked" | "unknown"; artifact_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateWeaveLangReleaseAssuranceReceipt(receipt: WeaveLangReleaseAssuranceReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID || receipt.contract_version !== WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION) throw new Error("WeaveLang release assurance schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.run_id.trim() || !receipt.release_id.trim() || !receipt.checks.length) throw new Error("WeaveLang release assurance identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("WeaveLang release assurance disposition is unknown"); if (receipt.artifact_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.artifact_digest)) throw new Error("WeaveLang release artifact digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("WeaveLang release receipt digest is invalid"); }
export function weaveLangReleaseAssuranceReceiptDigest(receipt: WeaveLangReleaseAssuranceReceipt): string { validateWeaveLangReleaseAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface MechanismControlPlaneReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; question_id: string; admitted_candidate_ids: string[]; disposition: "passed" | "blocked" | "unknown"; evidence_receipt_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateMechanismControlPlaneReceipt(receipt: MechanismControlPlaneReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MECHANISM_CONTROL_PLANE_FEATURE_ID || receipt.contract_version !== MECHANISM_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("mechanism control-plane schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.question_id.trim() || !receipt.admitted_candidate_ids.length || !receipt.checks.length) throw new Error("mechanism control-plane identity or checks are incomplete"); if (JSON.stringify([...new Set(receipt.admitted_candidate_ids)].sort()) !== JSON.stringify(receipt.admitted_candidate_ids)) throw new Error("mechanism candidate ordering is invalid"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("mechanism control-plane disposition is unknown"); if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("mechanism evidence digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("mechanism receipt digest is invalid"); }
export function mechanismControlPlaneReceiptDigest(receipt: MechanismControlPlaneReceipt): string { validateMechanismControlPlaneReceipt(receipt); return digestJsonSync(receipt); }

export interface MechanismGatewayReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; source_profile: string; target_profile: string; projected_candidate_ids: string[]; interoperability_profile: string; disposition: "passed" | "blocked" | "unknown"; projection_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateMechanismGatewayReceipt(receipt: MechanismGatewayReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MECHANISM_GATEWAY_FEATURE_ID || receipt.contract_version !== MECHANISM_GATEWAY_CONTRACT_VERSION) throw new Error("mechanism gateway schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.source_profile.trim() || !receipt.target_profile.trim() || !receipt.interoperability_profile.trim() || !receipt.projected_candidate_ids.length || !receipt.checks.length) throw new Error("mechanism gateway identity or checks are incomplete"); if (JSON.stringify([...new Set(receipt.projected_candidate_ids)].sort()) !== JSON.stringify(receipt.projected_candidate_ids)) throw new Error("mechanism gateway candidate ordering is invalid"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("mechanism gateway disposition is unknown"); if (receipt.projection_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.projection_digest)) throw new Error("mechanism gateway projection digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("mechanism gateway receipt digest is invalid"); }
export function mechanismGatewayReceiptDigest(receipt: MechanismGatewayReceipt): string { validateMechanismGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface EvidenceSurveillanceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  study_id: string;
  intent: string;
  selected_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  qualified_set: Record<string, unknown>;
  effect_receipts: readonly Record<string, unknown>[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateEvidenceSurveillanceReceipt(receipt: EvidenceSurveillanceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("evidence surveillance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.intent.trim() || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("evidence surveillance identity or checks are incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("evidence surveillance disposition is unknown");
  if (JSON.stringify(receipt.qualified_set.selected_source_ids) !== JSON.stringify(receipt.selected_source_ids) || receipt.qualified_set.study_id !== receipt.study_id || receipt.qualified_set.intent !== receipt.intent) throw new Error("qualified evidence set is not linked to its receipt");
  if (receipt.qualified_set.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.qualified_set.boundary !== PRECLINICAL_BOUNDARY || receipt.qualified_set.ordering_rule !== "relevance_score descending, source_id ascending") throw new Error("qualified evidence set schema, boundary, or ordering is invalid");
  if (new Set(receipt.selected_source_ids).size !== receipt.selected_source_ids.length) throw new Error("qualified evidence source identities are not unique");
  if (receipt.qualified_set.evidence_state === "proven" && (receipt.omissions.length || receipt.uncertainty.length)) throw new Error("proven evidence cannot contain unresolved omissions");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("evidence surveillance artifact digest is invalid");
  for (const effect of receipt.effect_receipts) if (effect.effect !== "read_local_data" || typeof effect.authorized !== "boolean" || typeof effect.reason !== "string" || typeof effect.receipt_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.receipt_digest)) throw new Error("evidence surveillance effect receipt is invalid");
}

export function evidenceSurveillanceReceiptDigest(receipt: EvidenceSurveillanceReceipt): string { validateEvidenceSurveillanceReceipt(receipt); return digestJsonSync(receipt); }

export interface RetrievalSynthesisReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  query_id: string;
  disposition: "passed" | "blocked" | "unknown";
  synthesis: Record<string, unknown>;
  effect_receipts: readonly Record<string, unknown>[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateRetrievalSynthesisReceipt(receipt: RetrievalSynthesisReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RETRIEVAL_SYNTHESIS_FEATURE_ID || receipt.contract_version !== RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("retrieval synthesis schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.query_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("retrieval synthesis identity or checks are incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("retrieval synthesis disposition is unknown");
  if (receipt.synthesis.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.synthesis.query_id !== receipt.query_id || receipt.synthesis.boundary !== PRECLINICAL_BOUNDARY || typeof receipt.synthesis.comparability_profile !== "string" || !receipt.synthesis.comparability_profile.trim()) throw new Error("retrieval synthesis linkage or boundary is invalid");
  if (JSON.stringify(receipt.synthesis.omissions) !== JSON.stringify(receipt.omissions) || JSON.stringify(receipt.synthesis.uncertainty) !== JSON.stringify(receipt.uncertainty)) throw new Error("retrieval synthesis omission linkage is invalid");
  if (!Array.isArray(receipt.synthesis.selected_evidence_ids) || new Set(receipt.synthesis.selected_evidence_ids).size !== receipt.synthesis.selected_evidence_ids.length || receipt.synthesis.selected_evidence_ids.length !== receipt.synthesis.selected_digests.length || receipt.synthesis.selected_evidence_ids.length !== receipt.synthesis.selected_modalities.length) throw new Error("retrieval synthesis selected evidence alignment is invalid");
  if (receipt.synthesis.evidence_state === "proven" && (receipt.omissions.length || receipt.uncertainty.length)) throw new Error("proven synthesis cannot contain unresolved omissions");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("retrieval synthesis artifact digest is invalid");
  for (const effect of receipt.effect_receipts) if (effect.effect !== "read_local_data" || typeof effect.authorized !== "boolean" || typeof effect.reason !== "string" || typeof effect.receipt_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.receipt_digest)) throw new Error("retrieval synthesis effect receipt is invalid");
}

export function retrievalSynthesisReceiptDigest(receipt: RetrievalSynthesisReceipt): string { validateRetrievalSynthesisReceipt(receipt); return digestJsonSync(receipt); }

export interface AdapterContextCompilationReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; query_id: string; resolved_fact_ids: string[]; disposition: "passed" | "blocked" | "unknown"; evidence_receipt_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateAdapterContextCompilationReceipt(receipt: AdapterContextCompilationReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ADAPTER_CONTEXT_COMPILATION_FEATURE_ID || receipt.contract_version !== ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("adapter context compilation schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.query_id.trim() || !receipt.checks.length) throw new Error("adapter context compilation identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("adapter context compilation disposition is unknown"); if (!receipt.resolved_fact_ids.length || new Set(receipt.resolved_fact_ids).size !== receipt.resolved_fact_ids.length) throw new Error("resolved decision fact identities are invalid"); if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("adapter context evidence digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter context artifact digest is invalid"); }
export function adapterContextCompilationReceiptDigest(receipt: AdapterContextCompilationReceipt): string { validateAdapterContextCompilationReceipt(receipt); return digestJsonSync(receipt); }

export interface KnowledgeWorkflowReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; workflow_id: string; disposition: "passed" | "blocked" | "unknown"; world: Record<string, unknown>; checks: string[]; omissions: string[]; uncertainty: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateKnowledgeWorkflowReceipt(receipt: KnowledgeWorkflowReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== KNOWLEDGE_WORKFLOW_FEATURE_ID || receipt.contract_version !== KNOWLEDGE_WORKFLOW_CONTRACT_VERSION) throw new Error("knowledge workflow schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.checks.length) throw new Error("knowledge workflow identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("knowledge workflow disposition is unknown"); if (receipt.world.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.world.workflow_id !== receipt.workflow_id || receipt.world.boundary !== PRECLINICAL_BOUNDARY || !Array.isArray(receipt.world.study_ids) || !receipt.world.study_ids.length || !Array.isArray(receipt.world.stages) || !receipt.world.stages.length) throw new Error("typed knowledge world linkage is invalid"); if (JSON.stringify(receipt.world.omissions) !== JSON.stringify(receipt.omissions) || JSON.stringify(receipt.world.uncertainty) !== JSON.stringify(receipt.uncertainty)) throw new Error("knowledge workflow omission linkage is invalid"); if (!Array.isArray(receipt.world.resolved_claim_ids) || new Set(receipt.world.resolved_claim_ids).size !== receipt.world.resolved_claim_ids.length) throw new Error("typed knowledge claim identities are not unique"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("knowledge workflow artifact digest is invalid"); }
export function knowledgeWorkflowReceiptDigest(receipt: KnowledgeWorkflowReceipt): string { validateKnowledgeWorkflowReceipt(receipt); return digestJsonSync(receipt); }

export interface ResourceWorkbenchReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; need_id: string; disposition: "qualified" | "partial" | "blocked" | "unknown"; qualified_resources: readonly Record<string, unknown>[]; omissions: readonly Record<string, unknown>[]; checks: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateResourceWorkbenchReceipt(receipt: ResourceWorkbenchReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID || receipt.contract_version !== RESOURCE_WORKBENCH_CONTRACT_VERSION) throw new Error("resource workbench schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.need_id.trim() || !receipt.checks.length) throw new Error("resource workbench identity or checks are incomplete"); if (!new Set(["qualified", "partial", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("resource workbench disposition is unknown"); receipt.qualified_resources.forEach((item, index) => { if (item.rank !== index + 1 || typeof item.resource_id !== "string" || !item.resource_id.trim() || typeof item.origin !== "string" || !item.origin.trim() || !Array.isArray(item.reasons) || !item.reasons.length || typeof item.artifact_digest !== "string" || !/^[0-9a-f]{64}$/.test(item.artifact_digest)) throw new Error("qualified resource ranking, reasons, or digest is invalid"); }); receipt.omissions.forEach((item) => { if (typeof item.resource_id !== "string" || !item.resource_id.trim() || typeof item.reason !== "string" || !item.reason.trim()) throw new Error("resource omission is incomplete"); }); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource workbench artifact digest is invalid"); }
export function resourceWorkbenchReceiptDigest(receipt: ResourceWorkbenchReceipt): string { validateResourceWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface IngestionGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  study_id: string;
  disposition: "admitted" | "partial" | "blocked";
  harmonized: Record<string, unknown>;
  admitted_bundles: string[];
  omitted_bundles: string[];
  effect_receipts: readonly Record<string, unknown>[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateIngestionGatewayReceipt(receipt: IngestionGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INGESTION_GATEWAY_FEATURE_ID || receipt.contract_version !== INGESTION_GATEWAY_CONTRACT_VERSION) throw new Error("ingestion gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.reasons.length) throw new Error("ingestion gateway identity, locality, or reasons are incomplete");
  if (!new Set(["admitted", "partial", "blocked"]).has(receipt.disposition)) throw new Error("ingestion gateway disposition is unknown");
  if (receipt.harmonized.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.harmonized.study_id !== receipt.study_id || receipt.harmonized.boundary !== PRECLINICAL_BOUNDARY) throw new Error("harmonized research object linkage is invalid");
  if (new Set(receipt.admitted_bundles).size !== receipt.admitted_bundles.length || new Set(receipt.omitted_bundles).size !== receipt.omitted_bundles.length) throw new Error("ingestion gateway bundle identities are not unique");
  if (receipt.disposition === "blocked" && receipt.effect_receipts.length) throw new Error("blocked gateway receipts cannot contain effects");
  if (receipt.effect_receipts.length !== receipt.admitted_bundles.length) throw new Error("each admitted bundle needs one effect receipt");
  for (const effect of receipt.effect_receipts) if (effect.action !== "admit-local-harmonization" || effect.authorized !== true || !receipt.admitted_bundles.includes(String(effect.bundle_id)) || typeof effect.source_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.source_digest)) throw new Error("ingestion gateway effect receipt is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("ingestion gateway artifact digest is invalid");
}

export function ingestionGatewayReceiptDigest(receipt: IngestionGatewayReceipt): string { validateIngestionGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface QualityEnvelopeReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  envelope_id: string;
  reference_schema: string;
  comparability_profile: string;
  decision: "qualified" | "partial" | "blocked" | "unknown";
  study_order: string[];
  modality_coverage: Record<string, number>;
  verdicts: readonly Record<string, unknown>[];
  omitted_modalities: string[];
  comparability_conflicts: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualityEnvelopeReceipt(receipt: QualityEnvelopeReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_ENVELOPE_FEATURE_ID || receipt.contract_version !== QUALITY_ENVELOPE_CONTRACT_VERSION) throw new Error("quality envelope schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.envelope_id.trim() || !receipt.reference_schema.trim() || !receipt.comparability_profile.trim() || !receipt.reasons.length) throw new Error("quality envelope identity, locality, profile, or reasons are incomplete");
  if (!new Set(["qualified", "partial", "blocked", "unknown"]).has(receipt.decision)) throw new Error("quality envelope decision is unknown");
  if (!receipt.study_order.length || JSON.stringify([...new Set(receipt.study_order)].sort()) !== JSON.stringify(receipt.study_order) || receipt.verdicts.length !== receipt.study_order.length) throw new Error("quality envelope study ordering is invalid");
  receipt.verdicts.forEach((verdict, index) => { if (verdict.study_id !== receipt.study_order[index] || typeof verdict.modality !== "string" || !verdict.modality.trim() || !new Set(["pass", "pass_with_warnings", "blocked", "unknown"]).has(verdict.quality_disposition) || typeof verdict.comparable !== "boolean" || !Array.isArray(verdict.reasons) || !verdict.reasons.length) throw new Error("quality envelope study verdict linkage is invalid"); });
  for (const [modality, count] of Object.entries(receipt.modality_coverage)) if (!modality.trim() || !Number.isInteger(count) || count < 0) throw new Error("quality envelope modality coverage is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality envelope artifact digest is invalid");
}

export function qualityEnvelopeReceiptDigest(receipt: QualityEnvelopeReceipt): string { validateQualityEnvelopeReceipt(receipt); return digestJsonSync(receipt); }

export interface ExperimentDesignReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  objective_id: string;
  decision: "admitted" | "partial" | "blocked";
  site_order: string[];
  assignments: readonly Record<string, unknown>[];
  modality_coverage: Record<string, number>;
  omitted_modalities: string[];
  comparability_conflicts: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateExperimentDesignReceipt(receipt: ExperimentDesignReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EXPERIMENT_DESIGN_CONTROL_FEATURE_ID || receipt.contract_version !== EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION) throw new Error("experiment design schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.objective_id.trim() || !receipt.reasons.length) throw new Error("experiment design identity, locality, or reasons are incomplete");
  if (!new Set(["admitted", "partial", "blocked"]).has(receipt.decision)) throw new Error("experiment design decision is unknown");
  if (!receipt.site_order.length || JSON.stringify([...new Set(receipt.site_order)].sort()) !== JSON.stringify(receipt.site_order)) throw new Error("experiment design site ordering is invalid");
  if (receipt.decision === "blocked" && receipt.assignments.length) throw new Error("blocked experiment design cannot contain assignments");
  for (const assignment of receipt.assignments) if (typeof assignment.site_id !== "string" || !assignment.site_id.trim() || typeof assignment.modality !== "string" || !assignment.modality.trim() || typeof assignment.instrument_profile !== "string" || !assignment.instrument_profile.trim() || assignment.authorized !== true || typeof assignment.budget !== "number" || !Number.isFinite(assignment.budget)) throw new Error("experiment design assignment is invalid");
  for (const [modality, count] of Object.entries(receipt.modality_coverage)) if (!modality.trim() || !Number.isInteger(count) || count < 0) throw new Error("experiment design modality coverage is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("experiment design artifact digest is invalid");
}

export function experimentDesignReceiptDigest(receipt: ExperimentDesignReceipt): string { validateExperimentDesignReceipt(receipt); return digestJsonSync(receipt); }

export interface ProtocolSimulationReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  protocol_id: string;
  design_digest: string;
  results: readonly Record<string, unknown>[];
  passed: number;
  failed_closed: number;
  approval_required: number;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateProtocolSimulationReceipt(receipt: ProtocolSimulationReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== PROTOCOL_SIMULATION_FEATURE_ID || receipt.contract_version !== PROTOCOL_SIMULATION_CONTRACT_VERSION) throw new Error("protocol simulation schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.protocol_id.trim() || !/^[0-9a-f]{64}$/.test(receipt.design_digest) || !receipt.results.length) throw new Error("protocol simulation identity, digest, locality, or results are incomplete");
  if (receipt.passed + receipt.failed_closed + receipt.approval_required !== receipt.results.length) throw new Error("protocol simulation state counts do not match results");
  const ids = receipt.results.map((result) => String(result.scenario_id ?? ""));
  if (ids.some((id) => !id.trim()) || JSON.stringify(ids) !== JSON.stringify([...new Set(ids)].sort())) throw new Error("protocol simulation scenario ordering is invalid");
  for (const result of receipt.results) if (!new Set(["passed", "failed_closed", "approval_required"]).has(result.state) || !Array.isArray(result.reasons) || !result.reasons.length) throw new Error("protocol simulation scenario result is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol simulation artifact digest is invalid");
}

export function protocolSimulationReceiptDigest(receipt: ProtocolSimulationReceipt): string { validateProtocolSimulationReceipt(receipt); return digestJsonSync(receipt); }

export interface InstrumentMeshReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  federation_id: string;
  action_id: string;
  decision: "admitted" | "approval_required" | "blocked" | "unknown";
  candidate_order: string[];
  selected_instrument_id: string | null;
  selected_site_id: string | null;
  selected_protocol_profile: string | null;
  satisfied_capabilities: string[];
  missing_capabilities: string[];
  missing_interlocks: string[];
  effect: Record<string, unknown> | null;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInstrumentMeshReceipt(receipt: InstrumentMeshReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INSTRUMENT_MESH_FEATURE_ID || receipt.contract_version !== INSTRUMENT_MESH_CONTRACT_VERSION) throw new Error("instrument mesh schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.action_id.trim() || !receipt.reasons.length) throw new Error("instrument mesh identity, locality, boundary, or reasons are incomplete");
  if (!new Set(["admitted", "approval_required", "blocked", "unknown"]).has(receipt.decision)) throw new Error("instrument mesh decision is unknown");
  if (JSON.stringify([...new Set(receipt.candidate_order)].sort()) !== JSON.stringify(receipt.candidate_order)) throw new Error("instrument mesh candidate ordering is invalid");
  if (receipt.missing_capabilities.some((item) => !item.trim()) || receipt.missing_interlocks.some((item) => !item.trim())) throw new Error("instrument mesh missing capability or interlock is empty");
  if (receipt.decision === "admitted") {
    if (!receipt.selected_instrument_id || !receipt.selected_site_id || !receipt.effect) throw new Error("admitted instrument mesh receipt needs selection and effect receipt");
    if (receipt.effect.authorized !== true || receipt.effect.executed !== false || receipt.effect.raw_data_local !== true) throw new Error("instrument mesh effect must be authorized, not executed, and local");
  } else if (receipt.effect !== null) throw new Error("non-admitted instrument mesh receipt cannot contain an effect");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("instrument mesh artifact digest is invalid");
}

export function instrumentMeshReceiptDigest(receipt: InstrumentMeshReceipt): string { validateInstrumentMeshReceipt(receipt); return digestJsonSync(receipt); }

export interface ComputationalExecutionReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  run_id: string;
  decision: "dry_run" | "admitted" | "approval_required" | "blocked";
  ordered_nodes: string[];
  admitted_nodes: string[];
  run: Record<string, unknown>;
  run_digest: string;
  authorized_effects: readonly Record<string, unknown>[];
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  effects_executed: boolean;
  raw_data_local: boolean;
  boundary: string;
}

export function validateComputationalExecutionReceipt(receipt: ComputationalExecutionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EXECUTION_CONTROL_FEATURE_ID || receipt.contract_version !== EXECUTION_CONTROL_CONTRACT_VERSION) throw new Error("computational execution schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || receipt.effects_executed || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.run_id.trim() || !receipt.ordered_nodes.length || !receipt.reasons.length) throw new Error("computational execution identity, locality, non-execution, graph, or reasons are incomplete");
  if (!new Set(["dry_run", "admitted", "approval_required", "blocked"]).has(receipt.decision)) throw new Error("computational execution decision is unknown");
  if (new Set(receipt.ordered_nodes).size !== receipt.ordered_nodes.length || new Set(receipt.admitted_nodes).size !== receipt.admitted_nodes.length || receipt.admitted_nodes.some((node) => !receipt.ordered_nodes.includes(node))) throw new Error("computational execution node identities are invalid");
  if (receipt.run.workflow_id !== receipt.workflow_id || receipt.run.status !== "planned") throw new Error("execution run linkage or planned status is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.run_digest)) throw new Error("computational execution run digest is invalid");
  if (receipt.decision === "admitted" && receipt.authorized_effects.length !== receipt.admitted_nodes.length) throw new Error("every admitted node needs an authorized effect");
  if (receipt.decision !== "admitted" && receipt.authorized_effects.length) throw new Error("non-admitted execution cannot contain effects");
  for (const effect of receipt.authorized_effects) if (effect.effect !== "execute_local_computation" || effect.authorized !== true || effect.executed !== false || typeof effect.payload_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.payload_digest)) throw new Error("computational execution effect receipt is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("computational execution artifact digest is invalid");
}

export function computationalExecutionReceiptDigest(receipt: ComputationalExecutionReceipt): string { validateComputationalExecutionReceipt(receipt); return digestJsonSync(receipt); }

export interface AnalysisPortfolioReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  question_id: string;
  estimand: string;
  verdict: "qualified" | "conditional" | "blocked";
  selected_candidate: string | null;
  candidate_order: string[];
  uncertainty: string[];
  omissions: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateAnalysisPortfolioReceipt(receipt: AnalysisPortfolioReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ANALYSIS_PORTFOLIO_FEATURE_ID || receipt.contract_version !== ANALYSIS_PORTFOLIO_CONTRACT_VERSION) throw new Error("analysis portfolio schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.question_id.trim() || !receipt.estimand.trim() || !receipt.candidate_order.length || !receipt.reasons.length) throw new Error("analysis portfolio identity, candidates, locality, boundary, or reasons are incomplete");
  if (!new Set(["qualified", "conditional", "blocked"]).has(receipt.verdict)) throw new Error("analysis portfolio verdict is unknown");
  if (JSON.stringify([...new Set(receipt.candidate_order)].sort()) !== JSON.stringify(receipt.candidate_order)) throw new Error("analysis portfolio candidate ordering is invalid");
  if (receipt.verdict === "qualified" && !receipt.selected_candidate) throw new Error("qualified analysis portfolio needs a selected candidate");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("analysis portfolio artifact digest is invalid");
}

export function analysisPortfolioReceiptDigest(receipt: AnalysisPortfolioReceipt): string { validateAnalysisPortfolioReceipt(receipt); return digestJsonSync(receipt); }

export interface InterpretationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  result_id: string;
  verdict: "qualified" | "conditional" | "blocked";
  claim_order: string[];
  covered_modalities: string[];
  omitted_modalities: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInterpretationAssuranceReceipt(receipt: InterpretationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INTERPRETATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== INTERPRETATION_ASSURANCE_CONTRACT_VERSION) throw new Error("interpretation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.result_id.trim() || !receipt.claim_order.length || !receipt.reasons.length) throw new Error("interpretation assurance identity, claims, locality, boundary, or reasons are incomplete");
  if (!new Set(["qualified", "conditional", "blocked"]).has(receipt.verdict)) throw new Error("interpretation assurance verdict is unknown");
  if (JSON.stringify([...new Set(receipt.claim_order)].sort()) !== JSON.stringify(receipt.claim_order)) throw new Error("interpretation assurance claim ordering is invalid");
  if (receipt.verdict === "qualified" && receipt.omitted_modalities.length) throw new Error("qualified interpretation cannot omit required modalities");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("interpretation assurance artifact digest is invalid");
}

export function interpretationAssuranceReceiptDigest(receipt: InterpretationAssuranceReceipt): string { validateInterpretationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ReplicationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  claim_id: string;
  protocol_digest: string;
  verdict: "replicated" | "partially_replicated" | "contradicted" | "null_result" | "insufficient_evidence" | "blocked";
  observation_order: string[];
  independent_site_order: string[];
  positive_count: number;
  null_count: number;
  negative_count: number;
  inconclusive_count: number;
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateReplicationAssuranceReceipt(receipt: ReplicationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== REPLICATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== REPLICATION_ASSURANCE_CONTRACT_VERSION) throw new Error("replication assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.claim_id.trim() || !receipt.observation_order.length || !receipt.reasons.length) throw new Error("replication assurance identity, observations, locality, boundary, or reasons are incomplete");
  if (!new Set(["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence", "blocked"]).has(receipt.verdict)) throw new Error("replication assurance verdict is unknown");
  if (JSON.stringify([...new Set(receipt.observation_order)].sort()) !== JSON.stringify(receipt.observation_order) || JSON.stringify([...new Set(receipt.independent_site_order)].sort()) !== JSON.stringify(receipt.independent_site_order)) throw new Error("replication assurance ordering is invalid");
  if (![receipt.positive_count, receipt.null_count, receipt.negative_count, receipt.inconclusive_count].every((value) => Number.isInteger(value) && value >= 0) || receipt.positive_count + receipt.null_count + receipt.negative_count + receipt.inconclusive_count !== receipt.observation_order.length) throw new Error("replication assurance counts do not match observations");
  if (!/^[0-9a-f]{64}$/.test(receipt.protocol_digest)) throw new Error("replication assurance protocol digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("replication assurance artifact digest is invalid");
}

export function replicationAssuranceReceiptDigest(receipt: ReplicationAssuranceReceipt): string { validateReplicationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ReleaseAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  run_id: string;
  release_id: string;
  verdict: "released" | "conditional" | "incomplete" | "incomparable" | "blocked";
  study_order: string[];
  modality_order: string[];
  artifact_order: string[];
  evidence_receipt_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  policy_decision: "allow" | "deny" | "redact" | "local_only" | "approval_required" | "unresolved";
  effect_receipt: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateReleaseAssuranceReceipt(receipt: ReleaseAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RELEASE_ASSURANCE_FEATURE_ID || receipt.contract_version !== RELEASE_ASSURANCE_CONTRACT_VERSION) throw new Error("release assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.run_id.trim() || !receipt.release_id.trim() || !receipt.study_order.length || !receipt.evidence_receipt_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("release assurance identity, studies, evidence, locality, boundary, or effects are incomplete");
  if (!new Set(["released", "conditional", "incomplete", "incomparable", "blocked"]).has(receipt.verdict)) throw new Error("release assurance verdict is unknown");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.artifact_order, receipt.evidence_receipt_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("release assurance ordering is invalid");
  if (!new Set(["allow", "deny", "redact", "local_only", "approval_required", "unresolved"]).has(receipt.policy_decision)) throw new Error("release assurance policy decision is unknown");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("release assurance artifact digest is invalid");
}

export function releaseAssuranceReceiptDigest(receipt: ReleaseAssuranceReceipt): string { validateReleaseAssuranceReceipt(receipt); return digestJsonSync(receipt); }

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
