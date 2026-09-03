//! Metamorphic response (26.12), sealed prospective evaluation (26.16) and matched designs (26.18).

use std::collections::BTreeMap;

use bioprism_bioevalx::design::{Arm, FactorialDesign};
use bioprism_bioevalx::error::{DesignError, MetamorphicError, RevealError};
use bioprism_bioevalx::metamorphic::{
    verdict, Direction, Family, Relation, Response, Suite, Trial, TrialVerdict,
};
use bioprism_bioevalx::reveal::{Commitment, Outcome, Registration};
use bioprism_evalengine::{attribute, Attribution, Conclusion, ScoreTier};
use bioprism_scope::Timestamp;
use serde_json::json;

fn at(rfc3339: &str) -> Timestamp {
    Timestamp::parse(rfc3339).expect("fixture timestamp parses")
}

fn trial(id: &str, relation: Relation, response: Response) -> Trial {
    Trial {
        id: id.into(),
        relation,
        response,
    }
}

#[test]
fn a_shortcut_and_a_blind_spot_are_different_findings() {
    assert_eq!(
        verdict(
            Relation::Invariant,
            Response::Moved {
                direction: Direction::Increase
            }
        ),
        TrialVerdict::FalseSensitivity
    );
    assert_eq!(
        verdict(
            Relation::DirectionalChange {
                expected: Direction::Decrease
            },
            Response::Unchanged
        ),
        TrialVerdict::FalseInvariance
    );
}

#[test]
fn a_family_report_never_offers_a_combined_failure_number() {
    let mut family = Family::declaring("rename", Relation::Invariant);
    family
        .record(trial("t1", Relation::Invariant, Response::Unchanged))
        .expect("distinct");
    family
        .record(trial(
            "t2",
            Relation::Invariant,
            Response::Moved {
                direction: Direction::Increase,
            },
        ))
        .expect("distinct");

    let report = family.report().expect("family is non-empty");

    assert_eq!(report.consistent, 1);
    assert_eq!(report.false_sensitivity, 1);
    assert_eq!(report.false_invariance, 0);
    assert_eq!(report.witnesses, vec!["t2".to_string()]);
}

#[test]
fn an_incomparable_trial_counts_toward_neither_consistency_nor_violation() {
    let mut family = Family::declaring("rename", Relation::Invariant);
    family
        .record(trial("t1", Relation::Invariant, Response::Unchanged))
        .expect("distinct");
    family
        .record(trial("t2", Relation::Invariant, Response::Incomparable))
        .expect("distinct");

    let report = family.report().expect("family is non-empty");

    assert_eq!(report.undetermined, 1);
    assert_eq!(report.evidential(), 1);
    assert_eq!(report.consistency(), Some(1.0));
}

#[test]
fn a_family_in_which_nothing_was_comparable_reports_no_consistency_rather_than_zero() {
    let mut family = Family::declaring("rename", Relation::Invariant);
    family
        .record(trial("t1", Relation::Invariant, Response::Incomparable))
        .expect("distinct");

    let report = family.report().expect("family is non-empty");

    assert_eq!(report.consistency(), None);
}

#[test]
fn a_family_cannot_mix_relations() {
    let mut family = Family::declaring("rename", Relation::Invariant);

    let outcome = family.record(trial(
        "t1",
        Relation::DirectionalChange {
            expected: Direction::Increase,
        },
        Response::Unchanged,
    ));

    assert!(matches!(
        outcome,
        Err(MetamorphicError::RelationMismatch { .. })
    ));
}

#[test]
fn a_suite_of_only_invariance_families_cannot_detect_a_blind_spot_and_says_so() {
    let mut suite = Suite::new();
    for id in ["rename", "reorder"] {
        let mut family = Family::declaring(id, Relation::Invariant);
        family
            .record(trial("t", Relation::Invariant, Response::Unchanged))
            .expect("distinct");
        suite.add(family).expect("distinct valid family");
    }

    let covered = suite.relations_covered();

    assert_eq!(covered.len(), 1);
    assert!(covered.contains(&Relation::Invariant));
    assert!(suite.failing().expect("families are non-empty").is_empty());
}

#[test]
fn an_empty_family_refuses_rather_than_reporting_perfect_consistency() {
    let family = Family::declaring("rename", Relation::Invariant);

    assert!(matches!(
        family.report(),
        Err(MetamorphicError::EmptyFamily)
    ));
}

