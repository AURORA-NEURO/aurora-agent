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
  REPLICATION_ASSURANCE_FEATURE_ID,
  REPLICATION_ASSURANCE_CONTRACT_VERSION,
  replicationAssuranceReceiptDigest,
  validateReplicationAssuranceReceipt,
  RELEASE_ASSURANCE_FEATURE_ID,
  RELEASE_ASSURANCE_CONTRACT_VERSION,
  releaseAssuranceReceiptDigest,
  validateReleaseAssuranceReceipt,
  DETERMINISM_GATEWAY_FEATURE_ID,
  DETERMINISM_GATEWAY_CONTRACT_VERSION,
  determinismGatewayReceiptDigest,
  validateDeterminismGatewayReceipt,
  PROVENANCE_ASSURANCE_FEATURE_ID,
  PROVENANCE_ASSURANCE_CONTRACT_VERSION,
  provenanceAssuranceReceiptDigest,
  validateProvenanceAssuranceReceipt,
  POLICY_GATEWAY_FEATURE_ID,
  POLICY_GATEWAY_CONTRACT_VERSION,
  policyGatewayReceiptDigest,
  validatePolicyGatewayReceipt,
  FEDERATION_WORKFLOW_FEATURE_ID,
  FEDERATION_WORKFLOW_CONTRACT_VERSION,
  federationWorkflowReceiptDigest,
  validateFederationWorkflowReceipt,
  RELIABILITY_COPILOT_FEATURE_ID,
  RELIABILITY_COPILOT_CONTRACT_VERSION,
  reliabilityCopilotReceiptDigest,
  validateReliabilityCopilotReceipt,
  INTEROPERABILITY_GATEWAY_FEATURE_ID,
  INTEROPERABILITY_GATEWAY_CONTRACT_VERSION,
  interoperabilityGatewayReceiptDigest,
  validateInteroperabilityGatewayReceipt,
  EVALUATION_ASSURANCE_FEATURE_ID,
  EVALUATION_ASSURANCE_CONTRACT_VERSION,
  evaluationAssuranceReceiptDigest,
  validateEvaluationAssuranceReceipt,
  RESEARCH_WORKBENCH_FEATURE_ID,
  RESEARCH_WORKBENCH_CONTRACT_VERSION,
  researchWorkbenchReceiptDigest,
  validateResearchWorkbenchReceipt,
  CONTRACT_FRONTIER_FEATURE_ID,
  CONTRACT_FRONTIER_CONTRACT_VERSION,
  contractFrontierReceiptDigest,
  validateContractFrontierReceipt,
  LIMITATION_CLOSURE_FEATURE_ID,
  LIMITATION_CLOSURE_CONTRACT_VERSION,
  limitationClosureReceiptDigest,
  validateLimitationClosureReceipt,
  DEPENDENCY_COMPOSITION_FEATURE_ID,
  DEPENDENCY_COMPOSITION_CONTRACT_VERSION,
  adapterCompositionReceiptDigest,
  validateAdapterCompositionReceipt,
  ADAPTER_SEMANTIC_PARITY_FEATURE_ID,
  ADAPTER_SEMANTIC_PARITY_CONTRACT_VERSION,
  adapterSemanticParityReceiptDigest,
  validateAdapterSemanticParityReceipt,
  ADAPTER_SCALE_FRONTIER_FEATURE_ID,
  ADAPTER_SCALE_FRONTIER_CONTRACT_VERSION,
  scaleFrontierReceiptDigest,
  validateScaleFrontierReceipt,
  ADVERSARIAL_RECOVERY_FEATURE_ID,
  ADVERSARIAL_RECOVERY_CONTRACT_VERSION,
  adversarialRecoveryReceiptDigest,
  validateAdversarialRecoveryReceipt,
  FEDERATED_COMMONS_FEATURE_ID,
  FEDERATED_COMMONS_CONTRACT_VERSION,
  federatedCommonsReceiptDigest,
  validateFederatedCommonsReceipt,
  BOUNDED_EVOLUTION_FEATURE_ID,
  BOUNDED_EVOLUTION_CONTRACT_VERSION,
  boundedEvolutionReceiptDigest,
  validateBoundedEvolutionReceipt,
  EVOLUTION_IDENTITY_FEATURE_ID,
  EVOLUTION_IDENTITY_CONTRACT_VERSION,
  evolutionIdentityReceiptDigest,
  validateEvolutionIdentityReceipt,
  EVOLUTION_ASSURANCE_FEATURE_ID,
  EVOLUTION_ASSURANCE_CONTRACT_VERSION,
  evolutionAssuranceReceiptDigest,
  validateEvolutionAssuranceReceipt,
  INTERPRETATION_PLANE_FEATURE_ID,
  INTERPRETATION_PLANE_CONTRACT_VERSION,
  interpretationPlaneReceiptDigest,
  validateInterpretationPlaneReceipt,
  KNOWLEDGE_GATEWAY_FEATURE_ID,
  KNOWLEDGE_GATEWAY_CONTRACT_VERSION,
  knowledgeGatewayReceiptDigest,
  validateKnowledgeGatewayReceipt,
  ORACLE_ASSURANCE_FEATURE_ID,
  ORACLE_ASSURANCE_CONTRACT_VERSION,
  oracleCapabilityManifestReceiptDigest,
  validateOracleCapabilityManifestReceipt,
  FEDERATED_INGESTION_FEATURE_ID,
  FEDERATED_INGESTION_CONTRACT_VERSION,
  federatedMultimodalIngestionReceiptDigest,
  validateFederatedMultimodalIngestionReceipt,
  QUALITY_ASSURANCE_FEATURE_ID,
  QUALITY_ASSURANCE_CONTRACT_VERSION,
  qualityAssuranceReceiptDigest,
  validateQualityAssuranceReceipt,
  MECHANISM_CONTROL_FEATURE_ID,
  MECHANISM_CONTROL_CONTRACT_VERSION,
  mechanismControlReceiptDigest,
  validateMechanismControlReceipt,
  EVIDENCE_WORKBENCH_FEATURE_ID,
  EVIDENCE_WORKBENCH_CONTRACT_VERSION,
  evidenceWorkbenchReceiptDigest,
  validateEvidenceWorkbenchReceipt,
  ANALYSIS_CONTROL_FEATURE_ID,
  ANALYSIS_CONTROL_CONTRACT_VERSION,
  analysisControlReceiptDigest,
  validateAnalysisControlReceipt,
  CONTEXT_ASSURANCE_FEATURE_ID,
  CONTEXT_ASSURANCE_CONTRACT_VERSION,
  contextAssuranceReceiptDigest,
  validateContextAssuranceReceipt,
  EVALUATION_ASSURANCE_BIOWORLDS_FEATURE_ID,
  EVALUATION_ASSURANCE_BIOWORLDS_CONTRACT_VERSION,
  bioworldsEvaluationAssuranceReceiptDigest,
  validateBioworldsEvaluationAssuranceReceipt,
  QUALITY_WORKBENCH_BIOLANG_FEATURE_ID,
  QUALITY_WORKBENCH_BIOLANG_CONTRACT_VERSION,
  biolangQualityWorkbenchReceiptDigest,
  validateBiolangQualityWorkbenchReceipt,
  RETRIEVAL_ASSURANCE_BIOLANG_FEATURE_ID,
  RETRIEVAL_ASSURANCE_BIOLANG_CONTRACT_VERSION,
  biolangRetrievalAssuranceReceiptDigest,
  validateBiolangRetrievalAssuranceReceipt,
  CLI_KNOWLEDGE_INTEROPERABILITY_FEATURE_ID,
  CLI_KNOWLEDGE_INTEROPERABILITY_CONTRACT_VERSION,
  cliKnowledgeInteroperabilityReceiptDigest,
  validateCliKnowledgeInteroperabilityReceipt,
  LAB_EVIDENCE_SURVEILLANCE_FEATURE_ID,
  LAB_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
  labEvidenceSurveillanceReceiptDigest,
  validateLabEvidenceSurveillanceReceipt,
  FIBER_MECHANISM_ASSURANCE_FEATURE_ID,
  FIBER_MECHANISM_ASSURANCE_CONTRACT_VERSION,
  fiberMechanismAssuranceReceiptDigest,
  validateFiberMechanismAssuranceReceipt,
  HUBAPI_QUALITY_ASSURANCE_FEATURE_ID,
  HUBAPI_QUALITY_ASSURANCE_CONTRACT_VERSION,
  hubapiQualityAssuranceReceiptDigest,
  validateHubapiQualityAssuranceReceipt,
  REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_FEATURE_ID,
  REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_CONTRACT_VERSION,
  registryResourceDiscoveryAssuranceReceiptDigest,
  validateRegistryResourceDiscoveryAssuranceReceipt,
  SERVICES_MECHANISM_WORKBENCH_FEATURE_ID,
  SERVICES_MECHANISM_WORKBENCH_CONTRACT_VERSION,
  servicesMechanismWorkbenchReceiptDigest,
  validateServicesMechanismWorkbenchReceipt,
  GOVERNANCE_INTERPRETATION_ASSURANCE_FEATURE_ID,
  GOVERNANCE_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
  governanceInterpretationAssuranceReceiptDigest,
  validateGovernanceInterpretationAssuranceReceipt,
  ORACLE_INGESTION_CONTROL_FEATURE_ID,
  ORACLE_INGESTION_CONTROL_CONTRACT_VERSION,
  oracleIngestionControlReceiptDigest,
  validateOracleIngestionControlReceipt,
  STEWARDSHIP_RELEASE_WORKBENCH_FEATURE_ID,
  STEWARDSHIP_RELEASE_WORKBENCH_CONTRACT_VERSION,
  stewardshipReleaseWorkbenchReceiptDigest,
  validateStewardshipReleaseWorkbenchReceipt,
  API_ANALYSIS_ASSURANCE_FEATURE_ID,
  API_ANALYSIS_ASSURANCE_CONTRACT_VERSION,
  apiAnalysisAssuranceReceiptDigest,
  validateApiAnalysisAssuranceReceipt,
  STORE_EVIDENCE_OPERATIONS_FEATURE_ID,
  STORE_EVIDENCE_OPERATIONS_CONTRACT_VERSION,
  storeEvidenceOperationsReceiptDigest,
  validateStoreEvidenceOperationsReceipt,
  POLICY_INTEROPERABILITY_CONTROL_FEATURE_ID,
  POLICY_INTEROPERABILITY_CONTROL_CONTRACT_VERSION,
  policyInteroperabilityControlReceiptDigest,
  validatePolicyInteroperabilityControlReceipt,
  SAFETY_MECHANISM_WORKFLOW_FEATURE_ID,
  SAFETY_MECHANISM_WORKFLOW_CONTRACT_VERSION,
  safetyMechanismWorkflowReceiptDigest,
  validateSafetyMechanismWorkflowReceipt,
  HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID,
  HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
  hubapiMultimodalInterpretationAssuranceReceiptDigest,
  validateHubapiMultimodalInterpretationAssuranceReceipt,
  BIOLANG_PUBLICATION_COPILOT_FEATURE_ID,
  BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION,
  biolangPublicationCopilotReceiptDigest,
  validateBiolangPublicationCopilotReceipt,
  API_RELEASE_ASSURANCE_FEATURE_ID,
  API_RELEASE_ASSURANCE_CONTRACT_VERSION,
  apiReleaseAssuranceReceiptDigest,
  validateApiReleaseAssuranceReceipt,
  BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID,
  BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION,
  bioevalxFederationGatewayReceiptDigest,
  validateBioevalxFederationGatewayReceipt,
  SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID,
  SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
  sectionInterpretationAssuranceReceiptDigest,
  validateSectionInterpretationAssuranceReceipt,
  OPS_RETRIEVAL_ASSURANCE_FEATURE_ID,
  OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
  opsRetrievalAssuranceReceiptDigest,
  validateOpsRetrievalAssuranceReceipt,
  CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_FEATURE_ID,
  CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_CONTRACT_VERSION,
  conformanceKnowledgeWorldAssuranceReceiptDigest,
  validateConformanceKnowledgeWorldAssuranceReceipt,
  BRAIN_EVIDENCE_SURVEILLANCE_FEATURE_ID,
  BRAIN_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
  brainEvidenceSurveillanceReceiptDigest,
  validateBrainEvidenceSurveillanceReceipt,
  BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
  BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
  brainMultimodalEvidenceSurveillanceReceiptDigest,
  validateBrainMultimodalEvidenceSurveillanceReceipt,
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

