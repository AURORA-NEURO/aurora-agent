//! MCP tool-call routing for the server.
//!
//! Keeping the exhaustive route table separate lets each tool implementation live with its
//! domain while preserving one authoritative dispatch boundary.

use super::*;

impl Server {
    pub(super) fn call_tool_inner(&self, request: &Request) -> Response {
        let id = request.id.clone();
        let Some(name) = request.params.get("name").and_then(Value::as_str) else {
            return Response::error(
                id,
                code::INVALID_PARAMS,
                "tools/call requires a name".into(),
                None,
            );
        };
        let arguments = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Response::error(
                id,
                code::INVALID_PARAMS,
                "tools/call arguments must be an object".into(),
                None,
            );
        }

        audit(name, &arguments);

        match validate_mission_tool_arguments(name, &arguments) {
            Ok(Some(report)) => {
                if let Some(detail) = schema_failure_detail(name, &report) {
                    return Response::result(
                        id,
                        tool_content(&json!({ "ok": false, "error": detail }), true),
                    );
                }
            }
            Ok(None) => {}
            Err(error) => {
                return Response::result(
                    id,
                    tool_content(&json!({ "ok": false, "error": error }), true),
                );
            }
        }

        let outcome = match name {
            "fiber_compile" => self.fiber_compile(&arguments),
            "fiber_refine" => self.fiber_refine(&arguments),
            "fiber_explain" => self.fiber_explain(&arguments),
            "fiber_verify" => self.fiber_verify(&arguments),
            "projection_bundle" => self.projection_bundle(&arguments),
            "world_index" => self.world_index(&arguments),
            "world_validate" => self.world_validate(&arguments),
            "domain_validate" => self.domain_validate(&arguments),
            "world_generate" => self.world_generate(&arguments),
            "project_ingest" => self.project_ingest(&arguments),
            "project_audit" => self.project_audit(&arguments),
            "repair_plan" => self.repair_plan(&arguments),
            "repair_verify" => self.repair_verify(&arguments),
            "factory_lifecycle_simulate" => self.factory_lifecycle_simulate(&arguments),
            "factory_authority_verify" => self.factory_authority_verify(&arguments),
            "artifact_registry_audit" => self.artifact_registry_audit(&arguments),
            "domain_report_project" => self.domain_report_project(&arguments),
            "domain_evidence_harmonize" => self.domain_evidence_harmonize(&arguments),
            "domain_decision_readiness_audit" => self.domain_decision_readiness_audit(&arguments),
            "domain_decision_readiness_query" => self.domain_decision_readiness_query(&arguments),
            "control_plane_readiness_audit" => self.control_plane_readiness_audit(&arguments),
            "control_plane_readiness_compare" => self.control_plane_readiness_compare(&arguments),
            "control_plane_readiness_compare_retained" => {
                self.control_plane_readiness_compare_retained(&arguments)
            }
            "control_plane_readiness_query" => self.control_plane_readiness_query(&arguments),
            "brain_model_select" => self.brain_model_select(&arguments),
            "brain_model_select_contextual" => self.brain_model_select_contextual(&arguments),
            "brain_prompt_assemble" => self.brain_prompt_assemble(&arguments),
            "brain_plan" => self.brain_plan(&arguments),
            "research_campaign_run_offline" => {
                crate::research_campaign::run_offline(&arguments, |relative| self.resolve(relative))
            }
            "neurosurgery_intake_plan" => self.neurosurgery_intake_plan(&arguments),
            "neurosurgery_intake_mission" => self.neurosurgery_intake_mission(&arguments),
            "neurosurgery_intake_portfolio" => self.neurosurgery_intake_portfolio(&arguments),
            "neurosurgery_catalogue" => self.neurosurgery_catalogue(&arguments),
            "neurosurgery_evidence_audit" => self.neurosurgery_evidence_audit(&arguments),
            "neurosurgery_specialty_evidence_map" => {
                self.neurosurgery_specialty_evidence_map(&arguments)
            }
            "neurosurgery_case_asset_manifest" => self.neurosurgery_case_asset_manifest(&arguments),
            "neurosurgery_case_fhir_import" => self.neurosurgery_case_fhir_import(&arguments),
            "neurosurgery_case_dicom_import" => self.neurosurgery_case_dicom_import(&arguments),
            "neurosurgery_case_dicom_evidence_workflow" => {
                self.neurosurgery_case_dicom_evidence_workflow(&arguments)
            }
            "neurosurgery_case_asset_review_disposition" => {
                self.neurosurgery_case_asset_review_disposition(&arguments)
            }
            "neurosurgery_evidence_synthesis" => self.neurosurgery_evidence_synthesis(&arguments),
            "neurosurgery_glioma_molecular_map" => {
                self.neurosurgery_glioma_molecular_map(&arguments)
            }
            "neurosurgery_evidence_graph" => self.neurosurgery_evidence_graph(&arguments),
            "neurosurgery_real_data_coverage" => self.neurosurgery_real_data_coverage(&arguments),
            "neurosurgery_real_data_cohort_landscape" => {
                self.neurosurgery_real_data_cohort_landscape(&arguments)
            }
            "neurosurgery_real_data_reconciliation" => {
                self.neurosurgery_real_data_reconciliation(&arguments)
            }
            "neurosurgery_real_data_freshness" => self.neurosurgery_real_data_freshness(&arguments),
            "neurosurgery_real_data_diff" => self.neurosurgery_real_data_diff(&arguments),
            "neurosurgery_real_data_refresh_audit" => {
                self.neurosurgery_real_data_refresh_audit(&arguments)
            }
            "neurosurgery_real_data_review_queue" => {
                self.neurosurgery_real_data_review_queue(&arguments)
            }
            "neurosurgery_real_data_review_disposition" => {
                self.neurosurgery_real_data_review_disposition(&arguments)
            }
            "neurosurgery_real_data_evidence_packet" => {
                self.neurosurgery_real_data_evidence_packet(&arguments)
            }
            "neurosurgery_real_data_autonomous_workflow" => {
                self.neurosurgery_real_data_autonomous_workflow(&arguments)
            }
            "neurosurgery_real_data_reasoning_context" => {
                self.neurosurgery_real_data_reasoning_context(&arguments)
            }
            "neurosurgery_real_data_draft_audit" => {
                self.neurosurgery_real_data_draft_audit(&arguments)
            }
            "neurosurgery_public_literature_evidence_packet" => {
                self.neurosurgery_public_literature_evidence_packet(&arguments)
            }
            "neurosurgery_public_literature_reasoning_context" => {
                self.neurosurgery_public_literature_reasoning_context(&arguments)
            }
            "neurosurgery_public_literature_draft_audit" => {
                self.neurosurgery_public_literature_draft_audit(&arguments)
            }
            "neurosurgery_public_literature_matrix" => {
                self.neurosurgery_public_literature_matrix(&arguments)
            }
            "neurosurgery_public_literature_freshness" => {
                self.neurosurgery_public_literature_freshness(&arguments)
            }
            "neurosurgery_public_literature_refresh_audit" => {
                self.neurosurgery_public_literature_refresh_audit(&arguments)
            }
            "neurosurgery_literature_link_audit" => {
                self.neurosurgery_literature_link_audit(&arguments)
            }
            "neurosurgery_public_literature_integrity_audit" => {
                self.neurosurgery_public_literature_integrity_audit(&arguments)
            }
            "neurosurgery_public_literature_review_queue" => {
                self.neurosurgery_public_literature_review_queue(&arguments)
            }
            "neurosurgery_public_literature_workbench" => {
                self.neurosurgery_public_literature_workbench(&arguments)
            }
            "neurosurgery_evidence_program" => self.neurosurgery_evidence_program(&arguments),
            "neurosurgery_public_literature_portfolio" => {
                self.neurosurgery_public_literature_portfolio(&arguments)
            }
            "neurosurgery_research_brief" => self.neurosurgery_research_brief(&arguments),
            "neurosurgery_research_plan" => self.neurosurgery_research_plan(&arguments),
            "neurosurgery_evidence_acquisition" => {
                self.neurosurgery_evidence_acquisition(&arguments)
            }
            "neurosurgery_plan" => self.neurosurgery_plan(&arguments),
            "neurosurgery_real_data_query" => self.neurosurgery_real_data_query(&arguments),
            "neurosurgery_real_data_trial_landscape" => {
                self.neurosurgery_real_data_trial_landscape(&arguments)
            }
            "neurosurgery_real_data_molecular_coverage" => {
                self.neurosurgery_real_data_molecular_coverage(&arguments)
            }
            "neurosurgery_public_literature_query" => {
                self.neurosurgery_public_literature_query(&arguments)
            }
            "neurosurgery_session" => self.neurosurgery_session(&arguments),
            "neurosurgery_mission" => self.neurosurgery_mission(&arguments),
            "brain_bandit_select" => self.brain_bandit_select(&arguments),
            "brain_bandit_update" => self.brain_bandit_update(&arguments),
            "brain_outcome_record" => self.brain_outcome_record(&arguments),
            "brain_job_submit" => self.brain_job_submit(&arguments),
            "brain_job_status" => self.brain_job_status(&arguments),
            "brain_job_events" => self.brain_job_events(&arguments),
            "brain_job_approval" => self.brain_job_approval(&arguments),
            "brain_job_claim" => self.brain_job_claim(&arguments),
            "brain_job_claim_next" => self.brain_job_claim_next(&arguments),
            "brain_job_renew" => self.brain_job_renew(&arguments),
            "brain_job_checkpoint" => self.brain_job_checkpoint(&arguments),
            "brain_job_complete" => self.brain_job_complete(&arguments),
            "brain_job_fail" => self.brain_job_fail(&arguments),
            "brain_job_reconcile" => self.brain_job_reconcile(&arguments),
            "brain_job_cancel" => self.brain_job_cancel(&arguments),
            "brain_model_health" => self.brain_model_health(&arguments),
            "brain_replay_evaluate" => self.brain_replay_evaluate(&arguments),
            "glioma_research_dry_run" => self.glioma_research_dry_run(&arguments),
            "glioma_workflow_plan" => self.glioma_workflow_plan(&arguments),
            "glioma_protocol_simulate" => self.glioma_protocol_simulate(&arguments),
            "glioma_protocol_scenario_ensemble" => {
                self.glioma_protocol_scenario_ensemble(&arguments)
            }
            "glioma_protocol_branch_optimize" => self.glioma_protocol_branch_optimize(&arguments),
            "glioma_protocol_autonomous_execute" => {
                self.glioma_protocol_autonomous_execute(&arguments)
            }
            "glioma_protocol_evidence_surface" => self.glioma_protocol_evidence_surface(&arguments),
            "glioma_protocol_multistudy_fusion" => {
                self.glioma_protocol_multistudy_fusion(&arguments)
            }
            "glioma_protocol_transport_gate" => self.glioma_protocol_transport_gate(&arguments),
            "glioma_protocol_execute" => self.glioma_protocol_execute(&arguments),
            "glioma_protocol_compensation" => self.glioma_protocol_compensation(&arguments),
            "glioma_action_portfolio_execute" => self.glioma_action_portfolio_execute(&arguments),
            "glioma_autonomous_campaign_execute" => {
                self.glioma_autonomous_campaign_execute(&arguments)
            }
            "glioma_research_autopilot_execute" => {
                self.glioma_research_autopilot_execute(&arguments)
            }
            "glioma_evidence_campaign_execute" => self.glioma_evidence_campaign_execute(&arguments),
            "glioma_evidence_refresh_campaign_execute" => {
                self.glioma_evidence_refresh_campaign_execute(&arguments)
            }
            "glioma_knowledge_resolution_campaign_execute" => {
                self.glioma_knowledge_resolution_campaign_execute(&arguments)
            }
            "glioma_decision_context_campaign_execute" => {
                self.glioma_decision_context_campaign_execute(&arguments)
            }
            "glioma_decision_context_query" => self.glioma_decision_context_query(&arguments),
            "glioma_decision_context_update" => self.glioma_decision_context_update(&arguments),
            "glioma_decision_context_snapshot_store" => {
                self.glioma_decision_context_snapshot_store(&arguments)
            }
            "glioma_decision_branch_campaign_execute" => {
                self.glioma_decision_branch_campaign_execute(&arguments)
            }
            "glioma_adaptive_decision_branch_campaign_execute" => {
                self.glioma_adaptive_decision_branch_campaign_execute(&arguments)
            }
            "glioma_decision_operating_cycle" => self.glioma_decision_operating_cycle(&arguments),
            "glioma_multimodal_ingestion_campaign_execute" => {
                self.glioma_multimodal_ingestion_campaign_execute(&arguments)
            }
            "glioma_multimodal_readiness_gate" => self.glioma_multimodal_readiness_gate(&arguments),
            "glioma_multimodal_operating_cycle" => {
                self.glioma_multimodal_operating_cycle(&arguments)
            }
            "glioma_computation_execute" => self.glioma_computation_execute(&arguments),
            "glioma_release_shareability_check" => {
                self.glioma_release_shareability_check(&arguments)
            }
            "glioma_release_metadata_normalize" => {
                self.glioma_release_metadata_normalize(&arguments)
            }
            "glioma_release_attestation_issue" => self.glioma_release_attestation_issue(&arguments),
            "glioma_release_signature_verify" => self.glioma_release_signature_verify(&arguments),
            "glioma_research_object_conformance_check" => {
                self.glioma_research_object_conformance_check(&arguments)
            }
            "glioma_release_preview" => self.glioma_release_preview(&arguments),
            "glioma_release_queue_snapshot" => self.glioma_release_queue_snapshot(&arguments),
            "glioma_archive_migration_execute" => self.glioma_archive_migration_execute(&arguments),
            "glioma_multistudy_release_compose" => {
                self.glioma_multistudy_release_compose(&arguments)
            }
            "glioma_comparative_release_explore" => {
                self.glioma_comparative_release_explore(&arguments)
            }
            "glioma_continuous_release_compile" => {
                self.glioma_continuous_release_compile(&arguments)
            }
            "glioma_release_event_protocol_replay" => {
                self.glioma_release_event_protocol_replay(&arguments)
            }
            "glioma_version_retention_plan" => self.glioma_version_retention_plan(&arguments),
            "glioma_distributed_archive_mirror" => {
                self.glioma_distributed_archive_mirror(&arguments)
            }
            "glioma_release_queue_schedule" => self.glioma_release_queue_schedule(&arguments),
            "glioma_research_object_exchange_plan" => {
                self.glioma_research_object_exchange_plan(&arguments)
            }
            "glioma_research_object_dependency_leakage_audit" => {
                self.glioma_research_object_dependency_leakage_audit(&arguments)
            }
            "glioma_compute_environment_lock" => self.glioma_compute_environment_lock(&arguments),
            "glioma_environment_resolution" => self.glioma_environment_resolution(&arguments),
            "glioma_reproducible_task_submit" => self.glioma_reproducible_task_submit(&arguments),
            "glioma_computation_event_stream" => self.glioma_computation_event_stream(&arguments),
            "glioma_decision_budget_snapshot" => self.glioma_decision_budget_snapshot(&arguments),
            "glioma_registry_artifact_resolve" => self.glioma_registry_artifact_resolve(&arguments),
            "glioma_compute_cache_govern" => self.glioma_compute_cache_govern(&arguments),
            "glioma_multistudy_cache_partition" => {
                self.glioma_multistudy_cache_partition(&arguments)
            }
            "glioma_compute_capacity_plan" => self.glioma_compute_capacity_plan(&arguments),
            "glioma_reproducibility_completeness_score" => {
                self.glioma_reproducibility_completeness_score(&arguments)
            }
            "glioma_reproducibility_bundle_compile" => {
                self.glioma_reproducibility_bundle_compile(&arguments)
            }
            "glioma_computation_lineage" => self.glioma_computation_lineage(&arguments),
            "glioma_computation_reproducibility" => {
                self.glioma_computation_reproducibility(&arguments)
            }
            "glioma_computation_portfolio_plan" => {
                self.glioma_computation_portfolio_plan(&arguments)
            }
            "glioma_computation_placement" => self.glioma_computation_placement(&arguments),
            "glioma_computation_placement_stress_evaluate" => {
                self.glioma_computation_placement_stress_evaluate(&arguments)
            }
            "glioma_computation_portfolio_execute" => {
                self.glioma_computation_portfolio_execute(&arguments)
            }
            "glioma_computation_campaign_execute" => {
                self.glioma_computation_campaign_execute(&arguments)
            }
            "glioma_computation_recovery_execute" => {
                self.glioma_computation_recovery_execute(&arguments)
            }
            "glioma_robustness_guided_computation_execute" => {
                self.glioma_robustness_guided_computation_execute(&arguments)
            }
            "glioma_computation_workflow_execute" => {
                self.glioma_computation_workflow_execute(&arguments)
            }
            "glioma_computation_operating_cycle" => {
                self.glioma_computation_operating_cycle(&arguments)
            }
            "glioma_computation_interpretation_frontier_compile" => {
                self.glioma_computation_interpretation_frontier_compile(&arguments)
            }
            "glioma_computation_interpretation_frontier_execute" => {
                self.glioma_computation_interpretation_frontier_execute(&arguments)
            }
            "glioma_computation_interpretation_evidence_gate" => {
                self.glioma_computation_interpretation_evidence_gate(&arguments)
            }
            "glioma_research_director_execute" => self.glioma_research_director_execute(&arguments),
            "glioma_program_scheduler_execute" => self.glioma_program_scheduler_execute(&arguments),
            "glioma_experiment_frontier_controller_execute" => {
                self.glioma_experiment_frontier_controller_execute(&arguments)
            }
            "glioma_causal_claim_adjudication_execute" => {
                self.glioma_causal_claim_adjudication_execute(&arguments)
            }
            "glioma_mechanism_discovery_engine_execute" => {
                self.glioma_mechanism_discovery_engine_execute(&arguments)
            }
            "glioma_evidence_gated_research_execute" => {
                self.glioma_evidence_gated_research_execute(&arguments)
            }
            "glioma_autonomous_research_engine_execute" => {
                self.glioma_autonomous_research_engine_execute(&arguments)
            }
            "glioma_autonomous_research_engine_evaluate" => {
                self.glioma_autonomous_research_engine_evaluate(&arguments)
            }
            "glioma_autonomous_research_engine_stress_evaluate" => {
                self.glioma_autonomous_research_engine_stress_evaluate(&arguments)
            }
            "glioma_autonomous_research_engine_trace_evaluate" => {
                self.glioma_autonomous_research_engine_trace_evaluate(&arguments)
            }
            "glioma_stage_worker_routes_compile" => {
                self.glioma_stage_worker_routes_compile(&arguments)
            }
            "glioma_autonomous_research_engine_stage_execute" => {
                self.glioma_autonomous_research_engine_stage_execute(&arguments)
            }
            "glioma_evidence_gated_stage_engine_execute" => {
                self.glioma_evidence_gated_stage_engine_execute(&arguments)
            }
            "glioma_temporal_multimodal_mechanism_fusion" => {
                self.glioma_temporal_multimodal_mechanism_fusion(&arguments)
            }
            "glioma_cross_model_claim_envelope" => {
                self.glioma_cross_model_claim_envelope(&arguments)
            }
            "glioma_cross_model_replication_frontier" => {
                self.glioma_cross_model_replication_frontier(&arguments)
            }
            "glioma_cross_model_replication_mission" => {
                self.glioma_cross_model_replication_mission(&arguments)
            }
            "glioma_cross_model_replication_mission_execute" => {
                self.glioma_cross_model_replication_mission_execute(&arguments)
            }
            "glioma_autonomous_program_cycle" => self.glioma_autonomous_program_cycle(&arguments),
            "glioma_adaptive_workflow" => self.glioma_adaptive_workflow(&arguments),
            "glioma_interpretation_synthesize" => self.glioma_interpretation_synthesize(&arguments),
            "glioma_interpretation_operating_cycle" => {
                self.glioma_interpretation_operating_cycle(&arguments)
            }
            "glioma_temporal_multimodal_fusion" => {
                self.glioma_temporal_multimodal_fusion(&arguments)
            }
            "glioma_temporal_spatial_alignment" => {
                self.glioma_temporal_spatial_alignment(&arguments)
            }
            "glioma_clonal_evolution" => self.glioma_clonal_evolution(&arguments),
            "glioma_clone_perturbation_panel" => self.glioma_clone_perturbation_panel(&arguments),
            "glioma_clone_panel_outcomes" => self.glioma_clone_panel_outcomes(&arguments),
            "glioma_clone_continuation" => self.glioma_clone_continuation(&arguments),
            "glioma_adaptive_clone_campaign_execute" => {
                self.glioma_adaptive_clone_campaign_execute(&arguments)
            }
            "glioma_adaptive_research_frontier" => {
                self.glioma_adaptive_research_frontier(&arguments)
            }
            "glioma_adaptive_frontier_execute" => self.glioma_adaptive_frontier_execute(&arguments),
            "glioma_adaptive_interpretation_campaign_execute" => {
                self.glioma_adaptive_interpretation_campaign_execute(&arguments)
            }
            "glioma_robustness_suite" => self.glioma_robustness_suite(&arguments),
            "glioma_trajectory_analyze" => self.glioma_trajectory_analyze(&arguments),
            "glioma_state_transition_analyze" => self.glioma_state_transition_analyze(&arguments),
            "glioma_transportability_analyze" => self.glioma_transportability_analyze(&arguments),
            "glioma_longitudinal_transport_analyze" => {
                self.glioma_longitudinal_transport_analyze(&arguments)
            }
            "glioma_multistudy_concordance_analyze" => {
                self.glioma_multistudy_concordance_analyze(&arguments)
            }
            "glioma_prospective_contradiction_plan" => {
                self.glioma_prospective_contradiction_plan(&arguments)
            }
            "glioma_registered_outcome_reporting_audit" => {
                self.glioma_registered_outcome_reporting_audit(&arguments)
            }
            "glioma_registered_outcome_record_build" => {
                self.glioma_registered_outcome_record_build(&arguments)
            }
            "glioma_registered_outcome_evidence_panel" => {
                self.glioma_registered_outcome_evidence_panel(&arguments)
            }
            "glioma_registered_outcome_sensitivity_analyze" => {
                self.glioma_registered_outcome_sensitivity_analyze(&arguments)
            }
            "glioma_causal_contrast" => self.glioma_causal_contrast(&arguments),
            "glioma_causal_mediation" => self.glioma_causal_mediation(&arguments),
            "glioma_stratified_causal_adjustment" => {
                self.glioma_stratified_causal_adjustment(&arguments)
            }
            "glioma_dynamic_policy_evaluate" => self.glioma_dynamic_policy_evaluate(&arguments),
            "glioma_dose_response" => self.glioma_dose_response(&arguments),
            "glioma_adaptive_allocation" => self.glioma_adaptive_allocation(&arguments),
            "glioma_sequential_design" => self.glioma_sequential_design(&arguments),
            "glioma_power_reestimate" => self.glioma_power_reestimate(&arguments),
            "glioma_power_stress_surface" => self.glioma_power_stress_surface(&arguments),
            "glioma_carryover_sequence_design" => self.glioma_carryover_sequence_design(&arguments),
            "glioma_closed_loop_campaign" => self.glioma_closed_loop_campaign(&arguments),
            "glioma_experiment_operating_cycle" => {
                self.glioma_experiment_operating_cycle(&arguments)
            }
            "glioma_combination_synergy" => self.glioma_combination_synergy(&arguments),
            "glioma_adaptive_dose_surface" => self.glioma_adaptive_dose_surface(&arguments),
            "glioma_multimodal_concordance" => self.glioma_multimodal_concordance(&arguments),
            "glioma_multimodal_dropout_stress" => self.glioma_multimodal_dropout_stress(&arguments),
            "glioma_multimodal_drift" => self.glioma_multimodal_drift(&arguments),
            "glioma_multimodal_evidence_fusion" => {
                self.glioma_multimodal_evidence_fusion(&arguments)
            }
            "glioma_multimodal_sensitivity" => self.glioma_multimodal_sensitivity(&arguments),
            "glioma_multimodal_decision_gate" => self.glioma_multimodal_decision_gate(&arguments),
            "glioma_multimodal_contradiction_adjudication" => {
                self.glioma_multimodal_contradiction_adjudication(&arguments)
            }
            "glioma_multimodal_quality_forecast" => {
                self.glioma_multimodal_quality_forecast(&arguments)
            }
            "glioma_multimodal_quality_scheduler" => {
                self.glioma_multimodal_quality_scheduler(&arguments)
            }
            "glioma_multimodal_quality_execute" => {
                self.glioma_multimodal_quality_execute(&arguments)
            }
            "glioma_multimodal_quality_adaptive_campaign" => {
                self.glioma_multimodal_quality_adaptive_campaign(&arguments)
            }
            "glioma_multimodal_quality_transport" => {
                self.glioma_multimodal_quality_transport(&arguments)
            }
            "glioma_multimodal_quality_root_cause" => {
                self.glioma_multimodal_quality_root_cause(&arguments)
            }
            "glioma_multimodal_quality_remediation" => {
                self.glioma_multimodal_quality_remediation(&arguments)
            }
            "glioma_multimodal_quality_recovery" => {
                self.glioma_multimodal_quality_recovery(&arguments)
            }
            "glioma_multimodal_missingness" => self.glioma_multimodal_missingness(&arguments),
            "glioma_multimodal_reliability" => self.glioma_multimodal_reliability(&arguments),
            "glioma_multimodal_portfolio" => self.glioma_multimodal_portfolio(&arguments),
            "glioma_multimodal_consensus" => self.glioma_multimodal_consensus(&arguments),
            "glioma_multimodal_harmonize" => self.glioma_multimodal_harmonize(&arguments),
            "glioma_multimodal_latent_factors" => self.glioma_multimodal_latent_factors(&arguments),
            "glioma_multimodal_graph_fusion" => self.glioma_multimodal_graph_fusion(&arguments),
            "glioma_pathway_activity" => self.glioma_pathway_activity(&arguments),
            "glioma_multimodal_mechanism_campaign" => {
                self.glioma_multimodal_mechanism_campaign(&arguments)
            }
            "glioma_multimodal_mechanism_campaign_execute" => {
                self.glioma_multimodal_mechanism_campaign_execute(&arguments)
            }
            "glioma_mechanism_autopilot_execute" => {
                self.glioma_mechanism_autopilot_execute(&arguments)
            }
            "glioma_spatial_niches" => self.glioma_spatial_niches(&arguments),
            "glioma_spatial_communication" => self.glioma_spatial_communication(&arguments),
            "glioma_spatial_state_propagation" => self.glioma_spatial_state_propagation(&arguments),
            "glioma_spatial_registration" => self.glioma_spatial_registration(&arguments),
            "glioma_causal_sensitivity" => self.glioma_causal_sensitivity(&arguments),
            "glioma_research_select_actions" => self.glioma_research_select_actions(&arguments),
            "glioma_scientific_frontier" => self.glioma_scientific_frontier(&arguments),
            "glioma_scientific_frontier_execute" => {
                self.glioma_scientific_frontier_execute(&arguments)
            }
            "glioma_program_catalog" => self.glioma_program_catalog(&arguments),
            "glioma_evidence_qualify" => self.glioma_evidence_qualify(&arguments),
            "glioma_evidence_cluster_index" => self.glioma_evidence_cluster_index(&arguments),
            "glioma_evidence_novelty_adjudication" => {
                self.glioma_evidence_novelty_adjudication(&arguments)
            }
            "glioma_evidence_stream_snapshot" => self.glioma_evidence_stream_snapshot(&arguments),
            "glioma_evidence_prospective_triage" => {
                self.glioma_evidence_prospective_triage(&arguments)
            }
            "glioma_evidence_researcher_workbench" => {
                self.glioma_evidence_researcher_workbench(&arguments)
            }
            "glioma_multimodal_researcher_workbench" => {
                self.glioma_multimodal_researcher_workbench(&arguments)
            }
            "glioma_evidence_verification_gate" => {
                self.glioma_evidence_verification_gate(&arguments)
            }
            "glioma_multisite_outcome_reconciliation" => {
                self.glioma_multisite_outcome_reconciliation(&arguments)
            }
            "glioma_evidence_knowledge_bridge" => self.glioma_evidence_knowledge_bridge(&arguments),
            "glioma_long_horizon_evidence_calibration" => {
                self.glioma_long_horizon_evidence_calibration(&arguments)
            }
            "glioma_federated_outcome_transport" => {
                self.glioma_federated_outcome_transport(&arguments)
            }
            "glioma_federated_evidence_operating_cycle" => {
                self.glioma_federated_evidence_operating_cycle(&arguments)
            }
            "glioma_federated_batch_scheduler" => self.glioma_federated_batch_scheduler(&arguments),
            "glioma_continual_promotion_control" => {
                self.glioma_continual_promotion_control(&arguments)
            }
            "glioma_federated_evidence_acquisition_policy" => {
                self.glioma_federated_evidence_acquisition_policy(&arguments)
            }
            "glioma_evidence_frontier_join" => self.glioma_evidence_frontier_join(&arguments),
            "glioma_multimodal_evidence_gap_router" => {
                self.glioma_multimodal_evidence_gap_router(&arguments)
            }
            "glioma_evidence_acquisition_feedback" => {
                self.glioma_evidence_acquisition_feedback(&arguments)
            }
            "glioma_federated_execution_handoff" => {
                self.glioma_federated_execution_handoff(&arguments)
            }
            "glioma_evidence_surveillance" => self.glioma_evidence_surveillance(&arguments),
            "glioma_evidence_novelty_radar" => self.glioma_evidence_novelty_radar(&arguments),
            "glioma_evidence_temporal_shift" => self.glioma_evidence_temporal_shift(&arguments),
            "glioma_federated_evidence_shift" => self.glioma_federated_evidence_shift(&arguments),
            "glioma_evidence_priority" => self.glioma_evidence_priority(&arguments),
            "glioma_evidence_acquisition_plan" => self.glioma_evidence_acquisition_plan(&arguments),
            "glioma_evidence_acquisition_campaign_execute" => {
                self.glioma_evidence_acquisition_campaign_execute(&arguments)
            }
            "glioma_evidence_operating_cycle" => self.glioma_evidence_operating_cycle(&arguments),
            "glioma_evidence_calibrate" => self.glioma_evidence_calibrate(&arguments),
            "glioma_evidence_triangulate" => self.glioma_evidence_triangulate(&arguments),
            "glioma_evidence_contradiction_cut" => {
                self.glioma_evidence_contradiction_cut(&arguments)
            }
            "glioma_knowledge_compile" => self.glioma_knowledge_compile(&arguments),
            "glioma_knowledge_compose" => self.glioma_knowledge_compose(&arguments),
            "glioma_knowledge_consistency" => self.glioma_knowledge_consistency(&arguments),
            "glioma_knowledge_drift" => self.glioma_knowledge_drift(&arguments),
            "glioma_knowledge_closure" => self.glioma_knowledge_closure(&arguments),
            "glioma_multi_study_knowledge" => self.glioma_multi_study_knowledge(&arguments),
            "glioma_prospective_knowledge_monitor" => {
                self.glioma_prospective_knowledge_monitor(&arguments)
            }
            "glioma_federated_continual_knowledge" => {
                self.glioma_federated_continual_knowledge(&arguments)
            }
            "glioma_federated_continual_agent" => self.glioma_federated_continual_agent(&arguments),
            "glioma_local_research_workflow" => self.glioma_local_research_workflow(&arguments),
            "glioma_workflow_recovery" => self.glioma_workflow_recovery(&arguments),
            "glioma_knowledge_action_outcome_assimilation" => {
                self.glioma_knowledge_action_outcome_assimilation(&arguments)
            }
            "glioma_claim_experiment_closure" => self.glioma_claim_experiment_closure(&arguments),
            "glioma_claim_evidence_reconciliation" => {
                self.glioma_claim_evidence_reconciliation(&arguments)
            }
            "glioma_closed_loop_frontier" => self.glioma_closed_loop_frontier(&arguments),
            "glioma_prospective_belief_calibration" => {
                self.glioma_prospective_belief_calibration(&arguments)
            }
            "glioma_frontier_campaign" => self.glioma_frontier_campaign(&arguments),
            "glioma_knowledge_protocol_gateway" => {
                self.glioma_knowledge_protocol_gateway(&arguments)
            }
            "glioma_multimodal_knowledge_protocol_gateway" => {
                self.glioma_multimodal_knowledge_protocol_gateway(&arguments)
            }
            "glioma_multimodal_ingestion_manifest" => {
                self.glioma_multimodal_ingestion_manifest(&arguments)
            }
            "glioma_multimodal_knowledge_workflow" => {
                self.glioma_multimodal_knowledge_workflow(&arguments)
            }
            "glioma_research_workflow_admission" => {
                self.glioma_research_workflow_admission(&arguments)
            }
            "glioma_federated_knowledge" => self.glioma_federated_knowledge(&arguments),
            "glioma_belief_revision" => self.glioma_belief_revision(&arguments),
            "glioma_knowledge_frontier" => self.glioma_knowledge_frontier(&arguments),
            "glioma_knowledge_gap_compile" => self.glioma_knowledge_gap_compile(&arguments),
            "glioma_knowledge_action_compile" => self.glioma_knowledge_action_compile(&arguments),
            "glioma_knowledge_action_bridge" => self.glioma_knowledge_action_bridge(&arguments),
            "glioma_knowledge_selection_cycle" => self.glioma_knowledge_selection_cycle(&arguments),
            "glioma_knowledge_action_dispatch" => self.glioma_knowledge_action_dispatch(&arguments),
            "glioma_autonomous_gap_cycle" => self.glioma_autonomous_gap_cycle(&arguments),
            "glioma_knowledge_synthesis_operating_cycle" => {
                self.glioma_knowledge_synthesis_operating_cycle(&arguments)
            }
            "glioma_decision_context" => self.glioma_decision_context(&arguments),
            "glioma_decision_context_artifact" => self.glioma_decision_context_artifact(&arguments),
            "glioma_multi_study_context_artifact" => {
                self.glioma_multi_study_context_artifact(&arguments)
            }
            "glioma_multi_study_workflow_plan" => self.glioma_multi_study_workflow_plan(&arguments),
            "glioma_multi_study_execution_receipt" => {
                self.glioma_multi_study_execution_receipt(&arguments)
            }
            "glioma_federated_decision_context" => {
                self.glioma_federated_decision_context(&arguments)
            }
            "glioma_federated_continual_context_promotion" => {
                self.glioma_federated_continual_context_promotion(&arguments)
            }
            "glioma_multi_study_context_epoch_replay" => {
                self.glioma_multi_study_context_epoch_replay(&arguments)
            }
            "glioma_decision_context_replay" => self.glioma_decision_context_replay(&arguments),
            "glioma_decision_branch_evidence" => self.glioma_decision_branch_evidence(&arguments),
            "glioma_decision_admission_gate" => self.glioma_decision_admission_gate(&arguments),
            "glioma_decision_value_optimizer" => self.glioma_decision_value_optimizer(&arguments),
            "glioma_decision_value_calibrator" => self.glioma_decision_value_calibrator(&arguments),
            "glioma_adaptive_decision_controller" => {
                self.glioma_adaptive_decision_controller(&arguments)
            }
            "glioma_decision_loop_governor" => self.glioma_decision_loop_governor(&arguments),
            "glioma_decision_action_graph" => self.glioma_decision_action_graph(&arguments),
            "glioma_decision_mission_execute" => self.glioma_decision_mission_execute(&arguments),
            "glioma_decision_omission_certificate" => {
                self.glioma_decision_omission_certificate(&arguments)
            }
            "glioma_decision_branch_plan" => self.glioma_decision_branch_plan(&arguments),
            "glioma_decision_action_plan" => self.glioma_decision_action_plan(&arguments),
            "glioma_multimodal_qc" => self.glioma_multimodal_qc(&arguments),
            "glioma_mechanism_explore" => self.glioma_mechanism_explore(&arguments),
            "glioma_mechanism_dynamics" => self.glioma_mechanism_dynamics(&arguments),
            "glioma_mechanism_discriminate" => self.glioma_mechanism_discriminate(&arguments),
            "glioma_mechanism_identifiability" => self.glioma_mechanism_identifiability(&arguments),
            "glioma_mechanism_invariance" => self.glioma_mechanism_invariance(&arguments),
            "glioma_mechanism_intervention_value" => {
                self.glioma_mechanism_intervention_value(&arguments)
            }
            "glioma_mechanism_bayesian_update" => self.glioma_mechanism_bayesian_update(&arguments),
            "glioma_mechanism_evidence_assimilation" => {
                self.glioma_mechanism_evidence_assimilation(&arguments)
            }
            "glioma_mechanism_closed_loop" => self.glioma_mechanism_closed_loop(&arguments),
            "glioma_mechanism_multi_fidelity_control" => {
                self.glioma_mechanism_multi_fidelity_control(&arguments)
            }
            "glioma_mechanism_feedback_replan" => self.glioma_mechanism_feedback_replan(&arguments),
            "glioma_mechanism_workflow_compile" => {
                self.glioma_mechanism_workflow_compile(&arguments)
            }
            "glioma_mechanism_workflow_assure" => self.glioma_mechanism_workflow_assure(&arguments),
            "glioma_mechanism_multi_study_workflow_compile" => {
                self.glioma_mechanism_multi_study_workflow_compile(&arguments)
            }
            "glioma_mechanism_prospective_control" => {
                self.glioma_mechanism_prospective_control(&arguments)
            }
            "glioma_mechanism_fidelity_bridge" => self.glioma_mechanism_fidelity_bridge(&arguments),
            "glioma_mechanism_robustness_stress" => {
                self.glioma_mechanism_robustness_stress(&arguments)
            }
            "glioma_mechanism_state_filter" => self.glioma_mechanism_state_filter(&arguments),
            "glioma_mechanism_state_smoother" => self.glioma_mechanism_state_smoother(&arguments),
            "glioma_mechanism_consensus" => self.glioma_mechanism_consensus(&arguments),
            "glioma_mechanism_calibrate" => self.glioma_mechanism_calibrate(&arguments),
            "glioma_mechanism_action_plan" => self.glioma_mechanism_action_plan(&arguments),
            "glioma_adaptive_mechanism_policy" => self.glioma_adaptive_mechanism_policy(&arguments),
            "glioma_adaptive_mechanism_campaign_execute" => {
                self.glioma_adaptive_mechanism_campaign_execute(&arguments)
            }
            "glioma_calibrated_mechanism_campaign_execute" => {
                self.glioma_calibrated_mechanism_campaign_execute(&arguments)
            }
            "glioma_mechanism_discrimination_campaign_execute" => {
                self.glioma_mechanism_discrimination_campaign_execute(&arguments)
            }
            "glioma_mechanism_operating_cycle" => self.glioma_mechanism_operating_cycle(&arguments),
            "glioma_mechanism_graph_propagate" => self.glioma_mechanism_graph_propagate(&arguments),
            "glioma_mechanism_counterfactual" => self.glioma_mechanism_counterfactual(&arguments),
            "glioma_mechanism_ensemble_counterfactual" => {
                self.glioma_mechanism_ensemble_counterfactual(&arguments)
            }
            "glioma_robust_intervention_portfolio" => {
                self.glioma_robust_intervention_portfolio(&arguments)
            }
            "glioma_mechanism_validation_plan" => self.glioma_mechanism_validation_plan(&arguments),
            "glioma_validation_batch_assess" => self.glioma_validation_batch_assess(&arguments),
            "glioma_validation_campaign_execute" => {
                self.glioma_validation_campaign_execute(&arguments)
            }
            "glioma_validation_replication_gate" => {
                self.glioma_validation_replication_gate(&arguments)
            }
            "glioma_validation_replication_campaign_execute" => {
                self.glioma_validation_replication_campaign_execute(&arguments)
            }
            "glioma_replication_federated_transport_execute" => {
                self.glioma_replication_federated_transport_execute(&arguments)
            }
            "glioma_replication_closure_frontier" => {
                self.glioma_replication_closure_frontier(&arguments)
            }
            "glioma_replication_closure_execute" => {
                self.glioma_replication_closure_execute(&arguments)
            }
            "glioma_replication_closure_campaign_execute" => {
                self.glioma_replication_closure_campaign_execute(&arguments)
            }
            "glioma_replication_closure_interpret" => {
                self.glioma_replication_closure_interpret(&arguments)
            }
            "glioma_mechanism_validation_protocol_compile" => {
                self.glioma_mechanism_validation_protocol_compile(&arguments)
            }
            "glioma_mechanism_validation_protocol_execute" => {
                self.glioma_mechanism_validation_protocol_execute(&arguments)
            }
            "glioma_information_design" => self.glioma_information_design(&arguments),
            "glioma_adaptive_panel" => self.glioma_adaptive_panel(&arguments),
            "glioma_replication_plan" => self.glioma_replication_plan(&arguments),
            "glioma_replication_continuation" => self.glioma_replication_continuation(&arguments),
            "glioma_replication_protocol_compile" => {
                self.glioma_replication_protocol_compile(&arguments)
            }
            "glioma_robust_experiment_design" => self.glioma_robust_experiment_design(&arguments),
            "glioma_blocked_randomization_design" => {
                self.glioma_blocked_randomization_design(&arguments)
            }
            "glioma_adaptive_information_campaign" => {
                self.glioma_adaptive_information_campaign(&arguments)
            }
            "glioma_adaptive_allocation_campaign_execute" => {
                self.glioma_adaptive_allocation_campaign_execute(&arguments)
            }
            "glioma_sequential_campaign_execute" => {
                self.glioma_sequential_campaign_execute(&arguments)
            }
            "glioma_active_learning" => self.glioma_active_learning(&arguments),
            "glioma_posterior_batch" => self.glioma_posterior_batch(&arguments),
            "glioma_active_learning_campaign_execute" => {
                self.glioma_active_learning_campaign_execute(&arguments)
            }
            "glioma_robust_active_learning" => self.glioma_robust_active_learning(&arguments),
            "glioma_robust_active_learning_campaign_execute" => {
                self.glioma_robust_active_learning_campaign_execute(&arguments)
            }
            "glioma_multi_fidelity_optimize" => self.glioma_multi_fidelity_optimize(&arguments),
            "glioma_instrument_calibration" => self.glioma_instrument_calibration(&arguments),
            "glioma_instrument_signal_extract" => self.glioma_instrument_signal_extract(&arguments),
            "glioma_instrument_batch_stability" => {
                self.glioma_instrument_batch_stability(&arguments)
            }
            "glioma_instrument_multichannel_concordance" => {
                self.glioma_instrument_multichannel_concordance(&arguments)
            }
            "glioma_federated_instrument_consensus" => {
                self.glioma_federated_instrument_consensus(&arguments)
            }
            "glioma_instrument_preflight" => self.glioma_instrument_preflight(&arguments),
            "glioma_instrument_fleet_schedule" => self.glioma_instrument_fleet_schedule(&arguments),
            "glioma_instrument_fleet_execute" => self.glioma_instrument_fleet_execute(&arguments),
            "glioma_instrument_execute" => self.glioma_instrument_execute(&arguments),
            "glioma_instrument_recovery_plan" => self.glioma_instrument_recovery_plan(&arguments),
            "glioma_instrument_campaign_execute" => {
                self.glioma_instrument_campaign_execute(&arguments)
            }
            "glioma_adaptive_instrument_campaign_execute" => {
                self.glioma_adaptive_instrument_campaign_execute(&arguments)
            }
            "glioma_instrument_operating_cycle" => {
                self.glioma_instrument_operating_cycle(&arguments)
            }
            "glioma_instrument_assay_adjudicate" => {
                self.glioma_instrument_assay_adjudicate(&arguments)
            }
            "glioma_instrument_science_loop_execute" => {
                self.glioma_instrument_science_loop_execute(&arguments)
            }
            "glioma_instrument_research_frontier_execute" => {
                self.glioma_instrument_research_frontier_execute(&arguments)
            }
            "glioma_experiment_design" => self.glioma_experiment_design(&arguments),
            "glioma_heterogeneity_adaptive_benchmark_power" => {
                self.glioma_heterogeneity_adaptive_benchmark_power(&arguments)
            }
            "glioma_heterogeneity_aware_experiment_portfolio" => {
                self.glioma_heterogeneity_aware_experiment_portfolio(&arguments)
            }
            "glioma_heterogeneity_portfolio_mission" => {
                self.glioma_heterogeneity_portfolio_mission(&arguments)
            }
            "glioma_contrast_panel_design" => self.glioma_contrast_panel_design(&arguments),
            "glioma_analysis_run" => self.glioma_analysis_run(&arguments),
            "glioma_replication_assess" => self.glioma_replication_assess(&arguments),
            "glioma_replication_meta_analyze" => self.glioma_replication_meta_analyze(&arguments),
            "glioma_replication_campaign_execute" => {
                self.glioma_replication_campaign_execute(&arguments)
            }
            "glioma_autonomous_research_mission_execute" => {
                self.glioma_autonomous_research_mission_execute(&arguments)
            }
            "glioma_autonomous_research_mission_recover" => {
                self.glioma_autonomous_research_mission_recover(&arguments)
            }
            "glioma_intent_mission_execute" => self.glioma_intent_mission_execute(&arguments),
            "glioma_multimodal_mission_execute" => {
                self.glioma_multimodal_mission_execute(&arguments)
            }
            "glioma_multi_fidelity_campaign_execute" => {
                self.glioma_multi_fidelity_campaign_execute(&arguments)
            }
            "glioma_federated_benchmark_consensus" => {
                self.glioma_federated_benchmark_consensus(&arguments)
            }
            "glioma_federated_benchmark_power" => self.glioma_federated_benchmark_power(&arguments),
            "glioma_federated_interpretation" => self.glioma_federated_interpretation(&arguments),
            "glioma_federated_benchmark_site_plan" => {
                self.glioma_federated_benchmark_site_plan(&arguments)
            }
            "glioma_federated_mechanism_transport" => {
                self.glioma_federated_mechanism_transport(&arguments)
            }
            "glioma_federated_mechanism_transport_campaign_execute" => {
                self.glioma_federated_mechanism_transport_campaign_execute(&arguments)
            }
            "glioma_federated_benchmark_campaign_execute" => {
                self.glioma_federated_benchmark_campaign_execute(&arguments)
            }
            "glioma_federated_benchmark_operating_cycle" => {
                self.glioma_federated_benchmark_operating_cycle(&arguments)
            }
            "glioma_federated_adaptive_campaign_execute" => {
                self.glioma_federated_adaptive_campaign_execute(&arguments)
            }
            "glioma_replay_campaign_execute" => self.glioma_replay_campaign_execute(&arguments),
            "glioma_replay_history_reconcile" => self.glioma_replay_history_reconcile(&arguments),
            "glioma_local_release_workflow_execute" => {
                self.glioma_local_release_workflow_execute(&arguments)
            }
            "glioma_multistudy_release_reconcile" => {
                self.glioma_multistudy_release_reconcile(&arguments)
            }
            "glioma_release_batch_execute" => self.glioma_release_batch_execute(&arguments),
            "glioma_federated_continual_release_reconcile" => {
                self.glioma_federated_continual_release_reconcile(&arguments)
            }
            "glioma_local_release_review_packet_compile" => {
                self.glioma_local_release_review_packet_compile(&arguments)
            }
            "glioma_portfolio_review_workbench_reconcile" => {
                self.glioma_portfolio_review_workbench_reconcile(&arguments)
            }
            "glioma_release_batch_review_workbench_reconcile" => {
                self.glioma_release_batch_review_workbench_reconcile(&arguments)
            }
            "glioma_local_release_signature_payload_prepare" => {
                self.glioma_local_release_signature_payload_prepare(&arguments)
            }
            "glioma_local_release_signature_verify" => {
                self.glioma_local_release_signature_verify(&arguments)
            }
            "glioma_release_trust_policy_payload_prepare" => {
                self.glioma_release_trust_policy_payload_prepare(&arguments)
            }
            "glioma_release_trust_policy_evaluate" => {
                self.glioma_release_trust_policy_evaluate(&arguments)
            }
            "glioma_research_object_release_gate" => {
                self.glioma_research_object_release_gate(&arguments)
            }
            "glioma_release_operating_cycle" => self.glioma_release_operating_cycle(&arguments),
            "glioma_research_object_prepare" => self.glioma_research_object_prepare(&arguments),
            "glioma_release_disclosure_register_build" => {
                self.glioma_release_disclosure_register_build(&arguments)
            }
            "glioma_release_disclosure_panel_reconcile" => {
                self.glioma_release_disclosure_panel_reconcile(&arguments)
            }
            "glioma_release_disclosure_batch_reconcile" => {
                self.glioma_release_disclosure_batch_reconcile(&arguments)
            }
            "glioma_multimodal_research_object_prepare" => {
                self.glioma_multimodal_research_object_prepare(&arguments)
            }
            "glioma_research_object_migration_plan" => {
                self.glioma_research_object_migration_plan(&arguments)
            }
            "glioma_research_object_dependency_closure" => {
                self.glioma_research_object_dependency_closure(&arguments)
            }
            "domain_evidence_harmonization_coverage" => {
                self.domain_evidence_harmonization_coverage(&arguments)
            }
            "domain_evidence_intake" => self.domain_evidence_intake(&arguments),
            "domain_evidence_coverage" => self.domain_evidence_coverage(&arguments),
            "domain_evidence_source_plan" => self.domain_evidence_source_plan(&arguments),
            "domain_evidence_source_execute" => self.domain_evidence_source_execute(&arguments),
            "domain_evidence_provider_normalize" => {
                self.domain_evidence_provider_normalize(&arguments)
            }
            "domain_evidence_provider_replay_verify" => {
                self.domain_evidence_provider_replay_verify(&arguments)
            }
            "domain_evidence_provider_connector_handoff" => {
                self.domain_evidence_provider_connector_handoff(&arguments)
            }
            "domain_evidence_provider_external_payload_receipt" => {
                self.domain_evidence_provider_external_payload_receipt(&arguments)
            }
            "domain_evidence_provider_external_payload_replay_verify" => {
                self.domain_evidence_provider_external_payload_replay_verify(&arguments)
            }
            "domain_evidence_provider_external_payload_normalize" => {
                self.domain_evidence_provider_external_payload_normalize(&arguments)
            }
            "domain_evidence_provider_external_payload_lineage_audit" => {
                self.domain_evidence_provider_external_payload_lineage_audit(&arguments)
            }
            "domain_evidence_provider_external_payload_execution_evidence" => {
                self.domain_evidence_provider_external_payload_execution_evidence(&arguments)
            }
            "domain_evidence_provider_external_payload_evidence_query" => {
                self.domain_evidence_provider_external_payload_evidence_query(&arguments)
            }
            "domain_acquisition_catalogue" => self.domain_acquisition_catalogue(&arguments),
            "context_compare" => self.context_compare(&arguments),
            "bioworlds_catalog" => self.bioworlds_catalog(&arguments),
            "modality_catalog" => self.modality_catalog(&arguments),
            "modality_support_check" => self.modality_support_check(&arguments),
            "modality_transport_check" => self.modality_transport_check(&arguments),
            "modality_comparability_check" => self.modality_comparability_check(&arguments),
            "literature_bind_check" => self.literature_bind_check(&arguments),
            "mutation_family" => self.mutation_family(&arguments),
            "prism_minimize" => self.prism_minimize(&arguments),
            "registry_gate" => self.registry_gate(&arguments),
            "registry_lifecycle_simulate" => self.registry_lifecycle_simulate(&arguments),
            "cache_invalidation_simulate" => self.cache_invalidation_simulate(&arguments),
            "storage_lifecycle_simulate" => self.storage_lifecycle_simulate(&arguments),
            "release_audit" => self.release_audit(&arguments),
            "operations_catalog" => self.operations_catalog(&arguments),
            "ops_acceptance" => self.ops_acceptance(&arguments),
            "ops_capacity" => self.ops_capacity(&arguments),
            "research_ci_check" => self.research_ci_check(&arguments),
            "capability_rank" => self.capability_rank(&arguments),
            "metrics_profile_audit" => self.metrics_profile_audit(&arguments),
            "metrics_analytics_audit" => self.metrics_analytics_audit(&arguments),
            "biocapability_evidence_audit" => self.biocapability_evidence_audit(&arguments),
            "safety_release_gate" => self.safety_release_gate(&arguments),
            "medical_boundary_check" => self.medical_boundary_check(&arguments),
            "hub_search" => self.hub_search(&arguments),
            "hub_submission_review" => self.hub_submission_review(&arguments),
            "hub_disclosure_review" => self.hub_disclosure_review(&arguments),
            "hub_card_render" => self.hub_card_render(&arguments),
            "hub_leaderboard_render" => self.hub_leaderboard_render(&arguments),
            "bioatlas_publication_audit" => self.bioatlas_publication_audit(&arguments),
            "measurement_compare" => self.measurement_compare(&arguments),
            "hub_resolve" => self.hub_resolve(&arguments),
            "hub_lock" => self.hub_lock(&arguments),
            "adapter_plan" => self.adapter_plan(&arguments),
            "adapter_execution_evidence" => self.adapter_execution_evidence(&arguments),
            "adapter_execution_evidence_query" => self.adapter_execution_evidence_query(&arguments),
            "tabular_ingest" => self.tabular_ingest(&arguments),
            "observed_world_declare" => self.observed_world_declare(&arguments),
            "world_claim_check" => self.world_claim_check(&arguments),
            "trace_analyze" => self.trace_analyze(&arguments),
            "trace_otel_ingest" => self.trace_otel_ingest(&arguments),
            "lineage_audit" => self.lineage_audit(&arguments),
            "preanalytic_apply" => self.preanalytic_apply(&arguments),
            "contradiction_review" => self.contradiction_review(&arguments),
            "lab_plan" => self.lab_plan(&arguments),
            "lab_pareto_audit" => self.lab_pareto_audit(&arguments),
            "lab_branch_audit" => self.lab_branch_audit(&arguments),
            "lab_holdout_audit" => self.lab_holdout_audit(&arguments),
            "lab_evolution_audit" => self.lab_evolution_audit(&arguments),
            "lab_space_audit" => self.lab_space_audit(&arguments),
            "obligation_gate_check" => self.obligation_gate_check(&arguments),
            "lens_catalogue" => Ok(json!({
                "ok": true,
                "schema": bioprism_lens::LENS_REPORT_SCHEMA_VERSION,
                "section_42_module_count": bioprism_lens::SECTION_42_MODULE_COUNT,
                "implemented": lens_catalogue(),
                "implemented_count": bioprism_lens::catalogue_ids().len(),
                "not_implemented": bioprism_lens::NOT_IMPLEMENTED,
                "guarantees": [
                    "declarations expose evidence and scope requirements before execution",
                    "implemented lenses are distinguished from the named section remainder",
                    "the catalogue does not claim a renderer or external data connector",
                ],
            })),
            "lens_leakage_check" => self.lens_leakage_check(&arguments),
            "scale_family_split_verify" => self.scale_family_split_verify(&arguments),
            "megafactory_twin_audit" => self.megafactory_twin_audit(&arguments),
            "megafactory_placement_audit" => self.megafactory_placement_audit(&arguments),
            "stewardship_review_check" => self.stewardship_review_check(&arguments),
            "quality_gate_run" => self.quality_gate_run(&arguments),
            "ledger_ingest" => self.ledger_ingest(&arguments),
            "fabric_synthesize" => self.fabric_synthesize(&arguments),
            "interweave_workflow_catalogue" => {
                let workflows = interweave_catalogue();
                let outstanding = outstanding_deliverables(&workflows);
                Ok(json!({
                    "ok": true,
                    "workflow_count": workflows.len(),
                    "deliverables_per_workflow": 9,
                    "workflows": workflows,
                    "outstanding_deliverables": outstanding,
                    "guarantees": [
                        "roles, effect envelopes, owed deliverables, and adapter requirements remain separate",
                        "the catalogue describes reference workflows and does not fabricate their missing artefacts",
                        "outstanding counts are derived from the typed deliverable set rather than a prose percentage",
                    ],
                }))
            }
            "interweave_workflow_execute" => self.interweave_workflow_execute(&arguments),
            "interweave_workflow_execution_evidence" => {
                self.interweave_workflow_execution_evidence(&arguments)
            }
            "interweave_workflow_execution_evidence_import" => {
                self.interweave_workflow_execution_evidence_import(&arguments)
            }
            "interweave_workflow_execution_evidence_query" => {
                self.interweave_workflow_execution_evidence_query(&arguments)
            }
            "interweave_workflow_execution_evidence_get" => {
                self.interweave_workflow_execution_evidence_get(&arguments)
            }
            "atlas_report" => self.atlas_report(&arguments),
            "atlas_surface_audit" => self.atlas_surface_audit(&arguments),
            "adaptive_panel" => self.adaptive_panel(&arguments),
            "posterior_gate" => self.posterior_gate(&arguments),
            "oracle_combine" => self.oracle_combine(&arguments),
            "oracle_reference_panel" => self.oracle_reference_panel(&arguments),
            "oracle_missingness" => self.oracle_missingness(&arguments),
            "evaluation_worldline_audit" => self.evaluation_worldline_audit(&arguments),
            "evaluation_reproduction_check" => self.evaluation_reproduction_check(&arguments),
            "evaluation_trajectory_check" => self.evaluation_trajectory_check(&arguments),
            "evaluation_observability_card" => self.evaluation_observability_card(&arguments),
            "federated_evaluation_consensus" => self.federated_evaluation_consensus(&arguments),
            "resource_workbench_discover" => self.resource_workbench_discover(&arguments),
            "resource_discovery_contract_v2" => self.resource_discovery_contract_v2(&arguments),
            "ids_federated_resource_discovery_interoperability" => {
                self.ids_federated_resource_discovery_interoperability(&arguments)
            }
            "worldfactory_protocol_simulation_federated_control_plane" => {
                self.worldfactory_protocol_simulation_federated_control_plane(&arguments)
            }
            "worldfactory_computational_execution_federated_control_plane" => {
                self.worldfactory_computational_execution_federated_control_plane(&arguments)
            }
            "evalengine_federated_protocol_simulation_copilot" => {
                self.evalengine_federated_protocol_simulation_copilot(&arguments)
            }
            "evalengine_local_mechanism_exploration_assurance" => {
                self.evalengine_local_mechanism_exploration_assurance(&arguments)
            }
            "packs_local_quality_control_assurance" => {
                self.packs_local_quality_control_assurance(&arguments)
            }
            "atlashub_replication_negative_results_federated_control_plane" => {
                self.atlashub_replication_negative_results_federated_control_plane(&arguments)
            }
            "mcp_replication_negative_results_assurance" => {
                self.mcp_replication_negative_results_assurance(&arguments)
            }
            "prism_protocol_simulation_assurance" => {
                self.prism_protocol_simulation_assurance(&arguments)
            }
            "scale_quality_control_contract_model" => {
                self.scale_quality_control_contract_model(&arguments)
            }
            "packs_protocol_simulation_workbench" => {
                self.packs_protocol_simulation_workbench(&arguments)
            }
            "oracle_evidence_surveillance_workflow_fabric" => {
                self.oracle_evidence_surveillance_workflow_fabric(&arguments)
            }
            "epistemic_retrieval_synthesis_federated_control_plane" => {
                self.epistemic_retrieval_synthesis_federated_control_plane(&arguments)
            }
            "epistemic_experiment_design_research_workbench" => {
                self.epistemic_experiment_design_research_workbench(&arguments)
            }
            "ids_context_compilation_federated_control_plane" => {
                self.ids_context_compilation_federated_control_plane(&arguments)
            }
            "ids_knowledge_representation_federated_control_plane" => {
                self.ids_knowledge_representation_federated_control_plane(&arguments)
            }
            "ids_multimodal_ingestion_research_copilot" => {
                self.ids_multimodal_ingestion_research_copilot(&arguments)
            }
            "ids_quality_control_assurance" => self.ids_quality_control_assurance(&arguments),
            "ids_mechanism_exploration_assurance" => {
                self.ids_mechanism_exploration_assurance(&arguments)
            }
            "ids_experiment_design_workbench" => self.ids_experiment_design_workbench(&arguments),
            "ids_protocol_simulation_workbench" => {
                self.ids_protocol_simulation_workbench(&arguments)
            }
            "ids_laboratory_integration_workflow_fabric" => {
                self.ids_laboratory_integration_workflow_fabric(&arguments)
            }
            "ids_computational_execution_workbench" => {
                self.ids_computational_execution_workbench(&arguments)
            }
            "ids_statistical_causal_ml_research_copilot" => {
                self.ids_statistical_causal_ml_research_copilot(&arguments)
            }
            "ids_retrieval_synthesis_assurance_harness" => {
                self.ids_retrieval_synthesis_assurance_harness(&arguments)
            }
            "ids_replication_negative_results_interoperability_gateway" => {
                self.ids_replication_negative_results_interoperability_gateway(&arguments)
            }
            "ids_publication_research_object_release_control_plane" => {
                self.ids_publication_research_object_release_control_plane(&arguments)
            }
            "ids_typed_determinism_interoperability_gateway" => {
                self.ids_typed_determinism_interoperability_gateway(&arguments)
            }
            "ids_typed_determinism_assurance" => self.ids_typed_determinism_assurance(&arguments),
            "ids_prospective_provenance_assurance" => {
                self.ids_prospective_provenance_assurance(&arguments)
            }
            "ids_policy_autonomy_workbench" => self.ids_policy_autonomy_workbench(&arguments),
            "ids_federation_security_contract" => self.ids_federation_security_contract(&arguments),
            "ids_performance_reliability_gateway" => {
                self.ids_performance_reliability_gateway(&arguments)
            }
            "ids_interoperability_extensibility_copilot" => {
                self.ids_interoperability_extensibility_copilot(&arguments)
            }
            "ids_provenance_signing_assurance" => self.ids_provenance_signing_assurance(&arguments),
            "ids_policy_autonomy_interoperability_gateway" => {
                self.ids_policy_autonomy_interoperability_gateway(&arguments)
            }
            "ids_federated_workflow_fabric" => self.ids_federated_workflow_fabric(&arguments),
            "ids_reliability_copilot" => self.ids_reliability_copilot(&arguments),
            "ids_interoperability_gateway" => self.ids_interoperability_gateway(&arguments),
            "ids_evaluation_assurance" => self.ids_evaluation_assurance(&arguments),
            "ids_research_workbench" => self.ids_research_workbench(&arguments),
            "ids_contract_frontier" => self.ids_contract_frontier(&arguments),
            "ids_limitation_closure" => self.ids_limitation_closure(&arguments),
            "ids_dependency_composition" => self.ids_dependency_composition(&arguments),
            "ids_semantic_parity" => self.ids_semantic_parity(&arguments),
            "ids_scale_frontier" => self.ids_scale_frontier(&arguments),
            "ids_adversarial_recovery" => self.ids_adversarial_recovery(&arguments),
            "ids_federated_commons" => self.ids_federated_commons(&arguments),
            "ids_bounded_evolution" => self.ids_bounded_evolution(&arguments),
            "worldgen_multimodal_ingestion" => self.worldgen_multimodal_ingestion(&arguments),
            "worldgen_multimodal_execution" => self.worldgen_multimodal_execution(&arguments),
            "atlasx_mechanism_contract" => self.atlasx_mechanism_contract(&arguments),
            "routing_execution_copilot" => self.routing_execution_copilot(&arguments),
            "routing_laboratory_inference_engine" => {
                self.routing_laboratory_inference_engine(&arguments)
            }
            "devx_context_compilation_contract" => {
                self.devx_context_compilation_contract(&arguments)
            }
            "devx_evidence_surveillance_control" => {
                self.devx_evidence_surveillance_control(&arguments)
            }
            "governance_research_release_compile" => {
                self.governance_research_release_compile(&arguments)
            }
            "release_assurance_harness" => self.release_assurance_harness(&arguments),
            "obligation_knowledge_representation_assurance" => {
                self.obligation_knowledge_representation_assurance(&arguments)
            }
            "obligation_security_federation_interoperability_gateway" => {
                self.obligation_security_federation_interoperability_gateway(&arguments)
            }
            "protocol_assurance_harness" => self.protocol_assurance_harness(&arguments),
            "federated_multimodal_assurance" => self.federated_multimodal_assurance(&arguments),
            "federated_knowledge_gateway" => self.federated_knowledge_gateway(&arguments),
            "federated_lens_assurance" => self.federated_lens_assurance(&arguments),
            "lab_semantic_parity" => self.lab_semantic_parity(&arguments),
            "federated_retrieval_assurance" => self.federated_retrieval_assurance(&arguments),
            "backends_federated_retrieval_synthesis_workflow" => {
                self.backends_federated_retrieval_synthesis_workflow(&arguments)
            }
            "retrieval_synthesis_operations" => self.retrieval_synthesis_operations(&arguments),
            "bioethics_evidence_surveillance" => self.bioethics_evidence_surveillance(&arguments),
            "bioethics_prospective_computational_execution_assurance" => {
                self.bioethics_prospective_computational_execution_assurance(&arguments)
            }
            "bioethics_scale_frontier_contract" => {
                self.bioethics_scale_frontier_contract(&arguments)
            }
            "scale_federation_trust_control_plane" => {
                self.scale_federation_trust_control_plane(&arguments)
            }
            "mcp_federated_quality_control" => self.mcp_federated_quality_control(&arguments),
            "onco_federated_provenance_signing" => {
                self.onco_federated_provenance_signing(&arguments)
            }
            "onco_instrument_research_workbench" => {
                self.onco_instrument_research_workbench(&arguments)
            }
            "mutation_federated_publication_release" => {
                self.mutation_federated_publication_release(&arguments)
            }
            "mutation_federated_continual_bounded_evolution_assurance" => {
                self.mutation_federated_continual_bounded_evolution_assurance(&arguments)
            }
            "mutation_federated_resource_discovery_control_plane" => {
                self.mutation_federated_resource_discovery_control_plane(&arguments)
            }
            "factory_prospective_evidence_surveillance" => {
                self.factory_prospective_evidence_surveillance(&arguments)
            }
            "factory_federated_quality_workbench" => {
                self.factory_federated_quality_workbench(&arguments)
            }
            "fiber_federated_resource_workbench" => {
                self.fiber_federated_resource_workbench(&arguments)
            }
            "fiber_federated_analysis_control_plane" => {
                self.fiber_federated_analysis_control_plane(&arguments)
            }
            "docgraph_instrument_action_contract" => {
                self.docgraph_instrument_action_contract(&arguments)
            }
            "lens_provenance_signing_copilot" => self.lens_provenance_signing_copilot(&arguments),
            "obligation_prospective_release_assurance" => {
                self.obligation_prospective_release_assurance(&arguments)
            }
            "atlasx_federated_execution_control_plane" => {
                self.atlasx_federated_execution_control_plane(&arguments)
            }
            "atlasx_computational_execution_assurance" => {
                self.atlasx_computational_execution_assurance(&arguments)
            }
            "atlasx_context_compilation_assurance" => {
                self.atlasx_context_compilation_assurance(&arguments)
            }
            "atlashub_quality_control_research_copilot" => {
                self.atlashub_quality_control_research_copilot(&arguments)
            }
            "atlashub_quality_control_contract_model" => {
                self.atlashub_quality_control_contract_model(&arguments)
            }
            "bioworlds_resource_discovery_copilot" => {
                self.bioworlds_resource_discovery_copilot(&arguments)
            }
            "bioworlds_knowledge_workflow_fabric" => {
                self.bioworlds_knowledge_workflow_fabric(&arguments)
            }
            "bioworlds_federated_context_research_workbench" => {
                self.bioworlds_federated_context_research_workbench(&arguments)
            }
            "adapter_federated_context_copilot" => {
                self.adapter_federated_context_copilot(&arguments)
            }
            "routing_limitation_closure_workflow" => {
                self.routing_limitation_closure_workflow(&arguments)
            }
            "devplat_multimodal_limitation_closure_assurance" => {
                self.devplat_multimodal_limitation_closure_assurance(&arguments)
            }
            "interweave_federated_interpretation_engine" => {
                self.interweave_federated_interpretation_engine(&arguments)
            }
            "interweave_federated_commons_assurance" => {
                self.interweave_federated_commons_assurance(&arguments)
            }
            "lab_instrument_interoperability_gateway" => {
                self.lab_instrument_interoperability_gateway(&arguments)
            }
            "policy_federated_analysis_copilot" => {
                self.policy_federated_analysis_copilot(&arguments)
            }
            "prism_analysis_workbench" => self.prism_analysis_workbench(&arguments),
            "services_multimodal_interpretation" => {
                self.services_multimodal_interpretation(&arguments)
            }
            "services_context_compilation_research_copilot" => {
                self.services_context_compilation_research_copilot(&arguments)
            }
            "federated_continual_retrieval_copilot" => {
                self.federated_continual_retrieval_copilot(&arguments)
            }
            "federated_context_compilation_assurance" => {
                self.federated_context_compilation_assurance(&arguments)
            }
            "federated_knowledge_representation_assurance" => {
                self.federated_knowledge_representation_assurance(&arguments)
            }
            "federated_resource_control_plane" => self.federated_resource_control_plane(&arguments),
            "weavelang_release_assurance" => self.weavelang_release_assurance(&arguments),
            "weavelang_federated_commons_assurance" => {
                self.weavelang_federated_commons_assurance(&arguments)
            }
            "federated_mechanism_control_plane" => {
                self.federated_mechanism_control_plane(&arguments)
            }
            "megafactory_mechanism_exploration_federated_control_plane" => {
                self.megafactory_mechanism_exploration_federated_control_plane(&arguments)
            }
            "federated_mechanism_gateway" => self.federated_mechanism_gateway(&arguments),
            "evidence_surveillance_copilot" => self.evidence_surveillance_copilot(&arguments),
            "ids_local_evidence_surveillance_inference" => {
                self.ids_local_evidence_surveillance_inference(&arguments)
            }
            "scope_federated_evidence_control" => self.scope_federated_evidence_control(&arguments),
            "scope_federated_commons_interoperability_gateway" => {
                self.scope_federated_commons_interoperability_gateway(&arguments)
            }
            "hubapi_federated_experiment_design_assurance" => {
                self.hubapi_federated_experiment_design_assurance(&arguments)
            }
            "fabric_experiment_design_contract_model" => {
                self.fabric_experiment_design_contract_model(&arguments)
            }
            "bioethics_multimodal_context_compilation_assurance" => {
                self.bioethics_multimodal_context_compilation_assurance(&arguments)
            }
            "bioethics_statistical_analysis_assurance" => {
                self.bioethics_statistical_analysis_assurance(&arguments)
            }
            "prism_laboratory_integration_copilot" => {
                self.prism_laboratory_integration_copilot(&arguments)
            }
            "scale_interpretation_visualization_assurance" => {
                self.scale_interpretation_visualization_assurance(&arguments)
            }
            "scale_interpretation_interoperability_gateway" => {
                self.scale_interpretation_interoperability_gateway(&arguments)
            }
            "bioethics_experiment_design_workflow_fabric" => {
                self.bioethics_experiment_design_workflow_fabric(&arguments)
            }
            "bioethics_multimodal_bounded_evolution_assurance" => {
                self.bioethics_multimodal_bounded_evolution_assurance(&arguments)
            }
            "stress_federated_multimodal_ingestion_contract_model" => {
                self.stress_federated_multimodal_ingestion_contract_model(&arguments)
            }
            "onco_computational_execution_contract_model" => {
                self.onco_computational_execution_contract_model(&arguments)
            }
            "oracle_interoperability_research_workbench" => {
                self.oracle_interoperability_research_workbench(&arguments)
            }
            "atlashub_provenance_signing_inference_engine" => {
                self.atlashub_provenance_signing_inference_engine(&arguments)
            }
            "hub_policy_autonomy_inference_engine" => {
                self.hub_policy_autonomy_inference_engine(&arguments)
            }
            "conformance_retrieval_synthesis_contract_model" => {
                self.conformance_retrieval_synthesis_contract_model(&arguments)
            }
            "adapter_local_evidence_surveillance_research_copilot" => {
                self.adapter_local_evidence_surveillance_research_copilot(&arguments)
            }
            "adapter_multimodal_evidence_surveillance_research_copilot" => {
                self.adapter_multimodal_evidence_surveillance_research_copilot(&arguments)
            }
            "adapter_throughput_evidence_surveillance_research_copilot" => {
                self.adapter_throughput_evidence_surveillance_research_copilot(&arguments)
            }
            "adapter_federated_continual_evidence_surveillance_research_copilot" => {
                self.adapter_federated_continual_evidence_surveillance_research_copilot(&arguments)
            }
            "adapter_local_evidence_surveillance_workflow_fabric" => {
                self.adapter_local_evidence_surveillance_workflow_fabric(&arguments)
            }
            "adapter_multimodal_evidence_surveillance_workflow_fabric" => {
                self.adapter_multimodal_evidence_surveillance_workflow_fabric(&arguments)
            }
            "adapter_throughput_evidence_surveillance_workflow_fabric" => {
                self.adapter_throughput_evidence_surveillance_workflow_fabric(&arguments)
            }
            "adapter_federated_continual_evidence_surveillance_workflow_fabric" => {
                self.adapter_federated_continual_evidence_surveillance_workflow_fabric(&arguments)
            }
            "adapter_local_evidence_surveillance_research_workbench" => {
                self.adapter_local_evidence_surveillance_research_workbench(&arguments)
            }
            "adapter_multimodal_evidence_surveillance_research_workbench" => {
                self.adapter_multimodal_evidence_surveillance_research_workbench(&arguments)
            }
            "adapter_throughput_evidence_surveillance_research_workbench" => {
                self.adapter_throughput_evidence_surveillance_research_workbench(&arguments)
            }
            "adapter_federated_continual_evidence_surveillance_research_workbench" => self
                .adapter_federated_continual_evidence_surveillance_research_workbench(&arguments),
            "multimodal_retrieval_synthesis" => self.multimodal_retrieval_synthesis(&arguments),
            "adapter_local_retrieval_synthesis_inference_engine" => {
                self.adapter_local_retrieval_synthesis_inference_engine(&arguments)
            }
            "adapter_local_retrieval_synthesis_contract_model" => {
                self.adapter_local_retrieval_synthesis_contract_model(&arguments)
            }
            "adapter_local_retrieval_synthesis_research_copilot" => {
                self.adapter_local_retrieval_synthesis_research_copilot(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_research_copilot" => {
                self.adapter_multimodal_retrieval_synthesis_research_copilot(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_research_copilot" => {
                self.adapter_throughput_retrieval_synthesis_research_copilot(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_research_copilot" => {
                self.adapter_federated_continual_retrieval_synthesis_research_copilot(&arguments)
            }
            "adapter_local_retrieval_synthesis_workflow_fabric" => {
                self.adapter_local_retrieval_synthesis_workflow_fabric(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_workflow_fabric" => {
                self.adapter_multimodal_retrieval_synthesis_workflow_fabric(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_workflow_fabric" => {
                self.adapter_throughput_retrieval_synthesis_workflow_fabric(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_workflow_fabric" => {
                self.adapter_federated_continual_retrieval_synthesis_workflow_fabric(&arguments)
            }
            "adapter_local_retrieval_synthesis_research_workbench" => {
                self.adapter_local_retrieval_synthesis_research_workbench(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_research_workbench" => {
                self.adapter_multimodal_retrieval_synthesis_research_workbench(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_research_workbench" => {
                self.adapter_throughput_retrieval_synthesis_research_workbench(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_research_workbench" => {
                self.adapter_federated_continual_retrieval_synthesis_research_workbench(&arguments)
            }
            "adapter_local_retrieval_synthesis_interoperability_gateway" => {
                self.adapter_local_retrieval_synthesis_interoperability_gateway(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_interoperability_gateway" => {
                self.adapter_multimodal_retrieval_synthesis_interoperability_gateway(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_interoperability_gateway" => {
                self.adapter_throughput_retrieval_synthesis_interoperability_gateway(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_interoperability_gateway" => self
                .adapter_federated_continual_retrieval_synthesis_interoperability_gateway(
                    &arguments,
                ),
            "adapter_local_retrieval_synthesis_assurance_harness" => {
                self.adapter_local_retrieval_synthesis_assurance_harness(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_assurance_harness" => {
                self.adapter_multimodal_retrieval_synthesis_assurance_harness(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_assurance_harness" => {
                self.adapter_throughput_retrieval_synthesis_assurance_harness(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_assurance_harness" => {
                self.adapter_federated_continual_retrieval_synthesis_assurance_harness(&arguments)
            }
            "adapter_local_retrieval_synthesis_federated_control_plane" => {
                self.adapter_local_retrieval_synthesis_federated_control_plane(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_federated_control_plane" => {
                self.adapter_multimodal_retrieval_synthesis_federated_control_plane(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_federated_control_plane" => {
                self.adapter_throughput_retrieval_synthesis_federated_control_plane(&arguments)
            }
            "adapter_federated_continual_retrieval_synthesis_federated_control_plane" => self
                .adapter_federated_continual_retrieval_synthesis_federated_control_plane(
                    &arguments,
                ),
            "foundation_mechanism_exploration_assurance" => {
                self.foundation_mechanism_exploration_assurance(&arguments)
            }
            "atlashub_mechanism_exploration_assurance" => {
                self.atlashub_mechanism_exploration_assurance(&arguments)
            }
            "dataops_provenance_signing_workflow_fabric" => {
                self.dataops_provenance_signing_workflow_fabric(&arguments)
            }
            "oraclex_publication_release" => self.oraclex_publication_release(&arguments),
            "oraclex_interpretation_inference" => self.oraclex_interpretation_inference(&arguments),
            "oraclex_performance_reliability_interoperability_gateway" => {
                self.oraclex_performance_reliability_interoperability_gateway(&arguments)
            }
            "oraclex_statistical_analysis_research_workbench" => {
                self.oraclex_statistical_analysis_research_workbench(&arguments)
            }
            "interweave_frontier_control" => self.interweave_frontier_control(&arguments),
            "influence_federated_continual_interpretation" => {
                self.influence_federated_continual_interpretation(&arguments)
            }
            "influence_local_evidence_surveillance_assurance" => {
                self.influence_local_evidence_surveillance_assurance(&arguments)
            }
            "safety_prospective_laboratory_integration_assurance" => {
                self.safety_prospective_laboratory_integration_assurance(&arguments)
            }
            "adapter_multimodal_retrieval_synthesis_inference_engine" => {
                self.adapter_multimodal_retrieval_synthesis_inference_engine(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_inference_engine" => {
                self.adapter_throughput_retrieval_synthesis_inference_engine(&arguments)
            }
            "adapter_throughput_retrieval_synthesis_contract_model" => {
                self.adapter_throughput_retrieval_synthesis_contract_model(&arguments)
            }
            "adapter_federated_retrieval_synthesis_inference_engine" => {
                self.adapter_federated_retrieval_synthesis_inference_engine(&arguments)
            }
            "adapter_federated_retrieval_synthesis_contract_model" => {
                self.adapter_federated_retrieval_synthesis_contract_model(&arguments)
            }
            "adapter_context_compilation_assurance" => {
                self.adapter_context_compilation_assurance(&arguments)
            }
            "multimodal_knowledge_workflow" => self.multimodal_knowledge_workflow(&arguments),
            "adapter_resource_workbench" => self.adapter_resource_workbench(&arguments),
            "adapter_ingestion_gateway" => self.adapter_ingestion_gateway(&arguments),
            "adapter_quality_envelope" => self.adapter_quality_envelope(&arguments),
            "adapter_experiment_design_control" => {
                self.adapter_experiment_design_control(&arguments)
            }
            "governance_experiment_design_assurance" => {
                self.governance_experiment_design_assurance(&arguments)
            }
            "adapter_protocol_simulation" => self.adapter_protocol_simulation(&arguments),
            "adapter_instrument_mesh" => self.adapter_instrument_mesh(&arguments),
            "adapter_execution_control" => self.adapter_execution_control(&arguments),
            "adapter_analysis_portfolio" => self.adapter_analysis_portfolio(&arguments),
            "adapter_interpretation_assurance" => self.adapter_interpretation_assurance(&arguments),
            "governance_federated_continual_interpretation_assurance" => {
                self.governance_federated_continual_interpretation_assurance(&arguments)
            }
            "adapter_replication_assurance" => self.adapter_replication_assurance(&arguments),
            "adapter_release_assurance" => self.adapter_release_assurance(&arguments),
            "adapter_determinism_gateway" => self.adapter_determinism_gateway(&arguments),
            "adapter_provenance_assurance" => self.adapter_provenance_assurance(&arguments),
            "adapter_policy_gateway" => self.adapter_policy_gateway(&arguments),
            "adapter_federation_workflow" => self.adapter_federation_workflow(&arguments),
            "adapter_reliability_copilot" => self.adapter_reliability_copilot(&arguments),
            "adapter_interoperability_gateway" => self.adapter_interoperability_gateway(&arguments),
            "adapter_evaluation_assurance" => self.adapter_evaluation_assurance(&arguments),
            "adapter_research_workbench" => self.adapter_research_workbench(&arguments),
            "adapter_contract_frontier" => self.adapter_contract_frontier(&arguments),
            "adapter_limitation_closure" => self.adapter_limitation_closure(&arguments),
            "adapter_dependency_composition" => self.adapter_dependency_composition(&arguments),
            "adapter_semantic_parity" => self.adapter_semantic_parity(&arguments),
            "adapter_scale_frontier" => self.adapter_scale_frontier(&arguments),
            "adapter_adversarial_recovery" => self.adapter_adversarial_recovery(&arguments),
            "adapter_federated_commons" => self.adapter_federated_commons(&arguments),
            "adapter_bounded_evolution" => self.adapter_bounded_evolution(&arguments),
            "mcp_bounded_evolution_assurance" => self.mcp_bounded_evolution_assurance(&arguments),
            "research_release_validate" => self.research_release_validate(&arguments),
            "research_release_batch_validate" => self.research_release_batch_validate(&arguments),
            "instrument_preflight" => self.instrument_preflight(&arguments),
            "multimodal_harmonize" => self.multimodal_harmonize(&arguments),
            "mcp_multimodal_ingestion_assurance" => {
                self.mcp_multimodal_ingestion_assurance(&arguments)
            }
            "weavelang_computational_execution_assurance" => {
                self.weavelang_computational_execution_assurance(&arguments)
            }
            "mcp_knowledge_representation_contract" => {
                self.mcp_knowledge_representation_contract(&arguments)
            }
            "registry_multimodal_scale_frontier_assurance" => {
                self.registry_multimodal_scale_frontier_assurance(&arguments)
            }
            "registry_knowledge_representation_assurance" => {
                self.registry_knowledge_representation_assurance(&arguments)
            }
            "registry_replication_workbench" => self.registry_replication_workbench(&arguments),
            "ops_context_compilation_federated_control_plane" => {
                self.ops_context_compilation_federated_control_plane(&arguments)
            }
            "oraclex_context_compilation_research_copilot" => {
                self.oraclex_context_compilation_research_copilot(&arguments)
            }
            "analysis_qualify" => self.analysis_qualify(&arguments),
            "protocol_matrix_simulate" => self.protocol_matrix_simulate(&arguments),
            "multimodal_replication_evaluate" => self.multimodal_replication_evaluate(&arguments),
            "quality_drift_evaluate" => self.quality_drift_evaluate(&arguments),
            "design_frontier_evaluate" => self.design_frontier_evaluate(&arguments),
            "autonomy_batch_admit" => self.autonomy_batch_admit(&arguments),
            "workflow_batch_execute" => self.workflow_batch_execute(&arguments),
            "runtime_interpretation_assurance" => self.runtime_interpretation_assurance(&arguments),
            "runtime_knowledge_representation_assurance" => {
                self.runtime_knowledge_representation_assurance(&arguments)
            }
            "fabric_experiment_design_interoperability_gateway" => {
                self.fabric_experiment_design_interoperability_gateway(&arguments)
            }
            "lab_federated_experiment_design_interoperability_gateway" => {
                self.lab_federated_experiment_design_interoperability_gateway(&arguments)
            }
            "stress_publication_research_object_workbench" => {
                self.stress_publication_research_object_workbench(&arguments)
            }
            "ids_federated_interpretation_visualization_assurance" => {
                self.ids_federated_interpretation_visualization_assurance(&arguments)
            }
            "bioeval_reference_audit" => self.bioeval_reference_audit(&arguments),
            "bioeval_acquisition_audit" => self.bioeval_acquisition_audit(&arguments),
            "bioeval_grounding_audit" => self.bioeval_grounding_audit(&arguments),
            "bioeval_estimand_audit" => self.bioeval_estimand_audit(&arguments),
            "bioeval_evaluator_audit" => self.bioeval_evaluator_audit(&arguments),
            "bioeval_plane_audit" => self.bioeval_plane_audit(&arguments),
            "bioeval_metamorphic_audit" => self.bioeval_metamorphic_audit(&arguments),
            "bioeval_waiver_audit" => self.bioeval_waiver_audit(&arguments),
            "bioeval_design_audit" => self.bioeval_design_audit(&arguments),
            "bioeval_mesh_audit" => self.bioeval_mesh_audit(&arguments),
            "bioeval_burden_audit" => self.bioeval_burden_audit(&arguments),
            "bioeval_reveal_audit" => self.bioeval_reveal_audit(&arguments),
            "bioeval_boundary_audit" => self.bioeval_boundary_audit(&arguments),
            "runtime_effect_check" => self.runtime_effect_check(&arguments),
            "runtime_tape_verify" => self.runtime_tape_verify(&arguments),
            "runtime_execution_simulate" => self.runtime_execution_simulate(&arguments),
            "runtime_workflow_execute" => self.runtime_workflow_execute(&arguments),
            "onco_boundary_check" => self.onco_boundary_check(&arguments),
            "onco_response_assess" => self.onco_response_assess(&arguments),
            "onco_worldline_view" => self.onco_worldline_view(&arguments),
            "onco_classification_check" => self.onco_classification_check(&arguments),
            "onco_outcome_analyze" => self.onco_outcome_analyze(&arguments),
            "oncoworlds_identity_join" => self.oncoworlds_identity_join(&arguments),
            "oncoworlds_model_transport" => self.oncoworlds_model_transport(&arguments),
            "oncoworlds_methylation_classify" => self.oncoworlds_methylation_classify(&arguments),
            "oncoworlds_methylation_compare" => self.oncoworlds_methylation_compare(&arguments),
            "oncoworlds_radiogenomic_check" => self.oncoworlds_radiogenomic_check(&arguments),
            "oncoworlds_clonal_history_check" => self.oncoworlds_clonal_history_check(&arguments),
            "oncoworlds_clonal_evidence_check" => self.oncoworlds_clonal_evidence_check(&arguments),
            "oncoworlds_era_shift_check" => self.oncoworlds_era_shift_check(&arguments),
            "oncoworlds_equity_check" => self.oncoworlds_equity_check(&arguments),
            "oncoworlds_entity_world_check" => self.oncoworlds_entity_world_check(&arguments),
            "oncoworlds_federated_statistical_analysis_workbench" => {
                self.oncoworlds_federated_statistical_analysis_workbench(&arguments)
            }
            "oncoworlds_prospective_evidence_surveillance_copilot" => {
                self.oncoworlds_prospective_evidence_surveillance_copilot(&arguments)
            }
            "oncoworlds_prospective_replication_negative_results_assurance" => {
                self.oncoworlds_prospective_replication_negative_results_assurance(&arguments)
            }
            "oncoworlds_federated_resource_discovery_assurance" => {
                self.oncoworlds_federated_resource_discovery_assurance(&arguments)
            }
            "stress_profile" => self.stress_profile(&arguments),
            "stress_report" => self.stress_report(&arguments),
            "bundle_verify" => self.bundle_verify(&arguments),
            "policy_screen" => self.policy_screen(&arguments),
            "bioethics_action_review" => self.bioethics_action_review(&arguments),
            "bioethics_human_subject_screen" => self.bioethics_human_subject_screen(&arguments),
            "bioethics_dual_use_review" => self.bioethics_dual_use_review(&arguments),
            "bioethics_validation_check" => self.bioethics_validation_check(&arguments),
            "bioethics_representation_audit" => self.bioethics_representation_audit(&arguments),
            "influence_analyze" => self.influence_analyze(&arguments),
            "routing_decide" => self.routing_decide(&arguments),
            "token_context_plan" => self.token_context_plan(&arguments),
            "bioql_compile" => self.bioql_compile(&arguments),
            "epistemic_voi" => self.epistemic_voi(&arguments),
            "epistemic_adaptive_acquisition" => self.epistemic_adaptive_acquisition(&arguments),
            "epistemic_adaptive_costed" => self.epistemic_adaptive_costed(&arguments),
            "epistemic_adaptive_execute" => self.epistemic_adaptive_execute(&arguments),
            "epistemic_decision_quotient" => self.epistemic_decision_quotient(&arguments),
            "epistemic_context_audit" => self.epistemic_context_audit(&arguments),
            "epistemic_selection_audit" => self.epistemic_selection_audit(&arguments),
            "routing_lab_run" => self.routing_lab_run(&arguments),
            "benchmark_trace_analyze" => self.benchmark_trace_analyze(&arguments),
            "benchmark_decision_audit" => self.benchmark_decision_audit(&arguments),
            "benchmark_integrity_audit" => self.benchmark_integrity_audit(&arguments),
            "benchmark_counterfactual_check" => self.benchmark_counterfactual_check(&arguments),
            "benchmark_oracle_review" => self.benchmark_oracle_review(&arguments),
            "benchmark_compile" => self.benchmark_compile(&arguments),
            "benchmark_compile_review" => self.benchmark_compile_review(&arguments),
            "pack_coverage_audit" => self.pack_coverage_audit(&arguments),
            "pack_release_audit" => self.pack_release_audit(&arguments),
            "pack_catalogue" => self.pack_catalogue(&arguments),
            "pack_health_assess" => self.pack_health_assess(&arguments),
            "foundation_contract_check" => self.foundation_contract_check(&arguments),
            "weavelang_compile" => self.weavelang_compile(&arguments),
            "choreography_check" => self.choreography_check(&arguments),
            "conformance_run" => self.conformance_run(&arguments),
            "conformance_context_compilation_federated_control" => {
                self.conformance_context_compilation_federated_control(&arguments)
            }
            "conformance_context_compilation_assurance" => {
                self.conformance_context_compilation_assurance(&arguments)
            }
            "federated_publication_release_inference" => {
                self.federated_publication_release_inference(&arguments)
            }
            "mutation_knowledge_federated_control" => {
                self.mutation_knowledge_federated_control(&arguments)
            }
            "provider_capability_gate" => self.provider_capability_gate(&arguments),
            "sdk_registry_check" => self.sdk_registry_check(&arguments),
            "governance_schema_check" => self.governance_schema_check(&arguments),
            "developer_platform_status" => self.developer_platform_status(&arguments),
            "developer_delivery_audit" => self.developer_delivery_audit(&arguments),
            "developer_delivery_receipt" => self.developer_delivery_receipt(&arguments),
            "developer_delivery_receipt_verify" => {
                self.developer_delivery_receipt_verify(&arguments)
            }
            "engineering_manifest_audit" => self.engineering_manifest_audit(&arguments),
            "engineering_execution_plan" => self.engineering_execution_plan(&arguments),
            "release_pipeline_audit" => self.release_pipeline_audit(&arguments),
            "operational_readiness_audit" => self.operational_readiness_audit(&arguments),
            "security_privacy_audit" => self.security_privacy_audit(&arguments),
            "sandbox_admission_audit" => self.sandbox_admission_audit(&arguments),
            "sandbox_runtime_simulate" => self.sandbox_runtime_simulate(&arguments),
            "security_program_audit" => self.security_program_audit(&arguments),
            "developer_workbench" => self.developer_workbench(&arguments),
            "developer_workbench_verify" => self.developer_workbench_verify(&arguments),
            "developer_workbench_import" => self.developer_workbench_import(&arguments),
            "developer_workbench_query" => self.developer_workbench_query(&arguments),
            "developer_workbench_get" => self.developer_workbench_get(&arguments),
            "ci_provider_normalize" => self.ci_provider_normalize(&arguments),
            "ci_provider_evidence_audit" => self.ci_provider_evidence_audit(&arguments),
            "ci_provider_evidence_import" => self.ci_provider_evidence_import(&arguments),
            "ci_provider_evidence_query" => self.ci_provider_evidence_query(&arguments),
            "ci_provider_evidence_get" => self.ci_provider_evidence_get(&arguments),
            "ci_execution_evidence_audit" => self.ci_execution_evidence_audit(&arguments),
            "execution_provenance_audit" => self.execution_provenance_audit(&arguments),
            "agent_mission" => self.agent_mission(&arguments),
            "autopilot_drive" => self.autopilot_drive(&arguments),
            "autopilot_verify" => self.autopilot_verify(&arguments),
            "autopilot_goal_step" => self.autopilot_goal_step(&arguments),
            "autopilot_goal_verify" => self.autopilot_goal_verify(&arguments),
            "capability_audit" => self.capability_audit(&arguments),
            "capability_dashboard" => self.capability_dashboard(&arguments),
            "capability_discover" => self.capability_discover(&arguments),
            "mission_evaluator_discover" => self.mission_evaluator_discover(&arguments),
            "mission_evaluator_review" => self.mission_evaluator_review(&arguments),
            "mission_evaluator_replay" => self.mission_evaluator_replay(&arguments),
            "mission_evaluator_replay_compare" => self.mission_evaluator_replay_compare(&arguments),
            "mission_evidence_bundle_verify" => self.mission_evidence_bundle_verify(&arguments),
            "mission_evidence_bundle_import" => self.mission_evidence_bundle_import(&arguments),
            "mission_evidence_bundle_query" => self.mission_evidence_bundle_query(&arguments),
            "mission_evidence_bundle_get" => self.mission_evidence_bundle_get(&arguments),
            "capability_route" => self.capability_route(&arguments),
            "capability_route_review" => self.capability_route_review(&arguments),
            "capability_route_plan" => self.capability_route_plan(&arguments),
            "capability_route_plan_verify" => self.capability_route_plan_verify(&arguments),
            "domain_workflow_catalogue" => self.domain_workflow_catalogue(&arguments),
            "domain_workflow_scaffold" => self.domain_workflow_scaffold(&arguments),
            "domain_workflow_instantiate" => self.domain_workflow_instantiate(&arguments),
            "domain_workflow_portfolio" => self.domain_workflow_portfolio(&arguments),
            "domain_workflow_portfolio_verify" => self.domain_workflow_portfolio_verify(&arguments),
            "domain_workflow_verify" => self.domain_workflow_verify(&arguments),
            "domain_workflow_reconcile" => self.domain_workflow_reconcile(&arguments),
            "domain_workflow_reconciliation_import" => {
                self.domain_workflow_reconciliation_import(&arguments)
            }
            "domain_workflow_reconciliation_query" => {
                self.domain_workflow_reconciliation_query(&arguments)
            }
            "domain_workflow_reconciliation_get" => {
                self.domain_workflow_reconciliation_get(&arguments)
            }
            "safety_posture" => self.safety_posture(&arguments),
            "security_redteam_simulate" => self.security_redteam_simulate(&arguments),
            "weave_protocol_catalog" => Ok(weave_protocol_catalog()),
            "workspace_capabilities" => Ok(workspace_capabilities()),
            "repository_catalog" => self.repository_catalog(&arguments),
            "repository_bundle" => self.repository_bundle(&arguments),
            "repository_impact" => self.repository_impact(&arguments),
            "telemetry_project" => self.telemetry_project(&arguments),
            other => Err(format!("unknown tool {other:?}")),
        };

        match outcome {
            Ok(mut value) => {
                self.index_trusted_tool_output(name, &arguments, &mut value);
                Response::result(id, tool_content(&value, false))
            }
            Err(message) => Response::result(
                id,
                tool_content(&json!({ "ok": false, "error": message }), true),
            ),
        }
    }
}