#[test]
fn malformed_persisted_families_are_rebuilt_through_trial_admission() {
    let duplicate_trials = json!({
        "id": "rename",
        "relation": "invariant",
        "trials": [
            {
                "id": "t1",
                "relation": "invariant",
                "response": "unchanged"
            },
            {
                "id": "t1",
                "relation": "invariant",
                "response": "unchanged"
            }
        ]
    });
    let wrong_relation = json!({
        "id": "effect",
        "relation": "invariant",
        "trials": [{
            "id": "t1",
            "relation": {
                "directional_change": {
                    "expected": "increase"
                }
            },
            "response": "unchanged"
        }]
    });

    let duplicate_result: Result<Family, _> = serde_json::from_value(duplicate_trials);
    let wrong_relation_result: Result<Family, _> = serde_json::from_value(wrong_relation);
    assert!(duplicate_result.is_err());
    assert!(wrong_relation_result.is_err());
}

#[test]
fn suites_reject_duplicate_family_identity() {
    let family = Family::declaring("rename", Relation::Invariant);
    let mut suite = Suite::new();
    suite.add(family.clone()).expect("first family");

    assert!(matches!(
        suite.add(family),
        Err(MetamorphicError::DuplicateFamily(id)) if id == "rename"
    ));

    let encoded = serde_json::json!({
        "families": [
            {
                "id": "rename",
                "relation": "invariant",
                "trials": [{
                    "id": "t1",
                    "relation": "invariant",
                    "response": "unchanged"
                }]
            },
            {
                "id": "rename",
                "relation": "invariant",
                "trials": [{
                    "id": "t2",
                    "relation": "invariant",
                    "response": "unchanged"
                }]
            }
        ]
    });
    let parsed: Result<Suite, _> = serde_json::from_value(encoded);
    assert!(parsed.is_err());
}

#[test]
fn malformed_family_identity_is_refused_on_admission() {
    let mut family = Family::declaring("rename\n", Relation::Invariant);
    let result = family.record(trial("t1", Relation::Invariant, Response::Unchanged));

    assert!(matches!(
        result,
        Err(MetamorphicError::InvalidFamily { .. })
    ));
}

#[test]
fn a_commitment_cannot_be_added_after_the_seal() {
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("first commitment");
    let sealed = registration
        .seal(
            &json!({"rule": "sign agreement"}),
            at("2026-06-01T00:00:00Z"),
        )
        .expect("something was committed");

    assert!(matches!(
        sealed.commit(Commitment::new("gene-y", json!({}), "plan-a")),
        Err(RevealError::AlreadySealed)
    ));
}

#[test]
fn a_rubric_edited_between_seal_and_score_cannot_be_used() {
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("first commitment");
    let sealed = registration
        .seal(
            &json!({"rule": "sign agreement"}),
            at("2026-06-01T00:00:00Z"),
        )
        .expect("something was committed");
    let revealed = sealed
        .reveal(vec![Outcome::new("gene-x", json!({"effect": "up"}))])
        .expect("valid reveal");

    match revealed.score_under(&json!({"rule": "sign agreement", "bonus": "partial"})) {
        Err(RevealError::RubricChanged { sealed, presented }) => {
            assert_ne!(sealed, presented);
        }
        other => panic!("expected a rubric refusal, got {other:?}"),
    }
}

#[test]
fn a_reformatted_but_identical_rubric_still_scores() {
    let rubric = json!({"b": 2, "a": 1});
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("first commitment");
    let sealed = registration
        .seal(&rubric, at("2026-06-01T00:00:00Z"))
        .expect("something was committed");
    let revealed = sealed
        .reveal(vec![Outcome::new("gene-x", json!({"effect": "up"}))])
        .expect("valid reveal");

    let scoring = revealed
        .score_under(&json!({"a": 1, "b": 2}))
        .expect("canonical bytes are key order independent");

    assert_eq!(scoring.scored.len(), 1);
    assert!(scoring.complete());
}

#[test]
fn an_outcome_for_something_nobody_committed_cannot_be_scored() {
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("first commitment");
    let rubric = json!({"rule": "sign agreement"});
    let sealed = registration
        .seal(&rubric, at("2026-06-01T00:00:00Z"))
        .expect("something was committed");
    let revealed = sealed
        .reveal(vec![Outcome::new("gene-z", json!({"effect": "up"}))])
        .expect("valid reveal");

    assert!(matches!(
        revealed.score_under(&rubric),
        Err(RevealError::UncommittedOutcome(_))
    ));
}

