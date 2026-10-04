//! MCP contract tests for domain evidence contracts.

use super::*;

#[test]
fn domain_report_projection_checks_catalogue_indexes_idempotently_and_reports_coverage() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "domain-report-subject",
        "source_tool": "modality_catalog",
        "report": {"observations": ["caller supplied"], "status": "review_required"},
        "claim_posture": {
            "status": "review_required",
            "does_not_claim": ["clinical validity", "execution completion"],
            "limitations": ["no external provenance was supplied"]
        }
    });
    let first = call(&mut server, "domain_report_project", arguments.clone());
    assert_eq!(first["workflow"], json!("domain_report_project"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["verification"]["method"],
        json!("domain_report_projection")
    );
    let second = call(&mut server, "domain_report_project", arguments);
    assert_eq!(
        first["artifact_registry"]["content_digest"],
        second["artifact_registry"]["content_digest"]
    );
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let coverage = call(
        &mut server,
        "domain_report_project",
        json!({"operation": "coverage", "include_report_digests": true}),
    );
    assert_eq!(coverage["workflow"], json!("domain_report_coverage"));
    assert_eq!(coverage["group_count"], json!(CAPABILITY_GROUP_COUNT));
    assert_eq!(coverage["reported_group_count"], json!(1));
    assert_eq!(
        coverage["missing_group_count"],
        json!(CAPABILITY_GROUP_COUNT - 1)
    );
    assert_eq!(coverage["complete"], json!(false));
    assert_eq!(coverage["readiness_claimed"], json!(false));
    assert_eq!(
        coverage["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap()["report_classes"]["ordinary"],
        json!(1)
    );
    assert_eq!(
        coverage["bridge_summary"]["lineage"]["reports_without_lineage_parents"],
        json!(1)
    );
    let filtered = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "coverage",
            "group_id": "biological_domains",
            "report_class": "ordinary",
            "bridge_mode": "inline"
        }),
    );
    assert_eq!(filtered["filters"]["report_class"], json!("ordinary"));
    assert_eq!(filtered["filters"]["bridge_mode"], json!("inline"));
    assert_eq!(filtered["reported_group_count"], json!(0));
    assert_eq!(filtered["missing_group_count"], json!(1));
    assert_eq!(coverage["coverage_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn domain_report_projection_refuses_unknown_source_and_domain_claims() {
    let mut server = server();
    let refused = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "domain-report-invalid",
            "source_tool": "not_a_declared_tool",
            "report": {"status": "review_required"},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert!(refused["error"].as_str().unwrap().contains("not declared"));

    let domain_refused = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["not_declared"],
            "subject_id": "domain-report-invalid-domain",
            "source_tool": "modality_catalog",
            "report": {"status": "review_required"},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(domain_refused["ok"], json!(false));
    assert!(
        domain_refused["error"]
            .as_str()
            .unwrap()
            .contains("not declared")
    );
}

#[test]
fn adapter_domain_report_operation_validates_and_joins_adapter_evidence() {
    let mut server = server();
    let result = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_adapter_execution",
            "evidence": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "adapter-domain-report-subject",
                "adapter_id": "bioprism.python.vcf_text",
                "adapter_version": "0.1.0",
                "source_id": "adapter-domain-report-source",
                "input_digest": "a".repeat(64),
                "output_digest": "b".repeat(64),
                "execution_status": "succeeded",
                "conformance_status": "verified",
                "semantic_loss_status": "unknown",
                "losses": [],
                "parent_digests": ["c".repeat(64)]
            },
            "conformance": {"status": "verified", "report_digest": "d".repeat(64)}
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-devplat-adapter-domain-report/0.1")
    );
    assert_eq!(result["workflow"], json!("adapter_domain_report"));
    assert_eq!(
        result["evidence"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(
        result["domain_report"]["workflow"],
        json!("domain_report_project")
    );
    assert_eq!(
        result["domain_report"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(
        result["domain_report"]["report"]["claim_posture"]["status"],
        json!("observed")
    );
    assert!(
        result["domain_report"]["report"]["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == &result["evidence"]["artifact_registry"]["content_digest"])
    );
    assert_eq!(result["readiness_claimed"], json!(false));
}

#[test]
fn provider_domain_report_operations_compose_inline_and_external_normalization() {
    let mut server = server();
    let payload = json!({"records": [{"id": "pmid:1", "title": "opaque"}]});
    let inline = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_provider_normalization",
            "normalization": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "provider-domain-report",
                "source_tool": "literature_bind_check",
                "connector_kind": "literature",
                "provider": "pubmed",
                "payload": payload,
                "outcome": "observed",
                "parent_digests": ["a".repeat(64)]
            }
        }),
    );
    assert_eq!(inline["ok"], json!(true));
    assert_eq!(inline["mode"], json!("inline"));
    assert_eq!(inline["workflow"], json!("provider_domain_report"));
    assert_eq!(
        inline["domain_report"]["report"]["source_tool"],
        json!("domain_evidence_provider_normalize")
    );
    assert_eq!(
        inline["domain_report"]["report"]["claim_posture"]["status"],
        json!("observed")
    );
    assert_eq!(
        inline["domain_report"]["report"]["report"]["kind"],
        json!("provider_normalization")
    );
    assert_eq!(
        inline["domain_report"]["report"]["report"]["payload_digest"],
        inline["normalization"]["payload_digest"]
    );
    assert!(inline["domain_report"]["report"]["parent_digests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|parent| parent == &inline["normalization"]["artifact_registry"]["content_digest"]));
    assert_eq!(inline["readiness_claimed"], json!(false));

    let payload_digest = ContentHash::of_value(&payload).unwrap().to_string();
    let byte_length = serde_json::to_vec(&payload).unwrap().len() as u64;
    let external = call(
        &mut server,
        "domain_report_project",
        json!({
            "operation": "from_external_provider_normalization",
            "normalization": {
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "external-provider-domain-report",
                "source_tool": "literature_bind_check",
                "provider": "pubmed",
                "connector_kind": "literature",
                "handoff_digest": "b".repeat(64),
                "transfer_id": "provider-domain-report-transfer",
                "payload_digest": payload_digest,
                "byte_length": byte_length,
                "storage_backend": "object_store",
                "locator_kind": "opaque",
                "locator": "store://caller/pubmed/provider-domain-report",
                "availability": "available",
                "retention": "durable",
                "payload": payload,
                "outcome": "observed"
            }
        }),
    );
    assert_eq!(external["ok"], json!(true));
    assert_eq!(external["mode"], json!("external_payload"));
    assert_eq!(
        external["domain_report"]["report"]["source_tool"],
        json!("domain_evidence_provider_external_payload_normalize")
    );
    assert_eq!(
        external["domain_report"]["report"]["report"]["materialization"]["locator_opened"],
        json!(false)
    );
    assert!(
        external["domain_report"]["report"]["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent
                == &external["normalization"]["receipt_artifact_registry"]["content_digest"])
    );
    assert_eq!(external["readiness_claimed"], json!(false));
}

#[test]
fn domain_evidence_harmonization_indexes_traceability_idempotently() {
    let mut server = server();
    let first = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "harmonization-subject",
            "source_tool": "modality_catalog",
            "report": {"observations": ["modality contract retained"]},
            "claim_posture": {
                "status": "observed",
                "does_not_claim": ["clinical validity"]
            }
        }),
    );
    let second = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_ir_and_query",
            "domains": ["BioQL syntax"],
            "subject_id": "harmonization-subject",
            "source_tool": "bioql_compile",
            "report": {"observations": ["query syntax contract retained"]},
            "claim_posture": {
                "status": "review_required",
                "does_not_claim": ["query execution", "biological truth"],
                "limitations": ["no source dataset supplied"]
            }
        }),
    );
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(second["artifact_registry"]["indexed"], json!(true));

    let arguments = json!({
        "subject_id": "harmonization-subject",
        "claim": {"id": "claim-opaque-1", "statement": "caller-owned claim"},
        "reports": [first["report"].clone(), second["report"].clone()],
        "links": [
            {"report_index": 0, "role": "supports"},
            {"report_index": 1, "role": "qualifies", "note": "syntax coverage is not execution"}
        ],
        "required_group_ids": ["biological_domains", "biological_ir_and_query"],
        "required_domains": ["modalities", "BioQL syntax"]
    });
    let harmonized = call(&mut server, "domain_evidence_harmonize", arguments.clone());
    assert_eq!(harmonized["workflow"], json!("domain_evidence_harmonize"));
    assert_eq!(
        harmonized["harmonization"]["coverage"]["traceability_state"],
        json!("complete")
    );
    assert_eq!(
        harmonized["harmonization"]["coverage"]["all_reports_linked"],
        json!(true)
    );
    assert_eq!(
        harmonized["harmonization"]["posture"]["qualification_link_count"],
        json!(1)
    );
    assert_eq!(harmonized["readiness_claimed"], json!(false));
    assert_eq!(harmonized["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        harmonized["artifact_registry"]["verification"]["method"],
        json!("domain_evidence_harmonization")
    );

    let coverage = call(
        &mut server,
        "domain_evidence_harmonization_coverage",
        json!({
            "subject_id": "harmonization-subject",
            "domain": "modalities",
            "traceability_state": "complete",
            "include_report_digests": true
        }),
    );
    assert_eq!(
        coverage["workflow"],
        json!("domain_evidence_harmonization_coverage")
    );
    assert_eq!(coverage["matching_count"], json!(1));
    assert_eq!(coverage["returned_count"], json!(1));
    assert_eq!(coverage["rows"][0]["report_count"], json!(2));
    assert_eq!(coverage["rows"][0]["link_count"], json!(2));
    assert_eq!(
        coverage["rows"][0]["report_digests"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        coverage["summary"]["domain_summary"]["modalities"]["report_count"],
        json!(1)
    );
    let invalid_cursor = call(
        &mut server,
        "domain_evidence_harmonization_coverage",
        json!({"after": "not-a-digest"}),
    );
    assert_eq!(invalid_cursor["__isError"], json!(true));

    let replay = call(&mut server, "domain_evidence_harmonize", arguments);
    assert_eq!(
        harmonized["artifact_registry"]["content_digest"],
        replay["artifact_registry"]["content_digest"]
    );
    assert_eq!(replay["artifact_registry"]["already_present"], json!(true));
}

#[test]
fn domain_evidence_harmonization_refuses_subject_or_catalogue_mismatch() {
    let mut server = server();
    let report = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "subject-a",
            "source_tool": "modality_catalog",
            "report": {"observations": ["bounded"]},
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    )["report"]
        .clone();
    let mismatched = call(
        &mut server,
        "domain_evidence_harmonize",
        json!({
            "subject_id": "subject-b",
            "claim": {"id": "claim-mismatch"},
            "reports": [report],
            "links": [{"report_index": 0, "role": "context"}]
        }),
    );
    assert_eq!(mismatched["__isError"], json!(true));
    assert!(mismatched["error"].as_str().unwrap().contains("subject"));

    let invalid_catalogue = call(
        &mut server,
        "domain_evidence_harmonize",
        json!({
            "subject_id": "subject-a",
            "claim": {"id": "claim-catalogue"},
            "reports": [{
                "schema": "bioprism-devplat-domain-report/0.1",
                "workflow": "domain_report_project",
                "subject_id": "subject-a",
                "group_id": "biological_domains",
                "source_tool": "modality_catalog",
                "domains": ["not-declared"],
                "report": {},
                "claim_posture": {"status": "observed", "does_not_claim": ["truth"]},
                "parent_digests": [],
                "non_claims": ["truth"]
            }],
            "links": [{"report_index": 0, "role": "context"}]
        }),
    );
    assert_eq!(invalid_catalogue["__isError"], json!(true));
}

