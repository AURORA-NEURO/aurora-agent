//! MCP contract tests for the program pipeline area.

use super::*;

#[test]
fn glioma_program_catalog_and_pipeline_are_reachable_through_mcp() {
    let mut server = server();
    let catalog = call(&mut server, "glioma_program_catalog", json!({}));
    assert_eq!(catalog["program_count"], json!(12));
    assert_eq!(catalog["feature_count"], json!(384));
    let features = catalog["features"]
        .as_array()
        .expect("catalog features are an array");
    assert_eq!(features.len(), 384);
    assert_eq!(
        catalog["implementation_status_source"],
        json!("research_feature_registry")
    );
    let implemented_ids = catalog["implemented_feature_ids"]
        .as_array()
        .expect("implemented ids are listed")
        .iter()
        .map(|value| value.as_str().expect("implemented id is a string"))
        .collect::<std::collections::BTreeSet<_>>();
    let planned_ids = catalog["planned_feature_ids"]
        .as_array()
        .expect("planned ids are listed")
        .iter()
        .map(|value| value.as_str().expect("planned id is a string"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        catalog["implemented_feature_count"],
        json!(implemented_ids.len())
    );
    assert_eq!(catalog["planned_feature_count"], json!(planned_ids.len()));
    assert!(implemented_ids.is_disjoint(&planned_ids));
    assert_eq!(implemented_ids.union(&planned_ids).count(), features.len());
    for feature in features {
        let feature_id = feature["feature_id"]
            .as_str()
            .expect("feature id is a string");
        let expected_status = if implemented_ids.contains(feature_id) {
            "implemented"
        } else {
            assert!(planned_ids.contains(feature_id));
            "planned"
        };
        assert_eq!(
            feature["implementation_status"],
            json!(expected_status),
            "catalog status matches the implementation manifest for {feature_id}"
        );
    }

    let replay_identity = ContentHash::of_value(&json!({"replay": "mcp-glioma"})).unwrap();
    let artifact_hash = ContentHash::of_value(&json!({"artifact": "local"})).unwrap();
    let intent = json!({
        "research_id": "mcp-glioma-research",
        "study_id": "mcp-glioma-study",
        "objective": "qualify a local preclinical glioma mechanism program",
        "output_uses": ["cohort_analysis", "method_development"],
        "model_systems": ["organoid"],
        "modalities": ["literature", "genomics", "computational", "replication"],
        "input_artifacts": [{
            "artifact_id": "local-glioma-input",
            "content_hash": artifact_hash,
            "content_type": "application/vnd.aurora.glioma-input+json",
            "local_only": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false
        }],
        "requested_autonomy": "a1",
        "approval_reference": null,
        "budget_units": 300,
        "max_retries": 1,
        "allow_instrument_execution": false,
        "allow_federation": false,
        "raw_data_local": true,
        "aggregate_only": true,
        "replay_identity": replay_identity,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let dry_run = call(
        &mut server,
        "glioma_research_dry_run",
        json!({"intent": intent.clone()}),
    );
    assert_eq!(dry_run["disposition"], json!("succeeded"));
    assert_eq!(dry_run["completed_order"].as_array().unwrap().len(), 12);
    assert!(
        dry_run["negative_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("simulation"))
    );

    let workflow = call(
        &mut server,
        "glioma_workflow_plan",
        json!({
            "request": {
                "intent": intent,
                "mode": "full_program",
                "completed_stages": [],
                "evidence": null,
                "qc_report": null,
                "mechanism_portfolio": null,
                "experiment_design": null,
                "max_parallelism": 2
            }
        }),
    );
    assert_eq!(workflow["dispatch"], json!("not_started"));
    assert_eq!(
        workflow["plan"]["output_schema"],
        json!("GliomaAdaptiveWorkflow1@1")
    );
    assert_eq!(
        workflow["next_ready_batch"],
        json!(["intent-normalization"])
    );

    let protocol = call(
        &mut server,
        "glioma_protocol_simulate",
        json!({
            "request": {
                "objective": "schedule a preclinical glioma assay",
                "model_system": "organoid",
                "tasks": [
                    {
                        "task_id": "prepare",
                        "label": "prepare organoid controls",
                        "resource_kind": "culture",
                        "resource_units": 1,
                        "duration_ticks": 2,
                        "depends_on": [],
                        "model_system": "organoid",
                        "output_schema": "Setup1@1",
                        "risk_milli": 10,
                        "requires_instrument": false
                    },
                    {
                        "task_id": "assay",
                        "label": "run invasion assay",
                        "resource_kind": "culture",
                        "resource_units": 1,
                        "duration_ticks": 3,
                        "depends_on": ["prepare"],
                        "model_system": "organoid",
                        "output_schema": "Assay1@1",
                        "risk_milli": 20,
                        "requires_instrument": false
                    }
                ],
                "resources": [{"resource_id": "culture", "kind": "culture", "capacity_units": 1}],
                "max_ticks": 20,
                "max_risk_milli": 100,
                "allow_instrument_execution": false,
                "approval_reference": null,
                "randomization_seed": artifact_hash
            }
        }),
    );
    assert_eq!(protocol["dispatch"], json!("not_started"));
    assert_eq!(protocol["simulation"]["disposition"], json!("feasible"));
    assert_eq!(
        protocol["simulation"]["schedule"].as_array().unwrap().len(),
        2
    );

    let protocol_ensemble = call(
        &mut server,
        "glioma_protocol_scenario_ensemble",
        json!({
            "request": {
                "objective": "stress a preclinical glioma assay schedule",
                "model_system": "organoid",
                "base_request": {
                    "objective": "schedule a preclinical glioma assay",
                    "model_system": "organoid",
                    "tasks": [
                        {
                            "task_id": "prepare",
                            "label": "prepare organoid controls",
                            "resource_kind": "culture",
                            "resource_units": 1,
                            "duration_ticks": 2,
                            "depends_on": [],
                            "model_system": "organoid",
                            "output_schema": "Setup1@1",
                            "risk_milli": 10,
                            "requires_instrument": false
                        },
                        {
                            "task_id": "assay",
                            "label": "run invasion assay",
                            "resource_kind": "culture",
                            "resource_units": 1,
                            "duration_ticks": 3,
                            "depends_on": ["prepare"],
                            "model_system": "organoid",
                            "output_schema": "Assay1@1",
                            "risk_milli": 20,
                            "requires_instrument": false
                        }
                    ],
                    "resources": [{"resource_id": "culture", "kind": "culture", "capacity_units": 1}],
                    "max_ticks": 20,
                    "max_risk_milli": 100,
                    "allow_instrument_execution": false,
                    "approval_reference": null,
                    "randomization_seed": artifact_hash
                },
                "scenarios": [
                    {"scenario_id":"nominal","duration_scale_milli":1000,"capacity_scale_milli":1000,"risk_delta_milli":0,"probability_milli":700,"allow_instrument_execution":false,"approval_reference":null},
                    {"scenario_id":"slow-biology","duration_scale_milli":5000,"capacity_scale_milli":1000,"risk_delta_milli":0,"probability_milli":300,"allow_instrument_execution":false,"approval_reference":null}
                ],
                "minimum_schedule_coverage_milli": 700,
                "maximum_expected_makespan_ticks": 20,
                "maximum_expected_risk_milli": 100
            }
        }),
    );
    assert_eq!(protocol_ensemble["dispatch"], json!("not_started"));
    assert_eq!(protocol_ensemble["simulation_only"], json!(true));
    assert_eq!(
        protocol_ensemble["ensemble"]["disposition"],
        json!("fragile")
    );
    assert!(
        protocol_ensemble["ensemble"]["bottleneck_task_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task == "assay")
    );

    let protocol_execution = call(
        &mut server,
        "glioma_protocol_execute",
        json!({
            "request": {
                "protocol": {
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
                    "randomization_seed": artifact_hash
                },
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(protocol_execution["dispatch"], json!("not_started"));
    assert_eq!(protocol_execution["simulation_only"], json!(true));
    assert_eq!(
        protocol_execution["execution"]["disposition"],
        json!("completed")
    );
    assert_eq!(
        protocol_execution["execution"]["completed_order"],
        json!(["assay", "prepare"])
    );

    let robustness = call(
        &mut server,
        "glioma_robustness_suite",
        json!({
            "request": {
                "objective": "stress-test a preclinical glioma invasion effect",
                "analysis": {
                    "objective": "estimate invasion effect",
                    "control_arm": "control",
                    "treatment_arm": "treated",
                    "model_system": "organoid",
                    "min_replicates_per_arm": 3,
                    "effect_threshold_milli": 100,
                    "alpha_milli": 50
                },
                "max_cases": 8,
                "include_row_jackknife": false,
                "min_eligible_cases": 2,
                "min_stability_milli": 900
            },
            "dataset": {
                "dataset_id": "mcp-robustness",
                "artifact": {
                    "artifact_id": "mcp-robustness-artifact",
                    "content_hash": artifact_hash,
                    "content_type": "application/vnd.aurora.glioma-analysis+json",
                    "local_only": true,
                    "contains_human_data": false,
                    "contains_direct_identifiers": false
                },
                "rows": [
                    {"row_id":"r1","arm_id":"control","model_system":"organoid","batch_id":"b1","outcome_milli":100},
                    {"row_id":"r2","arm_id":"control","model_system":"organoid","batch_id":"b2","outcome_milli":105},
                    {"row_id":"r3","arm_id":"control","model_system":"organoid","batch_id":"b3","outcome_milli":95},
                    {"row_id":"r4","arm_id":"control","model_system":"organoid","batch_id":"b4","outcome_milli":102},
                    {"row_id":"r5","arm_id":"treated","model_system":"organoid","batch_id":"b5","outcome_milli":300},
                    {"row_id":"r6","arm_id":"treated","model_system":"organoid","batch_id":"b6","outcome_milli":305},
                    {"row_id":"r7","arm_id":"treated","model_system":"organoid","batch_id":"b7","outcome_milli":295},
                    {"row_id":"r8","arm_id":"treated","model_system":"organoid","batch_id":"b8","outcome_milli":301}
                ]
            }
        }),
    );
    assert_eq!(robustness["dispatch"], json!("not_started"));
    assert_eq!(robustness["suite"]["disposition"], json!("stable"));
    assert_eq!(robustness["suite"]["cases"].as_array().unwrap().len(), 8);

    let trajectory = call(
        &mut server,
        "glioma_trajectory_analyze",
        json!({
            "request": {
                "objective": "compare longitudinal glioma invasion trajectories",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "min_timepoints_per_unit": 3,
                "min_units_per_arm": 2,
                "slope_threshold_milli_per_tick": 2,
                "max_residual_milli": 0,
                "max_monotonicity_violations": 0,
                "require_balanced_timepoints": true
            },
            "observations": [
                {"observation_id":"tc-0-0","unit_id":"control-0","arm_id":"control","model_system":"organoid","batch_id":"batch-0","timepoint":0,"outcome_milli":100},
                {"observation_id":"tc-0-1","unit_id":"control-0","arm_id":"control","model_system":"organoid","batch_id":"batch-1","timepoint":1,"outcome_milli":101},
                {"observation_id":"tc-0-2","unit_id":"control-0","arm_id":"control","model_system":"organoid","batch_id":"batch-2","timepoint":2,"outcome_milli":102},
                {"observation_id":"tc-1-0","unit_id":"control-1","arm_id":"control","model_system":"organoid","batch_id":"batch-0","timepoint":0,"outcome_milli":100},
                {"observation_id":"tc-1-1","unit_id":"control-1","arm_id":"control","model_system":"organoid","batch_id":"batch-1","timepoint":1,"outcome_milli":101},
                {"observation_id":"tc-1-2","unit_id":"control-1","arm_id":"control","model_system":"organoid","batch_id":"batch-2","timepoint":2,"outcome_milli":102},
                {"observation_id":"tt-0-0","unit_id":"treated-0","arm_id":"treated","model_system":"organoid","batch_id":"batch-0","timepoint":0,"outcome_milli":100},
                {"observation_id":"tt-0-1","unit_id":"treated-0","arm_id":"treated","model_system":"organoid","batch_id":"batch-1","timepoint":1,"outcome_milli":110},
                {"observation_id":"tt-0-2","unit_id":"treated-0","arm_id":"treated","model_system":"organoid","batch_id":"batch-2","timepoint":2,"outcome_milli":120},
                {"observation_id":"tt-1-0","unit_id":"treated-1","arm_id":"treated","model_system":"organoid","batch_id":"batch-0","timepoint":0,"outcome_milli":100},
                {"observation_id":"tt-1-1","unit_id":"treated-1","arm_id":"treated","model_system":"organoid","batch_id":"batch-1","timepoint":1,"outcome_milli":110},
                {"observation_id":"tt-1-2","unit_id":"treated-1","arm_id":"treated","model_system":"organoid","batch_id":"batch-2","timepoint":2,"outcome_milli":120}
            ]
        }),
    );
    assert_eq!(trajectory["dispatch"], json!("not_started"));
    assert_eq!(trajectory["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        trajectory["analysis"]["slope_effect_milli_per_tick"],
        json!(9)
    );

    let state_transition = call(
        &mut server,
        "glioma_state_transition_analyze",
        json!({
            "request": {
                "objective": "compare longitudinal glioma state transitions",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "state_order": ["low", "high"],
                "min_units_per_arm": 2,
                "min_transitions_per_arm": 2,
                "max_timepoint_gap": 2,
                "min_contrast_milli": 100
            },
            "observations": [
                {"observation_id":"st-c0-0","unit_id":"st-control-0","arm_id":"control","model_system":"organoid","batch_id":"st-b0","timepoint":0,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-c0-0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-c0-1","unit_id":"st-control-0","arm_id":"control","model_system":"organoid","batch_id":"st-b1","timepoint":1,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-c0-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-c1-0","unit_id":"st-control-1","arm_id":"control","model_system":"organoid","batch_id":"st-b2","timepoint":0,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-c1-0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-c1-1","unit_id":"st-control-1","arm_id":"control","model_system":"organoid","batch_id":"st-b3","timepoint":1,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-c1-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-t0-0","unit_id":"st-treated-0","arm_id":"treated","model_system":"organoid","batch_id":"st-b4","timepoint":0,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-t0-0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-t0-1","unit_id":"st-treated-0","arm_id":"treated","model_system":"organoid","batch_id":"st-b5","timepoint":1,"state_id":"high","state_score_milli":900,"artifact":{"artifact_id":"st-artifact-t0-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-t1-0","unit_id":"st-treated-1","arm_id":"treated","model_system":"organoid","batch_id":"st-b6","timepoint":0,"state_id":"low","state_score_milli":100,"artifact":{"artifact_id":"st-artifact-t1-0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"st-t1-1","unit_id":"st-treated-1","arm_id":"treated","model_system":"organoid","batch_id":"st-b7","timepoint":1,"state_id":"high","state_score_milli":900,"artifact":{"artifact_id":"st-artifact-t1-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-state-transition+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(state_transition["dispatch"], json!("not_started"));
    assert_eq!(
        state_transition["analysis"]["disposition"],
        json!("qualified")
    );
    assert!(
        state_transition["analysis"]["enriched_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry == "low:high")
    );

    let mut propagation_snapshots = Vec::new();
    for arm in ["control", "treated"] {
        for unit_index in 0..2 {
            for source_state in 0..2 {
                let lineage_id = format!("{arm}-{unit_index}-{source_state}");
                let mut counts = if source_state == 0 {
                    [100_u32, 0]
                } else {
                    [0_u32, 100]
                };
                for (step, timepoint_day) in [0_u32, 7, 14].into_iter().enumerate() {
                    propagation_snapshots.push(json!({
                        "observation_id": format!("{lineage_id}-{timepoint_day}"),
                        "experimental_unit_id": format!("{arm}-unit-{unit_index}"),
                        "lineage_id": lineage_id,
                        "arm_id": arm,
                        "model_system": "organoid",
                        "assay_batch_id": format!("{arm}-unit-{unit_index}-batch-{step}"),
                        "timepoint_day": timepoint_day,
                        "state_counts": counts,
                        "capture_fraction_ppm": 1_000_000,
                        "artifact": {
                            "artifact_id": format!("local:{lineage_id}-{timepoint_day}"),
                            "content_hash": artifact_hash,
                            "content_type": "application/vnd.aurora.glioma-lineage-propagation+json",
                            "local_only": true,
                            "contains_human_data": false,
                            "contains_direct_identifiers": false
                        }
                    }));
                    if step < 2 {
                        let matrix = if arm == "control" {
                            [[8_u32, 2_u32], [1_u32, 7_u32]]
                        } else {
                            [[6_u32, 4_u32], [3_u32, 5_u32]]
                        };
                        counts = [
                            (matrix[0][0] * counts[0] + matrix[0][1] * counts[1]) / 10,
                            (matrix[1][0] * counts[0] + matrix[1][1] * counts[1]) / 10,
                        ];
                    }
                }
            }
        }
    }
    let lineage_propagation = call(
        &mut server,
        "glioma_lineage_propagation_analyze",
        json!({
            "request": {
                "objective": "compare lineage-resolved glioma state propagation",
                "model_system": "organoid",
                "control_arm": "control",
                "treatment_arm": "treated",
                "state_order": ["npc_like", "mes_like"],
                "min_units_per_arm": 2,
                "min_lineages_per_arm": 2,
                "ridge_penalty_ppm": 1,
                "max_coefficient_ppm": 5_000_000,
                "max_prediction_error_ppm": 100_000,
                "bootstrap_replicates": 99,
                "bootstrap_seed": ContentHash::of_bytes(b"mcp-lineage-propagation").as_str(),
                "confidence_level_milli": 900,
                "minimum_effect_ppm": 10_000
            },
            "snapshots": propagation_snapshots.clone()
        }),
    );
    assert_eq!(
        lineage_propagation["dispatch"],
        json!("not_started"),
        "lineage propagation tool error: {}",
        lineage_propagation["error"]
    );
    assert_eq!(
        lineage_propagation["analysis"]["control"]["source_design_rank"],
        json!(2)
    );
    assert_eq!(
        lineage_propagation["analysis"]["held_out_forecasts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let lineage_decomposition = call(
        &mut server,
        "glioma_lineage_response_decompose",
        json!({
            "request": {
                "baseline_source_composition_ppm": [700_000, 300_000],
                "minimum_component_ppm": 10_000
            },
            "analysis": lineage_propagation["analysis"].clone()
        }),
    );
    assert_eq!(
        lineage_decomposition["dispatch"],
        json!("not_started"),
        "lineage-response decomposition tool error: {}",
        lineage_decomposition["error"]
    );
    assert_eq!(
        lineage_decomposition["analysis"]["decompositions"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let mut mouse_snapshots = propagation_snapshots;
    for snapshot in &mut mouse_snapshots {
        snapshot["model_system"] = json!("mouse_model");
        snapshot["observation_id"] = json!(format!(
            "mouse-{}",
            snapshot["observation_id"].as_str().unwrap()
        ));
        snapshot["experimental_unit_id"] = json!(format!(
            "mouse-{}",
            snapshot["experimental_unit_id"].as_str().unwrap()
        ));
        snapshot["lineage_id"] = json!(format!(
            "mouse-{}",
            snapshot["lineage_id"].as_str().unwrap()
        ));
        snapshot["artifact"]["artifact_id"] = json!(format!(
            "mouse-{}",
            snapshot["artifact"]["artifact_id"].as_str().unwrap()
        ));
    }
    let mouse_propagation = call(
        &mut server,
        "glioma_lineage_propagation_analyze",
        json!({
            "request": {
                "objective": "compare lineage-resolved glioma state propagation",
                "model_system": "mouse_model",
                "control_arm": "control",
                "treatment_arm": "treated",
                "state_order": ["npc_like", "mes_like"],
                "min_units_per_arm": 2,
                "min_lineages_per_arm": 2,
                "ridge_penalty_ppm": 1,
                "max_coefficient_ppm": 5_000_000,
                "max_prediction_error_ppm": 100_000,
                "bootstrap_replicates": 99,
                "bootstrap_seed": ContentHash::of_bytes(b"mcp-mouse-lineage-propagation").as_str(),
                "confidence_level_milli": 900,
                "minimum_effect_ppm": 10_000
            },
            "snapshots": mouse_snapshots
        }),
    );
    assert_eq!(
        mouse_propagation["analysis"]["disposition"],
        json!("qualified")
    );

    let lineage_transport = call(
        &mut server,
        "glioma_lineage_transport_analyze",
        json!({
            "request": {
                "objective": "compare lineage-resolved glioma state propagation",
                "minimum_model_systems": 2,
                "minimum_studies_per_model_system": 1,
                "minimum_effect_ppm": 10_000,
                "maximum_model_system_range_ppm": 5_000_000,
                "bootstrap_replicates": 99,
                "bootstrap_seed": ContentHash::of_bytes(b"mcp-lineage-transport").as_str(),
                "confidence_level_milli": 900
            },
            "studies": [
                {
                    "study_id": "organoid-study-1",
                    "analysis": lineage_propagation["analysis"].clone(),
                    "artifact": {"artifact_id":"organoid-summary","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-lineage-propagation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                },
                {
                    "study_id": "mouse-study-1",
                    "analysis": mouse_propagation["analysis"].clone(),
                    "artifact": {"artifact_id":"mouse-summary","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-lineage-propagation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}
                }
            ]
        }),
    );
    assert_eq!(lineage_transport["dispatch"], json!("not_started"));
    assert_eq!(
        lineage_transport["analysis"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        lineage_transport["analysis"]["model_system_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        lineage_transport["analysis"]["contrasts"][0]["systems"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let causal = call(
        &mut server,
        "glioma_causal_contrast",
        json!({
            "request": {
                "objective": "estimate treatment-associated invasion change",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "intervention_timepoint": 1,
                "min_units_per_arm": 2,
                "effect_threshold_milli": 5,
                "alpha_milli": 500
            },
            "observations": [
                {"observation_id":"cc0-pre","unit_id":"cc0","arm_id":"control","model_system":"organoid","batch_id":"b0","timepoint":0,"outcome_milli":100},
                {"observation_id":"cc0-post","unit_id":"cc0","arm_id":"control","model_system":"organoid","batch_id":"b1","timepoint":1,"outcome_milli":101},
                {"observation_id":"cc1-pre","unit_id":"cc1","arm_id":"control","model_system":"organoid","batch_id":"b0","timepoint":0,"outcome_milli":100},
                {"observation_id":"cc1-post","unit_id":"cc1","arm_id":"control","model_system":"organoid","batch_id":"b1","timepoint":1,"outcome_milli":101},
                {"observation_id":"ct0-pre","unit_id":"ct0","arm_id":"treated","model_system":"organoid","batch_id":"b0","timepoint":0,"outcome_milli":100},
                {"observation_id":"ct0-post","unit_id":"ct0","arm_id":"treated","model_system":"organoid","batch_id":"b1","timepoint":1,"outcome_milli":120},
                {"observation_id":"ct1-pre","unit_id":"ct1","arm_id":"treated","model_system":"organoid","batch_id":"b0","timepoint":0,"outcome_milli":100},
                {"observation_id":"ct1-post","unit_id":"ct1","arm_id":"treated","model_system":"organoid","batch_id":"b1","timepoint":1,"outcome_milli":120}
            ]
        }),
    );
    assert_eq!(causal["dispatch"], json!("not_started"));
    assert_eq!(causal["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        causal["analysis"]["difference_in_differences_milli"],
        json!(19)
    );

    let mediation = call(
        &mut server,
        "glioma_causal_mediation",
        json!({
            "request": {
                "objective": "test invasion mediator decomposition",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "min_units_per_arm": 2,
                "effect_threshold_milli": 20,
                "min_signal_to_noise_milli": 100,
                "max_leave_one_out_shift_milli": 200
            },
            "observations": [
                {"observation_id":"cm-c0","unit_id":"cm-control-0","arm_id":"control","mediator_milli":100,"outcome_milli":120,"uncertainty_milli":5,"artifact":{"artifact_id":"cm-artifact-c0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-mediation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"cm-c1","unit_id":"cm-control-1","arm_id":"control","mediator_milli":110,"outcome_milli":130,"uncertainty_milli":5,"artifact":{"artifact_id":"cm-artifact-c1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-mediation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"cm-t0","unit_id":"cm-treated-0","arm_id":"treated","mediator_milli":200,"outcome_milli":250,"uncertainty_milli":5,"artifact":{"artifact_id":"cm-artifact-t0","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-mediation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"cm-t1","unit_id":"cm-treated-1","arm_id":"treated","mediator_milli":210,"outcome_milli":260,"uncertainty_milli":5,"artifact":{"artifact_id":"cm-artifact-t1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-mediation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(mediation["dispatch"], json!("not_started"));
    assert_eq!(mediation["analysis"]["disposition"], json!("qualified"));
    assert!(
        mediation["analysis"]["indirect_effect_milli"]
            .as_i64()
            .is_some_and(|effect| effect > 0)
    );

    let multi_fidelity = call(
        &mut server,
        "glioma_multi_fidelity_optimize",
        json!({
            "request": {
                "objective": "choose the next invasion assay across fidelity levels",
                "direction": "maximize",
                "budget_units": 8,
                "max_selections": 1,
                "min_replicates_per_candidate": 1,
                "exploration_weight_milli": 250,
                "exploitation_weight_milli": 500,
                "transfer_weight_milli": 250,
                "risk_penalty_milli": 100,
                "cost_penalty_milli": 10,
                "max_risk_milli": 800,
                "min_transfer_reliability_milli": 200,
                "baseline_milli": null
            },
            "candidates": [
                {"candidate_id":"mf-screen-a","design_id":"mf-design-a","fidelity":"screening","model_system":"cell_line","dose_milli":100,"combination_milli":0,"cost_units":1,"risk_milli":100,"parent_candidate_id":null,"max_replicates":4},
                {"candidate_id":"mf-screen-b","design_id":"mf-design-b","fidelity":"screening","model_system":"cell_line","dose_milli":200,"combination_milli":0,"cost_units":1,"risk_milli":100,"parent_candidate_id":null,"max_replicates":4},
                {"candidate_id":"mf-valid-a","design_id":"mf-design-a","fidelity":"validation","model_system":"mouse_model","dose_milli":100,"combination_milli":0,"cost_units":4,"risk_milli":200,"parent_candidate_id":"mf-screen-a","max_replicates":4},
                {"candidate_id":"mf-valid-b","design_id":"mf-design-b","fidelity":"validation","model_system":"mouse_model","dose_milli":200,"combination_milli":0,"cost_units":4,"risk_milli":200,"parent_candidate_id":"mf-screen-b","max_replicates":4}
            ],
            "observations": [
                {"observation_id":"mf-obs-screen-a","candidate_id":"mf-screen-a","replicate_index":0,"outcome_milli":400,"uncertainty_milli":10,"artifact":{"artifact_id":"mf-artifact-screen-a","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-fidelity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"mf-obs-screen-b","candidate_id":"mf-screen-b","replicate_index":0,"outcome_milli":300,"uncertainty_milli":10,"artifact":{"artifact_id":"mf-artifact-screen-b","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-fidelity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"mf-obs-valid-b","candidate_id":"mf-valid-b","replicate_index":0,"outcome_milli":500,"uncertainty_milli":10,"artifact":{"artifact_id":"mf-artifact-valid-b","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-fidelity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(multi_fidelity["dispatch"], json!("not_started"));
    assert_eq!(
        multi_fidelity["optimization"]["selected_order"],
        json!(["mf-valid-a"])
    );
    assert_eq!(
        multi_fidelity["optimization"]["estimates"][2]["source"],
        json!("transferred")
    );

    let stratified_causal = call(
        &mut server,
        "glioma_stratified_causal_adjustment",
        json!({
            "request": {
                "objective": "adjust invasion effect by molecular stratum",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "min_units_per_arm_per_stratum": 2,
                "min_eligible_strata": 2,
                "effect_threshold_milli": 100,
                "max_stratum_imbalance_milli": 400,
                "max_leave_one_stratum_shift_milli": 80,
                "max_leave_one_batch_shift_milli": 80
            },
            "observations": [
                {"observation_id":"sc-a-c1","unit_id":"sc-c1","stratum_id":"low","arm_id":"control","model_system":"organoid","batch_id":"sc-b1","outcome_milli":100,"artifact":{"artifact_id":"sc-artifact-a-c1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-a-c2","unit_id":"sc-c2","stratum_id":"low","arm_id":"control","model_system":"organoid","batch_id":"sc-b2","outcome_milli":110,"artifact":{"artifact_id":"sc-artifact-a-c2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-a-t1","unit_id":"sc-t1","stratum_id":"low","arm_id":"treated","model_system":"organoid","batch_id":"sc-b3","outcome_milli":260,"artifact":{"artifact_id":"sc-artifact-a-t1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-a-t2","unit_id":"sc-t2","stratum_id":"low","arm_id":"treated","model_system":"organoid","batch_id":"sc-b4","outcome_milli":270,"artifact":{"artifact_id":"sc-artifact-a-t2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-b-c1","unit_id":"sc-c3","stratum_id":"high","arm_id":"control","model_system":"organoid","batch_id":"sc-b5","outcome_milli":200,"artifact":{"artifact_id":"sc-artifact-b-c1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-b-c2","unit_id":"sc-c4","stratum_id":"high","arm_id":"control","model_system":"organoid","batch_id":"sc-b6","outcome_milli":210,"artifact":{"artifact_id":"sc-artifact-b-c2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-b-t1","unit_id":"sc-t3","stratum_id":"high","arm_id":"treated","model_system":"organoid","batch_id":"sc-b7","outcome_milli":340,"artifact":{"artifact_id":"sc-artifact-b-t1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"sc-b-t2","unit_id":"sc-t4","stratum_id":"high","arm_id":"treated","model_system":"organoid","batch_id":"sc-b8","outcome_milli":350,"artifact":{"artifact_id":"sc-artifact-b-t2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-stratified-observation+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(stratified_causal["dispatch"], json!("not_started"));
    assert_eq!(
        stratified_causal["adjustment"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        stratified_causal["adjustment"]["adjusted_effect_milli"],
        json!(150)
    );

    let dose_response = call(
        &mut server,
        "glioma_dose_response",
        json!({
            "request": {
                "objective": "map a preclinical glioma invasion dose response",
                "model_system": "organoid",
                "control_dose_milli": 0,
                "direction": "increasing",
                "min_observations_per_dose": 1,
                "min_dose_levels": 3,
                "effect_threshold_milli": 100,
                "max_residual_milli": 0,
                "max_monotonicity_violations": 0
            },
            "observations": [
                {"observation_id":"d0","unit_id":"u0","model_system":"organoid","batch_id":"b0","dose_milli":0,"outcome_milli":100},
                {"observation_id":"d1","unit_id":"u1","model_system":"organoid","batch_id":"b1","dose_milli":10,"outcome_milli":200},
                {"observation_id":"d2","unit_id":"u2","model_system":"organoid","batch_id":"b2","dose_milli":20,"outcome_milli":300}
            ]
        }),
    );
    assert_eq!(dose_response["dispatch"], json!("not_started"));
    assert_eq!(dose_response["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        dose_response["analysis"]["half_maximal_dose_milli"],
        json!(10)
    );

    let adaptive_allocation = call(
        &mut server,
        "glioma_adaptive_allocation",
        json!({
            "request": {
                "objective": "allocate organoid invasion replicates",
                "model_system": "organoid",
                "control_arm_id": "control",
                "target_effect_milli": 100,
                "min_replicates_per_arm": 30,
                "min_probability_milli": 700,
                "max_posterior_uncertainty_milli": 60,
                "exploration_weight_milli": 300,
                "max_selected_arms": 1,
                "max_new_replicates": 20,
                "budget_units": 80,
                "risk_ceiling_milli": 700
            },
            "arms": [
                {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"adaptive-control-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-adaptive-arm+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":50,"failures":50,"prior_alpha":1,"prior_beta":1,"risk_milli":100,"cost_units":2},
                {"arm_id":"egfr","label":"EGFR perturbation","artifact":{"artifact_id":"adaptive-egfr-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-adaptive-arm+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":28,"failures":2,"prior_alpha":1,"prior_beta":1,"risk_milli":100,"cost_units":2},
                {"arm_id":"matrix","label":"matrix perturbation","artifact":{"artifact_id":"adaptive-matrix-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-adaptive-arm+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","successes":12,"failures":18,"prior_alpha":1,"prior_beta":1,"risk_milli":100,"cost_units":2}
            ]
        }),
    );
    assert_eq!(adaptive_allocation["dispatch"], json!("not_started"));
    assert_eq!(
        adaptive_allocation["allocation"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        adaptive_allocation["allocation"]["selected_order"],
        json!(["egfr"])
    );

    let closed_loop_campaign = call(
        &mut server,
        "glioma_closed_loop_campaign",
        json!({
            "request": {
                "objective": "discriminate invasion mechanisms in glioma organoids",
                "model_system": "organoid",
                "max_rounds": 2,
                "max_actions_per_round": 1,
                "budget_units": 8,
                "min_information_gain_milli": 100,
                "information_weight_milli": 700,
                "effect_weight_milli": 100,
                "feasibility_weight_milli": 200,
                "risk_penalty_milli": 200,
                "stop_concentration_milli": 900
            },
            "mechanisms": [
                {"mechanism_id":"integrin","prior_milli":500},
                {"mechanism_id":"hypoxia","prior_milli":500}
            ],
            "actions": [
                {"action_id":"assay-invasion","feature_id":"invasion-score","label":"organoid invasion assay","predicted_milli_by_mechanism":{"integrin":800,"hypoxia":100},"measurement_uncertainty_milli":50,"feasibility_milli":900,"expected_effect_milli":600,"cost_units":4,"risk_milli":100,"max_replicates":1},
                {"action_id":"assay-oxygen","feature_id":"oxygen-response","label":"oxygen response assay","predicted_milli_by_mechanism":{"integrin":400,"hypoxia":700},"measurement_uncertainty_milli":50,"feasibility_milli":800,"expected_effect_milli":300,"cost_units":4,"risk_milli":100,"max_replicates":1}
            ]
        }),
    );
    assert_eq!(closed_loop_campaign["dispatch"], json!("not_started"));
    assert_eq!(
        closed_loop_campaign["campaign"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        closed_loop_campaign["campaign"]["selected_action_order"],
        json!(["assay-invasion", "assay-oxygen"])
    );

    let experiment_cycle = call(
        &mut server,
        "glioma_experiment_operating_cycle",
        json!({
            "request": {
                "planning": {
                    "objective": "discriminate invasion mechanisms in glioma organoids",
                    "model_system": "organoid",
                    "max_rounds": 2,
                    "max_actions_per_round": 1,
                    "budget_units": 8,
                    "min_information_gain_milli": 100,
                    "information_weight_milli": 700,
                    "effect_weight_milli": 100,
                    "feasibility_weight_milli": 200,
                    "risk_penalty_milli": 200,
                    "stop_concentration_milli": 900
                },
                "mechanisms": [
                    {"mechanism_id":"integrin","prior_milli":500},
                    {"mechanism_id":"hypoxia","prior_milli":500}
                ],
                "actions": [
                    {"action_id":"assay-invasion","feature_id":"invasion-score","label":"organoid invasion assay","predicted_milli_by_mechanism":{"integrin":800,"hypoxia":100},"measurement_uncertainty_milli":50,"feasibility_milli":900,"expected_effect_milli":600,"cost_units":4,"risk_milli":100,"max_replicates":1},
                    {"action_id":"assay-oxygen","feature_id":"oxygen-response","label":"oxygen response assay","predicted_milli_by_mechanism":{"integrin":400,"hypoxia":700},"measurement_uncertainty_milli":50,"feasibility_milli":800,"expected_effect_milli":300,"cost_units":4,"risk_milli":100,"max_replicates":1}
                ],
                "observations": [],
                "simulation_only": true
            }
        }),
    );
    assert_eq!(experiment_cycle["dispatch"], json!("dry_run"));
    assert_eq!(experiment_cycle["simulation_only"], json!(true));
    assert!(
        !experiment_cycle["cycle"]["execution"]["rounds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        experiment_cycle["cycle"]["final_plan"]["digest"],
        experiment_cycle["cycle"]["execution"]["final_campaign"]["digest"]
    );

    let synergy = call(
        &mut server,
        "glioma_combination_synergy",
        json!({
            "request": {
                "objective": "map glioma combination suppression",
                "model_system": "organoid",
                "min_replicates_per_cell": 1,
                "min_combination_cells": 1,
                "synergy_threshold_milli": 50,
                "max_residual_milli": 0
            },
            "observations": [
                {"observation_id":"v","unit_id":"v","model_system":"organoid","batch_id":"b0","dose_a_milli":0,"dose_b_milli":0,"response_milli":0},
                {"observation_id":"a","unit_id":"a","model_system":"organoid","batch_id":"b1","dose_a_milli":10,"dose_b_milli":0,"response_milli":400},
                {"observation_id":"b","unit_id":"b","model_system":"organoid","batch_id":"b2","dose_a_milli":0,"dose_b_milli":10,"response_milli":400},
                {"observation_id":"ab","unit_id":"ab","model_system":"organoid","batch_id":"b3","dose_a_milli":10,"dose_b_milli":10,"response_milli":900}
            ]
        }),
    );
    assert_eq!(synergy["dispatch"], json!("not_started"));
    assert_eq!(synergy["analysis"]["disposition"], json!("qualified"));
    assert_eq!(synergy["analysis"]["cells"][0]["synergy_milli"], json!(260));

    let adaptive_dose_surface = call(
        &mut server,
        "glioma_adaptive_dose_surface",
        json!({
            "request": {
                "objective": "map an organoid glioma combination response surface",
                "model_system": "organoid",
                "observations": [
                    {"observation_id":"surface-control","unit_id":"surface-u0","batch_id":"surface-b0","model_system":"organoid","dose_a_milli":0,"dose_b_milli":0,"response_milli":0,"uncertainty_milli":20},
                    {"observation_id":"surface-low","unit_id":"surface-u1","batch_id":"surface-b1","model_system":"organoid","dose_a_milli":100,"dose_b_milli":100,"response_milli":300,"uncertainty_milli":35},
                    {"observation_id":"surface-high","unit_id":"surface-u2","batch_id":"surface-b2","model_system":"organoid","dose_a_milli":300,"dose_b_milli":300,"response_milli":760,"uncertainty_milli":40}
                ],
                "candidate_pairs": [
                    {"dose_a_milli":100,"dose_b_milli":100},
                    {"dose_a_milli":200,"dose_b_milli":200},
                    {"dose_a_milli":400,"dose_b_milli":400}
                ],
                "min_replicates_per_cell": 2,
                "budget_units": 4,
                "cost_per_pair_units": 2,
                "max_next_pairs": 2,
                "target_response_milli": 600,
                "exploration_weight_milli": 700,
                "max_estimate_uncertainty_milli": 1000,
                "max_total_dose_milli": 1000,
                "min_pair_separation_milli": 100
            }
        }),
    );
    assert_eq!(adaptive_dose_surface["dispatch"], json!("not_started"));
    assert_eq!(adaptive_dose_surface["simulation_only"], json!(true));
    assert_eq!(
        adaptive_dose_surface["plan"]["disposition"],
        json!("ready_for_validation")
    );
    assert_eq!(
        adaptive_dose_surface["plan"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let concordance = call(
        &mut server,
        "glioma_multimodal_concordance",
        json!({
            "request": {
                "study_id": "mcp-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "transcriptomics"],
                "min_shared_features": 3,
                "min_correlation_milli": 900
            },
            "vectors": [
                {
                    "observation_id": "genomics-vector",
                    "study_id": "mcp-study",
                    "sample_lineage": "mcp-sample",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "artifact": {"artifact_id":"mcp-genomics-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "features": [
                        {"feature_id":"feature-001","value_milli":1},
                        {"feature_id":"feature-002","value_milli":2},
                        {"feature_id":"feature-003","value_milli":3}
                    ]
                },
                {
                    "observation_id": "transcriptomics-vector",
                    "study_id": "mcp-study",
                    "sample_lineage": "mcp-sample",
                    "modality": "transcriptomics",
                    "model_system": "organoid",
                    "artifact": {"artifact_id":"mcp-transcriptomics-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "features": [
                        {"feature_id":"feature-001","value_milli":10},
                        {"feature_id":"feature-002","value_milli":20},
                        {"feature_id":"feature-003","value_milli":30}
                    ]
                }
            ]
        }),
    );
    assert_eq!(concordance["dispatch"], json!("not_started"));
    assert_eq!(
        concordance["concordance"]["disposition"],
        json!("qualified")
    );

    let dropout_stress = call(
        &mut server,
        "glioma_multimodal_dropout_stress",
        json!({
            "request": {
                "objective": "stress invasion endpoint under missing modalities",
                "endpoint_id": "invasion",
                "study_id": "mcp-dropout-study",
                "model_system": "organoid",
                "signals": [
                    {"modality":"imaging","signal_milli":500,"quality_milli":900,"feature_count":100,"required_for_endpoint":false},
                    {"modality":"spatial","signal_milli":520,"quality_milli":900,"feature_count":80,"required_for_endpoint":true}
                ],
                "scenarios": [
                    {"scenario_id":"complete","missing_modality_order":[],"weight_milli":500},
                    {"scenario_id":"spatial-missing","missing_modality_order":["spatial"],"weight_milli":500}
                ],
                "min_quality_milli": 700,
                "max_allowed_shift_milli": 150,
                "min_stability_milli": 700
            }
        }),
    );
    assert_eq!(dropout_stress["dispatch"], json!("not_started"));
    assert_eq!(dropout_stress["simulation_only"], json!(true));
    assert_eq!(
        dropout_stress["analysis"]["disposition"],
        json!("unresolved")
    );
    assert_eq!(
        dropout_stress["analysis"]["acquisition_order"],
        json!(["spatial"])
    );
    assert_eq!(
        dropout_stress["analysis"]["scenarios"][0]["disposition"],
        json!("stable")
    );
    assert_eq!(
        dropout_stress["analysis"]["scenarios"][1]["disposition"],
        json!("unresolved")
    );

    let missingness = call(
        &mut server,
        "glioma_multimodal_missingness",
        json!({
            "request": {
                "objective": "audit multimodal missingness before invasion analysis",
                "study_id": "mcp-missingness-study",
                "model_system": "organoid",
                "sample_ids": ["sample-a", "sample-b", "sample-c"],
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"sample_id":"sample-a","modality":"genomics","expected_feature_count":100,"observed_feature_count":100,"quality_milli":900},
                    {"sample_id":"sample-a","modality":"imaging","expected_feature_count":100,"observed_feature_count":100,"quality_milli":900},
                    {"sample_id":"sample-b","modality":"genomics","expected_feature_count":100,"observed_feature_count":100,"quality_milli":600},
                    {"sample_id":"sample-b","modality":"imaging","expected_feature_count":100,"observed_feature_count":100,"quality_milli":900},
                    {"sample_id":"sample-c","modality":"genomics","expected_feature_count":100,"observed_feature_count":0,"quality_milli":900},
                    {"sample_id":"sample-c","modality":"imaging","expected_feature_count":100,"observed_feature_count":0,"quality_milli":900}
                ],
                "min_quality_milli": 700,
                "min_complete_fraction_milli": 500,
                "max_missing_fraction_milli": 700,
                "min_samples_per_pattern": 1,
                "max_patterns": 8,
                "max_pairwise_dropout_milli": 300
            }
        }),
    );
    assert_eq!(missingness["dispatch"], json!("not_started"));
    assert_eq!(missingness["simulation_only"], json!(true));
    assert_eq!(missingness["audit"]["disposition"], json!("blocked"));
    assert_eq!(
        missingness["audit"]["acquisition_order"],
        json!(["genomics", "imaging"])
    );
    assert_eq!(
        missingness["audit"]["pairs"][0]["disposition"],
        json!("missing_required")
    );

    let reliability = call(
        &mut server,
        "glioma_multimodal_reliability",
        json!({
            "request": {
                "objective": "calibrate modality reliability before mechanism analysis",
                "study_id": "mcp-reliability-study",
                "model_system": "organoid",
                "sample_ids": ["sample-a", "sample-b"],
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"sample_id":"sample-a","modality":"genomics","replicate_index":0,"value_milli":100,"quality_milli":900},
                    {"sample_id":"sample-a","modality":"genomics","replicate_index":1,"value_milli":101,"quality_milli":900},
                    {"sample_id":"sample-a","modality":"imaging","replicate_index":0,"value_milli":100,"quality_milli":900},
                    {"sample_id":"sample-a","modality":"imaging","replicate_index":1,"value_milli":101,"quality_milli":900},
                    {"sample_id":"sample-b","modality":"genomics","replicate_index":0,"value_milli":100,"quality_milli":900},
                    {"sample_id":"sample-b","modality":"genomics","replicate_index":1,"value_milli":101,"quality_milli":900},
                    {"sample_id":"sample-b","modality":"imaging","replicate_index":0,"value_milli":100,"quality_milli":900},
                    {"sample_id":"sample-b","modality":"imaging","replicate_index":1,"value_milli":101,"quality_milli":900}
                ],
                "min_replicates_per_sample": 2,
                "max_replicates_per_sample": 4,
                "min_quality_milli": 700,
                "max_within_sample_mad_milli": 20,
                "max_leave_one_out_shift_milli": 20,
                "min_reliability_milli": 700
            }
        }),
    );
    assert_eq!(reliability["dispatch"], json!("not_started"));
    assert_eq!(reliability["simulation_only"], json!(true));
    assert_eq!(reliability["calibration"]["disposition"], json!("ready"));
    assert!(
        reliability["calibration"]["modality_summaries"][0]["reliability_milli"]
            .as_u64()
            .unwrap()
            >= 700
    );

    let portfolio = call(
        &mut server,
        "glioma_multimodal_portfolio",
        json!({
            "request": {
                "objective": "select an invasion-mechanism multimodal portfolio",
                "endpoint_id": "invasion",
                "model_system": "organoid",
                "required_dimension_order": ["function", "spatial", "state"],
                "capabilities": [
                    {"modality":"genomics","dimension_order":["state"],"reliability_milli":900,"cost_units":2,"throughput_units":800,"required_for_endpoint":true},
                    {"modality":"imaging","dimension_order":["spatial","state"],"reliability_milli":850,"cost_units":3,"throughput_units":700,"required_for_endpoint":false},
                    {"modality":"functional_perturbation","dimension_order":["function"],"reliability_milli":800,"cost_units":4,"throughput_units":500,"required_for_endpoint":false}
                ],
                "max_selected_modalities": 3,
                "budget_units": 9,
                "min_dimension_coverage_milli": 1000,
                "min_reliability_milli": 800,
                "min_redundancy_milli": 0,
                "max_alternatives": 4
            }
        }),
    );
    assert_eq!(portfolio["dispatch"], json!("not_started"));
    assert_eq!(portfolio["simulation_only"], json!(true));
    assert_eq!(portfolio["plan"]["disposition"], json!("ready"));
    assert_eq!(
        portfolio["plan"]["selected_dimension_coverage_milli"],
        json!(1000)
    );

    let drift = call(
        &mut server,
        "glioma_multimodal_drift",
        json!({
            "request": {
                "objective": "surveil multimodal QC drift before mechanism analysis",
                "study_id": "mcp-drift-study",
                "model_system": "organoid",
                "epoch_order": ["epoch-0", "epoch-1", "epoch-2"],
                "required_modalities": ["genomics", "imaging"],
                "metric_order": ["signal"],
                "observations": [
                    {"epoch_id":"epoch-0","epoch_index":0,"modality":"genomics","metric_id":"signal","value_milli":100,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900},
                    {"epoch_id":"epoch-1","epoch_index":1,"modality":"genomics","metric_id":"signal","value_milli":101,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900},
                    {"epoch_id":"epoch-2","epoch_index":2,"modality":"genomics","metric_id":"signal","value_milli":102,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900},
                    {"epoch_id":"epoch-0","epoch_index":0,"modality":"imaging","metric_id":"signal","value_milli":100,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900},
                    {"epoch_id":"epoch-1","epoch_index":1,"modality":"imaging","metric_id":"signal","value_milli":101,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900},
                    {"epoch_id":"epoch-2","epoch_index":2,"modality":"imaging","metric_id":"signal","value_milli":102,"expected_center_milli":100,"expected_spread_milli":20,"quality_milli":900}
                ],
                "baseline_epoch_count": 1,
                "min_eligible_epochs": 3,
                "min_quality_milli": 700,
                "max_allowed_drift_milli": 20,
                "max_allowed_slope_milli_per_epoch": 20
            }
        }),
    );
    assert_eq!(drift["dispatch"], json!("not_started"));
    assert_eq!(drift["simulation_only"], json!(true));
    assert_eq!(drift["surveillance"]["disposition"], json!("stable"));
    assert_eq!(
        drift["surveillance"]["summaries"].as_array().unwrap().len(),
        2
    );

    let evidence_fusion = call(
        &mut server,
        "glioma_multimodal_evidence_fusion",
        json!({
            "request": {
                "objective": "fuse multimodal invasion endpoint evidence",
                "endpoint_id": "invasion",
                "study_id": "mcp-fusion-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"modality":"genomics","value_milli":500,"uncertainty_milli":20,"reliability_milli":900,"quality_milli":900,"replicate_count":3},
                    {"modality":"imaging","value_milli":520,"uncertainty_milli":30,"reliability_milli":850,"quality_milli":900,"replicate_count":3}
                ],
                "min_modalities": 2,
                "min_reliability_milli": 700,
                "max_uncertainty_milli": 100,
                "max_contradiction_milli": 300,
                "min_fused_confidence_milli": 400
            }
        }),
    );
    assert_eq!(evidence_fusion["dispatch"], json!("not_started"));
    assert_eq!(evidence_fusion["simulation_only"], json!(true));
    assert_eq!(evidence_fusion["analysis"]["disposition"], json!("ready"));
    assert!(
        evidence_fusion["analysis"]["fused_value_milli"]
            .as_i64()
            .unwrap()
            >= 500
    );

    let sensitivity = call(
        &mut server,
        "glioma_multimodal_sensitivity",
        json!({
            "request": {
                "objective": "rank endpoint fragility before mechanism planning",
                "endpoint_id": "invasion",
                "study_id": "mcp-sensitivity-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"modality":"genomics","value_milli":500,"uncertainty_milli":20,"reliability_milli":900,"quality_milli":900,"replicate_count":3},
                    {"modality":"imaging","value_milli":520,"uncertainty_milli":30,"reliability_milli":850,"quality_milli":900,"replicate_count":3}
                ],
                "min_modalities": 2,
                "min_reliability_milli": 700,
                "max_uncertainty_milli": 100,
                "perturbation_milli": 100,
                "max_perturbation_span_milli": 300,
                "min_robustness_milli": 400
            }
        }),
    );
    assert_eq!(sensitivity["dispatch"], json!("not_started"));
    assert_eq!(sensitivity["simulation_only"], json!(true));
    assert_eq!(
        sensitivity["analysis"]["eligible_modality_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(sensitivity["analysis"]["baseline_value_milli"].is_number());

    let decision_gate = call(
        &mut server,
        "glioma_multimodal_decision_gate",
        json!({
            "request": {
                "objective": "gate invasion endpoint before mechanism planning",
                "endpoint_id": "invasion",
                "study_id": "mcp-decision-gate-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"modality":"genomics","value_milli":800,"uncertainty_milli":10,"reliability_milli":900,"quality_milli":900,"replicate_count":3},
                    {"modality":"imaging","value_milli":820,"uncertainty_milli":10,"reliability_milli":900,"quality_milli":900,"replicate_count":3}
                ],
                "direction": "higher_supports",
                "threshold_milli": 600,
                "min_modalities": 2,
                "min_reliability_milli": 700,
                "max_uncertainty_milli": 100,
                "max_interval_width_milli": 250,
                "min_margin_milli": 20,
                "min_confidence_milli": 500
            }
        }),
    );
    assert_eq!(decision_gate["dispatch"], json!("not_started"));
    assert_eq!(decision_gate["simulation_only"], json!(true));
    assert_eq!(decision_gate["analysis"]["disposition"], json!("supports"));
    assert!(
        decision_gate["analysis"]["confidence_milli"]
            .as_u64()
            .unwrap()
            >= 500
    );

    let contradiction = call(
        &mut server,
        "glioma_multimodal_contradiction_adjudication",
        json!({
            "request": {
                "objective": "adjudicate invasion contradictions before mechanism planning",
                "endpoint_id": "invasion",
                "study_id": "mcp-contradiction-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"modality":"genomics","value_milli":700,"uncertainty_milli":20,"reliability_milli":900,"quality_milli":900,"replicate_count":3},
                    {"modality":"imaging","value_milli":-500,"uncertainty_milli":20,"reliability_milli":900,"quality_milli":900,"replicate_count":3}
                ],
                "min_modalities": 2,
                "min_reliability_milli": 700,
                "max_uncertainty_milli": 100,
                "max_pair_difference_milli": 150,
                "min_trust_margin_milli": 200
            }
        }),
    );
    assert_eq!(contradiction["dispatch"], json!("not_started"));
    assert_eq!(contradiction["simulation_only"], json!(true));
    assert_eq!(
        contradiction["adjudication"]["disposition"],
        json!("unresolved")
    );
    assert_eq!(
        contradiction["adjudication"]["pairs"][0]["kind"],
        json!("sign_reversal")
    );

    let quality_forecast = call(
        &mut server,
        "glioma_multimodal_quality_forecast",
        json!({
            "request": {
                "objective": "forecast QC failure before autonomous acquisition",
                "study_id": "mcp-quality-forecast-study",
                "model_system": "organoid",
                "epoch_order": ["epoch-0", "epoch-1", "epoch-2"],
                "required_modalities": ["genomics", "imaging"],
                "observations": [
                    {"epoch_id":"epoch-0","epoch_index":0,"modality":"genomics","observed":true,"quality_milli":950,"expected_feature_count":100,"observed_feature_count":100},
                    {"epoch_id":"epoch-1","epoch_index":1,"modality":"genomics","observed":true,"quality_milli":850,"expected_feature_count":100,"observed_feature_count":100},
                    {"epoch_id":"epoch-2","epoch_index":2,"modality":"genomics","observed":true,"quality_milli":750,"expected_feature_count":100,"observed_feature_count":100},
                    {"epoch_id":"epoch-0","epoch_index":0,"modality":"imaging","observed":true,"quality_milli":900,"expected_feature_count":100,"observed_feature_count":100},
                    {"epoch_id":"epoch-1","epoch_index":1,"modality":"imaging","observed":true,"quality_milli":900,"expected_feature_count":100,"observed_feature_count":100},
                    {"epoch_id":"epoch-2","epoch_index":2,"modality":"imaging","observed":true,"quality_milli":900,"expected_feature_count":100,"observed_feature_count":100}
                ],
                "history_epochs": 3,
                "forecast_horizon": 2,
                "min_history_points": 2,
                "quality_floor_milli": 700,
                "max_negative_slope_milli_per_epoch": 100,
                "minimum_forecast_quality_milli": 700
            }
        }),
    );
    assert_eq!(quality_forecast["dispatch"], json!("not_started"));
    assert_eq!(quality_forecast["simulation_only"], json!(true));
    assert_eq!(
        quality_forecast["forecast"]["disposition"],
        json!("at_risk")
    );
    assert_eq!(
        quality_forecast["forecast"]["forecast_order"][0],
        json!("genomics")
    );

    let quality_schedule = call(
        &mut server,
        "glioma_multimodal_quality_scheduler",
        json!({
            "request": {
                "objective": "schedule preventive QC reacquisition before endpoint fusion",
                "study_id": "mcp-quality-schedule-study",
                "model_system": "organoid",
                "epoch_order": ["epoch-0", "epoch-1", "epoch-2"],
                "candidates": [
                    {"modality":"genomics","forecast_quality_milli":600,"quality_risk_milli":700,"scientific_value_milli":800,"cost_units":4,"duration_units":1,"deadline_epoch_index":0,"required":true,"fallback_modality":null},
                    {"modality":"imaging","forecast_quality_milli":900,"quality_risk_milli":300,"scientific_value_milli":700,"cost_units":2,"duration_units":1,"deadline_epoch_index":1,"required":false,"fallback_modality":null},
                    {"modality":"proteomics","forecast_quality_milli":850,"quality_risk_milli":200,"scientific_value_milli":500,"cost_units":9,"duration_units":1,"deadline_epoch_index":2,"required":false,"fallback_modality":null}
                ],
                "budget_units": 10,
                "horizon_units": 3,
                "min_forecast_quality_milli": 700,
                "max_selected": 3,
                "max_alternatives": 2
            }
        }),
    );
    assert_eq!(quality_schedule["dispatch"], json!("not_started"));
    assert_eq!(quality_schedule["simulation_only"], json!(true));
    assert_eq!(
        quality_schedule["plan"]["selected_order"][0],
        json!("genomics")
    );
    assert_eq!(
        quality_schedule["plan"]["disposition"],
        json!("conditional")
    );

    let approval_fields = json!({
        "approval_id": "mcp-quality-approval",
        "approver_id": "mcp-researcher",
        "scope": "mcp-quality-schedule-study",
        "issued_epoch": 10,
        "expires_epoch": 20,
        "revoked": false
    });
    let approval_digest = ContentHash::of_value(&approval_fields).unwrap();
    let quality_execution = call(
        &mut server,
        "glioma_multimodal_quality_execute",
        json!({
            "request": {
                "schedule": quality_schedule["plan"].clone(),
                "approval": {
                    "approval_id": "mcp-quality-approval",
                    "approver_id": "mcp-researcher",
                    "scope": "mcp-quality-schedule-study",
                    "issued_epoch": 10,
                    "expires_epoch": 20,
                    "revoked": false,
                    "approval_digest": approval_digest
                },
                "current_epoch": 12,
                "max_retries": 1,
                "stop_on_failure": true,
                "stop_on_quality_floor": false,
                "require_all_required": true,
                "quality_acceptance_floor_milli": 700,
                "execution_mode": "dry_run"
            }
        }),
    );
    assert_eq!(quality_execution["dispatch"], json!("not_started"));
    assert_eq!(quality_execution["simulation_only"], json!(true));
    assert_eq!(
        quality_execution["execution"]["disposition"],
        json!("blocked")
    );
    assert_eq!(
        quality_execution["execution"]["completed_order"][0],
        json!("genomics")
    );

    let quality_adaptive_campaign = call(
        &mut server,
        "glioma_multimodal_quality_adaptive_campaign",
        json!({
            "request": {
                "objective": "adaptively resolve QC risk before endpoint fusion",
                "study_id": "mcp-quality-schedule-study",
                "model_system": "organoid",
                "epoch_order": ["epoch-0", "epoch-1", "epoch-2"],
                "candidates": [
                    {"modality":"genomics","forecast_quality_milli":600,"quality_risk_milli":700,"scientific_value_milli":800,"cost_units":2,"duration_units":1,"deadline_epoch_index":0,"required":true,"fallback_modality":null},
                    {"modality":"imaging","forecast_quality_milli":900,"quality_risk_milli":200,"scientific_value_milli":700,"cost_units":2,"duration_units":1,"deadline_epoch_index":1,"required":false,"fallback_modality":null}
                ],
                "approval": {
                    "approval_id": "mcp-quality-approval",
                    "approver_id": "mcp-researcher",
                    "scope": "mcp-quality-schedule-study",
                    "issued_epoch": 10,
                    "expires_epoch": 20,
                    "revoked": false,
                    "approval_digest": approval_digest
                },
                "current_epoch": 12,
                "budget_units": 12,
                "horizon_units": 3,
                "min_forecast_quality_milli": 700,
                "max_selected": 2,
                "max_alternatives": 2,
                "max_rounds": 2,
                "max_retries": 0,
                "stop_on_quality_floor": false,
                "require_all_required": true,
                "execution_mode": "dry_run"
            }
        }),
    );
    assert_eq!(quality_adaptive_campaign["dispatch"], json!("not_started"));
    assert_eq!(quality_adaptive_campaign["simulation_only"], json!(true));
    assert_eq!(
        quality_adaptive_campaign["campaign"]["disposition"],
        json!("blocked")
    );
    assert_eq!(
        quality_adaptive_campaign["campaign"]["rounds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let quality_transport = call(
        &mut server,
        "glioma_multimodal_quality_transport",
        json!({
            "request": {
                "objective": "calibrate QC policy transport between local glioma studies",
                "source_study_id": "source-study",
                "target_study_id": "target-study",
                "source_model_system": "organoid",
                "target_model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "source_cells": [
                    {"cell_id":"source-g1","study_id":"source-study","cohort_id":"cohort-1","model_system":"organoid","modality":"genomics","quality_milli":900,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"source-g2","study_id":"source-study","cohort_id":"cohort-2","model_system":"organoid","modality":"genomics","quality_milli":900,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"source-i1","study_id":"source-study","cohort_id":"cohort-1","model_system":"organoid","modality":"imaging","quality_milli":850,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"source-i2","study_id":"source-study","cohort_id":"cohort-2","model_system":"organoid","modality":"imaging","quality_milli":850,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10}
                ],
                "target_cells": [
                    {"cell_id":"target-g1","study_id":"target-study","cohort_id":"cohort-1","model_system":"organoid","modality":"genomics","quality_milli":880,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"target-g2","study_id":"target-study","cohort_id":"cohort-2","model_system":"organoid","modality":"genomics","quality_milli":880,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"target-i1","study_id":"target-study","cohort_id":"cohort-1","model_system":"organoid","modality":"imaging","quality_milli":830,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10},
                    {"cell_id":"target-i2","study_id":"target-study","cohort_id":"cohort-2","model_system":"organoid","modality":"imaging","quality_milli":830,"coverage_milli":900,"alignment_milli":950,"drift_milli":20,"sample_count":10}
                ],
                "min_cells_per_modality": 2,
                "min_quality_milli": 700,
                "min_coverage_milli": 700,
                "min_alignment_milli": 800,
                "max_quality_gap_milli": 100,
                "max_coverage_gap_milli": 150,
                "max_drift_milli": 100,
                "min_transfer_confidence_milli": 700
            }
        }),
    );
    assert_eq!(quality_transport["dispatch"], json!("not_started"));
    assert_eq!(quality_transport["simulation_only"], json!(true));
    assert_eq!(
        quality_transport["calibration"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        quality_transport["calibration"]["transfer_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let quality_root_cause = call(
        &mut server,
        "glioma_multimodal_quality_root_cause",
        json!({
            "request": {
                "objective": "attribute a multimodal QC incident before reacquisition",
                "incident_id": "incident-mcp-1",
                "study_id": "root-cause-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "signals": [
                    {"signal_id":"batch-g","modality":"genomics","cause":"batch","scope":"batch","magnitude_milli":900,"reliability_milli":900,"replicate_count":3,"supports_cause":true,"observed":true},
                    {"signal_id":"batch-i","modality":"imaging","cause":"batch","scope":"batch","magnitude_milli":900,"reliability_milli":900,"replicate_count":3,"supports_cause":true,"observed":true},
                    {"signal_id":"instrument-i","modality":"imaging","cause":"instrument","scope":"batch","magnitude_milli":900,"reliability_milli":900,"replicate_count":3,"supports_cause":false,"observed":true}
                ],
                "min_signal_reliability_milli":700,
                "min_support_milli":400,
                "min_confidence_milli":600,
                "min_margin_milli":150
            }
        }),
    );
    assert_eq!(quality_root_cause["dispatch"], json!("not_started"));
    assert_eq!(quality_root_cause["simulation_only"], json!(true));
    assert_eq!(
        quality_root_cause["attribution"]["disposition"],
        json!("ready")
    );
    assert_eq!(
        quality_root_cause["attribution"]["primary_cause"],
        json!("batch")
    );

    let quality_remediation = call(
        &mut server,
        "glioma_multimodal_quality_remediation",
        json!({
            "request": {
                "objective": "recover a glioma multimodal QC incident",
                "incident_id": "incident-mcp-1",
                "study_id": "root-cause-study",
                "model_system": "organoid",
                "attributions": [{"cause":"batch","support_milli":800,"contradiction_milli":0,"net_score_milli":800,"confidence_milli":900,"signal_count":2,"modality_order":["genomics"],"disposition":"qualified","remediation_action":"re-harmonize batch controls"}],
                "candidates": [{"action_id":"reharmonize-batch","cause":"batch","action_kind":"reharmonize_batch","modalities":["genomics"],"cost_units":3,"duration_units":2,"risk_milli":100,"expected_recovery_milli":800,"prerequisites":["qc-evidence"],"evidence_ids":["evidence-1"],"available":true,"requires_approval":false}],
                "max_budget_units":10,
                "max_duration_units":10,
                "max_risk_milli":500,
                "min_recovery_milli":700,
                "require_approval_for_external":false
            }
        }),
    );
    assert_eq!(quality_remediation["dispatch"], json!("not_started"));
    assert_eq!(quality_remediation["simulation_only"], json!(true));
    assert_eq!(quality_remediation["plan"]["disposition"], json!("ready"));
    assert_eq!(
        quality_remediation["plan"]["selected_steps"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let quality_recovery = call(
        &mut server,
        "glioma_multimodal_quality_recovery",
        json!({
            "request": {
                "objective": "verify QC recovery before glioma analysis admission",
                "incident_id": "incident-mcp-1",
                "study_id": "root-cause-study",
                "model_system": "organoid",
                "remediation_action_id": "reharmonize-batch",
                "required_modalities": ["genomics"],
                "observations": [
                    {"observation_id":"baseline-g","modality":"genomics","phase":"baseline","quality_milli":600,"coverage_milli":650,"alignment_milli":700,"drift_milli":300,"reliability_milli":900,"sample_count":4},
                    {"observation_id":"post-g","modality":"genomics","phase":"post_remediation","quality_milli":900,"coverage_milli":900,"alignment_milli":900,"drift_milli":50,"reliability_milli":900,"sample_count":4}
                ],
                "min_samples_per_phase":2,
                "min_reliability_milli":700,
                "min_quality_milli":800,
                "min_coverage_milli":800,
                "min_alignment_milli":800,
                "max_drift_milli":100,
                "min_improvement_milli":50
            }
        }),
    );
    assert_eq!(quality_recovery["dispatch"], json!("not_started"));
    assert_eq!(quality_recovery["simulation_only"], json!(true));
    assert_eq!(
        quality_recovery["recovery"]["disposition"],
        json!("recovered")
    );
    assert_eq!(
        quality_recovery["recovery"]["proceed_modalities"],
        json!(["genomics"])
    );

    let harmonization = call(
        &mut server,
        "glioma_multimodal_harmonize",
        json!({
            "request": {
                "study_id": "mcp-harmonization-study",
                "model_system": "organoid",
                "required_modalities": ["genomics"],
                "reference_batch": "batch-1",
                "min_vectors_per_batch": 2,
                "min_shared_features": 2,
                "max_correction_milli": 100,
                "max_post_harmonization_spread_milli": 20
            },
            "vectors": [
                {"observation_id":"mh-v1","study_id":"mcp-harmonization-study","sample_lineage":"mh-s1","modality":"genomics","model_system":"organoid","batch_id":"batch-1","artifact":{"artifact_id":"mh-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"f1","value_milli":100},{"feature_id":"f2","value_milli":200}]},
                {"observation_id":"mh-v2","study_id":"mcp-harmonization-study","sample_lineage":"mh-s2","modality":"genomics","model_system":"organoid","batch_id":"batch-1","artifact":{"artifact_id":"mh-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"f1","value_milli":110},{"feature_id":"f2","value_milli":210}]},
                {"observation_id":"mh-v3","study_id":"mcp-harmonization-study","sample_lineage":"mh-s3","modality":"genomics","model_system":"organoid","batch_id":"batch-2","artifact":{"artifact_id":"mh-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"f1","value_milli":150},{"feature_id":"f2","value_milli":250}]},
                {"observation_id":"mh-v4","study_id":"mcp-harmonization-study","sample_lineage":"mh-s4","modality":"genomics","model_system":"organoid","batch_id":"batch-2","artifact":{"artifact_id":"mh-artifact-4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"f1","value_milli":160},{"feature_id":"f2","value_milli":260}]}
            ]
        }),
    );
    assert_eq!(harmonization["dispatch"], json!("not_started"));
    assert_eq!(
        harmonization["harmonization"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        harmonization["harmonization"]["max_correction_milli"],
        json!(50)
    );

    let latent_factors = call(
        &mut server,
        "glioma_multimodal_latent_factors",
        json!({
            "request": {
                "study_id": "mcp-latent-study",
                "model_system": "organoid",
                "required_modalities": ["genomics", "imaging"],
                "min_complete_samples": 3,
                "min_shared_features": 2,
                "components": 1,
                "max_iterations": 100,
                "convergence_tolerance_milli": 1,
                "min_explained_variance_milli": 500,
                "max_reconstruction_error_milli": 500,
                "require_all_modalities": true
            },
            "vectors": [
                {"observation_id":"ml-g1","study_id":"mcp-latent-study","sample_lineage":"ml-s1","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":100},{"feature_id":"y","value_milli":10}]},
                {"observation_id":"ml-i1","study_id":"mcp-latent-study","sample_lineage":"ml-s1","modality":"imaging","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":50},{"feature_id":"y","value_milli":5}]},
                {"observation_id":"ml-g2","study_id":"mcp-latent-study","sample_lineage":"ml-s2","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":200},{"feature_id":"y","value_milli":20}]},
                {"observation_id":"ml-i2","study_id":"mcp-latent-study","sample_lineage":"ml-s2","modality":"imaging","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":100},{"feature_id":"y","value_milli":10}]},
                {"observation_id":"ml-g3","study_id":"mcp-latent-study","sample_lineage":"ml-s3","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-5","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":300},{"feature_id":"y","value_milli":30}]},
                {"observation_id":"ml-i3","study_id":"mcp-latent-study","sample_lineage":"ml-s3","modality":"imaging","model_system":"organoid","artifact":{"artifact_id":"ml-artifact-6","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"x","value_milli":150},{"feature_id":"y","value_milli":15}]}
            ]
        }),
    );
    assert_eq!(latent_factors["dispatch"], json!("not_started"));
    assert_eq!(
        latent_factors["analysis"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        latent_factors["analysis"]["components"][0]["component_index"],
        json!(0)
    );

    let spatial_niches = call(
        &mut server,
        "glioma_spatial_niches",
        json!({
            "request": {
                "study_id": "mcp-spatial-study",
                "model_system": "organoid",
                "radius_milli": 1_500,
                "min_neighbors": 1,
                "min_cells_per_niche": 2,
                "min_interaction_enrichment_milli": 0
            },
            "cells": [
                {"cell_id":"sn-a1","sample_id":"sn-s1","lineage":"tumour","x_milli":0,"y_milli":0,"state_milli":900,"artifact":{"artifact_id":"sn-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sn-a2","sample_id":"sn-s1","lineage":"tumour","x_milli":1000,"y_milli":0,"state_milli":800,"artifact":{"artifact_id":"sn-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sn-a3","sample_id":"sn-s1","lineage":"tumour","x_milli":0,"y_milli":1000,"state_milli":850,"artifact":{"artifact_id":"sn-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sn-b1","sample_id":"sn-s1","lineage":"myeloid","x_milli":2000,"y_milli":0,"state_milli":200,"artifact":{"artifact_id":"sn-artifact-4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sn-b2","sample_id":"sn-s1","lineage":"myeloid","x_milli":3000,"y_milli":0,"state_milli":250,"artifact":{"artifact_id":"sn-artifact-5","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sn-b3","sample_id":"sn-s1","lineage":"myeloid","x_milli":2000,"y_milli":1000,"state_milli":150,"artifact":{"artifact_id":"sn-artifact-6","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(spatial_niches["dispatch"], json!("not_started"));
    assert_eq!(
        spatial_niches["analysis"]["disposition"],
        json!("qualified")
    );
    assert!(
        !spatial_niches["analysis"]["interactions"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let spatial_communication = call(
        &mut server,
        "glioma_spatial_communication",
        json!({
            "request": {
                "study_id": "mcp-communication-study",
                "model_system": "organoid",
                "radius_milli": 1_500,
                "min_neighbors": 1,
                "min_lineage_cells": 1,
                "min_signal_milli": 100,
                "min_enrichment_milli": 900,
                "max_pairs": 10
            },
            "cells": [
                {"cell_id":"sc-t1","sample_id":"sc-s1","lineage":"tumour","x_milli":0,"y_milli":0,"ligand_scores_milli":{"L1":900},"receptor_scores_milli":{},"artifact":{"artifact_id":"sc-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-communication+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sc-t2","sample_id":"sc-s1","lineage":"tumour","x_milli":500,"y_milli":0,"ligand_scores_milli":{"L1":800},"receptor_scores_milli":{},"artifact":{"artifact_id":"sc-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-communication+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sc-m1","sample_id":"sc-s1","lineage":"myeloid","x_milli":1000,"y_milli":0,"ligand_scores_milli":{},"receptor_scores_milli":{"R1":900},"artifact":{"artifact_id":"sc-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-communication+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sc-m2","sample_id":"sc-s1","lineage":"myeloid","x_milli":1000,"y_milli":500,"ligand_scores_milli":{},"receptor_scores_milli":{"R1":800},"artifact":{"artifact_id":"sc-artifact-4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-communication+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ],
            "pairs": [{"pair_id":"L1-R1","ligand_feature":"L1","receptor_feature":"R1"}]
        }),
    );
    assert_eq!(spatial_communication["dispatch"], json!("not_started"));
    assert_eq!(
        spatial_communication["analysis"]["disposition"],
        json!("partial")
    );
    assert!(
        !spatial_communication["analysis"]["enriched_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let spatial_state_propagation = call(
        &mut server,
        "glioma_spatial_state_propagation",
        json!({
            "request": {
                "study_id": "mcp-propagation-study",
                "model_system": "organoid",
                "radius_milli": 1500,
                "max_steps": 20,
                "self_retention_milli": 700,
                "neighbor_weight_milli": 300,
                "cross_lineage_weight_milli": 500,
                "convergence_tolerance_milli": 1,
                "hotspot_threshold_milli": 50
            },
            "cells": [
                {"cell_id":"sp-a","sample_id":"sp-s1","lineage":"tumour","x_milli":0,"y_milli":0,"state_milli":900,"artifact":{"artifact_id":"sp-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"sp-b","sample_id":"sp-s1","lineage":"tumour","x_milli":1000,"y_milli":0,"state_milli":0,"artifact":{"artifact_id":"sp-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(spatial_state_propagation["dispatch"], json!("not_started"));
    assert_eq!(
        spatial_state_propagation["analysis"]["disposition"],
        json!("qualified")
    );
    assert!(
        spatial_state_propagation["analysis"]["converged"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        spatial_state_propagation["analysis"]["edge_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let spatial_registration = call(
        &mut server,
        "glioma_spatial_registration",
        json!({
            "request": {
                "study_id": "mcp-registration-study",
                "model_system": "organoid",
                "reference_sample_id": "registration-reference",
                "min_cells_per_landmark": 1,
                "min_shared_lineages": 2,
                "max_residual_milli": 20,
                "max_landmark_spread_milli": 20
            },
            "cells": [
                {"cell_id":"reg-r-a","sample_id":"registration-reference","lineage":"tumour","x_milli":0,"y_milli":0,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-r-a","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"reg-r-b","sample_id":"registration-reference","lineage":"myeloid","x_milli":1000,"y_milli":0,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-r-b","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"reg-r-c","sample_id":"registration-reference","lineage":"astro","x_milli":0,"y_milli":1000,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-r-c","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"reg-s-a","sample_id":"registration-shifted","lineage":"tumour","x_milli":100,"y_milli":200,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-s-a","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"reg-s-b","sample_id":"registration-shifted","lineage":"myeloid","x_milli":1100,"y_milli":200,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-s-b","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"cell_id":"reg-s-c","sample_id":"registration-shifted","lineage":"astro","x_milli":100,"y_milli":1200,"state_milli":500,"artifact":{"artifact_id":"reg-artifact-s-c","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-spatial-registration+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(spatial_registration["dispatch"], json!("not_started"));
    assert_eq!(spatial_registration["simulation_only"], json!(true));
    assert_eq!(
        spatial_registration["analysis"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        spatial_registration["analysis"]["registered_cell_order"]
            .as_array()
            .unwrap()
            .len(),
        6
    );

    let sensitivity = call(
        &mut server,
        "glioma_causal_sensitivity",
        json!({
            "request": {
                "objective": "stress the invasion mechanism effect",
                "control_arm": "control",
                "treatment_arm": "treated",
                "model_system": "organoid",
                "expected_direction": "positive",
                "min_units_per_arm": 2,
                "effect_threshold_milli": 100,
                "max_confounder_strength_milli": 400,
                "strength_step_milli": 100,
                "max_leave_one_out_shift_milli": 100
            },
            "observations": [
                {"observation_id":"ms-c1","unit_id":"ms-c1","arm_id":"control","model_system":"organoid","outcome_milli":100,"confounder_score_milli":-500,"artifact":{"artifact_id":"ms-a1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sensitivity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"ms-c2","unit_id":"ms-c2","arm_id":"control","model_system":"organoid","outcome_milli":110,"confounder_score_milli":-400,"artifact":{"artifact_id":"ms-a2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sensitivity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"ms-t1","unit_id":"ms-t1","arm_id":"treated","model_system":"organoid","outcome_milli":300,"confounder_score_milli":400,"artifact":{"artifact_id":"ms-a3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sensitivity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"observation_id":"ms-t2","unit_id":"ms-t2","arm_id":"treated","model_system":"organoid","outcome_milli":310,"confounder_score_milli":500,"artifact":{"artifact_id":"ms-a4","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-sensitivity+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(sensitivity["dispatch"], json!("not_started"));
    assert_eq!(
        sensitivity["analysis"]["tipping_strength_milli"],
        json!(200)
    );
    assert_eq!(sensitivity["analysis"]["disposition"], json!("partial"));

    let consensus = call(
        &mut server,
        "glioma_multimodal_consensus",
        json!({
            "request": {
                "study_id": "mcp-consensus-study",
                "model_system": "organoid",
                "cluster_count": 2,
                "min_modalities_per_sample": 2,
                "min_modalities_per_feature": 1,
                "min_shared_features": 3,
                "max_iterations": 8,
                "max_distance_milli": 100
            },
            "vectors": [
                {"observation_id":"cs1-g","study_id":"mcp-consensus-study","sample_lineage":"cs1","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"cs1-g-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":1},{"feature_id":"feature-002","value_milli":2},{"feature_id":"feature-003","value_milli":3}]},
                {"observation_id":"cs1-t","study_id":"mcp-consensus-study","sample_lineage":"cs1","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"cs1-t-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":1},{"feature_id":"feature-002","value_milli":2},{"feature_id":"feature-003","value_milli":3}]},
                {"observation_id":"cs2-g","study_id":"mcp-consensus-study","sample_lineage":"cs2","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"cs2-g-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":2},{"feature_id":"feature-002","value_milli":3},{"feature_id":"feature-003","value_milli":4}]},
                {"observation_id":"cs2-t","study_id":"mcp-consensus-study","sample_lineage":"cs2","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"cs2-t-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":2},{"feature_id":"feature-002","value_milli":3},{"feature_id":"feature-003","value_milli":4}]},
                {"observation_id":"cs3-g","study_id":"mcp-consensus-study","sample_lineage":"cs3","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"cs3-g-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":900},{"feature_id":"feature-002","value_milli":901},{"feature_id":"feature-003","value_milli":902}]},
                {"observation_id":"cs3-t","study_id":"mcp-consensus-study","sample_lineage":"cs3","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"cs3-t-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":900},{"feature_id":"feature-002","value_milli":901},{"feature_id":"feature-003","value_milli":902}]},
                {"observation_id":"cs4-g","study_id":"mcp-consensus-study","sample_lineage":"cs4","modality":"genomics","model_system":"organoid","artifact":{"artifact_id":"cs4-g-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":901},{"feature_id":"feature-002","value_milli":902},{"feature_id":"feature-003","value_milli":903}]},
                {"observation_id":"cs4-t","study_id":"mcp-consensus-study","sample_lineage":"cs4","modality":"transcriptomics","model_system":"organoid","artifact":{"artifact_id":"cs4-t-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-vector+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"features":[{"feature_id":"feature-001","value_milli":901},{"feature_id":"feature-002","value_milli":902},{"feature_id":"feature-003","value_milli":903}]}
            ]
        }),
    );
    assert_eq!(consensus["dispatch"], json!("not_started"));
    assert_eq!(consensus["consensus"]["disposition"], json!("qualified"));
    assert_eq!(
        consensus["consensus"]["assignments"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let meta_analysis = call(
        &mut server,
        "glioma_replication_meta_analyze",
        json!({
            "request": {
                "objective": "pool independent organoid invasion effects",
                "model_system": "organoid",
                "min_studies": 3,
                "min_replicates_per_study": 3,
                "effect_threshold_milli": 100,
                "max_i2_milli": 200,
                "min_signal_to_noise_milli": 1000,
                "max_leave_one_out_shift_milli": 60
            },
            "studies": [
                {"study_id":"meta-study-1","site_id":"meta-site-a","model_system":"organoid","artifact":{"artifact_id":"meta-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-meta-study+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"effect_milli":300,"uncertainty_milli":20,"replicate_count":4},
                {"study_id":"meta-study-2","site_id":"meta-site-b","model_system":"organoid","artifact":{"artifact_id":"meta-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-meta-study+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"effect_milli":310,"uncertainty_milli":25,"replicate_count":4},
                {"study_id":"meta-study-3","site_id":"meta-site-c","model_system":"organoid","artifact":{"artifact_id":"meta-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-meta-study+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"effect_milli":295,"uncertainty_milli":22,"replicate_count":4}
            ]
        }),
    );
    assert_eq!(meta_analysis["dispatch"], json!("not_started"));
    assert_eq!(meta_analysis["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        meta_analysis["analysis"]["included_order"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let federated_benchmark = call(
        &mut server,
        "glioma_federated_benchmark_consensus",
        json!({
            "request": {
                "objective": "compare invasion model improvements across sites",
                "capability_id": "glioma:invasion-model",
                "benchmark_world": "glioma-world-v1",
                "metric_name": "holdout_auc",
                "model_system": "organoid",
                "minimum_sites": 3,
                "minimum_replicates_per_site": 3,
                "effect_threshold_milli": 50,
                "max_i2_milli": 250,
                "min_signal_to_noise_milli": 500,
                "max_site_spread_milli": 80,
                "max_leave_one_out_shift_milli": 60
            },
            "sites": [
                {"site_id":"bench-site-a","study_id":"bench-study-a","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"bench-artifact-a","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-benchmark+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":620,"uncertainty_milli":20,"replicate_count":4},
                {"site_id":"bench-site-b","study_id":"bench-study-b","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"bench-artifact-b","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-benchmark+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":625,"uncertainty_milli":22,"replicate_count":4},
                {"site_id":"bench-site-c","study_id":"bench-study-c","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"bench-artifact-c","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-benchmark+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":618,"uncertainty_milli":21,"replicate_count":4}
            ]
        }),
    );
    assert_eq!(federated_benchmark["dispatch"], json!("not_started"));
    assert_eq!(
        federated_benchmark["consensus"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        federated_benchmark["consensus"]["included_order"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let federated_site_plan = call(
        &mut server,
        "glioma_federated_benchmark_site_plan",
        json!({
            "request": {
                "benchmark": {
                    "objective": "plan the next invasion benchmark site",
                    "capability_id": "glioma:invasion-model",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_auc",
                    "model_system": "organoid",
                    "minimum_sites": 3,
                    "minimum_replicates_per_site": 3,
                    "effect_threshold_milli": 50,
                    "max_i2_milli": 250,
                    "min_signal_to_noise_milli": 500,
                    "max_site_spread_milli": 80,
                    "max_leave_one_out_shift_milli": 60
                },
                "current_sites": [
                    {"site_id":"plan-site-a","study_id":"plan-study-a","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"plan-artifact-a","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":620,"uncertainty_milli":20,"replicate_count":4},
                    {"site_id":"plan-site-b","study_id":"plan-study-b","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"plan-artifact-b","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":625,"uncertainty_milli":22,"replicate_count":4}
                ],
                "candidates": [{"candidate_id":"plan-candidate-c","site_id":"plan-site-c","study_id":"plan-study-c","independence_group":"plan-group-c","artifact":{"artifact_id":"plan-artifact-c","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"expected_candidate_score_milli":625,"uncertainty_milli":20,"replicate_count":4,"cost_units":1,"privacy_risk_milli":100}],
                "budget_units": 5,
                "max_new_sites": 2,
                "beam_width": 4,
                "privacy_budget_milli": 500,
                "conservatism_milli": 500
            }
        }),
    );
    assert_eq!(federated_site_plan["dispatch"], json!("not_started"));
    assert_eq!(federated_site_plan["simulation_only"], json!(true));
    assert_eq!(
        federated_site_plan["site_plan"]["disposition"],
        json!("ready_to_validate")
    );
    assert_eq!(
        federated_site_plan["site_plan"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let mechanism_discrimination = call(
        &mut server,
        "glioma_mechanism_discriminate",
        json!({
            "request": {
                "objective": "discriminate invasion mechanisms",
                "model_system": "organoid",
                "min_shared_features": 2,
                "max_mechanisms": 4,
                "max_actions": 4,
                "min_information_gain_milli": 10
            },
            "hypotheses": [
                {"mechanism_id":"motility","statement":"motility pathway drives invasion","predictions":[{"feature_id":"f1","predicted_milli":100,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":200,"uncertainty_milli":10}]},
                {"mechanism_id":"matrix","statement":"matrix remodeling drives invasion","predictions":[{"feature_id":"f1","predicted_milli":400,"uncertainty_milli":10},{"feature_id":"f2","predicted_milli":500,"uncertainty_milli":10}]}
            ],
            "observations": [
                {"feature_id":"f1","observed_milli":100,"uncertainty_milli":10,"artifact":{"artifact_id":"mechanism-observation","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"feature_id":"f2","observed_milli":200,"uncertainty_milli":10,"artifact":{"artifact_id":"mechanism-observation-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ],
            "actions": [
                {"action_id":"perturb-f1","feature_id":"f1","predicted_milli_by_mechanism":{"matrix":500,"motility":100},"measurement_uncertainty_milli":20,"feasibility_milli":1000,"cost_units":1}
            ]
        }),
    );
    assert_eq!(mechanism_discrimination["dispatch"], json!("not_started"));
    assert_eq!(
        mechanism_discrimination["discrimination"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_discrimination["discrimination"]["rankings"][0]["mechanism_id"],
        json!("motility")
    );
    assert_eq!(
        mechanism_discrimination["discrimination"]["selected_action_order"][0],
        json!("perturb-f1")
    );

    let mechanism_bayesian_update = call(
        &mut server,
        "glioma_mechanism_bayesian_update",
        json!({
            "request": {
                "objective": "update invasion mechanism posterior",
                "model_system": "organoid",
                "min_shared_features": 1,
                "max_hypotheses": 4,
                "likelihood_scale_milli": 100,
                "supported_posterior_floor_milli": 600,
                "contradicted_posterior_ceiling_milli": 100
            },
            "hypotheses": [
                {"mechanism_id":"motility","statement":"motility pathway drives invasion","prior_milli":500,"predictions":[{"feature_id":"f1","predicted_milli":100,"uncertainty_milli":10}]},
                {"mechanism_id":"matrix","statement":"matrix remodeling drives invasion","prior_milli":500,"predictions":[{"feature_id":"f1","predicted_milli":500,"uncertainty_milli":10}]}
            ],
            "observations": [
                {"feature_id":"f1","observed_milli":105,"uncertainty_milli":10,"artifact":{"artifact_id":"bayesian-observation","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-feature+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(mechanism_bayesian_update["dispatch"], json!("not_started"));
    assert_eq!(
        mechanism_bayesian_update["update"]["records"][0]["mechanism_id"],
        json!("motility")
    );
    assert_eq!(
        mechanism_bayesian_update["update"]["disposition"],
        json!("qualified")
    );

    let mechanism_state_filter = call(
        &mut server,
        "glioma_mechanism_state_filter",
        json!({
            "request": {
                "objective": "track longitudinal glioma invasion mechanisms",
                "model_system": "organoid",
                "mechanisms": [
                    {"mechanism_id":"growth","statement":"growth state drives invasion","prior_milli":500,"transition_milli_by_state":{"growth":900,"stress":100},"predictions_milli":{"invasion-score":800},"process_uncertainty_milli":100},
                    {"mechanism_id":"stress","statement":"stress state drives invasion","prior_milli":500,"transition_milli_by_state":{"growth":100,"stress":900},"predictions_milli":{"invasion-score":200},"process_uncertainty_milli":100}
                ],
                "observations": [{"timepoint":1,"feature_id":"invasion-score","modality":"imaging","observed_milli":790,"measurement_uncertainty_milli":100,"artifact":{"artifact_id":"state-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}],
                "min_coverage_milli":800,
                "max_entropy_milli":900
            }
        }),
    );
    assert_eq!(mechanism_state_filter["dispatch"], json!("not_started"));
    assert_eq!(mechanism_state_filter["simulation_only"], json!(true));
    assert_eq!(
        mechanism_state_filter["filter"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_state_filter["filter"]["dominant_mechanism_order"][0],
        json!("growth")
    );

    let mechanism_state_smoother = call(
        &mut server,
        "glioma_mechanism_state_smoother",
        json!({
            "request": {
                "objective": "retrospectively smooth longitudinal glioma invasion mechanisms",
                "model_system": "organoid",
                "mechanisms": [
                    {"mechanism_id":"growth","statement":"growth state drives invasion","prior_milli":500,"transition_milli_by_state":{"growth":900,"stress":100},"predictions_milli":{"invasion-score":800},"process_uncertainty_milli":100},
                    {"mechanism_id":"stress","statement":"stress state drives invasion","prior_milli":500,"transition_milli_by_state":{"growth":100,"stress":900},"predictions_milli":{"invasion-score":200},"process_uncertainty_milli":100}
                ],
                "observations": [
                    {"timepoint":1,"feature_id":"invasion-score","modality":"imaging","observed_milli":790,"measurement_uncertainty_milli":100,"artifact":{"artifact_id":"smooth-observation-1","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"timepoint":2,"feature_id":"invasion-score","modality":"imaging","observed_milli":210,"measurement_uncertainty_milli":100,"artifact":{"artifact_id":"smooth-observation-2","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "min_coverage_milli":800,
                "max_entropy_milli":950,
                "min_transition_support_milli":0
            }
        }),
    );
    assert_eq!(mechanism_state_smoother["dispatch"], json!("not_started"));
    assert_eq!(mechanism_state_smoother["simulation_only"], json!(true));
    assert_eq!(
        mechanism_state_smoother["smoother"]["posteriors"][0]["posterior_milli_by_mechanism"]
            .as_object()
            .unwrap()
            .values()
            .map(|value| value.as_u64().unwrap())
            .sum::<u64>(),
        1_000
    );
    assert_eq!(
        mechanism_state_smoother["smoother"]["transition_support"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let mechanism_consensus = call(
        &mut server,
        "glioma_mechanism_consensus",
        json!({
            "request": {
                "objective": "reconcile glioma invasion mechanism evidence",
                "model_system": "organoid",
                "mechanisms": ["growth", "stress"],
                "min_sources_per_mechanism": 2,
                "max_conflict_milli": 500,
                "max_leave_one_out_milli": 500
            },
            "evidence": [
                {"source_id":"imaging","evidence_id":"imaging-growth","mechanism_id":"growth","posterior_milli":900,"reliability_milli":900,"independence_group":"imaging","artifact":{"artifact_id":"consensus-i-growth","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"source_id":"imaging","evidence_id":"imaging-stress","mechanism_id":"stress","posterior_milli":100,"reliability_milli":900,"independence_group":"imaging","artifact":{"artifact_id":"consensus-i-stress","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"source_id":"pathway","evidence_id":"pathway-growth","mechanism_id":"growth","posterior_milli":800,"reliability_milli":900,"independence_group":"pathway","artifact":{"artifact_id":"consensus-p-growth","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"source_id":"pathway","evidence_id":"pathway-stress","mechanism_id":"stress","posterior_milli":200,"reliability_milli":900,"independence_group":"pathway","artifact":{"artifact_id":"consensus-p-stress","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(mechanism_consensus["dispatch"], json!("not_started"));
    assert_eq!(mechanism_consensus["simulation_only"], json!(true));
    assert_eq!(
        mechanism_consensus["consensus"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_consensus["consensus"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["posterior_milli"].as_u64().unwrap())
            .sum::<u64>(),
        1_000
    );

    let mechanism_action_plan = call(
        &mut server,
        "glioma_mechanism_action_plan",
        json!({
            "discrimination": mechanism_discrimination["discrimination"].clone(),
            "config": {
                "model_system": "organoid",
                "modality": "transcriptomics",
                "max_actions": 4,
                "budget_units": 4
            }
        }),
    );
    assert_eq!(mechanism_action_plan["dispatch"], json!("not_started"));
    assert_eq!(mechanism_action_plan["simulation_only"], json!(true));
    assert_eq!(
        mechanism_action_plan["plan"]["action_order"][0],
        json!("mechanism-assay:perturb-f1")
    );

    let mechanism_graph = call(
        &mut server,
        "glioma_mechanism_graph_propagate",
        json!({
            "request": {
                "objective": "propagate invasion mechanism support",
                "model_system": "organoid",
                "max_iterations": 100,
                "convergence_tolerance_milli": 1,
                "damping_milli": 600,
                "min_edge_confidence_milli": 500,
                "top_k": 3
            },
            "nodes": [
                {"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":800,"contradiction_milli":0},
                {"node_id":"stat3","label":"STAT3 state","modality":"transcriptomics","prior_milli":0,"support_milli":400,"contradiction_milli":0},
                {"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}
            ],
            "edges": [
                {"edge_id":"e-egfr-stat3","source_node_id":"egfr","target_node_id":"stat3","relation":"activates","confidence_milli":900,"evidence_order":["paper-1"]},
                {"edge_id":"e-stat3-invasion","source_node_id":"stat3","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-2"]}
            ]
        }),
    );
    assert_eq!(mechanism_graph["dispatch"], json!("not_started"));
    assert_eq!(
        mechanism_graph["propagation"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_graph["propagation"]["ranking_order"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let mechanism_counterfactual = call(
        &mut server,
        "glioma_mechanism_counterfactual",
        json!({
            "request": {
                "objective": "simulate EGFR inhibition on invasion mechanism",
                "model_system": "organoid",
                "max_iterations": 100,
                "convergence_tolerance_milli": 1,
                "damping_milli": 600,
                "min_edge_confidence_milli": 500,
                "min_effect_milli": 10,
                "top_k": 3
            },
            "nodes": [
                {"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":800,"contradiction_milli":0},
                {"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}
            ],
            "edges": [
                {"edge_id":"e-egfr-invasion","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-egfr"]}
            ],
            "interventions": [
                {"intervention_id":"inhibit-egfr","node_id":"egfr","delta_milli":-600,"rationale":"test whether EGFR support propagates to invasion","evidence_order":["paper-egfr"]}
            ]
        }),
    );
    assert_eq!(mechanism_counterfactual["dispatch"], json!("not_started"));
    assert_eq!(
        mechanism_counterfactual["counterfactual"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_counterfactual["counterfactual"]["intervention_order"],
        json!(["inhibit-egfr"])
    );
    assert_eq!(
        mechanism_counterfactual["counterfactual"]["contrasts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let mechanism_ensemble_counterfactual = call(
        &mut server,
        "glioma_mechanism_ensemble_counterfactual",
        json!({
            "request": {
                "objective": "rank robust EGFR inhibition targets",
                "model_system": "organoid",
                "max_iterations": 100,
                "convergence_tolerance_milli": 1,
                "damping_milli": 600,
                "min_edge_confidence_milli": 500,
                "min_effect_milli": 10,
                "min_model_agreement_milli": 750,
                "top_k": 2
            },
            "models": [
                {"model_id":"model-a","prior_milli":600,"nodes":[{"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":800,"contradiction_milli":0},{"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}],"edges":[{"edge_id":"model-a-edge","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-a"]}]},
                {"model_id":"model-b","prior_milli":400,"nodes":[{"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":700,"contradiction_milli":0},{"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}],"edges":[{"edge_id":"model-b-edge","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-b"]}]}
            ],
            "interventions": [
                {"intervention_id":"inhibit-egfr","node_id":"egfr","delta_milli":-600,"rationale":"test whether EGFR support propagates to invasion","evidence_order":["paper-egfr"]}
            ]
        }),
    );
    assert_eq!(
        mechanism_ensemble_counterfactual["dispatch"],
        json!("not_started")
    );
    assert_eq!(
        mechanism_ensemble_counterfactual["ensemble"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        mechanism_ensemble_counterfactual["ensemble"]["model_order"],
        json!(["model-a", "model-b"])
    );
    assert_eq!(
        mechanism_ensemble_counterfactual["ensemble"]["targets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let robust_intervention_portfolio = call(
        &mut server,
        "glioma_robust_intervention_portfolio",
        json!({
            "request": {
                "objective": "select a robust invasion-suppressing perturbation",
                "model_system": "organoid",
                "max_iterations": 100,
                "convergence_tolerance_milli": 1,
                "damping_milli": 600,
                "min_edge_confidence_milli": 500,
                "direction": "decrease",
                "budget_units": 2,
                "max_selected": 1,
                "min_robust_effect_milli": 10,
                "min_agreement_milli": 750,
                "risk_ceiling_milli": 700,
                "effect_weight_milli": 500,
                "tail_weight_milli": 300,
                "worst_case_weight_milli": 200,
                "feasibility_weight_milli": 1000,
                "risk_penalty_milli": 1
            },
            "models": [
                {"model_id":"model-a","prior_milli":600,"nodes":[{"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":800,"contradiction_milli":0},{"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}],"edges":[{"edge_id":"model-a-edge","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-a"]}]},
                {"model_id":"model-b","prior_milli":400,"nodes":[{"node_id":"egfr","label":"EGFR activation","modality":"genomics","prior_milli":100,"support_milli":700,"contradiction_milli":0},{"node_id":"invasion","label":"invasion phenotype","modality":"functional_perturbation","prior_milli":0,"support_milli":0,"contradiction_milli":0}],"edges":[{"edge_id":"model-b-edge","source_node_id":"egfr","target_node_id":"invasion","relation":"activates","confidence_milli":900,"evidence_order":["paper-b"]}]}
            ],
            "candidates": [
                {"candidate_id":"inhibit-egfr","label":"inhibit EGFR","intervention":{"intervention_id":"inhibit-egfr","node_id":"egfr","delta_milli":-600,"rationale":"test whether EGFR support propagates to invasion","evidence_order":["paper-egfr"]},"target_node_id":"invasion","redundancy_group":"egfr","feasibility_milli":1000,"cost_units":1,"risk_milli":100}
            ]
        }),
    );
    assert_eq!(
        robust_intervention_portfolio["dispatch"],
        json!("not_started")
    );
    assert_eq!(
        robust_intervention_portfolio["portfolio"]["selected_order"],
        json!(["inhibit-egfr"])
    );
    assert_eq!(
        robust_intervention_portfolio["portfolio"]["disposition"],
        json!("qualified")
    );

    let mechanism_validation_plan = call(
        &mut server,
        "glioma_mechanism_validation_plan",
        json!({
            "request": {
                "objective": "select a robust invasion-suppressing perturbation",
                "portfolio": robust_intervention_portfolio["portfolio"].clone(),
                "power": {
                    "objective": "select a robust invasion-suppressing perturbation",
                    "model_system": "organoid",
                    "endpoint": "invasion-index",
                    "control_arm_id": "control",
                    "target_effect_milli": 10,
                    "alpha_total_milli": 100,
                    "power_target_milli": 500,
                    "current_look": 1,
                    "max_looks": 2,
                    "min_replicates_per_arm": 1,
                    "max_replicates_per_arm": 16,
                    "max_new_replicates_per_arm": 4,
                    "budget_units": 16,
                    "risk_ceiling_milli": 700
                },
                "arms": [
                    {"arm_id":"control","candidate_id":null,"label":"vehicle control","target_node_id":"invasion","protocol_id":"invasion-v1","role":"control","artifact":{"artifact_id":"validation-control-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"arm_id":"egfr-arm","candidate_id":"inhibit-egfr","label":"EGFR perturbation","target_node_id":"invasion","protocol_id":"invasion-v1","role":"intervention","artifact":{"artifact_id":"validation-egfr-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "observations": [
                    {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"validation-control-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":100,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1},
                    {"arm_id":"egfr-arm","label":"EGFR perturbation","artifact":{"artifact_id":"validation-egfr-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":160,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1}
                ],
                "require_qualified_portfolio": true
            }
        }),
    );
    assert_eq!(mechanism_validation_plan["dispatch"], json!("not_started"));
    assert_eq!(
        mechanism_validation_plan["validation"]["disposition"],
        json!("qualified")
    );
    assert!(mechanism_validation_plan["validation"]["power_plan"].is_object());

    let mechanism_validation_protocol = call(
        &mut server,
        "glioma_mechanism_validation_protocol_compile",
        json!({
            "request": {
                "validation": mechanism_validation_plan["validation"].clone(),
                "arms": [
                    {"arm_id":"control","candidate_id":null,"label":"vehicle control","target_node_id":"invasion","protocol_id":"invasion-v1","role":"control","artifact":{"artifact_id":"validation-control-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"arm_id":"egfr-arm","candidate_id":"inhibit-egfr","label":"EGFR perturbation","target_node_id":"invasion","protocol_id":"invasion-v1","role":"intervention","artifact":{"artifact_id":"validation-egfr-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "resources": [
                    {"resource_id":"culture","kind":"culture","capacity_units":1},
                    {"resource_id":"compute","kind":"compute","capacity_units":1}
                ],
                "max_ticks": 200,
                "max_risk_milli": 1000,
                "allow_instrument_execution": false,
                "approval_reference": null,
                "randomization_seed": artifact_hash,
                "ticks_per_replicate": 2,
                "include_quality_task": true
            }
        }),
    );
    assert_eq!(
        mechanism_validation_protocol["dispatch"],
        json!("not_started")
    );
    assert_eq!(
        mechanism_validation_protocol["compilation"]["disposition"],
        json!("compiled")
    );
    assert_eq!(
        mechanism_validation_protocol["compilation"]["preflight"]["disposition"],
        json!("feasible")
    );
    assert!(
        mechanism_validation_protocol["compilation"]["task_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task.as_str().is_some_and(|task| task.ends_with(":qc")))
    );
    assert!(
        mechanism_validation_protocol["compilation"]["withheld_arm_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arm| arm == "egfr-arm")
    );

    let mechanism_validation_execution = call(
        &mut server,
        "glioma_mechanism_validation_protocol_execute",
        json!({
            "request": {
                "compilation": mechanism_validation_protocol["compilation"].clone(),
                "max_retries": 1,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(
        mechanism_validation_execution["execution_mode"],
        json!("dry_run_local_worker")
    );
    assert_eq!(
        mechanism_validation_execution["execution"]["disposition"],
        json!("completed")
    );
    assert!(
        mechanism_validation_execution["execution"]["execution"]["task_results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["artifact"].is_object())
    );

    let validation_batch_assessment = call(
        &mut server,
        "glioma_validation_batch_assess",
        json!({
            "request": {
                "power_request": {
                    "objective": "select a robust invasion-suppressing perturbation",
                    "model_system": "organoid",
                    "endpoint": "invasion-index",
                    "control_arm_id": "control",
                    "target_effect_milli": 10,
                    "alpha_total_milli": 100,
                    "power_target_milli": 500,
                    "current_look": 1,
                    "max_looks": 2,
                    "min_replicates_per_arm": 1,
                    "max_replicates_per_arm": 16,
                    "max_new_replicates_per_arm": 4,
                    "budget_units": 16,
                    "risk_ceiling_milli": 700
                },
                "execution": mechanism_validation_execution["execution"].clone(),
                "prior_observations": [
                    {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"validation-control-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":100,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1},
                    {"arm_id":"egfr-arm","label":"EGFR perturbation","artifact":{"artifact_id":"validation-egfr-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":160,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1}
                ],
                "new_observations": [
                    {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"validation-control-observation-new","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":102,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1}
                ],
                "advance_look": true,
                "require_complete_execution": true
            }
        }),
    );
    assert_eq!(
        validation_batch_assessment["assessment"]["disposition"],
        json!("evaluated")
    );
    assert_eq!(
        validation_batch_assessment["assessment"]["next_power_plan"]["current_look"],
        json!(2)
    );
    assert_eq!(
        validation_batch_assessment["assessment"]["merged_observations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|observation| observation["arm_id"] == "control")
            .unwrap()["observations"],
        json!(4)
    );

    let validation_campaign = call(
        &mut server,
        "glioma_validation_campaign_execute",
        json!({
            "request": {
                "validation": {
                    "objective": "select a robust invasion-suppressing perturbation",
                    "portfolio": robust_intervention_portfolio["portfolio"].clone(),
                    "power": {
                        "objective": "select a robust invasion-suppressing perturbation",
                        "model_system": "organoid",
                        "endpoint": "invasion-index",
                        "control_arm_id": "control",
                        "target_effect_milli": 10,
                        "alpha_total_milli": 100,
                        "power_target_milli": 500,
                        "current_look": 1,
                        "max_looks": 2,
                        "min_replicates_per_arm": 1,
                        "max_replicates_per_arm": 16,
                        "max_new_replicates_per_arm": 4,
                        "budget_units": 16,
                        "risk_ceiling_milli": 700
                    },
                    "arms": [
                        {"arm_id":"control","candidate_id":null,"label":"vehicle control","target_node_id":"invasion","protocol_id":"invasion-v1","role":"control","artifact":{"artifact_id":"campaign-control-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                        {"arm_id":"egfr-arm","candidate_id":"inhibit-egfr","label":"EGFR perturbation","target_node_id":"invasion","protocol_id":"invasion-v1","role":"intervention","artifact":{"artifact_id":"campaign-egfr-arm","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                    ],
                    "observations": [
                        {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"campaign-control-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":100,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1},
                        {"arm_id":"egfr-arm","label":"EGFR perturbation","artifact":{"artifact_id":"campaign-egfr-observation","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":160,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1}
                    ],
                    "require_qualified_portfolio": true
                },
                "resources": [
                    {"resource_id":"culture","kind":"culture","capacity_units":1},
                    {"resource_id":"compute","kind":"compute","capacity_units":1}
                ],
                "max_ticks": 200,
                "max_risk_milli": 1000,
                "allow_instrument_execution": false,
                "approval_reference": null,
                "randomization_seed": artifact_hash,
                "ticks_per_replicate": 2,
                "include_quality_task": true,
                "max_retries": 1,
                "require_artifacts": true,
                "max_rounds": 1,
                "observation_batches": [[
                    {"arm_id":"control","label":"vehicle control","artifact":{"artifact_id":"campaign-control-new","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"model_system":"organoid","mean_response_milli":101,"variance_milli2":100,"observations":2,"risk_milli":100,"cost_units":1}
                ]]
            }
        }),
    );
    assert_eq!(
        validation_campaign["campaign"]["disposition"],
        json!("completed")
    );
    assert_eq!(
        validation_campaign["campaign"]["rounds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(validation_campaign["campaign"]["rounds"][0]["assessment"].is_object());

    let validation_replication_gate = call(
        &mut server,
        "glioma_validation_replication_gate",
        json!({
            "request": {
                "validation_campaign": validation_campaign["campaign"].clone(),
                "local_site_id": "origin-site",
                "replication_plan": {
                    "objective": "replicate the validated invasion effect across independent organoid sites",
                    "model_system": "organoid",
                    "endpoint": "invasion-index",
                    "control_arm_id": "control",
                    "treatment_arm_id": "treated",
                    "target_effect_milli": 100,
                    "alpha_total_milli": 50,
                    "power_target_milli": 700,
                    "min_sites": 2,
                    "max_sites": 4,
                    "min_replicates_per_site": 2,
                    "max_replicates_per_site": 20,
                    "budget_units": 100,
                    "max_total_replicates": 80,
                    "max_site_heterogeneity_milli": 200,
                    "risk_ceiling_milli": 500
                },
                "observations": [],
                "minimum_quality_milli": 800,
                "negative_effect_threshold_milli": 50,
                "current_round": 1,
                "max_rounds": 4,
                "protocol_resources": [],
                "max_ticks": 200,
                "max_risk_milli": 500,
                "allow_instrument_execution": false,
                "approval_reference": null,
                "randomization_seed": artifact_hash,
                "ticks_per_replicate": 2,
                "include_quality_task": true
            }
        }),
    );
    assert_eq!(
        validation_replication_gate["dispatch"],
        json!("not_started")
    );
    assert_eq!(
        validation_replication_gate["gate"]["disposition"],
        json!("hold_sites")
    );
    assert_eq!(
        validation_replication_gate["gate"]["validation_eligible"],
        json!(true)
    );
    assert!(validation_replication_gate["gate"]["replication_plan"].is_null());

    let validation_replication_campaign = call(
        &mut server,
        "glioma_validation_replication_campaign_execute",
        json!({
            "request": {
                "gate": validation_replication_gate["gate"].clone(),
                "replication": {
                    "objective": "replicate the validated invasion effect across independent organoid sites",
                    "model_system": "organoid",
                    "target_model_system": "organoid",
                    "target_signature": [1, 2],
                    "min_sites": 2,
                    "min_replicates_per_site": 1,
                    "min_studies": 2,
                    "min_replicates_per_study": 1,
                    "effect_threshold_milli": 10,
                    "max_heterogeneity_milli": 500,
                    "max_i2_milli": 900,
                    "min_signal_to_noise_milli": 1,
                    "max_leave_one_out_shift_milli": 500,
                    "min_quality_milli": 500,
                    "distance_scale_milli": 1000,
                    "max_transport_gap_milli": 500,
                    "max_transport_heterogeneity_milli": 500,
                    "budget_units": 10,
                    "max_rounds": 1,
                    "max_actions_per_round": 1,
                    "max_retries": 0,
                    "initial_studies": [],
                    "initial_transport_studies": [],
                    "replay_identity": artifact_hash
                }
            }
        }),
    );
    assert_eq!(
        validation_replication_campaign["campaign"]["disposition"],
        json!("hold_independent_sites")
    );
    assert!(validation_replication_campaign["campaign"]["campaign"].is_null());

    let replication_federated_transport = call(
        &mut server,
        "glioma_replication_federated_transport_execute",
        json!({
            "request": {
                "replication": validation_replication_campaign["campaign"].clone(),
                "transport": {
                    "objective": "replicate the validated invasion effect across independent organoid sites",
                    "mechanism_id": "invasion",
                    "target_model_system": "organoid",
                    "target_signature": [1, 2],
                    "min_sites": 2,
                    "min_replicates_per_site": 1,
                    "min_quality_milli": 500,
                    "similarity_scale_milli": 1000,
                    "effect_threshold_milli": 10,
                    "min_signal_to_noise_milli": 1,
                    "max_heterogeneity_milli": 900,
                    "max_site_spread_milli": 1000,
                    "max_leave_one_out_shift_milli": 1000,
                    "require_target_model": true
                },
                "aggregate_quality": [],
                "actions": [{
                    "action_id": "aggregate-site-follow-up",
                    "target_site_id": "site-c",
                    "model_system": "organoid",
                    "population_signature": [1, 2],
                    "cost_units": 1,
                    "expected_information_milli": 100,
                    "expected_effect_milli": 100,
                    "expected_heterogeneity_reduction_milli": 100,
                    "feasibility_milli": 900,
                    "risk_milli": 10,
                    "requested_replicates": 1
                }],
                "budget_units": 1,
                "max_rounds": 1,
                "max_retries": 0,
                "stop_on_qualified": false,
                "stop_on_negative": false
            }
        }),
    );
    assert_eq!(
        replication_federated_transport["transport"]["disposition"],
        json!("blocked_by_replication")
    );
    assert!(replication_federated_transport["transport"]["campaign"].is_null());

    let replication_closure_frontier = call(
        &mut server,
        "glioma_replication_closure_frontier",
        json!({
            "request": {
                "replication": validation_replication_campaign["campaign"].clone(),
                "candidates": [{
                    "action_id": "confirm-negative",
                    "target": "confirm_negative_result",
                    "route": "glioma_validation_replication_campaign_execute",
                    "model_system": "organoid",
                    "rationale": "confirm only after the upstream independent-site gate is admitted",
                    "expected_information_milli": 800,
                    "reproducibility_milli": 900,
                    "feasibility_milli": 900,
                    "risk_milli": 100,
                    "cost_units": 1,
                    "requires_independent_site": true
                }],
                "budget_units": 2,
                "max_actions": 1,
                "max_risk_milli": 500,
                "min_utility_milli": 0
            }
        }),
    );
    assert_eq!(
        replication_closure_frontier["frontier"]["disposition"],
        json!("blocked")
    );
    assert!(
        replication_closure_frontier["frontier"]["selected_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let replication_closure_execution = call(
        &mut server,
        "glioma_replication_closure_execute",
        json!({
            "request": {
                "frontier": replication_closure_frontier["frontier"].clone(),
                "campaign": {
                    "objective": validation_replication_campaign["campaign"]["objective"].clone(),
                    "model_system": "organoid",
                    "target_model_system": "organoid",
                    "target_signature": [0, 0],
                    "min_sites": 1,
                    "min_replicates_per_site": 1,
                    "min_studies": 1,
                    "min_replicates_per_study": 1,
                    "effect_threshold_milli": 10,
                    "max_heterogeneity_milli": 500,
                    "max_i2_milli": 900,
                    "min_signal_to_noise_milli": 1,
                    "max_leave_one_out_shift_milli": 500,
                    "min_quality_milli": 500,
                    "distance_scale_milli": 1000,
                    "max_transport_gap_milli": 500,
                    "max_transport_heterogeneity_milli": 500,
                    "budget_units": 1,
                    "max_rounds": 1,
                    "max_actions_per_round": 1,
                    "max_retries": 0,
                    "initial_studies": [],
                    "initial_transport_studies": [],
                    "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            }
        }),
    );
    assert_eq!(
        replication_closure_execution["execution"]["disposition"],
        json!("held_by_frontier")
    );
    assert!(replication_closure_execution["execution"]["campaign"].is_null());

    let replication_closure_campaign = call(
        &mut server,
        "glioma_replication_closure_campaign_execute",
        json!({
            "request": {
                "campaign_id": "mcp-closure-campaign",
                "objective": validation_replication_campaign["campaign"]["objective"].clone(),
                "model_system": "organoid",
                "frontiers": [{
                    "frontier": replication_closure_frontier["frontier"].clone(),
                    "campaign": {
                        "objective": validation_replication_campaign["campaign"]["objective"].clone(),
                        "model_system": "organoid",
                        "target_model_system": "organoid",
                        "target_signature": [0, 0],
                        "min_sites": 1,
                        "min_replicates_per_site": 1,
                        "min_studies": 1,
                        "min_replicates_per_study": 1,
                        "effect_threshold_milli": 10,
                        "max_heterogeneity_milli": 500,
                        "max_i2_milli": 900,
                        "min_signal_to_noise_milli": 1,
                        "max_leave_one_out_shift_milli": 500,
                        "min_quality_milli": 500,
                        "distance_scale_milli": 1000,
                        "max_transport_gap_milli": 500,
                        "max_transport_heterogeneity_milli": 500,
                        "budget_units": 1,
                        "max_rounds": 1,
                        "max_actions_per_round": 1,
                        "max_retries": 0,
                        "initial_studies": [],
                        "initial_transport_studies": [],
                        "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
                    }
                }],
                "budget_units": 1,
                "max_rounds": 1,
                "stop_on_qualified": true,
                "stop_on_negative": true,
                "stop_on_unresolved": true,
                "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        }),
    );
    assert_eq!(
        replication_closure_campaign["campaign"]["disposition"],
        json!("held")
    );
    assert_eq!(
        replication_closure_campaign["campaign"]["stop_reason"],
        json!("held_by_frontier")
    );

    let closure_interpretation = call(
        &mut server,
        "glioma_replication_closure_interpret",
        json!({
            "request": {
                "campaign": replication_closure_campaign["campaign"].clone(),
                "minimum_campaign_evidence": 0,
                "synthesis": {
                    "objective": validation_replication_campaign["campaign"]["objective"].clone(),
                    "hypothesis": "the invasion mechanism is reproducible across the declared model boundary",
                    "model_system": "organoid",
                    "min_evidence": 1,
                    "min_independent_groups": 1,
                    "min_families": 1,
                    "min_quality_milli": 1,
                    "effect_threshold_milli": 10,
                    "max_disagreement_milli": 1000,
                    "max_leave_one_out_shift_milli": 1000,
                    "require_replication_family": true,
                    "replay_identity": replay_identity,
                    "evidence": [{
                        "evidence_id": "mcp-causal-seed",
                        "family": "causal_contrast",
                        "independent_group": "mcp-study",
                        "model_system": "organoid",
                        "direction": "positive",
                        "effect_milli": 100,
                        "uncertainty_milli": 20,
                        "quality_milli": 800,
                        "sample_count": 3,
                        "artifact": {
                            "artifact_id": "mcp-causal-artifact",
                            "content_hash": artifact_hash,
                            "content_type": "application/vnd.aurora.glioma.analysis+json",
                            "local_only": true,
                            "contains_human_data": false,
                            "contains_direct_identifiers": false
                        },
                        "negative_evidence": []
                    }]
                }
            }
        }),
    );
    assert_eq!(
        closure_interpretation["interpretation"]["disposition"],
        json!("partial")
    );

    let federated_interpretation = call(
        &mut server,
        "glioma_federated_interpretation",
        json!({
            "request": {
                "interpretation": closure_interpretation["interpretation"].clone(),
                "benchmark": {
                    "objective": validation_replication_campaign["campaign"]["objective"].clone(),
                    "capability_id": "glioma:invasion",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_effect",
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
                    "capability_id": "glioma:invasion",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_effect",
                    "model_system": "organoid",
                    "artifact": {
                        "artifact_id": "mcp-federated-artifact",
                        "content_hash": artifact_hash,
                        "content_type": "application/vnd.aurora.glioma.federated-benchmark+json",
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false
                    },
                    "baseline_score_milli": 500,
                    "candidate_score_milli": 650,
                    "uncertainty_milli": 100,
                    "replicate_count": 2
                }],
                "require_qualified_interpretation": false,
                "require_qualified_consensus": true
            }
        }),
    );
    assert_eq!(
        federated_interpretation["interpretation"]["disposition"],
        json!("partial")
    );
    assert_eq!(
        federated_interpretation["interpretation"]["alignment"],
        json!(false)
    );

    let information_design = call(
        &mut server,
        "glioma_information_design",
        json!({
            "acquisition_objective": "panel_predictive_diameter",
            "request": {
                "objective": "select an assay that separates EGFR and matrix invasion mechanisms",
                "model_system": "organoid",
                "budget_units": 4,
                "max_selected_actions": 1,
                "min_information_gain_milli": 10,
                "information_weight_milli": 800,
                "feasibility_weight_milli": 200,
                "risk_penalty_milli": 100,
                "cost_penalty_milli": 0,
                "risk_ceiling_milli": 700
            },
            "mechanisms": [
                {"mechanism_id":"egfr","prior_milli":500},
                {"mechanism_id":"matrix","prior_milli":500}
            ],
            "actions": [
                {"action_id":"uninformative","feature_id":"feature-uninformative","label":"uninformative assay","outcomes":[
                    {"outcome_id":"low","label":"low invasion","probability_milli_by_mechanism":{"egfr":500,"matrix":500}},
                    {"outcome_id":"high","label":"high invasion","probability_milli_by_mechanism":{"egfr":500,"matrix":500}}
                ],"feasibility_milli":900,"risk_milli":100,"cost_units":2,"max_replicates":1},
                {"action_id":"separating","feature_id":"feature-separating","label":"separating assay","outcomes":[
                    {"outcome_id":"low","label":"low invasion","probability_milli_by_mechanism":{"egfr":900,"matrix":100}},
                    {"outcome_id":"high","label":"high invasion","probability_milli_by_mechanism":{"egfr":100,"matrix":900}}
                ],"feasibility_milli":900,"risk_milli":100,"cost_units":2,"max_replicates":1}
            ]
        }),
    );
    assert_eq!(information_design["dispatch"], json!("not_started"));
    assert_eq!(
        information_design["design"]["acquisition_objective"],
        json!("panel_predictive_diameter")
    );
    assert_eq!(
        information_design["design"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        information_design["design"]["selected_order"],
        json!(["separating"])
    );
    assert_eq!(
        information_design["design"]["scores"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let adaptive_panel = call(
        &mut server,
        "glioma_adaptive_panel",
        json!({
            "request": {
                "objective": "compile a complementary glioma mechanism assay panel",
                "model_system": "organoid",
                "mechanisms": [
                    {"mechanism_id":"egfr","prior_milli":500},
                    {"mechanism_id":"matrix","prior_milli":500}
                ],
                "actions": [
                    {"action_id":"imaging-panel","feature_id":"invasion-image","label":"invasion imaging","independence_group":"imaging","outcomes":[
                        {"outcome_id":"low","probability_milli_by_mechanism":{"egfr":900,"matrix":100}},
                        {"outcome_id":"high","probability_milli_by_mechanism":{"egfr":100,"matrix":900}}
                    ],"feasibility_milli":900,"risk_milli":100,"cost_units":1,"max_replicates":1},
                    {"action_id":"pathway-panel","feature_id":"pathway-score","label":"pathway activity","independence_group":"pathway","outcomes":[
                        {"outcome_id":"low","probability_milli_by_mechanism":{"egfr":800,"matrix":200}},
                        {"outcome_id":"high","probability_milli_by_mechanism":{"egfr":200,"matrix":800}}
                    ],"feasibility_milli":900,"risk_milli":100,"cost_units":1,"max_replicates":1}
                ],
                "budget_units":2,
                "max_selected_actions":2,
                "min_information_gain_milli":10,
                "min_feasibility_milli":700,
                "risk_ceiling_milli":500,
                "diversity_weight_milli":100,
                "risk_penalty_milli":100,
                "cost_penalty_milli":10
            }
        }),
    );
    assert_eq!(adaptive_panel["dispatch"], json!("not_started"));
    assert_eq!(adaptive_panel["simulation_only"], json!(true));
    assert_eq!(adaptive_panel["panel"]["disposition"], json!("qualified"));
    assert_eq!(
        adaptive_panel["panel"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let replication_plan = call(
        &mut server,
        "glioma_replication_plan",
        json!({
            "request": {
                "objective": "replicate a glioma invasion effect across independent organoid sites",
                "model_system": "organoid",
                "endpoint": "invasion",
                "control_arm_id": "control",
                "treatment_arm_id": "treated",
                "target_effect_milli": 100,
                "alpha_total_milli": 50,
                "power_target_milli": 700,
                "min_sites": 2,
                "max_sites": 4,
                "min_replicates_per_site": 2,
                "max_replicates_per_site": 20,
                "budget_units": 100,
                "max_total_replicates": 80,
                "max_site_heterogeneity_milli": 200,
                "risk_ceiling_milli": 500
            },
            "observations": [
                {"site_id":"site-a","arm_id":"control","label":"site-a control","model_system":"organoid","mean_response_milli":100,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"rep-a-control","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"site_id":"site-a","arm_id":"treated","label":"site-a treated","model_system":"organoid","mean_response_milli":220,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"rep-a-treated","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"site_id":"site-b","arm_id":"control","label":"site-b control","model_system":"organoid","mean_response_milli":110,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"rep-b-control","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"site_id":"site-b","arm_id":"treated","label":"site-b treated","model_system":"organoid","mean_response_milli":230,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"rep-b-treated","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(replication_plan["dispatch"], json!("not_started"));
    assert_eq!(replication_plan["simulation_only"], json!(true));
    assert_eq!(
        replication_plan["plan"]["site_order"],
        json!(["site-a", "site-b"])
    );
    assert!(
        replication_plan["plan"]["pooled_effect_milli"]
            .as_i64()
            .unwrap()
            >= 100
    );

    let replication_continuation = call(
        &mut server,
        "glioma_replication_continuation",
        json!({
            "request": {
                "plan_request": {
                    "objective": "continue a glioma invasion replication wave",
                    "model_system": "organoid",
                    "endpoint": "invasion",
                    "control_arm_id": "control",
                    "treatment_arm_id": "treated",
                    "target_effect_milli": 200,
                    "alpha_total_milli": 50,
                    "power_target_milli": 700,
                    "min_sites": 2,
                    "max_sites": 4,
                    "min_replicates_per_site": 2,
                    "max_replicates_per_site": 20,
                    "budget_units": 100,
                    "max_total_replicates": 80,
                    "max_site_heterogeneity_milli": 200,
                    "risk_ceiling_milli": 500
                },
                "current_round": 1,
                "max_rounds": 4,
                "minimum_quality_milli": 700,
                "negative_effect_threshold_milli": 40,
                "observations": [
                    {"round":1,"quality_milli":900,"observation":{"site_id":"site-a","arm_id":"control","label":"site-a control","model_system":"organoid","mean_response_milli":100,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"cont-a-control","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}},
                    {"round":1,"quality_milli":900,"observation":{"site_id":"site-a","arm_id":"treated","label":"site-a treated","model_system":"organoid","mean_response_milli":220,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"cont-a-treated","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}},
                    {"round":1,"quality_milli":900,"observation":{"site_id":"site-b","arm_id":"control","label":"site-b control","model_system":"organoid","mean_response_milli":110,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"cont-b-control","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}},
                    {"round":1,"quality_milli":900,"observation":{"site_id":"site-b","arm_id":"treated","label":"site-b treated","model_system":"organoid","mean_response_milli":230,"variance_milli2":100,"observations":2,"cost_units_per_replicate":1,"risk_milli":100,"artifact":{"artifact_id":"cont-b-treated","content_hash":artifact_hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}}
                ],
                "previous_plan": null
            }
        }),
    );
    assert_eq!(replication_continuation["dispatch"], json!("not_started"));
    assert_eq!(replication_continuation["simulation_only"], json!(true));
    assert_eq!(
        replication_continuation["plan"]["disposition"],
        json!("continue")
    );
    assert_eq!(replication_continuation["plan"]["next_round"], json!(2));

    let replication_protocol_compile = call(
        &mut server,
        "glioma_replication_protocol_compile",
        json!({
            "request": {
                "continuation": replication_continuation["plan"].clone(),
                "resources": [
                    {"resource_id":"culture","kind":"culture","capacity_units":1},
                    {"resource_id":"compute","kind":"compute","capacity_units":1}
                ],
                "max_ticks": 200,
                "max_risk_milli": 1000,
                "allow_instrument_execution": false,
                "approval_reference": null,
                "randomization_seed": artifact_hash,
                "ticks_per_replicate": 2,
                "include_quality_task": true
            }
        }),
    );
    assert_eq!(
        replication_protocol_compile["dispatch"],
        json!("not_started")
    );
    assert_eq!(replication_protocol_compile["simulation_only"], json!(true));
    assert_eq!(
        replication_protocol_compile["compilation"]["disposition"],
        json!("compiled")
    );
    assert!(
        replication_protocol_compile["compilation"]["task_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task.as_str().unwrap().ends_with(":qc"))
    );

    let adaptive_information_campaign = call(
        &mut server,
        "glioma_adaptive_information_campaign",
        json!({
            "request": {
                "objective": "adaptively separate EGFR and matrix invasion mechanisms",
                "model_system": "organoid",
                "max_rounds": 3,
                "max_actions_per_round": 1,
                "budget_units": 6,
                "min_information_gain_milli": 10,
                "information_weight_milli": 800,
                "feasibility_weight_milli": 200,
                "risk_penalty_milli": 100,
                "cost_penalty_milli": 0,
                "risk_ceiling_milli": 700,
                "stop_concentration_milli": 900
            },
            "mechanisms": [
                {"mechanism_id":"egfr","prior_milli":500},
                {"mechanism_id":"matrix","prior_milli":500}
            ],
            "actions": [
                {"action_id":"uninformative","feature_id":"feature-uninformative","label":"uninformative assay","outcomes":[
                    {"outcome_id":"low","label":"low invasion","probability_milli_by_mechanism":{"egfr":500,"matrix":500}},
                    {"outcome_id":"high","label":"high invasion","probability_milli_by_mechanism":{"egfr":500,"matrix":500}}
                ],"feasibility_milli":900,"risk_milli":100,"cost_units":2,"max_replicates":1},
                {"action_id":"separating","feature_id":"feature-separating","label":"separating assay","outcomes":[
                    {"outcome_id":"low","label":"low invasion","probability_milli_by_mechanism":{"egfr":900,"matrix":100}},
                    {"outcome_id":"high","label":"high invasion","probability_milli_by_mechanism":{"egfr":100,"matrix":900}}
                ],"feasibility_milli":900,"risk_milli":100,"cost_units":2,"max_replicates":1}
            ]
        }),
    );
    assert_eq!(
        adaptive_information_campaign["dispatch"],
        json!("not_started")
    );
    assert_eq!(
        adaptive_information_campaign["campaign"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        adaptive_information_campaign["campaign"]["next_action_order"],
        json!(["separating"])
    );

    let adaptive_allocation_campaign = call(
        &mut server,
        "glioma_adaptive_allocation_campaign_execute",
        json!({
            "request": {
                "allocation": {
                    "objective": "allocate organoid invasion replicates",
                    "model_system": "organoid",
                    "control_arm_id": "control",
                    "target_effect_milli": 100,
                    "min_replicates_per_arm": 3,
                    "min_probability_milli": 700,
                    "max_posterior_uncertainty_milli": 200,
                    "exploration_weight_milli": 300,
                    "max_selected_arms": 1,
                    "max_new_replicates": 1,
                    "budget_units": 2,
                    "risk_ceiling_milli": 700
                },
                "arms": [
                    {"arm_id":"control","label":"vehicle control","model_system":"organoid","successes":1,"failures":1,"prior_alpha":1,"prior_beta":1,"risk_milli":100,"cost_units":1,"artifact":{"artifact_id":"control-seed","content_hash":"0000000000000000000000000000000000000000000000000000000000000000","content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                    {"arm_id":"egfr","label":"EGFR perturbation","model_system":"organoid","successes":1,"failures":1,"prior_alpha":1,"prior_beta":1,"risk_milli":100,"cost_units":1,"artifact":{"artifact_id":"egfr-seed","content_hash":"0000000000000000000000000000000000000000000000000000000000000000","content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
                ],
                "max_rounds": 3,
                "max_retries": 1,
                "stop_on_negative": false
            }
        }),
    );
    assert_eq!(
        adaptive_allocation_campaign["dispatch"],
        json!("not_started")
    );
    assert_eq!(adaptive_allocation_campaign["simulation_only"], json!(true));
    assert!(
        !adaptive_allocation_campaign["campaign"]["rounds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !adaptive_allocation_campaign["campaign"]["batches"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let instrument_calibration = call(
        &mut server,
        "glioma_instrument_calibration",
        json!({
            "request": {
                "objective": "qualify imaging control before invasion assay",
                "instrument_id": "imager-1",
                "model_system": "organoid",
                "metric_name": "control_intensity",
                "minimum_runs": 3,
                "reference_run_count": 2,
                "max_reference_mad_milli": 5,
                "max_drift_milli": 20,
                "max_slope_milli_per_tick": 10
            },
            "runs": [
                {"run_id":"cal-r1","sequence_index":1,"batch_id":"cal-b1","instrument_id":"imager-1","metric_name":"control_intensity","model_system":"organoid","observed_milli":500,"expected_milli":500,"artifact":{"artifact_id":"cal-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-control+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"run_id":"cal-r2","sequence_index":2,"batch_id":"cal-b2","instrument_id":"imager-1","metric_name":"control_intensity","model_system":"organoid","observed_milli":502,"expected_milli":500,"artifact":{"artifact_id":"cal-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-control+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}},
                {"run_id":"cal-r3","sequence_index":3,"batch_id":"cal-b3","instrument_id":"imager-1","metric_name":"control_intensity","model_system":"organoid","observed_milli":504,"expected_milli":500,"artifact":{"artifact_id":"cal-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-control+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}}
            ]
        }),
    );
    assert_eq!(instrument_calibration["dispatch"], json!("not_started"));
    assert_eq!(
        instrument_calibration["calibration"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        instrument_calibration["calibration"]["reference_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let instrument_preflight = call(
        &mut server,
        "glioma_instrument_preflight",
        json!({
            "request": {
                "objective": "preflight organoid imaging and wash",
                "instrument_id": "imager-1",
                "model_system": "organoid",
                "actions": [
                    {"action_id":"acquire","instrument_id":"imager-1","operation":"acquire_image","model_system":"organoid","requested_start_tick":1,"duration_ticks":2,"risk_milli":100,"requires_operator":false,"output_schema":"Image1@1","parameters":[]},
                    {"action_id":"wash","instrument_id":"imager-1","operation":"wash","model_system":"organoid","requested_start_tick":3,"duration_ticks":2,"risk_milli":100,"requires_operator":true,"output_schema":"Wash1@1","parameters":[{"name":"volume_microliter","value_milli":10000,"unit":"microliter_milli","minimum_milli":1,"maximum_milli":100000}]}
                ],
                "calibration": instrument_calibration["calibration"].clone(),
                "interlocks": {"observed_tick":1,"emergency_stop_clear":true,"guard_closed":true,"deck_clear":true,"consumables_available":true,"waste_capacity_milli":100000,"temperature_milli":37000,"minimum_temperature_milli":36000,"maximum_temperature_milli":38000,"calibration_valid_until_tick":100,"calibration_sequence_index":3},
                "authorization": {"authorization_id":"approval-1","operator_id":"operator-1","instrument_scope":"imager-1","approval_digest":artifact_hash,"issued_tick":0,"expires_tick":100,"revoked":false},
                "current_tick":1,
                "maximum_total_risk_milli":500,
                "maximum_duration_ticks":20,
                "minimum_waste_capacity_milli":100
            }
        }),
    );
    assert_eq!(instrument_preflight["dispatch"], json!("not_started"));
    assert_eq!(
        instrument_preflight["preflight"]["disposition"],
        json!("admitted")
    );
    assert_eq!(
        instrument_preflight["preflight"]["admitted_order"],
        json!(["acquire", "wash"])
    );

    let instrument_execution_request = json!({
        "objective": "preflight organoid imaging and wash",
        "plan": instrument_preflight["preflight"].clone(),
        "actions": [
            {"action_id":"acquire","instrument_id":"imager-1","operation":"acquire_image","model_system":"organoid","requested_start_tick":1,"duration_ticks":2,"risk_milli":100,"requires_operator":false,"output_schema":"Image1@1","parameters":[]},
            {"action_id":"wash","instrument_id":"imager-1","operation":"wash","model_system":"organoid","requested_start_tick":3,"duration_ticks":2,"risk_milli":100,"requires_operator":true,"output_schema":"Wash1@1","parameters":[{"name":"volume_microliter","value_milli":10000,"unit":"microliter_milli","minimum_milli":1,"maximum_milli":100000}]}
        ],
        "authorization": {"authorization_id":"approval-1","operator_id":"operator-1","instrument_scope":"imager-1","approval_digest":artifact_hash,"issued_tick":0,"expires_tick":100,"revoked":false},
        "live_interlocks": {"observed_tick":1,"emergency_stop_clear":true,"guard_closed":true,"deck_clear":true,"consumables_available":true,"waste_capacity_milli":100000,"temperature_milli":37000,"minimum_temperature_milli":36000,"maximum_temperature_milli":38000,"calibration_valid_until_tick":100,"calibration_sequence_index":3},
        "current_tick":1,
        "minimum_waste_capacity_milli":100,
        "max_retries":1,
        "require_artifacts":true
    });
    let instrument_execution = call(
        &mut server,
        "glioma_instrument_execute",
        json!({"request": instrument_execution_request.clone()}),
    );
    assert_eq!(instrument_execution["dispatch"], json!("not_started"));
    assert_eq!(instrument_execution["simulation_only"], json!(true));
    assert_eq!(
        instrument_execution["execution"]["disposition"],
        json!("completed")
    );
    assert_eq!(
        instrument_execution["execution"]["completed_order"],
        json!(["acquire", "wash"])
    );

    let instrument_assay = call(
        &mut server,
        "glioma_instrument_assay_adjudicate",
        json!({
            "request": {
                "objective": "qualify organoid imaging assay",
                "instrument_id": "imager-1",
                "modality": "imaging",
                "min_qc_milli": 900,
                "min_effect_milli": 500,
                "max_uncertainty_milli": 100,
                "min_replicates": 2,
                "require_negative_control": true,
                "max_negative_control_milli": 100
            },
            "execution": instrument_execution["execution"].clone(),
            "observations": [
                {"action_id":"acquire","artifact":{"artifact_id":"assay-acquire","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma.assay+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"signal_milli":900,"baseline_milli":100,"uncertainty_milli":10,"qc_milli":950,"replicate_count":3,"negative_control_milli":0},
                {"action_id":"wash","artifact":{"artifact_id":"assay-wash","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma.assay+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"signal_milli":850,"baseline_milli":100,"uncertainty_milli":10,"qc_milli":950,"replicate_count":3,"negative_control_milli":0}
            ]
        }),
    );
    assert_eq!(instrument_assay["dispatch"], json!("not_started"));
    assert_eq!(instrument_assay["simulation_only"], json!(true));
    assert_eq!(
        instrument_assay["assessment"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        instrument_assay["assessment"]["evidence_eligible"],
        json!(true)
    );
    assert_eq!(
        instrument_assay["assessment"]["qualified_order"],
        json!(["acquire", "wash"])
    );

    let instrument_campaign = call(
        &mut server,
        "glioma_instrument_campaign_execute",
        json!({
            "request": {
                "objective": "two-stage preclinical imaging campaign",
                "runs": [
                    {"run_id":"stage-a","execution":instrument_execution_request.clone()},
                    {"run_id":"stage-b","execution":instrument_execution_request.clone()}
                ],
                "max_runs": 2,
                "stop_on_negative": true
            }
        }),
    );
    assert_eq!(instrument_campaign["dispatch"], json!("not_started"));
    assert_eq!(instrument_campaign["simulation_only"], json!(true));
    assert_eq!(
        instrument_campaign["campaign"]["disposition"],
        json!("completed")
    );
    assert_eq!(
        instrument_campaign["campaign"]["completed_run_order"],
        json!(["stage-a", "stage-b"])
    );

    let adaptive_instrument_campaign = call(
        &mut server,
        "glioma_adaptive_instrument_campaign_execute",
        json!({
            "request": {
                "objective": "adaptive preclinical imaging campaign",
                "candidates": [
                    {"candidate_id":"adaptive-a","run_id":"adaptive-run-a","endpoint":"viability","execution":instrument_execution_request.clone(),"expected_information_milli":700,"frontier_novelty_milli":500,"reproducibility_milli":900,"estimated_cost_ticks":2,"risk_milli":10,"depends_on":[]},
                    {"candidate_id":"adaptive-b","run_id":"adaptive-run-b","endpoint":"invasion","execution":instrument_execution_request.clone(),"expected_information_milli":700,"frontier_novelty_milli":500,"reproducibility_milli":900,"estimated_cost_ticks":2,"risk_milli":10,"depends_on":["adaptive-a"]},
                    {"candidate_id":"adaptive-c","run_id":"adaptive-run-c","endpoint":"state","execution":instrument_execution_request.clone(),"expected_information_milli":700,"frontier_novelty_milli":500,"reproducibility_milli":900,"estimated_cost_ticks":2,"risk_milli":10,"depends_on":[]}
                ],
                "cost_budget_ticks":8,
                "risk_budget_milli":100,
                "minimum_information_milli":1000,
                "minimum_endpoint_count":2,
                "max_selected":3,
                "stop_on_negative":true
            }
        }),
    );
    assert_eq!(
        adaptive_instrument_campaign["dispatch"],
        json!("not_started")
    );
    assert_eq!(adaptive_instrument_campaign["simulation_only"], json!(true));
    assert_eq!(
        adaptive_instrument_campaign["campaign"]["disposition"],
        json!("executed")
    );
    assert_eq!(
        adaptive_instrument_campaign["campaign"]["selected_order"],
        json!(["adaptive-a", "adaptive-b", "adaptive-c"])
    );

    let instrument_operating_cycle = call(
        &mut server,
        "glioma_instrument_operating_cycle",
        json!({
            "request": {
                "campaign": {
                    "objective": "two-stage preclinical imaging campaign",
                    "runs": [
                        {"run_id":"stage-a","execution":instrument_execution_request.clone()},
                        {"run_id":"stage-b","execution":instrument_execution_request.clone()}
                    ],
                    "max_runs": 2,
                    "stop_on_negative": true
                },
                "require_all_admitted": true,
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(instrument_operating_cycle["dispatch"], json!("not_started"));
    assert_eq!(instrument_operating_cycle["simulation_only"], json!(true));
    assert_eq!(
        instrument_operating_cycle["cycle"]["disposition"],
        json!("executed")
    );
    assert_eq!(
        instrument_operating_cycle["cycle"]["campaign"]["completed_run_order"],
        json!(["stage-a", "stage-b"])
    );
    assert_eq!(
        instrument_operating_cycle["cycle"]["preflight"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let computation_execution = call(
        &mut server,
        "glioma_computation_execute",
        json!({
            "request": {
                "objective": "replay a glioma organoid multimodal computation",
                "model_system": "organoid",
                "tasks": [
                    {"task_id":"fit","operation":"model_fit","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["input:fit"],"output_schema":"Fit1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},
                    {"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true}
                ],
                "replay_identity": artifact_hash,
                "max_budget_units": 10,
                "max_retries": 1,
                "allow_cache": true,
                "require_local_artifacts": true,
                "cache": []
            }
        }),
    );
    assert_eq!(computation_execution["dispatch"], json!("not_started"));
    assert_eq!(
        computation_execution["execution"]["disposition"],
        json!("completed")
    );
    assert_eq!(
        computation_execution["execution"]["task_order"],
        json!(["normalize", "fit"])
    );

    let evidence_surveillance = call(
        &mut server,
        "glioma_evidence_surveillance",
        json!({
            "request": {
                "objective": "monitor invasion evidence",
                "required_modalities": ["genomics"],
                "required_model_systems": ["organoid"],
                "min_priority_milli": 500,
                "max_actions": 8,
                "score_shift_threshold_milli": 50
            },
            "previous": [{
                "evidence_id": "mcp-surveillance-1",
                "source_artifact": {"artifact_id":"mcp-surveillance-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
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
            "current": [
                {
                    "evidence_id": "mcp-surveillance-1",
                    "source_artifact": {"artifact_id":"mcp-surveillance-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "preclinical glioma",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "contradicted",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 2
                },
                {
                    "evidence_id": "mcp-surveillance-2",
                    "source_artifact": {"artifact_id":"mcp-surveillance-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "Matrix remodeling changes invasion",
                    "scope": "preclinical glioma",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 850,
                    "quality_milli": 850,
                    "reproducibility_milli": 850,
                    "release_epoch": 2
                }
            ]
        }),
    );
    assert_eq!(evidence_surveillance["dispatch"], json!("not_started"));
    assert_eq!(
        evidence_surveillance["surveillance"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        evidence_surveillance["surveillance"]["actions"][0]["kind"],
        json!("investigate_contradiction")
    );

    let evidence_triangulation = call(
        &mut server,
        "glioma_evidence_triangulate",
        json!({
            "request": {
                "objective": "triangulate EGFR invasion evidence",
                "min_source_kinds": 3,
                "min_independent_artifacts": 3,
                "min_support_milli": 600,
                "max_contradiction_milli": 200,
                "min_diversity_milli": 1000,
                "max_leave_one_artifact_shift_milli": 100,
                "max_claims": 8
            },
            "records": [
                {
                    "evidence_id": "mcp-triangulation-1",
                    "source_artifact": {"artifact_id":"mcp-triangulation-artifact-1","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "literature",
                    "claim": "EGFR signaling increases organoid invasion",
                    "scope": "organoid:invasion",
                    "modality": "functional_perturbation",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                },
                {
                    "evidence_id": "mcp-triangulation-2",
                    "source_artifact": {"artifact_id":"mcp-triangulation-artifact-2","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "assay",
                    "claim": "EGFR signaling increases organoid invasion",
                    "scope": "organoid:invasion",
                    "modality": "functional_perturbation",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                },
                {
                    "evidence_id": "mcp-triangulation-3",
                    "source_artifact": {"artifact_id":"mcp-triangulation-artifact-3","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "replication",
                    "claim": "EGFR signaling increases organoid invasion",
                    "scope": "organoid:invasion",
                    "modality": "functional_perturbation",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                }
            ]
        }),
    );
    assert_eq!(evidence_triangulation["dispatch"], json!("not_started"));
    assert_eq!(
        evidence_triangulation["triangulation"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        evidence_triangulation["triangulation"]["qualified_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        evidence_triangulation["triangulation"]["claims"][0]["independent_artifact_count"],
        json!(3)
    );

    let evidence_priority = call(
        &mut server,
        "glioma_evidence_priority",
        json!({
            "request": {
                "objective": "rank invasion evidence refreshes",
                "current_epoch": 10,
                "recency_half_life_epochs": 4,
                "required_modalities": [],
                "required_model_systems": [],
                "max_actions": 8,
                "min_priority_milli": 0,
                "weights": {
                    "recency_milli": 100,
                    "state_pressure_milli": 300,
                    "quality_milli": 100,
                    "relevance_milli": 150,
                    "reproducibility_milli": 150,
                    "coverage_debt_milli": 200
                }
            },
            "records": [{
                "evidence_id": "mcp-priority-1",
                "source_artifact": {"artifact_id":"mcp-priority-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
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
            }]
        }),
    );
    assert_eq!(evidence_priority["dispatch"], json!("not_started"));
    assert_eq!(
        evidence_priority["priority"]["actions"][0]["kind"],
        json!("refresh_stale")
    );

    let evidence_acquisition = call(
        &mut server,
        "glioma_evidence_acquisition_plan",
        json!({
            "request": {
                "objective": "close invasion evidence debt",
                "budget_units": 6,
                "max_candidates": 8,
                "max_selected": 3,
                "beam_width": 8,
                "min_source_families": 2,
                "max_per_independence_group": 2,
                "max_privacy_risk_milli": 500,
                "min_portfolio_score_milli": 100,
                "required_modalities": ["genomics", "imaging"],
                "required_model_systems": ["organoid"],
                "weights": {
                    "support_milli": 180,
                    "uncertainty_reduction_milli": 180,
                    "contradiction_resolution_milli": 160,
                    "freshness_milli": 90,
                    "workflow_leverage_milli": 150,
                    "reproducibility_milli": 120,
                    "failure_penalty_milli": 70,
                    "cost_penalty_milli": 50
                }
            },
            "candidates": [
                {"candidate_id":"literature-a","target_claim":"EGFR signaling increases invasion","source_family":"pubmed","source_kind":"literature","modality":"genomics","model_system":"organoid","independence_group":"pubmed","depends_on":[],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                {"candidate_id":"dataset-b","target_claim":"EGFR signaling increases invasion","source_family":"atlas","source_kind":"dataset","modality":"imaging","model_system":"organoid","independence_group":"atlas","depends_on":["literature-a"],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                {"candidate_id":"replicate-c","target_claim":"EGFR signaling increases invasion","source_family":"consortium","source_kind":"replication","modality":"imaging","model_system":"organoid","independence_group":"consortium","depends_on":[],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false}
            ]
        }),
    );
    assert_eq!(evidence_acquisition["dispatch"], json!("not_started"));
    assert_eq!(evidence_acquisition["preflight_required"], json!(true));
    assert_eq!(
        evidence_acquisition["acquisition"]["disposition"],
        json!("ready")
    );
    assert!(
        evidence_acquisition["acquisition"]["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "literature-a")
    );

    let evidence_acquisition_campaign = call(
        &mut server,
        "glioma_evidence_acquisition_campaign_execute",
        json!({
            "request": {
                "objective": "close invasion evidence debt",
                "plan": evidence_acquisition["acquisition"].clone(),
                "candidates": [
                    {"candidate_id":"literature-a","target_claim":"EGFR signaling increases invasion","source_family":"pubmed","source_kind":"literature","modality":"genomics","model_system":"organoid","independence_group":"pubmed","depends_on":[],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                    {"candidate_id":"dataset-b","target_claim":"EGFR signaling increases invasion","source_family":"atlas","source_kind":"dataset","modality":"imaging","model_system":"organoid","independence_group":"atlas","depends_on":["literature-a"],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                    {"candidate_id":"replicate-c","target_claim":"EGFR signaling increases invasion","source_family":"consortium","source_kind":"replication","modality":"imaging","model_system":"organoid","independence_group":"consortium","depends_on":[],"cost_units":2,"expected_support_milli":800,"expected_uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"freshness_milli":600,"workflow_leverage_milli":700,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false}
                ],
                "budget_units": 6,
                "max_retries": 2,
                "stop_on_negative": true,
                "require_artifacts": true
            }
        }),
    );
    assert_eq!(evidence_acquisition_campaign["dispatch"], json!("dry_run"));
    assert_eq!(
        evidence_acquisition_campaign["simulation_only"],
        json!(true)
    );
    assert!(
        !evidence_acquisition_campaign["campaign"]["unknown_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let priority_action_id = evidence_priority["priority"]["selected_order"][0]
        .as_str()
        .unwrap()
        .to_string();
    let evidence_campaign = call(
        &mut server,
        "glioma_evidence_campaign_execute",
        json!({
            "request": {
                "objective": "rank invasion evidence refreshes",
                "priority": evidence_priority["priority"].clone(),
                "candidates": [{
                    "action_id": priority_action_id,
                    "stage_kind": "evidence_surveillance",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "depends_on": [],
                    "cost_units": 1,
                    "information_gain_milli": 900,
                    "frontier_novelty_milli": 800,
                    "workflow_leverage_milli": 800,
                    "cross_stage_unlock_milli": 800,
                    "reproducibility_safety_milli": 900,
                    "federation_value_milli": 500,
                    "feasibility_milli": 900,
                    "autonomy_tier": "a1",
                    "effects": ["read_local_data", "execute_local_computation", "write_local_artifact"]
                }],
                "completed_action_order": [],
                "selection": {
                    "budget_units": 2,
                    "max_actions": 1,
                    "approval_granted": true,
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
    assert_eq!(evidence_campaign["dispatch"], json!("not_started"));
    assert_eq!(
        evidence_campaign["campaign"]["disposition"],
        json!("completed")
    );

    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "required_modalities": ["genomics"],
                "required_model_systems": ["organoid"],
                "min_support_milli": 700,
                "min_sources_per_claim": 1,
                "max_claims": 8
            },
            "records": [{
                "evidence_id": "mcp-evidence-1",
                "source_artifact": {"artifact_id":"mcp-evidence-artifact","content_hash":artifact_hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
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
            }]
        }),
    );
    assert_eq!(knowledge["dispatch"], json!("not_started"));
    assert_eq!(knowledge["knowledge"]["disposition"], json!("qualified"));

    let knowledge_frontier = call(
        &mut server,
        "glioma_knowledge_frontier",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "max_selected_claims": 8,
                "min_priority_milli": 0,
                "weights": {
                    "coverage_debt_milli": 250,
                    "contradiction_milli": 250,
                    "uncertainty_milli": 200,
                    "support_milli": 150,
                    "workflow_leverage_milli": 150
                }
            },
            "knowledge": knowledge["knowledge"].clone()
        }),
    );
    assert_eq!(knowledge_frontier["dispatch"], json!("not_started"));
    assert_eq!(
        knowledge_frontier["frontier"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let knowledge_gap_portfolio = call(
        &mut server,
        "glioma_knowledge_gap_compile",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "max_claims": 8,
                "max_candidates": 16,
                "max_candidates_per_claim": 8,
                "max_template_cost_units": 10,
                "min_frontier_priority_milli": 0,
                "templates": [
                    {"template_id":"imaging-atlas","source_family":"atlas","source_kind":"dataset","modality":"imaging","model_system":"organoid","cost_units":2,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                    {"template_id":"replication-site","source_family":"consortium","source_kind":"replication","modality":null,"model_system":null,"cost_units":3,"reproducibility_milli":900,"failure_probability_milli":150,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false}
                ]
            },
            "knowledge": knowledge["knowledge"].clone(),
            "frontier": knowledge_frontier["frontier"].clone()
        }),
    );
    assert_eq!(knowledge_gap_portfolio["dispatch"], json!("not_started"));
    assert_eq!(
        knowledge_gap_portfolio["next_route"],
        json!("glioma_evidence_acquisition_plan")
    );
    assert!(
        !knowledge_gap_portfolio["portfolio"]["candidates"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let autonomous_gap_cycle = call(
        &mut server,
        "glioma_autonomous_gap_cycle",
        json!({
            "request": {
                "gap": {
                    "objective": "rank invasion mechanisms",
                    "max_claims": 8,
                    "max_candidates": 16,
                    "max_candidates_per_claim": 8,
                    "max_template_cost_units": 10,
                    "min_frontier_priority_milli": 0,
                    "templates": [
                        {"template_id":"imaging-atlas","source_family":"atlas","source_kind":"dataset","modality":"imaging","model_system":"organoid","cost_units":2,"reproducibility_milli":800,"failure_probability_milli":100,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false},
                        {"template_id":"replication-site","source_family":"consortium","source_kind":"replication","modality":null,"model_system":null,"cost_units":3,"reproducibility_milli":900,"failure_probability_milli":150,"privacy_risk_milli":50,"local_only":true,"contains_human_data":false}
                    ]
                },
                "planning": {
                    "objective": "rank invasion mechanisms",
                    "budget_units": 12,
                    "max_candidates": 16,
                    "max_selected": 4,
                    "beam_width": 16,
                    "min_source_families": 1,
                    "max_per_independence_group": 2,
                    "max_privacy_risk_milli": 1000,
                    "min_portfolio_score_milli": 0,
                    "required_modalities": ["imaging"],
                    "required_model_systems": ["organoid"],
                    "weights": {
                        "support_milli": 180,
                        "uncertainty_reduction_milli": 180,
                        "contradiction_resolution_milli": 160,
                        "freshness_milli": 90,
                        "workflow_leverage_milli": 150,
                        "reproducibility_milli": 120,
                        "failure_penalty_milli": 70,
                        "cost_penalty_milli": 50
                    }
                },
                "execution_budget_units": 12,
                "execution_max_retries": 1,
                "stop_on_negative": false,
                "require_artifacts": true
            },
            "knowledge": knowledge["knowledge"].clone(),
            "frontier": knowledge_frontier["frontier"].clone()
        }),
    );
    assert_eq!(autonomous_gap_cycle["dispatch"], json!("dry_run"));
    assert_eq!(autonomous_gap_cycle["simulation_only"], json!(true));
    assert_eq!(
        autonomous_gap_cycle["cycle"]["phase_order"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let decision_context = call(
        &mut server,
        "glioma_decision_context",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "max_actions": 8,
                "default_cost_units": 5
            },
            "knowledge": knowledge["knowledge"].clone()
        }),
    );
    assert_eq!(decision_context["dispatch"], json!("not_started"));
    assert_eq!(
        decision_context["context"]["actions"][0]["candidate"]["stage_kind"],
        json!("mechanism_exploration")
    );

    let decision_admission = call(
        &mut server,
        "glioma_decision_admission_gate",
        json!({
            "request": {
                "objective": "admit a bounded glioma computation",
                "actions": [{
                    "action_id":"compute-glioma-state",
                    "stage_kind":"computational_execution",
                    "modality":"genomics",
                    "model_system":"organoid",
                    "depends_on":[],
                    "cost_units":3,
                    "autonomy_tier":"a1",
                    "effects":["execute_local_computation"],
                    "evidence_milli":900,
                    "freshness_milli":900,
                    "coverage_milli":900,
                    "contradiction_milli":50,
                    "reproducibility_milli":900,
                    "approval_granted":false,
                    "signed_preflight":true,
                    "local_only":true
                }],
                "budget_units":10,
                "max_admitted_actions":4,
                "require_dependency_closure":true,
                "min_evidence_milli":700,
                "min_freshness_milli":700,
                "min_coverage_milli":700,
                "max_contradiction_milli":200,
                "min_reproducibility_milli":700,
                "allow_instrument_execution":false,
                "allow_federation_export":false
            }
        }),
    );
    assert_eq!(decision_admission["dispatch"], json!("not_started"));
    assert_eq!(decision_admission["simulation_only"], json!(true));
    assert_eq!(
        decision_admission["admission"]["disposition"],
        json!("ready")
    );
    assert_eq!(
        decision_admission["admission"]["admitted_order"],
        json!(["compute-glioma-state"])
    );

    let decision_value = call(
        &mut server,
        "glioma_decision_value_optimizer",
        json!({
            "request": {
                "objective": "choose the next glioma research portfolio by expected information value",
                "candidates": [
                    {"action_id":"a-genomics","claim_id":"claim-invasion","modality":"genomics","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"genomics","available":true},
                    {"action_id":"b-imaging","claim_id":"claim-invasion","modality":"imaging","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"imaging","available":true},
                    {"action_id":"c-expensive","claim_id":"claim-invasion","modality":"proteomics","model_system":"organoid","depends_on":[],"cost_units":8,"information_gain_milli":900,"uncertainty_reduction_milli":800,"contradiction_resolution_milli":700,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"proteomics","available":true}
                ],
                "budget_units":6,
                "max_actions":2,
                "beam_width":8,
                "max_alternatives":3,
                "require_dependency_closure":true,
                "min_candidate_utility_milli":100,
                "weights":{"information_gain":30,"uncertainty_reduction":25,"contradiction_resolution":20,"reproducibility":15,"diversity":10,"failure_penalty":10}
            }
        }),
    );
    assert_eq!(decision_value["dispatch"], json!("not_started"));
    assert_eq!(decision_value["simulation_only"], json!(true));
    assert_eq!(
        decision_value["optimization"]["disposition"],
        json!("partial")
    );
    assert_eq!(
        decision_value["optimization"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let decision_value_calibration = call(
        &mut server,
        "glioma_decision_value_calibrator",
        json!({
            "request": {
                "objective": "calibrate the next glioma research portfolio from local outcomes",
                "candidates": [
                    {"action_id":"a-genomics","claim_id":"claim-invasion","modality":"genomics","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"genomics","available":true},
                    {"action_id":"b-imaging","claim_id":"claim-invasion","modality":"imaging","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"imaging","available":true}
                ],
                "observations": [{"action_id":"a-genomics","run_id":"run-001","predicted_utility_milli":700,"observed_information_gain_milli":900,"observed_uncertainty_reduction_milli":800,"observed_contradiction_resolution_milli":700,"observed_reproducibility_milli":900,"failed":false,"sample_weight":2}],
                "weights":{"information_gain":30,"uncertainty_reduction":25,"contradiction_resolution":20,"reproducibility":15,"diversity":10,"failure_penalty":10},
                "shrinkage_weight":10,
                "min_confidence_milli":100,
                "conflict_error_milli":3000,
                "max_results":2
            }
        }),
    );
    assert_eq!(decision_value_calibration["dispatch"], json!("not_started"));
    assert_eq!(decision_value_calibration["simulation_only"], json!(true));
    assert_eq!(
        decision_value_calibration["calibration"]["disposition"],
        json!("ready")
    );
    assert_eq!(
        decision_value_calibration["calibration"]["ranking_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let adaptive_decision = call(
        &mut server,
        "glioma_adaptive_decision_controller",
        json!({
            "request": {
                "calibration": {
                    "objective": "adapt the next glioma research portfolio from local outcomes",
                    "candidates": [
                        {"action_id":"a-genomics","claim_id":"claim-invasion","modality":"genomics","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"genomics","available":true},
                        {"action_id":"b-imaging","claim_id":"claim-invasion","modality":"imaging","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"uncertainty_reduction_milli":700,"contradiction_resolution_milli":600,"reproducibility_milli":900,"failure_probability_milli":50,"diversity_group":"imaging","available":true}
                    ],
                    "observations": [{"action_id":"a-genomics","run_id":"run-001","predicted_utility_milli":745,"observed_information_gain_milli":900,"observed_uncertainty_reduction_milli":800,"observed_contradiction_resolution_milli":700,"observed_reproducibility_milli":900,"failed":false,"sample_weight":2}],
                    "weights":{"information_gain":30,"uncertainty_reduction":25,"contradiction_resolution":20,"reproducibility":15,"diversity":10,"failure_penalty":10},
                    "shrinkage_weight":10,
                    "min_confidence_milli":100,
                    "conflict_error_milli":3000,
                    "max_results":2
                },
                "budget_units":6,
                "max_actions":2,
                "beam_width":8,
                "max_alternatives":3,
                "require_dependency_closure":true,
                "exploration_weight_milli":400,
                "min_controller_utility_milli":100
            }
        }),
    );
    assert_eq!(adaptive_decision["dispatch"], json!("not_started"));
    assert_eq!(adaptive_decision["simulation_only"], json!(true));
    assert_eq!(adaptive_decision["control"]["disposition"], json!("ready"));
    assert_eq!(
        adaptive_decision["control"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let decision_loop = call(
        &mut server,
        "glioma_decision_loop_governor",
        json!({
            "request": {
                "objective": "govern a bounded glioma research loop",
                "rounds": [
                    {"round_index":1,"action_order":["action-01"],"cost_units":3,"observed_information_gain_milli":800,"uncertainty_reduction_milli":400,"failure_count":0,"negative_count":0,"contradiction_count":0,"completed":true,"evidence_complete":true,"human_review_available":false}
                ],
                "budget_units":12,
                "max_rounds":4,
                "min_progress_milli":500,
                "min_gain_milli":250,
                "max_failures":3,
                "require_negative_visibility":true,
                "allow_continue_on_partial":false
            }
        }),
    );
    assert_eq!(decision_loop["dispatch"], json!("not_started"));
    assert_eq!(decision_loop["simulation_only"], json!(true));
    assert_eq!(
        decision_loop["governance"]["stop_reason"],
        json!("qualified")
    );
    assert_eq!(decision_loop["governance"]["next_round"], Value::Null);

    let decision_action_plan = call(
        &mut server,
        "glioma_decision_action_plan",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "completed_action_order": [],
                "selection": {
                    "budget_units": 10,
                    "max_actions": 1,
                    "approval_granted": true,
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
                }
            },
            "context": decision_context["context"].clone()
        }),
    );
    assert_eq!(decision_action_plan["dispatch"], json!("not_started"));
    assert_eq!(
        decision_action_plan["plan"]["disposition"],
        json!("qualified")
    );
    assert_eq!(
        decision_action_plan["plan"]["selected_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let autopilot = call(
        &mut server,
        "glioma_research_autopilot_execute",
        json!({
            "request": {
                "objective": "rank invasion mechanisms",
                "context": decision_context["context"].clone(),
                "completed_action_order": [],
                "selection": {
                    "budget_units": 10,
                    "max_actions": 1,
                    "approval_granted": true,
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
    assert_eq!(autopilot["dispatch"], json!("not_started"));
    assert_eq!(autopilot["simulation_only"], json!(true));
    assert_eq!(autopilot["run"]["disposition"], json!("completed"));
}
