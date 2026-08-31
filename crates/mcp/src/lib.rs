#![allow(clippy::all)]

//! Model Context Protocol server for the FIBER context compiler.
//!
//! Implements blueprint 11.10–11.11 and the MCP half of 43.35 (schemas, APIs and the Cognitive ABI).
//! This is the adoption wedge: an agent in any framework can compile a decision context, descend
//! only as far into the evidence as it needs, and verify a certificate — without linking the
//! engine or learning a new SDK.
//!
//! # Not implemented, deliberately
//!
//! * **One transport.** [`serve`] speaks newline-delimited JSON-RPC over a reader and a writer,
//!   which in practice is stdio. There is no HTTP, SSE or WebSocket binding here.
//! * **Six request methods and one notification's worth of protocol.** `initialize`, `ping`,
//!   `tools/list`, `tools/call`, `resources/list` and `resources/read` are answered;
//!   `notifications/initialized` advances the lifecycle and every other notification is dropped
//!   silently. Prompts, sampling, roots, completion and progress are absent, not stubbed.
//! * **No cancellation.** `notifications/cancelled` is one of the notifications dropped, so an
//!   in-flight tool call always runs to completion. The mission executor's own cancellation flag
//!   is internal and is not reachable from the protocol.
//! * **No sessions, no authentication, no per-caller isolation.** A server is one root and one
//!   lifecycle, served to whoever holds the pipe. Confinement to that root
//!   ([`Server::resolve`]) is the only access control in the crate.
//! * **No concurrency.** The event loop reads, answers, and only then reads again. Tool
//!   dispatch runs on its own thread for stack headroom, never for parallelism.
//! * **No state of its own.** Nothing persists across a process except what a tool was
//!   explicitly asked to write to an explicitly named path inside the root.

#![recursion_limit = "256"]

mod brain_control;
pub mod evolution_assurance;
pub mod federated_quality_control_assurance;
pub mod knowledge_representation_contract_model;
pub mod multimodal_ingestion_assurance;
pub mod replication_negative_results_assurance;
pub mod research_contracts;
pub mod resource_discovery_contract;
pub mod rpc;
pub mod server;