#[test]
fn domain_evidence_intake_accepts_one_declared_envelope_from_every_capability_group() {
    let mut server = server();
    let catalogue = call(&mut server, "workspace_capabilities", json!({}));
    let groups = catalogue
        .as_array()
        .expect("workspace catalogue is an array");
    assert_eq!(groups.len(), CAPABILITY_GROUP_COUNT);
    for group in groups {
        let group_id = group["id"].as_str().expect("group id");
        let source_tool = group["mcp_tools"][0].as_str().expect("source tool");
        let domain = group["domains"][0].as_str().expect("domain");
        let result = call(
            &mut server,
            "domain_evidence_intake",
            json!({
                "group_id": group_id,
                "domains": [domain],
                "subject_id": format!("all-domain-{group_id}"),
                "source_tool": source_tool,
                "request": {"probe": "retained"},
                "response": {"group_id": group_id, "source_tool": source_tool, "status": "bounded"},
                "outcome": "observed",
                "claim_posture": {
                    "status": "observed",
                    "does_not_claim": ["scientific truth", "execution completion"]
                }
            }),
        );
        assert_eq!(result["__isError"], json!(false), "group={group_id}");
        assert_eq!(result["workflow"], json!("domain_evidence_intake"));
        assert_eq!(result["group_id"], json!(group_id));
        assert_eq!(result["artifact_registry"]["indexed"], json!(true));
        assert_eq!(result["report"]["group_id"], json!(group_id));
        assert_eq!(result["report"]["source_tool"], json!(source_tool));
        assert_eq!(result["report"]["domains"], json!([domain]));
    }
}

