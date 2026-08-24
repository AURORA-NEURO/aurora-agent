import test from "node:test";
import assert from "node:assert/strict";
import {
  PRECLINICAL_BOUNDARY,
  validateEvidenceReceipt,
  validatePolicyReceipt,
  researchArtifactDigest,
  RELEASE_REVIEW_FEATURE_ID,
  releaseReviewDigest,
  validateReleaseReview,
  RESEARCH_INGESTION_FEATURE_ID,
  researchIngestionBundleDigest,
  validateResearchIngestionBundle,
  EXPERIMENT_DESIGN_FEATURE_ID,
  experimentDesignPlanDigest,
  validateExperimentDesignPlan,
  PROTOCOL_SIMULATION_FEATURE_ID,
  protocolSimulationReportDigest,
  validateProtocolSimulationReport,
  REPLICATION_FEATURE_ID,
  replicationReportDigest,
  validateReplicationReport,
  QUALITY_CONTROL_FEATURE_ID,
  qualityControlReceiptDigest,
  validateQualityControlReceipt,
  RESEARCH_CONTEXT_FEATURE_ID,
  researchContextReceiptDigest,
  validateResearchContextReceipt,
  REPLAY_AUDIT_FEATURE_ID,
  replayAuditReceiptDigest,
  validateReplayAuditReceipt,
  WORKFLOW_EXECUTION_FEATURE_ID,
  workflowExecutionReceiptDigest,
  validateWorkflowExecutionReceipt,
  EVALUATION_OBSERVABILITY_FEATURE_ID,
  evaluationCardReceiptDigest,
  validateEvaluationCardReceipt,
} from "../dist/index.js";

