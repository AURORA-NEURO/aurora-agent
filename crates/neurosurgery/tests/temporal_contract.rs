use bioprism_neurosurgery::{
    CaseRequest, NeurosurgicalAgent, Observation, ObservationKind, ObservationStatus, RequestUse,
    Specialty, TemporalAlignmentStatus, TemporalCoverageState, NEUROSURGERY_SCHEMA_VERSION,
};

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "temporal-glioma-contract".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which explicit intake dates can a reviewer align?".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: vec![
            Observation {
                kind: ObservationKind::Imaging,
                label: "baseline MRI".to_string(),
                value: "caller-supplied imaging summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("mri-baseline".to_string()),
                observed_at: Some("2024-01-02T00:00:00Z".to_string()),
                timepoint: Some("baseline".to_string()),
            },
            Observation {
                kind: ObservationKind::Histology,
                label: "pathology".to_string(),
                value: "caller-supplied pathology summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("pathology".to_string()),
                observed_at: Some("2024-01-02T00:00:00Z".to_string()),
                timepoint: Some("baseline".to_string()),
            },
            Observation {
                kind: ObservationKind::Imaging,
                label: "interval MRI".to_string(),
                value: "caller-supplied imaging summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("mri-interval".to_string()),
                observed_at: Some("2024-03-02T00:00:00Z".to_string()),
                timepoint: Some("interval".to_string()),
            },
            Observation {
                kind: ObservationKind::Molecular,
                label: "molecular assay".to_string(),
                value: "caller-supplied assay summary".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("assay".to_string()),
                observed_at: None,
                timepoint: Some("baseline".to_string()),
            },
        ],
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

#[test]
fn temporal_audit_preserves_ordered_timepoints_and_partial_coverage() {
    let report = NeurosurgicalAgent::default()
        .temporal_audit(&request())
        .expect("temporal audit is deterministic");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-temporal-alignment/0.1"
    );
    assert_eq!(report.observation_count, 4);
    assert_eq!(report.timestamped_observation_count, 3);
    assert_eq!(report.untimestamped_observation_count, 1);
    assert_eq!(report.labelled_without_timestamp_count, 1);
    assert_eq!(report.distinct_timestamp_count, 2);
    assert_eq!(report.duplicate_timestamp_count, 1);
    assert_eq!(report.status, TemporalAlignmentStatus::Partial);
    assert!(!report.coverage_complete);
    assert!(report
        .missing_time_aligned_kinds
        .contains(&ObservationKind::Molecular));
    let imaging = report
        .kind_coverage
        .iter()
        .find(|row| row.observation_kind == ObservationKind::Imaging)
        .expect("imaging coverage exists");
    assert_eq!(imaging.state, TemporalCoverageState::Complete);
    assert_eq!(report.timepoints[0].observation_indices, vec![0, 1]);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "timepoint_label_without_timestamp"));
    assert_eq!(report.provider, "none");
    assert!(!report.network);
}

#[test]
fn temporal_audit_flags_input_order_inversions_without_reordering_input() {
    let mut changed = request();
    changed.observations.swap(0, 2);
    let report = NeurosurgicalAgent::default()
        .temporal_audit(&changed)
        .expect("timestamps remain valid");
    assert_eq!(report.input_order_inversion_count, 1);
    assert_eq!(report.status, TemporalAlignmentStatus::RequiresReview);
    assert_eq!(report.observations[0].label, "interval MRI");
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "input_order_not_chronological"));
}

#[test]
fn invalid_observation_timestamp_is_refused_before_temporal_audit() {
    let mut changed = request();
    changed.observations[0].observed_at = Some("2024-02-30T00:00:00Z".to_string());
    assert!(NeurosurgicalAgent::default()
        .temporal_audit(&changed)
        .is_err());
}
