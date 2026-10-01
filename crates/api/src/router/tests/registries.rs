//! Content-addressed evidence, artifact, and report registry invariants.

use super::*;

#[test]
fn route_review_evidence_is_queryable_by_content_addressed_id() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let review_id = "a".repeat(64);
    router.record_tool_event(
        "review-request",
        "capability_route_review",
        &json!({
            "result": {
                "structuredContent": {
                    "workflow": "capability_route_review",
                    "review_id": review_id.clone(),
                }
            }
        }),
    );

    let filtered = router.handle(request(
        "GET",
        &format!("/v1/events?after=0&limit=10&review_id={review_id}"),
        json!({}),
    ));
    assert_eq!(filtered.status, 200);
    let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
    assert_eq!(filtered["page"]["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        filtered["page"]["events"][0]["request_id"],
        "review-request"
    );

    let evidence = router.handle(request(
        "GET",
        &format!("/v1/route-reviews/{review_id}/evidence?after=0&limit=10"),
        json!({}),
    ));
    assert_eq!(evidence.status, 200);
    let evidence: Value = serde_json::from_slice(&evidence.body).unwrap();
    assert_eq!(evidence["workflow"], "capability_route_review_evidence");
    assert_eq!(evidence["review_id"], review_id);
    assert_eq!(evidence["found"], true);
    assert_eq!(evidence["page"]["events"].as_array().unwrap().len(), 1);

    let invalid = router.handle(request(
        "GET",
        "/v1/route-reviews/not-a-review/evidence",
        json!({}),
    ));
    assert_eq!(invalid.status, 400);
}

