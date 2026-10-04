//! Operator evidence and gate-review invariants.

use super::*;

#[test]
fn recovery_matrix_separates_restart_boundaries_and_non_claims() {
    let mission_path = test_state_path("recovery-mission");
    let event_path = test_state_path("recovery-event");
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_state_path: Some(mission_path.clone()),
            event_state_path: Some(event_path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let response = router.handle(request("GET", "/v1/recovery", json!({})));
    assert_eq!(response.status, 200);
    let matrix: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(matrix["schema"], "bioprism-recovery-matrix/0.1");
    assert_eq!(matrix["automatic_resume"], false);
    assert_eq!(matrix["automatic_external_delivery"], false);
    let boundaries = matrix["boundaries"].as_array().unwrap();
    let mission_jobs = boundaries
        .iter()
        .find(|boundary| boundary["id"] == "mission_jobs")
        .unwrap();
    assert_eq!(mission_jobs["checkpoint_present"], true);
    assert_eq!(mission_jobs["state_digest"].as_str().unwrap().len(), 64);
    assert_eq!(mission_jobs["integrity_verified"], true);
    let event_rows = boundaries
        .iter()
        .find(|boundary| boundary["id"] == "event_rows")
        .unwrap();
    assert_eq!(event_rows["configured"], true);
    assert_eq!(event_rows["checkpoint_present"], true);
    assert_eq!(event_rows["state_digest"].as_str().unwrap().len(), 64);
    assert_eq!(event_rows["integrity_verified"], true);
    let delivery_attempts = boundaries
        .iter()
        .find(|boundary| boundary["id"] == "delivery_attempts")
        .unwrap();
    assert_eq!(delivery_attempts["configured"], true);
    assert_eq!(delivery_attempts["checkpoint_present"], true);
    assert!(delivery_attempts["restores"][0]
        .as_str()
        .unwrap()
        .contains("provenance"));
    let secrets = boundaries
        .iter()
        .find(|boundary| boundary["id"] == "webhook_signing_secrets")
        .unwrap();
    assert_eq!(secrets["checkpoint_present"], false);
    assert!(secrets["does_not_restore"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("signing secrets")));
    assert_eq!(matrix["observed"]["retained_events"], 0);
    let _ = std::fs::remove_file(mission_path);
    let _ = std::fs::remove_file(event_path);
}

#[test]
fn operations_snapshot_composes_bounded_operator_evidence() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let event = router.handle(request("POST", "/v1/tools/modality_catalog", json!({})));
    assert_eq!(event.status, 200);

    let response = router.handle(request(
        "GET",
        "/v1/operations/snapshot?after=0&limit=1",
        json!({}),
    ));
    assert_eq!(response.status, 200);
    let snapshot: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(snapshot["schema"], "bioprism-operations-snapshot/0.1");
    assert_eq!(snapshot["after"], 0);
    assert_eq!(snapshot["limit"], 1);
    assert_eq!(
        snapshot["recent_events"]["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(snapshot["event_metrics"]["retained_events"], 1);
    assert_eq!(snapshot["mission_summary"]["total"], 0);
    assert_eq!(snapshot["persistence"]["events"]["enabled"], false);
    assert_eq!(
        snapshot["persistence"]["workflow_reconciliations"]["enabled"],
        false
    );
    assert_eq!(snapshot["reconciliation_summary"]["registry_size"], 0);
    assert_eq!(snapshot["reconciliation_summary"]["ready_count"], 0);
    assert_eq!(
        snapshot["reconciliation_summary"]["readiness_claimed"],
        false
    );
    assert_eq!(snapshot["recovery"]["automatic_resume"], false);
    assert_eq!(
        snapshot["domain_coverage"]["schema"],
        "bioprism-domain-coverage/0.1"
    );
    assert!(snapshot["domain_coverage"]["group_count"].as_u64().unwrap() > 0);
    assert_eq!(snapshot["domain_coverage"]["truncated"], false);
    assert_eq!(snapshot["consistency"]["cross_store_atomic"], false);
    assert_eq!(snapshot["consistency"]["clock_free"], true);
    assert_eq!(snapshot["capabilities"]["operations_snapshot"], true);
    assert_eq!(
        snapshot["capabilities"]["workflow_reconciliation_registry"],
        true
    );
    assert!(snapshot["operator_actions"].as_array().unwrap().len() >= 3);
    assert!(snapshot["non_claims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| {
            item.as_str()
                .is_some_and(|value| value.contains("network delivery"))
        }));
}

#[test]
fn operations_snapshot_rejects_unbounded_or_unknown_queries() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let too_large = router.handle(request(
        "GET",
        "/v1/operations/snapshot?limit=257",
        json!({}),
    ));
    assert_eq!(too_large.status, 422);
    let unknown = router.handle(request(
        "GET",
        "/v1/operations/snapshot?status=running",
        json!({}),
    ));
    assert_eq!(unknown.status, 400);
}

