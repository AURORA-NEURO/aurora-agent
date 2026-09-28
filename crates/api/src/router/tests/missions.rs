//! Mission dispatch, checkpoint, and cancellation invariants.

use super::*;

#[test]
fn durable_mission_state_restores_terminal_jobs_and_fails_interrupted_jobs() {
    let path = test_state_path("restart");
    let progress = mission_progress_json(&MissionProgressState::new(1));
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version": LEGACY_MISSION_STATE_SCHEMA_VERSION,
            "missions": [
                {
                    "mission_id": "active-before-restart",
                    "total_steps": 1,
                    "status": "running",
                    "cancel_requested": false,
                    "cancel_reason": null,
                    "progress": progress,
                    "trace": [],
                    "result": null,
                    "result_omitted": null,
                    "error": null,
                    "recovered_after_restart": false
                },
                {
                    "mission_id": "terminal-before-restart",
                    "total_steps": 0,
                    "status": "succeeded",
                    "cancel_requested": false,
                    "cancel_reason": null,
                    "progress": mission_progress_json(&MissionProgressState::new(0)),
                    "trace": [],
                    "result": {"mission_status": "succeeded", "results": []},
                    "result_omitted": null,
                    "error": null,
                    "recovered_after_restart": false
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();

    let persistence = router.handle(request("GET", "/v1/missions/persistence", json!({})));
    let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
    assert_eq!(persistence["enabled"], true);
    assert_eq!(persistence["state_digest"].as_str().unwrap().len(), 64);
    assert_eq!(persistence["integrity_verified"], true);
    assert_eq!(persistence["event_log_durable"], false);
    assert_eq!(
        router
            .handle(request("POST", "/v1/missions/persistence/flush", json!({})))
            .status,
        200
    );

    let active = router.handle(request(
        "GET",
        "/v1/missions/active-before-restart",
        json!({}),
    ));
    let active: Value = serde_json::from_slice(&active.body).unwrap();
    assert_eq!(active["status"], "failed");
    assert_eq!(active["recovered_after_restart"], true);
    assert!(active["error"].as_str().unwrap().contains("not resumed"));

    let terminal = router.handle(request(
        "GET",
        "/v1/missions/terminal-before-restart",
        json!({}),
    ));
    let terminal: Value = serde_json::from_slice(&terminal.body).unwrap();
    assert_eq!(terminal["status"], "succeeded");
    assert_eq!(terminal["result"]["mission_status"], "succeeded");

    let persisted: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(persisted["schema_version"], MISSION_STATE_SCHEMA_VERSION);
    assert_eq!(persisted["state_digest"].as_str().unwrap().len(), 64);
    let persisted_active = persisted["missions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|mission| mission["mission_id"] == "active-before-restart")
        .unwrap();
    assert_eq!(persisted_active["status"], "failed");
    assert_eq!(persisted_active["recovered_after_restart"], true);
    let mut tampered = persisted;
    tampered["missions"][0]["status"] = json!("succeeded");
    std::fs::write(&path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
    let observed = router.handle(request("GET", "/v1/missions/persistence", json!({})));
    let observed: Value = serde_json::from_slice(&observed.body).unwrap();
    assert_eq!(observed["file_present"], true);
    assert_eq!(observed["integrity_verified"], false);
    let error = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .err()
    .expect("tampered mission checkpoint must be rejected");
    assert!(error.contains("state_digest"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn durable_evaluator_replay_summary_survives_result_omission() {
    let path = test_state_path("evaluator-replay-summary");
    let config = ApiConfig {
        mission_state_path: Some(path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let report = json!({
        "workflow": "agent_mission",
        "plan": {"mission_id": "summary-only"},
        "mission_status": "succeeded",
        "claim_lineage": {"claims": []}
    });
    let summary = evaluator_replay_summary(&report, "summary-only").expect("replay summary");
    router.mission_jobs.lock().unwrap().insert(
        "summary-only".into(),
        Arc::new(MissionJob {
            cancellation: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(MissionJobState {
                total_steps: 0,
                trace: Vec::new(),
                progress: MissionProgressState::new(0),
                status: "succeeded".into(),
                cancel_requested: false,
                cancel_reason: None,
                result: None,
                result_omitted: Some(json!({"bytes": 300_000, "sha256": "a".repeat(64)})),
                evaluator_replay_summary: Some(summary),
                route_review_provenance: None,
                error: None,
                recovered_after_restart: false,
                execution_provenance: None,
            })),
        }),
    );
    router.persist_mission_registry().unwrap();

    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let response = restored.handle(request(
        "GET",
        "/v1/missions/summary-only/evaluator-replay?include_fixtures=false&max_items=32",
        json!({}),
    ));
    assert_eq!(response.status, 200);
    let value: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(value["workflow"], "mission_evaluator_replay_query");
    assert_eq!(value["retention"]["mode"], "summary_only");
    assert_eq!(value["retention"]["result_retained"], false);
    assert_eq!(value["retention"]["summary_retained"], true);
    assert_eq!(
        value["replay"]["workflow"],
        "mission_evaluator_replay_summary"
    );
    assert_eq!(value["replay"]["coverage"]["catalogue_group_count"], 29);
    let comparison = restored.handle(request(
        "GET",
        "/v1/missions/summary-only/evaluator-replay/compare?max_items=32",
        json!({}),
    ));
    assert_eq!(comparison.status, 200);
    let comparison: Value = serde_json::from_slice(&comparison.body).unwrap();
    assert_eq!(comparison["catalog_drift"]["status"], "not_recorded");
    let bundle = restored.handle(request(
        "GET",
        "/v1/missions/summary-only/evidence-bundle?include_result=false&include_trace=false",
        json!({}),
    ));
    assert_eq!(bundle.status, 200);
    let bundle: Value = serde_json::from_slice(&bundle.body).unwrap();
    assert_eq!(bundle["retention"]["mode"], "summary_only");
    assert_eq!(bundle["result"], Value::Null);
    assert_eq!(
        bundle["evaluator_replay"]["workflow"],
        "mission_evaluator_replay_summary"
    );
    assert_eq!(bundle["bundle_digest"].as_str().unwrap().len(), 64);
    let invalid = restored.handle(request(
        "GET",
        "/v1/missions/summary-only/evaluator-replay?max_items=513",
        json!({}),
    ));
    assert_eq!(invalid.status, 422);
    let _ = std::fs::remove_file(path);
}

#[test]
fn durable_mission_state_omits_large_results_with_digest_metadata() {
    let state = MissionJobState {
        total_steps: 0,
        trace: Vec::new(),
        progress: MissionProgressState::new(0),
        status: "succeeded".into(),
        cancel_requested: false,
        cancel_reason: None,
        result: Some(Value::String(
            "x".repeat(MAX_PERSISTED_MISSION_RESULT_BYTES + 1),
        )),
        result_omitted: None,
        evaluator_replay_summary: None,
        route_review_provenance: None,
        error: None,
        recovered_after_restart: false,
        execution_provenance: None,
    };
    let persisted = durable_mission_state_json("large-result", &state);
    assert!(persisted["result"].is_null());
    assert_eq!(
        persisted["result_omitted"]["bytes"],
        (MAX_PERSISTED_MISSION_RESULT_BYTES + 3) as u64
    );
    assert!(persisted["result_omitted"]["sha256"].as_str().is_some());
}

#[test]
fn asynchronous_missions_validate_poll_and_reject_duplicate_ids() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let subscription = router.handle(request(
        "POST",
        "/v1/webhooks/subscriptions",
        json!({
            "id": "mission-events",
            "endpoint": "https://example.test/mission-events",
            "events": ["mission.trace"],
            "secret": "0123456789abcdef"
        }),
    ));
    assert_eq!(subscription.status, 201);
    let body = json!({
        "mission_id": "api-async-1",
        "goal": "plan an asynchronous cross-domain mission",
        "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}],
        "claim_requests": [{"id": "catalog-observed", "claim": "The catalogue response was returned by the named tool.", "domains": ["workspace"], "requires_steps": ["catalog"], "evidence_mode": "successful_tool_result"}]
    });
    let submitted = router.handle(request("POST", "/v1/missions", body.clone()));
    assert_eq!(submitted.status, 202);
    let duplicate = router.handle(request("POST", "/v1/missions", body));
    assert_eq!(duplicate.status, 409);

    let mut status = Value::Null;
    for _ in 0..100 {
        let response = router.handle(request("GET", "/v1/missions/api-async-1", json!({})));
        assert_eq!(response.status, 200);
        status = serde_json::from_slice(&response.body).unwrap();
        if status["status"] == "planned" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(status["status"], "planned");
    assert_eq!(status["result"]["mission_status"], "planned");
    assert_eq!(status["progress"]["phase"], "planned");
    assert_eq!(status["progress"]["total_steps"], 1);
    assert_eq!(status["progress"]["completed_steps"], 0);
    assert_eq!(status["progress"]["last_event"], "mission.completed");
    assert_eq!(
        status["result"]["claim_lineage"]["claims"][0]["id"],
        "catalog-observed"
    );
    assert_eq!(
        status["result"]["claim_lineage"]["claims"][0]["claimable"],
        false
    );
    let claims = router.handle(request("GET", "/v1/missions/api-async-1/claims", json!({})));
    assert_eq!(claims.status, 200);
    let claims: Value = serde_json::from_slice(&claims.body).unwrap();
    assert_eq!(
        claims["schema"],
        "bioprism-mission-claim-lineage-response/0.1"
    );
    assert_eq!(claims["claim_lineage"]["claims"][0]["claimable"], false);
    let replay = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/evaluator-replay?include_fixtures=false&max_items=64",
        json!({}),
    ));
    assert_eq!(replay.status, 200);
    let replay: Value = serde_json::from_slice(&replay.body).unwrap();
    assert_eq!(replay["workflow"], "mission_evaluator_replay_query");
    assert_eq!(replay["retention"]["mode"], "full");
    assert_eq!(replay["replay"]["workflow"], "mission_evaluator_replay");
    assert_eq!(replay["replay"]["fixtures"], json!([]));
    assert_eq!(replay["replay"]["omitted_fixtures"], 29);
    let invalid_replay_query = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/evaluator-replay?include_fixtures=maybe",
        json!({}),
    ));
    assert_eq!(invalid_replay_query.status, 422);
    let comparison = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/evaluator-replay/compare?include_fixtures=false&max_items=32",
        json!({}),
    ));
    assert_eq!(comparison.status, 200);
    let comparison: Value = serde_json::from_slice(&comparison.body).unwrap();
    assert_eq!(comparison["workflow"], "mission_evaluator_replay_compare");
    assert_eq!(comparison["catalog_drift"]["status"], "not_recorded");
    let bundle = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evidence-bundle?include_result=false&include_trace=false&max_items=32",
            json!({}),
        ));
    assert_eq!(bundle.status, 200);
    let bundle: Value = serde_json::from_slice(&bundle.body).unwrap();
    assert_eq!(bundle["workflow"], "mission_evidence_bundle_export");
    assert_eq!(bundle["retention"]["mode"], "full");
    assert_eq!(bundle["result"], Value::Null);
    assert_eq!(bundle["trace"], json!([]));
    assert_eq!(bundle["bundle_digest"].as_str().unwrap().len(), 64);
    let verification = router.handle(request(
        "POST",
        "/v1/evidence-bundles/verify",
        json!({"bundle": bundle.clone()}),
    ));
    assert_eq!(verification.status, 200);
    let verification: Value = serde_json::from_slice(&verification.body).unwrap();
    assert_eq!(verification["workflow"], "mission_evidence_bundle_verify");
    assert_eq!(verification["valid"], true);
    let mut tampered = bundle;
    tampered["catalog_drift"]["status"] = json!("drifted");
    let tampered_response = router.handle(request(
        "POST",
        "/v1/evidence-bundles/verify",
        json!({"bundle": tampered}),
    ));
    assert_eq!(tampered_response.status, 200);
    let tampered: Value = serde_json::from_slice(&tampered_response.body).unwrap();
    assert_eq!(tampered["valid"], false);
    let invalid_bundle = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/evidence-bundle?include_result=maybe",
        json!({}),
    ));
    assert_eq!(invalid_bundle.status, 422);
    let trace = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/trace?after=0&limit=64",
        json!({}),
    ));
    assert_eq!(trace.status, 200);
    let trace: Value = serde_json::from_slice(&trace.body).unwrap();
    assert_eq!(
        trace["trace_schema_version"],
        "bioprism-devplat-mission-trace/0.1"
    );
    assert_eq!(trace["events"][0]["event"], "mission.started");
    assert_eq!(trace["events"][0]["sequence"], 0);
    assert_eq!(
        trace["events"].as_array().unwrap().last().unwrap()["event"],
        "mission.completed"
    );
    assert_eq!(trace["gap"], false);
    assert_eq!(trace["terminal"], true);
    let events = router.handle(request("GET", "/v1/events?after=0&limit=64", json!({})));
    assert_eq!(events.status, 200);
    let events: Value = serde_json::from_slice(&events.body).unwrap();
    let event_rows = events["page"]["events"].as_array().unwrap();
    assert!(!event_rows.is_empty());
    assert!(event_rows
        .iter()
        .all(|event| event["event_type"] == "mission.trace"));
    assert_eq!(
        event_rows[0]["payload"]["trace"]["event"],
        "mission.started"
    );
    let deliveries = router.handle(request(
        "GET",
        "/v1/webhooks/subscriptions/mission-events/deliveries?after=0&limit=64",
        json!({}),
    ));
    assert_eq!(deliveries.status, 200);
    let deliveries: Value = serde_json::from_slice(&deliveries.body).unwrap();
    assert_eq!(deliveries["page"]["pending_count"], event_rows.len());
    let next_after = trace["next_after"].as_u64().unwrap();
    let empty_trace = router.handle(request(
        "GET",
        &format!("/v1/missions/api-async-1/trace?after={next_after}&limit=64"),
        json!({}),
    ));
    assert_eq!(empty_trace.status, 200);
    let empty_trace: Value = serde_json::from_slice(&empty_trace.body).unwrap();
    assert_eq!(empty_trace["events"].as_array().unwrap().len(), 0);
    let invalid_trace = router.handle(request(
        "GET",
        "/v1/missions/api-async-1/trace?unexpected=value",
        json!({}),
    ));
    assert_eq!(invalid_trace.status, 400);
    let inventory = router.handle(request(
        "GET",
        "/v1/missions?status=planned&limit=1",
        json!({}),
    ));
    assert_eq!(inventory.status, 200);
    let inventory: Value = serde_json::from_slice(&inventory.body).unwrap();
    assert_eq!(inventory["returned"], 1);
    assert_eq!(inventory["total_matching"], 1);
    assert_eq!(inventory["missions"][0]["mission_id"], "api-async-1");
    assert_eq!(inventory["missions"][0]["summary"]["total_steps"], 1);
    assert_eq!(inventory["missions"][0]["progress"]["phase"], "planned");
    assert_eq!(inventory["missions"][0]["progress"]["total_steps"], 1);
    assert_eq!(
        inventory["missions"][0]["summary"]["result_available"],
        true
    );
    let invalid_query = router.handle(request("GET", "/v1/missions?unexpected=value", json!({})));
    assert_eq!(invalid_query.status, 400);
    let cancel = router.handle(request(
        "POST",
        "/v1/missions/api-async-1/cancel",
        json!({"reason": "too late"}),
    ));
    assert_eq!(cancel.status, 409);
    let deleted = router.handle(request("DELETE", "/v1/missions/api-async-1", json!({})));
    assert_eq!(deleted.status, 200);
    let missing = router.handle(request("GET", "/v1/missions/api-async-1", json!({})));
    assert_eq!(missing.status, 404);
}

