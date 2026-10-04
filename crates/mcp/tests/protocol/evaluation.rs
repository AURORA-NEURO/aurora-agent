//! MCP contract tests for evaluation contracts.

use super::*;

#[test]
fn pack_health_assessment_exposes_digest_bound_score_refusal() {
    let mut server = server();
    let observations = bioprism_packs::Observations {
        calibration: DifficultyCalibration::new(
            [
                ("system-a", 99, 100),
                ("system-b", 98, 100),
                ("system-c", 100, 100),
            ]
            .into_iter()
            .map(|(system, passes, trials)| SystemObservation::new(system, passes, trials).unwrap())
            .collect(),
        ),
        ..Default::default()
    };
    let payload = call(
        &mut server,
        "pack_health_assess",
        json!({
            "pack": serde_json::to_value(protocol_pack_fixture()).unwrap(),
            "observations": serde_json::to_value(observations).unwrap(),
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["verdict"], json!("unreportable"));
    assert_eq!(payload["score_gate"]["reportable"], json!(false));
    assert!(payload["pack_digest"].is_string());
    assert!(
        payload["health"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| { finding["finding"] == json!("saturated") })
    );
}

#[test]
fn prism_minimize_preserves_the_oracle_signature_and_states_the_guarantee() {
    let mut server = server();
    let payload = call(
        &mut server,
        "prism_minimize",
        json!({
            "world": "fixtures/generated/discriminating_world.json",
            "facts": []
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["preserved"], json!(true));
    assert!(
        payload["minimization"]["guarantee"]
            .as_str()
            .unwrap()
            .contains("minimal")
    );
    assert!(payload["preservation"]["preservation"] == json!("preserved"));
}

#[test]
fn capability_rank_preserves_dominance_tradeoffs_and_condition_refusals() {
    let mut server = server();
    let dominated = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "max_items": 10
        }),
    );
    assert_eq!(dominated["ok"], json!(true));
    assert_eq!(dominated["partial_order"]["is_total"], json!(true));
    assert_eq!(
        dominated["partial_order"]["relations"][0]["dominance"]["dominance"],
        json!("left_dominates")
    );
    assert_eq!(
        dominated["partial_order"]["maximal_systems"],
        json!(["system-a"])
    );

    let tradeoff = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.6, "pack/4"),
                metric_vector("system-b", 0.7, 0.8, "pack/4")
            ]
        }),
    );
    assert_eq!(tradeoff["partial_order"]["is_total"], json!(false));
    assert_eq!(tradeoff["partial_order"]["unresolved_count"], json!(1));
    assert_eq!(
        tradeoff["partial_order"]["unresolved"][0]["dominance"]["because"]["because"],
        json!("trade_off")
    );

    let mismatched = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.9, 0.8, "pack/5")
            ]
        }),
    );
    assert_eq!(mismatched["ok"], json!(true));
    assert_eq!(
        mismatched["partial_order"]["unresolved"][0]["dominance"]["because"]["because"],
        json!("conditions_differ")
    );

    let policy = json!({
        "intended_use": "reference comparison",
        "weights": { "verify.oracle": 1.0, "safety.boundary": 1.0 }
    });
    let weighting = json!({
        "policy": policy,
        "digest": ContentHash::of_value(&json!({
            "intended_use": "reference comparison",
            "weights": { "verify.oracle": 1.0, "safety.boundary": 1.0 }
        })).unwrap()
    });
    let weighted = call(
        &mut server,
        "capability_rank",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "weighting": weighting
        }),
    );
    assert_eq!(weighted["ok"], json!(true));
    assert_eq!(weighted["total_order"]["leaders"], json!(["system-a"]));
    assert_eq!(weighted["total_order"]["overwrote_a_refusal"], json!(false));
    assert!(weighted["rank_instability"]["instability"].is_object());
}

#[test]
fn metrics_profile_audit_keeps_unmeasured_capabilities_and_uncontested_leads_visible() {
    let mut server = server();
    let mut incomplete = metric_vector("system-b", 0.7, 0.6, "pack/4");
    incomplete["grid"]["cells"]
        .as_object_mut()
        .unwrap()
        .remove("safety.boundary");
    let payload = call(
        &mut server,
        "metrics_profile_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                incomplete
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["summary"]["capability_count"], json!(2));
    assert_eq!(payload["summary"]["uncontested_lead_count"], json!(1));
    let rows = payload["per_capability"]["rows"].as_array().unwrap();
    let safety = rows
        .iter()
        .find(|row| row["capability"] == json!("safety.boundary"))
        .expect("safety row is retained");
    assert_eq!(safety["best"], json!(["system-a"]));
    assert_eq!(safety["unmeasured_for"], json!(["system-b"]));
    assert_eq!(safety["lead_is_uncontested"], json!(true));
    let system_b = payload["per_system"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["system"] == json!("system-b"))
        .unwrap();
    assert_eq!(system_b["unmeasured_capability_count"], json!(1));
}

