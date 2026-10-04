//! MCP contract tests for the experiments area.

use super::*;

#[test]
fn glioma_contrast_panel_design_exposes_factorial_estimands_and_gates() {
    let mut server = server();
    let design = call(
        &mut server,
        "glioma_contrast_panel_design",
        json!({
            "request": {
                "objective": "design invasion perturbation contrasts",
                "model_system": "organoid",
                "modality": "functional_perturbation",
                "factors": [
                    {"factor_id":"drug","label":"drug perturbation","level_order":["control","inhibitor"],"baseline_level":"control","perturbation_kind":"small_molecule"},
                    {"factor_id":"matrix","label":"matrix context","level_order":["control","stiff"],"baseline_level":"control","perturbation_kind":"microenvironment"}
                ],
                "replicates_per_condition": 3,
                "max_conditions": 16,
                "max_total_units": 32,
                "min_design_adequacy_milli": 700,
                "required_interaction_order": ["drug×matrix"]
            }
        }),
    );
    assert_eq!(design["dispatch"], json!("not_started"));
    assert_eq!(design["simulation_only"], json!(true));
    assert_eq!(design["design"]["disposition"], json!("qualified"));
    assert_eq!(design["design"]["conditions"].as_array().unwrap().len(), 4);
    assert_eq!(design["design"]["contrasts"].as_array().unwrap().len(), 2);
    assert!(
        design["design"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("not a formal power"))
    );
}