test("empty evidence is explicit unknown", () => {
  assert.doesNotThrow(() => validateEvidenceReceipt({
    schema_version: "aurora-research-contract/1.0",
    receipt_id: "evidence:q1",
    intent: "retrieve",
    sources: [],
    derivation: ["feature:AFA-bioir-P02-F01"],
    uncertainty: [{ kind: "epistemic", statement: "no evidence" }],
    omissions: [{ item: "query:q1", reason: "empty", could_change_decision: "unknown" }],
    competing_explanations: [],
    negative_evidence: [],
    conclusion_state: "unknown",
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("unresolved policy cannot allow", () => {
  assert.throws(() => validatePolicyReceipt({
    schema_version: "aurora-research-contract/1.0",
    receipt_id: "policy:q1",
    decision: "allow",
    reasons: ["unresolved"],
    evaluated_artifacts: [],
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("artifact digest is key-order stable", () => {
  assert.equal(researchArtifactDigest({ b: 2, a: 1 }), researchArtifactDigest({ a: 1, b: 2 }));
});

test("a passing release review requires provenance", () => {
  assert.throws(() => validateReleaseReview({
    schema_version: "aurora-research-contract/1.0",
    feature_id: RELEASE_REVIEW_FEATURE_ID,
    capability_id: "capability:demo",
    card_digest: "a".repeat(64),
    verdict: "pass",
    reasons: ["all gates passed"],
    replications: [],
    checks: [],
    provenance_complete: false,
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("release review digest is deterministic", () => {
  const review = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RELEASE_REVIEW_FEATURE_ID,
    capability_id: "capability:demo",
    card_digest: "a".repeat(64),
    verdict: "blocked",
    reasons: ["replication floor unmet"],
    replications: [],
    checks: [],
    provenance_complete: false,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.equal(releaseReviewDigest(review), releaseReviewDigest(review));
});

test("research ingestion bundle keeps raw data local", () => {
  const bundle = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESEARCH_INGESTION_FEATURE_ID,
    source_id: "study-a",
    adapter: "tabular",
    adapter_version: "0.1.0",
    source_digest: "a".repeat(64),
    ingestion_digest: "b".repeat(64),
    artifact: { content_hash: "b".repeat(64) },
    conformance: { verified: true },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateResearchIngestionBundle(bundle));
  assert.equal(researchIngestionBundleDigest(bundle), researchIngestionBundleDigest(bundle));
});

test("experiment design plan preserves allocation total", () => {
  const plan = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: EXPERIMENT_DESIGN_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      allocations: [{ arm_id: "control", units: 4 }, { arm_id: "treatment", units: 4 }],
      total_units: 8,
    },
    artifact: { content_hash: "c".repeat(64) },
  };
  assert.doesNotThrow(() => validateExperimentDesignPlan(plan));
  assert.equal(experimentDesignPlanDigest(plan), experimentDesignPlanDigest(plan));
});

test("protocol simulation report preserves fail-closed statuses", () => {
  const report = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: PROTOCOL_SIMULATION_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      results: [{ scenario_id: "partition", status: "requires_approval" }],
    },
    artifact: { content_hash: "d".repeat(64) },
  };
  assert.doesNotThrow(() => validateProtocolSimulationReport(report));
  assert.equal(protocolSimulationReportDigest(report), protocolSimulationReportDigest(report));
});

test("replication report preserves null-result disposition", () => {
  const report = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: REPLICATION_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      summary: { disposition: "null_result", total_observations: 2, reasons: ["null retained"] },
    },
    artifact: { content_hash: "e".repeat(64) },
  };
  assert.doesNotThrow(() => validateReplicationReport(report));
  assert.equal(replicationReportDigest(report), replicationReportDigest(report));
});

test("quality-control receipt preserves unknown and local-only gates", () => {
  const receipt = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: QUALITY_CONTROL_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      raw_data_local: true,
      summary: { disposition: "unknown", reasons: ["metric unmeasured"] },
    },
    artifact: { content_hash: "f".repeat(64) },
  };
  assert.doesNotThrow(() => validateQualityControlReceipt(receipt));
  assert.equal(qualityControlReceiptDigest(receipt), qualityControlReceiptDigest(receipt));
});

test("research-context receipt preserves closure and omission state", () => {
  const receipt = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: RESEARCH_CONTEXT_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      protected_closure_satisfied: true,
      supports_sufficiency_claim: false,
      unresolved_obligations: 2,
      section_digest: "a".repeat(64),
      certificate_digest: "b".repeat(64),
    },
    artifact: { content_hash: "c".repeat(64) },
  };
  assert.doesNotThrow(() => validateResearchContextReceipt(receipt));
  assert.equal(researchContextReceiptDigest(receipt), researchContextReceiptDigest(receipt));
});

test("replay-audit receipt preserves divergence status", () => {
  const receipt = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: REPLAY_AUDIT_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      status: "diverged",
      baseline_digest: "a".repeat(64),
      candidate_digest: "b".repeat(64),
      first_difference: "run.events",
      reasons: ["first observable replay divergence: run.events"],
    },
    artifact: { content_hash: "c".repeat(64) },
  };
  assert.doesNotThrow(() => validateReplayAuditReceipt(receipt));
  assert.equal(replayAuditReceiptDigest(receipt), replayAuditReceiptDigest(receipt));
});

test("workflow-execution receipt preserves deterministic dry-run order", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: WORKFLOW_EXECUTION_FEATURE_ID,
    workflow_id: "workflow:demo",
    mode: "dry_run",
    status: "dry_run",
    ordered_nodes: ["a", "b"],
    completed_nodes: [],
    run: { workflow_id: "workflow:demo", status: "planned" },
    run_digest: "a".repeat(64),
    remaining_budget: { cpu_seconds: 4 },
    artifact: { content_hash: "b".repeat(64) },
    reasons: ["preflight passed"],
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateWorkflowExecutionReceipt(receipt));
  assert.equal(workflowExecutionReceiptDigest(receipt), workflowExecutionReceiptDigest(receipt));
});

test("evaluation-card receipt keeps baseline omissions explicit", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: EVALUATION_OBSERVABILITY_FEATURE_ID,
    card: {
      schema_version: "aurora-research-contract/1.0",
      capability_id: "capability:demo",
      benchmark_world: "synthetic-v1",
      baselines: ["fixed"],
      metrics: [{ name: "auditable_discovery_rate", value: "0.4", uncertainty: "95%" }],
      uncertainty: [{ kind: "sampling", statement: "small sample" }],
      limitations: ["synthetic only"],
      release_verdict: "blocked",
      boundary: PRECLINICAL_BOUNDARY,
    },
    card_digest: "a".repeat(64),
    observations_digest: "b".repeat(64),
    baseline_counts: { fixed: 0 },
    omissions: ["baseline fixed is under-sampled"],
    reasons: ["baseline coverage is incomplete"],
    artifact: { content_hash: "c".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateEvaluationCardReceipt(receipt));
  assert.equal(evaluationCardReceiptDigest(receipt), evaluationCardReceiptDigest(receipt));
});
