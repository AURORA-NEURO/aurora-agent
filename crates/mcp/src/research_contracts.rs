//! MCP-facing validation for the shared research contracts.
//!
//! The MCP transport accepts JSON, but it does not own scientific semantics. These helpers perform
//! the same schema/boundary/policy checks as the Rust service before a tool result is returned.

use crate::resource_discovery_contract::{
    compile_resource_discovery_contract_v2, ResourceDiscoveryContractRequest,
    ResourceDiscoveryContractResponse, FEATURE_ID as RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
};
use bioprism_adapter::{
    assure_context_compilation as assure_adapter_context_compilation, ContextCompilationReceipt,
    ContextCompilationRequest, CONTEXT_COMPILATION_FEATURE_ID,
};
use bioprism_adapter::{
    compile_evidence_synthesis, EvidenceSynthesisRequest, RetrievalSynthesisReceipt,
    RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
use bioprism_adapter::{
    discover_resources as discover_adapter_resources,
    ResourceCandidate as AdapterResourceCandidate, ResourceNeed as AdapterResourceNeed,
    ResourceWorkbenchReceipt,
    RESOURCE_WORKBENCH_FEATURE_ID as ADAPTER_RESOURCE_WORKBENCH_FEATURE_ID,
};
use bioprism_adapter::{
    evaluate_quality_drift, harmonize_multimodal, HarmonizedResearchObject,
    MultimodalHarmonizationRequest, QualityDriftReceipt, QualityDriftRequest,
    MULTIMODAL_HARMONIZATION_FEATURE_ID, QUALITY_DRIFT_FEATURE_ID,
};
use bioprism_adapter::{
    operate_mechanism_control_plane, MechanismControlPlaneReceipt, MechanismControlPlaneRequest,
    MECHANISM_CONTROL_PLANE_FEATURE_ID,
};
use bioprism_adapter::{
    run_evidence_surveillance, EvidenceFeedRequest, EvidenceSurveillanceReceipt,
    EVIDENCE_SURVEILLANCE_FEATURE_ID,
};
use bioprism_adapter::{
    run_ingestion_gateway, IngestionGatewayReceipt, IngestionGatewayRequest,
    INGESTION_GATEWAY_FEATURE_ID,
};
use bioprism_adapter::{
    run_knowledge_workflow, ClaimsWorkflowRequest, KnowledgeWorkflowReceipt,
    KNOWLEDGE_WORKFLOW_FEATURE_ID,
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
pub const RETRIEVAL_SYNTHESIS_TOOL: &str = "multimodal_retrieval_synthesis";
pub const ADAPTER_CONTEXT_COMPILATION_TOOL: &str = "adapter_context_compilation_assurance";
pub const KNOWLEDGE_WORKFLOW_TOOL: &str = "multimodal_knowledge_workflow";
pub const ADAPTER_RESOURCE_WORKBENCH_TOOL: &str = "adapter_resource_workbench";
pub const INGESTION_GATEWAY_TOOL: &str = "adapter_ingestion_gateway";
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

pub fn assure_adapter_context_compilation_json(value: &Value) -> Result<Value, String> {
    let request: ContextCompilationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid adapter context compilation request: {error}"))?;
    let receipt =
        assure_adapter_context_compilation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize adapter context compilation receipt: {error}"))
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
