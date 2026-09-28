//! Domain workflow, evidence intake, and capability routing invariants.

use super::*;

#[test]
fn domain_report_routes_project_validate_index_and_cover_catalogue() {
    let artifact_path = test_state_path("domain-report-routes");
    let config = ApiConfig {
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let router = ApiRouter::new(root.clone(), config.clone()).unwrap();
    let projected = router.handle(request(
        "POST",
        "/v1/domain-reports",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-domain-report",
            "source_tool": "modality_catalog",
            "report": {"observations": ["caller supplied"]},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    ));
    assert_eq!(projected.status, 200);
    let projected: Value = serde_json::from_slice(&projected.body).unwrap();
    assert_eq!(projected["workflow"], "domain_report_project");
    assert_eq!(projected["artifact_registry"]["indexed"], true);
    let digest = projected["artifact_registry"]["content_digest"].clone();
    let coverage = router.handle(request(
        "GET",
        "/v1/domain-reports/coverage?include_report_digests=true",
        json!({}),
    ));
    assert_eq!(coverage.status, 200);
    let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
    assert_eq!(coverage["workflow"], "domain_report_coverage");
    let group_count = coverage["group_count"].as_u64().unwrap();
    assert_eq!(
        group_count as usize,
        coverage["groups"].as_array().unwrap().len()
    );
    assert!(group_count >= 30);
    assert_eq!(coverage["reported_group_count"], 1);
    assert!(coverage["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap()["report_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == &digest));
    let filtered = router.handle(request(
        "GET",
        "/v1/domain-reports/coverage?report_class=ordinary&bridge_mode=inline",
        json!({}),
    ));
    assert_eq!(filtered.status, 200);
    let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
    assert_eq!(filtered["filters"]["report_class"], "ordinary");
    assert_eq!(filtered["filters"]["bridge_mode"], "inline");
    assert_eq!(filtered["reported_group_count"], 0);
    let refused = router.handle(request(
        "POST",
        "/v1/domain-reports",
        json!({
            "group_id": "biological_domains",
            "domains": ["not_declared"],
            "subject_id": "api-domain-report-refused",
            "source_tool": "modality_catalog",
            "report": {},
            "claim_posture": {"status": "refused", "does_not_claim": ["truth"]}
        }),
    ));
    assert_eq!(refused.status, 422);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn capability_dashboard_route_preserves_filters_and_refuses_unbounded_queries() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let response = router.handle(request(
            "GET",
            "/v1/capabilities/dashboard?domain=verification&max_groups=4&include_tools=true&include_gaps=false",
            json!({}),
        ));
    assert_eq!(response.status, 200);
    let payload: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(payload["workflow"], "capability_dashboard");
    assert_eq!(payload["audit"]["query"]["domain"], "verification");
    assert_eq!(payload["audit"]["query"]["max_groups"], 4);
    assert_eq!(payload["audit"]["query"]["include_tools"], true);
    assert_eq!(payload["audit"]["query"]["include_gaps"], false);
    assert_eq!(
        payload["audit"]["selected_group_count"],
        payload["audit"]["groups"].as_array().unwrap().len()
    );
    assert!(payload["audit"]["selected_group_count"].as_u64().unwrap() >= 1);
    assert_eq!(payload["audit"]["groups"][0]["readiness"], "callable");
    assert_eq!(
        payload["audit"]["groups"][0]["artifact_evidence"]["state"],
        "missing"
    );
    assert_eq!(
        payload["audit"]["groups"][0]["workflow_reconciliation_evidence"]["state"],
        "missing"
    );
    assert_eq!(
        payload["audit"]["evidence"]["groups_with_artifact_evidence"],
        0
    );
    assert_eq!(payload["evidence_digest"].as_str().unwrap().len(), 64);

    let invalid = router.handle(request(
        "GET",
        "/v1/capabilities/dashboard?max_groups=513",
        json!({}),
    ));
    assert_eq!(invalid.status, 400);
    let invalid: Value = serde_json::from_slice(&invalid.body).unwrap();
    assert_eq!(invalid["error"]["code"], "invalid_query");
}

#[test]
fn capability_route_rest_endpoints_return_raw_planning_reports() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let route = router.handle(request(
        "POST",
        "/v1/capabilities/route",
        json!({
            "goal": "audit a cross-domain evidence workflow",
            "needs": [{"id": "audit", "tool": "capability_audit"}],
            "max_candidates_per_need": 4,
            "max_tools": 8,
            "include_tools": true
        }),
    ));
    assert_eq!(route.status, 200);
    let route: Value = serde_json::from_slice(&route.body).unwrap();
    assert_eq!(route["workflow"], "capability_route");
    assert_eq!(route["needs"][0]["resolution"], "explicit");
    assert_eq!(route["execution"], "not_started");
    assert_eq!(route["evidence"]["readiness_claimed"], false);
    assert_eq!(route["evidence_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        route["evidence_digest"],
        route["evidence"]["evidence_digest"]
    );
    assert_eq!(
        route["needs"][0]["candidate_group_evidence"][0]["workflow_reconciliation_evidence"]
            ["state"],
        "missing"
    );
    assert!(route["tool_schemas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|schema| { schema["name"] == "capability_audit" }));

    let review = router.handle(request(
        "POST",
        "/v1/capabilities/route/review",
        json!({
            "route": route.clone(),
            "selections": [{
                "need_id": "audit",
                "tool": "capability_audit",
                "domain": "developer_platform",
                "capability": "capability_audit",
                "objective": "audit the capability catalogue",
                "arguments": {}
            }],
            "validate_schemas": true
        }),
    ));
    assert_eq!(review.status, 200);
    let review: Value = serde_json::from_slice(&review.body).unwrap();
    assert_eq!(review["workflow"], "capability_route_review");
    assert_eq!(review["review_status"], "ready");
    assert_eq!(review["execution"], "not_started");
    assert_eq!(review["evidence_binding"]["present"], true);
    assert_eq!(review["evidence_digest"], route["evidence_digest"]);
    assert_eq!(
        review["mission_draft"]["route_evidence_digest"],
        route["evidence_digest"]
    );

    let planned = router.handle(request(
        "POST",
        "/v1/capabilities/route/plan",
        json!({
            "mission_id": "route-plan-api-test",
            "route": route,
            "selections": [{
                "need_id": "audit",
                "tool": "capability_audit",
                "domain": "developer_platform",
                "capability": "capability_audit",
                "objective": "audit the capability catalogue",
                "arguments": {}
            }],
            "validate_schemas": true
        }),
    ));
    assert_eq!(
        planned.status,
        200,
        "{}",
        String::from_utf8_lossy(&planned.body)
    );
    let planned: Value = serde_json::from_slice(&planned.body).unwrap();
    assert_eq!(planned["workflow"], "capability_route_plan");
    assert_eq!(planned["plan_status"], "ready_for_caller_inspection");
    assert_eq!(planned["dispatch"], "not_started");
    assert_eq!(planned["preflight"]["ok"], true);
    assert_eq!(planned["preflight"]["dispatch"], "not_started");
    assert_eq!(planned["mission"]["mission_id"], "route-plan-api-test");
    assert_eq!(planned["plan_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        planned["mission"]["route_review"]["review_id"],
        review["review_id"]
    );
    assert_eq!(planned["route_id"], review["route_id"]);

    let verified = router.handle(request(
        "POST",
        "/v1/capabilities/route/plan/verify",
        json!({
            "plan": planned,
            "route": route,
            "selections": [{
                "need_id": "audit",
                "tool": "capability_audit",
                "domain": "developer_platform",
                "capability": "capability_audit",
                "objective": "audit the capability catalogue",
                "arguments": {}
            }]
        }),
    ));
    assert_eq!(verified.status, 200);
    let verified: Value = serde_json::from_slice(&verified.body).unwrap();
    assert_eq!(verified["workflow"], "capability_route_plan_verify");
    assert_eq!(verified["valid"], true);
    assert_eq!(verified["verification_status"], "verified");
    assert_eq!(verified["route_replay"]["status"], "matched");
    assert_eq!(verified["mission_preflight"]["status"], "matched");
    assert_eq!(verified["dispatch"], "not_started");

    let shape_only = router.handle(request(
        "POST",
        "/v1/capabilities/route/plan/verify",
        json!({"plan": planned}),
    ));
    assert_eq!(shape_only.status, 200);
    let shape_only: Value = serde_json::from_slice(&shape_only.body).unwrap();
    assert_eq!(shape_only["valid"], true);
    assert_eq!(
        shape_only["verification_status"],
        "verified_without_route_replay"
    );

    let refused = router.handle(request(
        "POST",
        "/v1/capabilities/route/plan",
        json!({
            "mission_id": "route-plan-policy-refusal",
            "route": route,
            "selections": [{
                "need_id": "audit",
                "tool": "capability_audit",
                "domain": "developer_platform",
                "capability": "capability_audit",
                "objective": "audit the capability catalogue",
                "arguments": {}
            }],
            "policy": {"execute": true}
        }),
    ));
    assert_eq!(refused.status, 422);
}

#[test]
fn capability_route_plan_returns_a_bounded_outcome_for_every_catalogue_group() {
    let router = ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
    let catalogue = bioprism_mcp::workspace_capabilities();
    let groups = catalogue.as_array().unwrap();
    assert!(
        !groups.is_empty(),
        "the cross-domain catalogue must not be empty"
    );
    for group in groups {
        let group_id = group["id"].as_str().unwrap();
        let route_response = router.handle(request(
            "POST",
            "/v1/capabilities/route",
            json!({
                "goal": format!("prepare a bounded plan for {group_id}"),
                "needs": [{"id": group_id, "group_id": group_id, "max_items": 1}],
                "max_candidates_per_need": 1,
                "max_tools": 1
            }),
        ));
        assert_eq!(route_response.status, 200, "route failed for {group_id}");
        let route: Value = serde_json::from_slice(&route_response.body).unwrap();
        let candidates = route["needs"][0]["candidate_tools"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if candidates.is_empty() {
            assert_eq!(route["unresolved_needs"][0], group_id);
            continue;
        }
        let tool = candidates[0].as_str().unwrap();
        let domain = route["needs"][0]["candidate_domains"]
            .as_array()
            .and_then(|domains| domains.first())
            .and_then(Value::as_str)
            .unwrap_or(group_id);
        let plan_response = router.handle(request(
            "POST",
            "/v1/capabilities/route/plan",
            json!({
                "mission_id": format!("catalogue-plan-{group_id}"),
                "route": route,
                "selections": [{
                    "need_id": group_id,
                    "tool": tool,
                    "domain": domain,
                    "capability": group_id,
                    "objective": format!("inspect {group_id}"),
                    "arguments": {}
                }]
            }),
        ));
        assert_eq!(plan_response.status, 200, "plan failed for {group_id}");
        let plan: Value = serde_json::from_slice(&plan_response.body).unwrap();
        assert_eq!(plan["workflow"], "capability_route_plan");
        assert_eq!(plan["dispatch"], "not_started");
        assert!(matches!(
            plan["plan_status"].as_str(),
            Some("ready_for_caller_inspection")
                | Some("blocked_by_mission_preflight")
                | Some("blocked_by_route_review")
        ));
    }
}

#[test]
fn domain_evidence_route_harmonizes_reports_and_preserves_artifact_lineage() {
    let artifact_path = test_state_path("domain-evidence-route");
    let config = ApiConfig {
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let first = router.handle(request(
        "POST",
        "/v1/domain-reports",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-harmonization-subject",
            "source_tool": "modality_catalog",
            "report": {"observations": ["modality contract retained"]},
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    ));
    let second = router.handle(request(
        "POST",
        "/v1/domain-reports",
        json!({
            "group_id": "biological_ir_and_query",
            "domains": ["BioQL syntax"],
            "subject_id": "api-harmonization-subject",
            "source_tool": "bioql_compile",
            "report": {"observations": ["query syntax contract retained"]},
            "claim_posture": {"status": "review_required", "does_not_claim": ["execution"]}
        }),
    ));
    assert_eq!(first.status, 200);
    assert_eq!(second.status, 200);
    let first: Value = serde_json::from_slice(&first.body).unwrap();
    let second: Value = serde_json::from_slice(&second.body).unwrap();
    let harmonized = router.handle(request(
        "POST",
        "/v1/domain-evidence/harmonize",
        json!({
            "subject_id": "api-harmonization-subject",
            "claim": {"id": "api-claim-1", "statement": "opaque"},
            "reports": [first["report"].clone(), second["report"].clone()],
            "links": [
                {"report_index": 0, "role": "supports"},
                {"report_index": 1, "role": "qualifies", "note": "syntax is not execution"}
            ],
            "required_group_ids": ["biological_domains", "biological_ir_and_query"],
            "required_domains": ["modalities", "BioQL syntax"]
        }),
    ));
    assert_eq!(
        harmonized.status,
        200,
        "{}",
        String::from_utf8_lossy(&harmonized.body)
    );
    let harmonized: Value = serde_json::from_slice(&harmonized.body).unwrap();
    assert_eq!(harmonized["workflow"], "domain_evidence_harmonize");
    assert_eq!(
        harmonized["harmonization"]["coverage"]["traceability_state"],
        "complete"
    );
    assert_eq!(harmonized["harmonization"]["readiness_claimed"], false);
    assert_eq!(harmonized["artifact_registry"]["indexed"], true);
    assert_eq!(
        harmonized["artifact_registry"]["verification"]["method"],
        "domain_evidence_harmonization"
    );
    let artifacts = router.handle(request(
        "GET",
        "/v1/artifacts?kind=domain_evidence_harmonization&subject_id=api-harmonization-subject",
        json!({}),
    ));
    assert_eq!(artifacts.status, 200);
    let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
    assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);

    let coverage = router.handle(request(
            "GET",
            "/v1/domain-evidence/harmonization/coverage?subject_id=api-harmonization-subject&traceability_state=complete&include_report_digests=true",
            json!({}),
        ));
    assert_eq!(
        coverage.status,
        200,
        "{}",
        String::from_utf8_lossy(&coverage.body)
    );
    let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
    assert_eq!(
        coverage["workflow"],
        "domain_evidence_harmonization_coverage"
    );
    assert_eq!(coverage["matching_count"], 1);
    assert_eq!(
        coverage["rows"][0]["report_digests"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(coverage["rows"][0]["traceability_state"], "complete");

    let invalid_coverage = router.handle(request(
        "GET",
        "/v1/domain-evidence/harmonization/coverage?after=invalid",
        json!({}),
    ));
    assert_eq!(invalid_coverage.status, 422);

    let refused = router.handle(request(
        "POST",
        "/v1/domain-evidence/harmonize",
        json!({
            "subject_id": "api-other-subject",
            "claim": {"id": "api-claim-refused"},
            "reports": [first["report"].clone()],
            "links": [{"report_index": 0, "role": "context"}]
        }),
    ));
    assert_eq!(refused.status, 422);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn domain_evidence_intake_route_retains_raw_envelope_and_indexes_exact_digests() {
    let artifact_path = test_state_path("domain-evidence-intake-route");
    let config = ApiConfig {
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let source = router.handle(request(
            "POST",
            "/v1/domain-evidence/sources",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-intake-subject",
                "source_tool": "modality_catalog",
                "connector_kind": "literature",
                "locator_kind": "uri",
                "locator": "https://example.org/article/1",
                "retrieval_mode": "metadata_only",
                "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
                "does_not_claim": ["retrieval occurred"]
            }),
        ));
    assert_eq!(source.status, 200);
    let source: Value = serde_json::from_slice(&source.body).unwrap();
    assert_eq!(source["workflow"], "domain_evidence_source_plan");
    assert_eq!(source["retrieval_status"], "not_started");
    assert_eq!(source["artifact_registry"]["indexed"], true);
    let source_artifacts = router.handle(request(
        "GET",
        "/v1/artifacts?kind=domain_evidence_source_plan&subject_id=api-intake-subject",
        json!({}),
    ));
    assert_eq!(source_artifacts.status, 200);
    let source_artifacts: Value = serde_json::from_slice(&source_artifacts.body).unwrap();
    assert_eq!(source_artifacts["rows"].as_array().unwrap().len(), 1);
    let response = router.handle(request(
        "POST",
        "/v1/domain-evidence/intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-intake-subject",
            "source_tool": "modality_catalog",
            "request": {"modality": "single_cell"},
            "response": {"status": "bounded", "modalities": ["single_cell"]},
            "outcome": "observed",
            "source_plan_digest": source["plan_digest"].clone(),
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    ));
    assert_eq!(response.status, 200);
    let response: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(response["workflow"], "domain_evidence_intake");
    assert_eq!(response["request_supplied"], true);
    assert_eq!(response["source_plan_digest"], source["plan_digest"]);
    assert_eq!(response["request_digest"].as_str().unwrap().len(), 64);
    assert_eq!(response["response_digest"].as_str().unwrap().len(), 64);
    assert_eq!(response["artifact_registry"]["indexed"], true);
    assert_eq!(
        response["artifact_registry"]["verification"]["method"],
        "domain_evidence_intake"
    );
    assert_eq!(
        response["report"]["report"]["intake"]["response"]["status"],
        "bounded"
    );
    let artifacts = router.handle(request(
        "GET",
        "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-intake-subject",
        json!({}),
    ));
    assert_eq!(artifacts.status, 200);
    let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
    assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);
    let lineage = router.handle(request(
        "GET",
        &format!(
            "/v1/domain-evidence/lineage?content_digest={}",
            response["artifact_registry"]["content_digest"]
                .as_str()
                .unwrap()
        ),
        json!({}),
    ));
    assert_eq!(lineage.status, 200);
    let lineage: Value = serde_json::from_slice(&lineage.body).unwrap();
    assert_eq!(
        lineage["workflow"],
        "artifact_registry_domain_evidence_lineage"
    );
    assert_eq!(lineage["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        lineage["rows"][0]["source_plan"]["binding_state"],
        "retained_and_content_parented"
    );
    assert_eq!(
        lineage["rows"][0]["source_plan"]["content_parent_linked"],
        true
    );
    assert_eq!(lineage["rows"][0]["missing_parent_count"], 1);
    let filtered_lineage = router.handle(request(
            "GET",
            "/v1/domain-evidence/lineage?group_id=biological_domains&domain=MODALITIES&outcome=observed&max_items=1",
            json!({}),
        ));
    assert_eq!(filtered_lineage.status, 200);
    let filtered_lineage: Value = serde_json::from_slice(&filtered_lineage.body).unwrap();
    assert_eq!(filtered_lineage["rows"].as_array().unwrap().len(), 1);
    let coverage = router.handle(request(
        "GET",
        "/v1/domain-evidence/coverage?include_intake_digests=true",
        json!({}),
    ));
    assert_eq!(coverage.status, 200);
    let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
    assert_eq!(coverage["workflow"], "domain_evidence_intake_coverage");
    let group_count = coverage["group_count"].as_u64().unwrap();
    let reported_group_count = coverage["reported_group_count"].as_u64().unwrap();
    let missing_group_count = coverage["missing_group_count"].as_u64().unwrap();
    assert_eq!(
        group_count as usize,
        coverage["groups"].as_array().unwrap().len()
    );
    assert_eq!(reported_group_count + missing_group_count, group_count);
    assert_eq!(
        missing_group_count as usize,
        coverage["missing_group_ids"].as_array().unwrap().len()
    );
    assert!(group_count >= 30);
    assert_eq!(coverage["reported_group_count"], 1);
    assert_eq!(coverage["missing_group_count"], group_count - 1);
    assert_eq!(coverage["complete"], false);
    assert_eq!(coverage["groups_with_artifact_evidence"], 1);
    assert_eq!(coverage["artifact_evidence_records"], 2);
    let reported_group = coverage["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(
        reported_group["intake_digests"].as_array().unwrap().len(),
        1
    );
    assert_eq!(reported_group["artifact_evidence"]["state"], "observed");
    assert_eq!(
        reported_group["artifact_evidence"]["matching_record_count"],
        2
    );
    let filtered = router.handle(request(
        "GET",
        "/v1/domain-evidence/coverage?group_id=biological_domains&domain=MODALITIES",
        json!({}),
    ));
    assert_eq!(filtered.status, 200);
    let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
    assert_eq!(filtered["group_count"], 1);
    assert_eq!(filtered["reported_group_count"], 1);
    assert_eq!(filtered["complete"], true);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_artifacts = restored.handle(request(
        "GET",
        "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-intake-subject",
        json!({}),
    ));
    assert_eq!(restored_artifacts.status, 200);
    let restored_artifacts: Value = serde_json::from_slice(&restored_artifacts.body).unwrap();
    assert_eq!(restored_artifacts["rows"].as_array().unwrap().len(), 1);
    let restored_lineage = restored.handle(request(
        "GET",
        &format!(
            "/v1/domain-evidence/lineage?content_digest={}",
            response["artifact_registry"]["content_digest"]
                .as_str()
                .unwrap()
        ),
        json!({}),
    ));
    assert_eq!(restored_lineage.status, 200);
    let restored_lineage: Value = serde_json::from_slice(&restored_lineage.body).unwrap();
    assert_eq!(restored_lineage["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        restored_lineage["rows"][0]["intake_digest"],
        response["intake_digest"]
    );

    let refused = router.handle(request(
        "POST",
        "/v1/domain-evidence/intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["not-declared"],
            "subject_id": "api-intake-refused",
            "source_tool": "modality_catalog",
            "response": {},
            "outcome": "unknown",
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    ));
    assert_eq!(refused.status, 422);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn domain_evidence_source_execute_route_reads_file_and_restores_intake() {
    let artifact_path = test_state_path("domain-evidence-source-execute-route");
    let config = ApiConfig {
        artifact_state_path: Some(artifact_path.clone()),
        ..ApiConfig::default()
    };
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let router = ApiRouter::new(root.clone(), config.clone()).unwrap();
    let source = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-source-execute-subject",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "fixtures/fiber-v0.1/leakage_query.json",
            "retrieval_mode": "content",
            "retrieval_policy": {"network": "disabled", "max_bytes": 65536},
            "does_not_claim": ["source truth"]
        }),
    ));
    assert_eq!(source.status, 200);
    let source: Value = serde_json::from_slice(&source.body).unwrap();
    let executed = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources/execute",
        json!({"source_plan_digest": source["plan_digest"].clone()}),
    ));
    assert_eq!(
        executed.status,
        200,
        "{}",
        String::from_utf8_lossy(&executed.body)
    );
    let executed: Value = serde_json::from_slice(&executed.body).unwrap();
    assert_eq!(executed["workflow"], "domain_evidence_source_execute");
    assert_eq!(executed["outcome"], "observed");
    assert_eq!(executed["intake"]["artifact_registry"]["indexed"], true);
    assert_eq!(executed["raw_content_digest"].as_str().unwrap().len(), 64);
    let restored = ApiRouter::new(root, config).unwrap();
    let artifacts = restored.handle(request(
        "GET",
        "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-source-execute-subject",
        json!({}),
    ));
    assert_eq!(artifacts.status, 200);
    let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
    assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_file(artifact_path);
}

#[test]
fn domain_evidence_source_http_is_denied_without_operator_origin_approval() {
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let router = ApiRouter::new(root, ApiConfig::default()).unwrap();
    let source = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-source-http-default-deny",
            "source_tool": "modality_catalog",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": format!("http://127.0.0.1:{}/evidence", address.port()),
            "retrieval_mode": "content",
            "retrieval_policy": {
                "network": "enabled",
                "allowed_hosts": ["127.0.0.1"],
                "max_bytes": 4096
            },
            "does_not_claim": ["source truth"]
        }),
    ));
    assert_eq!(source.status, 200);
    let source: Value = serde_json::from_slice(&source.body).unwrap();
    let executed = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources/execute",
        json!({"source_plan_digest": source["plan_digest"].clone()}),
    ));
    assert_eq!(executed.status, 200);
    let executed: Value = serde_json::from_slice(&executed.body).unwrap();
    assert_eq!(executed["outcome"], "refused");
    assert!(
        executed["execution_result"]["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("operator's server-level allow-list")
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "default policy opened a socket"
    );
}

