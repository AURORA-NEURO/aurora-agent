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
  INGESTION_GATEWAY_FEATURE_ID,
  INGESTION_GATEWAY_CONTRACT_VERSION,
  ingestionGatewayReceiptDigest,
  validateIngestionGatewayReceipt,
  QUALITY_ENVELOPE_FEATURE_ID,
  QUALITY_ENVELOPE_CONTRACT_VERSION,
  qualityEnvelopeReceiptDigest,
  validateQualityEnvelopeReceipt,
  EXPERIMENT_DESIGN_CONTROL_FEATURE_ID,
  EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION,
  experimentDesignReceiptDigest,
  validateExperimentDesignReceipt,
  PROTOCOL_SIMULATION_FEATURE_ID,
  PROTOCOL_SIMULATION_CONTRACT_VERSION,
  protocolSimulationReceiptDigest,
  validateProtocolSimulationReceipt,
  INSTRUMENT_MESH_FEATURE_ID,
  INSTRUMENT_MESH_CONTRACT_VERSION,
  instrumentMeshReceiptDigest,
  validateInstrumentMeshReceipt,
  EXECUTION_CONTROL_FEATURE_ID,
  EXECUTION_CONTROL_CONTRACT_VERSION,
  computationalExecutionReceiptDigest,
  validateComputationalExecutionReceipt,
  ANALYSIS_PORTFOLIO_FEATURE_ID,
  ANALYSIS_PORTFOLIO_CONTRACT_VERSION,
  analysisPortfolioReceiptDigest,
  validateAnalysisPortfolioReceipt,
  INTERPRETATION_ASSURANCE_FEATURE_ID,
  INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
  interpretationAssuranceReceiptDigest,
  validateInterpretationAssuranceReceipt,
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
  FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID,
  FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION,
  federatedKnowledgeGatewayReceiptDigest,
  validateFederatedKnowledgeGatewayReceipt,
  FEDERATED_LENS_ASSURANCE_FEATURE_ID,
  FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION,
  federatedLensAssuranceReceiptDigest,
  validateFederatedLensAssuranceReceipt,
  SEMANTIC_PARITY_FEATURE_ID,
  SEMANTIC_PARITY_CONTRACT_VERSION,
  labSemanticParityReceiptDigest,
  validateLabSemanticParityReceipt,
  FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
  FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
  federatedRetrievalAssuranceReceiptDigest,
  validateFederatedRetrievalAssuranceReceipt,
  FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID,
  FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION,
  federatedContinualRetrievalReceiptDigest,
  validateFederatedContinualRetrievalReceipt,
  CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
  CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
  contextCompilationAssuranceReceiptDigest,
  validateContextCompilationAssuranceReceipt,
  KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
  KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION,
  knowledgeRepresentationAssuranceReceiptDigest,
  validateKnowledgeRepresentationAssuranceReceipt,
  RESOURCE_CONTROL_PLANE_FEATURE_ID,
  RESOURCE_CONTROL_PLANE_CONTRACT_VERSION,
  resourceControlPlaneReceiptDigest,
  validateResourceControlPlaneReceipt,
  WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID,
  WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION,
  weaveLangReleaseAssuranceReceiptDigest,
  validateWeaveLangReleaseAssuranceReceipt,
  MECHANISM_CONTROL_PLANE_FEATURE_ID,
  MECHANISM_CONTROL_PLANE_CONTRACT_VERSION,
  mechanismControlPlaneReceiptDigest,
  validateMechanismControlPlaneReceipt,
  MECHANISM_GATEWAY_FEATURE_ID,
  MECHANISM_GATEWAY_CONTRACT_VERSION,
  mechanismGatewayReceiptDigest,
  validateMechanismGatewayReceipt,
  EVIDENCE_SURVEILLANCE_FEATURE_ID,
  EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
  evidenceSurveillanceReceiptDigest,
  validateEvidenceSurveillanceReceipt,
  RETRIEVAL_SYNTHESIS_FEATURE_ID,
  RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
  retrievalSynthesisReceiptDigest,
  validateRetrievalSynthesisReceipt,
  ADAPTER_CONTEXT_COMPILATION_FEATURE_ID,
  ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION,
  adapterContextCompilationReceiptDigest,
  validateAdapterContextCompilationReceipt,
  KNOWLEDGE_WORKFLOW_FEATURE_ID,
  KNOWLEDGE_WORKFLOW_CONTRACT_VERSION,
  knowledgeWorkflowReceiptDigest,
  validateKnowledgeWorkflowReceipt,
  RESOURCE_WORKBENCH_FEATURE_ID,
  RESOURCE_WORKBENCH_CONTRACT_VERSION,
  resourceWorkbenchReceiptDigest,
  validateResourceWorkbenchReceipt,
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

