use bioprism_neurosurgery::{
    CaseRequest, DicomCaseImport, DicomEvidenceWorkflowQuery, NeurosurgicalAgent,
    RealDataReasoningContextQuery, RequestUse, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};

fn request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "deidentified-dicom-workflow-contract".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Bind imaging metadata to source-grounded glioma review".to_string(),
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

fn real_data() -> bioprism_neurosurgery::RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot should deserialize")
}

#[test]
fn composes_dicom_projection_with_real_evidence_workers() {
    let request = request();
    let real_data = real_data();
    let report = NeurosurgicalAgent::default()
        .case_dicom_evidence_workflow(
            &request,
            &import(),
            Some(&real_data),
            None,
            &DicomEvidenceWorkflowQuery::default(),
        )
        .expect("DICOM workflow should compose");

    assert_eq!(report.specialty, Specialty::Glioma);
    assert_eq!(report.dicom_import.projected_series_count, 2);
    assert_eq!(
        report
            .evidence_synthesis
            .case_asset_summary
            .as_ref()
            .unwrap()
            .report_digest,
        report.dicom_import.manifest_report.report_digest
    );
    assert_eq!(
        report.evidence_program.request_digest,
        report.request_digest
    );
    assert_eq!(
        report
            .evidence_acquisition
            .case_asset_report_digest
            .as_deref(),
        Some(report.dicom_import.manifest_report.report_digest.as_str())
    );
    assert_eq!(
        report.evidence_acquisition_session.plan_digest,
        report.evidence_acquisition.plan_digest
    );
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report.validate_integrity().expect("workflow integrity");
    report
        .validate_for_inputs(&request, &import(), Some(&real_data), None)
        .expect("workflow should replay against exact inputs");
}

#[test]
fn requires_the_real_glioma_plane() {
    let error = NeurosurgicalAgent::default()
        .case_dicom_evidence_workflow(
            &request(),
            &import(),
            None,
            None,
            &DicomEvidenceWorkflowQuery::default(),
        )
        .expect_err("glioma workflow without public population data must fail closed");
    assert!(error.to_string().contains("real-data bundle"));
}

#[test]
fn optional_real_context_is_bound_to_the_same_snapshot() {
    let request = request();
    let real_data = real_data();
    let query = DicomEvidenceWorkflowQuery {
        real_data_reasoning_context: Some(RealDataReasoningContextQuery::default()),
        ..DicomEvidenceWorkflowQuery::default()
    };
    let report = NeurosurgicalAgent::default()
        .case_dicom_evidence_workflow(&request, &import(), Some(&real_data), None, &query)
        .expect("real-data reasoning context should compose");
    let context = report
        .real_data_reasoning_context
        .as_ref()
        .expect("context requested by query");
    assert_eq!(
        context.bundle_digest,
        report.real_data_digest.clone().unwrap()
    );
    assert_eq!(
        context.query,
        query.real_data_reasoning_context.clone().unwrap()
    );
    report
        .validate_integrity()
        .expect("context-bound workflow integrity");
}
