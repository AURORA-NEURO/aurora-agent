use bioprism_neurosurgery::{
    NeurosurgeryError, RealDataCohortLandscapeQuery, RealDataQuery, RealDataRecordKind,
    RealGliomaBundle,
};

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_extended_snapshot.json"
    ))
    .expect("checked-in extended public snapshot parses")
}

#[test]
fn compares_real_tcga_projects_without_patient_level_values() {
    let report = bundle()
        .cohort_landscape(&RealDataCohortLandscapeQuery::default())
        .expect("cohort landscape should build from real metadata");
    assert_eq!(report.total_matching_projects, 2);
    assert_eq!(report.returned_project_count, 2);
    assert!(!report.truncated);
    assert_eq!(report.total_released_case_inventory, 617 + 516);
    assert_eq!(report.projects_with_data_type_metadata, 2);
    assert_eq!(report.projects_without_data_type_metadata, 0);
    assert!(report
        .project_rows
        .iter()
        .any(|row| row.project_id == "TCGA-GBM" && row.case_count == 617));
    assert!(report
        .project_rows
        .iter()
        .any(|row| row.project_id == "TCGA-LGG" && row.case_count == 516));
    assert!(!report.data_type_coverage.is_empty());
    assert!(!report.shared_data_types.is_empty());
    assert!(report
        .limitations
        .iter()
        .any(|limitation| limitation.contains("not a patient-level count")));
    report
        .validate_for_inputs(&bundle())
        .expect("cohort landscape should replay to the exact snapshot");
}

#[test]
fn cohort_landscape_preserves_project_bounds_and_rejects_wrong_scope() {
    let report = bundle()
        .cohort_landscape(&RealDataCohortLandscapeQuery {
            query: RealDataQuery {
                limit: 128,
                ..RealDataQuery::default()
            },
            max_projects: 1,
        })
        .expect("bounded cohort landscape should build");
    assert_eq!(report.returned_project_count, 1);
    assert_eq!(report.omitted_project_count, 1);
    assert!(report.truncated);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "project_rows_truncated" && reason.count == 1));

    let wrong_scope = RealDataCohortLandscapeQuery {
        query: RealDataQuery {
            record_kind: Some(RealDataRecordKind::LiteratureArticle),
            ..RealDataQuery::default()
        },
        ..RealDataCohortLandscapeQuery::default()
    };
    assert!(matches!(
        bundle().cohort_landscape(&wrong_scope),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}