test("federated knowledge gateway keeps manifest projection unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID,
    contract_version: FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION,
    request_id: "request:gateway",
    federation_id: "federation:preclinical",
    interoperability_profile: "ro-crate+prov-o:1",
    institution_ids: ["site:a", "site:b"],
    disposition: "unknown",
    manifest_digest: "a".repeat(64),
    permitted_tags: [],
    checks: ["missing tag projection remains unknown rather than an unrestricted export"],
    omissions: ["no permitted tag projection was supplied for federation"],
    artifact: { content_hash: "b".repeat(64) },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedKnowledgeGatewayReceipt(receipt));
  assert.equal(federatedKnowledgeGatewayReceiptDigest(receipt), federatedKnowledgeGatewayReceiptDigest(receipt));
});

test("federated lens assurance keeps missing lens unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_LENS_ASSURANCE_FEATURE_ID,
    contract_version: FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:lens",
    federation_id: "federation:lens",
    institution_ids: ["site:a", "site:b"],
    required_lens_ids: ["42.13.qc"],
    report_digests: [],
    absent_lens_ids: ["42.13.qc"],
    disposition: "unknown",
    checks: ["missing lens evidence remains unknown rather than negative"],
    omissions: ["required lens not run: 42.13.qc"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedLensAssuranceReceipt(receipt));
  assert.equal(federatedLensAssuranceReceiptDigest(receipt), federatedLensAssuranceReceiptDigest(receipt));
});

test("lab semantic parity keeps disagreement unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: SEMANTIC_PARITY_FEATURE_ID,
    contract_version: SEMANTIC_PARITY_CONTRACT_VERSION,
    request_id: "request:parity",
    federation_id: "federation:lab",
    protocol_id: "protocol:organoid",
    benchmark_id: "benchmark:lab",
    institution_ids: ["site:a", "site:b"],
    disposition: "unknown",
    semantic_digest: null,
    checks: ["semantic disagreement remains unknown rather than a consensus"],
    omissions: ["institution semantic or scenario identities disagree"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateLabSemanticParityReceipt(receipt));
  assert.equal(labSemanticParityReceiptDigest(receipt), labSemanticParityReceiptDigest(receipt));
});

test("federated retrieval assurance keeps missing evidence unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
    contract_version: FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:retrieval",
    federation_id: "federation:evidence",
    query_id: "query:mechanism",
    returned_source_ids: ["source:a"],
    disposition: "unknown",
    evidence_receipt_digest: null,
    checks: ["missing retrieval evidence remains unknown rather than synthesized"],
    omissions: ["requested source unavailable: source:b", "evidence derivation receipt is absent"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedRetrievalAssuranceReceipt(receipt));
  assert.equal(federatedRetrievalAssuranceReceiptDigest(receipt), federatedRetrievalAssuranceReceiptDigest(receipt));
});

test("federated continual retrieval keeps unanchored refresh unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID,
    contract_version: FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION,
    request_id: "request:continuum",
    federation_id: "federation:evidence",
    query_id: "query:mechanism",
    selected_source_ids: ["source:a"],
    stale_source_ids: ["source:a"],
    disposition: "unknown",
    prior_synthesis_digest: null,
    checks: ["stale or unanchored evidence remains unknown rather than synthesized"],
    omissions: ["prior synthesis digest is absent"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateFederatedContinualRetrievalReceipt(receipt));
  assert.equal(federatedContinualRetrievalReceiptDigest(receipt), federatedContinualRetrievalReceiptDigest(receipt));
});

