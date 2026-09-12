use bioprism_neurosurgery::{
    NeurosurgicalAgent, PublicLiteratureBundle, PublicLiteratureReviewQueueQuery,
    PublicLiteratureReviewStatus, Specialty,
};

fn snapshot() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public-literature snapshot parses")
}

#[test]
fn queue_projects_real_lane_missingness_into_bounded_tasks() {
    let report = NeurosurgicalAgent::default()
        .public_literature_review_queue(
            &snapshot(),
            &PublicLiteratureReviewQueueQuery {
                specialties: Some(vec![Specialty::Glioma]),
                max_items: 2,
            },
        )
        .expect("review queue validates the real snapshot");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-public-literature-review-queue/0.1"
    );
    assert_eq!(report.bundle_digest.len(), 64);
    assert_eq!(report.integrity_audit_digest.len(), 64);
    assert_eq!(report.candidate_item_count, 3);
    assert_eq!(report.returned_item_count, 2);
    assert_eq!(report.omitted_item_count, 1);
    assert_eq!(report.omitted_integrity_issue_count, 0);
    assert!(report.truncated);
    assert!(report.items.iter().all(|item| {
        item.specialty == Specialty::Glioma
            && item.source_id == "pubmed_glioma"
            && item
                .record_uri
                .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
            && item.status == PublicLiteratureReviewStatus::NeedsHumanReview
            && !item.reviewer_roles.is_empty()
    }));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.queue_digest.len(), 64);
    report
        .validate_integrity()
        .expect("review queue should carry a valid envelope");
    report
        .validate_for_inputs(&snapshot())
        .expect("review queue should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.returned_item_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.specialties = Some(vec![Specialty::ChiariMalformation]);
    assert!(rebound.validate_for_inputs(&snapshot()).is_err());
}

#[test]
fn queue_all_lanes_preserves_integrity_issue_count_and_rejects_bad_bounds() {
    let report = snapshot()
        .review_queue(&PublicLiteratureReviewQueueQuery::default())
        .expect("all-lane queue validates");
    assert_eq!(report.candidate_item_count, 91);
    assert_eq!(report.returned_item_count, 64);
    assert_eq!(report.omitted_item_count, 27);
    assert!(report.truncated);
    assert_eq!(report.omitted_integrity_issue_count, 0);

    let error = snapshot().review_queue(&PublicLiteratureReviewQueueQuery {
        specialties: None,
        max_items: 0,
    });
    assert!(error.is_err());
}
