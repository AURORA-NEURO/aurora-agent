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
  RESEARCH_RELEASE_FEATURE_ID,
  researchReleaseReceiptDigest,
  validateResearchReleaseReceipt,
  INSTRUMENT_PREFLIGHT_FEATURE_ID,
  instrumentPreflightReceiptDigest,
  validateInstrumentPreflightReceipt,
  MULTIMODAL_HARMONIZATION_FEATURE_ID,
  harmonizedResearchObjectDigest,
  validateHarmonizedResearchObject,
  ANALYSIS_QUALIFICATION_FEATURE_ID,
  qualifiedAnalysisResultDigest,
  validateQualifiedAnalysisResult,
  PROTOCOL_MATRIX_FEATURE_ID,
  protocolMatrixReceiptDigest,
  validateProtocolMatrixReceipt,
  MULTIMODAL_REPLICATION_FEATURE_ID,
  multimodalReplicationReportDigest,
  validateMultimodalReplicationReport,
  QUALITY_DRIFT_FEATURE_ID,
  qualityDriftReceiptDigest,
  validateQualityDriftReceipt,
  DESIGN_FRONTIER_FEATURE_ID,
  designFrontierReceiptDigest,
  validateDesignFrontierReceipt,
  AUTONOMY_BATCH_FEATURE_ID,
  batchAdmissionReceiptDigest,
  validateBatchAdmissionReceipt,
  WORKFLOW_BATCH_FEATURE_ID,
  workflowBatchReceiptDigest,
  validateWorkflowBatchReceipt,
  RESEARCH_RELEASE_BATCH_FEATURE_ID,
  researchReleaseBatchReceiptDigest,
  validateResearchReleaseBatchReceipt,
  FEDERATED_EVALUATION_FEATURE_ID,
  federatedEvaluationReceiptDigest,
  validateFederatedEvaluationReceipt,
  RESOURCE_WORKBENCH_FEATURE_ID,
  qualifiedResourceSetDigest,
  validateQualifiedResourceSet,
  RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
  RESOURCE_DISCOVERY_CONTRACT_VERSION,
  resourceDiscoveryContractReceiptDigest,
  validateResourceDiscoveryContractReceipt,
  GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID,
  GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION,
  signedResearchObjectReceiptDigest,
  validateSignedResearchObjectReceipt,
  RELEASE_HARNESS_FEATURE_ID,
  RELEASE_HARNESS_CONTRACT_VERSION,
  releaseHarnessReceiptDigest,
  validateReleaseHarnessReceipt,
  PROTOCOL_ASSURANCE_FEATURE_ID,
  PROTOCOL_ASSURANCE_CONTRACT_VERSION,
  protocolAssuranceReceiptDigest,
  validateProtocolAssuranceReceipt,
  FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID,
  FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION,
  federatedMultimodalAssuranceReceiptDigest,
  validateFederatedMultimodalAssuranceReceipt,
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

test("research-release receipt preserves localization and provenance", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESEARCH_RELEASE_FEATURE_ID,
    release_id: "release-1",
    research_object: {
      release_id: "release-1",
      artifact_ids: ["artifact:one"],
      evidence_receipt_ids: ["evidence:one"],
      boundary: PRECLINICAL_BOUNDARY,
      federation: {
        envelope: {
          raw_data_local: true,
          signature: "ed25519:key:signature",
          localization_statement: "raw data remains local",
          export: { content_hash: "c".repeat(64), provenance: [{ source_id: "artifact:one" }] },
        },
      },
    },
    release_digest: "a".repeat(64),
    omissions: ["evidence:one:missing control"],
    reasons: ["omission retained"],
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateResearchReleaseReceipt(receipt));
  assert.equal(researchReleaseReceiptDigest(receipt), researchReleaseReceiptDigest(receipt));
});

