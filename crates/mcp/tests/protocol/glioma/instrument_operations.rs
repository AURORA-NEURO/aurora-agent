//! MCP contract tests for the instrument operations area.

use super::*;

#[test]
fn glioma_instrument_fleet_schedule_closes_dependencies_and_requires_preflight() {
    let mut server = server();
    let schedule = call(
        &mut server,
        "glioma_instrument_fleet_schedule",
        json!({
            "request": {
                "objective": "coordinate organoid imaging and sequencing",
                "model_system": "organoid",
                "current_tick": 0,
                "max_end_tick": 30,
                "maximum_total_risk_milli": 1000,
                "operator_capacity": 1,
                "tasks": [
                    {"task_id":"image","label":"image","operation":"acquire_image","model_system":"organoid","candidate_instrument_order":["scope-a","scope-b"],"depends_on":[],"release_tick":0,"duration_ticks":5,"risk_milli":100,"information_milli":500,"requires_operator":false,"output_schema":"Image1@1"},
                    {"task_id":"sequence","label":"sequence","operation":"sequence","model_system":"organoid","candidate_instrument_order":["scope-a","scope-b"],"depends_on":[],"release_tick":0,"duration_ticks":5,"risk_milli":100,"information_milli":500,"requires_operator":false,"output_schema":"Sequence1@1"},
                    {"task_id":"integrate","label":"integrate","operation":"acquire_image","model_system":"organoid","candidate_instrument_order":["scope-a","scope-b"],"depends_on":["image","sequence"],"release_tick":0,"duration_ticks":5,"risk_milli":100,"information_milli":500,"requires_operator":false,"output_schema":"Integrate1@1"}
                ],
                "resources": [
                    {"instrument_id":"scope-a","model_system_order":["organoid"],"operation_order":["acquire_image","sequence"],"available_from_tick":0,"available_until_tick":30,"calibration_valid_until_tick":30,"enabled":true},
                    {"instrument_id":"scope-b","model_system_order":["organoid"],"operation_order":["acquire_image","sequence"],"available_from_tick":0,"available_until_tick":30,"calibration_valid_until_tick":30,"enabled":true}
                ]
            }
        }),
    );
    assert_eq!(schedule["dispatch"], json!("not_started"));
    assert_eq!(schedule["preflight_required"], json!(true));
    assert_eq!(schedule["schedule"]["disposition"], json!("ready"));
    assert_eq!(
        schedule["schedule"]["assignments"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(schedule["schedule"]["dispatch_permitted"], json!(false));
    assert!(
        schedule["schedule"]["critical_path_order"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "integrate")
    );
}

#[test]
fn glioma_instrument_fleet_execute_preserves_schedule_blocks_without_dispatch() {
    let mut server = server();
    let schedule = call(
        &mut server,
        "glioma_instrument_fleet_schedule",
        json!({
            "request": {
                "objective": "execute blocked organoid imaging fleet",
                "model_system": "organoid",
                "current_tick": 0,
                "max_end_tick": 30,
                "maximum_total_risk_milli": 1000,
                "operator_capacity": 1,
                "tasks": [
                    {"task_id":"image-blocked","label":"image blocked","operation":"acquire_image","model_system":"organoid","candidate_instrument_order":["scope-a"],"depends_on":[],"release_tick":0,"duration_ticks":5,"risk_milli":100,"information_milli":500,"requires_operator":false,"output_schema":"ImageBlocked1@1"}
                ],
                "resources": [
                    {"instrument_id":"scope-a","model_system_order":["organoid"],"operation_order":["sequence"],"available_from_tick":0,"available_until_tick":30,"calibration_valid_until_tick":30,"enabled":true}
                ]
            }
        }),
    );
    assert_eq!(schedule["schedule"]["disposition"], json!("blocked"));
    let execution = call(
        &mut server,
        "glioma_instrument_fleet_execute",
        json!({
            "request": {
                "objective": "execute blocked organoid imaging fleet",
                "schedule": schedule["schedule"].clone(),
                "runs": [],
                "stop_on_negative": true
            }
        }),
    );
    assert_eq!(execution["dispatch"], json!("not_started"));
    assert_eq!(execution["simulation_only"], json!(true));
    assert_eq!(execution["execution"]["disposition"], json!("blocked"));
    assert_eq!(
        execution["execution"]["blocked_order"],
        json!(["image-blocked"])
    );
}

#[test]
fn glioma_instrument_fleet_health_is_reachable_through_mcp() {
    let mut server = server();
    let summary = |instrument_id: &str,
                   tick: u64,
                   throughput: u16,
                   qc: u16,
                   calibration: u16,
                   downtime: u16| {
        let body = json!({
            "instrument_id": instrument_id,
            "site_id": "site-a",
            "tick": tick,
            "throughput_milli": throughput,
            "qc_pass_milli": qc,
            "downtime_milli": downtime,
            "calibration_error_milli": calibration,
            "run_count": 10
        });
        let mut observation = body.clone();
        observation["summary_digest"] = json!(bioprism_ids::ContentHash::of_value(&body)
            .unwrap()
            .to_string());
        observation
    };
    let output = call(
        &mut server,
        "glioma_instrument_fleet_health",
        json!({
            "request": {
                "fleet_id": "glioma-fleet",
                "observation_order": [
                    summary("scope-1", 1, 900, 950, 50, 0),
                    summary("scope-1", 2, 900, 950, 50, 0),
                    summary("scope-1", 3, 700, 700, 220, 1),
                    summary("scope-1", 4, 650, 650, 250, 1)
                ],
                "baseline_window_count": 2,
                "recent_window_count": 2,
                "minimum_observations": 4,
                "drift_threshold_milli": 100,
                "qc_failure_threshold_milli": 800,
                "downtime_cluster_threshold": 2,
                "max_alerts": 8,
                "current_tick": 10,
                "mask_site_identity": true
            }
        }),
    );
    assert_eq!(
        output["assessment"]["feature_id"],
        json!("GAF-GLIOMA-P08-F30")
    );
    assert_eq!(output["assessment"]["fleet_disposition"], json!("blocked"));
    assert_eq!(
        output["assessment"]["assessments"][0]["site_label"],
        Value::Null
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_acquisition_capacity_plan_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_acquisition_capacity_plan",
        json!({
            "request": {
                "objective": "allocate glioma organoid acquisition capacity",
                "current_tick": 1,
                "horizon_ticks": 10,
                "total_budget_units": 8,
                "demands": [
                    {"campaign_id":"campaign-a","priority_milli":1000,"fairness_weight_milli":1000,"minimum_units":2,"target_units":4,"maximum_units":6,"cost_per_unit":1,"latest_tick":11,"approved":true},
                    {"campaign_id":"campaign-b","priority_milli":100,"fairness_weight_milli":1000,"minimum_units":2,"target_units":4,"maximum_units":6,"cost_per_unit":1,"latest_tick":11,"approved":true}
                ],
                "resources": [{"resource_id":"scope-1","capacity_units":8,"operator_capacity_units":8,"maintenance_reserved_units":1,"budget_units":8,"enabled":true}]
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P08-F31"));
    assert!(output["plan"]["minimum_fairness_milli"].as_u64().unwrap() >= 500);
    assert_eq!(output["plan"]["dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_instrument_maintenance_plan_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_instrument_maintenance_plan",
        json!({
            "request": {
                "current_tick": 10,
                "horizon_end_tick": 30,
                "minimum_health_milli": 500,
                "devices": [{
                    "instrument_id":"scope-1",
                    "service_due_tick":12,
                    "calibration_valid_until_tick":25,
                    "maintenance_duration_ticks":2,
                    "health_score_milli":900,
                    "enabled":true
                }],
                "reservations": [{
                    "reservation_id":"run-a",
                    "instrument_id":"scope-1",
                    "start_tick":12,
                    "end_tick":16
                }]
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P08-F29"));
    assert_eq!(output["plan"]["windows"][0]["window_start_tick"], json!(16));
    assert_eq!(
        output["plan"]["windows"][0]["disposition"],
        json!("scheduled")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_assay_provenance_audit_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |value: &Value| {
        bioprism_ids::ContentHash::of_value(value)
            .unwrap()
            .to_string()
    };
    let lineage = hash(&json!("lineage-a"));
    let scope = hash(&json!("scope-a"));
    let protocol = hash(&json!("protocol-a"));
    let operator = hash(&json!("operator-a"));
    let artifact = hash(&json!("artifact-a"));
    let output = call(
        &mut server,
        "glioma_assay_provenance_audit",
        json!({
            "request": {
                "runs": [{
                    "run_id":"run-a",
                    "sample_lineage_digest":lineage,
                    "expected_sample_scope_digest":scope,
                    "observed_sample_scope_digest":scope,
                    "approved_protocol_digest":protocol,
                    "observed_protocol_digest":protocol,
                    "device_id":"device-a",
                    "calibration_valid_until_tick":200,
                    "operator_authority_digest":operator,
                    "observed_clock_tick":100,
                    "artifact_manifest_digest":artifact,
                    "artifact_predecessor_digest":null,
                    "artifact_sequence":0,
                    "lifecycle_status":"completed"
                }],
                "current_tick":100,
                "max_clock_skew_ticks":10,
                "require_operator_authority":true,
                "require_artifact_continuity":true
            }
        }),
    );
    assert_eq!(output["audit"]["feature_id"], json!("GAF-GLIOMA-P08-F27"));
    assert_eq!(
        output["audit"]["analysis_admission_order"],
        json!(["run-a"])
    );
    assert_eq!(
        output["audit"]["results"][0]["disposition"],
        json!("verified")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_acquisition_operations_snapshot_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_acquisition_operations_snapshot",
        json!({
            "request": {
                "items": [{
                    "acquisition_id":"item-a",
                    "campaign_id":"campaign-a",
                    "priority_milli":900,
                    "fairness_weight_milli":1000,
                    "submitted_tick":1,
                    "deadline_tick":30,
                    "requested_units":5,
                    "completed_units":0,
                    "assigned_device_id":"device-a",
                    "preflight_state":"ready",
                    "estimated_duration_ticks":4,
                    "approved":true
                }],
                "devices": [{
                    "device_id":"device-a",
                    "next_free_tick":10,
                    "calibration_valid_until_tick":40,
                    "operator_load_units":1,
                    "operator_capacity_units":4,
                    "last_observed_tick":10,
                    "enabled":true
                }],
                "current_tick":10,
                "horizon_ticks":30,
                "telemetry_stale_after_ticks":5,
                "max_reorder_proposals":4,
                "minimum_fairness_milli":500
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P08-F19")
    );
    assert_eq!(output["snapshot"]["queue_risk"], json!("healthy"));
    assert_eq!(output["snapshot"]["dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_instrument_operator_approval_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |value: &Value| {
        bioprism_ids::ContentHash::of_value(value)
            .unwrap()
            .to_string()
    };
    let output = call(
        &mut server,
        "glioma_instrument_operator_approval",
        json!({
            "request": {
                "approval_id":"approval-a",
                "plan_digest":hash(&json!("plan-a")),
                "device_id":"device-a",
                "sample_scope_digest":hash(&json!("scope-a")),
                "operator_id":"operator-a",
                "operator_authority_digest":hash(&json!("authority-a")),
                "issued_tick":10,
                "expires_tick":30,
                "current_tick":12,
                "effect_order":["capture","save"],
                "interlock_order":[{"interlock_id":"guard","state":"clear","observed_tick":12}],
                "uncertainty_milli":100,
                "maximum_uncertainty_milli":500,
                "operator_confirmed":true,
                "revoked":false,
                "already_consumed":false,
                "single_use":true,
                "stop_path_order":["stop-gateway","notify-operator"]
            }
        }),
    );
    assert_eq!(
        output["approval"]["feature_id"],
        json!("GAF-GLIOMA-P08-F17")
    );
    assert_eq!(output["approval"]["disposition"], json!("approved"));
    assert_eq!(output["approval"]["dispatch_permitted"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}
