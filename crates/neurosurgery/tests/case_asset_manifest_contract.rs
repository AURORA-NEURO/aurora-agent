use bioprism_neurosurgery::{
    CaseAsset, CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery, CaseAssetReviewDecision,
    CaseAssetReviewDisposition, CaseAssetSourceKind, CaseRequest, NeurosurgicalAgent,
    ObservationStatus, RequestUse, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};

fn request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "deidentified-case-asset-contract".to_string(),
        specialty,
        request_use: RequestUse::ResearchSynthesis,
        question: "Inventory real de-identified multimodal asset provenance".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

fn manifest(specialty: Specialty) -> CaseAssetManifest {
    CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![
            CaseAsset {
                asset_id: "asset-local-imaging-1".to_string(),
                kind: CaseAssetKind::ImagingSeries,
                status: ObservationStatus::Observed,
                source_kind: CaseAssetSourceKind::DicomArchive,
                source_id: Some("source-imaging-archive".to_string()),
                content_sha256: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                modality: Some("MR".to_string()),
                body_region: Some("brain".to_string()),
                observed_at: Some("2026-01-01T00:00:00Z".to_string()),
                timepoint: Some("baseline".to_string()),
            },
            CaseAsset {
                asset_id: "asset-local-pathology-1".to_string(),
                kind: CaseAssetKind::PathologyReport,
                status: ObservationStatus::Uninterpretable,
                source_kind: CaseAssetSourceKind::PathologyLaboratory,
                source_id: Some("source-pathology-laboratory".to_string()),
                content_sha256: None,
                modality: Some("histology-report".to_string()),
                body_region: Some("tumour-tissue".to_string()),
                observed_at: None,
                timepoint: None,
            },
        ],
    }
}

#[test]
fn manifest_projects_real_asset_metadata_without_echoing_local_ids() {
    let query = CaseAssetManifestQuery {
        requested_kinds: Some(vec![
            CaseAssetKind::ImagingSeries,
            CaseAssetKind::MolecularAssay,
        ]),
        max_review_items: 32,
    };
    let report = NeurosurgicalAgent::default()
        .case_asset_manifest(
            &request(Specialty::Glioma),
            &manifest(Specialty::Glioma),
            &query,
        )
        .expect("valid de-identified asset manifest should project");

    assert_eq!(
        report.schema_version,
        bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION
    );
    assert_eq!(report.asset_count, 2);
    assert_eq!(report.observed_asset_count, 1);
    assert_eq!(report.provenance_complete_asset_count, 1);
    assert_eq!(report.requested_kinds.len(), 2);
    assert_eq!(
        report.missing_requested_kinds,
        vec![CaseAssetKind::MolecularAssay]
    );
    assert!(report
        .coverage
        .iter()
        .any(|coverage| coverage.kind == CaseAssetKind::ImagingSeries
            && coverage.provenance_complete_count == 1));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "asset_uninterpretable"));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "requested_kind_missing"));
    assert!(report.deidentified);
    assert!(!report.raw_values_retained);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.report_digest.len(), 64);
    let encoded = serde_json::to_string(&report).expect("asset manifest report serializes");
    assert!(!encoded.contains("asset-local-imaging-1"));
    assert!(!encoded.contains("source-imaging-archive"));
}

