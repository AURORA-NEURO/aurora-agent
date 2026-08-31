//! What each verifier in this crate actually covers, stated as a test rather than as a comment.
//!
//! Every case here was found by pointing `bioprism-receipts-audit` at these verifiers and reading
//! what it accepted. Three of them were holes and are now closed; the rest record a boundary that
//! is deliberate, so a later reader can tell "this is checked" from "this is out of scope" without
//! rediscovering the difference by experiment.

use bioprism_devplat::{
    instantiate_domain_workflow, record_domain_evidence_provider_external_payload, run_workbench,
    verify_domain_evidence_provider_replay, verify_domain_workflow,
    verify_domain_workflow_portfolio, verify_workbench, ArtifactCard, ArtifactState, CellInput,
    CellKind, ChangeKind, CiCheck, CiRequest, DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderNormalizationRequest, DomainEvidenceProviderReplayRequest,
    EvidencePosture, NotebookPolicy, StudioCell, StudioChange, StudioSession, WorkbenchRequest,
    WorkbenchVerificationPolicy, WorkbenchVerificationRequest,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};

fn catalogue() -> Value {
    json!([{
        "id": "oncology_workflows",
        "domains": ["oncology"],
        "crates": ["bioprism-onco"],
        "mcp_tools": ["onco_boundary_check"],
        "cli_entrypoints": [],
        "status": "available"
    }])
}

fn tools() -> Value {
    json!([{ "name": "onco_boundary_check", "inputSchema": { "type": "object" } }])
}

fn instantiate_request() -> Value {
    json!({
        "workflow_id": "oncology_workflows",
        "mission_id": "coverage-1",
        "goal": "review the oncology boundary",
        "steps": [{ "id": "boundary", "tool": "onco_boundary_check", "arguments": {} }],
        "policy": { "execute": true }
    })
}

fn digest_of(value: &Value) -> String {
    ContentHash::of_value(value)
        .expect("the value canonicalises")
        .to_string()
}

// -- domain workflow replay ---------------------------------------------------------------------

/// The replay compares every field it produced, not a list of ten somebody maintained by hand.
///
/// Under the old list, `ok`, `schema`, `preflight`, `guarantees`, `limitations` and `links` were
/// each outside the comparison: a retained instantiation could have any of them replaced outright
/// and the replay still reported `matched`. `limitations` is the field that records what the plan
/// does *not* establish, so the one edit the comparison most needed to see was among the six it
/// could not.
#[test]
fn a_workflow_replay_catches_an_edit_to_every_field_the_instantiation_carries() {
    let instantiation = instantiate_domain_workflow(&catalogue(), &tools(), &instantiate_request())
        .expect("the oncology workflow instantiates");
    let fields: Vec<String> = instantiation
        .as_object()
        .expect("an instantiation is an object")
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        fields.len(),
        17,
        "the shape this test claims to cover: {fields:?}"
    );

    let pristine = json!({
        "instantiation": instantiation.clone(),
        "replay_request": instantiate_request(),
    });
    let verified = verify_domain_workflow(&catalogue(), &tools(), &pristine)
        .expect("an untouched retained workflow verifies");
    assert_eq!(verified["structural_valid"], json!(true));
    assert_eq!(verified["replay"]["matched"], json!(true));

    for field in &fields {
        let mut tampered = instantiation.clone();
        tampered[field] = json!("rewritten after retention");
        let request = json!({
            "instantiation": tampered,
            "replay_request": instantiate_request(),
        });
        match verify_domain_workflow(&catalogue(), &tools(), &request) {
            Ok(projection) => assert!(
                projection["structural_valid"] != json!(true)
                    || projection["replay"]["matched"] != json!(true),
                "{field} was rewritten and the replay still reported a match"
            ),
            Err(_) => {
                // Refused before the comparison, which is a stronger answer than a mismatch.
            }
        }
    }
}

/// A key the replay never produced stays uncompared, and that is deliberate.
///
/// The MCP surface writes `preflight_report` onto an instantiation after this kernel returns it,
/// so a field present in the retained document and absent from the replay cannot be read as
/// tampering without refusing every workflow the server hands back.
#[test]
fn a_key_the_replay_never_produced_is_left_uncompared_so_the_transports_own_field_survives() {
    let mut instantiation =
        instantiate_domain_workflow(&catalogue(), &tools(), &instantiate_request())
            .expect("the workflow instantiates");
    instantiation
        .as_object_mut()
        .expect("object")
        .insert("preflight_report".into(), json!({ "ok": true }));

    let request = json!({
        "instantiation": instantiation,
        "replay_request": instantiate_request(),
    });
    let projection = verify_domain_workflow(&catalogue(), &tools(), &request)
        .expect("a transport-annotated instantiation still verifies");
    assert_eq!(projection["structural_valid"], json!(true));
    assert_eq!(projection["replay"]["matched"], json!(true));
}

