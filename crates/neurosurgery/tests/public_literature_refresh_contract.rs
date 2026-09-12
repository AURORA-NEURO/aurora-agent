// Source touch permits Windows Application Control to relink this contract binary when needed.
use bioprism_neurosurgery::{
    PublicLiteratureBundle, PublicLiteratureRefreshAuditQuery, RealDataFreshnessQuery,
};

fn literature_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses")
}

#[test]
fn identical_real_snapshot_is_a_stable_noop_review() {
    let before = literature_bundle();
    let after = literature_bundle();
    let report = before
        .refresh_audit(&after, &PublicLiteratureRefreshAuditQuery::default())
        .expect("identical validated snapshots reconcile");

    assert_eq!(report.before_bundle_digest, report.after_bundle_digest);
    assert_eq!(report.diff.source_counts.added, 0);
    assert_eq!(report.diff.source_counts.removed, 0);
    assert_eq!(report.diff.source_counts.changed, 0);
    assert_eq!(report.diff.record_counts.added, 0);
    assert_eq!(report.diff.record_counts.removed, 0);
    assert_eq!(report.diff.record_counts.changed, 0);
    assert!(report.diff.source_identity_stable);
    assert!(report.diff.record_identity_stable);
    assert!(report.source_identity_stable);
    assert!(report.record_identity_stable);
    assert!(!report.structural_change_detected);
    assert!(!report.specialty_coverage_changed);
    assert!(!report.requires_refresh_review);
    assert!(report.review_reasons.is_empty());
    assert_eq!(report.matrix.specialty_count, 6);
    assert_eq!(report.matrix.non_empty_lane_count, 6);
    assert!(report.matrix.empty_lane_specialties.is_empty());
    assert_eq!(report.matrix.truncated_lane_count, 0);
    assert!(report.freshness.is_none());
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.audit_digest.len(), 64);
}

#[test]
fn metadata_drift_is_named_without_breaking_identity() {
    let before = literature_bundle();
    let mut after = literature_bundle();
    after.sources[0].retrieved_at = "2026-08-30T01:00:00Z".to_string();
    let report = before
        .refresh_audit(&after, &PublicLiteratureRefreshAuditQuery::default())
        .expect("metadata-only candidate reconciles");

    assert_eq!(report.diff.source_counts.changed, 1);
    assert_eq!(report.diff.record_counts.changed, 0);
    assert!(report.diff.source_identity_stable);
    assert!(report.diff.record_identity_stable);
    assert_eq!(report.diff.source_changes.len(), 1);
    assert_eq!(report.diff.source_changes[0].source_id, "pubmed_glioma");
    assert_eq!(
        report.diff.source_changes[0].changed_fields,
        vec!["retrieved_at"]
    );
    assert!(report.structural_change_detected);
    assert!(report.requires_refresh_review);
    assert_eq!(report.review_reasons[0].code, "structural_changes");
}

#[test]
fn explicit_freshness_clock_adds_review_obligation_without_imputing_quality() {
    let bundle = literature_bundle();
    let query = PublicLiteratureRefreshAuditQuery {
        freshness: Some(RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: None,
        }),
        ..Default::default()
    };
    let report = bundle
        .refresh_audit(&bundle, &query)
        .expect("explicit freshness clock reconciles");

    assert!(report.freshness.is_some());
    assert!(report.requires_refresh_review);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "freshness_review"));
    assert!(report.limitations.iter().any(|limitation| limitation
        .contains("age is not an evidence-quality score")
        || limitation.contains("freshness")));
}
// Source touch permits Windows Application Control to relink this contract binary when needed.