#[test]
fn domain_evidence_source_http_requires_both_plan_and_operator_approval() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
            )
            .unwrap();
    });

    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let router = ApiRouter::new_with_domain_evidence_source_origins(
        root,
        ApiConfig::default(),
        vec![format!("127.0.0.1:{}", address.port())],
    )
    .unwrap();
    let source = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "api-source-http-operator-allow",
            "source_tool": "modality_catalog",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": format!("http://127.0.0.1:{}/evidence", address.port()),
            "retrieval_mode": "content",
            "retrieval_policy": {
                "network": "enabled",
                "allowed_hosts": ["127.0.0.1"],
                "max_bytes": 4096
            },
            "does_not_claim": ["source truth"]
        }),
    ));
    assert_eq!(source.status, 200);
    let source: Value = serde_json::from_slice(&source.body).unwrap();
    let executed = router.handle(request(
        "POST",
        "/v1/domain-evidence/sources/execute",
        json!({"source_plan_digest": source["plan_digest"].clone()}),
    ));
    assert_eq!(executed.status, 200);
    let executed: Value = serde_json::from_slice(&executed.body).unwrap();
    assert_eq!(executed["outcome"], "observed");
    assert_eq!(executed["execution_result"]["http_status"], 200);
    assert_eq!(
        executed["execution_result"]["response"]["retrieval"]["body"]["ok"],
        true
    );
    server_thread.join().unwrap();
}