#[test]
fn glioma_heterogeneity_adaptive_benchmark_power_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_heterogeneity_adaptive_benchmark_power",
        json!({
            "request": {
                "objective": "size a preclinical glioma segmentation benchmark",
                "capability_id": "segmentation",
                "benchmark_world": "world-1",
                "metric_name": "dice",
                "model_system": "organoid",
                "minimum_sites": 2,
                "minimum_replicates_per_site": 3,
                "target_effect_milli": 200,
                "minimum_power_milli": 300,
                "maximum_heterogeneity_milli": 500,
                "maximum_privacy_noise_milli": 200,
                "maximum_budget_units": 200,
                "maximum_replicate_multiplier": 4,
                "maximum_surface_points": 32,
                "sites": [
                    {
                        "site": {
                            "site_id": "mcp-power-site-a",
                            "study_id": "mcp-power-study-a",
                            "capability_id": "segmentation",
                            "benchmark_world": "world-1",
                            "metric_name": "dice",
                            "model_system": "organoid",
                            "artifact": {
                                "artifact_id": "mcp-power-artifact-a",
                                "content_hash": zero,
                                "content_type": "aggregate",
                                "local_only": true,
                                "contains_human_data": false,
                                "contains_direct_identifiers": false
                            },
                            "baseline_score_milli": 500,
                            "candidate_score_milli": 700,
                            "uncertainty_milli": 20,
                            "replicate_count": 5
                        },
                        "attrition_milli": 100,
                        "modality_coverage_milli": 900,
                        "privacy_noise_milli": 50,
                        "cost_units": 10
                    },
                    {
                        "site": {
                            "site_id": "mcp-power-site-b",
                            "study_id": "mcp-power-study-b",
                            "capability_id": "segmentation",
                            "benchmark_world": "world-1",
                            "metric_name": "dice",
                            "model_system": "organoid",
                            "artifact": {
                                "artifact_id": "mcp-power-artifact-b",
                                "content_hash": zero,
                                "content_type": "aggregate",
                                "local_only": true,
                                "contains_human_data": false,
                                "contains_direct_identifiers": false
                            },
                            "baseline_score_milli": 500,
                            "candidate_score_milli": 710,
                            "uncertainty_milli": 20,
                            "replicate_count": 5
                        },
                        "attrition_milli": 100,
                        "modality_coverage_milli": 900,
                        "privacy_noise_milli": 50,
                        "cost_units": 10
                    }
                ]
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P12-F04"));
    assert_eq!(
        output["plan"]["recommendation"],
        json!("qualified_portfolio")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_active_learning_selects_an_uncertain_safe_assay() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let plan = call(
        &mut server,
        "glioma_active_learning",
        json!({
            "request": {
                "objective": "select the next invasion mechanism assay",
                "model_system": "organoid",
                "direction": "maximize",
                "budget_units": 4,
                "max_selections": 2,
                "min_observations_per_candidate": 1,
                "exploration_weight_milli": 400,
                "exploitation_weight_milli": 600,
                "cost_penalty_milli": 1,
                "risk_penalty_milli": 1,
                "max_risk_milli": 800,
                "min_uncertainty_milli": 900
            },
            "candidates": [
                {"candidate_id":"egfr","mechanism_id":"egfr-signaling","feature_vector":[100,0],"cost_units":2,"risk_milli":100,"max_replicates":3,"redundancy_group":"receptor","output_schema":"Assay1@1"},
                {"candidate_id":"matrix","mechanism_id":"matrix-remodeling","feature_vector":[0,100],"cost_units":2,"risk_milli":100,"max_replicates":3,"redundancy_group":"matrix","output_schema":"Assay1@1"},
                {"candidate_id":"unsafe","mechanism_id":"unsafe","feature_vector":[50,50],"cost_units":1,"risk_milli":900,"max_replicates":3,"redundancy_group":"unsafe","output_schema":"Assay1@1"}
            ],
            "observations": [
                {"observation_id":"obs-egfr","candidate_id":"egfr","outcome_milli":700,"uncertainty_milli":20,"artifact":{"artifact_id":"obs-egfr","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-active-learning+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(plan["plan"]["selected_order"], json!(["matrix", "egfr"]));
    assert_eq!(plan["plan"]["blocked_order"], json!(["unsafe"]));
    assert_eq!(plan["plan"]["disposition"], json!("partial"));
}

#[test]
fn glioma_posterior_batch_is_agent_callable_and_never_dispatches() {
    let mut server = server();
    let plan = call(
        &mut server,
        "glioma_posterior_batch",
        json!({
            "request": {
                "objective": "resolve organoid invasion disagreement",
                "model_system": "organoid",
                "budget_units": 1,
                "max_selections": 1,
                "max_risk_milli": 500,
                "min_marginal_reduction_milli": 1,
                "targets": [{"target_id":"invasion","weight_milli":1000}],
                "posterior_draws": [
                    {"draw_id":"draw-low","prior_weight_millionths":500000,"target_predictions_milli":[-1000],"candidate_outcome_probabilities":{"candidate-a":[1000000,0]}},
                    {"draw_id":"draw-high","prior_weight_millionths":500000,"target_predictions_milli":[1000],"candidate_outcome_probabilities":{"candidate-a":[0,1000000]}}
                ]
            },
            "candidates": [
                {"candidate_id":"candidate-a","mechanism_id":"invasion-control","output_schema":"GliomaOrganoidAssay1@1","cost_units":1,"risk_milli":100,"completed_replicates":0,"max_replicates":2,"redundancy_group":"invasion-assay"}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(
        plan["plan"]["output_schema"],
        json!("GliomaPosteriorDisagreementBatch1@2")
    );
    assert_eq!(plan["plan"]["selected_order"], json!(["candidate-a"]));
    assert!(plan["plan"]["input_digest"].is_string());
    assert!(plan["guarantees"].as_array().is_some_and(|items| {
        items.iter().any(|item| item == "the institution supplies posterior draws and calibrated candidate outcome probabilities; this route does not fit a model")
    }));
}

#[test]
fn glioma_sequential_design_exposes_stopping_and_negative_evidence() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let plan = call(
        &mut server,
        "glioma_sequential_design",
        json!({
            "request": {
                "objective": "select the next invasion assay batch",
                "model_system": "organoid",
                "endpoint": "invasion_fraction",
                "control_arm_id": "control",
                "target_effect_milli": 150,
                "success_probability_milli": 750,
                "futility_probability_milli": 700,
                "min_replicates_per_arm": 3,
                "max_new_replicates_per_round": 3,
                "max_rounds": 4,
                "max_selected_arms": 2,
                "budget_units": 20,
                "risk_ceiling_milli": 800,
                "exploration_weight_milli": 400
            },
            "observations": [
                {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"seq-control","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sequential+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":3,"failures":7,"prior_alpha":1,"prior_beta":1,"risk_milli":200,"cost_units":2},
                {"arm_id":"strong","label":"strong perturbation","artifact":{"artifact_id":"seq-strong","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sequential+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":10,"failures":0,"prior_alpha":1,"prior_beta":1,"risk_milli":200,"cost_units":2},
                {"arm_id":"weak","label":"weak perturbation","artifact":{"artifact_id":"seq-weak","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sequential+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":0,"failures":10,"prior_alpha":1,"prior_beta":1,"risk_milli":200,"cost_units":2}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(plan["simulation_only"], json!(true));
    assert!(
        plan["plan"]["success_stop_order"]
            .as_array()
            .is_some_and(|arms| arms.iter().any(|arm| arm == "strong"))
    );
    assert!(
        plan["plan"]["futility_stop_order"]
            .as_array()
            .is_some_and(|arms| arms.iter().any(|arm| arm == "weak"))
    );
    assert!(
        plan["plan"]["negative_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence
                .iter()
                .any(|item| item.as_str().is_some_and(|item| item.contains("weak"))))
    );
}

#[test]
fn glioma_power_reestimate_exposes_interim_boundaries_and_next_batch() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let plan = call(
        &mut server,
        "glioma_power_reestimate",
        json!({
            "request": {
                "objective": "re-estimate invasion assay power",
                "model_system": "organoid",
                "endpoint": "invasion_fraction",
                "control_arm_id": "control",
                "target_effect_milli": 100,
                "alpha_total_milli": 50,
                "power_target_milli": 700,
                "current_look": 1,
                "max_looks": 4,
                "min_replicates_per_arm": 4,
                "max_replicates_per_arm": 24,
                "max_new_replicates_per_arm": 8,
                "budget_units": 100,
                "risk_ceiling_milli": 300
            },
            "arms": [
                {"arm_id":"control","label":"vehicle","artifact":{"artifact_id":"power-control","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":200,"variance_milli2":100,"observations":6,"risk_milli":50,"cost_units":2},
                {"arm_id":"perturbation","label":"egfr perturbation","artifact":{"artifact_id":"power-perturbation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":310,"variance_milli2":100,"observations":6,"risk_milli":100,"cost_units":2}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(plan["simulation_only"], json!(true));
    assert_eq!(plan["plan"]["disposition"], json!("continue"));
    assert!(plan["plan"]["alpha_spent_milli"].as_u64().unwrap() < 50);
    assert_eq!(plan["plan"]["selected_order"], json!(["perturbation"]));
    assert!(
        plan["plan"]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|decision| {
                decision["arm_id"] == json!("perturbation")
                    && decision["planned_replicates"].as_u64().unwrap() > 0
            })
    );
}

#[test]
fn glioma_mechanism_dynamics_exposes_feedback_and_intervention_sensitivity() {
    let mut server = server();
    let plan = call(
        &mut server,
        "glioma_mechanism_dynamics",
        json!({
            "request": {
                "objective": "simulate hypoxia-driven invasion feedback",
                "model_system": "organoid",
                "max_steps": 12,
                "time_step_milli": 200,
                "stability_window": 3,
                "stability_delta_milli": 2,
                "divergence_abs_milli": 1000,
                "max_selected_interventions": 1,
                "budget_units": 4,
                "risk_ceiling_milli": 800,
                "sensitivity_delta_milli": 5
            },
            "nodes": [
                {"node_id":"hypoxia","label":"hypoxia state","initial_state_milli":300,"drift_milli":-30,"uncertainty_milli":20},
                {"node_id":"invasion","label":"invasion state","initial_state_milli":100,"drift_milli":-10,"uncertainty_milli":20}
            ],
            "edges": [
                {"edge_id":"hypoxia-to-invasion","source_node":"hypoxia","target_node":"invasion","weight_milli":500,"lag_steps":0,"confidence_milli":900},
                {"edge_id":"invasion-to-hypoxia","source_node":"invasion","target_node":"hypoxia","weight_milli":-100,"lag_steps":1,"confidence_milli":800}
            ],
            "interventions": [
                {"intervention_id":"oxygenation","target_node":"hypoxia","delta_milli":-250,"start_step":0,"duration_steps":6,"cost_units":2,"risk_milli":200,"confidence_milli":900}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(plan["simulation_only"], json!(true));
    assert_eq!(
        plan["plan"]["selected_intervention_order"],
        json!(["oxygenation"])
    );
    assert!(
        plan["plan"]["steps"]
            .as_array()
            .is_some_and(|steps| !steps.is_empty())
    );
    assert_eq!(plan["plan"]["sensitivities"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_heterogeneity_aware_experiment_portfolio_preserves_diversity_and_reserve() {
    let mut server = server();
    let response = call(
        &mut server,
        "glioma_heterogeneity_aware_experiment_portfolio",
        json!({
            "request": {
                "objective": "validate invasion mechanism across model systems",
                "budget_units": 30,
                "replication_reserve_fraction_milli": 200,
                "min_model_systems": 2,
                "min_power_milli": 100,
                "max_risk_milli": 700,
                "max_selected_arms": 3,
                "strata": [
                    {"stratum_id":"organoid","model_system":"organoid","prior_milli":500,"heterogeneity_milli":200,"required":true},
                    {"stratum_id":"in_silico","model_system":"in_silico","prior_milli":500,"heterogeneity_milli":500,"required":true}
                ],
                "candidates": [
                    {"candidate_id":"organoid-invasion","arm_id":"invasion","stratum_id":"organoid","modality":"imaging","independence_group":"site-a","cost_units_per_replicate":4,"max_replicates":3,"expected_effect_milli":800,"effect_uncertainty_milli":100,"reproducibility_milli":900,"risk_milli":100,"available":true},
                    {"candidate_id":"insilico-invasion","arm_id":"invasion","stratum_id":"in_silico","modality":"computational","independence_group":"compute-a","cost_units_per_replicate":2,"max_replicates":3,"expected_effect_milli":650,"effect_uncertainty_milli":120,"reproducibility_milli":950,"risk_milli":50,"available":true},
                    {"candidate_id":"unsafe-arm","arm_id":"unsafe","stratum_id":"organoid","modality":"functional_perturbation","independence_group":"site-b","cost_units_per_replicate":1,"max_replicates":1,"expected_effect_milli":900,"effect_uncertainty_milli":100,"reproducibility_milli":900,"risk_milli":900,"available":true}
                ]
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["portfolio"]["output_schema"],
        json!("GliomaHeterogeneityAwareExperimentPortfolio1@1")
    );
    assert!(
        response["portfolio"]["replication_reserve_units"]
            .as_u64()
            .unwrap_or_default()
            >= 1
    );
    assert!(response["portfolio"]["model_system_order"]
        .as_array()
        .is_some_and(|systems| systems.len() >= 2));
    assert!(response["portfolio"]["deferred"]
        .as_array()
        .is_some_and(|items| items
            .iter()
            .any(|item| item["candidate_id"] == "unsafe-arm")));
}

#[test]
fn glioma_heterogeneity_portfolio_mission_compiles_typed_action_bindings() {
    let mut server = server();
    let portfolio_response = call(
        &mut server,
        "glioma_heterogeneity_aware_experiment_portfolio",
        json!({
            "request": {
                "objective": "compile invasion workflow",
                "budget_units": 20,
                "replication_reserve_fraction_milli": 200,
                "min_model_systems": 2,
                "min_power_milli": 1,
                "max_risk_milli": 700,
                "max_selected_arms": 2,
                "strata": [
                    {"stratum_id":"organoid","model_system":"organoid","prior_milli":500,"heterogeneity_milli":200,"required":true},
                    {"stratum_id":"in_silico","model_system":"in_silico","prior_milli":500,"heterogeneity_milli":500,"required":true}
                ],
                "candidates": [
                    {"candidate_id":"organoid-invasion","arm_id":"invasion","stratum_id":"organoid","modality":"imaging","independence_group":"site-a","cost_units_per_replicate":2,"max_replicates":2,"expected_effect_milli":800,"effect_uncertainty_milli":100,"reproducibility_milli":900,"risk_milli":100,"available":true},
                    {"candidate_id":"insilico-invasion","arm_id":"invasion","stratum_id":"in_silico","modality":"computational","independence_group":"compute-a","cost_units_per_replicate":2,"max_replicates":2,"expected_effect_milli":650,"effect_uncertainty_milli":120,"reproducibility_milli":950,"risk_milli":50,"available":true}
                ]
            }
        }),
    );
    assert_eq!(
        portfolio_response["portfolio"]["output_schema"],
        json!("GliomaHeterogeneityAwareExperimentPortfolio1@1")
    );
    let mission = call(
        &mut server,
        "glioma_heterogeneity_portfolio_mission",
        json!({
            "request": {
                "mission_id": "invasion-mission",
                "objective": "compile a reproducible invasion workflow",
                "portfolio": portfolio_response["portfolio"].clone(),
                "bindings": [
                    {"portfolio_candidate_id":"organoid-invasion","action":{"action_id":"organoid-analysis","stage_kind":"computational_execution","modality":"computational","model_system":"in_silico","depends_on":[],"cost_units":1,"information_gain_milli":800,"frontier_novelty_milli":700,"workflow_leverage_milli":700,"cross_stage_unlock_milli":500,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}},
                    {"portfolio_candidate_id":"insilico-invasion","action":{"action_id":"insilico-analysis","stage_kind":"computational_execution","modality":"computational","model_system":"in_silico","depends_on":[],"cost_units":1,"information_gain_milli":800,"frontier_novelty_milli":700,"workflow_leverage_milli":700,"cross_stage_unlock_milli":500,"reproducibility_safety_milli":900,"federation_value_milli":300,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}}
                ],
                "completed_action_order": [],
                "observations": [],
                "budget_units": 4,
                "max_actions": 4,
                "beam_width": 32,
                "risk_budget_milli": 10000,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(mission["dispatch"], json!("not_started"));
    assert_eq!(mission["simulation_only"], json!(true));
    assert_eq!(
        mission["plan"]["output_schema"],
        json!("GliomaHeterogeneityPortfolioMission1@1")
    );
    assert!(mission["plan"]["scheduler"].is_object());
    assert!(mission["plan"]["selected_action_order"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
}

#[test]
fn glioma_robust_active_learning_preserves_model_disagreement() {
    let mut server = server();
    let plan = call(
        &mut server,
        "glioma_robust_active_learning",
        json!({
            "request": {
                "objective": "choose a robust organoid invasion assay",
                "model_system": "organoid",
                "direction": "maximize",
                "budget_units": 4,
                "max_selections": 2,
                "min_observations_per_candidate": 1,
                "lower_tail_weight_milli": 600,
                "disagreement_weight_milli": 300,
                "information_weight_milli": 100,
                "cost_penalty_milli": 1,
                "risk_penalty_milli": 1,
                "max_risk_milli": 800,
                "min_model_reliability_milli": 500,
                "models": [
                    {"model_id":"mechanistic","prior_weight_milli":600,"intercept_milli":100,"feature_weights":[4,1],"residual_milli":50,"reliability_milli":900},
                    {"model_id":"spatial","prior_weight_milli":400,"intercept_milli":50,"feature_weights":[1,4],"residual_milli":80,"reliability_milli":800}
                ]
            },
            "candidates": [
                {"candidate_id":"egfr","mechanism_id":"egfr","feature_vector":[100,0],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"receptor","output_schema":"Assay1@1"},
                {"candidate_id":"matrix","mechanism_id":"matrix","feature_vector":[0,100],"cost_units":2,"risk_milli":100,"max_replicates":2,"redundancy_group":"matrix","output_schema":"Assay1@1"}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert!(
        plan["plan"]["selected_order"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        plan["plan"]["scores"]
            .as_array()
            .unwrap()
            .iter()
            .all(|score| score["model_support_count"].as_u64().unwrap_or_default() == 2)
    );
}

#[test]
fn glioma_adaptive_mechanism_policy_selects_information_bearing_assay() {
    let mut server = server();
    let policy = call(
        &mut server,
        "glioma_adaptive_mechanism_policy",
        json!({
            "request": {
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
            }
        }),
    );
    assert_eq!(policy["dispatch"], json!("not_started"));
    assert_eq!(policy["simulation_only"], json!(true));
    assert_eq!(
        policy["policy"]["selected_action_order"][0],
        json!("assay-a")
    );
    assert!(
        policy["policy"]["scores"]
            .as_array()
            .unwrap()
            .iter()
            .any(|score| { score["information_gain_milli"].as_u64().unwrap_or_default() > 0 })
    );
}

#[test]
fn glioma_mechanism_calibration_exposes_prequential_gates() {
    let mut server = server();
    let artifact_hash = ContentHash::of_value(&json!({"artifact": "calibration"})).unwrap();
    let calibration = call(
        &mut server,
        "glioma_mechanism_calibrate",
        json!({
            "request": {
                "objective": "calibrate invasion mechanism probabilities",
                "model_system": "organoid",
                "min_observations_per_mechanism": 2,
                "max_mechanisms": 4,
                "max_rounds": 8,
                "max_calibration_error_milli": 200000,
                "max_brier_loss_milli": 200000
            },
            "observations": [
                {"round_index":0,"mechanism_id":"matrix","feature_id":"f1","predicted_milli":200000,"observed_milli":250000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-a1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":1,"mechanism_id":"matrix","feature_id":"f2","predicted_milli":300000,"observed_milli":350000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-a2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":0,"mechanism_id":"motility","feature_id":"f1","predicted_milli":800000,"observed_milli":750000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-a3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"round_index":1,"mechanism_id":"motility","feature_id":"f2","predicted_milli":700000,"observed_milli":650000,"uncertainty_milli":10000,"artifact":{"artifact_id":"cal-a4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(calibration["dispatch"], json!("not_started"));
    assert_eq!(
        calibration["calibration"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        calibration["calibration"]["prequential_holdout_round"],
        json!(1)
    );
    assert_eq!(
        calibration["calibration"]["scores"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn glioma_mechanism_autopilot_replans_and_retires_local_outcomes() {
    let mut server = server();
    let hash = "0".repeat(64);
    let request = json!({
        "campaign": {
            "objective": "autonomous invasion follow-up",
            "study_id": "autopilot-study",
            "model_system": "organoid",
            "graph": {
                "study_id": "autopilot-study",
                "model_system": "organoid",
                "required_modalities": ["proteomics"],
                "min_samples": 2,
                "min_modalities_per_sample": 1,
                "min_shared_features": 1,
                "neighbours": 1,
                "diffusion_steps": 1,
                "max_distance_milli": 1000,
                "min_consensus_support_milli": 500,
                "max_disagreement_milli": 200,
                "require_all_modalities": false
            },
            "pathway": {
                "objective": "rank invasion pathways",
                "study_id": "autopilot-study",
                "model_system": "organoid",
                "min_pathway_nodes": 1,
                "min_observed_nodes": 1,
                "min_modalities": 1,
                "min_confidence_milli": 700,
                "max_pathways": 4,
                "require_cross_modal": false,
                "min_edge_agreement_milli": 700,
                "require_edge_consistency": false
            },
            "selection": {"budget_units": 3, "max_actions": 1},
            "completed_action_order": []
        },
        "graph_vectors": [
            {"observation_id":"a","study_id":"autopilot-study","sample_lineage":"a","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"vimentin","value_milli":800}]},
            {"observation_id":"b","study_id":"autopilot-study","sample_lineage":"b","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"vimentin","value_milli":700}]}
        ],
        "pathway_definitions": [{"pathway_id":"invasion","label":"invasion","nodes":[{"node_id":"vimentin","label":"VIM","modality":"proteomics","expected_direction":1,"weight_milli":1000}],"edges":[]}],
        "pathway_observations": [{"observation_id":"p-a","study_id":"autopilot-study","sample_lineage":"a","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"p-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"vimentin","value_milli":800,"reliability_milli":900}],
        "candidates": [{"action_id":"assay-invasion","stage_kind":"mechanism_exploration","modality":"functional_perturbation","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":900,"frontier_novelty_milli":800,"workflow_leverage_milli":800,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":400,"feasibility_milli":950,"autonomy_tier":"a0","effects":["read_local_data","execute_local_computation","write_local_artifact"]}],
        "max_rounds": 2,
        "max_retries": 1,
        "require_artifacts": true,
        "require_ready_for_execution": false,
        "stop_on_negative": false
    });
    let response = call(
        &mut server,
        "glioma_mechanism_autopilot_execute",
        json!({"request": request}),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["autopilot"]["feature_id"],
        json!("GAF-GLIOMA-P07-F30")
    );
    assert_eq!(response["autopilot"]["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(
        response["autopilot"]["completed_action_order"],
        json!(["assay-invasion"])
    );
}

#[test]
fn glioma_mechanism_discovery_engine_composes_scientific_gates_before_execution() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_mechanism_discovery_engine_execute",
        json!({
            "request": {
                "objective": "discover invasion mechanism",
                "study_id": "discovery-study",
                "model_system": "organoid",
                "campaign": {
                    "objective": "discover invasion mechanism",
                    "study_id": "discovery-study",
                    "model_system": "organoid",
                    "graph": {"study_id":"discovery-study","model_system":"organoid","required_modalities":["proteomics"],"min_samples":2,"min_modalities_per_sample":1,"min_shared_features":1,"neighbours":1,"diffusion_steps":1,"max_distance_milli":1000,"min_consensus_support_milli":500,"max_disagreement_milli":200,"require_all_modalities":false},
                    "pathway": {"objective":"discover invasion mechanism","study_id":"discovery-study","model_system":"organoid","min_pathway_nodes":1,"min_observed_nodes":1,"min_modalities":1,"min_confidence_milli":100,"max_pathways":4,"require_cross_modal":false,"min_edge_agreement_milli":700,"require_edge_consistency":false},
                    "selection": {"budget_units":2,"max_actions":1},
                    "completed_action_order": []
                },
                "graph_vectors": [
                    {"observation_id":"v1","study_id":"discovery-study","sample_lineage":"a","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"egfr","value_milli":800}]},
                    {"observation_id":"v2","study_id":"discovery-study","sample_lineage":"b","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"egfr","value_milli":700}]}
                ],
                "pathway_definitions": [{"pathway_id":"invasion","label":"invasion","nodes":[{"node_id":"egfr","label":"EGFR","modality":"proteomics","expected_direction":1,"weight_milli":1000}],"edges":[]}],
                "pathway_observations": [{"observation_id":"p1","study_id":"discovery-study","sample_lineage":"a","modality":"proteomics","model_system":"organoid","artifact":{"artifact_id":"p","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"egfr","value_milli":800,"reliability_milli":900}],
                "dynamics": {"objective":"discover invasion mechanism","model_system":"organoid","max_steps":12,"time_step_milli":200,"stability_window":2,"stability_delta_milli":20,"divergence_abs_milli":1000,"max_selected_interventions":1,"budget_units":8,"risk_ceiling_milli":800,"sensitivity_delta_milli":5},
                "dynamics_nodes": [{"node_id":"invasion","label":"invasion","initial_state_milli":100,"drift_milli":-10,"uncertainty_milli":10}],
                "dynamics_edges": [],
                "dynamics_interventions": [{"intervention_id":"suppress-invasion","target_node":"invasion","delta_milli":-100,"start_step":0,"duration_steps":4,"cost_units":1,"risk_milli":100,"confidence_milli":900}],
                "robust": {"objective":"discover invasion mechanism","model_system":"organoid","max_iterations":64,"convergence_tolerance_milli":1,"damping_milli":600,"min_edge_confidence_milli":500,"direction":"decrease","budget_units":2,"max_selected":1,"min_robust_effect_milli":1,"min_agreement_milli":750,"risk_ceiling_milli":800,"effect_weight_milli":500,"tail_weight_milli":300,"worst_case_weight_milli":200,"feasibility_weight_milli":1000,"risk_penalty_milli":1},
                "robust_models": [{"model_id":"model-a","prior_milli":1000,"nodes":[{"node_id":"egfr","label":"EGFR","modality":"proteomics","prior_milli":0,"support_milli":900,"contradiction_milli":0},{"node_id":"invasion","label":"invasion","modality":"proteomics","prior_milli":0,"support_milli":0,"contradiction_milli":0}],"edges":[{"edge_id":"egfr-invasion","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["local-evidence"]}]}],
                "robust_candidates": [{"candidate_id":"egfr-invasion","label":"EGFR inhibition","intervention":{"intervention_id":"inhibit-egfr","node_id":"egfr","delta_milli":-600,"rationale":"test EGFR-to-invasion propagation","evidence_order":["local-evidence"]},"target_node_id":"invasion","redundancy_group":"egfr","feasibility_milli":1000,"cost_units":1,"risk_milli":100}],
                "action_candidates": [{"action_id":"assay-invasion","stage_kind":"mechanism_exploration","modality":"functional_perturbation","model_system":"organoid","depends_on":[],"cost_units":1,"information_gain_milli":900,"frontier_novelty_milli":800,"workflow_leverage_milli":800,"cross_stage_unlock_milli":700,"reproducibility_safety_milli":900,"federation_value_milli":400,"feasibility_milli":950,"autonomy_tier":"a0","effects":["read_local_data","execute_local_computation","write_local_artifact"]}],
                "selection": {"budget_units":2,"max_actions":1,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}},
                "completed_action_order": [],
                "budget_units": 2,
                "max_actions": 1,
                "max_rounds": 2,
                "max_retries": 1,
                "require_artifacts": true,
                "require_qualified_multimodal": true,
                "require_stable_dynamics": false
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["discovery"]["feature_id"],
        json!("GAF-GLIOMA-P07-F16")
    );
    assert_eq!(response["discovery"]["stop_reason"], json!("qualified"));
    assert_eq!(
        response["discovery"]["completed_action_order"],
        json!(["assay-invasion"])
    );
    assert_eq!(response["discovery"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_robust_experiment_design_allocates_lower_tail_batch() {
    let mut server = server();
    let result = call(
        &mut server,
        "glioma_robust_experiment_design",
        json!({
            "request": {
                "objective": "robustly distinguish invasion mechanisms",
                "model_system": "organoid",
                "scenarios": [
                    {"scenario_id":"mechanism-a","label":"mechanism A","weight_milli":500},
                    {"scenario_id":"mechanism-b","label":"mechanism B","weight_milli":500}
                ],
                "candidates": [
                    {"arm_id":"strong","label":"strong assay","feature_id":"assay-strong","cost_units_per_replicate":2,"risk_milli":200,"feasibility_milli":900,"max_replicates":3,"utility_milli_by_scenario":{"mechanism-a":[700,350,175],"mechanism-b":[600,300,150]}},
                    {"arm_id":"weak","label":"weak assay","feature_id":"assay-weak","cost_units_per_replicate":2,"risk_milli":200,"feasibility_milli":900,"max_replicates":3,"utility_milli_by_scenario":{"mechanism-a":[100,50,25],"mechanism-b":[50,25,12]}}
                ],
                "budget_units":8,
                "max_selected_arms":1,
                "min_replicates_per_selected_arm":2,
                "max_total_replicates":3,
                "min_feasibility_milli":700,
                "risk_ceiling_milli":500,
                "min_robust_utility_milli":400
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["design"]["selected_order"], json!(["strong"]));
    assert_eq!(result["design"]["disposition"], json!("partial"));
}