#[test]
fn operations_handoff_is_content_addressed_and_non_executing() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let response = router.handle(request(
        "POST",
        "/v1/operations/handoff",
        json!({
            "goal": "prepare an oncology evidence route",
            "group_ids": ["biological_domains"],
            "max_groups": 1
        }),
    ));
    assert_eq!(response.status, 200);
    let handoff: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(handoff["workflow"], "operations_domain_handoff");
    assert_eq!(handoff["schema"], "bioprism-operations-handoff/0.1");
    assert_eq!(handoff["execution"], "not_started");
    assert_eq!(handoff["selection"]["selector_mode"], "intersection");
    assert_eq!(handoff["coverage"]["included_group_count"], 1);
    assert_eq!(handoff["groups"][0]["id"], "biological_domains");
    assert_eq!(
        handoff["route_request"]["needs"][0]["group_id"],
        "biological_domains"
    );
    assert_eq!(handoff["handoff_id"].as_str().unwrap().len(), 64);
    assert!(handoff["next_steps"].as_array().unwrap().len() >= 3);

    let unresolved = router.handle(request(
        "POST",
        "/v1/operations/handoff",
        json!({ "domains": ["not-a-real-domain"] }),
    ));
    assert_eq!(unresolved.status, 200);
    let unresolved: Value = serde_json::from_slice(&unresolved.body).unwrap();
    assert_eq!(unresolved["handoff_status"], "unresolved_domain");
    assert_eq!(
        unresolved["coverage"]["unresolved_domains"][0],
        "not-a-real-domain"
    );

    let invalid = router.handle(request(
        "POST",
        "/v1/operations/handoff",
        json!({ "unexpected": true }),
    ));
    assert_eq!(invalid.status, 422);
}