// -- portfolio digest ---------------------------------------------------------------------------

/// A portfolio that carries no digest and one whose digest is the wrong shape are two findings.
///
/// They shared a message until this test was written, so a caller who forgot the field was told
/// their digest was malformed and sent to inspect a value that was not there.
#[test]
fn an_absent_portfolio_digest_is_reported_differently_from_a_malformed_one() {
    let portfolio = bioprism_devplat::build_domain_workflow_portfolio(
        &catalogue(),
        &tools(),
        &json!({
            "portfolio_id": "coverage-portfolio",
            "requests": [instantiate_request()],
            "policy": { "allow_partial": false },
        }),
    )
    .expect("the portfolio plans");

    let request = |portfolio: Value| json!({ "portfolio": portfolio, "replay_requests": [instantiate_request()] });
    let verified =
        verify_domain_workflow_portfolio(&catalogue(), &tools(), &request(portfolio.clone()))
            .expect("an untouched portfolio verifies");
    assert_eq!(verified["portfolio_digest_matched"], json!(true));
    assert_eq!(verified["verification_status"], json!("verified"));

    let mut absent = portfolio.clone();
    absent
        .as_object_mut()
        .expect("object")
        .remove("portfolio_digest");
    let absent = verify_domain_workflow_portfolio(&catalogue(), &tools(), &request(absent))
        .expect_err("a portfolio without its digest is refused");

    let mut malformed = portfolio;
    malformed["portfolio_digest"] = json!("not-a-digest");
    let malformed = verify_domain_workflow_portfolio(&catalogue(), &tools(), &request(malformed))
        .expect_err("a portfolio whose digest is the wrong shape is refused");

    assert_ne!(
        absent.to_string(),
        malformed.to_string(),
        "one message for both defects sends a caller who omitted the field to inspect its value"
    );
    assert!(
        absent.to_string().contains("must be a non-empty string"),
        "{absent}"
    );
    assert!(
        malformed
            .to_string()
            .contains("must be a 64-character hexadecimal digest"),
        "{malformed}"
    );
}

// -- workbench ----------------------------------------------------------------------------------

fn hashed(label: &str) -> String {
    ContentHash::of_bytes(label.as_bytes()).to_string()
}

fn session() -> StudioSession {
    StudioSession {
        session_id: "session-1".into(),
        owner: "agent-a".into(),
        goal: "author a verified oncology capability card".into(),
        environment_digest: Some(hashed("env")),
        artifacts: vec![ArtifactCard {
            id: "artifact-1".into(),
            title: "verification card".into(),
            path: "artifacts/verification.json".into(),
            domain: "oncology".into(),
            capability: "verification".into(),
            state: ArtifactState::Validated,
            evidence: EvidencePosture::Reproduced,
            digest: Some(hashed("artifact")),
            score: Some(0.8),
            tags: vec!["public-card".into()],
        }],
        cells: vec![StudioCell {
            id: "cell-1".into(),
            kind: CellKind::Query,
            source: "workspace.metrics_analytics_audit(...)".into(),
            inputs: vec![CellInput {
                artifact_id: "artifact-1".into(),
                digest: hashed("artifact"),
            }],
            depends_on: Vec::new(),
            executed: true,
            output_digest: Some(hashed("output")),
        }],
        changes: vec![StudioChange {
            id: "change-1".into(),
            artifact_id: "artifact-1".into(),
            kind: ChangeKind::Create,
            actor: "agent-a".into(),
            logical_time: 1,
            input_digest: None,
            output_digest: Some(hashed("artifact")),
            reason: "initial authored artifact".into(),
        }],
        policy: NotebookPolicy::default(),
    }
}

fn ci() -> CiRequest {
    CiRequest {
        workflow: "consumer contracts".into(),
        triggers: vec!["push".into(), "pull_request".into()],
        rust_toolchain: "1.85.0".into(),
        checks: vec![CiCheck {
            name: "workspace tests".into(),
            run: "cargo test --workspace --offline".into(),
            working_directory: Some("crates/devplat".into()),
            required: true,
        }],
        offline: true,
    }
}

fn workbench_request() -> Value {
    let report = run_workbench(&WorkbenchRequest {
        session: session(),
        dashboard: None,
        ci: Some(ci()),
    })
    .expect("the workbench run completes");
    let retained = serde_json::to_value(&report).expect("the report serialises");
    serde_json::to_value(WorkbenchVerificationRequest {
        session: session(),
        expected_report_digest: Some(digest_of(&retained)),
        report,
        ci_replay: Some(ci()),
        policy: WorkbenchVerificationPolicy {
            require_dashboard: false,
            require_ci: true,
            require_ci_replay: true,
        },
    })
    .expect("the verification request serialises")
}