pub use evolution_assurance::{
    assure_bounded_evolution, AssuranceCheck, AssuranceVerdict, EvolutionAssuranceError,
    EvolutionAssuranceReceipt, EvolutionAssuranceRequest,
    CONTRACT_VERSION as EVOLUTION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EVOLUTION_ASSURANCE_FEATURE_ID,
    REQUIRED_CHECKS as EVOLUTION_ASSURANCE_REQUIRED_CHECKS, TOOL_NAME as EVOLUTION_ASSURANCE_TOOL,
};
pub use federated_quality_control_assurance::{
    assure_federated_quality, federated_quality_control_manifest, QualityAssuranceError,
    QualityControlRequest5, QualityVerdict7, QualityVerdictDisposition, ResearchObject4,
    CONTENT_TYPE as FEDERATED_QUALITY_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_QUALITY_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_QUALITY_FEATURE_ID, INPUT_SCHEMA as FEDERATED_QUALITY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_QUALITY_OUTPUT_SCHEMA,
};
pub use knowledge_representation_contract_model::{
    knowledge_representation_contract_manifest, model_knowledge_representation_contract,
    model_knowledge_representation_contract_json, validate_knowledge_representation_contract_json,
    KnowledgeClaimAttestation, KnowledgeRepresentationError, KnowledgeRepresentationRequest,
    KnowledgeWorldDisposition, PeerKnowledgeSummary, TypedKnowledgeWorldReceipt,
    CONTRACT_VERSION as KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_REPRESENTATION_CONTRACT_FEATURE_ID,
    TOOL_NAME as KNOWLEDGE_REPRESENTATION_CONTRACT_TOOL,
};
pub use multimodal_ingestion_assurance::{
    assure_multimodal_ingestion, assure_multimodal_ingestion_json,
    multimodal_ingestion_assurance_manifest, validate_multimodal_ingestion_json,
    HarmonizedResearchObjectReceipt, IngestionDisposition, ModalityState, MultimodalIngestionError,
    MultimodalIngestionRequest, PeerModalitySummary, RawModalityAttestation,
    CONTRACT_VERSION as MULTIMODAL_INGESTION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_INGESTION_ASSURANCE_FEATURE_ID,
    TOOL_NAME as MULTIMODAL_INGESTION_ASSURANCE_TOOL,
};
pub use replication_negative_results_assurance::{
    assure_replication, replication_assurance_manifest, ClaimAndProtocol3,
    ReplicationAssuranceError, ReplicationObservation3, ReplicationOutcome, ReplicationRecord7,
    CONTENT_TYPE as REPLICATION_ASSURANCE_CONTENT_TYPE,
    CONTRACT_VERSION as REPLICATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as REPLICATION_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as REPLICATION_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as REPLICATION_ASSURANCE_OUTPUT_SCHEMA,
};
pub use research_contracts::DEVPLAT_MULTIMODAL_LIMITATION_CLOSURE_TOOL;
pub use research_contracts::{
    admit_autonomy_batch_json, admit_bounded_evolution_json, admit_computational_execution_json,
    admit_federated_commons_json, admit_federated_knowledge_json, admit_mechanism_gateway_json,
    admit_policy_json, assess_protocol_assurance_json, assess_release_harness_json,
    assure_adapter_context_compilation_json, assure_bounded_evolution_json,
    assure_context_compilation_json, assure_evaluation_run_json, assure_federated_lens_json,
    assure_federated_multimodal_json, assure_federated_retrieval_json,
    assure_governance_federated_continual_interpretation_json, assure_interpretation_json,
    assure_knowledge_representation_json, assure_multimodal_ingestion_assurance_json,
    assure_provenance_json, assure_registry_knowledge_representation_json,
    assure_registry_scale_frontier_json, assure_release_json, assure_replication_json,
    assure_weavelang_computational_execution_json, assure_weavelang_release_json,
    close_adapter_limitations_json, compile_adapter_capability_manifest_json,
    compile_evaluation_card_json, compile_evidence_synthesis_json, compile_experiment_design_json,
    compile_governance_research_release_json, compile_oraclex_context_json,
    compile_research_workbench_json, discover_adapter_resources_json, discover_resources_json,
    evaluate_adapter_semantic_parity_json, evaluate_design_frontier_json,
    evaluate_federated_evaluation_json, evaluate_multimodal_replication_json,
    evaluate_quality_drift_json, evaluate_quality_envelope_json, evaluate_semantic_parity_json,
    execute_workflow_batch_json, execute_workflow_json, harmonize_multimodal_json,
    infer_adapter_dependency_composition_json, instrument_preflight_json,
    integrate_instrument_mesh_json, model_mcp_knowledge_representation_contract_json,
    negotiate_determinism_json, negotiate_interoperability_json,
    operate_bioworlds_federated_context_research_workbench_json,
    operate_devplat_multimodal_limitation_closure_assurance_json,
    operate_devx_context_compilation_contract_json,
    operate_ids_interpretation_visualization_assurance_json,
    operate_influence_local_evidence_surveillance_assurance_json,
    operate_mechanism_control_plane_json, operate_megafactory_mechanism_exploration_json,
    operate_ops_context_compilation_json, operate_registry_replication_workbench_json,
    operate_resource_control_plane_json, operate_routing_laboratory_inference_json,
    plan_adapter_scale_frontier_json, plan_reliable_capability_json, qualify_analysis_json,
    qualify_analysis_portfolio_json, recover_adversarial_events_json,
    resource_discovery_contract_v2_json, run_atlashub_mechanism_exploration_assurance_json,
    run_dataops_provenance_signing_workflow_fabric_json, run_evidence_surveillance_json,
    run_federated_continual_evidence_surveillance_research_copilot_json,
    run_federated_continual_evidence_surveillance_research_workbench_json,
    run_federated_continual_evidence_surveillance_workflow_fabric_json,
    run_federated_continual_interpretation_json,
    run_federated_continual_retrieval_synthesis_assurance_harness_json,
    run_federated_continual_retrieval_synthesis_federated_control_plane_json,
    run_federated_continual_retrieval_synthesis_interoperability_gateway_json,
    run_federated_continual_retrieval_synthesis_research_copilot_json,
    run_federated_continual_retrieval_synthesis_research_workbench_json,
    run_federated_continual_retrieval_synthesis_workflow_json,
    run_federated_retrieval_synthesis_contract_model_json,
    run_federated_retrieval_synthesis_inference_engine_json,
    run_foundation_mechanism_exploration_assurance_json, run_ingestion_gateway_json,
    run_interweave_frontier_control_json, run_knowledge_workflow_json,
    run_local_evidence_surveillance_research_copilot_json,
    run_local_evidence_surveillance_research_workbench_json,
    run_local_evidence_surveillance_workflow_fabric_json,
    run_local_retrieval_synthesis_assurance_harness_json,
    run_local_retrieval_synthesis_contract_model_json,
    run_local_retrieval_synthesis_federated_control_plane_json,
    run_local_retrieval_synthesis_inference_engine_json,
    run_local_retrieval_synthesis_interoperability_gateway_json,
    run_local_retrieval_synthesis_research_copilot_json,
    run_local_retrieval_synthesis_research_workbench_json,
    run_local_retrieval_synthesis_workflow_json,
    run_multimodal_evidence_surveillance_research_copilot_json,
    run_multimodal_evidence_surveillance_research_workbench_json,
    run_multimodal_evidence_surveillance_workflow_fabric_json,
    run_multimodal_retrieval_synthesis_assurance_harness_json,
    run_multimodal_retrieval_synthesis_federated_control_plane_json,
    run_multimodal_retrieval_synthesis_inference_engine_json,
    run_multimodal_retrieval_synthesis_interoperability_gateway_json,
    run_multimodal_retrieval_synthesis_research_copilot_json,
    run_multimodal_retrieval_synthesis_research_workbench_json,
    run_multimodal_retrieval_synthesis_workflow_json,
    run_obligation_knowledge_representation_assurance_json,
    run_obligation_security_federation_interoperability_gateway_json,
    run_oraclex_interpretation_inference_json,
    run_oraclex_performance_reliability_interoperability_gateway_json,
    run_oraclex_publication_release_json, run_oraclex_statistical_analysis_research_workbench_json,
    run_throughput_evidence_surveillance_research_copilot_json,
    run_throughput_evidence_surveillance_research_workbench_json,
    run_throughput_evidence_surveillance_workflow_fabric_json,
    run_throughput_retrieval_synthesis_assurance_harness_json,
    run_throughput_retrieval_synthesis_contract_model_json,
    run_throughput_retrieval_synthesis_federated_control_plane_json,
    run_throughput_retrieval_synthesis_inference_engine_json,
    run_throughput_retrieval_synthesis_interoperability_gateway_json,
    run_throughput_retrieval_synthesis_research_copilot_json,
    run_throughput_retrieval_synthesis_research_workbench_json,
    run_throughput_retrieval_synthesis_workflow_json, runtime_interpretation_assurance_json,
    schedule_federation_workflow_json, simulate_protocol_draft_json, simulate_protocol_matrix_json,
    synthesize_federated_continuum_json, validate_adapter_context_compilation_json,
    validate_adapter_resource_workbench_json, validate_adapter_scale_frontier_json,
    validate_adapter_semantic_parity_json, validate_adversarial_recovery_json,
    validate_analysis_portfolio_json, validate_atlashub_mechanism_exploration_assurance_json,
    validate_autonomy_batch_receipt_json,
    validate_bioworlds_federated_context_research_workbench_json,
    validate_bounded_evolution_assurance_json, validate_bounded_evolution_json,
    validate_computational_execution_json, validate_context_compilation_assurance_json,
    validate_contract_frontier_json, validate_dataops_provenance_signing_workflow_fabric_json,
    validate_dependency_composition_json, validate_design_frontier_receipt_json,
    validate_determinism_json, validate_devplat_multimodal_limitation_closure_assurance_json,
    validate_devx_context_compilation_contract_json, validate_evaluation_assurance_json,
    validate_evaluation_card_receipt_json, validate_evidence_receipt_json,
    validate_evidence_surveillance_json, validate_evidence_synthesis_json,
    validate_experiment_design_json, validate_federated_commons_json,
    validate_federated_continual_evidence_surveillance_research_copilot_json,
    validate_federated_continual_evidence_surveillance_research_workbench_json,
    validate_federated_continual_evidence_surveillance_workflow_fabric_json,
    validate_federated_continual_interpretation_json, validate_federated_continual_retrieval_json,
    validate_federated_continual_retrieval_synthesis_assurance_harness_json,
    validate_federated_continual_retrieval_synthesis_federated_control_plane_json,
    validate_federated_continual_retrieval_synthesis_interoperability_gateway_json,
    validate_federated_continual_retrieval_synthesis_research_copilot_json,
    validate_federated_continual_retrieval_synthesis_research_workbench_json,
    validate_federated_continual_retrieval_synthesis_workflow_json,
    validate_federated_evaluation_receipt_json, validate_federated_knowledge_gateway_json,
    validate_federated_lens_assurance_json, validate_federated_multimodal_assurance_json,
    validate_federated_retrieval_assurance_json,
    validate_federated_retrieval_synthesis_contract_model_json,
    validate_federated_retrieval_synthesis_inference_engine_json,
    validate_federation_workflow_json, validate_foundation_mechanism_exploration_assurance_json,
    validate_governance_federated_continual_interpretation_json,
    validate_governance_research_release_json, validate_harmonized_research_object_json,
    validate_ids_interpretation_visualization_assurance_json,
    validate_influence_local_evidence_surveillance_assurance_json, validate_ingestion_gateway_json,
    validate_instrument_mesh_json, validate_instrument_preflight_receipt_json,
    validate_interoperability_gateway_json, validate_interpretation_assurance_json,
    validate_interweave_frontier_control_json, validate_knowledge_representation_assurance_json,
    validate_knowledge_workflow_json, validate_limitation_closure_json,
    validate_local_evidence_surveillance_research_copilot_json,
    validate_local_evidence_surveillance_research_workbench_json,
    validate_local_evidence_surveillance_workflow_fabric_json,
    validate_local_retrieval_synthesis_assurance_harness_json,
    validate_local_retrieval_synthesis_contract_model_json,
    validate_local_retrieval_synthesis_federated_control_plane_json,
    validate_local_retrieval_synthesis_inference_engine_json,
    validate_local_retrieval_synthesis_interoperability_gateway_json,
    validate_local_retrieval_synthesis_research_copilot_json,
    validate_local_retrieval_synthesis_research_workbench_json,
    validate_local_retrieval_synthesis_workflow_json,
    validate_mcp_knowledge_representation_contract_json, validate_mechanism_control_plane_json,
    validate_mechanism_gateway_json, validate_megafactory_mechanism_exploration_json,
    validate_multimodal_evidence_surveillance_research_copilot_json,
    validate_multimodal_evidence_surveillance_research_workbench_json,
    validate_multimodal_evidence_surveillance_workflow_fabric_json,
    validate_multimodal_ingestion_assurance_json, validate_multimodal_replication_report_json,
    validate_multimodal_retrieval_synthesis_assurance_harness_json,
    validate_multimodal_retrieval_synthesis_federated_control_plane_json,
    validate_multimodal_retrieval_synthesis_inference_engine_json,
    validate_multimodal_retrieval_synthesis_interoperability_gateway_json,
    validate_multimodal_retrieval_synthesis_research_copilot_json,
    validate_multimodal_retrieval_synthesis_research_workbench_json,
    validate_multimodal_retrieval_synthesis_workflow_json,
    validate_obligation_knowledge_representation_assurance_json,
    validate_obligation_security_federation_interoperability_gateway_json,
    validate_ops_context_compilation_json, validate_oraclex_context_json,
    validate_oraclex_interpretation_inference_json,
    validate_oraclex_performance_reliability_interoperability_gateway_json,
    validate_oraclex_publication_release_json,
    validate_oraclex_statistical_analysis_research_workbench_json, validate_policy_gateway_json,
    validate_policy_receipt_json, validate_protocol_assurance_json,
    validate_protocol_matrix_receipt_json, validate_protocol_simulation_json,
    validate_provenance_json, validate_qualified_analysis_result_json,
    validate_qualified_resource_set_json, validate_quality_drift_receipt_json,
    validate_quality_envelope_json, validate_registry_knowledge_representation_json,
    validate_registry_replication_workbench_json, validate_registry_scale_frontier_json,
    validate_release_assurance_json, validate_release_harness_json,
    validate_reliability_copilot_json, validate_replication_assurance_json,
    validate_research_release_batch_receipt_json, validate_research_release_receipt_json,
    validate_research_workbench_json, validate_resource_control_plane_json,
    validate_resource_discovery_contract_v2_json, validate_routing_laboratory_inference_json,
    validate_runtime_interpretation_assurance_json, validate_semantic_parity_json,
    validate_throughput_evidence_surveillance_research_copilot_json,
    validate_throughput_evidence_surveillance_research_workbench_json,
    validate_throughput_evidence_surveillance_workflow_fabric_json,
    validate_throughput_retrieval_synthesis_assurance_harness_json,
    validate_throughput_retrieval_synthesis_contract_model_json,
    validate_throughput_retrieval_synthesis_federated_control_plane_json,
    validate_throughput_retrieval_synthesis_inference_engine_json,
    validate_throughput_retrieval_synthesis_interoperability_gateway_json,
    validate_throughput_retrieval_synthesis_research_copilot_json,
    validate_throughput_retrieval_synthesis_research_workbench_json,
    validate_throughput_retrieval_synthesis_workflow_json,
    validate_weavelang_computational_execution_json, validate_weavelang_release_assurance_json,
    validate_workflow_batch_receipt_json, validate_workflow_execution_receipt_json,
    ADAPTER_CONTEXT_COMPILATION_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_TOOL,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_TOOL,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_TOOL,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_TOOL,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_TOOL,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_TOOL,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_TOOL,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL,
    ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_TOOL,
    ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_TOOL,
    ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_TOOL,
    ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_TOOL,
    ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_TOOL, ADAPTER_RESOURCE_WORKBENCH_TOOL,
    ADAPTER_SCALE_FRONTIER_TOOL, ADAPTER_SEMANTIC_PARITY_TOOL,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_TOOL,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL,
    ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_TOOL,
    ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_TOOL,
    ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_TOOL,
    ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_TOOL,
    ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_TOOL, ADVERSARIAL_RECOVERY_TOOL,
    ANALYSIS_PORTFOLIO_TOOL, ANALYSIS_QUALIFICATION_TOOL,
    ATLASHUB_MECHANISM_EXPLORATION_ASSURANCE_TOOL, AUTONOMY_BATCH_TOOL, BOUNDED_EVOLUTION_TOOL,
    CONTEXT_COMPILATION_ASSURANCE_TOOL, CONTRACT_FRONTIER_TOOL,
    DATAOPS_PROVENANCE_SIGNING_WORKFLOW_FABRIC_TOOL, DEPENDENCY_COMPOSITION_TOOL,
    DESIGN_FRONTIER_TOOL, DETERMINISM_GATEWAY_TOOL, EVALUATION_ASSURANCE_TOOL,
    EVALUATION_OBSERVABILITY_TOOL, EVIDENCE_SURVEILLANCE_TOOL, EXECUTION_CONTROL_TOOL,
    EXPERIMENT_DESIGN_CONTROL_TOOL, FEDERATED_COMMONS_TOOL, FEDERATED_CONTINUAL_RETRIEVAL_TOOL,
    FEDERATED_EVALUATION_TOOL, FEDERATED_KNOWLEDGE_GATEWAY_TOOL, FEDERATED_LENS_ASSURANCE_TOOL,
    FEDERATED_MULTIMODAL_ASSURANCE_TOOL, FEDERATED_RETRIEVAL_ASSURANCE_TOOL,
    FEDERATION_WORKFLOW_TOOL, FOUNDATION_MECHANISM_EXPLORATION_ASSURANCE_TOOL,
    GOVERNANCE_RESEARCH_RELEASE_TOOL, IDS_INTERPRETATION_VISUALIZATION_ASSURANCE_TOOL,
    INGESTION_GATEWAY_TOOL, INSTRUMENT_MESH_TOOL, INSTRUMENT_PREFLIGHT_TOOL,
    INTEROPERABILITY_GATEWAY_TOOL, INTERPRETATION_ASSURANCE_TOOL, INTERWEAVE_FRONTIER_CONTROL_TOOL,
    KNOWLEDGE_REPRESENTATION_ASSURANCE_TOOL, KNOWLEDGE_WORKFLOW_TOOL, LIMITATION_CLOSURE_TOOL,
    MECHANISM_CONTROL_PLANE_TOOL, MECHANISM_GATEWAY_TOOL, MULTIMODAL_HARMONIZATION_TOOL,
    MULTIMODAL_REPLICATION_TOOL, OBLIGATION_KNOWLEDGE_REPRESENTATION_ASSURANCE_TOOL,
    OBLIGATION_SECURITY_FEDERATION_INTEROPERABILITY_GATEWAY_TOOL,
    ORACLEX_INTERPRETATION_INFERENCE_TOOL,
    ORACLEX_PERFORMANCE_RELIABILITY_INTEROPERABILITY_GATEWAY_TOOL,
    ORACLEX_PUBLICATION_RELEASE_TOOL, ORACLEX_STATISTICAL_ANALYSIS_RESEARCH_WORKBENCH_TOOL,
    POLICY_GATEWAY_TOOL, PROTOCOL_ASSURANCE_TOOL, PROTOCOL_MATRIX_TOOL, PROTOCOL_SIMULATION_TOOL,
    PROVENANCE_ASSURANCE_TOOL, QUALITY_DRIFT_TOOL, QUALITY_ENVELOPE_TOOL,
    RELEASE_ASSURANCE_HARNESS_TOOL, RELEASE_ASSURANCE_TOOL, RELIABILITY_COPILOT_TOOL,
    REPLICATION_ASSURANCE_TOOL, RESEARCH_COMPILE_TOOL, RESEARCH_CONTRACT_SCHEMA_VERSION,
    RESEARCH_RELEASE_BATCH_VALIDATE_TOOL, RESEARCH_RELEASE_VALIDATE_TOOL, RESEARCH_WORKBENCH_TOOL,
    RESOURCE_CONTROL_PLANE_TOOL, RESOURCE_DISCOVERY_CONTRACT_TOOL, RESOURCE_WORKBENCH_TOOL,
    RETRIEVAL_SYNTHESIS_TOOL, RUNTIME_INTERPRETATION_ASSURANCE_TOOL, SEMANTIC_PARITY_TOOL,
    WEAVELANG_RELEASE_ASSURANCE_TOOL, WORKFLOW_BATCH_TOOL, WORKFLOW_EXECUTION_TOOL,
};
pub use research_contracts::{
    authorize_worldfactory_computational_execution_json,
    validate_worldfactory_computational_execution_json, WORLDFACTORY_COMPUTATIONAL_EXECUTION_TOOL,
};
pub use research_contracts::{
    interoperate_ids_resources_json, validate_ids_resource_interoperability_json,
    IDS_RESOURCE_INTEROPERABILITY_TOOL,
};
pub use research_contracts::{
    mcp_replication_negative_results_assurance_manifest_json,
    run_mcp_replication_negative_results_assurance_json,
    validate_mcp_replication_negative_results_assurance_json,
    MCP_REPLICATION_NEGATIVE_RESULTS_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_adapter_federated_context_copilot_json,
    validate_adapter_federated_context_copilot_json, ADAPTER_FEDERATED_CONTEXT_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_atlashub_quality_control_contract_model_json,
    validate_atlashub_quality_control_contract_model_json,
    ATLASHUB_QUALITY_CONTROL_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    operate_atlashub_quality_control_copilot_json, validate_atlashub_quality_control_copilot_json,
    ATLASHUB_QUALITY_CONTROL_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_atlashub_replication_control_json, validate_atlashub_replication_control_json,
    ATLASHUB_REPLICATION_CONTROL_TOOL,
};
pub use research_contracts::{
    operate_atlasx_computational_execution_assurance_json,
    validate_atlasx_computational_execution_assurance_json,
    ATLASX_COMPUTATIONAL_EXECUTION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_atlasx_context_compilation_json, validate_atlasx_context_compilation_json,
    ATLASX_CONTEXT_COMPILATION_TOOL,
};
pub use research_contracts::{
    operate_atlasx_federated_execution_json, validate_atlasx_federated_execution_json,
    ATLASX_FEDERATED_EXECUTION_TOOL,
};
pub use research_contracts::{
    operate_atlasx_mechanism_contract_json, validate_atlasx_mechanism_contract_json,
    ATLASX_MECHANISM_CONTRACT_TOOL,
};
pub use research_contracts::{
    operate_bioethics_evidence_surveillance_json, validate_bioethics_evidence_surveillance_json,
    BIOETHICS_EVIDENCE_SURVEILLANCE_TOOL,
};
pub use research_contracts::{
    operate_bioethics_prospective_computational_execution_json,
    validate_bioethics_prospective_computational_execution_json,
    BIOETHICS_PROSPECTIVE_COMPUTATIONAL_EXECUTION_TOOL,
};
pub use research_contracts::{
    operate_bioethics_scale_frontier_json, validate_bioethics_scale_frontier_json,
    BIOETHICS_SCALE_FRONTIER_TOOL,
};
pub use research_contracts::{
    operate_bioworlds_knowledge_workflow_json, validate_bioworlds_knowledge_workflow_json,
    BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_TOOL, BIOWORLDS_KNOWLEDGE_WORKFLOW_TOOL,
};
pub use research_contracts::{
    operate_bioworlds_resource_discovery_json, validate_bioworlds_resource_discovery_json,
    BIOWORLDS_RESOURCE_DISCOVERY_TOOL,
};
pub use research_contracts::{
    operate_docgraph_instrument_action_json, validate_docgraph_instrument_action_json,
    DOCGRAPH_INSTRUMENT_ACTION_TOOL,
};
pub use research_contracts::{
    operate_epistemic_experiment_design_research_workbench_json,
    validate_epistemic_experiment_design_research_workbench_json,
    EPISTEMIC_EXPERIMENT_DESIGN_RESEARCH_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_epistemic_retrieval_synthesis_json, validate_epistemic_retrieval_synthesis_json,
    EPISTEMIC_RETRIEVAL_SYNTHESIS_TOOL,
};
pub use research_contracts::{
    operate_evalengine_local_mechanism_exploration_assurance_json,
    validate_evalengine_local_mechanism_exploration_assurance_json,
    EVALENGINE_LOCAL_MECHANISM_EXPLORATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_evalengine_protocol_simulation_copilot_json,
    validate_evalengine_protocol_simulation_copilot_json,
    EVALENGINE_PROTOCOL_SIMULATION_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_factory_federated_quality_workbench_json,
    validate_factory_federated_quality_workbench_json, FACTORY_FEDERATED_QUALITY_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_factory_prospective_evidence_json, validate_factory_prospective_evidence_json,
    FACTORY_PROSPECTIVE_EVIDENCE_TOOL,
};
pub use research_contracts::{
    operate_federated_quality_control_json, validate_federated_quality_control_json,
    FEDERATED_QUALITY_CONTROL_TOOL,
};
pub use research_contracts::{
    operate_fiber_federated_analysis_json, validate_fiber_federated_analysis_json,
    FIBER_FEDERATED_ANALYSIS_TOOL,
};
pub use research_contracts::{
    operate_fiber_federated_resource_json, validate_fiber_federated_resource_json,
    FIBER_FEDERATED_RESOURCE_TOOL,
};
pub use research_contracts::{
    operate_ids_adversarial_recovery_json, validate_ids_adversarial_recovery_json,
    IDS_ADVERSARIAL_RECOVERY_TOOL,
};
pub use research_contracts::{
    operate_ids_bounded_evolution_json, validate_ids_bounded_evolution_json,
    IDS_BOUNDED_EVOLUTION_TOOL,
};
pub use research_contracts::{
    operate_ids_computational_execution_json, validate_ids_computational_execution_json,
    IDS_COMPUTATIONAL_EXECUTION_TOOL,
};
pub use research_contracts::{
    operate_ids_context_compilation_json, validate_ids_context_compilation_json,
    IDS_CONTEXT_COMPILATION_TOOL,
};
pub use research_contracts::{
    operate_ids_contract_frontier_json, validate_ids_contract_frontier_json,
    IDS_CONTRACT_FRONTIER_TOOL,
};
pub use research_contracts::{
    operate_ids_dependency_composition_json, validate_ids_dependency_composition_json,
    IDS_DEPENDENCY_COMPOSITION_TOOL,
};
pub use research_contracts::{
    operate_ids_evaluation_json, validate_ids_evaluation_json, IDS_EVALUATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_ids_experiment_design_json, validate_ids_experiment_design_json,
    IDS_EXPERIMENT_DESIGN_TOOL,
};
pub use research_contracts::{
    operate_ids_federated_commons_json, validate_ids_federated_commons_json,
    IDS_FEDERATED_COMMONS_TOOL,
};
pub use research_contracts::{
    operate_ids_federated_workflow_json, validate_ids_federated_workflow_json,
    IDS_FEDERATED_WORKFLOW_TOOL,
};
pub use research_contracts::{
    operate_ids_federation_security_json, validate_ids_federation_security_json,
    IDS_FEDERATION_SECURITY_TOOL,
};
pub use research_contracts::{
    operate_ids_interoperability_extensibility_json,
    validate_ids_interoperability_extensibility_json, IDS_INTEROPERABILITY_EXTENSIBILITY_TOOL,
};
pub use research_contracts::{
    operate_ids_interoperability_json, validate_ids_interoperability_json,
    IDS_INTEROPERABILITY_GATEWAY_TOOL,
};
pub use research_contracts::{
    operate_ids_knowledge_representation_json, validate_ids_knowledge_representation_json,
    IDS_KNOWLEDGE_REPRESENTATION_TOOL,
};
pub use research_contracts::{
    operate_ids_laboratory_integration_json, validate_ids_laboratory_integration_json,
    IDS_LABORATORY_INTEGRATION_TOOL,
};
pub use research_contracts::{
    operate_ids_limitation_closure_json, validate_ids_limitation_closure_json,
    IDS_LIMITATION_CLOSURE_TOOL,
};
pub use research_contracts::{
    operate_ids_mechanism_exploration_json, validate_ids_mechanism_exploration_json,
    IDS_MECHANISM_EXPLORATION_TOOL,
};
pub use research_contracts::{
    operate_ids_multimodal_ingestion_json, validate_ids_multimodal_ingestion_json,
    IDS_MULTIMODAL_INGESTION_TOOL,
};
pub use research_contracts::{
    operate_ids_performance_reliability_json, validate_ids_performance_reliability_json,
    IDS_PERFORMANCE_RELIABILITY_TOOL,
};
pub use research_contracts::{
    operate_ids_policy_autonomy_json, validate_ids_policy_autonomy_json, IDS_POLICY_AUTONOMY_TOOL,
};
pub use research_contracts::{
    operate_ids_policy_autonomy_workbench_json, validate_ids_policy_autonomy_workbench_json,
    IDS_POLICY_AUTONOMY_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_ids_prospective_provenance_json, validate_ids_prospective_provenance_json,
    IDS_PROSPECTIVE_PROVENANCE_TOOL,
};
pub use research_contracts::{
    operate_ids_protocol_simulation_json, validate_ids_protocol_simulation_json,
    IDS_PROTOCOL_SIMULATION_TOOL,
};
pub use research_contracts::{
    operate_ids_provenance_signing_json, validate_ids_provenance_signing_json,
    IDS_PROVENANCE_SIGNING_TOOL,
};
pub use research_contracts::{
    operate_ids_publication_release_json, validate_ids_publication_release_json,
    IDS_PUBLICATION_RELEASE_TOOL,
};
pub use research_contracts::{
    operate_ids_quality_control_json, validate_ids_quality_control_json, IDS_QUALITY_CONTROL_TOOL,
};
pub use research_contracts::{
    operate_ids_reliability_json, validate_ids_reliability_json, IDS_RELIABILITY_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_ids_replication_interoperability_json, validate_ids_replication_interoperability_json,
    IDS_REPLICATION_INTEROPERABILITY_TOOL,
};
pub use research_contracts::{
    operate_ids_research_workbench_json, validate_ids_research_workbench_json,
    IDS_RESEARCH_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_ids_retrieval_synthesis_assurance_json,
    validate_ids_retrieval_synthesis_assurance_json, IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_ids_scale_frontier_json, validate_ids_scale_frontier_json, IDS_SCALE_FRONTIER_TOOL,
};
pub use research_contracts::{
    operate_ids_semantic_parity_json, validate_ids_semantic_parity_json, IDS_SEMANTIC_PARITY_TOOL,
};
pub use research_contracts::{
    operate_ids_statistical_causal_ml_json, validate_ids_statistical_causal_ml_json,
    IDS_STATISTICAL_CAUSAL_ML_TOOL,
};
pub use research_contracts::{
    operate_ids_typed_determinism_assurance_json, validate_ids_typed_determinism_assurance_json,
    IDS_TYPED_DETERMINISM_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_ids_typed_determinism_json, validate_ids_typed_determinism_json,
    IDS_TYPED_DETERMINISM_TOOL,
};
pub use research_contracts::{
    operate_interweave_federated_commons_assurance_json,
    validate_interweave_federated_commons_assurance_json,
    INTERWEAVE_FEDERATED_COMMONS_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_interweave_federated_interpretation_json,
    validate_interweave_federated_interpretation_json, INTERWEAVE_FEDERATED_INTERPRETATION_TOOL,
};
pub use research_contracts::{
    operate_lab_instrument_interoperability_json, validate_lab_instrument_interoperability_json,
    LABORATORY_INTEGRATION_TOOL,
};
pub use research_contracts::{
    operate_lens_provenance_signing_json, validate_lens_provenance_signing_json,
    LENS_PROVENANCE_SIGNING_TOOL,
};
pub use research_contracts::{
    operate_mutation_federated_continual_bounded_evolution_assurance_json,
    validate_mutation_federated_continual_bounded_evolution_assurance_json,
    MUTATION_FEDERATED_EVOLUTION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_mutation_federated_resource_discovery_json,
    validate_mutation_federated_resource_discovery_json,
    MUTATION_RESOURCE_DISCOVERY_CONTROL_PLANE_TOOL,
};
pub use research_contracts::{
    operate_mutation_publication_release_json, validate_mutation_publication_release_json,
    MUTATION_PUBLICATION_RELEASE_TOOL,
};
pub use research_contracts::{
    operate_obligation_prospective_release_json, validate_obligation_prospective_release_json,
    OBLIGATION_PROSPECTIVE_RELEASE_TOOL,
};
pub use research_contracts::{
    operate_onco_federated_provenance_json, validate_onco_federated_provenance_json,
    ONCO_FEDERATED_PROVENANCE_TOOL,
};
pub use research_contracts::{
    operate_onco_instrument_research_workbench_json,
    validate_onco_instrument_research_workbench_json, ONCO_INSTRUMENT_RESEARCH_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_oncoworlds_analysis_workbench_json, validate_oncoworlds_analysis_workbench_json,
    ONCOWORLDS_ANALYSIS_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_oncoworlds_evidence_surveillance_copilot_json,
    validate_oncoworlds_evidence_surveillance_copilot_json,
    ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_oncoworlds_replication_assurance_json, validate_oncoworlds_replication_assurance_json,
    ONCOWORLDS_REPLICATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_oncoworlds_resource_discovery_assurance_json,
    validate_oncoworlds_resource_discovery_assurance_json,
    ONCOWORLDS_RESOURCE_DISCOVERY_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_packs_local_quality_control_json, validate_packs_local_quality_control_json,
    PACKS_LOCAL_QUALITY_CONTROL_ASSURANCE_TOOL,
};
pub use research_contracts::{
    operate_policy_analysis_copilot_json, validate_policy_analysis_copilot_json,
    POLICY_ANALYSIS_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_prism_analysis_workbench_json, validate_prism_analysis_workbench_json,
    PRISM_ANALYSIS_WORKBENCH_TOOL,
};
pub use research_contracts::{
    operate_retrieval_synthesis_operations_json, validate_retrieval_synthesis_operations_json,
    RETRIEVAL_SYNTHESIS_OPERATIONS_TOOL,
};
pub use research_contracts::{
    operate_routing_execution_copilot_json, validate_routing_execution_copilot_json,
    ROUTING_EXECUTION_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_routing_limitation_closure_json, validate_routing_limitation_closure_json,
    ROUTING_LIMITATION_CLOSURE_TOOL,
};
pub use research_contracts::{
    operate_safety_prospective_laboratory_integration_assurance_json,
    validate_safety_prospective_laboratory_integration_assurance_json,
    SAFETY_PROSPECTIVE_LABORATORY_INTEGRATION_TOOL,
};
pub use research_contracts::{
    operate_scale_federation_trust_json, validate_scale_federation_trust_json,
    SCALE_FEDERATION_TRUST_TOOL,
};
pub use research_contracts::{
    operate_services_context_compilation_copilot_json,
    validate_services_context_compilation_copilot_json, SERVICES_CONTEXT_COMPILATION_COPILOT_TOOL,
};
pub use research_contracts::{
    operate_services_multimodal_interpretation_json,
    validate_services_multimodal_interpretation_json, SERVICES_MULTIMODAL_INTERPRETATION_TOOL,
};
pub use research_contracts::{
    operate_worldgen_multimodal_execution_json, validate_worldgen_multimodal_execution_json,
    WORLDGEN_MULTIMODAL_EXECUTION_TOOL,
};
pub use research_contracts::{
    operate_worldgen_multimodal_ingestion_json, validate_worldgen_multimodal_ingestion_json,
    WORLDGEN_MULTIMODAL_INGESTION_TOOL,
};
pub use research_contracts::{
    run_atlashub_provenance_signing_inference_engine_json,
    validate_atlashub_provenance_signing_inference_engine_json,
    ATLASHUB_PROVENANCE_SIGNING_INFERENCE_ENGINE_TOOL,
};
pub use research_contracts::{
    run_backends_federated_retrieval_synthesis_workflow_json,
    validate_backends_federated_retrieval_synthesis_workflow_json,
    BACKENDS_FEDERATED_RETRIEVAL_SYNTHESIS_WORKFLOW_TOOL,
};
pub use research_contracts::{
    run_bioethics_experiment_design_workflow_fabric_json,
    validate_bioethics_experiment_design_workflow_fabric_json,
    BIOETHICS_EXPERIMENT_DESIGN_WORKFLOW_FABRIC_TOOL,
};
pub use research_contracts::{
    run_bioethics_multimodal_bounded_evolution_assurance_json,
    validate_bioethics_multimodal_bounded_evolution_assurance_json,
    BIOETHICS_MULTIMODAL_BOUNDED_EVOLUTION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_bioethics_multimodal_context_compilation_json,
    validate_bioethics_multimodal_context_compilation_json,
    BIOETHICS_MULTIMODAL_CONTEXT_COMPILATION_TOOL,
};
pub use research_contracts::{
    run_bioethics_statistical_analysis_assurance_json,
    validate_bioethics_statistical_analysis_assurance_json,
    BIOETHICS_STATISTICAL_ANALYSIS_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_conformance_context_compilation_assurance_json,
    validate_conformance_context_compilation_assurance_json,
    CONFORMANCE_CONTEXT_COMPILATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_conformance_retrieval_synthesis_contract_model_json,
    validate_conformance_retrieval_synthesis_contract_model_json,
    CONFORMANCE_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    run_context_compilation_federated_control_json,
    validate_context_compilation_federated_control_json,
    CONTEXT_COMPILATION_FEDERATED_CONTROL_TOOL,
};
pub use research_contracts::{
    run_devx_evidence_surveillance_control_json, validate_devx_evidence_surveillance_control_json,
    DEVX_EVIDENCE_SURVEILLANCE_CONTROL_TOOL,
};
pub use research_contracts::{
    run_fabric_experiment_design_contract_model_json,
    validate_fabric_experiment_design_contract_model_json,
    FABRIC_EXPERIMENT_DESIGN_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    run_fabric_experiment_design_interoperability_gateway_json,
    validate_fabric_experiment_design_interoperability_gateway_json,
    FABRIC_EXPERIMENT_DESIGN_INTEROPERABILITY_GATEWAY_TOOL,
};
pub use research_contracts::{
    run_federated_publication_release_inference_json,
    validate_federated_publication_release_inference_json,
    FEDERATED_PUBLICATION_RELEASE_INFERENCE_TOOL,
};
pub use research_contracts::{
    run_governance_experiment_design_assurance_json,
    validate_governance_experiment_design_assurance_json,
    GOVERNANCE_EXPERIMENT_DESIGN_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_hub_policy_autonomy_inference_engine_json,
    validate_hub_policy_autonomy_inference_engine_json, HUB_POLICY_AUTONOMY_INFERENCE_ENGINE_TOOL,
};
pub use research_contracts::{
    run_hubapi_experiment_design_assurance_json, validate_hubapi_experiment_design_assurance_json,
    HUBAPI_EXPERIMENT_DESIGN_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_ids_local_evidence_surveillance_inference_json,
    validate_ids_local_evidence_surveillance_inference_json,
    IDS_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_TOOL,
};
pub use research_contracts::{
    run_lab_federated_experiment_design_interoperability_gateway_json,
    validate_lab_federated_experiment_design_interoperability_gateway_json,
    LAB_EXPERIMENT_DESIGN_INTEROPERABILITY_GATEWAY_TOOL,
};
pub use research_contracts::{
    run_mutation_knowledge_federated_control_json,
    validate_mutation_knowledge_federated_control_json, MUTATION_KNOWLEDGE_FEDERATED_CONTROL_TOOL,
};
pub use research_contracts::{
    run_onco_computational_execution_contract_model_json,
    validate_onco_computational_execution_contract_model_json,
    ONCO_COMPUTATIONAL_EXECUTION_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    run_oracle_evidence_surveillance_workflow_fabric_json,
    validate_oracle_evidence_surveillance_workflow_fabric_json,
    ORACLE_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_TOOL,
};
pub use research_contracts::{
    run_oracle_interoperability_research_workbench_json,
    validate_oracle_interoperability_research_workbench_json,
    ORACLE_INTEROPERABILITY_RESEARCH_WORKBENCH_TOOL,
};
pub use research_contracts::{
    run_packs_protocol_simulation_workbench_json,
    validate_packs_protocol_simulation_workbench_json, PACKS_PROTOCOL_SIMULATION_WORKBENCH_TOOL,
};
pub use research_contracts::{
    run_prism_laboratory_integration_copilot_json,
    validate_prism_laboratory_integration_copilot_json, PRISM_LABORATORY_INTEGRATION_COPILOT_TOOL,
};
pub use research_contracts::{
    run_prism_protocol_simulation_assurance_json,
    validate_prism_protocol_simulation_assurance_json, PRISM_PROTOCOL_SIMULATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_runtime_knowledge_representation_assurance_json,
    validate_runtime_knowledge_representation_assurance_json,
    RUNTIME_KNOWLEDGE_REPRESENTATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_scale_interpretation_interoperability_gateway_json,
    validate_scale_interpretation_interoperability_gateway_json,
    SCALE_INTERPRETATION_INTEROPERABILITY_GATEWAY_TOOL,
};
pub use research_contracts::{
    run_scale_interpretation_visualization_assurance_json,
    validate_scale_interpretation_visualization_assurance_json,
    SCALE_INTERPRETATION_VISUALIZATION_ASSURANCE_TOOL,
};
pub use research_contracts::{
    run_scale_quality_control_contract_model_json, validate_scale_quality_control_contract_json,
    SCALE_QUALITY_CONTROL_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    run_scope_federated_evidence_control_json, validate_scope_federated_evidence_control_json,
    SCOPE_FEDERATED_EVIDENCE_CONTROL_TOOL,
};
pub use research_contracts::{
    run_scope_federated_interoperability_gateway_json,
    validate_scope_federated_interoperability_gateway_json, SCOPE_FEDERATED_INTEROPERABILITY_TOOL,
};
pub use research_contracts::{
    run_stress_federated_multimodal_ingestion_contract_model_json,
    validate_stress_federated_multimodal_ingestion_contract_model_json,
    STRESS_FEDERATED_MULTIMODAL_INGESTION_CONTRACT_MODEL_TOOL,
};
pub use research_contracts::{
    run_stress_publication_research_object_workbench_json,
    validate_stress_publication_research_object_workbench_json,
    STRESS_PUBLICATION_RESEARCH_OBJECT_WORKBENCH_TOOL,
};
pub use research_contracts::{
    run_weavelang_federated_commons_assurance_json,
    validate_weavelang_federated_commons_assurance_json,
    WEAVELANG_FEDERATED_COMMONS_ASSURANCE_TOOL,
};
pub use research_contracts::{
    simulate_worldfactory_protocol_json, validate_worldfactory_protocol_simulation_json,
    WORLDFACTORY_PROTOCOL_SIMULATION_TOOL,
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
