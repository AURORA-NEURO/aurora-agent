//! `bioprism-mcp` — stdio MCP server.
//!
//! Usage: `bioprism-mcp [--root <dir>]`
//!
//! The root defaults to the working directory and confines every path the server will read or
//! write. stdout carries JSON-RPC only; audit records and diagnostics go to stderr.

use bioprism_mcp::{serve, Server};
use std::io::{self, BufReader};
use std::path::PathBuf;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mut root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => match arguments.next() {
                Some(value) => root = PathBuf::from(value),
                None => {
                    eprintln!("--root requires a directory");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                println!(
                    "bioprism-mcp — Model Context Protocol server for the FIBER context compiler\n\n\
                     USAGE\n  bioprism-mcp [--root <dir>]\n\n\
                     Speaks JSON-RPC 2.0 over newline-delimited stdio. Every path an agent supplies \n\
                     is resolved inside --root; absolute paths, traversal and symlink escapes are refused.\n\n\
                     The client must initialize, acknowledge notifications/initialized, and then use\n\
                     tools/list, tools/call, resources/list or resources/read. fiber_compile returns\n\
                     a content-addressed refinement handle.\n\n\
                     Tools: fiber_compile, fiber_refine, fiber_explain, fiber_verify, projection_bundle, world_validate,\n\
                     context_compare, bioworlds_catalog, modality_catalog, mutation_family,\n\
                     prism_minimize, registry_gate, registry_lifecycle_simulate, cache_invalidation_simulate, storage_lifecycle_simulate, release_audit, operations_catalog, ops_acceptance, ops_capacity,\n\
                     research_ci_check, capability_rank, metrics_profile_audit, biocapability_evidence_audit,\n\
                     safety_release_gate, medical_boundary_check, hub_search, measurement_compare,\n\
                     hub_resolve, hub_lock, adapter_plan, tabular_ingest, observed_world_declare, world_claim_check,\n\
                    trace_analyze, trace_otel_ingest,\n\
                    lineage_audit, preanalytic_apply,\n\
                    contradiction_review,\n\
                     lab_plan, lens_catalogue, lens_leakage_check, scale_family_split_verify, megafactory_twin_audit,\n\
                     megafactory_placement_audit, stewardship_review_check,\n\
                     quality_gate_run, ledger_ingest,\n\
                     fabric_synthesize,\n\
                     interweave_workflow_catalogue,\n\
                     atlas_report, bundle_verify, adaptive_panel, posterior_gate, oracle_combine,\n\
                     oracle_reference_panel, oracle_missingness, bioeval_reference_audit, bioeval_acquisition_audit, bioeval_grounding_audit, bioeval_estimand_audit, bioeval_evaluator_audit, bioeval_plane_audit, bioeval_metamorphic_audit, bioeval_waiver_audit, bioeval_design_audit, bioeval_mesh_audit, bioeval_burden_audit, bioeval_reveal_audit, bioeval_boundary_audit,\n\
                     evaluation_worldline_audit,\n\
                     evaluation_reproduction_check, evaluation_trajectory_check, evaluation_observability_card, federated_evaluation_consensus, resource_workbench_discover, resource_discovery_contract_v2, governance_research_release_compile, release_assurance_harness, protocol_assurance_harness, federated_multimodal_assurance, federated_knowledge_gateway, federated_lens_assurance, lab_semantic_parity, federated_retrieval_assurance, federated_continual_retrieval_copilot, federated_context_compilation_assurance, federated_knowledge_representation_assurance, federated_resource_control_plane, weavelang_release_assurance, federated_mechanism_control_plane, federated_mechanism_gateway, evidence_surveillance_copilot, multimodal_retrieval_synthesis, adapter_context_compilation_assurance, multimodal_knowledge_workflow, adapter_resource_workbench, adapter_ingestion_gateway, adapter_quality_envelope, adapter_experiment_design_control, adapter_protocol_simulation, adapter_instrument_mesh, research_release_validate, research_release_batch_validate, instrument_preflight, multimodal_harmonize, multimodal_replication_evaluate, quality_drift_evaluate, design_frontier_evaluate, autonomy_batch_admit, workflow_batch_execute, analysis_qualify, protocol_matrix_simulate,\n\
                     runtime_effect_check, runtime_tape_verify, runtime_execution_simulate, runtime_workflow_execute, onco_boundary_check,\n\
                     onco_response_assess, onco_worldline_view, onco_classification_check,\n\
                     oncoworlds_identity_join, oncoworlds_model_transport,\n\
                     oncoworlds_methylation_classify, oncoworlds_methylation_compare,\n\
                     oncoworlds_radiogenomic_check, oncoworlds_clonal_history_check,\n\
                     onco_outcome_analyze, stress_profile, stress_report,\n\
                     policy_screen, bioethics_action_review, bioethics_human_subject_screen,\n\
                     bioethics_dual_use_review, bioethics_validation_check, bioethics_representation_audit,\n\
                     influence_analyze, routing_decide, token_context_plan, bioql_compile, epistemic_voi,\n\
                     benchmark_trace_analyze, pack_catalogue, pack_health_assess, world_generate, factory_lifecycle_simulate, factory_authority_verify, artifact_registry_audit, foundation_contract_check,\n\
                     weavelang_compile,\n\
                     choreography_check, conformance_run,\n\
                     provider_capability_gate, sdk_registry_check, hub_submission_review, hub_disclosure_review,\n\
                     hub_card_render, hub_leaderboard_render, bioatlas_publication_audit, telemetry_project,\n\
                     governance_schema_check,\n\
                     developer_platform_status, capability_audit, capability_dashboard, capability_discover, capability_route, agent_mission, developer_workbench, developer_workbench_verify, developer_workbench_import, developer_workbench_query, developer_workbench_get, ci_provider_normalize, ci_provider_evidence_audit, ci_provider_evidence_import, ci_provider_evidence_query, ci_provider_evidence_get, ci_execution_evidence_audit, execution_provenance_audit, developer_delivery_audit, engineering_execution_plan, safety_posture, security_redteam_simulate, security_privacy_audit, sandbox_admission_audit, sandbox_runtime_simulate, security_program_audit, weave_protocol_catalog, world_index,\n\
                     workspace_capabilities, brain_model_select, brain_model_select_contextual, brain_prompt_assemble, brain_plan, brain_bandit_select, brain_bandit_update, brain_outcome_record, brain_job_submit, brain_job_status, brain_job_events, brain_job_approval, brain_job_claim, brain_job_claim_next, brain_job_renew, brain_job_checkpoint, brain_job_complete, brain_job_fail, brain_job_reconcile, brain_job_cancel, brain_model_health, brain_replay_evaluate, repository_catalog, repository_bundle, repository_impact\n\
                     Resources: fiber-world, fiber-query, context-certificate schemas and the\n\
                     workspace capability catalog"
                );
                return;
            }
            other => {
                eprintln!("unrecognised argument {other:?}");
                std::process::exit(2);
            }
        }
    }

    if !root.is_dir() {
        eprintln!("root is not a directory: {}", root.display());
        std::process::exit(2);
    }

    let mut server = Server::new(root);
    let stdin = BufReader::new(io::stdin());
    let mut stdout = io::stdout();

    if let Err(error) = serve(&mut server, stdin, &mut stdout) {
        eprintln!("server error: {error}");
        std::process::exit(1);
    }
}
