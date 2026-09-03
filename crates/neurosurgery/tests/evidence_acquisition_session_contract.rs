use bioprism_neurosurgery::{
    EvidenceAcquisitionQuery, EvidenceAcquisitionSessionStatus, NeurosurgicalAgent,
    PublicLiteratureBundle, RealGliomaBundle,
};

fn request() -> bioprism_neurosurgery::CaseRequest {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request fixture parses")
}

fn bundles() -> (RealGliomaBundle, PublicLiteratureBundle) {
    (
        serde_json::from_str(include_str!(
            "../../../data/neurosurgery/glioma_public_snapshot.json"
        ))
        .expect("real glioma snapshot parses"),
        serde_json::from_str(include_str!(
            "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
        ))
        .expect("public literature snapshot parses"),
    )
}

#[test]
fn acquisition_session_replays_every_real_snapshot_step_and_finishes_at_review() {
    let (real_data, literature) = bundles();
    let agent = NeurosurgicalAgent::default();
    let query = EvidenceAcquisitionQuery {
        max_steps: 4,
        max_references_per_step: 1,
        freshness: None,
    };
    let started = agent
        .evidence_acquisition_start(&request(), Some(&real_data), Some(&literature), &query)
        .expect("acquisition session starts");
    assert_eq!(
        started.session.status,
        EvidenceAcquisitionSessionStatus::Planned
    );
    assert_eq!(started.session.next_sequence, 1);
    assert_eq!(started.plan.steps.len(), 4);
    let first = agent
        .evidence_acquisition_advance(
            &started.session,
            &request(),
            Some(&real_data),
            Some(&literature),
            &query,
            2,
        )
        .expect("first replay wave succeeds");
    assert_eq!(first.steps_executed, 2);
    assert!(!first.complete);
    assert_eq!(first.session.events.len(), 2);
    assert_eq!(first.session.next_sequence, 3);
    let second = agent
        .evidence_acquisition_advance(
            &first.session,
            &request(),
            Some(&real_data),
            Some(&literature),
            &query,
            2,
        )
        .expect("second replay wave succeeds");
    assert_eq!(second.steps_executed, 2);
    assert!(second.complete);
    assert_eq!(
        second.session.status,
        EvidenceAcquisitionSessionStatus::AwaitingHumanReview
    );
    let finished = agent
        .evidence_acquisition_finish(
            &second.session,
            &request(),
            Some(&real_data),
            Some(&literature),
            &query,
        )
        .expect("completed acquisition finishes");
    assert_eq!(finished.steps_executed, 4);
    assert_eq!(finished.event_count, 4);
    assert!(finished.human_review_required);
    assert_eq!(finished.provider, "none");
    assert!(!finished.network);
}

#[test]
fn acquisition_session_rejects_tampering_and_missing_planes() {
    let (real_data, literature) = bundles();
    let agent = NeurosurgicalAgent::default();
    let query = EvidenceAcquisitionQuery {
        max_steps: 2,
        max_references_per_step: 1,
        freshness: None,
    };
    let started = agent
        .evidence_acquisition_start(&request(), Some(&real_data), Some(&literature), &query)
        .expect("acquisition session starts");
    let mut tampered = started.session.clone();
    tampered.event_chain_digest = "0".repeat(64);
    let error = agent
        .evidence_acquisition_advance(
            &tampered,
            &request(),
            Some(&real_data),
            Some(&literature),
            &query,
            1,
        )
        .expect_err("tampered checkpoint is refused");
    assert!(error.to_string().contains("acquisition event-chain digest"));

    let missing = agent
        .evidence_acquisition_start(&request(), None, Some(&literature), &query)
        .expect("literature-only session starts with an explicit missing plane");
    assert_eq!(
        missing.session.status,
        EvidenceAcquisitionSessionStatus::NeedsEvidence
    );
    assert!(!missing.plan.steps.is_empty());
    let error = agent
        .evidence_acquisition_finish(
            &missing.session,
            &request(),
            None,
            Some(&literature),
            &query,
        )
        .expect_err("missing real plane cannot be finalized");
    assert!(error.to_string().contains("required source planes"));
}