#[test]
fn evidence_registry_import_is_idempotent_indexed_and_restart_safe() {
    let path = test_state_path("evidence-registry");
    let artifact_path = test_state_path("evidence-registry-artifacts");
    let config = ApiConfig {
        evidence_state_path: Some(path.clone()),
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let mut bundle = json!({
        "schema": "bioprism-api/mission-evidence-bundle/0.1",
        "workflow": "mission_evidence_bundle_export",
        "mission_id": "registry-mission",
        "retention": {"mode": "summary_only", "result_retained": false, "result_included": false},
        "result": null,
        "result_digest": null,
        "evaluator_replay": {"workflow": "mission_evaluator_replay_summary", "bindings": [{"domain": "oncology", "adapter_id": "oncoworlds.assay_fidelity"}]},
        "catalog_drift": {"status": "unchanged"},
        "trace": [],
        "export": {"format": "json", "include_result": false, "include_trace": true, "trace_included": true, "include_fixtures": false, "max_items": 16, "digest_algorithm": "sha256", "execution": "not_started"},
        "execution": "not_started"
    });
    bundle["bundle_digest"] = json!(ContentHash::of_value(&bundle).unwrap().to_string());
    let imported = router.handle(request(
        "POST",
        "/v1/evidence-bundles",
        json!({"bundle": bundle.clone()}),
    ));
    assert_eq!(imported.status, 201);
    let imported: Value = serde_json::from_slice(&imported.body).unwrap();
    assert_eq!(imported["workflow"], "mission_evidence_bundle_import");
    assert_eq!(imported["created"], true);
    assert_eq!(imported["artifact_registry"]["indexed"], true);
    assert_eq!(
        imported["artifact_registry"]["kind"],
        "mission_evidence_bundle"
    );
    let artifact_digest = imported["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let digest = imported["bundle_digest"].as_str().unwrap().to_string();
    let duplicate = router.handle(request(
        "POST",
        "/v1/evidence-bundles",
        json!({"bundle": bundle}),
    ));
    assert_eq!(duplicate.status, 200);
    let duplicate: Value = serde_json::from_slice(&duplicate.body).unwrap();
    assert_eq!(duplicate["already_present"], true);
    assert_eq!(duplicate["artifact_registry"]["indexed"], true);
    assert_eq!(
        duplicate["artifact_registry"]["content_digest"],
        artifact_digest
    );
    let queried = router.handle(request(
        "GET",
        "/v1/evidence-bundles?mission_id=registry-mission&domain=oncology&limit=10",
        json!({}),
    ));
    assert_eq!(queried.status, 200);
    let queried: Value = serde_json::from_slice(&queried.body).unwrap();
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["bundle_digest"], digest);
    assert_eq!(queried["rows"][0]["domains"], json!(["oncology"]));
    let fetched = router.handle(request(
        "GET",
        &format!("/v1/evidence-bundles/{digest}"),
        json!({}),
    ));
    assert_eq!(fetched.status, 200);
    let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
    assert_eq!(fetched["bundle"]["bundle_digest"], digest);
    assert_eq!(
        router
            .handle(request(
                "POST",
                "/v1/evidence-bundles/persistence/flush",
                json!({})
            ))
            .status,
        200
    );
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let status = restored.handle(request(
        "GET",
        "/v1/evidence-bundles/persistence",
        json!({}),
    ));
    let status: Value = serde_json::from_slice(&status.body).unwrap();
    assert_eq!(status["integrity_verified"], true);
    assert_eq!(status["registry_size"], 1);
    let artifact = restored.handle(request(
        "GET",
        &format!("/v1/artifacts/{artifact_digest}"),
        json!({}),
    ));
    assert_eq!(artifact.status, 200);
    let artifact: Value = serde_json::from_slice(&artifact.body).unwrap();
    assert_eq!(artifact["record"]["content_digest"], artifact_digest);
    let restored_query = restored.handle(request(
        "GET",
        "/v1/evidence-bundles?mission_id=registry-mission&limit=10",
        json!({}),
    ));
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn workflow_reconciliation_registry_is_idempotent_indexed_and_restart_safe() {
    let path = test_state_path("workflow-reconciliation-registry");
    let artifact_path = test_state_path("workflow-reconciliation-artifacts");
    let config = ApiConfig {
        reconciliation_state_path: Some(path.clone()),
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let mut record = json!({
        "ok": true,
        "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
        "workflow": "domain_workflow_reconcile",
        "workflow_id": "documentation_and_knowledge",
        "workflow_digest": "a".repeat(64),
        "catalog_digest": "b".repeat(64),
        "domain_contract_digest": "c".repeat(64),
        "mission_id": "reconciliation-api-mission",
        "mission_plan_digest": "d".repeat(64),
        "source": "mission_report",
        "completion": {"status": "complete", "ready": true, "review_required": true},
        "evidence": {"evidence_valid": true},
        "integrity": {"valid": true, "finding_count": 0, "findings": []},
        "execution": "not_started"
    });
    record["reconciliation_digest"] = json!(ContentHash::of_value(&record).unwrap().to_string());
    let imported = router.handle(request(
        "POST",
        "/v1/domain-workflows/reconciliations",
        json!({"record": record.clone()}),
    ));
    assert_eq!(imported.status, 201);
    let imported: Value = serde_json::from_slice(&imported.body).unwrap();
    assert_eq!(imported["created"], true);
    assert_eq!(imported["artifact_registry"]["indexed"], true);
    let artifact_digest = imported["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let digest = imported["reconciliation_digest"]
        .as_str()
        .unwrap()
        .to_string();

    let duplicate = router.handle(request(
        "POST",
        "/v1/domain-workflows/reconciliations",
        json!({"record": record}),
    ));
    assert_eq!(duplicate.status, 200);
    let duplicate: Value = serde_json::from_slice(&duplicate.body).unwrap();
    assert_eq!(duplicate["already_present"], true);
    assert_eq!(duplicate["artifact_registry"]["indexed"], true);
    assert_eq!(
        duplicate["artifact_registry"]["content_digest"],
        artifact_digest
    );
    let queried = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=reconciliation-api-mission&completion_status=complete&limit=10",
            json!({}),
        ));
    assert_eq!(queried.status, 200);
    let queried: Value = serde_json::from_slice(&queried.body).unwrap();
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["reconciliation_digest"], digest);
    let fetched = router.handle(request(
        "GET",
        &format!("/v1/domain-workflows/reconciliations/{digest}"),
        json!({}),
    ));
    assert_eq!(fetched.status, 200);
    let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
    assert_eq!(fetched["record"]["reconciliation_digest"], digest);
    let persistence = router.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations/persistence",
        json!({}),
    ));
    let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
    assert_eq!(persistence["enabled"], true);
    assert_eq!(persistence["integrity_verified"], true);
    let artifact = router.handle(request(
        "GET",
        &format!("/v1/artifacts/{artifact_digest}"),
        json!({}),
    ));
    assert_eq!(artifact.status, 200);

    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_query = restored.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations?mission_id=reconciliation-api-mission",
        json!({}),
    ));
    assert_eq!(restored_query.status, 200);
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(restored_query["registry_generation"], 1);
    let restored_artifact = restored.handle(request(
        "GET",
        &format!("/v1/artifacts/{artifact_digest}"),
        json!({}),
    ));
    assert_eq!(restored_artifact.status, 200);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn artifact_registry_routes_join_lineage_and_restore_only_digest_valid_records() {
    let path = test_state_path("artifact-registry");
    let config = ApiConfig {
        artifact_state_path: Some(path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let leaf = router.handle(request(
        "POST",
        "/v1/artifacts",
        json!({
            "kind": "domain_report",
            "subject_id": "leaf",
            "domains": ["oncology", "genomics"],
            "parent_digests": [],
            "artifact": {"status": "review_required"}
        }),
    ));
    assert_eq!(leaf.status, 201);
    let leaf: Value = serde_json::from_slice(&leaf.body).unwrap();
    let leaf_digest = leaf["content_digest"].as_str().unwrap().to_string();
    let root = router.handle(request(
        "POST",
        "/v1/artifacts",
        json!({
            "kind": "mission_report",
            "subject_id": "root",
            "domains": ["oncology"],
            "parent_digests": [leaf_digest, "f".repeat(64)],
            "artifact": {"status": "partial"}
        }),
    ));
    assert_eq!(root.status, 201);
    let root: Value = serde_json::from_slice(&root.body).unwrap();
    let root_digest = root["content_digest"].as_str().unwrap().to_string();
    let lineage = router.handle(request(
        "GET",
        &format!("/v1/artifacts/{root_digest}/lineage"),
        json!({}),
    ));
    assert_eq!(lineage.status, 200);
    let lineage: Value = serde_json::from_slice(&lineage.body).unwrap();
    assert_eq!(lineage["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        lineage["missing_parent_digests"].as_array().unwrap().len(),
        1
    );
    assert!(lineage["does_not_claim"]
        .as_array()
        .unwrap()
        .iter()
        .any(|claim| claim
            .as_str()
            .unwrap()
            .contains("causal provenance or scientific validity")));
    let query = router.handle(request(
        "GET",
        "/v1/artifacts?domain=oncology&limit=10",
        json!({}),
    ));
    assert_eq!(query.status, 200);
    let query: Value = serde_json::from_slice(&query.body).unwrap();
    assert_eq!(query["rows"].as_array().unwrap().len(), 2);
    let persistence = router.handle(request("GET", "/v1/artifacts/persistence", json!({})));
    let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
    assert_eq!(persistence["integrity_verified"], true);
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_query = restored.handle(request(
        "GET",
        "/v1/artifacts?domain=oncology&limit=10",
        json!({}),
    ));
    assert_eq!(restored_query.status, 200);
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    assert_eq!(restored_query["rows"].as_array().unwrap().len(), 2);
    assert_eq!(restored_query["registry_generation"], 2);
    let cross_store = restored.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
    assert_eq!(cross_store.status, 200);
    let cross_store: Value = serde_json::from_slice(&cross_store.body).unwrap();
    assert_eq!(
        cross_store["workflow"],
        "artifact_registry_cross_store_audit"
    );
    assert_eq!(cross_store["consistent"], true);
    assert_eq!(
        cross_store["stores"]["artifact_registry"]["record_count"],
        2
    );
    assert_eq!(
        cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
        0
    );
    assert_eq!(cross_store["findings"], json!([]));
    let _ = std::fs::remove_file(path);
}

#[test]
fn workflow_execution_evidence_api_shares_and_restores_registry() {
    let evidence_path = test_state_path("workflow-execution-evidence-registry");
    let artifact_path = test_state_path("workflow-execution-evidence-artifacts");
    let config = ApiConfig {
        artifact_state_path: Some(artifact_path.clone()),
        workflow_execution_evidence_state_path: Some(evidence_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let executed = router.handle(request(
        "POST",
        "/v1/tools/interweave_workflow_execute",
        json!({
            "workflow": "biomedical_research_data_audit",
            "problem": {
                "actions": ["hold", "release"],
                "models": ["safe", "unsafe"],
                "loss": [0.0, 2.0, 2.0, 0.0]
            },
            "belief": {"mass": [0.6, 0.4]},
            "acquisitions": [{
                "id": "screen",
                "cost": 0.01,
                "outcomes": [
                    {"label": "negative", "likelihood": [0.9, 0.2]},
                    {"label": "positive", "likelihood": [0.1, 0.8]}
                ]
            }],
            "budget": 0.1,
            "max_steps": 1,
            "provider": "mcp-simulated",
            "capabilities": ["data.read", "analysis.sandbox"],
            "authorization": {"grant_id": "grant-1", "provider": "mcp-simulated"},
            "observations": [{"acquisition_id": "screen", "outcome_label": "negative"}],
            "evidence": {
                "subject_id": "api-workflow-evidence-subject",
                "domains": ["biomedical_research", "privacy"],
                "parent_digests": ["a".repeat(64)]
            }
        }),
    ));
    assert_eq!(
        executed.status,
        200,
        "{}",
        String::from_utf8_lossy(&executed.body)
    );
    let envelope: Value = serde_json::from_slice(&executed.body).unwrap();
    let text = envelope["mcp"]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let result: Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        result["workflow_execution_evidence"]["ok"], true,
        "{result}"
    );
    let evidence_digest = result["workflow_execution_evidence"]["evidence_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let cross_store = router.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
    let cross_store: Value = serde_json::from_slice(&cross_store.body).unwrap();
    assert_eq!(
        cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
        1
    );
    drop(router);

    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_cross_store =
        restored.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
    let restored_cross_store: Value = serde_json::from_slice(&restored_cross_store.body).unwrap();
    assert_eq!(
        restored_cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
        1
    );
    let fetched = restored.handle(request(
        "POST",
        "/v1/tools/interweave_workflow_execution_evidence_get",
        json!({"evidence_digest": evidence_digest}),
    ));
    assert_eq!(fetched.status, 200);
    let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
    assert!(fetched["mcp"]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("review_required"));
    let _ = std::fs::remove_file(evidence_path);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn developer_workbench_verification_route_replays_retained_report() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let digest = "a".repeat(64);
    let session = json!({
        "session_id": "api-workbench-verify",
        "owner": "agent-a",
        "goal": "verify a retained authoring handoff",
        "artifacts": [{
            "id": "artifact-1", "title": "card", "path": "card.json", "domain": "oncology",
            "capability": "verification", "state": "validated", "evidence": "reproduced", "digest": digest
        }],
        "cells": [],
        "changes": []
    });
    let ci = json!({
        "workflow": "consumer contracts", "triggers": ["pull_request"], "rust_toolchain": "stable",
        "offline": true, "checks": [{"name": "unit", "run": "cargo test -p bioprism-devplat", "required": true}]
    });
    let planned = router.handle(request(
        "POST",
        "/v1/tools/developer_workbench",
        json!({"session": session.clone(), "ci": ci.clone()}),
    ));
    assert_eq!(planned.status, 200);
    let planned: Value = serde_json::from_slice(&planned.body).unwrap();
    let retained: Value = serde_json::from_str(
        planned["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let verified = router.handle(request(
        "POST",
        "/v1/developer-workbench/verify",
        json!({
            "session": session,
            "report": retained,
            "ci_replay": ci,
            "policy": {"require_ci": true, "require_ci_replay": true}
        }),
    ));
    assert_eq!(verified.status, 200);
    let verified: Value = serde_json::from_slice(&verified.body).unwrap();
    assert_eq!(verified["workflow"], "developer_workbench_verify");
    assert_eq!(verified["valid"], true);
    assert_eq!(verified["status"], "verified");
    assert_eq!(verified["ci_verified"], true);
    assert_eq!(verified["execution"], "not_started");
}

#[test]
fn developer_workbench_report_registry_is_queryable_and_restart_safe() {
    let path = test_state_path("workbench-registry");
    let config = ApiConfig {
        workbench_state_path: Some(path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let planned = router.handle(request(
        "POST",
        "/v1/tools/developer_workbench",
        json!({
            "session": {
                "session_id": "api-workbench-registry",
                "owner": "agent-a",
                "goal": "retain a report",
                "artifacts": [{
                    "id": "artifact-1", "title": "card", "path": "card.json",
                    "domain": "oncology", "capability": "evidence", "state": "validated",
                    "evidence": "observed", "digest": "a".repeat(64)
                }],
                "cells": [], "changes": []
            },
            "dashboard": {"domains": ["oncology"], "limit": 10}
        }),
    ));
    let planned: Value = serde_json::from_slice(&planned.body).unwrap();
    let retained: Value = serde_json::from_str(
        planned["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let imported = router.handle(request(
        "POST",
        "/v1/developer-workbench/reports",
        json!({"report": retained}),
    ));
    assert_eq!(imported.status, 201);
    let imported: Value = serde_json::from_slice(&imported.body).unwrap();
    let digest = imported["workbench_report_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let queried = router.handle(request(
        "GET",
        "/v1/developer-workbench/reports?domain=oncology&capability=evidence&limit=10",
        json!({}),
    ));
    let queried: Value = serde_json::from_slice(&queried.body).unwrap();
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["workbench_report_digest"], digest);
    let fetched = router.handle(request(
        "GET",
        &format!("/v1/developer-workbench/reports/{digest}"),
        json!({}),
    ));
    assert_eq!(fetched.status, 200);
    let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
    assert_eq!(
        fetched["report"]["schema_version"],
        "bioprism-devplat-workbench/0.1"
    );
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let status = restored.handle(request(
        "GET",
        "/v1/developer-workbench/reports/persistence",
        json!({}),
    ));
    let status: Value = serde_json::from_slice(&status.body).unwrap();
    assert_eq!(status["integrity_verified"], true);
    assert_eq!(status["registry_size"], 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn ci_provider_evidence_registry_reaudits_joins_and_restores() {
    let path = test_state_path("ci-provider-evidence-registry");
    let config = ApiConfig {
        ci_provider_evidence_state_path: Some(path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let imported = router.handle(request(
            "POST",
            "/v1/ci/provider-evidence",
            json!({
                "ci": {
                    "workflow": "api-ci",
                    "triggers": ["push"],
                    "rust_toolchain": "stable",
                    "checks": [{"name": "unit", "run": "cargo test -p bioprism-devplat", "required": true}],
                    "offline": true
                },
                "provider": "generic",
                "payload": {
                    "run_id": "api-provider-run-1",
                    "conclusion": "success",
                    "checks": [{"name": "unit", "status": "success"}]
                },
                "artifacts": [{
                    "id": "artifact-1", "kind": "test-report", "digest": "a".repeat(64),
                    "check": "unit", "run_id": "api-provider-run-1", "provider": "generic",
                    "uri": "https://example.test/artifact-1", "digest_scope": "local_response_bytes"
                }],
                "logs": [{
                    "id": "log-1", "digest": "b".repeat(64), "check": "unit",
                    "run_id": "api-provider-run-1", "provider": "generic", "truncated": false
                }],
                "attestations": [{
                    "id": "attestation-1", "subject": "artifact-1", "issuer": "test",
                    "statement_digest": "c".repeat(64), "method": "detached", "subject_digest": "a".repeat(64)
                }]
            }),
        ));
    assert_eq!(
        imported.status,
        201,
        "{}",
        String::from_utf8_lossy(&imported.body)
    );
    let imported: Value = serde_json::from_slice(&imported.body).unwrap();
    assert_eq!(imported["conformance_ready"], true);
    assert_eq!(imported["local_byte_hash_artifact_count"], 1);
    assert_eq!(imported["attestation_subject_digest_binding_count"], 1);
    assert_eq!(
        imported["artifact_record_digest"].as_str().unwrap().len(),
        64
    );
    let digest = imported["provider_evidence_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let queried = router.handle(request(
            "GET",
            "/v1/ci/provider-evidence?provider=generic&conformance_ready=true&include_records=true&min_local_byte_hash_artifacts=1&min_attestation_subject_digest_bindings=1&max_items=10",
            json!({}),
        ));
    assert_eq!(queried.status, 200);
    let queried: Value = serde_json::from_slice(&queried.body).unwrap();
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["provider_evidence_digest"], digest);
    assert_eq!(queried["rows"][0]["audit"]["artifact_count"], 1);
    assert_eq!(queried["rows"][0]["local_byte_hash_artifact_count"], 1);
    let ambiguous = router.handle(request(
        "GET",
        "/v1/ci/provider-evidence?limit=10&max_items=10",
        json!({}),
    ));
    assert_eq!(ambiguous.status, 400);
    let fetched = router.handle(request(
        "GET",
        &format!("/v1/ci/provider-evidence/{digest}"),
        json!({}),
    ));
    assert_eq!(fetched.status, 200);
    let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
    assert_eq!(fetched["audit"]["run_id"], "api-provider-run-1");
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let status = restored.handle(request(
        "GET",
        "/v1/ci/provider-evidence/persistence",
        json!({}),
    ));
    let status: Value = serde_json::from_slice(&status.body).unwrap();
    assert_eq!(status["integrity_verified"], true);
    assert_eq!(status["registry_size"], 1);
    let _ = std::fs::remove_file(path);
}
