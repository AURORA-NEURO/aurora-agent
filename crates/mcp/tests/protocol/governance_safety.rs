//! MCP contract tests for governance safety contracts.

use super::*;

#[test]
fn sdk_registry_check_refuses_invalid_manifests_before_resolution() {
    let mut server = server();
    let manifest = PluginManifest::new("schema-oracle", "0.4.0", "fixture")
        .speaking(RegistryPolicy::workspace_versions())
        .providing(Capability::new(CapabilityKind::Oracle).at(Priority(10)))
        .claiming(Determinism::Deterministic)
        .at_grade(AbiGrade::P1);
    let mut invalid = serde_json::to_value(&manifest).unwrap();
    invalid["capabilities"] = json!([]);
    let payload = call(
        &mut server,
        "sdk_registry_check",
        json!({
            "manifests": [
                serde_json::to_value(manifest).unwrap(),
                invalid
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(false));
    assert_eq!(payload["stage"], json!("manifest_validation"));
    assert_eq!(payload["registry"], Value::Null);
    assert!(
        payload["manifests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| { row["valid"] == json!(false) })
    );
}

#[test]
fn hub_submission_review_replays_append_only_public_moderation() {
    let mut server = server();
    let submitter =
        Submitter::unverified(SubmitterId::parse("lab-a").unwrap()).declaring_no_conflicts();
    let draft = SubmissionDraft {
        id: Some(SubmissionId::parse("sub-mcp-1").unwrap()),
        submitter: Some(SubmitterId::parse("lab-a").unwrap()),
        content: Some(ContentHash::of_bytes(b"mcp-artifact")),
        scope: Some(DeclaredScope {
            disease: vec!["glioma".into()],
            modality: vec!["mri".into()],
            decision_family: vec!["evidence-acquisition".into()],
            intended_use: "compare synthetic context compilers".into(),
            out_of_scope: vec!["patient care".into()],
        }),
        licence: Some(Licence::permissive("CC0-1.0")),
        provenance: Some(HubProvenance {
            ancestors: Vec::new(),
            build: BuildProvenance {
                toolchain: "rustc".into(),
                source_digest: ContentHash::of_bytes(b"source"),
                reproducible: true,
            },
            attestations: vec!["local".into()],
        }),
        does_not_establish: vec![NonClaim::clinical_validity()],
        attributions: Vec::new(),
        evidence_scale: Some(EvidenceScale::new(10, 5)),
        claimed_verification: None,
        submitted_at: HubEpoch(1),
    };
    let payload = call(
        &mut server,
        "hub_submission_review",
        json!({
            "draft": serde_json::to_value(draft).unwrap(),
            "submitter": serde_json::to_value(submitter).unwrap(),
            "moderation": {
                "actor": "hub",
                "at": 1,
                "transitions": [
                    { "to": "under_review", "decision": { "actor": "reviewer", "at": 2, "reason": null, "superseded_by": null } },
                    { "to": "accepted", "decision": { "actor": "reviewer", "at": 3, "reason": null, "superseded_by": null } }
                ],
                "attestations": [
                    { "to": "reproduced", "actor": "reviewer", "at": 4 }
                ]
            }
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["schema"], json!("bioprism-mcp/hub-submission/0.1"));
    assert_eq!(payload["state"], json!("accepted"));
    assert_eq!(payload["verification"], json!("reproduced"));
    assert_eq!(payload["event_count"], json!(4));
    assert_eq!(payload["ledger"]["events"].as_array().unwrap().len(), 4);
    assert!(
        payload["limitation_card"]
            .as_str()
            .unwrap()
            .contains("does not establish")
    );
}

#[test]
fn artifact_registry_audit_joins_cross_domain_records_without_inventing_provenance() {
    let mut server = server();
    let leaf = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "register",
            "registration": {
                "kind": "domain_report",
                "subject_id": "mission-leaf",
                "domains": ["oncology", "genomics"],
                "parent_digests": [],
                "artifact": {"status": "review_required"}
            }
        }),
    );
    assert_eq!(leaf["created"], json!(true));
    let leaf_digest = leaf["content_digest"].as_str().unwrap();
    let root = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "register",
            "registration": {
                "kind": "mission_report",
                "subject_id": "mission-root",
                "domains": ["oncology"],
                "parent_digests": [leaf_digest, "f".repeat(64)],
                "artifact": {"status": "partial"}
            }
        }),
    );
    assert_eq!(root["created"], json!(true));
    let lineage = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "lineage",
            "content_digest": root["content_digest"]
        }),
    );
    assert_eq!(lineage["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        lineage["missing_parent_digests"].as_array().unwrap().len(),
        1
    );
    assert_eq!(lineage["cycles"], json!([]));
    assert!(
        lineage["does_not_claim"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "parent presence proves causal provenance or scientific validity")
    );
    let cross_store = call(
        &mut server,
        "artifact_registry_audit",
        json!({"operation": "cross_store"}),
    );
    assert_eq!(
        cross_store["workflow"],
        json!("artifact_registry_cross_store_audit")
    );
    assert_eq!(cross_store["consistent"], json!(true));
    assert_eq!(
        cross_store["coverage"]["mission_evidence_bundle"]["complete"],
        json!(true)
    );
    assert_eq!(
        cross_store["stores"]["artifact_registry"]["record_count"],
        json!(2)
    );
    assert!(
        cross_store["does_not_claim"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "the four stores were read in one atomic transaction")
    );
}

#[test]
fn artifact_registry_domain_evidence_lineage_traces_intake_and_reverse_children() {
    let mut server = server();
    let intake = call(
        &mut server,
        "domain_evidence_intake",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "mcp-lineage-subject",
            "source_tool": "modality_catalog",
            "request": {"modality": "single_cell"},
            "response": {"modalities": ["single_cell"]},
            "outcome": "observed",
            "claim_posture": {
                "status": "observed",
                "does_not_claim": ["clinical validity"]
            }
        }),
    );
    let intake_digest = intake["artifact_registry"]["content_digest"]
        .as_str()
        .unwrap()
        .to_string();
    let trace = call(
        &mut server,
        "artifact_registry_audit",
        json!({
            "operation": "domain_evidence_lineage",
            "content_digest": intake_digest,
            "include_children": true
        }),
    );
    assert_eq!(
        trace["workflow"],
        json!("artifact_registry_domain_evidence_lineage")
    );
    assert_eq!(trace["rows"].as_array().unwrap().len(), 1);
    assert_eq!(trace["rows"][0]["outcome"], json!("observed"));
    assert_eq!(
        trace["rows"][0]["source_plan"]["binding_state"],
        json!("not_declared")
    );
    assert_eq!(
        trace["rows"][0]["request_digest"].as_str().unwrap().len(),
        64
    );
    assert!(
        trace["rows"][0]["artifact_lookup"]
            .as_str()
            .unwrap()
            .contains("/v1/artifacts/")
    );
    assert!(trace["guarantees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item
            == "reverse child links are computed only from exact declared parent content digests"));
}

