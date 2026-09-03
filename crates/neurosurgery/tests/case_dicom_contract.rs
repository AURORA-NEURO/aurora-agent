use bioprism_neurosurgery::{
    CaseRequest, DicomCaseImport, DicomCaseImportQuery, NeurosurgicalAgent, RealGliomaBundle,
    RequestUse, Specialty, CASE_DICOM_IMPORT_SCHEMA_VERSION, NEUROSURGERY_SCHEMA_VERSION,
};
use serde_json::json;

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "deidentified-dicom-contract".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Inventory de-identified imaging metadata".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

fn import() -> DicomCaseImport {
    serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM fixture should deserialize")
}

fn real_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot should deserialize")
}

#[test]
fn imports_real_deidentified_dicom_json_as_digest_only_series() {
    let report = NeurosurgicalAgent::default()
        .case_dicom_import(&request(), &import())
        .expect("DICOM metadata should import");
    assert_eq!(report.dataset_count, 2);
    assert_eq!(report.projected_series_count, 2);
    assert_eq!(report.unclassified_dataset_count, 0);
    assert_eq!(report.manifest_report.asset_count, 2);
    assert!(report
        .review_items
        .iter()
        .all(|item| item.code == "content_digest_missing"));
    assert!(report
        .series
        .iter()
        .all(|row| row.metadata_digest.len() == 64));
    assert!(report.series.iter().all(|row| row.study_ref.is_some()));
    assert!(!serde_json::to_string(&report)
        .expect("report serializes")
        .contains("deidentified-dicom-metadata-export-001"));
    report.validate_integrity().expect("report integrity");
    report
        .validate_for_inputs(&request(), &import())
        .expect("report should replay");
}

#[test]
fn refuses_identifiers_pixels_and_synthetic_metadata() {
    let agent = NeurosurgicalAgent::default();
    let mut identified = import();
    identified.datasets[0]["00100010"] = json!({"vr": "PN", "Value": ["redacted"]});
    assert!(agent.case_dicom_import(&request(), &identified).is_err());

    let mut pixels = import();
    pixels.datasets[0]["7FE00010"] = json!({"vr": "OW", "Value": ["base64"]});
    assert!(agent.case_dicom_import(&request(), &pixels).is_err());

    let mut synthetic = import();
    synthetic.synthetic_data = true;
    assert!(agent.case_dicom_import(&request(), &synthetic).is_err());
}

#[test]
fn missing_series_uid_requires_explicit_index_fallback_and_review() {
    let agent = NeurosurgicalAgent::default();
    let mut missing = import();
    missing.datasets[0]
        .as_object_mut()
        .unwrap()
        .remove("0020000E");
    assert!(agent.case_dicom_import(&request(), &missing).is_ok());
    let report = agent
        .case_dicom_import(&request(), &missing)
        .expect("second dataset still projects");
    assert_eq!(report.projected_series_count, 1);
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "series_uid_missing"));

    missing.query = DicomCaseImportQuery {
        allow_missing_series_uid: true,
        ..missing.query
    };
    let report = agent
        .case_dicom_import(&request(), &missing)
        .expect("explicit index fallback should project");
    assert_eq!(report.projected_series_count, 2);
    assert!(report
        .series
        .iter()
        .any(|row| row.series_ref == "dataset-0"));
    assert!(report
        .review_items
        .iter()
        .any(|item| item.code == "series_uid_missing"));
    assert_eq!(import().schema_version, CASE_DICOM_IMPORT_SCHEMA_VERSION);
}

#[test]
fn rejects_invalid_calendar_dates() {
    let agent = NeurosurgicalAgent::default();
    let mut invalid = import();
    invalid.datasets[0]["00080020"] = json!({"vr": "DA", "Value": ["20250229"]});
    assert!(agent.case_dicom_import(&request(), &invalid).is_err());
}

#[test]
fn mission_binds_dicom_receipt_to_synthesis_program_and_acquisition() {
    let request = request();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_dicom(&request, &real_bundle(), None, None, &import(), 32)
        .expect("DICOM-backed glioma mission should compose");
    mission
        .validate_integrity()
        .expect("DICOM-backed mission integrity");
    assert!(
        mission
            .validate_for_inputs(&request, Some(&real_bundle()), None)
            .is_err(),
        "exact replay must require the original DICOM metadata"
    );
    mission
        .validate_for_inputs_with_case_imports(
            &request,
            Some(&real_bundle()),
            None,
            Some(&import()),
            None,
        )
        .expect("DICOM-backed mission replay");
    let dicom = mission
        .case_dicom_import
        .as_ref()
        .expect("mission carries DICOM import receipt");
    let manifest = mission
        .case_asset_manifest
        .as_ref()
        .expect("mission carries DICOM-derived asset manifest");
    assert_eq!(&dicom.manifest_report, manifest);
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|report| report.case_asset_report_digest.as_deref()),
        Some(manifest.report_digest.as_str())
    );
    assert!(mission
        .evidence_program
        .as_ref()
        .is_some_and(|report| report.lanes.iter().any(|lane| {
            lane.tracks
                .iter()
                .any(|track| track.asset_coverage.is_some())
        })));
    assert_eq!(
        mission
            .evidence_acquisition
            .as_ref()
            .and_then(|report| report.case_asset_report_digest.as_deref()),
        Some(manifest.report_digest.as_str())
    );
}