#[test]
fn domain_workflow_routes_expose_catalogue_and_scoped_preflight() {
    let reconciliation_path = test_state_path("domain-workflow-auto-reconciliation");
    let config = ApiConfig {
        reconciliation_state_path: Some(reconciliation_path.clone()),
        ..ApiConfig::default()
    };
    let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
    let catalogue = router.handle(request("GET", "/v1/domain-workflows", json!({})));
    assert_eq!(catalogue.status, 200);
    let catalogue: Value = serde_json::from_slice(&catalogue.body).unwrap();
    assert_eq!(catalogue["workflow"], "domain_workflow_catalogue");
    let workflow_count = catalogue["workflow_count"].as_u64().unwrap();
    assert_eq!(
        workflow_count as usize,
        catalogue["workflows"].as_array().unwrap().len()
    );
    assert!(workflow_count >= 30);
    assert_eq!(
        catalogue["workflow_count"],
        catalogue["coverage"]["group_count"]
    );
    assert!(catalogue["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|workflow| workflow["workflow_id"] == "autonomous_research_campaigns"));
    assert_eq!(catalogue["coverage"]["all_groups_have_workflow"], true);
    assert_eq!(
        catalogue["coverage"]["all_workflows_have_domain_contract"],
        true
    );
    assert!(catalogue["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|workflow| workflow["domain_contract"].is_object()));

    let scaffolded = router.handle(request(
        "POST",
        "/v1/domain-workflows/scaffold",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "api-scaffold-1",
            "goal": "prepare a repository discovery starting plan",
            "tools": ["workspace_capabilities"],
            "arguments": {"workspace_capabilities": {}}
        }),
    ));
    assert_eq!(scaffolded.status, 200);
    let scaffolded: Value = serde_json::from_slice(&scaffolded.body).unwrap();
    assert_eq!(scaffolded["workflow"], "domain_workflow_scaffold");
    assert_eq!(scaffolded["execution"], "not_started");
    assert_eq!(scaffolded["readiness_claimed"], false);
    assert_eq!(scaffolded["mission"]["policy"]["execute"], false);
    assert_eq!(scaffolded["selection"]["strategy"], "explicit_tools");
    assert_eq!(scaffolded["preflight_report"]["dispatch"], "not_started");
    assert_eq!(scaffolded["preflight_status"], "ready");

    let instantiated = router.handle(request(
        "POST",
        "/v1/domain-workflows/instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "api-workflow-1",
            "goal": "discover repository capabilities",
            "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
            "policy": {"execute": true}
        }),
    ));
    assert_eq!(instantiated.status, 200);
    let instantiated: Value = serde_json::from_slice(&instantiated.body).unwrap();
    assert_eq!(instantiated["workflow"], "domain_workflow_instantiate");
    assert_eq!(
        instantiated["preflight_report"]["workflow"],
        "agent_mission"
    );
    assert_eq!(instantiated["execution"], "not_started");
    assert_eq!(
        instantiated["selection"]["all_selected_tools_available"],
        true
    );
    assert_eq!(
        instantiated["evidence_plan"]["steps"][0]["step_id"],
        "catalog"
    );

    let portfolio = router.handle(request(
        "POST",
        "/v1/domain-workflows/portfolio",
        json!({
            "requests": [{
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-portfolio-1",
                "goal": "discover repository capabilities",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                "policy": {"execute": true}
            }]
        }),
    ));
    assert_eq!(portfolio.status, 200);
    let portfolio: Value = serde_json::from_slice(&portfolio.body).unwrap();
    assert_eq!(portfolio["workflow"], "domain_workflow_portfolio");
    assert_eq!(portfolio["valid"], true);
    assert_eq!(portfolio["portfolio_ready"], true);
    assert_eq!(
        portfolio["portfolio_status"],
        "ready_for_authoritative_preflight"
    );
    assert_eq!(portfolio["summary"]["preflight_status"], "matched");
    assert_eq!(portfolio["items"][0]["status"], "instantiated");
    assert_eq!(portfolio["items"][0]["mission_preflight"]["matched"], true);
    assert_eq!(portfolio["dispatch"], "not_started");

    let portfolio_verified = router.handle(request(
        "POST",
        "/v1/domain-workflows/portfolio/verify",
        json!({
            "portfolio": portfolio.clone(),
            "replay_requests": [{
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-portfolio-1",
                "goal": "discover repository capabilities",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                "policy": {"execute": true}
            }],
            "policy": {"require_replay": true}
        }),
    ));
    assert_eq!(portfolio_verified.status, 200);
    let portfolio_verified: Value = serde_json::from_slice(&portfolio_verified.body).unwrap();
    assert_eq!(
        portfolio_verified["workflow"],
        "domain_workflow_portfolio_verify"
    );
    assert_eq!(portfolio_verified["valid"], true);
    assert_eq!(portfolio_verified["verification_status"], "verified");
    assert_eq!(portfolio_verified["summary"]["replay_matched_count"], 1);
    assert_eq!(portfolio_verified["items"][0]["status"], "verified");
    assert_eq!(portfolio_verified["dispatch"], "not_started");

    let verified = router.handle(request(
        "POST",
        "/v1/domain-workflows/verify",
        json!({
            "instantiation": instantiated.clone(),
            "replay_request": {
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-workflow-1",
                "goal": "discover repository capabilities",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                "policy": {"execute": true}
            }
        }),
    ));
    assert_eq!(verified.status, 200);
    let verified: Value = serde_json::from_slice(&verified.body).unwrap();
    assert_eq!(verified["workflow"], "domain_workflow_verify");
    assert_eq!(verified["valid"], true);
    assert_eq!(verified["verification_status"], "verified");
    assert_eq!(verified["replay"]["matched"], true);
    assert_eq!(verified["mission_preflight"]["matched"], true);
    assert_eq!(verified["dispatch"], "not_started");

    let executed = router.handle(request(
        "POST",
        "/v1/tools/agent_mission",
        instantiated["mission"].clone(),
    ));
    assert_eq!(executed.status, 200);
    let executed: Value = serde_json::from_slice(&executed.body).unwrap();
    let mission_report: Value = serde_json::from_str(
        executed["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(mission_report["mission_status"], "succeeded");
    assert_eq!(mission_report["workflow_reconciliation"]["present"], true);
    assert_eq!(mission_report["workflow_reconciliation"]["automatic"], true);
    let automatic_digest = mission_report["workflow_reconciliation"]["reconciliation_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let automatic_query = router.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations?mission_id=api-workflow-1",
        json!({}),
    ));
    assert_eq!(automatic_query.status, 200);
    let automatic_query: Value = serde_json::from_slice(&automatic_query.body).unwrap();
    assert_eq!(automatic_query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        automatic_query["rows"][0]["reconciliation_digest"],
        automatic_digest
    );
    let reconciled = router.handle(request(
        "POST",
        "/v1/domain-workflows/reconcile",
        json!({"instantiation": instantiated, "mission_report": mission_report}),
    ));
    assert_eq!(reconciled.status, 200);
    let reconciled: Value = serde_json::from_slice(&reconciled.body).unwrap();
    assert_eq!(reconciled["workflow"], "domain_workflow_reconcile");
    assert_eq!(reconciled["completion"]["status"], "complete");
    assert_eq!(reconciled["completion"]["ready"], true);

    let refused = router.handle(request(
        "POST",
        "/v1/domain-workflows/instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "api-workflow-refused",
            "goal": "refuse cross-group selection",
            "steps": [{"id": "compile", "tool": "bioql_compile"}]
        }),
    ));
    assert_eq!(refused.status, 422);
    let refused: Value = serde_json::from_slice(&refused.body).unwrap();
    assert_eq!(refused["error"]["code"], "invalid_domain_workflow");
    drop(router);
    let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
    let restored_query = restored.handle(request(
        "GET",
        "/v1/domain-workflows/reconciliations?mission_id=api-workflow-1",
        json!({}),
    ));
    assert_eq!(restored_query.status, 200);
    let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
    assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        restored_query["rows"][0]["reconciliation_digest"],
        automatic_digest
    );
    let _ = std::fs::remove_file(reconciliation_path);
}
