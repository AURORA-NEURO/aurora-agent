use bioprism_neurosurgery::{
    NeurosurgeryError, RealDataQuery, RealDataRecordKind, RealDataTrialLandscapeQuery,
    RealGliomaBundle,
};

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("checked-in public snapshot parses")
}

#[test]
fn landscape_aggregates_real_registry_metadata_and_replays() {
    let report = bundle()
        .trial_landscape(&RealDataTrialLandscapeQuery::default())
        .expect("default trial landscape is valid");
    assert_eq!(report.total_matching_trials, 5);
    assert_eq!(report.returned_trial_count, 5);
    assert_eq!(report.phase_annotated_trial_count, 5);
    assert!(!report.truncated);
    assert_eq!(
        report
            .status_counts
            .iter()
            .map(|bucket| (bucket.label.as_str(), bucket.count))
            .collect::<Vec<_>>(),
        vec![
            ("COMPLETED", 2),
            ("RECRUITING", 1),
            ("SUSPENDED", 1),
            ("TERMINATED", 1),
        ]
    );
    assert_eq!(
        report
            .phase_counts
            .iter()
            .map(|bucket| (bucket.label.as_str(), bucket.count))
            .collect::<Vec<_>>(),
        vec![("NA", 1), ("PHASE1", 3), ("PHASE2", 4)]
    );
    assert_eq!(report.study_type_counts[0].label, "INTERVENTIONAL");
    assert_eq!(report.study_type_counts[0].count, 5);
    assert_eq!(report.distinct_intervention_count, 7);
    assert_eq!(report.omitted_intervention_count, 0);
    assert_eq!(report.earliest_last_update.as_deref(), Some("2013-06-20"));
    assert_eq!(report.latest_last_update.as_deref(), Some("2025-03-13"));
    assert!(report.review_reasons.is_empty());
    report
        .validate_for_inputs(&bundle())
        .expect("landscape is digest-bound to the exact snapshot");
}

#[test]
fn landscape_keeps_query_truncation_and_metadata_gaps_explicit() {
    let report = bundle()
        .trial_landscape(&RealDataTrialLandscapeQuery {
            query: RealDataQuery {
                trial_phase: Some("phase2".to_string()),
                limit: 2,
                ..RealDataQuery::default()
            },
            max_interventions: 2,
        })
        .expect("bounded trial landscape is valid");
    assert_eq!(report.total_matching_trials, 4);
    assert_eq!(report.returned_trial_count, 2);
    assert_eq!(report.omitted_trial_count, 2);
    assert!(report.truncated);
    assert_eq!(report.omitted_intervention_count, 1);
    assert!(report.intervention_truncated);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "trial_rows_truncated"));
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "interventions_truncated"));
}

#[test]
fn landscape_rejects_non_trial_scope() {
    let query = RealDataTrialLandscapeQuery {
        query: RealDataQuery {
            record_kind: Some(RealDataRecordKind::LiteratureArticle),
            ..RealDataQuery::default()
        },
        ..RealDataTrialLandscapeQuery::default()
    };
    assert!(matches!(
        bundle().trial_landscape(&query),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}