fn workbench_verifies(document: &Value) -> bool {
    let Ok(request) = serde_json::from_value::<WorkbenchVerificationRequest>(document.clone())
    else {
        return false;
    };
    verify_workbench(&request)
        .map(|report| report.valid && report.report_digest_matched == Some(true))
        .unwrap_or(false)
}

/// A field the reader discards is a field `expected_report_digest` does not cover.
///
/// The digest is recomputed by re-serialising the *parsed* report, so any key the reader dropped
/// is outside the seal by construction: the recomputation never sees it, the claimed digest still
/// agrees, and a report with content nobody hashed reads as verified. Refusing the unknown field
/// is the only place that difference can be caught. Both positions below sit under `/report`,
/// which is exactly what `expected_report_digest` covers, and both accepted an injected key
/// before the readers under the seal were tightened.
#[test]
fn an_injected_key_below_the_retained_report_is_refused_rather_than_dropped_by_the_reader() {
    let pristine = workbench_request();
    assert!(workbench_verifies(&pristine));

    for pointer in ["/report/audit", "/report/ci"] {
        let mut tampered = pristine.clone();
        tampered
            .pointer_mut(pointer)
            .expect("the position exists")
            .as_object_mut()
            .expect("the position is an object")
            .insert("injected".into(), json!("added after the digest was taken"));
        assert!(
            !workbench_verifies(&tampered),
            "a key injected at {pointer} was dropped by the reader and the request still verified"
        );
    }
}

/// The two positions where an injected key still survives, and why they are left open.
///
/// `developer_workbench` writes `ok`, `workflow` and `workbench_schema_version` onto the report
/// root before returning it, and this crate's own workbench registry strips exactly those three
/// before hashing. Refusing an unrecognised key at that root would refuse the document the shipped
/// tool produces.
#[test]
fn an_injected_key_on_the_transport_boundary_is_tolerated_and_recorded_here() {
    let pristine = workbench_request();
    for pointer in ["", "/report"] {
        let mut tampered = pristine.clone();
        let slot = if pointer.is_empty() {
            &mut tampered
        } else {
            tampered.pointer_mut(pointer).expect("the position exists")
        };
        slot.as_object_mut()
            .expect("the position is an object")
            .insert("workbench_schema_version".into(), json!("transport"));
        assert!(
            workbench_verifies(&tampered),
            "the recorded boundary at {pointer:?} no longer tolerates a transport field — if the \
             transport stopped writing one, close the boundary and delete this test"
        );
    }
}

/// The caller-supplied halves of a verification request, which no digest covers.
///
/// `expected_report_digest` seals `/report` and nothing else. `session`, `ci_replay` and `policy`
/// are inputs the caller hands in beside it so the verifier can replay the run; they are compared
/// against the report rather than hashed. Refusing an unrecognised key on them would reject a
/// forward-compatible request without protecting any digest, so the reader keeps dropping it.
/// That is a decision rather than an oversight, and this test is where the decision is written
/// down.
#[test]
fn an_injected_key_on_a_caller_supplied_request_field_is_tolerated_and_recorded_here() {
    let pristine = workbench_request();
    assert!(workbench_verifies(&pristine));

    for pointer in [
        "/session",
        "/session/artifacts/0",
        "/session/cells/0",
        "/session/cells/0/inputs/0",
        "/session/changes/0",
        "/session/policy",
        "/ci_replay",
        "/ci_replay/checks/0",
        "/policy",
    ] {
        let mut tampered = pristine.clone();
        tampered
            .pointer_mut(pointer)
            .expect("the position exists")
            .as_object_mut()
            .expect("the position is an object")
            .insert(
                "injected".into(),
                json!("a field a later caller might send"),
            );
        assert!(
            workbench_verifies(&tampered),
            "the recorded boundary at {pointer} no longer tolerates an unrecognised key. If this \
             input was brought under a digest, close the boundary and delete this entry"
        );
    }
}

// -- provider replay: the fields no expected digest covers ---------------------------------------

fn provider_observation() -> DomainEvidenceProviderNormalizationRequest {
    DomainEvidenceProviderNormalizationRequest {
        group_id: "biological_domains".into(),
        domains: vec!["oncology".into()],
        subject_id: "subject-1".into(),
        source_tool: "literature_bind_check".into(),
        connector_kind: "literature".into(),
        provider: "pubmed".into(),
        payload: json!({ "records": [{ "id": "pmid:1", "title": "opaque" }] }),
        request: Some(json!({ "query": "oncology" })),
        outcome: "observed".into(),
        claim_posture: json!({
            "status": "review_required",
            "does_not_claim": [
                "provider authenticity",
                "scientific or clinical validity",
                "provenance completeness",
                "execution or external effect"
            ]
        }),
        parent_digests: vec!["a".repeat(64)],
        source_plan_digest: Some("b".repeat(64)),
    }
}