#[test]
fn metrics_analytics_audit_keeps_domains_missingness_and_paired_contrasts_typed() {
    let mut server = server();
    let payload = call(
        &mut server,
        "metrics_analytics_audit",
        json!({
            "observations": [
                {
                    "id": "verification-1",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.80,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "replicate_group": "world-1",
                    "cost": 4.0,
                    "latency_ms": 20.0,
                    "evidence": "observed"
                },
                {
                    "id": "verification-2",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.90,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "replicate_group": "world-2",
                    "cost": 5.0,
                    "latency_ms": 25.0,
                    "evidence": "reproduced"
                },
                {
                    "id": "verification-missing",
                    "dimension": "verification",
                    "domain": "oncology",
                    "system": "agent-a",
                    "value": 0.0,
                    "direction": "higher_is_better",
                    "unit": "fraction",
                    "condition": "pack/4",
                    "evidence": "missing"
                }
            ],
            "pairs": [
                {
                    "id": "robustness-1",
                    "dimension": "robustness",
                    "domain": "oncology",
                    "baseline": 0.90,
                    "variant": 0.72,
                    "direction": "higher_is_better",
                    "tolerance": 0.20,
                    "evidence": "observed"
                },
                {
                    "id": "cross-modal-1",
                    "dimension": "cross_modal_consistency",
                    "domain": "oncology",
                    "baseline": 0.80,
                    "variant": 0.82,
                    "direction": "higher_is_better",
                    "tolerance": 0.05,
                    "evidence": "reproduced"
                }
            ],
            "calibration": [
                { "id": "forecast-1", "domain": "oncology", "predicted": 0.9, "observed": 1.0, "evidence": "observed" },
                { "id": "forecast-2", "domain": "oncology", "predicted": 0.1, "observed": 0.0, "evidence": "declared" }
            ],
            "calibration_bins": 5
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["workflow"], json!("metrics_descriptive_analytics"));
    assert_eq!(payload["coverage"]["measured_observations"], json!(2));
    assert_eq!(payload["coverage"]["excluded_observations"], json!(1));
    assert_eq!(payload["dimensions"][0]["values"]["count"], json!(2));
    assert_eq!(payload["paired"].as_array().unwrap().len(), 2);
    assert_eq!(payload["calibration"]["measured"], json!(1));
    assert!(payload["caveats"].as_array().unwrap().len() >= 4);
}

#[test]
fn metrics_analytics_audit_refuses_mixed_direction_with_a_structured_tool_error() {
    let mut server = server();
    let response = call(
        &mut server,
        "metrics_analytics_audit",
        json!({
            "observations": [
                {
                    "id": "one", "dimension": "latency", "domain": "runtime", "system": "a",
                    "value": 10.0, "direction": "lower_is_better", "unit": "ms", "condition": "v1", "evidence": "observed"
                },
                {
                    "id": "two", "dimension": "latency", "domain": "runtime", "system": "a",
                    "value": 0.5, "direction": "higher_is_better", "unit": "fraction", "condition": "v1", "evidence": "observed"
                }
            ]
        }),
    );
    assert_eq!(response["ok"], json!(false));
    assert!(
        response["error"]
            .as_str()
            .unwrap()
            .contains("metrics analytics refused")
    );
}

#[test]
fn adaptive_panel_preserves_clustered_audit_and_selection_refusals() {
    let panel = AdaptivePanel::new(PanelConfig::default());
    let result = call(
        &mut server(),
        "adaptive_panel",
        json!({
            "panel": serde_json::to_value(panel).unwrap(),
            "candidates": [{
                "instance": "inst-1",
                "capability": "capability-a",
                "parent": "parent-1",
                "cost": 1.0
            }],
            "capability": "capability-a"
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/adaptive-panel/0.1"));
    assert_eq!(result["audit_summary"]["trials"], json!(0));
    assert_eq!(result["selection"]["ok"], json!(true));
    assert_eq!(
        result["selection"]["value"]["record"]["chosen"]["instance"],
        json!("inst-1")
    );
    assert_eq!(result["capability"]["estimate"], Value::Null);
    assert!(
        result["capability"]["estimate_refusal"]
            .as_str()
            .unwrap()
            .contains("no recorded trials")
    );
}

#[test]
fn bioeval_reference_audit_validates_mass_and_preserves_distributed_truth() {
    let distribution = ReferenceDistribution::new(
        [
            ("progression".to_string(), 0.6),
            ("stable".to_string(), 0.4),
        ],
        Dispersion::Mixed {
            aleatoric_fraction: 0.5,
        },
    )
    .unwrap();
    let result = call(
        &mut server(),
        "bioeval_reference_audit",
        json!({
            "reference": serde_json::to_value(ReferenceStandard::Distribution(distribution)).unwrap(),
            "state": "progression"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-reference-audit/0.1")
    );
    assert_eq!(result["reference_kind"], json!("distribution"));
    assert_eq!(result["can_certify_clean_pass"], json!(false));
    assert_eq!(result["modal_state"], json!("progression"));
    assert_eq!(result["modal_mass"], json!(0.6));
    assert_eq!(result["queried_state_mass"], json!(0.6));
    assert_eq!(result["dispersion"], json!("mixed"));
    assert!(result["entropy_bits"].as_f64().unwrap() > 0.9);

    let invalid = call(
        &mut server(),
        "bioeval_reference_audit",
        json!({
            "reference": {
                "standard": "distribution",
                "mass": {"progression": 0.8},
                "dispersion": {"kind": "aleatoric"}
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].is_string());
}

#[test]
fn bioeval_acquisition_audit_preserves_obligation_stopping_and_named_regret() {
    let result = call(
        &mut server(),
        "bioeval_acquisition_audit",
        json!({
            "obligations": [
                { "id": "subtype", "required": true },
                { "id": "context", "required": false }
            ],
            "actions": [
                { "id": "read-notes", "kind": "metadata", "cost": 2, "closes": ["context"] },
                { "id": "search", "kind": "retrieval", "cost": 5, "closes": [] },
                { "id": "panel", "kind": "assay", "cost": 40, "closes": ["subtype"] },
                { "id": "extra", "kind": "analysis", "cost": 1, "closes": [] }
            ],
            "stopped_after": true,
            "reference_policy": { "name": "random-acquisition", "cost": 30, "admissible": false }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-acquisition-audit/0.1")
    );
    assert_eq!(result["status"], json!("admissible"));
    assert_eq!(result["required_open_count"], json!(0));
    assert_eq!(result["cost"], json!(48));
    assert_eq!(result["findings"]["deferred_decisive_cost"], json!(7));
    assert_eq!(
        result["findings"]["redundant_action_ids"],
        json!(["extra", "search"])
    );
    assert_eq!(
        result["findings"]["unnecessary_action_ids"],
        json!(["extra"])
    );
    assert_eq!(result["regret"]["cost_difference"], json!(18));
    assert_eq!(result["regret"]["like_for_like"], json!(false));

    let missing_reference = call(
        &mut server(),
        "bioeval_acquisition_audit",
        json!({
            "obligations": [],
            "actions": [],
            "require_reference": true
        }),
    );
    assert_eq!(missing_reference["__isError"], json!(false));
    assert_eq!(missing_reference["ok"], json!(false));
    assert_eq!(missing_reference["stage"], json!("reference_policy"));
    assert_eq!(missing_reference["fail_closed"], json!(true));
}

#[test]
fn bioeval_grounding_audit_preserves_states_locators_staleness_and_lineage() {
    let result = call(
        &mut server(),
        "bioeval_grounding_audit",
        json!({
            "claims": [
                { "id": "supported" },
                { "id": "contested" },
                { "id": "unverified" },
                { "id": "contradicted" },
                { "id": "unsupported" }
            ],
            "evidence": [
                { "id": "shown", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-1"], "locator_status": { "locator": "resolved", "digest": "sha256:shown" } },
                { "id": "changed", "last_modified": "2026-06-01T00:00:00Z", "lineage": ["specimen-2"], "locator_status": { "locator": "resolved", "digest": "sha256:changed" } },
                { "id": "asserted", "last_modified": "2026-01-01T00:00:00Z", "locator_status": { "locator": "not_checked" } },
                { "id": "opposed", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-3"], "locator_status": { "locator": "unresolvable", "detail": "fixture missing" } },
                { "id": "adjacent", "last_modified": "2026-01-01T00:00:00Z", "lineage": ["specimen-4"], "locator_status": { "locator": "resolved", "digest": "sha256:adjacent" } },
                { "id": "orphan", "last_modified": "2026-06-01T00:00:00Z", "locator_status": { "locator": "not_checked" } }
            ],
            "edges": [
                { "claim": "supported", "evidence": "shown", "kind": "supports" },
                { "claim": "contested", "evidence": "changed", "kind": "supports" },
                { "claim": "contested", "evidence": "opposed", "kind": "contradicts" },
                { "claim": "unverified", "evidence": "asserted", "kind": "supports" },
                { "claim": "contradicted", "evidence": "opposed", "kind": "contradicts" },
                { "claim": "unsupported", "evidence": "adjacent", "kind": "adjacent" }
            ],
            "stale_against": "2026-03-01T00:00:00Z",
            "max_items": 3
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-grounding-audit/0.1")
    );
    assert_eq!(result["census"]["supported"], json!(1));
    assert_eq!(result["census"]["contested"], json!(1));
    assert_eq!(result["census"]["support_unverified"], json!(1));
    assert_eq!(result["census"]["contradicted"], json!(1));
    assert_eq!(result["census"]["unsupported"], json!(1));
    assert_eq!(result["census"]["adjacent_citations"], json!(1));
    assert_eq!(result["census"]["fully_grounded"], json!(false));
    assert_eq!(result["staleness"]["stale_count"], json!(2));
    assert_eq!(
        result["findings"]["lineage_gap_evidence"]["ids"],
        json!(["asserted", "orphan"])
    );
    assert_eq!(
        result["findings"]["orphan_evidence"]["ids"],
        json!(["orphan"])
    );
    assert_eq!(result["claims"]["omitted"], json!(2));
    assert_eq!(result["graph"]["duplicate_edge_count"], json!(0));

    let invalid = call(
        &mut server(),
        "bioeval_grounding_audit",
        json!({
            "claims": [{ "id": "claim" }],
            "evidence": [],
            "edges": [{ "claim": "claim", "evidence": "missing", "kind": "supports" }]
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["ok"], json!(false));
    assert_eq!(invalid["stage"], json!("edge_validation"));
    assert_eq!(invalid["fail_closed"], json!(true));
}

#[test]
fn bioeval_estimand_audit_preserves_claim_language_identification_and_transport() {
    let result = call(
        &mut server(),
        "bioeval_estimand_audit",
        json!({
            "estimand": {
                "intervention": "knockdown",
                "comparator": "control",
                "unit": "cell line",
                "outcome": "viability",
                "horizon": "72h",
                "scope": "pdac-twin"
            },
            "kind": "intervention",
            "basis": { "evidentiary": "model_conditional", "model": "pdac-twin-v2" },
            "identification": {
                "identification": "probed",
                "strategy": "backdoor",
                "assumptions": ["no unmeasured confounding"],
                "checks": [
                    { "name": "negative-control", "passed": false, "detail": "signal remained" },
                    { "name": "sensitivity", "passed": true, "detail": "stable" }
                ]
            },
            "corroborations": [
                { "source": "GSE-14520", "kind": "intervention", "detail": "external replication" }
            ],
            "transport_requests": [
                { "target": "pdac-twin", "declared_scopes": ["pdac-twin"] },
                { "target": "patients", "declared_scopes": ["pdac-twin"] }
            ],
            "require_identification": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-estimand-audit/0.1")
    );
    assert_eq!(result["estimand"]["five_elements_complete"], json!(true));
    assert_eq!(result["claim"]["kind"], json!("intervention"));
    assert_eq!(result["claim"]["still_model_conditional"], json!(false));
    assert!(
        result["claim"]["claim_language"]
            .as_str()
            .unwrap()
            .contains("changes")
    );
    assert_eq!(
        result["claim"]["identification_summary"]["status"],
        json!("probed")
    );
    assert_eq!(
        result["claim"]["identification_summary"]["failed_check_count"],
        json!(1)
    );
    assert_eq!(result["transport"]["status"], json!("partially_declared"));
    assert_eq!(result["transport"]["accepted"], json!(1));
    assert_eq!(result["transport"]["refused"], json!(1));

    let same_model = call(
        &mut server(),
        "bioeval_estimand_audit",
        json!({
            "estimand": {
                "intervention": "knockdown",
                "comparator": "control",
                "unit": "cell line",
                "outcome": "viability",
                "horizon": "72h",
                "scope": "pdac-twin"
            },
            "kind": "association",
            "basis": { "evidentiary": "model_conditional", "model": "pdac-twin-v2" },
            "corroborations": [
                { "source": "pdac-twin-v2", "kind": "association", "detail": "ran again" }
            ]
        }),
    );
    assert_eq!(same_model["__isError"], json!(false));
    assert_eq!(same_model["ok"], json!(false));
    assert_eq!(same_model["stage"], json!("corroboration_validation"));
    assert_eq!(same_model["fail_closed"], json!(true));
}

#[test]
fn bioeval_evaluator_audit_separates_harness_health_task_outcomes_and_hidden_data() {
    let result = call(
        &mut server(),
        "bioeval_evaluator_audit",
        json!({
            "runs": [
                {
                    "evaluator": "grader-a",
                    "health": { "health": "healthy" },
                    "reached": "met",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "grader-b",
                    "health": { "health": "healthy" },
                    "reached": "not_met",
                    "diagnostic": { "command": "pytest", "exit_state": "1", "diff": "expected output missing", "logs": [], "hidden_data_access": [] }
                },
                {
                    "evaluator": "grader-b",
                    "health": { "health": "healthy" },
                    "reached": "inapplicable",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "grader-c",
                    "health": { "health": "healthy" },
                    "reached": "not_met",
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "timeout",
                    "health": { "health": "timed_out", "after": "120s" },
                    "reached": null,
                    "diagnostic": { "command": "", "exit_state": "", "diff": "" }
                },
                {
                    "evaluator": "broken-fixture",
                    "health": { "health": "fixture_broken", "detail": "expected file absent" },
                    "reached": "met",
                    "diagnostic": { "command": "grader", "exit_state": "fixture-error", "diff": "", "logs": [], "hidden_data_access": ["read expected_outputs/"] }
                }
            ],
            "max_items": 2
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-evaluator-audit/0.1")
    );
    assert_eq!(result["panel"]["run_count"], json!(6));
    assert_eq!(result["panel"]["healthy_count"], json!(4));
    assert_eq!(result["panel"]["unhealthy_count"], json!(2));
    assert_eq!(result["panel"]["task_evidence_count"], json!(3));
    assert_eq!(result["panel"]["refused_task_outcome_count"], json!(3));
    assert_eq!(result["panel"]["outcomes"]["met"], json!(1));
    assert_eq!(result["panel"]["outcomes"]["not_met"], json!(1));
    assert_eq!(result["panel"]["outcomes"]["inapplicable"], json!(1));
    assert_eq!(
        result["panel"]["posture"],
        json!("review_required_hidden_data")
    );
    assert_eq!(
        result["findings"]["duplicate_evaluator_ids"]["ids"],
        json!(["grader-b"])
    );
    assert_eq!(result["runs"]["omitted"], json!(4));
    assert_eq!(result["runs"]["rows"][1]["task_outcome"], json!("not_met"));

    let hidden_refusal = call(
        &mut server(),
        "bioeval_evaluator_audit",
        json!({
            "runs": [{
                "evaluator": "grader",
                "health": { "health": "healthy" },
                "reached": "met",
                "diagnostic": { "command": "grader", "exit_state": "0", "diff": "", "logs": [], "hidden_data_access": ["secret"] }
            }],
            "fail_on_hidden_data": true
        }),
    );
    assert_eq!(hidden_refusal["__isError"], json!(false));
    assert_eq!(hidden_refusal["ok"], json!(false));
    assert_eq!(hidden_refusal["stage"], json!("hidden_data_policy"));
    assert_eq!(hidden_refusal["fail_closed"], json!(true));
}

#[test]
fn bioeval_plane_audit_keeps_unscored_and_inapplicable_out_of_the_fold() {
    let incomplete = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "fixed-model",
                "tier": "fixed_input_model",
                "dimensions": [
                    { "id": "accuracy", "required": "fixed_input_model", "weight": 2.0 },
                    { "id": "assay-selection", "required": "tool_using_agent", "weight": 1.0 },
                    { "id": "calibration", "required": "fixed_input_model", "weight": 1.0 }
                ],
                "cells": {
                    "accuracy": { "state": "scored", "score": 0.8 },
                    "assay-selection": { "state": "inapplicable", "required": "tool_using_agent", "declared": "fixed_input_model" },
                    "calibration": { "state": "unscored", "reason": "no_reference_standard", "note": "reference panel pending" }
                }
            },
            "max_items": 2
        }),
    );
    assert_eq!(incomplete["__isError"], json!(false));
    assert_eq!(incomplete["ok"], json!(true));
    assert_eq!(
        incomplete["schema"],
        json!("bioprism-mcp/bioeval-plane-audit/0.1")
    );
    assert_eq!(incomplete["plane"]["scored_count"], json!(1));
    assert_eq!(incomplete["plane"]["unscored_count"], json!(1));
    assert_eq!(incomplete["plane"]["inapplicable_count"], json!(1));
    assert_eq!(incomplete["findings"]["fold_blocked"], json!(true));
    assert_eq!(
        incomplete["findings"]["unscored_dimensions"]["ids"],
        json!(["calibration"])
    );
    assert_eq!(incomplete["dimensions"]["omitted"], json!(1));
    assert!(incomplete["fold"]["value"].is_null());

    let required = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "fixed-model",
                "tier": "fixed_input_model",
                "dimensions": [{ "id": "accuracy", "required": "fixed_input_model", "weight": 1.0 }],
                "cells": { "accuracy": { "state": "unscored", "reason": "not_attempted" } }
            },
            "require_fold": true
        }),
    );
    assert_eq!(required["ok"], json!(false));
    assert_eq!(required["stage"], json!("fold_policy"));
    assert_eq!(required["fail_closed"], json!(true));

    let folded = call(
        &mut server(),
        "bioeval_plane_audit",
        json!({
            "plane": {
                "system": "pipeline",
                "tier": "workflow_pipeline",
                "dimensions": [
                    { "id": "accuracy", "required": "fixed_input_model", "weight": 2.0 },
                    { "id": "workflow", "required": "workflow_pipeline", "weight": 1.0 },
                    { "id": "agent-action", "required": "tool_using_agent", "weight": 1.0 }
                ],
                "cells": {
                    "accuracy": { "state": "scored", "score": 0.75 },
                    "workflow": { "state": "scored", "score": 0.9 },
                    "agent-action": { "state": "inapplicable", "required": "tool_using_agent", "declared": "workflow_pipeline" }
                }
            },
            "require_fold": true
        }),
    );
    assert_eq!(folded["ok"], json!(true));
    assert_eq!(folded["fold"]["folded"], json!(true));
    assert!((folded["fold"]["value"].as_f64().unwrap() - 0.8).abs() < 1e-12);
    assert_eq!(folded["fold"]["included"], json!(["accuracy", "workflow"]));
    assert_eq!(folded["fold"]["excluded"][0]["id"], json!("agent-action"));
}

#[test]
fn bioeval_metamorphic_audit_separates_failure_directions_and_undetermined_trials() {
    let result = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [
                {
                    "id": "formatting",
                    "relation": "invariant",
                    "trials": [
                        { "id": "same", "relation": "invariant", "response": { "response": "unchanged" } },
                        { "id": "filename-shortcut", "relation": "invariant", "response": { "response": "moved", "direction": "increase" } },
                        { "id": "incomparable-format", "relation": "invariant", "response": { "response": "incomparable" } }
                    ]
                },
                {
                    "id": "biology-change",
                    "relation": { "directional_change": { "expected": "increase" } },
                    "trials": [
                        { "id": "expected-change", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "moved", "direction": "increase" } },
                        { "id": "blind-spot", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "unchanged" } },
                        { "id": "wrong-way", "relation": { "directional_change": { "expected": "increase" } }, "response": { "response": "moved", "direction": "decrease" } }
                    ]
                }
            ],
            "max_items": 2,
            "require_both_relations": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-metamorphic-audit/0.1")
    );
    assert_eq!(result["suite"]["family_count"], json!(2));
    assert_eq!(result["suite"]["trial_count"], json!(6));
    assert_eq!(
        result["suite"]["relation_coverage"]["complete"],
        json!(true)
    );
    assert_eq!(result["suite"]["failing_family_count"], json!(2));
    assert_eq!(result["suite"]["undetermined_trial_count"], json!(1));
    assert_eq!(result["suite"]["has_suite_wide_consistency"], json!(false));
    assert_eq!(
        result["findings"]["false_sensitivity_trials"]["ids"],
        json!(["filename-shortcut"])
    );
    assert_eq!(
        result["findings"]["false_invariance_trials"]["ids"],
        json!(["blind-spot"])
    );
    assert_eq!(
        result["findings"]["wrong_direction_trials"]["ids"],
        json!(["wrong-way"])
    );
    assert_eq!(
        result["findings"]["undetermined_families"]["ids"],
        json!(["formatting"])
    );
    assert_eq!(result["families"]["rows"][0]["trials"]["omitted"], json!(1));

    let undetermined_refusal = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [{
                "id": "oracle-gap",
                "relation": "invariant",
                "trials": [{ "id": "unknown", "relation": "invariant", "response": { "response": "incomparable" } }]
            }],
            "fail_on_undetermined": true
        }),
    );
    assert_eq!(undetermined_refusal["ok"], json!(false));
    assert_eq!(undetermined_refusal["stage"], json!("oracle_quality"));
    assert_eq!(undetermined_refusal["fail_closed"], json!(true));

    let coverage_refusal = call(
        &mut server(),
        "bioeval_metamorphic_audit",
        json!({
            "families": [{
                "id": "only-invariant",
                "relation": "invariant",
                "trials": [{ "id": "same", "relation": "invariant", "response": { "response": "unchanged" } }]
            }],
            "require_both_relations": true
        }),
    );
    assert_eq!(coverage_refusal["ok"], json!(false));
    assert_eq!(coverage_refusal["stage"], json!("relation_coverage"));
    assert_eq!(coverage_refusal["fail_closed"], json!(true));
}

#[test]
fn bioeval_waiver_audit_preserves_gate_verdicts_and_nonwaivable_vetoes() {
    let arguments = json!({
        "version": "release-2026.08",
        "at": "2026-08-16T12:00:00Z",
        "gates": [
            { "id": "health", "kind": "benchmark_health", "verdict": { "verdict": "violated", "detail": "calibration below floor" } },
            { "id": "unknown-rate", "kind": "maximum_unknown_rate", "verdict": { "verdict": "unevaluable", "missing": "reference panel" } },
            { "id": "safety", "kind": "safety_veto", "verdict": { "verdict": "violated", "detail": "forbidden action" } },
            { "id": "confidence", "kind": "confidence_requirement", "verdict": { "verdict": "met" } }
        ],
        "waivers": [{
            "gate": "health",
            "authoriser": "release-board",
            "rationale": "ship only the documented calibration exception",
            "expiry": "2026-09-01T00:00:00Z",
            "affected_versions": ["release-2026.08"],
            "follow_up": "recalibrate before the next release"
        }],
        "max_items": 3
    });
    let result = call(&mut server(), "bioeval_waiver_audit", arguments.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-waiver-audit/0.1")
    );
    assert_eq!(result["release"]["blocking_before"], json!(3));
    assert_eq!(result["release"]["blocking_after"], json!(2));
    assert_eq!(result["release"]["waived_count"], json!(1));
    assert_eq!(result["release"]["unevaluable_count"], json!(1));
    assert_eq!(result["release"]["releasable"], json!(false));
    assert_eq!(result["findings"]["waived_gates"]["ids"], json!(["health"]));
    assert_eq!(
        result["findings"]["still_blocking"]["ids"],
        json!(["safety", "unknown-rate"])
    );
    assert_eq!(
        result["gates"]["rows"][0]["verdict"]["verdict"],
        json!("violated")
    );
    assert_eq!(result["gates"]["rows"][0]["blocks_after"], json!(false));
    assert_eq!(
        result["waivers"]["rows"][0]["waiver"]["follow_up"],
        json!("recalibrate before the next release")
    );

    let release_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": arguments["gates"],
            "waivers": arguments["waivers"],
            "require_releasable": true
        }),
    );
    assert_eq!(release_refusal["ok"], json!(false));
    assert_eq!(release_refusal["stage"], json!("release_gate_policy"));
    assert_eq!(release_refusal["fail_closed"], json!(true));

    let unknown_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": arguments["gates"],
            "waivers": arguments["waivers"],
            "require_no_unevaluable": true
        }),
    );
    assert_eq!(unknown_refusal["ok"], json!(false));
    assert_eq!(unknown_refusal["stage"], json!("unknown_rate_policy"));

    let veto_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-08-16T12:00:00Z",
            "gates": [arguments["gates"][2]],
            "waivers": [{
                "gate": "safety",
                "authoriser": "release-board",
                "rationale": "attempted override",
                "expiry": "2026-09-01T00:00:00Z",
                "affected_versions": ["release-2026.08"],
                "follow_up": "review safety finding"
            }]
        }),
    );
    assert_eq!(veto_refusal["ok"], json!(false));
    assert_eq!(veto_refusal["stage"], json!("waiver_application"));
    assert_eq!(veto_refusal["fail_closed"], json!(true));

    let expiry_refusal = call(
        &mut server(),
        "bioeval_waiver_audit",
        json!({
            "version": "release-2026.08",
            "at": "2026-09-02T00:00:00Z",
            "gates": [arguments["gates"][0]],
            "waivers": arguments["waivers"]
        }),
    );
    assert_eq!(expiry_refusal["ok"], json!(false));
    assert_eq!(expiry_refusal["stage"], json!("waiver_application"));
}