test("context compilation keeps missing context unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
    contract_version: CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:context",
    federation_id: "federation:context",
    query_id: "query:mechanism",
    resolved_context_ids: ["context:a"],
    disposition: "unknown",
    evidence_receipt_digest: null,
    checks: ["incomplete context remains unknown rather than certified"],
    omissions: ["required context unavailable: context:b"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateContextCompilationAssuranceReceipt(receipt));
  assert.equal(contextCompilationAssuranceReceiptDigest(receipt), contextCompilationAssuranceReceiptDigest(receipt));
});

test("knowledge representation keeps missing fact unknown", () => {
  const receipt = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
    contract_version: KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION,
    request_id: "request:knowledge",
    federation_id: "federation:knowledge",
    query_id: "query:mechanism",
    resolved_fact_ids: ["fact:a"],
    disposition: "unknown",
    evidence_receipt_digest: null,
    checks: ["incomplete representation remains unknown rather than asserted"],
    omissions: ["required fact unavailable: fact:b"],
    artifact: { content_hash: "b".repeat(64) },
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateKnowledgeRepresentationAssuranceReceipt(receipt));
  assert.equal(knowledgeRepresentationAssuranceReceiptDigest(receipt), knowledgeRepresentationAssuranceReceiptDigest(receipt));
});

