//! Disagreement classification, adjudication, and appeals (31.15).

mod common;

use bioprism_oracle::{
    Disagreement, DisagreementSource, EvidenceTier, Independence, Judgement, OracleError, Plane,
    Position, Resolution, Settlement, SharedResource, ValidityWindow,
};
use common::{
    circular_judgement, confidence, judgement, manifest, manifest_versioned, now, oracle_ref, ts,
};

fn split_at(tier: EvidenceTier) -> Disagreement {
    Disagreement::between(
        tier,
        &[
            judgement("left", tier, Position::Supported, 1.0),
            judgement("right", tier, Position::Contradicted, 1.0),
        ],
    )
}

#[test]
fn a_disagreement_records_every_position_and_who_held_it() {
    let disagreement = split_at(EvidenceTier::Property);

    assert_eq!(disagreement.positions.len(), 2);
    assert_eq!(disagreement.contested().len(), 2);
    assert!(disagreement.resolution.is_open());
    assert_eq!(
        disagreement.positions[&Position::Supported][0],
        oracle_ref("left", 1)
    );
}

#[test]
fn an_adjudicator_at_the_disputed_tier_cannot_settle_the_dispute() {
    let disagreement = split_at(EvidenceTier::Property);
    let peer = judgement("third", EvidenceTier::Property, Position::Supported, 1.0);

    let error = disagreement
        .adjudicate(&peer, &now())
        .expect_err("a same-tier third opinion is a vote, not an adjudication");

    assert_eq!(
        error,
        OracleError::AdjudicationTierTooLow {
            dispute: EvidenceTier::Property,
            offered: EvidenceTier::Property,
        }
    );
}

#[test]
fn a_judge_cannot_settle_a_deterministic_dispute() {
    let disagreement = split_at(EvidenceTier::Deterministic);
    let judge = judgement("judge", EvidenceTier::Judge, Position::Supported, 1.0);

    assert!(disagreement.adjudicate(&judge, &now()).is_err());
}

#[test]
fn adjudication_retains_the_position_it_overturned() {
    let disagreement = Disagreement::between(
        EvidenceTier::Property,
        &[
            judgement("left", EvidenceTier::Property, Position::Supported, 1.0),
            judgement("right", EvidenceTier::Property, Position::Contradicted, 1.0),
        ],
    );
    let stronger = judgement("rerun", EvidenceTier::Execution, Position::Unresolved, 1.0);

    let error = disagreement
        .clone()
        .adjudicate(&stronger, &now())
        .expect_err("an abstention settles nothing");
    assert!(matches!(error, OracleError::AdjudicationAbstains { .. }));

    let decisive = judgement(
        "rerun",
        EvidenceTier::Execution,
        Position::Contradicted,
        1.0,
    );
    let settled = disagreement
        .adjudicate(&decisive, &now())
        .expect("a stronger, committed, admissible oracle settles it");

    match &settled.resolution {
        Resolution::Upheld { position, .. } => assert_eq!(*position, Position::Contradicted),
        other => panic!("expected the standing position to be upheld, got {other:?}"),
    }
    assert_eq!(
        settled.positions.len(),
        2,
        "the losing position and the oracles that held it are still on the record"
    );
    assert!(settled.positions.contains_key(&Position::Supported));
}

#[test]
fn an_adjudicator_taking_a_position_nobody_held_names_what_it_superseded() {
    let disagreement = Disagreement::between(
        EvidenceTier::Judge,
        &[
            judgement("judge_a", EvidenceTier::Judge, Position::Supported, 0.9),
            judgement("judge_b", EvidenceTier::Judge, Position::Unresolved, 0.5),
        ],
    );
    assert_eq!(disagreement.contested(), [Position::Supported].into());

    let settled = disagreement
        .adjudicate(
            &judgement(
                "schema",
                EvidenceTier::Deterministic,
                Position::Contradicted,
                1.0,
            ),
            &now(),
        )
        .expect("a deterministic oracle outranks any judge");

    match &settled.resolution {
        Resolution::Overturned {
            by,
            position,
            superseded,
            ..
        } => {
            assert_eq!(*position, Position::Contradicted);
            assert_eq!(by, &oracle_ref("schema", 1));
            assert_eq!(*superseded, [Position::Supported].into());
        }
        other => panic!("expected an overturned resolution, got {other:?}"),
    }
    assert!(
        settled.positions.contains_key(&Position::Supported),
        "the overturned position and its holders are still listed"
    );
}

