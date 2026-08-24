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
    compile_evaluation_card, EvaluationCardReceipt, EvaluationCardRequest,
    EVALUATION_OBSERVABILITY_FEATURE_ID,
};
use serde_json::Value;

/// Stable MCP tool name reserved for the evidence-to-typed-knowledge vertical.
pub const RESEARCH_COMPILE_TOOL: &str = "aurora_research_compile_evidence";
pub const WORKFLOW_EXECUTION_TOOL: &str = "runtime_workflow_execute";
pub const EVALUATION_OBSERVABILITY_TOOL: &str = "evaluation_observability_card";
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
