//! MCP contract tests for developer operations contracts.

use super::*;

#[test]
fn release_audit_composes_required_gates_and_keeps_advisory_impact_separate() {
    let mut server = server();
    let bundle = ResultBundle::builder("release-audit-bundle")
        .carrying("query", EntryRole::Query, json!({ "goal": "release" }))
        .unwrap()
        .build()
        .unwrap();
    let dataset = QualityDataset::new("release-dataset")
        .unwrap()
        .with_column("age", [json!(41), json!(42)])
        .unwrap();
    let gate = QualityGate::new("release-quality")
        .unwrap()
        .with(
            "age_range",
            QualityCheck::InRange {
                column: "age".into(),
                min: 0.0,
                max: 120.0,
            },
        )
        .unwrap();
    let catalogue = call(
        &mut server,
        "repository_catalog",
        json!({ "prefix": "docs/", "limit": 1 }),
    );
    let changed = catalogue["modules"][0]["id"]
        .as_str()
        .expect("repository catalog returns a module id")
        .to_string();
    let payload = call(
        &mut server,
        "release_audit",
        json!({
            "checks": [
                {
                    "kind": "bundle_verify",
                    "arguments": { "bundle": serde_json::to_value(&bundle).unwrap() }
                },
                {
                    "kind": "quality_gate_run",
                    "arguments": {
                        "dataset": serde_json::to_value(dataset).unwrap(),
                        "gate": serde_json::to_value(gate).unwrap()
                    }
                },
                {
                    "kind": "repository_impact",
                    "required": false,
                    "arguments": { "changed": changed }
                }
            ],
            "include_details": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["release_ready"], json!(true));
    assert_eq!(payload["required_check_count"], json!(2));
    assert_eq!(payload["blocking_count"], json!(0));
    assert_eq!(payload["checks"][0]["passed"], json!(true));
    assert_eq!(payload["checks"][1]["passed"], json!(true));
    assert_eq!(payload["checks"][2]["advisory"], json!(true));
    assert_eq!(payload["checks"][2]["gate"], Value::Null);
    assert!(payload["checks"][0]["result"].is_object());
    let mut tampered = serde_json::to_value(bundle).unwrap();
    tampered["contents"]["query"]["goal"] = json!("tampered");
    let blocked = call(
        &mut server,
        "release_audit",
        json!({
            "checks": [
                {
                    "kind": "bundle_verify",
                    "arguments": { "bundle": tampered }
                }
            ]
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(blocked["release_ready"], json!(false));
    assert_eq!(blocked["blocking_count"], json!(1));
    assert_eq!(blocked["checks"][0]["passed"], json!(false));
    assert_eq!(blocked["blockers"][0]["fail_closed"], json!(true));
}

#[test]
fn research_ci_check_keeps_failures_and_undetermined_checks_distinct() {
    let mut server = server();
    let digest = "a".repeat(64);
    let result = json!({
        "subject": "reference-result",
        "observations": [
            { "observation": "claim", "id": "claim-1", "resolves_to": digest },
            { "observation": "split", "name": "train", "members": ["p1"] },
            { "observation": "split", "name": "test", "members": ["p2"] },
            { "observation": "figure", "name": "roc", "declared": digest, "recomputed": digest },
            { "observation": "cell", "id": "cell-1", "previously_passed": true, "passes_now": true },
            { "observation": "dependency", "name": "rustc", "pinned": true },
            { "observation": "egress_event", "connector": "site-a", "permitted": "aggregate_only", "requested": "aggregate_only" },
            { "observation": "non_claim", "statement": "does not establish clinical validity" },
            { "observation": "world_reference", "world": "world-a", "rung": "observed" }
        ]
    });
    let passed = call(
        &mut server,
        "research_ci_check",
        json!({ "result": result.clone() }),
    );
    assert_eq!(passed["ok"], json!(true));
    assert_eq!(passed["publishable"], json!(true));
    assert!(passed["failed_checks"].as_array().unwrap().is_empty());
    assert!(passed["undetermined_checks"].as_array().unwrap().is_empty());
    assert_eq!(passed["check_count"], json!(8));

    let mut regressed = result;
    regressed["observations"][4]["passes_now"] = json!(false);
    let blocked = call(
        &mut server,
        "research_ci_check",
        json!({ "result": regressed }),
    );
    assert_eq!(blocked["publishable"], json!(false));
    assert!(
        blocked["failed_checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check == "decision cell regression")
    );
    assert!(
        blocked["undetermined_checks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn developer_platform_status_verifies_local_contracts_and_marks_foreign_artifacts() {
    let mut server = server();
    let payload = call(&mut server, "developer_platform_status", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["devplat"]["digest"].is_string());
    assert!(payload["walkthroughs"].as_array().unwrap().len() >= 6);
    assert!(
        payload["walkthroughs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["documents_absent_artifact"] == json!(true))
    );
    assert_eq!(payload["cookbook"]["verification"]["clean"], json!(true));
    assert_eq!(payload["diagnostic_catalogue"]["clean"], json!(true));
    assert!(
        payload["developer_contract"]["surface_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(payload["limitations"].as_array().unwrap().len() >= 3);

    let bounded = call(
        &mut server,
        "developer_platform_status",
        json!({ "max_items": 1 }),
    );
    assert_eq!(bounded["detail_mode"], json!("summary"));
    assert_eq!(
        bounded["developer_contract"]["surfaces_returned"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        bounded["developer_contract"]["omitted_surfaces"]
            .as_u64()
            .unwrap()
            > 0
    );

    let detailed = call(
        &mut server,
        "developer_platform_status",
        json!({ "include_details": true, "max_items": 1 }),
    );
    assert_eq!(detailed["detail_mode"], json!("full"));
    assert!(detailed["details"]["developer_contract"].is_array());
}

#[test]
fn developer_delivery_audit_composes_local_health_and_blocks_missing_evidence() {
    let mut server = server();
    let payload = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {
                "id": "delivery-1",
                "targets": [
                    "local_delivery",
                    "developer_platform",
                    "developer_claims",
                    "repository_scope",
                    "sdk_admission",
                    "conformance",
                    "provider_capability",
                    "governance_schema",
                    "release"
                ]
            }
        }),
    );
    assert_eq!(payload["__isError"], json!(false));
    assert_eq!(payload["readiness"]["platform_checks_clean"], json!(true));
    assert_eq!(payload["readiness"]["repository_scope_clean"], json!(false));
    assert_eq!(payload["readiness"]["local_delivery_ready"], json!(false));
    assert_eq!(
        payload["external_surface_posture"]["foreign_artifacts_present"],
        json!(true)
    );
    assert_eq!(
        payload["external_surface_posture"]["local_integration_foundations"][0]["artifact"],
        json!("python/prism_sdk")
    );
    let local_actions = payload["external_surface_posture"]["local_integration_foundations"]
        .as_array()
        .unwrap();
    let autonomous_action = local_actions
        .iter()
        .find(|entry| entry["kind"] == json!("approval_bounded_composite_action"))
        .expect("the in-repository autonomous action is catalogued");
    assert_eq!(autonomous_action["local_ci_exercise"], json!(true));
    assert_eq!(
        autonomous_action["hosted_consumer_execution_verified"],
        json!(false)
    );
    assert_eq!(payload["release_request"]["ready"], json!(false));
    let targets = payload["release_request"]["targets"].as_array().unwrap();
    let sdk = targets
        .iter()
        .find(|row| row["target"] == json!("sdk_admission"))
        .unwrap();
    assert_eq!(sdk["available"], json!(false));
    assert_eq!(sdk["eligible"], json!(false));
    let claims = targets
        .iter()
        .find(|row| row["target"] == json!("developer_claims"))
        .unwrap();
    assert_eq!(claims["eligible"], json!(false));
    assert_eq!(
        claims["blockers"][0],
        json!("unguarded_developer_claims_present")
    );

    let no_request = call(&mut server, "developer_delivery_audit", json!({}));
    assert_eq!(no_request["release_request"]["present"], json!(false));
    assert_eq!(no_request["release_request"]["ready"], json!(false));

    let duplicate = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {
                "id": "duplicate",
                "targets": ["local_delivery", "local_delivery"]
            }
        }),
    );
    assert_eq!(duplicate["__isError"], json!(true));
    assert!(duplicate["error"].as_str().unwrap().contains("duplicate"));
}

#[test]
fn engineering_manifest_audit_keeps_topology_ticket_readiness_and_raci_separate() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-engineering-manifest/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "baseline": {
            "language": "Rust 2021", "runtime": "cargo", "api": "MCP JSON-RPC",
            "storage": "in-memory", "observability": "structured stderr audit", "deployment": "local process"
        },
        "packages": [
            { "id": "core", "path": "crates/core", "language": "rust", "kind": "library", "owner": "platform", "depends_on": [], "public": true },
            { "id": "api", "path": "crates/api", "language": "rust", "kind": "service", "owner": "platform", "depends_on": ["core"], "public": true }
        ],
        "tickets": [
            { "id": "T-001", "title": "ship core", "package": "core", "contract": "core-contract", "status": "done", "depends_on": [], "acceptance": ["core tests pass"] },
            { "id": "T-002", "title": "ship api", "package": "api", "contract": "api-contract", "status": "planned", "depends_on": ["T-001"], "acceptance": ["protocol tests pass"] }
        ],
        "adrs": [{ "id": "ADR-001", "title": "use rust", "status": "accepted", "decision": "Rust owns canonical semantics", "affects": ["core", "api"] }],
        "ownership": [{ "surface": "api", "accountable": "platform-lead", "responsible": ["api-team"], "independent_reviewer": "review-board" }]
    });
    let result = call(
        &mut server,
        "engineering_manifest_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["audit"]["package_order"], json!(["core", "api"]));
    assert_eq!(
        result["audit"]["ticket_readiness"][1]["state"],
        json!("actionable")
    );
    assert_eq!(result["blocking_issue_count"], json!(0));
    assert_eq!(result["manifest_digest"].as_str().unwrap().len(), 64);

    let mut cyclic = manifest;
    cyclic["packages"][0]["depends_on"] = json!(["api", "missing"]);
    cyclic["ownership"][0]["independent_reviewer"] = json!("platform-lead");
    let refused = call(
        &mut server,
        "engineering_manifest_audit",
        json!({ "manifest": cyclic }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["valid"], json!(false));
    assert!(
        refused["audit"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "package_cycle")
    );
    assert!(
        refused["audit"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "reviewer_not_independent")
    );
}

#[test]
fn engineering_execution_plan_derives_waves_critical_path_and_fail_closed_manifest_gate() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-engineering-manifest/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "baseline": { "language": "Rust 2021", "runtime": "cargo", "api": "MCP JSON-RPC", "storage": "in-memory", "observability": "structured stderr audit", "deployment": "local process" },
        "packages": [
            { "id": "core", "path": "crates/core", "language": "rust", "kind": "library", "owner": "platform", "depends_on": [], "public": true },
            { "id": "api", "path": "crates/api", "language": "rust", "kind": "service", "owner": "platform", "depends_on": ["core"], "public": true }
        ],
        "tickets": [
            { "id": "T-001", "title": "ship core", "package": "core", "contract": "core-contract", "status": "done", "depends_on": [], "acceptance": ["core tests pass"] },
            { "id": "T-002", "title": "ship api", "package": "api", "contract": "api-contract", "status": "planned", "depends_on": ["T-001"], "acceptance": ["protocol tests pass"] },
            { "id": "T-003", "title": "publish api", "package": "api", "contract": "release-contract", "status": "planned", "depends_on": ["T-002"], "acceptance": ["release evidence exists"] }
        ],
        "adrs": [{ "id": "ADR-001", "title": "use rust", "status": "accepted", "decision": "Rust owns canonical semantics", "affects": ["core", "api"] }],
        "ownership": [{ "surface": "api", "accountable": "platform-lead", "responsible": ["api-team"], "independent_reviewer": "review-board" }]
    });
    let request =
        json!({ "schema": "bioprism-engineering-plan/0.1", "manifest": manifest.clone() });
    let result = call(
        &mut server,
        "engineering_execution_plan",
        json!({ "request": request.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["engineering_plan_ready"], json!(true));
    assert_eq!(result["audit"]["waves"].as_array().unwrap().len(), 2);
    assert_eq!(result["audit"]["waves"][0]["ticket_ids"], json!(["T-002"]));
    assert_eq!(result["audit"]["waves"][1]["ticket_ids"], json!(["T-003"]));
    assert_eq!(
        result["audit"]["critical_path"],
        json!(["T-001", "T-002", "T-003"])
    );
    assert_eq!(result["audit"]["planned_ticket_count"], json!(2));
    assert_eq!(result["plan_digest"].as_str().unwrap().len(), 64);

    let mut refused = request;
    refused["manifest"]["tickets"][1]["depends_on"] = json!(["missing"]);
    let refusal = call(
        &mut server,
        "engineering_execution_plan",
        json!({ "request": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["engineering_plan_ready"], json!(false));
    assert_eq!(refusal["audit"]["planning_started"], json!(false));
    assert!(
        refusal["audit"]["manifest_issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "missing_ticket_dependency")
    );
    assert!(
        refusal["audit"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "manifest_invalid")
    );
}

#[test]
fn release_pipeline_audit_preserves_promotion_provenance_and_rollback_boundaries() {
    let mut server = server();
    let digest = "a".repeat(64);
    let manifest = json!({
        "schema": "bioprism-release-pipeline/0.1",
        "project": { "id": "aurora-agent", "version": "0.1.0", "repository": "github.com/AURORA-NEURO/aurora-agent" },
        "source": { "ref_name": "main", "commit_digest": digest, "workflow": "release.yml" },
        "environments": [
            { "id": "staging", "class": "staging", "protected": true, "required_approvals": 0, "secrets_allowed": true, "immutable_artifacts": true },
            { "id": "production", "class": "production", "protected": true, "required_approvals": 1, "secrets_allowed": true, "immutable_artifacts": true }
        ],
        "stages": [
            { "id": "build", "kind": "build", "environment": "staging", "depends_on": [], "command": "cargo build --locked", "produces": ["binary"], "required": true },
            { "id": "test", "kind": "test", "environment": "staging", "depends_on": ["build"], "command": "cargo test --locked", "produces": [], "required": true }
        ],
        "artifacts": [{ "id": "binary", "kind": "binary", "digest": digest, "produced_by": "build", "inputs": [], "attestations": ["prov", "sig"], "immutable": true }],
        "attestations": [
            { "id": "prov", "kind": "provenance", "artifact": "binary", "digest": digest, "issuer": "ci", "statement": "built from pinned source" },
            { "id": "sig", "kind": "signature", "artifact": "binary", "digest": digest, "issuer": "release-key", "statement": "signed artifact" },
            { "id": "approval", "kind": "approval", "artifact": "binary", "digest": digest, "issuer": "release-board", "statement": "approved" }
        ],
        "promotions": [
            { "id": "to-production", "kind": "advance", "from": "staging", "to": "production", "artifacts": ["binary"], "required_attestations": ["prov", "sig"], "approvals": ["approval"], "rollback_target": "rollback" },
            { "id": "rollback", "kind": "rollback", "from": "production", "to": "staging", "artifacts": ["binary"], "required_attestations": ["prov"], "approvals": [] }
        ]
    });
    let result = call(
        &mut server,
        "release_pipeline_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["release_ready"], json!(true));
    assert_eq!(result["audit"]["stage_order"], json!(["build", "test"]));
    assert_eq!(
        result["audit"]["promotion_audits"][0]["rollback_present"],
        json!(true)
    );
    assert_eq!(result["blocking_issue_count"], json!(0));

    let mut refused = manifest;
    refused["attestations"][1]["digest"] = json!("b".repeat(64));
    refused["promotions"][0]["rollback_target"] = json!(null);
    refused["stages"][0]["depends_on"] = json!(["test"]);
    let refusal = call(
        &mut server,
        "release_pipeline_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["release_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "attestation_digest_mismatch")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "production_rollback_missing")
    );
    assert!(issues.iter().any(|issue| issue["code"] == "stage_cycle"));
}

#[test]
fn operational_readiness_audit_keeps_observation_fallback_and_incident_closure_explicit() {
    let mut server = server();
    let manifest = json!({
        "schema": "bioprism-operational-readiness/0.1",
        "service": { "id": "prism-api", "version": "0.1.0", "owner": "platform-oncall", "criticality": "critical" },
        "contracts": [{ "id": "availability", "kind": "availability", "objective": "serve health checks", "target": "99.9%", "required": true }],
        "indicators": [{ "id": "availability-sli", "contract": "availability", "metric": "request_success_ratio", "source": "telemetry-digest", "status": "observed", "measurement": "0.999", "evidence_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }],
        "dependencies": [{ "id": "registry", "name": "artifact registry", "owner": "release-team", "criticality": "critical", "failure_mode": "artifact fetch unavailable", "fallback": "pinned offline mirror" }],
        "runbooks": [{ "id": "api-degraded", "trigger": "availability below target", "owner": "platform-oncall", "steps": ["freeze rollout", "restore last known good"], "review_status": "reviewed", "incident_classes": ["availability"] }],
        "incidents": [{ "id": "inc-1", "severity": "sev2", "state": "closed", "runbook": "api-degraded", "owner": "platform-oncall", "timeline": ["detected", "contained", "restored"], "postmortem": "postmortem-digest" }],
        "controls": { "on_call": true, "alerting": true, "tracing": true, "audit_logging": true, "backup": true, "restore_test": true, "access_review": true }
    });
    let result = call(
        &mut server,
        "operational_readiness_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["operationally_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["observed_indicators"], json!(1));
    assert_eq!(
        result["audit"]["dependency_audits"][0]["fallback_present"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["incident_audits"][0]["postmortem_present"],
        json!(true)
    );

    let mut refused = manifest;
    refused["indicators"][0]["status"] = json!("not_observed");
    refused["dependencies"][0]["fallback"] = json!(null);
    refused["controls"]["restore_test"] = json!(false);
    refused["incidents"][0]["postmortem"] = json!(null);
    let refusal = call(
        &mut server,
        "operational_readiness_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["operationally_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "indicator_not_observed")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "critical_dependency_fallback_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "required_control_disabled")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "closed_incident_postmortem_missing")
    );
}

#[test]
fn security_privacy_audit_keeps_asset_flow_identity_threat_and_review_layers_explicit() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-security-privacy/0.1",
        "system": { "id": "prism-api", "version": "0.1.0", "owner": "platform" },
        "assets": [{ "id": "patient-records", "name": "records", "classification": "regulated", "owner": "privacy", "purpose": "care research", "retention_days": 365, "residency": "us", "deletion_process": "erase workflow" }],
        "flows": [{ "id": "api-to-vendor", "asset": "patient-records", "source": "api", "destination": "approved-vendor", "purpose": "care research", "legal_basis": "consent", "decision": "allow", "authorization_evidence": digest }],
        "identities": [{ "id": "researcher", "principal": "team", "role": "research", "authentication": "oidc", "mfa": true, "least_privilege": true, "assets": ["patient-records"] }],
        "threats": [{ "id": "exfiltration", "category": "data-exfiltration", "severity": "high", "status": "mitigated", "control": "audit_logging", "evidence_digest": digest }],
        "reviews": [{ "id": "pia-1", "kind": "privacy_impact", "scope": "patient-records", "reviewer": "independent-reviewer", "status": "complete", "evidence_digest": digest, "expires_at": "2027-01-01", "findings": ["none"] }],
        "controls": { "access_control": true, "encryption_at_rest": true, "encryption_in_transit": true, "key_rotation": true, "audit_logging": true, "vulnerability_management": true, "backup_restore": true, "incident_response": true, "vendor_review": true, "data_subject_rights": true }
    });
    let result = call(
        &mut server,
        "security_privacy_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["security_privacy_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["sensitive_assets"], json!(1));
    assert_eq!(
        result["audit"]["flow_audits"][0]["authorization_present"],
        json!(true)
    );
    assert_eq!(result["audit"]["identity_audits"][0]["ready"], json!(true));
    assert_eq!(
        result["audit"]["threat_audits"][0]["evidence_valid"],
        json!(true)
    );
    assert_eq!(result["audit"]["review_audits"][0]["complete"], json!(true));

    let mut refused = manifest;
    refused["assets"][0]["retention_days"] = Value::Null;
    refused["flows"][0]["authorization_evidence"] = Value::Null;
    refused["identities"][0]["mfa"] = json!(false);
    refused["threats"][0]["evidence_digest"] = Value::Null;
    refused["reviews"][0]["status"] = json!("expired");
    refused["controls"]["encryption_at_rest"] = json!(false);
    let refusal = call(
        &mut server,
        "security_privacy_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["security_privacy_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "sensitive_retention_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "flow_authorization_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "sensitive_mfa_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "mitigation_evidence_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "review_evidence_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "required_control_disabled")
    );
}

#[test]
fn sandbox_admission_audit_keeps_artifact_isolation_capability_resource_and_output_layers_explicit()
{
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-sandbox/0.1",
        "system": { "id": "prism-sandbox", "version": "0.1.0", "owner": "platform" },
        "artifacts": [
            { "id": "source", "kind": "source_code", "digest": digest, "source": "repo/source.py", "producer": "ci", "trust": "reviewed" },
            { "id": "dataset", "kind": "dataset", "digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "source": "registry/dataset", "producer": "registry", "trust": "untrusted", "inputs": ["source"] }
        ],
        "profiles": [{
            "id": "profile", "artifact": "dataset", "runtime": "oci", "image_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "environment_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", "user": "runner", "rootless": true, "read_only_root": true, "no_privilege_escalation": true, "network": "allowlist", "network_allowlist": ["packages.example"], "mounts": [{ "id": "input", "source_artifact": "dataset", "target": "/inputs/data", "mode": "read_only" }], "capabilities": ["network"], "resources": { "cpu_millis": 1000, "memory_mb": 1024, "wall_time_seconds": 60, "processes": 8, "output_bytes": 1000000 }, "output_quarantine": true, "release_requires_review": true
        }],
        "capabilities": [{ "id": "network", "profile": "profile", "kind": "network_egress", "target": "packages.example", "decision": "allow", "evidence_digest": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" }],
        "outputs": [{ "id": "result", "profile": "profile", "artifact": "dataset", "digest": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "destination": "quarantine", "quarantined": true, "released": false, "reviewed": false, "parents": ["dataset"] }]
    });
    let result = call(
        &mut server,
        "sandbox_admission_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["sandbox_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["untrusted_artifacts"], json!(1));
    assert_eq!(
        result["audit"]["profile_audits"][0]["isolation_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["capability_audits"][0]["evidence_valid"],
        json!(true)
    );
    assert_eq!(result["audit"]["resource_audits"][0]["ready"], json!(true));
    assert_eq!(
        result["audit"]["output_audits"][0]["quarantined"],
        json!(true)
    );

    let mut refused = manifest;
    refused["profiles"][0]["rootless"] = json!(false);
    refused["profiles"][0]["network"] = json!("unrestricted");
    refused["profiles"][0]["resources"]["memory_mb"] = Value::Null;
    refused["capabilities"][0]["target"] = json!("*");
    refused["capabilities"][0]["evidence_digest"] = Value::Null;
    let refusal = call(
        &mut server,
        "sandbox_admission_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["sandbox_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "rootless_required")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "network_boundary_invalid")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "resource_limits_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "capability_target_broad")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "dangerous_capability_evidence_missing")
    );
}

#[test]
fn sandbox_runtime_simulate_preserves_admission_capability_resource_and_refusal_layers() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let runtime_manifest = json!({
        "schema": "bioprism-sandbox-runtime/0.1",
        "admission": {
            "schema": "bioprism-sandbox/0.1",
            "system": { "id": "runtime", "version": "0.1.0", "owner": "platform" },
            "artifacts": [
                { "id": "source", "kind": "source_code", "digest": digest, "source": "repo/source.py", "producer": "ci", "trust": "reviewed" },
                { "id": "dataset", "kind": "dataset", "digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "source": "registry/dataset", "producer": "registry", "trust": "untrusted", "inputs": ["source"] }
            ],
            "profiles": [{
                "id": "profile", "artifact": "dataset", "runtime": "oci", "image_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "environment_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", "user": "runner", "rootless": true, "read_only_root": true, "no_privilege_escalation": true, "network": "allowlist", "network_allowlist": ["packages.example"], "mounts": [{ "id": "input", "source_artifact": "dataset", "target": "/inputs/data", "mode": "read_only" }], "capabilities": ["read", "network"], "resources": { "cpu_millis": 1000, "memory_mb": 1024, "wall_time_seconds": 60, "processes": 8, "output_bytes": 1000000 }, "output_quarantine": true, "release_requires_review": true
            }],
            "capabilities": [
                { "id": "read", "profile": "profile", "kind": "filesystem_read", "target": "/inputs/data", "decision": "allow" },
                { "id": "network", "profile": "profile", "kind": "network_egress", "target": "packages.example", "decision": "allow", "evidence_digest": digest }
            ]
        },
        "profile": "profile",
        "requests": [
            { "id": "read-input", "kind": "filesystem_read", "target": "/inputs/data", "cpu_millis": 100, "memory_mb": 128, "wall_time_seconds": 5, "processes": 1, "output_bytes": 1000 },
            { "id": "fetch-package", "kind": "network_egress", "target": "packages.example", "cpu_millis": 100, "memory_mb": 128, "wall_time_seconds": 5, "processes": 1, "output_bytes": 1000 }
        ]
    });
    let result = call(
        &mut server,
        "sandbox_runtime_simulate",
        json!({ "manifest": runtime_manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["sandbox_runtime_ready"], json!(true));
    assert_eq!(result["audit"]["admission_valid"], json!(true));
    assert_eq!(result["audit"]["simulated_count"], json!(2));
    assert_eq!(result["audit"]["usage"]["cpu_millis"], json!(200));
    assert_eq!(result["audit"]["steps"][0]["decision"], json!("simulated"));
    assert!(result["trace_digest"].is_string());

    let mut refused = runtime_manifest;
    refused["requests"][0]["cpu_millis"] = json!(2000);
    let refusal = call(
        &mut server,
        "sandbox_runtime_simulate",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["audit"]["refused_count"], json!(1));
    assert_eq!(refusal["audit"]["not_run_count"], json!(1));
    assert_eq!(refusal["audit"]["stopped_on_refusal"], json!(true));
    assert_eq!(
        refusal["audit"]["steps"][0]["refusal"],
        json!("resource_budget_exceeded")
    );
    assert_eq!(refusal["audit"]["steps"][1]["decision"], json!("not_run"));
}

#[test]
fn security_program_audit_keeps_scope_campaign_finding_incident_and_disclosure_layers_explicit() {
    let mut server = server();
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = json!({
        "schema": "bioprism-security-program/0.1",
        "system": { "id": "aurora-security", "version": "0.1.0", "owner": "security-owner", "mission": "bounded adversarial assurance" },
        "scopes": [{ "id": "api-staging", "name": "staging API", "kind": "api", "target": "api-staging.internal", "owner": "service-owner", "authorization_digest": digest, "allowed_methods": ["authenticated-read", "rate-limited-input"], "forbidden_actions": ["production-write", "credential-exfiltration"], "environments": ["isolated-staging"], "data_handling": "synthetic fixtures only" }],
        "campaigns": [{ "id": "campaign-1", "scope": "api-staging", "operator": "red-team", "independent_reviewer": "independent-reviewer", "methodology": "bounded mutation and manual review", "hypothesis": "invalid input can cross a trust boundary", "status": "completed", "started_at": "2026-01-01", "completed_at": "2026-01-02", "evidence_digest": digest, "stop_conditions": ["stop on production boundary"], "finding_ids": ["finding-1"] }],
        "findings": [{ "id": "finding-1", "campaign": "campaign-1", "title": "boundary mismatch", "severity": "high", "status": "closed", "evidence_digest": digest, "reproduction_digest": digest, "regression_digest": digest, "discovered_at": "2026-01-02", "affected_targets": ["api-staging"], "remediation_ids": ["remediation-1"], "incident_id": "incident-1", "public_safe": true }],
        "remediations": [{ "id": "remediation-1", "finding": "finding-1", "owner": "service-owner", "action": "validate boundary before dispatch", "status": "complete", "due_at": "2026-01-10", "verification_digest": digest }],
        "incidents": [{ "id": "incident-1", "finding": "finding-1", "severity": "high", "owner": "incident-owner", "status": "closed", "opened_at": "2026-01-02", "contained_at": "2026-01-02", "closed_at": "2026-01-03", "containment_evidence": digest, "closure_evidence": digest, "notification_required": true, "timeline": [{ "epoch": 1, "actor": "incident-owner", "event": "incident opened", "evidence_digest": digest }, { "epoch": 2, "actor": "incident-owner", "event": "containment verified", "evidence_digest": digest }] }],
        "disclosures": [{ "id": "advisory-1", "finding": "finding-1", "stage": "advisory", "audience": "affected operators", "requested_at": "2026-01-04", "approver": "independent-reviewer", "approval_digest": digest, "advisory_digest": digest, "published_at": "2026-01-04" }],
        "controls": { "scope_authorization": true, "operator_separation": true, "independent_review": true, "evidence_retention": true, "remediation_tracking": true, "incident_response": true, "disclosure_review": true, "regression_testing": true }
    });
    let result = call(
        &mut server,
        "security_program_audit",
        json!({ "manifest": manifest.clone() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["security_program_ready"], json!(true));
    assert_eq!(result["audit"]["counts"]["authorized_scopes"], json!(1));
    assert_eq!(
        result["audit"]["finding_audits"][0]["incident_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["remediation_audits"][0]["verification_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["incident_audits"][0]["closure_valid"],
        json!(true)
    );
    assert_eq!(
        result["audit"]["disclosure_audits"][0]["approval_valid"],
        json!(true)
    );

    let mut refused = manifest;
    refused["scopes"][0]["authorization_digest"] = Value::Null;
    refused["campaigns"][0]["independent_reviewer"] = Value::Null;
    refused["findings"][0]["evidence_digest"] = Value::Null;
    refused["remediations"][0]["verification_digest"] = Value::Null;
    refused["incidents"][0]["closure_evidence"] = Value::Null;
    refused["controls"]["disclosure_review"] = json!(false);
    let refusal = call(
        &mut server,
        "security_program_audit",
        json!({ "manifest": refused }),
    );
    assert_eq!(refusal["ok"], json!(true));
    assert_eq!(refusal["valid"], json!(false));
    assert_eq!(refusal["security_program_ready"], json!(false));
    let issues = refusal["audit"]["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "scope_authorization_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "campaign_independent_review_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "finding_evidence_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "remediation_verification_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "incident_closure_missing")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "required_control_disabled")
    );
}

#[test]
fn developer_workbench_audits_notebook_digests_queries_dashboard_and_plans_ci() {
    let mut server = server();
    let digest = "a".repeat(64);
    let output_digest = "b".repeat(64);
    let payload = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {
                "session_id": "studio-1",
                "owner": "agent-a",
                "goal": "author an oncology capability card",
                "environment_digest": digest,
                "artifacts": [{
                    "id": "artifact-1",
                    "title": "verification card",
                    "path": "artifacts/verification.json",
                    "domain": "oncology",
                    "capability": "verification",
                    "state": "validated",
                    "evidence": "reproduced",
                    "digest": digest,
                    "score": 0.8,
                    "tags": ["public-card"]
                }],
                "cells": [{
                    "id": "cell-1",
                    "kind": "query",
                    "source": "workspace.metrics_analytics_audit(...)" ,
                    "inputs": [{"artifact_id": "artifact-1", "digest": digest}],
                    "depends_on": [],
                    "executed": true,
                    "output_digest": output_digest
                }],
                "changes": [{
                    "id": "change-1",
                    "artifact_id": "artifact-1",
                    "kind": "create",
                    "actor": "agent-a",
                    "logical_time": 1,
                    "input_digest": null,
                    "output_digest": digest,
                    "reason": "initial artifact"
                }]
            },
            "dashboard": {"domains": ["oncology"], "include_holes": true, "limit": 10},
            "ci": {
                "workflow": "consumer contracts",
                "triggers": ["push", "pull_request"],
                "rust_toolchain": "1.85.0",
                "offline": true,
                "checks": [{"name": "tests", "run": "cargo test --workspace --offline", "required": true}]
            }
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["workflow"], json!("developer_workbench"));
    assert_eq!(payload["audit"]["stale_cells"].as_array().unwrap().len(), 0);
    assert_eq!(payload["dashboard"]["rows"][0]["score"], json!(0.8));
    assert_eq!(payload["ci"]["execution"], json!("not_executed"));
    assert_eq!(payload["ci"]["network_access"], json!("denied_by_plan"));
    assert!(
        payload["ci"]["workflow_yaml"]
            .as_str()
            .unwrap()
            .contains("cargo test --workspace --offline")
    );

    let mut stale = json!({
        "session": {
            "session_id": "studio-2", "owner": "agent-a", "goal": "stale check",
            "artifacts": [{
                "id": "artifact-1", "title": "card", "path": "card.json", "domain": "bioir",
                "capability": "retrieval", "state": "validated", "evidence": "observed", "digest": digest
            }],
            "cells": [{
                "id": "cell-1", "kind": "review", "source": "review", "inputs": [{"artifact_id": "artifact-1", "digest": digest}],
                "depends_on": [], "executed": true, "output_digest": output_digest
            }],
            "changes": []
        }
    });
    stale["session"]["artifacts"][0]["digest"] = "c".repeat(64).into();
    let stale_result = call(&mut server, "developer_workbench", stale);
    assert_eq!(stale_result["__isError"], json!(false));
    assert_eq!(stale_result["audit"]["stale_cells"], json!(["cell-1"]));
    assert_eq!(stale_result["audit"]["release_ready"], json!(false));
}

#[test]
fn developer_workbench_refuses_notebook_cycles_and_unsafe_ci() {
    let mut server = server();
    let cycle = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {
                "session_id": "cycle", "owner": "agent-a", "goal": "cycle",
                "artifacts": [],
                "cells": [
                    {"id": "a", "kind": "review", "source": "a", "depends_on": ["b"], "executed": true, "output_digest": "a".repeat(64)},
                    {"id": "b", "kind": "review", "source": "b", "depends_on": ["a"], "executed": true, "output_digest": "b".repeat(64)}
                ],
                "changes": []
            }
        }),
    );
    assert_eq!(cycle["__isError"], json!(true));
    assert!(cycle["error"].as_str().unwrap().contains("cycle"));

    let unsafe_ci = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "ci", "owner": "agent-a", "goal": "ci", "artifacts": [], "cells": [], "changes": []},
            "ci": {
                "workflow": "ci", "triggers": ["push"], "rust_toolchain": "stable",
                "checks": [{"name": "bad", "run": "cargo test", "working_directory": "../outside"}]
            }
        }),
    );
    assert_eq!(unsafe_ci["__isError"], json!(true));
    assert!(
        unsafe_ci["error"]
            .as_str()
            .unwrap()
            .contains("parent directory")
    );
}

#[test]
fn developer_workbench_verify_replays_retained_projection_without_execution() {
    let mut server = server();
    let digest = "a".repeat(64);
    let output_digest = "b".repeat(64);
    let session = json!({
        "session_id": "verify-studio",
        "owner": "agent-a",
        "goal": "verify an oncology authoring handoff",
        "artifacts": [{
            "id": "artifact-1", "title": "card", "path": "card.json", "domain": "oncology",
            "capability": "verification", "state": "validated", "evidence": "reproduced",
            "digest": digest, "score": 0.9
        }],
        "cells": [{
            "id": "cell-1", "kind": "query", "source": "metrics", "inputs": [{"artifact_id": "artifact-1", "digest": digest}],
            "depends_on": [], "executed": true, "output_digest": output_digest
        }],
        "changes": [{
            "id": "change-1", "artifact_id": "artifact-1", "kind": "create", "actor": "agent-a",
            "logical_time": 1, "output_digest": digest, "reason": "initial card"
        }]
    });
    let ci = json!({
        "workflow": "consumer contracts", "triggers": ["pull_request"], "rust_toolchain": "stable",
        "offline": true, "checks": [{"name": "unit", "run": "cargo test -p bioprism-devplat", "required": true}]
    });
    let retained = call(
        &mut server,
        "developer_workbench",
        json!({"session": session.clone(), "dashboard": {"domains": ["oncology"], "limit": 4}, "ci": ci.clone()}),
    );
    assert_eq!(retained["ok"], json!(true));
    let verified = call(
        &mut server,
        "developer_workbench_verify",
        json!({
            "session": session.clone(),
            "report": retained.clone(),
            "ci_replay": ci.clone(),
            "policy": {"require_dashboard": true, "require_ci": true, "require_ci_replay": true}
        }),
    );
    assert_eq!(verified["ok"], json!(true));
    assert_eq!(verified["workflow"], json!("developer_workbench_verify"));
    assert_eq!(verified["valid"], json!(true));
    assert_eq!(verified["status"], json!("verified"));
    assert_eq!(verified["dashboard_verified"], json!(true));
    assert_eq!(verified["ci_verified"], json!(true));
    assert_eq!(verified["execution"], json!("not_started"));
    assert_eq!(verified["network_access"], json!("not_started"));

    let mut tampered = retained;
    tampered["audit"]["ordered_cells"] = json!([]);
    let mismatch = call(
        &mut server,
        "developer_workbench_verify",
        json!({"session": session, "report": tampered, "ci_replay": ci}),
    );
    assert_eq!(mismatch["ok"], json!(true));
    assert_eq!(mismatch["valid"], json!(false));
    assert_eq!(mismatch["status"], json!("mismatch"));
    assert!(
        mismatch["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "audit_mismatch")
    );
}

#[test]
fn developer_workbench_registry_retains_queries_and_fetches_reports() {
    let mut server = server();
    let retained = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {
                "session_id": "registry-protocol",
                "owner": "agent-a",
                "goal": "retain a cross-domain report",
                "artifacts": [{
                    "id": "artifact-1", "title": "result", "path": "result.json",
                    "domain": "oncology", "capability": "evidence", "state": "validated",
                    "evidence": "observed", "digest": "a".repeat(64), "score": 0.8
                }],
                "cells": [], "changes": []
            },
            "dashboard": {"domains": ["oncology"], "limit": 10}
        }),
    );
    assert_eq!(retained["ok"], json!(true));
    let imported = call(
        &mut server,
        "developer_workbench_import",
        json!({"report": retained.clone()}),
    );
    assert_eq!(imported["ok"], json!(true));
    assert_eq!(imported["created"], json!(true));
    let digest = imported["workbench_report_digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);

    let repeated = call(
        &mut server,
        "developer_workbench_import",
        json!({"report": retained}),
    );
    assert_eq!(repeated["already_present"], json!(true));
    let queried = call(
        &mut server,
        "developer_workbench_query",
        json!({"domain": "oncology", "capability": "evidence"}),
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["workbench_report_digest"], digest);
    let fetched = call(
        &mut server,
        "developer_workbench_get",
        json!({"workbench_report_digest": digest}),
    );
    assert_eq!(fetched["ok"], json!(true));
    assert_eq!(fetched["workbench_report_digest"], digest);
    assert_eq!(
        fetched["report"]["schema_version"],
        "bioprism-devplat-workbench/0.1"
    );
}

#[test]
fn ci_execution_evidence_audit_reconciles_plan_and_run_without_execution() {
    let mut server = server();
    let ci = json!({
        "workflow": "consumer contracts",
        "triggers": ["push", "pull_request"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [
            {"name": "tests", "run": "cargo test --workspace --offline", "required": true},
            {"name": "lint", "run": "cargo clippy --workspace --offline", "required": false}
        ]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "ci-evidence", "owner": "agent-a", "goal": "reconcile CI", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let plan_digest = planned["ci"]["digest"].as_str().unwrap().to_owned();
    let result = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({
            "ci": ci.clone(),
            "evidence": {
                "run_id": "run-42",
                "provider": "github_actions",
                "source": "provider_observed",
                "plan_digest": plan_digest,
                "conclusion": "success",
                "checks": [
                    {"name": "tests", "status": "passed", "result_digest": "a".repeat(64)},
                    {"name": "lint", "status": "passed", "result_digest": "b".repeat(64)}
                ]
            }
        }),
    );
    assert_eq!(result["workflow"], json!("ci_execution_evidence_audit"));
    assert_eq!(result["valid"], json!(true));
    assert_eq!(result["ci_evidence_ready"], json!(true));
    assert_eq!(result["audit"]["complete"], json!(true));
    assert_eq!(result["audit"]["passed_check_count"], json!(2));
    assert_eq!(result["audit"]["verification"], json!("structural_only"));
    assert_eq!(
        result["audit"]["execution"],
        json!("evidence_supplied_not_executed_here")
    );

    let incomplete = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({
            "ci": ci,
            "evidence": {
                "run_id": "run-43",
                "provider": "caller",
                "source": "caller_attested",
                "plan_digest": result["plan_digest"].clone(),
                "conclusion": "success",
                "checks": [
                    {"name": "tests", "status": "failed", "result_digest": "c".repeat(64)}
                ]
            }
        }),
    );
    assert_eq!(incomplete["valid"], json!(false));
    assert_eq!(incomplete["ci_evidence_ready"], json!(false));
    assert_eq!(incomplete["audit"]["required_failed"], json!(["tests"]));
    assert!(
        incomplete["audit"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "missing_check_evidence")
    );
}

#[test]
fn ci_provider_normalize_projects_github_payload_into_auditable_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "provider-normalizer",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [
            {"name": "tests", "run": "cargo test --workspace --offline", "required": true},
            {"name": "lint", "run": "cargo clippy --workspace --offline", "required": false}
        ]
    });
    let normalized = call(
        &mut server,
        "ci_provider_normalize",
        json!({
            "ci": ci.clone(),
            "provider": "github_actions",
            "payload": {
                "run": {"id": 9001, "conclusion": "success", "html_url": "https://example.test/runs/9001"},
                "jobs": [
                    {"name": "tests", "conclusion": "success"},
                    {"name": "lint", "conclusion": "success"}
                ]
            }
        }),
    );
    assert_eq!(normalized["workflow"], json!("ci_provider_normalize"));
    assert_eq!(normalized["provider"], json!("github_actions"));
    assert_eq!(normalized["source"], json!("provider_observed"));
    assert_eq!(normalized["run_id"], json!("9001"));
    assert_eq!(normalized["derived_result_digest_count"], json!(2));
    assert_eq!(
        normalized["evidence"]["checks"][0]["status"],
        json!("passed")
    );
    assert_eq!(
        normalized["evidence"]["run_url"],
        json!("https://example.test/runs/9001")
    );

    let gitlab = call(
        &mut server,
        "ci_provider_normalize",
        json!({
            "ci": ci.clone(),
            "provider": "gitlab_ci",
            "payload": {
                "pipeline": {"id": 9002, "status": "success", "web_url": "https://gitlab.example/pipelines/9002"},
                "jobs": [
                    {"name": "tests", "status": "success", "duration": 1.5},
                    {"name": "lint", "status": "skipped"}
                ]
            }
        }),
    );
    assert_eq!(gitlab["source"], json!("provider_observed"));
    assert_eq!(gitlab["run_id"], json!("9002"));
    assert_eq!(gitlab["evidence"]["checks"][0]["duration_ms"], json!(1500));

    let audited = call(
        &mut server,
        "ci_execution_evidence_audit",
        json!({"ci": ci, "evidence": normalized["evidence"].clone()}),
    );
    assert_eq!(audited["valid"], json!(true));
    assert_eq!(audited["ci_evidence_ready"], json!(true));
}