#[test]
fn bioeval_design_audit_keeps_single_factor_contrasts_and_interaction_holes_visible() {
    let complete = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "p1", "levels": { "planner": "tree", "verifier": "off" }, "conclusion": "pass", "tier": "execution" },
            { "id": "v1", "levels": { "planner": "react", "verifier": "on" }, "conclusion": "pass", "tier": "execution" },
            { "id": "both", "levels": { "planner": "tree", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "controlled": true,
        "max_items": 2,
        "require_contrasts": true,
        "require_complete_interactions": true,
        "require_attribution": true
    });
    let result = call(&mut server(), "bioeval_design_audit", complete.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-design-audit/0.1")
    );
    assert_eq!(result["design"]["contrast_count"], json!(4));
    assert_eq!(result["design"]["unattributable_arm_count"], json!(1));
    assert_eq!(result["interactions"]["estimable_count"], json!(1));
    assert_eq!(result["interactions"]["missing_count"], json!(0));
    assert_eq!(result["attributions"]["total"], json!(4));
    assert_eq!(result["attributions"]["causal_count"], json!(4));
    assert_eq!(
        result["findings"]["unattributable_arms"]["ids"],
        json!(["both"])
    );
    assert_eq!(result["arms"]["omitted"], json!(2));
    assert_eq!(result["attributions"]["rows"][0]["causal"], json!(true));

    let incomplete = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "p1", "levels": { "planner": "tree", "verifier": "off" }, "conclusion": "pass", "tier": "execution" },
            { "id": "v1", "levels": { "planner": "react", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "require_complete_interactions": true
    });
    let interaction_refusal = call(&mut server(), "bioeval_design_audit", incomplete);
    assert_eq!(interaction_refusal["ok"], json!(false));
    assert_eq!(interaction_refusal["stage"], json!("interaction_coverage"));
    assert_eq!(interaction_refusal["fail_closed"], json!(true));

    let no_contrast = json!({
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            { "id": "base", "levels": { "planner": "react", "verifier": "off" }, "conclusion": "fail", "tier": "execution" },
            { "id": "both", "levels": { "planner": "tree", "verifier": "on" }, "conclusion": "pass", "tier": "execution" }
        ],
        "require_contrasts": true
    });
    let contrast_refusal = call(&mut server(), "bioeval_design_audit", no_contrast);
    assert_eq!(contrast_refusal["ok"], json!(false));
    assert_eq!(contrast_refusal["stage"], json!("contrast_coverage"));
}