#[test]
fn domain_decision_readiness_audit_retains_cross_domain_policy_and_review_state() {
    let mut server = server();
    let first = call(
        &mut server,
        "domain_report_project",
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "readiness-subject",
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
            "subject_id": "readiness-subject",
            "source_tool": "bioql_compile",
            "report": {"observations": ["query syntax contract retained"]},
            "claim_posture": {
                "status": "review_required",
                "does_not_claim": ["query execution", "biological truth"]
            }
        }),
    );
    let arguments = json!({
        "subject_id": "readiness-subject",
        "claim": {"id": "claim-readiness-1", "statement": "caller-owned structural claim"},
        "reports": [first["report"].clone(), second["report"].clone()],
        "links": [
            {"report_index": 0, "role": "supports"},
            {"report_index": 1, "role": "context"}
        ],
        "policy": {
            "required_group_ids": ["biological_domains", "biological_ir_and_query"],
            "required_domains": ["modalities", "BioQL syntax"],
            "minimum_supporting_reports": 1,
            "minimum_qualifying_reports": 0,
            "allow_review_required": true
        }
    });

    let first_audit = call(
        &mut server,
        "domain_decision_readiness_audit",
        arguments.clone(),
    );
    assert_eq!(
        first_audit["workflow"],
        json!("domain_decision_readiness_audit")
    );
    assert_eq!(
        first_audit["audit"]["decision_state"],
        json!("review_required")
    );
    assert_eq!(first_audit["audit"]["policy_satisfied"], json!(false));
    assert_eq!(first_audit["readiness_claimed"], json!(false));
    assert_eq!(first_audit["execution"], json!("not_started"));
    assert_eq!(first_audit["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        first_audit["artifact_registry"]["verification"]["method"],
        json!("domain_decision_readiness")
    );
    assert_eq!(
        first_audit["audit"]["counts"]["supporting_reports"],
        json!(1)
    );
    assert_eq!(
        first_audit["audit"]["counts"]["review_required_reports"],
        json!(1)
    );

    let replay = call(&mut server, "domain_decision_readiness_audit", arguments);
    assert_eq!(
        replay["artifact_registry"]["content_digest"],
        first_audit["artifact_registry"]["content_digest"]
    );
    assert_eq!(replay["artifact_registry"]["already_present"], json!(true));
}

#[test]
fn hub_disclosure_replay_preserves_ratchet_and_refuses_unacknowledged_scores() {
    let mut server = server();
    let pack = ContentHash::of_bytes(b"mcp-disclosure-pack");
    let second_pack = ContentHash::of_bytes(b"mcp-split-integrity-pack");
    let payload = call(
        &mut server,
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() },
                { "kind": "disclose", "pack": serde_json::to_value(&pack).unwrap(), "at": 5 },
                { "kind": "headline_eligibility", "pack": serde_json::to_value(&pack).unwrap(), "computed_at": 6 },
                { "kind": "headline_eligibility", "pack": serde_json::to_value(&pack).unwrap(), "computed_at": 6, "acknowledges_disclosure": true },
                { "kind": "contaminate", "pack": serde_json::to_value(&pack).unwrap(), "witness": { "kind": "training_corpus_overlap", "detail": "training snapshot contains the public instances", "observed_at": 7, "reported_by": "audit-1" } },
                { "kind": "declare_held_out", "pack": serde_json::to_value(&second_pack).unwrap() },
                { "kind": "split_integrity", "pack": serde_json::to_value(&second_pack).unwrap(), "at": 8, "reported_by": "oracle-1", "verdict": { "status": "invalid", "witnesses": [{ "type": "identity_leakage", "alias": "ALT-1", "subjects": ["S1", "S2"], "splits": ["train", "test"] }], "oracle_kind": "deterministic_split_integrity_v1" } }
            ]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["schema"], json!("bioprism-mcp/hub-disclosure/0.1"));
    assert_eq!(payload["action_failures"], json!(0));
    assert_eq!(payload["trace"][2]["result"]["eligible"], json!(false));
    assert_eq!(payload["trace"][2]["result"]["fail_closed"], json!(true));
    assert_eq!(payload["trace"][3]["result"]["eligible"], json!(true));
    assert_eq!(
        payload["trace"][4]["result"]["state"]["disclosure"],
        json!("contaminated")
    );
    assert_eq!(
        payload["trace"][6]["result"]["state"]["disclosure"],
        json!("contaminated")
    );
    assert_eq!(payload["entries"].as_array().unwrap().len(), 2);
    assert!(
        payload["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("ratchet"))
    );
}

#[test]
fn hub_cards_and_leaderboards_compose_moderation_disclosure_and_comparability_gates() {
    let mut server = server();
    let review = call(
        &mut server,
        "hub_submission_review",
        hub_review_fixture("sub-mcp-card", b"mcp-card-artifact"),
    );
    assert_eq!(review["ok"], json!(true));

    let pack = ContentHash::of_bytes(b"mcp-board-pack");
    let disclosure = call(
        &mut server,
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() }
            ]
        }),
    );

    let withheld = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card"
        }),
    );
    assert_eq!(withheld["ok"], json!(true));
    assert_eq!(withheld["schema"], json!("bioprism-mcp/hub-card/0.1"));
    assert_eq!(withheld["card"]["state"], json!("available"));
    assert_eq!(withheld["card"]["score"]["display"], json!("withheld"));

    let refused = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card",
            "score": serde_json::to_value(HubScore::point(0.82)).unwrap(),
            "pack": serde_json::to_value(&pack).unwrap(),
            "computed_at": 4
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["stage"], json!("card_disclosure_gate"));
    assert_eq!(refused["score"], Value::Null);
    assert_eq!(refused["fail_closed"], json!(true));

    let published = call(
        &mut server,
        "hub_card_render",
        json!({
            "moderation": review["ledger"].clone(),
            "submission": "sub-mcp-card",
            "score": serde_json::to_value(HubScore::point(0.82)).unwrap(),
            "pack": serde_json::to_value(&pack).unwrap(),
            "computed_at": 4,
            "disclosure": disclosure["ledger"].clone()
        }),
    );
    assert_eq!(published["ok"], json!(true));
    assert_eq!(published["card"]["score"]["display"], json!("published"));
    assert_eq!(published["card"]["score"]["score"]["value"], json!(0.82));
    assert_eq!(published["score"]["attached"], json!(true));

    let conditions = ComparabilityConditions {
        pack,
        pack_version: "1.0.0".into(),
        split: "hidden-holdout".into(),
        metric: "first-divergence-rate".into(),
        higher_is_better: false,
        oracle_tier: "deterministic".into(),
        access_mode: AccessTier::Public,
        budget: BudgetEnvelope::unbounded(),
        protocol: ContentHash::of_bytes(b"mcp-scoring-protocol"),
    };
    let board = Board {
        id: BoardId::parse("glioma-first-divergence").unwrap(),
        conditions: conditions.clone(),
        min_verification: VerificationStatus::SelfReported,
    };
    let ranked_entry = Entry {
        submission: SubmissionId::parse("sub-mcp-card").unwrap(),
        conditions: conditions.clone(),
        score: HubScore::point(0.18),
        computed_at: HubEpoch(4),
        acknowledges_disclosure: false,
        scale: EvidenceScale::new(10, 5),
    };
    let unranked_entry = Entry {
        submission: SubmissionId::parse("sub-mcp-missing").unwrap(),
        conditions,
        score: HubScore::point(0.11),
        computed_at: HubEpoch(4),
        acknowledges_disclosure: false,
        scale: EvidenceScale::new(10, 5),
    };
    let leaderboard = call(
        &mut server,
        "hub_leaderboard_render",
        json!({
            "board": serde_json::to_value(board).unwrap(),
            "entries": [serde_json::to_value(ranked_entry).unwrap(), serde_json::to_value(unranked_entry).unwrap()],
            "moderation": review["ledger"].clone(),
            "disclosure": disclosure["ledger"].clone(),
            "include_details": true
        }),
    );
    assert_eq!(leaderboard["ok"], json!(true));
    assert_eq!(
        leaderboard["schema"],
        json!("bioprism-mcp/hub-leaderboard/0.1")
    );
    assert_eq!(leaderboard["ranked_count"], json!(1));
    assert_eq!(leaderboard["unranked_count"], json!(1));
    assert_eq!(leaderboard["leader_count"], json!(1));
    assert_eq!(leaderboard["rendered"]["ranked"][0]["rank"], json!(1));
    assert_eq!(
        leaderboard["rendered"]["unranked"][0]["reason"],
        json!({ "reason": "not_published", "state": null })
    );
    assert!(leaderboard["headline"].as_str().unwrap().contains("Rank 1"));
}