#[test]
fn commitments_that_never_got_an_outcome_are_listed_rather_than_dropped() {
    let mut registration = Registration::open("prospective-2026-q3");
    for target in ["gene-x", "gene-y"] {
        registration
            .commit(Commitment::new(target, json!({"effect": "up"}), "plan-a"))
            .expect("distinct targets");
    }
    let rubric = json!({"rule": "sign agreement"});
    let sealed = registration
        .seal(&rubric, at("2026-06-01T00:00:00Z"))
        .expect("something was committed");
    let revealed = sealed
        .reveal(vec![Outcome::new("gene-x", json!({"effect": "up"}))])
        .expect("valid reveal");

    let scoring = revealed.score_under(&rubric).expect("rubric is unchanged");

    assert!(!scoring.complete());
    assert_eq!(scoring.unrevealed, vec!["gene-y".to_string()]);
}

#[test]
fn sealing_with_nothing_committed_refuses() {
    let registration = Registration::open("empty");

    assert!(matches!(
        registration.seal(&json!({}), at("2026-06-01T00:00:00Z")),
        Err(RevealError::NothingCommitted)
    ));
}

#[test]
fn commitment_and_outcome_records_are_validated_at_state_boundaries() {
    let mut invalid_study = Registration::open("study\n");
    assert!(matches!(
        invalid_study.commit(Commitment::new("gene-x", json!({}), "plan-a")),
        Err(RevealError::InvalidStudy(_))
    ));

    let mut registration = Registration::open("prospective-2026-q3");
    assert!(matches!(
        registration.commit(Commitment::new(" ", json!({}), "plan-a")),
        Err(RevealError::InvalidCommitment { .. })
    ));
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("valid commitment");
    let sealed = registration
        .seal(&json!({"rule": "sign agreement"}), at("2026-06-01T00:00:00Z"))
        .expect("sealed");

    let duplicate = sealed.reveal(vec![
        Outcome::new("gene-x", json!({"effect": "up"})),
        Outcome::new("gene-x", json!({"effect": "up"})),
    ]);
    assert!(matches!(
        duplicate,
        Err(RevealError::DuplicateOutcome(target)) if target == "gene-x"
    ));
}

#[test]
fn persisted_open_registrations_are_rebuilt_through_commit_admission() {
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("valid commitment");
    let mut encoded = serde_json::to_value(&registration).expect("registration serializes");
    encoded["commitments"]["gene-x"]["target"] = serde_json::json!("gene-y");

    let parsed: Result<Registration, _> = serde_json::from_value(encoded);
    assert!(parsed.is_err());
}

#[test]
fn invalid_outcome_identity_is_refused_before_scoring() {
    let mut registration = Registration::open("prospective-2026-q3");
    registration
        .commit(Commitment::new("gene-x", json!({"effect": "up"}), "plan-a"))
        .expect("valid commitment");
    let sealed = registration
        .seal(&json!({"rule": "sign agreement"}), at("2026-06-01T00:00:00Z"))
        .expect("sealed");

    assert!(matches!(
        sealed.reveal(vec![Outcome::new("gene\n", json!({}))]),
        Err(RevealError::InvalidOutcome { .. })
    ));
}

fn levels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn design_of_two_factors() -> FactorialDesign {
    let mut design = FactorialDesign::declare(
        "cell-7",
        ["planner".to_string(), "verifier".to_string()],
        "base",
    );
    for (id, planner, verifier, conclusion) in [
        ("base", "react", "off", Conclusion::Fail),
        ("p1", "tree", "off", Conclusion::Pass),
        ("v1", "react", "on", Conclusion::Pass),
        ("both", "tree", "on", Conclusion::Pass),
    ] {
        design
            .add(Arm::new(
                id,
                levels(&[("planner", planner), ("verifier", verifier)]),
                conclusion,
                ScoreTier::Execution,
            ))
            .expect("distinct arms");
    }
    design
}

#[test]
fn an_arm_that_leaves_a_declared_factor_unassigned_is_refused() {
    let mut design = FactorialDesign::declare(
        "cell-7",
        ["planner".to_string(), "verifier".to_string()],
        "base",
    );

    let outcome = design.add(Arm::new(
        "partial",
        levels(&[("planner", "react")]),
        Conclusion::Pass,
        ScoreTier::Execution,
    ));

    assert!(matches!(outcome, Err(DesignError::UnassignedFactor { .. })));
}

