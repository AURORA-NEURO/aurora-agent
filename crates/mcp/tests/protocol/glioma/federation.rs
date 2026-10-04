//! MCP contract tests for the federation area.

use super::*;

#[test]
fn glioma_federated_aggregate_anomaly_detector_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federated_aggregate_anomaly_detect",
        json!({
            "request": {
                "objective": "audit a preclinical glioma segmentation benchmark",
                "capability_id": "segmentation",
                "benchmark_world": "glioma-world-v1",
                "metric_name": "dice",
                "protocol_version": "dice-v1",
                "model_system": "organoid",
                "minimum_sites": 2,
                "minimum_replicates_per_site": 3,
                "minimum_value_milli": 0,
                "maximum_value_milli": 1000,
                "maximum_uncertainty_milli": 100,
                "robust_outlier_threshold_milli": 100,
                "maximum_temporal_drift_milli": 100,
                "maximum_suppressed_fraction_milli": 500,
                "observations": [
                    {
                        "site_id": "mcp-anomaly-site-a",
                        "study_id": "mcp-anomaly-study-a",
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "dice",
                        "model_system": "organoid",
                        "aggregate_value_milli": 700,
                        "uncertainty_milli": 20,
                        "replicate_count": 5,
                        "historical_value_milli": 700,
                        "protocol_version": "dice-v1",
                        "privacy_suppressed": false,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    },
                    {
                        "site_id": "mcp-anomaly-site-b",
                        "study_id": "mcp-anomaly-study-b",
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "dice",
                        "model_system": "organoid",
                        "aggregate_value_milli": 950,
                        "uncertainty_milli": 20,
                        "replicate_count": 5,
                        "historical_value_milli": 700,
                        "protocol_version": "dice-v1",
                        "privacy_suppressed": false,
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
        json!("GAF-GLIOMA-P12-F11")
    );
    assert_eq!(output["assessment"]["disposition"], json!("review"));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_federated_site_selection_plan_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federated_site_selection_plan",
        json!({
            "request": {
                "objective": "select a preclinical glioma segmentation consortium",
                "capability_id": "segmentation",
                "benchmark_world": "glioma-world-v1",
                "model_system": "organoid",
                "minimum_sites": 2,
                "maximum_sites": 2,
                "minimum_capacity_units": 50,
                "maximum_cost_units": 100,
                "maximum_freshness_age_hours": 24,
                "maximum_same_group_fraction_milli": 500,
                "required_representation_tags": ["organoid", "imaging"],
                "envelopes": [
                    {
                        "site_id": "mcp-selection-site-a",
                        "institution_group": "mcp-group-a",
                        "capability_ids": ["segmentation"],
                        "model_systems": ["organoid"],
                        "representation_tags": ["organoid"],
                        "capacity_units": 100,
                        "estimated_cost_units": 10,
                        "freshness_age_hours": 2,
                        "privacy_approved": true,
                        "revoked": false,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "manifest_digest": zero
                    },
                    {
                        "site_id": "mcp-selection-site-b",
                        "institution_group": "mcp-group-b",
                        "capability_ids": ["segmentation"],
                        "model_systems": ["organoid"],
                        "representation_tags": ["imaging"],
                        "capacity_units": 100,
                        "estimated_cost_units": 10,
                        "freshness_age_hours": 2,
                        "privacy_approved": true,
                        "revoked": false,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "manifest_digest": zero
                    }
                ]
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P12-F09"));
    assert_eq!(output["plan"]["representation_coverage_milli"], json!(1000));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_federation_capacity_plan_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federation_capacity_plan",
        json!({
            "request": {
                "objective": "schedule a preclinical glioma benchmark federation",
                "required_quorum_sites": 2,
                "planning_horizon": 2,
                "demand_units_per_window": 100,
                "maximum_schedule_units": 200,
                "minimum_privacy_budget_milli": 500,
                "maximum_latency_minutes": 60,
                "maximum_site_commitment_fraction_milli": 800,
                "observations": [
                    {
                        "site_id": "mcp-capacity-site-a",
                        "institution_group": "mcp-capacity-group-a",
                        "epoch": 1,
                        "capacity_units": 100,
                        "committed_capacity_units": 10,
                        "privacy_budget_milli": 800,
                        "expected_latency_minutes": 20,
                        "active_workflow_count": 2,
                        "availability_milli": 900,
                        "eligible_for_quorum": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    },
                    {
                        "site_id": "mcp-capacity-site-b",
                        "institution_group": "mcp-capacity-group-b",
                        "epoch": 1,
                        "capacity_units": 100,
                        "committed_capacity_units": 10,
                        "privacy_budget_milli": 800,
                        "expected_latency_minutes": 20,
                        "active_workflow_count": 2,
                        "availability_milli": 900,
                        "eligible_for_quorum": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "source_digest": zero
                    }
                ]
            }
        }),
    );
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P12-F31"));
    assert_eq!(output["plan"]["disposition"], json!("scheduled"));
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_federated_benchmark_dry_run_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federated_benchmark_dry_run",
        json!({
            "request": {
                "objective": "preflight a synthetic glioma segmentation federation",
                "capability_id": "segmentation",
                "benchmark_world": "glioma-world-v1",
                "model_system": "organoid",
                "required_schema_version": "schema-v1",
                "minimum_sites": 2,
                "maximum_budget_units": 100,
                "require_approval": true,
                "fixtures": [
                    {
                        "site_id": "mcp-dry-run-site-a",
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "model_system": "organoid",
                        "schema_version": "schema-v1",
                        "synthetic_artifact_count": 2,
                        "projected_cost_units": 10,
                        "approval_required": false,
                        "approval_granted": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "declared_failure_modes": [],
                        "fixture_digest": zero
                    },
                    {
                        "site_id": "mcp-dry-run-site-b",
                        "capability_id": "segmentation",
                        "benchmark_world": "glioma-world-v1",
                        "model_system": "organoid",
                        "schema_version": "schema-v1",
                        "synthetic_artifact_count": 2,
                        "projected_cost_units": 10,
                        "approval_required": true,
                        "approval_granted": true,
                        "local_only": true,
                        "contains_human_data": false,
                        "contains_direct_identifiers": false,
                        "declared_failure_modes": [],
                        "fixture_digest": zero
                    }
                ]
            }
        }),
    );
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P12-F13"));
    assert_eq!(output["report"]["disposition"], json!("ready"));
    assert_eq!(output["report"]["simulation_only"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_replay_discrepancy_scan_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let one = "1".repeat(64);
    let output = call(
        &mut server,
        "glioma_federated_replay_discrepancy_scan",
        json!({
            "request": {
                "objective": "localize a preclinical glioma replay mismatch",
                "replay_group_id": "mcp-replay-1",
                "reference_site_id": "mcp-site-a",
                "required_stage_order": ["normalize"],
                "permitted_site_order": ["mcp-site-a", "mcp-site-b"],
                "federation_policy_digest": zero,
                "max_diagnostics_per_site": 4,
                "attestations": [
                    {
                        "site_id": "mcp-site-a",
                        "replay_group_id": "mcp-replay-1",
                        "run_id": "mcp-run-1",
                        "workflow_digest": zero,
                        "input_data_version_digest": zero,
                        "environment_digest": zero,
                        "dependency_lock_digest": zero,
                        "numeric_kernel_digest": zero,
                        "seed_digest": zero,
                        "output_digest": zero,
                        "stage_digests": {"normalize": zero},
                        "missing_fields": [],
                        "signer_id": "mcp-signer-a",
                        "signature_digest": zero,
                        "signature_valid": true,
                        "permitted_summary_only": true,
                        "contains_raw_inputs": false,
                        "contains_human_data": false,
                        "contains_clinical_decision": false,
                        "generated_at_unix_seconds": 1
                    },
                    {
                        "site_id": "mcp-site-b",
                        "replay_group_id": "mcp-replay-1",
                        "run_id": "mcp-run-1",
                        "workflow_digest": zero,
                        "input_data_version_digest": zero,
                        "environment_digest": one,
                        "dependency_lock_digest": zero,
                        "numeric_kernel_digest": zero,
                        "seed_digest": zero,
                        "output_digest": zero,
                        "stage_digests": {"normalize": zero},
                        "missing_fields": [],
                        "signer_id": "mcp-signer-b",
                        "signature_digest": one,
                        "signature_valid": true,
                        "permitted_summary_only": true,
                        "contains_raw_inputs": false,
                        "contains_human_data": false,
                        "contains_clinical_decision": false,
                        "generated_at_unix_seconds": 1
                    }
                ]
            }
        }),
    );
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P09-F04"));
    assert_eq!(output["report"]["disposition"], json!("divergent"));
    assert_eq!(
        output["report"]["discrepancies"][0]["divergence_kind"],
        json!("environment")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["simulation_only"], json!(true));
}

#[test]
fn glioma_federated_workflow_template_exchange_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let environment_lock = call(
        &mut server,
        "glioma_compute_environment_lock",
        json!({
            "request": {
                "objective": "qualify mcp federated template environment",
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
        "glioma_federated_workflow_template_exchange",
        json!({
            "request": {
                "manifest": {
                    "template_id": "mcp-glioma-normalize",
                    "version": "1.0.0",
                    "workflow_manifest_digest": hash,
                    "task_order": ["normalize"],
                    "input_schema_order": ["image-stack"],
                    "output_schema_order": ["normalized-image-stack"],
                    "effect_order": ["execute_local_computation", "read_local_artifact"],
                    "deterministic": true,
                    "local_only": true,
                    "source_site_id": "mcp-site-a"
                },
                "environment_lock": environment_lock,
                "validation_card": {
                    "card_id": "mcp-card-1",
                    "template_digest": hash,
                    "benchmark_digest": hash,
                    "metric_order": ["exact_replay_rate"],
                    "required_gate_order": ["held_out_replay"],
                    "held_out_run_count": 2,
                    "passed": true
                },
                "sharing_policy": {
                    "allowed_site_order": ["mcp-site-a", "mcp-site-b"],
                    "revoked_site_order": [],
                    "allow_adaptations": true,
                    "allow_environment_metadata": true,
                    "allow_aggregate_attestations": true,
                    "expires_at_tick": 100
                },
                "attestations": [],
                "current_tick": 1
            }
        }),
    );
    assert_eq!(output["package"]["feature_id"], json!("GAF-GLIOMA-P09-F16"));
    assert_eq!(output["package"]["disposition"], json!("local_only"));
    assert_eq!(output["package"]["portability_claim"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_replay_conformance_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federated_replay_conformance",
        json!({
            "request": {
                "reference": {
                    "reference_id": "mcp-reference-1",
                    "workflow_digest": hash,
                    "tolerance_profile_version": "tol-1",
                    "metric_order": [{
                        "metric": "replay_rate",
                        "reference_milli": 950,
                        "max_absolute_milli": 20,
                        "max_relative_milli": 30
                    }],
                    "required_site_order": ["mcp-site-a"],
                    "generated_tick": 1
                },
                "attestations": [],
                "policy": {
                    "allowed_site_order": ["mcp-site-a"],
                    "federation_policy_version": "policy-1",
                    "max_age_ticks": 10,
                    "require_signed_attestations": true,
                    "require_aggregate_only": true,
                    "expires_at_tick": 100
                },
                "current_tick": 2
            }
        }),
    );
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P09-F28"));
    assert_eq!(output["report"]["disposition"], json!("unresolved"));
    assert_eq!(output["report"]["portability_claim"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_compute_capacity_exchange_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let summary = |site_id: &str, cost: u64| {
        json!({
            "site_id": site_id,
            "membership_version": "members-1",
            "workflow_envelope_digest": hash.clone(),
            "policy_digest": hash.clone(),
            "sampled_tick": 1,
            "expires_at_tick": 100,
            "available_concurrency": 4,
            "resource_capacity_units": 32,
            "memory_capacity_mb": 16384,
            "accelerator_capacity_count": 2,
            "cost_per_resource_milli": cost,
            "cost_per_tick_milli": 1,
            "cost_uncertainty_milli": 50,
            "capability_order": ["cuda", "imaging"],
            "localization_scope_order": ["organoid"],
            "aggregate_only": true,
            "local_only": true,
            "contains_raw_data": false,
            "contains_human_data": false,
            "contains_direct_identifiers": false,
            "contains_clinical_decision": false,
            "reconstruction_risk_milli": 10,
            "signer_id": format!("signer-{site_id}"),
            "signature_digest": hash.clone(),
            "attestation_digest": hash.clone()
        })
    };
    let output = call(
        &mut server,
        "glioma_federated_compute_capacity_exchange",
        json!({
            "request": {
                "workflow_id": "glioma-imaging",
                "workflow_envelope_digest": hash,
                "resource_units": 4,
                "estimated_duration_ticks": 10,
                "permitted_site_order": ["site-a", "site-b"],
                "summaries": [summary("site-a", 30), summary("site-b", 10)],
                "policy": {
                    "federation_policy_version": "federation-1",
                    "allowed_site_order": ["site-a", "site-b"],
                    "revoked_site_order": [],
                    "required_capability_order": ["cuda", "imaging"],
                    "required_localization_scope_order": ["organoid"],
                    "max_age_ticks": 10,
                    "expires_at_tick": 100,
                    "require_signed_summaries": false,
                    "require_aggregate_only": true,
                    "require_local_only": true,
                    "max_reconstruction_risk_milli": 100,
                    "max_estimated_cost_milli": 10000,
                    "policy_digest": hash
                },
                "current_tick": 5
            }
        }),
    );
    assert_eq!(
        output["envelope"]["feature_id"],
        json!("GAF-GLIOMA-P09-F32")
    );
    assert_eq!(
        output["envelope"]["placement_order"],
        json!(["site-b", "site-a"])
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_benchmark_campaign_replans_aggregate_sites_in_sandbox() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let campaign = call(
        &mut server,
        "glioma_federated_benchmark_campaign_execute",
        json!({
            "request": {
                "benchmark": {
                    "objective": "compare glioma invasion model improvements",
                    "capability_id": "glioma:invasion-model",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_auc",
                    "model_system": "organoid",
                    "minimum_sites": 2,
                    "minimum_replicates_per_site": 2,
                    "effect_threshold_milli": 25,
                    "max_i2_milli": 500,
                    "min_signal_to_noise_milli": 100,
                    "max_site_spread_milli": 500,
                    "max_leave_one_out_shift_milli": 500
                },
                "initial_sites": [{
                    "site_id": "site-seed",
                    "study_id": "study-seed",
                    "capability_id": "glioma:invasion-model",
                    "benchmark_world": "glioma-world-v1",
                    "metric_name": "holdout_auc",
                    "model_system": "organoid",
                    "artifact": {"artifact_id": "artifact-seed", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false},
                    "baseline_score_milli": 500,
                    "candidate_score_milli": 620,
                    "uncertainty_milli": 40,
                    "replicate_count": 3
                }],
                "actions": [{
                    "action_id": "expand-site",
                    "kind": "expand_coverage",
                    "target_site_id": null,
                    "cost_units": 1,
                    "expected_information_milli": 900,
                    "expected_effect_milli": 100,
                    "feasibility_milli": 900,
                    "risk_milli": 50,
                    "requested_replicates": 3
                }],
                "budget_units": 1,
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": false,
                "stop_on_negative": false
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(campaign["campaign"]["sites"].as_array().unwrap().len(), 2);
}

#[test]
fn glioma_federated_benchmark_operating_cycle_runs_boundary_consensus_and_campaign() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let cycle = call(
        &mut server,
        "glioma_federated_benchmark_operating_cycle",
        json!({
            "request": {
                "campaign": {
                    "benchmark": {
                        "objective": "compare glioma invasion model improvements",
                        "capability_id": "glioma:invasion-model",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "holdout_auc",
                        "model_system": "organoid",
                        "minimum_sites": 2,
                        "minimum_replicates_per_site": 2,
                        "effect_threshold_milli": 25,
                        "max_i2_milli": 500,
                        "min_signal_to_noise_milli": 100,
                        "max_site_spread_milli": 500,
                        "max_leave_one_out_shift_milli": 500
                    },
                    "initial_sites": [{"site_id": "site-seed-cycle", "study_id": "study-seed-cycle", "capability_id": "glioma:invasion-model", "benchmark_world": "glioma-world-v1", "metric_name": "holdout_auc", "model_system": "organoid", "artifact": {"artifact_id": "artifact-seed-cycle", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}, "baseline_score_milli": 500, "candidate_score_milli": 620, "uncertainty_milli": 40, "replicate_count": 3}],
                    "actions": [{"action_id": "expand-site-cycle", "kind": "expand_coverage", "target_site_id": null, "cost_units": 1, "expected_information_milli": 900, "expected_effect_milli": 100, "feasibility_milli": 900, "risk_milli": 50, "requested_replicates": 3}],
                    "budget_units": 1,
                    "max_rounds": 2,
                    "max_retries": 1,
                    "stop_on_qualified": false,
                    "stop_on_negative": false
                },
                "execution_mode": "local_simulation",
                "require_aggregate_only": true
            }
        }),
    );
    assert_eq!(cycle["dispatch"], json!("dry_run"));
    assert_eq!(cycle["simulation_only"], json!(true));
    assert_eq!(
        cycle["cycle"]["phase_order"][0],
        json!("aggregate_boundary")
    );
    assert_eq!(
        cycle["cycle"]["campaign"]["sites"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn glioma_federated_adaptive_campaign_bridges_projection_to_observed_aggregates() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let campaign = call(
        &mut server,
        "glioma_federated_adaptive_campaign_execute",
        json!({
            "request": {
                "planning": {
                    "benchmark": {
                        "objective": "validate an organoid invasion model across sites",
                        "capability_id": "glioma:invasion-model",
                        "benchmark_world": "glioma-world-v1",
                        "metric_name": "holdout_auc",
                        "model_system": "organoid",
                        "minimum_sites": 3,
                        "minimum_replicates_per_site": 3,
                        "effect_threshold_milli": 80,
                        "max_i2_milli": 250,
                        "min_signal_to_noise_milli": 500,
                        "max_site_spread_milli": 160,
                        "max_leave_one_out_shift_milli": 100
                    },
                    "current_sites": [
                        {"site_id":"site-a","study_id":"study-a","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"artifact-a","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":600,"uncertainty_milli":45,"replicate_count":4},
                        {"site_id":"site-b","study_id":"study-b","capability_id":"glioma:invasion-model","benchmark_world":"glioma-world-v1","metric_name":"holdout_auc","model_system":"organoid","artifact":{"artifact_id":"artifact-b","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"candidate_score_milli":615,"uncertainty_milli":45,"replicate_count":4}
                    ],
                    "candidates": [
                        {"candidate_id":"candidate-c","site_id":"candidate-site-c","study_id":"candidate-study-c","independence_group":"group-c","artifact":{"artifact_id":"candidate-artifact-c","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"expected_candidate_score_milli":625,"uncertainty_milli":30,"replicate_count":4,"cost_units":4,"privacy_risk_milli":100},
                        {"candidate_id":"candidate-d","site_id":"candidate-site-d","study_id":"candidate-study-d","independence_group":"group-d","artifact":{"artifact_id":"candidate-artifact-d","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"baseline_score_milli":500,"expected_candidate_score_milli":900,"uncertainty_milli":30,"replicate_count":4,"cost_units":9,"privacy_risk_milli":100}
                    ],
                    "budget_units": 12,
                    "max_new_sites": 2,
                    "beam_width": 8,
                    "privacy_budget_milli": 500,
                    "conservatism_milli": 500
                },
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": false,
                "stop_on_negative": false,
                "require_aggregate_only": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["simulation_only"], json!(true));
    assert!(!campaign["campaign"]["planned_site_order"].is_null());
    assert!(campaign["campaign"]["campaign"].is_object());
    assert_eq!(
        campaign["campaign"]["projected_disposition"],
        json!("qualified")
    );
    assert!(
        campaign["campaign"]["uncertainty"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap_or_default().contains("scenario"))
    );
}

#[test]
fn glioma_federated_mechanism_transport_preserves_model_and_direction_gates() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let analysis = call(
        &mut server,
        "glioma_federated_mechanism_transport",
        json!({
            "request": {
                "objective": "transport invasion mechanism across preclinical models",
                "mechanism_id": "invasion-mechanism",
                "target_model_system": "organoid",
                "target_signature": [0, 0, 0],
                "min_sites": 2,
                "min_replicates_per_site": 2,
                "min_quality_milli": 700,
                "similarity_scale_milli": 1000,
                "effect_threshold_milli": 500,
                "min_signal_to_noise_milli": 500,
                "max_heterogeneity_milli": 250,
                "max_site_spread_milli": 1000,
                "max_leave_one_out_shift_milli": 500,
                "require_target_model": true
            },
            "sites": [
                {"site_id": "organoid-a", "study_id": "study-a", "mechanism_id": "invasion-mechanism", "model_system": "organoid", "effect_milli": 900, "uncertainty_milli": 100, "quality_milli": 900, "replicate_count": 3, "population_signature": [0, 0, 0], "artifact": {"artifact_id": "artifact-a", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}},
                {"site_id": "mouse-b", "study_id": "study-b", "mechanism_id": "invasion-mechanism", "model_system": "mouse_model", "effect_milli": 850, "uncertainty_milli": 100, "quality_milli": 900, "replicate_count": 3, "population_signature": [1, 0, 0], "artifact": {"artifact_id": "artifact-b", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}}
            ]
        }),
    );
    assert_eq!(analysis["dispatch"], json!("not_started"));
    assert_eq!(analysis["analysis"]["disposition"], json!("qualified"));
    assert_eq!(
        analysis["analysis"]["included_order"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        analysis["analysis"]["model_coverage"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
}

#[test]
fn glioma_federated_mechanism_transport_campaign_replays_aggregate_follow_up() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let campaign = call(
        &mut server,
        "glioma_federated_mechanism_transport_campaign_execute",
        json!({
            "request": {
                "transport": {
                    "objective": "transport invasion mechanism across organoid sites",
                    "mechanism_id": "invasion-mechanism",
                    "target_model_system": "organoid",
                    "target_signature": [0, 0],
                    "min_sites": 2,
                    "min_replicates_per_site": 1,
                    "min_quality_milli": 500,
                    "similarity_scale_milli": 1000,
                    "effect_threshold_milli": 100,
                    "min_signal_to_noise_milli": 1,
                    "max_heterogeneity_milli": 800,
                    "max_site_spread_milli": 1000,
                    "max_leave_one_out_shift_milli": 1000,
                    "require_target_model": true
                },
                "initial_sites": [
                    {"site_id": "organoid-a", "study_id": "study-a", "mechanism_id": "invasion-mechanism", "model_system": "organoid", "effect_milli": 500, "uncertainty_milli": 100, "quality_milli": 900, "replicate_count": 2, "population_signature": [0, 0], "artifact": {"artifact_id": "artifact-a", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}},
                    {"site_id": "organoid-b", "study_id": "study-b", "mechanism_id": "invasion-mechanism", "model_system": "organoid", "effect_milli": 520, "uncertainty_milli": 100, "quality_milli": 900, "replicate_count": 2, "population_signature": [0, 0], "artifact": {"artifact_id": "artifact-b", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}}
                ],
                "actions": [{
                    "action_id": "replicate-organoid-c",
                    "target_site_id": "organoid-c",
                    "model_system": "organoid",
                    "population_signature": [0, 0],
                    "cost_units": 1,
                    "expected_information_milli": 900,
                    "expected_effect_milli": 510,
                    "expected_heterogeneity_reduction_milli": 500,
                    "feasibility_milli": 900,
                    "risk_milli": 20,
                    "requested_replicates": 2
                }],
                "budget_units": 2,
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": false,
                "stop_on_negative": false
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(
        campaign["campaign"]["feature_id"],
        json!("GAF-GLIOMA-P12-F28")
    );
    assert_eq!(campaign["campaign"]["sites"].as_array().unwrap().len(), 3);
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_federated_decision_capsule_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let context_body = json!({
        "capsule_id": "capsule-mcp",
        "source_site_id": "site-a",
        "objective": "prioritize glioma mechanism validation",
        "context_digest": hash.clone(),
        "claim_order": ["claim-a"],
        "evidence_coverage_order": ["coverage-a"],
        "omission_order": ["missing-spatial-coverage"],
        "downstream_action_order": ["action-a"],
        "uncertainty_order": ["uncertainty-a"],
        "generated_tick": 1,
        "expires_at_tick": 100,
        "local_only": true,
        "contains_raw_data": false,
        "contains_human_data": false,
        "contains_direct_identifiers": false,
        "contains_clinical_decision": false,
    });
    let signature = bioprism_ids::ContentHash::of_value(&context_body)
        .unwrap()
        .to_string();
    let output = call(
        &mut server,
        "glioma_federated_decision_capsule",
        json!({
            "request": {
                "question_scope": "preclinical organoid invasion",
                "context": {
                    "capsule_id": "capsule-mcp",
                    "source_site_id": "site-a",
                    "objective": "prioritize glioma mechanism validation",
                    "context_digest": hash.clone(),
                    "claim_order": ["claim-a"],
                    "evidence_coverage_order": ["coverage-a"],
                    "omission_order": ["missing-spatial-coverage"],
                    "downstream_action_order": ["action-a"],
                    "uncertainty_order": ["uncertainty-a"],
                    "generated_tick": 1,
                    "expires_at_tick": 100,
                    "local_only": true,
                    "contains_raw_data": false,
                    "contains_human_data": false,
                    "contains_direct_identifiers": false,
                    "contains_clinical_decision": false,
                    "signature_digest": signature
                },
                "policy": {
                    "policy_version": "policy-1",
                    "allowed_site_order": ["site-a"],
                    "revoked_site_order": [],
                    "allowed_action_order": ["action-a"],
                    "max_age_ticks": 10,
                    "expires_at_tick": 100,
                    "require_local_only": true,
                    "require_no_raw_data": true,
                    "policy_digest": hash.clone()
                },
                "current_tick": 5
            }
        }),
    );
    assert_eq!(output["capsule"]["feature_id"], json!("GAF-GLIOMA-P04-F08"));
    assert_eq!(output["capsule"]["disposition"], json!("accepted"));
    assert_eq!(
        output["capsule"]["omission_order"],
        json!(["missing-spatial-coverage"])
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_cross_site_protocol_conformance_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |value: &Value| {
        bioprism_ids::ContentHash::of_value(value)
            .unwrap()
            .to_string()
    };
    let steps = json!([
        {"step_id":"capture","semantic_role":"role-capture","unit":"millivolt","value_milli":100,"tolerance_milli":20,"required_capability":"imaging"},
        {"step_id":"expose","semantic_role":"role-expose","unit":"millivolt","value_milli":200,"tolerance_milli":20,"required_capability":"imaging"}
    ]);
    let reference_body = json!({
        "protocol_id":"glioma-invasion",
        "version":"2026.1",
        "step_order":steps,
        "required_calibration_class":"cal-v2",
        "expires_at_tick":100
    });
    let reference_digest = hash(&reference_body);
    let site_body = json!({
        "site_id":"site-a",
        "protocol_id":"glioma-invasion",
        "version":"2026.1",
        "step_order":reference_body["step_order"].clone(),
        "capability_order":["imaging"],
        "calibration_class":"cal-v2",
        "calibration_valid_until_tick":100,
        "adaptation_order":[]
    });
    let site_digest = hash(&site_body);
    let output = call(
        &mut server,
        "glioma_cross_site_protocol_conformance",
        json!({
            "request": {
                "reference": {
                    "protocol_id":"glioma-invasion",
                    "version":"2026.1",
                    "step_order":reference_body["step_order"],
                    "required_calibration_class":"cal-v2",
                    "expires_at_tick":100,
                    "protocol_digest":reference_digest
                },
                "site_order": [{
                    "site_id":"site-a",
                    "protocol_id":"glioma-invasion",
                    "version":"2026.1",
                    "step_order":site_body["step_order"],
                    "capability_order":["imaging"],
                    "calibration_class":"cal-v2",
                    "calibration_valid_until_tick":100,
                    "adaptation_order":[],
                    "descriptor_digest":site_digest
                }],
                "current_tick":5,
                "minimum_quorum":1,
                "allow_bounded_adaptations":true
            }
        }),
    );
    assert_eq!(output["matrix"]["feature_id"], json!("GAF-GLIOMA-P08-F28"));
    assert_eq!(output["matrix"]["pooling_permitted"], json!(true));
    assert_eq!(
        output["matrix"]["results"][0]["status"],
        json!("conformant")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_device_capability_manifest_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |value: &Value| {
        bioprism_ids::ContentHash::of_value(value)
            .unwrap()
            .to_string()
    };
    let unsigned = json!({
        "site_id":"site-a",
        "device_id":"device-a",
        "device_class":"high-content-imager",
        "capabilities":[{"assay_id":"invasion-imaging","protocol_version":"2026.1","max_parallel_units":2,"required_calibration_class":"cal-v2"}],
        "availability":[{"start_tick":10,"end_tick":100}],
        "calibration_valid_until_tick":80,
        "calibration_digest":hash(&json!("calibration-a")),
        "attestation_digest":hash(&json!("attestation-a")),
        "signing_key_id":"site-key-1",
        "policy_digest":hash(&json!("policy-a")),
        "issued_tick":1,
        "expires_tick":90,
        "current_tick":20,
        "revoked":false,
        "revocation_digest":null,
        "data_locality":"aggregate_metadata",
        "secrets_excluded":true,
        "raw_sample_identifiers_excluded":true
    });
    let output = call(
        &mut server,
        "glioma_federated_device_capability_manifest",
        json!({
            "request": {
                "site_id":unsigned["site_id"],
                "device_id":unsigned["device_id"],
                "device_class":unsigned["device_class"],
                "capabilities":unsigned["capabilities"],
                "availability":unsigned["availability"],
                "calibration_valid_until_tick":unsigned["calibration_valid_until_tick"],
                "calibration_digest":unsigned["calibration_digest"],
                "attestation_digest":unsigned["attestation_digest"],
                "signing_key_id":unsigned["signing_key_id"],
                "signature_digest":hash(&unsigned),
                "policy_digest":unsigned["policy_digest"],
                "issued_tick":unsigned["issued_tick"],
                "expires_tick":unsigned["expires_tick"],
                "current_tick":unsigned["current_tick"],
                "revoked":false,
                "revocation_digest":null,
                "data_locality":unsigned["data_locality"],
                "secrets_excluded":true,
                "raw_sample_identifiers_excluded":true
            }
        }),
    );
    assert_eq!(
        output["manifest"]["feature_id"],
        json!("GAF-GLIOMA-P08-F08")
    );
    assert_eq!(output["manifest"]["availability_state"], json!("available"));
    assert_eq!(output["manifest"]["schedulable"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_instrument_operations_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |value: &Value| {
        bioprism_ids::ContentHash::of_value(value)
            .unwrap()
            .to_string()
    };
    let site_body = |site_id: &str| {
        json!({
            "site_id":site_id,
            "capability_manifest_digest":hash(&json!("manifest")),
            "protocol_conformance_digest":hash(&json!("protocol")),
            "calibration_class":"cal-v2",
            "service_capacity_units":100,
            "available_capacity_units":60,
            "observed_tick":90,
            "expires_tick":150,
            "privacy_count":8,
            "revoked":false,
            "raw_data_excluded":true,
            "credentials_excluded":true,
            "approved_for_exchange":true
        })
    };
    let site_a = site_body("site-a");
    let site_b = site_body("site-b");
    let output = call(
        &mut server,
        "glioma_federated_instrument_operations",
        json!({
            "request": {
                "sites":[
                    {"site_id":site_a["site_id"],"capability_manifest_digest":site_a["capability_manifest_digest"],"protocol_conformance_digest":site_a["protocol_conformance_digest"],"calibration_class":"cal-v2","service_capacity_units":100,"available_capacity_units":60,"observed_tick":90,"expires_tick":150,"privacy_count":8,"revoked":false,"raw_data_excluded":true,"credentials_excluded":true,"approved_for_exchange":true,"summary_digest":hash(&site_a)},
                    {"site_id":site_b["site_id"],"capability_manifest_digest":site_b["capability_manifest_digest"],"protocol_conformance_digest":site_b["protocol_conformance_digest"],"calibration_class":"cal-v2","service_capacity_units":100,"available_capacity_units":60,"observed_tick":90,"expires_tick":150,"privacy_count":8,"revoked":false,"raw_data_excluded":true,"credentials_excluded":true,"approved_for_exchange":true,"summary_digest":hash(&site_b)}
                ],
                "current_tick":100,
                "minimum_privacy_count":5,
                "maximum_staleness_ticks":20,
                "minimum_eligible_sites":2,
                "allow_metadata_exchange":true
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P08-F32")
    );
    assert_eq!(output["snapshot"]["sharing_permitted"], json!(true));
    assert_eq!(
        output["snapshot"]["aggregate_available_capacity_units"],
        json!(120)
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_release_compile_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let contribution = |site: &str| {
        json!({
            "site_id": site,
            "contribution_id": format!("contribution-{site}"),
            "aggregate_digest": zero,
            "schema_version": "aggregate-1",
            "policy_digest": zero,
            "issued_epoch": 90,
            "expires_epoch": 110,
            "aggregate_only": true,
            "raw_data_local": true,
            "contains_human_data": false,
            "permitted": true,
            "localization_statement": format!("raw data local to {site}"),
            "uncertainty_milli": 200,
            "heterogeneity_milli": 300,
            "source_count": 4,
            "limitation_order": ["preclinical"]
        })
    };
    let output = call(
        &mut server,
        "glioma_federated_release_compile",
        json!({"request": {
            "research_id": "research-mcp",
            "study_id": "study-mcp",
            "object_version": "v1",
            "contributions": [contribution("site-a"), contribution("site-b")],
            "policy": {
                "required_schema_version": "aggregate-1",
                "required_policy_digest": zero,
                "now_epoch": 100,
                "required_quorum": 2,
                "max_uncertainty_milli": 500,
                "max_heterogeneity_milli": 500,
                "require_localization_statement": true
            }
        }}),
    );
    assert_eq!(output["object"]["feature_id"], json!("GAF-GLIOMA-P11-F16"));
    assert_eq!(output["object"]["disposition"], json!("ready_for_review"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_release_sharing_gate_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let contribution = |site: &str| json!({"site_id":site,"contribution_id":format!("sharing-{site}"),"aggregate_digest":zero,"schema_version":"aggregate-1","policy_digest":zero,"issued_epoch":90,"expires_epoch":110,"aggregate_only":true,"raw_data_local":true,"contains_human_data":false,"permitted":true,"localization_statement":format!("raw data local to {site}"),"uncertainty_milli":100,"heterogeneity_milli":100,"source_count":4,"limitation_order":["preclinical"]});
    let compiled = call(
        &mut server,
        "glioma_federated_release_compile",
        json!({"request":{"research_id":"sharing-research","study_id":"sharing-study","object_version":"v1","contributions":[contribution("site-a"),contribution("site-b")],"policy":{"required_schema_version":"aggregate-1","required_policy_digest":zero,"now_epoch":100,"required_quorum":2,"max_uncertainty_milli":500,"max_heterogeneity_milli":500,"require_localization_statement":true}}}),
    );
    let output = call(
        &mut server,
        "glioma_federated_release_sharing_check",
        json!({"request":{"object":compiled["object"],"fields":[{"field_id":"effect","source_site_id":"site-a","aggregate_only":true,"local_only":false,"contains_human_data":false,"policy_permitted":true,"localization_statement":"site-a local","recipient_scope_order":["consortium"]},{"field_id":"uncertainty","source_site_id":"site-b","aggregate_only":true,"local_only":false,"contains_human_data":false,"policy_permitted":true,"localization_statement":"site-b local","recipient_scope_order":["consortium"]}],"policy":{"recipient_scope":"consortium","required_quorum":2,"now_epoch":100,"revoked_site_order":[],"denied_field_order":[],"permit_redaction":true,"require_localization":true}}}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(
        output["decision"]["feature_id"],
        json!("GAF-GLIOMA-P11-F28")
    );
    assert_eq!(output["decision"]["disposition"], json!("shareable"));
    assert_eq!(
        output["decision"]["share_order"],
        json!(["effect", "uncertainty"])
    );
}

#[test]
fn glioma_consortium_publication_steward_is_reachable_through_mcp() {
    let mut server = server();
    let digest = ContentHash::of_value(&json!({"object": "consortium"}))
        .unwrap()
        .to_string();
    let output = call(
        &mut server,
        "glioma_consortium_publication_steward",
        json!({"request": {
            "candidate_id":"candidate-consortium",
            "object_digest":digest,
            "sites":[
                {"site_id":"site-a","object_digest":digest,"decision":"approve","authority_active":true,"authority_revoked":false,"signed":true,"dissent_reason":null,"correction_digest":null},
                {"site_id":"site-b","object_digest":digest,"decision":"reject","authority_active":true,"authority_revoked":false,"signed":true,"dissent_reason":"replication concern","correction_digest":null}
            ],
            "required_quorum":1,
            "allow_partial_publication":false
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["state"]["feature_id"], json!("GAF-GLIOMA-P11-F32"));
    assert_eq!(output["state"]["disposition"], json!("ready"));
    assert_eq!(output["state"]["dissent_order"], json!(["site-b"]));
}

#[test]
fn glioma_site_capability_envelope_compile_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_site_capability_envelope_compile",
        json!({"request": {
            "site_id": "site-mcp",
            "registry_version": "registry-1",
            "capabilities": [{
                "capability_id": "cap-imaging",
                "model_system": "organoid",
                "assay_class": "imaging",
                "standard_order": ["ome-ngff-0.5"],
                "compute_class": "gpu-local",
                "review_capacity": 4,
                "confidence_milli": 900,
                "valid_until_epoch": 110,
                "approved": true,
                "attestation_digest": zero,
                "local_only": true
            }],
            "policy": {
                "now_epoch": 100,
                "required_model_systems": ["organoid"],
                "required_assay_classes": ["imaging"],
                "required_standards": ["ome-ngff-0.5"],
                "min_confidence_milli": 800,
                "require_local_only": true
            }
        }}),
    );
    assert_eq!(
        output["envelope"]["feature_id"],
        json!("GAF-GLIOMA-P12-F05")
    );
    assert_eq!(output["envelope"]["disposition"], json!("eligible"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_cross_site_evidence_explore_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let observation = |id: &str, value_milli: i64| {
        json!({
            "observation_id":id,
            "cell_key":"organoid|invasion|imaging|day7",
            "model_system":"organoid",
            "assay":"invasion",
            "method":"imaging",
            "window":"day7",
            "value_milli":value_milli,
            "uncertainty_milli":50,
            "aggregate_site_count":3,
            "suppressed":false,
            "comparable":true,
            "revoked":false,
            "mapping_confidence_milli":900,
            "provenance_digest":zero
        })
    };
    let output = call(
        &mut server,
        "glioma_cross_site_evidence_explore",
        json!({
            "request": {
                "objective":"compare glioma invasion across sites",
                "observations":[observation("obs-a",100),observation("obs-b",200)],
                "suppression_threshold_sites":2,
                "minimum_mapping_confidence_milli":700,
                "max_cells":8
            }
        }),
    );
    assert_eq!(output["view"]["feature_id"], json!("GAF-GLIOMA-P12-F18"));
    assert_eq!(
        output["view"]["cells"][0]["disposition"],
        json!("available")
    );
    assert_eq!(
        output["view"]["cells"][0]["aggregate_value_milli"],
        json!(150)
    );
    assert_eq!(output["view"]["indirect_query_protection"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_site_participation_review_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_site_participation_review",
        json!({
            "request": {
                "benchmark_id": "benchmark-mcp",
                "site_id": "site-a",
                "research_purpose": "compare invasion phenotypes across preclinical models",
                "policy_allows_purpose": true,
                "requested_field_order": ["effect", "uncertainty"],
                "requested_model_order": ["organoid", "xenograft"],
                "requested_assay": "organoid-imaging",
                "required_protocol_version": "protocol-v1",
                "local_protocol_version": "protocol-v1",
                "local_field_order": ["effect", "uncertainty"],
                "local_model_order": ["organoid", "xenograft"],
                "local_assay_order": ["organoid-imaging"],
                "proposed_workload_units": 20,
                "workload_capacity_units": 100,
                "proposed_privacy_cost_milli": 50,
                "privacy_budget_milli": 500,
                "approval_recorded": true,
                "withdrawal_requested": false,
                "withdrawal_reason": null,
                "aggregate_only": true,
                "raw_data_local": true,
                "contains_human_data": false,
                "contains_direct_identifiers": false
            }
        }),
    );
    assert_eq!(output["review"]["feature_id"], json!("GAF-GLIOMA-P12-F17"));
    assert_eq!(output["review"]["disposition"], json!("approved"));
    assert_eq!(output["review"]["query_dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federation_operations_snapshot_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_federation_operations_snapshot",
        json!({
            "request": {
                "federation_id": "consortium-mcp",
                "current_tick": 100,
                "heartbeat_max_age_ticks": 10,
                "queue_pressure_threshold_milli": 800,
                "required_standard_order": ["cwl-1.2", "prov-o-2013"],
                "sites": [{
                    "site_id": "site-a",
                    "capability_manifest_digest": zero,
                    "standards_order": ["cwl-1.2", "prov-o-2013"],
                    "service_available": true,
                    "heartbeat_tick": 95,
                    "revoked": false,
                    "maintenance": false,
                    "local_policy_failover_allowed": true,
                    "open_incident_order": [],
                    "incident_severity_milli": 0,
                    "queue_depth": 10,
                    "queue_capacity": 100
                }]
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P12-F32")
    );
    assert_eq!(output["snapshot"]["ready_site_order"], json!(["site-a"]));
    assert_eq!(output["snapshot"]["dispatch_permitted"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_federated_benchmark_record_execute_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_federated_benchmark_record_execute",
        json!({
            "request": {
                "benchmark_id": "benchmark-mcp",
                "benchmark_version": "2026.1",
                "policy_scope": "glioma-v1",
                "executor_version": "executor-mcp-1",
                "replay_identity": "0".repeat(64),
                "current_epoch": 100,
                "max_staleness_epochs": 10,
                "required_quorum": 1,
                "analysis_digest": "1".repeat(64),
                "uncertainty_digest": "2".repeat(64),
                "contributions": [{
                    "site_id": "site-a",
                    "benchmark_id": "benchmark-mcp",
                    "benchmark_version": "2026.1",
                    "policy_scope": "glioma-v1",
                    "attestation_digest": "3".repeat(64),
                    "aggregate_digest": "4".repeat(64),
                    "metric_digest": "5".repeat(64),
                    "uncertainty_milli": 120,
                    "observed_epoch": 95,
                    "valid_until_epoch": 110,
                    "signed": true,
                    "signer_revoked": false,
                    "aggregate_only": true,
                    "raw_data_local": true,
                    "contains_human_data": false,
                    "contains_direct_identifiers": false
                }]
            }
        }),
    );
    assert_eq!(output["record"]["feature_id"], json!("GAF-GLIOMA-P12-F08"));
    assert_eq!(output["record"]["status"], json!("completed"));
    assert_eq!(output["record"]["included_site_order"], json!(["site-a"]));
    assert_eq!(output["dispatch"], json!("not_started"));
}
