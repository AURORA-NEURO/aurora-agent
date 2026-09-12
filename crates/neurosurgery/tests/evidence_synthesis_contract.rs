use bioprism_neurosurgery::{
    CaseRequest, EvidenceRecord, EvidenceState, EvidenceSynthesisPlane, EvidenceSynthesisQuery,
    EvidenceTier, NeurosurgicalAgent, Observation, ObservationKind, ObservationStatus, RequestUse,
    Specialty, ToolCapability, NEUROSURGERY_SCHEMA_VERSION,
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
        case_id: "deidentified-glioma-synthesis-case".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Align molecular and imaging research evidence for a de-identified case"
            .to_string(),
        direct_identifier_fields: Vec::new(),
        observations: vec![Observation {
            kind: ObservationKind::Imaging,
            label: "private imaging label must not be echoed".to_string(),
            value: "private imaging value must remain in caller case".to_string(),
            status: ObservationStatus::Observed,
            source_id: Some("caller-metadata-source".to_string()),
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
            timepoint: Some("baseline".to_string()),
        }],
        evidence: vec![EvidenceRecord {
            id: "caller-guideline-1".to_string(),
            title: "Caller-provided review reference".to_string(),
            citation: "https://example.invalid/caller-reference".to_string(),
            tier: EvidenceTier::Guideline,
            population: Some("public research context".to_string()),
            year: Some(2025),
            supports: vec![ToolCapability::EvidenceSynthesis],
        }],
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

#[test]
fn synthesis_aligns_case_and_both_real_public_planes_without_echoing_case_text() {
    let query = EvidenceSynthesisQuery {
        max_references: 200,
        include_source_text: true,
        freshness: Some(bioprism_neurosurgery::RealDataFreshnessQuery {
            as_of: "2026-08-30T00:00:00Z".to_string(),
            max_age_days: 3650,
            source_id: None,
        }),
        ..EvidenceSynthesisQuery::default()
    };
    let request = request();
    let report = NeurosurgicalAgent::default()
        .evidence_synthesis(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            &query,
        )
        .expect("validated public bundles should align");

    assert_eq!(
        report.schema_version,
        bioprism_neurosurgery::EVIDENCE_SYNTHESIS_SCHEMA_VERSION
    );
    assert!(report
        .synthesis_digest
        .chars()
        .all(|c| c.is_ascii_hexdigit()));
    assert_eq!(report.case_observations.len(), 1);
    assert!(report
        .references
        .iter()
        .any(|reference| reference.plane == EvidenceSynthesisPlane::CallerEvidence));
    assert!(report
        .references
        .iter()
        .any(|reference| reference.plane == EvidenceSynthesisPlane::RealGliomaPopulation));
    assert!(report
        .references
        .iter()
        .any(|reference| reference.plane == EvidenceSynthesisPlane::PublicLiterature));
    assert!(report.literature_link_audit.is_some());
    assert!(report.real_data_freshness.is_some());
    assert!(report.public_literature_freshness.is_some());
    assert_eq!(
        report.lanes.len(),
        bioprism_neurosurgery::required_capabilities(Specialty::Glioma).len()
    );
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    let encoded = serde_json::to_string(&report).expect("report serializes");
    assert!(!encoded.contains("private imaging label"));
    assert!(!encoded.contains("private imaging value"));
}

#[test]
fn case_only_synthesis_keeps_population_absence_explicit() {
    let report = NeurosurgicalAgent::default()
        .evidence_synthesis(&request(), None, None, &EvidenceSynthesisQuery::default())
        .expect("case-only alignment is a valid research handoff");

    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "real_glioma_population_unattached"));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "public_literature_unattached"));
    assert!(report
        .lanes
        .iter()
        .any(|lane| lane.capability == ToolCapability::EvidenceSynthesis
            && lane.evidence_state == EvidenceState::Measured));
}

#[test]
fn synthesis_rejects_a_public_query_for_the_wrong_specialty() {
    let query = EvidenceSynthesisQuery {
        public_literature_query: Some(bioprism_neurosurgery::PublicLiteratureQuery {
            specialty: Some(Specialty::ChiariMalformation),
            limit: 1,
            ..Default::default()
        }),
        ..EvidenceSynthesisQuery::default()
    };
    assert!(NeurosurgicalAgent::default()
        .evidence_synthesis(&request(), None, Some(&literature_bundle()), &query)
        .is_err());
}

#[test]
fn synthesis_preserves_an_explicit_synthetic_case_label_without_public_data() {
    let mut request = request();
    request.request_use = RequestUse::SyntheticCaseSimulation;
    let report = NeurosurgicalAgent::default()
        .evidence_synthesis(&request, None, None, &EvidenceSynthesisQuery::default())
        .expect("an unattached synthetic educational case remains a bounded audit");
    assert!(report.synthetic_data);
    assert!(report
        .references
        .iter()
        .all(|reference| reference.plane == EvidenceSynthesisPlane::CallerEvidence));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "real_glioma_population_unattached"));
}

#[test]
fn synthesis_report_is_self_validating_and_replay_bound() {
    let request = request();
    let real = real_bundle();
    let literature = literature_bundle();
    let report = NeurosurgicalAgent::default()
        .evidence_synthesis(
            &request,
            Some(&real),
            Some(&literature),
            &EvidenceSynthesisQuery::default(),
        )
        .expect("validated snapshots should synthesize");

    report
        .validate_integrity()
        .expect("fresh synthesis should satisfy its envelope contract");
    report
        .validate_for_inputs(&request, Some(&real), Some(&literature), None, None)
        .expect("fresh synthesis should replay against its exact inputs");

    let mut tampered = report.clone();
    tampered.lanes[0].case_observation_count += 1;
    assert!(tampered.validate_integrity().is_err());

    let mut rebound = request.clone();
    rebound.question.push_str(" (rebound)");
    assert!(report
        .validate_for_inputs(&rebound, Some(&real), Some(&literature), None, None)
        .is_err());
}
