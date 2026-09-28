//! MCP contract tests for the biological models area.

use super::*;

#[test]
fn glioma_temporal_multimodal_mechanism_fusion_exposes_next_measurement_frontier() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_temporal_multimodal_mechanism_fusion",
        json!({
            "request": {
                "objective": "rank invasion-state mechanisms",
                "model_system": "organoid",
                "mechanisms": [
                    {"mechanism_id":"integrin","statement":"integrin-dependent invasion state","prior_milli":500,"predictions":[
                        {"feature_id":"invasion","modality":"transcriptomics","timepoint":1,"expected_milli":100,"uncertainty_milli":50},
                        {"feature_id":"stress","modality":"transcriptomics","timepoint":1,"expected_milli":200,"uncertainty_milli":50}
                    ]},
                    {"mechanism_id":"hypoxia","statement":"hypoxia-driven invasion state","prior_milli":500,"predictions":[
                        {"feature_id":"invasion","modality":"transcriptomics","timepoint":1,"expected_milli":900,"uncertainty_milli":50},
                        {"feature_id":"stress","modality":"transcriptomics","timepoint":1,"expected_milli":800,"uncertainty_milli":50}
                    ]}
                ],
                "observations": [{"feature_id":"invasion","modality":"transcriptomics","timepoint":1,"observed_milli":110,"measurement_uncertainty_milli":50,"qc_milli":950,"source_id":"source-invasion","independence_group":"site-a","artifact":{"artifact_id":"fusion-input","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}],
                "max_next_actions": 4,
                "min_support_milli": 500,
                "contradiction_threshold_milli": 600
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("analysis_only"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["fusion"]["output_schema"],
        json!("GliomaTemporalMultimodalMechanismFusion1@1")
    );
    assert_eq!(response["fusion"]["disposition"], json!("partial"));
    assert_eq!(
        response["fusion"]["acquisitions"][0]["feature_id"],
        json!("stress")
    );
}

#[test]
fn glioma_cross_model_claim_envelope_keeps_transportability_qualified_and_auditable() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_cross_model_claim_envelope",
        json!({
            "request": {
                "objective": "validate invasion mechanism across model systems",
                "claim_id": "invasion-claim",
                "min_model_systems": 2,
                "min_studies_per_system": 2,
                "practical_effect_milli": 50,
                "hidden_bias_budget_milli": 10,
                "max_between_system_range_milli": 200,
                "estimates": [
                    {"estimate_id":"o1","study_id":"study-o1","model_system":"organoid","independent_group":"group-o1","interval_low_milli":200,"interval_high_milli":400,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"o2","study_id":"study-o2","model_system":"organoid","independent_group":"group-o2","interval_low_milli":220,"interval_high_milli":420,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i1","study_id":"study-i1","model_system":"in_silico","independent_group":"group-i1","interval_low_milli":180,"interval_high_milli":360,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i2","study_id":"study-i2","model_system":"in_silico","independent_group":"group-i2","interval_low_milli":190,"interval_high_milli":370,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ]
            }
        }),
    );
    assert_eq!(response["dispatch"], json!("analysis_only"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(
        response["envelope"]["output_schema"],
        json!("GliomaCrossModelClaimEnvelope1@1")
    );
    assert_eq!(response["envelope"]["disposition"], json!("qualified"));
    assert_eq!(response["envelope"]["sign_stable"], json!(true));
    assert!(response["envelope"]["next_action_order"]
        .as_array()
        .is_some_and(|actions| actions.is_empty()));
}

#[test]
fn glioma_cross_model_replication_frontier_selects_a_bounded_diverse_follow_up() {
    let mut server = server();
    let hash = "0".repeat(64);
    let envelope = call(
        &mut server,
        "glioma_cross_model_claim_envelope",
        json!({
            "request": {
                "objective": "plan invasion claim follow-up",
                "claim_id": "invasion-follow-up",
                "min_model_systems": 2,
                "min_studies_per_system": 2,
                "practical_effect_milli": 50,
                "hidden_bias_budget_milli": 10,
                "max_between_system_range_milli": 200,
                "estimates": [
                    {"estimate_id":"o1","study_id":"study-o1","model_system":"organoid","independent_group":"group-o1","interval_low_milli":200,"interval_high_milli":400,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"o2","study_id":"study-o2","model_system":"organoid","independent_group":"group-o2","interval_low_milli":220,"interval_high_milli":420,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i1","study_id":"study-i1","model_system":"in_silico","independent_group":"group-i1","interval_low_milli":180,"interval_high_milli":360,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i2","study_id":"study-i2","model_system":"in_silico","independent_group":"group-i2","interval_low_milli":190,"interval_high_milli":370,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ]
            }
        }),
    );
    let frontier = call(
        &mut server,
        "glioma_cross_model_replication_frontier",
        json!({
            "request": {
                "source": envelope["envelope"].clone(),
                "candidates": [
                    {"action_id":"replicate-organoid","kind":"independent_replication","model_system":"organoid","independent_group":"site-b","rationale":"test independent organoid reproducibility","expected_information_milli":700,"expected_range_reduction_milli":20,"reproducibility_milli":900,"feasibility_milli":850,"risk_milli":100,"cost_units":2,"depends_on":[]},
                    {"action_id":"acquire-mouse","kind":"acquire_missing_model_system","model_system":"mouse_model","independent_group":"site-c","rationale":"test a distinct in vivo model system","expected_information_milli":700,"expected_range_reduction_milli":120,"reproducibility_milli":850,"feasibility_milli":700,"risk_milli":200,"cost_units":2,"depends_on":[]},
                    {"action_id":"resolve-insilico","kind":"resolve_heterogeneity","model_system":"in_silico","independent_group":"compute-b","rationale":"stress the computational stratum","expected_information_milli":650,"expected_range_reduction_milli":60,"reproducibility_milli":900,"feasibility_milli":900,"risk_milli":50,"cost_units":1,"depends_on":[]}
                ],
                "budget_units":4,
                "max_actions":2,
                "max_risk_milli":500,
                "min_information_milli":600,
                "target_between_system_range_milli":40
            }
        }),
    );
    assert_eq!(frontier["dispatch"], json!("planning_only"));
    assert_eq!(frontier["simulation_only"], json!(true));
    assert_eq!(
        frontier["frontier"]["output_schema"],
        json!("GliomaCrossModelReplicationFrontier1@1")
    );
    assert!(!frontier["frontier"]["selected_order"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        frontier["frontier"]["selected_order"]
            .as_array()
            .unwrap()
            .len()
            <= 2
    );
    assert_eq!(
        frontier["action_candidates"][0]["stage_kind"],
        json!("replication_robustness")
    );
    assert_eq!(
        frontier["action_candidates"][0]["autonomy_tier"],
        json!("a1")
    );
}

#[test]
fn glioma_cross_model_replication_mission_bridges_frontier_into_p07_scheduler() {
    let mut server = server();
    let hash = "0".repeat(64);
    let envelope = call(
        &mut server,
        "glioma_cross_model_claim_envelope",
        json!({
            "request": {
                "objective": "plan invasion claim follow-up",
                "claim_id": "invasion-mission-follow-up",
                "min_model_systems": 2,
                "min_studies_per_system": 2,
                "practical_effect_milli": 50,
                "hidden_bias_budget_milli": 10,
                "max_between_system_range_milli": 200,
                "estimates": [
                    {"estimate_id":"o1","study_id":"study-o1","model_system":"organoid","independent_group":"group-o1","interval_low_milli":200,"interval_high_milli":400,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"o2","study_id":"study-o2","model_system":"organoid","independent_group":"group-o2","interval_low_milli":220,"interval_high_milli":420,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-o2","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i1","study_id":"study-i1","model_system":"in_silico","independent_group":"group-i1","interval_low_milli":180,"interval_high_milli":360,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i1","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"estimate_id":"i2","study_id":"study-i2","model_system":"in_silico","independent_group":"group-i2","interval_low_milli":190,"interval_high_milli":370,"quality_milli":900,"uncertainty_milli":100,"artifact":{"artifact_id":"artifact-i2","content_type":"application/json","content_hash":hash,"local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ]
            }
        }),
    );
    let mission = call(
        &mut server,
        "glioma_cross_model_replication_mission",
        json!({
            "request": {
                "mission_id": "invasion-replication-mission",
                "objective": "compile an executable but bounded invasion replication workflow",
                "frontier_request": {
                    "source": envelope["envelope"].clone(),
                    "candidates": [
                        {"action_id":"replicate-organoid","kind":"independent_replication","model_system":"organoid","independent_group":"site-b","rationale":"independent organoid replication","expected_information_milli":700,"expected_range_reduction_milli":20,"reproducibility_milli":900,"feasibility_milli":850,"risk_milli":100,"cost_units":2,"depends_on":[]},
                        {"action_id":"resolve-insilico","kind":"resolve_heterogeneity","model_system":"in_silico","independent_group":"compute-b","rationale":"stress the computational stratum","expected_information_milli":650,"expected_range_reduction_milli":60,"reproducibility_milli":900,"feasibility_milli":900,"risk_milli":50,"cost_units":1,"depends_on":[]}
                    ],
                    "budget_units":3,
                    "max_actions":2,
                    "max_risk_milli":500,
                    "min_information_milli":600,
                    "target_between_system_range_milli":40
                },
                "completed_action_order": [],
                "observations": [],
                "budget_units": 3,
                "max_actions": 2,
                "beam_width": 32,
                "risk_budget_milli": 10000,
                "approval_granted": false,
                "allow_instrument_execution": false,
                "allow_federation": false,
                "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            }
        }),
    );
    assert_eq!(mission["dispatch"], json!("planning_only"));
    assert_eq!(mission["simulation_only"], json!(true));
    assert_eq!(
        mission["mission"]["output_schema"],
        json!("GliomaCrossModelReplicationMission1@1")
    );
    assert!(mission["mission"]["scheduler"].is_object());
    assert!(mission["mission"]["selected_action_order"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(mission["mission"]["scheduler"]["decisions"]
        .as_array()
        .is_some_and(|items| items
            .iter()
            .all(|item| item["stage_kind"] == "replication_robustness")));

    let execution = call(
        &mut server,
        "glioma_cross_model_replication_mission_execute",
        json!({
            "request": {
                "mission": {
                    "mission_id": "invasion-replication-mission",
                    "objective": "compile an executable but bounded invasion replication workflow",
                    "frontier_request": {
                        "source": envelope["envelope"].clone(),
                        "candidates": [
                            {"action_id":"replicate-organoid","kind":"independent_replication","model_system":"organoid","independent_group":"site-b","rationale":"independent organoid replication","expected_information_milli":700,"expected_range_reduction_milli":20,"reproducibility_milli":900,"feasibility_milli":850,"risk_milli":100,"cost_units":2,"depends_on":[]},
                            {"action_id":"resolve-insilico","kind":"resolve_heterogeneity","model_system":"in_silico","independent_group":"compute-b","rationale":"stress the computational stratum","expected_information_milli":650,"expected_range_reduction_milli":60,"reproducibility_milli":900,"feasibility_milli":900,"risk_milli":50,"cost_units":1,"depends_on":[]}
                        ],
                        "budget_units":3,
                        "max_actions":2,
                        "max_risk_milli":500,
                        "min_information_milli":600,
                        "target_between_system_range_milli":40
                    },
                    "completed_action_order": [],
                    "observations": [],
                    "budget_units": 3,
                    "max_actions": 2,
                    "beam_width": 32,
                    "risk_budget_milli": 10000,
                    "approval_granted": false,
                    "allow_instrument_execution": false,
                    "allow_federation": false,
                    "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
                },
                "source_artifacts": [],
                "completed_artifacts": [],
                "scope": null,
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(execution["dispatch"], json!("dry_run_only"));
    assert_eq!(execution["simulation_only"], json!(true));
    assert!(execution["run"]["execution"]["results"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(execution["run"]["execution"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["negative_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence
                .iter()
                .any(|entry| entry == "synthetic-dry-run-not-biological-evidence"))));
}

#[test]
fn glioma_transportability_analysis_preserves_model_distance() {
    let mut server = server();
    let hash = "0".repeat(64);
    let analysis = call(
        &mut server,
        "glioma_transportability_analyze",
        json!({
            "request": {
                "objective": "transport an invasion effect to organoids",
                "target_model_system": "organoid",
                "target_signature": [100,100],
                "min_studies": 2,
                "min_replicates_per_study": 3,
                "min_quality_milli": 700,
                "distance_scale_milli": 200,
                "max_transport_gap_milli": 700,
                "max_heterogeneity_milli": 250,
                "effect_threshold_milli": 100,
                "min_signal_to_noise_milli": 500,
                "max_leave_one_out_shift_milli": 500
            },
            "studies": [
                {"study_id":"a","model_system":"organoid","population_signature":[100,100],"effect_milli":400,"uncertainty_milli":40,"replicates":4,"quality_milli":900,"artifact":{"artifact_id":"a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"study_id":"b","model_system":"organoid","population_signature":[110,95],"effect_milli":420,"uncertainty_milli":40,"replicates":4,"quality_milli":900,"artifact":{"artifact_id":"b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(analysis["dispatch"], json!("not_started"));
    assert_eq!(analysis["analysis"]["disposition"], json!("qualified"));
    assert!(
        analysis["analysis"]["transport_gap_milli"]
            .as_u64()
            .unwrap_or_default()
            < 700
    );
}

#[test]
fn glioma_temporal_multimodal_fusion_replays_longitudinal_state_transitions() {
    let mut server = server();
    let hash = "0".repeat(64);
    let observation = |id: &str, timepoint: u32, modality: &str, x: i64, y: i64| {
        json!({
            "observation_id": id,
            "study_id": "temporal-protocol-study",
            "sample_id": "sample-a",
            "timepoint": timepoint,
            "modality": modality,
            "model_system": "organoid",
            "artifact": {"artifact_id": id, "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
            "features": [
                {"feature_id": "state_x", "value_milli": x},
                {"feature_id": "state_y", "value_milli": y}
            ]
        })
    };
    let response = call(
        &mut server,
        "glioma_temporal_multimodal_fusion",
        json!({
            "request": {
                "study_id": "temporal-protocol-study",
                "model_system": "organoid",
                "required_modalities": ["transcriptomics", "imaging"],
                "min_timepoints_per_sample": 2,
                "min_modalities_per_timepoint": 2,
                "min_shared_features": 2,
                "min_transition_support_milli": 800,
                "max_state_change_milli": 50,
                "require_complete_time_grid": true
            },
            "observations": [
                observation("a-t0-rna", 0, "transcriptomics", 100, 100),
                observation("a-t0-img", 0, "imaging", 110, 90),
                observation("a-t1-rna", 1, "transcriptomics", 300, 100),
                observation("a-t1-img", 1, "imaging", 290, 110)
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        response["analysis"]["emerging_transition_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn glioma_temporal_spatial_alignment_emits_gap_aware_follow_up() {
    let mut server = server();
    let hash = "0".repeat(64);
    let observation = |id: &str, timepoint: u32| {
        json!({
            "observation_id": id,
            "study_id": "alignment-protocol-study",
            "sample_id": "sample-a",
            "timepoint": timepoint,
            "modality": "imaging",
            "model_system": "organoid",
            "artifact": {"artifact_id": id, "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
            "features": [{"feature_id": "state", "value_milli": 500}]
        })
    };
    let response = call(
        &mut server,
        "glioma_temporal_spatial_alignment",
        json!({
            "request": {
                "study_id": "alignment-protocol-study",
                "model_system": "organoid",
                "temporal_request": {
                    "study_id": "alignment-protocol-study",
                    "model_system": "organoid",
                    "required_modalities": ["imaging"],
                    "min_timepoints_per_sample": 2,
                    "min_modalities_per_timepoint": 1,
                    "min_shared_features": 1,
                    "min_transition_support_milli": 500,
                    "max_state_change_milli": 500,
                    "require_complete_time_grid": false
                },
                "registration_request": {
                    "study_id": "alignment-protocol-study",
                    "model_system": "organoid",
                    "reference_sample_id": "sample-a",
                    "min_cells_per_landmark": 1,
                    "min_shared_lineages": 1,
                    "max_residual_milli": 100,
                    "max_landmark_spread_milli": 100
                },
                "propagation_request": {
                    "study_id": "alignment-protocol-study",
                    "model_system": "organoid",
                    "radius_milli": 2000,
                    "max_steps": 4,
                    "self_retention_milli": 800,
                    "neighbor_weight_milli": 200,
                    "cross_lineage_weight_milli": 1000,
                    "convergence_tolerance_milli": 10,
                    "hotspot_threshold_milli": 100
                },
                "sample_timepoints": [{"sample_id": "sample-a", "timepoint": 0}],
                "min_spatial_coverage_milli": 800,
                "max_state_gap_milli": 1000
            },
            "observations": [observation("obs-a-t0", 0), observation("obs-a-t1", 1)],
            "cells": [
                {"cell_id":"cell-a","sample_id":"sample-a","lineage":"tumour","x_milli":0,"y_milli":0,"state_milli":500,"artifact":{"artifact_id":"cell-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"cell-b","sample_id":"sample-a","lineage":"tumour","x_milli":1000,"y_milli":0,"state_milli":500,"artifact":{"artifact_id":"cell-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("local_analysis"));
    assert_eq!(response["simulation_only"], json!(false));
    assert_eq!(response["analysis"]["disposition"], json!("qualified"));
    assert!(
        response["analysis"]["priority_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action == "publish_aligned_state_map")
    );
}

#[test]
fn glioma_clonal_evolution_reconstructs_preclinical_marker_branch() {
    let mut server = server();
    let hash = "0".repeat(64);
    let profile = |id: &str, clone_id: &str, timepoint: u32, markers: Vec<(&str, &str)>| {
        json!({
            "profile_id": id,
            "study_id": "clonal-protocol-study",
            "sample_lineage": "lineage-a",
            "clone_id": clone_id,
            "timepoint": timepoint,
            "model_system": "organoid",
            "abundance_milli": if timepoint == 0 { 400 } else { 700 },
            "artifact": {"artifact_id": id, "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
            "markers": markers.into_iter().map(|(marker_id, state)| json!({"marker_id": marker_id, "state": state, "confidence_milli": 900})).collect::<Vec<_>>()
        })
    };
    let response = call(
        &mut server,
        "glioma_clonal_evolution",
        json!({
            "request": {
                "study_id": "clonal-protocol-study",
                "model_system": "organoid",
                "min_shared_markers": 2,
                "min_parent_score_milli": 600,
                "max_time_gap": 5,
                "min_abundance_milli": 1,
                "allow_parallel_branches": true,
                "max_parent_candidates": 2
            },
            "profiles": [
                profile("root", "clone-a", 0, vec![("egfr", "present"), ("tp53", "present")]),
                profile("child", "clone-b", 1, vec![("egfr", "present"), ("tp53", "present"), ("ecDNA", "present")])
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["simulation_only"], json!(true));
    assert_eq!(response["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        response["analysis"]["branch_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        response["analysis"]["gained_marker_order"],
        json!(["ecDNA"])
    );
}

#[test]
fn glioma_clone_perturbation_panel_covers_evolutionary_branches_under_budget() {
    let mut server = server();
    let hash = "0".repeat(64);
    let profile = |id: &str, clone_id: &str, timepoint: u32, markers: Vec<&str>| {
        json!({
            "profile_id": id,
            "study_id": "panel-protocol-study",
            "sample_lineage": "lineage-a",
            "clone_id": clone_id,
            "timepoint": timepoint,
            "model_system": "organoid",
            "abundance_milli": if timepoint == 0 { 400 } else { 600 },
            "artifact": {"artifact_id": id, "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
            "markers": markers.into_iter().map(|marker_id| json!({"marker_id": marker_id, "state": "present", "confidence_milli": 900})).collect::<Vec<_>>()
        })
    };
    let evolution = call(
        &mut server,
        "glioma_clonal_evolution",
        json!({
            "request": {"study_id": "panel-protocol-study", "model_system": "organoid", "min_shared_markers": 1, "min_parent_score_milli": 500, "max_time_gap": 5, "min_abundance_milli": 1, "allow_parallel_branches": true, "max_parent_candidates": 2},
            "profiles": [
                profile("root", "clone-a", 0, vec!["egfr", "tp53"]),
                profile("ec", "clone-b", 1, vec!["egfr", "tp53", "ecDNA"]),
                profile("pt", "clone-c", 1, vec!["egfr", "tp53", "pten"])
            ]
        }),
    );
    assert_eq!(evolution["analysis"]["disposition"], json!("qualified"));
    let panel = call(
        &mut server,
        "glioma_clone_perturbation_panel",
        json!({
            "request": {"study_id": "panel-protocol-study", "model_system": "organoid", "budget_milli": 10, "min_coverage_milli": 500, "max_selected": 2, "require_branch_coverage": true, "allow_uncertain_targets": true},
            "graph": evolution["analysis"].clone(),
            "candidates": [
                {"candidate_id": "ec-panel", "kind": "inhibit", "target_marker_order": ["ecDNA"], "cost_milli": 5, "expected_effect_milli": 900, "purpose": "branch-selective perturbation readout", "artifact": {"artifact_id": "ec-panel", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}},
                {"candidate_id": "pt-panel", "kind": "inhibit", "target_marker_order": ["pten"], "cost_milli": 5, "expected_effect_milli": 900, "purpose": "branch-selective perturbation readout", "artifact": {"artifact_id": "pt-panel", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}}
            ]
        }),
    );
    assert_eq!(panel["dispatch"], json!("not_started"));
    assert_eq!(panel["simulation_only"], json!(true));
    assert_eq!(panel["panel"]["disposition"], json!("qualified"));
    assert_eq!(
        panel["panel"]["selected_order"].as_array().unwrap().len(),
        2
    );
    assert!(
        panel["panel"]["uncovered_branch_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_clone_panel_outcomes_requires_replicates_before_qualification() {
    let mut server = server();
    let hash = "0".repeat(64);
    let profile = |id: &str, clone_id: &str, timepoint: u32, markers: Vec<&str>| {
        json!({
            "profile_id": id,
            "study_id": "outcome-protocol-study",
            "sample_lineage": "lineage-a",
            "clone_id": clone_id,
            "timepoint": timepoint,
            "model_system": "organoid",
            "abundance_milli": if timepoint == 0 { 400 } else { 600 },
            "artifact": {"artifact_id": id, "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
            "markers": markers.into_iter().map(|marker_id| json!({"marker_id": marker_id, "state": "present", "confidence_milli": 900})).collect::<Vec<_>>()
        })
    };
    let evolution = call(
        &mut server,
        "glioma_clonal_evolution",
        json!({
            "request": {"study_id": "outcome-protocol-study", "model_system": "organoid", "min_shared_markers": 1, "min_parent_score_milli": 500, "max_time_gap": 5, "min_abundance_milli": 1, "allow_parallel_branches": true, "max_parent_candidates": 2},
            "profiles": [
                profile("root", "clone-a", 0, vec!["egfr", "tp53"]),
                profile("ec", "clone-b", 1, vec!["egfr", "tp53", "ecDNA"]),
                profile("pt", "clone-c", 1, vec!["egfr", "tp53", "pten"])
            ]
        }),
    );
    let panel = call(
        &mut server,
        "glioma_clone_perturbation_panel",
        json!({
            "request": {"study_id": "outcome-protocol-study", "model_system": "organoid", "budget_milli": 10, "min_coverage_milli": 500, "max_selected": 2, "require_branch_coverage": true, "allow_uncertain_targets": true},
            "graph": evolution["analysis"].clone(),
            "candidates": [
                {"candidate_id": "ec-panel", "kind": "inhibit", "target_marker_order": ["ecDNA"], "cost_milli": 5, "expected_effect_milli": 900, "purpose": "branch assay", "artifact": {"artifact_id": "ec-panel", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}},
                {"candidate_id": "pt-panel", "kind": "inhibit", "target_marker_order": ["pten"], "cost_milli": 5, "expected_effect_milli": 900, "purpose": "branch assay", "artifact": {"artifact_id": "pt-panel", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}}
            ]
        }),
    );
    let mut observations = Vec::new();
    let mut observation_index = 0_u32;
    for branch in panel["panel"]["branch_coverage"].as_array().unwrap() {
        for candidate in branch["selected_candidate_order"].as_array().unwrap() {
            for replicate in 0..2 {
                observations.push(json!({
                    "observation_id": format!("obs{observation_index}"),
                    "study_id": "outcome-protocol-study",
                    "model_system": "organoid",
                    "candidate_id": candidate.as_str().unwrap(),
                    "branch_id": branch["branch_id"].as_str().unwrap(),
                    "replicate_id": format!("r{replicate}"),
                    "state": "measured",
                    "effect_milli": -800,
                    "uncertainty_milli": 100,
                    "artifact": {"artifact_id": format!("artifact{observation_index}"), "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}
                }));
                observation_index += 1;
            }
        }
    }
    let analysis = call(
        &mut server,
        "glioma_clone_panel_outcomes",
        json!({
            "request": {"study_id": "outcome-protocol-study", "model_system": "organoid", "min_replicates": 2, "effect_threshold_milli": 500, "max_uncertainty_milli": 200, "require_all_selected": true, "require_all_branches": true},
            "panel": panel["panel"].clone(),
            "observations": observations
        }),
    );
    assert_eq!(analysis["dispatch"], json!("not_started"));
    assert_eq!(analysis["simulation_only"], json!(true));
    assert_eq!(analysis["analysis"]["disposition"], json!("qualified"));
    assert!(
        analysis["analysis"]["next_action_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // A missing replicate/cell reopens the loop and is compiled into a bounded continuation
    // plan. The MCP route must remain planning-only and preserve the unresolved target.
    let mut incomplete_observations = observations;
    incomplete_observations.pop();
    let incomplete = call(
        &mut server,
        "glioma_clone_panel_outcomes",
        json!({
            "request": {"study_id": "outcome-protocol-study", "model_system": "organoid", "min_replicates": 2, "effect_threshold_milli": 500, "max_uncertainty_milli": 200, "require_all_selected": true, "require_all_branches": true},
            "panel": panel["panel"].clone(),
            "observations": incomplete_observations
        }),
    );
    let target = incomplete["analysis"]["next_action_order"][0]
        .as_str()
        .unwrap();
    let continuation = call(
        &mut server,
        "glioma_clone_continuation",
        json!({
            "request": {"study_id": "outcome-protocol-study", "model_system": "organoid", "budget_milli": 10, "max_selected": 3, "max_risk_tier": 2, "require_approval": true, "allow_instrument_actions": false, "allow_federation": false},
            "outcome": incomplete["analysis"].clone(),
            "candidates": [
                {"action_id": "measure-prereq", "target_id": target, "kind": "measure", "cost_milli": 2, "information_gain_milli": 900, "risk_tier": 1, "dependencies": [], "requires_instrument": false, "requires_federation": false, "description": "measure the unresolved clone-panel cell", "artifact": {"artifact_id": "measure-prereq", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}},
                {"action_id": "replicate-followup", "target_id": target, "kind": "replicate", "cost_milli": 3, "information_gain_milli": 800, "risk_tier": 1, "dependencies": ["measure-prereq"], "requires_instrument": false, "requires_federation": false, "description": "replicate the unresolved clone-panel cell", "artifact": {"artifact_id": "replicate-followup", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}}
            ]
        }),
    );
    assert_eq!(continuation["dispatch"], json!("not_started"));
    assert_eq!(continuation["simulation_only"], json!(true));
    assert_eq!(
        continuation["plan"]["disposition"],
        json!("approval_required")
    );
    assert_eq!(
        continuation["plan"]["pending_approval_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn glioma_adaptive_clone_campaign_closes_evolution_to_observed_outcomes() {
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
    let profile = |id: &str, clone_id: &str, timepoint: u32, markers: Vec<&str>| {
        json!({
            "profile_id": id,
            "study_id": "adaptive-clone-study",
            "sample_lineage": "lineage-a",
            "clone_id": clone_id,
            "timepoint": timepoint,
            "model_system": "organoid",
            "abundance_milli": if timepoint == 0 { 400 } else { 700 },
            "artifact": artifact(id),
            "markers": markers.into_iter().map(|marker_id| json!({"marker_id": marker_id, "state": "present", "confidence_milli": 900})).collect::<Vec<_>>()
        })
    };
    let campaign = call(
        &mut server,
        "glioma_adaptive_clone_campaign_execute",
        json!({
            "request": {
                "evolution": {"study_id":"adaptive-clone-study","model_system":"organoid","min_shared_markers":1,"min_parent_score_milli":500,"max_time_gap":5,"min_abundance_milli":1,"allow_parallel_branches":true,"max_parent_candidates":2},
                "profiles": [
                    profile("root", "clone-a", 0, vec!["egfr", "tp53"]),
                    profile("ec", "clone-b", 1, vec!["egfr", "tp53", "ecDNA"]),
                    profile("pt", "clone-c", 1, vec!["egfr", "tp53", "pten"])
                ],
                "panel": {"study_id":"adaptive-clone-study","model_system":"organoid","budget_milli":10,"min_coverage_milli":500,"max_selected":2,"require_branch_coverage":true,"allow_uncertain_targets":true},
                "perturbations": [
                    {"candidate_id":"ec-panel","kind":"inhibit","target_marker_order":["ecDNA"],"cost_milli":5,"expected_effect_milli":900,"purpose":"test ecDNA branch dependency","artifact":artifact("ec-panel")},
                    {"candidate_id":"pt-panel","kind":"inhibit","target_marker_order":["pten"],"cost_milli":5,"expected_effect_milli":900,"purpose":"test PTEN branch dependency","artifact":artifact("pt-panel")}
                ],
                "outcome": {"study_id":"adaptive-clone-study","model_system":"organoid","min_replicates":1,"effect_threshold_milli":500,"max_uncertainty_milli":200,"require_all_selected":true,"require_all_branches":true},
                "continuation": {"study_id":"adaptive-clone-study","model_system":"organoid","budget_milli":10,"max_selected":2,"max_risk_tier":2,"require_approval":false,"allow_instrument_actions":false,"allow_federation":false},
                "continuation_candidates": [{"action_id":"measure-unresolved","target_id":"measure:ec-panel@branch","kind":"measure","cost_milli":2,"information_gain_milli":900,"risk_tier":1,"dependencies":[],"requires_instrument":false,"requires_federation":false,"description":"measure an unresolved clone branch","artifact":artifact("measure-unresolved")}],
                "max_rounds":2,
                "max_retries":1,
                "stop_on_qualified":true,
                "stop_on_negative":true,
                "require_artifacts":true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(campaign["campaign"]["stop_reason"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["final_outcome"]["disposition"],
        json!("qualified")
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
    assert!(
        campaign["campaign"]["observations"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
}

#[test]
fn glioma_multimodal_graph_fusion_replays_dropout_and_disagreement() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_multimodal_graph_fusion",
        json!({
            "request": {
                "study_id": "graph-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "transcriptomics"],
                "min_samples": 3,
                "min_modalities_per_sample": 2,
                "min_shared_features": 2,
                "neighbours": 2,
                "diffusion_steps": 3,
                "max_distance_milli": 1000,
                "min_consensus_support_milli": 500,
                "max_disagreement_milli": 200,
                "require_all_modalities": false
            },
            "vectors": [
                {"observation_id":"a-g","study_id":"graph-study","sample_lineage":"a","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"a-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":0},{"feature_id":"y","value_milli":0}]},
                {"observation_id":"a-t","study_id":"graph-study","sample_lineage":"a","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"a-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":1000},{"feature_id":"y","value_milli":1000}]},
                {"observation_id":"b-g","study_id":"graph-study","sample_lineage":"b","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"b-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":0},{"feature_id":"y","value_milli":0}]},
                {"observation_id":"b-t","study_id":"graph-study","sample_lineage":"b","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"b-t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":0},{"feature_id":"y","value_milli":0}]},
                {"observation_id":"c-g","study_id":"graph-study","sample_lineage":"c","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"c-g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"reliability_milli":900,"features":[{"feature_id":"x","value_milli":0},{"feature_id":"y","value_milli":0}]}
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["analysis"]["disposition"], json!("partial"));
    assert_eq!(response["analysis"]["missing_sample_order"], json!(["c"]));
    assert!(
        !response["analysis"]["contradictory_pair_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_pathway_activity_ranks_cross_modal_mechanism_state() {
    let mut server = server();
    let hash = "0".repeat(64);
    let response = call(
        &mut server,
        "glioma_pathway_activity",
        json!({
            "request": {
                "objective": "rank invasion pathways",
                "study_id": "pathway-study",
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
            "definitions": [{
                "pathway_id": "invasion",
                "label": "invasion programme",
                "nodes": [
                    {"node_id":"egfr","label":"EGFR","modality":"genomics","expected_direction":1,"weight_milli":1000},
                    {"node_id":"vim","label":"VIM","modality":"transcriptomics","expected_direction":1,"weight_milli":1000}
                ],
                "edges": [{"source_node_id":"egfr","target_node_id":"vim","relation":1,"confidence_milli":900}]
            }],
            "observations": [
                {"observation_id":"g","study_id":"pathway-study","sample_lineage":"sample-1","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"g","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"egfr","value_milli":700,"reliability_milli":900},
                {"observation_id":"t","study_id":"pathway-study","sample_lineage":"sample-1","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"t","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"feature_id":"vim","value_milli":800,"reliability_milli":900}
            ]
        }),
    );
    assert_eq!(response["dispatch"], json!("not_started"));
    assert_eq!(response["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        response["analysis"]["pathways"][0]["direction"],
        json!("activated")
    );
}