test("replication assurance preserves partition and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: REPLICATION_ASSURANCE_FEATURE_ID, contract_version: REPLICATION_ASSURANCE_CONTRACT_VERSION, claim_id: "claim:mechanism", protocol_digest: "a".repeat(64), verdict: "partially_replicated", observation_order: ["obs:a", "obs:b"], independent_site_order: ["site:a", "site:b"], positive_count: 2, null_count: 0, negative_count: 0, inconclusive_count: 0, omissions: ["site:c: federation partition"], uncertainty: ["obs:a: interval remains wide"], negative_evidence: ["obs:b: null secondary endpoint"], semantic_loss: [{ field: "omissions", severity: "decision_relevant" }], reasons: ["protected omission prevents unconditional replication"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateReplicationAssuranceReceipt(receipt)); assert.equal(replicationAssuranceReceiptDigest(receipt), replicationAssuranceReceiptDigest(receipt)); });

test("release assurance preserves multimodal omissions and effect boundary", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RELEASE_ASSURANCE_FEATURE_ID, contract_version: RELEASE_ASSURANCE_CONTRACT_VERSION, run_id: "run:release", release_id: "release:2026-q3", verdict: "conditional", study_order: ["study:imaging", "study:omics"], modality_order: ["imaging", "omics"], artifact_order: ["artifact:imaging", "artifact:omics"], evidence_receipt_order: ["evidence:1"], omissions: ["study:replicate missing"], uncertainty: ["study:omics: batch interval is bounded"], negative_evidence: ["null secondary endpoint"], semantic_loss: [{ field: "omissions", severity: "decision_relevant" }], reasons: ["protected omission prevents unconditional release"], policy_decision: "allow", effect_receipt: "block_unsafe_release_and_retain_local_receipt", artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateReleaseAssuranceReceipt(receipt)); assert.equal(releaseAssuranceReceiptDigest(receipt), releaseAssuranceReceiptDigest(receipt)); });

test("typed determinism gateway preserves migration and canonical digest", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: DETERMINISM_GATEWAY_FEATURE_ID, contract_version: DETERMINISM_GATEWAY_CONTRACT_VERSION, capability_id: "capability:qc", endpoint_id: "endpoint:site-a", negotiated_version: "1.0.0", verdict: "migrated", canonical_field_order: ["algorithm", "schema", "threshold"], canonical_input_digest: "d".repeat(64), omissions: ["legacy fields remain unknown"], uncertainty: ["migration cannot infer omitted semantics"], semantic_loss: [{ field: "legacy_fields", severity: "unknown" }], reasons: ["compatible migration retained unknown fields"], effect_receipt: "exchange:permitted-artifacts", artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateDeterminismGatewayReceipt(receipt)); assert.equal(determinismGatewayReceiptDigest(receipt), determinismGatewayReceiptDigest(receipt)); });

test("provenance assurance preserves lineage and signing boundary", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: PROVENANCE_ASSURANCE_FEATURE_ID, contract_version: PROVENANCE_ASSURANCE_CONTRACT_VERSION, envelope_id: "envelope:qc", root_artifact_id: "artifact:root", root_digest: "a".repeat(64), verdict: "conditional", lineage_order: ["artifact:imaging", "artifact:omics", "artifact:root"], derivation_order: ["step:root"], study_order: ["study:1", "study:2"], modality_order: ["imaging", "omics"], tool_order: ["b".repeat(64)], omissions: ["tool attestation pending"], uncertainty: ["lineage integrity is not signer authorization"], negative_evidence: ["null secondary endpoint"], semantic_loss: [{ field: "omissions", severity: "decision_relevant" }], reasons: ["protected provenance gap prevents unconditional signing"], signer_public_key_hex: "c".repeat(64), signer_signature_hex: "d".repeat(128), effect_receipt: "block_unsafe_release_and_retain_provenance_receipt", artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateProvenanceAssuranceReceipt(receipt)); assert.equal(provenanceAssuranceReceiptDigest(receipt), provenanceAssuranceReceiptDigest(receipt)); });

test("policy gateway preserves tier budget and unresolved state", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: POLICY_GATEWAY_FEATURE_ID, contract_version: POLICY_GATEWAY_CONTRACT_VERSION, request_id: "request:qc", action_id: "action:compute", decision: "approval_required", required_tier: "a3", permitted_actions: ["compute_local"], budget_order: ["cpu_seconds"], reasons: ["required autonomy approval reference is absent"], uncertainty: ["A3 action lacks signed preflight evidence"], effect_receipt: "block_or_localize_action_no_external_effect", artifact: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validatePolicyGatewayReceipt(receipt)); assert.equal(policyGatewayReceiptDigest(receipt), policyGatewayReceiptDigest(receipt)); });

test("federation workflow preserves checkpoints compensation and partition", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: FEDERATION_WORKFLOW_FEATURE_ID, contract_version: FEDERATION_WORKFLOW_CONTRACT_VERSION, workflow_id: "workflow:qc", decision: "partial", task_order: ["task:a", "task:b"], checkpoint_order: ["checkpoint:a", "checkpoint:b"], compensation_order: ["retain-a", "retain-b"], total_budget_units: 30, omissions: ["network partition prevents destination confirmation"], uncertainty: ["destination admission remains unknown"], semantic_loss: [{ field: "network_partition", severity: "decision_relevant" }], reasons: ["partitioned work remains local-only"], effect_receipt: "retain_local_checkpoint_and_block_remote_schedule", envelope: { export: { content_hash: "a".repeat(64) } }, artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateFederationWorkflowReceipt(receipt)); assert.equal(federationWorkflowReceiptDigest(receipt), federationWorkflowReceiptDigest(receipt)); });

