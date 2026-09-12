use bioprism_neurosurgery::{
    NeurosurgeryError, RealDataCoverageQuery, RealDataRecordKind, RealGliomaBundle,
};

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

#[test]
fn coverage_report_preserves_real_source_temporal_and_linkage_axes() {
    let report = bundle()
        .coverage_report(&RealDataCoverageQuery::default())
        .expect("coverage report compiles");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-coverage/0.1"
    );
    assert_eq!(report.total_record_count, 88);
    assert_eq!(report.matched_record_count, 88);
    assert_eq!(report.source_count, 5);
    assert_eq!(report.sources.len(), 5);
    assert!(report
        .sources
        .iter()
        .all(|source| source.declared_record_count == source.observed_record_count));
    assert_eq!(report.linkage.portal_study_count, 7);
    assert_eq!(report.linkage.portal_study_with_pmid_count, 6);
    assert_eq!(report.linkage.portal_study_without_pmid_count, 1);
    assert_eq!(report.linkage.portal_molecular_profile_count, 54);
    assert_eq!(report.linkage.explicit_profile_relationship_count, 54);
    assert_eq!(report.linkage.literature_article_count, 20);
    assert_eq!(report.linkage.literature_linked_to_portal_count, 6);
    assert_eq!(report.linkage.literature_without_portal_count, 14);
    assert_eq!(report.linkage.literature_abstract_count, 20);
    assert_eq!(report.linkage.literature_abstract_missing_count, 0);
    assert_eq!(report.linkage.literature_abstract_truncated_count, 0);
    assert!(report
        .time_axes
        .iter()
        .any(|axis| axis.axis == "clinical_trial_last_update" && axis.observed_count == 5));
    assert!(report
        .time_axes
        .iter()
        .any(|axis| axis.axis == "literature_publication_date"
            && axis.observed_count == 19
            && axis.missing_count == 1));
    assert_eq!(report.gaps.len(), 2);
    assert!(report
        .gaps
        .iter()
        .any(|gap| gap.code == "literature_without_portal"));
    assert_eq!(report.coverage_digest.len(), 64);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report
        .validate_integrity()
        .expect("coverage report should carry a valid envelope");
    report
        .validate_for_inputs(&bundle())
        .expect("coverage report should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.matched_record_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.record_kind = Some(RealDataRecordKind::ClinicalTrial);
    assert!(rebound.validate_for_inputs(&bundle()).is_err());
}

#[test]
fn coverage_query_facets_are_deterministic_and_keep_unknown_dates_outside_ranges() {
    let report = bundle()
        .coverage_report(&RealDataCoverageQuery {
            record_kind: Some(RealDataRecordKind::ClinicalTrial),
            from_year: Some(2020),
            to_year: Some(2025),
            ..RealDataCoverageQuery::default()
        })
        .expect("faceted coverage report compiles");
    assert_eq!(report.total_record_count, 88);
    assert_eq!(report.matched_record_count, 4);
    assert_eq!(report.record_kind_counts.len(), 1);
    assert_eq!(report.record_kind_counts[0].count, 4);
    let axis = report
        .time_axes
        .iter()
        .find(|axis| axis.axis == "clinical_trial_last_update")
        .expect("trial time axis is present");
    assert_eq!(axis.observed_count, 4);
    assert_eq!(axis.missing_count, 0);
    assert!(report
        .time_axes
        .iter()
        .find(|axis| axis.axis == "literature_publication_date")
        .is_some_and(|axis| axis.observed_count == 0 && axis.missing_count == 0));
}

#[test]
fn coverage_query_refuses_unknown_sources_and_reversed_years() {
    let missing_source = bundle().coverage_report(&RealDataCoverageQuery {
        source_id: Some("not-in-bundle".to_string()),
        ..RealDataCoverageQuery::default()
    });
    assert!(matches!(
        missing_source,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
    let reversed = bundle().coverage_report(&RealDataCoverageQuery {
        from_year: Some(2025),
        to_year: Some(2024),
        ..RealDataCoverageQuery::default()
    });
    assert!(matches!(
        reversed,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}
