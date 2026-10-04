//! MCP contract tests for the autonomous research area.

use super::*;

#[test]
fn glioma_sequential_campaign_executes_and_replans_in_sandbox() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_sequential_campaign_execute",
        json!({
            "request": {
                "design": {
                    "objective": "close the replicate loop for organoid invasion",
                    "model_system": "organoid",
                    "endpoint": "invasion_fraction",
                    "control_arm_id": "control",
                    "target_effect_milli": 150,
                    "success_probability_milli": 750,
                    "futility_probability_milli": 700,
                    "min_replicates_per_arm": 3,
                    "max_new_replicates_per_round": 2,
                    "max_rounds": 4,
                    "max_selected_arms": 2,
                    "budget_units": 12,
                    "risk_ceiling_milli": 800,
                    "exploration_weight_milli": 400
                },
                "arms": [
                    {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"campaign-control","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sequential+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":1,"failures":1,"prior_alpha":1,"prior_beta":1,"risk_milli":200,"cost_units":1},
                    {"arm_id":"candidate","label":"candidate perturbation","artifact":{"artifact_id":"campaign-candidate","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sequential+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":1,"failures":0,"prior_alpha":1,"prior_beta":1,"risk_milli":200,"cost_units":1}
                ],
                "max_retries": 1
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("not_started"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert!(
        campaign["campaign"]["rounds"]
            .as_array()
            .is_some_and(|rounds| !rounds.is_empty())
    );
    assert!(
        campaign["campaign"]["batches"]
            .as_array()
            .is_some_and(|batches| !batches.is_empty())
    );
    assert!(campaign["campaign"]["final_plan"]["decisions"].is_array());
}

#[test]
fn glioma_active_learning_campaign_executes_and_replans_in_sandbox() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_active_learning_campaign_execute",
        json!({
            "request": {
                "active_learning": {
                    "objective": "refine an organoid invasion mechanism",
                    "model_system": "organoid",
                    "direction": "maximize",
                    "budget_units": 4,
                    "max_selections": 1,
                    "min_observations_per_candidate": 1,
                    "exploration_weight_milli": 500,
                    "exploitation_weight_milli": 500,
                    "cost_penalty_milli": 1,
                    "risk_penalty_milli": 1,
                    "max_risk_milli": 800,
                    "min_uncertainty_milli": 900
                },
                "candidates": [
                    {"candidate_id":"egfr","mechanism_id":"egfr-signaling","feature_vector":[100,0],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"receptor","output_schema":"Assay1@1"},
                    {"candidate_id":"matrix","mechanism_id":"matrix-remodeling","feature_vector":[0,100],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"matrix","output_schema":"Assay1@1"}
                ],
                "observations": [{"observation_id":"seed","candidate_id":"egfr","outcome_milli":500,"uncertainty_milli":50,"artifact":{"artifact_id":"seed","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}],
                "max_rounds": 3,
                "max_retries": 1,
                "stop_on_unresolved": false
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("not_started"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(
        campaign["campaign"]["output_schema"],
        json!("GliomaActiveLearningCampaign1@2")
    );
    assert!(campaign["campaign"]["input_digest"].is_string());
    assert!(
        campaign["campaign"]["rounds"]
            .as_array()
            .is_some_and(|rounds| !rounds.is_empty())
    );
    assert!(
        campaign["campaign"]["budget_spent_units"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
}

#[test]
fn glioma_mechanism_discrimination_campaign_replans_measurement_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_mechanism_discrimination_campaign_execute",
        json!({
            "request": {
                "discrimination": {
                    "objective": "resolve glioma invasion mechanisms",
                    "model_system": "organoid",
                    "min_shared_features": 2,
                    "max_mechanisms": 4,
                    "max_actions": 2,
                    "min_information_gain_milli": 10
                },
                "hypotheses": [
                    {"mechanism_id":"motility","statement":"motility drives invasion","predictions":[{"feature_id":"f1","predicted_milli":100,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":200,"uncertainty_milli":10}]},
                    {"mechanism_id":"matrix","statement":"matrix remodeling drives invasion","predictions":[{"feature_id":"f1","predicted_milli":400,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":500,"uncertainty_milli":10}]}
                ],
                "actions": [{"action_id":"measure-f1","feature_id":"f1","predicted_milli_by_mechanism":{"matrix":500,"motility":100},"measurement_uncertainty_milli":20,"feasibility_milli":1000,"cost_units":1}],
                "observations": [
                    {"feature_id":"f1","observed_milli":100,"uncertainty_milli":10,"artifact":{"artifact_id":"campaign-f1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"feature_id":"f2","observed_milli":200,"uncertainty_milli":10,"artifact":{"artifact_id":"campaign-f2","content_hash":hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "budget_units": 1,
                "max_rounds": 3,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["completed_action_order"],
        json!(["measure-f1"])
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);

    let cycle = call(
        &mut server,
        "glioma_mechanism_operating_cycle",
        json!({
            "request": {
                "campaign": {
                    "discrimination": {
                        "objective": "resolve glioma invasion mechanisms",
                        "model_system": "organoid",
                        "min_shared_features": 2,
                        "max_mechanisms": 4,
                        "max_actions": 2,
                        "min_information_gain_milli": 10
                    },
                    "hypotheses": [
                        {"mechanism_id":"motility","statement":"motility drives invasion","predictions":[{"feature_id":"f1","predicted_milli":100,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":200,"uncertainty_milli":10}]},
                        {"mechanism_id":"matrix","statement":"matrix remodeling drives invasion","predictions":[{"feature_id":"f1","predicted_milli":400,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":500,"uncertainty_milli":10}]}
                    ],
                    "actions": [{"action_id":"measure-f1","feature_id":"f1","predicted_milli_by_mechanism":{"matrix":500,"motility":100},"measurement_uncertainty_milli":20,"feasibility_milli":1000,"cost_units":1}],
                    "observations": [
                        {"feature_id":"f1","observed_milli":100,"uncertainty_milli":10,"artifact":{"artifact_id":"cycle-f1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                        {"feature_id":"f2","observed_milli":200,"uncertainty_milli":10,"artifact":{"artifact_id":"cycle-f2","content_hash":hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                    ],
                    "budget_units": 1,
                    "max_rounds": 3,
                    "max_retries": 1,
                    "stop_on_qualified": true
                },
                    "action_plan": {"model_system":"organoid","modality":"transcriptomics","max_actions":2,"budget_units":2}
            }
        }),
    );
    assert_eq!(cycle["dispatch"], json!("dry_run"));
    assert_eq!(cycle["simulation_only"], json!(true));
    assert_eq!(
        cycle["cycle"]["campaign"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        cycle["cycle"]["action_plan"]["source_discrimination_digest"],
        cycle["cycle"]["campaign"]["final_discrimination"]["digest"]
    );

    let readiness = call(
        &mut server,
        "glioma_multimodal_readiness_gate",
        json!({
            "request": {
                "campaign": {
                    "request": {
                        "study_id": "study-mcp-ingestion",
                        "required_modalities": ["genomics", "imaging"],
                        "required_model_systems": ["organoid"],
                        "expected_coordinate_system": "pixel",
                        "expected_unit_system": "count",
                        "max_missing_fraction_milli": 100
                    },
                    "observations": [{
                        "observation_id": "seed-genomics",
                        "study_id": "study-mcp-ingestion",
                        "sample_lineage": "sample-seed",
                        "modality": "genomics",
                        "model_system": "organoid",
                        "batch_id": "batch-1",
                        "coordinate_system": "pixel",
                        "unit_system": "count",
                        "missing_fraction_milli": 0,
                        "feature_count": 10,
                        "artifact": {"artifact_id":"ingestion-artifact","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                    }],
                    "max_actions_per_round": 4,
                    "budget_units": 2,
                    "cost_per_action_units": 1,
                    "max_rounds": 3,
                    "max_retries": 1,
                    "stop_on_qualified": true
                },
                "min_comparable_observations": 1,
                "min_coverage_milli": 1000,
                "min_quality_milli": 800,
                "require_complete_report": true,
                "required_surfaces": ["analysis", "mechanism", "experiment_design", "replication"]
            }
        }),
    );
    assert_eq!(readiness["dispatch"], json!("dry_run"));
    assert_eq!(readiness["simulation_only"], json!(true));
    assert_eq!(readiness["readiness"]["coverage_milli"], json!(1000));
    assert!(
        readiness["readiness"]["admitted_surface_order"]
            .as_array()
            .unwrap()
            .contains(&json!("analysis"))
    );
    assert!(
        readiness["readiness"]["conditional_surface_order"]
            .as_array()
            .unwrap()
            .contains(&json!("replication"))
    );

    let evidence_cycle = call(
        &mut server,
        "glioma_evidence_operating_cycle",
        json!({
            "request": {
                "planning": {
                    "objective": "map reproducible preclinical glioma invasion evidence",
                    "budget_units": 2,
                    "max_candidates": 8,
                    "max_selected": 1,
                    "beam_width": 8,
                    "min_source_families": 1,
                    "max_per_independence_group": 1,
                    "max_privacy_risk_milli": 200,
                    "min_portfolio_score_milli": 0,
                    "required_modalities": ["literature"],
                    "required_model_systems": ["organoid"],
                    "weights": {"support_milli": 180, "uncertainty_reduction_milli": 180, "contradiction_resolution_milli": 160, "freshness_milli": 90, "workflow_leverage_milli": 150, "reproducibility_milli": 120, "failure_penalty_milli": 70, "cost_penalty_milli": 50}
                },
                "candidates": [{"candidate_id":"literature-invasion", "target_claim":"invasion program is reproducible", "source_family":"preclinical-literature", "source_kind":"literature", "modality":"literature", "model_system":"organoid", "independence_group":"source-a", "depends_on":[], "cost_units":1, "expected_support_milli":800, "expected_uncertainty_reduction_milli":700, "contradiction_resolution_milli":500, "freshness_milli":800, "workflow_leverage_milli":700, "reproducibility_milli":800, "failure_probability_milli":50, "privacy_risk_milli":10, "local_only":true, "contains_human_data":false}],
                "max_retries": 1,
                "stop_on_negative": false,
                "require_artifacts": true,
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(evidence_cycle["dispatch"], json!("dry_run"));
    assert_eq!(evidence_cycle["simulation_only"], json!(true));
    assert_eq!(
        evidence_cycle["cycle"]["phase_order"][0],
        json!("portfolio_selection")
    );
    assert!(
        !evidence_cycle["cycle"]["campaign"]["unknown_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let multimodal_cycle = call(
        &mut server,
        "glioma_multimodal_operating_cycle",
        json!({
            "request": {
                "readiness": {
                    "campaign": {
                        "request": {"study_id":"cycle-study", "required_modalities":["genomics"], "required_model_systems":["organoid"], "expected_coordinate_system":"sample-local", "expected_unit_system":"normalized", "max_missing_fraction_milli":100},
                        "observations": [{"observation_id":"cycle-observation", "study_id":"cycle-study", "sample_lineage":"sample-1", "modality":"genomics", "model_system":"organoid", "batch_id":"batch-1", "coordinate_system":"sample-local", "unit_system":"normalized", "missing_fraction_milli":10, "feature_count":100, "artifact":{"artifact_id":"cycle-artifact", "content_hash":hash, "content_type":"application/json", "local_only":true, "contains_human_data":false, "contains_direct_identifiers":false}}],
                        "max_actions_per_round": 2,
                        "budget_units": 2,
                        "cost_per_action_units": 1,
                        "max_rounds": 2,
                        "max_retries": 1,
                        "stop_on_qualified": true
                    },
                    "min_comparable_observations": 1,
                    "min_coverage_milli": 1000,
                    "min_quality_milli": 800,
                    "require_complete_report": true,
                    "required_surfaces": ["analysis"]
                },
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(multimodal_cycle["dispatch"], json!("dry_run"));
    assert_eq!(multimodal_cycle["simulation_only"], json!(true));
    assert_eq!(
        multimodal_cycle["cycle"]["phase_order"][1],
        json!("surface_readiness")
    );
    assert_eq!(multimodal_cycle["cycle"]["disposition"], json!("ready"));
}

#[test]
fn glioma_scientific_frontier_admits_only_ready_next_batch() {
    let mut server = server();
    let hash = "0".repeat(64);
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {"objective":"rank invasion mechanisms","required_modalities":["genomics"],"required_model_systems":["organoid"],"min_support_milli":700,"min_sources_per_claim":1,"max_claims":8},
            "records": [{"evidence_id":"frontier-e1","source_artifact":{"artifact_id":"frontier-a1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}]
        }),
    );
    let knowledge_frontier = call(
        &mut server,
        "glioma_knowledge_frontier",
        json!({"request":{"objective":"rank invasion mechanisms","max_selected_claims":4,"min_priority_milli":0,"weights":{"coverage_debt_milli":250,"contradiction_milli":250,"uncertainty_milli":200,"support_milli":150,"workflow_leverage_milli":150}},"knowledge":knowledge["knowledge"].clone()}),
    );
    let readiness = call(
        &mut server,
        "glioma_multimodal_readiness_gate",
        json!({
            "request": {
                "campaign": {"request":{"study_id":"frontier-study","required_modalities":["genomics","imaging"],"required_model_systems":["organoid"],"expected_coordinate_system":"sample-local","expected_unit_system":"count","max_missing_fraction_milli":100},"observations":[{"observation_id":"frontier-observation-genomics","study_id":"frontier-study","sample_lineage":"sample-1","modality":"genomics","model_system":"organoid","batch_id":"batch-1","coordinate_system":"sample-local","unit_system":"count","missing_fraction_milli":0,"feature_count":10,"artifact":{"artifact_id":"frontier-artifact-genomics","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},{"observation_id":"frontier-observation-imaging","study_id":"frontier-study","sample_lineage":"sample-1","modality":"imaging","model_system":"organoid","batch_id":"batch-1","coordinate_system":"sample-local","unit_system":"count","missing_fraction_milli":0,"feature_count":10,"artifact":{"artifact_id":"frontier-artifact-imaging","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}],"max_actions_per_round":2,"budget_units":2,"cost_per_action_units":1,"max_rounds":2,"max_retries":1,"stop_on_qualified":true},
                "min_comparable_observations":1,"min_coverage_milli":1000,"min_quality_milli":800,"require_complete_report":true,"required_surfaces":["mechanism"]
            }
        }),
    );
    let plan = call(
        &mut server,
        "glioma_scientific_frontier",
        json!({
            "request": {
                "objective":"rank invasion mechanisms",
                "knowledge":knowledge["knowledge"].clone(),
                "frontier":knowledge_frontier["frontier"].clone(),
                "readiness":readiness["readiness"].clone(),
                "candidates":[{"action_id":"frontier-mechanism","stage_kind":"mechanism_exploration","modality":"genomics","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":900,"frontier_novelty_milli":800,"workflow_leverage_milli":800,"cross_stage_unlock_milli":900,"reproducibility_safety_milli":900,"federation_value_milli":200,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}],
                "candidate_claim_links":[{"action_id":"frontier-mechanism","claim_order":knowledge["knowledge"]["claim_order"].clone()}],
                "completed_action_order":[],
                "selection":{"budget_units":2,"max_actions":1,"approval_granted":true,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}}
            }
        }),
    );
    assert_eq!(
        plan["dispatch"],
        json!("not_started"),
        "scientific-frontier planning error: {}",
        plan["error"]
    );
    assert_eq!(plan["simulation_only"], json!(true));
    assert_eq!(
        plan["plan"]["admitted_order"],
        json!(["frontier-mechanism"])
    );
    assert_eq!(
        plan["plan"]["selection"]["selected_order"],
        json!(["frontier-mechanism"])
    );
    assert_eq!(plan["next_route"], json!("glioma_autonomous_program_cycle"));
}

#[test]
fn glioma_robust_active_learning_campaign_replans_in_sandbox() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_robust_active_learning_campaign_execute",
        json!({
            "request": {
                "robust_active_learning": {
                    "objective": "execute robust organoid invasion assays",
                    "model_system": "organoid",
                    "direction": "maximize",
                    "budget_units": 4,
                    "max_selections": 1,
                    "min_observations_per_candidate": 1,
                    "lower_tail_weight_milli": 600,
                    "disagreement_weight_milli": 300,
                    "information_weight_milli": 100,
                    "cost_penalty_milli": 1,
                    "risk_penalty_milli": 1,
                    "max_risk_milli": 800,
                    "min_model_reliability_milli": 500,
                    "models": [{"model_id":"mechanistic","prior_weight_milli":1000,"intercept_milli":0,"feature_weights":[1,2],"residual_milli":40,"reliability_milli":900}]
                },
                "candidates": [
                    {"candidate_id":"egfr","mechanism_id":"egfr","feature_vector":[100,0],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"receptor","output_schema":"Assay1@1"},
                    {"candidate_id":"matrix","mechanism_id":"matrix","feature_vector":[0,100],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"matrix","output_schema":"Assay1@1"}
                ],
                "observations": [],
                "max_rounds": 3,
                "max_retries": 1,
                "stop_on_unresolved": false
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("not_started"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(
        campaign["campaign"]["output_schema"],
        json!("GliomaRobustActiveLearningCampaign1@2")
    );
    assert!(campaign["campaign"]["input_digest"].is_string());
    assert!(
        campaign["campaign"]["completed_order"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}

#[test]
fn glioma_dynamic_policy_evaluation_ranks_supported_workflow_frontier() {
    let mut server = server();
    let hash = "0".repeat(64);
    let evaluation = call(
        &mut server,
        "glioma_dynamic_policy_evaluate",
        json!({
            "request": {
                "objective": "rank organoid invasion experiment policies",
                "model_system": "organoid",
                "horizon": 3,
                "reference_policy_id": "reference",
                "maximize_effect": true,
                "min_trajectories": 2,
                "min_coverage_milli": 500,
                "min_propensity_milli": 100,
                "max_weight_milli": 20000,
                "min_effect_milli": 20,
                "max_leave_one_out_shift_milli": 500
            },
            "policies": [
                {"policy_id":"candidate","label":"candidate","risk_milli":200,"cost_units":2,"rules":[
                    {"time_step":0,"state_key":"baseline","action_id":"candidate"},
                    {"time_step":1,"state_key":"baseline","action_id":"candidate"},
                    {"time_step":2,"state_key":"baseline","action_id":"candidate"}
                ]},
                {"policy_id":"reference","label":"reference","risk_milli":100,"cost_units":1,"rules":[
                    {"time_step":0,"state_key":"baseline","action_id":"reference"},
                    {"time_step":1,"state_key":"baseline","action_id":"reference"},
                    {"time_step":2,"state_key":"baseline","action_id":"reference"}
                ]}
            ],
            "trajectories": [
                {"trajectory_id":"t-0","unit_id":"u-0","model_system":"organoid","artifact":{"artifact_id":"a-0","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"observations":[
                    {"observation_id":"t-0-0","time_step":0,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500},
                    {"observation_id":"t-0-1","time_step":1,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500},
                    {"observation_id":"t-0-2","time_step":2,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500}
                ]},
                {"trajectory_id":"t-1","unit_id":"u-1","model_system":"organoid","artifact":{"artifact_id":"a-1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"observations":[
                    {"observation_id":"t-1-0","time_step":0,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500},
                    {"observation_id":"t-1-1","time_step":1,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500},
                    {"observation_id":"t-1-2","time_step":2,"state_key":"baseline","action_id":"reference","outcome_milli":10,"propensity_milli":500}
                ]},
                {"trajectory_id":"t-2","unit_id":"u-2","model_system":"organoid","artifact":{"artifact_id":"a-2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"observations":[
                    {"observation_id":"t-2-0","time_step":0,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500},
                    {"observation_id":"t-2-1","time_step":1,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500},
                    {"observation_id":"t-2-2","time_step":2,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500}
                ]},
                {"trajectory_id":"t-3","unit_id":"u-3","model_system":"organoid","artifact":{"artifact_id":"a-3","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"observations":[
                    {"observation_id":"t-3-0","time_step":0,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500},
                    {"observation_id":"t-3-1","time_step":1,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500},
                    {"observation_id":"t-3-2","time_step":2,"state_key":"baseline","action_id":"candidate","outcome_milli":80,"propensity_milli":500}
                ]}
            ]
        }),
    );
    assert_eq!(evaluation["dispatch"], json!("not_started"));
    assert_eq!(
        evaluation["evaluation"]["selected_policy_id"],
        json!("candidate")
    );
    assert_eq!(evaluation["evaluation"]["disposition"], json!("qualified"));
}

#[test]
fn glioma_evidence_refresh_campaign_replans_stale_evidence_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_evidence_refresh_campaign_execute",
        json!({
            "request": {
                "surveillance": {
                    "objective": "refresh stale preclinical glioma invasion evidence",
                    "required_modalities": [],
                    "required_model_systems": [],
                    "min_priority_milli": 0,
                    "max_actions": 4,
                    "score_shift_threshold_milli": 50
                },
                "previous_records": [{
                    "evidence_id": "refresh-stale-1",
                    "source_artifact": {"artifact_id":"refresh-artifact","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "preclinical glioma",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                }],
                "current_records": [{
                    "evidence_id": "refresh-stale-1",
                    "source_artifact": {"artifact_id":"refresh-artifact","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "preclinical glioma",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "stale",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                }],
                "budget_units": 1,
                "cost_per_action_units": 1,
                "max_rounds": 1,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["current_records"][0]["state"],
        json!("supported")
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_knowledge_resolution_campaign_recompiles_frontier_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_knowledge_resolution_campaign_execute",
        json!({
            "request": {
                "knowledge": {
                    "objective": "resolve preclinical glioma invasion claims",
                    "required_modalities": ["genomics"],
                    "required_model_systems": ["organoid"],
                    "min_support_milli": 700,
                    "min_sources_per_claim": 1,
                    "max_claims": 8
                },
                "frontier": {
                    "objective": "resolve preclinical glioma invasion claims",
                    "max_selected_claims": 1,
                    "min_priority_milli": 0,
                    "weights": {
                        "coverage_debt_milli": 250,
                        "contradiction_milli": 250,
                        "uncertainty_milli": 200,
                        "support_milli": 150,
                        "workflow_leverage_milli": 150
                    }
                },
                "records": [{
                    "evidence_id": "knowledge-campaign-seed",
                    "source_artifact": {"artifact_id":"knowledge-campaign-artifact","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "preclinical glioma invasion",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                }],
                "budget_units": 1,
                "cost_per_action_units": 1,
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["final_knowledge"]["disposition"],
        json!("qualified")
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_multimodal_ingestion_campaign_replans_missing_modality_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_multimodal_ingestion_campaign_execute",
        json!({
            "request": {
                "request": {
                    "study_id": "study-mcp-ingestion",
                    "required_modalities": ["genomics", "imaging"],
                    "required_model_systems": ["organoid"],
                    "expected_coordinate_system": "pixel",
                    "expected_unit_system": "count",
                    "max_missing_fraction_milli": 100
                },
                "observations": [{
                    "observation_id": "seed-genomics",
                    "study_id": "study-mcp-ingestion",
                    "sample_lineage": "sample-seed",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "batch_id": "batch-1",
                    "coordinate_system": "pixel",
                    "unit_system": "count",
                    "missing_fraction_milli": 0,
                    "feature_count": 10,
                    "artifact": {"artifact_id":"ingestion-artifact","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                }],
                "max_actions_per_round": 4,
                "budget_units": 2,
                "cost_per_action_units": 1,
                "max_rounds": 3,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["final_report"]["disposition"],
        json!("qualified")
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_replication_campaign_routes_heterogeneity_to_bounded_action() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_replication_campaign_execute",
        json!({
            "request": {
                "objective": "replicate a preclinical glioma invasion effect",
                "model_system": "organoid",
                "target_model_system": "organoid",
                "target_signature": [1, 2],
                "min_sites": 3,
                "min_replicates_per_site": 2,
                "min_studies": 3,
                "min_replicates_per_study": 2,
                "effect_threshold_milli": 10,
                "max_heterogeneity_milli": 500,
                "max_i2_milli": 500,
                "min_signal_to_noise_milli": 10,
                "max_leave_one_out_shift_milli": 1000,
                "min_quality_milli": 500,
                "distance_scale_milli": 1000,
                "max_transport_gap_milli": 500,
                "max_transport_heterogeneity_milli": 500,
                "budget_units": 16,
                "max_rounds": 2,
                "max_actions_per_round": 1,
                "max_retries": 1,
                "initial_studies": [
                    {"study_id":"s1","site_id":"site-1","model_system":"organoid","effect_milli":250,"uncertainty_milli":100,"replicate_count":3,"artifact":{"artifact_id":"a1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "initial_transport_studies": [],
                "replay_identity": hash
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(
        campaign["campaign"]["rounds"][0]["candidate_actions"][0]["kind"],
        json!("replicate_study")
    );
    assert_eq!(campaign["campaign"]["disposition"], json!("partial"));
}

#[test]
fn glioma_autonomous_research_mission_adapts_frontier_in_sandbox() {
    let mut server = server();
    let mission = call(
        &mut server,
        "glioma_autonomous_research_mission_execute",
        json!({
            "request": {
                "mission_id": "m-invasion-frontier",
                "objective": "resolve preclinical glioma invasion mechanism",
                "candidates": [
                    {"action_id":"mechanism-organoid","stage_kind":"mechanism_exploration","modality":"transcriptomics","model_system":"organoid","depends_on":[],"cost_units":2,"information_gain_milli":800,"frontier_novelty_milli":700,"workflow_leverage_milli":600,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":200,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation","write_local_artifact"]},
                    {"action_id":"mechanism-mouse","stage_kind":"mechanism_exploration","modality":"imaging","model_system":"mouse_model","depends_on":[],"cost_units":2,"information_gain_milli":500,"frontier_novelty_milli":700,"workflow_leverage_milli":600,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":200,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation","write_local_artifact"]}
                ],
                "completed_action_order": [],
                "selection": {"budget_units":4,"max_actions":1,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}},
                "gates": {"required_stages":["mechanism_exploration"],"min_completed_actions":1,"min_information_gain_milli":500,"max_uncertainty_milli":10000,"min_model_systems":1,"min_modalities":1},
                "max_rounds":2,
                "max_retries":1,
                "require_artifacts":true,
                "stop_on_negative":false
            }
        }),
    );
    assert_eq!(mission["dispatch"], json!("dry_run"));
    assert_eq!(mission["simulation_only"], json!(true));
    assert_eq!(mission["mission"]["disposition"], json!("qualified"));
    assert_eq!(mission["mission"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_autonomous_research_mission_recovery_keeps_clean_run_without_retry() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_autonomous_research_mission_recover",
        json!({
            "request": {
                "initial": {
                    "mission_id": "m-invasion-recovery",
                    "objective": "resolve preclinical glioma invasion mechanism",
                    "candidates": [
                        {"action_id":"mechanism-organoid","stage_kind":"mechanism_exploration","modality":"transcriptomics","model_system":"organoid","depends_on":[],"cost_units":2,"information_gain_milli":800,"frontier_novelty_milli":700,"workflow_leverage_milli":600,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":200,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation","write_local_artifact"]}
                    ],
                    "completed_action_order": [],
                    "selection": {"budget_units":2,"max_actions":1,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}},
                    "gates": {"required_stages":["mechanism_exploration"],"min_completed_actions":1,"min_information_gain_milli":500,"max_uncertainty_milli":10000,"min_model_systems":1,"min_modalities":1},
                    "max_rounds":1,
                    "max_retries":1,
                    "require_artifacts":true,
                    "stop_on_negative":false
                },
                "recovery_budget_units": 2,
                "recovery_max_rounds": 1,
                "require_clean_qualification": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(
        campaign["campaign"]["disposition"],
        json!("no_recovery_needed")
    );
    assert!(campaign["campaign"]["recovery"].is_null());
}

#[test]
fn glioma_multi_fidelity_campaign_replans_screening_batches_in_sandbox() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_multi_fidelity_campaign_execute",
        json!({
            "request": {
                "optimization": {
                    "objective": "maximize preclinical glioma invasion suppression",
                    "direction": "maximize",
                    "budget_units": 6,
                    "max_selections": 1,
                    "min_replicates_per_candidate": 1,
                    "exploration_weight_milli": 500,
                    "exploitation_weight_milli": 300,
                    "transfer_weight_milli": 200,
                    "risk_penalty_milli": 1,
                    "cost_penalty_milli": 1,
                    "max_risk_milli": 800,
                    "min_transfer_reliability_milli": 250,
                    "baseline_milli": 0
                },
                "candidates": [
                    {"candidate_id":"screen-egfr","design_id":"egfr","fidelity":"screening","model_system":"organoid","dose_milli":100,"combination_milli":0,"cost_units":2,"risk_milli":100,"parent_candidate_id":null,"max_replicates":2},
                    {"candidate_id":"screen-matrix","design_id":"matrix","fidelity":"screening","model_system":"organoid","dose_milli":120,"combination_milli":0,"cost_units":2,"risk_milli":100,"parent_candidate_id":null,"max_replicates":2}
                ],
                "observations": [],
                "max_rounds": 4,
                "max_retries": 1,
                "require_artifacts": true,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert!(campaign["campaign"]["rounds"].as_array().unwrap().len() >= 2);
    assert!(
        campaign["campaign"]["observations"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
}

#[test]
fn glioma_adaptive_mechanism_campaign_replans_from_sandbox_observation() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_adaptive_mechanism_campaign_execute",
        json!({
            "request": {
                "policy": {
                    "objective": "discriminate invasion mechanisms in organoids",
                    "model_system": "organoid",
                    "horizon": 2,
                    "budget_units": 2,
                    "max_actions": 4,
                    "outcome_bucket_width_milli": 10,
                    "information_weight_milli": 700,
                    "effect_weight_milli": 100,
                    "robustness_weight_milli": 200,
                    "risk_penalty_milli": 1,
                    "cost_penalty_milli": 1,
                    "redundancy_penalty_milli": 20,
                    "min_feasibility_milli": 500,
                    "stop_entropy_milli": 50,
                    "models": [
                        {"model_id":"m1","label":"invasion-led","prior_milli":500},
                        {"model_id":"m2","label":"matrix-led","prior_milli":500}
                    ],
                    "actions": [
                        {"action_id":"assay-a","label":"measure invasion","target_node_id":"node-a","modality":"functional_perturbation","redundancy_group":"pathway","cost_units":1,"risk_milli":50,"feasibility_milli":950,"predictions":[{"model_id":"m1","outcome_milli":100,"effect_milli":100,"uncertainty_milli":20},{"model_id":"m2","outcome_milli":900,"effect_milli":900,"uncertainty_milli":20}]},
                        {"action_id":"assay-b","label":"measure matrix","target_node_id":"node-b","modality":"spatial","redundancy_group":"spatial","cost_units":1,"risk_milli":50,"feasibility_milli":950,"predictions":[{"model_id":"m1","outcome_milli":200,"effect_milli":200,"uncertainty_milli":20},{"model_id":"m2","outcome_milli":220,"effect_milli":220,"uncertainty_milli":20}]}
                    ],
                    "observations": []
                },
                "max_rounds": 2,
                "max_retries": 1,
                "require_artifacts": true,
                "stop_on_converged": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert!(
        !campaign["campaign"]["rounds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !campaign["campaign"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        campaign["campaign"]["completed_action_order"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}

#[test]
fn glioma_calibrated_mechanism_campaign_discounts_model_trust() {
    let mut server = server();
    let artifact_hash = ContentHash::of_value(&json!({"artifact": "calibrated-campaign"})).unwrap();
    let calibration = call(
        &mut server,
        "glioma_mechanism_calibrate",
        json!({
            "request": {"objective":"calibrated invasion policy","model_system":"organoid","min_observations_per_mechanism":2,"max_mechanisms":4,"max_rounds":4,"max_calibration_error_milli":200000,"max_brier_loss_milli":200000},
            "observations": [
                {"round_index":0,"mechanism_id":"m1","feature_id":"f1","predicted_milli":800000,"observed_milli":780000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-m1-f1","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":1,"mechanism_id":"m1","feature_id":"f2","predicted_milli":700000,"observed_milli":680000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-m1-f2","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":0,"mechanism_id":"m2","feature_id":"f1","predicted_milli":200000,"observed_milli":220000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-m2-f1","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":1,"mechanism_id":"m2","feature_id":"f2","predicted_milli":300000,"observed_milli":320000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-m2-f2","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    let campaign = call(
        &mut server,
        "glioma_calibrated_mechanism_campaign_execute",
        json!({
            "request": {
                "policy": {
                    "policy": {
                        "objective":"calibrated invasion policy",
                        "model_system":"organoid",
                        "horizon":1,"budget_units":2,"max_actions":4,"outcome_bucket_width_milli":10,
                        "information_weight_milli":700,"effect_weight_milli":100,"robustness_weight_milli":200,
                        "risk_penalty_milli":1,"cost_penalty_milli":1,"redundancy_penalty_milli":20,
                        "min_feasibility_milli":500,"stop_entropy_milli":50,
                        "models":[{"model_id":"m1","label":"invasion-led","prior_milli":500},{"model_id":"m2","label":"matrix-led","prior_milli":500}],
                        "actions":[
                            {"action_id":"assay-a","label":"measure invasion","target_node_id":"node-a","modality":"functional_perturbation","redundancy_group":"pathway","cost_units":1,"risk_milli":50,"feasibility_milli":950,"predictions":[{"model_id":"m1","outcome_milli":100,"effect_milli":100,"uncertainty_milli":20},{"model_id":"m2","outcome_milli":900,"effect_milli":900,"uncertainty_milli":20}]},
                            {"action_id":"assay-b","label":"measure matrix","target_node_id":"node-b","modality":"spatial","redundancy_group":"spatial","cost_units":1,"risk_milli":50,"feasibility_milli":950,"predictions":[{"model_id":"m1","outcome_milli":200,"effect_milli":200,"uncertainty_milli":20},{"model_id":"m2","outcome_milli":220,"effect_milli":220,"uncertainty_milli":20}]}
                        ],
                        "observations":[]
                    },
                    "max_rounds":2,"max_retries":1,"require_artifacts":true,"stop_on_converged":false
                },
                "calibration": calibration["calibration"].clone(),
                "min_coverage_milli":1000,"max_calibration_error_milli":200000,"max_brier_loss_milli":200000,"min_trust_milli":500,"calibration_exploration_weight_milli":500,
                "max_rounds":2,"max_retries":1,"require_artifacts":true,"stop_on_calibrated":false,"allow_uncalibrated_exploration":false
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["calibration_gate_open"], json!(true));
    assert!(
        !campaign["campaign"]["rounds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        campaign["campaign"]["rounds"][0]["action_scores"]
            .as_array()
            .unwrap()
            .iter()
            .any(|score| score["posterior_model_trust_milli"]
                .as_u64()
                .unwrap_or_default()
                > 0)
    );
}

#[test]
fn glioma_research_director_compiles_and_executes_a_focus_aware_batch() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_research_director_execute",
        json!({
            "request": {
                "intent": {
                    "research_id": "director-research",
                    "study_id": "director-study",
                    "objective": "identify reproducible invasion mechanisms in organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "mechanism_first",
                "completed_checkpoints": [],
                "budget_units": 80,
                "max_actions": 5,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["director"]["disposition"], json!("partial"));
    assert!(
        !response["director"]["next_stage_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        response["director"]["negative_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("synthetic-dry-run"))
    );
}

#[test]
fn glioma_program_scheduler_batches_independent_intents_and_preserves_job_state() {
    let mut server = server();
    let hash = "0".repeat(64);
    let director = |job_id: &str, research_id: &str, study_id: &str, objective: &str| {
        json!({
            "intent": {
                "research_id": research_id,
                "study_id": study_id,
                "objective": objective,
                "output_uses": ["cohort_analysis"],
                "model_systems": ["organoid"],
                "modalities": ["transcriptomics", "imaging", "spatial"],
                "input_artifacts": [{"artifact_id":format!("input-{job_id}"),"content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                "requested_autonomy": "a1",
                "approval_reference": null,
                "budget_units": 160,
                "max_retries": 1,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "raw_data_local": true,
                "aggregate_only": true,
                "replay_identity": hash,
                "boundary": PRECLINICAL_BOUNDARY
            },
            "focus": "mechanism_first",
            "completed_checkpoints": [],
            "budget_units": 80,
            "max_actions": 2,
            "approval_granted": false,
            "allow_instrument_execution": false,
            "allow_federation": false,
            "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
            "max_retries": 1,
            "require_artifacts": true
        })
    };
    let response = call(
        &mut server,
        "glioma_program_scheduler_execute",
        json!({
            "request": {
                "objective": "schedule independent glioma invasion and stemness programs",
                "jobs": [
                    {"job_id":"invasion","director":director("invasion","scheduler-invasion","study-invasion","identify reproducible invasion mechanisms"),"priority_milli":900,"fairness_weight_milli":400},
                    {"job_id":"stemness","director":director("stemness","scheduler-stemness","study-stemness","identify reproducible stemness mechanisms"),"priority_milli":500,"fairness_weight_milli":900}
                ],
                "global_budget_units": 80,
                "max_rounds": 2,
                "max_jobs_per_round": 2,
                "resource_capacities": [
                    {"model_system":"organoid","modality":"literature","capacity_units":4},
                    {"model_system":"organoid","modality":"spatial","capacity_units":4}
                ]
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["schedule"]["feature_id"],
        json!("GAF-GLIOMA-P07-F15")
    );
    assert_eq!(response["schedule"]["jobs"].as_array().unwrap().len(), 2);
    assert!(
        !response["schedule"]["rounds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        response["schedule"]["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|job| job["job_id"].is_string() && job["pending_action_order"].is_array())
    );
}

#[test]
fn glioma_evidence_gate_holds_uncertain_claims_and_admits_qualified_work() {
    let mut server = server();
    let hash = "0".repeat(64);
    let director = json!({
        "intent": {
            "research_id": "gated-research",
            "study_id": "gated-study",
            "objective": "identify reproducible invasion mechanisms in organoids",
            "output_uses": ["cohort_analysis"],
            "model_systems": ["organoid"],
            "modalities": ["transcriptomics"],
            "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
            "requested_autonomy": "a1",
            "approval_reference": null,
            "budget_units": 80,
            "max_retries": 1,
            "allow_instrument_execution": false,
            "allow_federation": false,
            "raw_data_local": true,
            "aggregate_only": true,
            "replay_identity": hash,
            "boundary": PRECLINICAL_BOUNDARY
        },
        "focus": "mechanism_first",
        "completed_checkpoints": [],
        "budget_units": 80,
        "max_actions": 2,
        "approval_granted": false,
        "allow_instrument_execution": false,
        "allow_federation": false,
        "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
        "max_retries": 1,
        "require_artifacts": true
    });
    let triangulated = call(
        &mut server,
        "glioma_evidence_triangulate",
        json!({
            "request": {"objective":"triangulate invasion evidence","min_source_kinds":3,"min_independent_artifacts":3,"min_support_milli":600,"max_contradiction_milli":200,"min_diversity_milli":1000,"max_leave_one_artifact_shift_milli":100,"max_claims":8},
            "records": [
                {"evidence_id":"gate-e1","source_artifact":{"artifact_id":"gate-a1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"literature","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1},
                {"evidence_id":"gate-e2","source_artifact":{"artifact_id":"gate-a2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"assay","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1},
                {"evidence_id":"gate-e3","source_artifact":{"artifact_id":"gate-a3","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"replication","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}
            ]
        }),
    );
    let contradiction_cut = call(
        &mut server,
        "glioma_evidence_contradiction_cut",
        json!({
            "request": {"objective":"route contradictory invasion evidence","min_confidence_milli":100,"max_audits":4,"budget_units":3,"require_independent_replication":true},
            "evidence": [
                {"evidence_id":"cut-support","claim_id":"invasion-claim","polarity":"support","source_family":"assay","independence_group":"site-a","confidence_milli":800,"audit_cost_units":1},
                {"evidence_id":"cut-contradict","claim_id":"invasion-claim","polarity":"contradict","source_family":"replication","independence_group":"site-b","confidence_milli":800,"audit_cost_units":1}
            ]
        }),
    );
    assert_eq!(contradiction_cut["dispatch"], json!("not_started"));
    assert_eq!(contradiction_cut["cut"]["disposition"], json!("covered"));
    assert_eq!(
        contradiction_cut["cut"]["conflicts"][0]["covered_by_audit"],
        json!(true)
    );
    let admitted = call(
        &mut server,
        "glioma_evidence_gated_research_execute",
        json!({"request":{"director":director,"triangulation":triangulated["triangulation"].clone(),"min_qualified_claims":1,"require_global_qualification":true}}),
    );
    assert_eq!(admitted["simulation_only"], json!(true));
    assert!(admitted["gated_research"]["director"].is_object());
    assert!(matches!(
        admitted["gated_research"]["disposition"].as_str(),
        Some("research_executed") | Some("research_planned") | Some("research_blocked")
    ));

    let mut uncertain = triangulated["triangulation"].clone();
    uncertain["disposition"] = json!("partial");
    // The digest is deliberately left unchanged: the gate must reject a tampered or stale
    // triangulation rather than executing from an unverified verdict.
    let held = call(
        &mut server,
        "glioma_evidence_gated_research_execute",
        json!({"request":{"director":director,"triangulation":uncertain,"min_qualified_claims":1,"require_global_qualification":true}}),
    );
    assert!(held["error"].is_string());
}

#[test]
fn glioma_autonomous_research_engine_replans_the_full_stage_graph() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_research_engine_execute",
        json!({
            "request": {
                "mission_id": "engine-protocol",
                "intent": {
                    "research_id": "engine-research",
                    "study_id": "engine-study",
                    "objective": "identify reproducible invasion mechanisms in organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "mechanism_first",
                "completed_checkpoints": [],
                "budget_units": 160,
                "max_actions": 2,
                "max_cycles": 8,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert!(response["engine"]["cycles"].as_array().unwrap().len() > 1);
    assert!(
        !response["engine"]["completed_checkpoints"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        response["engine"]["negative_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("synthetic-dry-run"))
    );
}

#[test]
fn glioma_autonomous_research_engine_evaluation_is_held_out_and_non_executing() {
    let mut server = server();
    let hash = "0".repeat(64);
    let evaluation = call(
        &mut server,
        "glioma_autonomous_research_engine_evaluate",
        json!({
            "request": {
                "mission_id": "engine-evaluation",
                "intent": {
                    "research_id": "engine-evaluation-research",
                    "study_id": "engine-evaluation-study",
                    "objective": "identify reproducible invasion mechanisms in organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "mechanism_first",
                "completed_checkpoints": [],
                "budget_units": 160,
                "max_actions": 2,
                "max_cycles": 8,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            },
            "held_out_utility_milli": {"intent-normalization":10,"multimodal-ingestion-qc":40,"molecular-landscape":90}
        }),
    );
    assert_eq!(evaluation["evaluation_only"], json!(true));
    assert_eq!(evaluation["dispatch"], json!("not_started"));
    assert_eq!(
        evaluation["evaluation"]["metrics"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(evaluation["evaluation"]["oracle_utility_milli"], json!(50));
}

#[test]
fn glioma_autonomous_research_engine_stress_evaluation_is_bounded_and_non_executing() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_research_engine_stress_evaluate",
        json!({
            "request": {
                "mission_id": "engine-stress-evaluation",
                "intent": {
                    "research_id": "engine-stress-research",
                    "study_id": "engine-stress-study",
                    "objective": "identify reproducible invasion mechanisms in organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "mechanism_first",
                "completed_checkpoints": [],
                "budget_units": 160,
                "max_actions": 2,
                "max_cycles": 8,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            },
            "held_out_scenarios": {
                "baseline": {"intent-normalization": 10, "multimodal-ingestion-qc": 40, "molecular-landscape": 90},
                "stress-negative": {"intent-normalization": -10, "multimodal-ingestion-qc": -40, "molecular-landscape": -90}
            }
        }),
    );
    assert_eq!(response["evaluation_only"], json!(true));
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(
        response["evaluation"]["scenarios"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        response["evaluation"]["metrics"].as_array().unwrap().len(),
        4
    );
    assert!(response["evaluation"]["negative_evidence"].is_array());
}

#[test]
fn glioma_autonomous_research_engine_trace_evaluation_replays_without_dispatch() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_research_engine_trace_evaluate",
        json!({
            "request": {
                "mission_id": "engine-trace-evaluation",
                "intent": {
                    "research_id": "engine-trace-research",
                    "study_id": "engine-trace-study",
                    "objective": "identify reproducible invasion mechanisms in organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "adaptive",
                "completed_checkpoints": [],
                "budget_units": 160,
                "max_actions": 2,
                "max_cycles": 8,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            },
            "outcome_traces": {
                "negative-world": {
                    "intent-normalization": {"disposition": "negative", "retryable": false}
                }
            }
        }),
    );
    assert_eq!(response["evaluation_only"], json!(true));
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(
        response["evaluation"]["metrics"].as_array().unwrap().len(),
        7
    );
    assert_eq!(
        response["evaluation"]["scenarios"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert!(response["evaluation"]["negative_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("negative-result-retained")));
}

#[test]
fn glioma_stage_worker_routes_compile_blocks_uncovered_stages_without_dispatch() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_stage_worker_routes_compile",
        json!({
            "request": {
                "intent": {
                    "research_id": "worker-route-research",
                    "study_id": "worker-route-study",
                    "objective": "identify reproducible invasion mechanisms in glioma organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "workers": [{
                    "worker_id": "intent-worker",
                    "capability_version": "1",
                    "stage_kinds": ["intent_normalization"],
                    "modalities": [],
                    "model_systems": [],
                    "output_schemas": ["GliomaIntent1@1"],
                    "max_autonomy": "a1",
                    "local_only": true,
                    "available": true,
                    "deterministic": true,
                    "priority": 10
                }],
                "require_deterministic": true,
                "require_all_ready": false
            }
        }),
    );
    assert_eq!(response["evaluation_only"], json!(true));
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(
        response["route_plan"]["output_schema"],
        json!("GliomaStageWorkerRoute1@1")
    );
    assert!(response["route_plan"]["selected_order"]
        .as_array()
        .unwrap()
        .iter()
        .any(|stage| stage == "intent-normalization"));
    assert!(response["route_plan"]["blocked_order"]
        .as_array()
        .unwrap()
        .iter()
        .any(|stage| stage == "mechanism-exploration"));
}

#[test]
fn glioma_autonomous_stage_engine_executes_through_routed_synthetic_worker() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_research_engine_stage_execute",
        json!({
            "request": {
                "mission_id": "stage-engine-rehearsal",
                "intent": {
                    "research_id": "stage-engine-research",
                    "study_id": "stage-engine-study",
                    "objective": "identify reproducible invasion mechanisms in glioma organoids",
                    "output_uses": ["cohort_analysis", "method_development"],
                    "model_systems": ["organoid", "in_silico"],
                    "modalities": ["literature", "genomics", "transcriptomics", "imaging", "spatial", "computational"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 64,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "adaptive",
                "completed_checkpoints": [],
                "budget_units": 64,
                "max_actions": 1,
                "max_cycles": 1,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            },
            "workers": [{
                "worker_id": "intent-worker",
                "capability_version": "1",
                "stage_kinds": ["intent_normalization"],
                "modalities": [],
                "model_systems": [],
                "output_schemas": ["GliomaIntent1@1"],
                "max_autonomy": "a1",
                "local_only": true,
                "available": true,
                "deterministic": true,
                "priority": 10
            }],
            "require_deterministic": true,
            "require_all_ready": false
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["execution"]["output_schema"],
        json!("GliomaAutonomousResearchStageExecution1@1")
    );
    assert_eq!(
        response["execution"]["engine"]["cycles"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(response["execution"]["engine"]["completed_checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .any(|checkpoint| checkpoint["stage_kind"] == "intent_normalization"));
    assert!(response["execution"]["engine"]["negative_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item
            .as_str()
            .unwrap()
            .contains("synthetic-dry-run-not-biological-evidence")));
}

#[test]
fn glioma_evidence_gated_stage_engine_holds_before_routing_on_unresolved_evidence() {
    let mut server = server();
    let hash = "0".repeat(64);
    let triangulated = call(
        &mut server,
        "glioma_evidence_triangulate",
        json!({
            "request": {"objective":"triangulate invasion evidence","min_source_kinds":3,"min_independent_artifacts":3,"min_support_milli":600,"max_contradiction_milli":200,"min_diversity_milli":1000,"max_leave_one_artifact_shift_milli":100,"max_claims":8},
            "records": [
                {"evidence_id":"hold-e1","source_artifact":{"artifact_id":"hold-a1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"literature","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"unknown","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1},
                {"evidence_id":"hold-e2","source_artifact":{"artifact_id":"hold-a2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"assay","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"unknown","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1},
                {"evidence_id":"hold-e3","source_artifact":{"artifact_id":"hold-a3","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"replication","claim":"EGFR signaling increases organoid invasion","scope":"organoid:invasion","modality":"functional_perturbation","model_system":"organoid","state":"unknown","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}
            ]
        }),
    );
    let response = call(
        &mut server,
        "glioma_evidence_gated_stage_engine_execute",
        json!({
            "request": {
                "engine": {
                    "mission_id":"evidence-gated-stage-mission",
                    "intent": {
                        "research_id":"evidence-gated-stage-research","study_id":"evidence-gated-stage-study",
                        "objective":"identify reproducible invasion mechanisms in glioma organoids",
                        "output_uses":["cohort_analysis","method_development"],"model_systems":["organoid","in_silico"],"modalities":["literature","transcriptomics","imaging","computational"],
                        "input_artifacts":[{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"requested_autonomy":"a1","approval_reference":null,"budget_units":80,"max_retries":1,"allow_instrument_execution":false,"allow_federation":false,"raw_data_local":true,"aggregate_only":true,"replay_identity":hash,"boundary":PRECLINICAL_BOUNDARY
                    },
                    "focus":"adaptive","completed_checkpoints":[],"budget_units":80,"max_actions":2,"max_cycles":1,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,
                    "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},"max_retries":1,"require_artifacts":true
                },
                "triangulation":triangulated["triangulation"],"min_qualified_claims":1,"require_global_qualification":true,
                "workers":[{"worker_id":"intent-worker","capability_version":"1","stage_kinds":["intent_normalization"],"modalities":[],"model_systems":[],"output_schemas":["GliomaIntent1@1"],"max_autonomy":"a1","local_only":true,"available":true,"deterministic":true,"priority":10}],
                "require_deterministic":true
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["execution"]["output_schema"],
        json!("GliomaEvidenceGatedStageExecution1@1")
    );
    assert_eq!(response["execution"]["disposition"], json!("evidence_hold"));
    assert!(response["execution"]["route_plan"].is_null());
}

#[test]
fn glioma_autonomous_research_engine_invokes_configured_institution_worker() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut server = server().with_glioma_action_executor(RejectingInstitutionWorker {
        calls: calls.clone(),
    });
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_research_engine_execute",
        json!({
            "request": {
                "mission_id": "engine-institution-worker",
                "intent": {
                    "research_id": "worker-research",
                    "study_id": "worker-study",
                    "objective": "identify reproducible invasion mechanisms in glioma organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["transcriptomics", "imaging", "spatial"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 160,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "focus": "mechanism_first",
                "completed_checkpoints": [],
                "budget_units": 160,
                "max_actions": 2,
                "max_cycles": 8,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("institution_local"));
    assert_eq!(response["simulation_only"], json!(false));
    assert!(calls.load(Ordering::SeqCst) > 0);
    assert_eq!(response["engine"]["stop_reason"], json!("executor_failed"));
    assert!(response["engine"]["negative_evidence"].is_array());
}

#[test]
fn glioma_intent_mission_compiles_and_executes_the_full_stage_action_graph() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_intent_mission_execute",
        json!({
            "request": {
                "intent": {
                    "research_id": "intent-mission-research",
                    "study_id": "intent-mission-study",
                    "objective": "map reproducible invasion mechanisms in glioma organoids",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid"],
                    "modalities": ["literature", "genomics", "computational", "replication", "organoid_assay"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 512,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "mission_id": "intent-mission",
                "selection": {"budget_units":512,"max_actions":14,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}},
                "gates": {"required_stages":["mechanism_exploration"],"min_completed_actions":1,"min_information_gain_milli":500,"max_uncertainty_milli":10000,"min_model_systems":1,"min_modalities":1},
                "max_rounds":14,
                "max_retries":1,
                "require_artifacts":true,
                "stop_on_negative":false,
                "recovery_budget_units":128,
                "recovery_max_rounds":4,
                "require_clean_recovery":false
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["campaign"]["disposition"], json!("executed"));
    assert_eq!(
        response["campaign"]["plan"]["disposition"],
        json!("admitted")
    );
    assert!(
        response["campaign"]["candidate_order"]
            .as_array()
            .unwrap()
            .len()
            >= 10
    );
    assert!(response["campaign"]["campaign"].is_object());
}

#[test]
fn glioma_autonomous_program_cycle_exposes_stage_gates_and_handoff() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_autonomous_program_cycle",
        json!({
            "request": {
                "engine": {
                    "mission_id": "program-cycle-protocol",
                    "intent": {
                        "research_id": "program-cycle-research",
                        "study_id": "program-cycle-study",
                        "objective": "identify reproducible invasion mechanisms in organoids",
                        "output_uses": ["cohort_analysis"],
                        "model_systems": ["organoid"],
                        "modalities": ["transcriptomics", "imaging", "spatial"],
                        "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                        "requested_autonomy": "a1",
                        "approval_reference": null,
                        "budget_units": 160,
                        "max_retries": 1,
                        "allow_instrument_execution": false,
                        "allow_federation": false,
                        "raw_data_local": true,
                        "aggregate_only": true,
                        "replay_identity": hash,
                        "boundary": PRECLINICAL_BOUNDARY
                    },
                    "focus": "mechanism_first",
                    "completed_checkpoints": [],
                    "budget_units": 160,
                    "max_actions": 2,
                    "max_cycles": 8,
                    "approval_granted": false,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5},
                    "max_retries": 1,
                    "require_artifacts": true
                },
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["cycle"]["gates"].as_array().unwrap().len(), 14);
    assert!(response["cycle"]["progress_milli"].as_u64().unwrap() > 0);
    assert!(response["cycle"]["next_operator_action"].is_string());
}

#[test]
fn glioma_adaptive_workflow_plans_dependency_closed_batch() {
    let mut server = server();
    let response = call(
        &mut server,
        "glioma_adaptive_workflow",
        json!({
            "request": {
                "mission_id": "adaptive-scheduler-protocol",
                "objective": "identify reproducible invasion mechanisms in organoids",
                "candidates": [
                    {"action_id":"pre","stage_kind":"multimodal_ingestion_qc","modality":"spatial","model_system":"organoid","depends_on":[],"cost_units":2,"information_gain_milli":500,"frontier_novelty_milli":500,"workflow_leverage_milli":500,"cross_stage_unlock_milli":500,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":800,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]},
                    {"action_id":"child","stage_kind":"mechanism_exploration","modality":"computational","model_system":"organoid","depends_on":["pre"],"cost_units":2,"information_gain_milli":950,"frontier_novelty_milli":950,"workflow_leverage_milli":950,"cross_stage_unlock_milli":950,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":800,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}
                ],
                "completed_action_order": [],
                "observations": [{"action_id":"pre","outcome":"qualified","information_gain_milli":500,"uncertainty_milli":200,"round":1}],
                "budget_units": 4,
                "max_actions": 2,
                "beam_width": 16,
                "risk_budget_milli": 2000,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["plan"]["selected_order"], json!(["pre", "child"]));
    assert!(response["plan"]["negative_evidence"].is_array());
}

#[test]
fn glioma_adaptive_workflow_prefers_cross_stage_portfolio_when_utilities_are_near_tied() {
    let mut server = server();
    let response = call(
        &mut server,
        "glioma_adaptive_workflow",
        json!({
            "request": {
                "mission_id": "adaptive-scheduler-stage-coverage",
                "objective": "separate invasion mechanisms with complementary preclinical workflows",
                "candidates": [
                    {"action_id":"mechanism-primary","stage_kind":"mechanism_exploration","modality":"spatial","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":700,"frontier_novelty_milli":700,"workflow_leverage_milli":700,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":800,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]},
                    {"action_id":"mechanism-repeat","stage_kind":"mechanism_exploration","modality":"spatial","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":699,"frontier_novelty_milli":699,"workflow_leverage_milli":699,"cross_stage_unlock_milli":699,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":800,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]},
                    {"action_id":"design-orthogonal","stage_kind":"experiment_design","modality":"transcriptomics","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":698,"frontier_novelty_milli":698,"workflow_leverage_milli":698,"cross_stage_unlock_milli":698,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":800,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}
                ],
                "completed_action_order": [],
                "observations": [],
                "budget_units": 2,
                "max_actions": 2,
                "beam_width": 16,
                "risk_budget_milli": 2000,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["plan"]["selected_order"],
        json!(["design-orthogonal", "mechanism-primary"])
    );
}

#[test]
fn glioma_adaptive_frontier_turns_interpretation_debt_into_next_actions() {
    let mut server = server();
    let hash = "0".repeat(64);
    let synthesis_response = call(
        &mut server,
        "glioma_interpretation_synthesize",
        json!({
            "request": {
                "objective": "identify reproducible invasion mechanisms",
                "hypothesis": "a preclinical invasion mechanism is reproducible",
                "model_system": "organoid",
                "min_evidence": 2,
                "min_independent_groups": 2,
                "min_families": 3,
                "min_quality_milli": 700,
                "effect_threshold_milli": 100,
                "max_disagreement_milli": 100,
                "max_leave_one_out_shift_milli": 50,
                "require_replication_family": true,
                "replay_identity": hash,
                "evidence": [
                    {"evidence_id":"causal-a","family":"causal_contrast","independent_group":"site-a","model_system":"organoid","direction":"positive","effect_milli":300,"uncertainty_milli":50,"quality_milli":900,"sample_count":6,"artifact":{"artifact_id":"causal-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                    {"evidence_id":"sensitivity-b","family":"sensitivity","independent_group":"site-b","model_system":"organoid","direction":"positive","effect_milli":280,"uncertainty_milli":60,"quality_milli":850,"sample_count":8,"artifact":{"artifact_id":"sensitivity-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]}
                ]
            }
        }),
    );
    let response = call(
        &mut server,
        "glioma_adaptive_research_frontier",
        json!({
            "request": {
                "synthesis": synthesis_response["synthesis"].clone(),
                "completed_actions": [],
                "budget_units": 80,
                "max_actions": 3,
                "approval_granted": true,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["frontier"]["disposition"], json!("partial"));
    assert!(
        !response["frontier"]["next_action_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        response["frontier"]["uncertainty"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .unwrap()
                .contains("replication-family-required"))
    );
}

#[test]
fn glioma_adaptive_frontier_execute_runs_selected_actions_and_replays() {
    let mut server = server();
    let hash = "0".repeat(64);
    let synthesis_response = call(
        &mut server,
        "glioma_interpretation_synthesize",
        json!({
            "request": {
                "objective": "execute reproducible invasion followups",
                "hypothesis": "a preclinical invasion mechanism is reproducible",
                "model_system": "organoid",
                "min_evidence": 2,
                "min_independent_groups": 2,
                "min_families": 2,
                "min_quality_milli": 700,
                "effect_threshold_milli": 100,
                "max_disagreement_milli": 700,
                "max_leave_one_out_shift_milli": 700,
                "require_replication_family": true,
                "replay_identity": hash,
                "evidence": [
                    {"evidence_id":"causal-a","family":"causal_contrast","independent_group":"site-a","model_system":"organoid","direction":"positive","effect_milli":300,"uncertainty_milli":50,"quality_milli":900,"sample_count":6,"artifact":{"artifact_id":"causal-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                    {"evidence_id":"replication-b","family":"replication","independent_group":"site-b","model_system":"organoid","direction":"positive","effect_milli":280,"uncertainty_milli":60,"quality_milli":850,"sample_count":8,"artifact":{"artifact_id":"replication-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]}
                ]
            }
        }),
    );
    let request = json!({
        "frontier": {
            "synthesis": synthesis_response["synthesis"].clone(),
            "completed_actions": [],
            "budget_units": 80,
            "max_actions": 3,
            "approval_granted": true,
            "allow_instrument_execution": false,
            "allow_federation": false,
            "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
        },
        "max_retries": 1,
        "require_artifacts": true,
        "allow_unresolved_dispatch": false
    });
    let first = call(
        &mut server,
        "glioma_adaptive_frontier_execute",
        json!({"request": request}),
    );
    let second = call(
        &mut server,
        "glioma_adaptive_frontier_execute",
        json!({"request": request}),
    );
    assert_eq!(first["dispatch"], json!("dry_run"));
    assert_eq!(first["simulation_only"], json!(true));
    assert_eq!(first["execution"]["disposition"], json!("executed"));
    assert!(
        !first["execution"]["dispatched_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(first["execution"]["digest"], second["execution"]["digest"]);
}

#[test]
fn glioma_adaptive_interpretation_campaign_executes_and_stops_without_fabricating_reanalysis() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_adaptive_interpretation_campaign_execute",
        json!({
            "request": {
                "initial_synthesis": {
                    "objective": "run adaptive invasion interpretation",
                    "hypothesis": "a preclinical invasion mechanism is reproducible",
                    "model_system": "organoid",
                    "min_evidence": 2,
                    "min_independent_groups": 2,
                    "min_families": 2,
                    "min_quality_milli": 700,
                    "effect_threshold_milli": 100,
                    "max_disagreement_milli": 700,
                    "max_leave_one_out_shift_milli": 700,
                    "require_replication_family": true,
                    "replay_identity": hash,
                    "evidence": [
                        {"evidence_id":"causal-a","family":"causal_contrast","independent_group":"site-a","model_system":"organoid","direction":"positive","effect_milli":300,"uncertainty_milli":50,"quality_milli":900,"sample_count":6,"artifact":{"artifact_id":"causal-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                        {"evidence_id":"replication-b","family":"replication","independent_group":"site-b","model_system":"organoid","direction":"positive","effect_milli":280,"uncertainty_milli":60,"quality_milli":850,"sample_count":8,"artifact":{"artifact_id":"replication-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]}
                    ]
                },
                "completed_actions": [],
                "budget_units": 80,
                "max_rounds": 3,
                "max_actions_per_round": 3,
                "max_retries": 1,
                "require_artifacts": true,
                "allow_unresolved_dispatch": false,
                "stop_on_qualified": false,
                "stop_on_negative": true,
                "approval_granted": true,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights": {"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["campaign"]["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(
        response["campaign"]["stop_reason"],
        json!("planner_no_progress")
    );
    assert_eq!(
        response["campaign"]["rounds"][0]["execution"]["disposition"],
        json!("executed")
    );
    assert!(
        response["campaign"]["negative_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("synthetic-dry-run"))
    );
}

#[test]
fn glioma_multimodal_mechanism_campaign_closes_analysis_to_action() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign_input = json!({
        "request": {
            "objective": "find an executable invasion follow-up",
            "study_id": "campaign-study",
            "model_system": "organoid",
            "graph": {
                "study_id": "campaign-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "transcriptomics"],
                "min_samples": 3,
                "min_modalities_per_sample": 2,
                "min_shared_features": 2,
                "neighbours": 2,
                "diffusion_steps": 2,
                "max_distance_milli": 1000,
                "min_consensus_support_milli": 500,
                "max_disagreement_milli": 200,
                "require_all_modalities": false
            },
            "pathway": {
                "objective": "rank invasion pathways",
                "study_id": "campaign-study",
                "model_system": "organoid",
                "min_pathway_nodes": 2,
                "min_observed_nodes": 2,
                "min_modalities": 2,
                "min_confidence_milli": 700,
                "max_pathways": 4,
                "require_cross_modal": true,
                "min_edge_agreement_milli": 700,
                "require_edge_consistency": false
            },
            "selection": {"budget_units": 3, "max_actions": 1},
            "completed_action_order": []
        },
        "graph_vectors": [
            {"observation_id":"a-g","study_id":"campaign-study","sample_lineage":"a","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"a-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":700},{"feature_id":"y","value_milli":700}]},
            {"observation_id":"a-t","study_id":"campaign-study","sample_lineage":"a","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"a-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":700},{"feature_id":"y","value_milli":700}]},
            {"observation_id":"b-g","study_id":"campaign-study","sample_lineage":"b","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"b-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":710},{"feature_id":"y","value_milli":690}]},
            {"observation_id":"b-t","study_id":"campaign-study","sample_lineage":"b","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"b-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":710},{"feature_id":"y","value_milli":690}]},
            {"observation_id":"c-g","study_id":"campaign-study","sample_lineage":"c","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"c-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":680},{"feature_id":"y","value_milli":720}]},
            {"observation_id":"c-t","study_id":"campaign-study","sample_lineage":"c","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"c-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":680},{"feature_id":"y","value_milli":720}]}
        ],
        "pathway_definitions": [{"pathway_id":"invasion","label":"invasion","nodes":[{"node_id":"egfr","label":"EGFR","modality":"genomics","expected_direction":1,"weight_milli":1000},{"node_id":"vim","label":"VIM","modality":"transcriptomics","expected_direction":1,"weight_milli":1000}],"edges":[{"source_node_id":"egfr","target_node_id":"vim","relation":1,"confidence_milli":900}]}],
        "pathway_observations": [
            {"observation_id":"p-g","study_id":"campaign-study","sample_lineage":"a","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"p-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"egfr","value_milli":700,"reliability_milli":900},
            {"observation_id":"p-t","study_id":"campaign-study","sample_lineage":"a","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"p-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"vim","value_milli":800,"reliability_milli":900}
        ],
        "candidates": [{"action_id":"validate-invasion","stage_kind":"experiment_design","modality":"functional_perturbation","model_system":"organoid","depends_on":[],"cost_units":2,"information_gain_milli":900,"frontier_novelty_milli":700,"workflow_leverage_milli":900,"cross_stage_unlock_milli":800,"reproducibility_safety_milli":900,"federation_value_milli":500,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation","write_local_artifact"]}]
    });
    let response = call(
        &mut server,
        "glioma_multimodal_mechanism_campaign",
        campaign_input.clone(),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(
        response["campaign"]["disposition"],
        json!("ready_for_execution")
    );
    assert_eq!(
        response["campaign"]["next_action_order"],
        json!(["validate-invasion"])
    );

    let mut execution_input = campaign_input;
    execution_input["max_retries"] = json!(1);
    execution_input["require_artifacts"] = json!(true);
    let execution = call(
        &mut server,
        "glioma_multimodal_mechanism_campaign_execute",
        execution_input,
    );
    assert_eq!(execution["dispatch"], json!("dry_run"));
    assert_eq!(execution["simulation_only"], json!(true));
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    assert_eq!(
        execution["execution"]["executed_order"],
        json!(["validate-invasion"])
    );
    assert_eq!(
        execution["execution"]["execution"]["completed_order"],
        json!(["validate-invasion"])
    );
}

#[test]
fn glioma_experiment_frontier_controller_escalates_and_replans_from_local_observations() {
    let mut server = server();
    let response = call(
        &mut server,
        "glioma_experiment_frontier_controller_execute",
        json!({
            "request": {
                "objective": "select discriminating invasion assays",
                "model_system": "organoid",
                "mechanisms": [
                    {"mechanism_id":"invasion","prior_milli":600},
                    {"mechanism_id":"repair","prior_milli":400}
                ],
                "candidates": [
                    {
                        "candidate_id":"low-fidelity-imaging",
                        "action_family":"imaging",
                        "label":"low fidelity invasion imaging",
                        "model_system":"organoid",
                        "modality":"imaging",
                        "fidelity_level":1,
                        "clone_ids":["clone-a"],
                        "outcomes":[
                            {"outcome_id":"signal","label":"invasion signal","probability_milli_by_mechanism":{"invasion":800,"repair":200},"effect_milli":400},
                            {"outcome_id":"null","label":"null","probability_milli_by_mechanism":{"invasion":200,"repair":800},"effect_milli":-100}
                        ],
                        "prerequisites":[],"cost_units":1,"risk_milli":100,"power_milli":900,"feasibility_milli":900,"supports_replication":true
                    },
                    {
                        "candidate_id":"high-fidelity-spatial",
                        "action_family":"spatial",
                        "label":"high fidelity spatial assay",
                        "model_system":"organoid",
                        "modality":"spatial",
                        "fidelity_level":2,
                        "clone_ids":["clone-b"],
                        "outcomes":[
                            {"outcome_id":"signal","label":"invasion signal","probability_milli_by_mechanism":{"invasion":900,"repair":100},"effect_milli":500},
                            {"outcome_id":"null","label":"null","probability_milli_by_mechanism":{"invasion":100,"repair":900},"effect_milli":-100}
                        ],
                        "prerequisites":["low-fidelity-imaging"],"cost_units":2,"risk_milli":100,"power_milli":950,"feasibility_milli":700,"supports_replication":true
                    }
                ],
                "initial_observations":[],
                "budget_units":4,
                "max_rounds":3,
                "max_actions_per_round":1,
                "max_retries":1,
                "min_information_gain_milli":1,
                "min_power_milli":500,
                "risk_ceiling_milli":500,
                "require_fidelity_escalation":true,
                "information_weight_milli":500,
                "power_weight_milli":100,
                "diversity_weight_milli":100,
                "feasibility_weight_milli":100,
                "replication_weight_milli":100,
                "risk_penalty_milli":10,
                "cost_penalty_milli":5
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["frontier"]["feature_id"],
        json!("GAF-GLIOMA-P06-F08")
    );
    assert_eq!(response["frontier"]["disposition"], json!("qualified"));
    assert_eq!(
        response["frontier"]["completed_order"],
        json!(["high-fidelity-spatial", "low-fidelity-imaging"])
    );
    assert_eq!(response["frontier"]["rounds"].as_array().unwrap().len(), 2);
}

#[test]
fn glioma_autonomous_campaign_execution_is_reachable_through_mcp() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let replay_identity = "1".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_autonomous_campaign_execute",
        json!({
            "request": {
                "intent": {
                    "research_id": "mcp-autonomous-campaign",
                    "study_id": "study-001",
                    "objective": "test preclinical glioma invasion mechanism",
                    "output_uses": ["method_development"],
                    "model_systems": ["organoid"],
                    "modalities": ["genomics", "computational"],
                    "input_artifacts": [{
                        "artifact_id": "local:matrix",
                        "content_hash": artifact_hash,
                        "content_type": "application/octet-stream",
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false
                    }],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 4,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": replay_identity,
                    "boundary": "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
                },
                "initial_candidates": [{
                    "action_id": "mcp-seed",
                    "stage_kind": "experiment_design",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "depends_on": [],
                    "cost_units": 2,
                    "information_gain_milli": 800,
                    "frontier_novelty_milli": 700,
                    "workflow_leverage_milli": 700,
                    "cross_stage_unlock_milli": 700,
                    "reproducibility_safety_milli": 900,
                    "federation_value_milli": 400,
                    "feasibility_milli": 900,
                    "autonomy_tier": "a1",
                    "effects": ["read_local_data", "execute_local_computation", "write_local_artifact"]
                }],
                "selection": {
                    "budget_units": 4,
                    "max_actions": 1,
                    "approval_granted": false,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "weights": {
                        "information_gain": 25,
                        "frontier_novelty": 20,
                        "workflow_leverage": 15,
                        "cross_stage_unlock": 15,
                        "reproducibility_safety": 10,
                        "federation_value": 10,
                        "feasibility": 5
                    }
                },
                "max_rounds": 2,
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("not_started"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("completed"));
    assert_eq!(campaign["campaign"]["completed_order"], json!(["mcp-seed"]));
}

#[test]
fn glioma_protocol_autonomous_controller_executes_a_selected_branch() {
    let mut server = server();
    let protocol = json!({
        "objective": "autonomously run an organoid invasion protocol",
        "model_system": "organoid",
        "tasks": [
            {"task_id":"prepare","label":"prepare organoids","resource_kind":"culture","resource_units":1,"duration_ticks":1,"depends_on":[],"model_system":"organoid","output_schema":"Setup1@1","risk_milli":100,"requires_instrument":false},
            {"task_id":"assay","label":"run invasion assay","resource_kind":"imaging","resource_units":1,"duration_ticks":2,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":200,"requires_instrument":false}
        ],
        "resources": [
            {"resource_id":"culture","kind":"culture","capacity_units":1},
            {"resource_id":"imaging","kind":"imaging","capacity_units":1}
        ],
        "max_ticks": 10,
        "max_risk_milli": 500,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": ContentHash::of_bytes(b"autonomous-protocol-mcp")
    });
    let result = call(
        &mut server,
        "glioma_protocol_autonomous_execute",
        json!({
            "request": {
                "mission_id": "mission-autonomous-protocol-mcp",
                "objective": "autonomously run an organoid invasion protocol",
                "base_protocol": protocol,
                "branch_candidates": [{
                    "candidate_id":"fast-assay",
                    "task":{"task_id":"assay","label":"fast assay","resource_kind":"imaging","resource_units":1,"duration_ticks":1,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":200,"requires_instrument":false},
                    "expected_information_milli":850,
                    "evidence_prior_milli":800,
                    "cost_units":2
                }],
                "compensation_candidates": [{
                    "candidate_id":"assay-compensation",
                    "replaces_task_id":"assay",
                    "output_schema":"Assay1@1",
                    "model_system":"organoid",
                    "resource_kind":"imaging",
                    "resource_units":1,
                    "duration_ticks":1,
                    "cost_units":1,
                    "risk_milli":200,
                    "expected_information_milli":800,
                    "depends_on":["prepare"],
                    "unlocks_task_order":["assay"]
                }],
                "budget_units":5,
                "max_rounds":3,
                "max_branches":8,
                "beam_width":8,
                "max_retries":0,
                "require_artifacts":true,
                "branch_weights":{"information_milli":400,"feasibility_milli":300,"time_milli":100,"risk_milli":100,"cost_milli":100}
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["run"]["disposition"], json!("completed"));
    assert_eq!(result["run"]["rounds"].as_array().unwrap().len(), 1);
}