test("instrument preflight receipt preserves no-hardware boundary", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: INSTRUMENT_PREFLIGHT_FEATURE_ID,
    run_id: "run:instrument-1",
    study_id: "study:organoid-1",
    decision: "ready",
    ordered_actions: ["action-1"],
    action_digests: { "action-1": "a".repeat(64) },
    remaining_budget: { minutes: 2 },
    omissions: [],
    reasons: ["checks passed; no hardware effect performed"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateInstrumentPreflightReceipt(receipt));
  assert.equal(instrumentPreflightReceiptDigest(receipt), instrumentPreflightReceiptDigest(receipt));
});

test("harmonized research object preserves local multimodal limits", () => {
  const object = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: MULTIMODAL_HARMONIZATION_FEATURE_ID,
    study_id: "study:organoid-1",
    reference_schema: "aurora-multimodal/1",
    decision: "partial",
    modality_order: ["image", "rna"],
    alignment: { image: ["a", "z"], rna: ["a", "z"] },
    omitted_modalities: ["proteomics"],
    semantic_loss: [{ field: "image.qc", reason: "not supplied" }],
    reasons: ["required modality omitted"],
    artifact: { content_hash: "d".repeat(64) },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateHarmonizedResearchObject(object));
  assert.equal(harmonizedResearchObjectDigest(object), harmonizedResearchObjectDigest(object));
});

test("qualified analysis result preserves omission-aware qualification", () => {
  const result = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: ANALYSIS_QUALIFICATION_FEATURE_ID,
    question_id: "question:effect",
    estimand: "average treatment effect in organoid model",
    verdict: "conditional",
    selected_candidate: "candidate-a",
    candidate_order: ["candidate-a"],
    uncertainty: ["candidate-a: interval is bounded"],
    omissions: ["missing independent site"],
    negative_evidence: ["candidate-a: null replication pending"],
    reasons: ["protected omissions prevent unconditional qualification"],
    artifact: { content_hash: "e".repeat(64) },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateQualifiedAnalysisResult(result));
  assert.equal(qualifiedAnalysisResultDigest(result), qualifiedAnalysisResultDigest(result));
});

test("protocol matrix receipt partitions statuses and preserves digest", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: PROTOCOL_MATRIX_FEATURE_ID,
    protocol_id: "protocol:matrix-1",
    total_cells: 2,
    passed_cells: 1,
    failed_closed_cells: 1,
    approval_cells: 0,
    cells: [
      { cell_id: "matrix-cell-0000", status: "passed", reasons: ["simulation passed"] },
      { cell_id: "matrix-cell-0001", status: "failed_closed", reasons: ["budget exhausted"] },
    ],
    artifact: { content_hash: "f".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateProtocolMatrixReceipt(receipt));
  assert.equal(protocolMatrixReceiptDigest(receipt), protocolMatrixReceiptDigest(receipt));
});

test("multimodal replication report preserves comparability omissions", () => {
  const report = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: MULTIMODAL_REPLICATION_FEATURE_ID,
    capability_id: "capability:multimodal-replication",
    claim: "organoid mechanism reproduces across sites",
    request_digest: "b".repeat(64),
    required_modalities: ["image", "rna"],
    summary: { disposition: "partially_replicated", total_observations: 2, reasons: ["one study omitted rna"] },
    studies: [
      { study_id: "study-a", site: "site-a", reasons: [], comparable: true },
      { study_id: "study-b", site: "site-b", reasons: ["required modalities omitted: rna"], comparable: false },
    ],
    artifact: { content_hash: "a".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateMultimodalReplicationReport(report));
  assert.equal(multimodalReplicationReportDigest(report), multimodalReplicationReportDigest(report));
});

test("quality drift receipt keeps unknown metric and baseline digest", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: QUALITY_DRIFT_FEATURE_ID,
    dataset_id: "dataset:drift",
    modality: "image",
    request_digest: "a".repeat(64),
    summary: { disposition: "unknown", stable: 1, drifted: 0, unknown: 1, reasons: ["metric snr is unmeasured"] },
    metrics: [
      { metric_id: "focus", status: "stable", delta: 0.01, reasons: [] },
      { metric_id: "snr", status: "unknown", delta: null, reasons: ["metric snr is unmeasured"] },
    ],
    artifact: { content_hash: "b".repeat(64) },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateQualityDriftReceipt(receipt));
  assert.equal(qualityDriftReceiptDigest(receipt), qualityDriftReceiptDigest(receipt));
});