#[test]
fn safety_posture_keeps_residual_and_unanalysed_threats_separate() {
    let mut server = server();
    let payload = call(&mut server, "safety_posture", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["coverage"]["mitigated"].is_number());
    assert!(payload["coverage"]["declared_only"].is_number());
    assert!(payload["coverage"]["unmitigated"].is_number());
    assert!(payload["residual_threat_ids"].is_array());
    assert!(payload["unanalysed_threat_ids"].is_array());
    assert_eq!(
        payload["perimeter_controls_are_not_claimed_as_enforced"],
        json!(true)
    );
}

#[test]
fn security_redteam_simulation_keeps_the_full_safety_loop_typed_and_honest() {
    let mut server = server();
    let payload = call(
        &mut server,
        "security_redteam_simulate",
        json!({
            "findings": [
                {
                    "id": "F-confirmed",
                    "campaign": "sandbox-escape",
                    "boundary": "agent_sandbox",
                    "class": "sandbox_bypass",
                    "status": "confirmed",
                    "reproduction": "probe-17",
                    "embargoed": true,
                    "minimised": true
                },
                {
                    "id": "F-reported",
                    "campaign": "privacy",
                    "boundary": "public_api",
                    "class": "privacy_leakage",
                    "status": "reported"
                }
            ],
            "vulnerabilities": [{
                "id": "V-holdout",
                "class": "hidden_test_exposure",
                "severity": "high",
                "epoch": 1,
                "impact": { "infrastructure": false, "data": true, "result_integrity": true },
                "advisory": {
                    "affected_versions": "0.1.0",
                    "impact": "holdout labels were exposed",
                    "mitigation": "rotate the holdout",
                    "fixed_versions": "0.1.1",
                    "result_implications": "runs before rotation require review",
                    "timeline": "reported e1; fixed e3",
                    "credit": "red-team",
                    "residual_risk": "old mirrors may retain copies"
                },
                "transitions": [
                    { "to": "triaged", "epoch": 2, "note": "reproduced" },
                    { "to": "fixed", "epoch": 3, "note": "holdout rotated" },
                    { "to": "disclosed", "epoch": 4, "note": "advisory published" }
                ]
            }],
            "deliveries": [
                {
                    "id": "sealed-output",
                    "kind": "agent_output",
                    "origin": "agent_sandbox",
                    "to": "artifact_service",
                    "via": "sealed_output_bundle"
                },
                {
                    "id": "artifact-fetch",
                    "kind": "agent_output",
                    "origin": "artifact_service",
                    "to": "evaluator_sandbox",
                    "via": "artifact_fetch"
                },
                {
                    "id": "hidden-oracle",
                    "kind": "hidden_oracle_asset",
                    "origin": "artifact_service",
                    "to": "agent_sandbox",
                    "via": "hidden_oracle_mount"
                }
            ],
            "incidents": [{
                "id": "I-holdout",
                "class": "hidden_holdout_leak",
                "opened_at": 5,
                "requests": [{
                    "action": "freeze_publication",
                    "requested_by": "operator:red-team",
                    "requested_at": 6
                }],
                "blast_radius": {
                    "completeness": "complete",
                    "dispositions": {
                        "run-1": "invalidated",
                        "run-2": "cleared"
                    }
                },
                "timeline": [
                    { "epoch": 5, "actor": "operator:red-team", "event": "incident opened" },
                    { "epoch": 6, "actor": "operator:red-team", "event": "publication freeze requested" }
                ]
            }],
            "audit_records": [
                {
                    "event": "security_quarantine",
                    "actor": "operator:red-team",
                    "subject": "hidden-oracle",
                    "epoch": 6,
                    "statement": {
                        "kind": "observed",
                        "observation": {
                            "kind": "boundary_crossing_refused",
                            "artifact": "hidden-oracle",
                            "from": "artifact_service",
                            "to": "agent_sandbox"
                        }
                    }
                },
                {
                    "event": "reviewer_decision",
                    "actor": "operator:red-team",
                    "subject": "V-holdout",
                    "epoch": 7,
                    "statement": {
                        "kind": "asserted",
                        "by": "operator:red-team",
                        "claim": "advisory reviewed"
                    }
                }
            ],
            "attestations": [
                { "kind": "digests_compared", "component": "holdout", "observed": true },
                { "kind": "built_from_manifest", "manifest": "m1", "runner": "builder", "observed": true }
            ],
            "boundary_universe": ["agent_sandbox", "evaluator_sandbox", "public_api"],
            "include_details": true
        }),
    );
    if payload["ok"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(payload["regression_corpus"]["sentinel_count"], json!(1));
    assert_eq!(
        payload["findings"][1]["regression_gate"]["eligible"],
        json!(false)
    );
    if payload["vulnerabilities"][0]["disclosed"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(payload["boundary"]["delivery_rows"][0]["ok"], json!(true));
    assert_eq!(
        payload["boundary"]["delivery_rows"][2]["fail_closed"],
        json!(true)
    );
    assert_eq!(
        payload["boundary"]["within_trial_evaluator_to_agent"],
        json!([])
    );
    assert!(
        !payload["boundary"]["feedback_loops"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["allowed"],
        json!(true)
    );
    assert_eq!(payload["audit"]["verified"], json!(true));
    assert_eq!(payload["audit"]["assertion_count"], json!(1));
    assert_eq!(payload["attestations"][0]["observed"], json!(true));
    assert_eq!(payload["attestations"][1]["ok"], json!(false));
}

#[test]
fn security_redteam_simulation_refuses_missing_advisories_and_partial_lineage() {
    let mut server = server();
    let payload = call(
        &mut server,
        "security_redteam_simulate",
        json!({
            "vulnerabilities": [{
                "id": "V-skip",
                "class": "sandbox_bypass",
                "severity": "critical",
                "epoch": 10,
                "transitions": [
                    { "to": "triaged", "epoch": 11 },
                    { "to": "fixed", "epoch": 12 },
                    { "to": "disclosed", "epoch": 13 }
                ]
            }],
            "incidents": [{
                "id": "I-partial",
                "class": "compromised_key",
                "opened_at": 1,
                "blast_radius": {
                    "completeness": "partial",
                    "unreachable_edges": 2,
                    "dispositions": { "run-1": "cleared" }
                }
            }],
            "max_items": 10
        }),
    );
    if payload["ok"] != json!(true) {
        panic!("security redteam response: {}", payload);
    }
    if payload["vulnerabilities"][0]["disclosed"] != json!(false) {
        panic!("security redteam response: {}", payload);
    }
    assert_eq!(
        payload["vulnerabilities"][0]["transitions"][2]["fail_closed"],
        json!(true)
    );
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["allowed"],
        json!(false)
    );
    assert_eq!(
        payload["incidents"][0]["containment_claim"]["fail_closed"],
        json!(true)
    );
}

#[test]
fn registry_gate_fails_closed_on_an_unattested_document() {
    let mut server = server();
    let payload = call(
        &mut server,
        "registry_gate",
        json!({ "pack": WORLD, "policy": "experimental" }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["passed"], json!(false));
    assert_eq!(payload["blocked"], json!(true));
    assert_eq!(payload["outcome"]["outcome"], json!("block"));
}

#[test]
fn registry_lifecycle_keeps_invalid_packs_and_independent_actions_explicit() {
    let mut server = server();
    let payload = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "packs": [{ "not": "an attested benchmark pack" }],
            "actions": [
                { "op": "publish", "pack_index": 0, "tier": "exploratory" },
                { "op": "resolve", "name": "missing@0.1.0" },
                { "op": "verify_all" }
            ],
            "include_index": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["packs"][0]["valid"], json!(false));
    assert_eq!(payload["actions"][0]["ok"], json!(false));
    assert_eq!(payload["actions"][0]["fail_closed"], json!(true));
    assert_eq!(payload["actions"][1]["ok"], json!(true));
    assert_eq!(payload["actions"][1]["result"]["found"], json!(false));
    assert_eq!(payload["actions"][2]["result"]["clean"], json!(true));
    assert_eq!(payload["final"]["artifact_count"], json!(0));
    assert!(payload["registry"].is_object());
    assert!(
        payload["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("continu"))
    );
}

#[test]
fn registry_lifecycle_returns_continuation_state_and_preserves_withdrawal_history() {
    let pack = attested_minimal_registry_pack();
    let mut server = server();
    let first = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "packs": [pack],
            "actions": [
                { "op": "publish", "pack_index": 0, "tier": "unranked" },
                { "op": "resolve", "name": "mcp/demo@0.1.0" }
            ]
        }),
    );
    assert_eq!(first["actions"][0]["ok"], json!(true));
    let digest = first["actions"][0]["result"]["digest"]
        .as_str()
        .expect("publish returns digest");
    assert_eq!(first["actions"][1]["result"]["digest"], json!(digest));
    assert_eq!(first["final"]["integrity_clean"], json!(true));

    let resumed = call(
        &mut server,
        "registry_lifecycle_simulate",
        json!({
            "index": first["registry"],
            "actions": [
                { "op": "inspect", "digest": digest },
                { "op": "withdraw", "digest": digest, "reason": "fixture lifecycle complete" },
                { "op": "history", "digest": digest },
                { "op": "verify_all" }
            ]
        }),
    );
    assert_eq!(
        resumed["initial_integrity"]["operations_allowed"],
        json!(true)
    );
    assert_eq!(resumed["actions"][0]["result"]["found"], json!(true));
    assert_eq!(resumed["actions"][1]["ok"], json!(true));
    assert_eq!(resumed["actions"][2]["result"]["event_count"], json!(2));
    assert_eq!(resumed["actions"][3]["result"]["clean"], json!(true));
    assert_eq!(resumed["final"]["artifact_count"], json!(1));
    assert_eq!(resumed["final"]["log_count"], json!(2));
}