#[test]
fn bioeval_mesh_audit_collapses_shared_inputs_and_separates_disagreement_kinds() {
    let result = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "system_artifacts": ["system-weights"],
            "evaluators": [
                { "id": "reader-a", "kind": "expert_review", "inputs": ["report-77"] },
                { "id": "reader-b", "kind": "expert_review", "inputs": ["report-77"] },
                { "id": "imaging", "kind": "expert_review", "inputs": ["mri-4"] },
                { "id": "molecular", "kind": "executable_analysis", "inputs": ["panel-9"] },
                { "id": "silent", "kind": "statistical_reference", "inputs": ["reference-3"] }
            ],
            "verdicts": [
                { "evaluator": "reader-a", "position": "progression" },
                { "evaluator": "reader-b", "position": "treatment-effect" },
                { "evaluator": "imaging", "position": "progression" },
                { "evaluator": "molecular", "position": "pseudoprogression" },
                { "evaluator": "silent", "position": "", "abstained": true }
            ],
            "expected": "progression",
            "max_items": 3
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-mesh-audit/0.1")
    );
    assert_eq!(result["mesh"]["evaluator_count"], json!(5));
    assert_eq!(result["mesh"]["independent_class_count"], json!(4));
    assert_eq!(result["mesh"]["independence_verified"], json!(true));
    assert_eq!(result["disagreements"]["within_class_count"], json!(1));
    assert_eq!(result["disagreements"]["across_class_count"], json!(4));
    assert_eq!(
        result["findings"]["abstaining_evaluators"]["ids"],
        json!(["silent"])
    );
    assert_eq!(result["independent_ratings"]["status"], json!("refused"));
    assert_eq!(result["findings"]["rating_projection_refused"], json!(true));
    assert_eq!(result["contributions"]["status"], json!("accepted"));
    assert!(
        result["contributions"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["conclusion"] == json!("unknown"))
    );
    assert_eq!(result["classes"]["rows"][0]["size"], json!(2));

    let circular_refusal = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "system_artifacts": ["system-weights"],
            "evaluators": [{ "id": "distilled", "kind": "calibrated_model_judge", "inputs": ["answer"], "derived_from": ["system-weights"] }]
        }),
    );
    assert_eq!(circular_refusal["ok"], json!(false));
    assert_eq!(circular_refusal["stage"], json!("evaluator_admission"));
    assert_eq!(circular_refusal["fail_closed"], json!(true));

    let independence_refusal = call(
        &mut server(),
        "bioeval_mesh_audit",
        json!({
            "evaluators": [{ "id": "silent", "kind": "expert_review", "inputs": [] }],
            "require_independence": true
        }),
    );
    assert_eq!(independence_refusal["ok"], json!(false));
    assert_eq!(independence_refusal["stage"], json!("independence_policy"));
}

#[test]
fn bioeval_burden_audit_preserves_inherited_residuals_waste_and_fork_refusals() {
    let result = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [
                { "id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL" },
                { "id": "compute", "class": "compute_and_money", "initial": 10, "unit": "hour" }
            ],
            "branches": [
                { "id": "candidate-a", "parent": "root" },
                { "id": "candidate-b", "parent": "root" }
            ],
            "draws": [
                { "branch": "root", "action": "extract", "resource": "biopsy", "amount": 30, "unit": "uL", "outcome": "wasted", "destructive": true },
                { "branch": "candidate-a", "action": "sequence-a", "resource": "biopsy", "amount": 60, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "candidate-b", "action": "sequence-b", "resource": "biopsy", "amount": 60, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "candidate-a", "action": "retry", "resource": "compute", "amount": 2, "unit": "hour", "outcome": "wasted", "destructive": false }
            ],
            "inspect_branches": ["root", "candidate-a", "candidate-b"],
            "joint_branches": ["candidate-a", "candidate-b"],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-burden-audit/0.1")
    );
    assert_eq!(result["burden"]["resource_count"], json!(2));
    assert_eq!(result["burden"]["branch_count"], json!(3));
    assert_eq!(result["burden"]["draw_count"], json!(4));
    assert_eq!(result["joint_feasibility"]["status"], json!("refused"));
    assert_eq!(result["findings"]["joint_feasibility_refused"], json!(true));
    assert_eq!(result["wasted_nonrenewable"]["total"], json!(1));
    assert_eq!(result["findings"]["failed_draws_still_counted"], json!(2));
    assert_eq!(
        result["branches"]["rows"][1]["residual"]["biopsy"],
        json!(10)
    );

    let joint_policy_refusal = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [{ "id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL" }],
            "branches": [{ "id": "a" }, { "id": "b" }],
            "draws": [
                { "branch": "a", "action": "a", "resource": "biopsy", "amount": 80, "unit": "uL", "outcome": "productive", "destructive": true },
                { "branch": "b", "action": "b", "resource": "biopsy", "amount": 80, "unit": "uL", "outcome": "productive", "destructive": true }
            ],
            "joint_branches": ["a", "b"],
            "require_joint_feasible": true
        }),
    );
    assert_eq!(joint_policy_refusal["ok"], json!(false));
    assert_eq!(
        joint_policy_refusal["stage"],
        json!("joint_feasibility_policy")
    );
    assert_eq!(joint_policy_refusal["fail_closed"], json!(true));

    let unit_refusal = call(
        &mut server(),
        "bioeval_burden_audit",
        json!({
            "root": "root",
            "resources": [{ "id": "biopsy", "class": "tissue_aliquot", "initial": 10, "unit": "uL" }],
            "draws": [{ "branch": "root", "action": "bad-unit", "resource": "biopsy", "amount": 1, "unit": "mL", "outcome": "productive", "destructive": true }]
        }),
    );
    assert_eq!(unit_refusal["ok"], json!(false));
    assert_eq!(unit_refusal["stage"], json!("draw_admission"));
}