test("resource control plane keeps missing qualification unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RESOURCE_CONTROL_PLANE_FEATURE_ID, contract_version: RESOURCE_CONTROL_PLANE_CONTRACT_VERSION, request_id: "request:resources", federation_id: "federation:resources", institution_ids: ["site:a", "site:b"], qualified_resource_ids: ["resource:a"], disposition: "unknown", qualification_digest: null, checks: ["incomplete qualification remains unknown rather than executable"], omissions: ["qualification receipt is absent"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateResourceControlPlaneReceipt(receipt)); assert.equal(resourceControlPlaneReceiptDigest(receipt), resourceControlPlaneReceiptDigest(receipt)); });

test("WeaveLang release keeps incomplete closure unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID, contract_version: WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION, request_id: "request:release", run_id: "run:high-throughput", release_id: "release:2026", disposition: "unknown", artifact_digest: null, checks: ["incomplete release closure remains unknown rather than published"], omissions: ["evidence receipts are absent"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateWeaveLangReleaseAssuranceReceipt(receipt)); assert.equal(weaveLangReleaseAssuranceReceiptDigest(receipt), weaveLangReleaseAssuranceReceiptDigest(receipt)); });

test("mechanism control plane keeps missing candidate unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: MECHANISM_CONTROL_PLANE_FEATURE_ID, contract_version: MECHANISM_CONTROL_PLANE_CONTRACT_VERSION, request_id: "request:mechanism", federation_id: "federation:mechanism", question_id: "question:organoid", admitted_candidate_ids: ["candidate:a"], disposition: "unknown", evidence_receipt_digest: null, checks: ["incomplete mechanism evidence remains unknown rather than admitted"], omissions: ["required mechanism candidate unavailable: candidate:b"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateMechanismControlPlaneReceipt(receipt)); assert.equal(mechanismControlPlaneReceiptDigest(receipt), mechanismControlPlaneReceiptDigest(receipt)); });

test("mechanism gateway keeps missing projection unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: MECHANISM_GATEWAY_FEATURE_ID, contract_version: MECHANISM_GATEWAY_CONTRACT_VERSION, request_id: "request:gateway", federation_id: "federation:mechanism", source_profile: "mechanism-v1", target_profile: "mechanism-v2", projected_candidate_ids: ["candidate:a"], interoperability_profile: "ro-crate+prov-o:1", disposition: "unknown", projection_digest: null, checks: ["incomplete candidate projection remains unknown rather than interoperable"], omissions: ["projection receipt is absent"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateMechanismGatewayReceipt(receipt)); assert.equal(mechanismGatewayReceiptDigest(receipt), mechanismGatewayReceiptDigest(receipt)); });

test("evidence surveillance preserves negative sources and effect receipts", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVIDENCE_SURVEILLANCE_FEATURE_ID, contract_version: EVIDENCE_SURVEILLANCE_CONTRACT_VERSION, request_id: "request:surveillance", study_id: "study:organoid", intent: "monitor mechanism evidence", selected_source_ids: ["source:primary"], disposition: "unknown", qualified_set: { schema_version: "aurora-research-contract/1.0", set_id: "qualified-evidence:request:surveillance", study_id: "study:organoid", intent: "monitor mechanism evidence", selected_source_ids: ["source:primary"], selected_source_digests: ["a".repeat(64)], evidence_state: "unknown", negative_source_ids: ["source:negative"], omissions: ["required evidence source unavailable: source:negative"], uncertainty: ["incomplete feed coverage remains unknown"], ordering_rule: "relevance_score descending, source_id ascending", boundary: PRECLINICAL_BOUNDARY }, effect_receipts: [{ effect: "read_local_data", authorized: true, reason: "local evidence feed read is policy-authorized", receipt_digest: "c".repeat(64) }], checks: ["incomplete feed coverage remains unknown rather than promoted"], omissions: ["required evidence source unavailable: source:negative"], uncertainty: ["incomplete feed coverage remains unknown"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateEvidenceSurveillanceReceipt(receipt)); assert.equal(evidenceSurveillanceReceiptDigest(receipt), evidenceSurveillanceReceiptDigest(receipt)); });

test("multimodal retrieval synthesis keeps missing modality unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RETRIEVAL_SYNTHESIS_FEATURE_ID, contract_version: RETRIEVAL_SYNTHESIS_CONTRACT_VERSION, request_id: "request:synthesis", query_id: "query:multimodal", disposition: "unknown", synthesis: { schema_version: "aurora-research-contract/1.0", synthesis_id: "evidence-synthesis:request:synthesis", query_id: "query:multimodal", intent: "compare imaging and omics", comparability_profile: "protocol-v2", selected_evidence_ids: ["evidence:imaging"], selected_modalities: ["imaging"], selected_digests: ["a".repeat(64)], evidence_state: "unknown", negative_evidence_ids: [], contradictory_evidence_ids: [], omissions: ["required modality unavailable or incomparable: omics"], uncertainty: [], boundary: PRECLINICAL_BOUNDARY }, effect_receipts: [{ effect: "read_local_data", authorized: true, reason: "retrieval read is policy-authorized", receipt_digest: "c".repeat(64) }], checks: ["incomplete comparability or evidence coverage remains unknown"], omissions: ["required modality unavailable or incomparable: omics"], uncertainty: [], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateRetrievalSynthesisReceipt(receipt)); assert.equal(retrievalSynthesisReceiptDigest(receipt), retrievalSynthesisReceiptDigest(receipt)); });

test("adapter context compilation keeps missing fact unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ADAPTER_CONTEXT_COMPILATION_FEATURE_ID, contract_version: ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION, request_id: "request:context", query_id: "query:mechanism", resolved_fact_ids: ["fact:a"], disposition: "unknown", evidence_receipt_digest: null, checks: ["incomplete decision context remains unknown rather than certified"], omissions: ["required decision fact unavailable: fact:b"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAdapterContextCompilationReceipt(receipt)); assert.equal(adapterContextCompilationReceiptDigest(receipt), adapterContextCompilationReceiptDigest(receipt)); });

test("knowledge workflow keeps missing claim unknown", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: KNOWLEDGE_WORKFLOW_FEATURE_ID, contract_version: KNOWLEDGE_WORKFLOW_CONTRACT_VERSION, request_id: "request:knowledge", workflow_id: "workflow:multimodal", disposition: "unknown", world: { schema_version: "aurora-research-contract/1.0", world_id: "typed-knowledge-world:workflow:multimodal", workflow_id: "workflow:multimodal", study_ids: ["study:a", "study:b"], resolved_claim_ids: ["claim:a"], disposition: "unknown", evidence_receipt_digest: null, omissions: ["required claim unavailable: claim:b"], uncertainty: ["claim derivation receipt is absent"], stages: ["scope_studies", "resolve_claim_identities", "attach_evidence_derivation", "emit_typed_knowledge_world"], boundary: PRECLINICAL_BOUNDARY }, checks: ["incomplete claim closure remains unknown rather than asserted"], omissions: ["required claim unavailable: claim:b"], uncertainty: ["claim derivation receipt is absent"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateKnowledgeWorkflowReceipt(receipt)); assert.equal(knowledgeWorkflowReceiptDigest(receipt), knowledgeWorkflowReceiptDigest(receipt)); });

test("resource workbench omits protected resource", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RESOURCE_WORKBENCH_FEATURE_ID, contract_version: RESOURCE_WORKBENCH_CONTRACT_VERSION, request_id: "request:resources", need_id: "need:imaging", disposition: "unknown", qualified_resources: [], omissions: [{ resource_id: "resource:protected", reason: "resource is protected by institution policy" }], checks: ["no resource could be qualified; unknown is preserved"], artifact: { content_hash: "b".repeat(64) }, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateResourceWorkbenchReceipt(receipt)); assert.equal(resourceWorkbenchReceiptDigest(receipt), resourceWorkbenchReceiptDigest(receipt)); });

test("ingestion gateway blocks incomplete authority without effects", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: INGESTION_GATEWAY_FEATURE_ID, contract_version: INGESTION_GATEWAY_CONTRACT_VERSION, request_id: "gateway:typescript", study_id: "study:organoid", disposition: "blocked", harmonized: { schema_version: "aurora-research-contract/1.0", study_id: "study:organoid", modality_order: ["image"], alignment: { image: ["a", "z"] }, boundary: PRECLINICAL_BOUNDARY }, admitted_bundles: [], omitted_bundles: ["bundle:image"], effect_receipts: [], semantic_loss: [{ field: "authorization", reason: "missing", severity: "decision_relevant" }], reasons: ["authorization was incomplete; no external effect was authorized"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateIngestionGatewayReceipt(receipt)); assert.equal(ingestionGatewayReceiptDigest(receipt), ingestionGatewayReceiptDigest(receipt)); });

test("quality envelope keeps multi-study comparability conflicts blocked", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: QUALITY_ENVELOPE_FEATURE_ID, contract_version: QUALITY_ENVELOPE_CONTRACT_VERSION, envelope_id: "envelope:typescript", reference_schema: "aurora-qc/1", comparability_profile: "protocol-v2|instrument-v3", decision: "blocked", study_order: ["study:image", "study:rna"], modality_coverage: { imaging: 1, transcriptomics: 1 }, verdicts: [{ study_id: "study:image", modality: "imaging", quality_disposition: "pass", comparable: true, reasons: ["quality gates passed"] }, { study_id: "study:rna", modality: "transcriptomics", quality_disposition: "pass", comparable: true, reasons: ["quality gates passed"] }], omitted_modalities: [], comparability_conflicts: ["modality imaging has incompatible profiles"], semantic_loss: [{ field: "comparability", reason: "profiles differ", severity: "decision_relevant" }], reasons: ["comparability conflict blocks qualification"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateQualityEnvelopeReceipt(receipt)); assert.equal(qualityEnvelopeReceiptDigest(receipt), qualityEnvelopeReceiptDigest(receipt)); });