test("reliability copilot preserves dry-run retry and failure receipts", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RELIABILITY_COPILOT_FEATURE_ID, contract_version: RELIABILITY_COPILOT_CONTRACT_VERSION, workload_id: "workload:qc", decision: "partial", invocation_order: ["invoke:a", "invoke:b"], retry_order: ["invoke:b"], tool_order: ["tool:qc"], budget_used_units: 15, timeout_order: ["invoke:a", "invoke:b"], omissions: ["failed or timed-out invocations remain unresolved"], uncertainty: [], failure_reasons: ["invoke:b: timeout"], effect_receipts: ["bounded-tool-invocation:invoke:a:not-executed"], artifact: { content_hash: "a".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateReliabilityCopilotReceipt(receipt)); assert.equal(reliabilityCopilotReceiptDigest(receipt), reliabilityCopilotReceiptDigest(receipt)); });

test("interoperability gateway preserves migration loss and digest-only effects", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: INTEROPERABILITY_GATEWAY_FEATURE_ID, contract_version: INTEROPERABILITY_GATEWAY_CONTRACT_VERSION, request_id: "request:qc", endpoint_id: "endpoint:site-a", negotiated_version: "1.0.0", disposition: "migrated", capability_order: ["artifact-digest", "qc-summary"], artifact_digest_order: ["a".repeat(64)], replay_token: "b".repeat(64), omissions: ["legacy fields remain unknown"], uncertainty: [], semantic_loss: [{ field: "legacy_fields", severity: "unknown" }], checks: ["capability names canonicalized"], effect_receipts: ["exchange:permitted-artifact-digests-only"], artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateInteroperabilityGatewayReceipt(receipt)); assert.equal(interoperabilityGatewayReceiptDigest(receipt), interoperabilityGatewayReceiptDigest(receipt)); });

test("evaluation assurance preserves witnesses counterexamples and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVALUATION_ASSURANCE_FEATURE_ID, contract_version: EVALUATION_ASSURANCE_CONTRACT_VERSION, run_id: "run:qc", capability_id: "capability:qc", benchmark_id: "benchmark:heldout", baseline_id: "baseline:v1", verdict: "blocked", metric_order: ["adr"], gate_order: ["baseline_delta", "policy_allow"], witness_order: ["w:adr"], counterexample_order: ["metric-under-baseline:adr"], omissions: [], uncertainty: [], negative_evidence: ["null secondary metric"], reasons: ["one or more baseline, witness, or measurement gates failed"], effect_receipts: ["block:unsafe-release:blocked"], replay_identity: "a".repeat(64), artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateEvaluationAssuranceReceipt(receipt)); assert.equal(evaluationAssuranceReceiptDigest(receipt), evaluationAssuranceReceiptDigest(receipt)); });

test("research workbench preserves multimodal views and locality", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RESEARCH_WORKBENCH_FEATURE_ID, contract_version: RESEARCH_WORKBENCH_CONTRACT_VERSION, workspace_id: "workspace:atlas", disposition: "partial", study_order: ["study:imaging", "study:omics"], modality_order: ["imaging", "omics"], view_order: ["view:comparison"], panel_order: ["panel:view:comparison"], artifact_order: ["a".repeat(64), "b".repeat(64)], omissions: ["view:comparison:missing-modality:spatial-transcriptomics"], uncertainty: ["study:omics:comparability-or-provenance-unknown"], negative_evidence: ["study:imaging:null secondary endpoint"], action_receipts: ["view:view:comparison:conditional-comparability"], artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateResearchWorkbenchReceipt(receipt)); assert.equal(researchWorkbenchReceiptDigest(receipt), researchWorkbenchReceiptDigest(receipt)); });

test("contract frontier preserves versioned manifests and migration loss", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: CONTRACT_FRONTIER_FEATURE_ID, contract_version: CONTRACT_FRONTIER_CONTRACT_VERSION, adapter_id: "adapter:multimodal", capability_id: "capability:harmonize", negotiated_version: "2.0.0", disposition: "migrated", input_schema: "AdapterContractInput2", output_schema: "AdapterCapabilityManifest6", modality_order: ["imaging", "omics"], effect_order: ["exchange:permitted-artifacts"], permission_order: ["connect:approved-endpoints"], artifact_digest_order: ["a".repeat(64)], omissions: ["legacy contract fields remain unknown after additive migration"], uncertainty: ["semantic parity for omitted legacy fields is unmeasured"], semantic_loss: ["legacy_fields:unknown"], checks: ["effect and permission names canonicalized"], effect_receipts: ["exchange:permitted-capability-manifest-and-digests"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateContractFrontierReceipt(receipt)); assert.equal(contractFrontierReceiptDigest(receipt), contractFrontierReceiptDigest(receipt)); });

test("limitation closure preserves unresolved cases and digest-only effects", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: LIMITATION_CLOSURE_FEATURE_ID, contract_version: LIMITATION_CLOSURE_CONTRACT_VERSION, request_id: "closure:adapter-1", disposition: "partial", case_order: ["case:missing-calibration"], resolved_order: [], unresolved_order: ["case:missing-calibration"], evidence_order: ["a".repeat(64)], omissions: ["case:missing-calibration:closure-criteria-unmet"], uncertainty: ["case:missing-calibration:measurement-not-available"], negative_evidence: ["case:missing-calibration:null-recovery"], reasons: ["unresolved limitation remains visible"], effect_receipts: ["exchange:permitted-limitation-digests-only"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateLimitationClosureReceipt(receipt)); assert.equal(limitationClosureReceiptDigest(receipt), limitationClosureReceiptDigest(receipt)); });

test("adapter dependency composition preserves missing capability and provider order", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: DEPENDENCY_COMPOSITION_FEATURE_ID, contract_version: DEPENDENCY_COMPOSITION_CONTRACT_VERSION, request_id: "composition:multimodal", objective_id: "objective:qc", disposition: "partial", component_order: ["component:features", "component:final"], selected_order: ["component:features", "component:final"], missing_capability_order: ["capability:missing"], dependency_order: ["component:final->capability:features"], modality_order: ["imaging", "omics"], artifact_order: ["a".repeat(64), "b".repeat(64)], omissions: ["capability:missing:no-compatible-provider"], uncertainty: ["capability:features:multiple-providers-ranked-by-component-id"], negative_evidence: ["capability:missing:negative-provider-evidence"], reasons: ["missing capabilities remain explicit and cannot be executed"], effect_receipts: ["exchange:permitted-composition-manifest-and-digests"], artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAdapterCompositionReceipt(receipt)); assert.equal(adapterCompositionReceiptDigest(receipt), adapterCompositionReceiptDigest(receipt)); });

test("adapter semantic parity preserves missing modality and digest comparison", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ADAPTER_SEMANTIC_PARITY_FEATURE_ID, contract_version: ADAPTER_SEMANTIC_PARITY_CONTRACT_VERSION, request_id: "parity:adapter", objective_id: "objective:qc", disposition: "unknown", adapter_order: ["adapter:a", "adapter:b"], study_order: ["study:a", "study:b"], schema_order: ["a".repeat(64)], semantic_digest: "b".repeat(64), modality_order: ["imaging", "omics"], artifact_order: ["c".repeat(64)], checks: ["semantic disagreement remains unknown"], omissions: ["modality:spatial:missing"], uncertainty: ["adapter schema, semantic, or modality digests disagree"], negative_evidence: ["modality:spatial:no-admitted-adapter-evidence"], effect_receipts: ["block:adapter-semantic-parity:unknown"], artifact: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAdapterSemanticParityReceipt(receipt)); assert.equal(adapterSemanticParityReceiptDigest(receipt), adapterSemanticParityReceiptDigest(receipt)); });

test("adapter scale frontier preserves blocked budget cells", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ADAPTER_SCALE_FRONTIER_FEATURE_ID, contract_version: ADAPTER_SCALE_FRONTIER_CONTRACT_VERSION, request_id: "scale:adapter", workflow_id: "workflow:high-throughput", disposition: "partial", scenario_order: ["scenario:a", "scenario:b"], admissible_order: ["scenario:b"], blocked_order: ["scenario:a"], frontier_order: ["scenario:a", "scenario:b"], max_admitted_concurrency: 8, checks: ["scenarios are ordered by stable id"], omissions: ["scenario:scenario:a:capacity-below-required"], uncertainty: [], negative_evidence: [], effect_receipts: ["exchange:permitted-scale-frontier-digests-only"], artifact: { content_hash: "a".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateScaleFrontierReceipt(receipt)); assert.equal(scaleFrontierReceiptDigest(receipt), scaleFrontierReceiptDigest(receipt)); });