#[test]
fn durable_mission_queue_checkpoint_tracks_commit_and_survives_restart() {
    let queue_path = test_state_path("mission-queue");
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_queue_state_path: Some(queue_path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let submitted = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "durable-queue-1",
                "goal": "exercise the durable mission queue",
                "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities", "arguments": {}, "depends_on": [], "bindings": [], "required": true}],
                "route_review": {
                    "ok": true,
                    "workflow": "capability_route_review",
                    "review_id": "a".repeat(64),
                    "route_id": "b".repeat(64),
                    "catalog_digest": "c".repeat(64),
                    "goal": "exercise the durable mission queue",
                    "findings": [],
                    "review_status": "ready",
                    "handoff_status": "mission_preflight_required",
                    "mission_draft": {
                        "goal": "exercise the durable mission queue",
                        "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities", "arguments": {}, "depends_on": [], "bindings": [], "required": true}],
                        "dependency_waves": [["catalog"]]
                    },
                    "execution": "not_started"
                }
            }),
        ));
    assert_eq!(submitted.status, 202);
    let mut mission = Value::Null;
    for _ in 0..100 {
        let response = router.handle(request("GET", "/v1/missions/durable-queue-1", json!({})));
        mission = serde_json::from_slice(&response.body).unwrap();
        if mission["status"] == "planned" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(mission["status"], "planned");
    assert_eq!(mission["queue"]["state"], "succeeded");
    assert_eq!(mission["queue"]["attempts"], 1);
    assert_eq!(
        mission["route_review_provenance"]["route_id"],
        "b".repeat(64)
    );

    let queue = router.handle(request("GET", "/v1/missions/queue", json!({})));
    assert_eq!(queue.status, 200);
    let queue: Value = serde_json::from_slice(&queue.body).unwrap();
    assert_eq!(queue["queue"]["enabled"], true);
    assert_eq!(queue["queue"]["file_present"], true);
    assert_eq!(queue["queue"]["integrity_verified"], true);
    assert_eq!(queue["queue"]["jobs"][0]["spec_returned"], false);
    assert_eq!(queue["queue"]["jobs"][0]["state"], "succeeded");
    assert_eq!(
        queue["queue"]["jobs"][0]["route_review_provenance"]["review_id"],
        "a".repeat(64)
    );
    assert_eq!(queue["queue"]["state_digest"].as_str().unwrap().len(), 64);
    assert_eq!(queue["queue"]["authority"]["configured"], true);
    assert_eq!(queue["queue"]["authority"]["integrity_verified"], true);
    assert!(queue["queue"]["authority"]["revision"].as_u64().unwrap() >= 1);
    assert_eq!(
        queue["queue"]["authority"]["event_count"],
        queue["queue"]["authority"]["revision"]
    );
    drop(router);

    let restored = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_queue_state_path: Some(queue_path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let restored_queue =
        restored.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
    assert_eq!(restored_queue.status, 200);
    let restored_queue: Value = serde_json::from_slice(&restored_queue.body).unwrap();
    assert_eq!(restored_queue["integrity_verified"], true);
    assert_eq!(restored_queue["jobs"][0]["state"], "succeeded");
    assert_eq!(
        restored_queue["jobs"][0]["route_review_provenance"]["catalog_digest"],
        "c".repeat(64)
    );
    assert_eq!(
        restored_queue["authority"]["queue_state_digest"],
        restored_queue["state_digest"]
    );
    let _ = std::fs::remove_file(queue_path);
}

#[test]
fn mission_queue_authority_reports_and_audits_explicit_orphan_lock_release() {
    let queue_path = test_state_path("mission-queue-lock-release");
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_queue_state_path: Some(queue_path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let lock_path = queue_path.with_file_name(format!(
        ".{}.authority-lock",
        queue_path.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&lock_path).unwrap();
    std::fs::write(
        lock_path.join("owner.json"),
        serde_json::to_vec(&json!({
            "owner_id": "dead-api-process",
            "acquired_unix_nanos": 7
        }))
        .unwrap(),
    )
    .unwrap();
    let before = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
    let before: Value = serde_json::from_slice(&before.body).unwrap();
    assert_eq!(before["authority"]["lock_present"], true);
    let release = router.handle(request(
        "POST",
        "/v1/missions/queue/authority/release-lock",
        json!({
            "operator": "on-call",
            "reason": "confirmed the previous API process exited"
        }),
    ));
    assert_eq!(release.status, 200);
    let release: Value = serde_json::from_slice(&release.body).unwrap();
    assert_eq!(
        release["receipt"]["previous_owner"]["owner_id"],
        "dead-api-process"
    );
    let after = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
    let after: Value = serde_json::from_slice(&after.body).unwrap();
    assert_eq!(after["authority"]["lock_present"], false);
    assert!(
        after["authority"]["revision"].as_u64().unwrap()
            > before["authority"]["revision"].as_u64().unwrap()
    );
    drop(router);
    let _ = std::fs::remove_file(queue_path);
    let _ = std::fs::remove_dir_all(lock_path);
}

#[test]
fn mission_queue_returns_backpressure_before_accepting_over_capacity_work() {
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            mission_queue_max_jobs: 1,
            mission_queue_max_active_leases: 1,
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let first = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "backpressure-1",
                "goal": "fill the bounded queue",
                "steps": [{"id": "step-1", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}]
            }),
        ));
    assert_eq!(first.status, 202);
    let second = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "backpressure-2",
                "goal": "must be refused before queue mutation",
                "steps": [{"id": "step-1", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}]
            }),
        ));
    assert_eq!(second.status, 429);
    let second: Value = serde_json::from_slice(&second.body).unwrap();
    assert_eq!(second["error"]["code"], "mission_queue_backpressure");
    let status = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
    let status: Value = serde_json::from_slice(&status.body).unwrap();
    assert_eq!(status["admission_policy"]["max_jobs"], 1);
    assert_eq!(status["registry_size"], 1);
}