test("design frontier receipt retains blocked scenario", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: DESIGN_FRONTIER_FEATURE_ID,
    study_id: "study:frontier",
    feasible_scenarios: 1,
    blocked_scenarios: 1,
    scenarios: [
      { scenario_id: "nominal", disposition: "feasible", reasons: ["compiled"] },
      { scenario_id: "underpowered", disposition: "blocked", reasons: ["resource limit"] },
    ],
    artifact: { content_hash: "c".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateDesignFrontierReceipt(receipt));
  assert.equal(designFrontierReceiptDigest(receipt), designFrontierReceiptDigest(receipt));
});

test("autonomy batch receipt retains denied action", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: AUTONOMY_BATCH_FEATURE_ID,
    actor: "agent:batch",
    total_actions: 3,
    allowed_actions: 1,
    approval_actions: 1,
    denied_actions: 1,
    actions: [
      { action_id: "a", decision: "allowed", reasons: ["grant admits action"] },
      { action_id: "b", decision: "approval_required", reasons: ["signed preflight required"] },
      { action_id: "c", decision: "denied", reasons: ["unknown evidence"] },
    ],
    artifact: { content_hash: "d".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateBatchAdmissionReceipt(receipt));
  assert.equal(batchAdmissionReceiptDigest(receipt), batchAdmissionReceiptDigest(receipt));
});

test("workflow batch receipt retains blocked run", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: WORKFLOW_BATCH_FEATURE_ID,
    total_workflows: 2,
    succeeded_workflows: 1,
    dry_run_workflows: 0,
    blocked_workflows: 1,
    entries: [
      { workflow_id: "workflow:a", disposition: "succeeded", reasons: ["completed"], ordered_nodes: ["a"] },
      { workflow_id: "workflow:b", disposition: "blocked", reasons: ["budget exceeded"], ordered_nodes: [] },
    ],
    artifact: { content_hash: "e".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateWorkflowBatchReceipt(receipt));
  assert.equal(workflowBatchReceiptDigest(receipt), workflowBatchReceiptDigest(receipt));
});

test("research-release batch receipt retains blocked release", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESEARCH_RELEASE_BATCH_FEATURE_ID,
    total_releases: 2,
    published_releases: 1,
    blocked_releases: 1,
    entries: [
      { release_id: "release:a", disposition: "published", release_digest: "f".repeat(64), reasons: ["signed"] },
      { release_id: "release:b", disposition: "blocked", release_digest: null, reasons: ["policy denied"] },
    ],
    artifact: { content_hash: "a".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateResearchReleaseBatchReceipt(receipt));
  assert.equal(researchReleaseBatchReceiptDigest(receipt), researchReleaseBatchReceiptDigest(receipt));
});

test("federated evaluation receipt preserves contradiction", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_EVALUATION_FEATURE_ID,
    capability_id: "capability:mechanism",
    benchmark_world: "world:preclinical",
    minimum_sites: 2,
    total_sites: 3,
    agreeing_sites: 2,
    contradictory_sites: 1,
    blocked_sites: 0,
    disposition: "contradicted",
    entries: [
      { site_id: "site:a", disposition: "accepted", card_digest: "a".repeat(64), reasons: ["matches consensus"] },
      { site_id: "site:b", disposition: "accepted", card_digest: "a".repeat(64), reasons: ["matches consensus"] },
      { site_id: "site:c", disposition: "contradictory", card_digest: "b".repeat(64), reasons: ["digest differs"] },
    ],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedEvaluationReceipt(receipt));
  assert.equal(federatedEvaluationReceiptDigest(receipt), federatedEvaluationReceiptDigest(receipt));
});