#[test]
fn biocapability_evidence_audit_composes_metrics_value_reference_and_claim_readiness() {
    let mut server = server();
    let payload = call(
        &mut server,
        "biocapability_evidence_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "evidence": [
                { "id": "grounding", "dimension": "evidence_grounding", "status": "observed", "domain": "oncology", "source": "ledger:run-1", "scope": "pack/4" },
                { "id": "acquisition", "dimension": "information_acquisition", "status": "observed", "domain": "bioir", "action_changed": true, "cost": 12.0 },
                { "id": "resource", "dimension": "resource_efficiency", "status": "observed", "domain": "operations", "denominator": "successful reproduced runs", "cost": 12.0 },
                { "id": "time", "dimension": "temporal_validity", "status": "observed", "domain": "evaluation", "decision_epoch": 10, "evidence_epoch": 9 },
                { "id": "modal", "dimension": "cross_modal_consistency", "status": "observed", "domain": "oncology", "modalities": ["mri", "pathology"], "agreement": true },
                { "id": "causal", "dimension": "causal_identification", "status": "observed", "domain": "inference", "identification": "identified", "estimand": "treatment effect" },
                { "id": "repro", "dimension": "reproducibility", "status": "reproduced", "domain": "research_ci", "replications": 3, "environment_pinned": true },
                { "id": "translation", "dimension": "translation_maturity", "status": "observed", "domain": "oncoworlds", "source_population": "xenograft", "target_population": "patient", "bridge": true },
                { "id": "coordination", "dimension": "multi_agent_coordination", "status": "observed", "domain": "orchestration", "agents": ["planner", "verifier"], "coordination_overhead": 1.2 }
            ],
            "claim_requests": [{
                "id": "public-profile",
                "claim": "publishable capability profile",
                "requires": [
                    "evidence_grounding",
                    "information_acquisition",
                    "resource_efficiency",
                    "temporal_validity",
                    "cross_modal_consistency",
                    "causal_identification",
                    "reproducibility",
                    "translation_maturity",
                    "multi_agent_coordination"
                ]
            }],
            "information": {
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
            },
            "reference": {
                "standard": "distribution",
                "mass": { "progression": 0.6, "stable": 0.4 },
                "dispersion": { "kind": "mixed", "aleatoric_fraction": 0.5 }
            },
            "reference_state": "progression",
            "max_items": 20
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["metrics_ok"], json!(true));
    assert_eq!(
        payload["claim_requests"]["rows"][0]["eligible"],
        json!(true)
    );
    assert_eq!(
        payload["release_posture"]["ready_for_requested_claims"],
        json!(true)
    );
    assert_eq!(payload["subaudits"]["information_value"]["ok"], json!(true));
    assert_eq!(
        payload["subaudits"]["reference_quality"]["can_certify_clean_pass"],
        json!(false)
    );
    assert_eq!(
        payload["evidence"]["dimensions"].as_array().unwrap().len(),
        9
    );
    assert_eq!(payload["evidence"]["invalid_item_count"], json!(0));
}

