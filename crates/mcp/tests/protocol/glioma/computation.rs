//! MCP contract tests for the computation area.

use super::*;

#[test]
fn glioma_compute_environment_lock_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_compute_environment_lock",
        json!({
            "request": {
                "objective": "qualify a local glioma computation environment",
                "workflow_manifest_digest": hash,
                "workflow_task_order": ["model", "normalize"],
                "architecture": {
                    "os_family": "linux",
                    "os_version": "6.8",
                    "architecture": "x86_64",
                    "abi": "gnu",
                    "cpu_feature_order": ["avx2"],
                    "accelerator_order": ["none"],
                    "host_digest": hash
                },
                "trusted_source_order": ["registry://trusted"],
                "require_signed_metadata": true,
                "require_portable_dependencies": true,
                "dependencies": [{
                    "name": "numpy",
                    "kind": "library",
                    "version_constraint": "=2.1.0",
                    "resolved_version": "2.1.0",
                    "source": "registry://trusted",
                    "source_digest": hash,
                    "build_digest": hash,
                    "runtime_abi": "abi-v1",
                    "required": true,
                    "available": true,
                    "portable": true,
                    "metadata_signed": true,
                    "source_mutable": false,
                    "compromised": false,
                    "compatible_architecture_order": ["x86_64"],
                    "contains_human_data": false,
                    "contains_clinical_decision": false
                }]
            }
        }),
    );
    assert_eq!(output["lock"]["feature_id"], json!("GAF-GLIOMA-P09-F06"));
    assert_eq!(output["lock"]["disposition"], json!("qualified"));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_environment_resolution_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_environment_resolution",
        json!({
            "request": {
                "objective": "repair a missing local glioma dependency",
                "approval_granted": true,
                "allow_version_changes": false,
                "allow_source_changes": false,
                "max_changes": 2,
                "max_cost_units": 4,
                "base": {
                    "objective": "qualify a local glioma computation environment",
                    "workflow_manifest_digest": hash,
                    "workflow_task_order": ["model", "normalize"],
                    "architecture": {
                        "os_family": "linux",
                        "os_version": "6.8",
                        "architecture": "x86_64",
                        "abi": "gnu",
                        "cpu_feature_order": ["avx2"],
                        "accelerator_order": ["none"],
                        "host_digest": hash
                    },
                    "trusted_source_order": ["registry://trusted"],
                    "require_signed_metadata": true,
                    "require_portable_dependencies": true,
                    "dependencies": [{
                        "name": "numpy",
                        "kind": "library",
                        "version_constraint": "=2.1.0",
                        "resolved_version": "2.1.0",
                        "source": "registry://trusted",
                        "source_digest": hash,
                        "build_digest": hash,
                        "runtime_abi": "abi-v1",
                        "required": true,
                        "available": false,
                        "portable": true,
                        "metadata_signed": true,
                        "source_mutable": false,
                        "compromised": false,
                        "compatible_architecture_order": ["x86_64"],
                        "contains_human_data": false,
                        "contains_clinical_decision": false
                    }]
                },
                "candidates": [{
                    "candidate_id": "candidate-numpy-2.1.0",
                    "dependency_name": "numpy",
                    "kind": "library",
                    "proposed_version": "2.1.0",
                    "proposed_source": "registry://trusted",
                    "proposed_source_digest": hash,
                    "proposed_build_digest": hash,
                    "proposed_runtime_abi": "abi-v1",
                    "proposed_available": true,
                    "proposed_portable": true,
                    "proposed_metadata_signed": true,
                    "proposed_source_mutable": false,
                    "proposed_compromised": false,
                    "proposed_compatible_architecture_order": ["x86_64"],
                    "rationale": "trusted mirror provides the exact required build",
                    "cost_units": 1
                }]
            }
        }),
    );
    assert_eq!(
        output["proposal"]["feature_id"],
        json!("GAF-GLIOMA-P09-F09")
    );
    assert_eq!(output["proposal"]["disposition"], json!("proposed"));
    assert_eq!(
        output["proposal"]["resulting_lock"]["disposition"],
        json!("qualified")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_reproducible_task_submit_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let environment_lock = call(
        &mut server,
        "glioma_compute_environment_lock",
        json!({
            "request": {
                "objective": "qualify mcp task environment",
                "workflow_manifest_digest": hash,
                "workflow_task_order": ["normalize"],
                "architecture": {
                    "os_family": "linux",
                    "os_version": "6.8",
                    "architecture": "x86_64",
                    "abi": "gnu",
                    "cpu_feature_order": ["avx2"],
                    "accelerator_order": ["none"],
                    "host_digest": hash
                },
                "trusted_source_order": ["registry://trusted"],
                "require_signed_metadata": true,
                "require_portable_dependencies": true,
                "dependencies": [{
                    "name": "numpy",
                    "kind": "library",
                    "version_constraint": "=2.1.0",
                    "resolved_version": "2.1.0",
                    "source": "registry://trusted",
                    "source_digest": hash,
                    "build_digest": hash,
                    "runtime_abi": "abi-v1",
                    "required": true,
                    "available": true,
                    "portable": true,
                    "metadata_signed": true,
                    "source_mutable": false,
                    "compromised": false,
                    "compatible_architecture_order": ["x86_64"],
                    "contains_human_data": false,
                    "contains_clinical_decision": false
                }]
            }
        }),
    )["lock"]
        .clone();
    let output = call(
        &mut server,
        "glioma_reproducible_task_submit",
        json!({
            "request": {
                "idempotency_key": "mcp-task-key-1",
                "current_tick": 1,
                "prior_submissions": [],
                "task": {
                    "task_id": "normalize",
                    "workflow_id": "mcp-glioma-workflow",
                    "replay_identity": hash,
                    "input_schema_order": ["image-stack"],
                    "inputs": [{
                        "artifact": {
                            "artifact_id": "mcp-local-input",
                            "content_hash": hash,
                            "content_type": "image-stack",
                            "local_only": true,
                            "contains_human_data": false,
                            "contains_direct_identifiers": false
                        },
                        "schema": "image-stack",
                        "authorized": true,
                        "local_only": true
                    }],
                    "output_schema": "normalized-image-stack",
                    "estimated_cost_units": 2,
                    "estimated_duration_ticks": 5,
                    "deterministic": true,
                    "locality_required": true,
                    "effects_local_only": true
                },
                "environment_lock": environment_lock,
                "policy": {
                    "grant_id": "mcp-grant-1",
                    "site_id": "mcp-site-a",
                    "approved": true,
                    "allow_local_compute": true,
                    "allow_external_effects": false,
                    "expires_at_tick": 100
                },
                "budget": {
                    "max_cost_units": 10,
                    "max_duration_ticks": 20,
                    "max_memory_mb": 1024,
                    "max_accelerator_count": 0
                }
            }
        }),
    );
    assert_eq!(
        output["exchange"]["feature_id"],
        json!("GAF-GLIOMA-P09-F21")
    );
    assert_eq!(output["exchange"]["status"], json!("ready"));
    assert_eq!(output["exchange"]["result_state"], json!("not_started"));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_computation_event_stream_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_computation_event_stream",
        json!({
            "request": {
                "run_id": "mcp-run-1",
                "replay_identity": hash,
                "events": [
                    {
                        "event_id": "event-2",
                        "run_id": "mcp-run-1",
                        "replay_identity": hash,
                        "sequence": 2,
                        "emitted_tick": 2,
                        "task_id": "normalize",
                        "kind": "task_completed",
                        "payload_digest": hash,
                        "payload_redacted": false,
                        "local_only": true
                    },
                    {
                        "event_id": "event-1",
                        "run_id": "mcp-run-1",
                        "replay_identity": hash,
                        "sequence": 1,
                        "emitted_tick": 1,
                        "task_id": "normalize",
                        "kind": "task_started",
                        "payload_digest": hash,
                        "payload_redacted": false,
                        "local_only": true
                    }
                ],
                "filter": {
                    "kind_order": [],
                    "task_id_order": [],
                    "include_resource_events": true,
                    "redact_payloads": true,
                    "max_events": 16
                },
                "access": {
                    "site_id": "mcp-site-a",
                    "authorized": true,
                    "local_only": true,
                    "allow_resource_events": true,
                    "allow_qc_events": true,
                    "allow_recovery_events": true
                }
            }
        }),
    );
    assert_eq!(output["batch"]["feature_id"], json!("GAF-GLIOMA-P09-F23"));
    assert_eq!(
        output["batch"]["event_order"],
        json!(["event-1", "event-2"])
    );
    assert_eq!(
        output["batch"]["events"][0]["payload_redacted"],
        json!(true)
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_decision_budget_snapshot_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_decision_budget_snapshot",
        json!({
            "request": {
                "campaign_id": "campaign-mcp",
                "objective": "allocate preclinical glioma research capacity",
                "budgets": [
                    {"resource": "assay", "approved_units": 100, "alert_threshold_milli": 800},
                    {"resource": "compute", "approved_units": 100, "alert_threshold_milli": 800},
                    {"resource": "time", "approved_units": 100, "alert_threshold_milli": 800},
                    {"resource": "review", "approved_units": 100, "alert_threshold_milli": 800}
                ],
                "events": [{
                    "event_id": "event-mcp",
                    "branch_id": "branch-a",
                    "resource": "compute",
                    "consumed_units": 30,
                    "quoted_units": 50,
                    "state": "running",
                    "event_tick": 9
                }],
                "forecasts": [{
                    "branch_id": "branch-a",
                    "resource": "assay",
                    "additional_units": 85,
                    "confidence_milli": 400,
                    "required": true,
                    "forecast_tick": 10
                }],
                "branch_plans": [{
                    "branch_id": "branch-a",
                    "resource": "compute",
                    "planned_units": 80,
                    "priority_milli": 900,
                    "mandatory": true
                }],
                "current_tick": 10,
                "forecast_horizon_ticks": 50
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P04-F19")
    );
    assert_eq!(output["snapshot"]["disposition"], json!("warning"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_registry_artifact_resolve_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_registry_artifact_resolve",
        json!({
            "request": {
                "artifact_ref": {
                    "artifact_id": "mcp-image-stack",
                    "content_hash": hash,
                    "content_type": "image-stack",
                    "schema_version": "ome-ngff-0.5"
                },
                "candidates": [{
                    "artifact_id": "mcp-image-stack",
                    "content_hash": hash,
                    "content_type": "image-stack",
                    "schema_version": "ome-ngff-0.5",
                    "license": "CC-BY-4.0",
                    "source_uri": "registry://trusted/mcp-image-stack",
                    "source_digest": hash,
                    "signed_metadata": true,
                    "available": true,
                    "stale": false,
                    "corrupt": false,
                    "local_only": true,
                    "contains_human_data": false,
                    "contains_direct_identifiers": false,
                    "contains_clinical_decision": false
                }],
                "trust": {
                    "approved_source_prefix_order": ["registry://trusted"],
                    "allowed_license_order": ["CC-BY-4.0"],
                    "allowed_content_type_order": ["image-stack"],
                    "require_signed_metadata": true,
                    "require_local_only": true,
                    "max_candidate_age_ticks": 10
                },
                "access": {
                    "grant_id": "mcp-artifact-grant",
                    "site_id": "mcp-site-a",
                    "authorized": true,
                    "expires_at_tick": 100,
                    "allow_registry_read": true
                },
                "current_tick": 1
            }
        }),
    );
    assert_eq!(
        output["resolution"]["feature_id"],
        json!("GAF-GLIOMA-P09-F22")
    );
    assert_eq!(output["resolution"]["disposition"], json!("resolved"));
    assert_eq!(output["resolution"]["handle"]["local_only"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_compute_cache_govern_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_compute_cache_govern",
        json!({
            "request": {
                "key": {
                    "task_id": "normalize",
                    "input_hash_order": [hash],
                    "code_digest": hash,
                    "environment_digest": hash,
                    "policy_digest": hash,
                    "semantic_version": "1.0.0",
                    "output_schema": "normalized-image"
                },
                "existing_entries": [{
                    "entry_id": "cache-1",
                    "key": {
                        "task_id": "normalize",
                        "input_hash_order": [hash],
                        "code_digest": hash,
                        "environment_digest": hash,
                        "policy_digest": hash,
                        "semantic_version": "1.0.0",
                        "output_schema": "normalized-image"
                    },
                    "artifact": {
                        "artifact_id": "normalized-image-1",
                        "content_hash": hash,
                        "content_type": "normalized-image",
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false
                    },
                    "created_tick": 1,
                    "last_used_tick": 1,
                    "size_units": 2,
                    "pinned": false
                }],
                "incoming_entry": null,
                "policy": {
                    "allow_reuse": true,
                    "require_local_only": true,
                    "max_entries": 2,
                    "max_total_size_units": 10,
                    "retention_ticks": 10,
                    "eviction_policy": "least_recently_used",
                    "policy_digest": hash
                },
                "current_tick": 5
            }
        }),
    );
    assert_eq!(
        output["decision"]["feature_id"],
        json!("GAF-GLIOMA-P09-F29")
    );
    assert_eq!(output["decision"]["disposition"], json!("hit"));
    assert_eq!(output["decision"]["cache_hit"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_multistudy_cache_partition_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_multistudy_cache_partition",
        json!({
            "request": {
                "requesting_study_id": "study-b",
                "requested_scope": "deidentified-organoid",
                "requested_cache_key_digest": hash,
                "existing_entries": [{
                    "entry_id": "protected-a",
                    "artifact_id": "artifact-protected-a",
                    "content_hash": hash,
                    "cache_key_digest": hash,
                    "source_study_id": "study-a",
                    "deidentification_scope": "deidentified-organoid",
                    "sensitivity": "protected_study",
                    "public_reference": false,
                    "local_only": true,
                    "contains_human_data": false,
                    "contains_direct_identifiers": false,
                    "contains_clinical_decision": false
                }],
                "policy": {
                    "policy_digest": hash,
                    "allowed_scope_order": ["deidentified-organoid"],
                    "allow_public_reference_reuse": true,
                    "allow_cross_study_study_local": false,
                    "require_local_only": true,
                    "expires_at_tick": 100
                },
                "current_tick": 1
            }
        }),
    );
    assert_eq!(
        output["decision"]["feature_id"],
        json!("GAF-GLIOMA-P09-F30")
    );
    assert_eq!(output["decision"]["disposition"], json!("denied"));
    assert_eq!(output["decision"]["cache_hit"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_compute_capacity_plan_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_compute_capacity_plan",
        json!({
            "request": {
                "objective": "schedule reproducible preclinical glioma imaging workloads",
                "jobs": [
                    {
                        "job_id": "job-a",
                        "workflow_class": "imaging",
                        "fairness_group": "site-a",
                        "priority_milli": 500,
                        "resource_units": 2,
                        "memory_mb": 512,
                        "accelerator_count": 0,
                        "estimated_duration_ticks": 10,
                        "budget_units": 2,
                        "required": false,
                        "submitted_tick": 1
                    },
                    {
                        "job_id": "job-b",
                        "workflow_class": "imaging",
                        "fairness_group": "site-b",
                        "priority_milli": 500,
                        "resource_units": 2,
                        "memory_mb": 512,
                        "accelerator_count": 0,
                        "estimated_duration_ticks": 10,
                        "budget_units": 2,
                        "required": false,
                        "submitted_tick": 1
                    }
                ],
                "observations": [],
                "telemetry": {
                    "available_concurrency": 1,
                    "resource_capacity_units": 4,
                    "memory_capacity_mb": 2048,
                    "accelerator_capacity_count": 0,
                    "sampled_tick": 1,
                    "max_age_ticks": 10
                },
                "policy": {
                    "max_concurrency": 1,
                    "max_budget_units": 10,
                    "max_job_age_ticks": 100,
                    "max_duration_ticks": 100,
                    "fairness_weight_milli": 1000,
                    "require_fresh_telemetry": true,
                    "policy_digest": hash
                },
                "current_tick": 2,
                "horizon_ticks": 100
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P09-F31"));
    assert_eq!(
        output["plan"]["scheduled_order"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        output["plan"]["deferred_order"].as_array().unwrap().len(),
        1
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_computation_portfolio_planner_closes_dependencies() {
    let mut server = server();
    let plan = call(
        &mut server,
        "glioma_computation_portfolio_plan",
        json!({
            "request": {
                "objective": "choose a reproducible organoid multimodal computation portfolio",
                "model_system": "organoid",
                "budget_units": 4,
                "duration_ticks": 2,
                "max_tasks": 2,
                "max_modalities": 2,
                "min_modalities": 2,
                "information_weight_milli": 5,
                "uncertainty_weight_milli": 3,
                "coverage_weight_milli": 2,
                "cost_penalty_milli": 1,
                "duration_penalty_milli": 1,
                "require_deterministic": true,
                "completed_order": []
            },
            "candidates": [
                {"candidate_id":"integrate","task":{"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["input:integrate"],"output_schema":"Integrate1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"spatial","information_gain_milli":900,"uncertainty_reduction_milli":500,"coverage_debt_milli":400,"redundancy_group":"integration","required":false},
                {"candidate_id":"normalize","task":{"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"transcriptomics","information_gain_milli":400,"uncertainty_reduction_milli":500,"coverage_debt_milli":300,"redundancy_group":"normalization","required":false}
            ]
        }),
    );
    assert_eq!(plan["dispatch"], json!("not_started"));
    assert_eq!(
        plan["plan"]["dependency_order"],
        json!(["normalize", "integrate"])
    );
    assert_eq!(plan["plan"]["disposition"], json!("qualified"));
}

#[test]
fn glioma_computation_placement_builds_locality_aware_pre_dispatch_schedule() {
    let mut server = server();
    let replay = "1".repeat(64);
    let schedule = call(
        &mut server,
        "glioma_computation_placement",
        json!({
            "request": {
                "objective": "place a reproducible organoid imaging and transcriptomics DAG",
                "model_system": "organoid",
                "replay_identity": replay,
                "current_tick": 0,
                "max_end_tick": 100,
                "max_budget_units": 100,
                "max_transfer_cost_units": 100,
                "tasks": [
                    {"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["matrix"],"output_schema":"Integrate1@1","estimated_cost_units":10,"estimated_duration_ticks":5,"deterministic":true},
                    {"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["matrix"],"output_schema":"Normalize1@1","estimated_cost_units":10,"estimated_duration_ticks":5,"deterministic":true}
                ],
                "workers": [
                    {"worker_id":"gpu-a","model_system_order":["organoid"],"operation_order":["normalize","integrate"],"local_artifact_order":["matrix"],"available_from_tick":0,"available_until_tick":100,"max_task_cost_units":1000,"transfer_ticks_per_artifact":2,"transfer_cost_units_per_artifact":3,"speed_milli":1000,"enabled":true},
                    {"worker_id":"cpu-b","model_system_order":["organoid"],"operation_order":["normalize","integrate"],"local_artifact_order":[],"available_from_tick":0,"available_until_tick":100,"max_task_cost_units":1000,"transfer_ticks_per_artifact":2,"transfer_cost_units_per_artifact":3,"speed_milli":1000,"enabled":true}
                ],
                "completed_task_order": [],
                "cache": []
            }
        }),
    );
    assert_eq!(schedule["dispatch"], json!("not_started"));
    assert_eq!(schedule["preflight_required"], json!(true));
    assert_eq!(schedule["schedule"]["disposition"], json!("ready"));
    assert_eq!(
        schedule["schedule"]["assigned_order"],
        json!(["integrate", "normalize"])
    );
    assert_eq!(schedule["schedule"]["total_transfer_cost_units"], json!(0));
    assert_eq!(schedule["schedule"]["dispatch_permitted"], json!(false));

    let evaluation = call(
        &mut server,
        "glioma_computation_placement_stress_evaluate",
        json!({
            "request": {
                "base": {
                    "objective": "place a reproducible organoid imaging and transcriptomics DAG",
                    "model_system": "organoid",
                    "replay_identity": replay,
                    "current_tick": 0,
                    "max_end_tick": 100,
                    "max_budget_units": 100,
                    "max_transfer_cost_units": 100,
                    "tasks": [
                        {"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["matrix"],"output_schema":"Integrate1@1","estimated_cost_units":10,"estimated_duration_ticks":5,"deterministic":true},
                        {"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["matrix"],"output_schema":"Normalize1@1","estimated_cost_units":10,"estimated_duration_ticks":5,"deterministic":true}
                    ],
                    "workers": [
                        {"worker_id":"gpu-a","model_system_order":["organoid"],"operation_order":["normalize","integrate"],"local_artifact_order":["matrix"],"available_from_tick":0,"available_until_tick":100,"max_task_cost_units":1000,"transfer_ticks_per_artifact":2,"transfer_cost_units_per_artifact":3,"speed_milli":1000,"enabled":true},
                        {"worker_id":"cpu-b","model_system_order":["organoid"],"operation_order":["normalize","integrate"],"local_artifact_order":[],"available_from_tick":0,"available_until_tick":100,"max_task_cost_units":1000,"transfer_ticks_per_artifact":2,"transfer_cost_units_per_artifact":3,"speed_milli":1000,"enabled":true}
                    ],
                    "completed_task_order": [],
                    "cache": []
                },
                "scenarios": [
                    {"scenario_id":"nominal","disabled_worker_order":[],"transfer_cost_multiplier_milli":1000,"transfer_ticks_multiplier_milli":1000,"budget_multiplier_milli":1000,"end_tick_multiplier_milli":1000},
                    {"scenario_id":"gpu-loss","disabled_worker_order":["gpu-a"],"transfer_cost_multiplier_milli":1500,"transfer_ticks_multiplier_milli":1500,"budget_multiplier_milli":1000,"end_tick_multiplier_milli":1000}
                ],
                "require_non_degradation": false
            }
        }),
    );
    assert_eq!(evaluation["dispatch"], json!("not_started"));
    assert_eq!(evaluation["simulation_only"], json!(true));
    assert_eq!(
        evaluation["evaluation"]["output_schema"],
        json!("GliomaComputationPlacementStressEvaluation1@1")
    );
    assert_eq!(
        evaluation["evaluation"]["scenario_order"],
        json!(["gpu-loss", "nominal"])
    );
}

#[test]
fn glioma_computation_portfolio_executor_runs_selected_dag() {
    let mut server = server();
    let execution = call(
        &mut server,
        "glioma_computation_portfolio_execute",
        json!({
            "request": {
                "portfolio": {
                    "objective": "execute a reproducible organoid multimodal computation portfolio",
                    "model_system": "organoid",
                    "budget_units": 4,
                    "duration_ticks": 2,
                    "max_tasks": 2,
                    "max_modalities": 2,
                    "min_modalities": 2,
                    "information_weight_milli": 5,
                    "uncertainty_weight_milli": 3,
                    "coverage_weight_milli": 2,
                    "cost_penalty_milli": 1,
                    "duration_penalty_milli": 1,
                    "require_deterministic": true,
                    "completed_order": []
                },
                "candidates": [
                    {"candidate_id":"integrate","task":{"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["input:integrate"],"output_schema":"Integrate1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"spatial","information_gain_milli":900,"uncertainty_reduction_milli":500,"coverage_debt_milli":400,"redundancy_group":"integration","required":false},
                    {"candidate_id":"normalize","task":{"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"transcriptomics","information_gain_milli":400,"uncertainty_reduction_milli":500,"coverage_debt_milli":300,"redundancy_group":"normalization","required":false}
                ],
                "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000",
                "max_retries": 1,
                "allow_cache": true,
                "require_local_artifacts": true,
                "cache": []
            }
        }),
    );
    assert_eq!(execution["dispatch"], json!("not_started"));
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    assert_eq!(
        execution["execution"]["execution"]["task_order"],
        json!(["normalize", "integrate"])
    );
}

#[test]
fn glioma_robustness_guided_computation_executes_a_replayable_frontier() {
    let mut server = server();
    let hash = "0".repeat(64);
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
                "dataset_id": "guided-computation-robustness",
                "artifact": {"artifact_id":"guided-robustness-artifact","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
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
    let guided = call(
        &mut server,
        "glioma_robustness_guided_computation_execute",
        json!({
            "request": {
                "objective": "stress-test a preclinical glioma invasion effect",
                "model_system": "organoid",
                "robustness": robustness["suite"].clone(),
                "portfolio": {
                    "objective": "stress-test a preclinical glioma invasion effect",
                    "model_system": "organoid",
                    "budget_units": 4,
                    "duration_ticks": 2,
                    "max_tasks": 2,
                    "max_modalities": 2,
                    "min_modalities": 1,
                    "information_weight_milli": 5,
                    "uncertainty_weight_milli": 3,
                    "coverage_weight_milli": 4,
                    "cost_penalty_milli": 1,
                    "duration_penalty_milli": 1,
                    "require_deterministic": true,
                    "completed_order": []
                },
                "candidates": [
                    {"candidate":{"candidate_id":"normalize","task":{"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"transcriptomics","information_gain_milli":400,"uncertainty_reduction_milli":500,"coverage_debt_milli":300,"redundancy_group":"normalization","required":false},"target_case_ids":[],"scientific_role":"normalization re-analysis"},
                    {"candidate":{"candidate_id":"integrate","task":{"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["input:integrate"],"output_schema":"Integrate1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"spatial","information_gain_milli":900,"uncertainty_reduction_milli":500,"coverage_debt_milli":400,"redundancy_group":"integration","required":false},"target_case_ids":[],"scientific_role":"multimodal integration re-analysis"}
                ],
                "replay_identity": hash,
                "max_retries": 1,
                "allow_cache": true,
                "require_local_artifacts": true,
                "cache": [],
                "minimum_case_coverage": 0,
                "minimum_fragile_case_coverage": 0,
                "stop_if_stable": false
            }
        }),
    );
    assert_eq!(guided["dispatch"], json!("dry_run"));
    assert_eq!(guided["simulation_only"], json!(true));
    assert_eq!(guided["computation"]["disposition"], json!("executed"));
    assert_eq!(
        guided["computation"]["portfolio_execution"]["planned_task_order"],
        json!(["normalize", "integrate"])
    );
}

#[test]
fn glioma_computation_campaign_replans_and_executes_seed_round() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_computation_campaign_execute",
        json!({
            "request": {
                "objective": "autonomously execute a reproducible glioma invasion analysis",
                "model_system": "organoid",
                "initial_candidates": [
                    {"candidate_id":"integrate","task":{"task_id":"integrate","operation":"integrate","model_system":"organoid","depends_on":["normalize"],"input_artifact_ids":["input:integrate"],"output_schema":"Integrate1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"spatial","information_gain_milli":900,"uncertainty_reduction_milli":500,"coverage_debt_milli":400,"redundancy_group":"integration","required":false},
                    {"candidate_id":"normalize","task":{"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"transcriptomics","information_gain_milli":400,"uncertainty_reduction_milli":500,"coverage_debt_milli":300,"redundancy_group":"normalization","required":false}
                ],
                "budget_units": 4,
                "duration_ticks": 2,
                "max_rounds": 3,
                "max_retries": 1,
                "max_tasks": 2,
                "max_modalities": 2,
                "min_modalities": 2,
                "information_weight_milli": 5,
                "uncertainty_weight_milli": 3,
                "coverage_weight_milli": 2,
                "cost_penalty_milli": 1,
                "duration_penalty_milli": 1,
                "require_deterministic": true,
                "allow_cache": true,
                "require_local_artifacts": true,
                "cache": [],
                "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("completed"));
    assert_eq!(campaign["campaign"]["stop_reason"], json!("completed"));
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(
        campaign["campaign"]["completed_order"],
        json!(["integrate", "normalize"])
    );
}

#[test]
fn glioma_computation_recovery_keeps_clean_initial_campaign_without_recovery_dispatch() {
    let mut server = server();
    let campaign = call(
        &mut server,
        "glioma_computation_recovery_execute",
        json!({
            "request": {
                "initial": {
                    "objective": "recover a reproducible glioma computation",
                    "model_system": "organoid",
                    "initial_candidates": [{"candidate_id":"normalize","task":{"task_id":"normalize","operation":"normalize","model_system":"organoid","depends_on":[],"input_artifact_ids":["input:normalize"],"output_schema":"Normalize1@1","estimated_cost_units":2,"estimated_duration_ticks":1,"deterministic":true},"modality":"transcriptomics","information_gain_milli":700,"uncertainty_reduction_milli":600,"coverage_debt_milli":400,"redundancy_group":"normalization","required":true}],
                    "budget_units":4,"duration_ticks":2,"max_rounds":2,"max_retries":1,"max_tasks":1,"max_modalities":1,"min_modalities":1,
                    "information_weight_milli":5,"uncertainty_weight_milli":3,"coverage_weight_milli":2,"cost_penalty_milli":1,"duration_penalty_milli":1,
                    "require_deterministic":true,"allow_cache":true,"require_local_artifacts":true,"cache":[],"replay_identity":"0000000000000000000000000000000000000000000000000000000000000000"
                },
                "recovery_budget_units":4,"recovery_duration_ticks":2,"max_recovery_rounds":2,"require_clean_completion":true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("completed"));
    assert_eq!(
        campaign["campaign"]["stop_reason"],
        json!("initial_completed")
    );
    assert!(campaign["campaign"]["recovery"].is_null());
    assert!(
        campaign["campaign"]["invalidated_cache_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_computation_workflow_compiles_intent_and_executes_closed_dag() {
    let mut server = server();
    let workflow = call(
        &mut server,
        "glioma_computation_workflow_execute",
        json!({
            "request": {
                "objective": "profile invasive organoid state across modalities",
                "study_id": "study-glioma-01",
                "model_system": "organoid",
                "modalities": ["transcriptomics", "imaging"],
                "operations": ["export", "model_fit"],
                "input_artifact_ids": ["artifact-imaging", "artifact-rna"],
                "budget_units": 100,
                "duration_ticks": 500,
                "max_tasks": 64,
                "max_modalities": 4,
                "min_modalities": 2,
                "information_weight_milli": 5,
                "uncertainty_weight_milli": 4,
                "coverage_weight_milli": 3,
                "cost_penalty_milli": 1,
                "duration_penalty_milli": 1,
                "require_deterministic": true,
                "max_rounds": 4,
                "max_retries": 1,
                "allow_cache": true,
                "require_local_artifacts": true,
                "cache": [],
                "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        }),
    );
    assert_eq!(workflow["dispatch"], json!("dry_run"));
    assert_eq!(workflow["simulation_only"], json!(true));
    assert_eq!(
        workflow["workflow"]["within_declared_resources"],
        json!(true)
    );
    assert_eq!(
        workflow["workflow"]["requested_terminal_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        workflow["workflow"]["candidates"].as_array().unwrap().len(),
        14
    );
    assert_eq!(workflow["campaign"]["disposition"], json!("completed"));
    assert_eq!(workflow["campaign"]["stop_reason"], json!("completed"));
}

#[test]
fn glioma_computation_operating_cycle_compiles_gates_and_executes_replayable_campaign() {
    let mut server = server();
    let cycle = call(
        &mut server,
        "glioma_computation_operating_cycle",
        json!({
            "request": {
                "workflow": {
                    "objective": "profile invasive organoid state across modalities",
                    "study_id": "study-glioma-01",
                    "model_system": "organoid",
                    "modalities": ["transcriptomics", "imaging"],
                    "operations": ["export", "model_fit"],
                    "input_artifact_ids": ["artifact-imaging", "artifact-rna"],
                    "budget_units": 100,
                    "duration_ticks": 500,
                    "max_tasks": 64,
                    "max_modalities": 4,
                    "min_modalities": 2,
                    "information_weight_milli": 5,
                    "uncertainty_weight_milli": 4,
                    "coverage_weight_milli": 3,
                    "cost_penalty_milli": 1,
                    "duration_penalty_milli": 1,
                    "require_deterministic": true,
                    "max_rounds": 4,
                    "max_retries": 1,
                    "allow_cache": true,
                    "require_local_artifacts": true,
                    "cache": [],
                    "replay_identity": "0000000000000000000000000000000000000000000000000000000000000000"
                },
                "require_within_resources": true,
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(cycle["dispatch"], json!("dry_run"));
    assert_eq!(cycle["simulation_only"], json!(true));
    assert_eq!(cycle["cycle"]["disposition"], json!("executed"));
    assert_eq!(
        cycle["cycle"]["phase_order"],
        json!([
            "workflow_compile",
            "resource_gate",
            "computation_campaign",
            "operator_handoff"
        ])
    );
    assert_eq!(
        cycle["cycle"]["workflow"]["within_declared_resources"],
        json!(true)
    );
    assert_eq!(
        cycle["cycle"]["campaign"]["completed_order"]
            .as_array()
            .unwrap()
            .len(),
        14
    );
}

#[test]
fn glioma_reproducibility_completeness_score_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let manifest = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"mcp-research","study_id":"mcp-study","objective":"release a preclinical glioma result","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05_mechanism","p10_interpretation"],"artifacts":[{"artifact_id":"mcp-artifact","content_hash":zero,"content_type":"application/vnd.aurora.glioma+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null-result"],"limitations":["single-model"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let dimensions = [
        "data_scope",
        "code",
        "environment",
        "methods",
        "artifacts",
        "uncertainty",
        "negative_outcomes",
        "lineage",
        "independent_replay",
    ];
    let evidence = dimensions.iter().enumerate().map(|(index, dimension)| json!({"dimension":dimension,"evidence_id":format!("mcp-evidence-{index}"),"present":true,"quality_milli":950,"coverage_milli":950,"source_digest":zero,"note":"explicit local evidence"})).collect::<Vec<_>>();
    let output = call(
        &mut server,
        "glioma_reproducibility_completeness_score",
        json!({"request":{"manifest":manifest,"evidence":evidence,"required_dimension_order":dimensions,"independent_replay_count":2,"minimum_score_milli":800,"minimum_independent_replays":2,"require_exact_replay":false,"require_negative_outcome_accounting":true,"require_uncertainty_accounting":true,"replay":null}}),
    );
    assert_eq!(output["profile"]["feature_id"], json!("GAF-GLIOMA-P11-F02"));
    assert_eq!(output["profile"]["disposition"], json!("complete"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_reproducibility_bundle_compile_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let manifest = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"bundle-mcp-research","study_id":"bundle-mcp-study","objective":"offline reproducibility","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05"],"artifacts":[{"artifact_id":"root","content_hash":zero,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null"],"limitations":["preclinical"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let shareability = call(
        &mut server,
        "glioma_release_shareability_check",
        json!({
            "request": {
                "candidate_manifest_digest": zero,
                "root_order": ["root"],
                "dependencies": [
                    {"artifact_id":"root","dependency_order":["upstream"],"license_id":"MIT","fields":[{"field_id":"summary","classification":"aggregate_result","requested_export":true,"source_digest":zero}],"local_only":false,"embargo_until_epoch":null,"rights_confirmed":true,"contains_human_data":false,"intended_audience":"consortium"},
                    {"artifact_id":"upstream","dependency_order":[],"license_id":"MIT","fields":[{"field_id":"methods","classification":"public_metadata","requested_export":true,"source_digest":zero}],"local_only":false,"embargo_until_epoch":null,"rights_confirmed":true,"contains_human_data":false,"intended_audience":"consortium"}
                ],
                "policy": {"allowed_license_order":["MIT"],"forbidden_license_order":[],"audience":"consortium","now_epoch":20260923,"permit_aggregate_export":true,"permit_local_only_export":false,"permit_human_data":false}
            }
        }),
    );
    let output = call(
        &mut server,
        "glioma_reproducibility_bundle_compile",
        json!({
            "request": {
                "manifest": manifest,
                "shareability": shareability["decision"],
                "members": [
                    {"artifact_id":"root","relative_path":"root.json","content_hash":zero,"content_type":"application/json","dependency_order":["upstream"],"export_permitted":true,"local_only":false},
                    {"artifact_id":"upstream","relative_path":"upstream.json","content_hash":zero,"content_type":"application/json","dependency_order":[],"export_permitted":true,"local_only":false}
                ],
                "workflow_digest": zero,
                "environment_digest": zero,
                "replay_instruction_order": ["run-workflow"],
                "target_profile": "offline-linux-x86_64",
                "max_members": 8
            }
        }),
    );
    assert_eq!(output["bundle"]["feature_id"], json!("GAF-GLIOMA-P11-F13"));
    assert_eq!(output["bundle"]["disposition"], json!("complete"));
    assert_eq!(output["dispatch"], json!("not_started"));
}