#[test]
fn only_pairs_differing_in_one_factor_become_contrasts() {
    let design = design_of_two_factors();

    let contrasts = design.single_factor_contrasts();

    assert_eq!(contrasts.len(), 4, "base-vs-both differs in two factors");
    assert!(contrasts
        .iter()
        .all(|c| !(c.baseline == "base" && c.variant == "both")));
}

#[test]
fn an_arm_differing_from_the_baseline_in_two_factors_is_reported_as_unattributable() {
    let design = design_of_two_factors();

    assert_eq!(design.unattributable(), vec!["both"]);
}

#[test]
fn an_interaction_names_the_cells_a_design_would_need_to_estimate_it() {
    let mut design = FactorialDesign::declare(
        "cell-7",
        ["planner".to_string(), "verifier".to_string()],
        "base",
    );
    for (id, planner, verifier) in [
        ("base", "react", "off"),
        ("p1", "tree", "off"),
        ("v1", "react", "on"),
    ] {
        design
            .add(Arm::new(
                id,
                levels(&[("planner", planner), ("verifier", verifier)]),
                Conclusion::Pass,
                ScoreTier::Execution,
            ))
            .expect("distinct arms");
    }

    assert!(design.estimable_interactions().is_empty());
    assert_eq!(
        design.missing_for_interaction("planner", "verifier"),
        vec![("tree".to_string(), "on".to_string())]
    );

    let full = design_of_two_factors();
    assert_eq!(
        full.estimable_interactions(),
        vec![("planner".to_string(), "verifier".to_string())]
    );
    assert!(full
        .missing_for_interaction("planner", "verifier")
        .is_empty());
}

#[test]
fn a_contrast_fork_carries_the_other_factors_as_held_fixed_and_attributes() {
    let design = design_of_two_factors();

    let forks = design.contrast_forks(true).expect("design is valid");
    let planner_fork = forks
        .iter()
        .find(|f| f.baseline.arm == "base" && f.variant.arm == "p1")
        .expect("base and p1 differ in planner only");

    assert!(planner_fork.held_fixed.contains("verifier"));
    assert!(!planner_fork.held_fixed.contains("planner"));
    assert!(matches!(
        attribute(planner_fork),
        Attribution::Attributed { .. }
    ));
}

#[test]
fn two_arms_occupying_the_same_cell_are_refused() {
    let mut design = FactorialDesign::declare("cell-7", ["planner".to_string()], "base");
    design
        .add(Arm::new(
            "base",
            levels(&[("planner", "react")]),
            Conclusion::Pass,
            ScoreTier::Execution,
        ))
        .expect("first arm");

    let outcome = design.add(Arm::new(
        "twin",
        levels(&[("planner", "react")]),
        Conclusion::Pass,
        ScoreTier::Execution,
    ));

    assert!(matches!(outcome, Err(DesignError::DuplicateCell { .. })));
}

#[test]
fn persisted_designs_are_rebuilt_through_complete_arm_admission() {
    let design = design_of_two_factors();
    let mut encoded = serde_json::to_value(&design).expect("design serializes");
    encoded["arms"][1]["levels"]
        .as_object_mut()
        .expect("levels object")
        .remove("verifier");
    let parsed: Result<FactorialDesign, _> = serde_json::from_value(encoded);

    assert!(parsed.is_err());
}

#[test]
fn a_persisted_design_cannot_move_its_baseline_after_observing_arms() {
    let design = design_of_two_factors();
    let mut encoded = serde_json::to_value(&design).expect("design serializes");
    encoded["baseline"] = serde_json::json!("post-hoc-favourite");
    let parsed: Result<FactorialDesign, _> = serde_json::from_value(encoded);

    assert!(parsed.is_err());
}

#[test]
fn malformed_arm_identity_is_refused_before_it_can_form_a_contrast() {
    let mut design = FactorialDesign::declare(
        "cell-7",
        ["planner".to_string()],
        "base",
    );
    let result = design.add(Arm::new(
        "base",
        levels(&[("planner", " ")]),
        Conclusion::Pass,
        ScoreTier::Execution,
    ));

    assert!(matches!(result, Err(DesignError::InvalidArm { .. })));
}

#[test]
fn a_mutated_design_identity_is_rechecked_before_fork_generation() {
    let mut design = design_of_two_factors();
    design.cell_id = "cell-7\n".into();

    assert!(matches!(
        design.contrast_forks(true),
        Err(DesignError::InvalidDesign { .. })
    ));
}