fn provider_replay_request() -> Value {
    let observation = provider_observation();
    let normalized = bioprism_devplat::normalize_domain_evidence_provider(&observation)
        .expect("the observation normalizes");
    let normalization = serde_json::to_value(&normalized).expect("the normalization serialises");
    let intake = bioprism_devplat::intake_domain_evidence(&normalized.intake_arguments)
        .expect("the normalized arguments intake");
    serde_json::to_value(DomainEvidenceProviderReplayRequest {
        expected_payload_digest: normalized.payload_digest.clone(),
        expected_request_digest: normalized.request_digest.clone(),
        expected_shape_digest: normalized.shape_audit.shape_digest.clone(),
        expected_normalization_digest: digest_of(&normalization),
        expected_intake_digest: intake["intake_digest"]
            .as_str()
            .expect("the intake carries its digest")
            .into(),
        observation,
    })
    .expect("the replay request serialises")
}

fn replay_matches(document: &Value) -> bool {
    let Ok(request) =
        serde_json::from_value::<DomainEvidenceProviderReplayRequest>(document.clone())
    else {
        return false;
    };
    verify_domain_evidence_provider_replay(&request)
        .map(|verification| verification.matched)
        .unwrap_or(false)
}

/// An open hole, pinned so it is visible rather than rediscovered.
///
/// `claim_posture` and `parent_digests` are covered by none of the five expected digests this
/// replay compares. `payload_digest` names the payload only; `intake_digest` is taken over an
/// observation object that omits both fields; and `normalization_digest` cannot see them because
/// `DomainEvidenceProviderNormalization::intake_arguments`, which does carry `claim_posture`, is
/// `#[serde(skip)]`. A rewritten claim posture — the field that records what the evidence does not
/// claim — therefore replays as matched.
///
/// Closing it means changing what `intake_digest` hashes, and that digest is a published wire
/// value recorded in intake artifacts and reconciliation records across the workspace, so the hole
/// is recorded here rather than closed in passing.
#[test]
fn a_rewritten_claim_posture_or_lineage_still_replays_as_matched_and_is_recorded_as_a_hole() {
    let pristine = provider_replay_request();
    assert!(replay_matches(&pristine));

    let mut rewritten = pristine.clone();
    rewritten["claim_posture"] = json!({
        "status": "observed",
        "does_not_claim": ["nothing at all"]
    });
    assert!(
        replay_matches(&rewritten),
        "the claim posture is now covered by one of the expected digests — delete this test and \
         the gap it records"
    );

    let mut relineaged = pristine;
    relineaged["parent_digests"] = json!(["f".repeat(64)]);
    assert!(
        replay_matches(&relineaged),
        "the lineage digests are now covered — delete this test and the gap it records"
    );
}

// -- external payload replay ---------------------------------------------------------------------

/// `domains` is a set on the wire and an array only in JSON, so its order carries no identity.
///
/// Recorded because the mutation battery generates an array reordering at this position and would
/// otherwise report it as a hole every time somebody reads the output.
#[test]
fn the_external_receipts_domain_list_is_a_set_and_its_order_is_not_part_of_its_identity() {
    let receipt = |domains: Vec<String>| DomainEvidenceProviderExternalPayloadReceiptRequest {
        group_id: "biological_domains".into(),
        domains,
        subject_id: "subject-1".into(),
        source_tool: "literature_bind_check".into(),
        provider: "pubmed".into(),
        connector_kind: "literature".into(),
        handoff_digest: "a".repeat(64),
        transfer_id: "transfer-1".into(),
        payload_digest: "b".repeat(64),
        byte_length: 4096,
        storage_backend: "object_store".into(),
        locator_kind: "opaque".into(),
        locator: "store://caller/pubmed/objects/1".into(),
        content_type: Some("application/json".into()),
        content_encoding: Some("gzip".into()),
        request_digest: Some("c".repeat(64)),
        parent_digests: vec!["d".repeat(64)],
        availability: "available".into(),
        retention: "durable".into(),
        attempt_id: Some("attempt-1".into()),
    };
    let forward = record_domain_evidence_provider_external_payload(&receipt(vec![
        "genomics".into(),
        "oncology".into(),
    ]))
    .expect("the receipt records");
    let reversed = record_domain_evidence_provider_external_payload(&receipt(vec![
        "oncology".into(),
        "genomics".into(),
    ]))
    .expect("the receipt records");
    assert_eq!(
        forward.receipt_digest, reversed.receipt_digest,
        "the receipt sorts its domains, so two orderings name one receipt"
    );
}
