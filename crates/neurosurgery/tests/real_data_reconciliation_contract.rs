use bioprism_neurosurgery::{
    RealDataReconciliationQuery, RealDataReconciliationReport, RealGliomaBundle,
    MAX_REAL_DATA_RECONCILIATION_ISSUES,
};

fn bundle() -> RealGliomaBundle {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/neurosurgery/glioma_public_snapshot.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("snapshot should exist"))
        .expect("snapshot should decode")
}

#[test]
fn baseline_snapshot_reconciles_without_hidden_identifier_findings() {
    let report = bundle()
        .reconcile(&RealDataReconciliationQuery::default())
        .expect("baseline snapshot should reconcile");
    report.validate_integrity().expect("report should validate");
    assert_eq!(report.candidate_issue_count, 0);
    assert!(!report.requires_review);
    assert_eq!(report.counts.portal_study_count, 7);
    assert_eq!(report.counts.portal_pmid_missing_literature_count, 0);
}

#[test]
fn reconciliation_bound_is_fail_closed_and_report_tampering_is_rejected() {
    let data = bundle();
    assert!(data
        .reconcile(&RealDataReconciliationQuery { max_issues: 0 })
        .is_err());
    assert!(data
        .reconcile(&RealDataReconciliationQuery {
            max_issues: MAX_REAL_DATA_RECONCILIATION_ISSUES + 1,
        })
        .is_err());

    let mut report = data
        .reconcile(&RealDataReconciliationQuery::default())
        .expect("baseline snapshot should reconcile");
    report.reconciliation_digest = "0".repeat(64);
    assert!(report.validate_integrity().is_err());
}

#[test]
fn reconciliation_rejects_semantically_malformed_issue_rows() {
    let mut data = bundle();
    data.portal_studies[0].pmid = Some("999999999".to_string());
    let hashes = data
        .canonical_source_hashes()
        .expect("canonical source hashes should compute");
    for source in &mut data.sources {
        source.content_sha256 = hashes
            .get(&source.source_id)
            .expect("every source should have a hash")
            .clone();
    }
    let mut report = data
        .reconcile(&RealDataReconciliationQuery::default())
        .expect("drifted snapshot should reconcile");
    report.issues[0].record_kind = bioprism_neurosurgery::RealDataRecordKind::LiteratureArticle;
    assert!(report.validate_integrity().is_err());
}

#[test]
fn reconciliation_surfaces_real_snapshot_identifier_drift_as_review_work() {
    let mut data = bundle();
    data.portal_studies[0].pmid = Some("999999999".to_string());
    let hashes = data
        .canonical_source_hashes()
        .expect("canonical source hashes should compute");
    for source in &mut data.sources {
        source.content_sha256 = hashes
            .get(&source.source_id)
            .expect("every source should have a hash")
            .clone();
    }
    let report = data
        .reconcile(&RealDataReconciliationQuery { max_issues: 4 })
        .expect("validated metadata drift should reconcile");
    report
        .validate_for_inputs(&data)
        .expect("report should replay");
    assert_eq!(report.counts.portal_pmid_missing_literature_count, 1);
    assert_eq!(report.candidate_issue_count, 1);
    assert!(report.requires_review);
    assert_eq!(report.issues[0].identifier, "999999999");
    assert!(matches!(
        report.issues[0].kind,
        bioprism_neurosurgery::RealDataReconciliationIssueKind::PortalPmidMissingLiterature
    ));
}

#[test]
fn autonomous_workflow_turns_identifier_drift_into_a_provenance_action() {
    let mut data = bundle();
    data.portal_studies[0].pmid = Some("999999999".to_string());
    let hashes = data
        .canonical_source_hashes()
        .expect("canonical source hashes should compute");
    for source in &mut data.sources {
        source.content_sha256 = hashes
            .get(&source.source_id)
            .expect("every source should have a hash")
            .clone();
    }
    let report = data
        .autonomous_workflow(&Default::default())
        .expect("identifier drift should remain reviewable");
    assert!(report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::ReconcileIdentifiers
    }));
    assert_eq!(
        report.state,
        bioprism_neurosurgery::RealDataAutonomousWorkflowState::NeedsMetadataReview
    );
    assert!(!report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::HumanSynthesisGate
    }));
    report
        .validate_for_inputs(&data)
        .expect("workflow should replay against the drifted snapshot");
}

#[test]
fn evidence_packet_carries_identifier_drift_into_the_model_handoff() {
    let mut data = bundle();
    data.portal_studies[0].pmid = Some("999999999".to_string());
    let hashes = data
        .canonical_source_hashes()
        .expect("canonical source hashes should compute");
    for source in &mut data.sources {
        source.content_sha256 = hashes
            .get(&source.source_id)
            .expect("every source should have a hash")
            .clone();
    }
    let packet = data
        .evidence_packet(&Default::default())
        .expect("packet should retain reconciliation findings");
    assert_eq!(packet.reconciliation.candidate_issue_count, 1);
    assert!(packet.reconciliation.requires_review);
    assert!(packet
        .reconciliation
        .issues
        .iter()
        .any(|issue| issue.identifier == "999999999"));
    packet
        .validate_for_inputs(&data)
        .expect("packet with identifier drift should replay exactly");
}

#[test]
fn reconciliation_report_round_trips_as_a_metadata_only_envelope() {
    let report = bundle()
        .reconcile(&RealDataReconciliationQuery::default())
        .expect("baseline snapshot should reconcile");
    let encoded = serde_json::to_string(&report).expect("report should encode");
    let decoded: RealDataReconciliationReport =
        serde_json::from_str(&encoded).expect("report should decode");
    assert_eq!(decoded, report);
    decoded
        .validate_for_inputs(&bundle())
        .expect("report should replay");
    assert!(decoded.human_review_required);
    assert_eq!(decoded.provider, "none");
    assert!(!decoded.synthetic_data);
}