test("experiment design blocks incomplete authorization without assignments", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EXPERIMENT_DESIGN_CONTROL_FEATURE_ID, contract_version: EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION, request_id: "design:typescript", objective_id: "objective:organoid", decision: "blocked", site_order: ["site:a", "site:b"], assignments: [], modality_coverage: { imaging: 1, transcriptomics: 1 }, omitted_modalities: [], comparability_conflicts: [], semantic_loss: [{ field: "authorization", reason: "missing", severity: "decision_relevant" }], reasons: ["policy or independent authorization is incomplete"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateExperimentDesignReceipt(receipt)); assert.equal(experimentDesignReceiptDigest(receipt), experimentDesignReceiptDigest(receipt)); });

test("protocol simulation preserves approval-required scenarios", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: PROTOCOL_SIMULATION_FEATURE_ID, contract_version: PROTOCOL_SIMULATION_CONTRACT_VERSION, protocol_id: "protocol:typescript", design_digest: "a".repeat(64), results: [{ scenario_id: "scenario:approval", state: "approval_required", reasons: ["effect approval required"] }, { scenario_id: "scenario:nominal", state: "passed", reasons: ["completed"] }], passed: 1, failed_closed: 0, approval_required: 1, omissions: ["scenario scenario:approval did not complete as passed"], uncertainty: [], semantic_loss: [], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateProtocolSimulationReceipt(receipt)); assert.equal(protocolSimulationReceiptDigest(receipt), protocolSimulationReceiptDigest(receipt)); });