#[test]
fn domain_evidence_intake_replays_idempotently_and_refuses_catalogue_mismatch() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "intake-replay",
        "source_tool": "modality_catalog",
        "response": {"status": "refused", "reason": "caller withheld execution"},
        "outcome": "refused",
        "claim_posture": {"status": "refused", "does_not_claim": ["execution", "truth"]}
    });
    let first = call(&mut server, "domain_evidence_intake", arguments.clone());
    let second = call(&mut server, "domain_evidence_intake", arguments);
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["content_digest"],
        second["artifact_registry"]["content_digest"]
    );
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    assert_eq!(first["outcome"], json!("refused"));
    assert_eq!(first["request_supplied"], json!(false));

    let invalid = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["not-declared"],
            "subject_id": "intake-invalid",
            "source_tool": "modality_catalog",
            "response": {},
            "outcome": "unknown",
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(invalid["__isError"], json!(true));
    assert!(invalid["error"].as_str().unwrap().contains("not declared"));
}

#[test]
fn domain_evidence_coverage_preserves_missing_groups_outcomes_and_digest_rows() {
    let mut server = server();
    let intake = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "coverage-subject",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "partial",
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(intake["artifact_registry"]["indexed"], json!(true));
    let coverage = call(
        &mut server,
        "domain_evidence_coverage",
        json!({
            "include_intake_digests": true
        }),
    );
    assert_eq!(
        coverage["workflow"],
        json!("domain_evidence_intake_coverage")
    );
    assert_eq!(coverage["group_count"], json!(CAPABILITY_GROUP_COUNT));
    assert_eq!(coverage["reported_group_count"], json!(1));
    assert_eq!(
        coverage["missing_group_count"],
        json!(CAPABILITY_GROUP_COUNT - 1)
    );
    assert_eq!(coverage["complete"], json!(false));
    assert_eq!(coverage["tool_coverage_complete"], json!(false));
    assert_eq!(coverage["domain_coverage_complete"], json!(false));
    assert_eq!(coverage["groups_with_artifact_evidence"], json!(1));
    assert_eq!(coverage["artifact_evidence_records"], json!(1));
    assert_eq!(
        coverage["artifact_evidence_scope"],
        json!("current_digest_verified_artifact_registry_exact_declared_matches")
    );
    assert_eq!(
        coverage["domain_summary"]["modalities"]["intake_count"],
        json!(1)
    );
    let group = coverage["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["id"] == "biological_domains")
        .unwrap();
    assert_eq!(group["outcomes"], json!(["partial"]));
    assert!(group["declared_tools"].as_array().unwrap().len() > 1);
    assert!(
        group["missing_source_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "bioworlds_catalog")
    );
    assert_eq!(group["tool_coverage_state"], json!("partial"));
    assert_eq!(group["domain_coverage_state"], json!("partial"));
    assert_eq!(group["artifact_evidence"]["state"], json!("observed"));
    assert_eq!(
        group["artifact_evidence"]["matching_record_count"],
        json!(1)
    );
    assert_eq!(
        group["artifact_evidence"]["family_counts"]["source_or_harmonization"],
        json!(1)
    );
    assert_eq!(
        group["source_tool_coverage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["tool"] == "modality_catalog")
            .unwrap()["intake_count"],
        json!(1)
    );
    assert_eq!(group["intake_digests"].as_array().unwrap().len(), 1);
    let filtered = call(
        &mut server,
        "domain_evidence_coverage",
        json!({"group_id": "biological_domains", "domain": "MODALITIES"}),
    );
    assert_eq!(filtered["group_count"], json!(1));
    assert_eq!(filtered["reported_group_count"], json!(1));
    assert_eq!(filtered["complete"], json!(true));
    assert_eq!(filtered["tool_coverage_complete"], json!(false));
    assert_eq!(filtered["domain_coverage_complete"], json!(false));
}

#[test]
fn domain_evidence_source_plan_is_catalogue_bound_digest_addressed_and_non_executing() {
    let mut server = server();
    let arguments = json!({
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "source-plan-subject",
        "source_tool": "modality_catalog",
        "connector_kind": "literature",
        "locator_kind": "uri",
        "locator": "https://example.org/article/1",
        "retrieval_mode": "metadata_only",
        "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
        "does_not_claim": ["retrieval occurred", "source is true"]
    });
    let first = call(
        &mut server,
        "domain_evidence_source_plan",
        arguments.clone(),
    );
    assert_eq!(first["workflow"], json!("domain_evidence_source_plan"));
    assert_eq!(first["retrieval_status"], json!("not_started"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first["artifact_registry"]["verification"]["method"],
        json!("domain_evidence_source_plan")
    );
    assert_eq!(first["plan_digest"].as_str().unwrap().len(), 64);
    let bound_intake = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-subject",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "observed",
            "source_plan_digest": first["plan_digest"].clone(),
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(bound_intake["source_plan_digest"], first["plan_digest"]);
    assert!(
        bound_intake["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|digest| digest == &first["plan_digest"])
    );
    let mut expected_arguments = arguments.clone();
    expected_arguments["subject_id"] = json!("source-plan-expected");
    expected_arguments["expected_content_digest"] = json!("a".repeat(64));
    let expected_plan = call(
        &mut server,
        "domain_evidence_source_plan",
        expected_arguments,
    );
    let mismatch = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-expected",
            "source_tool": "modality_catalog",
            "response": {"status": "bounded"},
            "outcome": "observed",
            "source_plan_digest": expected_plan["plan_digest"].clone(),
            "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
        }),
    );
    assert_eq!(mismatch["__isError"], json!(true));
    assert!(
        mismatch["error"]
            .as_str()
            .unwrap()
            .contains("response digest differs")
    );
    let second = call(&mut server, "domain_evidence_source_plan", arguments);
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    let credential_refused = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-plan-refused",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": "https://user:secret@example.org/evidence",
            "retrieval_mode": "content",
            "does_not_claim": ["retrieval occurred"]
        }),
    );
    assert_eq!(credential_refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_source_execute_reads_confined_file_and_retains_raw_and_json_digests() {
    let mut server = server();
    let planned = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-subject",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "fixtures/fiber-v0.1/leakage_query.json",
            "retrieval_mode": "content",
            "retrieval_policy": {"network": "disabled", "max_bytes": 65536, "cache": "content_addressed"},
            "does_not_claim": ["source truth", "scientific validity"]
        }),
    );
    let executed = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": planned["plan_digest"].clone()}),
    );
    assert_eq!(
        executed["workflow"],
        json!("domain_evidence_source_execute")
    );
    assert_eq!(executed["outcome"], json!("observed"));
    assert_eq!(
        executed["intake"]["workflow"],
        json!("domain_evidence_intake")
    );
    assert_eq!(
        executed["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert_eq!(executed["raw_content_digest"].as_str().unwrap().len(), 64);
    assert_eq!(executed["response_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        executed["execution_result"]["response"]["retrieval"]["body_encoding"],
        json!("json")
    );
    assert!(
        executed["intake"]["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|digest| digest == &planned["artifact_registry"]["content_digest"])
    );
    let repeated = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": planned["plan_digest"].clone()}),
    );
    assert_eq!(repeated["outcome"], json!("observed"));
    assert_eq!(
        repeated["intake"]["artifact_registry"]["already_present"],
        json!(true)
    );

    let traversal_plan = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-refused",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "../outside.json",
            "retrieval_mode": "content",
            "does_not_claim": ["source truth"]
        }),
    );
    let refused = call(
        &mut server,
        "domain_evidence_source_execute",
        json!({"source_plan_digest": traversal_plan["plan_digest"].clone()}),
    );
    assert_eq!(refused["outcome"], json!("refused"));
    assert_eq!(refused["intake"]["outcome"], json!("refused"));
    assert_eq!(
        refused["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
}

#[test]
fn domain_evidence_provider_normalize_retains_caller_managed_payload_with_explicit_digests() {
    let mut server = server();
    let planned = call(
        &mut server,
        "domain_evidence_source_plan",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "locator_kind": "opaque",
            "locator": "caller://pubmed/query/oncology",
            "retrieval_mode": "reference_only",
            "does_not_claim": ["provider authenticity", "clinical truth"]
        }),
    );
    let normalized = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "pubmed",
            "payload": {"records": [{"id": "pmid:1", "title": "opaque"}]},
            "request": {"query": "oncology"},
            "outcome": "observed",
            "source_plan_digest": planned["plan_digest"].clone()
        }),
    );
    assert_eq!(
        normalized["workflow"],
        json!("domain_evidence_provider_normalize")
    );
    assert_eq!(normalized["connector_kind"], json!("literature"));
    assert_eq!(normalized["provider"], json!("pubmed"));
    assert_eq!(normalized["payload_digest"].as_str().unwrap().len(), 64);
    assert_eq!(normalized["shape_audit"]["status"], json!("structured"));
    assert_eq!(
        normalized["shape_audit"]["recognized_container"],
        json!("records")
    );
    assert_eq!(
        normalized["shape_audit"]["identifier_coverage"]["present_record_count"],
        json!(1)
    );
    assert_eq!(normalized["record_index"]["indexed_record_count"], json!(1));
    assert_eq!(normalized["record_index"]["omitted_record_count"], json!(0));
    assert_eq!(normalized["intake"]["outcome"], json!("observed"));
    assert_eq!(
        normalized["intake"]["artifact_registry"]["indexed"],
        json!(true)
    );
    assert!(
        normalized["intake"]["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|digest| digest == &planned["artifact_registry"]["content_digest"])
    );

    let unknown = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-unknown",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "caller",
            "payload": {"records": []}
        }),
    );
    assert_eq!(unknown["outcome"], json!("unknown"));
    assert_eq!(unknown["intake"]["outcome"], json!("unknown"));
    assert_eq!(unknown["shape_audit"]["status"], json!("structured"));

    let fhir = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-fhir",
            "source_tool": "literature_bind_check",
            "connector_kind": "fhir",
            "provider": "caller",
            "payload": {"resourceType": "Bundle", "entry": [{"resource": {"resourceType": "Patient", "id": "opaque"}}]}
        }),
    );
    assert_eq!(fhir["shape_audit"]["recognized_container"], json!("entry"));
    assert_eq!(fhir["shape_audit"]["status"], json!("structured"));

    let object_store = call(
        &mut server,
        "domain_evidence_provider_normalize",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-object-store",
            "source_tool": "literature_bind_check",
            "connector_kind": "object_store",
            "provider": "caller",
            "payload": {"objects": [{"key": "opaque", "content_digest": "opaque"}]}
        }),
    );
    assert_eq!(object_store["shape_audit"]["status"], json!("structured"));
    assert_eq!(
        object_store["shape_audit"]["content_digest_coverage"]["present_record_count"],
        json!(1)
    );

    let replay = call(
        &mut server,
        "domain_evidence_provider_replay_verify",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-subject",
            "source_tool": "literature_bind_check",
            "connector_kind": "literature",
            "provider": "pubmed",
            "payload": {"records": [{"id": "pmid:1", "title": "opaque"}]},
            "request": {"query": "oncology"},
            "outcome": "observed",
            "parent_digests": normalized["intake"]["parent_digests"].clone(),
            "source_plan_digest": planned["plan_digest"].clone(),
            "expected_payload_digest": normalized["payload_digest"].clone(),
            "expected_request_digest": normalized["request_digest"].clone(),
            "expected_shape_digest": normalized["shape_audit"]["shape_digest"].clone(),
            "expected_normalization_digest": ContentHash::of_value(&normalized["normalization"])
                .unwrap()
                .to_string(),
            "expected_intake_digest": normalized["intake"]["intake_digest"].clone()
        }),
    );
    assert_eq!(replay["replay_status"], json!("matched"));
    assert_eq!(replay["matched"], json!(true));
    assert_eq!(replay["replay"]["differences"], json!([]));
    assert_eq!(replay["artifact_registry"]["created"], json!(true));
    assert_eq!(replay["replay_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn domain_evidence_provider_connector_handoff_is_scoped_secret_safe_and_idempotent() {
    let mut server = server();
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology", "genomics"],
        "subject_id": "connector-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "manifest": {
            "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
            "connector_id": "caller.pubmed",
            "version": "1.2.0",
            "provider": "pubmed",
            "connector_kind": "literature",
            "domains": ["genomics", "oncology"],
            "capabilities": ["query", "retain"],
            "transport": "caller_managed",
            "auth_posture": {
                "status": "caller_asserted",
                "secret_refs": ["secret://caller/pubmed"],
                "does_not_claim": ["provider authentication"]
            }
        },
        "status": "prepared",
        "request_digest": "a".repeat(64),
        "payload_digest": "b".repeat(64),
        "source_plan_digest": "c".repeat(64),
        "parent_digests": ["d".repeat(64)],
        "attempt_id": "attempt-1"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_connector_handoff")
    );
    assert_eq!(first["execution"], json!("not_started"));
    assert_eq!(first["readiness_claimed"], json!(false));
    assert_eq!(first["handoff"]["status"], json!("prepared"));
    assert_eq!(
        first["handoff"]["manifest"]["auth_posture"]["secret_refs"][0],
        json!("secret://caller/pubmed")
    );
    assert_eq!(first["handoff_digest"].as_str().unwrap().len(), 64);
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    let second = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        request,
    );
    assert_eq!(second["handoff_digest"], first["handoff_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));
    assert!(
        !serde_json::to_string(&first["handoff"])
            .unwrap()
            .contains("credential_material")
    );

    let refused = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "connector-refused",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "manifest": {
                "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
                "connector_id": "caller.pubmed",
                "version": "1.2.0",
                "provider": "pubmed",
                "connector_kind": "literature",
                "domains": ["oncology"],
                "capabilities": ["query"],
                "transport": "caller_managed",
                "auth_posture": {"status": "unknown", "does_not_claim": ["auth"]}
            },
            "credential_material": "must-refuse"
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_receipt_is_out_of_line_and_restart_safe() {
    let mut server = server();
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-2026-08-17-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/1",
        "content_type": "application/json",
        "content_encoding": "gzip",
        "request_digest": "c".repeat(64),
        "parent_digests": ["d".repeat(64)],
        "availability": "available",
        "retention": "durable",
        "attempt_id": "attempt-1"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_external_payload_receipt")
    );
    assert_eq!(first["receipt"]["byte_length"], json!(4096));
    assert_eq!(first["receipt"]["retention"], json!("durable"));
    assert_eq!(first["receipt"]["readiness_claimed"], json!(false));
    assert_eq!(first["receipt"]["execution"], json!("not_started"));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["receipt_digest"].as_str().unwrap().len(), 64);
    assert_eq!(first["receipt"]["handoff_digest"], json!("a".repeat(64)));
    assert!(
        !serde_json::to_string(&first)
            .unwrap()
            .contains("credential_material")
    );

    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        request,
    );
    assert_eq!(second["receipt_digest"], first["receipt_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let refused = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "external-provider-refused",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "handoff_digest": "a".repeat(64),
            "transfer_id": "export-2",
            "payload_digest": "b".repeat(64),
            "byte_length": 1,
            "storage_backend": "object_store",
            "locator_kind": "uri",
            "locator": "https://user:pass@example.org/object",
            "credential_material": "never"
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_replay_is_metadata_only_and_idempotent() {
    let mut server = server();
    let receipt = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-2026-08-17-replay-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/1",
        "availability": "available",
        "retention": "durable"
    });
    let recorded = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        receipt.clone(),
    );
    let replay_request = json!({
        "expected_receipt_digest": recorded["receipt_digest"],
        "expected_handoff_digest": "a".repeat(64),
        "expected_payload_digest": "b".repeat(64),
        "expected_byte_length": 4096,
        "group_id": receipt["group_id"],
        "domains": receipt["domains"],
        "subject_id": receipt["subject_id"],
        "source_tool": receipt["source_tool"],
        "provider": receipt["provider"],
        "connector_kind": receipt["connector_kind"],
        "handoff_digest": receipt["handoff_digest"],
        "transfer_id": receipt["transfer_id"],
        "payload_digest": receipt["payload_digest"],
        "byte_length": receipt["byte_length"],
        "storage_backend": receipt["storage_backend"],
        "locator_kind": receipt["locator_kind"],
        "locator": receipt["locator"],
        "availability": receipt["availability"],
        "retention": receipt["retention"]
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        replay_request.clone(),
    );
    assert_eq!(first["replay_status"], json!("matched"));
    assert_eq!(first["matched"], json!(true));
    assert_eq!(first["replay"]["matches"]["receipt_digest"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert!(first["replay"]["receipt"].get("records").is_none());
    assert!(
        first["replay"]["receipt"]
            .get("credential_material")
            .is_none()
    );
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        replay_request,
    );
    assert_eq!(second["replay_digest"], first["replay_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let mut mismatch_request = receipt;
    mismatch_request["byte_length"] = json!(8192);
    mismatch_request["expected_receipt_digest"] =
        first["replay"]["expected_receipt_digest"].clone();
    mismatch_request["expected_handoff_digest"] = json!("a".repeat(64));
    mismatch_request["expected_payload_digest"] = json!("b".repeat(64));
    mismatch_request["expected_byte_length"] = json!(4096);
    let mismatch = call(
        &mut server,
        "domain_evidence_provider_external_payload_replay_verify",
        mismatch_request,
    );
    assert_eq!(mismatch["replay_status"], json!("mismatch"));
    assert_eq!(mismatch["matched"], json!(false));
    assert_eq!(
        mismatch["replay"]["differences"],
        json!(["byte_length", "receipt_digest"])
    );
}

#[test]
fn domain_evidence_provider_external_payload_normalize_requires_digest_verified_materialization() {
    let mut server = server();
    let payload = json!({"records": [{"id": "pmid:1", "title": "opaque"}]});
    let payload_digest = ContentHash::of_value(&payload).unwrap().to_string();
    let byte_length = serde_json::to_vec(&payload).unwrap().len() as u64;
    let request = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "external-provider-materialized",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "export-materialized-1",
        "payload_digest": payload_digest,
        "byte_length": byte_length,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/materialized-1",
        "availability": "available",
        "retention": "durable",
        "payload": payload,
        "outcome": "observed"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        request.clone(),
    );
    assert_eq!(
        first["workflow"],
        json!("domain_evidence_provider_external_payload_normalize")
    );
    assert_eq!(first["materialization"]["matched"], json!(true));
    assert_eq!(first["materialization"]["locator_opened"], json!(false));
    assert_eq!(
        first["normalization"]["payload_digest"],
        first["payload_digest"]
    );
    assert_eq!(first["normalization"]["outcome"], json!("observed"));
    assert_eq!(first["receipt_artifact_registry"]["created"], json!(true));
    assert_eq!(first["artifact_registry"]["indexed"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));

    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        request.clone(),
    );
    assert_eq!(second["receipt_digest"], first["receipt_digest"]);
    assert_eq!(second["receipt_artifact_registry"]["created"], json!(false));

    let mut drift = request;
    drift["payload"] = json!({"records": [{"id": "pmid:drift"}]});
    let refused = call(
        &mut server,
        "domain_evidence_provider_external_payload_normalize",
        drift,
    );
    assert_eq!(refused["__isError"], json!(true));
}

#[test]
fn domain_evidence_provider_external_payload_lineage_audit_reconciles_handoff_scope_and_payload() {
    let mut server = server();
    let handoff = call(
        &mut server,
        "domain_evidence_provider_connector_handoff",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "lineage-subject",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "manifest": {
                "schema": "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
                "connector_id": "caller.pubmed",
                "version": "1.2.0",
                "provider": "pubmed",
                "connector_kind": "literature",
                "domains": ["oncology"],
                "capabilities": ["query", "retain"],
                "transport": "caller_managed",
                "auth_posture": {
                    "status": "caller_asserted",
                    "secret_refs": ["secret://caller/pubmed"],
                    "does_not_claim": ["provider authentication"]
                }
            },
            "status": "prepared",
            "payload_digest": "b".repeat(64)
        }),
    );
    let receipt = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "lineage-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": handoff["handoff_digest"].clone(),
        "transfer_id": "transfer-lineage-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/objects/lineage-1",
        "availability": "available",
        "retention": "durable"
    });
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        receipt.clone(),
    );
    assert_eq!(first["lineage_status"], json!("matched"));
    assert_eq!(first["payload_binding_status"], json!("matched"));
    assert_eq!(first["audit"]["matches"]["payload_digest"], json!(true));
    assert_eq!(first["receipt_registry"]["created"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        receipt,
    );
    assert_eq!(second["lineage_digest"], first["lineage_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let orphaned = call(
        &mut server,
        "domain_evidence_provider_external_payload_lineage_audit",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "orphaned-lineage-subject",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "handoff_digest": "c".repeat(64),
            "transfer_id": "transfer-lineage-orphan",
            "payload_digest": "d".repeat(64),
            "byte_length": 1,
            "storage_backend": "caller_managed",
            "locator_kind": "opaque",
            "locator": "caller://orphaned",
            "availability": "unknown",
            "retention": "unknown"
        }),
    );
    assert_eq!(orphaned["lineage_status"], json!("orphaned"));
    assert_eq!(orphaned["payload_binding_status"], json!("not_available"));
    assert_eq!(orphaned["readiness_claimed"], json!(false));
}