#[test]
fn asynchronous_workflow_missions_checkpoint_reconciliation_before_terminal_state() {
    let event_path = test_state_path("async-workflow-events");
    let mission_path = test_state_path("async-workflow-missions");
    let reconciliation_path = test_state_path("async-workflow-reconciliation");
    let artifact_path = test_state_path("async-workflow-artifacts");
    let config = ApiConfig {
        event_state_path: Some(event_path.clone()),
        mission_state_path: Some(mission_path.clone()),
        reconciliation_state_path: Some(reconciliation_path.clone()),
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let instantiated = router.handle(request(
        "POST",
        "/v1/domain-workflows/instantiate",
        json!({
            "workflow_id": "biological_domains",
            "mission_id": "async-workflow-reconcile",
            "goal": "execute a reviewed biological workflow asynchronously",
            "steps": [{
                "id": "catalog",
                "domain": "biological",
                "capability": "catalogue",
                "objective": "inspect modality support",
                "tool": "modality_catalog",
                "arguments": {}
            }],
            "policy": {"execute": true}
        }),
    ));
    assert_eq!(instantiated.status, 200);
    let instantiated: Value = serde_json::from_slice(&instantiated.body).unwrap();
    let completed = json!({
        "result": {
            "isError": false,
            "content": [{"type": "text", "text": "{}"}]
        }
    });
    router.record_tool_event("async-workflow-gate-1", "modality_catalog", &completed);
    router.record_tool_event(
        "async-workflow-gate-2",
        "bioeval_reference_audit",
        &completed,
    );
    router.record_tool_event("async-workflow-gate-3", "safety_release_gate", &completed);
    let gates = router.handle(request(
        "GET",
        "/v1/operations/gates?after=0&limit=256",
        json!({}),
    ));
    assert_eq!(gates.status, 200);
    let gates: Value = serde_json::from_slice(&gates.body).unwrap();
    let review = router.handle(request(
        "POST",
        "/v1/operations/gate-reviews",
        json!({
            "gate_digest": gates["gate_digest"],
            "reviewer": "operator-async-workflow",
            "rationale": "reviewed the bounded asynchronous workflow dispatch",
            "group_ids": ["biological_domains"],
            "accepted_gates": {"biological_domains": operations_required_gates()}
        }),
    ));
    assert_eq!(review.status, 201);
    let review: Value = serde_json::from_slice(&review.body).unwrap();

    let mut mission = instantiated["mission"].clone();
    mission["operations_gate_acceptance"] = review["acceptance"].clone();
    let submitted = router.handle(request("POST", "/v1/missions", mission));
    assert_eq!(
        submitted.status,
        202,
        "{}",
        String::from_utf8_lossy(&submitted.body)
    );

    let mut terminal = Value::Null;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let response = router.handle(request(
            "GET",
            "/v1/missions/async-workflow-reconcile",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        terminal = serde_json::from_slice(&response.body).unwrap();
        if ["succeeded", "failed", "cancelled"]
            .iter()
            .any(|status| terminal["status"] == *status)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(terminal["status"], "succeeded");
    assert_eq!(terminal["result"]["mission_status"], "succeeded");
    assert_eq!(
        terminal["result"]["workflow_reconciliation"]["present"],
        true
    );
    assert_eq!(
        terminal["result"]["workflow_reconciliation"]["automatic"],
        true
    );
    assert_eq!(terminal["result"]["artifact_registry"]["indexed"], true);
    let mission_artifact_digest = terminal["result"]["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let reconciliation_digest = terminal["result"]["workflow_reconciliation"]
        ["reconciliation_digest"]
        .as_str()
        .unwrap()
        .to_owned();

    let persisted = std::fs::read_to_string(&reconciliation_path).unwrap();
    assert!(persisted.contains("async-workflow-reconcile"));
    let persisted_artifacts = std::fs::read_to_string(&artifact_path).unwrap();
    assert!(persisted_artifacts.contains(&mission_artifact_digest));
    let artifact_query = router.handle(request(
        "GET",
        "/v1/artifacts?kind=mission_report&subject_id=async-workflow-reconcile",
        json!({}),
    ));
    assert_eq!(artifact_query.status, 200);
    let artifact_query: Value = serde_json::from_slice(&artifact_query.body).unwrap();
    assert_eq!(artifact_query["rows"].as_array().unwrap().len(), 1);
    let query = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=async-workflow-reconcile&completion_status=complete",
            json!({}),
        ));
    assert_eq!(query.status, 200);
    let query: Value = serde_json::from_slice(&query.body).unwrap();
    assert_eq!(query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        query["rows"][0]["reconciliation_digest"],
        reconciliation_digest
    );

    let flush = router.handle(request("POST", "/v1/missions/persistence/flush", json!({})));
    assert_eq!(flush.status, 200);
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_status = restored.handle(request(
        "GET",
        "/v1/missions/async-workflow-reconcile",
        json!({}),
    ));
    assert_eq!(restored_status.status, 200);
    let restored_status: Value = serde_json::from_slice(&restored_status.body).unwrap();
    assert_eq!(restored_status["status"], "succeeded");
    assert_eq!(
        restored_status["result"]["workflow_reconciliation"]["reconciliation_digest"],
        reconciliation_digest
    );
    let restored_artifact = restored.handle(request(
        "GET",
        &format!("/v1/artifacts/{mission_artifact_digest}"),
        json!({}),
    ));
    assert_eq!(restored_artifact.status, 200);
    let restored_artifact: Value = serde_json::from_slice(&restored_artifact.body).unwrap();
    assert_eq!(
        restored_artifact["record"]["content_digest"],
        mission_artifact_digest
    );
    let restored_query = restored.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations?mission_id=async-workflow-reconcile",
        json!({}),
    ));
    assert_eq!(restored_query.status, 200);
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        restored_query["rows"][0]["reconciliation_digest"],
        reconciliation_digest
    );
    let _ = std::fs::remove_file(event_path);
    let _ = std::fs::remove_file(mission_path);
    let _ = std::fs::remove_file(reconciliation_path);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn mission_preflight_returns_authoritative_plan_without_queueing_or_dispatching() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let response = router.handle(request(
        "POST",
        "/v1/missions/preflight",
        json!({
            "mission_id": "api-preflight-1",
            "goal": "preview a cross-domain plan",
            "steps": [{
                "id": "catalog",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "discover routes",
                "tool": "workspace_capabilities"
            }],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
        }),
    ));
    assert_eq!(response.status, 200);
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(body["preflight"], true);
    assert_eq!(body["dispatch"], "not_started");
    assert_eq!(body["execution"], "planned");
    assert_eq!(body["results"].as_array().unwrap().len(), 0);
    assert_eq!(
        body["operations_evidence"]["decision"],
        "insufficient_evidence"
    );
    assert_eq!(body["operations_evidence"]["acceptance_required"], true);
    assert_eq!(body["operations_evidence"]["acceptance_valid"], false);
    let reviewed = router.handle(request(
        "POST",
        "/v1/missions/preflight",
        json!({
            "mission_id": "api-route-review-1",
            "goal": "preview a reviewed route",
            "steps": [{
                "id": "catalog",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "discover routes",
                "tool": "workspace_capabilities",
                "arguments": {},
                "depends_on": [],
                "bindings": [],
                "required": true
            }],
            "route_review": {
                "ok": true,
                "workflow": "capability_route_review",
                "review_id": "a".repeat(64),
                "route_id": "b".repeat(64),
                "catalog_digest": "c".repeat(64),
                "goal": "preview a reviewed route",
                "findings": [],
                "review_status": "ready",
                "handoff_status": "mission_preflight_required",
                "execution": "not_started",
                "evidence_digest": "e".repeat(64),
                "evidence_scope": "capability_route",
                "evidence_binding": {
                    "present": true,
                    "evidence_digest": "e".repeat(64),
                    "scope": "capability_route",
                    "summary": {"evidence_digest": "e".repeat(64), "scope": "capability_route"},
                    "posture": "carried_forward_not_recomputed",
                    "readiness_claimed": false,
                    "execution": "not_started"
                },
                "mission_draft": {
                    "goal": "preview a reviewed route",
                    "steps": [{
                        "id": "catalog",
                        "domain": "workspace",
                        "capability": "discovery",
                        "objective": "discover routes",
                        "tool": "workspace_capabilities",
                        "arguments": {},
                        "depends_on": [],
                        "bindings": [],
                        "required": true
                    }],
                    "dependency_waves": [["catalog"]],
                    "route_evidence_digest": "e".repeat(64),
                    "route_evidence_scope": "capability_route"
                }
            }
        }),
    ));
    assert_eq!(reviewed.status, 200);
    let reviewed_body: Value = serde_json::from_slice(&reviewed.body).unwrap();
    assert_eq!(
        reviewed_body["plan"]["route_review_provenance"]["present"],
        true
    );
    assert_eq!(
        reviewed_body["plan"]["route_review_provenance"]["evidence_present"],
        true
    );
    let missing = router.handle(request("GET", "/v1/missions/api-preflight-1", json!({})));
    assert_eq!(missing.status, 404);

    let blocked = router.handle(request(
        "POST",
        "/v1/missions",
        json!({
            "mission_id": "api-execute-without-gates",
            "goal": "must require reviewed domain evidence",
            "steps": [{
                "id": "catalog",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "discover routes",
                "tool": "workspace_capabilities"
            }],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
        }),
    ));
    assert_eq!(blocked.status, 422);
    let blocked_body: Value = serde_json::from_slice(&blocked.body).unwrap();
    assert_eq!(
        blocked_body["error"]["code"],
        "operations_gate_acceptance_required"
    );

    let refused = router.handle(request(
        "POST",
        "/v1/missions/preflight",
        json!({
            "mission_id": "api-preflight-invalid-policy",
            "goal": "must retain execution authorization checks",
            "steps": [{
                "id": "catalog",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "discover routes",
                "tool": "workspace_capabilities"
            }],
            "policy": {"execute": true}
        }),
    ));
    assert_eq!(refused.status, 422);
    let refused_body: Value = serde_json::from_slice(&refused.body).unwrap();
    assert!(refused_body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("allow-list"));
}

