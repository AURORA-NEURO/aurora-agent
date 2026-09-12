use bioprism_neurosurgery::{PublicLiteratureBundle, PublicLiteraturePortfolioQuery, Specialty};

fn snapshot() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public-literature snapshot parses")
}

#[test]
fn portfolio_runs_independent_real_queries_and_review_queues_per_lane() {
    let report = snapshot()
        .literature_portfolio(&PublicLiteraturePortfolioQuery {
            specialties: Some(vec![Specialty::Glioma, Specialty::ChiariMalformation]),
            max_hits_per_lane: 2,
            max_review_items_per_lane: 2,
            max_issues_per_lane: 8,
            ..Default::default()
        })
        .expect("portfolio validates against real snapshot");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-public-literature-portfolio/0.1"
    );
    assert_eq!(report.specialty_count, 2);
    assert_eq!(report.lanes.len(), 2);
    assert_eq!(report.lanes[0].specialty, Specialty::Glioma);
    assert_eq!(report.lanes[1].specialty, Specialty::ChiariMalformation);
    assert_eq!(report.total_match_count, 47);
    assert_eq!(report.total_returned_count, 4);
    assert_eq!(report.lanes[0].query_result.returned_matches, 2);
    assert_eq!(report.lanes[0].review_queue.returned_item_count, 2);
    assert_eq!(report.lanes[1].review_queue.returned_item_count, 2);
    assert!(report.lanes.iter().all(|lane| {
        lane.workbench.profile.specialty == lane.specialty
            && lane.query_result.query.specialty == Some(lane.specialty)
    }));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.portfolio_digest.len(), 64);
    report
        .validate_integrity()
        .expect("portfolio should carry a valid envelope");
    report
        .validate_for_inputs(&snapshot())
        .expect("portfolio should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.total_returned_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.text = Some("not-present".to_string());
    assert!(rebound.validate_for_inputs(&snapshot()).is_err());
}

#[test]
fn default_portfolio_covers_all_six_real_snapshot_lanes() {
    let report = snapshot()
        .literature_portfolio(&PublicLiteraturePortfolioQuery {
            max_hits_per_lane: 1,
            max_review_items_per_lane: 1,
            max_issues_per_lane: 1,
            ..Default::default()
        })
        .expect("default portfolio validates against real snapshot");
    assert_eq!(report.specialty_count, 6);
    assert_eq!(report.non_empty_lane_count, 6);
    assert_eq!(report.total_match_count, 145);
    assert_eq!(report.total_returned_count, 6);
    assert_eq!(report.total_review_issue_count, 91);
    assert_eq!(report.lanes.len(), 6);
    assert!(report.empty_lane_specialties.is_empty());
    assert!(report.lanes.iter().all(|lane| {
        lane.workbench.record_count > 0
            && lane.query_result.returned_matches == 1
            && lane.review_queue.returned_item_count == 1
    }));
}