#[test]
fn domain_evidence_provider_external_payload_execution_evidence_is_observation_bound_and_idempotent()
 {
    let mut server = server();
    let base = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "execution-evidence-subject",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "transfer-execution-evidence-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/pubmed/execution-evidence-1",
        "availability": "available",
        "retention": "durable"
    });
    let receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        base.clone(),
    );
    let mut matched_request = base.clone();
    matched_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    matched_request["execution_status"] = json!("transferred");
    matched_request["executor_id"] = json!("caller-transfer-worker");
    matched_request["observed_payload_digest"] = json!("b".repeat(64));
    matched_request["observed_byte_length"] = json!(4096);
    matched_request["locator_opened"] = json!(true);
    matched_request["observation_digest"] = json!("c".repeat(64));
    let first = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        matched_request.clone(),
    );
    assert_eq!(first["evidence_status"], json!("matched"));
    assert_eq!(
        first["evidence"]["matches"]["observed_payload_digest"],
        json!(true)
    );
    assert_eq!(first["receipt_registry"]["already_present"], json!(true));
    assert_eq!(first["artifact_registry"]["created"], json!(true));
    assert_eq!(first["readiness_claimed"], json!(false));
    let second = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        matched_request,
    );
    assert_eq!(second["evidence_digest"], first["evidence_digest"]);
    assert_eq!(second["artifact_registry"]["created"], json!(false));
    assert_eq!(second["artifact_registry"]["already_present"], json!(true));

    let mut partial_request = base.clone();
    partial_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    partial_request["execution_status"] = json!("transferred");
    partial_request["executor_id"] = json!("caller-transfer-worker");
    partial_request["observed_payload_digest"] = json!("b".repeat(64));
    let partial = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        partial_request,
    );
    assert_eq!(partial["evidence_status"], json!("partial"));

    let mut mismatch_request = base.clone();
    mismatch_request["expected_receipt_digest"] = receipt["receipt_digest"].clone();
    mismatch_request["execution_status"] = json!("transferred");
    mismatch_request["executor_id"] = json!("caller-transfer-worker");
    mismatch_request["observed_payload_digest"] = json!("d".repeat(64));
    mismatch_request["observed_byte_length"] = json!(4096);
    let mismatch = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        mismatch_request,
    );
    assert_eq!(mismatch["evidence_status"], json!("mismatch"));

    let mut orphaned_request = base;
    orphaned_request["expected_receipt_digest"] = json!("e".repeat(64));
    orphaned_request["execution_status"] = json!("unknown");
    orphaned_request["executor_id"] = json!("caller-transfer-worker");
    let orphaned = call(
        &mut server,
        "domain_evidence_provider_external_payload_execution_evidence",
        orphaned_request,
    );
    assert_eq!(orphaned["evidence_status"], json!("orphaned"));
}