#[test]
fn asynchronous_mission_submission_rejects_known_tool_schema_mismatch() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let response = router.handle(request(
        "POST",
        "/v1/missions",
        json!({
            "mission_id": "api-schema-invalid",
            "goal": "refuse invalid arguments before queueing",
            "steps": [{
                "id": "compile",
                "domain": "fiber",
                "capability": "compile",
                "objective": "must be refused",
                "tool": "fiber_compile",
                "arguments": {"world": "fixture.json"}
            }],
            "policy": {"execute": true, "allowed_tools": ["fiber_compile"]}
        }),
    ));
    assert_eq!(response.status, 422);
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("authoritative schema validation refused"));
}

#[test]
fn oversized_mission_events_keep_trace_projection_when_raw_response_is_omitted() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let report = json!({
        "execution_trace_schema_version": "bioprism-devplat-mission-trace/0.1",
        "execution_trace": [{
            "sequence": 0,
            "event": "mission.completed",
            "wave": null,
            "step_id": null,
            "tool": null,
            "status": "succeeded",
            "arguments_digest": null,
            "bytes": 0,
            "detail": null
        }],
        "mission_status": "succeeded",
        "returned_bytes": 0,
        "large_result": "x".repeat(70_000)
    });
    let wire = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "isError": false,
            "content": [{"type": "text", "text": serde_json::to_string(&report).unwrap()}]
        }
    });
    router.record_tool_event("request-oversized", "agent_mission", &wire);
    let page = router.events.lock().unwrap().events(0, 10).unwrap();
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].payload["response_omitted"], true);
    assert_eq!(
        page.events[0].payload["mission_trace"]["execution_trace"][0]["event"],
        "mission.completed"
    );
}