#[test]
fn bioeval_reveal_audit_freezes_rubric_and_retains_unrevealed_commitments() {
    let result = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [
                { "target": "case-a", "prediction": { "class": "stable" }, "analysis_plan": "plan-v1" },
                { "target": "case-b", "prediction": { "class": "progression" }, "analysis_plan": "plan-v1" }
            ],
            "rubric": { "version": 1, "rules": ["predeclared"] },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "case-a", "observed": { "class": "stable" } }],
            "score_rubric": { "version": 1, "rules": ["predeclared"] }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-reveal-audit/0.1")
    );
    assert_eq!(result["commitments"]["total"], json!(2));
    assert_eq!(result["outcomes"]["total"], json!(1));
    assert_eq!(result["scoring"]["status"], json!("accepted"));
    assert_eq!(result["scoring"]["complete"], json!(false));
    assert_eq!(result["findings"]["selective_publication"], json!(true));
    assert_eq!(
        result["findings"]["unrevealed_commitments"]["ids"],
        json!(["case-b"])
    );
    assert_eq!(result["seal_lock"]["status"], json!("refused"));
    assert_eq!(result["reveal_lock"]["status"], json!("refused"));

    let rubric_refusal = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [{ "target": "case-a", "prediction": "stable", "analysis_plan": "plan-v1" }],
            "rubric": { "version": 1 },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "case-a", "observed": "stable" }],
            "score_rubric": { "version": 2 },
            "require_rubric_match": true
        }),
    );
    assert_eq!(rubric_refusal["ok"], json!(false));
    assert_eq!(rubric_refusal["stage"], json!("rubric_integrity_policy"));
    assert_eq!(rubric_refusal["fail_closed"], json!(true));

    let uncommitted = call(
        &mut server(),
        "bioeval_reveal_audit",
        json!({
            "study": "prospective-2026",
            "commitments": [{ "target": "case-a", "prediction": "stable", "analysis_plan": "plan-v1" }],
            "rubric": { "version": 1 },
            "sealed_at": "2026-08-16T12:00:00Z",
            "outcomes": [{ "target": "new-case", "observed": "stable" }],
            "score_rubric": { "version": 1 }
        }),
    );
    assert_eq!(uncommitted["ok"], json!(true));
    assert_eq!(uncommitted["scoring"]["status"], json!("refused"));
    assert_eq!(
        uncommitted["findings"]["uncommitted_outcome_refused"],
        json!(true)
    );
}

#[test]
fn bioeval_boundary_audit_separates_authorization_denial_violations_vetoes_and_bypass() {
    let result = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "policies": [{
                "id": "consent-study",
                "recipient": "evaluator",
                "information_type": "deidentified",
                "purpose": "study",
                "transmission_principle": "consent",
                "channels": ["inter_agent_messages"]
            }],
            "flows": [
                {
                    "id": "authorized",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "evaluator",
                    "information_type": "deidentified",
                    "purpose": "study",
                    "transmission_principle": "consent",
                    "channel": "inter_agent_messages",
                    "effect": { "effect": "materialized" },
                    "irreversible": false
                },
                {
                    "id": "respected-denial",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "vendor",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "external_queries",
                    "effect": { "effect": "proposed", "denied_by": "policy-deny" },
                    "irreversible": false
                },
                {
                    "id": "materialized-violation",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "vendor",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "external_queries",
                    "effect": { "effect": "materialized" },
                    "irreversible": false
                },
                {
                    "id": "irreversible-veto",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "public",
                    "information_type": "identifier",
                    "purpose": "publication",
                    "transmission_principle": "none",
                    "channel": "final_output",
                    "effect": { "effect": "materialized" },
                    "irreversible": true
                },
                {
                    "id": "bypass",
                    "sender": "agent",
                    "subject": "participant-1",
                    "recipient": "logger",
                    "information_type": "identifier",
                    "purpose": "debug",
                    "transmission_principle": "none",
                    "channel": "logs",
                    "effect": { "effect": "bypass_attempted", "detail": "used an alternate path" },
                    "irreversible": false
                }
            ],
            "utility": 0.8,
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioeval-boundary-audit/0.1")
    );
    assert_eq!(result["boundary"]["authorised_count"], json!(1));
    assert_eq!(result["boundary"]["compliant_count"], json!(1));
    assert_eq!(result["boundary"]["violation_count"], json!(3));
    assert_eq!(result["boundary"]["veto_count"], json!(2));
    assert_eq!(
        result["findings"]["compliant_proposals"]["ids"],
        json!(["respected-denial"])
    );
    assert_eq!(result["composite"]["status"], json!("refused"));
    assert_eq!(result["findings"]["bypass_is_veto"], json!(true));

    let veto_policy_refusal = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "flows": [{
                "id": "veto",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "public",
                "information_type": "identifier",
                "purpose": "publication",
                "transmission_principle": "none",
                "channel": "final_output",
                "effect": { "effect": "materialized" },
                "irreversible": true
            }],
            "require_no_vetoes": true
        }),
    );
    assert_eq!(veto_policy_refusal["ok"], json!(false));
    assert_eq!(veto_policy_refusal["stage"], json!("veto_policy"));

    let missing_principle = call(
        &mut server(),
        "bioeval_boundary_audit",
        json!({
            "flows": [{
                "id": "missing",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "vendor",
                "information_type": "identifier",
                "purpose": "debug",
                "transmission_principle": "",
                "channel": "logs",
                "effect": { "effect": "materialized" },
                "irreversible": false
            }]
        }),
    );
    assert_eq!(missing_principle["ok"], json!(false));
    assert_eq!(missing_principle["stage"], json!("flow_assessment"));
}

