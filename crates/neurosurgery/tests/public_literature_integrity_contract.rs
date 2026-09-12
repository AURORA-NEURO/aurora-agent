use bioprism_neurosurgery::{
    PublicLiteratureBundle, PublicLiteratureIntegrityAuditQuery, Specialty,
};

fn literature_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses")
}

#[test]
fn checked_in_snapshot_reports_real_missingness_without_quality_inference() {
    let report = literature_bundle()
        .integrity_audit(&PublicLiteratureIntegrityAuditQuery::default())
        .expect("validated snapshot supports integrity audit");

    assert_eq!(report.counts.selected_record_count, 145);
    assert_eq!(report.counts.selected_source_count, 6);
    assert_eq!(report.counts.unique_pmid_count, 145);
    assert_eq!(report.counts.doi_count, 145);
    assert_eq!(report.counts.missing_doi_count, 0);
    assert_eq!(report.counts.abstract_count, 138);
    assert_eq!(report.counts.missing_abstract_count, 7);
    assert_eq!(report.counts.abstract_truncated_count, 0);
    assert_eq!(report.counts.empty_publication_type_count, 0);
    assert_eq!(report.counts.empty_mesh_term_count, 84);
    assert_eq!(report.counts.duplicate_doi_group_count, 0);
    assert_eq!(report.counts.cross_specialty_duplicate_doi_group_count, 0);
    assert_eq!(report.issues.len(), 91);
    assert!(report.requires_integrity_review);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "missing_abstract" && reason.count == 7));
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "missing_mesh_terms" && reason.count == 84));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.audit_digest.len(), 64);
    report
        .validate_integrity()
        .expect("integrity audit should carry a valid envelope");
    report
        .validate_for_inputs(&literature_bundle())
        .expect("integrity audit should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.counts.selected_record_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.specialties = Some(vec![Specialty::Glioma]);
    assert!(rebound.validate_for_inputs(&literature_bundle()).is_err());
}

#[test]
fn specialty_scope_and_issue_projection_are_bounded() {
    let query = PublicLiteratureIntegrityAuditQuery {
        specialties: Some(vec![Specialty::Glioma]),
        max_issues: 2,
    };
    let report = literature_bundle()
        .integrity_audit(&query)
        .expect("scoped integrity audit succeeds");

    assert_eq!(report.counts.selected_record_count, 25);
    assert_eq!(report.counts.selected_source_count, 1);
    assert_eq!(report.counts.missing_abstract_count, 0);
    assert_eq!(report.counts.empty_mesh_term_count, 3);
    assert_eq!(report.issues.len(), 2);
    assert_eq!(report.omitted_issue_count, 1);
    assert!(report.truncated);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "projection_truncated"));
}

#[test]
fn invalid_integrity_queries_fail_closed() {
    let mut query = PublicLiteratureIntegrityAuditQuery {
        max_issues: 0,
        ..Default::default()
    };
    assert!(literature_bundle().integrity_audit(&query).is_err());

    query = PublicLiteratureIntegrityAuditQuery {
        specialties: Some(vec![Specialty::Glioma, Specialty::Glioma]),
        max_issues: 128,
    };
    assert!(literature_bundle().integrity_audit(&query).is_err());
}
