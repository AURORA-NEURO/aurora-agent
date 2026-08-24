//! MCP-facing validation for the shared research contracts.
//!
//! The MCP transport accepts JSON, but it does not own scientific semantics. These helpers perform
//! the same schema/boundary/policy checks as the Rust service before a tool result is returned.

use bioprism_foundation::{EvidenceReceipt, PolicyReceipt};
use bioprism_runtime::{
    execute_workflow, WorkflowExecutionReceipt, WorkflowExecutionRequest,
    WORKFLOW_EXECUTION_FEATURE_ID,
};
use bioprism_evalengine::{
    compile_evaluation_card, qualify_analysis,
    AnalysisQualificationRequest, EvaluationCardReceipt, EvaluationCardRequest,
    QualifiedAnalysisResult, ANALYSIS_QUALIFICATION_FEATURE_ID,
    EVALUATION_OBSERVABILITY_FEATURE_ID,
};
use bioprism_lab::{
    instrument_preflight, InstrumentPreflightReceipt, InstrumentPreflightRequest,
    INSTRUMENT_PREFLIGHT_FEATURE_ID,
};
use bioprism_adapter::{
    harmonize_multimodal, HarmonizedResearchObject, MultimodalHarmonizationRequest,
    MULTIMODAL_HARMONIZATION_FEATURE_ID,
};
use bioprism_services::{ResearchReleaseReceipt, RESEARCH_RELEASE_FEATURE_ID};
use serde_json::Value;

/// Stable MCP tool name reserved for the evidence-to-typed-knowledge vertical.
pub const RESEARCH_COMPILE_TOOL: &str = "aurora_research_compile_evidence";
pub const WORKFLOW_EXECUTION_TOOL: &str = "runtime_workflow_execute";
pub const EVALUATION_OBSERVABILITY_TOOL: &str = "evaluation_observability_card";
pub const RESEARCH_RELEASE_VALIDATE_TOOL: &str = "research_release_validate";
pub const INSTRUMENT_PREFLIGHT_TOOL: &str = "instrument_preflight";
pub const MULTIMODAL_HARMONIZATION_TOOL: &str = "multimodal_harmonize";
pub const ANALYSIS_QUALIFICATION_TOOL: &str = "analysis_qualify";
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