test("resource workbench receipt preserves protected omission", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESOURCE_WORKBENCH_FEATURE_ID,
    need_id: "need:organoid",
    requester: "researcher:alice",
    disposition: "blocked",
    considered_candidates: 1,
    qualified_count: 0,
    resources: [],
    omissions: [{ resource_id: "resource:protected", reason: "raw research data is not institution-local" }],
    reasons: ["no candidate satisfied the typed resource need; omissions remain explicit"],
    artifact: { content_hash: "c".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateQualifiedResourceSet(receipt));
  assert.equal(qualifiedResourceSetDigest(receipt), qualifiedResourceSetDigest(receipt));
});

test("resource discovery contract receipt preserves migration notes", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
    contract_version: RESOURCE_DISCOVERY_CONTRACT_VERSION,
    request_id: "request:resource-v2",
    requested_by: "admin:consortium",
    compatibility_profile: "qualified-resource-set/v1",
    result: { feature_id: RESOURCE_WORKBENCH_FEATURE_ID, boundary: PRECLINICAL_BOUNDARY },
    migration_notes: ["v1 semantic fields remain stable"],
    artifact: { content_hash: "d".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateResourceDiscoveryContractReceipt(receipt));
  assert.equal(resourceDiscoveryContractReceiptDigest(receipt), resourceDiscoveryContractReceiptDigest(receipt));
});

test("signed research object receipt preserves locality and migration", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID,
    contract_version: GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION,
    run_id: "run:1",
    release_id: "release:1",
    origin: "site-a",
    purpose: "federated preclinical reproduction",
    artifact_ids: ["artifact:a"],
    evidence_receipt_ids: ["evidence:a"],
    release_digest: "a".repeat(64),
    signer_public_key_hex: "b".repeat(64),
    signer_signature_hex: "c".repeat(128),
    migration_notes: ["migrated from v1"],
    omissions: ["protected:raw-bytes"],
    raw_data_local: true,
    artifact: { content_hash: "d".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateSignedResearchObjectReceipt(receipt));
  assert.equal(signedResearchObjectReceiptDigest(receipt), signedResearchObjectReceiptDigest(receipt));
});

test("release harness keeps unknown replay gate", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RELEASE_HARNESS_FEATURE_ID,
    contract_version: RELEASE_HARNESS_CONTRACT_VERSION,
    request_id: "request:harness",
    object_digest: "a".repeat(64),
    disposition: "unknown",
    checks: [{ check_id: "replay-identity", disposition: "unknown", reason: "replay identity is unmeasured" }],
    omissions: ["replay identity is unmeasured"],
    reasons: ["an unmeasured release assurance gate prevents a pass"],
    artifact: { content_hash: "e".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateReleaseHarnessReceipt(receipt));
  assert.equal(releaseHarnessReceiptDigest(receipt), releaseHarnessReceiptDigest(receipt));
});

test("protocol assurance keeps unknown simulation cells", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: PROTOCOL_ASSURANCE_FEATURE_ID,
    contract_version: PROTOCOL_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:protocol",
    protocol_id: "protocol:organoid",
    disposition: "unknown",
    total_cells: 2,
    passed_cells: 1,
    blocked_cells: 0,
    unknown_cells: 1,
    checks: ["unknown simulation cells prevent a pass"],
    omissions: ["unknown simulation cells remain unmeasured"],
    simulation_digest: "a".repeat(64),
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateProtocolAssuranceReceipt(receipt));
  assert.equal(protocolAssuranceReceiptDigest(receipt), protocolAssuranceReceiptDigest(receipt));
});

test("federated multimodal assurance keeps locality and unknown state", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID,
    contract_version: FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:federated",
    federation_id: "federation:preclinical",
    benchmark_id: "benchmark:multimodal",
    institution_ids: ["site:a", "site:b"],
    disposition: "unknown",
    harmonized_digest: "a".repeat(64),
    checks: ["partial harmonization remains unknown rather than comparable"],
    omissions: ["modality semantic loss remains bounded and must be reported"],
    artifact: { content_hash: "b".repeat(64) },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedMultimodalAssuranceReceipt(receipt));
  assert.equal(federatedMultimodalAssuranceReceiptDigest(receipt), federatedMultimodalAssuranceReceiptDigest(receipt));
});
