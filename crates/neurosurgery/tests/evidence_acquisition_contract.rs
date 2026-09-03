use bioprism_neurosurgery::{
    CaseAsset, CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery, CaseAssetSourceKind,
    EvidenceAcquisitionQuery, EvidenceAcquisitionSourceQuery, EvidenceAcquisitionStepStatus,
    NeurosurgicalAgent, ObservationStatus, PublicLiteratureBundle, RealDataFreshnessQuery,
    RealGliomaBundle, ResearchPlanSource, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};

fn real_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real bundle parses")
}

fn literature_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("literature bundle parses")
}

fn request(specialty: Specialty) -> bioprism_neurosurgery::CaseRequest {
    bioprism_neurosurgery::CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "acquisition-contract-001".to_string(),
        specialty,
        request_use: bioprism_neurosurgery::RequestUse::ResearchSynthesis,
        question: "Map real evidence gaps for qualified review".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

#[test]
fn acquisition_wave_is_dual_plane_bounded_and_replayable() {
    let agent = NeurosurgicalAgent::default();
    let request = request(Specialty::Glioma);
    let query = EvidenceAcquisitionQuery {
        max_steps: 8,
        max_references_per_step: 3,
        freshness: Some(RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: None,
        }),
    };
    let real = real_bundle();
    let literature = literature_bundle();
    let first = agent
        .evidence_acquisition(&request, Some(&real), Some(&literature), &query)
        .expect("dual-plane acquisition wave should compile");
    let second = agent
        .evidence_acquisition(&request, Some(&real), Some(&literature), &query)
        .expect("replay should compile identically");
    assert_eq!(first, second);
    assert_eq!(first.plan_digest.len(), 64);
    assert!(first.ready_for_local_replay);
    assert!(first.required_sources.is_empty());
    assert!(first.steps.len() <= 8);
    assert!(first
        .steps
        .iter()
        .any(|step| step.source == ResearchPlanSource::RealGliomaPopulation));
    assert!(first
        .steps
        .iter()
        .any(|step| step.source == ResearchPlanSource::PublicLiterature));
    assert!(first.steps.iter().all(|step| {
        matches!(
            step.query,
            EvidenceAcquisitionSourceQuery::RealGliomaPopulation(_)
                | EvidenceAcquisitionSourceQuery::PublicLiterature(_)
        )
    }));
    assert!(first.steps.iter().any(|step| {
        step.status == EvidenceAcquisitionStepStatus::CandidatesFound
            || step.status == EvidenceAcquisitionStepStatus::Truncated
    }));
    assert_eq!(
        first
            .real_data_freshness
            .as_ref()
            .unwrap()
            .query
            .max_age_days,
        30
    );
    assert_eq!(
        first
            .public_literature_freshness
            .as_ref()
            .unwrap()
            .query
            .max_age_days,
        30
    );
    assert_eq!(first.audit.provider, "none");
}

#[test]
fn acquisition_wave_keeps_missing_sources_explicit() {
    let agent = NeurosurgicalAgent::default();
    let request = request(Specialty::Glioma);
    let report = agent
        .evidence_acquisition(&request, None, None, &EvidenceAcquisitionQuery::default())
        .expect("missing bundles should produce an explicit handoff");
    assert!(!report.ready_for_local_replay);
    assert!(report.steps.is_empty());
    assert_eq!(
        report.required_sources,
        vec![
            ResearchPlanSource::RealGliomaPopulation,
            ResearchPlanSource::PublicLiterature
        ]
    );
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
}

#[test]
fn acquisition_wave_rejects_synthetic_real_bundle() {
    let agent = NeurosurgicalAgent::default();
    let request = request(Specialty::Glioma);
    let mut real = real_bundle();
    real.synthetic_data = true;
    let error = agent
        .evidence_acquisition(
            &request,
            Some(&real),
            None,
            &EvidenceAcquisitionQuery::default(),
        )
        .expect_err("synthetic bundles must fail closed");
    assert!(error.to_string().contains("synthetic_data=true"));
}

