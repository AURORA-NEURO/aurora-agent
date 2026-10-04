//! HTTP, REST, and JSON-RPC boundary tests.

use super::*;

#[test]
fn rest_and_json_rpc_share_tool_dispatch_and_auth_is_fail_closed() {
    let router = ApiRouter::new(
        std::env::current_dir().unwrap(),
        ApiConfig {
            bearer_token: Some("0123456789abcdef".into()),
            ..ApiConfig::default()
        },
    )
    .unwrap();
    let denied = router.handle(request("GET", "/v1/tools", json!({})));
    assert_eq!(denied.status, 401);

    let mut rest = request("POST", "/v1/tools/modality_catalog", json!({}));
    rest.headers
        .insert("authorization".into(), "Bearer 0123456789abcdef".into());
    let response = router.handle(rest);
    assert_eq!(response.status, 200);
    let value: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(router.event_metrics().retained_events, 1);

    let mut rpc = request(
        "POST",
        "/v1/rpc",
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    );
    rpc.headers
        .insert("authorization".into(), "Bearer 0123456789abcdef".into());
    assert_eq!(router.handle(rpc).status, 200);
}

#[test]
fn bounded_evolution_rest_alias_returns_typed_receipt_and_preserves_boundary() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let boundary = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
    let response = router.handle(request(
        "POST",
        "/v1/research/evolution/admit",
        json!({
            "request": {
                "request_id": "evolution:api",
                "workflow_id": "workflow:high-throughput",
                "objective_id": "objective:bounded-evolution",
                "candidates": [{
                    "candidate_id": "candidate:a",
                    "artifact_digest": "a".repeat(64),
                    "baseline_digest": "b".repeat(64),
                    "required_evidence": ["c".repeat(64)],
                    "replayable": true,
                    "deterministic": true,
                    "safety_reviewed": true,
                    "policy_allow": true,
                    "resource_cost": 1,
                    "affected_surface": "adapter:research-contract",
                    "boundary": boundary
                }],
                "evidence_order": ["c".repeat(64)],
                "replay_identity": "d".repeat(64),
                "budget": 2,
                "max_concurrency": 1,
                "policy_allow": true,
                "protected_closure": true,
                "signed_approval": true,
                "raw_data_local": true,
                "boundary": boundary
            }
        }),
    ));
    assert_eq!(response.status, 200);
    let body: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(
        body["feature_id"],
        bioprism_adapter::BOUNDED_EVOLUTION_FEATURE_ID
    );
    assert_eq!(body["tool"], "adapter_bounded_evolution");
    assert_eq!(body["ok"], true);
}

#[test]
fn shared_router_handles_concurrent_requests_with_unique_request_ids() {
    let router =
        Arc::new(ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap());
    let handles = (0..32)
        .map(|_| {
            let router = Arc::clone(&router);
            std::thread::spawn(move || {
                let response = router.handle(request("GET", "/healthz", json!({})));
                assert_eq!(response.status, 200);
                response.headers.get("x-request-id").cloned()
            })
        })
        .collect::<Vec<_>>();
    let ids = handles
        .into_iter()
        .map(|handle| handle.join().unwrap().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 32);
}