test("instrument mesh preserves approval without physical effect", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: INSTRUMENT_MESH_FEATURE_ID, contract_version: INSTRUMENT_MESH_CONTRACT_VERSION, request_id: "request:mesh", federation_id: "federation:typescript", action_id: "action:image", decision: "approval_required", candidate_order: ["scope-a@site-1"], selected_instrument_id: "scope-a", selected_site_id: "site-1", selected_protocol_profile: "ome-ngff-v0.5", satisfied_capabilities: ["image.acquire"], missing_capabilities: [], missing_interlocks: [], effect: null, omissions: [], uncertainty: [], semantic_loss: [], reasons: ["independent authorization reference is required before any external effect"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateInstrumentMeshReceipt(receipt)); assert.equal(instrumentMeshReceiptDigest(receipt), instrumentMeshReceiptDigest(receipt)); });

test("computational execution admission keeps the run planned", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EXECUTION_CONTROL_FEATURE_ID, contract_version: EXECUTION_CONTROL_CONTRACT_VERSION, request_id: "request:execution", workflow_id: "workflow:execution", run_id: "run:execution", decision: "admitted", ordered_nodes: ["a-read", "b-compute"], admitted_nodes: ["a-read", "b-compute"], run: { workflow_id: "workflow:execution", run_id: "run:execution", status: "planned" }, run_digest: "a".repeat(64), authorized_effects: [{ node_id: "a-read", effect: "execute_local_computation", authorized: true, executed: false, payload_digest: "b".repeat(64) }, { node_id: "b-compute", effect: "execute_local_computation", authorized: true, executed: false, payload_digest: "c".repeat(64) }], omissions: [], uncertainty: [], semantic_loss: [], reasons: ["workflow graph, locality, policy, authority, and replay gates passed"], artifact: { content_hash: "d".repeat(64) }, effects_executed: false, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateComputationalExecutionReceipt(receipt)); assert.equal(computationalExecutionReceiptDigest(receipt), computationalExecutionReceiptDigest(receipt)); });

test("analysis portfolio preserves negative evidence and conditionality", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ANALYSIS_PORTFOLIO_FEATURE_ID, contract_version: ANALYSIS_PORTFOLIO_CONTRACT_VERSION, question_id: "question:effect", estimand: "average treatment effect in organoid model", verdict: "conditional", selected_candidate: "candidate:a", candidate_order: ["candidate:a", "candidate:b"], uncertainty: ["candidate:a: interval is wide"], omissions: ["missing independent site"], negative_evidence: ["candidate:b: null replication not available"], semantic_loss: [], reasons: ["protected omissions prevent unconditional analytical qualification"], artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAnalysisPortfolioReceipt(receipt)); assert.equal(analysisPortfolioReceiptDigest(receipt), analysisPortfolioReceiptDigest(receipt)); });

test("interpretation assurance preserves omitted modality and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: INTERPRETATION_ASSURANCE_FEATURE_ID, contract_version: INTERPRETATION_ASSURANCE_CONTRACT_VERSION, result_id: "result:interpretation", verdict: "conditional", claim_order: ["claim:a"], covered_modalities: ["imaging"], omitted_modalities: ["omics"], uncertainty: ["claim:a: measurement uncertainty remains"], negative_evidence: ["claim:a: null replicate is absent"], semantic_loss: [], reasons: ["required modality coverage is incomplete"], artifact: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateInterpretationAssuranceReceipt(receipt)); assert.equal(interpretationAssuranceReceiptDigest(receipt), interpretationAssuranceReceiptDigest(receipt)); });
