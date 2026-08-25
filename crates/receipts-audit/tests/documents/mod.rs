//! Well-formed instances of every digest-sealed document type the battery audits.
//!
//! Each one is built by the producer that owns it — `ContextCertificate::to_json`,
//! `build_autopilot_report`, `run_research`, `build_delivery_receipt` — rather than hand-written
//! as JSON, so the battery starts from a document the shipping code actually emits. The mission
//! evidence bundle is the exception and is assembled here: `bioprism-devplat` exposes the
//! verifier for an exported bundle without exposing an in-process exporter, so this module builds
//! the export shape and seals it with the same `ContentHash::of_value` the exporter uses. That is
//! a stated difference in provenance, and it is why the bundle battery is a test of the verifier
//! only, with nothing to say about the exporter.
//!
//! The research dossier is memoised: `run_research` executes a full protocol, and running it once
//! per test would dominate the battery's runtime without covering anything a single dossier does
//! not already cover.

use std::sync::OnceLock;

use bioprism_autopilot::{
    build_autopilot_report, plan_next_action, AttemptKind, AttemptRecord, AutonomyGrant,
    DriveHistory, FinalDisposition, NextAction,
};
use bioprism_devplat::{
    build_delivery_receipt, plan_mission, DeliveryReceiptRequest, MissionReport, MissionRequest,
    MissionStepResult, MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION, MISSION_SCHEMA_VERSION,
    MISSION_TRACE_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use bioprism_research::{run_research, ResearchRequest};
use bioprism_section::{
    Backend, CertificateProfile, ContextCertificate, LeakageWitness, OmissionManifest,
    OracleVerdict, PlanDescriptor, ReferenceOmissions, SourceHashes,
};
use serde_json::{json, Value};

fn seal(document: &mut Value, field: &str) {
    let digest = ContentHash::of_value(document)
        .expect("the document canonicalises")
        .to_string();
    document[field] = Value::String(digest);
}

// -- context certificate ----------------------------------------------------------------------

fn certificate_body() -> ContextCertificate {
    ContextCertificate {
        world_id: "world.audit".into(),
        query_id: "query.audit".into(),
        selected_facts: vec!["fact.alpha".into(), "fact.beta".into()],
        selected_factors: vec!["factor.alpha".into()],
        protected_closure: vec!["fact.alpha".into()],
        omissions: ReferenceOmissions {
            total_facts: 5,
            exploratory_facts: 3,
            classification: "no_backward_dependency_path_or_temporally_inaccessible".into(),
            inaccessible_selected_before_cut: vec!["fact.future".into()],
        },
        plan: PlanDescriptor {
            backend: Backend::BackwardFactorSliceReference,
            compiled_factor_count: 1,
            compiled_fact_count: 2,
            total_factor_count: 6,
            total_fact_count: 5,
            max_selected_factor_arity: 2,
            fallback: None,
        },
        oracle: OracleVerdict::new(
            "deterministic_split_integrity_v1",
            vec![LeakageWitness::PreprocessingLeakage {
                detail: "preprocessing fit used all subjects before the split".into(),
            }],
        ),
        source_hashes: SourceHashes {
            world_sha256: "0a".repeat(32),
            query_sha256: "1b".repeat(32),
            decision_section_sha256: "2c".repeat(32),
        },
        limitations: vec!["reference slicer over a synthetic decision world".into()],
        manifest: OmissionManifest::default(),
    }
}

pub fn certificate() -> Value {
    certificate_body()
        .to_json(CertificateProfile::Reference)
        .expect("the reference certificate serialises")
}

pub fn extended_certificate() -> Value {
    certificate_body()
        .to_json(CertificateProfile::Extended)
        .expect("the extended certificate serialises")
}

// -- autopilot report -------------------------------------------------------------------------

fn step(id: &str, tool: &str, depends_on: &[&str]) -> Value {
    json!({
        "id": id,
        "domain": "metrics",
        "capability": "analytics",
        "objective": format!("run {id}"),
        "tool": tool,
        "arguments": {},
        "depends_on": depends_on,
        "bindings": [],
        "required": true,
    })
}

fn workflow_binding(step_ids: &[&str]) -> Value {
    let plan = json!({
        "steps": step_ids.iter().map(|id| json!({ "step_id": id })).collect::<Vec<_>>()
    });
    let digest = ContentHash::of_value(&plan)
        .expect("the evidence plan canonicalises")
        .to_string();
    let zeros = "0".repeat(64);
    json!({
        "workflow_id": "workflow.audit",
        "workflow_digest": zeros,
        "catalog_digest": zeros,
        "domain_contract_digest": zeros,
        "domain_contract": {},
        "evidence_plan": plan,
        "evidence_plan_digest": digest,
    })
}

fn mission() -> Value {
    json!({
        "mission_id": "mission.audit",
        "goal": "drive the audited workflow",
        "steps": [step("a", "tool_a", &[]), step("b", "tool_b", &["a"])],
        "workflow_binding": workflow_binding(&["a", "b"]),
    })
}

fn succeeded_step(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "succeeded".into(),
        required: true,
        arguments_digest: Some("3d".repeat(32)),
        bytes: 24,
        wire: Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "content": [{ "type": "text", "text": "{\"value\":7}" }] }
        })),
        error: None,
    }
}

