//! MCP-facing validation for the shared research contracts.
//!
//! The MCP transport accepts JSON, but it does not own scientific semantics. These helpers perform
//! the same schema/boundary/policy checks as the Rust service before a tool result is returned.

use crate::evolution_assurance::{
    assure_bounded_evolution, EvolutionAssuranceError, EvolutionAssuranceReceipt,
    EvolutionAssuranceRequest, FEATURE_ID as EVOLUTION_ASSURANCE_FEATURE_ID,
};
use crate::resource_discovery_contract::{
    compile_resource_discovery_contract_v2, ResourceDiscoveryContractRequest,
    ResourceDiscoveryContractResponse, FEATURE_ID as RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
};
use bioprism_adapter::{
    admit_bounded_evolution, BoundedEvolutionError, BoundedEvolutionReceipt,
    BoundedEvolutionRequest, BOUNDED_EVOLUTION_FEATURE_ID,
};
use bioprism_adapter::{
    admit_computational_execution, ComputationalExecutionReceipt, ComputationalExecutionRequest,
    EXECUTION_CONTROL_FEATURE_ID,
};
use bioprism_adapter::{
    admit_federated_commons, FederatedCommonsError, FederatedCommonsReceipt,
    FederatedCommonsRequest, FEDERATED_COMMONS_FEATURE_ID,
};
use bioprism_adapter::{
    admit_policy_action, ActionAndAuthority, PolicyGatewayReceipt, POLICY_GATEWAY_FEATURE_ID,
};
use bioprism_adapter::{
    assure_context_compilation as assure_adapter_context_compilation, ContextCompilationReceipt,
    ContextCompilationRequest, CONTEXT_COMPILATION_FEATURE_ID,
};
use bioprism_adapter::{
    assure_evaluation_run, CapabilityRun, EvaluationAssuranceError, EvaluationAssuranceReceipt,
    EVALUATION_ASSURANCE_FEATURE_ID,
};
use bioprism_adapter::{
    assure_interpretation, EvidenceBackedResult, InterpretationAssuranceReceipt,
    INTERPRETATION_ASSURANCE_FEATURE_ID,
};
use bioprism_adapter::{
    assure_provenance, ArtifactAndDerivation, ProvenanceAssuranceError, SignedProvenanceEnvelope,
    PROVENANCE_ASSURANCE_FEATURE_ID,
};
use bioprism_adapter::{
    assure_release, ReleaseAssuranceReceipt, ValidatedResearchRun as AdapterValidatedResearchRun,
    RELEASE_ASSURANCE_FEATURE_ID,
};
use bioprism_adapter::{
    assure_replication, ReplicationAssuranceReceipt, ReplicationAssuranceRequest,
    REPLICATION_ASSURANCE_FEATURE_ID,
};
use bioprism_adapter::{
    close_adapter_limitations, AdapterClosureReceipt, LimitationClosureError,
    LimitationClosureRequest, LIMITATION_CLOSURE_FEATURE_ID,
};
use bioprism_adapter::{
    compile_adapter_capability_manifest, AdapterCapabilityManifest, AdapterContractInput,
    ContractFrontierError, CONTRACT_FRONTIER_FEATURE_ID,
};
use bioprism_adapter::{
    compile_evidence_synthesis, EvidenceSynthesisRequest, RetrievalSynthesisReceipt,
    RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
use bioprism_adapter::{
    run_local_retrieval_synthesis_inference_engine,
    LocalRetrievalSynthesisInferenceEngineReceipt,
    LocalRetrievalSynthesisInferenceEngineRequest,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID,
};
use bioprism_adapter::{
    compile_experiment_design, ExperimentDesignReceipt, FederatedExperimentDesignRequest,
    EXPERIMENT_DESIGN_CONTROL_FEATURE_ID,
};
use bioprism_adapter::{
    compile_research_workbench, InteractiveResearchWorkspace, ResearchWorkbenchError,
    ResearchWorkspaceState, RESEARCH_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    discover_resources as discover_adapter_resources,
    ResourceCandidate as AdapterResourceCandidate, ResourceNeed as AdapterResourceNeed,
    ResourceWorkbenchReceipt,
    RESOURCE_WORKBENCH_FEATURE_ID as ADAPTER_RESOURCE_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    evaluate_adapter_semantic_parity, AdapterSemanticParityReceipt, AdapterSemanticParityRequest,
    SemanticParityError, ADAPTER_SEMANTIC_PARITY_FEATURE_ID,
};
use bioprism_adapter::{
    evaluate_quality_drift, harmonize_multimodal, HarmonizedResearchObject,
    MultimodalHarmonizationRequest, QualityDriftReceipt, QualityDriftRequest,
    MULTIMODAL_HARMONIZATION_FEATURE_ID, QUALITY_DRIFT_FEATURE_ID,
};
use bioprism_adapter::{
    evaluate_quality_envelope, QualityEnvelopeReceipt, QualityEnvelopeRequest,
    QUALITY_ENVELOPE_FEATURE_ID,
};
use bioprism_adapter::{
    infer_adapter_dependency_composition, AdapterCompositionReceipt, AdapterCompositionRequest,
    DependencyCompositionError, DEPENDENCY_COMPOSITION_FEATURE_ID,
};
use bioprism_adapter::{
    integrate_instrument_mesh, InstrumentActionRequest, InstrumentCapability,
    InstrumentMeshReceipt, INSTRUMENT_MESH_FEATURE_ID,
};
use bioprism_adapter::{
    negotiate_capability, CanonicalCapabilityOutput, TypedCapabilityInput,
    DETERMINISM_GATEWAY_FEATURE_ID,
};
use bioprism_adapter::{
    negotiate_interoperability, InteroperabilityGatewayError, InteroperabilityRequest,
    NegotiatedIntegration, INTEROPERABILITY_GATEWAY_FEATURE_ID,
};
use bioprism_adapter::{
    operate_mechanism_control_plane, MechanismControlPlaneReceipt, MechanismControlPlaneRequest,
    MECHANISM_CONTROL_PLANE_FEATURE_ID,
};
use bioprism_adapter::{
    plan_adapter_scale_frontier, ScaleFrontierError, ScaleFrontierReceipt, ScaleFrontierRequest,
    ADAPTER_SCALE_FRONTIER_FEATURE_ID,
};
use bioprism_adapter::{
    plan_reliable_capability, CapabilityWorkload, ReliabilityCopilotError,
    ReliableCapabilityResult, RELIABILITY_COPILOT_FEATURE_ID,
};
use bioprism_adapter::{
    qualify_analysis_portfolio, AnalysisPortfolioReceipt, AnalysisPortfolioRequest,
    ANALYSIS_PORTFOLIO_FEATURE_ID,
};
use bioprism_adapter::{
    recover_adversarial_events, AdversarialRecoveryError, AdversarialRecoveryReceipt,
    AdversarialRecoveryRequest, ADVERSARIAL_RECOVERY_FEATURE_ID,
};
use bioprism_adapter::{
    run_evidence_surveillance, EvidenceFeedRequest, EvidenceSurveillanceReceipt,
    EVIDENCE_SURVEILLANCE_FEATURE_ID,
};
use bioprism_adapter::{
    run_local_evidence_surveillance_research_copilot,
    LocalEvidenceSurveillanceResearchCopilotReceipt,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
};
use bioprism_adapter::{
    run_multimodal_evidence_surveillance_research_copilot,
    MultimodalEvidenceSurveillanceResearchCopilotReceipt,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
};
use bioprism_adapter::{
    run_ingestion_gateway, IngestionGatewayReceipt, IngestionGatewayRequest,
    INGESTION_GATEWAY_FEATURE_ID,
};
use bioprism_adapter::{
    run_throughput_evidence_surveillance_research_copilot,
    ThroughputEvidenceSurveillanceResearchCopilotReceipt,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
};
use bioprism_adapter::{
    run_knowledge_workflow, ClaimsWorkflowRequest, KnowledgeWorkflowReceipt,
    KNOWLEDGE_WORKFLOW_FEATURE_ID,
};
use bioprism_adapter::{
    run_federated_continual_evidence_surveillance_research_copilot,
    FederatedContinualEvidenceSurveillanceResearchCopilotReceipt,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
};
use bioprism_adapter::{
    schedule_federation_workflow, FederationRequest, FederationWorkflowReceipt,
    FEDERATION_WORKFLOW_FEATURE_ID,
};
use bioprism_adapter::{
    schedule_local_evidence_surveillance_workflow,
    LocalEvidenceSurveillanceWorkflowReceipt,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
};
use bioprism_adapter::{
    schedule_multimodal_evidence_surveillance_workflow,
    MultimodalEvidenceSurveillanceWorkflowReceipt,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
};
use bioprism_adapter::{
    schedule_throughput_evidence_surveillance_workflow,
    ThroughputEvidenceSurveillanceWorkflowReceipt,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
};
use bioprism_adapter::{
    schedule_federated_continual_evidence_surveillance_workflow,
    FederatedContinualEvidenceSurveillanceWorkflowReceipt,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
};
use bioprism_adapter::{
    render_local_evidence_surveillance_research_workbench,
    LocalEvidenceSurveillanceResearchWorkbenchReceipt,
    LocalEvidenceSurveillanceResearchWorkbenchRequest,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    render_multimodal_evidence_surveillance_research_workbench,
    MultimodalEvidenceSurveillanceResearchWorkbenchReceipt,
    MultimodalEvidenceSurveillanceResearchWorkbenchRequest,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    render_throughput_evidence_surveillance_research_workbench,
    ThroughputEvidenceSurveillanceResearchWorkbenchReceipt,
    ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    render_federated_continual_evidence_surveillance_research_workbench,
    FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt,
    FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    simulate_protocol_draft, ProtocolDraft, ProtocolSimulationReceipt,
    PROTOCOL_SIMULATION_FEATURE_ID,
};
use bioprism_atlashub::{
    synthesize_federated_continuum, FederatedContinualRetrievalReceipt,
    FederatedContinualRetrievalRequest, FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID,
};
use bioprism_devplat::{
    assure_context_compilation, ContextCompilationAssuranceReceipt,
    ContextCompilationAssuranceRequest, CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
use bioprism_evalengine::{
    compile_evaluation_card, evaluate_federated_evaluation, evaluate_multimodal_replication,
    qualify_analysis, AnalysisQualificationRequest, EvaluationCardReceipt, EvaluationCardRequest,
    FederatedEvaluationReceipt, FederatedEvaluationRequest, MultimodalReplicationReport,
    MultimodalReplicationRequest, QualifiedAnalysisResult, ANALYSIS_QUALIFICATION_FEATURE_ID,
    EVALUATION_OBSERVABILITY_FEATURE_ID, FEDERATED_EVALUATION_FEATURE_ID,
    MULTIMODAL_REPLICATION_FEATURE_ID,
};
use bioprism_fiber::{
    admit_mechanism_gateway, MechanismGatewayReceipt, MechanismGatewayRequest,
    MECHANISM_GATEWAY_FEATURE_ID,
};
use bioprism_fiber::{
    assure_federated_retrieval, discover_resources, FederatedRetrievalAssuranceReceipt,
    FederatedRetrievalAssuranceRequest, QualifiedResourceSet,
    ResourceCandidate as FiberResourceCandidate, ResourceNeed as FiberResourceNeed,
    FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
    RESOURCE_WORKBENCH_FEATURE_ID as FIBER_RESOURCE_WORKBENCH_FEATURE_ID,
};
use bioprism_foundation::{EvidenceReceipt, PolicyReceipt};
use bioprism_governance::{
    compile_signed_research_object, SignedResearchObject, ValidatedResearchRun,
    RESEARCH_RELEASE_CONTRACT_FEATURE_ID,
};
use bioprism_lab::{
    evaluate_design_frontier, evaluate_semantic_parity, instrument_preflight,
    simulate_protocol_matrix, DesignFrontierReceipt, DesignFrontierRequest,
    InstrumentPreflightReceipt, InstrumentPreflightRequest, LabSemanticParityReceipt,
    LabSemanticParityRequest, ProtocolMatrixReceipt, ProtocolMatrixRequest,
    DESIGN_FRONTIER_FEATURE_ID, INSTRUMENT_PREFLIGHT_FEATURE_ID, PROTOCOL_MATRIX_FEATURE_ID,
    SEMANTIC_PARITY_FEATURE_ID,
};
use bioprism_lens::{
    assure_federated_lens, FederatedLensAssuranceReceipt, FederatedLensAssuranceRequest,
    FEDERATED_LENS_ASSURANCE_FEATURE_ID,
};
use bioprism_obligation::{
    assess_release_harness, ReleaseHarnessReceipt, ReleaseHarnessRequest,
    RELEASE_HARNESS_FEATURE_ID,
};
use bioprism_ops::{
    assure_knowledge_representation, KnowledgeRepresentationAssuranceReceipt,
    KnowledgeRepresentationAssuranceRequest, KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
};
use bioprism_policy::{
    admit_autonomy_batch, BatchAdmissionReceipt, BatchAdmissionRequest, AUTONOMY_BATCH_FEATURE_ID,
};
use bioprism_policy::{
    assess_protocol_assurance, ProtocolAssuranceReceipt, ProtocolAssuranceRequest,
    PROTOCOL_ASSURANCE_FEATURE_ID,
};
use bioprism_routing::{
    assure_federated_multimodal, FederatedMultimodalAssuranceReceipt,
    FederatedMultimodalAssuranceRequest, FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID,
};
use bioprism_runtime::{
    execute_workflow, execute_workflow_batch, WorkflowBatchReceipt, WorkflowBatchRequest,
    WorkflowExecutionReceipt, WorkflowExecutionRequest, WORKFLOW_BATCH_FEATURE_ID,
    WORKFLOW_EXECUTION_FEATURE_ID,
};
use bioprism_services::{
    ResearchReleaseBatchReceipt, ResearchReleaseReceipt, RESEARCH_RELEASE_BATCH_FEATURE_ID,
    RESEARCH_RELEASE_FEATURE_ID,
};
use bioprism_store::{
    admit_federated_knowledge, FederatedKnowledgeGatewayReceipt, FederatedKnowledgeGatewayRequest,
    FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID,
};
use bioprism_weave::{
    operate_resource_control_plane, ResourceControlPlaneReceipt, ResourceControlPlaneRequest,
    RESOURCE_CONTROL_PLANE_FEATURE_ID,
};
use bioprism_weavelang::{
    assure_weavelang_release, WeaveLangReleaseAssuranceReceipt, WeaveLangReleaseAssuranceRequest,
    WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID,
};
use serde_json::Value;

/// Stable MCP tool name reserved for the evidence-to-typed-knowledge vertical.
pub const RESEARCH_COMPILE_TOOL: &str = "aurora_research_compile_evidence";
pub const WORKFLOW_EXECUTION_TOOL: &str = "runtime_workflow_execute";
pub const EVALUATION_OBSERVABILITY_TOOL: &str = "evaluation_observability_card";
pub const FEDERATED_EVALUATION_TOOL: &str = "federated_evaluation_consensus";
pub const RESEARCH_RELEASE_VALIDATE_TOOL: &str = "research_release_validate";
pub const RESEARCH_RELEASE_BATCH_VALIDATE_TOOL: &str = "research_release_batch_validate";
pub const INSTRUMENT_PREFLIGHT_TOOL: &str = "instrument_preflight";
pub const MULTIMODAL_HARMONIZATION_TOOL: &str = "multimodal_harmonize";
pub const ANALYSIS_QUALIFICATION_TOOL: &str = "analysis_qualify";
pub const PROTOCOL_MATRIX_TOOL: &str = "protocol_matrix_simulate";
pub const MULTIMODAL_REPLICATION_TOOL: &str = "multimodal_replication_evaluate";
pub const QUALITY_DRIFT_TOOL: &str = "quality_drift_evaluate";
pub const DESIGN_FRONTIER_TOOL: &str = "design_frontier_evaluate";
pub const AUTONOMY_BATCH_TOOL: &str = "autonomy_batch_admit";
pub const WORKFLOW_BATCH_TOOL: &str = "workflow_batch_execute";
pub const RESOURCE_WORKBENCH_TOOL: &str = "resource_workbench_discover";
pub const RESOURCE_DISCOVERY_CONTRACT_TOOL: &str = "resource_discovery_contract_v2";
pub const GOVERNANCE_RESEARCH_RELEASE_TOOL: &str = "governance_research_release_compile";
pub const RELEASE_ASSURANCE_HARNESS_TOOL: &str = "release_assurance_harness";
pub const PROTOCOL_ASSURANCE_TOOL: &str = "protocol_assurance_harness";
pub const FEDERATED_MULTIMODAL_ASSURANCE_TOOL: &str = "federated_multimodal_assurance";
pub const FEDERATED_KNOWLEDGE_GATEWAY_TOOL: &str = "federated_knowledge_gateway";
pub const FEDERATED_LENS_ASSURANCE_TOOL: &str = "federated_lens_assurance";
pub const SEMANTIC_PARITY_TOOL: &str = "lab_semantic_parity";
pub const FEDERATED_RETRIEVAL_ASSURANCE_TOOL: &str = "federated_retrieval_assurance";
pub const FEDERATED_CONTINUAL_RETRIEVAL_TOOL: &str = "federated_continual_retrieval_copilot";
pub const CONTEXT_COMPILATION_ASSURANCE_TOOL: &str = "federated_context_compilation_assurance";
pub const KNOWLEDGE_REPRESENTATION_ASSURANCE_TOOL: &str =
    "federated_knowledge_representation_assurance";
pub const RESOURCE_CONTROL_PLANE_TOOL: &str = "federated_resource_control_plane";
pub const WEAVELANG_RELEASE_ASSURANCE_TOOL: &str = "weavelang_release_assurance";
pub const MECHANISM_CONTROL_PLANE_TOOL: &str = "federated_mechanism_control_plane";
pub const MECHANISM_GATEWAY_TOOL: &str = "federated_mechanism_gateway";
pub const EVIDENCE_SURVEILLANCE_TOOL: &str = "evidence_surveillance_copilot";
pub const ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL: &str = "adapter_local_evidence_surveillance_research_copilot";
pub const ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL: &str = "adapter_multimodal_evidence_surveillance_research_copilot";
pub const ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL: &str = "adapter_throughput_evidence_surveillance_research_copilot";
pub const ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL: &str = "adapter_federated_continual_evidence_surveillance_research_copilot";
pub const ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL: &str = "adapter_local_evidence_surveillance_workflow_fabric";
pub const ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL: &str = "adapter_multimodal_evidence_surveillance_workflow_fabric";
pub const ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL: &str = "adapter_throughput_evidence_surveillance_workflow_fabric";
pub const ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL: &str = "adapter_federated_continual_evidence_surveillance_workflow_fabric";
pub const ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_TOOL: &str = "adapter_local_evidence_surveillance_research_workbench";
pub const ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_TOOL: &str = "adapter_multimodal_evidence_surveillance_research_workbench";
pub const ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_TOOL: &str = "adapter_throughput_evidence_surveillance_research_workbench";
pub const ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_TOOL: &str = "adapter_federated_continual_evidence_surveillance_research_workbench";
pub const RETRIEVAL_SYNTHESIS_TOOL: &str = "multimodal_retrieval_synthesis";
pub const ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_TOOL: &str = "adapter_local_retrieval_synthesis_inference_engine";
pub const ADAPTER_CONTEXT_COMPILATION_TOOL: &str = "adapter_context_compilation_assurance";
pub const KNOWLEDGE_WORKFLOW_TOOL: &str = "multimodal_knowledge_workflow";
pub const ADAPTER_RESOURCE_WORKBENCH_TOOL: &str = "adapter_resource_workbench";
pub const INGESTION_GATEWAY_TOOL: &str = "adapter_ingestion_gateway";
pub const QUALITY_ENVELOPE_TOOL: &str = "adapter_quality_envelope";
pub const EXPERIMENT_DESIGN_CONTROL_TOOL: &str = "adapter_experiment_design_control";
pub const PROTOCOL_SIMULATION_TOOL: &str = "adapter_protocol_simulation";
pub const INSTRUMENT_MESH_TOOL: &str = "adapter_instrument_mesh";
pub const EXECUTION_CONTROL_TOOL: &str = "adapter_execution_control";
pub const ANALYSIS_PORTFOLIO_TOOL: &str = "adapter_analysis_portfolio";
pub const INTERPRETATION_ASSURANCE_TOOL: &str = "adapter_interpretation_assurance";
pub const REPLICATION_ASSURANCE_TOOL: &str = "adapter_replication_assurance";
pub const RELEASE_ASSURANCE_TOOL: &str = "adapter_release_assurance";
pub const DETERMINISM_GATEWAY_TOOL: &str = "adapter_determinism_gateway";
pub const PROVENANCE_ASSURANCE_TOOL: &str = "adapter_provenance_assurance";
pub const POLICY_GATEWAY_TOOL: &str = "adapter_policy_gateway";
pub const FEDERATION_WORKFLOW_TOOL: &str = "adapter_federation_workflow";
pub const RELIABILITY_COPILOT_TOOL: &str = "adapter_reliability_copilot";
pub const INTEROPERABILITY_GATEWAY_TOOL: &str = "adapter_interoperability_gateway";
pub const EVALUATION_ASSURANCE_TOOL: &str = "adapter_evaluation_assurance";
pub const RESEARCH_WORKBENCH_TOOL: &str = "adapter_research_workbench";
pub const CONTRACT_FRONTIER_TOOL: &str = "adapter_contract_frontier";
pub const LIMITATION_CLOSURE_TOOL: &str = "adapter_limitation_closure";
pub const DEPENDENCY_COMPOSITION_TOOL: &str = "adapter_dependency_composition";
pub const ADAPTER_SEMANTIC_PARITY_TOOL: &str = "adapter_semantic_parity";
pub const ADAPTER_SCALE_FRONTIER_TOOL: &str = "adapter_scale_frontier";
pub const ADVERSARIAL_RECOVERY_TOOL: &str = "adapter_adversarial_recovery";
pub const FEDERATED_COMMONS_TOOL: &str = "adapter_federated_commons";
pub const BOUNDED_EVOLUTION_TOOL: &str = "adapter_bounded_evolution";
pub const RESEARCH_CONTRACT_SCHEMA_VERSION: &str =
    bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION;

pub fn validate_policy_receipt_json(value: &Value) -> Result<PolicyReceipt, String> {
    let receipt: PolicyReceipt =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    receipt.validate().map_err(|error| error.to_string())?;
    Ok(receipt)
}

pub fn validate_evidence_receipt_json(value: &Value) -> Result<EvidenceReceipt, String> {
    let receipt: EvidenceReceipt =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    receipt.validate().map_err(|error| error.to_string())?;
    Ok(receipt)
}

pub fn execute_workflow_json(value: &Value) -> Result<Value, String> {
    let request: WorkflowExecutionRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid workflow execution request: {error}"))?;
    let receipt = execute_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize workflow receipt: {error}"))
}

pub fn validate_workflow_execution_receipt_json(
    value: &Value,
) -> Result<WorkflowExecutionReceipt, String> {
    let receipt: WorkflowExecutionReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid workflow execution receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != WORKFLOW_EXECUTION_FEATURE_ID {
        return Err("workflow execution feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn compile_evaluation_card_json(value: &Value) -> Result<Value, String> {
    let request: EvaluationCardRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evaluation-card request: {error}"))?;
    let receipt = compile_evaluation_card(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize evaluation-card receipt: {error}"))
}

pub fn validate_evaluation_card_receipt_json(
    value: &Value,
) -> Result<EvaluationCardReceipt, String> {
    let receipt: EvaluationCardReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evaluation-card receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != EVALUATION_OBSERVABILITY_FEATURE_ID {
        return Err("evaluation-observability feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_federated_evaluation_json(value: &Value) -> Result<Value, String> {
    let request: FederatedEvaluationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated evaluation request: {error}"))?;
    let receipt = evaluate_federated_evaluation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated evaluation receipt: {error}"))
}

pub fn validate_federated_evaluation_receipt_json(
    value: &Value,
) -> Result<FederatedEvaluationReceipt, String> {
    let receipt: FederatedEvaluationReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated evaluation receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_EVALUATION_FEATURE_ID {
        return Err("federated evaluation feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn validate_research_release_receipt_json(
    value: &Value,
) -> Result<ResearchReleaseReceipt, String> {
    let receipt: ResearchReleaseReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research-release receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RESEARCH_RELEASE_FEATURE_ID {
        return Err("research-release feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn validate_research_release_batch_receipt_json(
    value: &Value,
) -> Result<ResearchReleaseBatchReceipt, String> {
    let receipt: ResearchReleaseBatchReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research-release batch receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RESEARCH_RELEASE_BATCH_FEATURE_ID {
        return Err("research-release batch feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn instrument_preflight_json(value: &Value) -> Result<Value, String> {
    let request: InstrumentPreflightRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid instrument preflight request: {error}"))?;
    let receipt = instrument_preflight(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize instrument preflight receipt: {error}"))
}

pub fn validate_instrument_preflight_receipt_json(
    value: &Value,
) -> Result<InstrumentPreflightReceipt, String> {
    let receipt: InstrumentPreflightReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid instrument preflight receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != INSTRUMENT_PREFLIGHT_FEATURE_ID {
        return Err("instrument preflight feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn harmonize_multimodal_json(value: &Value) -> Result<Value, String> {
    let request: MultimodalHarmonizationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid multimodal harmonization request: {error}"))?;
    let object = harmonize_multimodal(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(object)
        .map_err(|error| format!("cannot serialize harmonized research object: {error}"))
}

pub fn validate_harmonized_research_object_json(
    value: &Value,
) -> Result<HarmonizedResearchObject, String> {
    let object: HarmonizedResearchObject = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid harmonized research object: {error}"))?;
    object.validate().map_err(|error| error.to_string())?;
    if object.feature_id != MULTIMODAL_HARMONIZATION_FEATURE_ID {
        return Err("multimodal harmonization feature id mismatch".into());
    }
    Ok(object)
}

pub fn qualify_analysis_json(value: &Value) -> Result<Value, String> {
    let request: AnalysisQualificationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid analysis qualification request: {error}"))?;
    let result = qualify_analysis(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(result)
        .map_err(|error| format!("cannot serialize qualified analysis result: {error}"))
}

pub fn validate_qualified_analysis_result_json(
    value: &Value,
) -> Result<QualifiedAnalysisResult, String> {
    let result: QualifiedAnalysisResult = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid qualified analysis result: {error}"))?;
    result.validate().map_err(|error| error.to_string())?;
    if result.feature_id != ANALYSIS_QUALIFICATION_FEATURE_ID {
        return Err("analysis qualification feature id mismatch".into());
    }
    Ok(result)
}

pub fn simulate_protocol_matrix_json(value: &Value) -> Result<Value, String> {
    let request: ProtocolMatrixRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol matrix request: {error}"))?;
    let receipt = simulate_protocol_matrix(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize protocol matrix receipt: {error}"))
}

pub fn validate_protocol_matrix_receipt_json(
    value: &Value,
) -> Result<ProtocolMatrixReceipt, String> {
    let receipt: ProtocolMatrixReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol matrix receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != PROTOCOL_MATRIX_FEATURE_ID {
        return Err("protocol matrix feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_multimodal_replication_json(value: &Value) -> Result<Value, String> {
    let request: MultimodalReplicationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid multimodal replication request: {error}"))?;
    let report = evaluate_multimodal_replication(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(report)
        .map_err(|error| format!("cannot serialize multimodal replication report: {error}"))
}

pub fn validate_multimodal_replication_report_json(
    value: &Value,
) -> Result<MultimodalReplicationReport, String> {
    let report: MultimodalReplicationReport = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid multimodal replication report: {error}"))?;
    report.validate().map_err(|error| error.to_string())?;
    if report.feature_id != MULTIMODAL_REPLICATION_FEATURE_ID {
        return Err("multimodal replication feature id mismatch".into());
    }
    Ok(report)
}

pub fn evaluate_quality_drift_json(value: &Value) -> Result<Value, String> {
    let request: QualityDriftRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid quality drift request: {error}"))?;
    let receipt = evaluate_quality_drift(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize quality drift receipt: {error}"))
}

pub fn validate_quality_drift_receipt_json(value: &Value) -> Result<QualityDriftReceipt, String> {
    let receipt: QualityDriftReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid quality drift receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != QUALITY_DRIFT_FEATURE_ID {
        return Err("quality drift feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_design_frontier_json(value: &Value) -> Result<Value, String> {
    let request: DesignFrontierRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid design frontier request: {error}"))?;
    let receipt = evaluate_design_frontier(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize design frontier receipt: {error}"))
}

pub fn validate_design_frontier_receipt_json(
    value: &Value,
) -> Result<DesignFrontierReceipt, String> {
    let receipt: DesignFrontierReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid design frontier receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != DESIGN_FRONTIER_FEATURE_ID {
        return Err("design frontier feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_autonomy_batch_json(value: &Value) -> Result<Value, String> {
    let request: BatchAdmissionRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid autonomy batch request: {error}"))?;
    let receipt = admit_autonomy_batch(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize autonomy batch receipt: {error}"))
}

pub fn validate_autonomy_batch_receipt_json(
    value: &Value,
) -> Result<BatchAdmissionReceipt, String> {
    let receipt: BatchAdmissionReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid autonomy batch receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != AUTONOMY_BATCH_FEATURE_ID {
        return Err("autonomy batch feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn execute_workflow_batch_json(value: &Value) -> Result<Value, String> {
    let request: WorkflowBatchRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid workflow batch request: {error}"))?;
    let receipt = execute_workflow_batch(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize workflow batch receipt: {error}"))
}

pub fn validate_workflow_batch_receipt_json(value: &Value) -> Result<WorkflowBatchReceipt, String> {
    let receipt: WorkflowBatchReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid workflow batch receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != WORKFLOW_BATCH_FEATURE_ID {
        return Err("workflow batch feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn discover_resources_json(value: &Value) -> Result<Value, String> {
    let need: FiberResourceNeed = serde_json::from_value(
        value
            .get("need")
            .cloned()
            .ok_or("need is required and must be a serialized ResourceNeed")?,
    )
    .map_err(|error| format!("invalid resource need: {error}"))?;
    let candidates: Vec<FiberResourceCandidate> = serde_json::from_value(
        value
            .get("candidates")
            .cloned()
            .ok_or("candidates is required and must be an array of ResourceCandidate")?,
    )
    .map_err(|error| format!("invalid resource candidates: {error}"))?;
    let receipt = discover_resources(&need, &candidates).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize qualified resource set: {error}"))
}

pub fn validate_qualified_resource_set_json(value: &Value) -> Result<QualifiedResourceSet, String> {
    let receipt: QualifiedResourceSet = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid qualified resource set: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FIBER_RESOURCE_WORKBENCH_FEATURE_ID {
        return Err("resource workbench feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn resource_discovery_contract_v2_json(value: &Value) -> Result<Value, String> {
    let request: ResourceDiscoveryContractRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid resource discovery contract request: {error}"))?;
    let response =
        compile_resource_discovery_contract_v2(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(response)
        .map_err(|error| format!("cannot serialize resource discovery contract: {error}"))
}

pub fn validate_resource_discovery_contract_v2_json(
    value: &Value,
) -> Result<ResourceDiscoveryContractResponse, String> {
    let response: ResourceDiscoveryContractResponse = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid resource discovery contract response: {error}"))?;
    response.validate().map_err(|error| error.to_string())?;
    if response.feature_id != RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID {
        return Err("resource discovery contract feature id mismatch".into());
    }
    Ok(response)
}

pub fn compile_governance_research_release_json(value: &Value) -> Result<Value, String> {
    let run: ValidatedResearchRun = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid validated research run: {error}"))?;
    let object = compile_signed_research_object(&run).map_err(|error| error.to_string())?;
    serde_json::to_value(object)
        .map_err(|error| format!("cannot serialize signed research object: {error}"))
}

pub fn validate_governance_research_release_json(
    value: &Value,
) -> Result<SignedResearchObject, String> {
    let object: SignedResearchObject = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid signed research object: {error}"))?;
    object.validate().map_err(|error| error.to_string())?;
    if object.feature_id != RESEARCH_RELEASE_CONTRACT_FEATURE_ID {
        return Err("governance research-release feature id mismatch".into());
    }
    Ok(object)
}

pub fn assess_release_harness_json(value: &Value) -> Result<Value, String> {
    let request: ReleaseHarnessRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid release assurance request: {error}"))?;
    let receipt = assess_release_harness(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize release assurance receipt: {error}"))
}

pub fn validate_release_harness_json(value: &Value) -> Result<ReleaseHarnessReceipt, String> {
    let receipt: ReleaseHarnessReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid release assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RELEASE_HARNESS_FEATURE_ID {
        return Err("release assurance harness feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assess_protocol_assurance_json(value: &Value) -> Result<Value, String> {
    let request: ProtocolAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol assurance request: {error}"))?;
    let receipt = assess_protocol_assurance(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize protocol assurance receipt: {error}"))
}

pub fn validate_protocol_assurance_json(value: &Value) -> Result<ProtocolAssuranceReceipt, String> {
    let receipt: ProtocolAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != PROTOCOL_ASSURANCE_FEATURE_ID {
        return Err("protocol assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_federated_multimodal_json(value: &Value) -> Result<Value, String> {
    let request: FederatedMultimodalAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated multimodal assurance request: {error}"))?;
    let receipt = assure_federated_multimodal(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| {
        format!("cannot serialize federated multimodal assurance receipt: {error}")
    })
}

pub fn validate_federated_multimodal_assurance_json(
    value: &Value,
) -> Result<FederatedMultimodalAssuranceReceipt, String> {
    let receipt: FederatedMultimodalAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated multimodal assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID {
        return Err("federated multimodal assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_federated_knowledge_json(value: &Value) -> Result<Value, String> {
    let request: FederatedKnowledgeGatewayRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated knowledge gateway request: {error}"))?;
    let receipt = admit_federated_knowledge(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated knowledge gateway receipt: {error}"))
}

pub fn validate_federated_knowledge_gateway_json(
    value: &Value,
) -> Result<FederatedKnowledgeGatewayReceipt, String> {
    let receipt: FederatedKnowledgeGatewayReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated knowledge gateway receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID {
        return Err("federated knowledge gateway feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_federated_lens_json(value: &Value) -> Result<Value, String> {
    let request: FederatedLensAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated lens assurance request: {error}"))?;
    let receipt = assure_federated_lens(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated lens assurance receipt: {error}"))
}

pub fn validate_federated_lens_assurance_json(
    value: &Value,
) -> Result<FederatedLensAssuranceReceipt, String> {
    let receipt: FederatedLensAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated lens assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_LENS_ASSURANCE_FEATURE_ID {
        return Err("federated lens assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_semantic_parity_json(value: &Value) -> Result<Value, String> {
    let request: LabSemanticParityRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid lab semantic parity request: {error}"))?;
    let receipt = evaluate_semantic_parity(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize lab semantic parity receipt: {error}"))
}

pub fn validate_semantic_parity_json(value: &Value) -> Result<LabSemanticParityReceipt, String> {
    let receipt: LabSemanticParityReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid lab semantic parity receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != SEMANTIC_PARITY_FEATURE_ID {
        return Err("lab semantic parity feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_federated_retrieval_json(value: &Value) -> Result<Value, String> {
    let request: FederatedRetrievalAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated retrieval assurance request: {error}"))?;
    let receipt = assure_federated_retrieval(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated retrieval assurance receipt: {error}"))
}

pub fn validate_federated_retrieval_assurance_json(
    value: &Value,
) -> Result<FederatedRetrievalAssuranceReceipt, String> {
    let receipt: FederatedRetrievalAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated retrieval assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID {
        return Err("federated retrieval assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn synthesize_federated_continuum_json(value: &Value) -> Result<Value, String> {
    let request: FederatedContinualRetrievalRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated continual retrieval request: {error}"))?;
    let receipt = synthesize_federated_continuum(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated continual retrieval receipt: {error}"))
}

pub fn validate_federated_continual_retrieval_json(
    value: &Value,
) -> Result<FederatedContinualRetrievalReceipt, String> {
    let receipt: FederatedContinualRetrievalReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated continual retrieval receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID {
        return Err("federated continual retrieval feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_context_compilation_json(value: &Value) -> Result<Value, String> {
    let request: ContextCompilationAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid context compilation assurance request: {error}"))?;
    let receipt = assure_context_compilation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize context compilation assurance receipt: {error}"))
}

pub fn validate_context_compilation_assurance_json(
    value: &Value,
) -> Result<ContextCompilationAssuranceReceipt, String> {
    let receipt: ContextCompilationAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid context compilation assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID {
        return Err("context compilation assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_knowledge_representation_json(value: &Value) -> Result<Value, String> {
    let request: KnowledgeRepresentationAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge representation assurance request: {error}"))?;
    let receipt = assure_knowledge_representation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| {
        format!("cannot serialize knowledge representation assurance receipt: {error}")
    })
}

pub fn validate_knowledge_representation_assurance_json(
    value: &Value,
) -> Result<KnowledgeRepresentationAssuranceReceipt, String> {
    let receipt: KnowledgeRepresentationAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge representation assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID {
        return Err("knowledge representation assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn operate_resource_control_plane_json(value: &Value) -> Result<Value, String> {
    let request: ResourceControlPlaneRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid resource control-plane request: {error}"))?;
    let receipt = operate_resource_control_plane(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize resource control-plane receipt: {error}"))
}

pub fn validate_resource_control_plane_json(
    value: &Value,
) -> Result<ResourceControlPlaneReceipt, String> {
    let receipt: ResourceControlPlaneReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid resource control-plane receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RESOURCE_CONTROL_PLANE_FEATURE_ID {
        return Err("resource control-plane feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_weavelang_release_json(value: &Value) -> Result<Value, String> {
    let request: WeaveLangReleaseAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid WeaveLang release assurance request: {error}"))?;
    let receipt = assure_weavelang_release(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize WeaveLang release assurance receipt: {error}"))
}

pub fn validate_weavelang_release_assurance_json(
    value: &Value,
) -> Result<WeaveLangReleaseAssuranceReceipt, String> {
    let receipt: WeaveLangReleaseAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid WeaveLang release assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID {
        return Err("WeaveLang release assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn operate_mechanism_control_plane_json(value: &Value) -> Result<Value, String> {
    let request: MechanismControlPlaneRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid mechanism control-plane request: {error}"))?;
    let receipt = operate_mechanism_control_plane(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize mechanism control-plane receipt: {error}"))
}

pub fn validate_mechanism_control_plane_json(
    value: &Value,
) -> Result<MechanismControlPlaneReceipt, String> {
    let receipt: MechanismControlPlaneReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid mechanism control-plane receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != MECHANISM_CONTROL_PLANE_FEATURE_ID {
        return Err("mechanism control-plane feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_mechanism_gateway_json(value: &Value) -> Result<Value, String> {
    let request: MechanismGatewayRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid mechanism gateway request: {error}"))?;
    let receipt = admit_mechanism_gateway(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize mechanism gateway receipt: {error}"))
}

pub fn validate_mechanism_gateway_json(value: &Value) -> Result<MechanismGatewayReceipt, String> {
    let receipt: MechanismGatewayReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid mechanism gateway receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != MECHANISM_GATEWAY_FEATURE_ID {
        return Err("mechanism gateway feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_evidence_surveillance_json(value: &Value) -> Result<Value, String> {
    let request: EvidenceFeedRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evidence surveillance request: {error}"))?;
    let receipt = run_evidence_surveillance(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize evidence surveillance receipt: {error}"))
}

pub fn run_local_evidence_surveillance_research_copilot_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research copilot request: {error}"))?;
    let receipt = run_local_evidence_surveillance_research_copilot(&request)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize research copilot receipt: {error}"))
}

pub fn validate_local_evidence_surveillance_research_copilot_json(
    value: &Value,
) -> Result<LocalEvidenceSurveillanceResearchCopilotReceipt, String> {
    let receipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research copilot receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID {
        return Err("research copilot feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn validate_evidence_surveillance_json(
    value: &Value,
) -> Result<EvidenceSurveillanceReceipt, String> {
    let receipt: EvidenceSurveillanceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evidence surveillance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != EVIDENCE_SURVEILLANCE_FEATURE_ID {
        return Err("evidence surveillance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_multimodal_evidence_surveillance_research_copilot_json(
    value: &Value,
) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid multimodal research copilot request: {error}"))?;
    let receipt = run_multimodal_evidence_surveillance_research_copilot(&request)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| {
        format!("cannot serialize multimodal research copilot receipt: {error}")
    })
}

pub fn validate_multimodal_evidence_surveillance_research_copilot_json(
    value: &Value,
) -> Result<MultimodalEvidenceSurveillanceResearchCopilotReceipt, String> {
    let receipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid multimodal research copilot receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID {
        return Err("multimodal research copilot feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn compile_evidence_synthesis_json(value: &Value) -> Result<Value, String> {
    let request: EvidenceSynthesisRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid retrieval synthesis request: {error}"))?;
    let receipt = compile_evidence_synthesis(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize retrieval synthesis receipt: {error}"))
}

pub fn validate_evidence_synthesis_json(
    value: &Value,
) -> Result<RetrievalSynthesisReceipt, String> {
    let receipt: RetrievalSynthesisReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid retrieval synthesis receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RETRIEVAL_SYNTHESIS_FEATURE_ID {
        return Err("retrieval synthesis feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_local_retrieval_synthesis_inference_engine_json(value: &Value) -> Result<Value, String> {
    let request: LocalRetrievalSynthesisInferenceEngineRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid local retrieval synthesis engine request: {error}"))?;
    let receipt = run_local_retrieval_synthesis_inference_engine(&request)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize local retrieval synthesis engine receipt: {error}"))
}

pub fn validate_local_retrieval_synthesis_inference_engine_json(
    value: &Value,
) -> Result<LocalRetrievalSynthesisInferenceEngineReceipt, String> {
    let receipt: LocalRetrievalSynthesisInferenceEngineReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid local retrieval synthesis engine receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID {
        return Err("local retrieval synthesis engine feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_throughput_evidence_surveillance_research_copilot_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid throughput research copilot request: {error}"))?;
    let receipt = run_throughput_evidence_surveillance_research_copilot(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize throughput research copilot receipt: {error}"))
}

pub fn validate_throughput_evidence_surveillance_research_copilot_json(value: &Value) -> Result<ThroughputEvidenceSurveillanceResearchCopilotReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid throughput research copilot receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID { return Err("throughput research copilot feature id mismatch".into()); }
    Ok(receipt)
}

pub fn assure_adapter_context_compilation_json(value: &Value) -> Result<Value, String> {
    let request: ContextCompilationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter context compilation request: {error}"))?;
    let receipt =
        assure_adapter_context_compilation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter context compilation receipt: {error}"))
}

pub fn run_federated_continual_evidence_surveillance_research_copilot_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid federated continual research copilot request: {error}"))?;
    let receipt = run_federated_continual_evidence_surveillance_research_copilot(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize federated continual research copilot receipt: {error}"))
}

pub fn validate_federated_continual_evidence_surveillance_research_copilot_json(value: &Value) -> Result<FederatedContinualEvidenceSurveillanceResearchCopilotReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid federated continual research copilot receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID { return Err("federated continual research copilot feature id mismatch".into()); }
    Ok(receipt)
}

pub fn validate_adapter_context_compilation_json(
    value: &Value,
) -> Result<ContextCompilationReceipt, String> {
    let receipt: ContextCompilationReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter context compilation receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != CONTEXT_COMPILATION_FEATURE_ID {
        return Err("adapter context compilation feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_local_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid local evidence workflow request: {error}"))?;
    let receipt = schedule_local_evidence_surveillance_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize local evidence workflow receipt: {error}"))
}

pub fn validate_local_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<LocalEvidenceSurveillanceWorkflowReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid local evidence workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID { return Err("local evidence workflow feature id mismatch".into()); }
    Ok(receipt)
}

pub fn run_multimodal_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid multimodal evidence workflow request: {error}"))?;
    let receipt = schedule_multimodal_evidence_surveillance_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize multimodal evidence workflow receipt: {error}"))
}

pub fn validate_multimodal_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<MultimodalEvidenceSurveillanceWorkflowReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid multimodal evidence workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID { return Err("multimodal evidence workflow feature id mismatch".into()); }
    Ok(receipt)
}

pub fn run_throughput_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid throughput evidence workflow request: {error}"))?;
    let receipt = schedule_throughput_evidence_surveillance_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize throughput evidence workflow receipt: {error}"))
}

pub fn validate_throughput_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<ThroughputEvidenceSurveillanceWorkflowReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid throughput evidence workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID { return Err("throughput evidence workflow feature id mismatch".into()); }
    Ok(receipt)
}

pub fn run_federated_continual_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<Value, String> {
    let request = serde_json::from_value(value.clone()).map_err(|error| format!("invalid federated continual evidence workflow request: {error}"))?;
    let receipt = schedule_federated_continual_evidence_surveillance_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize federated continual evidence workflow receipt: {error}"))
}

pub fn validate_federated_continual_evidence_surveillance_workflow_fabric_json(value: &Value) -> Result<FederatedContinualEvidenceSurveillanceWorkflowReceipt, String> {
    let receipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid federated continual evidence workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID { return Err("federated continual evidence workflow feature id mismatch".into()); }
    Ok(receipt)
}

pub fn run_local_evidence_surveillance_research_workbench_json(value: &Value) -> Result<Value, String> {
    let request: LocalEvidenceSurveillanceResearchWorkbenchRequest = serde_json::from_value(value.clone()).map_err(|error| format!("invalid local evidence workbench request: {error}"))?;
    let receipt = render_local_evidence_surveillance_research_workbench(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize local evidence workbench receipt: {error}"))
}

pub fn validate_local_evidence_surveillance_research_workbench_json(value: &Value) -> Result<LocalEvidenceSurveillanceResearchWorkbenchReceipt, String> {
    let receipt: LocalEvidenceSurveillanceResearchWorkbenchReceipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid local evidence workbench receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID { return Err("local evidence workbench feature id mismatch".into()); }
    Ok(receipt)
}

pub fn run_multimodal_evidence_surveillance_research_workbench_json(value: &Value) -> Result<Value, String> {
    let request: MultimodalEvidenceSurveillanceResearchWorkbenchRequest = serde_json::from_value(value.clone()).map_err(|error| format!("invalid multimodal evidence workbench request: {error}"))?;
    let receipt = render_multimodal_evidence_surveillance_research_workbench(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| format!("cannot serialize multimodal evidence workbench receipt: {error}"))
}

pub fn validate_multimodal_evidence_surveillance_research_workbench_json(value: &Value) -> Result<MultimodalEvidenceSurveillanceResearchWorkbenchReceipt, String> {
    let receipt: MultimodalEvidenceSurveillanceResearchWorkbenchReceipt = serde_json::from_value(value.clone()).map_err(|error| format!("invalid multimodal evidence workbench receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID { return Err("multimodal evidence workbench feature id mismatch".into()); }
    Ok(receipt)
}
pub fn run_throughput_evidence_surveillance_research_workbench_json(value: &Value) -> Result<Value, String> { let request: ThroughputEvidenceSurveillanceResearchWorkbenchRequest = serde_json::from_value(value.clone()).map_err(|e| format!("invalid throughput evidence workbench request: {e}"))?; let receipt = render_throughput_evidence_surveillance_research_workbench(&request).map_err(|e| e.to_string())?; serde_json::to_value(receipt).map_err(|e| format!("cannot serialize throughput evidence workbench receipt: {e}")) }
pub fn validate_throughput_evidence_surveillance_research_workbench_json(value: &Value) -> Result<ThroughputEvidenceSurveillanceResearchWorkbenchReceipt, String> { let receipt: ThroughputEvidenceSurveillanceResearchWorkbenchReceipt = serde_json::from_value(value.clone()).map_err(|e| format!("invalid throughput evidence workbench receipt: {e}"))?; receipt.validate().map_err(|e| e.to_string())?; if receipt.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID { return Err("throughput evidence workbench feature id mismatch".into()); } Ok(receipt) }
pub fn run_federated_continual_evidence_surveillance_research_workbench_json(value: &Value) -> Result<Value, String> { let request: FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest = serde_json::from_value(value.clone()).map_err(|e| format!("invalid federated continual evidence workbench request: {e}"))?; let receipt = render_federated_continual_evidence_surveillance_research_workbench(&request).map_err(|e| e.to_string())?; serde_json::to_value(receipt).map_err(|e| format!("cannot serialize federated continual evidence workbench receipt: {e}")) }
pub fn validate_federated_continual_evidence_surveillance_research_workbench_json(value: &Value) -> Result<FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt, String> { let receipt: FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt = serde_json::from_value(value.clone()).map_err(|e| format!("invalid federated continual evidence workbench receipt: {e}"))?; receipt.validate().map_err(|e| e.to_string())?; if receipt.feature_id != ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID { return Err("federated continual evidence workbench feature id mismatch".into()); } Ok(receipt) }

pub fn run_knowledge_workflow_json(value: &Value) -> Result<Value, String> {
    let request: ClaimsWorkflowRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge workflow request: {error}"))?;
    let receipt = run_knowledge_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize knowledge workflow receipt: {error}"))
}

pub fn validate_knowledge_workflow_json(value: &Value) -> Result<KnowledgeWorkflowReceipt, String> {
    let receipt: KnowledgeWorkflowReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != KNOWLEDGE_WORKFLOW_FEATURE_ID {
        return Err("knowledge workflow feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn discover_adapter_resources_json(value: &Value) -> Result<Value, String> {
    let request_id = value
        .get("request_id")
        .and_then(Value::as_str)
        .ok_or("request_id is required")?;
    let need: AdapterResourceNeed =
        serde_json::from_value(value.get("need").cloned().ok_or("need is required")?)
            .map_err(|error| format!("invalid adapter resource need: {error}"))?;
    let candidates: Vec<AdapterResourceCandidate> = serde_json::from_value(
        value
            .get("candidates")
            .cloned()
            .ok_or("candidates are required")?,
    )
    .map_err(|error| format!("invalid adapter resource candidates: {error}"))?;
    let receipt = discover_adapter_resources(request_id, &need, &candidates)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter resource receipt: {error}"))
}

pub fn validate_adapter_resource_workbench_json(
    value: &Value,
) -> Result<ResourceWorkbenchReceipt, String> {
    let receipt: ResourceWorkbenchReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter resource receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ADAPTER_RESOURCE_WORKBENCH_FEATURE_ID {
        return Err("adapter resource workbench feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn run_ingestion_gateway_json(value: &Value) -> Result<Value, String> {
    let request: IngestionGatewayRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid ingestion gateway request: {error}"))?;
    let receipt = run_ingestion_gateway(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize ingestion gateway receipt: {error}"))
}

pub fn validate_ingestion_gateway_json(value: &Value) -> Result<IngestionGatewayReceipt, String> {
    let receipt: IngestionGatewayReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid ingestion gateway receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != INGESTION_GATEWAY_FEATURE_ID {
        return Err("ingestion gateway feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_quality_envelope_json(value: &Value) -> Result<Value, String> {
    let request: QualityEnvelopeRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid quality envelope request: {error}"))?;
    let receipt = evaluate_quality_envelope(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize quality envelope receipt: {error}"))
}

pub fn validate_quality_envelope_json(value: &Value) -> Result<QualityEnvelopeReceipt, String> {
    let receipt: QualityEnvelopeReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid quality envelope receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != QUALITY_ENVELOPE_FEATURE_ID {
        return Err("quality envelope feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn compile_experiment_design_json(value: &Value) -> Result<Value, String> {
    let request: FederatedExperimentDesignRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid experiment design request: {error}"))?;
    let receipt = compile_experiment_design(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize experiment design receipt: {error}"))
}

pub fn validate_experiment_design_json(value: &Value) -> Result<ExperimentDesignReceipt, String> {
    let receipt: ExperimentDesignReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid experiment design receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != EXPERIMENT_DESIGN_CONTROL_FEATURE_ID {
        return Err("experiment design feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn simulate_protocol_draft_json(value: &Value) -> Result<Value, String> {
    let request: ProtocolDraft = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol draft: {error}"))?;
    let receipt = simulate_protocol_draft(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize protocol simulation receipt: {error}"))
}

pub fn validate_protocol_simulation_json(
    value: &Value,
) -> Result<ProtocolSimulationReceipt, String> {
    let receipt: ProtocolSimulationReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid protocol simulation receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != PROTOCOL_SIMULATION_FEATURE_ID {
        return Err("protocol simulation feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn integrate_instrument_mesh_json(value: &Value) -> Result<Value, String> {
    let request = value
        .get("request")
        .ok_or("request is required and must be an InstrumentActionRequest")?;
    let request: InstrumentActionRequest = serde_json::from_value(request.clone())
        .map_err(|error| format!("invalid instrument mesh request: {error}"))?;
    let capabilities: Vec<InstrumentCapability> = value
        .get("capabilities")
        .ok_or_else(|| "capabilities is required and must be an array".to_string())
        .and_then(|items| {
            serde_json::from_value(items.clone())
                .map_err(|error| format!("invalid instrument capabilities: {error}"))
        })?;
    let receipt =
        integrate_instrument_mesh(&request, &capabilities).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize instrument mesh receipt: {error}"))
}

pub fn validate_instrument_mesh_json(value: &Value) -> Result<InstrumentMeshReceipt, String> {
    let receipt: InstrumentMeshReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid instrument mesh receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != INSTRUMENT_MESH_FEATURE_ID {
        return Err("instrument mesh feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_computational_execution_json(value: &Value) -> Result<Value, String> {
    let request: ComputationalExecutionRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid computational execution request: {error}"))?;
    let receipt = admit_computational_execution(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize computational execution receipt: {error}"))
}

pub fn validate_computational_execution_json(
    value: &Value,
) -> Result<ComputationalExecutionReceipt, String> {
    let receipt: ComputationalExecutionReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid computational execution receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != EXECUTION_CONTROL_FEATURE_ID {
        return Err("computational execution feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn qualify_analysis_portfolio_json(value: &Value) -> Result<Value, String> {
    let request: AnalysisPortfolioRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid analysis portfolio request: {error}"))?;
    let receipt = qualify_analysis_portfolio(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize analysis portfolio receipt: {error}"))
}

pub fn validate_analysis_portfolio_json(value: &Value) -> Result<AnalysisPortfolioReceipt, String> {
    let receipt: AnalysisPortfolioReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid analysis portfolio receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != ANALYSIS_PORTFOLIO_FEATURE_ID {
        return Err("analysis portfolio feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_interpretation_json(value: &Value) -> Result<Value, String> {
    let request: EvidenceBackedResult = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid interpretation assurance request: {error}"))?;
    let receipt = assure_interpretation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize interpretation assurance receipt: {error}"))
}

pub fn validate_interpretation_assurance_json(
    value: &Value,
) -> Result<InterpretationAssuranceReceipt, String> {
    let receipt: InterpretationAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid interpretation assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != INTERPRETATION_ASSURANCE_FEATURE_ID {
        return Err("interpretation assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_replication_json(value: &Value) -> Result<Value, String> {
    let request: ReplicationAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid replication assurance request: {error}"))?;
    let receipt = assure_replication(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize replication assurance receipt: {error}"))
}

pub fn validate_replication_assurance_json(
    value: &Value,
) -> Result<ReplicationAssuranceReceipt, String> {
    let receipt: ReplicationAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid replication assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != REPLICATION_ASSURANCE_FEATURE_ID {
        return Err("replication assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_release_json(value: &Value) -> Result<Value, String> {
    let request: AdapterValidatedResearchRun = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid release assurance request: {error}"))?;
    let receipt = assure_release(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize release assurance receipt: {error}"))
}

pub fn validate_release_assurance_json(value: &Value) -> Result<ReleaseAssuranceReceipt, String> {
    let receipt: ReleaseAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid release assurance receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != RELEASE_ASSURANCE_FEATURE_ID {
        return Err("release assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn negotiate_determinism_json(value: &Value) -> Result<Value, String> {
    let request: TypedCapabilityInput = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid typed determinism request: {error}"))?;
    let receipt = negotiate_capability(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize canonical capability output: {error}"))
}

pub fn validate_determinism_json(value: &Value) -> Result<CanonicalCapabilityOutput, String> {
    let receipt: CanonicalCapabilityOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid canonical capability output: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != DETERMINISM_GATEWAY_FEATURE_ID {
        return Err("typed determinism feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_provenance_json(value: &Value) -> Result<Value, String> {
    let request: ArtifactAndDerivation = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid provenance assurance request: {error}"))?;
    let receipt = assure_provenance(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize signed provenance envelope: {error}"))
}

pub fn validate_provenance_json(value: &Value) -> Result<SignedProvenanceEnvelope, String> {
    let receipt: SignedProvenanceEnvelope = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid signed provenance envelope: {error}"))?;
    receipt
        .validate()
        .map_err(|error: ProvenanceAssuranceError| error.to_string())?;
    if receipt.feature_id != PROVENANCE_ASSURANCE_FEATURE_ID {
        return Err("provenance assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_policy_json(value: &Value) -> Result<Value, String> {
    let request: ActionAndAuthority = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid policy gateway request: {error}"))?;
    let receipt = admit_policy_action(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize policy gateway receipt: {error}"))
}

pub fn validate_policy_gateway_json(value: &Value) -> Result<PolicyGatewayReceipt, String> {
    let receipt: PolicyGatewayReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid policy gateway receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != POLICY_GATEWAY_FEATURE_ID {
        return Err("policy gateway feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn schedule_federation_workflow_json(value: &Value) -> Result<Value, String> {
    let request: FederationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federation workflow request: {error}"))?;
    let receipt = schedule_federation_workflow(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federation workflow receipt: {error}"))
}

pub fn validate_federation_workflow_json(
    value: &Value,
) -> Result<FederationWorkflowReceipt, String> {
    let receipt: FederationWorkflowReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federation workflow receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEDERATION_WORKFLOW_FEATURE_ID {
        return Err("federation workflow feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn plan_reliable_capability_json(value: &Value) -> Result<Value, String> {
    let request: CapabilityWorkload = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid reliability copilot workload: {error}"))?;
    let receipt = plan_reliable_capability(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize reliable capability result: {error}"))
}

pub fn validate_reliability_copilot_json(
    value: &Value,
) -> Result<ReliableCapabilityResult, String> {
    let receipt: ReliableCapabilityResult = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid reliable capability result: {error}"))?;
    receipt
        .validate()
        .map_err(|error: ReliabilityCopilotError| error.to_string())?;
    if receipt.feature_id != RELIABILITY_COPILOT_FEATURE_ID {
        return Err("reliability copilot feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn negotiate_interoperability_json(value: &Value) -> Result<Value, String> {
    let request: InteroperabilityRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid interoperability gateway request: {error}"))?;
    let receipt = negotiate_interoperability(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize negotiated integration: {error}"))
}

pub fn validate_interoperability_gateway_json(
    value: &Value,
) -> Result<NegotiatedIntegration, String> {
    let receipt: NegotiatedIntegration = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid negotiated integration: {error}"))?;
    receipt
        .validate()
        .map_err(|error: InteroperabilityGatewayError| error.to_string())?;
    if receipt.feature_id != INTEROPERABILITY_GATEWAY_FEATURE_ID {
        return Err("interoperability gateway feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_evaluation_run_json(value: &Value) -> Result<Value, String> {
    let request: CapabilityRun = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evaluation assurance run: {error}"))?;
    let receipt = assure_evaluation_run(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize evaluation assurance receipt: {error}"))
}

pub fn validate_evaluation_assurance_json(
    value: &Value,
) -> Result<EvaluationAssuranceReceipt, String> {
    let receipt: EvaluationAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid evaluation assurance receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: EvaluationAssuranceError| error.to_string())?;
    if receipt.feature_id != EVALUATION_ASSURANCE_FEATURE_ID {
        return Err("evaluation assurance feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn compile_research_workbench_json(value: &Value) -> Result<Value, String> {
    let request: ResearchWorkspaceState = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research workbench state: {error}"))?;
    let receipt = compile_research_workbench(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize interactive research workspace: {error}"))
}

pub fn validate_research_workbench_json(
    value: &Value,
) -> Result<InteractiveResearchWorkspace, String> {
    let receipt: InteractiveResearchWorkspace = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid interactive research workspace: {error}"))?;
    receipt
        .validate()
        .map_err(|error: ResearchWorkbenchError| error.to_string())?;
    if receipt.feature_id != RESEARCH_WORKBENCH_FEATURE_ID {
        return Err("research workbench feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn compile_adapter_capability_manifest_json(value: &Value) -> Result<Value, String> {
    let request: AdapterContractInput = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter contract frontier input: {error}"))?;
    let receipt =
        compile_adapter_capability_manifest(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter capability manifest: {error}"))
}

pub fn validate_contract_frontier_json(value: &Value) -> Result<AdapterCapabilityManifest, String> {
    let receipt: AdapterCapabilityManifest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter capability manifest: {error}"))?;
    receipt
        .validate()
        .map_err(|error: ContractFrontierError| error.to_string())?;
    if receipt.feature_id != CONTRACT_FRONTIER_FEATURE_ID {
        return Err("adapter contract frontier feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn close_adapter_limitations_json(value: &Value) -> Result<Value, String> {
    let request: LimitationClosureRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid limitation closure request: {error}"))?;
    let receipt = close_adapter_limitations(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize limitation closure receipt: {error}"))
}

pub fn validate_limitation_closure_json(value: &Value) -> Result<AdapterClosureReceipt, String> {
    let receipt: AdapterClosureReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid limitation closure receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: LimitationClosureError| error.to_string())?;
    if receipt.feature_id != LIMITATION_CLOSURE_FEATURE_ID {
        return Err("limitation closure feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn infer_adapter_dependency_composition_json(value: &Value) -> Result<Value, String> {
    let request: AdapterCompositionRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter dependency composition request: {error}"))?;
    let receipt =
        infer_adapter_dependency_composition(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt).map_err(|error| {
        format!("cannot serialize adapter dependency composition receipt: {error}")
    })
}

pub fn validate_dependency_composition_json(
    value: &Value,
) -> Result<AdapterCompositionReceipt, String> {
    let receipt: AdapterCompositionReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter dependency composition receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: DependencyCompositionError| error.to_string())?;
    if receipt.feature_id != DEPENDENCY_COMPOSITION_FEATURE_ID {
        return Err("adapter dependency composition feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn evaluate_adapter_semantic_parity_json(value: &Value) -> Result<Value, String> {
    let request: AdapterSemanticParityRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter semantic parity request: {error}"))?;
    let receipt = evaluate_adapter_semantic_parity(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter semantic parity receipt: {error}"))
}

pub fn validate_adapter_semantic_parity_json(
    value: &Value,
) -> Result<AdapterSemanticParityReceipt, String> {
    let receipt: AdapterSemanticParityReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter semantic parity receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: SemanticParityError| error.to_string())?;
    if receipt.feature_id != ADAPTER_SEMANTIC_PARITY_FEATURE_ID {
        return Err("adapter semantic parity feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn plan_adapter_scale_frontier_json(value: &Value) -> Result<Value, String> {
    let request: ScaleFrontierRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter scale frontier request: {error}"))?;
    let receipt = plan_adapter_scale_frontier(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter scale frontier receipt: {error}"))
}

pub fn validate_adapter_scale_frontier_json(value: &Value) -> Result<ScaleFrontierReceipt, String> {
    let receipt: ScaleFrontierReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter scale frontier receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: ScaleFrontierError| error.to_string())?;
    if receipt.feature_id != ADAPTER_SCALE_FRONTIER_FEATURE_ID {
        return Err("adapter scale frontier feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn recover_adversarial_events_json(value: &Value) -> Result<Value, String> {
    let request: AdversarialRecoveryRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adversarial recovery request: {error}"))?;
    let receipt = recover_adversarial_events(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adversarial recovery receipt: {error}"))
}

pub fn validate_adversarial_recovery_json(
    value: &Value,
) -> Result<AdversarialRecoveryReceipt, String> {
    let receipt: AdversarialRecoveryReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adversarial recovery receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: AdversarialRecoveryError| error.to_string())?;
    if receipt.feature_id != ADVERSARIAL_RECOVERY_FEATURE_ID {
        return Err("adversarial recovery feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_federated_commons_json(value: &Value) -> Result<Value, String> {
    let request: FederatedCommonsRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated commons request: {error}"))?;
    let receipt = admit_federated_commons(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize federated commons receipt: {error}"))
}

pub fn validate_federated_commons_json(value: &Value) -> Result<FederatedCommonsReceipt, String> {
    let receipt: FederatedCommonsReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid federated commons receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: FederatedCommonsError| error.to_string())?;
    if receipt.feature_id != FEDERATED_COMMONS_FEATURE_ID {
        return Err("federated commons feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn admit_bounded_evolution_json(value: &Value) -> Result<Value, String> {
    let request: BoundedEvolutionRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid bounded evolution request: {error}"))?;
    let receipt = admit_bounded_evolution(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize bounded evolution receipt: {error}"))
}

pub fn validate_bounded_evolution_json(value: &Value) -> Result<BoundedEvolutionReceipt, String> {
    let receipt: BoundedEvolutionReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid bounded evolution receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: BoundedEvolutionError| error.to_string())?;
    if receipt.feature_id != BOUNDED_EVOLUTION_FEATURE_ID {
        return Err("bounded evolution feature id mismatch".into());
    }
    Ok(receipt)
}

pub fn assure_bounded_evolution_json(value: &Value) -> Result<Value, String> {
    let request: EvolutionAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid bounded evolution assurance request: {error}"))?;
    let receipt = assure_bounded_evolution(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize bounded evolution assurance receipt: {error}"))
}

pub fn validate_bounded_evolution_assurance_json(
    value: &Value,
) -> Result<EvolutionAssuranceReceipt, String> {
    let receipt: EvolutionAssuranceReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid bounded evolution assurance receipt: {error}"))?;
    receipt
        .validate()
        .map_err(|error: EvolutionAssuranceError| error.to_string())?;
    if receipt.feature_id != EVOLUTION_ASSURANCE_FEATURE_ID {
        return Err("bounded evolution assurance feature id mismatch".into());
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PRECLINICAL_BOUNDARY;
    use serde_json::json;

    #[test]
    fn unresolved_policy_is_refused_at_mcp_boundary() {
        let result = validate_policy_receipt_json(&json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "receipt_id": "policy:mcp",
            "decision": "allow",
            "reasons": ["unresolved"],
            "evaluated_artifacts": [],
            "authority_reference": null,
            "boundary": PRECLINICAL_BOUNDARY
        }));
        assert!(result.is_err());
    }
}