#[test]
fn operations_domain_activity_separates_catalogue_from_observation() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    assert_eq!(
        router
            .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
            .status,
        200
    );
    let response = router.handle(request(
        "GET",
        "/v1/operations/domains?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(response.status, 200);
    let activity: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(activity["workflow"], "operations_domain_activity");
    assert_eq!(
        activity["schema"],
        "bioprism-operations-domain-activity/0.1"
    );
    assert_eq!(activity["event_cursor"]["returned_events"], 1);
    assert_eq!(activity["summary"]["tool_events_scanned"], 1);
    assert_eq!(activity["summary"]["attributed_tool_events"], 1);
    assert!(
        activity["summary"]["groups_with_observed_activity"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(activity["observation_policy"]["readiness_claimed"], false);
    assert!(activity["groups"].as_array().unwrap().iter().any(|group| {
        group["observed_tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|tool| tool == "modality_catalog"))
    }));

    let invalid = router.handle(request(
        "GET",
        "/v1/operations/domains?limit=257",
        json!({}),
    ));
    assert_eq!(invalid.status, 422);
}

#[test]
fn operations_domain_gates_require_separate_evidence_channels() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let completed = json!({
        "result": {
            "isError": false,
            "content": [{"type": "text", "text": "{}"}]
        }
    });
    router.record_tool_event("gate-1", "modality_catalog", &completed);
    router.record_tool_event("gate-2", "bioeval_reference_audit", &completed);
    router.record_tool_event("gate-3", "safety_release_gate", &completed);

    let response = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(response.status, 200);
    let gates: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(gates["workflow"], "operations_domain_gates");
    assert_eq!(gates["schema"], "bioprism-operations-domain-gates/0.1");
    let initial_gate_digest = gates["gate_digest"].as_str().unwrap().to_owned();
    assert_eq!(gates["summary"]["tool_events_scanned"], 3);
    assert_eq!(gates["summary"]["completed_tool_events"], 3);
    assert_eq!(gates["summary"]["readiness_claimed"], false);
    assert_eq!(gates["gate_policy"]["readiness_claimed"], false);
    assert_eq!(gates["gate_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        gates["gate_digest_scope"],
        "operations_evidence_and_reconciliation_projection_without_gate_digest"
    );
    let biological = gates["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(
        biological["gates"]["observed_activity"]["state"],
        "observed"
    );
    assert_eq!(
        biological["gates"]["transport_completion"]["state"],
        "observed"
    );
    assert_eq!(
        biological["gates"]["evaluation_evidence"]["state"],
        "observed"
    );
    assert_eq!(biological["gates"]["safety_evidence"]["state"], "observed");
    assert_eq!(biological["gates"]["release_evidence"]["state"], "observed");
    assert_eq!(
        biological["gates"]["reconciliation_evidence"]["state"],
        "missing"
    );
    assert_eq!(biological["gate_state"], "review_required");
    assert_eq!(
        biological["gates"]["evaluation_evidence"]["scope"],
        "cross_domain_control_plane_event_page"
    );
    assert_eq!(
        biological["gates"]["domain_evaluator_evidence"]["state"],
        "observed"
    );
    assert_eq!(
        biological["gates"]["domain_evaluator_evidence"]["scope"],
        "completed_evaluator_tool_exact_or_catalogue_group_binding"
    );
    assert_eq!(biological["gates"]["artifact_evidence"]["state"], "missing");
    assert_eq!(gates["summary"]["groups_with_artifact_evidence"], 0);
    assert_eq!(
        gates["gate_policy"]["optional_evidence_gates"],
        json!(["artifact_evidence"])
    );

    let artifact = router.handle(request(
        "POST",
        "/v1/artifacts",
        json!({
            "kind": "domain_report",
            "subject_id": "gate-artifact",
            "domains": ["oncology"],
            "parent_digests": [],
            "artifact": {"status": "review"}
        }),
    ));
    assert_eq!(artifact.status, 201);
    let with_artifact = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(with_artifact.status, 200);
    let with_artifact: Value = serde_json::from_slice(&with_artifact.body).unwrap();
    let biological = with_artifact["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(
        biological["gates"]["artifact_evidence"]["state"],
        "observed"
    );
    assert_eq!(
        biological["gates"]["artifact_evidence"]["matching_record_count"],
        1
    );
    assert_eq!(biological["gate_state"], "review_required");
    assert!(
        with_artifact["summary"]["groups_with_artifact_evidence"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut incomplete_reconciliation = json!({
        "ok": true,
        "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
        "workflow": "domain_workflow_reconcile",
        "workflow_id": "biological_domains",
        "workflow_digest": "a".repeat(64),
        "catalog_digest": "b".repeat(64),
        "domain_contract_digest": "c".repeat(64),
        "mission_id": "gate-reconciliation-mission",
        "mission_plan_digest": "d".repeat(64),
        "source": "mission_report",
        "completion": {"status": "partial", "ready": false, "review_required": true},
        "evidence": {"evidence_valid": false},
        "integrity": {"valid": true, "finding_count": 1, "findings": [{"code": "evidence_incomplete", "severity": "warning", "message": "evidence remains incomplete"}]},
        "execution": "not_started"
    });
    incomplete_reconciliation["reconciliation_digest"] =
        json!(ContentHash::of_value(&incomplete_reconciliation)
            .unwrap()
            .to_string());
    let imported = router.handle(request(
        "POST",
        "/v1/domain-workflows/reconciliations",
        json!({"record": incomplete_reconciliation}),
    ));
    assert_eq!(imported.status, 201);
    let blocked = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(blocked.status, 200);
    let blocked: Value = serde_json::from_slice(&blocked.body).unwrap();
    let biological = blocked["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(
        biological["gates"]["reconciliation_evidence"]["state"],
        "incomplete"
    );
    assert_eq!(biological["gate_state"], "insufficient_evidence");
    assert_eq!(blocked["summary"]["groups_reconciliation_blocked"], 1);
    assert_ne!(
        blocked["gate_digest"].as_str(),
        Some(initial_gate_digest.as_str())
    );

    let invalid = router.handle(request("GET", "/v1/operations/gates?limit=257", json!({})));
    assert_eq!(invalid.status, 422);
}

#[test]
fn operations_gate_reconciliation_matrix_binds_every_workspace_group() {
    let reconciliation_path = test_state_path("gate-reconciliation-matrix");
    let config = ApiConfig {
        reconciliation_state_path: Some(reconciliation_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let group_ids = operations_domain_coverage()["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|group| group["id"].as_str())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!group_ids.is_empty());

    for group_id in &group_ids {
        let mut record = json!({
            "ok": true,
            "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
            "workflow": "domain_workflow_reconcile",
            "workflow_id": group_id,
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "mission_id": format!("gate-reconciliation-matrix-{group_id}"),
            "mission_plan_digest": "d".repeat(64),
            "source": "mission_report",
            "completion": {"status": "partial", "ready": false, "review_required": true},
            "evidence": {"evidence_valid": false},
            "integrity": {"valid": true, "finding_count": 1, "findings": [{"code": "evidence_incomplete", "severity": "warning", "message": "evidence remains incomplete"}]},
            "execution": "not_started"
        });
        record["reconciliation_digest"] =
            json!(ContentHash::of_value(&record).unwrap().to_string());
        let imported = router.handle(request(
            "POST",
            "/v1/domain-workflows/reconciliations",
            json!({"record": record}),
        ));
        assert_eq!(imported.status, 201, "failed to import {group_id}");
    }

    let response = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=256",
        json!({}),
    ));
    assert_eq!(response.status, 200);
    let gates: Value = serde_json::from_slice(&response.body).unwrap();
    let rows = gates["groups"].as_array().unwrap();
    assert_eq!(rows.len(), group_ids.len());
    assert_eq!(
        gates["summary"]["groups_reconciliation_blocked"],
        group_ids.len()
    );
    for group_id in &group_ids {
        let group = rows
            .iter()
            .find(|group| group["id"] == *group_id)
            .unwrap_or_else(|| panic!("missing gate row for {group_id}"));
        assert_eq!(
            group["gates"]["reconciliation_evidence"]["workflow_id"],
            *group_id
        );
        assert_eq!(
            group["gates"]["reconciliation_evidence"]["state"],
            "incomplete"
        );
        assert_eq!(
            group["gates"]["reconciliation_evidence"]["readiness_claimed"],
            false
        );
        let expected_gate_state = if group["missing_tool_count"].as_u64().unwrap_or(0) > 0 {
            "catalogue_blocked"
        } else {
            "insufficient_evidence"
        };
        assert_eq!(
            group["gate_state"], expected_gate_state,
            "catalogue precedence must remain explicit for {group_id}"
        );
    }

    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_query = restored.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations?limit=256",
        json!({}),
    ));
    assert_eq!(restored_query.status, 200);
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    let restored_rows = restored_query["rows"].as_array().unwrap();
    assert_eq!(restored_rows.len(), group_ids.len());
    assert!(restored_rows
        .iter()
        .all(|row| { row["completion_status"] == "partial" && row["workflow_id"].is_string() }));
    let _ = std::fs::remove_file(reconciliation_path);
}

#[test]
fn operations_gate_reviews_are_content_addressed_replayable_and_restart_aware() {
    let path = test_state_path("gate-reviews");
    let config = ApiConfig {
        event_state_path: Some(path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let completed = json!({
        "result": {
            "isError": false,
            "content": [{"type": "text", "text": "{}"}]
        }
    });
    router.record_tool_event("review-gate-1", "modality_catalog", &completed);
    router.record_tool_event("review-gate-2", "bioeval_reference_audit", &completed);
    router.record_tool_event("review-gate-3", "safety_release_gate", &completed);
    let gates = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=256",
        json!({}),
    ));
    let gates: Value = serde_json::from_slice(&gates.body).unwrap();
    let review = router.handle(request(
        "POST",
        "/v1/operations/gate-reviews",
        json!({
            "gate_digest": gates["gate_digest"],
            "reviewer": "operator-1",
            "rationale": "reviewed bounded evidence page",
            "group_ids": ["biological_domains"],
            "accepted_gates": {
                "biological_domains": operations_required_gates()
            }
        }),
    ));
    assert_eq!(review.status, 201);
    let review: Value = serde_json::from_slice(&review.body).unwrap();
    let review_id = review["review_id"].as_str().unwrap().to_string();
    assert_eq!(review_id.len(), 64);
    assert_eq!(review["acceptance"]["review_id"], review["review_id"]);

    let replay = router.handle(request(
        "GET",
        &format!("/v1/operations/gate-reviews?review_id={review_id}"),
        json!({}),
    ));
    assert_eq!(replay.status, 200);
    let replay: Value = serde_json::from_slice(&replay.body).unwrap();
    assert_eq!(replay["found"], true);
    assert_eq!(replay["review_count"], 1);

    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let preflight = restored.handle(request(
        "POST",
        "/v1/missions/preflight",
        json!({
            "mission_id": "review-bound-mission",
            "goal": "preview reviewed biological route",
            "steps": [{
                "id": "catalog",
                "domain": "biological",
                "capability": "catalogue",
                "objective": "inspect modality support",
                "tool": "modality_catalog"
            }],
            "policy": {"execute": true, "allowed_tools": ["modality_catalog"]},
            "operations_gate_acceptance": review["acceptance"]
        }),
    ));
    assert_eq!(preflight.status, 200);
    let preflight: Value = serde_json::from_slice(&preflight.body).unwrap();
    assert_eq!(preflight["operations_evidence"]["review_present"], true);
    assert_eq!(preflight["operations_evidence"]["acceptance_valid"], true);
    assert_eq!(
        preflight["operations_evidence"]["decision"],
        "review_required"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn executable_missions_retain_gate_review_and_domain_evaluator_provenance() {
    let event_path = test_state_path("mission-provenance-events");
    let mission_path = test_state_path("mission-provenance-missions");
    let config = ApiConfig {
        event_state_path: Some(event_path.clone()),
        mission_state_path: Some(mission_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let completed = json!({
        "result": {
            "isError": false,
            "content": [{"type": "text", "text": "{}"}]
        }
    });
    router.record_tool_event("provenance-gate-1", "modality_catalog", &completed);
    router.record_tool_event("provenance-gate-2", "bioeval_reference_audit", &completed);
    router.record_tool_event("provenance-gate-3", "safety_release_gate", &completed);
    let gates = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=256",
        json!({}),
    ));
    let gates: Value = serde_json::from_slice(&gates.body).unwrap();
    let review = router.handle(request(
        "POST",
        "/v1/operations/gate-reviews",
        json!({
            "gate_digest": gates["gate_digest"],
            "reviewer": "operator-provenance",
            "rationale": "reviewed domain-bound evidence",
            "group_ids": ["biological_domains"],
            "accepted_gates": {"biological_domains": operations_required_gates()}
        }),
    ));
    assert_eq!(review.status, 201);
    let review: Value = serde_json::from_slice(&review.body).unwrap();

    let submitted = router.handle(request(
        "POST",
        "/v1/missions",
        json!({
            "mission_id": "provenance-mission",
            "goal": "execute a reviewed bounded biological inspection",
            "steps": [{
                "id": "catalog",
                "domain": "biological",
                "capability": "catalogue",
                "objective": "inspect modality support",
                "tool": "modality_catalog"
            }],
            "policy": {"execute": true, "allowed_tools": ["modality_catalog"]},
            "operations_gate_acceptance": review["acceptance"]
        }),
    ));
    assert_eq!(submitted.status, 202);
    let submitted: Value = serde_json::from_slice(&submitted.body).unwrap();
    assert_eq!(
        submitted["execution_provenance"]["schema"],
        "bioprism-mission-execution-provenance/0.1"
    );
    assert_eq!(
        submitted["execution_provenance"]["review_id"],
        review["review_id"]
    );
    assert!(submitted["execution_provenance"]["review_event_id"].is_u64());
    assert!(submitted["execution_provenance"]["accepted_event_id"].is_u64());
    assert_eq!(
        submitted["execution_provenance"]["readiness_claimed"],
        false
    );

    let provenance = router.handle(request(
        "GET",
        "/v1/missions/provenance-mission/provenance",
        json!({}),
    ));
    assert_eq!(provenance.status, 200);
    let provenance: Value = serde_json::from_slice(&provenance.body).unwrap();
    assert_eq!(
        provenance["provenance"]["gate_digest"],
        gates["gate_digest"]
    );
    let biological_evidence = provenance["provenance"]["operations_evidence"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"] == "biological_domains")
        .unwrap();
    assert_eq!(
        biological_evidence["gates"]["domain_evaluator_evidence"]["state"],
        "observed"
    );

    let events = router.handle(request("GET", "/v1/events?after=0&limit=32", json!({})));
    let events: Value = serde_json::from_slice(&events.body).unwrap();
    assert!(events["page"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| {
            event["event_type"] == "mission.execution.accepted"
                && event["payload"]["provenance"]["review_id"] == review["review_id"]
        }));
    router.handle(request("POST", "/v1/missions/persistence/flush", json!({})));

    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_provenance = restored.handle(request(
        "GET",
        "/v1/missions/provenance-mission/provenance",
        json!({}),
    ));
    assert_eq!(restored_provenance.status, 200);
    let restored_provenance: Value = serde_json::from_slice(&restored_provenance.body).unwrap();
    assert_eq!(
        restored_provenance["provenance"]["provenance_digest"],
        provenance["provenance"]["provenance_digest"]
    );
    let _ = std::fs::remove_file(event_path);
    let _ = std::fs::remove_file(mission_path);
}
