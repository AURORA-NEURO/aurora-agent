//! Model Context Protocol server for the FIBER context compiler.
//!
//! Implements blueprint 11.10–11.11 and the MCP half of 43.35 (schemas, APIs and the Cognitive ABI).
//! This is the adoption wedge: an agent in any framework can compile a decision context, descend
//! only as far into the evidence as it needs, and verify a certificate — without linking the
//! engine or learning a new SDK.

mod brain_control;
pub mod research_contracts;
pub mod rpc;
pub mod server;

pub use research_contracts::{
    admit_autonomy_batch_json, compile_evaluation_card_json, evaluate_design_frontier_json, evaluate_multimodal_replication_json, evaluate_quality_drift_json, execute_workflow_batch_json, execute_workflow_json,
    harmonize_multimodal_json, instrument_preflight_json, qualify_analysis_json,
    simulate_protocol_matrix_json, validate_evaluation_card_receipt_json,
    validate_evidence_receipt_json, validate_harmonized_research_object_json,
    validate_instrument_preflight_receipt_json, validate_multimodal_replication_report_json,
    validate_autonomy_batch_receipt_json, validate_design_frontier_receipt_json, validate_quality_drift_receipt_json, validate_workflow_batch_receipt_json,
    validate_policy_receipt_json, validate_protocol_matrix_receipt_json,
    validate_qualified_analysis_result_json, validate_research_release_receipt_json,
    validate_workflow_execution_receipt_json, ANALYSIS_QUALIFICATION_TOOL,
    EVALUATION_OBSERVABILITY_TOOL, INSTRUMENT_PREFLIGHT_TOOL, MULTIMODAL_HARMONIZATION_TOOL,
    AUTONOMY_BATCH_TOOL, DESIGN_FRONTIER_TOOL, MULTIMODAL_REPLICATION_TOOL, PROTOCOL_MATRIX_TOOL, QUALITY_DRIFT_TOOL, RESEARCH_COMPILE_TOOL, WORKFLOW_BATCH_TOOL,
    RESEARCH_CONTRACT_SCHEMA_VERSION, RESEARCH_RELEASE_VALIDATE_TOOL, WORKFLOW_EXECUTION_TOOL,
};
pub use rpc::{Request, Response};
pub use server::{
    resource_definitions, tool_definitions, workspace_capabilities, Lifecycle, Server,
    ADAPTIVE_QUERY_SCHEMA_URI, CAPABILITIES_URI, CERTIFICATE_SCHEMA_URI,
    MISSION_TRACE_SCHEMA_VERSION, PROTOCOL_VERSION, QUERY_SCHEMA_URI, SERVER_NAME,
    WORLD_SCHEMA_URI,
};

use std::io::{BufRead, Write};

/// Runs the stdio event loop until the input stream closes.
pub fn serve<R: BufRead, W: Write>(
    server: &mut Server,
    input: R,
    output: &mut W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match Request::parse(&line) {
            Ok(request) => server.handle(&request),
            Err(failure) => Some(*failure),
        };

        if let Some(response) = response {
            writeln!(output, "{}", response.to_json())?;
            output.flush()?;
        }
    }
    Ok(())
}
