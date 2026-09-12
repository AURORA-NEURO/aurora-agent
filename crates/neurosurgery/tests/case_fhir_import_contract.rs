use bioprism_neurosurgery::{
    CaseAssetKind, CaseRequest, DicomCaseImport, FhirCaseImport, FhirCaseImportQuery,
    FhirResourceHint, NeurosurgicalAgent, ObservationStatus, RequestUse, Specialty,
    CASE_FHIR_ASSET_KIND_EXTENSION_URL, CASE_FHIR_IMPORT_SCHEMA_VERSION,
    NEUROSURGERY_SCHEMA_VERSION,
};

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "deidentified-fhir-contract".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Inventory sanitized FHIR asset metadata".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

fn import() -> FhirCaseImport {
    FhirCaseImport {
        schema_version: CASE_FHIR_IMPORT_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        deidentified: true,
        synthetic_data: false,
        source_id: "fhir-export-2026-01".to_string(),
        bundle: serde_json::json!({
            "resourceType": "Bundle",
            "type": "collection",
            "entry": [
                {
                    "resource": {
                        "resourceType": "ImagingStudy",
                        "id": "img-1",
                        "extension": [{
                            "url": CASE_FHIR_ASSET_KIND_EXTENSION_URL,
                            "valueCode": "imaging_series"
                        }],
                        "status": "available"
                    }
                },
                {
                    "resource": {
                        "resourceType": "Observation",
                        "id": "obs-1"
                    }
                }
            ]
        }),
        resource_hints: vec![FhirResourceHint {
            resource_id: "img-1".to_string(),
            asset_kind: CaseAssetKind::ImagingSeries,
            status: ObservationStatus::Observed,
            source_id: None,
            content_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
            timepoint: Some("baseline".to_string()),
        }],
        query: FhirCaseImportQuery {
            requested_kinds: Some(vec![CaseAssetKind::ImagingSeries]),
            max_review_items: 32,
        },
    }
}

#[test]
fn imports_real_sanitized_bundle_as_digest_only_assets() {
    let report = NeurosurgicalAgent::default()
        .case_fhir_import(&request(), &import())
        .expect("sanitized bundle should import");
    assert_eq!(report.resource_count, 2);
    assert_eq!(report.projected_asset_count, 1);
    assert_eq!(report.unclassified_resource_count, 1);
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "asset_kind_missing"));
    assert!(report.manifest_report.assets[0].asset_ref.len() == 64);
    assert!(!serde_json::to_string(&report)
        .expect("report serializes")
        .contains("fhir-export-2026-01"));
    report.validate_integrity().expect("report integrity");
    report
        .validate_for_inputs(&request(), &import())
        .expect("report should replay");
}

#[test]
fn importer_rejects_identifiers_synthetic_data_and_replay_drift() {
    let agent = NeurosurgicalAgent::default();
    let mut identified = import();
    identified.bundle["entry"][0]["resource"]["subject"] = serde_json::json!({
        "reference": "Patient/deidentified"
    });
    assert!(agent.case_fhir_import(&request(), &identified).is_err());

    let mut synthetic = import();
    synthetic.synthetic_data = true;
    assert!(agent.case_fhir_import(&request(), &synthetic).is_err());

    let report = agent
        .case_fhir_import(&request(), &import())
        .expect("baseline import");
    let mut changed = import();
    changed.bundle["entry"][0]["resource"]["id"] = serde_json::json!("img-2");
    assert!(report.validate_for_inputs(&request(), &changed).is_err());
}

#[test]
fn fhir_metadata_rebinds_mission_case_planes_and_replays() {
    let request = request();
    let real_data: bioprism_neurosurgery::RealGliomaBundle = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_fhir(
            &request,
            Some(&real_data),
            None,
            None,
            None,
            None,
            None,
            &import(),
            32,
        )
        .expect("FHIR metadata should compose into a real-data mission");
    assert!(
        mission
            .validate_for_inputs(&request, Some(&real_data), None)
            .is_err(),
        "exact replay must require the original FHIR Bundle"
    );
    mission
        .validate_for_inputs_with_case_imports(
            &request,
            Some(&real_data),
            None,
            None,
            Some(&import()),
        )
        .expect("FHIR-backed mission should replay against the exact inputs");
    let fhir = mission
        .case_fhir_import
        .as_ref()
        .expect("mission carries the FHIR receipt");
    assert_eq!(
        mission.case_asset_manifest.as_ref(),
        Some(&fhir.manifest_report)
    );
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|report| report.case_asset_report_digest.as_deref()),
        Some(fhir.manifest_report.report_digest.as_str())
    );
    assert_eq!(
        mission.mission_audit.as_ref().map(|audit| audit.fail_count),
        Some(0)
    );
    assert_eq!(mission.provider, "none");
    assert!(!mission.network);
    assert!(mission.human_review_required);
}

#[test]
fn dicom_and_fhir_metadata_compose_into_one_multimodal_mission_manifest() {
    let request = request();
    let real_data: bioprism_neurosurgery::RealGliomaBundle = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let dicom: DicomCaseImport = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM metadata fixture parses");
    let fhir = import();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_imports(
            &request,
            Some(&real_data),
            None,
            None,
            None,
            None,
            None,
            Some(&dicom),
            Some(&fhir),
            32,
        )
        .expect("multimodal case imports should compose");
    let manifest = mission
        .case_asset_manifest
        .as_ref()
        .expect("mission carries the composed manifest");
    assert_eq!(manifest.asset_count, 3);
    assert_eq!(
        mission
            .case_dicom_import
            .as_ref()
            .map(|report| report.projected_series_count),
        Some(2)
    );
    assert_eq!(
        mission
            .case_fhir_import
            .as_ref()
            .map(|report| report.projected_asset_count),
        Some(1)
    );
    assert_eq!(
        mission.mission_audit.as_ref().map(|audit| audit.fail_count),
        Some(0)
    );
    assert!(mission
        .validate_for_inputs_with_case_imports(
            &request,
            Some(&real_data),
            None,
            Some(&dicom),
            Some(&fhir),
        )
        .is_ok());
    assert!(mission
        .validate_for_inputs(&request, Some(&real_data), None)
        .is_err());
    assert!(manifest.assets.iter().any(|asset| {
        asset.kind == CaseAssetKind::ImagingSeries
            && asset.source_kind == bioprism_neurosurgery::CaseAssetSourceKind::DicomArchive
    }));
    assert!(manifest.assets.iter().any(|asset| {
        asset.kind == CaseAssetKind::ImagingSeries
            && asset.source_kind == bioprism_neurosurgery::CaseAssetSourceKind::CallerExport
    }));
}
