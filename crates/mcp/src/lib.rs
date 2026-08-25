//! Model Context Protocol server for the FIBER context compiler.
//!
//! Implements blueprint 11.10–11.11 and the MCP half of 43.35 (schemas, APIs and the Cognitive ABI).
//! This is the adoption wedge: an agent in any framework can compile a decision context, descend
//! only as far into the evidence as it needs, and verify a certificate — without linking the
//! engine or learning a new SDK.

#![recursion_limit = "256"]

mod brain_control;
pub mod research_contracts;
pub mod resource_discovery_contract;
pub mod rpc;
pub mod server;

pub use research_contracts::{
    admit_autonomy_batch_json, admit_computational_execution_json, admit_federated_knowledge_json,
    admit_mechanism_gateway_json, admit_policy_json, assess_protocol_assurance_json,
    assess_release_harness_json, assure_adapter_context_compilation_json,
    assure_context_compilation_json, assure_federated_lens_json, assure_federated_multimodal_json,
    assure_federated_retrieval_json, assure_interpretation_json,
    assure_knowledge_representation_json, assure_provenance_json, assure_release_json,
    assure_replication_json, assure_weavelang_release_json, compile_evaluation_card_json,
    compile_evidence_synthesis_json, compile_experiment_design_json,
    compile_governance_research_release_json, discover_adapter_resources_json,
    discover_resources_json, evaluate_design_frontier_json, evaluate_federated_evaluation_json,
    evaluate_multimodal_replication_json, evaluate_quality_drift_json,
    evaluate_quality_envelope_json, evaluate_semantic_parity_json, execute_workflow_batch_json,
    execute_workflow_json, harmonize_multimodal_json, instrument_preflight_json,
    integrate_instrument_mesh_json, negotiate_determinism_json,
    operate_mechanism_control_plane_json, operate_resource_control_plane_json,
    qualify_analysis_json, qualify_analysis_portfolio_json, resource_discovery_contract_v2_json,
    run_evidence_surveillance_json, run_ingestion_gateway_json, run_knowledge_workflow_json,
    schedule_federation_workflow_json, simulate_protocol_draft_json, simulate_protocol_matrix_json,
    synthesize_federated_continuum_json, validate_adapter_context_compilation_json,
    validate_adapter_resource_workbench_json, validate_analysis_portfolio_json,
    validate_autonomy_batch_receipt_json, validate_computational_execution_json,
    validate_context_compilation_assurance_json, validate_design_frontier_receipt_json,
    validate_determinism_json, validate_evaluation_card_receipt_json,
    validate_evidence_receipt_json, validate_evidence_surveillance_json,
    validate_evidence_synthesis_json, validate_experiment_design_json,
    validate_federated_continual_retrieval_json, validate_federated_evaluation_receipt_json,
    validate_federated_knowledge_gateway_json, validate_federated_lens_assurance_json,
    validate_federated_multimodal_assurance_json, validate_federated_retrieval_assurance_json,
    validate_federation_workflow_json, validate_governance_research_release_json,
    validate_harmonized_research_object_json, validate_ingestion_gateway_json,
    validate_instrument_mesh_json, validate_instrument_preflight_receipt_json,
    validate_interpretation_assurance_json, validate_knowledge_representation_assurance_json,
    validate_knowledge_workflow_json, validate_mechanism_control_plane_json,
    validate_mechanism_gateway_json, validate_multimodal_replication_report_json,
    validate_policy_gateway_json, validate_policy_receipt_json, validate_protocol_assurance_json,
    validate_protocol_matrix_receipt_json, validate_protocol_simulation_json,
    validate_provenance_json, validate_qualified_analysis_result_json,
    validate_qualified_resource_set_json, validate_quality_drift_receipt_json,
    validate_quality_envelope_json, validate_release_assurance_json, validate_release_harness_json,
    validate_replication_assurance_json, validate_research_release_batch_receipt_json,
    validate_research_release_receipt_json, validate_resource_control_plane_json,
    validate_resource_discovery_contract_v2_json, validate_semantic_parity_json,
    validate_weavelang_release_assurance_json, validate_workflow_batch_receipt_json,
    validate_workflow_execution_receipt_json, ADAPTER_CONTEXT_COMPILATION_TOOL,
    ADAPTER_RESOURCE_WORKBENCH_TOOL, ANALYSIS_PORTFOLIO_TOOL, ANALYSIS_QUALIFICATION_TOOL,
    AUTONOMY_BATCH_TOOL, CONTEXT_COMPILATION_ASSURANCE_TOOL, DESIGN_FRONTIER_TOOL,
    DETERMINISM_GATEWAY_TOOL, EVALUATION_OBSERVABILITY_TOOL, EVIDENCE_SURVEILLANCE_TOOL,
    EXECUTION_CONTROL_TOOL, EXPERIMENT_DESIGN_CONTROL_TOOL, FEDERATED_CONTINUAL_RETRIEVAL_TOOL,
    FEDERATED_EVALUATION_TOOL, FEDERATED_KNOWLEDGE_GATEWAY_TOOL, FEDERATED_LENS_ASSURANCE_TOOL,
    FEDERATED_MULTIMODAL_ASSURANCE_TOOL, FEDERATED_RETRIEVAL_ASSURANCE_TOOL,
    FEDERATION_WORKFLOW_TOOL, GOVERNANCE_RESEARCH_RELEASE_TOOL, INGESTION_GATEWAY_TOOL,
    INSTRUMENT_MESH_TOOL, INSTRUMENT_PREFLIGHT_TOOL, INTERPRETATION_ASSURANCE_TOOL,
    KNOWLEDGE_REPRESENTATION_ASSURANCE_TOOL, KNOWLEDGE_WORKFLOW_TOOL, MECHANISM_CONTROL_PLANE_TOOL,
    MECHANISM_GATEWAY_TOOL, MULTIMODAL_HARMONIZATION_TOOL, MULTIMODAL_REPLICATION_TOOL,
    POLICY_GATEWAY_TOOL, PROTOCOL_ASSURANCE_TOOL, PROTOCOL_MATRIX_TOOL, PROTOCOL_SIMULATION_TOOL,
    PROVENANCE_ASSURANCE_TOOL, QUALITY_DRIFT_TOOL, QUALITY_ENVELOPE_TOOL,
    RELEASE_ASSURANCE_HARNESS_TOOL, RELEASE_ASSURANCE_TOOL, REPLICATION_ASSURANCE_TOOL,
    RESEARCH_COMPILE_TOOL, RESEARCH_CONTRACT_SCHEMA_VERSION, RESEARCH_RELEASE_BATCH_VALIDATE_TOOL,
    RESEARCH_RELEASE_VALIDATE_TOOL, RESOURCE_CONTROL_PLANE_TOOL, RESOURCE_DISCOVERY_CONTRACT_TOOL,
    RESOURCE_WORKBENCH_TOOL, RETRIEVAL_SYNTHESIS_TOOL, SEMANTIC_PARITY_TOOL,
    WEAVELANG_RELEASE_ASSURANCE_TOOL, WORKFLOW_BATCH_TOOL, WORKFLOW_EXECUTION_TOOL,
};
pub use resource_discovery_contract::{
    compile_resource_discovery_contract_v2, ResourceDiscoveryContractError,
    ResourceDiscoveryContractRequest, ResourceDiscoveryContractResponse,
    CONTRACT_VERSION as RESOURCE_DISCOVERY_CONTRACT_VERSION,
    FEATURE_ID as RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
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
