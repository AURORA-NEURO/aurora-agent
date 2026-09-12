use bioprism_neurosurgery::{
    NeurosurgeryError, PublicLiteratureBundle, RealDataFreshnessQuery, RealDataFreshnessState,
    RealDataFreshnessStatus, RealGliomaBundle,
};

fn glioma_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

fn literature_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses")
}

#[test]
fn freshness_audit_is_digest_bound_and_keeps_current_and_stale_sources_distinct() {
    let report = glioma_bundle()
        .freshness_report(&RealDataFreshnessQuery {
            as_of: "2026-08-31T00:00:00Z".to_string(),
            max_age_days: 1,
            source_id: None,
        })
        .expect("freshness report compiles");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-freshness/0.1"
    );
    assert_eq!(report.status, RealDataFreshnessStatus::Current);
    assert_eq!(report.source_count, 5);
    assert_eq!(report.current_source_count, 5);
    assert_eq!(report.stale_source_count, 0);
    assert_eq!(report.future_dated_source_count, 0);
    assert_eq!(report.freshness_digest.len(), 64);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report
        .validate_integrity()
        .expect("freshness report should carry a valid envelope");
    report
        .validate_for_real_inputs(&glioma_bundle())
        .expect("freshness report should replay against the exact glioma snapshot");
    let mut tampered = report.clone();
    tampered.current_source_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.max_age_days = 2;
    assert!(rebound.validate_for_real_inputs(&glioma_bundle()).is_err());

    let stale = glioma_bundle()
        .freshness_report(&RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: Some("pubmed_glioblastoma".to_string()),
        })
        .expect("single-source freshness report compiles");
    assert_eq!(stale.status, RealDataFreshnessStatus::Stale);
    assert_eq!(stale.source_count, 1);
    assert_eq!(stale.current_source_count, 0);
    assert_eq!(stale.stale_source_count, 1);
    assert_eq!(stale.sources[0].state, RealDataFreshnessState::Stale);
    assert!(stale.sources[0].age_days.is_some_and(|days| days > 30));
}

#[test]
fn historical_as_of_keeps_future_metadata_explicit_and_public_literature_uses_same_contract() {
    let future = glioma_bundle()
        .freshness_report(&RealDataFreshnessQuery {
            as_of: "2026-08-29T00:00:00Z".to_string(),
            max_age_days: 365,
            source_id: None,
        })
        .expect("historical freshness report compiles");
    assert_eq!(future.status, RealDataFreshnessStatus::RequiresReview);
    assert_eq!(future.future_dated_source_count, 5);
    assert!(future
        .sources
        .iter()
        .all(|source| source.state == RealDataFreshnessState::FutureDated
            && source.age_days.is_none()));

    let literature = literature_bundle()
        .freshness_report(&RealDataFreshnessQuery {
            as_of: "2026-08-31T00:00:00Z".to_string(),
            max_age_days: 1,
            source_id: None,
        })
        .expect("public literature freshness report compiles");
    assert_eq!(literature.status, RealDataFreshnessStatus::Current);
    assert_eq!(literature.source_count, 6);
    literature
        .validate_integrity()
        .expect("public freshness report should carry a valid envelope");
    literature
        .validate_for_public_inputs(&literature_bundle())
        .expect("public freshness report should replay against the exact snapshot");
}

#[test]
fn freshness_rejects_invalid_as_of_and_unknown_source_without_defaults() {
    let invalid = glioma_bundle().freshness_report(&RealDataFreshnessQuery {
        as_of: "2026-02-30T00:00:00Z".to_string(),
        max_age_days: 365,
        source_id: None,
    });
    assert!(matches!(
        invalid,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let unknown = glioma_bundle().freshness_report(&RealDataFreshnessQuery {
        as_of: "2026-08-31T00:00:00Z".to_string(),
        max_age_days: 365,
        source_id: Some("not-in-bundle".to_string()),
    });
    assert!(matches!(
        unknown,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let too_wide = glioma_bundle().freshness_report(&RealDataFreshnessQuery {
        as_of: "2026-08-31T00:00:00Z".to_string(),
        max_age_days: 3_651,
        source_id: None,
    });
    assert!(matches!(too_wide, Err(NeurosurgeryError::TooMany { .. })));
}
