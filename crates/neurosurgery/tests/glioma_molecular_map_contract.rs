use bioprism_neurosurgery::{
    CaseRequest, EvidenceSynthesisPlane, GliomaEvidenceState, GliomaMarker,
    GliomaMarkerObservation, GliomaMolecularMapQuery, GliomaMolecularPanel, NeurosurgicalAgent,
    Observation, ObservationKind, ObservationStatus, RequestUse, Specialty,
    NEUROSURGERY_SCHEMA_VERSION,
};

fn real_bundle() -> bioprism_neurosurgery::RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

fn literature_bundle() -> bioprism_neurosurgery::PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses")
}

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "deidentified-molecular-map-contract".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Map typed marker coverage to source-addressable public context".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: vec![Observation {
            kind: ObservationKind::Molecular,
            label: "private marker label".to_string(),
            value: "private marker value".to_string(),
            status: ObservationStatus::Observed,
            source_id: Some("caller-source".to_string()),
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
            timepoint: None,
        }],
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: Some(GliomaMolecularPanel {
            schema_version: bioprism_neurosurgery::GLIOMA_MOLECULAR_SCHEMA_VERSION.to_string(),
            observations: vec![
                GliomaMarkerObservation {
                    marker: GliomaMarker::Idh1Mutation,
                    state: GliomaEvidenceState::Present,
                    assay: Some("validated-panel".to_string()),
                    specimen: Some("tumour tissue".to_string()),
                    source_id: Some("molecular-source".to_string()),
                    observed_at: Some("2026-01-01T00:00:00Z".to_string()),
                },
                GliomaMarkerObservation {
                    marker: GliomaMarker::MgmtPromoterMethylation,
                    state: GliomaEvidenceState::NotCollected,
                    assay: None,
                    specimen: None,
                    source_id: None,
                    observed_at: None,
                },
            ],
        }),
    }
}

#[test]
fn map_is_source_addressable_and_preserves_marker_missingness() {
    let query = GliomaMolecularMapQuery {
        markers: Some(vec![
            GliomaMarker::Idh1Mutation,
            GliomaMarker::MgmtPromoterMethylation,
            GliomaMarker::EgfrAmplification,
        ]),
        real_data_query: Some(Default::default()),
        public_literature_query: Some(Default::default()),
        freshness: Some(bioprism_neurosurgery::RealDataFreshnessQuery {
            as_of: "2026-08-30T00:00:00Z".to_string(),
            max_age_days: 3650,
            source_id: None,
        }),
        max_hits_per_marker: 2,
        max_references: 24,
        include_source_text: false,
    };
    let request = request();
    let report = NeurosurgicalAgent::default()
        .glioma_molecular_map(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            &query,
        )
        .expect("validated real bundles should support molecular mapping");

    assert_eq!(
        report.schema_version,
        bioprism_neurosurgery::GLIOMA_MOLECULAR_MAP_SCHEMA_VERSION
    );
    assert_eq!(report.specialty, Specialty::Glioma);
    assert_eq!(report.markers.len(), 3);
    assert!(report.real_data_digest.is_some());
    assert!(report.public_literature_digest.is_some());
    assert!(report.real_data_freshness.is_some());
    assert!(report.public_literature_freshness.is_some());
    assert!(report
        .references
        .iter()
        .any(|reference| { reference.plane == EvidenceSynthesisPlane::RealGliomaPopulation }));
    assert!(report
        .references
        .iter()
        .any(|reference| { reference.plane == EvidenceSynthesisPlane::PublicLiterature }));
    let mgmt = report
        .markers
        .iter()
        .find(|marker| marker.marker == GliomaMarker::MgmtPromoterMethylation)
        .expect("requested MGMT marker is mapped");
    assert_eq!(mgmt.state, GliomaEvidenceState::NotCollected);
    assert!(mgmt
        .review_reasons
        .iter()
        .any(|reason| reason == "marker_not_collected"));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "marker_not_collected"));
    assert!(report.provenance_bound);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert!(report
        .map_digest
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
    let encoded = serde_json::to_string(&report).expect("map serializes");
    assert!(!encoded.contains("private marker label"));
    assert!(!encoded.contains("private marker value"));
}

#[test]
fn map_integrity_and_input_binding_reject_tampering() {
    let request = request();
    let query = GliomaMolecularMapQuery {
        markers: Some(vec![GliomaMarker::Idh1Mutation]),
        real_data_query: Some(Default::default()),
        public_literature_query: Some(Default::default()),
        max_hits_per_marker: 2,
        max_references: 16,
        ..Default::default()
    };
    let report = NeurosurgicalAgent::default()
        .glioma_molecular_map(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            &query,
        )
        .expect("map should build from validated snapshots");
    assert!(report.validate_integrity().is_ok());
    assert!(report
        .validate_for_inputs(&request, Some(&real_bundle()), Some(&literature_bundle()))
        .is_ok());

    let mut tampered = report.clone();
    tampered.markers[0].search_terms[0] = "untrusted synonym".to_string();
    assert!(tampered.validate_integrity().is_err());

    let mut rebound = report;
    rebound.request_digest = "f".repeat(64);
    assert!(rebound.validate_integrity().is_err());
}

#[test]
fn map_rejects_non_glioma_requests_and_duplicate_marker_queries() {
    let mut non_glioma = request();
    non_glioma.specialty = Specialty::ChiariMalformation;
    assert!(NeurosurgicalAgent::default()
        .glioma_molecular_map(&non_glioma, None, None, &Default::default())
        .is_err());

    let duplicate = GliomaMolecularMapQuery {
        markers: Some(vec![GliomaMarker::Idh1Mutation, GliomaMarker::Idh1Mutation]),
        ..Default::default()
    };
    assert!(NeurosurgicalAgent::default()
        .glioma_molecular_map(&request(), None, None, &duplicate)
        .is_err());
}