test("adversarial recovery preserves blocked events and checkpoints", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ADVERSARIAL_RECOVERY_FEATURE_ID, contract_version: ADVERSARIAL_RECOVERY_CONTRACT_VERSION, request_id: "recovery:adapter", workflow_id: "workflow:federated", disposition: "partial", event_order: ["event:a", "event:b"], recovered_order: ["event:a"], blocked_order: ["event:b"], replay_order: ["event:a", "event:b"], checkpoint_order: ["a".repeat(64), "b".repeat(64)], recovery_digest: null, checks: ["adversarial event kinds fail closed without remote effects"], omissions: ["event:event:b:non-recoverable"], uncertainty: [], negative_evidence: ["event:event:b:adversarial-kind-poisoned_artifact"], effect_receipts: ["exchange:permitted-recovery-checkpoints-and-digests-only"], artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAdversarialRecoveryReceipt(receipt)); assert.equal(adversarialRecoveryReceiptDigest(receipt), adversarialRecoveryReceiptDigest(receipt)); });
test("federated commons preserves partial purpose-bound exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: FEDERATED_COMMONS_FEATURE_ID, contract_version: FEDERATED_COMMONS_CONTRACT_VERSION, request_id: "commons:adapter", federation_id: "federation:preclinical", objective_id: "objective:benchmark", required_purpose: "benchmark", disposition: "partial", institution_order: ["site:a", "site:b"], admitted_order: ["site:a"], denied_order: ["site:b"], semantic_profile_order: ["ome-ngff+anndata:v1"], artifact_order: ["a".repeat(64)], checks: ["purpose, aggregate-only, policy, locality, and semantic-profile gates are explicit"], omissions: ["institution:site:b:raw-or-nonaggregate-exchange-denied"], uncertainty: [], negative_evidence: ["institution:site:b:purpose-not-authorized"], effect_receipts: ["exchange:permitted-purpose-bound-aggregate-digests-only"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateFederatedCommonsReceipt(receipt)); assert.equal(federatedCommonsReceiptDigest(receipt), federatedCommonsReceiptDigest(receipt)); });
test("bounded evolution preserves budget and replay gates", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: BOUNDED_EVOLUTION_FEATURE_ID, contract_version: BOUNDED_EVOLUTION_CONTRACT_VERSION, request_id: "evolution:adapter", workflow_id: "workflow:high-throughput", objective_id: "objective:bounded-evolution", disposition: "partial", candidate_order: ["candidate:a", "candidate:b"], admitted_order: ["candidate:a"], blocked_order: ["candidate:b"], evidence_order: ["a".repeat(64)], replay_order: ["b".repeat(64), "c".repeat(64)], budget: 8, budget_remaining: 1, max_concurrency: 2, checks: ["replay, determinism, safety, evidence, policy, budget, and boundary gates are explicit"], omissions: ["candidate:candidate:b:budget-ceiling-exceeded"], uncertainty: [], negative_evidence: ["candidate:candidate:b:required-evidence-not-present"], effect_receipts: ["effect:admission-receipt-only-no-deployment"], artifact: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBoundedEvolutionReceipt(receipt)); assert.equal(boundedEvolutionReceiptDigest(receipt), boundedEvolutionReceiptDigest(receipt)); });
test("evolution identity preserves generation lineage and digest", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVOLUTION_IDENTITY_FEATURE_ID, contract_version: EVOLUTION_IDENTITY_CONTRACT_VERSION, workflow_id: "workflow:high-throughput", candidate_id: "candidate:a", generation: 2, parent_digest: "a".repeat(64), baseline_digest: "b".repeat(64), artifact_digest: "c".repeat(64), replay_identity: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateEvolutionIdentityReceipt(receipt)); assert.equal(evolutionIdentityReceiptDigest(receipt), evolutionIdentityReceiptDigest(receipt)); });
test("evolution assurance preserves blocked release evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVOLUTION_ASSURANCE_FEATURE_ID, contract_version: EVOLUTION_ASSURANCE_CONTRACT_VERSION, request_id: "assurance:request", workflow_id: "workflow:high-throughput", source_receipt_digest: "a".repeat(64), replay_identity: "b".repeat(64), benchmark_digest: "c".repeat(64), verdict: "blocked", passed_checks: ["canonical-order"], failed_checks: ["release-boundary"], missing_checks: [], omissions: ["assurance:unsafe-release"], uncertainty: [], negative_evidence: ["check:release-boundary:deployment effect"], effect_receipts: ["block:unsafe-release:blocked"], artifact: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateEvolutionAssuranceReceipt(receipt)); assert.equal(evolutionAssuranceReceiptDigest(receipt), evolutionAssuranceReceiptDigest(receipt)); });
test("interpretation plane preserves digest-only federation boundary", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: INTERPRETATION_PLANE_FEATURE_ID, contract_version: INTERPRETATION_PLANE_CONTRACT_VERSION, request_id: "interpretation:plane", workflow_id: "workflow:interpretation", disposition: "partial", interpretation_order: ["result:a"], blocked_order: ["result:b"], replay_identity: "a".repeat(64), budget: 10, budget_remaining: 8, max_concurrency: 2, checks: ["digest-only summaries remain local or policy-permitted"], omissions: ["result:result:b:protected-omission-or-uncertainty"], uncertainty: [], negative_evidence: ["result:result:b:state-unknown-cannot-export"], effect_receipts: ["exchange:permitted-summary:result:a", "manage:local-capability:result:a"], artifact: { content_hash: "b".repeat(64), media_type: "application/vnd.aurora.ids-interpretation-plane+json", scope: "ids-interpretation-plane:interpretation:plane" }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateInterpretationPlaneReceipt(receipt)); assert.equal(interpretationPlaneReceiptDigest(receipt), interpretationPlaneReceiptDigest(receipt)); });
test("knowledge gateway preserves unresolved typed world state", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: KNOWLEDGE_GATEWAY_FEATURE_ID, contract_version: KNOWLEDGE_GATEWAY_CONTRACT_VERSION, request_id: "gateway:knowledge", federation_id: "federation:commons", disposition: "unknown", world: { world_id: "knowledge-world:gateway:knowledge", scope: "organoid:neural", target_schema: "typed-knowledge-world/6", claim_order: [], artifact_order: [], evidence_order: [], provenance_order: [], omissions: [], uncertainty: ["request:protected-closure-incomplete"], negative_evidence: [], world_digest: "a".repeat(64), boundary: PRECLINICAL_BOUNDARY }, replay_identity: "b".repeat(64), checks: ["scope and protected-closure gates remain explicit"], omissions: [], uncertainty: ["request:protected-closure-incomplete"], negative_evidence: [], effect_receipts: ["block:knowledge-gateway:unknown"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateKnowledgeGatewayReceipt(receipt)); assert.equal(knowledgeGatewayReceiptDigest(receipt), knowledgeGatewayReceiptDigest(receipt)); });
test("oracle assurance preserves partial admission and provenance", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ORACLE_ASSURANCE_FEATURE_ID, contract_version: ORACLE_ASSURANCE_CONTRACT_VERSION, manifest_id: "oracle-manifest:assurance", request_id: "oracle:assurance", workflow_id: "workflow:benchmark", benchmark_id: "benchmark:held-out-family", scope: "organoid:neural", disposition: "partial", admitted_order: ["oracle:a"], blocked_order: ["oracle:b"], evidence_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], source_receipt_digest: "c".repeat(64), benchmark_digest: "d".repeat(64), replay_identity: "e".repeat(64), budget: 10, budget_remaining: 8, checks: ["canonical oracle ordering and content-addressed manifest"], omissions: ["oracle:oracle:b:protected-closure-or-evidence-incomplete"], uncertainty: [], negative_evidence: ["oracle:oracle:b:state-contradicted-not-admitted"], effect_receipts: ["verify:oracle:oracle:a"], artifact: { content_hash: "f".repeat(64), media_type: "application/vnd.aurora.oracle-capability-manifest+json", scope: "oracle-assurance:oracle:assurance" }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateOracleCapabilityManifestReceipt(receipt)); assert.equal(oracleCapabilityManifestReceiptDigest(receipt), oracleCapabilityManifestReceiptDigest(receipt)); });
test("federated ingestion preserves partial multimodal object and locality", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: FEDERATED_INGESTION_FEATURE_ID, contract_version: FEDERATED_INGESTION_CONTRACT_VERSION, request_id: "ingestion:federated", workflow_id: "workflow:continual-ingestion", institution_id: "site:a", disposition: "partial", object: { object_id: "harmonized-object:ingestion:federated", study_id: "study:organoid", scope: "organoid:neural", semantic_profile: "imaging:ome-ngff/0.5", modality_order: ["imaging"], accepted_order: ["imaging:a"], blocked_order: ["omics:a"], artifact_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], omissions: ["modality:omics:required-but-not-admitted"], uncertainty: [], negative_evidence: ["modality:omics:a:state-contradicted-not-harmonized"], replay_identity: "c".repeat(64), object_digest: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["raw modality payloads remain institution-local; only typed digests and manifests cross sites"], omissions: ["modality:omics:required-but-not-admitted"], uncertainty: [], negative_evidence: ["modality:omics:a:state-contradicted-not-harmonized"], effect_receipts: ["exchange:permitted-harmonized-manifest:imaging:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateFederatedMultimodalIngestionReceipt(receipt)); assert.equal(federatedMultimodalIngestionReceiptDigest(receipt), federatedMultimodalIngestionReceiptDigest(receipt)); });
test("quality assurance preserves cross-study witness and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: QUALITY_ASSURANCE_FEATURE_ID, contract_version: QUALITY_ASSURANCE_CONTRACT_VERSION, request_id: "quality:assurance", workflow_id: "workflow:multi-study-qc", disposition: "partial", verdict: { verdict_id: "quality-verdict:quality:assurance", disposition: "partial", study_order: ["study:a", "study:b"], qualified_order: ["study:a"], blocked_order: ["study:b"], comparability_digest: "a".repeat(64), artifact_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], witness_order: ["study:study:b:quality-metric-not-pass"], omissions: [], uncertainty: [], negative_evidence: ["study:study:b:failed-or-unmeasured-quality"], replay_identity: "d".repeat(64), verdict_digest: "e".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["cross-study comparability and modality quality metrics are explicit gates"], omissions: [], uncertainty: [], negative_evidence: ["study:study:b:failed-or-unmeasured-quality"], effect_receipts: ["exchange:permitted-quality-manifest:study:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateQualityAssuranceReceipt(receipt)); assert.equal(qualityAssuranceReceiptDigest(receipt), qualityAssuranceReceiptDigest(receipt)); });
test("mechanism control preserves ranked competing portfolio", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: MECHANISM_CONTROL_FEATURE_ID, contract_version: MECHANISM_CONTROL_CONTRACT_VERSION, request_id: "mechanism:control", workflow_id: "workflow:mechanism-exploration", objective_id: "objective:organoid", disposition: "partial", portfolio: { portfolio_id: "mechanism-portfolio:mechanism:control", disposition: "partial", study_order: ["study:a", "study:b"], ranked_order: ["mechanism:b"], rank_score_order: [90], competing_order: ["mechanism:b"], blocked_order: ["mechanism:a"], comparability_digest: "a".repeat(64), evidence_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: ["mechanism:mechanism:a:cross-study-comparability-mismatch"], uncertainty: [], negative_evidence: ["mechanism:mechanism:a:comparability-not-admitted"], replay_identity: "d".repeat(64), portfolio_digest: "e".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["deterministic support-score ranking with mechanism-id tie break"], omissions: ["mechanism:mechanism:a:cross-study-comparability-mismatch"], uncertainty: [], negative_evidence: ["mechanism:mechanism:a:comparability-not-admitted"], effect_receipts: ["exchange:permitted-mechanism-summary:mechanism:b"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateMechanismControlReceipt(receipt)); assert.equal(mechanismControlReceiptDigest(receipt), mechanismControlReceiptDigest(receipt)); });

