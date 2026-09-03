use bioprism_neurosurgery::{
    build_evidence_program, CaseAsset, CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery,
    CaseAssetSourceKind, CaseRequest, EvidenceProgramQuery, NeurosurgicalAgent, Observation,
    ObservationKind, ObservationStatus, RequestUse, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};

fn request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: format!("{}-program", specialty.slug()),
        specialty,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which source-grounded research tracks should a reviewer inspect?".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

fn literature() -> bioprism_neurosurgery::PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public snapshot parses")
}

#[test]
fn program_projects_real_pmids_into_specialty_tracks() {
    let report = build_evidence_program(
        &request(Specialty::ChiariMalformation),
        None,
        Some(&literature()),
        &EvidenceProgramQuery::default(),
    )
    .expect("program validates");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-evidence-program/0.1"
    );
    assert_eq!(report.specialty_count, 1);
    assert_eq!(report.lanes[0].specialty, Specialty::ChiariMalformation);
    assert_eq!(report.lanes[0].track_count, 6);
    assert!(report.non_empty_track_count > 0);
    assert!(report.lanes[0].tracks.iter().all(|track| {
        track.observation_coverage.len() == track.required_observation_kinds.len()
            && track.missing_observation_kinds.len() <= track.required_observation_kinds.len()
            && (!track.observation_coverage_complete || track.missing_observation_kinds.is_empty())
    }));
    assert!(report.lanes[0]
        .tracks
        .iter()
        .any(|track| !track.observation_coverage_complete));
    let references = report
        .lanes
        .iter()
        .flat_map(|lane| lane.tracks.iter())
        .flat_map(|track| track.references.iter());
    assert!(references.clone().all(|reference| {
        reference.record_id.starts_with("PMID-")
            && reference.uri.contains("pubmed.ncbi.nlm.nih.gov")
    }));
    assert!(references.count() > 0);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
}

#[test]
fn program_integrity_rejects_track_tampering_and_input_rebinding() {
    let request = request(Specialty::ChiariMalformation);
    let literature = literature();
    let report = build_evidence_program(
        &request,
        None,
        Some(&literature),
        &EvidenceProgramQuery::default(),
    )
    .expect("program validates");
    assert!(report.validate_integrity().is_ok());
    assert!(report
        .validate_for_inputs(&request, None, Some(&literature), None, None)
        .is_ok());

    let mut tampered = report.clone();
    tampered.lanes[0].tracks[0].label = "untrusted track label".to_string();
    assert!(tampered.validate_integrity().is_err());

    let mut rebound_request = request.clone();
    rebound_request.case_id = "different-case".to_string();
    assert!(report
        .validate_for_inputs(&rebound_request, None, Some(&literature), None, None)
        .is_err());
}

#[test]
fn program_rejects_unbounded_controls_and_missing_sources() {
    let no_source = build_evidence_program(
        &request(Specialty::Glioma),
        None,
        None,
        &EvidenceProgramQuery::default(),
    );
    assert!(no_source.is_err());

    let too_many = build_evidence_program(
        &request(Specialty::Glioma),
        None,
        Some(&literature()),
        &EvidenceProgramQuery {
            max_references_per_track: 17,
            ..Default::default()
        },
    );
    assert!(too_many.is_err());
}

#[test]
fn program_refuses_clinical_or_identified_requests_at_the_library_boundary() {
    let literature = literature();
    let mut clinical = request(Specialty::Glioma);
    clinical.request_use = RequestUse::TreatmentRecommendation;
    let clinical_error = build_evidence_program(
        &clinical,
        None,
        Some(&literature),
        &EvidenceProgramQuery::default(),
    )
    .expect_err("clinical use must be refused");
    assert!(matches!(
        clinical_error,
        bioprism_neurosurgery::NeurosurgeryError::ClinicalUseRefused { .. }
    ));

    let mut identified = request(Specialty::Glioma);
    identified.direct_identifier_fields = vec!["medical_record_number".to_string()];
    let identifier_error = build_evidence_program(
        &identified,
        None,
        Some(&literature),
        &EvidenceProgramQuery::default(),
    )
    .expect_err("direct identifiers must be refused");
    assert!(matches!(
        identifier_error,
        bioprism_neurosurgery::NeurosurgeryError::DirectIdentifiers { .. }
    ));
}

#[test]
fn program_reuses_typed_intake_coverage_without_exposing_values() {
    let mut case = request(Specialty::ChiariMalformation);
    case.observations.push(Observation {
        kind: ObservationKind::Imaging,
        label: "caller-supplied MRI metadata".to_string(),
        value: "redacted-at-boundary".to_string(),
        status: Default::default(),
        source_id: Some("asset:mri-baseline".to_string()),
        observed_at: None,
        timepoint: None,
    });
    let report = build_evidence_program(
        &case,
        None,
        Some(&literature()),
        &EvidenceProgramQuery::default(),
    )
    .expect("program validates");
    let track = &report.lanes[0].tracks[0];
    let imaging = track
        .observation_coverage
        .iter()
        .find(|coverage| coverage.observation_kind == ObservationKind::Imaging)
        .expect("junction track includes imaging coverage");
    assert_eq!(
        imaging.state,
        bioprism_neurosurgery::EvidenceState::Measured
    );
    assert_eq!(imaging.observed_count, 1);
    assert_eq!(imaging.provenance_complete_count, 1);
    assert!(track.observation_provenance_complete);
    let serialized = serde_json::to_string(&report).expect("report serializes");
    assert!(!serialized.contains("redacted-at-boundary"));
}

#[test]
fn program_joins_real_asset_coverage_to_each_protocol_track() {
    let request = request(Specialty::Glioma);
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "real-mri-baseline".to_string(),
            kind: CaseAssetKind::ImagingSeries,
            status: ObservationStatus::Observed,
            source_kind: CaseAssetSourceKind::DicomArchive,
            source_id: Some("dicom-archive".to_string()),
            content_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: None,
            timepoint: Some("baseline".to_string()),
        }],
    };
    let report = NeurosurgicalAgent::default()
        .evidence_program_with_case_assets(
            &request,
            None,
            Some(&literature()),
            &manifest,
            &CaseAssetManifestQuery::default(),
            &EvidenceProgramQuery::default(),
        )
        .expect("asset-aware program validates");
    let imaging = report.lanes[0].tracks[1]
        .asset_coverage
        .as_ref()
        .expect("asset coverage is present when a manifest is supplied")
        .iter()
        .find(|coverage| coverage.asset_kind == CaseAssetKind::ImagingSeries)
        .expect("imaging asset mapping is explicit");
    assert_eq!(
        imaging.state,
        bioprism_neurosurgery::EvidenceProgramAssetCoverageState::Observed
    );
    assert_eq!(imaging.observed_count, 1);
    assert_eq!(
        report.lanes[0].tracks[0].asset_coverage_complete,
        Some(false)
    );
    assert!(report.lanes[0].tracks[0]
        .missing_asset_kinds
        .contains(&CaseAssetKind::PathologyReport));
    assert!(report.lanes[0].tracks[0]
        .review_worklist
        .iter()
        .any(|item| item.code == "asset_class_missing"));
    let serialized = serde_json::to_string(&report).expect("asset-aware program serializes");
    assert!(!serialized.contains("real-mri-baseline"));
    assert!(!serialized.contains("dicom-archive"));
}
