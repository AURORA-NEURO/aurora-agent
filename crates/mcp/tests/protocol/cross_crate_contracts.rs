//! MCP contract tests for cross crate contracts contracts.

use super::*;

#[test]
fn capability_audit_proves_catalogue_and_transport_schema_parity() {
    let mut server = server();
    let result = call(&mut server, "capability_audit", json!({}));
    assert_eq!(result["workflow"], json!("capability_audit"));
    assert_eq!(
        result["healthy"],
        json!(true),
        "catalog_only={:?}; advertised_only={:?}; schema_findings={:?}",
        result["catalog_only_tools"],
        result["advertised_only_tools"],
        result["schema_quality"]["findings"]
    );
    assert_eq!(result["total_groups"], json!(CAPABILITY_GROUP_COUNT));
    assert_eq!(result["unique_catalog_tools"], json!(TOOL_DEFINITION_COUNT));
    assert_eq!(
        result["advertised_tool_count"],
        json!(TOOL_DEFINITION_COUNT)
    );
    assert_eq!(result["catalog_only_tools"], json!([]));
    assert_eq!(result["advertised_only_tools"], json!([]));
    assert_eq!(
        result["schema_quality"]["checked"],
        json!(TOOL_DEFINITION_COUNT)
    );
    assert_eq!(
        result["schema_quality"]["valid"],
        json!(TOOL_DEFINITION_COUNT)
    );
    assert_eq!(result["schema_quality"]["findings"], json!([]));
    assert!(
        !result["duplicate_group_memberships"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        result["invariants"]["multi_group_membership_is_allowed"],
        json!(true)
    );
    assert_eq!(
        result["invariants"]["all_input_schemas_are_well_formed"],
        json!(true)
    );
    assert_eq!(
        result["groups"].as_array().unwrap().len(),
        CAPABILITY_GROUP_COUNT
    );

    let compact = call(
        &mut server,
        "capability_audit",
        json!({"include_groups": false}),
    );
    assert!(compact.get("groups").is_none());
    assert_eq!(compact["healthy"], json!(true));

    let refused = call(
        &mut server,
        "capability_audit",
        json!({"include_groups": "yes"}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn capability_dashboard_separates_domain_surfaces_and_bounded_inventory() {
    let mut server = server();
    let source_plan = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "dashboard-evidence-subject",
            "source_tool": "modality_catalog",
            "connector_kind": "literature",
            "locator_kind": "uri",
            "locator": "https://example.org/dashboard-evidence",
            "retrieval_mode": "metadata_only",
            "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
            "does_not_claim": ["retrieval occurred"]
        }),
    );
    assert_eq!(source_plan["artifact_registry"]["indexed"], json!(true));
    let oncology = call(
        &mut server,
        "capability_dashboard",
        json!({"domain": "oncology", "include_tools": true, "include_gaps": true}),
    );
    assert_eq!(oncology["workflow"], json!("capability_dashboard"));
    assert_eq!(
        oncology["schema"],
        json!("bioprism-devplat-capability-dashboard/0.1")
    );
    assert_eq!(oncology["audit"]["selected_group_count"], json!(1));
    assert_eq!(
        oncology["audit"]["groups"][0]["id"],
        json!("biological_domains")
    );
    assert_eq!(
        oncology["audit"]["groups"][0]["readiness"],
        json!("callable")
    );
    assert!(
        oncology["audit"]["groups"][0]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "onco_response_assess")
    );
    assert_eq!(
        oncology["audit"]["groups"][0]["artifact_evidence"]["state"],
        json!("observed")
    );
    assert_eq!(
        oncology["audit"]["groups"][0]["artifact_evidence"]["matching_record_count"],
        json!(1)
    );
    assert_eq!(
        oncology["audit"]["groups"][0]["workflow_reconciliation_evidence"]["state"],
        json!("missing")
    );
    assert_eq!(
        oncology["audit"]["evidence"]["groups_with_artifact_evidence"],
        json!(1)
    );
    assert_eq!(
        oncology["audit"]["evidence"]["artifact_evidence_records"],
        json!(1)
    );
    assert_eq!(oncology["evidence_digest"].as_str().unwrap().len(), 64);
    assert_eq!(oncology["capability_dashboard_ready"], json!(true));
    assert_eq!(oncology["catalog_digest"].as_str().unwrap().len(), 64);
    assert_eq!(oncology["dashboard_digest"].as_str().unwrap().len(), 64);

    let bounded = call(
        &mut server,
        "capability_dashboard",
        json!({"max_groups": 1, "include_tools": false}),
    );
    assert_eq!(bounded["audit"]["selected_group_count"], json!(1));
    assert!(
        bounded["audit"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("bounded"))
    );
    assert!(bounded["audit"]["groups"][0].get("tools").is_none());

    let refused = call(
        &mut server,
        "capability_dashboard",
        json!({"max_groups": 0}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn new_surfaces_fail_closed_on_duplicate_facts_unknown_schemas_and_unbounded_requests() {
    let mut server = server();
    let request = json!({
        "principal": {
            "id": "agent-1",
            "role": "researcher",
            "clearance": { "max_classification": "public_aggregate", "compartments": [] },
            "site": "us",
            "authorities": []
        },
        "purpose": "research_analysis",
        "channel": "local_compute",
        "at": "2026-08-14T00:00:00Z"
    });
    let duplicate = call(
        &mut server,
        "policy_screen",
        json!({
            "world": WORLD,
            "request": request,
            "facts": ["fact.cohort", "fact.cohort"]
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(duplicate["error"].as_str().unwrap().contains("duplicate"));

    let unknown_schema = call(
        &mut server,
        "governance_schema_check",
        json!({ "schema": "invented/9.9" }),
    );
    assert_eq!(unknown_schema["__isError"], json!(true));
    assert!(
        unknown_schema["error"]
            .as_str()
            .unwrap()
            .contains("unknown schema")
    );

    let unbounded = call(
        &mut server,
        "developer_platform_status",
        json!({ "max_items": 0 }),
    );
    assert_eq!(unbounded["__isError"], json!(true));
    assert!(unbounded["error"].as_str().unwrap().contains("max_items"));

    let rank_too_small = call(
        &mut server,
        "capability_rank",
        json!({ "vectors": [metric_vector("only", 0.5, 0.5, "pack/4")] }),
    );
    assert_eq!(rank_too_small["__isError"], json!(true));
    assert!(
        rank_too_small["error"]
            .as_str()
            .unwrap()
            .contains("fewer than 2 items")
    );

    let conflicting_ci_inputs = call(
        &mut server,
        "research_ci_check",
        json!({ "document": "missing.json", "result": { "subject": "inline" } }),
    );
    assert_eq!(conflicting_ci_inputs["__isError"], json!(true));
    assert!(
        conflicting_ci_inputs["error"]
            .as_str()
            .unwrap()
            .contains("either document or inline")
    );

    let hub_unbounded = call(&mut server, "hub_lock", json!({ "max_items": 0 }));
    assert_eq!(hub_unbounded["__isError"], json!(true));
    assert!(
        hub_unbounded["error"]
            .as_str()
            .unwrap()
            .contains("max_items")
    );

    let tabular_conflict = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "source.csv",
            "csv": "subject\nS1\n",
            "document": WORLD,
            "profile": serde_json::to_value(TabularProfile::new("dataset")).unwrap()
        }),
    );
    assert_eq!(tabular_conflict["__isError"], json!(true));
    assert!(
        tabular_conflict["error"]
            .as_str()
            .unwrap()
            .contains("either csv or document")
    );

    let adaptive_unbounded = call(
        &mut server,
        "adaptive_panel",
        json!({ "panel": serde_json::to_value(AdaptivePanel::new(PanelConfig::default())).unwrap(), "max_items": 0 }),
    );
    assert_eq!(adaptive_unbounded["__isError"], json!(true));
    assert!(
        adaptive_unbounded["error"]
            .as_str()
            .unwrap()
            .contains("max_items")
    );

    let oracle_empty = call(
        &mut server,
        "oracle_combine",
        json!({ "subject": "artifact-1", "at": "2026-08-14T00:00:00Z", "judgements": [] }),
    );
    assert_eq!(oracle_empty["__isError"], json!(true));
    assert!(
        oracle_empty["error"]
            .as_str()
            .unwrap()
            .contains("fewer than 1 items")
    );

    let bundle_conflict = call(
        &mut server,
        "bundle_verify",
        json!({ "bundle": {}, "document": "missing.json" }),
    );
    assert_eq!(bundle_conflict["__isError"], json!(true));
    assert!(
        bundle_conflict["error"]
            .as_str()
            .unwrap()
            .contains("either bundle or document")
    );

    let capacity_conflict = call(
        &mut server,
        "ops_capacity",
        json!({ "model": {}, "workload": {}, "degradation_plan": {} }),
    );
    assert_eq!(capacity_conflict["__isError"], json!(true));
    assert!(
        capacity_conflict["error"]
            .as_str()
            .unwrap()
            .contains("invalid capacity model")
    );

    let duplicate_labels = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "duplicate-labels",
            "sources": [],
            "design": serde_json::to_value(StudyDesign::new(
                0,
                Selection::Undeclared
            )).unwrap(),
            "outcome_labels": ["same", "same"]
        }),
    );
    assert_eq!(duplicate_labels["__isError"], json!(true));
    assert!(
        duplicate_labels["error"]
            .as_str()
            .unwrap()
            .contains("duplicate outcome")
    );

    let influence_duplicate_group = call(
        &mut server,
        "influence_analyze",
        json!({
            "label": "bounded-input",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"], "table": [1.0, 2.0] }],
            "free": ["a"],
            "factor_group": ["f.a", "f.a"],
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(influence_duplicate_group["__isError"], json!(true));
    assert!(
        influence_duplicate_group["error"]
            .as_str()
            .unwrap()
            .contains("duplicate factor")
    );

    let influence_foreign_assumption = call(
        &mut server,
        "influence_analyze",
        json!({
            "label": "bounded-input",
            "variables": { "a": 2 },
            "assumed_variables": ["missing"],
            "factors": [{ "id": "f.a", "scope": ["a"] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(influence_foreign_assumption["__isError"], json!(true));
    assert!(
        influence_foreign_assumption["error"]
            .as_str()
            .unwrap()
            .contains("undeclared")
    );
}

#[test]
fn ops_acceptance_keeps_unverifiable_criteria_out_of_release_passes() {
    let result = call(&mut server(), "ops_acceptance", json!({ "max_items": 20 }));

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["summary"]["total"], json!(14));
    assert_eq!(result["summary"]["met"], json!(0));
    assert_eq!(result["summary"]["refuted"], json!(2));
    assert_eq!(result["summary"]["unverifiable"], json!(12));
    assert_eq!(result["summary"]["is_release_ready"], json!(false));
    assert_eq!(result["findings"].as_array().unwrap().len(), 14);
}

#[test]
fn ops_capacity_requires_qualified_work_and_visible_saturation() {
    let work_units =
        Assumption::measured("supply", 10.0, "work/epoch", "protocol fixture").unwrap();
    let model = CapacityModel::new(work_units, 1024);
    let calls = Assumption::assumed("calls", 2.0, "calls/epoch", "protocol fixture").unwrap();
    let cost = Assumption::measured("scan-cost", 3.0, "work/step", "protocol fixture").unwrap();
    let operation = Operation::new(
        "scan",
        Bound::Bounded { steps: 2 },
        ArtifactHandling::Streamed,
        cost,
    )
    .unwrap();
    let workload = Workload::new("protocol-workload", calls)
        .unwrap()
        .with(operation);
    let plan = DegradationPlan::declare(
        "visible-backpressure",
        [Concession::Throughput],
        "queue_depth",
    )
    .unwrap();
    let result = call(
        &mut server(),
        "ops_capacity",
        json!({
            "model": serde_json::to_value(model).unwrap(),
            "workload": serde_json::to_value(workload).unwrap(),
            "demand": serde_json::to_value(Demand { calls_per_epoch: 10.0 }).unwrap(),
            "degradation_plan": serde_json::to_value(plan).unwrap()
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["fully_measured"], json!(false));
    assert_eq!(result["saturation"]["ok"], json!(true));
    assert_eq!(
        result["saturation"]["value"]["saturation"],
        json!("saturated")
    );
    assert_eq!(
        result["saturation"]["value"]["plan"]["visible_as"],
        json!("queue_depth")
    );
}

#[test]
fn posterior_gate_keeps_vector_and_release_scalar_separate() {
    let scored = compose(
        "result-1",
        &[Contribution::new(
            ScoreTier::Execution,
            "deterministic-check",
            Conclusion::Pass,
        )],
        &UnknownPolicy::Block,
    )
    .unwrap();
    let observations = vec![
        Observation::new("capability-a", "parent-1", scored.clone()),
        Observation::new("capability-a", "parent-2", scored),
    ];
    let gate = ReleaseGate::new("release-a", "a named test gate for this protocol surface")
        .unwrap()
        .require("capability-a", CoverageFloor::requiring(2, 2.0).grounded());
    let result = call(
        &mut server(),
        "posterior_gate",
        json!({
            "observations": serde_json::to_value(observations).unwrap(),
            "gate": serde_json::to_value(gate).unwrap()
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/posterior-gate/0.1"));
    assert_eq!(result["schema_version"], json!("07.0.1"));
    assert_eq!(
        result["capabilities"]["capability-a"]["pass_rate"]["mean"],
        json!(1.0)
    );
    assert_eq!(result["unprovenanced_observations"], json!(2));
    assert_eq!(result["gate"]["ok"], json!(true));
    assert!(result["gate"]["value"]["sensitivity"].is_array());
}

#[test]
fn oracle_combine_keeps_grounded_decisions_and_suppressed_judgements_visible() {
    let at = UtcTimestamp::parse("2026-08-14T00:00:00Z").unwrap();
    let validity = ValidityWindow::new(at.clone(), None).unwrap();
    let deterministic = OracleManifest::new(
        OracleRef::new(
            OracleId::parse("reference:checksum").unwrap(),
            OracleVersion::new(1, 0, 0),
        ),
        OracleEvidenceTier::Deterministic,
        [Plane::Artifact],
        [],
        validity.clone(),
    )
    .unwrap()
    .disclaiming_the_rest();
    let judge = OracleManifest::new(
        OracleRef::new(
            OracleId::parse("review:human").unwrap(),
            OracleVersion::new(1, 0, 0),
        ),
        OracleEvidenceTier::Judge,
        [Plane::Artifact],
        [],
        validity,
    )
    .unwrap()
    .disclaiming_the_rest();
    let grounded = Judgement::from_manifest(
        &deterministic,
        &at,
        Position::Supported,
        Confidence::CERTAIN,
    );
    let opinion = Judgement::from_manifest(
        &judge,
        &at,
        Position::Contradicted,
        Confidence::new(0.99).unwrap(),
    );
    let result = call(
        &mut server(),
        "oracle_combine",
        json!({
            "subject": "artifact-1",
            "at": "2026-08-14T00:00:00Z",
            "judgements": [grounded, opinion],
            "minimum_deciding_tier": "judge"
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/oracle-combine/0.1"));
    assert_eq!(result["status"], json!("valid"));
    assert_eq!(result["deciding_tier"], json!("deterministic"));
    assert_eq!(result["suppressed_override"], json!(true));
    assert_eq!(result["contributing"].as_array().unwrap().len(), 1);
    assert_eq!(result["withheld"].as_array().unwrap().len(), 1);
}

#[test]
fn oracle_reference_panel_keeps_reader_splits_and_blinding_failures_explicit() {
    let panel = ReaderPanel::new([
        Read::independent("reader-a", "positive").citing(["feature-a"]),
        Read::independent("reader-b", "negative"),
        Read::post_discussion("reader-c", "positive"),
    ])
    .unwrap();
    let result = call(
        &mut server(),
        "oracle_reference_panel",
        json!({
            "panel": serde_json::to_value(panel).unwrap(),
            "rule": serde_json::to_value(ConsensusRule::Majority).unwrap(),
            "model_call": "positive"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["readers"], json!(2));
    assert_eq!(result["reads"].as_array().unwrap().len(), 3);
    assert_eq!(result["consensus"]["determination"], json!("unresolved"));
    assert_eq!(result["per_reader"]["reader-a"], json!(true));
    assert_eq!(result["per_reader"]["reader-b"], json!(false));

    let unblinded = ReaderPanel::new([Read::independent("reader-a", "positive")])
        .unwrap()
        .with_adjudication(Adjudication::new(
            "adjudicator",
            "positive",
            Blinding::unblinded(),
        ));
    let refused = call(
        &mut server(),
        "oracle_reference_panel",
        json!({
            "panel": serde_json::to_value(unblinded).unwrap(),
            "rule": serde_json::to_value(ConsensusRule::Adjudicated).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["consensus"]["determination"], json!("unresolved"));
}

#[test]
fn oracle_missingness_keeps_separation_complete_case_and_egress_distinct() {
    let pattern = AbsencePattern::new()
        .observe("site-a", 0, 10)
        .observe("site-b", 10, 0);
    let result = call(
        &mut server(),
        "oracle_missingness",
        json!({
            "pattern": serde_json::to_value(pattern).unwrap(),
            "field": serde_json::to_value(Field::individual("genomics")).unwrap(),
            "boundary": serde_json::to_value(Boundary::aggregate_only("federated-site")).unwrap(),
            "small_cell_floor": 5,
            "mechanism": serde_json::to_value(MissingnessMechanism::DependsOnUnobserved { suspected: "outcome".into() }).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["informativeness"]["determination"],
        json!("contradicted")
    );
    assert_eq!(result["egress"]["determination"], json!("contradicted"));
    assert_eq!(
        result["complete_case"]["determination"],
        json!("contradicted")
    );
    assert_eq!(result["small_cell_floor"], json!(5));
}

#[test]
fn lab_plan_orders_reachable_evidence_and_refuses_privacy_crossings() {
    let graph = json!({
        "goal": "choose a safe assay",
        "obligations": {
            "identity": {
                "id": "identity",
                "statement": "the specimen identity is established",
                "value": 3.0,
                "mandatory": true,
                "history": []
            },
            "assay": {
                "id": "assay",
                "statement": "the assay is validated",
                "depends_on": ["identity"],
                "value": 2.0,
                "mandatory": false,
                "history": []
            }
        }
    });
    let fast = AcquisitionAction {
        id: "inspect-metadata".into(),
        kind: AcquisitionKind::InspectMetadata,
        targets: vec!["identity".into()],
        value: 10.0,
        cost: AcquisitionCost::new(1, 0),
        privacy: PrivacyBoundary::Inside,
    };
    let blocked = AcquisitionAction {
        id: "private-query".into(),
        kind: AcquisitionKind::QueryDatabase,
        targets: vec!["identity".into()],
        value: 100.0,
        cost: AcquisitionCost::new(1, 0),
        privacy: PrivacyBoundary::Crosses {
            policy: "no-private-database".into(),
        },
    };
    let result = call(
        &mut server(),
        "lab_plan",
        json!({
            "graph": graph,
            "actions": [fast, blocked],
            "budget": AcquisitionCost::new(2, 0),
            "max_items": 10
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["ordered"][0]["action"], json!("inspect-metadata"));
    assert_eq!(result["excluded"][0][0], json!("private-query"));
    assert_eq!(
        result["excluded"][0][1]["excluded_because"],
        json!("crosses_boundary")
    );
    assert_eq!(result["should_escalate"], json!(true));
}

#[test]
fn lab_pareto_audit_preserves_tradeoffs_holes_archive_and_ambiguous_selection() {
    let result = call(
        &mut server(),
        "lab_pareto_audit",
        json!({
            "objectives": [
                { "axis": "admissible_rate", "direction": "higher_is_better" },
                { "axis": "cost_units", "direction": "lower_is_better" }
            ],
            "profiles": [
                {
                    "candidate": "cheap",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.80 },
                        "cost_units": { "state": "measured", "value": 10.0 }
                    }
                },
                {
                    "candidate": "accurate",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.95 },
                        "cost_units": { "state": "measured", "value": 40.0 }
                    }
                },
                {
                    "candidate": "dominated",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.70 },
                        "cost_units": { "state": "measured", "value": 50.0 }
                    }
                },
                {
                    "candidate": "hole",
                    "values": {
                        "admissible_rate": { "state": "measured", "value": 0.90 },
                        "cost_units": { "state": "unmeasured", "reason": "not_attempted" }
                    }
                }
            ],
            "relations": [{ "left": "cheap", "right": "accurate" }],
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-pareto-audit/0.1"));
    assert_eq!(result["front"]["count"], json!(3));
    assert_eq!(result["archived_count"], json!(1));
    assert_eq!(result["front"]["unresolved_count"], json!(1));
    assert_eq!(
        result["front"]["selection"]["selection"],
        json!("ambiguous")
    );
    assert_eq!(
        result["relations"][0]["relation"]["relation"],
        json!("incomparable")
    );
    assert_eq!(
        result["relations"][0]["relation"]["incomparable_because"],
        json!("trade_off")
    );
    assert_eq!(result["archived"][0]["dominated_by"], json!("accurate"));

    let refused = call(
        &mut server(),
        "lab_pareto_audit",
        json!({
            "objectives": [{ "axis": "cost_units", "direction": "lower_is_better" }],
            "profiles": [{
                "candidate": "missing-axis",
                "values": {}
            }]
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("profile_insertion"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn lab_branch_audit_keeps_undetermined_escalation_cost_catches_and_escapes_separate() {
    let result = call(
        &mut server(),
        "lab_branch_audit",
        json!({
            "policy": {
                "ceiling": { "max_branches": 4, "max_verifier_calls": 2 },
                "on_undetermined": "escalate",
                "rules": [
                    {
                        "id": "irreversible",
                        "trigger": { "trigger": "reversibility_at_least", "level": "irreversible" },
                        "action": "fork_suffixes",
                        "cost": { "branches": 2, "verifier_calls": 1 }
                    },
                    {
                        "id": "unmeasured-failure",
                        "trigger": { "trigger": "historical_failure_rate_at_least", "rate": 0.2 },
                        "action": "invoke_verifier",
                        "cost": { "branches": 1, "verifier_calls": 1 }
                    }
                ]
            },
            "decisions": [
                {
                    "decision": "external-write",
                    "features": {
                        "reversibility": "irreversible",
                        "permission": "external_effect",
                        "value_at_stake": "severe",
                        "unseparated_hypotheses": 2,
                        "unmet_mandatory_obligations": 1,
                        "historical_failure_rate": 0.8,
                        "verifier_available": false
                    },
                    "caught": { "what": "unsafe suffix", "would_have_been": "write would proceed" },
                    "escaped": "a secondary harm remained"
                },
                {
                    "decision": "benign-read",
                    "features": {
                        "reversibility": "reversible",
                        "permission": "read_only",
                        "value_at_stake": "negligible",
                        "unseparated_hypotheses": 0,
                        "unmet_mandatory_obligations": 0,
                        "historical_failure_rate": 0.0,
                        "verifier_available": true
                    }
                },
                {
                    "decision": "unmeasured-class",
                    "features": {
                        "reversibility": "reversible",
                        "permission": "write_scoped",
                        "value_at_stake": "moderate",
                        "unseparated_hypotheses": 1,
                        "unmet_mandatory_obligations": 0,
                        "historical_failure_rate": null,
                        "verifier_available": true
                    },
                    "escaped": "harm escaped without a measured failure rate"
                }
            ],
            "max_rows": 2
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-branch-audit/0.1"));
    assert_eq!(result["yield"]["decisions"], json!(3));
    assert_eq!(result["yield"]["escalations"], json!(2));
    assert_eq!(result["yield"]["escalations_on_undetermined"], json!(1));
    assert_eq!(result["yield"]["catches"], json!(1));
    assert_eq!(result["yield"]["wasted_escalations"], json!(1));
    assert_eq!(result["verdict"]["verdict"], json!("mixed"));
    assert_eq!(result["rows"].as_array().unwrap().len(), 2);
    assert_eq!(result["rows_omitted"], json!(1));

    let refused = call(
        &mut server(),
        "lab_branch_audit",
        json!({
            "policy": {
                "ceiling": { "max_branches": 1, "max_verifier_calls": 1 },
                "on_undetermined": "escalate",
                "rules": [{
                    "id": "over-budget",
                    "trigger": { "trigger": "no_verifier_available" },
                    "action": "invoke_verifier",
                    "cost": { "branches": 2, "verifier_calls": 0 }
                }]
            },
            "decisions": [{
                "decision": "one",
                "features": {
                    "reversibility": "reversible",
                    "permission": "read_only",
                    "value_at_stake": "negligible",
                    "unseparated_hypotheses": 0,
                    "unmet_mandatory_obligations": 0,
                    "historical_failure_rate": 0.0,
                    "verifier_available": true
                }
            }]
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("policy_validation"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn lab_holdout_audit_never_mints_clean_scores_after_selection_and_rollback() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let result = call(
        &mut server(),
        "lab_holdout_audit",
        json!({
            "cost_ceiling": 100,
            "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
            "holdouts": [{
                "id": "private-a",
                "partition": "rotating_private_certification",
                "query_budget": 4
            }],
            "current": "v1",
            "operations": [
                { "kind": "checkpoint", "label": "before-v2" },
                { "kind": "promote", "configuration": "v2", "selected_using": "private-a", "rationale": "won the development panel" },
                { "kind": "rollback", "checkpoint": "before-v2" },
                { "kind": "measure", "holdout": "private-a", "configuration": "v2", "metric": "admissible_rate", "value": 0.9 },
                { "kind": "measure", "holdout": "private-a", "configuration": "v1", "metric": "admissible_rate", "value": 0.8 },
                { "kind": "measure", "holdout": "private-a", "configuration": "v1", "metric": "admissible_rate", "value": 0.7 }
            ],
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/lab-holdout-audit/0.1")
    );
    assert_eq!(result["current"], json!("v1"));
    assert_eq!(result["measurement_count"], json!(1));
    assert_eq!(result["measurement_refusal_count"], json!(2));
    assert_eq!(result["rollback_count"], json!(1));
    assert_eq!(result["operations"][1]["result"], json!("accepted"));
    assert_eq!(
        result["operations"][2]["complete_restoration"],
        json!(false)
    );
    assert_eq!(
        result["operations"][3]["result"],
        json!("measurement_refused")
    );
    assert!(
        result["operations"][3]["refusal"]
            .as_str()
            .unwrap()
            .contains("used to select")
    );
    assert_eq!(
        result["operations"][4]["result"],
        json!("clean_measurement")
    );
    assert_eq!(
        result["operations"][5]["result"],
        json!("measurement_refused")
    );
    assert!(result["holdouts"][0]["exposure"].as_array().unwrap().len() >= 3);
    assert_eq!(result["holdouts"][0]["retired"], json!(false));
}

#[test]
fn lab_space_audit_preserves_lineage_diffs_and_fail_closed_candidate_validation() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .costing(2)
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let result = call(
        &mut server(),
        "lab_space_audit",
        json!({
            "cost_ceiling": 10,
            "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
            "inspect": ["v2"],
            "comparisons": [{"before": "v1", "after": "v2"}],
            "include_components": true,
            "max_rows": 1
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/lab-space-audit/0.1"));
    assert_eq!(result["registered_count"], json!(2));
    assert_eq!(result["candidate_rows_omitted"], json!(1));
    assert_eq!(result["inspection_rows"][0]["lineage"], json!(["v2", "v1"]));
    assert_eq!(result["inspection_rows"][0]["root"], json!("v1"));
    assert_eq!(
        result["comparison_rows"][0]["derived_relation"],
        json!(true)
    );
    assert!(
        result["comparison_rows"][0]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change.as_str().unwrap().contains("cost_units 0 -> 2"))
    );
    assert_eq!(result["comparison_rows"][0]["change_count"], json!(1));

    let invalid = call(
        &mut server(),
        "lab_space_audit",
        json!({
            "cost_ceiling": 10,
            "candidates": [{
                "id": "unsafe",
                "components": [
                    {"id": "select", "kind": "context_selector"},
                    {"id": "run", "kind": "executor"},
                    {"id": "stop", "kind": "terminator"}
                ],
                "cost_units": 0,
                "touches_protected": ["benchmark_splits"]
            }]
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["ok"], json!(false));
    assert_eq!(invalid["stage"], json!("candidate_validation"));
    assert_eq!(invalid["fail_closed"], json!(true));
    assert_eq!(invalid["space_committed"], json!(false));
    assert_eq!(
        invalid["candidate_rows"][0]["registration"],
        json!("not_attempted")
    );
}

#[test]
fn lab_evolution_audit_only_claims_clean_directional_improvement_and_retains_contamination() {
    let v1 = CandidateArchitecture::new("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let v2 = CandidateArchitecture::new("v2")
        .derived_from("v1")
        .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
        .with_component(ComponentSpec::new("run", ComponentKind::Executor))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
    let base = json!({
        "cost_ceiling": 100,
        "candidates": [serde_json::to_value(&v1).unwrap(), serde_json::to_value(&v2).unwrap()],
        "baseline": "v1",
        "candidate": "v2",
        "holdout": {
            "id": "private-a",
            "partition": "rotating_private_certification",
            "query_budget": 4
        },
        "card_id": "card-v2",
        "proposal": {
            "id": "proposal-v2",
            "rationale": "widen the protected closure",
            "target_failure_clusters": ["cluster:missing-closure"],
            "changed_artifacts": ["component select depth 3 -> 5"],
            "regression_cells": ["cell:closure"],
            "touches_protected": []
        },
        "rollback_handle": "v1",
        "direction": "higher_is_better",
        "would_have_to_be_true": ["the gain survives a second rotating private set"],
        "max_rows": 10
    });
    let claimed = call(
        &mut server(),
        "lab_evolution_audit",
        json!({
            "cost_ceiling": base["cost_ceiling"],
            "candidates": base["candidates"],
            "baseline": base["baseline"],
            "candidate": base["candidate"],
            "holdout": base["holdout"],
            "measurements": [
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.70 },
                { "configuration": "v2", "metric": "admissible_rate", "value": 0.83 }
            ],
            "card_id": base["card_id"],
            "proposal": base["proposal"],
            "rollback_handle": base["rollback_handle"],
            "direction": base["direction"],
            "would_have_to_be_true": base["would_have_to_be_true"],
            "max_rows": base["max_rows"]
        }),
    );
    assert_eq!(claimed["__isError"], json!(false));
    assert_eq!(claimed["ok"], json!(true));
    assert_eq!(
        claimed["schema"],
        json!("bioprism-mcp/lab-evolution-audit/0.1")
    );
    assert_eq!(claimed["status"], json!("improvement_claimed"));
    assert_eq!(claimed["claimable"], json!(true));
    assert!((claimed["claim"]["delta"].as_f64().unwrap() - 0.13).abs() < 1e-9);
    assert!(
        claimed["sentence"]
            .as_str()
            .unwrap()
            .contains("rotating_private_certification")
    );

    let contaminated = call(
        &mut server(),
        "lab_evolution_audit",
        json!({
            "cost_ceiling": base["cost_ceiling"],
            "candidates": base["candidates"],
            "baseline": base["baseline"],
            "candidate": base["candidate"],
            "holdout": base["holdout"],
            "measurements": [
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.70 },
                { "configuration": "v1", "metric": "admissible_rate", "value": 0.71 },
                { "configuration": "v2", "metric": "admissible_rate", "value": 0.83 }
            ],
            "card_id": base["card_id"],
            "proposal": base["proposal"],
            "rollback_handle": base["rollback_handle"],
            "direction": base["direction"],
            "would_have_to_be_true": base["would_have_to_be_true"]
        }),
    );
    assert_eq!(contaminated["__isError"], json!(false));
    assert_eq!(contaminated["ok"], json!(true));
    assert_eq!(contaminated["status"], json!("contaminated"));
    assert_eq!(contaminated["claimable"], json!(false));
    assert_eq!(
        contaminated["card"]["surface"]["surface"],
        json!("contaminated")
    );
    assert!(
        contaminated["claim_refusal"]
            .as_str()
            .unwrap()
            .contains("contaminated")
    );
}

#[test]
fn megafactory_twin_audit_requires_discrepancy_stable_direction_for_oracle_status() {
    let model = json!({
        "id": "reference",
        "compartments": ["a", "b", "c"],
        "rates": [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        "known_misspecification": "linear transfer has no saturation"
    });
    let stable = call(
        &mut server(),
        "megafactory_twin_audit",
        json!({
            "reference": model,
            "alternatives": [
                {
                    "id": "faster",
                    "compartments": ["a", "b", "c"],
                    "rates": [[0.0, 0.0, 0.5], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                    "known_misspecification": "linear transfer has no saturation"
                },
                {
                    "id": "slower",
                    "compartments": ["a", "b", "c"],
                    "rates": [[0.0, 0.0, 0.1], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                    "known_misspecification": "linear transfer has no saturation"
                }
            ],
            "initial": [1.0, 5.0, 0.0],
            "steps": 3,
            "intervention": { "compartment": "a", "hold_at": 1.0 },
            "outcome_compartment": "c"
        }),
    );
    assert_eq!(stable["ok"], json!(true));
    assert_eq!(stable["oracle_eligible"], json!(true));
    assert_eq!(stable["probe"]["models_disagreeing"], json!([]));

    let unstable = call(
        &mut server(),
        "megafactory_twin_audit",
        json!({
            "reference": {
                "id": "reference",
                "compartments": ["a", "b", "c"],
                "rates": [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
                "known_misspecification": "linear transfer has no saturation"
            },
            "alternatives": [{
                "id": "refilled",
                "compartments": ["a", "b", "c"],
                "rates": [[0.0, 0.0, 0.3], [0.9, 0.0, 0.0], [0.0, 0.0, 0.0]],
                "known_misspecification": "the refill compartment is treated as a source"
            }],
            "initial": [1.0, 5.0, 0.0],
            "steps": 3,
            "intervention": { "compartment": "a", "hold_at": 1.0 },
            "outcome_compartment": "c"
        }),
    );
    assert_eq!(unstable["ok"], json!(true));
    assert_eq!(unstable["oracle_eligible"], json!(false));
    assert!(
        unstable["headline"]
            .as_str()
            .unwrap()
            .contains("not benchmark ground truth")
    );
}

#[test]
fn megafactory_placement_audit_exposes_transfer_fencing_and_non_idempotent_duplicates() {
    let job = Job::new(
        "job-placement-mcp",
        ResourceClass::Evaluate,
        Idempotency::NonIdempotent,
        json!({ "suite": "release" }),
    );
    let worker = WorkerProfile::new(
        WorkerCapability::new("worker-mcp", vec![ResourceClass::Evaluate]),
        TrustDomain::new("worker-domain"),
        Locale::new("us"),
        Attestation::Attested {
            measurement: ContentHash::of_bytes(b"worker-image"),
            vouched_by: "attestor".into(),
        },
    );
    let request = WorkRequest {
        data_locale: Locale::new("eu"),
        access_tier: PlacementAccessTier::Restricted,
        oracle_domain: TrustDomain::new("oracle-domain"),
        input_bytes: 4096,
    };
    let result = call(
        &mut server(),
        "megafactory_placement_audit",
        json!({
            "job": serde_json::to_value(job).unwrap(),
            "request": serde_json::to_value(request).unwrap(),
            "worker": serde_json::to_value(worker).unwrap(),
            "item": "item-1",
            "commit_count": 2,
            "supersede_fence": true
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["placement"]["data_local"], json!(false));
    assert_eq!(result["placement"]["transfer_bytes"], json!(4096));
    assert_eq!(result["fencing"]["stale_admission"]["ok"], json!(false));
    assert_eq!(result["fencing"]["current_admission"]["ok"], json!(true));
    assert_eq!(
        result["ledger"]["duplicates"]["repeated_effect_incidents"],
        json!(1)
    );
    assert_eq!(result["ledger"]["has_incidents"], json!(true));
}

#[test]
fn influence_analyze_reports_bounds_and_unknown_structural_inputs_distinctly() {
    let bounded = call(
        &mut server(),
        "influence_analyze",
        json!({
            "label": "small-region",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"], "table": [1.0, 2.0] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(bounded["ok"], json!(true));
    assert_eq!(bounded["execute"], json!(false));
    assert_eq!(bounded["analysis"]["estimate"]["kind"], json!("bounded"));
    assert!(bounded["analysis"]["estimate"]["value"].is_number());
    assert!(bounded["analysis"]["estimate"]["method"].is_string());

    let unknown = call(
        &mut server(),
        "influence_analyze",
        json!({
            "label": "structural-region",
            "variables": { "a": 2 },
            "factors": [{ "id": "f.a", "scope": ["a"] }],
            "free": ["a"],
            "factor": "f.a",
            "perturbation": { "class": "removal" }
        }),
    );
    assert_eq!(unknown["ok"], json!(true));
    assert_eq!(unknown["analysis"]["estimate"]["kind"], json!("unknown"));
    assert!(unknown["analysis"]["estimate"]["reason"].is_string());
}

#[test]
fn token_context_plan_keeps_dry_run_restricted_data_fail_closed() {
    let request = json!({
        "world_ref": "world/demo",
        "decision_ref": "decision/demo",
        "role": "researcher",
        "policy_id": "policy/minimal",
        "envelope": { "total": 100 },
        "depth": "dry_run",
        "compiler_version": "compiler/1.0.0",
    });
    let restricted = json!([{
        "node_id": "raw/secret",
        "kind": "evidence",
        "restricted": true,
        "estimate": { "tokens": 10, "method": { "method": "declared_by_caller" } },
    }]);
    let refused = call(
        &mut server(),
        "token_context_plan",
        json!({ "request": request, "candidates": restricted }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("restricted"));

    let accepted = call(
        &mut server(),
        "token_context_plan",
        json!({
            "request": {
                "world_ref": "world/demo",
                "decision_ref": "decision/demo",
                "role": "researcher",
                "policy_id": "policy/minimal",
                "envelope": { "total": 100 },
                "depth": "l1",
                "compiler_version": "compiler/1.0.0",
            },
            "candidates": [{
                "node_id": "invariant/identity",
                "kind": "invariant",
                "mandatory": true,
                "estimate": { "tokens": 20, "method": { "method": "declared_by_caller" } },
            }, {
                "node_id": "evidence/summary",
                "kind": "summary",
                "estimate": { "tokens": 30, "method": { "method": "declared_by_caller" } },
            }],
        }),
    );
    assert_eq!(accepted["__isError"], json!(false));
    assert_eq!(accepted["plan"]["mandatory_estimate"]["tokens"], json!(20));
    assert_eq!(accepted["plan"]["optional_estimate"]["tokens"], json!(30));
}

#[test]
fn lens_catalogue_exposes_questions_and_unimplemented_section_remainder() {
    let result = call(&mut server(), "lens_catalogue", json!({}));
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["section_42_module_count"], json!(31));
    assert_eq!(result["implemented_count"], json!(6));
    assert!(
        result["implemented"]
            .as_array()
            .unwrap()
            .iter()
            .any(|lens| {
                lens["id"] == json!("cohort_leakage")
                    && lens["requires"].as_array().unwrap().len() >= 4
                    && lens["refuses"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|reason| reason == "scope_precondition_unmet")
            })
    );
    assert!(result["not_implemented"].as_array().unwrap().len() >= 10);
}

#[test]
fn lens_leakage_check_seals_nonvisual_findings_and_preserves_underdetermination() {
    let result = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": { "cohort": "C-1" },
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": ["ALT-77"],
                    "site": { "recorded": "known", "value": "MGH" },
                    "label_source_time": { "recorded": "known", "value": "2025-01-01T00:00:00Z" }
                }, {
                    "subject": "S003",
                    "split": "test",
                    "aliases": ["ALT-77"],
                    "site": { "recorded": "known", "value": "DFCI" },
                    "label_source_time": { "recorded": "known", "value": "2026-05-01T00:00:00Z" }
                }],
                "preprocessing": [{ "name": "normalizer", "fit_over": ["train", "test"] }],
                "decision_time": { "recorded": "known", "value": "2026-01-01T00:00:00Z" }
            },
            "include_spoken": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["outcome"], json!("answered"));
    assert_eq!(result["blueprint_module"], json!("42.10"));
    assert_eq!(result["witness_count"], json!(4));
    assert!(result["receipt"].as_str().unwrap().len() >= 32);
    assert!(
        result["report"]["outcome"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "identity_leakage")
    );
    assert!(
        result["spoken"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| { line.as_str().is_some_and(|line| line.contains("ALT-77")) })
    );

    let underdetermined = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": { "cohort": "C-1" },
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": [],
                    "site": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } },
                    "label_source_time": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } }
                }],
                "preprocessing": [],
                "decision_time": { "recorded": "missing", "missingness": { "class": "never_measured", "reason": "unrecorded" } }
            }
        }),
    );
    assert_eq!(underdetermined["__isError"], json!(false));
    assert_eq!(underdetermined["outcome"], json!("answered"));
    assert!(
        underdetermined["report"]["outcome"]["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "check_not_runnable")
    );
}

#[test]
fn lens_leakage_check_refuses_unbound_scope_before_answering() {
    let result = call(
        &mut server(),
        "lens_leakage_check",
        json!({
            "scope": {},
            "cohort": {
                "subjects": [{
                    "subject": "S001",
                    "split": "train",
                    "aliases": [],
                    "site": { "recorded": "known", "value": "MGH" },
                    "label_source_time": { "recorded": "known", "value": "2025-01-01T00:00:00Z" }
                }],
                "preprocessing": [],
                "decision_time": { "recorded": "known", "value": "2026-01-01T00:00:00Z" }
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["outcome"], json!("refused"));
    assert_eq!(result["report"]["outcome"]["outcome"], json!("refused"));
    assert_eq!(
        result["report"]["outcome"]["reason"],
        json!("scope_precondition_unmet")
    );
}

#[test]
fn ledger_ingest_preserves_quarantine_idempotency_time_axes_and_projections() {
    let parent = ledger_event_fixture(
        "specimen.collected",
        "patient-7/specimen-1",
        "2025-01-01T00:00:00Z",
        "parent-key",
    );
    let child = ledger_event_fixture(
        "measurement.recorded",
        "patient-7/specimen-1",
        "2024-01-01T00:00:00Z",
        "child-key",
    )
    .caused_by([bioprism_ids::EventId::parse("evt-000000000000").unwrap()]);
    let result = call(
        &mut server(),
        "ledger_ingest",
        json!({
            "events": serde_json::to_value(vec![child, parent.clone(), parent]).unwrap(),
            "include_receipts": true,
            "cut": serde_json::to_value(TemporalCut::known_at(
                LedgerRecordTime::parse("2024-06-01T00:00:00Z").unwrap()
            )).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["schema"], json!("bioprism-mcp/ledger-ingest/0.1"));
    assert_eq!(result["entries"], json!(2));
    assert_eq!(result["admissions"]["recorded"], json!(2));
    assert_eq!(result["admissions"]["duplicates"], json!(1));
    assert_eq!(result["admissions"]["quarantined"], json!(1));
    assert_eq!(result["admissions"]["released"], json!(1));
    assert_eq!(result["chain"]["status"], json!("intact"));
    assert_eq!(result["clock_anomalies"].as_array().unwrap().len(), 1);
    assert_eq!(result["quarantine"]["count"], json!(0));
    assert_eq!(result["latest_by_subject"]["count"], json!(1));
    assert_eq!(result["cut"]["count"], json!(1));
    assert!(
        result["latest_by_subject"]["items"][0]["payload_digest"]
            .as_str()
            .unwrap()
            .len()
            >= 32
    );
}

#[test]
fn fabric_synthesize_keeps_hard_rejections_out_of_the_pareto_frontier() {
    let goal = FabricGoal::new("produce a bounded decision", "decision-artifact").unwrap();
    let admissible = FabricCandidate::new("minimal", RoleGraph::new()).terminating_at("done");
    let rejected = FabricCandidate::new("unfinished", RoleGraph::new());
    let result = call(
        &mut server(),
        "fabric_synthesize",
        json!({
            "goal": serde_json::to_value(goal).unwrap(),
            "candidates": serde_json::to_value(vec![admissible, rejected]).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["candidate_count"], json!(2));
    assert_eq!(result["admissible_count"], json!(1));
    assert_eq!(result["eliminated_count"], json!(1));
    assert_eq!(result["artifact"]["frontier"], json!(["minimal"]));
    assert!(
        result["artifact"]["eliminated"]["unfinished"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason["reason"] == "missing_terminal_states")
    );
    assert!(result["unimplemented_stages"].as_array().unwrap().len() >= 5);
}

#[test]
fn bioql_compile_returns_a_typed_contract_and_refuses_missing_access_declarations() {
    let schema = QuerySchema::new().with(
        CollectionDecl::new("lesions")
            .declare(
                "tumor_volume",
                BioType::quantity(Unit::parse("mm3").expect("mm3 is a known unit")),
            )
            .costing(10),
    );
    let result = call(
        &mut server(),
        "bioql_compile",
        json!({
            "query": "select tumor_volume from lesions where tumor_volume > 12.5 mm3 labels { \"phi:deidentified\" } cost limit 100",
            "schema": serde_json::to_value(&schema).unwrap(),
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["typed_query"]["collection"], json!("lesions"));
    assert_eq!(result["typed_query"]["cost_estimate"], json!(20));
    assert_eq!(result["execution"], json!("not_performed"));

    let refused = call(
        &mut server(),
        "bioql_compile",
        json!({
            "query": "select tumor_volume from lesions cost limit 100",
            "schema": serde_json::to_value(schema).unwrap(),
        }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["fail_closed"], json!(true));
    assert!(
        refused["refusal"]
            .as_str()
            .unwrap()
            .contains("access labels")
    );
}

#[test]
fn weavelang_compile_returns_digests_and_replay_is_explicitly_local() {
    let result = call(
        &mut server(),
        "weavelang_compile",
        json!({
            "source": bioprism_weavelang::reference::COMPLETE_PROGRAM,
            "execute": false,
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert!(result["program"]["digest"].as_str().unwrap().len() >= 32);
    assert!(result["program"]["semantic_digest"].as_str().unwrap().len() >= 32);
    assert_eq!(result["execution"]["status"], json!("not_requested"));
    assert_eq!(result["execution"]["mode"], json!("replay"));

    let invalid = call(
        &mut server(),
        "weavelang_compile",
        json!({ "source": "not a weave program" }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(
        invalid["error"]
            .as_str()
            .unwrap()
            .contains("WeaveLang compilation refused")
    );
}

#[test]
fn choreography_check_separates_projection_refusal_from_bounded_model_results() {
    let valid = call(
        &mut server(),
        "choreography_check",
        json!({
            "global": {
                "node": "interaction",
                "from": "lead",
                "to": "reviewer",
                "branches": [{
                    "label": "approve",
                    "continuation": { "node": "end" }
                }]
            },
            "bound": { "max_states": 100, "max_depth": 20, "channel_capacity": 2 }
        }),
    );
    assert_eq!(valid["__isError"], json!(false));
    assert_eq!(valid["well_formed"], json!(true));
    assert_eq!(valid["projection_count"], json!(2));
    assert!(valid["model_check"]["deadlock"].is_object());

    let invalid = call(
        &mut server(),
        "choreography_check",
        json!({
            "global": {
                "node": "interaction",
                "from": "lead",
                "to": "lead",
                "branches": [{
                    "label": "self",
                    "continuation": { "node": "end" }
                }]
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["well_formed"], json!(false));
    assert_eq!(invalid["fail_closed"], json!(true));
    assert!(invalid["protocol_error"].is_object());
}

#[test]
fn projection_bundle_keeps_four_views_bound_to_one_compiled_certificate() {
    let result = call(
        &mut server(),
        "projection_bundle",
        json!({
            "world": WORLD,
            "query": QUERY,
            "include_views": false,
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(
        result["projections"],
        json!(["graph", "hypergraph", "timeline", "table"])
    );
    assert!(
        result["provenance"]["section_sha256"]
            .as_str()
            .unwrap()
            .len()
            >= 32
    );
    assert_eq!(result["fidelity"].as_array().unwrap().len(), 4);
    assert!(result["views"].is_null());
}