test("evidence workbench preserves stale alert and view-only effects", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVIDENCE_WORKBENCH_FEATURE_ID, contract_version: EVIDENCE_WORKBENCH_CONTRACT_VERSION, request_id: "evidence:workbench", workflow_id: "workflow:surveillance", study_id: "study:organoid", disposition: "partial", evidence: { set_id: "qualified-evidence:evidence:workbench", source_order: ["source:a", "source:b"], qualified_order: ["source:a"], alert_order: ["source:b:freshness-stale"], blocked_order: ["source:b"], evidence_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], omissions: ["source:b:freshness-not-current"], uncertainty: [], negative_evidence: [], replay_identity: "c".repeat(64), set_digest: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["stale and incomplete evidence remains researcher-visible"], omissions: ["source:b:freshness-not-current"], uncertainty: [], negative_evidence: [], effect_receipts: ["view:authorized-research-state:source:a", "view:authorized-research-state:source:b"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateEvidenceWorkbenchReceipt(receipt)); assert.equal(evidenceWorkbenchReceiptDigest(receipt), evidenceWorkbenchReceiptDigest(receipt)); });

test("analysis control preserves ranked digest-only portfolio", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ANALYSIS_CONTROL_FEATURE_ID, contract_version: ANALYSIS_CONTROL_CONTRACT_VERSION, request_id: "analysis:control", workflow_id: "workflow:analysis", objective_id: "objective:organoid", disposition: "partial", portfolio: { portfolio_id: "analysis-portfolio:analysis:control", disposition: "partial", candidate_order: ["candidate:a", "candidate:b"], admitted_order: ["candidate:a"], blocked_order: ["candidate:b"], rank_score_order: [90], class_order: ["causal"], result_order: ["a".repeat(64)], model_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], replay_identity: "d".repeat(64), portfolio_digest: "e".repeat(64), omissions: ["candidate:candidate:b:cross-study-comparability-missing"], uncertainty: [], negative_evidence: [], boundary: PRECLINICAL_BOUNDARY }, checks: ["comparability, policy, federation, locality, approval, and budget gates are explicit"], omissions: ["candidate:candidate:b:cross-study-comparability-missing"], uncertainty: [], negative_evidence: [], effect_receipts: ["exchange:digest-only-analysis-manifest:candidate:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateAnalysisControlReceipt(receipt)); assert.equal(analysisControlReceiptDigest(receipt), analysisControlReceiptDigest(receipt)); });

test("context assurance preserves stale fact and signed digest exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: CONTEXT_ASSURANCE_FEATURE_ID, contract_version: CONTEXT_ASSURANCE_CONTRACT_VERSION, request_id: "context:assurance", workflow_id: "workflow:context", question_id: "question:organoid", disposition: "partial", context: { context_id: "compiled-context:context:assurance", disposition: "partial", fact_order: ["fact:a", "fact:b"], selected_order: ["fact:a"], blocked_order: ["fact:b"], class_order: ["mechanism"], semantic_order: ["a".repeat(64)], evidence_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: [], uncertainty: ["fact:fact:b:stale-context"], negative_evidence: [], replay_identity: "d".repeat(64), context_digest: "e".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["freshness, comparability, policy, federation, approval, locality, and budget gates are explicit"], omissions: [], uncertainty: ["fact:fact:b:stale-context"], negative_evidence: [], effect_receipts: ["exchange:signed-context-digest:fact:a", "exchange:signed-context-digest:fact:b"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateContextAssuranceReceipt(receipt)); assert.equal(contextAssuranceReceiptDigest(receipt), contextAssuranceReceiptDigest(receipt)); });

test("bioworlds evaluation assurance retains null and negative outcomes", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: EVALUATION_ASSURANCE_BIOWORLDS_FEATURE_ID, contract_version: EVALUATION_ASSURANCE_BIOWORLDS_CONTRACT_VERSION, request_id: "evaluation:assurance", workflow_id: "workflow:evaluation", capability_id: "capability:mechanism", benchmark_id: "benchmark:organoid", disposition: "conditional", summary: { summary_id: "evaluation-summary:evaluation:assurance", disposition: "conditional", observation_order: ["observation:a", "observation:b"], admitted_order: ["observation:a"], blocked_order: ["observation:b"], metric_order: ["metric:effect"], site_order: ["site:a"], baseline_order: ["a".repeat(64)], artifact_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], positive_count: 0, null_count: 1, negative_count: 1, inconclusive_count: 0, omissions: [], uncertainty: ["site-floor:1-of-2-independent-sites"], negative_evidence: ["observation:observation:a:outcome-null-retained"], replay_identity: "d".repeat(64), summary_digest: "e".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["site floor, comparability, baseline, policy, federation, approval, locality, and budget gates are explicit"], omissions: [], uncertainty: ["site-floor:1-of-2-independent-sites"], negative_evidence: ["observation:observation:a:outcome-null-retained"], effect_receipts: ["exchange:evaluation-manifest-digest-only:observation:a", "exchange:evaluation-manifest-digest-only:observation:b"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBioworldsEvaluationAssuranceReceipt(receipt)); assert.equal(bioworldsEvaluationAssuranceReceiptDigest(receipt), bioworldsEvaluationAssuranceReceiptDigest(receipt)); });

test("biolang quality workbench preserves quarantine and local manifest effects", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: QUALITY_WORKBENCH_BIOLANG_FEATURE_ID, contract_version: QUALITY_WORKBENCH_BIOLANG_CONTRACT_VERSION, request_id: "quality:workbench", workflow_id: "workflow:quality", study_id: "study:organoid", disposition: "conditional", summary: { summary_id: "quality-summary:quality:workbench", disposition: "conditional", observation_order: ["observation:a", "observation:b"], qualified_order: ["observation:b"], warning_order: [], quarantined_order: ["observation:a"], unknown_order: [], batch_order: ["batch:a"], sample_order: ["sample:observation:a", "sample:observation:b"], metric_order: ["metric:signal"], artifact_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], passed_count: 1, warning_count: 0, quarantined_count: 1, unknown_count: 0, omissions: ["observation:observation:a:required-threshold-failed"], uncertainty: [], negative_evidence: ["observation:observation:a:contradicted-quality-evidence"], replay_identity: "c".repeat(64), summary_digest: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["quality, baseline, policy, protected closure, approval, locality, budget, and release-fraction gates are explicit"], omissions: ["observation:observation:a:required-threshold-failed"], uncertainty: [], negative_evidence: ["observation:observation:a:contradicted-quality-evidence"], effect_receipts: ["write:local-quality-manifest:batch:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBiolangQualityWorkbenchReceipt(receipt)); assert.equal(biolangQualityWorkbenchReceiptDigest(receipt), biolangQualityWorkbenchReceiptDigest(receipt)); });

test("biolang retrieval assurance preserves missing modality and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: RETRIEVAL_ASSURANCE_BIOLANG_FEATURE_ID, contract_version: RETRIEVAL_ASSURANCE_BIOLANG_CONTRACT_VERSION, request_id: "retrieval:assurance", workflow_id: "workflow:synthesis", query_id: "query:multimodal", disposition: "conditional", summary: { summary_id: "retrieval-assurance-summary:retrieval:assurance", disposition: "conditional", candidate_order: ["evidence:imaging", "evidence:omics"], ranked_order: ["evidence:imaging", "evidence:omics"], selected_order: ["evidence:imaging"], blocked_order: [], unknown_order: ["evidence:omics"], study_order: ["study:a"], modality_order: ["imaging", "omics"], artifact_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], selected_count: 1, blocked_count: 0, unknown_count: 1, omissions: ["modality:omics:required-but-not-admitted"], uncertainty: ["evidence:evidence:omics:state-unknown-not-admitted"], negative_evidence: ["evidence:evidence:imaging:negative-result-retained"], replay_identity: "c".repeat(64), summary_digest: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }, checks: ["artifact and provenance digests are required before synthesis admission"], omissions: ["modality:omics:required-but-not-admitted"], uncertainty: ["evidence:evidence:omics:state-unknown-not-admitted"], negative_evidence: ["evidence:evidence:imaging:negative-result-retained"], effect_receipts: ["block:unsafe-release", "evaluate:retrieval-assurance:evidence:imaging"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBiolangRetrievalAssuranceReceipt(receipt)); assert.equal(biolangRetrievalAssuranceReceiptDigest(receipt), biolangRetrievalAssuranceReceiptDigest(receipt)); });

test("CLI knowledge interoperability preserves unknown claim and artifact exchange gate", () => { const world = { schema_version: "aurora-research-contract/1.0", world_id: "typed-knowledge-world:knowledge:interop", target_schema: "typed-knowledge-world/6", claim_order: ["claim:a", "claim:b"], admitted_order: ["claim:a"], blocked_order: [], unknown_order: ["claim:b"], subject_order: ["subject:network"], predicate_order: ["expresses"], evidence_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], omissions: ["claim:claim:b:required-but-not-admitted"], uncertainty: ["claim:claim:b:state-unknown-not-admitted"], negative_evidence: ["claim:claim:a:negative-result-retained"], replay_identity: "c".repeat(64), world_digest: "d".repeat(64), boundary: PRECLINICAL_BOUNDARY }; const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: CLI_KNOWLEDGE_INTEROPERABILITY_FEATURE_ID, contract_version: CLI_KNOWLEDGE_INTEROPERABILITY_CONTRACT_VERSION, request_id: "knowledge:interop", workflow_id: "workflow:knowledge", disposition: "conditional", world, checks: ["claim identity and priority ordering are deterministic"], omissions: world.omissions, uncertainty: world.uncertainty, negative_evidence: world.negative_evidence, effect_receipts: ["block:knowledge-world-release", "exchange:permitted-artifacts:claim:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateCliKnowledgeInteroperabilityReceipt(receipt)); assert.equal(cliKnowledgeInteroperabilityReceiptDigest(receipt), cliKnowledgeInteroperabilityReceiptDigest(receipt)); });

test("lab evidence surveillance preserves unknown state and bounded tool gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: LAB_EVIDENCE_SURVEILLANCE_FEATURE_ID, contract_version: LAB_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION, feed_id: "feed:surveillance", workflow_id: "workflow:evidence", disposition: "partial", qualified_order: ["evidence:a"], blocked_order: [], unknown_order: ["evidence:b"], source_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], omissions: ["evidence:evidence:b:required-but-not-qualified"], uncertainty: ["evidence:evidence:b:state-unknown-not-qualified"], negative_evidence: ["evidence:evidence:a:negative-result-retained"], replay_identity: "c".repeat(64), effect_receipts: ["block:evidence-surveillance-release", "invoke:declared-tools:tool:local-index"], artifact: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateLabEvidenceSurveillanceReceipt(receipt)); assert.equal(labEvidenceSurveillanceReceiptDigest(receipt), labEvidenceSurveillanceReceiptDigest(receipt)); });

test("fiber mechanism assurance preserves unknown candidates and unsafe release gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: FIBER_MECHANISM_ASSURANCE_FEATURE_ID, contract_version: FIBER_MECHANISM_ASSURANCE_CONTRACT_VERSION, question_id: "question:mechanism", workflow_id: "workflow:mechanism", target_schema: "mechanism-portfolio/7", disposition: "conditional", ranked_order: ["candidate:a", "candidate:b"], admitted_order: ["candidate:a"], blocked_order: [], unknown_order: ["candidate:b"], mechanism_order: ["mechanism:a"], study_order: ["study:imaging", "study:omics"], modality_order: ["imaging", "omics"], artifact_order: ["a".repeat(64)], evidence_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: ["candidate:candidate:b:required-but-not-admitted"], uncertainty: ["candidate:candidate:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "d".repeat(64), effect_receipts: ["block:unsafe-release"], artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateFiberMechanismAssuranceReceipt(receipt)); assert.equal(fiberMechanismAssuranceReceiptDigest(receipt), fiberMechanismAssuranceReceiptDigest(receipt)); });

test("hubapi quality assurance preserves witness and unsafe release gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: HUBAPI_QUALITY_ASSURANCE_FEATURE_ID, contract_version: HUBAPI_QUALITY_ASSURANCE_CONTRACT_VERSION, object_id: "object:quality", study_id: "study:organoid", scope: "organoid:neural", target_schema: "quality-verdict/7", disposition: "conditional", ranked_metric_order: ["metric:a", "metric:b"], passed_order: ["metric:a"], failed_order: ["metric:b"], unknown_order: [], witness_order: ["metric:metric:a:value=900:threshold=700:pass"], modality_order: ["imaging", "omics"], artifact_order: ["a".repeat(64)], evidence_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: [], uncertainty: [], negative_evidence: ["metric:metric:b:below-threshold-negative-result"], replay_identity: "d".repeat(64), effect_receipts: ["block:unsafe-release"], artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateHubapiQualityAssuranceReceipt(receipt)); assert.equal(hubapiQualityAssuranceReceiptDigest(receipt), hubapiQualityAssuranceReceiptDigest(receipt)); });
test("registry resource discovery preserves stale omission and manifest exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_FEATURE_ID, contract_version: REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_CONTRACT_VERSION, request_id: "request:resources", federation_id: "federation:organoids", requester: "researcher:alice", scope: "organoid:neural", disposition: "partial", candidate_order: ["resource:a", "resource:b"], selected_order: ["resource:a"], omitted_order: ["resource:b"], semantic_order: ["a".repeat(64)], artifact_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], checks: ["candidate ranking is deterministic by trust then resource identity"], omissions: ["resource:resource:b:missing-modality"], uncertainty: ["resource:resource:b:stale-registry-epoch"], negative_evidence: [], replay_identity: "d".repeat(64), effect_receipts: ["exchange:signed-resource-manifest:resource:a"], federation_manifest: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateRegistryResourceDiscoveryAssuranceReceipt(receipt)); assert.equal(registryResourceDiscoveryAssuranceReceiptDigest(receipt), registryResourceDiscoveryAssuranceReceiptDigest(receipt)); });
test("services mechanism workbench preserves unknown candidates and local report effect", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: SERVICES_MECHANISM_WORKBENCH_FEATURE_ID, contract_version: SERVICES_MECHANISM_WORKBENCH_CONTRACT_VERSION, request_id: "request:mechanisms", workflow_id: "workflow:mechanism-batch", objective_id: "objective:organoid", target_schema: "mechanism-workbench/1", scope: "organoid:neural", disposition: "partial", ranked_order: ["candidate:a", "candidate:b"], admitted_order: ["candidate:a"], blocked_order: ["candidate:b"], unknown_order: ["candidate:b"], mechanism_order: ["mechanism:candidate:a"], study_order: ["study:imaging", "study:omics"], modality_order: ["imaging", "omics"], score_order: [900, 700], artifact_order: ["a".repeat(64)], evidence_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: ["candidate:candidate:b:budget-exhausted"], uncertainty: ["candidate:candidate:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "d".repeat(64), effect_receipts: ["write:local-mechanism-workbench:request:mechanisms"], artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateServicesMechanismWorkbenchReceipt(receipt)); assert.equal(servicesMechanismWorkbenchReceiptDigest(receipt), servicesMechanismWorkbenchReceiptDigest(receipt)); });
test("governance interpretation assurance preserves unknown and baseline gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: GOVERNANCE_INTERPRETATION_ASSURANCE_FEATURE_ID, contract_version: GOVERNANCE_INTERPRETATION_ASSURANCE_CONTRACT_VERSION, request_id: "request:interpretation", workflow_id: "workflow:visualization", objective_id: "objective:organoid", scope: "organoid:neural", disposition: "partial", ranked_order: ["interpretation:a", "interpretation:b"], admitted_order: ["interpretation:a"], blocked_order: ["interpretation:b"], unknown_order: ["interpretation:b"], result_order: ["result:a"], visualization_order: ["view:a"], support_order: [900, 700], semantic_order: ["a".repeat(64)], artifact_order: ["b".repeat(64)], evidence_order: ["c".repeat(64)], provenance_order: ["d".repeat(64)], baseline_order: ["e".repeat(64)], omissions: ["interpretation:interpretation:b:baseline-missing"], uncertainty: ["interpretation:interpretation:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "f".repeat(64), effect_receipts: ["evaluate:interpretation-assurance:request:interpretation"], artifact: { content_hash: "0".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateGovernanceInterpretationAssuranceReceipt(receipt)); assert.equal(governanceInterpretationAssuranceReceiptDigest(receipt), governanceInterpretationAssuranceReceiptDigest(receipt)); });
test("oracle ingestion control preserves missing modality and aggregate exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: ORACLE_INGESTION_CONTROL_FEATURE_ID, contract_version: ORACLE_INGESTION_CONTROL_CONTRACT_VERSION, request_id: "request:ingestion", workflow_id: "workflow:ingestion", federation_id: "federation:organoid", disposition: "partial", modality_order: ["imaging", "omics"], accepted_order: ["imaging"], blocked_order: ["omics"], unknown_order: ["omics"], study_order: ["study:organoid"], semantic_profile_order: ["ome-ngff/0.5"], artifact_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], omissions: ["modality:omics:required-but-not-admitted"], uncertainty: ["modality:omics:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "c".repeat(64), effect_receipts: ["exchange:aggregate-ingestion-manifest:request:ingestion"], aggregate_manifest: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateOracleIngestionControlReceipt(receipt)); assert.equal(oracleIngestionControlReceiptDigest(receipt), oracleIngestionControlReceiptDigest(receipt)); });
test("stewardship release workbench preserves unknown and signed manifest exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: STEWARDSHIP_RELEASE_WORKBENCH_FEATURE_ID, contract_version: STEWARDSHIP_RELEASE_WORKBENCH_CONTRACT_VERSION, request_id: "request:release", workflow_id: "workflow:publish", federation_id: "federation:commons", disposition: "partial", object_order: ["object:a", "object:b"], admitted_order: ["object:a"], blocked_order: ["object:b"], unknown_order: ["object:b"], origin_order: ["institution:a"], artifact_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], evidence_order: ["c".repeat(64)], release_order: ["d".repeat(64)], omissions: ["object:object:b:source-evidence-missing"], uncertainty: ["object:object:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "e".repeat(64), effect_receipts: ["exchange:signed-research-object-manifest:request:release"], federation_manifest: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateStewardshipReleaseWorkbenchReceipt(receipt)); assert.equal(stewardshipReleaseWorkbenchReceiptDigest(receipt), stewardshipReleaseWorkbenchReceiptDigest(receipt)); });
test("API analysis assurance preserves influence gap and digest-only exchange", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: API_ANALYSIS_ASSURANCE_FEATURE_ID, contract_version: API_ANALYSIS_ASSURANCE_CONTRACT_VERSION, request_id: "request:analysis", workflow_id: "workflow:analysis", question_id: "question:organoid", disposition: "partial", result_id: "qualified-analysis:request:analysis", estimand: "synaptic-density-delta", candidate_order: ["candidate:a", "candidate:b"], admitted_order: ["candidate:a"], blocked_order: ["candidate:b"], selected_candidate: "candidate:a", class_order: ["causal"], result_order: ["a".repeat(64)], model_order: ["b".repeat(64)], evidence_order: ["c".repeat(64)], provenance_order: ["d".repeat(64)], replay_identity: "e".repeat(64), benchmark_digest: "f".repeat(64), evidence_receipt_digest: "0".repeat(64), artifact: { content_hash: "1".repeat(64) }, checks: ["comparability, influence, policy, federation, locality, approval, and budget gates are explicit"], omissions: ["candidate:candidate:b:influence-evidence-missing"], uncertainty: [], negative_evidence: [], effect_receipts: ["exchange:digest-only-analysis-assurance:request:analysis"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateApiAnalysisAssuranceReceipt(receipt)); assert.equal(apiAnalysisAssuranceReceiptDigest(receipt), apiAnalysisAssuranceReceiptDigest(receipt)); });
test("store evidence operations preserves checkpoint and negative evidence", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: STORE_EVIDENCE_OPERATIONS_FEATURE_ID, contract_version: STORE_EVIDENCE_OPERATIONS_CONTRACT_VERSION, request_id: "request:evidence-ops", feed_id: "feed:surveillance", workflow_id: "workflow:evidence", federation_id: "federation:commons", disposition: "partial", alert_order: ["alert:a", "alert:b"], qualified_order: ["alert:a"], blocked_order: [], unknown_order: ["alert:b"], source_order: ["a".repeat(64)], provenance_order: ["b".repeat(64)], evidence_order: ["c".repeat(64)], checkpoint_id: "checkpoint:17", replay_identity: "d".repeat(64), telemetry_digest: "e".repeat(64), omissions: ["alert:alert:b:required-digest-missing"], uncertainty: ["alert:alert:b:state-unknown-not-qualified"], negative_evidence: ["alert:alert:a:negative-result-retained"], effect_receipts: ["checkpoint:evidence-operations:checkpoint:17", "exchange:permitted-evidence-summary:request:evidence-ops"], federation_manifest: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateStoreEvidenceOperationsReceipt(receipt)); assert.equal(storeEvidenceOperationsReceiptDigest(receipt), storeEvidenceOperationsReceiptDigest(receipt)); });
test("policy interoperability control preserves schema migration gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: POLICY_INTEROPERABILITY_CONTROL_FEATURE_ID, contract_version: POLICY_INTEROPERABILITY_CONTROL_CONTRACT_VERSION, request_id: "request:interop", workflow_id: "workflow:interop", federation_id: "federation:commons", disposition: "partial", offer_order: ["offer:a", "offer:b"], accepted_order: ["offer:a"], blocked_order: ["offer:b"], unknown_order: [], capability_order: ["capability:offer:a"], schema_order: ["research-capability/2"], input_order: ["a".repeat(64)], output_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], evidence_order: ["d".repeat(64)], migration_order: ["e".repeat(64)], replay_identity: "f".repeat(64), benchmark_digest: "0".repeat(64), omissions: ["offer:offer:b:schema-migration-required"], uncertainty: [], negative_evidence: [], effect_receipts: ["block:policy-interoperability-release:request:interop", "exchange:permitted-capability-summary:request:interop"], integration_artifact: { content_hash: "1".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validatePolicyInteroperabilityControlReceipt(receipt)); assert.equal(policyInteroperabilityControlReceiptDigest(receipt), policyInteroperabilityControlReceiptDigest(receipt)); });
test("safety mechanism workflow preserves unsafe action block", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: SAFETY_MECHANISM_WORKFLOW_FEATURE_ID, contract_version: SAFETY_MECHANISM_WORKFLOW_CONTRACT_VERSION, request_id: "request:mechanism-workflow", workflow_id: "workflow:mechanism", federation_id: "federation:commons", disposition: "unknown", candidate_order: ["candidate:a"], ranked_order: ["candidate:a"], admitted_order: [], blocked_order: [], unknown_order: ["candidate:a"], mechanism_order: [], evidence_order: [], provenance_order: [], action_order: [], replay_identity: "a".repeat(64), benchmark_digest: "b".repeat(64), checkpoint_id: "checkpoint:mechanism:7", checkpoint_digest: "c".repeat(64), omissions: ["candidate:candidate:a:requested-action-not-permitted"], uncertainty: ["candidate:candidate:a:state-unknown-not-admitted"], negative_evidence: [], effect_receipts: ["block:safety-workflow:request:mechanism-workflow", "checkpoint:mechanism-workflow:checkpoint:mechanism:7"], workflow_artifact: { content_hash: "d".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateSafetyMechanismWorkflowReceipt(receipt)); assert.equal(safetyMechanismWorkflowReceiptDigest(receipt), safetyMechanismWorkflowReceiptDigest(receipt)); });
test("hubapi multimodal interpretation preserves comparability and locality", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID, contract_version: HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION, request_id: "request:interpretation", workflow_id: "workflow:multimodal", objective_id: "objective:organoid", scope: "organoid:neural", disposition: "partial", ranked_order: ["interpretation:a", "interpretation:b"], admitted_order: ["interpretation:a"], blocked_order: ["interpretation:b"], unknown_order: ["interpretation:b"], result_order: ["result:a"], visualization_order: ["visualization:a"], study_order: ["study:a", "study:b"], modality_order: ["imaging", "transcriptomics"], support_order: [900, 700], semantic_order: ["a".repeat(64)], artifact_order: ["b".repeat(64)], evidence_order: ["c".repeat(64)], provenance_order: ["d".repeat(64)], comparability_order: ["e".repeat(64)], baseline_order: ["f".repeat(64)], omissions: ["interpretation:interpretation:b:modality-missing"], uncertainty: ["interpretation:interpretation:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "0".repeat(64), benchmark_digest: "1".repeat(64), effect_receipts: ["evaluate:interpretation-assurance:request:interpretation"], artifact: { content_hash: "2".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateHubapiMultimodalInterpretationAssuranceReceipt(receipt)); assert.equal(hubapiMultimodalInterpretationAssuranceReceiptDigest(receipt), hubapiMultimodalInterpretationAssuranceReceiptDigest(receipt)); });
test("biolang publication copilot preserves tool and replay gates", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: BIOLANG_PUBLICATION_COPILOT_FEATURE_ID, contract_version: BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION, request_id: "request:publication", workflow_id: "workflow:release", scope: "organoid:neural", disposition: "partial", ranked_order: ["release:a", "release:b"], admitted_order: ["release:a"], blocked_order: ["release:b"], unknown_order: ["release:b"], release_order: ["release:a"], artifact_order: ["artifact:a"], evidence_order: ["evidence:a"], tool_invocation_order: ["tool:ro-crate-pack"], provenance_order: ["a".repeat(64)], replay_order: ["b".repeat(64)], benchmark_order: ["c".repeat(64)], omissions: ["release:release:b:benchmark-missing"], uncertainty: ["release:release:b:replay-mismatch"], negative_evidence: [], replay_identity: "d".repeat(64), benchmark_digest: "e".repeat(64), effect_receipts: ["invoke:declared-tools:request:publication"], objects: [{ run_id: "run:a", release_id: "release:a", artifact_ids: ["artifact:a"], evidence_receipt_ids: ["evidence:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }], publication_artifact: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBiolangPublicationCopilotReceipt(receipt)); assert.equal(biolangPublicationCopilotReceiptDigest(receipt), biolangPublicationCopilotReceiptDigest(receipt)); });
test("API release assurance preserves benchmark and unsafe release gate", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: API_RELEASE_ASSURANCE_FEATURE_ID, contract_version: API_RELEASE_ASSURANCE_CONTRACT_VERSION, request_id: "request:release", workflow_id: "workflow:publication", scope: "organoid:neural", disposition: "partial", candidate_order: ["release:a", "release:b"], admitted_order: ["release:a"], blocked_order: ["release:b"], unknown_order: ["release:b"], release_order: ["release:a"], artifact_order: ["artifact:a"], evidence_order: ["evidence:a"], provenance_order: ["a".repeat(64)], replay_order: ["b".repeat(64)], benchmark_order: ["c".repeat(64)], omissions: ["release:release:b:benchmark-missing"], uncertainty: ["release:release:b:replay-mismatch"], negative_evidence: [], replay_identity: "d".repeat(64), benchmark_digest: "e".repeat(64), effect_receipts: ["evaluate:release-assurance:request:release"], objects: [{ run_id: "run:a", release_id: "release:a", artifact_ids: ["artifact:a"], evidence_receipt_ids: ["evidence:a"], raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }], artifact: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateApiReleaseAssuranceReceipt(receipt)); assert.equal(apiReleaseAssuranceReceiptDigest(receipt), apiReleaseAssuranceReceiptDigest(receipt)); });
test("bioevalx federation gateway preserves endpoint and protocol gates", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID, contract_version: BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION, request_id: "request:federation", workflow_id: "workflow:publish", federation_id: "federation:commons", endpoint: "https://hub.example/research", protocol: "mcp/2025-06-18", disposition: "partial", candidate_order: ["release:a", "release:b"], admitted_order: ["release:a"], blocked_order: ["release:b"], unknown_order: ["release:b"], release_order: ["release:a"], artifact_order: ["artifact:a"], evidence_order: ["evidence:a"], provenance_order: ["a".repeat(64)], replay_order: ["b".repeat(64)], benchmark_order: ["c".repeat(64)], omissions: ["request:protocol-not-pinned"], uncertainty: ["release:release:b:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "d".repeat(64), benchmark_digest: "e".repeat(64), effect_receipts: ["exchange:permitted-artifacts:request:federation"], objects: [{ run_id: "run:a", release_id: "release:a", artifact_ids: ["artifact:a"], evidence_receipt_ids: ["evidence:a"], endpoint: "https://hub.example/research", protocol: "mcp/2025-06-18", raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }], federation_artifact: { content_hash: "f".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBioevalxFederationGatewayReceipt(receipt)); assert.equal(bioevalxFederationGatewayReceiptDigest(receipt), bioevalxFederationGatewayReceiptDigest(receipt)); });
test("section interpretation assurance keeps protected closure denial blocked", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID, contract_version: SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION, request_id: "request:section", workflow_id: "workflow:interpretation", federation_id: "federation:commons", scope: "organoid:neural", disposition: "blocked", candidate_order: ["interpretation:a"], admitted_order: [], blocked_order: ["interpretation:a"], unknown_order: [], result_order: [], visualization_order: [], study_order: [], modality_order: [], support_order: [900], semantic_order: [], artifact_order: [], evidence_order: [], provenance_order: [], comparability_order: [], omissions: ["request:protected-closure-incomplete"], uncertainty: [], negative_evidence: ["request:policy-denied"], replay_identity: "a".repeat(64), benchmark_digest: null, effect_receipts: ["block:unsafe-release"], interpretations: [], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateSectionInterpretationAssuranceReceipt(receipt)); assert.equal(sectionInterpretationAssuranceReceiptDigest(receipt), sectionInterpretationAssuranceReceiptDigest(receipt)); });
test("ops retrieval assurance keeps unknown evidence blocked", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: OPS_RETRIEVAL_ASSURANCE_FEATURE_ID, contract_version: OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION, request_id: "request:retrieval", study_id: "study:organoid", scope: "organoid:neural", disposition: "unknown", candidate_order: ["evidence:a"], admitted_order: [], blocked_order: ["evidence:a"], unknown_order: ["evidence:a"], source_order: [], modality_order: [], support_order: [900], semantic_order: [], artifact_order: [], provenance_order: [], omissions: ["evidence:evidence:a:benchmark-missing"], uncertainty: ["evidence:evidence:a:state-unknown-not-admitted"], negative_evidence: [], replay_identity: "a".repeat(64), benchmark_digest: null, effect_receipts: ["block:unsafe-release"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateOpsRetrievalAssuranceReceipt(receipt)); assert.equal(opsRetrievalAssuranceReceiptDigest(receipt), opsRetrievalAssuranceReceiptDigest(receipt)); });
test("conformance knowledge world keeps comparability omission explicit", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_FEATURE_ID, contract_version: CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_CONTRACT_VERSION, request_id: "request:world", workflow_id: "workflow:knowledge", scope: "organoid:neural", disposition: "unknown", candidate_order: ["claim:a"], admitted_order: [], blocked_order: ["claim:a"], unknown_order: [], predicate_order: [], study_order: [], modality_order: [], support_order: [900], semantic_order: [], artifact_order: [], evidence_order: [], provenance_order: [], comparability_order: [], omissions: ["claim:claim:a:comparability-missing"], uncertainty: [], negative_evidence: [], replay_identity: "a".repeat(64), benchmark_digest: "b".repeat(64), effect_receipts: ["block:unsafe-release"], artifact: { content_hash: "c".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateConformanceKnowledgeWorldAssuranceReceipt(receipt)); assert.equal(conformanceKnowledgeWorldAssuranceReceiptDigest(receipt), conformanceKnowledgeWorldAssuranceReceiptDigest(receipt)); });
test("brain evidence surveillance keeps empty feed fail closed", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: BRAIN_EVIDENCE_SURVEILLANCE_FEATURE_ID, contract_version: BRAIN_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION, request_id: "request:evidence", study_id: "study:organoid", scope: "organoid:neural", disposition: "unknown", candidate_order: ["evidence:a"], qualified_order: [], blocked_order: ["evidence:a"], unknown_order: ["evidence:a"], source_order: [], modality_order: [], relevance_order: [900], semantic_order: [], artifact_order: [], provenance_order: [], omissions: [], uncertainty: ["evidence:evidence:a:state-unknown-not-qualified"], negative_evidence: [], replay_identity: "a".repeat(64), effect_receipts: ["block:unsafe-release"], artifact: { content_hash: "b".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBrainEvidenceSurveillanceReceipt(receipt)); assert.equal(brainEvidenceSurveillanceReceiptDigest(receipt), brainEvidenceSurveillanceReceiptDigest(receipt)); });
test("brain multimodal surveillance keeps missing modality explicit", () => { const receipt = { schema_version: "aurora-research-contract/1.0", feature_id: BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID, contract_version: BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION, request_id: "request:multimodal", study_order: ["study:a", "study:b"], scope: "organoid:neural", disposition: "partial", candidate_order: ["evidence:a"], qualified_order: ["evidence:a"], blocked_order: [], unknown_order: [], source_order: ["source:a"], modality_order: ["imaging"], relevance_order: [900], semantic_order: ["a".repeat(64)], artifact_order: ["b".repeat(64)], provenance_order: ["c".repeat(64)], omissions: ["modality:transcriptomics:required-coverage-missing"], uncertainty: [], negative_evidence: [], replay_identity: "d".repeat(64), effect_receipts: ["read:local-research-artifacts:request:multimodal"], artifact: { content_hash: "e".repeat(64) }, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY }; assert.doesNotThrow(() => validateBrainMultimodalEvidenceSurveillanceReceipt(receipt)); assert.equal(brainMultimodalEvidenceSurveillanceReceiptDigest(receipt), brainMultimodalEvidenceSurveillanceReceiptDigest(receipt)); });