#[test]
fn biocapability_evidence_audit_blocks_temporal_leaks_and_declared_only_claims() {
    let mut server = server();
    let payload = call(
        &mut server,
        "biocapability_evidence_audit",
        json!({
            "vectors": [
                metric_vector("system-a", 0.9, 0.8, "pack/4"),
                metric_vector("system-b", 0.7, 0.6, "pack/4")
            ],
            "evidence": [
                { "id": "future", "dimension": "temporal_validity", "status": "observed", "decision_epoch": 10, "evidence_epoch": 11 },
                { "id": "declared-grounding", "dimension": "evidence_grounding", "status": "declared", "source": "operator assertion", "scope": "unknown" },
                { "id": "unknown", "dimension": "not-a-real-dimension", "status": "observed" }
            ],
            "claim_requests": [{
                "id": "temporal-profile",
                "claim": "temporally valid profile",
                "requires": ["temporal_validity", "evidence_grounding"]
            }]
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["evidence"]["invalid_item_count"], json!(2));
    let temporal = payload["evidence"]["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["dimension"] == json!("temporal_validity"))
        .unwrap();
    assert_eq!(temporal["state"], json!("blocked"));
    assert_eq!(
        payload["claim_requests"]["rows"][0]["eligible"],
        json!(false)
    );
    assert_eq!(
        payload["release_posture"]["ready_for_requested_claims"],
        json!(false)
    );
}

#[test]
fn safety_release_gate_distinguishes_clear_conditioned_blocked_and_unrated() {
    let mut server = server();
    let complete_low = json!({
        "capability_uplift": "low",
        "actionability": "low",
        "scale": "low",
        "expertise_reduction": "low",
        "target_specificity": "low",
        "reversibility": "low",
        "detectability": "high",
        "available_safeguards": "high",
        "legitimate_scientific_value": "high"
    });
    let cleared = call(
        &mut server,
        "safety_release_gate",
        json!({ "assessment": risk_assessment(complete_low) }),
    );
    assert_eq!(cleared["ok"], json!(true));
    assert_eq!(cleared["decision"]["decision"], json!("cleared"));
    assert_eq!(cleared["cleared"], json!(true));
    assert!(cleared["unrated_dimensions"].as_array().unwrap().is_empty());

    let conditioned = call(
        &mut server,
        "safety_release_gate",
        json!({
            "assessment": risk_assessment(json!({
                "capability_uplift": "high",
                "actionability": "low",
                "scale": "low",
                "expertise_reduction": "low",
                "target_specificity": "low",
                "reversibility": "low",
                "detectability": "high",
                "available_safeguards": "high",
                "legitimate_scientific_value": "high"
            }))
        }),
    );
    assert_eq!(conditioned["decision"]["decision"], json!("conditioned"));
    assert_eq!(
        conditioned["decision"]["driven_by"],
        json!(["capability_uplift"])
    );
    assert_eq!(conditioned["cleared"], json!(false));

    let blocked = call(
        &mut server,
        "safety_release_gate",
        json!({
            "assessment": risk_assessment(json!({
                "capability_uplift": "high",
                "actionability": "high",
                "scale": "low",
                "expertise_reduction": "low",
                "target_specificity": "low",
                "reversibility": "low",
                "detectability": "high",
                "available_safeguards": "high",
                "legitimate_scientific_value": "high"
            }))
        }),
    );
    assert_eq!(blocked["decision"]["decision"], json!("blocked"));
    assert_eq!(
        blocked["decision"]["driven_by"].as_array().unwrap().len(),
        2
    );

    let incomplete = call(
        &mut server,
        "safety_release_gate",
        json!({ "assessment": risk_assessment(json!({
            "capability_uplift": "low"
        })) }),
    );
    assert_eq!(incomplete["__isError"], json!(true));
    assert!(incomplete["error"].as_str().unwrap().contains("unrated"));
}

#[test]
fn medical_boundary_admits_research_and_returns_structured_clinical_refusal() {
    let mut server = server();
    let admitted = call(
        &mut server,
        "medical_boundary_check",
        json!({
            "output": {
                "side": "research",
                "use_case": "evidence_synthesis",
                "label": "evidence synthesis for a research report"
            }
        }),
    );
    assert_eq!(admitted["ok"], json!(true));
    assert_eq!(admitted["admitted"], json!(true));
    assert_eq!(admitted["use_case"], json!("evidence_synthesis"));
    assert!(
        admitted["research_only_label"]
            .as_str()
            .unwrap()
            .contains("not a medical device")
    );

    let refused = call(
        &mut server,
        "medical_boundary_check",
        json!({
            "output": {
                "side": "clinical",
                "category": "treatment_selection",
                "label": "choose a treatment"
            }
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["admitted"], json!(false));
    assert_eq!(refused["clinical_output_is_never_admitted"], json!(true));
    assert!(
        refused["refusal"]
            .as_str()
            .unwrap()
            .contains("research-only")
    );
    assert_eq!(refused["__isError"], json!(false));
}

#[test]
fn hub_search_preserves_facet_reasons_authority_freshness_and_empty_query_refusal() {
    let mut server = server();
    let registry = RegistryId::parse("origin").unwrap();
    let namespace = Namespace::parse("bioprism").unwrap();
    let authority = Authority::new(registry.clone())
        .owning(namespace)
        .expect("authority builds");
    let mut federation = Federation::new();
    federation
        .admit(authority.clone())
        .expect("federation admits");
    let mut catalog = Catalog::origin(authority);
    catalog
        .record(
            PackRelease::new(
                PackName::parse("bioprism/onco").unwrap(),
                Version::new(1, 0, 0),
                "sha256:onco",
            )
            .described("oncology reference pack")
            .keyworded(["onco"])
            .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    catalog
        .record(
            PackRelease::new(
                PackName::parse("bioprism/other").unwrap(),
                Version::new(1, 0, 0),
                "sha256:other",
            )
            .described("other pack")
            .keyworded(["onco"])
            .at_tier(TrustTier::Unranked),
        )
        .unwrap();
    let payload = call(
        &mut server,
        "hub_search",
        json!({
            "federation": serde_json::to_value(&federation).unwrap(),
            "catalogs": [serde_json::to_value(&catalog).unwrap()],
            "query": serde_json::to_value(Query::new(vec![
                Facet::Keyword("onco".into()),
                Facet::TierAtLeast(TrustTier::Reviewed)
            ])).unwrap(),
            "max_items": 1
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["match_count"], json!(1));
    assert_eq!(payload["matches"][0]["name"], json!("bioprism/onco"));
    assert_eq!(
        payload["matches"][0]["authority"]["authority"],
        json!("authoritative")
    );
    assert_eq!(
        payload["matches"][0]["freshness"]["freshness"],
        json!("authoritative")
    );
    assert_eq!(payload["matches"][0]["why"].as_array().unwrap().len(), 2);
    assert_eq!(payload["excluded_count"], json!(1));
    assert_eq!(
        payload["excluded"][0]["failed"],
        json!("tier at least reviewed")
    );
    assert_eq!(payload["truncated"], json!(false));

    let refused = call(
        &mut server,
        "hub_search",
        json!({
            "federation": serde_json::to_value(&federation).unwrap(),
            "catalogs": [serde_json::to_value(&catalog).unwrap()],
            "query": serde_json::to_value(Query::new(vec![])).unwrap()
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("no facets"));
}

#[test]
fn hub_resolve_and_lock_preserve_federation_and_dependency_provenance() {
    let mut server = server();
    let registry = RegistryId::parse("origin").unwrap();
    let authority = Authority::new(registry)
        .owning(Namespace::parse("bioprism").unwrap())
        .unwrap();
    let mut federation = Federation::new();
    federation.admit(authority.clone()).unwrap();
    let child = PackName::parse("bioprism/child").unwrap();
    let root = PackName::parse("bioprism/root").unwrap();
    let mut catalog = Catalog::origin(authority);
    catalog
        .record(
            PackRelease::new(child.clone(), Version::new(1, 0, 0), "sha256:child")
                .described("child dependency")
                .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    catalog
        .record(
            PackRelease::new(root.clone(), Version::new(1, 0, 0), "sha256:root")
                .depending_on(child, VersionReq::Any)
                .described("root pack")
                .at_tier(TrustTier::Reviewed),
        )
        .unwrap();
    let request = HubRequest::new(root, VersionReq::Any);
    let args = json!({
        "federation": serde_json::to_value(&federation).unwrap(),
        "catalogs": [serde_json::to_value(&catalog).unwrap()],
        "request": serde_json::to_value(&request).unwrap()
    });
    let resolved = call(&mut server, "hub_resolve", args.clone());
    assert_eq!(resolved["ok"], json!(true));
    assert_eq!(
        resolved["resolution"]["subject"]["name"],
        json!("bioprism/root")
    );
    assert_eq!(resolved["authoritative"], json!(true));

    let locked = call(
        &mut server,
        "hub_lock",
        json!({
            "federation": args["federation"].clone(),
            "catalogs": args["catalogs"].clone(),
            "request": args["request"].clone(),
            "max_items": 10
        }),
    );
    assert_eq!(locked["ok"], json!(true));
    assert_eq!(locked["entry_count"], json!(2));
    assert_eq!(locked["fully_authoritative"], json!(true));
    assert_eq!(locked["entries"].as_array().unwrap().len(), 2);
    assert!(locked["entries"][0]["locked"]["required_by"].is_array());
}

#[test]
fn policy_screen_denies_unknown_policy_and_admits_only_under_a_typed_rule() {
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

    let denied = call(
        &mut server,
        "policy_screen",
        json!({ "world": WORLD, "request": request.clone(), "facts": ["fact.cohort"] }),
    );
    assert_eq!(denied["ok"], json!(true));
    assert_eq!(denied["admitted_count"], json!(0));
    assert_eq!(denied["refused_count"], json!(1));
    assert_eq!(
        denied["refused"][0]["constraint"],
        json!("unlabelled_evidence")
    );
    assert_eq!(denied["complete"], json!(false));
    assert_eq!(denied["trace"]["supports_sufficiency_claim"], json!(false));

    let label = json!({
        "classification": "public_aggregate",
        "compartments": [],
        "purposes": { "only": ["research_analysis"] },
        "residency": "anywhere",
        "export": "unrestricted",
        "retention": "indefinite",
        "min_cell_size": 0
    });
    let admitted = call(
        &mut server,
        "policy_screen",
        json!({
            "world": WORLD,
            "request": request,
            "facts": ["fact.cohort"],
            "rules": [{
                "id": "world-cohort-policy",
                "version": 1,
                "scope": { "cohort": "RG-DEMO-001" },
                "label": label
            }]
        }),
    );
    assert_eq!(admitted["ok"], json!(true));
    assert_eq!(admitted["admitted_count"], json!(1));
    assert_eq!(admitted["refused_count"], json!(0));
    assert_eq!(admitted["complete"], json!(true));
    assert_eq!(
        admitted["admitted"][0]["admission"]["mode"],
        json!({ "mode": "central" })
    );
    assert!(admitted["policy_version"].as_str().unwrap().len() >= 32);
}

#[test]
fn governance_schema_surface_lists_contracts_and_checks_a_reference_certificate() {
    let mut server = server();
    let catalog = call(&mut server, "governance_schema_check", json!({}));
    assert_eq!(catalog["ok"], json!(true));
    assert_eq!(catalog["schema_count"], json!(3));
    assert!(catalog["schemas"][0]["hashed_paths"].is_array());

    let checked = call(
        &mut server,
        "governance_schema_check",
        json!({ "document": "fixtures/fiber-v0.1/golden/reference_certificate.json" }),
    );
    assert_eq!(checked["ok"], json!(true));
    assert_eq!(checked["mode"], json!("document"));
    assert_eq!(
        checked["schema"]["id"],
        json!("fiber-context-certificate/0.1")
    );
    assert_eq!(checked["conforms"], json!(true));
    assert_eq!(checked["is_clean"], json!(true));
}

#[test]
fn bundle_verify_recomputes_carried_content_and_refuses_tampering() {
    let bundle = ResultBundle::builder("protocol-bundle")
        .carrying("query", EntryRole::Query, json!({ "goal": "verify" }))
        .unwrap()
        .build()
        .unwrap();
    let result = call(
        &mut server(),
        "bundle_verify",
        json!({ "bundle": serde_json::to_value(&bundle).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["not_recomputed"], json!([]));
    assert!(result["honest_label"].as_str().unwrap().contains("1 of 1"));

    let mut tampered = serde_json::to_value(bundle).unwrap();
    tampered["contents"]["query"]["goal"] = json!("tampered");
    let refused = call(
        &mut server(),
        "bundle_verify",
        json!({ "bundle": tampered }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert!(refused["refusal"].as_str().unwrap().contains("digest"));
}

#[test]
fn bundle_verify_applies_an_offline_registry_policy_to_public_attestations() {
    let bundle = ResultBundle::builder("policy-bundle")
        .carrying("query", EntryRole::Query, json!({ "goal": "policy" }))
        .unwrap()
        .build()
        .unwrap();
    let signing = SigningKey::new(KeyIdentity::new("policy-publisher"), [0x71; 32]);
    let public = signing.verification_key(KeyValidity::unbounded());
    let signed = PubliclyAttestedBundle::produce_with(
        bundle,
        &signing,
        AttestationPurpose::PublisherManifest,
        ClaimedProducer::new("AURORA Policy Publisher"),
        None,
        None,
        Some(100),
    )
    .unwrap();
    let mut registry = KeyRegistry::new();
    registry
        .register_root(
            RegisteredKey::root(
                public.identity().clone(),
                public.public_key(),
                KeyValidity::unbounded(),
                "AURORA Policy Publisher",
                [KeyRole::Publisher].into_iter().collect(),
                std::collections::BTreeSet::new(),
            )
            .unwrap(),
        )
        .unwrap();
    let result = call(
        &mut server(),
        "bundle_verify",
        json!({
            "publicly_attested_bundle": serde_json::to_value(signed).unwrap(),
            "trust_registry": serde_json::to_value(registry).unwrap(),
            "trust_policy": serde_json::to_value(TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest)).unwrap(),
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["verification_mode"],
        json!("ed25519_registry_policy")
    );
    assert_eq!(result["trust_report"]["verdict"], json!("trusted"));
    assert_eq!(
        result["trust_report"]["key_identity"],
        json!("policy-publisher")
    );
}

#[test]
fn obligation_gate_check_keeps_effective_states_and_mandatory_closure_visible() {
    let at = Timestamp::parse("2026-08-14T00:00:00Z").unwrap();
    let mut graph = ObligationGraph::new("publish a validation report");
    graph
        .insert(
            Obligation::new("identity", "the specimen identity is established")
                .mandatory()
                .with_value(3.0),
        )
        .unwrap();
    graph
        .insert(
            Obligation::new("validation", "the assay validation is complete")
                .depending_on(["identity"])
                .mandatory()
                .with_value(5.0),
        )
        .unwrap();
    graph
        .insert(
            Obligation::new("consent", "the release consent is recorded")
                .mandatory()
                .with_value(4.0),
        )
        .unwrap();
    graph
        .record(
            "identity",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.95)
                .with_evidence(["evidence://identity"]),
        )
        .unwrap();
    let action = ObligationAction::new("publish", RegretClass::Irreversible)
        .described("publish the validation result")
        .requiring(ObligationPredicate::satisfied("validation"));

    let blocked = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap(),
            "max_items": 1
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(
        blocked["schema"],
        json!("bioprism-mcp/obligation-gate-check/0.1")
    );
    assert_eq!(blocked["outcome_kind"], json!("blocked"));
    assert_eq!(
        blocked["gate"]["reason"]["reason"],
        json!("prerequisites_unmet")
    );
    assert_eq!(blocked["graph"]["valid"], json!(true));
    assert_eq!(blocked["graph"]["omitted_effective_states"], json!(2));
    assert_eq!(
        blocked["graph"]["frontier"][0]["obligation"],
        json!("validation")
    );

    graph
        .record(
            "validation",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.9)
                .with_evidence(["evidence://validation"]),
        )
        .unwrap();
    let blocked_closure = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap()
        }),
    );
    assert_eq!(blocked_closure["outcome_kind"], json!("blocked"));
    assert_eq!(
        blocked_closure["gate"]["reason"]["reason"],
        json!("mandatory_obligation_outstanding")
    );
    assert_eq!(
        blocked_closure["gate"]["reason"]["obligation"],
        json!("consent")
    );

    graph
        .record(
            "consent",
            StateRecord::new(ObligationState::Satisfied, "reviewer", at, 0.9)
                .with_evidence(["evidence://consent"]),
        )
        .unwrap();
    let allowed = call(
        &mut server(),
        "obligation_gate_check",
        json!({
            "graph": serde_json::to_value(&graph).unwrap(),
            "action": serde_json::to_value(&action).unwrap()
        }),
    );
    assert_eq!(allowed["outcome_kind"], json!("allowed"));
    assert_eq!(allowed["allowed"], json!(true));
    assert_eq!(allowed["refusal"], Value::Null);
    assert_eq!(allowed["gate"]["checked"], json!(["validation"]));
    assert_eq!(allowed["graph"]["undischarged"], json!([]));
    assert_eq!(allowed["graph"]["sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn bioethics_action_review_never_executes_and_requires_both_referral_acts() {
    let plan = ActionPlan::new("resistance-screen", OutputUse::CohortAnalysis)
        .with_step(PlannedStep::new(ActionKind::Analysis, "analyse the cohort"))
        .with_step(PlannedStep::new(
            ActionKind::LiquidHandler,
            "plate a dilution series",
        ));
    let refused = call(
        &mut server(),
        "bioethics_action_review",
        json!({ "plan": serde_json::to_value(&plan).unwrap() }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["physical_step_count"], json!(1));
    assert_eq!(refused["referral"]["status"], json!("not_attempted"));
    assert_eq!(refused["referral"]["fail_closed"], json!(true));

    let referred = call(
        &mut server(),
        "bioethics_action_review",
        json!({
            "plan": serde_json::to_value(&plan).unwrap(),
            "authorisation": serde_json::to_value(
                Authorisation::new()
                    .approved_by("principal investigator")
                    .safety_reviewed_by("institutional biosafety committee")
            ).unwrap()
        }),
    );
    assert_eq!(referred["referral"]["status"], json!("referred"));
    assert_eq!(
        referred["referral"]["executes_physical_action"],
        json!(false)
    );

    let clinical = ActionPlan::new("individual", OutputUse::TreatmentRecommendation)
        .with_step(PlannedStep::new(ActionKind::Analysis, "analyse"));
    let clinical_result = call(
        &mut server(),
        "bioethics_action_review",
        json!({ "plan": serde_json::to_value(clinical).unwrap() }),
    );
    assert_eq!(clinical_result["ok"], json!(false));
    assert_eq!(clinical_result["fail_closed"], json!(true));
}

#[test]
fn bioethics_human_subject_screen_keeps_review_consent_and_return_of_results_separate() {
    let study = StudyDescription::new(
        "reader-agreement-study",
        PurposeSet::of([Purpose::ResearchAnalysis]),
    )
    .engaging(EngagementKind::ExpertPerformanceStudy)
    .returning(ReturnOfResults::IndividualFindings);
    let consent = Consent::new("consent-1", PurposeSet::of([Purpose::ResearchAnalysis]));
    let result = call(
        &mut server(),
        "bioethics_human_subject_screen",
        json!({
            "study": serde_json::to_value(study).unwrap(),
            "consent": serde_json::to_value(consent).unwrap(),
            "at": "2026-01-01T00:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["requires_institutional_review"], json!(true));
    assert_eq!(result["consent"]["status"], json!("admitted"));
    assert_eq!(result["return_of_results"]["status"], json!("refused"));
    assert_eq!(result["clearance_issued"], json!(false));

    let undetermined = call(
        &mut server(),
        "bioethics_human_subject_screen",
        json!({
            "study": serde_json::to_value(StudyDescription::new(
                "undescribed",
                PurposeSet::of([Purpose::MethodDevelopment])
            )).unwrap()
        }),
    );
    assert_eq!(
        undetermined["determination"]["determination"],
        json!("undetermined")
    );
    assert_eq!(undetermined["clearance_issued"], json!(false));
}

#[test]
fn bioethics_dual_use_review_requires_assessment_before_the_safety_gate() {
    let release = CapabilityRelease::new(
        "variant-caller",
        SurfaceAssessment::assessed("biosafety reviewer", [MisuseSurface::SequenceDesign]),
    );
    let mut risk = RiskAssessment::for_subject("variant-caller")
        .in_category(SensitiveCategory::BiologicalDesign);
    for dimension in RiskDimension::ALL {
        risk = risk.rating(dimension, Rating::Low);
    }
    let accepted = call(
        &mut server(),
        "bioethics_dual_use_review",
        json!({
            "release": serde_json::to_value(&release).unwrap(),
            "risk": serde_json::to_value(&risk).unwrap(),
            "withhold": "exploit_detail",
            "finding": "screening gap"
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["decision"]["decision"], json!("cleared"));
    assert_eq!(accepted["withholding"]["status"], json!("admitted"));

    let unassessed = call(
        &mut server(),
        "bioethics_dual_use_review",
        json!({
            "release": serde_json::to_value(CapabilityRelease::new(
                "variant-caller",
                SurfaceAssessment::NotAssessed
            )).unwrap(),
            "risk": serde_json::to_value(risk).unwrap()
        }),
    );
    assert_eq!(unassessed["ok"], json!(false));
    assert!(
        unassessed["refusal"]
            .as_str()
            .unwrap()
            .contains("no misuse-surface")
    );
}

#[test]
fn bioethics_validation_check_does_not_mint_verification_from_missing_evidence() {
    let empty = call(
        &mut server(),
        "bioethics_validation_check",
        json!({
            "dossier": serde_json::to_value(ValidationDossier::new("module", "author-a")).unwrap()
        }),
    );
    assert_eq!(empty["ok"], json!(true));
    assert_eq!(empty["verification"]["status"], json!("refused"));
    assert!(empty["missing_count"].as_u64().unwrap() > 0);

    let mut complete = ValidationDossier::new("module", "author-a");
    for kind in EvidenceKind::ALL {
        complete = complete.with(BioethicsEvidenceRecord::new(
            kind,
            format!("reference-{}", kind.as_str()),
            if kind == EvidenceKind::IndependentReproduction {
                "reproducer-b"
            } else {
                "author-a"
            },
        ));
    }
    let verified = call(
        &mut server(),
        "bioethics_validation_check",
        json!({ "dossier": serde_json::to_value(complete).unwrap() }),
    );
    assert_eq!(verified["verification"]["status"], json!("verified"));
    assert_eq!(verified["missing_count"], json!(0));
}

#[test]
fn bioethics_representation_audit_preserves_unmeasured_and_suppressed_strata() {
    let observations = vec![
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::AgeAndSex, "adult"),
            StratumCoverage::Measured,
        ),
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::Geography, "rural"),
            StratumCoverage::Unmeasured,
        ),
        StratumObservation::new(
            BioethicsStratum::new(ContextAxis::SiteResources, "low-resource"),
            StratumCoverage::SuppressedSmallGroup { below: 10 },
        ),
    ];
    let result = call(
        &mut server(),
        "bioethics_representation_audit",
        json!({
            "subject": "cohort-1",
            "observations": serde_json::to_value(observations).unwrap(),
            "attribution": {
                "axis": "age_and_sex",
                "matched": [],
                "finding": "performance difference"
            }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["measured_count"], json!(1));
    assert_eq!(result["unmeasured_count"], json!(1));
    assert_eq!(result["suppressed_count"], json!(1));
    assert_eq!(result["complete"], json!(false));
    assert_eq!(
        result["attribution"]["standing"]["status"],
        json!("refused")
    );
}

#[test]
fn stewardship_review_check_issues_scoped_approval_and_refuses_self_review() {
    let revision = EvaluatorRevision::new(
        "evaluator",
        SchemaVersion::parse("1.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v1"),
        false,
    );
    let mut record = ReviewRecord::new(
        revision,
        Actor::author("author"),
        Actor::independent_reviewer("reviewer"),
    )
    .against(full_corpus(1));
    for dimension in ReviewDimension::mandatory_for(false) {
        record = record
            .finding(
                dimension,
                StewardshipFinding::passed("checked against the declared corpus"),
            )
            .unwrap();
    }
    let issued = call(
        &mut server(),
        "stewardship_review_check",
        json!({ "review": serde_json::to_value(record).unwrap() }),
    );
    assert_eq!(issued["__isError"], json!(false));
    assert_eq!(issued["decision"], json!("issued"));
    assert_eq!(issued["covered_dimensions"].as_array().unwrap().len(), 6);
    assert!(
        issued["unreviewed_dimensions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["dimension"] == "adversarial_injection")
    );

    let revision = EvaluatorRevision::new(
        "evaluator",
        SchemaVersion::parse("1.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v1"),
        false,
    );
    let mut self_review = ReviewRecord::new(
        revision,
        Actor::author("same-person"),
        Actor::independent_reviewer("same-person"),
    )
    .against(full_corpus(1));
    for dimension in ReviewDimension::mandatory_for(false) {
        self_review = self_review
            .finding(
                dimension,
                StewardshipFinding::passed("checked against the declared corpus"),
            )
            .unwrap();
    }
    let refused = call(
        &mut server(),
        "stewardship_review_check",
        json!({ "review": serde_json::to_value(self_review).unwrap() }),
    );
    assert_eq!(refused["__isError"], json!(false));
    assert_eq!(refused["decision"], json!("refused"));
    assert!(
        refused["refusal"]
            .as_str()
            .unwrap()
            .contains("authored the evaluator")
    );
}

#[test]
fn foundation_contract_check_keeps_admissibility_world_authority_and_plane_checks_separate() {
    let result = call(
        &mut server(),
        "foundation_contract_check",
        json!({
            "contract": {
                "id": "fbc:test:001",
                "intent": "distinguish two declared outcomes",
                "evidence_obligations": ["reference"],
                "actions": ["inspect", "abstain"],
                "claim_schema": "typed-result-v1",
                "falsifiers": ["reference-disagrees"],
                "reference_standard": "deterministic-fixture",
                "minimum_reviewers": 1,
                "uncertainty_required": true,
                "terminations": ["success", "underdetermined"]
            },
            "world": {
                "id": "observed-world",
                "class": "observed_replay",
                "design_support": [],
                "reveal_policy": null,
                "withholds_for_scoring": false
            },
            "claim": "real_treatment_effect",
            "transition": {
                "plane": "observation",
                "effects": ["latent_biological_state"]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["contract"]["ok"], json!(true));
    assert_eq!(result["world"]["ok"], json!(false));
    assert!(
        result["world"]["claim"]
            .as_str()
            .unwrap()
            .contains("real treatment effect")
    );
    assert_eq!(result["transition"]["ok"], json!(false));
    assert_eq!(result["verdict"], json!("refused"));

    let missing = call(
        &mut server(),
        "foundation_contract_check",
        json!({
            "contract": {
                "id": "fbc:test:missing",
                "intent": "not falsifiable",
                "actions": ["inspect"],
                "claim_schema": "typed-result-v1",
                "reference_standard": "deterministic-fixture",
                "terminations": ["success"]
            }
        }),
    );
    assert_eq!(missing["__isError"], json!(false));
    assert_eq!(missing["verdict"], json!("refused"));
    assert_eq!(missing["contract"]["ok"], json!(false));
    assert!(
        missing["contract"]["refusal"]
            .as_str()
            .unwrap()
            .contains("falsifier")
    );
}