#[test]
fn domain_evidence_provider_external_payload_evidence_query_joins_rows_and_paginates_deterministically()
 {
    let mut server = server();
    let first = json!({
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "query-subject-1",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "a".repeat(64),
        "transfer_id": "query-transfer-1",
        "payload_digest": "b".repeat(64),
        "byte_length": 4096,
        "storage_backend": "object_store",
        "locator_kind": "opaque",
        "locator": "store://caller/query/1",
        "availability": "available",
        "retention": "durable"
    });
    let second = json!({
        "group_id": "biological_domains",
        "domains": ["genomics"],
        "subject_id": "query-subject-2",
        "source_tool": "literature_bind_check",
        "provider": "pubmed",
        "connector_kind": "literature",
        "handoff_digest": "c".repeat(64),
        "transfer_id": "query-transfer-2",
        "payload_digest": "d".repeat(64),
        "byte_length": 2048,
        "storage_backend": "caller_managed",
        "locator_kind": "opaque",
        "locator": "caller://query/2",
        "availability": "unknown",
        "retention": "unknown"
    });
    let first_receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        first,
    );
    let _second_receipt = call(
        &mut server,
        "domain_evidence_provider_external_payload_receipt",
        second,
    );
    let page = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"max_items": 1, "include_artifacts": true}),
    );
    assert_eq!(page["ok"], json!(true));
    assert_eq!(
        page["workflow"],
        json!("domain_evidence_provider_external_payload_evidence_query")
    );
    assert_eq!(page["rows"].as_array().unwrap().len(), 1);
    assert_eq!(page["rows"][0]["join_status"], json!("receipt_only"));
    assert!(page["rows"][0].get("receipt_artifact").is_some());
    assert_eq!(page["has_more"], json!(true));
    let next_after = page["next_after"].clone();
    assert_ne!(next_after, Value::Null);
    let next = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"after": next_after, "max_items": 2}),
    );
    assert_eq!(next["ok"], json!(true));
    assert_eq!(next["rows"].as_array().unwrap().len(), 1);
    assert_eq!(next["has_more"], json!(false));
    let filtered = call(
        &mut server,
        "domain_evidence_provider_external_payload_evidence_query",
        json!({"subject_id": "query-subject-1"}),
    );
    assert_eq!(filtered["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        filtered["rows"][0]["receipt_digest"],
        first_receipt["receipt_digest"]
    );
    assert_eq!(filtered["readiness_claimed"], json!(false));
}
