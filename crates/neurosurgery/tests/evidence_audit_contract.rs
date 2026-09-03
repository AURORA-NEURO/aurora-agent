use bioprism_neurosurgery::{
    CaseRequest, EvidenceAuditReport, EvidenceRecord, EvidenceTier, GliomaEvidenceState,
    GliomaMarker, GliomaMarkerObservation, GliomaMolecularPanel, NeurosurgeryError,
    NeurosurgicalAgent, Observation, ObservationKind, ObservationStatus, RequestUse, Specialty,
    ToolCapability, NEUROSURGERY_SCHEMA_VERSION,
};

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "audit-glioma-001".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which intake gaps should a reviewer resolve?".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: vec![
            Observation {
                kind: ObservationKind::Histology,
                label: "histology".to_string(),
                value: "caller summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("pathology-1".to_string()),
                observed_at: None,
                timepoint: None,
            },
            Observation {
                kind: ObservationKind::Imaging,
                label: "imaging".to_string(),
                value: "caller summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: None,
                observed_at: None,
                timepoint: None,
            },
            Observation {
                kind: ObservationKind::Molecular,
                label: "molecular".to_string(),
                value: "not collected".to_string(),
                status: ObservationStatus::NotCollected,
                source_id: None,
                observed_at: None,
                timepoint: None,
            },
            Observation {
                kind: ObservationKind::Neuroanatomy,
                label: "anatomy".to_string(),
                value: "conflicting summaries".to_string(),
                status: ObservationStatus::Conflicting,
                source_id: Some("anatomy-1".to_string()),
                observed_at: None,
                timepoint: None,
            },
        ],
        evidence: vec![
            EvidenceRecord {
                id: "guideline-1".to_string(),
                title: "Declared guideline".to_string(),
                citation: "https://example.invalid/guideline".to_string(),
                tier: EvidenceTier::Guideline,
                population: None,
                year: Some(2025),
                supports: vec![ToolCapability::EvidenceSynthesis],
            },
            EvidenceRecord {
                id: "unverified-1".to_string(),
                title: "Declared citation".to_string(),
                citation: "https://example.invalid/citation".to_string(),
                tier: EvidenceTier::Unverified,
                population: None,
                year: None,
                supports: Vec::new(),
            },
        ],
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

#[test]
fn audit_preserves_granular_states_and_provenance_gaps() {
    let report: EvidenceAuditReport = NeurosurgicalAgent::default()
        .audit_evidence(&request())
        .expect("research request audits");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-evidence-audit/0.1"
    );
    assert_eq!(report.required_observation_kinds.len(), 6);
    assert!(!report.coverage_complete);
    assert_eq!(report.provenance_gap_count, 1);
    assert_eq!(report.evidence_record_count, 2);
    assert_eq!(report.verified_evidence_count, 1);
    assert_eq!(report.unverified_evidence_count, 1);
    assert_eq!(report.evidence_supporting_synthesis_count, 1);
    let imaging = report
        .items
        .iter()
        .find(|item| item.observation_kind == ObservationKind::Imaging)
        .expect("imaging item exists");
    assert_eq!(
        imaging.state,
        bioprism_neurosurgery::EvidenceState::Measured
    );
    assert_eq!(imaging.provenance_complete_count, 0);
    let molecular = report
        .items
        .iter()
        .find(|item| item.observation_kind == ObservationKind::Molecular)
        .expect("molecular item exists");
    assert_eq!(
        molecular.state,
        bioprism_neurosurgery::EvidenceState::Unmeasured
    );
    let anatomy = report
        .items
        .iter()
        .find(|item| item.observation_kind == ObservationKind::Neuroanatomy)
        .expect("anatomy item exists");
    assert_eq!(
        anatomy.state,
        bioprism_neurosurgery::EvidenceState::Conflicting
    );
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.audit_digest.len(), 64);
    report
        .validate_integrity()
        .expect("intake audit is self-consistent");
    report
        .validate_for_request(&request())
        .expect("intake audit replays against the exact request");
    let mut tampered = report.clone();
    tampered.provenance_gap_count = 0;
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn audit_refuses_clinical_requests_before_projecting_coverage() {
    let mut clinical = request();
    clinical.request_use = RequestUse::IndividualDiagnosis;
    let error = NeurosurgicalAgent::default()
        .audit_evidence(&clinical)
        .expect_err("clinical audit is refused");
    assert!(matches!(
        error,
        NeurosurgeryError::ClinicalUseRefused { .. }
    ));
}

#[test]
fn audit_counts_a_complete_typed_molecular_panel_as_molecular_coverage() {
    let mut request = request();
    request
        .observations
        .retain(|observation| observation.kind != ObservationKind::Molecular);
    request.glioma_molecular = Some(GliomaMolecularPanel {
        observations: GliomaMarker::ALL
            .into_iter()
            .map(|marker| GliomaMarkerObservation {
                marker,
                state: GliomaEvidenceState::Present,
                assay: Some("research-panel-v1".to_string()),
                specimen: Some("tumour-baseline".to_string()),
                source_id: Some("molecular-source-1".to_string()),
                observed_at: None,
            })
            .collect(),
        ..GliomaMolecularPanel::default()
    });
    let report = NeurosurgicalAgent::default()
        .audit_evidence(&request)
        .expect("typed molecular panel audits");
    let molecular = report
        .items
        .iter()
        .find(|item| item.observation_kind == ObservationKind::Molecular)
        .expect("molecular item exists");
    assert_eq!(
        molecular.state,
        bioprism_neurosurgery::EvidenceState::Measured
    );
    assert_eq!(molecular.observed_count, GliomaMarker::ALL.len());
    assert_eq!(molecular.provenance_complete_count, GliomaMarker::ALL.len());
}