#[test]
fn acquisition_wave_rejects_real_bundle_for_non_glioma_lane() {
    let agent = NeurosurgicalAgent::default();
    let error = agent
        .evidence_acquisition(
            &request(Specialty::ChiariMalformation),
            Some(&real_bundle()),
            None,
            &EvidenceAcquisitionQuery::default(),
        )
        .expect_err("real glioma bundle must not cross specialty lanes");
    assert!(error.to_string().contains("only implemented for glioma"));
}

#[test]
fn acquisition_wave_carries_digest_bound_asset_review_obligations() {
    let agent = NeurosurgicalAgent::default();
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
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
            timepoint: Some("baseline".to_string()),
        }],
    };
    let asset_report = agent
        .case_asset_manifest(&request, &manifest, &CaseAssetManifestQuery::default())
        .expect("asset projection should validate");
    let report = agent
        .evidence_acquisition_with_case_assets(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            Some(&asset_report),
            &EvidenceAcquisitionQuery::default(),
        )
        .expect("asset-aware acquisition should compile");
    assert_eq!(
        report.case_asset_report_digest.as_deref(),
        Some(asset_report.report_digest.as_str())
    );
    assert_eq!(report.case_asset_review_items, asset_report.review_items);
    assert_eq!(
        report.case_asset_omitted_review_item_count,
        asset_report.omitted_review_item_count
    );
    let disposition = agent
        .case_asset_review_disposition(&asset_report, &[])
        .expect("empty disposition ledger should bind to the projection");
    let disposition_report = agent
        .evidence_acquisition_with_case_assets_and_dispositions(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            Some(&asset_report),
            &disposition,
            &EvidenceAcquisitionQuery::default(),
        )
        .expect("disposition-aware acquisition should compile");
    assert_eq!(
        disposition_report
            .case_asset_review_disposition_digest
            .as_deref(),
        Some(disposition.disposition_digest.as_str())
    );
    let started = agent
        .evidence_acquisition_start_with_case_assets_and_dispositions(
            &request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            Some(&asset_report),
            &disposition,
            &EvidenceAcquisitionQuery::default(),
        )
        .expect("disposition-aware acquisition worker should start");
    assert_eq!(
        started
            .session
            .case_asset_review_disposition_digest
            .as_deref(),
        Some(disposition.disposition_digest.as_str())
    );
    let encoded = serde_json::to_string(&report).expect("acquisition report serializes");
    assert!(!encoded.contains("real-mri-baseline"));
    assert!(!encoded.contains("dicom-archive"));
}

#[test]
fn acquisition_wave_rejects_asset_projection_bound_to_another_request() {
    let agent = NeurosurgicalAgent::default();
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![],
    };
    let other_request = request(Specialty::Glioma);
    let report = agent
        .case_asset_manifest(
            &other_request,
            &manifest,
            &CaseAssetManifestQuery::default(),
        )
        .expect("empty manifest projection should still be digest bound");
    let mut tampered = report.clone();
    tampered.requested_kinds.push(CaseAssetKind::ImagingSeries);
    let digest_error = agent
        .evidence_acquisition_with_case_assets(
            &other_request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            Some(&tampered),
            &EvidenceAcquisitionQuery::default(),
        )
        .expect_err("tampered asset projection must fail closed");
    assert!(digest_error
        .to_string()
        .contains("report digest does not match"));
    let mut changed_request = request(Specialty::Glioma);
    changed_request.case_id = "different-case".to_string();
    let error = agent
        .evidence_acquisition_with_case_assets(
            &changed_request,
            Some(&real_bundle()),
            Some(&literature_bundle()),
            Some(&report),
            &EvidenceAcquisitionQuery::default(),
        )
        .expect_err("asset projection from another request must fail closed");
    assert!(error
        .to_string()
        .contains("case-asset acquisition projection"));
}
