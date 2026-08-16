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
                     oracle_reference_panel, oracle_missingness, bioeval_reference_audit, bioeval_acquisition_audit, bioeval_grounding_audit, bioeval_estimand_audit, bioeval_evaluator_audit, bioeval_plane_audit, bioeval_metamorphic_audit, bioeval_waiver_audit,\n\
                     evaluation_worldline_audit,\n\
                     evaluation_reproduction_check, evaluation_trajectory_check,\n\
                     runtime_effect_check, runtime_tape_verify, runtime_execution_simulate, onco_boundary_check,\n\
                     onco_response_assess, onco_worldline_view, onco_classification_check,\n\
                     oncoworlds_identity_join, oncoworlds_model_transport,\n\
                     oncoworlds_methylation_classify, oncoworlds_methylation_compare,\n\
                     oncoworlds_radiogenomic_check, oncoworlds_clonal_history_check,\n\
                     onco_outcome_analyze, stress_profile, stress_report,\n\
                     policy_screen, bioethics_action_review, bioethics_human_subject_screen,\n\
                     bioethics_dual_use_review, bioethics_validation_check, bioethics_representation_audit,\n\
                     influence_analyze, routing_decide, token_context_plan, bioql_compile, epistemic_voi,\n\
                     benchmark_trace_analyze, pack_catalogue, pack_health_assess, world_generate, factory_lifecycle_simulate, foundation_contract_check,\n\
                     weavelang_compile,\n\
                     choreography_check, conformance_run,\n\
                     provider_capability_gate, sdk_registry_check, hub_submission_review, hub_disclosure_review,\n\
                     hub_card_render, hub_leaderboard_render, bioatlas_publication_audit, telemetry_project,\n\
                     governance_schema_check,\n\
                     developer_platform_status, capability_audit, capability_discover, capability_route, agent_mission, developer_workbench, developer_delivery_audit, safety_posture, security_redteam_simulate, weave_protocol_catalog, world_index,\n\
                     workspace_capabilities, repository_catalog, repository_bundle, repository_impact\n\
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