#[test]
fn an_expired_oracle_cannot_settle_a_disagreement() {
    let expired = bioprism_oracle::OracleManifest::new(
        oracle_ref("stale", 1),
        EvidenceTier::Deterministic,
        [Plane::Artifact],
        [],
        ValidityWindow::new(ts("2020-01-01T00:00:00Z"), Some(ts("2021-01-01T00:00:00Z")))
            .expect("the window opens before it closes"),
    )
    .expect("fixture manifest is well formed");
    let adjudicator =
        Judgement::from_manifest(&expired, &now(), Position::Contradicted, confidence(1.0));

    let error = split_at(EvidenceTier::Property)
        .adjudicate(&adjudicator, &now())
        .expect_err("31.16 inadmissibility applies in adjudication too");

    assert!(matches!(error, OracleError::InadmissibleAdjudicator { .. }));
}

#[test]
fn a_deterministic_disagreement_routes_to_artifact_repair_not_to_a_judge() {
    let disagreement = split_at(EvidenceTier::Deterministic);

    assert!(disagreement
        .would_be_settled_by
        .iter()
        .any(|settlement| matches!(settlement, Settlement::ArtifactRepair { .. })));
    assert!(
        !disagreement
            .would_be_settled_by
            .iter()
            .any(|settlement| matches!(settlement, Settlement::HigherTierOracle { .. })),
        "nothing outranks the top rung, so no stronger oracle can be requested"
    );
}

#[test]
fn a_weaker_disagreement_routes_upward_to_a_strictly_stronger_rung() {
    let disagreement = split_at(EvidenceTier::Judge);

    assert!(disagreement
        .would_be_settled_by
        .contains(&Settlement::HigherTierOracle {
            at_least: EvidenceTier::Statistical,
        }));
}

#[test]
fn one_oracle_disagreeing_with_itself_across_versions_is_classified_as_a_version_mismatch() {
    let disagreement = Disagreement::between(
        EvidenceTier::Deterministic,
        &[
            Judgement::from_manifest(
                &manifest_versioned("schema", 1, EvidenceTier::Deterministic, Plane::Artifact),
                &now(),
                Position::Supported,
                confidence(1.0),
            ),
            Judgement::from_manifest(
                &manifest_versioned("schema", 2, EvidenceTier::Deterministic, Plane::Artifact),
                &now(),
                Position::Contradicted,
                confidence(1.0),
            ),
        ],
    );

    match &disagreement.source {
        DisagreementSource::VersionMismatch { id, versions } => {
            assert_eq!(id, "test:schema");
            assert_eq!(versions.len(), 2);
        }
        other => panic!("expected a version mismatch, got {other:?}"),
    }
    assert!(disagreement
        .would_be_settled_by
        .iter()
        .any(|settlement| matches!(settlement, Settlement::VersionAlignment { .. })));
}

#[test]
fn a_disagreement_involving_a_circular_oracle_is_classified_as_an_independence_violation() {
    let disagreement = Disagreement::between(
        EvidenceTier::Property,
        &[
            judgement("clean", EvidenceTier::Property, Position::Supported, 1.0),
            circular_judgement("echo", EvidenceTier::Execution, Position::Contradicted, 1.0),
        ],
    );

    match &disagreement.source {
        DisagreementSource::IndependenceViolation { circular } => {
            assert_eq!(circular.len(), 1);
        }
        other => panic!("expected an independence violation, got {other:?}"),
    }
    assert!(disagreement
        .would_be_settled_by
        .iter()
        .any(|settlement| matches!(settlement, Settlement::IndependentReview { .. })));
}

#[test]
fn two_independent_same_version_oracles_that_differ_are_a_genuine_ambiguity() {
    let disagreement = split_at(EvidenceTier::Property);
    assert_eq!(disagreement.source, DisagreementSource::GenuineAmbiguity);
}

#[test]
fn a_disagreement_may_be_closed_as_unresolvable_without_being_erased() {
    let closed = split_at(EvidenceTier::Deterministic)
        .declare_unresolvable("the source specimen is no longer available");

    match &closed.resolution {
        Resolution::Unresolvable { reason } => assert!(reason.contains("specimen")),
        other => panic!("expected an unresolvable resolution, got {other:?}"),
    }
    assert_eq!(closed.positions.len(), 2);
}

#[test]
fn a_circular_manifest_reports_its_own_circularity() {
    let echo =
        manifest("echo", EvidenceTier::Deterministic).with_independence(Independence::sharing([
            SharedResource::TrainingData,
            SharedResource::Labels,
        ]));

    assert!(echo.independence.is_circular());
    assert_eq!(echo.effective_tier(), EvidenceTier::Execution);
    assert_eq!(echo.declared_tier, EvidenceTier::Deterministic);
}