#[test]
fn ci_provider_evidence_audit_binds_rows_and_preserves_structural_limits() {
    let mut server = server();
    let ci = json!({
        "workflow": "provider-evidence",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let digest = |label: &str| ContentHash::of_bytes(label.as_bytes()).to_string();
    let audited = call(
        &mut server,
        "ci_provider_evidence_audit",
        json!({
            "ci": ci.clone(),
            "provider": "github_actions",
            "payload": {
                "run": {"id": 9020, "conclusion": "success"},
                "jobs": [{"name": "tests", "conclusion": "success"}]
            },
            "artifacts": [{
                "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                "check": "tests", "run_id": "9020", "provider": "github_actions",
                "uri": "https://example.test/artifact"
            }],
            "logs": [{
                "id": "log-tests", "digest": digest("log"), "check": "tests",
                "run_id": "9020", "provider": "github_actions", "truncated": false
            }],
            "attestations": [{
                "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                "statement_digest": digest("statement"), "method": "declared_provider_statement"
            }]
        }),
    );
    assert_eq!(audited["workflow"], json!("ci_provider_evidence_audit"));
    assert_eq!(audited["valid"], json!(true));
    assert_eq!(audited["conformance_ready"], json!(true));
    assert_eq!(audited["audit"]["artifact_count"], json!(1));
    assert_eq!(audited["audit"]["linked_log_count"], json!(1));
    assert_eq!(audited["audit"]["attestation_subject_count"], json!(1));
    assert_eq!(audited["audit"]["verification"], json!("structural_only"));

    let tampered = json!({
        "ci": ci,
        "provider": "github_actions",
        "payload": {
            "run": {"id": 9020, "conclusion": "success"},
            "jobs": [{"name": "tests", "conclusion": "success"}]
        },
        "artifacts": [{
            "id": "artifact-tests", "kind": "junit", "digest": "z".repeat(64),
            "check": "unknown", "run_id": "wrong", "provider": "wrong"
        }],
        "attestations": [{
            "id": "attestation-tests", "subject": "missing", "issuer": "caller",
            "statement_digest": digest("statement"), "method": "declared"
        }]
    });
    let refused_rows = call(&mut server, "ci_provider_evidence_audit", tampered);
    assert_eq!(refused_rows["valid"], json!(false));
    assert_eq!(refused_rows["conformance_ready"], json!(false));
    for code in [
        "artifact_digest_invalid",
        "unknown_check_binding",
        "run_binding_mismatch",
        "provider_binding_mismatch",
        "attestation_subject_unknown",
    ] {
        assert!(
            refused_rows["audit"]["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["code"] == code)
        );
    }
}

#[test]
fn ci_provider_evidence_registry_import_query_get_and_failed_retention_are_deterministic() {
    let mut server = server();
    let ci = json!({
        "workflow": "provider-registry",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let digest = |label: &str| ContentHash::of_bytes(label.as_bytes()).to_string();
    let request = json!({
        "ci": ci.clone(),
        "provider": "github_actions",
        "payload": {
            "run": {"id": 9030, "conclusion": "success"},
            "jobs": [{"name": "tests", "conclusion": "success"}]
        },
        "artifacts": [{
            "id": "artifact-registry", "kind": "junit", "digest": digest("artifact-registry"),
            "check": "tests", "run_id": "9030", "provider": "github_actions"
        }],
        "logs": [{
            "id": "log-registry", "digest": digest("log-registry"), "check": "tests",
            "run_id": "9030", "provider": "github_actions"
        }],
        "attestations": [{
            "id": "attestation-registry", "subject": "artifact-registry", "issuer": "ci",
            "statement_digest": digest("statement-registry"), "method": "declared"
        }]
    });
    let imported = call(&mut server, "ci_provider_evidence_import", request.clone());
    assert_eq!(imported["created"], json!(true));
    assert_eq!(imported["conformance_ready"], json!(true));
    assert_eq!(
        imported["artifact_record_digest"].as_str().unwrap().len(),
        64
    );
    assert_eq!(imported["log_record_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        imported["attestation_record_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    let digest = imported["provider_evidence_digest"].clone();
    let duplicate = call(&mut server, "ci_provider_evidence_import", request);
    assert_eq!(duplicate["created"], json!(false));
    assert_eq!(duplicate["already_present"], json!(true));
    assert_eq!(duplicate["provider_evidence_digest"], digest);
    let queried = call(
        &mut server,
        "ci_provider_evidence_query",
        json!({"provider": "github_actions", "conformance_ready": true, "include_records": true}),
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["provider_evidence_digest"], digest);
    assert_eq!(queried["rows"][0]["audit"]["artifact_count"], json!(1));
    let fetched = call(
        &mut server,
        "ci_provider_evidence_get",
        json!({"provider_evidence_digest": digest}),
    );
    assert_eq!(fetched["audit"]["run_id"], json!("9030"));
    assert_eq!(fetched["audit"]["verification"], json!("structural_only"));

    let failed = call(
        &mut server,
        "ci_provider_evidence_import",
        json!({
            "ci": ci,
            "provider": "generic",
            "payload": {
                "run_id": "9031",
                "conclusion": "failure",
                "checks": [{"name": "tests", "status": "failure"}]
            }
        }),
    );
    assert_eq!(failed["created"], json!(true));
    assert_eq!(failed["structurally_valid"], json!(true));
    assert_eq!(failed["conformance_ready"], json!(false));
    let failed_rows = call(
        &mut server,
        "ci_provider_evidence_query",
        json!({"conformance_ready": false}),
    );
    assert_eq!(failed_rows["rows"].as_array().unwrap().len(), 1);
}

#[test]
fn developer_delivery_composes_provider_normalization_into_ci_release_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery-provider",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_provider": {
                "ci": ci,
                "provider": "github_actions",
                "payload": {
                    "run": {"id": 9010, "conclusion": "success"},
                    "jobs": [{"name": "tests", "conclusion": "success"}]
                }
            },
            "release_request": {"id": "delivery-provider-1", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(delivery["workflow"], json!("developer_delivery_audit"));
    assert_eq!(
        delivery["ci_provider_normalization"]["provider"],
        json!("github_actions")
    );
    assert_eq!(
        delivery["ci_provider_normalization"]["run_id"],
        json!("9010")
    );
    assert_eq!(
        delivery["readiness"]["ci_execution_evidence_ready"],
        json!(true)
    );
    assert_eq!(delivery["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(delivery["release_request"]["ready"], json!(true));

    let rejected = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_evidence": {"ci": {}, "evidence": {}},
            "ci_provider": {"ci": {}, "provider": "generic", "payload": {"run_id": "1", "checks": []}}
        }),
    );
    assert_eq!(rejected["__isError"], json!(true));
}

#[test]
fn provider_evidence_flows_into_delivery_receipts_and_tamper_verification() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery-provider-evidence",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "offline": true,
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let digest = |label: &str| ContentHash::of_bytes(label.as_bytes()).to_string();
    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_provider_evidence": {
                "ci": ci.clone(),
                "provider": "github_actions",
                "payload": {
                    "run": {"id": 9030, "conclusion": "success"},
                    "jobs": [{"name": "tests", "conclusion": "success"}]
                },
                "artifacts": [{
                    "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                    "check": "tests", "run_id": "9030", "provider": "github_actions"
                }],
                "logs": [{
                    "id": "log-tests", "digest": digest("log"), "check": "tests",
                    "run_id": "9030", "provider": "github_actions"
                }],
                "attestations": [{
                    "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                    "statement_digest": digest("statement"), "method": "declared"
                }]
            },
            "release_request": {"id": "delivery-provider-evidence-1", "targets": ["ci_provider_evidence"]}
        }),
    );
    assert_eq!(
        delivery["readiness"]["ci_provider_evidence_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["ci_provider_evidence"]["conformance_ready"],
        json!(true)
    );
    assert_eq!(delivery["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(
        delivery["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(delivery["release_request"]["ready"], json!(true));

    let receipt = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-provider-evidence-1",
            "delivery": {
                "ci_provider_evidence": {
                    "ci": ci,
                    "provider": "github_actions",
                    "payload": {
                        "run": {"id": 9030, "conclusion": "success"},
                        "jobs": [{"name": "tests", "conclusion": "success"}]
                    },
                    "artifacts": [{
                        "id": "artifact-tests", "kind": "junit", "digest": digest("artifact"),
                        "check": "tests", "run_id": "9030", "provider": "github_actions"
                    }],
                    "logs": [{
                        "id": "log-tests", "digest": digest("log"), "check": "tests",
                        "run_id": "9030", "provider": "github_actions"
                    }],
                    "attestations": [{
                        "id": "attestation-tests", "subject": "artifact-tests", "issuer": "caller",
                        "statement_digest": digest("statement"), "method": "declared"
                    }]
                },
                "release_request": {"id": "delivery-provider-evidence-1", "targets": ["ci_provider_evidence"]}
            }
        }),
    );
    assert_eq!(receipt["valid"], json!(true));
    assert_eq!(receipt["receipt_ready"], json!(true));
    assert_eq!(
        receipt["evidence"][10]["name"],
        json!("ci_provider_evidence")
    );
    assert_eq!(receipt["evidence"][10]["ready"], json!(true));
    assert_eq!(
        receipt["evidence"][10]["digest"].as_str().unwrap().len(),
        64
    );

    let verified = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": receipt.clone(), "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(verified["verified"], json!(true));
    let mut tampered = receipt.clone();
    tampered["evidence"][10]["digest"] = json!("0".repeat(64));
    let rejected = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": tampered, "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(rejected["verified"], json!(false));
    assert!(
        rejected["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "evidence_mismatch")
    );
}

#[test]
fn developer_delivery_can_gate_ci_evidence_only_when_explicitly_requested() {
    let mut server = server();
    let ci = json!({
        "workflow": "delivery contracts",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "delivery-ci", "owner": "agent-a", "goal": "delivery evidence", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let evidence = json!({
        "run_id": "delivery-run",
        "provider": "github_actions",
        "source": "provider_observed",
        "plan_digest": planned["ci"]["digest"].clone(),
        "conclusion": "success",
        "checks": [{"name": "tests", "status": "passed", "result_digest": "a".repeat(64)}]
    });
    let payload = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "ci_evidence": {"ci": ci.clone(), "evidence": evidence},
            "release_request": {"id": "delivery-ci-1", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(
        payload["readiness"]["ci_execution_evidence_ready"],
        json!(true)
    );
    assert_eq!(payload["ci_evidence"]["ci_evidence_ready"], json!(true));
    assert_eq!(
        payload["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(payload["release_request"]["ready"], json!(true));
    assert_eq!(
        payload["release_request"]["targets"][0]["eligible"],
        json!(true)
    );

    let missing = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {"id": "delivery-ci-2", "targets": ["ci_execution_evidence"]}
        }),
    );
    assert_eq!(
        missing["readiness"]["ci_execution_evidence_ready"],
        json!(false)
    );
    assert_eq!(missing["release_request"]["ready"], json!(false));
    assert_eq!(
        missing["release_request"]["targets"][0]["blockers"],
        json!(["ci_evidence_arguments_missing"])
    );
}

#[test]
fn developer_delivery_receipt_canonicalizes_explicit_targets_and_evidence() {
    let mut server = server();
    let ci = json!({
        "workflow": "receipt contracts",
        "triggers": ["push"],
        "rust_toolchain": "stable",
        "checks": [{"name": "tests", "run": "cargo test -p core", "required": true}]
    });
    let planned = call(
        &mut server,
        "developer_workbench",
        json!({
            "session": {"session_id": "receipt-ci", "owner": "agent-a", "goal": "receipt evidence", "artifacts": [], "cells": [], "changes": []},
            "ci": ci.clone()
        }),
    );
    let evidence = json!({
        "run_id": "receipt-run",
        "provider": "github_actions",
        "source": "provider_observed",
        "plan_digest": planned["ci"]["digest"].clone(),
        "conclusion": "success",
        "checks": [{"name": "tests", "status": "passed", "result_digest": "b".repeat(64)}]
    });
    let receipt = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-ci-1",
            "delivery": {
                "ci_evidence": {"ci": ci, "evidence": evidence},
                "release_request": {"id": "receipt-delivery-1", "targets": ["ci_execution_evidence"]}
            }
        }),
    );
    assert_eq!(receipt["workflow"], json!("developer_delivery_receipt"));
    assert_eq!(receipt["valid"], json!(true));
    assert_eq!(receipt["receipt_ready"], json!(true));
    assert_eq!(receipt["target_count"], json!(1));
    assert_eq!(receipt["ready_target_count"], json!(1));
    assert_eq!(receipt["evidence"][8]["name"], json!("ci_evidence"));
    assert_eq!(receipt["evidence"][8]["ready"], json!(true));
    assert_eq!(
        receipt["delivery"]["workflow"],
        json!("developer_delivery_audit")
    );
    assert_eq!(receipt["receipt_digest"].as_str().unwrap().len(), 64);

    let verified = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": receipt.clone(), "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(
        verified["workflow"],
        json!("developer_delivery_receipt_verify")
    );
    assert_eq!(verified["verified"], json!(true));
    assert_eq!(verified["receipt_digest_match"], json!(true));

    let mut tampered = receipt.clone();
    tampered["targets"][0]["ready"] = json!(false);
    let rejected = call(
        &mut server,
        "developer_delivery_receipt_verify",
        json!({"receipt": tampered, "delivery": receipt["delivery"].clone()}),
    );
    assert_eq!(rejected["verified"], json!(false));
    assert!(
        rejected["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "targets_mismatch")
    );

    let blocked = call(
        &mut server,
        "developer_delivery_receipt",
        json!({
            "receipt_id": "receipt-ci-2",
            "delivery": {"release_request": {"id": "receipt-delivery-2", "targets": ["ci_execution_evidence"]}}
        }),
    );
    assert_eq!(blocked["valid"], json!(true));
    assert_eq!(blocked["receipt_ready"], json!(false));
    assert_eq!(blocked["blocked_target_count"], json!(1));
}

#[test]
fn execution_provenance_reconciles_mission_trace_and_delegated_checks() {
    let mut server = server();
    let mission = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-provenance-1",
            "goal": "produce a bounded provenance trace",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
        }),
    );
    assert_eq!(mission["mission_status"], json!("succeeded"));
    let provenance = call(
        &mut server,
        "execution_provenance_audit",
        json!({
            "mission": mission.clone(),
            "delegated_checks": [{
                "name": "mission_trace_shape",
                "kind": "structural",
                "required": true,
                "status": "passed",
                "result_digest": "a".repeat(64),
                "source": "caller_attested"
            }]
        }),
    );
    assert_eq!(provenance["workflow"], json!("execution_provenance_audit"));
    assert_eq!(provenance["valid"], json!(true));
    assert_eq!(provenance["provenance_ready"], json!(true));
    assert_eq!(provenance["delegated_check_count"], json!(1));

    let delivery = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "execution_provenance": {
                "mission": mission.clone(),
                "delegated_checks": [{
                    "name": "mission_trace_shape",
                    "kind": "structural",
                    "required": true,
                    "status": "passed",
                    "result_digest": "a".repeat(64),
                    "source": "caller_attested"
                }]
            },
            "release_request": {"id": "delivery-provenance-1", "targets": ["execution_provenance"]}
        }),
    );
    assert_eq!(
        delivery["readiness"]["execution_provenance_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["execution_provenance"]["provenance_ready"],
        json!(true)
    );
    assert_eq!(
        delivery["release_request"]["available_target_count"],
        json!(13)
    );
    assert_eq!(delivery["release_request"]["ready"], json!(true));
    assert_eq!(
        delivery["release_request"]["targets"][0]["eligible"],
        json!(true)
    );

    let missing_provenance = call(
        &mut server,
        "developer_delivery_audit",
        json!({
            "release_request": {"id": "delivery-provenance-2", "targets": ["execution_provenance"]}
        }),
    );
    assert_eq!(
        missing_provenance["readiness"]["execution_provenance_ready"],
        json!(false)
    );
    assert_eq!(missing_provenance["release_request"]["ready"], json!(false));
    assert_eq!(
        missing_provenance["release_request"]["targets"][0]["blockers"],
        json!(["execution_provenance_arguments_missing"])
    );

    let mut tampered = mission;
    tampered["execution_trace"][1]["sequence"] = json!(99);
    let rejected = call(
        &mut server,
        "execution_provenance_audit",
        json!({"mission": tampered}),
    );
    assert_eq!(rejected["valid"], json!(false));
    assert!(
        rejected["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "trace_identity_error")
    );
}
