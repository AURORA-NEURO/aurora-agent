//! MCP contract tests for the catalogue reachability area.

use super::*;

#[test]
fn glioma_multisite_benchmark_workflow_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_multisite_benchmark_workflow",
        json!({
            "request": {
                "benchmark": {
                    "objective": "route a preclinical glioma benchmark",
                    "capability_id": "glioma:invasion-model",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_auc",
                    "model_system": "organoid",
                    "minimum_sites": 1,
                    "minimum_replicates_per_site": 1,
                    "effect_threshold_milli": 1,
                    "max_i2_milli": 1000,
                    "min_signal_to_noise_milli": 1,
                    "max_site_spread_milli": 1000,
                    "max_leave_one_out_shift_milli": 1000
                },
                "sites": [{
                    "site_id": "mcp-site-a",
                    "study_id": "mcp-study-a",
                    "policy": {
                        "site_id": "mcp-site-a",
                        "policy_version": "policy-1",
                        "approval_granted": true,
                        "query_allowed": true,
                        "release_allowed": true,
                        "withdrawn": false,
                        "rationale": null
                    }
                }],
                "events": [{
                    "event_id": "mcp-validation-a",
                    "sequence": 1,
                    "site_id": "mcp-site-a",
                    "kind": {
                        "stage_succeeded": {
                            "stage": "local_validation",
                            "aggregate": null
                        }
                    }
                }],
                "budget": {
                    "max_events": 16,
                    "max_query_sites": 2,
                    "max_retry_attempts_per_stage": 2
                }
            }
        }),
    );
    assert_eq!(
        output["workflow"]["feature_id"],
        json!("GAF-GLIOMA-P12-F14")
    );
    assert_eq!(output["workflow"]["disposition"], json!("partial"));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_continual_benchmark_monitor_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_continual_benchmark_monitor",
        json!({
            "request": {
                "objective": "monitor a preclinical glioma segmentation benchmark",
                "capability_id": "segmentation",
                "benchmark_world": "glioma-world-v1",
                "metric_name": "dice",
                "model_system": "organoid",
                "minimum_sites": 3,
                "minimum_windows": 3,
                "maximum_uncertainty_milli": 100,
                "maximum_step_drift_milli": 100,
                "maximum_change_point_milli": 150,
                "maximum_epoch_gap": 2,
                "windows": [
                    {
                        "window_id": "mcp-window-1",
                        "epoch": 1,
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "dice",
                        "model_system": "organoid",
                        "aggregate_effect_milli": 500,
                        "uncertainty_milli": 20,
                        "site_count": 4,
                        "immutable_snapshot": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    },
                    {
                        "window_id": "mcp-window-2",
                        "epoch": 2,
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "dice",
                        "model_system": "organoid",
                        "aggregate_effect_milli": 510,
                        "uncertainty_milli": 20,
                        "site_count": 4,
                        "immutable_snapshot": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    },
                    {
                        "window_id": "mcp-window-3",
                        "epoch": 3,
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "dice",
                        "model_system": "organoid",
                        "aggregate_effect_milli": 800,
                        "uncertainty_milli": 20,
                        "site_count": 4,
                        "immutable_snapshot": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    }
                ]
            }
        }),
    );
    assert_eq!(
        output["assessment"]["feature_id"],
        json!("GAF-GLIOMA-P12-F15")
    );
    assert_eq!(output["assessment"]["disposition"], json!("drifted"));
    assert_eq!(output["assessment"]["rerun_required"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_action_portfolio_execution_is_reachable_through_mcp() {
    let mut server = server();
    let execution = call(
        &mut server,
        "glioma_action_portfolio_execute",
        json!({
            "request": {
                "candidates": [{
                    "action_id": "local-genomics",
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
                "completed_actions": [],
                "selection": {
                    "budget_units": 5,
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
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(execution["dispatch"], json!("not_started"));
    assert_eq!(execution["simulation_only"], json!(true));
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    assert_eq!(
        execution["execution"]["completed_order"],
        json!(["local-genomics"])
    );
}

#[test]
fn glioma_knowledge_synthesis_operating_cycle_emits_typed_p01_handoff() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_knowledge_synthesis_operating_cycle",
        json!({
            "request": {
                "knowledge": {
                    "objective": "resolve organoid invasion evidence",
                    "required_modalities": ["genomics"],
                    "required_model_systems": ["organoid"],
                    "min_support_milli": 500,
                    "min_sources_per_claim": 1,
                    "max_claims": 8
                },
                "records": [{
                    "evidence_id": "evidence-1",
                    "source_artifact": {"artifact_id":"evidence-1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "assay",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "organoid",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 850,
                    "release_epoch": 1
                }],
                "composition": {"objective":"resolve organoid invasion evidence","min_path_length":2,"max_paths":8,"min_strength_milli":0,"max_contradiction_milli":1000,"require_supported_root":false},
                "relations": [],
                "revision": {"objective":"resolve organoid invasion evidence","min_support_milli":0,"min_conflict_milli":1,"max_hypotheses":1,"beam_width":8,"allow_contested":true},
                "conflicts": [],
                "frontier": {"objective":"resolve organoid invasion evidence","max_selected_claims":4,"min_priority_milli":0,"weights":{"coverage_debt_milli":250,"contradiction_milli":250,"uncertainty_milli":200,"support_milli":150,"workflow_leverage_milli":150}},
                "gap": {"objective":"resolve organoid invasion evidence","max_claims":8,"max_candidates":8,"max_candidates_per_claim":4,"max_template_cost_units":4,"min_frontier_priority_milli":0,"templates":[{"template_id":"local-assay","source_family":"local-assay","source_kind":"assay","modality":"genomics","model_system":"organoid","cost_units":1,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":0,"local_only":true,"contains_human_data":false}]}
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["cycle"]["phase_order"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        response["next_route"],
        json!("glioma_evidence_acquisition_plan")
    );
    assert!(
        !response["cycle"]["next_action"]
            .as_str()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_interpretation_synthesis_exposes_cross_family_stability() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_interpretation_synthesize",
        json!({
            "request": {
                "objective": "decide whether invasion signal is reproducibly supported",
                "hypothesis": "integrated invasion program is activated",
                "model_system": "organoid",
                "min_evidence": 3,
                "min_independent_groups": 2,
                "min_families": 2,
                "min_quality_milli": 700,
                "effect_threshold_milli": 100,
                "max_disagreement_milli": 150,
                "max_leave_one_out_shift_milli": 200,
                "require_replication_family": true,
                "replay_identity": hash,
                "evidence": [
                    {"evidence_id":"causal-a","family":"causal_contrast","independent_group":"site-a","model_system":"organoid","direction":"positive","effect_milli":300,"uncertainty_milli":50,"quality_milli":900,"sample_count":6,"artifact":{"artifact_id":"causal-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                    {"evidence_id":"meta-b","family":"meta_analysis","independent_group":"site-b","model_system":"organoid","direction":"positive","effect_milli":280,"uncertainty_milli":60,"quality_milli":850,"sample_count":8,"artifact":{"artifact_id":"meta-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                    {"evidence_id":"replication-c","family":"replication","independent_group":"site-c","model_system":"organoid","direction":"positive","effect_milli":320,"uncertainty_milli":55,"quality_milli":900,"sample_count":7,"artifact":{"artifact_id":"replication-c","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]}
                ]
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["synthesis"]["disposition"], json!("qualified"));
    assert!(response["synthesis"]["stability_milli"].as_u64().unwrap() > 0);
    assert_eq!(
        response["synthesis"]["family_order"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn glioma_interpretation_operating_cycle_gates_and_routes_next_actions() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_interpretation_operating_cycle",
        json!({
            "request": {
                "synthesis": {
                    "objective": "decide whether invasion signal is reproducibly supported",
                    "hypothesis": "integrated invasion program is activated",
                    "model_system": "organoid",
                    "min_evidence": 3,
                    "min_independent_groups": 2,
                    "min_families": 2,
                    "min_quality_milli": 700,
                    "effect_threshold_milli": 100,
                    "max_disagreement_milli": 150,
                    "max_leave_one_out_shift_milli": 200,
                    "require_replication_family": true,
                    "replay_identity": hash,
                    "evidence": [
                        {"evidence_id":"causal-a","family":"causal_contrast","independent_group":"site-a","model_system":"organoid","direction":"positive","effect_milli":300,"uncertainty_milli":50,"quality_milli":900,"sample_count":6,"artifact":{"artifact_id":"causal-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                        {"evidence_id":"meta-b","family":"meta_analysis","independent_group":"site-b","model_system":"organoid","direction":"positive","effect_milli":280,"uncertainty_milli":60,"quality_milli":850,"sample_count":8,"artifact":{"artifact_id":"meta-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]},
                        {"evidence_id":"replication-c","family":"replication","independent_group":"site-c","model_system":"organoid","direction":"positive","effect_milli":320,"uncertainty_milli":55,"quality_milli":900,"sample_count":7,"artifact":{"artifact_id":"replication-c","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"negative_evidence":[]}
                    ]
                },
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
    assert_eq!(response["cycle"]["disposition"], json!("ready"));
    assert_eq!(
        response["cycle"]["phase_order"],
        json!([
            "interpretation_gate",
            "adaptive_frontier",
            "operator_handoff"
        ])
    );
    assert!(
        !response["cycle"]["frontier"]["next_action_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_multimodal_mission_expands_model_and_modality_coverage() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_multimodal_mission_execute",
        json!({
            "request": {
                "intent": {
                    "research_id": "multimodal-mission-research",
                    "study_id": "multimodal-mission-study",
                    "objective": "map invasion mechanisms across glioma organoid and cell-line systems",
                    "output_uses": ["cohort_analysis"],
                    "model_systems": ["organoid", "cell_line"],
                    "modalities": ["literature", "transcriptomics", "imaging", "computational", "organoid_assay", "replication"],
                    "input_artifacts": [{"artifact_id":"input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],
                    "requested_autonomy": "a1",
                    "approval_reference": null,
                    "budget_units": 2000,
                    "max_retries": 1,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "raw_data_local": true,
                    "aggregate_only": true,
                    "replay_identity": hash,
                    "boundary": PRECLINICAL_BOUNDARY
                },
                "mission_id": "multimodal-mission",
                "selection": {"budget_units":2000,"max_actions":32,"approval_granted":false,"allow_instrument_execution":false,"allow_federation":false,"weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}},
                "gates": {"required_stages":["mechanism_exploration"],"min_completed_actions":2,"min_information_gain_milli":500,"max_uncertainty_milli":20000,"min_model_systems":2,"min_modalities":3},
                "max_rounds":16,
                "max_retries":1,
                "require_artifacts":true,
                "stop_on_negative":false,
                "recovery_budget_units":512,
                "recovery_max_rounds":4,
                "require_clean_recovery":false,
                "max_variants_per_stage":8
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("dry_run"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["campaign"]["disposition"], json!("executed"));
    assert!(
        response["campaign"]["candidate_order"]
            .as_array()
            .unwrap()
            .len()
            > 14
    );
    assert!(
        response["campaign"]["modality_order"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
    assert!(
        response["campaign"]["model_system_order"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
}

#[test]
fn glioma_knowledge_composition_exposes_supported_paths_and_bottlenecks() {
    let mut server = server();
    let hash = "0".repeat(64);
    let compiled = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {
                "objective": "compose invasion mechanism evidence",
                "required_modalities": [],
                "required_model_systems": [],
                "min_support_milli": 700,
                "min_sources_per_claim": 1,
                "max_claims": 8
            },
            "records": [
                {"evidence_id":"e1","source_artifact":{"artifact_id":"e1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1},
                {"evidence_id":"e2","source_artifact":{"artifact_id":"e2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"invasion increases dissemination","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}
            ]
        }),
    );
    let knowledge = compiled["knowledge"].clone();
    let claims = knowledge["claim_order"].as_array().unwrap();
    let composed = call(
        &mut server,
        "glioma_knowledge_compose",
        json!({
            "request": {"objective":"compose invasion mechanism evidence","min_path_length":2,"max_paths":8,"min_strength_milli":500,"max_contradiction_milli":500,"require_supported_root":true},
            "knowledge": knowledge.clone(),
            "relations": [{"relation_id":"r1","from_claim_id":claims[0],"to_claim_id":claims[1],"kind":"supports","strength_milli":900}]
        }),
    );
    assert_eq!(composed["dispatch"], json!("not_started"));
    assert_eq!(composed["composition"]["disposition"], json!("qualified"));
    assert_eq!(
        composed["composition"]["selected_path_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        composed["composition"]["bottleneck_claim_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let revision = call(
        &mut server,
        "glioma_belief_revision",
        json!({
            "request": {"objective":"compose invasion mechanism evidence","min_support_milli":700,"min_conflict_milli":500,"max_hypotheses":1,"beam_width":16,"allow_contested":false},
            "knowledge": knowledge.clone(),
            "conflicts": [{"conflict_id":"knowledge-conflict","left_claim_id":claims[0],"right_claim_id":claims[1],"contradiction_milli":900,"evidence_order":["e1","e2"]}]
        }),
    );
    assert_eq!(revision["dispatch"], json!("not_started"));
    assert_eq!(revision["simulation_only"], json!(true));
    assert_eq!(
        revision["revision"]["retained_claim_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        revision["revision"]["rival_claim_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let context = call(
        &mut server,
        "glioma_decision_context",
        json!({
            "request": {"objective":"compose invasion mechanism evidence","max_actions":8,"default_cost_units":4},
            "knowledge": knowledge
        }),
    );
    let graph = call(
        &mut server,
        "glioma_decision_action_graph",
        json!({
            "request": {"objective":"compose invasion mechanism evidence","max_nodes":8,"max_waves":8,"budget_units":100,"require_qualified_composition":true},
            "context": context["context"].clone(),
            "composition": composed["composition"].clone()
        }),
    );
    assert_eq!(graph["dispatch"], json!("not_started"));
    assert_eq!(graph["graph"]["disposition"], json!("qualified"));
    assert_eq!(
        graph["graph"]["parallel_waves"].as_array().unwrap().len(),
        2
    );
    let mission = call(
        &mut server,
        "glioma_decision_mission_execute",
        json!({
            "request": {
                "mission_id": "decision-mission",
                "objective": "compose invasion mechanism evidence",
                "context": context["context"].clone(),
                "graph": graph["graph"].clone(),
                "selection": {"budget_units": 100, "max_actions": 1, "approval_granted": false, "allow_instrument_execution": false, "allow_federation": false},
                "gates": {
                    "required_stages": ["mechanism_exploration"],
                    "min_completed_actions": 1,
                    "min_information_gain_milli": 100,
                    "max_uncertainty_milli": 10000,
                    "min_model_systems": 1,
                    "min_modalities": 1
                },
                "max_rounds": 2,
                "max_retries": 1,
                "require_artifacts": true,
                "stop_on_negative": false,
                "allow_partial_graph": false
            }
        }),
    );
    assert_eq!(mission["dispatch"], json!("dry_run"));
    assert_eq!(mission["run"]["disposition"], json!("executed"));
    assert_eq!(mission["run"]["mission"]["disposition"], json!("qualified"));
    let omission = call(
        &mut server,
        "glioma_decision_omission_certificate",
        json!({
            "request": {
                "objective": "compose invasion mechanism evidence",
                "required_claim_order": [claims[0].as_str().unwrap(), claims[1].as_str().unwrap()],
                "required_modality_order": ["genomics"],
                "required_model_system_order": ["organoid"],
                "require_dependency_closed": true,
                "max_next_actions": 8
            },
            "context": context["context"].clone(),
            "graph": graph["graph"].clone()
        }),
    );
    assert_eq!(omission["dispatch"], json!("not_started"));
    assert_eq!(omission["certificate"]["disposition"], json!("qualified"));
    assert_eq!(omission["certificate"]["completeness_milli"], json!(1000));
}

#[test]
fn glioma_causal_claim_adjudication_keeps_missing_timepoints_as_a_hold() {
    let mut server = server();
    let hash = "0".repeat(64);
    let artifact = |id: &str| {
        json!({
            "artifact_id": id,
            "content_hash": hash,
            "content_type": "application/json",
            "local_only": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false
        })
    };
    let response = call(
        &mut server,
        "glioma_causal_claim_adjudication_execute",
        json!({
            "request": {
                "objective":"test invasion mechanism claim",
                "hypothesis":"perturbing invasion reduces the invasion endpoint",
                "model_system":"organoid",
                "contrast_request":{"objective":"test invasion mechanism claim","control_arm":"control","treatment_arm":"perturbation","model_system":"organoid","intervention_timepoint":2,"min_units_per_arm":2,"effect_threshold_milli":10,"alpha_milli":1000},
                "contrast_observations":[
                    {"observation_id":"c1-baseline","unit_id":"c1","arm_id":"control","model_system":"organoid","batch_id":"batch","timepoint":1,"outcome_milli":100},
                    {"observation_id":"c1-post","unit_id":"c1","arm_id":"control","model_system":"organoid","batch_id":"batch","timepoint":2,"outcome_milli":110},
                    {"observation_id":"c2-baseline","unit_id":"c2","arm_id":"control","model_system":"organoid","batch_id":"batch","timepoint":1,"outcome_milli":90},
                    {"observation_id":"c2-post","unit_id":"c2","arm_id":"control","model_system":"organoid","batch_id":"batch","timepoint":2,"outcome_milli":100},
                    {"observation_id":"t1-baseline","unit_id":"t1","arm_id":"perturbation","model_system":"organoid","batch_id":"batch","timepoint":1,"outcome_milli":100},
                    {"observation_id":"t1-post","unit_id":"t1","arm_id":"perturbation","model_system":"organoid","batch_id":"batch","timepoint":2,"outcome_milli":160},
                    {"observation_id":"t2-baseline","unit_id":"t2","arm_id":"perturbation","model_system":"organoid","batch_id":"batch","timepoint":1,"outcome_milli":90}
                ],
                "sensitivity_request":{"objective":"test invasion mechanism claim","control_arm":"control","treatment_arm":"perturbation","model_system":"organoid","expected_direction":"positive","min_units_per_arm":2,"effect_threshold_milli":10,"max_confounder_strength_milli":100,"strength_step_milli":100,"max_leave_one_out_shift_milli":1000},
                "sensitivity_observations":[
                    {"observation_id":"s-c1","unit_id":"c1","arm_id":"control","model_system":"organoid","outcome_milli":100,"confounder_score_milli":0,"artifact":artifact("a-c1")},
                    {"observation_id":"s-c2","unit_id":"c2","arm_id":"control","model_system":"organoid","outcome_milli":90,"confounder_score_milli":0,"artifact":artifact("a-c2")},
                    {"observation_id":"s-c3","unit_id":"c3","arm_id":"control","model_system":"organoid","outcome_milli":95,"confounder_score_milli":0,"artifact":artifact("a-c3")},
                    {"observation_id":"s-t1","unit_id":"t1","arm_id":"perturbation","model_system":"organoid","outcome_milli":160,"confounder_score_milli":0,"artifact":artifact("a-t1")},
                    {"observation_id":"s-t2","unit_id":"t2","arm_id":"perturbation","model_system":"organoid","outcome_milli":150,"confounder_score_milli":0,"artifact":artifact("a-t2")},
                    {"observation_id":"s-t3","unit_id":"t3","arm_id":"perturbation","model_system":"organoid","outcome_milli":155,"confounder_score_milli":0,"artifact":artifact("a-t3")}
                ],
                "replication_request":{"objective":"test invasion mechanism claim","model_system":"organoid","min_sites":2,"min_replicates_per_site":3,"effect_threshold_milli":10,"heterogeneity_tolerance_milli":100},
                "meta_request":{"objective":"test invasion mechanism claim","model_system":"organoid","min_studies":2,"min_replicates_per_study":3,"effect_threshold_milli":10,"max_i2_milli":1000,"min_signal_to_noise_milli":1,"max_leave_one_out_shift_milli":1000},
                "studies":[
                    {"study_id":"study-a","site_id":"site-a","model_system":"organoid","artifact":artifact("study-a"),"effect_milli":50,"uncertainty_milli":20,"replicate_count":3},
                    {"study_id":"study-b","site_id":"site-b","model_system":"organoid","artifact":artifact("study-b"),"effect_milli":55,"uncertainty_milli":20,"replicate_count":3}
                ],
                "min_robust_strength_milli":100,
                "max_i2_milli":1000,
                "min_claim_confidence_milli":500,
                "max_actions":8
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("local_analysis"));
    assert_eq!(response["simulation_only"], json!(false));
    assert_eq!(
        response["adjudication"]["feature_id"],
        json!("GAF-GLIOMA-P10-F07")
    );
    assert_eq!(response["adjudication"]["disposition"], json!("unresolved"));
    assert_eq!(
        response["adjudication"]["gates"][0]["disposition"],
        json!("hold")
    );
    assert!(
        response["adjudication"]["action_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "collect_timepoints")
    );
}

#[test]
fn glioma_evidence_calibration_preserves_negative_and_unknown_outcomes() {
    let mut server = server();
    let artifact_hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_evidence_calibrate",
        json!({
            "request": {
                "objective": "calibrate source families before mechanistic planning",
                "bin_count": 4,
                "min_observations_per_family": 1,
                "min_resolved_per_bin": 1,
                "max_expected_calibration_error_milli": 800
            },
            "observations": [
                {
                    "observation_id": "cal-low",
                    "source_family": "assay",
                    "predicted_support_milli": 100,
                    "outcome": "supported",
                    "quality_milli": 900,
                    "independent_group": "replicate-a",
                    "artifact": {"artifact_id":"cal-artifact-low","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                },
                {
                    "observation_id": "cal-high",
                    "source_family": "assay",
                    "predicted_support_milli": 900,
                    "outcome": "contradicted",
                    "quality_milli": 900,
                    "independent_group": "replicate-b",
                    "artifact": {"artifact_id":"cal-artifact-high","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                },
                {
                    "observation_id": "cal-unknown",
                    "source_family": "literature",
                    "predicted_support_milli": 700,
                    "outcome": "unknown",
                    "quality_milli": 900,
                    "independent_group": "review-a",
                    "artifact": {"artifact_id":"cal-artifact-unknown","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-calibration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                }
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert!(
        response["calibration"]["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "cal-high")
    );
    assert!(
        response["calibration"]["unknown_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "cal-unknown")
    );
    assert_eq!(
        response["calibration"]["families"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn glioma_protocol_compensation_is_reachable_and_planning_only() {
    let mut server = server();
    let seed = ContentHash::of_bytes(b"compensation-mcp");
    let protocol = json!({
        "objective": "execute a preclinical glioma assay in a sandbox",
        "model_system": "organoid",
        "tasks": [
            {"task_id":"prepare","label":"prepare organoid controls","resource_kind":"culture","resource_units":1,"duration_ticks":2,"depends_on":[],"model_system":"organoid","output_schema":"Setup1@1","risk_milli":10,"requires_instrument":false},
            {"task_id":"assay","label":"run invasion assay","resource_kind":"culture","resource_units":1,"duration_ticks":3,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":20,"requires_instrument":false}
        ],
        "resources": [{"resource_id":"culture","kind":"culture","capacity_units":1}],
        "max_ticks": 20,
        "max_risk_milli": 100,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": seed
    });
    let execution = call(
        &mut server,
        "glioma_protocol_execute",
        json!({"request": {"protocol": protocol.clone(), "max_retries": 1, "require_artifacts": true}}),
    );
    assert_eq!(execution["execution"]["disposition"], json!("completed"));

    let compensation = call(
        &mut server,
        "glioma_protocol_compensation",
        json!({
            "request": {
                "objective": "execute a preclinical glioma assay in a sandbox",
                "protocol": protocol,
                "execution": execution["execution"].clone(),
                "candidates": [{
                    "candidate_id": "assay-retry-local",
                    "replaces_task_id": "assay",
                    "output_schema": "Assay1@1",
                    "model_system": "organoid",
                    "resource_kind": "culture",
                    "resource_units": 1,
                    "duration_ticks": 3,
                    "cost_units": 2,
                    "risk_milli": 20,
                    "expected_information_milli": 900,
                    "depends_on": ["prepare"],
                    "unlocks_task_order": ["assay"]
                }],
                "budget_units": 5,
                "max_selected": 1
            }
        }),
    );
    assert_eq!(compensation["dispatch"], json!("not_started"));
    assert_eq!(compensation["simulation_only"], json!(true));
    assert_eq!(compensation["plan"]["disposition"], json!("qualified"));
    assert_eq!(compensation["plan"]["blocked_task_order"], json!([]));
}

#[test]
fn glioma_protocol_branch_optimizer_is_reachable_and_simulation_bound() {
    let mut server = server();
    let protocol = json!({
        "objective": "choose an organoid invasion branch",
        "model_system": "organoid",
        "tasks": [
            {"task_id":"prepare","label":"prepare organoids","resource_kind":"culture","resource_units":1,"duration_ticks":1,"depends_on":[],"model_system":"organoid","output_schema":"Setup1@1","risk_milli":100,"requires_instrument":false},
            {"task_id":"assay","label":"run invasion assay","resource_kind":"imaging","resource_units":1,"duration_ticks":3,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":200,"requires_instrument":false}
        ],
        "resources": [
            {"resource_id":"culture","kind":"culture","capacity_units":1},
            {"resource_id":"imaging","kind":"imaging","capacity_units":1}
        ],
        "max_ticks": 10,
        "max_risk_milli": 500,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": ContentHash::of_bytes(b"branch-mcp")
    });
    let result = call(
        &mut server,
        "glioma_protocol_branch_optimize",
        json!({
            "request": {
                "objective": "choose an organoid invasion branch",
                "base_protocol": protocol,
                "candidates": [
                    {"candidate_id":"slow-high-information","task":{"task_id":"assay","label":"slow assay","resource_kind":"imaging","resource_units":1,"duration_ticks":10,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":200,"requires_instrument":false},"expected_information_milli":950,"evidence_prior_milli":800,"cost_units":2},
                    {"candidate_id":"fast-moderate-information","task":{"task_id":"assay","label":"fast assay","resource_kind":"imaging","resource_units":1,"duration_ticks":2,"depends_on":["prepare"],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":200,"requires_instrument":false},"expected_information_milli":700,"evidence_prior_milli":800,"cost_units":2}
                ],
                "budget_units": 8,
                "max_branches": 8,
                "beam_width": 8,
                "weights": {"information_milli":400,"feasibility_milli":300,"time_milli":100,"risk_milli":100,"cost_milli":100}
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["plan"]["disposition"], json!("qualified"));
    assert_eq!(
        result["plan"]["selected_candidate_order"],
        json!(["fast-moderate-information"])
    );
}

#[test]
fn glioma_protocol_evidence_surface_compiles_quality_gated_endpoint() {
    let mut server = server();
    let protocol = json!({
        "objective": "compile organoid invasion evidence",
        "model_system": "organoid",
        "tasks": [{"task_id":"assay","label":"run invasion assay","resource_kind":"imaging","resource_units":1,"duration_ticks":2,"depends_on":[],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":100,"requires_instrument":false}],
        "resources": [{"resource_id":"imaging","kind":"imaging","capacity_units":1}],
        "max_ticks": 10,
        "max_risk_milli": 500,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": ContentHash::of_bytes(b"evidence-surface-mcp")
    });
    let execution = call(
        &mut server,
        "glioma_protocol_execute",
        json!({"request":{"protocol":protocol.clone(),"max_retries":0,"require_artifacts":true}}),
    );
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    let result = call(
        &mut server,
        "glioma_protocol_evidence_surface",
        json!({
            "request": {
                "objective": "compile organoid invasion evidence",
                "protocol": protocol,
                "execution": execution["execution"].clone(),
                "measurements": [
                    {"measurement_id":"m1","task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":400,"uncertainty_milli":50,"quality_milli":900,"replicate_index":1},
                    {"measurement_id":"m2","task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":420,"uncertainty_milli":50,"quality_milli":900,"replicate_index":2}
                ],
                "min_replicates":2,
                "min_quality_milli":700,
                "max_uncertainty_milli":200,
                "contradiction_threshold_milli":100
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["surface"]["disposition"], json!("qualified"));
    assert_eq!(
        result["surface"]["qualified_endpoint_order"],
        json!(["invasion"])
    );
}

#[test]
fn glioma_protocol_multistudy_fusion_fuses_local_surfaces() {
    let mut server = server();
    let protocol = json!({
        "objective": "fuse invasion evidence",
        "model_system": "organoid",
        "tasks": [{"task_id":"assay","label":"run invasion assay","resource_kind":"imaging","resource_units":1,"duration_ticks":2,"depends_on":[],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":100,"requires_instrument":false}],
        "resources": [{"resource_id":"imaging","kind":"imaging","capacity_units":1}],
        "max_ticks": 10,
        "max_risk_milli": 500,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": ContentHash::of_bytes(b"multistudy-fusion-mcp")
    });
    let execution = call(
        &mut server,
        "glioma_protocol_execute",
        json!({"request":{"protocol":protocol.clone(),"max_retries":0,"require_artifacts":true}}),
    );
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    let surface = |server: &mut Server, first: i32, second: i32| {
        call(
            server,
            "glioma_protocol_evidence_surface",
            json!({
                "request": {
                    "objective": "fuse invasion evidence",
                    "protocol": protocol.clone(),
                    "execution": execution["execution"].clone(),
                    "measurements": [
                        {"measurement_id":format!("m-{first}"),"task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":first,"uncertainty_milli":50,"quality_milli":900,"replicate_index":1},
                        {"measurement_id":format!("m-{second}"),"task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":second,"uncertainty_milli":50,"quality_milli":900,"replicate_index":2}
                    ],
                    "min_replicates":2,
                    "min_quality_milli":700,
                    "max_uncertainty_milli":200,
                    "contradiction_threshold_milli":100
                }
            }),
        )
    };
    let surface_a = surface(&mut server, 400, 420);
    let surface_b = surface(&mut server, 430, 440);
    let result = call(
        &mut server,
        "glioma_protocol_multistudy_fusion",
        json!({
            "request": {
                "objective": "fuse invasion evidence",
                "studies": [
                    {"study_id":"study-a","site_id":"site-a","model_system":"organoid","surface":surface_a["surface"].clone()},
                    {"study_id":"study-b","site_id":"site-b","model_system":"mouse_model","surface":surface_b["surface"].clone()}
                ],
                "min_studies":2,
                "min_quality_milli":700,
                "max_heterogeneity_milli":100,
                "contradiction_threshold_milli":100
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["fusion"]["disposition"], json!("qualified"));
    assert_eq!(
        result["fusion"]["qualified_endpoint_order"],
        json!(["invasion"])
    );
}

#[test]
fn glioma_partition_resilient_context_checkpoint_is_reachable_through_mcp() {
    let mut server = server();
    let anchor = ContentHash::of_bytes(b"anchor").to_string();
    let value = json!({"kind":"text","value":"same objective"});
    let value_digest = ContentHash::of_value(&json!({"field":"objective","value":value.clone()}))
        .unwrap()
        .to_string();
    let delta = |site_id: &str| {
        let signer = ContentHash::of_bytes(format!("signer-{site_id}").as_bytes()).to_string();
        let fields = json!([{"field":"objective","value":value.clone(),"value_digest":value_digest.clone()}]);
        let body = json!({"site_id":site_id,"study_id":"checkpoint-study","epoch":4,"parent_checkpoint_digest":anchor.clone(),"observed_tick":20,"signer_digest":signer.clone(),"fields":fields.clone(),"local_only":true,"contains_human_data":false,"contains_direct_identifiers":false});
        json!({"site_id":site_id,"study_id":"checkpoint-study","epoch":4,"parent_checkpoint_digest":anchor,"observed_tick":20,"signer_digest":signer,"fields":fields,"local_only":true,"contains_human_data":false,"contains_direct_identifiers":false,"delta_digest":ContentHash::of_value(&body).unwrap().to_string()})
    };
    let output = call(
        &mut server,
        "glioma_partition_resilient_context_checkpoint",
        json!({"request":{"objective":"reconcile a glioma context after a partition","study_id":"checkpoint-study","expected_epoch":4,"checkpoint_anchor_digest":anchor,"minimum_sites":2,"max_sites":4,"max_fields_per_delta":8,"current_tick":20,"max_staleness_ticks":5,"network_state":"reconnected","conflict_policy":"preserve_conflicts","deltas":[delta("site-a"),delta("site-b")]}}),
    );
    assert_eq!(
        output["checkpoint"]["feature_id"],
        json!("GAF-GLIOMA-P04-F30")
    );
    assert_eq!(output["checkpoint"]["disposition"], json!("converged"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_qualification_preservation_audit_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_qualification_preservation_audit",
        json!({
            "request": {
                "research_object_digest": zero,
                "qualifications": [{
                    "qualification_id": "negative",
                    "kind": "negative_evidence",
                    "source_digest": zero,
                    "release_digest": zero,
                    "source_present": true,
                    "release_present": true,
                    "source_strength_milli": 800,
                    "release_strength_milli": 800,
                    "lineage_bound": true,
                    "required": true
                }],
                "claims": [{
                    "claim_id": "claim-a",
                    "claim_strength_milli": 700,
                    "evidence_strength_milli": 800,
                    "qualification_order": ["negative"]
                }],
                "required_kind_order": ["negative_evidence"]
            }
        }),
    );
    assert_eq!(output["audit"]["feature_id"], json!("GAF-GLIOMA-P11-F04"));
    assert_eq!(output["audit"]["disposition"], json!("complete"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_artifact_integrity_scan_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let candidate = |id: &str| {
        json!({
            "artifact_id": id,
            "relative_path": format!("{id}.json"),
            "declared_content_hash": zero,
            "observed_content_hash": zero,
            "declared_bytes": 12,
            "observed_bytes": 12,
            "content_type": "application/json",
            "metadata_valid": true,
            "executable_payload": false,
            "link_target": null,
            "requested_export": true
        })
    };
    let output = call(
        &mut server,
        "glioma_artifact_integrity_scan",
        json!({
            "request": {
                "candidate_manifest_digest": zero,
                "release_root": "release",
                "required_artifact_order": ["a", "b"],
                "allowed_content_types": ["application/json"],
                "candidates": [candidate("a"), candidate("b")],
                "max_total_bytes": 1024,
                "max_memory_bytes": 64,
                "stream_chunk_bytes": 32
            }
        }),
    );
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P11-F11"));
    assert_eq!(output["report"]["disposition"], json!("clear"));
    assert_eq!(output["report"]["verified_order"], json!(["a", "b"]));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_replay_fidelity_gate_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let manifest = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"fidelity-mcp-research","study_id":"fidelity-mcp-study","objective":"clean-room replay of a glioma mechanism result","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05"],"artifacts":[{"artifact_id":"root","content_hash":zero,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null"],"limitations":["preclinical"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let shareability = call(
        &mut server,
        "glioma_release_shareability_check",
        json!({"request":{"candidate_manifest_digest":zero,"root_order":["root"],"dependencies":[{"artifact_id":"root","dependency_order":[],"license_id":"MIT","fields":[{"field_id":"summary","classification":"aggregate_result","requested_export":true,"source_digest":zero}],"local_only":false,"embargo_until_epoch":null,"rights_confirmed":true,"contains_human_data":false,"intended_audience":"consortium"}],"policy":{"allowed_license_order":["MIT"],"forbidden_license_order":[],"audience":"consortium","now_epoch":20260923,"permit_aggregate_export":true,"permit_local_only_export":true,"permit_human_data":false}}}),
    );
    let bundle = call(
        &mut server,
        "glioma_reproducibility_bundle_compile",
        json!({"request":{"manifest":manifest.clone(),"shareability":shareability["decision"],"members":[{"artifact_id":"root","relative_path":"root.json","content_hash":zero,"content_type":"application/json","dependency_order":[],"export_permitted":true,"local_only":false}],"workflow_digest":zero,"environment_digest":zero,"replay_instruction_order":["run-workflow"],"target_profile":"clean-room","max_members":8}}),
    );
    let report = call(
        &mut server,
        "glioma_replay_fidelity_execute",
        json!({"request":{"candidate":manifest.clone(),"bundle":bundle["bundle"].clone(),"tasks":[{"task_id":"mechanism","artifact_id":"root","expected_content_hash":zero,"expected_lineage_digest":zero,"expected_metrics":[],"expected_uncertainty_order":[],"expected_negative_evidence_order":[],"cost_units":1,"required":true,"depends_on":[]}],"reference_environment_digest":zero,"replay_environment_digest":zero,"resource_cap_ticks":100,"max_retries":1,"min_required_coverage_milli":1000}}),
    );
    assert_eq!(report["dispatch"], json!("dry_run"));
    assert_eq!(report["report"]["feature_id"], json!("GAF-GLIOMA-P11-F27"));
    assert_eq!(report["report"]["disposition"], json!("pass"));
}

#[test]
fn glioma_aggregate_phenotype_summary_compile_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_aggregate_phenotype_summary_compile",
        json!({"request": {
            "site_id": "site-mcp",
            "local_dictionary_digest": zero,
            "fields": [{
                "field_id": "invasion-field",
                "local_concept": "invasion",
                "target_concept": "invasion",
                "value": "0.5",
                "unit": "score",
                "mapping": "exact",
                "source_digest": zero,
                "suppressed": false,
                "source_count": 4,
                "uncertainty_milli": 200
            }],
            "policy": {
                "schema_version": "phenotype-1",
                "estimand": "mean-invasion-score",
                "required_concept_order": ["invasion"],
                "min_source_count": 2,
                "max_uncertainty_milli": 500,
                "allow_comparable_pool": false
            }
        }}),
    );
    assert_eq!(output["summary"]["feature_id"], json!("GAF-GLIOMA-P12-F06"));
    assert_eq!(output["summary"]["disposition"], json!("comparable"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_benchmark_director_snapshot_is_reachable_through_mcp() {
    let mut server = server();
    let run = |run_id: &str, status: &str| {
        json!({
            "run_id": run_id,
            "objective": "compare invasion phenotypes",
            "status": status,
            "required_sites": 3,
            "admitted_sites": 3,
            "completed_sites": 3,
            "requested_budget_units": 100,
            "spent_budget_units": 20,
            "privacy_budget_milli": 1000,
            "privacy_spent_milli": 100,
            "workload_units": 10,
            "anomaly_count": 0,
            "uncertainty_milli": 300,
            "scientific_success_observed": false,
            "release_ready": false,
            "signed_run": false,
            "approval_complete": true,
            "freshness_tick": 95,
            "deadline_tick": 200
        })
    };
    let output = call(
        &mut server,
        "glioma_benchmark_director_snapshot",
        json!({
            "request": {
                "objective": "compare invasion phenotypes",
                "current_tick": 100,
                "total_budget_units": 1000,
                "total_privacy_budget_milli": 10000,
                "max_reallocation_units": 200,
                "minimum_quorum_sites": 2,
                "runs": [run("run-a", "completed")]
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P12-F19")
    );
    assert_eq!(
        output["snapshot"]["completion_without_success_order"],
        json!(["run-a"])
    );
    assert_eq!(output["snapshot"]["dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_benchmark_job_execute_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let event = |event_id: &str, sequence: u64, site_id: Option<&str>, kind: Value| json!({"event_id":event_id,"sequence":sequence,"site_id":site_id,"kind":kind});
    let output = call(
        &mut server,
        "glioma_benchmark_job_execute",
        json!({
            "request": {
                "job_id":"job-mcp",
                "objective":"compare invasion phenotypes",
                "idempotency_key":"mcp-idempotency",
                "site_order":[
                    {"site_id":"site-a","initially_approved":false,"revoked":false},
                    {"site_id":"site-b","initially_approved":false,"revoked":false}
                ],
                "quorum_sites":2,
                "max_query_sites":2,
                "max_retry_attempts":2,
                "max_events":32,
                "budget_units":100,
                "privacy_budget_milli":100,
                "events":[
                    event("create",1,None,json!("created")),
                    event("approve-a",2,Some("site-a"),json!("approval_granted")),
                    event("approve-b",3,Some("site-b"),json!("approval_granted")),
                    event("start-a",4,Some("site-a"),json!("query_started")),
                    event("start-b",5,Some("site-b"),json!("query_started")),
                    event("done-a",6,Some("site-a"),json!({"query_completed":{"result_digest":zero,"budget_cost_units":10,"privacy_cost_milli":10}})),
                    event("done-b",7,Some("site-b"),json!({"query_completed":{"result_digest":zero,"budget_cost_units":10,"privacy_cost_milli":10}}))
                ]
            }
        }),
    );
    assert_eq!(output["job"]["feature_id"], json!("GAF-GLIOMA-P12-F23"));
    assert_eq!(output["job"]["disposition"], json!("completed"));
    assert_eq!(output["job"]["quorum_satisfied"], json!(true));
    assert_eq!(output["job"]["dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_quorum_admission_assess_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let contribution = |id: &str, site: &str, group: &str, model: &str| {
        json!({
            "contribution_id": id,
            "site_id": site,
            "study_id": format!("study-{site}"),
            "independence_group": group,
            "signer_id": format!("signer-{site}"),
            "signature_digest": zero,
            "policy_scope": "glioma-benchmark-v1",
            "schema_version": "aggregate-v1",
            "benchmark_world": "invasion-world",
            "metric_name": "invasion_score",
            "model_system": model,
            "assay": "organoid-imaging",
            "artifact_digest": zero,
            "observed_tick": 95,
            "aggregate_count": 12,
            "privacy_cost_milli": 50,
            "approved": true,
            "revoked": false,
            "signature_valid": true,
            "aggregate_only": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false
        })
    };
    let output = call(
        &mut server,
        "glioma_quorum_admission_assess",
        json!({
            "request": {
                "benchmark_id": "benchmark-mcp",
                "benchmark_world": "invasion-world",
                "schema_version": "aggregate-v1",
                "metric_name": "invasion_score",
                "required_policy_scope": "glioma-benchmark-v1",
                "required_assay": "organoid-imaging",
                "required_model_order": ["organoid", "xenograft"],
                "current_tick": 100,
                "max_staleness_ticks": 10,
                "minimum_independent_sites": 2,
                "privacy_budget_milli": 500,
                "require_signatures": true,
                "contributions": [
                    contribution("a", "site-a", "group-a", "organoid"),
                    contribution("b", "site-b", "group-b", "xenograft")
                ]
            }
        }),
    );
    assert_eq!(
        output["decision"]["feature_id"],
        json!("GAF-GLIOMA-P12-F27")
    );
    assert_eq!(output["decision"]["disposition"], json!("admit"));
    assert_eq!(output["decision"]["query_admission_permitted"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_contribution_integrity_verify_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let contribution = |id: &str, site: &str| {
        json!({
            "contribution_id": id,
            "site_id": site,
            "study_id": format!("study-{site}"),
            "benchmark_id": "benchmark-mcp",
            "policy_scope": "glioma-v1",
            "schema_version": "aggregate-v1",
            "signer_id": format!("signer-{site}"),
            "signature_digest": zero,
            "signature_valid": true,
            "approved": true,
            "revoked": false,
            "observed_tick": 95,
            "artifact_digest": zero,
            "aggregate_digest": zero,
            "aggregate_only": true,
            "raw_data_local": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false
        })
    };
    let output = call(
        &mut server,
        "glioma_contribution_integrity_verify",
        json!({
            "request": {
                "benchmark_id": "benchmark-mcp",
                "required_policy_scope": "glioma-v1",
                "required_schema_version": "aggregate-v1",
                "current_tick": 100,
                "max_staleness_ticks": 10,
                "require_signatures": true,
                "contributions": [contribution("a", "site-a")]
            }
        }),
    );
    assert_eq!(
        output["integrity"]["feature_id"],
        json!("GAF-GLIOMA-P12-F25")
    );
    assert_eq!(output["integrity"]["status"], json!("verified"));
    assert_eq!(
        output["integrity"]["aggregate_consumption_permitted"],
        json!(true)
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_participant_exchange_execute_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_participant_exchange_execute",
        json!({
            "request": {
                "api_version": "participant-api/1.0",
                "exchange_id": "exchange-mcp",
                "idempotency_key": "idem-mcp",
                "site_id": "site-a",
                "action": "submit_contribution",
                "policy_scope": "glioma-v1",
                "required_policy_scope": "glioma-v1",
                "local_approval": true,
                "revoked": false,
                "capability_manifest_digest": zero,
                "proposal_digest": zero,
                "contribution_digest": zero,
                "receipt_digest": zero,
                "replay_of_exchange_digest": null
            }
        }),
    );
    assert_eq!(
        output["exchange"]["feature_id"],
        json!("GAF-GLIOMA-P12-F21")
    );
    assert_eq!(output["exchange"]["status"], json!("accepted"));
    assert!(output["exchange"]["receipt"].is_object());
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_signed_aggregate_submit_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_signed_aggregate_submit",
        json!({
            "request": {
                "api_version": "aggregate-result-api/1.0",
                "submission_id": "submission-mcp",
                "idempotency_key": "idem-mcp",
                "site_id": "site-a",
                "benchmark_id": "benchmark-mcp",
                "required_benchmark_id": "benchmark-mcp",
                "schema_version": "aggregate-v1",
                "required_schema_version": "aggregate-v1",
                "policy_scope": "glioma-v1",
                "required_policy_scope": "glioma-v1",
                "aggregate_digest": zero,
                "signature_digest": zero,
                "signer_id": "signer-a",
                "signature_valid": true,
                "calibration_digest": zero,
                "provenance_digest": zero,
                "local_approval": true,
                "revoked": false,
                "aggregate_only": true,
                "raw_data_local": true,
                "contains_human_data": false,
                "contains_direct_identifiers": false,
                "privacy_cost_milli": 25,
                "privacy_budget_remaining_milli": 100,
                "replay_of_submission_digest": null
            }
        }),
    );
    assert_eq!(
        output["contribution"]["feature_id"],
        json!("GAF-GLIOMA-P12-F22")
    );
    assert_eq!(output["contribution"]["status"], json!("accepted"));
    assert_eq!(
        output["contribution"]["aggregate_consumption_permitted"],
        json!(true)
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_site_provenance_attest_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let one = "1".repeat(64);
    let output = call(
        &mut server,
        "glioma_site_provenance_attest",
        json!({
            "request": {
                "site_id": "site-a",
                "contribution_id": "contribution-mcp",
                "benchmark_id": "benchmark-mcp",
                "aggregate_digest": zero,
                "source_digest_order": [zero, one],
                "lineage_digest": "2".repeat(64),
                "analysis_version": "analysis-2026.1",
                "calibration_digest": "3".repeat(64),
                "policy_scope": "glioma-federation-v1",
                "policy_decision": "approved",
                "environment_lock_digest": "4".repeat(64),
                "signer": {
                    "authority_id": "site-a-authority",
                    "key_id": "site-a-key-1",
                    "algorithm": "institution-signature-seam-v1",
                    "active": true,
                    "revoked": false,
                    "chain_valid": true,
                    "chain_order": ["site-a-authority", "consortium-root"],
                    "revocation_epoch": null
                },
                "issued_at_epoch": 100,
                "now_epoch": 105,
                "valid_until_epoch": 110,
                "aggregate_only": true,
                "raw_data_local": true,
                "contains_human_data": false,
                "contains_direct_identifiers": false
            }
        }),
    );
    assert_eq!(
        output["attestation"]["feature_id"],
        json!("GAF-GLIOMA-P12-F07")
    );
    assert_eq!(output["attestation"]["status"], json!("signed"));
    assert_eq!(output["attestation"]["freshness_valid"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_benchmark_governance_cycle_compile_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_benchmark_governance_cycle_compile",
        json!({
            "request": {
                "proposal_id": "proposal-mcp",
                "benchmark_id": "benchmark-mcp",
                "policy_version": "policy-1",
                "proposal_digest": "0".repeat(64),
                "current_epoch": 100,
                "required_quorum": 1,
                "transitions": [{
                    "transition_id": "proposal-to-review",
                    "from_stage": "proposal",
                    "to_stage": "site_review",
                    "actor_id": "board-mcp",
                    "actor_role": "governance-chair",
                    "site_id": null,
                    "policy_version": "policy-1",
                    "authorized": true,
                    "decision": "advance",
                    "rationale_digest": "1".repeat(64),
                    "epoch": 100
                }],
                "votes": [{
                    "vote_id": "vote-mcp",
                    "site_id": "site-a",
                    "actor_id": "pi-a",
                    "policy_version": "policy-1",
                    "authorized": true,
                    "decision": "approve",
                    "rationale_digest": "2".repeat(64),
                    "epoch": 100
                }]
            }
        }),
    );
    assert_eq!(output["cycle"]["feature_id"], json!("GAF-GLIOMA-P12-F16"));
    assert_eq!(output["cycle"]["status"], json!("in_progress"));
    assert_eq!(output["cycle"]["current_stage"], json!("site_review"));
    assert_eq!(output["dispatch"], json!("not_started"));
}
