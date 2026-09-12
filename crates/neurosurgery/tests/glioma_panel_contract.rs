use bioprism_neurosurgery::{
    CaseRequest, EvidenceRecord, EvidenceTier, GliomaEvidenceState, GliomaMarker,
    GliomaMarkerObservation, GliomaMolecularPanel, NeurosurgeryError, NeurosurgicalAgent,
    Observation, ObservationKind, ObservationStatus, RequestUse, Specialty, ToolCapability,
    ToolRunStatus, NEUROSURGERY_SCHEMA_VERSION,
};

fn observation(kind: ObservationKind, label: &str) -> Observation {
    Observation {
        kind,
        label: label.to_string(),
        value: "caller-supplied de-identified research summary".to_string(),
        status: ObservationStatus::Observed,
        source_id: Some(format!("source-{label}")),
        observed_at: Some("2024-01-01T00:00:00Z".to_string()),
        timepoint: Some("caller-timepoint".to_string()),
    }
}

fn complete_request(panel: GliomaMolecularPanel) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "synthetic-panel-contract-001".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::SyntheticCaseSimulation,
        question: "Which research evidence dimensions remain to be reviewed?".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: vec![
            observation(ObservationKind::Imaging, "imaging"),
            observation(ObservationKind::Histology, "histology"),
            observation(ObservationKind::Neuroanatomy, "neuroanatomy"),
            observation(ObservationKind::LongitudinalOutcome, "outcome"),
        ],
        evidence: vec![EvidenceRecord {
            id: "guideline-1".to_string(),
            title: "Caller-selected guideline".to_string(),
            citation: "Caller citation".to_string(),
            tier: EvidenceTier::Guideline,
            population: None,
            year: Some(2024),
            supports: vec![ToolCapability::EvidenceSynthesis],
        }],
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: Some(panel),
    }
}

fn complete_panel() -> GliomaMolecularPanel {
    GliomaMolecularPanel {
        observations: GliomaMarker::ALL
            .into_iter()
            .enumerate()
            .map(|(index, marker)| GliomaMarkerObservation {
                marker,
                state: if index % 2 == 0 {
                    GliomaEvidenceState::Present
                } else {
                    GliomaEvidenceState::Absent
                },
                assay: Some("research-panel-v1".to_string()),
                specimen: Some("tumour-baseline".to_string()),
                source_id: Some("molecular-source-1".to_string()),
                observed_at: Some("2026-08-29T00:00:00Z".to_string()),
            })
            .collect(),
        ..GliomaMolecularPanel::default()
    }
}

#[test]
fn complete_typed_panel_satisfies_molecular_context_without_classifying() {
    let panel = complete_panel();
    let summary = panel.summary().expect("complete panel validates");
    assert_eq!(summary.marker_count, GliomaMarker::ALL.len());
    assert_eq!(summary.measured_count, GliomaMarker::ALL.len());
    assert_eq!(summary.provenance_complete_count, GliomaMarker::ALL.len());
    assert!(summary
        .markers
        .iter()
        .all(|marker| marker.provenance_complete));
    assert_eq!(summary.not_collected_count, 0);
    assert!(summary.research_gaps.is_empty());
    assert_eq!(summary.source_ids, vec!["molecular-source-1"]);

    let response = NeurosurgicalAgent::default()
        .run(&complete_request(panel))
        .expect("panel-backed research request is valid");
    assert!(response
        .evidence_gaps
        .iter()
        .all(|gap| { gap.capability != ToolCapability::MolecularContext }));
    let molecular_run = response
        .tool_runs
        .iter()
        .find(|run| run.capability == ToolCapability::MolecularContext)
        .expect("molecular tool is on the glioma route");
    assert_eq!(molecular_run.status, ToolRunStatus::Completed);
    assert!(molecular_run
        .findings
        .iter()
        .any(|finding| finding.code == "molecular_panel_inventory"));
    assert!(response.glioma_molecular.is_some());
    assert!(response
        .report
        .known_inputs
        .iter()
        .any(|input| input.contains("typed glioma molecular panel")));
    let serialized = serde_json::to_value(response).expect("response serializes");
    assert!(serialized.get("diagnosis").is_none());
    assert!(serialized.get("treatment_recommendation").is_none());
}

#[test]
fn panel_missingness_and_provenance_are_explicit_and_distinct() {
    let panel = GliomaMolecularPanel {
        observations: vec![
            GliomaMarkerObservation {
                marker: GliomaMarker::Idh1Mutation,
                state: GliomaEvidenceState::Present,
                assay: None,
                specimen: None,
                source_id: None,
                observed_at: None,
            },
            GliomaMarkerObservation {
                marker: GliomaMarker::H3K27Alteration,
                state: GliomaEvidenceState::Uninterpretable,
                assay: Some("research-panel-v1".to_string()),
                specimen: Some("tumour-baseline".to_string()),
                source_id: Some("source-2".to_string()),
                observed_at: None,
            },
            GliomaMarkerObservation {
                marker: GliomaMarker::H3G34Mutation,
                state: GliomaEvidenceState::Conflicting,
                assay: Some("research-panel-v1".to_string()),
                specimen: Some("tumour-baseline".to_string()),
                source_id: Some("source-2".to_string()),
                observed_at: None,
            },
            GliomaMarkerObservation {
                marker: GliomaMarker::Codeletion1p19q,
                state: GliomaEvidenceState::NotCollected,
                assay: None,
                specimen: None,
                source_id: None,
                observed_at: None,
            },
        ],
        ..GliomaMolecularPanel::default()
    };
    let summary = panel.summary().expect("explicit gaps validate");
    assert_eq!(summary.measured_count, 1);
    assert_eq!(summary.not_collected_count, GliomaMarker::ALL.len() - 3);
    assert_eq!(summary.uninterpretable_count, 1);
    assert_eq!(summary.conflicting_count, 1);
    assert_eq!(summary.provenance_complete_count, 0);
    assert_eq!(summary.missing_provenance_count, 1);
    assert_eq!(summary.missing_assay_count, 1);
    assert_eq!(summary.missing_specimen_count, 1);
    assert!(summary
        .research_gaps
        .iter()
        .any(|gap| gap.contains("missing source_id, assay, specimen")));
    assert!(summary
        .research_gaps
        .iter()
        .any(|gap| gap.contains("uninterpretable")));
    assert!(summary
        .research_gaps
        .iter()
        .any(|gap| gap.contains("conflicting")));
}

#[test]
fn typed_panel_rejects_duplicates_wrong_specialty_and_bad_timestamps() {
    let mut duplicate = complete_panel();
    duplicate
        .observations
        .push(duplicate.observations[0].clone());
    assert!(matches!(
        duplicate.validate(),
        Err(NeurosurgeryError::GliomaPanelRejected { .. })
    ));

    let mut wrong_specialty = complete_request(complete_panel());
    wrong_specialty.specialty = Specialty::ChiariMalformation;
    assert!(matches!(
        NeurosurgicalAgent::default().run(&wrong_specialty),
        Err(NeurosurgeryError::GliomaPanelRejected { .. })
    ));

    let mut bad_timestamp = complete_panel();
    bad_timestamp.observations[0].observed_at = Some("2026-13-29T00:00:00Z".to_string());
    assert!(matches!(
        bad_timestamp.validate(),
        Err(NeurosurgeryError::GliomaPanelRejected { .. })
    ));
}