#[test]
fn evaluation_worldline_audit_separates_future_leakage_from_dangling_context() {
    let stamp = |value: &str| Timestamp::parse(value).unwrap();
    let mut worldline = Worldline::new();
    worldline
        .observe(
            EvalObservation::new(
                "early",
                stamp("2026-01-01T00:00:00Z"),
                stamp("2026-01-02T00:00:00Z"),
                stamp("2026-01-03T00:00:00Z"),
                stamp("2026-01-04T00:00:00Z"),
            )
            .unwrap(),
        )
        .unwrap();
    worldline
        .observe(
            EvalObservation::new(
                "future",
                stamp("2026-01-05T00:00:00Z"),
                stamp("2026-01-06T00:00:00Z"),
                stamp("2026-01-07T00:00:00Z"),
                stamp("2026-01-10T00:00:00Z"),
            )
            .unwrap(),
        )
        .unwrap();
    worldline
        .decide(Decision {
            id: "decision-1".into(),
            at: stamp("2026-01-08T00:00:00Z"),
            context: vec!["early".into(), "future".into(), "missing".into()],
        })
        .expect("valid decision");

    let result = call(
        &mut server(),
        "evaluation_worldline_audit",
        json!({
            "worldline": serde_json::to_value(worldline).unwrap(),
            "at": "2026-01-08T00:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-worldline-audit/0.1")
    );
    assert_eq!(result["leak_count"], json!(1));
    assert_eq!(result["dangling_count"], json!(1));
    assert_eq!(result["leaks"][0]["observation"], json!("future"));
    assert_eq!(result["dangling_references"][0][1], json!("missing"));
    assert_eq!(result["admissible_at"][0], json!("early"));
}

#[test]
fn evaluation_reproduction_check_keeps_divergence_and_validity_refusal_visible() {
    let mut reexecution = Reexecution::declaring(
        "workflow-1",
        true,
        vec![
            OutputSpec::exact("digest"),
            OutputSpec::numeric("score", 0.1).unwrap(),
        ],
    )
    .unwrap();
    reexecution
        .observe(
            "digest",
            Observed::Digests {
                original: "abc".into(),
                rerun: "abc".into(),
            },
        )
        .unwrap();
    reexecution
        .observe(
            "score",
            Observed::Numbers {
                original: 1.0,
                rerun: 1.5,
            },
        )
        .unwrap();

    let result = call(
        &mut server(),
        "evaluation_reproduction_check",
        json!({
            "reexecution": serde_json::to_value(reexecution).unwrap(),
            "biological_claim": "the treatment is effective"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-reproduction-check/0.1")
    );
    assert_eq!(result["reproduced"], json!(false));
    assert_eq!(result["verdict_count"], json!(2));
    assert_eq!(result["matched_count"], json!(1));
    assert_eq!(result["diverged_count"], json!(1));
    assert_eq!(result["missing_count"], json!(0));
    assert_eq!(result["verdicts"][1]["output"], json!("score"));
    assert_eq!(result["verdicts"][1]["verdict"], json!("diverged"));
    assert_eq!(result["first_divergence"]["output"], json!("score"));
    assert_eq!(result["validity_claim"]["ok"], json!(false));
    assert_eq!(result["portability_demonstrated"], json!(false));
}

#[test]
fn evaluation_trajectory_check_reports_vacuity_and_bounded_suffix_separately() {
    let mut trajectory = Trajectory::of(vec![
        Step::new("edit").irreversible(),
        Step::new("verify").at_distance(2.0),
        Step::new("finish").at_distance(1.0),
    ]);
    trajectory
        .require(PathProperty::PrecededBy {
            before: "edit".into(),
            after: "inspect".into(),
        })
        .unwrap();

    let result = call(
        &mut server(),
        "evaluation_trajectory_check",
        json!({
            "trajectory": serde_json::to_value(trajectory).unwrap(),
            "step": 0,
            "horizon": 2
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/evaluation-trajectory-check/0.1")
    );
    assert_eq!(result["step_records"].as_array().unwrap().len(), 3);
    assert_eq!(result["property_count"], json!(1));
    assert_eq!(result["violated_count"], json!(1));
    assert_eq!(result["vacuous_count"], json!(0));
    assert_eq!(result["property_outcomes"][0]["held"], json!(false));
    assert_eq!(result["recovery_count"], json!(0));
    assert_eq!(result["property_outcomes"][0]["violations"][0], json!(0));
    assert_eq!(result["bounded_suffix"]["complete"], json!(true));
    assert_eq!(result["bounded_suffix"]["value"]["downstream"], json!(1.0));
}

#[test]
fn stress_profile_reports_breaking_points_and_generator_posture() {
    let cohort = Cohort::new(
        "cohort-1",
        vec![
            Subject::new("p1", "site-a", true, 3.0, 1000.0),
            Subject::new("p2", "site-b", true, 2.5, 1100.0),
            Subject::new("n1", "site-a", false, 0.5, 900.0),
            Subject::new("n2", "site-b", false, 0.2, 950.0),
        ],
    );
    let stress = Stress::new(
        "deployment-prevalence",
        Knob::PrevalenceShift {
            target_prevalence: 0.25,
        },
        Magnitude::FULL,
        7,
    );
    let result = call(
        &mut server(),
        "stress_profile",
        json!({
            "cohort": serde_json::to_value(cohort).unwrap(),
            "stress": serde_json::to_value(stress).unwrap(),
            "procedures": serde_json::to_value(vec![Procedure::MarkerRanking]).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert!(result["headline"].as_str().unwrap().contains("prevalence"));
    assert_eq!(result["profile"]["sweep"].as_array().unwrap().len(), 8);
    assert!(
        result["profile"]["generator_defects"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn routing_decide_abstains_without_two_architecture_supporting_panels() {
    let approved = ApprovedSet::new([
        RoutingArchitecture::FullContext,
        RoutingArchitecture::FiberCompiled,
    ])
    .unwrap();
    let policy = RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap();
    let result = call(
        &mut server(),
        "routing_decide",
        json!({
            "fingerprint": serde_json::to_value(routing_fingerprint_fixture()).unwrap(),
            "evidence": [],
            "policy": serde_json::to_value(policy).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["decision"]["abstained"], json!(true));
    assert_eq!(result["decision"]["confidence"], json!(0.0));
    assert_eq!(
        result["holdout_check"],
        json!("caller_must_supply_unseen_identity")
    );
}

#[test]
fn routing_decide_refuses_evidence_leakage_when_task_identity_is_supplied() {
    let fingerprint = routing_fingerprint_fixture();
    let ledger = EvidenceLedger::new([RoutingObservation {
        task_id: "seen-task".into(),
        fingerprint: fingerprint.clone(),
        architecture: RoutingArchitecture::FullContext,
        verdict_preserving: true,
        closure_complete: true,
        status: OracleStatus::Valid,
        facts_exposed: 10,
        total_facts: 10,
    }])
    .unwrap();
    let approved = ApprovedSet::new([RoutingArchitecture::FullContext]).unwrap();
    let policy = RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap();
    let result = call(
        &mut server(),
        "routing_decide",
        json!({
            "fingerprint": serde_json::to_value(fingerprint).unwrap(),
            "evidence": serde_json::to_value(ledger).unwrap(),
            "policy": serde_json::to_value(policy).unwrap(),
            "task_id": "seen-task",
        }),
    );
    assert_eq!(result["__isError"], json!(true));
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("contains that task's own outcome")
    );
}

#[test]
fn routing_lab_run_keeps_holdout_comparators_and_bounded_task_rows_visible() {
    let reference = bioprism_worldgen::generate(&bioprism_worldgen::WorldSpec::reference_like(2));
    let discriminating =
        bioprism_worldgen::generate(&bioprism_worldgen::WorldSpec::discriminating(2));
    let approved = ApprovedSet::new([
        RoutingArchitecture::FullContext,
        RoutingArchitecture::FiberCompiled,
    ])
    .unwrap();
    let settings = bioprism_routing::LabSettings::new(
        RoutingPolicy::defaulting_to(approved, RoutingArchitecture::FullContext).unwrap(),
        RoutingArchitecture::FullContext,
    )
    .unwrap();
    let result = call(
        &mut server(),
        "routing_lab_run",
        json!({
            "tasks": [
                { "task_id": "reference-task", "world": reference.world, "query": reference.query },
                { "task_id": "discriminating-task", "world": discriminating.world, "query": discriminating.query }
            ],
            "settings": serde_json::to_value(settings).unwrap(),
            "include_rows": true,
            "max_rows": 1
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/routing-lab-run/0.1"));
    assert_eq!(result["tasks"], json!(2));
    assert_eq!(result["holdout_label"], json!("leave-one-task-out"));
    assert_eq!(result["report"]["task_rows"].as_array().unwrap().len(), 1);
    assert_eq!(result["report"]["task_rows_omitted"], json!(1));
    assert!(result["report"]["account"]["router"].is_object());
    assert!(result["report"]["verdict"].is_string());
    assert!(
        result["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("route_unseen"))
    );
}

#[test]
fn scale_family_split_verify_keeps_lineage_families_intact() {
    let parent = GeneratedItem::parent("world-1", "decision", "digest-parent");
    let child = GeneratedItem::descendant(
        "world-1-mutated",
        "world-1",
        "fault",
        "signature",
        "digest-child",
        "decision",
    );
    let valid = call(
        &mut server(),
        "scale_family_split_verify",
        json!({
            "corpus": serde_json::to_value(vec![parent.clone(), child.clone()]).unwrap(),
            "assignment": { "world-1": "public", "world-1-mutated": "public" }
        }),
    );
    assert_eq!(valid["__isError"], json!(false));
    assert_eq!(valid["valid"], json!(true));
    assert_eq!(valid["family_count"], json!(1));
    assert_eq!(valid["report"]["intact_families"], json!(1));
    assert_eq!(valid["report"]["items_by_tier"]["public"], json!(2));

    let straddled = call(
        &mut server(),
        "scale_family_split_verify",
        json!({
            "corpus": serde_json::to_value(vec![parent, child]).unwrap(),
            "assignment": { "world-1": "public", "world-1-mutated": "hidden" }
        }),
    );
    assert_eq!(straddled["__isError"], json!(false));
    assert_eq!(straddled["valid"], json!(false));
    assert!(straddled["refusal"].as_str().unwrap().contains("straddles"));
}

#[test]
fn quality_gate_run_keeps_failures_and_unrunnable_checks_separate() {
    let dataset = QualityDataset::new("patients")
        .unwrap()
        .with_column("subject", [json!("S1"), json!("S1")])
        .unwrap()
        .with_column("age", [json!(41), json!(null)])
        .unwrap();
    let gate = QualityGate::new("release-quality")
        .unwrap()
        .with(
            "subject_unique",
            QualityCheck::Unique {
                column: "subject".into(),
            },
        )
        .unwrap()
        .with(
            "foreign_site",
            QualityCheck::ForeignKey {
                column: "site".into(),
                reference: "sites".into(),
            },
        )
        .unwrap();
    let result = call(
        &mut server(),
        "quality_gate_run",
        json!({
            "dataset": serde_json::to_value(dataset).unwrap(),
            "gate": serde_json::to_value(gate).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["schema"], json!("bioprism-mcp/quality-gate/0.1"));
    assert_eq!(result["verdict"], json!("failed"));
    assert_eq!(result["passed"], json!(false));
    assert_eq!(
        result["report"]["outcomes"]["subject_unique"]["Fail"]["witness"]["row"],
        json!(1)
    );
    let foreign_site = result["report"]["outcomes"]["foreign_site"].to_string();
    assert!(
        foreign_site.contains("MissingReferenceSet"),
        "{foreign_site}"
    );
    assert!(foreign_site.contains("site"), "{foreign_site}");

    let indeterminate_dataset = QualityDataset::new("empty-measurement")
        .unwrap()
        .with_column("age", [json!(null), json!(null)])
        .unwrap();
    let indeterminate_gate = QualityGate::new("age-quality")
        .unwrap()
        .with(
            "age_range",
            QualityCheck::InRange {
                column: "age".into(),
                min: 0.0,
                max: 120.0,
            },
        )
        .unwrap();
    let indeterminate = call(
        &mut server(),
        "quality_gate_run",
        json!({
            "dataset": serde_json::to_value(indeterminate_dataset).unwrap(),
            "gate": serde_json::to_value(indeterminate_gate).unwrap(),
        }),
    );
    assert_eq!(indeterminate["verdict"], json!("indeterminate"));
    assert_eq!(indeterminate["passed"], json!(false));
}

#[test]
fn benchmark_trace_analyze_keeps_causal_localization_and_segmentation_distinct() {
    let result = call(
        &mut server(),
        "benchmark_trace_analyze",
        json!({
            "failing": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "summary": "choose route", "alternatives": ["safe", "unsafe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "reference": {
                "trace_id": "reference-run",
                "succeeded": true,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "summary": "choose safe", "alternatives": ["safe", "unsafe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "succeeded" }, "caused_by": 1, "visible": ["task"] }
                ]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["trace_id"], json!("failed-run"));
    assert_eq!(result["event_count"], json!(3));
    assert_eq!(result["summary"]["episode_count"], json!(1));
    assert!(result["summary"]["boundary_count"].as_u64().unwrap() >= 1);
    assert!(result["analysis"]["candidates"].is_array());
    assert!(
        result["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("does not replay"))
    );
}

#[test]
fn benchmark_decision_audit_preserves_firewall_coverage_and_failure_evidence() {
    let result = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({
            "trace": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "unsafe", "alternatives": ["safe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "reference": {
                "trace_id": "reference-run",
                "succeeded": true,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "safe", "alternatives": ["safe"] }, "visible": ["task"] },
                    { "step": 2, "kind": "termination", "payload": { "summary": "succeeded" }, "caused_by": 1, "visible": ["task"] }
                ]
            },
            "actions": [
                {
                    "label": "future-safe",
                    "semantic_property": "avoid the irreversible side effect",
                    "provenance": { "source": "from_future", "from_step": 3 },
                    "feasibility": { "state": "feasible" },
                    "strong": true
                }
            ],
            "claims": [
                { "status": "evidenced", "claim": "the choice differed", "citations": [{ "cites": "event", "step": 1 }] },
                { "status": "hypothesis", "claim": "the tool was confusing", "why": "no direct evidence" }
            ],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-decision-audit/0.1")
    );
    assert_eq!(result["decision"]["selected_step"], json!(1));
    assert_eq!(result["decision"]["causal_alignment"], json!("aligned"));
    assert_eq!(result["decision"]["action_counts"]["all"], json!(3));
    assert_eq!(
        result["decision"]["action_counts"]["visible_to_agent"],
        json!(2)
    );
    assert_eq!(
        result["decision"]["action_counts"]["validation_only"],
        json!(1)
    );
    assert_eq!(result["failure_card"]["evidence_ratio"], json!(0.5));
    assert_eq!(
        result["failure_card"]["hypotheses"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(result["trace_digest"].as_str().unwrap().len(), 64);

    let leak = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({
            "trace": {
                "trace_id": "failed-run",
                "succeeded": false,
                "events": [
                    { "step": 0, "kind": "goal", "payload": { "summary": "solve" } },
                    { "step": 1, "kind": "choice", "payload": { "action": "unsafe" } },
                    { "step": 2, "kind": "termination", "payload": { "summary": "failed" }, "caused_by": 1 }
                ]
            },
            "decision_step": 1,
            "actions": [{
                "label": "leaked",
                "provenance": { "source": "visible_at_decision_time", "from_step": 2 },
                "feasibility": { "state": "feasible" },
                "strong": true
            }]
        }),
    );
    assert_eq!(leak["__isError"], json!(false));
    assert_eq!(leak["ok"], json!(false));
    assert_eq!(leak["stage"], json!("hindsight_firewall"));
    assert_eq!(leak["fail_closed"], json!(true));

    let invalid = call(
        &mut server(),
        "benchmark_decision_audit",
        json!({ "trace": { "trace_id": "t", "succeeded": false, "events": [] }, "max_items": "100" }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].as_str().unwrap().contains("max_items"));
}

#[test]
fn benchmark_integrity_audit_keeps_duplicates_leaks_holdouts_and_effective_denominators_separate() {
    let result = call(
        &mut server(),
        "benchmark_integrity_audit",
        json!({
            "instances": [
                { "instance_id": "a", "content": { "world": "W", "sample": "A" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["A"] },
                { "instance_id": "b", "content": { "world": "W", "sample": "A" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["A"] },
                { "instance_id": "c", "content": { "world": "W", "sample": "B" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["B"] },
                { "instance_id": "d", "content": { "world": "W2", "sample": "D" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["D"] },
                { "instance_id": "e", "content": { "world": "W3", "sample": "E" }, "acceptable_verdicts": ["pass"], "required_witnesses": ["w"], "identifiers": ["E"] }
            ],
            "panel_runs": [
                { "instance_id": "a", "architecture": "strong", "tier": "strong", "passed": true },
                { "instance_id": "a", "architecture": "weak", "tier": "weak", "passed": false },
                { "instance_id": "d", "architecture": "weak", "tier": "weak", "passed": true },
                { "instance_id": "e", "architecture": "strong", "tier": "strong", "passed": true }
            ],
            "known_instances": ["a", "b", "c", "d", "e", "unmeasured"],
            "safety_vetoes": ["e"],
            "bench_instances": [
                { "instance_id": "x1", "parent_digest": "p1", "mutation_family": "f1", "oracle_signature": "o1" },
                { "instance_id": "x2", "parent_digest": "p1", "mutation_family": "f1", "oracle_signature": "o1" },
                { "instance_id": "x3", "parent_digest": "p1", "mutation_family": "f2", "oracle_signature": "o1" },
                { "instance_id": "x4", "parent_digest": "p2", "mutation_family": "f1", "oracle_signature": "o2" }
            ],
            "exposure": {
                "a": { "published": true, "repositories": ["repo-a"], "answer_searchable": false, "first_published": "2025-01-01", "assessed": true },
                "b": { "published": true, "repositories": ["repo-b"], "answer_searchable": true, "first_published": "2025-01-01", "assessed": true },
                "e": { "published": false, "repositories": [], "answer_searchable": false, "first_published": null, "assessed": true }
            },
            "probes": {
                "d": [{ "channel": "metadata_only", "solved": true, "note": "metadata disclosed the answer" }],
                "e": [{ "channel": "filename_only", "solved": false, "note": "probe did not solve" }]
            },
            "private_share": 100,
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-integrity-audit/0.1")
    );
    assert_eq!(result["counts"]["instances"], json!(5));
    assert_eq!(result["dedup"]["examined"], json!(5));
    assert_eq!(result["dedup"]["distinct"], json!(3));
    assert!(result["dedup"]["groups"].as_array().unwrap().len() >= 2);
    assert_eq!(result["holdout"]["counts"]["private"], json!(5));
    assert_eq!(result["contamination"]["counts"]["unassessed"], json!(1));
    assert_eq!(
        result["contamination"]["counts"]["leaks_through_channel"],
        json!(1)
    );
    assert_eq!(result["contamination"]["admissible"], json!(1));
    assert_eq!(result["calibration"]["unmeasured"], json!(3));
    assert_eq!(result["calibration"]["safety_vetoes"], json!(1));
    assert_eq!(result["effective_diversity"]["instances"], json!(4));
    assert_eq!(
        result["effective_diversity"]["equivalence_classes"],
        json!(3)
    );
    assert!(
        result["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("semantic similarity"))
    );
}

#[test]
fn benchmark_counterfactual_check_enforces_one_factor_matching_and_grades_contrast() {
    let source = json!({
        "schema_version": "bioprism-decision-cell/0.1",
        "cell_id": "cell-source",
        "decision_point": "choose evidence",
        "world": { "locator": "world-a", "sha256": "a".repeat(64) },
        "query": { "locator": "query-a", "sha256": "b".repeat(64) },
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "require_protected_closure": true
    });
    let followup = json!({
        "schema_version": "bioprism-decision-cell/0.1",
        "cell_id": "cell-followup",
        "decision_point": "choose evidence",
        "world": { "locator": "world-a", "sha256": "a".repeat(64) },
        "query": { "locator": "query-b", "sha256": "c".repeat(64) },
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "require_protected_closure": true
    });
    let result = call(
        &mut server(),
        "benchmark_counterfactual_check",
        json!({
            "source": source,
            "followup": followup,
            "intervention": {
                "factor": "fresh evidence",
                "target": "evidence_availability",
                "from": { "available": false },
                "to": { "available": true },
                "changes": ["query"]
            },
            "expected": { "expect": "invariant", "rationale": "the correct verdict remains pass" },
            "source_verdict": "pass",
            "followup_verdict": "pass"
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["pair"]["differing_fields"], json!(["query"]));
    assert_eq!(result["pair"]["realism_reviewed"], json!(false));
    assert_eq!(result["outcome"]["outcome"], json!("as_predicted"));
    assert_eq!(result["satisfied"], json!(true));
    assert_eq!(result["cell_digests"]["source"].as_str().unwrap().len(), 64);

    let mut mismatched_followup = followup;
    mismatched_followup["acceptable_verdicts"] = json!(["abstain"]);
    let refused = call(
        &mut server(),
        "benchmark_counterfactual_check",
        json!({
            "source": source,
            "followup": mismatched_followup,
            "intervention": {
                "factor": "fresh evidence",
                "target": "evidence_availability",
                "from": { "available": false },
                "to": { "available": true },
                "changes": ["query"]
            },
            "expected": { "expect": "invariant", "rationale": "unchanged" },
            "source_verdict": "pass",
            "followup_verdict": "abstain"
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("matched_pair"));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(refused["refusal"].as_str().unwrap().contains("not matched"));
}

#[test]
fn benchmark_oracle_review_requires_gate_before_grading_or_cell_packaging() {
    let proposal = json!({
        "oracle_id": "oracle-demo",
        "decision_point": "choose evidence",
        "strength": "exact_state_predicate",
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "can_see": ["declared world"],
        "blind_spots": ["hidden grader state"],
        "exploits": []
    });
    let result = call(
        &mut server(),
        "benchmark_oracle_review",
        json!({
            "proposal": proposal,
            "reviewer": "reviewer-1",
            "grade": { "verdict": "pass", "witnesses": ["evidence"], "closure_complete": true },
            "cell": {
                "cell_id": "cell-reviewed",
                "world": { "locator": "world.json", "sha256": "a".repeat(64) },
                "query": { "locator": "query.json", "sha256": "b".repeat(64) }
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["grade"]["acceptance"]["outcome"], json!("passed"));
    assert_eq!(result["grade"]["passed"], json!(true));
    assert_eq!(result["cell"]["cell_id"], json!("cell-reviewed"));
    assert_eq!(result["cell"]["acceptable_verdicts"], json!(["pass"]));
    assert_eq!(result["reviewer"], json!("reviewer-1"));
    assert_eq!(result["review_digest"].as_str().unwrap().len(), 64);
    assert_eq!(result["reviewed_oracle"]["reviewer"], json!("reviewer-1"));

    let exploit = call(
        &mut server(),
        "benchmark_oracle_review",
        json!({
            "proposal": {
                "oracle_id": "oracle-exploit",
                "decision_point": "choose",
                "strength": "exact_state_predicate",
                "acceptable_verdicts": ["pass"],
                "required_witnesses": [],
                "can_see": ["world"],
                "blind_spots": ["grader"],
                "exploits": [{ "name": "grader-read", "description": "read grader", "scored_as_pass": true, "fulfils_task_intent": false }]
            },
            "reviewer": "reviewer-1"
        }),
    );
    assert_eq!(exploit["__isError"], json!(false));
    assert_eq!(exploit["ok"], json!(false));
    assert_eq!(exploit["stage"], json!("oracle_review"));
    assert_eq!(exploit["fail_closed"], json!(true));
    assert!(exploit["refusal"].as_str().unwrap().contains("exploit"));
}

#[test]
fn benchmark_compile_composes_causal_minimization_and_oracle_synthesis_without_execution() {
    let trace = |trace_id: &str, tool: &str, succeeded: bool| {
        json!({
            "trace_id": trace_id,
            "succeeded": succeeded,
            "events": [
                { "step": 0, "kind": "goal", "payload": { "summary": "rank the candidates" } },
                { "step": 1, "kind": "action", "payload": { "tool": "choose_assay", "irreversible": true }, "caused_by": 0 },
                { "step": 2, "kind": "result", "payload": { "summary": "assay selected" }, "caused_by": 1 },
                { "step": 3, "kind": "action", "payload": { "tool": tool }, "caused_by": 2 },
                { "step": 4, "kind": "claim", "payload": { "summary": "reported a hit" }, "caused_by": 3 },
                { "step": 5, "kind": "termination", "payload": { "summary": "done" }, "caused_by": 4 }
            ]
        })
    };
    let signature = |invalid: bool| {
        if invalid {
            json!({ "verdict": "invalid", "witnesses": ["identity_leakage"], "divergence_step": 3 })
        } else {
            json!({ "verdict": "valid", "witnesses": [], "divergence_step": 3 })
        }
    };
    let subsets = vec![
        (vec![], false),
        (vec!["panel_manifest"], true),
        (vec!["unused_service"], false),
        (vec!["stale_memory"], false),
        (vec!["panel_manifest", "unused_service"], true),
        (vec!["panel_manifest", "stale_memory"], true),
        (vec!["unused_service", "stale_memory"], false),
        (
            vec!["panel_manifest", "unused_service", "stale_memory"],
            true,
        ),
    ];
    let observations = subsets
        .into_iter()
        .map(|(kept, invalid)| json!({ "kept": kept, "signature": signature(invalid) }))
        .collect::<Vec<_>>();
    let arguments = json!({
        "trace": trace("run_fail", "run_wrong_panel", false),
        "reference": trace("run_pass", "run_right_panel", true),
        "context": [
            { "id": "panel_manifest", "tier": "artifact", "guard": "removable" },
            { "id": "unused_service", "tier": "service", "guard": "removable" },
            { "id": "stale_memory", "tier": "memory_entry", "guard": "removable" }
        ],
        "probe_observations": observations,
        "budget": { "max_evaluations": 100 }
    });
    let result = call(&mut server(), "benchmark_compile", arguments.clone());
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/benchmark-compile/0.1")
    );
    assert_eq!(
        result["class"],
        json!({ "class": "candidate_research_cell" })
    );
    assert_eq!(result["cell_step"], json!(3));
    assert_eq!(result["minimization"]["minimal"], json!(["panel_manifest"]));
    assert_eq!(
        result["minimization"]["removed"].as_array().unwrap().len(),
        2
    );
    assert_eq!(result["oracle"]["strength"], json!("exact_state_predicate"));
    assert!(
        result["unmeasured_stages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|stage| stage == "state_reconstruction")
    );
    assert_eq!(
        result["probe"]["execution"],
        json!("caller-supplied observation table; no world or architecture was run")
    );

    let mut missing = arguments.clone();
    missing["probe_observations"].as_array_mut().unwrap().pop();
    let refused = call(&mut server(), "benchmark_compile", missing);
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("minimization_probe"));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(refused["refusal"].as_str().unwrap().contains("observation"));

    let mut malformed_claim = arguments.clone();
    malformed_claim["claims"] = json!([{
        "status": "evidenced",
        "claim": "the panel caused the failure",
        "citations": []
    }]);
    let claim_refused = call(&mut server(), "benchmark_compile", malformed_claim);
    assert_eq!(claim_refused["__isError"], json!(false));
    assert_eq!(claim_refused["ok"], json!(false));
    assert_eq!(claim_refused["stage"], json!("claim_attribution"));
    assert_eq!(claim_refused["fail_closed"], json!(true));

    let mut reviewed_arguments = arguments;
    reviewed_arguments["reviewer"] = json!("reviewer-1");
    reviewed_arguments["world"] = json!({ "locator": "world.json", "sha256": "a".repeat(64) });
    reviewed_arguments["query"] = json!({ "locator": "query.json", "sha256": "b".repeat(64) });
    reviewed_arguments["grade"] = json!({ "verdict": "invalid", "witnesses": ["identity_leakage"], "closure_complete": true });
    let reviewed = call(
        &mut server(),
        "benchmark_compile_review",
        reviewed_arguments,
    );
    assert_eq!(reviewed["__isError"], json!(false));
    assert_eq!(reviewed["ok"], json!(true));
    assert_eq!(
        reviewed["schema"],
        json!("bioprism-mcp/benchmark-compile-review/0.1")
    );
    assert_eq!(reviewed["reviewer"], json!("reviewer-1"));
    assert_eq!(reviewed["grade"]["acceptance"]["outcome"], json!("passed"));
    assert_eq!(reviewed["cell"]["cell_id"], json!("dc_run_fail#step3"));
}

#[test]
fn pack_catalogue_exposes_agent_and_biological_declarations_without_scores() {
    let result = call(
        &mut server(),
        "pack_catalogue",
        json!({ "section": "29", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["section"], json!("29"));
    assert_eq!(result["section_counts"]["15"], json!(25));
    assert_eq!(result["section_counts"]["29"], json!(21));
    assert_eq!(result["returned"].as_array().unwrap().len(), 3);
    assert_eq!(result["omitted"], json!(18));
    assert!(result["returned"].as_array().unwrap().iter().all(|pack| {
        pack["blueprint_module"]
            .as_str()
            .unwrap()
            .starts_with("29.")
    }));
    assert!(
        result["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("not measured"))
    );
}

#[test]
fn pack_coverage_audit_exposes_portfolio_gaps_and_refuses_unknown_subsets() {
    let result = call(
        &mut server(),
        "pack_coverage_audit",
        json!({ "section": "15", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/pack-coverage-audit/0.1")
    );
    assert_eq!(result["selected_pack_count"], json!(25));
    assert!(result["summary"]["families"].as_u64().unwrap() > 0);
    assert!(result["summary"]["covered"].as_u64().unwrap() > 0);
    assert_eq!(result["rows"].as_array().unwrap().len(), 3);
    assert!(result["rows_omitted"].as_u64().unwrap() > 0);
    assert!(
        result["summary"]["gap_summary"]
            .as_str()
            .unwrap()
            .contains("capability families")
    );

    let refused = call(
        &mut server(),
        "pack_coverage_audit",
        json!({ "pack_ids": ["pack-does-not-exist"] }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("pack_selection"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn pack_release_audit_preserves_stable_order_and_unsequenced_remainder() {
    let result = call(
        &mut server(),
        "pack_release_audit",
        json!({ "section": "15", "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/pack-release-audit/0.1")
    );
    assert_eq!(result["selected_pack_count"], json!(25));
    assert_eq!(result["sequenced_count"], json!(13));
    assert_eq!(result["unsequenced_count"], json!(12));
    assert_eq!(result["release_order"].as_array().unwrap().len(), 3);
    assert_eq!(result["release_order_omitted"], json!(10));
    assert_eq!(result["unsequenced"].as_array().unwrap().len(), 3);
    assert_eq!(result["unsequenced_omitted"], json!(9));
    assert_eq!(result["release_order"][0]["selected_position"], json!(1));
    assert_eq!(result["release_order"][0]["portfolio_position"], json!(1));
    assert!(result["wave_counts"].is_object());
    assert!(result["axis_counts"].is_object());
    assert!(
        result["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("not an approval"))
    );

    let refused = call(
        &mut server(),
        "pack_release_audit",
        json!({ "section": "15", "pack_ids": ["bio.statistical-estimands"] }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("pack_selection"));
    assert_eq!(
        refused["out_of_section_pack_ids"],
        json!(["bio.statistical-estimands"])
    );
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn conformance_run_verifies_fixtures_before_returning_release_evidence() {
    let result = call(
        &mut server(),
        "conformance_run",
        json!({ "include_details": true, "max_items": 3 }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["suite"]["fixture_drift"], json!([]));
    assert!(result["suite"]["case_count"].as_u64().unwrap() > 0);
    assert!(result["results"].as_array().unwrap().len() <= 3);
    assert!(result["release_decision"].is_object());
    assert!(
        result["summary"]
            .as_str()
            .unwrap()
            .contains("fiber-compiler-conformance")
    );
}

#[test]
fn provider_capability_gate_does_not_turn_untested_runtime_checks_into_claims() {
    let result = call(
        &mut server(),
        "provider_capability_gate",
        json!({
            "card": { "provider": "runtime-a", "states": {}, "measurements": [] },
            "required": ["host_escape"],
            "other_card": { "provider": "runtime-b", "states": {}, "measurements": [] }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["gate"]["outcome"], json!("blocked"));
    assert!(
        result["gate"]["unproven"][0]
            .as_str()
            .unwrap()
            .contains("untested")
    );
    assert_eq!(
        result["differential"]["HostEscape"]["drift"],
        json!("indeterminate")
    );
    assert_eq!(result["claims"], json!([]));
}