fn mission_report(dispatched: &Value, results: Vec<MissionStepResult>) -> Value {
    let request: MissionRequest =
        serde_json::from_value(dispatched.clone()).expect("the dispatched mission parses");
    let plan = plan_mission(&request).expect("the dispatched mission plans");
    let succeeded = results.iter().filter(|row| row.status == "succeeded").count();
    let report = MissionReport {
        schema_version: MISSION_SCHEMA_VERSION.into(),
        plan,
        execution: "executed".into(),
        mission_status: "succeeded".into(),
        succeeded,
        refused: 0,
        blocked: 0,
        cancelled: 0,
        required_failures: 0,
        returned_bytes: 48,
        results,
        execution_trace_schema_version: MISSION_TRACE_SCHEMA_VERSION.into(),
        execution_trace: Vec::new(),
        claim_requests: Vec::new(),
        evaluator_review: None,
        claim_lineage: json!({}),
        trace_observer: None,
        guarantees: Vec::new(),
        limitations: Vec::new(),
    };
    serde_json::to_value(report).expect("the mission report serialises")
}

fn complete_reconciliation() -> Value {
    json!({
        "present": true,
        "reconciliation_digest": "4e".repeat(32),
        "completion": { "status": "complete" },
        "integrity": { "valid": true },
    })
}

pub fn autopilot_report() -> Value {
    let grant: AutonomyGrant =
        serde_json::from_value(json!({ "allowed_tools": ["tool_a", "tool_b"], "max_attempts": 3 }))
            .expect("the grant validates");
    let mut history = DriveHistory::new(mission()).expect("the mission is drivable");
    let dispatched = match plan_next_action(&grant, &history).expect("the planner runs") {
        NextAction::DispatchFull { mission, .. } => mission,
        other => panic!("expected a full dispatch, got {other:?}"),
    };
    let report = mission_report(
        &dispatched,
        vec![
            succeeded_step("a", "tool_a"),
            succeeded_step("b", "tool_b"),
        ],
    );
    history.push(
        AttemptRecord::delivered(
            AttemptKind::Full,
            dispatched,
            report,
            Some(complete_reconciliation()),
            None,
        )
        .expect("the attempt records"),
    );
    let NextAction::StopSuccess { evidence } =
        plan_next_action(&grant, &history).expect("the planner runs")
    else {
        panic!("a complete drive must stop on success");
    };
    build_autopilot_report(&grant, &history, &FinalDisposition::Succeeded { evidence })
        .expect("the report builds")
}

// -- research dossier -------------------------------------------------------------------------

pub fn research_request() -> ResearchRequest {
    serde_json::from_value(json!({
        "research_id": "receipts-audit",
        "question": "Does the equal-engineering panel separate under hub attachment?",
        "family": "reference_like",
        "distractor_points": [40],
        "seed": 11,
    }))
    .expect("the research request validates")
}

pub fn dossier() -> &'static Value {
    static DOSSIER: OnceLock<Value> = OnceLock::new();
    DOSSIER.get_or_init(|| run_research(&research_request()).expect("the research run completes"))
}

// -- mission evidence bundle ------------------------------------------------------------------

pub fn evidence_bundle() -> Value {
    let result = json!({
        "schema_version": MISSION_SCHEMA_VERSION,
        "mission_status": "succeeded",
        "succeeded": 2,
        "required_failures": 0,
    });
    let result_digest = ContentHash::of_value(&result)
        .expect("the retained result canonicalises")
        .to_string();
    let mut bundle = json!({
        "schema": MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION,
        "workflow": "mission_evidence_bundle_export",
        "mission_id": "mission.audit",
        "retention": {
            "mode": "full",
            "result_retained": true,
            "result_included": true,
            "summary_retained": true,
        },
        "result": result,
        "result_digest": result_digest,
        "evaluator_replay": { "workflow": "mission_evaluator_replay_summary" },
        "catalog_drift": { "status": "not_recorded" },
        "trace": [
            { "sequence": 1, "event": "mission_started" },
            { "sequence": 2, "event": "mission_succeeded" }
        ],
        "export": {
            "format": "json",
            "include_result": true,
            "include_trace": true,
            "trace_included": true,
            "digest_algorithm": "sha256",
            "execution": "not_started",
        }
    });
    seal(&mut bundle, "bundle_digest");
    bundle
}

// -- delivery receipt -------------------------------------------------------------------------

pub fn delivery_audit() -> Value {
    json!({
        "ok": true,
        "workflow": "developer_delivery_audit",
        "platform": { "ok": true },
        "repository": { "ok": true },
        "repository_impact": null,
        "sdk": null,
        "conformance": { "ok": true },
        "provider": null,
        "governance": null,
        "release": null,
        "ci_evidence": null,
        "execution_provenance": { "provenance_ready": true },
        "readiness": {
            "platform_checks_clean": true,
            "repository_scope_clean": true,
            "repository_impact_clean": false,
            "sdk_admission_clean": false,
            "conformance_release": false,
            "provider_capability_gate_cleared": false,
            "governance_document_clean": false,
            "release_audit_ready": false,
            "ci_execution_evidence_ready": false,
            "execution_provenance_ready": true,
        },
        "release_request": {
            "present": true,
            "id": "delivery.audit",
            "targets": [{
                "target": "execution_provenance",
                "available": true,
                "eligible": true,
                "blockers": [],
                "notes": [],
            }],
            "ready": true,
        }
    })
}

pub fn delivery_receipt() -> Value {
    let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
        receipt_id: "receipt.audit".into(),
        delivery: delivery_audit(),
    })
    .expect("the delivery receipt builds");
    serde_json::to_value(receipt).expect("the delivery receipt serialises")
}
