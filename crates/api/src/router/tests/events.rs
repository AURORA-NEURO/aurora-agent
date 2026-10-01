//! Event retention, trace, and webhook delivery invariants.

use super::*;

#[test]
fn durable_event_state_restores_cursor_and_requires_secret_rebind() {
    let path = test_state_path("events");
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            event_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let created = router.handle(request(
        "POST",
        "/v1/webhooks/subscriptions",
        json!({
            "id": "local",
            "endpoint": "https://example.test/hook",
            "secret": "a-secret-key"
        }),
    ));
    assert_eq!(created.status, 201);
    let call = router.handle(request("POST", "/v1/tools/modality_catalog", json!({})));
    assert_eq!(call.status, 200);
    let flush = router.handle(request("POST", "/v1/events/persistence/flush", json!({})));
    assert_eq!(flush.status, 200);
    let checkpoint = std::fs::read_to_string(&path).unwrap();
    assert!(!checkpoint.contains("a-secret-key"));
    assert!(checkpoint.contains("secrets_persisted"));
    assert!(checkpoint.contains("delivery_attempts_durable"));
    let persistence = router.handle(request("GET", "/v1/events/persistence", json!({})));
    let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
    assert_eq!(persistence["enabled"], true);
    assert_eq!(persistence["schema_version"], EVENT_STATE_SCHEMA_VERSION);
    assert_eq!(persistence["state_digest"].as_str().unwrap().len(), 64);
    assert_eq!(persistence["integrity_verified"], true);
    assert_eq!(persistence["subscriptions_durable"], true);
    assert_eq!(persistence["webhook_deliveries_durable"], true);
    assert_eq!(persistence["delivery_attempts_durable"], true);
    assert_eq!(persistence["delivery_receipt_metadata_durable"], true);
    assert_eq!(persistence["secrets_persisted"], false);
    assert_eq!(router.event_metrics().retained_events, 1);
    assert_eq!(router.event_metrics().pending_deliveries, 1);
    assert_eq!(router.event_metrics().retained_delivery_attempts, 1);

    let restored = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            event_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    assert_eq!(restored.event_metrics().retained_events, 1);
    assert_eq!(restored.event_metrics().next_event_id, 2);
    assert_eq!(restored.event_metrics().subscriptions, 1);
    assert_eq!(restored.event_metrics().active_subscriptions, 0);
    assert_eq!(restored.event_metrics().pending_deliveries, 1);
    assert_eq!(restored.event_metrics().retained_delivery_attempts, 1);
    let listed = restored.handle(request("GET", "/v1/webhooks/subscriptions", json!({})));
    let listed: Value = serde_json::from_slice(&listed.body).unwrap();
    assert_eq!(listed["subscriptions"][0]["secret_bound"], false);
    assert_eq!(listed["subscriptions"][0]["rebind_required"], true);
    let pending = restored.handle(request(
        "GET",
        "/v1/webhooks/subscriptions/local/deliveries?after=0&limit=10",
        json!({}),
    ));
    let pending: Value = serde_json::from_slice(&pending.body).unwrap();
    assert_eq!(
        pending["page"]["deliveries"][0]["state"],
        "secret_rebind_required"
    );
    let attempts = restored.handle(request(
        "GET",
        "/v1/webhooks/subscriptions/local/attempts?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(attempts.status, 200);
    let attempts: Value = serde_json::from_slice(&attempts.body).unwrap();
    assert_eq!(attempts["page"]["attempts"][0]["action"], "enqueue");
    assert_eq!(attempts["page"]["attempts"][0]["outcome"], "pending");
    let old_signature = pending["page"]["deliveries"][0]["signature"]
        .as_str()
        .unwrap()
        .to_string();
    let rebind = restored.handle(request(
        "POST",
        "/v1/webhooks/subscriptions/local/rebind",
        json!({"secret": "new-secret-key"}),
    ));
    assert_eq!(rebind.status, 200);
    let rebind: Value = serde_json::from_slice(&rebind.body).unwrap();
    assert_eq!(rebind["subscription"]["secret_bound"], true);
    assert_eq!(rebind["subscription"]["rebind_required"], false);
    assert_eq!(rebind["resigned_deliveries"], 1);
    let rebound = restored.handle(request(
        "GET",
        "/v1/webhooks/subscriptions/local/deliveries?after=0&limit=10",
        json!({}),
    ));
    let rebound: Value = serde_json::from_slice(&rebound.body).unwrap();
    assert_eq!(rebound["page"]["deliveries"][0]["state"], "pending");
    assert_ne!(rebound["page"]["deliveries"][0]["signature"], old_signature);
    let _ = std::fs::remove_file(path);
}

#[test]
fn tampered_event_checkpoint_is_rejected_before_restore() {
    let path = test_state_path("events-tampered");
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            event_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    assert_eq!(
        router
            .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
            .status,
        200
    );
    assert_eq!(
        router
            .handle(request("POST", "/v1/events/persistence/flush", json!({})))
            .status,
        200
    );
    let mut checkpoint: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    checkpoint["dropped_events"] = json!(checkpoint["dropped_events"].as_u64().unwrap() + 1);
    std::fs::write(&path, serde_json::to_vec_pretty(&checkpoint).unwrap()).unwrap();
    let observed = router.handle(request("GET", "/v1/events/persistence", json!({})));
    let observed: Value = serde_json::from_slice(&observed.body).unwrap();
    assert_eq!(observed["file_present"], true);
    assert_eq!(observed["integrity_verified"], false);

    let error = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            event_state_path: Some(path.clone()),
            ..ApiConfig::default()
        },
    )
    .err()
    .expect("tampered event checkpoint must be rejected");
    assert!(error.contains("state_digest"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn delivery_receipt_events_keep_a_bounded_join_projection_and_cursor_filter() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let subscription = router.handle(request(
        "POST",
        "/v1/webhooks/subscriptions",
        json!({
            "id": "receipt-sub",
            "endpoint": "https://example.test/hook",
            "secret": "a-secret-key"
        }),
    ));
    assert_eq!(subscription.status, 201);
    let output = json!({
        "ok": true,
        "workflow": "developer_delivery_receipt",
        "receipt_id": "receipt-api-1",
        "receipt_digest": "a".repeat(64),
        "valid": true,
        "receipt_ready": true,
        "release_candidate": true,
        "target_count": 1,
        "ready_target_count": 1,
        "ready_evidence_count": 2,
        "large_detail": "x".repeat(70_000)
    });
    let wire = json!({
        "jsonrpc": "2.0",
        "id": "req-1",
        "result": { "content": [{ "type": "text", "text": output.to_string() }] }
    });
    router.record_tool_event("req-1", "developer_delivery_receipt", &wire);

    let filtered = router.handle(request(
        "GET",
        "/v1/delivery-receipts/receipt-api-1/events?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(filtered.status, 200);
    let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
    assert_eq!(filtered["workflow"], "developer_delivery_receipt_events");
    assert_eq!(filtered["found"], true);
    assert_eq!(
        filtered["page"]["events"][0]["payload"]["delivery_receipt"]["receipt_id"],
        "receipt-api-1"
    );
    let attempts = router.handle(request(
        "GET",
        "/v1/delivery-receipts/receipt-api-1/attempts?after=0&limit=10",
        json!({}),
    ));
    assert_eq!(attempts.status, 200);
    let attempts: Value = serde_json::from_slice(&attempts.body).unwrap();
    assert_eq!(attempts["found"], true);
    assert_eq!(
        attempts["page"]["attempts"][0]["receipt_id"],
        "receipt-api-1"
    );
    assert_eq!(
        attempts["page"]["attempts"][0]["receipt_digest"],
        "a".repeat(64)
    );
    assert_eq!(
        filtered["page"]["events"][0]["payload"]["response_omitted"],
        true
    );

    let query = router.handle(request(
        "GET",
        "/v1/events?after=0&limit=10&receipt_id=receipt-api-1",
        json!({}),
    ));
    assert_eq!(query.status, 200);
    let query: Value = serde_json::from_slice(&query.body).unwrap();
    assert_eq!(query["page"]["events"].as_array().unwrap().len(), 1);

    let conflict = router.handle(request(
        "GET",
        "/v1/events?after=0&limit=10&review_id=a&receipt_id=receipt-api-1",
        json!({}),
    ));
    assert_eq!(conflict.status, 400);
}

#[test]
fn webhook_lifecycle_is_cursor_based_and_secrets_do_not_return() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let created = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({ "id": "local", "endpoint": "https://example.test/hook", "secret": "a-secret-key", "events": ["tool.completed"] }),
        ));
    assert_eq!(created.status, 201);
    assert!(!String::from_utf8(created.body.clone())
        .unwrap()
        .contains("a-secret-key"));
    let mut call = request("POST", "/v1/tools/modality_catalog", json!({}));
    let response = router.handle(call.clone());
    assert_eq!(response.status, 200);
    call.target = "/v1/webhooks/subscriptions/local/deliveries".into();
    call.method = "GET".into();
    call.body.clear();
    call.headers.remove("content-type");
    let deliveries = router.handle(call);
    assert_eq!(deliveries.status, 200);
    let value: Value = serde_json::from_slice(&deliveries.body).unwrap();
    assert_eq!(value["page"]["deliveries"].as_array().unwrap().len(), 1);
}

#[test]
fn webhook_failures_are_inspectable_and_replay_resets_the_attempt_budget() {
    struct PermanentSender;
    impl DeliverySender for PermanentSender {
        fn send(
            &mut self,
            _endpoint: &str,
            _envelope: &Value,
        ) -> Result<(), crate::events::DeliverySendError> {
            Err(crate::events::DeliverySendError::permanent(
                "operator blocked egress",
            ))
        }
    }

    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let created = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({ "id": "replayable", "endpoint": "https://example.test/hook", "secret": "a-secret-key" }),
        ));
    assert_eq!(created.status, 201);
    assert_eq!(
        router
            .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
            .status,
        200
    );
    let report = router.deliver_once(&mut PermanentSender, 10).unwrap();
    assert_eq!(report.failed, 1);
    let deliveries = router.handle(request(
        "GET",
        "/v1/webhooks/subscriptions/replayable/deliveries?after=0&limit=10",
        json!({}),
    ));
    let deliveries: Value = serde_json::from_slice(&deliveries.body).unwrap();
    assert_eq!(deliveries["page"]["deliveries"][0]["state"], "failed");
    assert_eq!(
        deliveries["page"]["deliveries"][0]["last_error_retryable"],
        false
    );
    let replay = router.handle(request(
        "POST",
        "/v1/webhooks/subscriptions/replayable/replay",
        json!({ "delivery_ids": [1] }),
    ));
    assert_eq!(replay.status, 200);
    let replay: Value = serde_json::from_slice(&replay.body).unwrap();
    assert_eq!(replay["replayed"][0]["state"], "pending");
    assert_eq!(replay["replayed"][0]["attempt"], 1);
}

#[test]
fn mission_execution_trace_survives_rest_and_event_projection() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let response = router.handle(request(
            "POST",
            "/v1/tools/agent_mission",
            json!({
                "mission_id": "api-trace-1",
                "goal": "inspect the trace contract",
                "steps": [
                    {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
                ]
            }),
        ));
    assert_eq!(response.status, 200);
    let value: Value = serde_json::from_slice(&response.body).unwrap();
    let trace: Value = serde_json::from_str(
        value["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(trace["execution_trace"][0]["event"], "mission.started");
    assert_eq!(trace["execution_trace"][1]["event"], "mission.completed");

    let events = router.handle(request("GET", "/v1/events?after=0&limit=10", json!({})));
    assert_eq!(events.status, 200);
    let page: Value = serde_json::from_slice(&events.body).unwrap();
    assert_eq!(page["page"]["events"].as_array().unwrap().len(), 1);
    let projected: Value = page["page"]["events"][0]["payload"]["response"].clone();
    assert_eq!(projected["result"]["isError"], false);
    let projected_trace: Value =
        serde_json::from_str(projected["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        projected_trace["execution_trace_schema_version"],
        "bioprism-devplat-mission-trace/0.1"
    );
}
