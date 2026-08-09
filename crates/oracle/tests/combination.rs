//! Set-valued combination (40.21) and the refusal to vote or average.

mod common;

use std::collections::BTreeSet;

use bioprism_oracle::{EvidenceTier, MeshPolicy, OverrideRule, Plane, Position, VerdictBasis};
use bioprism_section::OracleStatus;
use common::{judgement, now};

fn combine(judgements: Vec<bioprism_oracle::Judgement>) -> bioprism_oracle::CombinedVerdict {
    MeshPolicy::default().combine("result-bundle-1", &now(), judgements)
}

#[test]
fn a_judge_may_not_overturn_a_deterministic_verdict() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            1.0,
        ),
        judgement("judge", EvidenceTier::Judge, Position::Supported, 0.99),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Invalid);
    assert_eq!(verdict.acceptable, BTreeSet::from([Position::Contradicted]));
    assert_eq!(verdict.deciding_tier(), Some(EvidenceTier::Deterministic));

    let suppressed = verdict
        .suppressed
        .first()
        .expect("the judge's attempt is on the record");
    assert_eq!(suppressed.attempted_position, Position::Supported);
    assert_eq!(suppressed.rule, OverrideRule::NondeterministicOverGrounded);
}

#[test]
fn a_confident_judge_still_cannot_outrank_a_deterministic_oracle() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            0.5,
        ),
        judgement("judge", EvidenceTier::Judge, Position::Supported, 1.0),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Invalid);
    let suppressed = &verdict.suppressed[0];
    assert_eq!(
        suppressed.attempted_confidence.value(),
        1.0,
        "the refused confidence is recorded, so the refusal can be audited rather than assumed"
    );
}

#[test]
fn three_agreeing_judges_do_not_outweigh_one_deterministic_contradiction() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            1.0,
        ),
        judgement("judge_a", EvidenceTier::Judge, Position::Supported, 0.9),
        judgement("judge_b", EvidenceTier::Judge, Position::Supported, 0.9),
        judgement("judge_c", EvidenceTier::Judge, Position::Supported, 0.9),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Invalid);
    assert_eq!(verdict.suppressed.len(), 3);
    assert_eq!(verdict.contributing.len(), 1);
}

#[test]
fn same_tier_disagreement_yields_underdetermined_with_both_positions_retained() {
    let verdict = combine(vec![
        judgement(
            "schema_a",
            EvidenceTier::Deterministic,
            Position::Supported,
            1.0,
        ),
        judgement(
            "schema_b",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            1.0,
        ),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Underdetermined);
    assert_eq!(
        verdict.acceptable,
        BTreeSet::from([Position::Supported, Position::Contradicted]),
        "both positions survive; there is no midpoint between them"
    );

    let disagreement = verdict
        .disagreements
        .first()
        .expect("a same-tier split produces a disagreement record");
    assert_eq!(disagreement.tier, EvidenceTier::Deterministic);
    assert_eq!(disagreement.positions.len(), 2);
    assert!(disagreement.resolution.is_open());
}

#[test]
fn combination_reports_the_observed_confidence_range_and_never_a_mean() {
    let verdict = combine(vec![
        judgement("judge_a", EvidenceTier::Judge, Position::Supported, 0.4),
        judgement("judge_b", EvidenceTier::Judge, Position::Supported, 1.0),
    ]);

    let envelope = verdict.confidence.expect("two judgements decided");
    assert_eq!(envelope.low.value(), 0.4);
    assert_eq!(envelope.high.value(), 1.0);
    assert!(!envelope.is_point());
}

#[test]
fn an_abstention_does_not_manufacture_disagreement_with_a_contradiction() {
    let verdict = combine(vec![
        judgement(
            "schema_a",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            1.0,
        ),
        judgement(
            "schema_b",
            EvidenceTier::Deterministic,
            Position::Unresolved,
            1.0,
        ),
        judgement(
            "schema_c",
            EvidenceTier::Deterministic,
            Position::NotEvaluable,
            1.0,
        ),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Invalid);
    assert_eq!(verdict.acceptable, BTreeSet::from([Position::Contradicted]));
    assert!(
        verdict.disagreements.is_empty(),
        "abstaining is not a way to soften a hard failure into underdetermination"
    );
    assert_eq!(
        verdict.contributing.len(),
        2,
        "the oracle that applied and could not decide still participated at the deciding tier"
    );
    assert_eq!(verdict.admissible().count(), 3);
}

#[test]
fn an_out_of_scope_deterministic_oracle_does_not_veto_a_judge_that_does_apply() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::NotEvaluable,
            1.0,
        ),
        judgement("judge", EvidenceTier::Judge, Position::Supported, 0.9),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Valid);
    assert_eq!(verdict.deciding_tier(), Some(EvidenceTier::Judge));
    assert_eq!(
        verdict.withheld.len(),
        1,
        "the out-of-scope oracle stays on the evidence graph without setting the deciding tier"
    );
}