#[test]
fn manifest_rejects_synthetic_or_unsafe_drift_before_projection() {
    let agent = NeurosurgicalAgent::default();
    assert!(
        serde_json::from_value::<CaseAssetManifest>(serde_json::json!({
            "schema_version": bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION,
            "specialty": "glioma",
            "assets": []
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<CaseAssetManifest>(serde_json::json!({
            "schema_version": bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION,
            "specialty": "glioma",
            "synthetic_data": false,
            "assets": [{
                "asset_id": "asset-without-state",
                "kind": "imaging_series",
                "source_kind": "dicom_archive"
            }]
        }))
        .is_err()
    );
    let mut synthetic = manifest(Specialty::Glioma);
    synthetic.synthetic_data = true;
    assert!(agent
        .case_asset_manifest(
            &request(Specialty::Glioma),
            &synthetic,
            &CaseAssetManifestQuery::default(),
        )
        .is_err());

    let mut identifiers = manifest(Specialty::Glioma);
    identifiers.direct_identifier_fields = vec!["patient_name".to_string()];
    assert!(agent
        .case_asset_manifest(
            &request(Specialty::Glioma),
            &identifiers,
            &CaseAssetManifestQuery::default(),
        )
        .is_err());

    let mut drift = manifest(Specialty::ChiariMalformation);
    assert!(agent
        .case_asset_manifest(
            &request(Specialty::Glioma),
            &drift,
            &CaseAssetManifestQuery::default(),
        )
        .is_err());
    drift.specialty = Specialty::Glioma;
    drift.assets[0].content_sha256 = Some("not-a-sha256".to_string());
    assert!(agent
        .case_asset_manifest(
            &request(Specialty::Glioma),
            &drift,
            &CaseAssetManifestQuery::default(),
        )
        .is_err());
}

#[test]
fn persisted_manifest_reports_are_integrity_checked_before_joining_synthesis() {
    let agent = NeurosurgicalAgent::default();
    let request_case = request(Specialty::Glioma);
    let report = agent
        .case_asset_manifest(
            &request_case,
            &manifest(Specialty::Glioma),
            &CaseAssetManifestQuery::default(),
        )
        .expect("valid manifest should project");
    report
        .validate_for_request(&request_case)
        .expect("fresh projection should validate");

    let mut tampered = report.clone();
    tampered.assets[0].modality = Some("CT".to_string());
    assert!(tampered.validate_integrity().is_err());
    assert!(agent
        .evidence_synthesis_with_case_assets(
            &request_case,
            None,
            None,
            &bioprism_neurosurgery::EvidenceSynthesisQuery::default(),
            Some(&tampered),
        )
        .is_err());

    let other_request = request(Specialty::ChiariMalformation);
    assert!(report.validate_for_request(&other_request).is_err());
}

#[test]
fn review_dispositions_are_digest_bound_deterministic_and_leave_pending_obligations_visible() {
    let agent = NeurosurgicalAgent::default();
    let request_case = request(Specialty::Glioma);
    let report = agent
        .case_asset_manifest(
            &request_case,
            &manifest(Specialty::Glioma),
            &CaseAssetManifestQuery::default(),
        )
        .expect("valid manifest should project");
    assert!(report.review_items.len() >= 2);
    let first = report.review_items[0].sequence;
    let second = report.review_items[1].sequence;
    let reviewed = CaseAssetReviewDecision {
        sequence: first,
        disposition: CaseAssetReviewDisposition::Reviewed,
        reviewer_id: "clinician-a".to_string(),
    };
    let unresolved = CaseAssetReviewDecision {
        sequence: second,
        disposition: CaseAssetReviewDisposition::Unresolved,
        reviewer_id: "clinician-b".to_string(),
    };
    let left = agent
        .case_asset_review_disposition(&report, &[reviewed.clone(), unresolved.clone()])
        .expect("valid dispositions should apply");
    let right = agent
        .case_asset_review_disposition(&report, &[unresolved.clone(), reviewed.clone()])
        .expect("decision order should not alter the digest");
    assert_eq!(left, right);
    assert_eq!(left.submitted_decision_count, 2);
    assert_eq!(left.resolved_decision_count, 1);
    assert_eq!(left.unresolved_decision_count, 1);
    assert_eq!(left.unresolved_sequences, vec![second]);
    assert_eq!(
        left.undecided_returned_item_count,
        report.review_items.len() - 2
    );
    assert_eq!(
        left.pending_item_count,
        report.omitted_review_item_count + report.review_items.len() - 1
    );
    assert_eq!(left.report_digest, report.report_digest);
    assert_eq!(left.disposition_digest.len(), 64);
    left.validate_integrity()
        .expect("accepted disposition report should validate after persistence");
    let synthesis = agent
        .evidence_synthesis_with_case_assets_and_dispositions(
            &request_case,
            None,
            None,
            &bioprism_neurosurgery::EvidenceSynthesisQuery::default(),
            Some(&report),
            Some(&left),
        )
        .expect("validated dispositions should join evidence synthesis");
    assert_eq!(
        synthesis.case_asset_review_disposition_digest,
        Some(left.disposition_digest.clone())
    );
    assert_eq!(
        synthesis.case_asset_review_pending_item_count,
        Some(left.pending_item_count)
    );
    assert_eq!(
        synthesis.case_asset_review_resolved_decision_count,
        Some(left.resolved_decision_count)
    );
    assert_eq!(
        synthesis.case_asset_review_unresolved_decision_count,
        Some(left.unresolved_decision_count)
    );
    assert!(agent
        .evidence_synthesis_with_case_assets_and_dispositions(
            &request_case,
            None,
            None,
            &bioprism_neurosurgery::EvidenceSynthesisQuery::default(),
            None,
            Some(&left),
        )
        .is_err());
    let mut tampered_ledger = left.clone();
    tampered_ledger.pending_item_count = 0;
    assert!(tampered_ledger.validate_integrity().is_err());
    let mut incomplete_ledger = left.clone();
    incomplete_ledger.undecided_sequences = vec![u16::MAX];
    incomplete_ledger.undecided_returned_item_count = 1;
    assert!(incomplete_ledger.validate_integrity().is_err());

    let duplicate = vec![reviewed.clone(), reviewed];
    assert!(agent
        .case_asset_review_disposition(&report, &duplicate)
        .is_err());
    let unknown = CaseAssetReviewDecision {
        sequence: u16::MAX,
        disposition: CaseAssetReviewDisposition::Reviewed,
        reviewer_id: "clinician-a".to_string(),
    };
    assert!(agent
        .case_asset_review_disposition(&report, &[unknown])
        .is_err());
    let mut tampered = report.clone();
    tampered.review_items[0].reason.push_str(" drift");
    assert!(agent
        .case_asset_review_disposition(&tampered, &[unresolved])
        .is_err());
}
