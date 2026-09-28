//! MCP contract tests for runtime infrastructure contracts.

use super::*;

#[test]
fn telemetry_projection_reports_loss_and_metric_observation_posture() {
    let mut server = server();
    let event = DomainEvent::new("evt-1", "job.completed", 7)
        .with_field(
            "subject",
            OpsField::new(json!("S001"), ScopeClass::Identity),
        )
        .with_field("count", OpsField::new(json!(3), ScopeClass::Unclassified));
    let policy = RedactionPolicy::new("telemetry-v1")
        .declare(
            ScopeClass::Identity,
            OpsTreatment::Coarsen {
                to: "subject".into(),
            },
        )
        .unwrap()
        .declare(ScopeClass::Unclassified, OpsTreatment::Drop)
        .unwrap();
    let spans = SignalId::parse("spans_emitted").unwrap();
    let total = SignalId::parse("operations_total").unwrap();
    let metric = MetricDefinition::new(
        "trace_coverage",
        OpsDerivation::Ratio {
            numerator: spans.clone(),
            denominator: total.clone(),
        },
        "ratio",
    )
    .unwrap();
    let observations = OpsObservations::new()
        .record(OpsSample::observed(spans, 98.0, "counter", 7))
        .record(OpsSample::observed(total, 100.0, "counter", 7));
    let payload = call(
        &mut server,
        "telemetry_project",
        json!({
            "event": serde_json::to_value(event).unwrap(),
            "policy": serde_json::to_value(policy).unwrap(),
            "trace": "trace-1",
            "metric": serde_json::to_value(metric).unwrap(),
            "observations": serde_json::to_value(observations).unwrap(),
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["schema"],
        json!("bioprism-mcp/telemetry-projection/0.1")
    );
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["loss"]["dropped"], json!(["count"]));
    assert_eq!(payload["metric"]["ok"], json!(true));
    assert_eq!(payload["metric"]["value"]["value"], json!(0.98));
    assert_eq!(payload["record"]["event_id"], json!("evt-1"));
}

#[test]
fn factory_lifecycle_replays_safe_retry_quarantine_compensation_and_commit_boundaries() {
    let mut server = server();
    let jobs = [
        Job::new(
            "a-idempotent",
            ResourceClass::Compile,
            Idempotency::Idempotent,
            json!({ "kind": "pure-build" }),
        ),
        Job::new(
            "b-nonidempotent",
            ResourceClass::Compile,
            Idempotency::NonIdempotent,
            json!({ "kind": "external-effect" }),
        ),
        Job::new(
            "c-compensable",
            ResourceClass::Compile,
            Idempotency::Compensable,
            json!({ "kind": "reversible-effect" }),
        ),
    ];
    let worker = WorkerCapability::new("worker-1", vec![ResourceClass::Compile])
        .with_lease_duration_nanos(30_000_000_000);
    let payload = call(
        &mut server,
        "factory_lifecycle_simulate",
        json!({
            "jobs": jobs.iter().map(serde_json::to_value).collect::<Result<Vec<_>, _>>().unwrap(),
            "workers": [serde_json::to_value(worker).unwrap()],
            "actions": [
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 0 },
                { "kind": "recover_expired", "now_nanos": 30_000_000_000i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 31_000_000_000i64 },
                { "kind": "stage", "job_id": "a-idempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 31_000_000_001i64, "output": { "digest": "a-out" } },
                { "kind": "commit", "job_id": "a-idempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 31_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 31_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 61_000_000_003i64 },
                { "kind": "release_quarantine", "job_id": "b-nonidempotent", "operator": "reviewer-1" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 62_000_000_000i64 },
                { "kind": "fail", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 2, "now_nanos": 62_000_000_001i64, "reason": "external service rejected the request" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_000i64 },
                { "kind": "stage", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 3, "now_nanos": 63_000_000_001i64, "output": { "effect_id": "effect-1" } },
                { "kind": "commit", "job_id": "b-nonidempotent", "worker_id": "worker-1", "attempt": 3, "now_nanos": 63_000_000_002i64 },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 63_000_000_003i64 },
                { "kind": "recover_expired", "now_nanos": 93_000_000_003i64 },
                { "kind": "compensate", "job_id": "c-compensable" },
                { "kind": "lease", "worker_id": "worker-1", "now_nanos": 94_000_000_000i64 },
                { "kind": "stage", "job_id": "c-compensable", "worker_id": "worker-1", "attempt": 2, "now_nanos": 94_000_000_001i64, "output": { "compensated": true } },
                { "kind": "commit", "job_id": "c-compensable", "worker_id": "worker-1", "attempt": 2, "now_nanos": 94_000_000_002i64 }
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["action_count"], json!(19));
    assert_eq!(payload["action_failures"], json!(0));
    assert_eq!(payload["quarantined"], json!([]));
    assert_eq!(payload["dead_lettered"], json!([]));
    assert_eq!(
        payload["trace"][1]["result"][0]["outcome"],
        json!("requeued")
    );
    assert_eq!(
        payload["trace"][6]["result"][0]["outcome"],
        json!("quarantined")
    );
    assert_eq!(
        payload["trace"][14]["result"][0]["outcome"],
        json!("awaiting_compensation")
    );

    let jobs = payload["jobs"].as_array().unwrap();
    for id in ["a-idempotent", "b-nonidempotent", "c-compensable"] {
        let job = jobs
            .iter()
            .find(|row| row["id"] == json!(id))
            .expect("simulated job is included in final snapshot");
        assert_eq!(job["job"]["state"], json!("succeeded"));
        assert!(job["committed_result"].is_object());
    }
    assert!(
        payload["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("idempotency"))
    );
}

#[test]
fn factory_authority_verify_audits_legacy_queue_checkpoint_without_dispatch() {
    let mut server = server();
    let checkpoint = JobStore::new().snapshot().unwrap();
    let payload = call(
        &mut server,
        "factory_authority_verify",
        json!({
            "checkpoint": serde_json::to_value(checkpoint).unwrap(),
            "include_events": true,
            "max_events": 8,
        }),
    );

    assert_eq!(payload["valid"], json!(true));
    assert_eq!(payload["revision"], json!(0));
    assert_eq!(payload["authority_epoch"], json!(1));
    assert_eq!(payload["event_count"], json!(0));
    assert_eq!(payload["events"], json!([]));
    assert_eq!(payload["job_count"], json!(0));
    assert_eq!(payload["active_lease_count"], json!(0));
    assert!(
        payload["does_not_claim"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "multi-host consensus or network-partition tolerance")
    );
}

#[test]
fn control_plane_readiness_joins_components_without_widening_authority() {
    let mut server = server();
    let domain_report = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "control-plane-subject",
            "source_tool": "modality_catalog",
            "report": {"observations": ["retained"]},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    let report = domain_report["report"].clone();
    let report_digest = ContentHash::of_value(&report).unwrap().to_string();
    let readiness = call(
        &mut server,
        "domain_decision_readiness_audit",
        json!({
            "subject_id": "control-plane-subject",
            "claim": {"id": "control-plane-claim", "statement": "opaque"},
            "reports": [report],
            "links": [{"report_index": 0, "report_digest": report_digest, "role": "supports"}],
            "policy": {
                "required_group_ids": ["biological_domains"],
                "required_domains": ["modalities"],
                "minimum_supporting_reports": 1,
                "allow_review_required": true
            }
        }),
    );
    let control = call(
        &mut server,
        "control_plane_readiness_audit",
        json!({
            "subject_id": "control-plane-subject",
            "policy": {"require_route_review": true},
            "readiness_audit": readiness,
            "route_review": {
                "ok": true,
                "workflow": "capability_route_review",
                "route_id": "a".repeat(64),
                "review_id": "b".repeat(64),
                "catalog_digest": "c".repeat(64),
                "review_status": "ready",
                "findings": [],
                "execution": "not_started"
            }
        }),
    );
    assert_eq!(control["workflow"], json!("control_plane_readiness_audit"));
    assert_eq!(
        control["audit"]["component_states"]["capability_route"]["satisfied"],
        json!(true)
    );
    assert_eq!(control["audit"]["control_plane_state"], json!("incomplete"));
    assert_eq!(control["audit"]["policy_satisfied"], json!(false));
    assert_eq!(control["readiness_claimed"], json!(false));
    assert_eq!(control["execution"], json!("not_started"));
    assert_eq!(control["artifact_registry"]["indexed"], json!(true));

    let query = call(
        &mut server,
        "control_plane_readiness_query",
        json!({
            "subject_id": "control-plane-subject",
            "control_plane_state": "incomplete",
            "include_audits": true
        }),
    );
    assert_eq!(
        query["workflow"],
        json!("artifact_registry_control_plane_readiness_query")
    );
    assert_eq!(query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        query["rows"][0]["audit"]["subject_id"],
        json!("control-plane-subject")
    );
}

#[test]
fn control_plane_readiness_compare_reports_structural_regression_and_recovery() {
    let mut server = server();
    let domain_report = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "control-plane-compare-subject",
            "source_tool": "modality_catalog",
            "report": {"observations": ["retained"]},
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    );
    let report = domain_report["report"].clone();
    let report_digest = ContentHash::of_value(&report).unwrap().to_string();
    let readiness = call(
        &mut server,
        "domain_decision_readiness_audit",
        json!({
            "subject_id": "control-plane-compare-subject",
            "claim": {"id": "control-plane-compare-claim"},
            "reports": [report],
            "links": [{"report_index": 0, "report_digest": report_digest, "role": "supports"}],
            "policy": {
                "required_group_ids": ["biological_domains"],
                "required_domains": ["modalities"],
                "minimum_supporting_reports": 1
            }
        }),
    );
    let before = call(
        &mut server,
        "control_plane_readiness_audit",
        json!({
            "subject_id": "control-plane-compare-subject",
            "policy": {"require_route_review": true},
            "readiness_audit": readiness.clone()
        }),
    );
    let after = call(
        &mut server,
        "control_plane_readiness_audit",
        json!({
            "subject_id": "control-plane-compare-subject",
            "policy": {"require_route_review": true},
            "readiness_audit": readiness,
            "route_review": {
                "ok": true,
                "workflow": "capability_route_review",
                "route_id": "a".repeat(64),
                "review_id": "b".repeat(64),
                "catalog_digest": "c".repeat(64),
                "review_status": "ready",
                "findings": [],
                "execution": "not_started"
            }
        }),
    );
    assert_eq!(before["audit"]["subject_id"], after["audit"]["subject_id"]);
    let before_content_digest = before["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_string();
    let after_content_digest = after["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_string();
    let comparison = call(
        &mut server,
        "control_plane_readiness_compare",
        json!({"before": before, "after": after}),
    );
    assert_eq!(
        comparison["workflow"],
        json!("control_plane_readiness_compare")
    );
    assert_eq!(
        comparison["comparison"]["state_direction"],
        json!("improved")
    );
    assert_eq!(
        comparison["comparison"]["evidence_direction"],
        json!("improved")
    );
    assert!(
        !comparison["comparison"]["comparison_digest"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert_eq!(comparison["readiness_claimed"], json!(false));
    assert_eq!(comparison["execution"], json!("not_started"));

    let retained_comparison = call(
        &mut server,
        "control_plane_readiness_compare_retained",
        json!({
            "before_content_digest": before_content_digest,
            "after_content_digest": after_content_digest,
            "subject_id": "control-plane-compare-subject"
        }),
    );
    assert_eq!(
        retained_comparison["workflow"],
        json!("control_plane_readiness_compare_retained")
    );
    assert_eq!(
        retained_comparison["comparison"]["state_direction"],
        json!("improved")
    );
    assert_eq!(
        retained_comparison["source"],
        json!("content_addressed_artifact_registry")
    );
    assert_eq!(retained_comparison["readiness_claimed"], json!(false));
    assert_eq!(retained_comparison["execution"], json!("not_started"));
}

#[test]
fn cache_invalidation_simulation_keeps_partial_unknowns_and_replayable_misses_visible() {
    let mut server = server();
    let arguments = json!({
        "schema": {
            "name": "decision-cache",
            "components": ["input", "code"],
            "reuse": "same_build_only"
        },
        "entries": [
            {
                "components": { "input": "world@1", "code": "build-a" },
                "value": { "answer": "derived" },
                "produced_by": "build-a",
                "written_at": 1,
                "dependencies": { "kind": "declared", "resources": ["derived"] }
            },
            {
                "components": { "input": "world@2", "code": "build-a" },
                "value": { "answer": "legacy" },
                "produced_by": "build-a",
                "written_at": 1,
                "dependencies": "undeclared"
            }
        ],
        "graph": {
            "declared": [{ "resource": "derived", "depends_on": ["input"] }],
            "opaque": ["input"]
        },
        "changed": "input",
        "apply": true,
        "apply_at": 2,
        "lookups": [
            { "components": { "input": "world@2", "code": "build-a" }, "requested_by": "build-a" }
        ]
    });
    let payload = call(
        &mut server,
        "cache_invalidation_simulate",
        arguments.clone(),
    );
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["invalidation"]["plan"]["completeness"]["Partial"].is_object());
    assert_eq!(
        payload["invalidation"]["apply_report"]["invalidation_was_complete"],
        json!(false)
    );
    assert_eq!(
        payload["invalidation"]["apply_report"]["removed"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        payload["invalidation"]["apply_report"]["marked_unproven"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        payload["graph"]["opaque_resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "input")
    );
    assert_eq!(payload["lookups"]["pre_apply"][0]["hit"], json!(true));
    assert_eq!(payload["lookups"]["post_apply"][0]["hit"], json!(false));
    assert_eq!(payload["cache"]["unproven"].as_array().unwrap().len(), 1);

    let digest = payload["cache"]["unproven"][0]
        .as_str()
        .expect("unproven entry has a digest")
        .to_string();
    let resumed = call(
        &mut server,
        "cache_invalidation_simulate",
        json!({
            "schema": arguments["schema"].clone(),
            "entries": [arguments["entries"][1].clone()],
            "graph": { "declared": [], "opaque": [] },
            "reprove": [{ "digest": digest, "by": "build-b" }]
        }),
    );
    assert_eq!(resumed["reprove"][0]["ok"], json!(true));
    assert_eq!(resumed["cache"]["unproven"].as_array().unwrap().len(), 0);
}

#[test]
fn storage_lifecycle_simulation_plans_pins_reserve_and_non_copyable_allowance() {
    let mut server = server();
    let payload = call(
        &mut server,
        "storage_lifecycle_simulate",
        json!({
            "now": 20,
            "tiering_policy": {
                "demote_to_warm_after": 5,
                "demote_to_cold_after": 12,
                "promote_after_accesses": 3,
                "promote_within": 2
            },
            "records": [
                { "object": "stale-hot", "tier": "hot", "last_access": 0, "bytes": 100 },
                { "object": "pinned-hot", "tier": "hot", "last_access": 0, "bytes": 200, "pinned": true },
                { "object": "recent-cold", "tier": "cold", "last_access": 19, "recent_accesses": 3, "bytes": 50 }
            ],
            "apply_tiering": true,
            "quota": { "limit": 1000, "reserve": 100 },
            "charges": [
                { "class": "objects", "purpose": "ingest", "bytes": 850 },
                { "class": "events", "purpose": "ingest", "bytes": 100 },
                { "class": "events", "purpose": "cleanup", "bytes": 100 }
            ],
            "releases": [{ "class": "objects", "bytes": 50 }],
            "delegations": [{
                "bytes": 50,
                "charges": [{ "class": "cache", "purpose": "cleanup", "bytes": 30 }]
            }],
            "absorb_delegated": [0]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["tiering"]["transition_count"], json!(3));
    assert_eq!(payload["tiering"]["apply_report"]["applied"], json!(3));
    let records = payload["tiering"]["records"].as_array().unwrap();
    assert_eq!(records[1]["tier"], json!("Warm"));
    assert_eq!(records[2]["tier"], json!("Hot"));
    assert!(
        payload["quota"]["charges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["ok"] == json!(false) && row["fail_closed"] == json!(true))
    );
    assert_eq!(payload["quota"]["remaining"], json!(70));
    assert_eq!(payload["quota"]["reserve"], json!(100));
    assert_eq!(payload["quota"]["absorptions"][0]["ok"], json!(true));
    assert!(
        payload["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("allowance is not copied"))
    );
}

#[test]
fn operations_catalog_executes_topology_parity_and_keeps_metric_debt_explicit() {
    let mut server = server();
    let payload = call(&mut server, "operations_catalog", json!({ "max_items": 2 }));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["topologies"]["promise_parity"]["holds"],
        json!(true)
    );
    assert_eq!(payload["data_classes"].as_array().unwrap().len(), 5);
    assert_eq!(payload["deployment_planes"].as_array().unwrap().len(), 9);
    assert_eq!(payload["service_contracts"]["summary"]["total"], json!(9));
    assert!(payload["metrics"]["named_in_scope"].as_u64().unwrap() >= 100);
    assert!(payload["metrics"]["named_but_undefined"].as_u64().unwrap() >= 100);
    assert_eq!(
        payload["metrics"]["undefined_metrics_returned"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        payload["metrics"]["omitted_undefined_metrics"]
            .as_u64()
            .unwrap()
            > 0
    );

    let detailed = call(
        &mut server,
        "operations_catalog",
        json!({ "include_details": true, "max_items": 1 }),
    );
    assert_eq!(detailed["detail_mode"], json!("full"));
    assert!(detailed["details"]["service_entries"].is_array());
    assert!(
        detailed["details"]["undefined_metrics"]
            .as_array()
            .unwrap()
            .len()
            >= 100
    );
}

#[test]
fn trace_analysis_ingests_segments_and_localizes_lossless_divergence() {
    let mut server = server();
    let failing = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"}}
{"step":1,"kind":"observation","payload":{"summary":"evidence"},"visible":["evidence-a"]}
{"step":2,"kind":"choice","payload":{"summary":"choose","chosen":"a","alternatives":["a","b"]},"visible":["evidence-a"]}
{"step":3,"kind":"action","payload":{"tool":"search"},"caused_by":2}
{"step":4,"kind":"result","payload":{"summary":"done"}}
{"step":5,"kind":"termination","payload":{"summary":"finished"}}
"#;
    let passing = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"}}
{"step":1,"kind":"observation","payload":{"summary":"evidence"},"visible":["evidence-a"]}
{"step":2,"kind":"choice","payload":{"summary":"choose","chosen":"b","alternatives":["a","b"]},"visible":["evidence-a","evidence-b"]}
{"step":3,"kind":"action","payload":{"tool":"search-alt"},"caused_by":2}
{"step":4,"kind":"result","payload":{"summary":"done"}}
{"step":5,"kind":"termination","payload":{"summary":"finished"}}
"#;

    let payload = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "failed-run",
            "jsonl": failing,
            "succeeded": false,
            "passing_trace_id": "passing-run",
            "passing_jsonl": passing,
            "passing_succeeded": true,
            "max_items": 10
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["valid"], json!(true));
    assert_eq!(payload["lossless"], json!(true));
    assert_eq!(payload["compilable"], json!(true));
    assert!(payload["trace_sha256"].as_str().unwrap().len() >= 32);
    assert!(payload["candidate_count"].as_u64().unwrap() >= 2);
    assert!(payload["excluded_count"].as_u64().unwrap() >= 4);
    assert!(payload["review_reduction"].as_f64().unwrap() > 0.0);
    assert_eq!(payload["divergence"]["kind"], json!("diverged"));
    assert_eq!(payload["divergence"]["failing_step"], json!(2));
    assert_eq!(payload["divergence_actionable"], json!(true));
    assert!(
        payload["divergence"]["visibility_gap"]
            .as_array()
            .unwrap()
            .contains(&json!("evidence-b"))
    );
    assert!(
        payload["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate["score"]["is_divergence"] == json!(true))
    );
    assert_eq!(
        payload["proposals"].as_array().unwrap().len(),
        payload["candidates"].as_array().unwrap().len()
    );
    assert_eq!(payload["approval_required"], json!(true));
}

#[test]
fn trace_analysis_preserves_import_loss_and_fails_closed_on_unsafe_inputs() {
    let mut server = server();
    let lossy = r#"
{"step":0,"kind":"goal","payload":{"summary":"solve"},"unknown":"retained-as-loss"}
not-json
{"step":2,"kind":"mystery","payload":{"summary":"cannot type"}}
{"step":3,"kind":"choice","payload":{"summary":"choose","alternatives":["a","b"]}}
"#;
    let payload = call(
        &mut server,
        "trace_analyze",
        json!({ "trace_id": "lossy", "jsonl": lossy }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["dropped_events"], json!(2));
    assert_eq!(payload["compilable"], json!(false));
    assert!(payload["loss"]["unparsed_lines"].as_array().unwrap().len() == 1);
    assert!(payload["loss"]["untyped_events"].as_array().unwrap().len() == 1);
    assert_eq!(payload["divergence"], Value::Null);

    let duplicate = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "duplicate",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}\n{\"step\":0,\"kind\":\"choice\",\"payload\":{}}"
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(
        duplicate["error"]
            .as_str()
            .unwrap()
            .contains("more than once")
    );

    let conflict = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "conflict",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}",
            "document": "fixtures/fiber-v0.1/radiogenomic_world.json"
        }),
    );
    assert_eq!(conflict["__isError"], json!(true));
    assert!(
        conflict["error"]
            .as_str()
            .unwrap()
            .contains("either jsonl or document")
    );

    let bounded = call(
        &mut server,
        "trace_analyze",
        json!({
            "trace_id": "bounded",
            "jsonl": "{\"step\":0,\"kind\":\"goal\",\"payload\":{}}",
            "max_bytes": 0
        }),
    );
    assert_eq!(bounded["__isError"], json!(true));
    assert!(bounded["error"].as_str().unwrap().contains("max_bytes"));
}

#[test]
fn trace_otel_ingest_maps_spans_and_reports_compilation_readiness() {
    let mut server = server();
    let otlp = json!({
        "resourceSpans": [{
            "resource": {"attributes": [
                {"key": "service.name", "value": {"stringValue": "agent"}}
            ]},
            "scopeSpans": [{
                "scope": {"name": "fixture", "version": "1"},
                "spans": [
                    {
                        "traceId": "trace-a",
                        "spanId": "root",
                        "name": "agent.goal",
                        "startTimeUnixNano": "10",
                        "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "goal"}}]
                    },
                    {
                        "traceId": "trace-a",
                        "spanId": "child",
                        "parentSpanId": "root",
                        "name": "agent.tool.call",
                        "startTimeUnixNano": "20",
                        "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "action"}}],
                        "events": [{
                            "name": "tool.input",
                            "timeUnixNano": "21",
                            "attributes": [{"key": "arg.count", "value": {"intValue": "2"}}]
                        }]
                    }
                ]
            }]
        }]
    });
    let payload = call(
        &mut server,
        "trace_otel_ingest",
        json!({
            "trace_id": "otel-run",
            "otlp_json": otlp.to_string(),
            "include_events": true,
            "succeeded": false
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(
        payload["schema"],
        json!("bioprism-mcp/trace-otel-ingest/0.1")
    );
    assert_eq!(payload["mapping"]["format"], json!("otlp_json"));
    assert_eq!(payload["mapping"]["accepted_span_count"], json!(2));
    assert_eq!(payload["mapping"]["span_event_count"], json!(1));
    assert_eq!(payload["lossless"], json!(true));
    assert_eq!(payload["compilable"], json!(true));
    assert_eq!(payload["events"][1]["caused_by"], json!(0));
    assert_eq!(payload["events"][1]["kind"], json!("action"));
    assert_eq!(
        payload["events"][1]["payload"]["events"][0]["name"],
        json!("tool.input")
    );
}

#[test]
fn trace_otel_ingest_keeps_ambiguous_exports_non_compilable_and_bounded() {
    let mut server = server();
    let otlp = json!({
        "resourceSpans": [{
            "scopeSpans": [{
                "spans": [{
                    "traceId": "trace-a",
                    "spanId": "span-a",
                    "name": "tool.call"
                }]
            }]
        }]
    })
    .to_string();
    let payload = call(
        &mut server,
        "trace_otel_ingest",
        json!({"trace_id": "ambiguous", "otlp_json": otlp}),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["lossless"], json!(false));
    assert_eq!(payload["compilable"], json!(false));
    assert_eq!(payload["events_included"], json!(false));

    let bounded = call(
        &mut server,
        "trace_otel_ingest",
        json!({
            "trace_id": "bounded",
            "otlp_json": otlp,
            "max_spans": 0
        }),
    );
    assert_eq!(bounded["__isError"], json!(true));
    assert!(bounded["error"].as_str().unwrap().contains("max_spans"));
}

#[test]
fn weave_protocol_catalog_exposes_typed_antecedents() {
    let mut server = server();
    let payload = call(&mut server, "weave_protocol_catalog", json!({}));
    assert_eq!(payload["ok"], json!(true));
    let acts = payload["acts"].as_array().unwrap();
    let accept = acts
        .iter()
        .find(|act| act["kind"] == json!("accept"))
        .expect("accept act");
    assert_eq!(accept["requires_antecedent"], json!(["propose"]));
}

#[test]
fn runtime_effect_check_is_deny_by_default_and_never_executes() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileRead])
        .allowing_path("/work/");
    let allowed = call(
        &mut server(),
        "runtime_effect_check",
        json!({
            "policy": serde_json::to_value(&policy).unwrap(),
            "request": EffectRequest::FileRead { path: "/work/data.txt".into() }
        }),
    );
    assert_eq!(allowed["ok"], json!(true));
    assert_eq!(allowed["authorization"], json!("perform"));

    let denied = call(
        &mut server(),
        "runtime_effect_check",
        json!({
            "policy": serde_json::to_value(policy).unwrap(),
            "request": EffectRequest::FileRead { path: "/etc/passwd".into() }
        }),
    );
    assert_eq!(denied["ok"], json!(false));
    assert_eq!(denied["fail_closed"], json!(true));
    assert!(denied["refusal"].as_str().unwrap().contains("path"));
}

#[test]
fn runtime_tape_verify_accepts_only_verified_chain_state() {
    let tape = WorldTape::new(RunId::parse("run-mcp").unwrap());
    let result = call(
        &mut server(),
        "runtime_tape_verify",
        json!({ "tape": serde_json::to_value(tape).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-tape-verify/0.1")
    );
    assert_eq!(result["chain_verified"], json!(true));
    assert_eq!(result["entries"], json!(0));
    assert_eq!(result["checkpoint_count"], json!(0));
    assert_eq!(result["artifact_consumed_count"], json!(0));
    assert_eq!(result["artifact_created_count"], json!(0));
    assert_eq!(result["simulated_steps"].as_array().unwrap().len(), 0);
    assert_eq!(result["simulated_step_count"], json!(0));
}

#[test]
fn runtime_execution_simulate_records_replays_and_forks_without_live_effects() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::ClockNow,
            EffectKind::RandomBytes,
            EffectKind::FileRead,
            EffectKind::FileWrite,
        ])
        .allowing_path("/work/");
    let budget = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(4));
    let result = call(
        &mut server(),
        "runtime_execution_simulate",
        json!({
            "run": "run-execution-mcp",
            "policy": serde_json::to_value(policy).unwrap(),
            "requests": [
                { "kind": "clock_now" },
                { "kind": "file_read", "path": "/work/input.txt" },
                { "kind": "random_bytes", "count": 4 },
                { "kind": "file_write", "path": "/work/output.txt", "content": "done" }
            ],
            "world": {
                "seed": 7,
                "clock_start": 10,
                "clock_tick": 2,
                "base_files": { "/work/input.txt": "fixture" }
            },
            "budget": serde_json::to_value(budget).unwrap(),
            "fork": {
                "step": 2,
                "run": "run-execution-child",
                "requests": [{ "kind": "clock_now" }]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-execution-simulate/0.1")
    );
    assert_eq!(result["recorded_requests"], json!(4));
    assert_eq!(result["recording_complete"], json!(true));
    assert_eq!(result["partial_recording"], json!(false));
    assert_eq!(result["live_outcome_count"], json!(4));
    assert!(result["policy_journal_count"].as_u64().unwrap() >= 4);
    assert_eq!(result["replay"]["verified"], json!(true));
    assert_eq!(result["replay"]["matched"], json!(true));
    assert_eq!(result["replay_complete"], json!(true));
    assert_eq!(result["world"]["calls"], json!(4));
    assert_eq!(
        result["world"]["file_changes"][0]["path"],
        json!("/work/output.txt")
    );
    assert_eq!(result["fork"]["ok"], json!(true));
    assert_eq!(result["fork"]["inherited_steps"], json!(2));
    assert_eq!(result["fork"]["observed_state"]["fork_step"], json!(2));
    assert!(result["fork"]["child_tape"]["lineage"].is_object());
}

#[test]
fn runtime_execution_simulate_reports_budget_exhaustion_and_keeps_partial_replay_explicit() {
    let policy = EffectPolicy::evaluation_default().declaring([EffectKind::ClockNow]);
    let budget = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(1));
    let result = call(
        &mut server(),
        "runtime_execution_simulate",
        json!({
            "policy": serde_json::to_value(policy).unwrap(),
            "requests": [{ "kind": "clock_now" }, { "kind": "clock_now" }],
            "budget": serde_json::to_value(budget).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/runtime-execution-simulate/0.1")
    );
    assert!(
        result["execution_error"]
            .as_str()
            .unwrap()
            .contains("budget exhausted")
    );
    assert_eq!(result["recorded_requests"], json!(1));
    assert_eq!(result["recording_complete"], json!(false));
    assert_eq!(result["partial_recording"], json!(true));
    assert_eq!(result["replay"]["verified"], json!(true));
    assert_eq!(result["budget"]["aborted_on"], json!("tool_calls"));
}

#[test]
fn interweave_workflow_catalogue_derives_owed_deliverables_without_fabricating_artifacts() {
    let result = call(&mut server(), "interweave_workflow_catalogue", json!({}));
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["workflow_count"], json!(6));
    assert_eq!(result["deliverables_per_workflow"], json!(9));
    assert_eq!(result["workflows"].as_array().unwrap().len(), 6);
    assert_eq!(
        result["outstanding_deliverables"]
            .as_object()
            .unwrap()
            .len(),
        6
    );
    assert!(
        result["outstanding_deliverables"]
            .as_object()
            .unwrap()
            .values()
            .all(|count| count == 9)
    );
    assert!(
        result["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| workflow["present"].as_array().unwrap().is_empty())
    );
}

#[test]
fn epistemic_voi_keeps_gross_cost_net_and_action_change_separate() {
    let result = call(
        &mut server(),
        "epistemic_voi",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisition": {
                "id": "assay",
                "cost": 0.1,
                "outcomes": [
                    { "label": "positive", "likelihood": [0.9, 0.1] },
                    { "label": "negative", "likelihood": [0.1, 0.9] }
                ]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["mode"], json!("single"));
    assert!(result["value"]["gross"].as_f64().unwrap() > 0.0);
    assert_eq!(result["value"]["cost"], json!(0.1));
    assert!(result["value"]["net"].as_f64().unwrap() > 0.0);
    assert_eq!(result["actions"]["without"], json!("treat"));
    assert_eq!(result["value"]["action_after"], json!([0, 1]));
    assert_eq!(result["value"]["action_without"], json!(0));

    let invalid = call(
        &mut server(),
        "epistemic_voi",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisition": {
                "id": "broken",
                "cost": 0.1,
                "outcomes": [
                    { "label": "only", "likelihood": [0.2, 0.2] }
                ]
            }
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(
        invalid["error"]
            .as_str()
            .unwrap()
            .contains("invariant failed")
    );
}

#[test]
fn epistemic_adaptive_acquisition_projects_branch_dependent_named_policy() {
    let result = call(
        &mut server(),
        "epistemic_adaptive_acquisition",
        json!({
            "problem": {
                "actions": ["choose-m0", "choose-m1"],
                "models": ["m0", "m1"],
                "loss": [0.0, 1.0, 1.0, 0.0]
            },
            "belief": { "mass": [0.9, 0.1] },
            "acquisitions": [
                {
                    "id": "screen",
                    "cost": 0.01,
                    "outcomes": [
                        { "label": "positive", "likelihood": [0.9, 0.2] },
                        { "label": "negative", "likelihood": [0.1, 0.8] }
                    ]
                },
                {
                    "id": "confirm",
                    "cost": 0.1,
                    "outcomes": [
                        { "label": "positive", "likelihood": [0.01, 0.99] },
                        { "label": "negative", "likelihood": [0.99, 0.01] }
                    ]
                }
            ],
            "budget": 0.11,
            "max_steps": 2
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-adaptive-acquisition/0.1")
    );
    assert_eq!(result["policy"]["root"]["kind"], json!("acquire"));
    assert_eq!(result["policy"]["root"]["id"], json!("screen"));
    assert!(result["policy"]["expected_total"].as_f64().unwrap() < 0.1);
    assert!(
        result["policy"]["root"]["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|outcome| outcome["next"]["kind"] == json!("acquire")
                && outcome["next"]["id"] == json!("confirm"))
    );
    assert!(
        result["policy"]["root"]["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|outcome| outcome["next"]["kind"] == json!("stop"))
    );
    assert_eq!(
        result["policy"]["root"]["outcomes"][0]["posterior"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let refused = call(
        &mut server(),
        "epistemic_adaptive_acquisition",
        json!({
            "problem": {
                "actions": ["choose-m0", "choose-m1"],
                "models": ["m0", "m1"],
                "loss": [0.0, 1.0, 1.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisitions": [{
                "id": "too-many-steps",
                "cost": 0.1,
                "outcomes": [
                    { "label": "yes", "likelihood": [0.9, 0.1] },
                    { "label": "no", "likelihood": [0.1, 0.9] }
                ]
            }],
            "budget": 1.0,
            "max_steps": 17
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("max_steps"));
}

#[test]
fn epistemic_adaptive_execute_requires_authorization_and_replays_simulated_prefixes() {
    let base = json!({
        "problem": {
            "actions": ["choose-m0", "choose-m1"],
            "models": ["m0", "m1"],
            "loss": [0.0, 1.0, 1.0, 0.0]
        },
        "belief": { "mass": [0.9, 0.1] },
        "acquisitions": [
            {
                "id": "screen",
                "cost": 0.01,
                "outcomes": [
                    { "label": "positive", "likelihood": [0.9, 0.2] },
                    { "label": "negative", "likelihood": [0.1, 0.8] }
                ]
            },
            {
                "id": "confirm",
                "cost": 0.1,
                "outcomes": [
                    { "label": "positive", "likelihood": [0.01, 0.99] },
                    { "label": "negative", "likelihood": [0.99, 0.01] }
                ]
            }
        ],
        "budget": 0.11,
        "max_steps": 2
    });

    let denied = call(&mut server(), "epistemic_adaptive_execute", base.clone());
    assert_eq!(denied["ok"], json!(true));
    assert_eq!(denied["completed"], json!(false));
    assert_eq!(denied["receipt"]["status"], json!("refused"));
    assert_eq!(denied["receipt"]["observations"], json!([]));
    assert_eq!(
        denied["receipt"]["refusal"],
        json!("authorization_required")
    );

    let mut authorized = base;
    authorized["provider"] = json!("mcp-simulated");
    authorized["authorization"] = json!({ "grant_id": "grant-1", "provider": "mcp-simulated" });
    authorized["observations"] = json!([
        { "acquisition_id": "screen", "outcome_label": "negative" },
        { "acquisition_id": "confirm", "outcome_label": "negative" }
    ]);
    let simulated = call(
        &mut server(),
        "epistemic_adaptive_execute",
        authorized.clone(),
    );
    assert_eq!(simulated["completed"], json!(true));
    assert_eq!(simulated["receipt"]["status"], json!("completed"));
    assert_eq!(simulated["provenance_counts"]["simulated"], json!(2));

    let replay = call(
        &mut server(),
        "epistemic_adaptive_execute",
        json!({
            "mode": "replay",
            "problem": authorized["problem"].clone(),
            "belief": authorized["belief"].clone(),
            "acquisitions": authorized["acquisitions"].clone(),
            "budget": 0.11,
            "max_steps": 2,
            "receipt": simulated["receipt"].clone()
        }),
    );
    assert_eq!(replay["completed"], json!(true));
    assert_eq!(replay["receipt"]["status"], json!("completed"));
    assert_eq!(replay["provenance_counts"]["replayed"], json!(2));
    assert_eq!(replay["provenance_counts"]["simulated"], json!(0));
}

#[test]
fn interweave_workflow_execute_binds_all_reference_workflows_and_replays_receipts() {
    let workflows = [
        "reliable_software_repair",
        "scientific_claim_reproduction",
        "biomedical_research_data_audit",
        "incident_response",
        "evidence_grounded_policy_comparison",
        "dataset_transformation_molecule",
    ];
    let base = json!({
        "problem": {
            "actions": ["choose-m0", "choose-m1"],
            "models": ["m0", "m1"],
            "loss": [0.0, 1.0, 1.0, 0.0]
        },
        "belief": { "mass": [0.5, 0.5] },
        "acquisitions": [{
            "id": "screen",
            "cost": 0.01,
            "outcomes": [
                { "label": "positive", "likelihood": [0.9, 0.2] },
                { "label": "negative", "likelihood": [0.1, 0.8] }
            ]
        }],
        "budget": 0.1,
        "max_steps": 1,
        "provider": "mcp-simulated",
        "capabilities": ["receipt-only"],
        "observations": [{ "acquisition_id": "screen", "outcome_label": "negative" }]
    });
    for workflow in workflows {
        let mut denied = base.clone();
        denied["workflow"] = json!(workflow);
        let refused = call(&mut server(), "interweave_workflow_execute", denied.clone());
        assert_eq!(refused["ok"], json!(true));
        assert_eq!(refused["workflow"], json!(workflow));
        assert_eq!(refused["receipt"]["adaptive"]["status"], json!("refused"));
        assert_eq!(
            refused["receipt"]["adaptive"]["refusal"],
            json!("authorization_required")
        );

        denied["authorization"] = json!({
            "grant_id": "workflow-grant",
            "provider": "mcp-simulated"
        });
        let simulated = call(&mut server(), "interweave_workflow_execute", denied.clone());
        assert_eq!(simulated["completed"], json!(true));
        assert_eq!(
            simulated["receipt"]["adaptive"]["status"],
            json!("completed")
        );
        assert_eq!(simulated["provenance_counts"]["simulated"], json!(1));
        assert_eq!(
            simulated["release_posture"],
            json!("workflow_receipt_only_external_release_not_authorized")
        );

        let replay = call(
            &mut server(),
            "interweave_workflow_execute",
            json!({
                "mode": "replay",
                "workflow": workflow,
                "problem": denied["problem"].clone(),
                "belief": denied["belief"].clone(),
                "acquisitions": denied["acquisitions"].clone(),
                "budget": 0.1,
                "max_steps": 1,
                "provider": "mcp-simulated",
                "capabilities": ["receipt-only"],
                "receipt": simulated["receipt"].clone()
            }),
        );
        assert_eq!(replay["completed"], json!(true));
        assert_eq!(replay["provenance_counts"]["replayed"], json!(1));
        assert_eq!(replay["provenance_counts"]["simulated"], json!(0));
    }
}

#[test]
fn interweave_workflow_execution_evidence_is_digest_bound_queryable_and_non_promoting() {
    let request = json!({
        "workflow": "biomedical_research_data_audit",
        "problem": {
            "actions": ["hold", "release"],
            "models": ["safe", "unsafe"],
            "loss": [0.0, 2.0, 2.0, 0.0]
        },
        "belief": { "mass": [0.6, 0.4] },
        "acquisitions": [{
            "id": "screen",
            "cost": 0.01,
            "outcomes": [
                { "label": "negative", "likelihood": [0.9, 0.2] },
                { "label": "positive", "likelihood": [0.1, 0.8] }
            ]
        }],
        "budget": 0.1,
        "max_steps": 1,
        "provider": "mcp-simulated",
        "capabilities": ["data.read", "analysis.sandbox"],
        "authorization": { "grant_id": "grant-1", "provider": "mcp-simulated" },
        "observations": [{ "acquisition_id": "screen", "outcome_label": "negative" }],
        "evidence": {
            "subject_id": "bioaudit-subject-1",
            "domains": ["biomedical_research", "privacy"],
            "parent_digests": ["a".repeat(64)]
        }
    });
    let first = call(
        &mut server(),
        "interweave_workflow_execute",
        request.clone(),
    );
    assert_eq!(first["workflow_execution_evidence"]["ok"], json!(true));
    assert_eq!(
        first["workflow_execution_evidence"]["evidence"]["provenance"]["mode"],
        json!("simulated")
    );
    assert_eq!(
        first["workflow_execution_evidence"]["evidence"]["readiness_claimed"],
        json!(false)
    );
    let digest = first["workflow_execution_evidence"]["evidence_digest"]
        .as_str()
        .unwrap()
        .to_string();
    let query = call(
        &mut server(),
        "interweave_workflow_execution_evidence_query",
        json!({ "subject_id": "bioaudit-subject-1", "include_records": false }),
    );
    assert_eq!(query["rows"].as_array().unwrap().len(), 0);

    let mut shared = server();
    let imported = call(
        &mut shared,
        "interweave_workflow_execution_evidence_import",
        json!({ "evidence": first["workflow_execution_evidence"]["evidence"].clone() }),
    );
    assert_eq!(imported["registry"]["created"], json!(true));
    let queried = call(
        &mut shared,
        "interweave_workflow_execution_evidence_query",
        json!({ "subject_id": "bioaudit-subject-1", "include_records": false }),
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    let fetched = call(
        &mut shared,
        "interweave_workflow_execution_evidence_get",
        json!({ "evidence_digest": digest }),
    );
    assert_eq!(
        fetched["record"]["claim_posture"]["status"],
        json!("review_required")
    );

    let mut tampered = first["workflow_execution_evidence"]["evidence"].clone();
    tampered["subject_id"] = json!("tampered");
    let refused = call(
        &mut shared,
        "interweave_workflow_execution_evidence_import",
        json!({ "evidence": tampered }),
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn epistemic_adaptive_costed_keeps_latency_infeasibility_out_of_scalarization() {
    let result = call(
        &mut server(),
        "epistemic_adaptive_costed",
        json!({
            "problem": {
                "actions": ["choose-m0", "choose-m1"],
                "models": ["m0", "m1"],
                "loss": [0.0, 1.0, 1.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "acquisitions": [{
                "acquisition": {
                    "id": "slow-screen",
                    "cost": 0.01,
                    "outcomes": [
                        { "label": "positive", "likelihood": [0.9, 0.1] },
                        { "label": "negative", "likelihood": [0.1, 0.9] }
                    ]
                },
                "cost": {
                    "tokens": 1.0,
                    "compute_ms": 1.0,
                    "latency_ms": 100.0,
                    "money_usd": 0.0,
                    "privacy_loss": 0.0,
                    "specimen_units": 0.0,
                    "expert_minutes": 0.0
                }
            }],
            "budget": {
                "tokens": 100.0,
                "compute_ms": 100.0,
                "latency_ms": 10.0,
                "money_usd": 1.0,
                "privacy_loss": 1.0,
                "specimen_units": 1.0,
                "expert_minutes": 10.0
            },
            "weights": {
                "tokens": 0.0,
                "compute_ms": 0.0,
                "latency_ms": 1.0,
                "money_usd": 0.0,
                "privacy_loss": 0.0,
                "specimen_units": 0.0,
                "expert_minutes": 0.0
            },
            "max_steps": 1
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["cost_dimensions"].as_array().unwrap().len(), 7);
    assert_eq!(result["policy"]["budget"]["latency_ms"], json!(10.0));
    assert!(result["policy"]["root"].get("Stop").is_some());
}

#[test]
fn epistemic_decision_quotient_keeps_permitted_boundary_and_merges_only_equivalent_models() {
    let result = call(
        &mut server(),
        "epistemic_decision_quotient",
        json!({
            "problem": {
                "actions": ["accept", "defer", "reject"],
                "models": ["m-a", "m-b", "m-c"],
                "loss": [
                    0.0, 7.0, 0.0,
                    4.0, 11.0, 5.0,
                    8.0, 15.0, 8.0
                ]
            },
            "permitted_actions": ["reject", "accept", "defer"]
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-decision-quotient/0.1")
    );
    assert_eq!(
        result["quotient"]["permitted_actions"],
        json!(["accept", "defer", "reject"])
    );
    assert_eq!(result["summary"]["original_model_count"], json!(3));
    assert_eq!(result["summary"]["quotient_model_count"], json!(2));
    assert_eq!(result["summary"]["merged_model_count"], json!(1));
    assert_eq!(
        result["quotient"]["model_to_class"]["m-a"],
        result["quotient"]["model_to_class"]["m-b"]
    );
    assert_ne!(
        result["quotient"]["model_to_class"]["m-a"],
        result["quotient"]["model_to_class"]["m-c"]
    );

    let refused = call(
        &mut server(),
        "epistemic_decision_quotient",
        json!({
            "problem": {
                "actions": ["accept"],
                "models": ["m"],
                "loss": []
            },
            "permitted_actions": ["accept"]
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("invariant failed")
    );
}

#[test]
fn epistemic_context_audit_keeps_frontier_sufficiency_and_subset_refusals_distinct() {
    let result = call(
        &mut server(),
        "epistemic_context_audit",
        json!({
            "problem": {
                "actions": ["treat", "abstain"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.5, 0.5] },
            "evidence_pool": {
                "items": [
                    { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
                    { "id": "marker", "cost": 1.0, "likelihood": [0.1, 0.9] }
                ]
            },
            "criterion": "bayes_regret",
            "tolerance": 1.0,
            "compatibility_floor": 0.0,
            "subsets": [[0], [0, 1], [0, 0]],
            "include_frontier": true,
            "max_rows": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-context-audit/0.1")
    );
    assert_eq!(result["criterion"], json!("bayes_regret"));
    assert_eq!(result["evidence_pool"]["item_count"], json!(2));
    assert_eq!(result["evidence_pool"]["full_rate"], json!(3.0));
    assert_eq!(result["frontier"]["evaluated"], json!(4));
    assert!(result["sufficiency"]["outcome"].is_string());
    assert_eq!(result["subset_count"], json!(3));
    assert_eq!(result["subset_refusal_count"], json!(1));
    assert_eq!(result["subset_rows"][2]["result"], json!("refused"));
    assert!(result["identification"]["status"].is_string());
}

#[test]
fn epistemic_selection_audit_gates_guarantees_and_exact_comparisons() {
    let result = call(
        &mut server(),
        "epistemic_selection_audit",
        json!({
            "problem": {
                "actions": ["treat", "defer"],
                "models": ["responsive", "resistant"],
                "loss": [0.0, 10.0, 10.0, 0.0]
            },
            "belief": { "mass": [0.4, 0.6] },
            "evidence_pool": {
                "items": [
                    { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
                    { "id": "marker", "cost": 1.0, "likelihood": [0.8, 0.2] },
                    { "id": "uninformative", "cost": 1.0, "likelihood": [1.0, 1.0] }
                ]
            },
            "constraint": { "cardinality": 2 },
            "protected": [],
            "check_submodularity": true,
            "include_lazy": true,
            "compare_optimum": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/epistemic-selection-audit/0.1")
    );
    assert_eq!(result["objective"], json!("regret_reduction"));
    assert_eq!(result["evidence_pool"]["count"], json!(3));
    assert_eq!(result["submodularity"]["status"], json!("evaluated"));
    assert_eq!(
        result["comparisons"]["exact_optimum"]["status"],
        json!("evaluated")
    );
    assert!(result["greedy"]["chosen"].is_array());
    assert!(result["lazy"]["chosen"].is_array());
    assert!(result["comparisons"]["greedy_lazy_agree"].is_boolean());
    assert!(result["greedy"]["guarantee"]["applicability"].is_string());
}

#[test]
fn brain_control_plane_is_idempotent_hash_chained_and_approval_gated() {
    let mut server = server();
    let spec_digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let submitted = call(
        &mut server,
        "brain_job_submit",
        json!({
            "idempotency_key": "request-001",
            "spec_digest": spec_digest,
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
        }),
    );
    assert_eq!(submitted["__isError"], json!(false));
    assert_eq!(submitted["created"], json!(true));
    let job_id = submitted["job"]["job_id"].as_str().unwrap().to_string();
    assert_eq!(
        submitted["job"]["spec"],
        json!("not_returned; caller resolver owns rehydration")
    );
    assert!(submitted["job"].get("prompt").is_none());

    let idempotent = call(
        &mut server,
        "brain_job_submit",
        json!({
            "idempotency_key": "request-001",
            "spec_digest": spec_digest,
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
        }),
    );
    assert_eq!(idempotent["__isError"], json!(false));
    assert_eq!(idempotent["idempotent"], json!(true));
    assert_eq!(idempotent["job"]["job_id"], json!(job_id));

    let requested = call(
        &mut server,
        "brain_job_approval",
        json!({"job_id": job_id, "action": "request", "reason": "write operation"}),
    );
    assert_eq!(requested["__isError"], json!(false));
    assert_eq!(requested["job"]["state"], json!("waiting_approval"));

    let missing_proof = call(
        &mut server,
        "brain_job_approval",
        json!({"job_id": job_id, "action": "approve"}),
    );
    assert_eq!(missing_proof["__isError"], json!(true));

    let approved = call(
        &mut server,
        "brain_job_approval",
        json!({
            "job_id": job_id,
            "action": "approve",
            "authorization_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        }),
    );
    assert_eq!(approved["__isError"], json!(false));
    assert_eq!(approved["job"]["state"], json!("queued"));
    assert_eq!(
        approved["authorization"]["verified_by_server"],
        json!(false)
    );
    assert_eq!(approved["authorization"]["execution"], json!("not_started"));

    let status = call(&mut server, "brain_job_status", json!({"job_id": job_id}));
    assert_eq!(status["__isError"], json!(false));
    assert_eq!(status["job"]["state"], json!("queued"));
    let events = call(
        &mut server,
        "brain_job_events",
        json!({"job_id": job_id, "limit": 16}),
    );
    assert_eq!(events["__isError"], json!(false));
    assert_eq!(events["events"].as_array().unwrap().len(), 3);
    assert_eq!(events["chain"], json!("sha256_prev_digest"));
    assert!(events["head_digest"].as_str().unwrap().len() == 64);

    let claimed = call(
        &mut server,
        "brain_job_claim",
        json!({"job_id": job_id, "worker_id": "worker-a", "lease_ms": 1000}),
    );
    assert_eq!(claimed["__isError"], json!(false));
    assert_eq!(claimed["job"]["state"], json!("leased"));
    assert_eq!(claimed["job"]["attempts"], json!(1));
    let renewed = call(
        &mut server,
        "brain_job_renew",
        json!({"job_id": job_id, "worker_id": "worker-a", "lease_ms": 1000}),
    );
    assert_eq!(renewed["__isError"], json!(false));
    let checkpointed = call(
        &mut server,
        "brain_job_checkpoint",
        json!({
            "job_id": job_id,
            "worker_id": "worker-a",
            "phase": "preflight",
            "checkpoint_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "side_effect_boundary": "preflight",
        }),
    );
    assert_eq!(checkpointed["__isError"], json!(false));
    assert_eq!(checkpointed["job"]["state"], json!("running"));
    let completed = call(
        &mut server,
        "brain_job_complete",
        json!({
            "job_id": job_id,
            "worker_id": "worker-a",
            "result_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        }),
    );
    assert_eq!(completed["__isError"], json!(false));
    assert_eq!(completed["job"]["state"], json!("succeeded"));
}

#[test]
fn brain_control_plane_health_and_replay_remain_value_only() {
    let mut server = server();
    let health = call(
        &mut server,
        "brain_model_health",
        json!({
            "operation": "record",
            "provider": "openai",
            "model": "gpt-test",
            "status": "success",
            "latency_ms": 120,
            "quality": 0.9,
            "tokens": 512,
            "registered": true,
            "credential_ready": true,
            "eligible": true,
        }),
    );
    assert_eq!(health["__isError"], json!(false));
    assert_eq!(health["health"][0]["provider"], json!("openai"));
    assert_eq!(health["health"][0]["attempts"], json!(1));
    assert_eq!(health["health"][0]["success_rate"], json!(1.0));
    assert!(health["health"][0].get("secret").is_none());
    let snapshot = call(
        &mut server,
        "brain_model_health",
        json!({"operation": "snapshot", "provider": "openai"}),
    );
    assert_eq!(snapshot["__isError"], json!(false));
    assert_eq!(
        snapshot["model_health"]["openai/gpt-test"]["attempts"],
        json!(1)
    );
    assert_eq!(
        snapshot["model_health"]["openai/gpt-test"]["last_latency_ms"],
        json!(120)
    );
    assert_eq!(
        snapshot["model_health"]["openai/gpt-test"]["quality_observations"],
        json!(1)
    );
    assert_eq!(
        snapshot["model_health"]["openai/gpt-test"]["quality_mean"],
        json!(0.9)
    );

    let evidence = json!({
        "schema": "bioprism-brain-domain-evaluator/0.1",
        "domain": "engineering",
        "capability": "code_change",
        "risk_class": "reversible",
        "signals": {
            "schema_valid": 1.0,
            "tests_passed": 1.0,
            "evidence_complete": 1.0,
        },
        "references": [],
        "limitations": [],
        "retention": "value_only_digests_and_signal_scores",
    });
    let evidence_digest = bioprism_ids::ContentHash::of_value(&evidence)
        .unwrap()
        .to_string();
    let replay = call(
        &mut server,
        "brain_replay_evaluate",
        json!({
            "case_id": "case-001",
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
            "evidence_digest": evidence_digest,
            "signals": {
                "schema_valid": true,
                "tests_passed": true,
                "evidence_complete": true,
            },
        }),
    );
    assert_eq!(replay["__isError"], json!(false));
    assert_eq!(replay["passed"], json!(true));
    assert_eq!(
        replay["execution"],
        json!("offline_value_only_replay; no provider or domain tool invocation")
    );
    assert_eq!(
        replay["truth_authority"],
        json!("caller_declared_normalized_signals")
    );

    let secret_attempt = call(
        &mut server,
        "brain_replay_evaluate",
        json!({
            "case_id": "case-002",
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
            "evidence_digest": evidence_digest,
            "signals": {"schema_valid": true, "tests_passed": true, "evidence_complete": true},
            "api_key": "must-never-cross-the-boundary",
        }),
    );
    assert_eq!(secret_attempt["__isError"], json!(true));
}