#[test]
fn an_unresolved_deterministic_oracle_does_block_a_weaker_tier_from_deciding() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Unresolved,
            1.0,
        ),
        judgement("judge", EvidenceTier::Judge, Position::Supported, 0.9),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Underdetermined);
    assert_eq!(verdict.acceptable, BTreeSet::from([Position::Unresolved]));
    assert_eq!(
        verdict.suppressed.len(),
        1,
        "an oracle that applies and cannot decide holds its rung; the judge may not fill the gap"
    );
}

#[test]
fn a_mesh_in_which_every_oracle_is_out_of_scope_says_so_rather_than_passing() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::NotEvaluable,
            1.0,
        ),
        judgement("judge", EvidenceTier::Judge, Position::NotEvaluable, 1.0),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Underdetermined);
    assert_eq!(verdict.basis, VerdictBasis::NoApplicableOracle);
    assert_eq!(verdict.acceptable, BTreeSet::from([Position::NotEvaluable]));
}

#[test]
fn a_lower_tier_agreement_is_withheld_evidence_and_not_a_second_vote() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Supported,
            1.0,
        ),
        judgement("judge", EvidenceTier::Judge, Position::Supported, 0.6),
    ]);

    assert_eq!(verdict.status(), OracleStatus::Valid);
    assert_eq!(verdict.contributing.len(), 1);
    assert_eq!(verdict.withheld.len(), 1);
    assert!(verdict.suppressed.is_empty());
    let envelope = verdict
        .confidence
        .expect("the deciding tier had a judgement");
    assert!(
        envelope.is_point() && envelope.low.value() == 1.0,
        "the agreeing judge's 0.6 does not dilute the deterministic oracle's certainty"
    );
}

#[test]
fn a_policy_floor_refuses_to_decide_on_judge_evidence_alone() {
    let verdict = MeshPolicy::grounded_only().combine(
        "result-bundle-1",
        &now(),
        vec![judgement(
            "judge",
            EvidenceTier::Judge,
            Position::Contradicted,
            0.95,
        )],
    );

    assert_eq!(verdict.status(), OracleStatus::Underdetermined);
    assert_eq!(
        verdict.basis,
        VerdictBasis::BelowPolicyFloor {
            best: EvidenceTier::Judge,
            required: EvidenceTier::Execution,
        }
    );
    assert_eq!(
        verdict.withheld.len(),
        1,
        "the opinion is kept, not counted"
    );
}

#[test]
fn a_verdict_from_artifact_and_analytical_oracles_does_not_establish_biological_validity() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Supported,
            1.0,
        ),
        judgement("rerun", EvidenceTier::Execution, Position::Supported, 1.0),
    ]);

    let established = verdict.establishes();
    assert!(established.contains(&Plane::Artifact));
    assert!(
        established.contains(&Plane::Analytical),
        "planes are orthogonal to the ladder; the execution oracle covered ground the schema \
         oracle did not"
    );
    assert!(!established.contains(&Plane::Biological));
    assert!(verdict.does_not_establish().contains(&Plane::Biological));
}

#[test]
fn a_contradicted_verdict_establishes_nothing_at_all() {
    let verdict = combine(vec![
        judgement(
            "schema",
            EvidenceTier::Deterministic,
            Position::Contradicted,
            1.0,
        ),
        judgement("rerun", EvidenceTier::Execution, Position::Supported, 1.0),
    ]);

    assert!(
        verdict.establishes().is_empty(),
        "finding a defect proves the artifact wrong, not any part of it right"
    );
}

#[test]
fn a_verdict_resting_only_on_judges_says_so() {
    let verdict = combine(vec![
        judgement("judge_a", EvidenceTier::Judge, Position::Supported, 0.8),
        judgement("judge_b", EvidenceTier::Judge, Position::Supported, 0.7),
    ]);

    assert!(verdict.is_judge_only());
    assert_eq!(verdict.status(), OracleStatus::Valid);
}
